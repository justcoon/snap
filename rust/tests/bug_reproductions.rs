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
#[ignore = "Resolved in BUG-006 (see docs/bugs/resolution_BUG-006_walkthrough.md)"]
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
#[ignore = "Resolved in BUG-007 (see docs/bugs/resolution_BUG-007_walkthrough.md)"]
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

/// SPEC §7.2:
/// "snap config [--global] contributor.id <id>
/// Sets the contributor identity...
/// The --global flag may precede or follow the key:
/// snap config contributor.id --global <id>
/// is identical to the form shown above."
///
/// BUG-008: `parse_args` only accepts `--global` before the key in `snap config`.
/// When `--global` follows `contributor.id` (e.g. `snap config contributor.id --global <id>`),
/// `parse_args` fails with `ParseError::InvalidCommandOrArguments`.
#[test]
#[ignore = "Resolved in BUG-008 (see docs/bugs/resolution_BUG-008_walkthrough.md)"]
fn test_bug_008_config_flag_after_key_supported() {
    use snap::cli::args::{parse_args, Command};

    let args = vec![
        "snap".to_string(),
        "config".to_string(),
        "contributor.id".to_string(),
        "--global".to_string(),
        "alice@example.com".to_string(),
    ];
    let cmd = parse_args(&args[1..]);
    assert!(
        cmd.is_ok(),
        "Expected 'snap config contributor.id --global alice@example.com' to succeed, but got: {:?}",
        cmd
    );
    assert_eq!(
        cmd.unwrap(),
        Command::Config {
            is_global: true,
            key: "contributor.id".to_string(),
            value: "alice@example.com".to_string(),
        }
    );
}

/// SPEC §2:
/// "Every tracked tree is prefix-free by path segment: if `a` is a file, no `a/...` path is
/// present. This is validated for every patch's authored result and enforced during concurrent
/// replay by §6.4."
/// SPEC §4.5:
/// "5. The authored result tree of every patch (the tree resulting from applying its changes to its
/// base tree) is prefix-free by path segment."
///
/// BUG-009: `validate_repository` checks `check_prefix_free` only on `patch.changes` in isolation,
/// but never verifies that the authored result tree (applying `patch.changes` to `base_tree`)
/// is prefix-free. A patch creating a file "dir" when "dir/file.txt" exists in base tree is accepted.
#[test]
#[ignore = "Resolved in BUG-009 (see docs/bugs/resolution_BUG-009_walkthrough.md)"]
fn test_bug_009_validate_repository_authored_result_prefix_free() {
    let author = ContributorId::parse("alice@example.com").unwrap();
    let v1 = Version::parse("(alice@example.com->1)").unwrap();
    let v2 = Version::parse("(alice@example.com->2)").unwrap();

    let patch1 = Patch {
        author: author.clone(),
        revision: 1,
        base: Version::empty(),
        message: "create nested file".to_string(),
        changes: vec![Change::Put {
            path: "dir/file.txt".to_string(),
            content: BASE64_STANDARD.encode(b"hello"),
        }],
    };

    // Patch 2 creates regular file "dir" while "dir/file.txt" exists in base tree.
    // Resulting authored tree contains both "dir" and "dir/file.txt" (not prefix-free).
    let patch2 = Patch {
        author: author.clone(),
        revision: 2,
        base: v1.clone(),
        message: "shadow directory with file".to_string(),
        changes: vec![Change::Put {
            path: "dir".to_string(),
            content: BASE64_STANDARD.encode(b"shadow"),
        }],
    };

    let repo = Repository::new(v2, vec![patch1, patch2]);
    let res = validate_repository(&repo);
    assert!(
        res.is_err(),
        "Expected validate_repository to reject patch whose authored result tree is not prefix-free, but got Ok(())"
    );
}

/// SPEC §4.3:
/// "A text or put creation requires the path to be absent in the patch's exact base tree...
/// A change that does not alter path existence or bytes is invalid, except that an empty text
/// edit may create an empty file."
/// SPEC §4.4:
/// "The script MUST consume the complete old token sequence; there is no implicit trailing retain...
/// An empty script is valid only when creating an empty text file."
///
/// BUG-010: In `validate_repository` step 4, when `base_bytes` is `None` for a `Change::Text`,
/// validation is skipped. An invalid text creation with `Retain(5)` or `Delete(3)` passes step 4.
#[test]
#[ignore = "Resolved in BUG-010 (see docs/bugs/resolution_BUG-010_walkthrough.md)"]
fn test_bug_010_validate_repository_text_creation_from_absent_validation() {
    use snap::core::patch::TextEditOp;

    let author = ContributorId::parse("alice@example.com").unwrap();
    let v1 = Version::parse("(alice@example.com->1)").unwrap();

    // Patch creates "file.txt" (absent in base), but specifies TextEditOp::Retain(5)
    // which cannot consume tokens from an absent (empty) file.
    let patch = Patch {
        author: author.clone(),
        revision: 1,
        base: Version::empty(),
        message: "invalid text creation with retain".to_string(),
        changes: vec![Change::Text {
            path: "file.txt".to_string(),
            edit: vec![TextEditOp::Retain(5)],
        }],
    };

    let repo = Repository::new(v1, vec![patch]);
    let res = validate_repository(&repo);
    assert!(
        res.is_err(),
        "Expected validate_repository to reject text creation with Retain operation from absent file, but got Ok(())"
    );
}
