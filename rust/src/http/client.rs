use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use url::Url;

use crate::cli::CliError;
use crate::core::patch::Repository;
use crate::core::validation::validate_repository;

/// Default timeout for HTTP client socket connect, read, and write operations.
pub const DEFAULT_CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Configuration options for the HTTP snapshot client (§7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientConfig {
    /// Connection connect, read, and write timeout (default: 10s).
    pub timeout: Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_CLIENT_TIMEOUT,
        }
    }
}

/// Synchronously fetch and strictly validate a remote repository over HTTP using default configuration.
pub fn fetch_repository(url_str: &str) -> Result<Repository, CliError> {
    fetch_repository_with_config(url_str, &HttpClientConfig::default())
}

/// Synchronously fetch and strictly validate a remote repository over HTTP with custom configuration.
pub fn fetch_repository_with_config(
    url_str: &str,
    config: &HttpClientConfig,
) -> Result<Repository, CliError> {
    let parsed_url = Url::parse(url_str)
        .map_err(|e| CliError::Custom(format!("invalid HTTP repository URL: {e}")))?;

    if parsed_url.scheme() == "https" {
        return Err(CliError::Custom("https is not supported".into()));
    }
    if parsed_url.scheme() != "http" {
        return Err(CliError::Custom(format!(
            "invalid HTTP repository URL: expected http scheme, got {}",
            parsed_url.scheme()
        )));
    }

    let host = parsed_url
        .host_str()
        .ok_or_else(|| CliError::Custom("missing host in URL".into()))?;
    let port = parsed_url.port_or_known_default().unwrap_or(80);

    let mut path_and_query = parsed_url.path().to_string();
    if let Some(query) = parsed_url.query() {
        path_and_query.push('?');
        path_and_query.push_str(query);
    }

    let authority = match parsed_url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    };

    // Resolve socket address and connect
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| CliError::Custom(format!("failed to resolve {host}:{port}: {e}")))?;

    let mut stream = None;
    for addr in addrs {
        if let Ok(s) = TcpStream::connect_timeout(&addr, config.timeout) {
            stream = Some(s);
            break;
        }
    }
    let mut stream =
        stream.ok_or_else(|| CliError::Custom(format!("failed to connect to {host}:{port}")))?;

    stream
        .set_read_timeout(Some(config.timeout))
        .map_err(CliError::Io)?;
    stream
        .set_write_timeout(Some(config.timeout))
        .map_err(CliError::Io)?;

    // Send HTTP GET request
    let request =
        format!("GET {path_and_query} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(CliError::Io)?;
    stream.flush().map_err(CliError::Io)?;

    // Read full response
    let mut response_buf = Vec::new();
    stream
        .read_to_end(&mut response_buf)
        .map_err(CliError::Io)?;

    // Parse HTTP response
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut resp = httparse::Response::new(&mut headers);
    let body_offset = match resp.parse(&response_buf) {
        Ok(httparse::Status::Complete(offset)) => offset,
        Ok(httparse::Status::Partial) => {
            return Err(CliError::Custom("incomplete HTTP response".into()));
        }
        Err(e) => return Err(CliError::Custom(format!("invalid HTTP response: {e}"))),
    };

    let status = resp.code.unwrap_or(0);
    if status != 200 {
        return Err(CliError::Custom(format!("HTTP {status}")));
    }

    // Check for chunked transfer encoding and content length (RFC 7230 §3.3.3)
    let mut is_chunked = false;
    let mut content_length: Option<usize> = None;
    for h in resp.headers {
        if h.name.eq_ignore_ascii_case("transfer-encoding") {
            if let Ok(val) = std::str::from_utf8(h.value) {
                if val.trim().eq_ignore_ascii_case("chunked") {
                    is_chunked = true;
                }
            }
        } else if h.name.eq_ignore_ascii_case("content-length") {
            let val = std::str::from_utf8(h.value)
                .map_err(|_| CliError::Custom("invalid Content-Length header".into()))?;
            let len = val
                .trim()
                .parse::<usize>()
                .map_err(|_| CliError::Custom("invalid Content-Length header".into()))?;
            if let Some(prev) = content_length {
                if prev != len {
                    return Err(CliError::Custom(
                        "conflicting Content-Length headers".into(),
                    ));
                }
            }
            content_length = Some(len);
        }
    }

    let raw_body = &response_buf[body_offset..];
    let body_bytes = if is_chunked {
        decode_chunked(raw_body)?
    } else if let Some(expected_len) = content_length {
        // RFC 7230 §3.3.3: If fewer bytes than Content-Length received, message is truncated.
        if raw_body.len() < expected_len {
            return Err(CliError::Custom(format!(
                "truncated HTTP response: expected {expected_len} bytes, received {}",
                raw_body.len()
            )));
        }
        raw_body[..expected_len].to_vec()
    } else {
        raw_body.to_vec()
    };

    let repo = Repository::from_json_slice(&body_bytes)?;
    validate_repository(&repo)?;
    Ok(repo)
}

fn decode_chunked(input: &[u8]) -> Result<Vec<u8>, CliError> {
    let mut out = Vec::new();
    let mut cursor = 0;
    let mut terminated = false;
    while cursor < input.len() {
        let crlf = input[cursor..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| CliError::Custom("invalid chunked transfer encoding".into()))?;
        let len_slice = &input[cursor..cursor + crlf];
        let len_str = std::str::from_utf8(len_slice)
            .map_err(|_| CliError::Custom("invalid chunk length encoding".into()))?;
        let hex_part = len_str.split(';').next().unwrap_or("");
        let chunk_len = usize::from_str_radix(hex_part.trim(), 16)
            .map_err(|_| CliError::Custom("invalid chunk length hex".into()))?;
        cursor += crlf + 2;
        if chunk_len == 0 {
            terminated = true;
            break;
        }
        if cursor + chunk_len > input.len() {
            return Err(CliError::Custom("incomplete chunk data".into()));
        }
        out.extend_from_slice(&input[cursor..cursor + chunk_len]);
        cursor += chunk_len;
        if cursor + 2 > input.len() || &input[cursor..cursor + 2] != b"\r\n" {
            return Err(CliError::Custom("missing CRLF after chunk data".into()));
        }
        cursor += 2;
    }
    if !terminated {
        return Err(CliError::Custom(
            "truncated chunked stream: missing terminating chunk".into(),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    #[test]
    fn test_fetch_repository_success_and_error() {
        let listener = TcpListener::bind((crate::http::server::SERVER_HOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            // First request: 200 OK with valid repository
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = r#"{"format":1,"frontier":[],"patches":[]}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }

            // Second request: 302 Redirect
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 302 Found\r\nLocation: /other\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        // Test 1: Fetch valid
        let url = format!(
            "http://{}:{port}{}",
            crate::http::server::SERVER_HOST,
            crate::http::server::REPOSITORY_ENDPOINT
        );
        let repo = fetch_repository(&url).unwrap();
        assert_eq!(repo.format, 1);

        // Test 2: Status 302 fails
        let err = fetch_repository(&url).unwrap_err();
        assert!(format!("{err}").contains("HTTP 302"));
    }

    #[test]
    fn test_regression_bug_003_http_chunked_missing_crlf_rejected() {
        // Valid chunked payload with CRLF
        let valid = b"4\r\ntest\r\n0\r\n\r\n";
        assert_eq!(decode_chunked(valid).unwrap(), b"test");

        // Missing CRLF after chunk data
        let missing_crlf = b"4\r\ntest0\r\n\r\n";
        let err = decode_chunked(missing_crlf).unwrap_err();
        assert!(format!("{err}").contains("missing CRLF after chunk data"));

        // Truncated chunked stream without terminating chunk
        let truncated = b"4\r\ntest\r\n";
        let err = decode_chunked(truncated).unwrap_err();
        assert!(format!("{err}").contains("missing terminating chunk"));
    }

    #[test]
    fn test_regression_bug_004_http_chunked_parses_chunk_extensions() {
        // Chunk headers with extensions (both data chunk and last chunk)
        let chunked_with_ext = b"4;ext=dummy;foo=bar\r\ntest\r\n0;final=true\r\n\r\n";
        assert_eq!(decode_chunked(chunked_with_ext).unwrap(), b"test");
    }

    #[test]
    fn test_regression_bug_007_content_length_truncation_and_framing() {
        let listener = TcpListener::bind((crate::http::server::SERVER_HOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        std::thread::spawn(move || {
            // Request 1: Server promises Content-Length 500, but only sends 30 bytes before closing
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 500\r\nConnection: close\r\n\r\n{\"version\":\"()\",\"patches\":[]}";
                let _ = stream.write_all(resp.as_bytes());
            }

            // Request 2: Server sends extra trailing junk after Content-Length bytes
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = r#"{"format":1,"frontier":[],"patches":[]}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}trailingjunkignored",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let url = format!(
            "http://{}:{port}{}",
            crate::http::server::SERVER_HOST,
            crate::http::server::REPOSITORY_ENDPOINT
        );

        // Test 1: Premature truncation detected as error
        let err = fetch_repository(&url).unwrap_err();
        assert!(format!("{err}").contains("truncated HTTP response"));

        // Test 2: Trailing bytes ignored per Content-Length framing
        let repo = fetch_repository(&url).unwrap();
        assert_eq!(repo.format, 1);
    }
}
