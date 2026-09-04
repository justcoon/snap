#![allow(clippy::unwrap_used, clippy::expect_used)]

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

struct TestTempDir(std::path::PathBuf);

impl TestTempDir {
    fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("{name}_{}_{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        TestTempDir(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// SPEC §7.4:
/// "`snap log` prints patches in reverse canonical integration order, one tab-separated line each"
///
/// BUG-005: `cmd_log` traverses `repo.patches.iter().rev()`. Because `repo.patches` is stored
/// sorted by author then revision (§4.1), traversing in reverse author order fails to follow
/// reverse canonical integration order (§6.1). When Alice's patch depends on Bob's patch,
/// Bob's patch is printed before Alice's patch, inverting causal history in the log!
#[test]
#[ignore = "Resolved in BUG-005 (see docs/bugs/resolution_BUG-005_walkthrough.md)"]
fn test_bug_005_log_reverse_canonical_integration_order() {
    let temp = TestTempDir::new("snap_test_bug_005");
    let dot_snap = temp.path().join(".snap");
    std::fs::create_dir_all(&dot_snap).unwrap();

    let author_alice = ContributorId::parse("alice@example.com").unwrap();
    let author_bob = ContributorId::parse("bob@example.com").unwrap();

    let patch_bob = Patch {
        author: author_bob,
        revision: 1,
        base: Version::empty(),
        message: "root commit by bob".to_string(),
        changes: vec![Change::Put {
            path: "file.txt".to_string(),
            content: BASE64_STANDARD.encode(b"bob content"),
        }],
    };

    let patch_alice = Patch {
        author: author_alice,
        revision: 1,
        base: Version::parse("(bob@example.com->1)").unwrap(),
        message: "child commit by alice".to_string(),
        changes: vec![Change::Put {
            path: "file.txt".to_string(),
            content: BASE64_STANDARD.encode(b"alice content"),
        }],
    };

    let version = Version::parse("(alice@example.com->1,bob@example.com->1)").unwrap();
    // In repo.patches, patches are sorted by author: Alice (index 0), Bob (index 1)
    let repo = Repository::new(version, vec![patch_alice, patch_bob]);
    validate_repository(&repo).expect("Repository must be valid");

    std::fs::write(
        dot_snap.join("repository.json"),
        repo.to_json_pretty().unwrap(),
    )
    .unwrap();

    let snap_bin = env!("CARGO_BIN_EXE_snap");
    let output = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .arg("log")
        .output()
        .expect("snap log must execute");

    assert!(
        output.status.success(),
        "snap log failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    assert!(
        lines.len() >= 2,
        "Expected at least 2 log lines, got stdout:\n{stdout}"
    );

    // Canonical integration order starts from empty base, integrating Bob first, then Alice second.
    // Reverse canonical integration order MUST list Alice first, then Bob second.
    assert!(
        lines[0].starts_with("(alice@example.com->1"),
        "SPEC §7.4 requires reverse canonical integration order: child commit Alice must appear first, but line 0 was: {}",
        lines[0]
    );
}

/// SPEC §4.4, §7.7:
/// "`snap revert <version>` reverts the tree to a previously recorded version without rewriting history.
/// Creates a new patch whose changes transform the current tree into the target tree...
/// An empty script is valid only when creating an empty text file."
///
/// BUG-006: In `cmd_revert`, when restoring an empty text file from absent (`(None, Some(new_bytes))` where
/// `new_bytes` is `b""`), it constructs `Change::Text { path, edit: vec![TextEditOp::Insert(vec![])] }`.
/// `validate_repository` rejects empty insert operations (`insert operation cannot be empty`), causing
/// `snap revert` to fail with validation errors when reverting back to an empty file.
#[test]
fn test_bug_006_revert_empty_text_file_from_absent() {
    let temp = TestTempDir::new("snap_test_bug_006");
    let snap_bin = env!("CARGO_BIN_EXE_snap");

    // Initialize repository
    let out = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .arg("init")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "snap init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Configure contributor
    let out = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .args(["config", "contributor.id", "tester@example.com"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "snap config failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Create an empty text file "empty.txt"
    std::fs::write(temp.path().join("empty.txt"), b"").unwrap();

    // Commit empty file -> creates (tester@example.com->1)
    let out = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .args(["commit", "create empty file"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "Failed to commit empty file: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Delete empty file
    std::fs::remove_file(temp.path().join("empty.txt")).unwrap();

    // Commit deletion -> creates (tester@example.com->2)
    let out = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .args(["commit", "delete empty file"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "Failed to commit delete: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Now revert back to (tester@example.com->1) to restore the empty file
    let out = std::process::Command::new(snap_bin)
        .current_dir(temp.path())
        .args(["revert", "(tester@example.com->1)"])
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "Expected snap revert to succeed when restoring an empty text file, but failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// RFC 7230 §3.3.3 / SPEC §7.1, §7.8:
/// "If a message is received with a Content-Length header field and a message-body is received
/// of length less than the number of octets indicated by the Content-Length, the message has
/// been truncated and MUST be treated as an error."
///
/// BUG-007: HTTP client `fetch_repository` completely ignores `Content-Length`. If the remote
/// server promises N bytes via `Content-Length` but closes the connection prematurely after sending
/// fewer bytes (e.g. truncated snapshot JSON), `fetch_repository` silently accepts the truncated body.
#[test]
fn test_bug_007_http_client_content_length_truncation_rejected() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let valid_repo = Repository::new(Version::empty(), vec![]);
    let valid_json = valid_repo.to_json_pretty().unwrap();

    let server_handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);

            // Server promises 500 bytes via Content-Length, but only sends the ~39 bytes of valid_json
            // before closing the connection.
            let promised_len = valid_json.len() + 300;
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {promised_len}\r\nConnection: close\r\n\r\n"
            )
            .into_bytes();
            response.extend_from_slice(valid_json.as_bytes());

            let _ = stream.write_all(&response);
            let _ = stream.flush();
            // Connection closes here when stream is dropped
        }
    });

    let url = format!("http://127.0.0.1:{port}/repository.json");
    let res = fetch_repository(&url);
    server_handle.join().unwrap();

    assert!(
        res.is_err(),
        "Expected fetch_repository to reject truncated response body where fewer bytes than Content-Length were received, but got Ok"
    );
}
