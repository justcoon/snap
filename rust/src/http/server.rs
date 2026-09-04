use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::cli::CliError;
use crate::core::patch::Repository;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() -> Result<(), CliError> {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handle_signal as *const () as usize;
        sa.sa_flags = 0; // No SA_RESTART so blocking system calls return EINTR
        libc::sigemptyset(&mut sa.sa_mask);

        if libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut()) != 0 {
            return Err(CliError::Custom("failed to install SIGINT handler".into()));
        }
        if libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut()) != 0 {
            return Err(CliError::Custom("failed to install SIGTERM handler".into()));
        }
    }
    Ok(())
}

/// Serve a frozen in-memory repository snapshot over HTTP on 127.0.0.1.
pub fn serve_repository(repo: &Repository, port: Option<u16>) -> Result<(), CliError> {
    // 1. Snapshot the repository into memory as formatted JSON
    let snapshot_json = repo.to_json_pretty()?;
    let snapshot_bytes = snapshot_json.into_bytes();

    // 2. Bind to 127.0.0.1 on specified or default port
    let bind_port = port.unwrap_or(8765);
    let listener = TcpListener::bind(("127.0.0.1", bind_port))
        .map_err(|e| CliError::Custom(format!("failed to bind to 127.0.0.1:{bind_port}: {e}")))?;

    // 3. Resolve bound address and announce startup URL on stdout
    let local_addr = listener
        .local_addr()
        .map_err(|e| CliError::Custom(format!("failed to get listener address: {e}")))?;
    let actual_port = local_addr.port();

    println!("http://127.0.0.1:{actual_port}/repository.json");
    std::io::stdout().flush().map_err(CliError::Io)?;

    // 4. Register signal handlers for graceful SIGINT and SIGTERM termination
    install_signal_handlers()?;
    SHUTDOWN.store(false, Ordering::SeqCst);

    let raw_fd = listener.as_raw_fd();

    // 5. Accept loop with polling
    while !SHUTDOWN.load(Ordering::SeqCst) {
        let mut pfd = libc::pollfd {
            fd: raw_fd,
            events: libc::POLLIN,
            revents: 0,
        };

        // Poll with 100ms timeout to periodically re-check SHUTDOWN flag
        let poll_res = unsafe { libc::poll(&mut pfd, 1, 100) };
        if poll_res < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                if SHUTDOWN.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            }
            return Err(CliError::Io(err));
        }

        if poll_res == 0 {
            // Timeout expired; re-check SHUTDOWN
            continue;
        }

        if (pfd.revents & libc::POLLIN) != 0 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = handle_connection(&mut stream, &snapshot_bytes);
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::Interrupted =>
                {
                    continue;
                }
                Err(e) => return Err(CliError::Io(e)),
            }
        }
    }

    Ok(())
}

fn handle_connection(stream: &mut TcpStream, snapshot_bytes: &[u8]) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut buf = [0u8; 8192];
    let mut total_read = 0;

    loop {
        match stream.read(&mut buf[total_read..]) {
            Ok(0) => break,
            Ok(n) => {
                total_read += n;
                let mut headers = [httparse::EMPTY_HEADER; 64];
                let mut req = httparse::Request::new(&mut headers);
                match req.parse(&buf[..total_read]) {
                    Ok(httparse::Status::Complete(_)) => {
                        let method = req.method.unwrap_or("");
                        let path = req.path.unwrap_or("");
                        send_response(stream, method, path, snapshot_bytes)?;
                        return Ok(());
                    }
                    Ok(httparse::Status::Partial) => {
                        if total_read >= buf.len() {
                            let resp = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                            stream.write_all(resp.as_bytes())?;
                            stream.flush()?;
                            return Ok(());
                        }
                        continue;
                    }
                    Err(_) => {
                        let resp = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
                        stream.write_all(resp.as_bytes())?;
                        stream.flush()?;
                        return Ok(());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Ok(()),
        }
    }

    Ok(())
}

fn send_response(
    stream: &mut TcpStream,
    method: &str,
    path: &str,
    snapshot_bytes: &[u8],
) -> std::io::Result<()> {
    if path != "/repository.json" {
        // Any other path returns 404
        let resp = "HTTP/1.1 404 Not Found\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(resp.as_bytes())?;
        stream.flush()?;
        return Ok(());
    }

    match method {
        "GET" => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                snapshot_bytes.len()
            );
            stream.write_all(header.as_bytes())?;
            stream.write_all(snapshot_bytes)?;
            stream.flush()?;
        }
        "HEAD" => {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                snapshot_bytes.len()
            );
            stream.write_all(header.as_bytes())?;
            stream.flush()?;
        }
        _ => {
            // Other methods return 405 with Allow: GET, HEAD
            let header = "HTTP/1.1 405 Method Not Allowed\r\nAllow: GET, HEAD\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(header.as_bytes())?;
            stream.flush()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patch::Repository;
    use crate::core::version::Version;

    #[test]
    fn test_server_endpoint_routing() {
        let repo = Repository::new(Version::empty(), Vec::new());
        let snapshot_json = repo.to_json_pretty().unwrap();
        let snapshot_bytes = snapshot_json.into_bytes();

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn a thread to serve 4 requests
        std::thread::spawn(move || {
            for _ in 0..4 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = handle_connection(&mut stream, &snapshot_bytes);
                }
            }
        });

        // 1. GET /repository.json
        {
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            client
                .write_all(b"GET /repository.json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut resp = Vec::new();
            client.read_to_end(&mut resp).unwrap();
            let resp_str = String::from_utf8_lossy(&resp);
            assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
            assert!(resp_str.contains("Content-Type: application/json; charset=utf-8"));
            assert!(resp_str.contains("\"format\": 1"));
        }

        // 2. HEAD /repository.json
        {
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            client
                .write_all(b"HEAD /repository.json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut resp = Vec::new();
            client.read_to_end(&mut resp).unwrap();
            let resp_str = String::from_utf8_lossy(&resp);
            assert!(resp_str.starts_with("HTTP/1.1 200 OK\r\n"));
            assert!(resp_str.contains("Content-Type: application/json; charset=utf-8"));
            assert!(resp_str.ends_with("\r\n\r\n"));
        }

        // 3. POST /repository.json -> 405
        {
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            client
                .write_all(b"POST /repository.json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut resp = Vec::new();
            client.read_to_end(&mut resp).unwrap();
            let resp_str = String::from_utf8_lossy(&resp);
            assert!(resp_str.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
            assert!(resp_str.contains("Allow: GET, HEAD"));
        }

        // 4. GET /invalid -> 404
        {
            let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
            client
                .write_all(b"GET /invalid HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .unwrap();
            let mut resp = Vec::new();
            client.read_to_end(&mut resp).unwrap();
            let resp_str = String::from_utf8_lossy(&resp);
            assert!(resp_str.starts_with("HTTP/1.1 404 Not Found\r\n"));
        }
    }
}
