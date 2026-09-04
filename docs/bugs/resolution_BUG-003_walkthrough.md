# Bug Resolution Walkthrough: BUG-003

## Executive Summary
- **Bug ID:** `BUG-003`
- **Title:** HTTP client `decode_chunked` tolerates missing CRLF after chunk data and accepts truncated bodies
- **Affected Subsystems:**
  - HTTP Snapshot Client & Chunked Decoder ([`rust/src/http/client.rs`](../../rust/src/http/client.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_003_http_chunked_missing_crlf_should_error`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> RFC 7230 §4.1 / SPEC §7.1, §7.8:
> ```text
> chunk = chunk-size [ chunk-ext ] CRLF
>         chunk-data CRLF
> ```
> Every chunk data segment must be terminated by a mandatory CRLF (`\r\n`).
> A chunked stream must terminate with `last-chunk` (e.g. `0\r\n\r\n`).

### Root Cause
1. In `rust/src/http/client.rs:decode_chunked`, the trailing CRLF check after reading `chunk-data` was written as an optional check:
   ```rust
   if cursor + 2 <= input.len() && &input[cursor..cursor + 2] == b"\r\n" {
       cursor += 2;
   }
   ```
   If the CRLF was missing or if input was shorter than 2 bytes, the parser silently skipped the delimiter rather than raising an error.
2. If the input stream ended without a terminating `0` chunk (`last-chunk`), the loop simply exited and returned the partial buffer without verifying that `last-chunk` was ever received.
3. Consequently, truncated HTTP responses and malformed chunk streams lacking the required trailing `\r\n` passed through `decode_chunked` without error.

### Code Changes Applied
1. **[`rust/src/http/client.rs`](../../rust/src/http/client.rs):**
   - Strictly validated that `cursor + 2 <= input.len()` and `&input[cursor..cursor + 2] == b"\r\n"`, returning `Err(CliError::Custom("missing CRLF after chunk data".into()))` if violated.
   - Introduced a `terminated` boolean flag set upon encountering `chunk_len == 0`.
   - Verified that `terminated == true` after loop termination, returning `Err(CliError::Custom("truncated chunked stream: missing terminating chunk".into()))` if missing.
   - Added permanent subsystem unit regression test: `test_regression_bug_003_http_chunked_missing_crlf_rejected`.
2. **[`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs):**
   - Verified that `test_bug_003_http_chunked_missing_crlf_should_error` passes.
   - Annotated with `#[ignore = "Resolved in BUG-003 (see docs/bugs/resolution_BUG-003_walkthrough.md)"]`.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_003_http_chunked_missing_crlf_should_error ... FAILED

failures:
---- test_bug_003_http_chunked_missing_crlf_should_error stdout ----
thread panicked at tests/bug_reproductions.rs:121:5:
Expected fetch_repository to reject chunk missing trailing CRLF, but got Ok
```

### After Fix:
```text
running 1 test
test test_bug_003_http_chunked_missing_crlf_should_error ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_003_http_chunked_missing_crlf_rejected` to [`rust/src/http/client.rs`](../../rust/src/http/client.rs).
   - Confirmed passing in `cargo test --lib test_regression_bug_003`.
   - In-process suite: 49 passed; 0 failed.
2. **Active Burndown Annotation:**
   - Annotated `test_bug_003` with `#[ignore = "Resolved in BUG-003..."]` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs).
   - Explicit targeting: `cargo test --test bug_reproductions -- --ignored test_bug_003` $\to$ **`ok`**.
   - Backlog burndown: `cargo test --test bug_reproductions` $\to$ displays 3 ignored (`BUG-001`, `BUG-002`, `BUG-003`) and 1 remaining open bug (`BUG-004`).
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean.
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-003`: HTTP client `decode_chunked` tolerates missing CRLF after chunk data and accepts truncated bodies
- **Identified Defect:** Optional conditional check on CRLF after chunk data allowed payloads without mandatory CRLF to pass; loop exited without ensuring `0` terminating chunk was received.
- **Remediation Applied:** Enforced mandatory `\r\n` check and termination flag in `decode_chunked`, added unit regression test `test_regression_bug_003_http_chunked_missing_crlf_rejected` in `rust/src/http/client.rs`, and marked `test_bug_003` as resolved in the reproducer backlog.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_003`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 49 unit/property tests passed.
- **Unintended Side-Effects:** None: Strict compliance with RFC 7230 §4.1 framing rules.
