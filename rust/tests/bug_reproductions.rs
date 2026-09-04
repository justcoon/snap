use base64::prelude::*;
use std::io::{Read, Write};

use snap::core::patch::{Change, Patch, Repository};
use snap::core::validation::validate_repository;
use snap::core::version::{ContributorId, Version};
use snap::http::client::fetch_repository;

/// SPEC §4.2:
/// "`message` is a nonempty UTF-8 string. It may contain tab and LF but no other
/// ASCII control character."
///
/// BUG-001: `validate_repository` does not validate patch messages or changes,
/// allowing repositories with patches containing disallowed ASCII control characters
/// (e.g. \x01, \r, \x07) to pass repository validation.
#[test]
#[ignore = "Resolved in BUG-001 (see docs/bugs/resolution_BUG-001_walkthrough.md)"]
fn test_bug_001_validate_repository_allows_control_chars_in_patch_message() {
    let author = ContributorId::parse("alice@example.com").unwrap();
    let version = Version::parse("(alice@example.com->1)").unwrap();
    let patch = Patch {
        author: author.clone(),
        revision: 1,
        base: Version::empty(),
        message: "illegal\x01control\x07char".to_string(),
        changes: vec![Change::Put {
            path: "hello.txt".to_string(),
            content: BASE64_STANDARD.encode(b"world"),
        }],
    };
    let repo = Repository::new(version, vec![patch]);

    // SPEC §4.2: message cannot contain ASCII control characters other than \t and \n.
    // Therefore, validate_repository MUST reject this repository.
    let res = validate_repository(&repo);
    assert!(
        res.is_err(),
        "Expected validate_repository to reject patch with control characters in message, but got Ok(())"
    );
}

/// SPEC §4.2:
/// "`changes` is nonempty, sorted by path, and contains at most one change per path."
///
/// BUG-002: `validate_repository` does not check if changes within a patch are sorted
/// or if duplicate paths exist. A patch containing duplicate changes for the same path
/// is accepted by `validate_repository`.
#[test]
#[ignore = "Resolved in BUG-002 (see docs/bugs/resolution_BUG-002_walkthrough.md)"]
fn test_bug_002_validate_repository_accepts_duplicate_change_paths() {
    let author = ContributorId::parse("alice@example.com").unwrap();
    let version = Version::parse("(alice@example.com->1)").unwrap();
    let patch = Patch {
        author: author.clone(),
        revision: 1,
        base: Version::empty(),
        message: "add file twice".to_string(),
        // Duplicate change for the same path "notes.txt"
        changes: vec![
            Change::Put {
                path: "notes.txt".to_string(),
                content: BASE64_STANDARD.encode(b"first"),
            },
            Change::Put {
                path: "notes.txt".to_string(),
                content: BASE64_STANDARD.encode(b"second"),
            },
        ],
    };
    let repo = Repository::new(version, vec![patch]);

    // SPEC §4.2 explicitly requires at most one change per path in changes.
    let res = validate_repository(&repo);
    assert!(
        res.is_err(),
        "Expected validate_repository to reject patch with duplicate change paths, but got Ok(())"
    );
}

/// RFC 7230 §4.1 / SPEC §7.1, §7.8:
/// `chunk = chunk-size [ chunk-ext ] CRLF chunk-data CRLF`
/// Every chunk data must be followed by CRLF.
///
/// BUG-003: HTTP client `decode_chunked` accepts malformed chunked transfer encoding
/// where chunk data is not followed by CRLF (or where the chunked stream ends
/// prematurely without a terminating 0\r\n\r\n chunk).
#[test]
#[ignore = "Resolved in BUG-003 (see docs/bugs/resolution_BUG-003_walkthrough.md)"]
fn test_bug_003_http_chunked_missing_crlf_should_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let valid_repo = Repository::new(Version::empty(), vec![]);
    let valid_json = valid_repo.to_json_pretty().unwrap();

    let server_handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            // Malformed chunked payload: valid repo JSON chunk data is directly followed by '0\r\n\r\n',
            // missing the mandatory \r\n after the chunk data.
            // RFC 7230 §4.1 strictly requires CRLF after each chunk-data.
            let chunk_hex = format!("{:x}", valid_json.len());
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            )
            .into_bytes();
            response.extend_from_slice(format!("{chunk_hex}\r\n").as_bytes());
            response.extend_from_slice(valid_json.as_bytes());
            // Intentionally omit \r\n here! Immediately append '0\r\n\r\n'
            response.extend_from_slice(b"0\r\n\r\n");

            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });

    let url = format!("http://127.0.0.1:{port}/repository.json");
    let res = fetch_repository(&url);
    server_handle.join().unwrap();

    assert!(
        res.is_err(),
        "Expected fetch_repository to reject chunk missing trailing CRLF, but got Ok"
    );
}

/// RFC 7230 §4.1:
/// `chunk = chunk-size [ chunk-ext ] CRLF chunk-data CRLF`
/// Chunk size line may contain chunk extensions (e.g. `;name=val`).
///
/// BUG-004: HTTP client `decode_chunked` fails to parse chunk headers that contain
/// RFC-compliant chunk extensions, failing with "invalid chunk length hex".
#[test]
#[ignore = "Resolved in BUG-004 (see docs/bugs/resolution_BUG-004_walkthrough.md)"]
fn test_bug_004_http_chunked_fails_on_valid_chunk_extensions() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let valid_repo = Repository::new(Version::empty(), vec![]);
    let valid_json = valid_repo.to_json_pretty().unwrap();

    let server_handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);

            // Send chunk with chunk extension: e.g. <hex>;ext=foo\r\n<data>\r\n0\r\n\r\n
            let chunk_hex = format!("{:x}", valid_json.len());
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            )
            .into_bytes();
            response.extend_from_slice(format!("{chunk_hex};extension=dummy\r\n").as_bytes());
            response.extend_from_slice(valid_json.as_bytes());
            response.extend_from_slice(b"\r\n0\r\n\r\n");

            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });

    let url = format!("http://127.0.0.1:{port}/repository.json");
    let res = fetch_repository(&url);
    server_handle.join().unwrap();

    assert!(
        res.is_ok(),
        "Expected fetch_repository to succeed with valid chunk extensions, but failed with: {:?}",
        res.err()
    );
}
