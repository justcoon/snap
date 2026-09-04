# Snap Bug Hunting Report

## Executive Summary
- **Report Date:** 2026-09-04
- **Total Bugs Identified & Proven:** 4 (Minimum required: 3)
- **Primary Subsystems Affected:**
  - Repository Graph & Invariant Validation (`rust/src/core/validation.rs`)
  - CLI Commit Validation & State Mutation (`rust/src/cli/commands/commit.rs`)
  - HTTP Snapshot Client & Chunked Decoder (`rust/src/http/client.rs`)
- **Reproducer Suite Location:** [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs)
- **Failing Tests Count:** 4 / 4 failing tests proving existence of all discovered bugs.

---

## Proven Bugs & Corresponding Failing Test Cases

| Bug ID | Title | Subsystem / File | SPEC Reference | Failing Test Case | Status |
|---|---|---|---|---|---|
| `BUG-001` | `validate_repository` does not validate patch messages for ASCII control characters | `rust/src/core/validation.rs` | §4.2 | `test_bug_001_validate_repository_allows_control_chars_in_patch_message` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-001_walkthrough.md)) |
| `BUG-002` | `validate_repository` accepts patches containing duplicate change paths | `rust/src/core/validation.rs` | §4.2 | `test_bug_002_validate_repository_accepts_duplicate_change_paths` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-002_walkthrough.md)) |
| `BUG-003` | HTTP client `decode_chunked` tolerates missing CRLF after chunk data and accepts truncated bodies | `rust/src/http/client.rs` | RFC 7230 §4.1 / SPEC §7.1, §7.8 | `test_bug_003_http_chunked_missing_crlf_should_error` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-003_walkthrough.md)) |
| `BUG-004` | HTTP client `decode_chunked` fails to parse RFC 7230 chunk extensions | `rust/src/http/client.rs` | RFC 7230 §4.1 / SPEC §7.8 | `test_bug_004_http_chunked_fails_on_valid_chunk_extensions` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-004_walkthrough.md)) |

---

## Detailed Breakdown & Root Cause Analysis

### Bug BUG-001: Repository Validation Ignores Disallowed ASCII Control Characters in Patch Messages
- **Location:** [`rust/src/core/validation.rs:116`](../../rust/src/core/validation.rs#L116) and [`rust/src/cli/commands/commit.rs:20`](../../rust/src/cli/commands/commit.rs#L20)
- **Violated Contract:**
  > SPEC §4.2: "`message` is a nonempty UTF-8 string. It may contain tab and LF but no other ASCII control character. `snap commit` limits user-supplied messages to 4096 bytes..."
- **Current Behavior:**
  `validate_repository` validates patch ordering, causal closure, and changes against base trees, but never invokes `patch.validate()` or checks `patch.message` for ASCII control characters. Similarly, `cmd_commit` only checks `message.is_empty() || message.len() > MAX_COMMIT_MESSAGE_BYTES`. Consequently, a commit message containing bytes like `\x01` or `\r` passes validation and is accepted into `.snap/repository.json`.
- **Expected Behavior:**
  `validate_repository` and `cmd_commit` must reject any commit message containing ASCII control characters other than `\t` and `\n`.
- **Reproducer:** [`test_bug_001_validate_repository_allows_control_chars_in_patch_message`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Added `ValidationError::InvalidPatch(PatchError)` and called `patch.validate()` for each patch in `validate_repository`.
  - Added character-level control character validation in `cmd_commit` (`message.chars().any(|c| c.is_ascii_control() && c != '\t' && c != '\n')`).
  - Verified `test_bug_001` passed cleanly; regression suite `./verify --lang rust` passed 28/28.
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-001_walkthrough.md`](resolution_BUG-001_walkthrough.md).

---

### Bug BUG-002: Repository Validation Accepts Patches with Duplicate or Unsorted Change Paths
- **Location:** [`rust/src/core/validation.rs:218`](../../rust/src/core/validation.rs#L218)
- **Violated Contract:**
  > SPEC §4.2: "`changes` is nonempty, sorted by path, and contains at most one change per path."
- **Current Behavior:**
  In `rust/src/core/validation.rs`, the loop `for change in &patch.changes` inspects whether each individual change is valid against the materialized base tree. However, it fails to check that `changes` is sorted by path and contains at most one change per path. As a result, a patch containing duplicate operations for `"notes.txt"` is erroneously accepted by `validate_repository`.
- **Expected Behavior:**
  `validate_repository` must reject any patch where `changes` is empty, unsorted, or contains duplicate paths for the same file.
- **Reproducer:** [`test_bug_002_validate_repository_accepts_duplicate_change_paths`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Invoking `patch.validate().map_err(ValidationError::InvalidPatch)?;` on each patch inside `validate_repository` ensures patches with duplicate change paths, unsorted paths, or empty changes are rejected.
  - Added permanent subsystem unit regression test `test_regression_bug_002_duplicate_change_paths_rejected` in `rust/src/core/validation.rs`.
  - Annotated reproducer `test_bug_002` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (48/48 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-002_walkthrough.md`](resolution_BUG-002_walkthrough.md).

---

### Bug BUG-003: HTTP Chunked Decoder Accepts Missing Trailing CRLF and Incomplete Chunk Data
- **Location:** [`rust/src/http/client.rs:163`](../../rust/src/http/client.rs#L163)
- **Violated Contract:**
  > RFC 7230 §4.1:
  > ```text
  > chunk = chunk-size [ chunk-ext ] CRLF
  >         chunk-data CRLF
  > ```
  > Every chunk data segment must be terminated by a mandatory CRLF (`\r\n`).
- **Current Behavior:**
  In `decode_chunked`, the code reads:
  ```rust
  out.extend_from_slice(&input[cursor..cursor + chunk_len]);
  cursor += chunk_len;
  if cursor + 2 <= input.len() && &input[cursor..cursor + 2] == b"\r\n" {
      cursor += 2;
  }
  ```
  If the `\r\n` is completely missing after `chunk-data`, the cursor is not advanced past CRLF, but rather proceeds to parse the subsequent bytes as the next chunk length or terminates without error. Truncated or malformed HTTP payloads lacking CRLF are accepted as valid.
- **Expected Behavior:**
  The decoder must strictly verify that each chunk data segment is immediately followed by CRLF (`\r\n`), returning an error if missing.
- **Reproducer:** [`test_bug_003_http_chunked_missing_crlf_should_error`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Enforced mandatory verification that `cursor + 2 <= input.len()` and `&input[cursor..cursor + 2] == b"\r\n"`, erroring with `CliError::Custom("missing CRLF after chunk data")`.
  - Added stream termination tracking requiring `chunk_len == 0` terminating chunk before EOF.
  - Added permanent unit regression test `test_regression_bug_003_http_chunked_missing_crlf_rejected` in `rust/src/http/client.rs`.
  - Annotated reproducer `test_bug_003` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (49/49 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-003_walkthrough.md`](resolution_BUG-003_walkthrough.md).

---

### Bug BUG-004: HTTP Chunked Decoder Crashes on Valid RFC 7230 Chunk Extensions
- **Location:** [`rust/src/http/client.rs:150-153`](../../rust/src/http/client.rs#L150-L153)
- **Violated Contract:**
  > RFC 7230 §4.1:
  > ```text
  > chunk-ext = *( ";" chunk-ext-name [ "=" chunk-ext-val ] )
  > ```
  > Valid HTTP/1.1 chunk headers may include optional chunk extensions following a semicolon `;`.
- **Current Behavior:**
  `decode_chunked` takes the entire slice up to CRLF and directly calls `usize::from_str_radix(len_str.trim(), 16)`. When an HTTP server sends standard chunk extensions (e.g. `1a;name=val\r\n`), parsing fails with `CliError::Custom("invalid chunk length hex")`.
- **Expected Behavior:**
  The chunk size parser must isolate the hex chunk size before any `;` character, ignoring or processing valid chunk extensions.
- **Reproducer:** [`test_bug_004_http_chunked_fails_on_valid_chunk_extensions`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Isolated hex chunk length segment before `;` via `len_str.split(';').next().unwrap_or("")` in `decode_chunked`, ignoring optional RFC 7230 chunk extensions.
  - Added permanent unit regression test `test_regression_bug_004_http_chunked_parses_chunk_extensions` in `rust/src/http/client.rs`.
  - Annotated reproducer `test_bug_004` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (50/50 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-004_walkthrough.md`](resolution_BUG-004_walkthrough.md).
