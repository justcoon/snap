# Bug Resolution Walkthrough: BUG-004

## Executive Summary
- **Bug ID:** `BUG-004`
- **Title:** HTTP client `decode_chunked` fails to parse RFC 7230 chunk extensions
- **Affected Subsystems:**
  - HTTP Snapshot Client & Chunked Decoder ([`rust/src/http/client.rs`](../../rust/src/http/client.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_004_http_chunked_fails_on_valid_chunk_extensions`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> RFC 7230 §4.1:
> ```text
> chunk = chunk-size [ chunk-ext ] CRLF
>         chunk-data CRLF
> chunk-ext = *( ";" chunk-ext-name [ "=" chunk-ext-val ] )
> ```
> Valid HTTP/1.1 chunk headers may include optional chunk extensions following a semicolon `;`.
> Recipients must ignore unrecognized chunk extensions (RFC 7230 §4.1.1).

### Root Cause
1. In `rust/src/http/client.rs:decode_chunked`, the chunk size parsing logic was:
   ```rust
   let len_str = std::str::from_utf8(len_slice)
       .map_err(|_| CliError::Custom("invalid chunk length encoding".into()))?;
   let chunk_len = usize::from_str_radix(len_str.trim(), 16)
       .map_err(|_| CliError::Custom("invalid chunk length hex".into()))?;
   ```
2. `usize::from_str_radix` was passed the entire line preceding CRLF (`len_str.trim()`). When chunk extensions were present (such as `1a;name=val\r\n` or `0;last=true\r\n`), parsing failed with `CliError::Custom("invalid chunk length hex")`.

### Code Changes Applied
1. **[`rust/src/http/client.rs`](../../rust/src/http/client.rs):**
   - Split `len_str` by `;` and isolated the leading hex size segment:
     ```rust
     let hex_part = len_str.split(';').next().unwrap_or("");
     let chunk_len = usize::from_str_radix(hex_part.trim(), 16)
         .map_err(|_| CliError::Custom("invalid chunk length hex".into()))?;
     ```
   - Added permanent subsystem unit regression test: `test_regression_bug_004_http_chunked_parses_chunk_extensions`.
2. **[`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs):**
   - Verified that `test_bug_004_http_chunked_fails_on_valid_chunk_extensions` passes.
   - Annotated with `#[ignore = "Resolved in BUG-004 (see docs/bugs/resolution_BUG-004_walkthrough.md)"]`.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_004_http_chunked_fails_on_valid_chunk_extensions ... FAILED

failures:
---- test_bug_004_http_chunked_fails_on_valid_chunk_extensions stdout ----
thread panicked at tests/bug_reproductions.rs:166:5:
Expected fetch_repository to succeed with valid chunk extensions, but failed with: Some(Custom("invalid chunk length hex"))
```

### After Fix:
```text
running 1 test
test test_bug_004_http_chunked_fails_on_valid_chunk_extensions ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_004_http_chunked_parses_chunk_extensions` to [`rust/src/http/client.rs`](../../rust/src/http/client.rs).
   - Confirmed passing in `cargo test --lib test_regression_bug_004`.
   - In-process suite: 50 passed; 0 failed.
2. **Active Burndown Annotation:**
   - Annotated `test_bug_004` with `#[ignore = "Resolved in BUG-004..."]` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs).
   - Explicit targeting: `cargo test --test bug_reproductions -- --ignored test_bug_004` $\to$ **`ok`**.
   - Backlog burndown: `cargo test --test bug_reproductions` $\to$ **All 4 bugs are resolved (4 ignored; 0 failed)**!
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean.
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-004`: HTTP client `decode_chunked` fails to parse RFC 7230 chunk extensions
- **Identified Defect:** `decode_chunked` attempted to parse chunk size string without stripping optional `;` chunk extension parameters.
- **Remediation Applied:** Isolated hex chunk length before any semicolon in `decode_chunked`, added unit regression test `test_regression_bug_004_http_chunked_parses_chunk_extensions` in `rust/src/http/client.rs`, and marked `test_bug_004` as resolved in the reproducer backlog.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_004`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 50 unit/property tests passed.
- **Unintended Side-Effects:** None: Correct adherence to RFC 7230 §4.1 extension specification.
