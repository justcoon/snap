# Bug Resolution Walkthrough: BUG-007

## Executive Summary
- **Bug ID:** `BUG-007`
- **Title:** HTTP client `fetch_repository` ignores `Content-Length` and accepts prematurely truncated response bodies
- **Subsystem:** HTTP Snapshot Client (`rust/src/http/client.rs`)
- **SPEC Clause Violated:** RFC 7230 §3.3.3 ("If a message is received with a Content-Length header field and a message-body is received of length less than the number of octets indicated by the Content-Length, the message has been truncated and MUST be treated as an error."), SPEC §7.1, §7.8
- **Reproducer:** `test_bug_007_http_client_content_length_truncation_rejected` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs)
- **Status:** 🟢 `FIXED`

---

## Root Cause Analysis
In `rust/src/http/client.rs`, `fetch_repository` read the response buffer up to socket EOF and inspected `Transfer-Encoding` for `chunked`. For non-chunked responses, it took `&response_buf[body_offset..]` unconditionally without checking for `Content-Length`:
```rust
let raw_body = &response_buf[body_offset..];
let body_bytes = if is_chunked {
    decode_chunked(raw_body)?
} else {
    raw_body.to_vec()
};
```
When an HTTP server specified `Content-Length: <expected_len>` but terminated the TCP connection prematurely (e.g. broken connection, EOF after sending partial JSON), `fetch_repository` never checked if the received bytes matched `expected_len`. If the truncated slice happened to parse into a valid JSON repository, the client silently returned `Ok(repo)`.

Furthermore, if the server sent trailing bytes after the advertised `Content-Length`, `raw_body.to_vec()` included the trailing junk, corrupting JSON parsing.

Per RFC 7230 §3.3.3:
> "If a message is received with a Content-Length header field and a message-body is received of length less than the number of octets indicated by the Content-Length, the message has been truncated and MUST be treated as an error."

---

## Changes Made

### 1. Added Strict `Content-Length` Parsing and Body Framing
**File:** [`rust/src/http/client.rs`](../../rust/src/http/client.rs)
- Parsed `Content-Length` header in the HTTP header loop:
  - Verified valid integer syntax (`usize`).
  - Verified that conflicting multiple `Content-Length` headers trigger an error.
- Enforced framing and truncation checks:
  ```rust
  let body_bytes = if is_chunked {
      decode_chunked(raw_body)?
  } else if let Some(expected_len) = content_length {
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
  ```
- Guaranteed that premature EOF is rejected with a descriptive error, and trailing bytes beyond `Content-Length` are ignored.

### 2. Added Permanent Subsystem Regression Test
**File:** [`rust/src/http/client.rs`](../../rust/src/http/client.rs)
- Added `test_regression_bug_007_content_length_truncation_and_framing` testing:
  1. Truncated HTTP responses (socket closed before advertised Content-Length bytes received) are rejected as errors.
  2. Extra trailing junk after Content-Length bytes is safely framed and ignored.

---

## Verification & Active Burndown

### 1. Failing Reproducer to Passing Transition
```console
$ cargo test --test bug_reproductions -- --ignored test_bug_007
running 1 test
test test_bug_007_http_client_content_length_truncation_rejected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.00s
```

### 2. Burndown Suite Status
```console
$ cargo test --test bug_reproductions
running 7 tests
test test_bug_001_validate_repository_allows_control_chars_in_patch_message ... ignored, Resolved in BUG-001
test test_bug_002_validate_repository_accepts_duplicate_change_paths ... ignored, Resolved in BUG-002
test test_bug_003_http_chunked_missing_crlf_should_error ... ignored, Resolved in BUG-003
test test_bug_004_http_chunked_fails_on_valid_chunk_extensions ... ignored, Resolved in BUG-004
test test_bug_005_log_reverse_canonical_integration_order ... ignored, Resolved in BUG-005
test test_bug_006_revert_empty_text_file_from_absent ... ignored, Resolved in BUG-006
test test_bug_007_http_client_content_length_truncation_rejected ... ignored, Resolved in BUG-007

test result: ok. 0 passed; 0 failed; 7 ignored; finished in 0.00s
```

### 3. Unit & Property Tests
```console
$ cargo test --lib
test result: ok. 53 passed; 0 failed; 0 ignored; finished in 0.21s
```

### 4. Shared Acceptance Suite Execution
```console
$ ./verify --lang rust
snap tests — candidate=/var/folders/7f/s_dm8hkd2z78nfn6r6trdw200000gn/T/snap-rust.ZpC7Ii, 28 case(s)
  ✓ init creates an empty repository 1054ms
  ✓ initialization preserves files and rejects nested or existing repositories 274ms
  ✓ local and global contributor configuration have strict precedence 477ms
  ✓ commit status and log expose exact deterministic history 527ms
  ✓ diff renders canonical repeated-line edits and missing final newlines 401ms
  ✓ binary and empty files are versioned byte exactly 288ms
  ✓ revert is additive and restores file-directory transitions 541ms
  ✓ working tree scans reject symlinks and special files without mutation 304ms
  ✓ local merge converges concurrent text changes and is idempotent 609ms
  ✓ merge applies every whole-file conflict rule with sorted warnings 554ms
  ✓ canonical namespace winners replace conflicting files in both directions 833ms
  ✓ server exposes one immutable repository snapshot and exits on SIGTERM 443ms
  ✓ HTTP merge and diff use one exact validated GET without redirects 554ms
  ✓ command grammar and common failures use stable exit channels 525ms
  ✓ repository reader rejects malformed schemas histories paths and edits 580ms
  ✓ cross-repository dot collisions fail before changing local state 313ms
  ✓ concurrent creates choose the canonical later value independent of merge direction 496ms
  ✓ three-way text history converges across different merge association orders 1349ms
  ✓ CLI versions are canonical known causal frontiers 531ms
  ✓ merge refuses dirty and unsupported working trees without importing history 352ms
  ✓ vector clocks use causal closure componentwise join and canonical Snap order 671ms
  ✓ text OT covers overlapping deletes split counts insert priority and trailing inserts 1935ms
  ✓ repository validation rejects every malformed layer before mutation 788ms
  ✓ every command rejects unknown misplaced duplicate and extra arguments 1008ms
  ✓ configuration versions paths and text use their exact canonical boundaries 1192ms
  ✓ local exchange preserves text bytes and malformed remotes never mutate 821ms
  ✓ patch histories require exact schemas canonical order and valid base transitions 540ms
  ✓ terminal presentation is colorful readable and explicitly controllable 1949ms

28 passed in 19909ms
```

---

## Bug Fix Discrepancy Check
- **Bug ID & Title:** `BUG-007`: HTTP client `fetch_repository` ignores `Content-Length` and accepts prematurely truncated response bodies
- **Identified Defect:** `fetch_repository` took raw body buffer up to socket EOF without verifying whether `Content-Length` was fulfilled.
- **Remediation Applied:** Validated `Content-Length` against received bytes (`raw_body.len() < expected_len` errors; `raw_body[..expected_len]` frames body).
- **Failing Reproducer Status:** Confirmed `test_bug_007_http_client_content_length_truncation_rejected` now passes cleanly (`1 passed`).
- **Regression Verification:** All 53 in-process unit/property tests and all 28 acceptance test suites pass with 100% compliance.
