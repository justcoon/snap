# Snap Bug Hunting Report

## Executive Summary
- **Report Date:** 2026-09-04
- **Total Bugs Identified & Proven:** 8 (All 8 fixed; Minimum required per hunt: 3)
- **Primary Subsystems Affected:**
  - Repository Graph & Invariant Validation (`rust/src/core/validation.rs`)
  - CLI Commit Validation & State Mutation (`rust/src/cli/commands/commit.rs`)
  - HTTP Snapshot Client & Chunked Decoder (`rust/src/http/client.rs`)
  - CLI Log Command & History Replay Order (`rust/src/cli/commands/log.rs`)
  - CLI Revert Command & Empty File Reversion (`rust/src/cli/commands/revert.rs`)
  - CLI Argument Parsing (`rust/src/cli/args.rs`)
- **Reproducer Suite Location:** [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs)
- **Active Burndown Status:** 0 open bugs remaining; all 8 discovered bugs verified fixed and cleanly ignored in the reproducer burndown backlog.

---

## Proven Bugs & Corresponding Failing Test Cases

| Bug ID | Title | Subsystem / File | SPEC Reference | Failing Test Case | Status |
|---|---|---|---|---|---|
| `BUG-001` | `validate_repository` does not validate patch messages for ASCII control characters | `rust/src/core/validation.rs` | §4.2 | `test_bug_001_validate_repository_allows_control_chars_in_patch_message` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-001_walkthrough.md)) |
| `BUG-002` | `validate_repository` accepts patches containing duplicate change paths | `rust/src/core/validation.rs` | §4.2 | `test_bug_002_validate_repository_accepts_duplicate_change_paths` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-002_walkthrough.md)) |
| `BUG-003` | HTTP client `decode_chunked` tolerates missing CRLF after chunk data and accepts truncated bodies | `rust/src/http/client.rs` | RFC 7230 §4.1 / SPEC §7.1, §7.8 | `test_bug_003_http_chunked_missing_crlf_should_error` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-003_walkthrough.md)) |
| `BUG-004` | HTTP client `decode_chunked` fails to parse RFC 7230 chunk extensions | `rust/src/http/client.rs` | RFC 7230 §4.1 / SPEC §7.8 | `test_bug_004_http_chunked_fails_on_valid_chunk_extensions` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-004_walkthrough.md)) |
| `BUG-005` | `cmd_log` traverses `repo.patches` in reverse author order instead of reverse canonical integration order | `rust/src/cli/commands/log.rs` | §7.4 | `test_bug_005_log_reverse_canonical_integration_order` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-005_walkthrough.md)) |
| `BUG-006` | `cmd_revert` creates invalid empty `TextEditOp::Insert([])` when reverting/restoring an empty text file | `rust/src/cli/commands/revert.rs` | §4.4, §7.7 | `test_bug_006_revert_empty_text_file_from_absent` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-006_walkthrough.md)) |
| `BUG-007` | HTTP client `fetch_repository` ignores `Content-Length` and accepts prematurely truncated response bodies | `rust/src/http/client.rs` | RFC 7230 §3.3.3 / SPEC §7.1, §7.8 | `test_bug_007_http_client_content_length_truncation_rejected` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-007_walkthrough.md)) |
| `BUG-008` | `snap config` does not support `--global` flag after the key | `rust/src/cli/args.rs` | §7.2 | `test_bug_008_config_flag_after_key_supported` | 🟢 `FIXED` ([Walkthrough](resolution_BUG-008_walkthrough.md)) |

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

---

### Bug BUG-005: `snap log` Reverses Author Ordering Rather than Canonical Integration Order
- **Location:** [`rust/src/cli/commands/log.rs:11-15`](../../rust/src/cli/commands/log.rs#L11-L15)
- **Violated Contract:**
  > SPEC §7.4:
  > "Prints patches in reverse canonical integration order, one tab-separated line each:
  > `<result_version>\t<author>\t<message>`"
- **Current Behavior:**
  In `rust/src/cli/commands/log.rs`, entries are collected using `repo.patches.iter().rev()`. By SPEC §4.1, `repo.patches` is stored sorted by author (`ContributorId`), then revision. Reversing `repo.patches` merely reverses contributor ID sort order. In a repository where contributor `Alice` authors a child commit based on a root commit authored by `Bob`, `repo.patches` contains `[Alice->1, Bob->1]`. `repo.patches.iter().rev()` prints `Bob->1` first and `Alice->1` second, completely inverting historical causation and placing the root commit before the dependent commit.
- **Expected Behavior:**
  `snap log` must determine canonical integration order (§6.1) starting from the empty tree and version, and print the resulting patches in exact reverse canonical integration order (newest integrated patch first).
- **Reproducer:** [`test_bug_005_log_reverse_canonical_integration_order`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Implemented `canonical_integration_order` in `rust/src/core/replay.rs` conforming to SPEC §6.1.
  - Refactored `cmd_log` in `rust/src/cli/commands/log.rs` to traverse `canonical_integration_order` in reverse (`ordered_patches.iter().rev()`).
  - Added permanent unit regression test `test_regression_bug_005_canonical_integration_order` in `rust/src/core/replay.rs`.
  - Annotated reproducer `test_bug_005` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (51/51 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-005_walkthrough.md`](resolution_BUG-005_walkthrough.md).

---

### Bug BUG-006: `snap revert` Emits Invalid Empty Insert Operation When Restoring Empty Text Files
- **Location:** [`rust/src/cli/commands/revert.rs:66-72`](../../rust/src/cli/commands/revert.rs#L66-L72)
- **Violated Contract:**
  > SPEC §4.4:
  > "`{"insert": [s...]}` inserts one or more nonempty text tokens... An empty script is valid only when creating an empty text file."
  > SPEC §7.7:
  > "`snap revert <version>` reverts the tree to a previously recorded version without rewriting history. Creates a new patch whose changes transform the current tree into the target tree... Records the new patch, updates the frontier, and prints the new version."
- **Current Behavior:**
  When reverting a file from absent (`None`) to present with empty text content (`Some(b"")`), `cmd_revert` executes:
  ```rust
  let tokens = tokenize_text(new_bytes)?;
  changes.push(Change::Text {
      path: path.to_string(),
      edit: vec![TextEditOp::Insert(tokens)],
  });
  ```
  Because `new_bytes` is empty, `tokens` is empty (`vec![]`), producing an invalid edit operation `TextEditOp::Insert(vec![])`. When the new patch is validated and replayed via `validate_repository`, it fails with `PatchError::InvalidChange("insert operation cannot be empty")` or `DiffError::EmptyInsert`, causing `snap revert` to fail with exit code 1.
- **Expected Behavior:**
  When restoring an absent file to an empty text file, `cmd_revert` must generate an empty edit script (`edit: vec![]`), which is the canonical representation under SPEC §4.4 for creating an empty text file.
- **Reproducer:** [`test_bug_006_revert_empty_text_file_from_absent`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Extracted pure `compute_revert_changes` in `rust/src/cli/commands/revert.rs`.
  - In absent-to-present text transition, emit `vec![]` (empty edit script) when `tokens.is_empty()` per SPEC §4.4.
  - Added permanent unit regression test `test_regression_bug_006_revert_empty_text_file` in `rust/src/cli/commands/revert.rs`.
  - Annotated reproducer `test_bug_006` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (52/52 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-006_walkthrough.md`](resolution_BUG-006_walkthrough.md).

---

### Bug BUG-007: HTTP Client Ignores `Content-Length` and Accepts Truncated Message Bodies
- **Location:** [`rust/src/http/client.rs:129-135`](../../rust/src/http/client.rs#L129-L135)
- **Violated Contract:**
  > RFC 7230 §3.3.3:
  > "If a message is received with a Content-Length header field and a message-body is received of length less than the number of octets indicated by the Content-Length, the message has been truncated and MUST be treated as an error."
  > SPEC §7.1, §7.8:
  > Conforms to HTTP/1.1 client framing and repository fetching.
- **Current Behavior:**
  `fetch_repository` reads until EOF or end-of-headers, then directly passes `&response_buf[body_offset..]` to `Repository::from_json_slice` without checking if `Content-Length` was specified and matched the received byte count. If a server response specifies `Content-Length: 500` but drops the connection after sending a 39-byte valid repository JSON payload, the client treats the truncated response as successful and returns `Ok(repo)`.
- **Expected Behavior:**
  When `Content-Length` is present in the response headers, `fetch_repository` must verify that the body contains at least `Content-Length` bytes (and frame the body to exactly `Content-Length` bytes), returning an error if fewer bytes were received before the connection closed.
- **Reproducer:** [`test_bug_007_http_client_content_length_truncation_rejected`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Added strict `Content-Length` header validation and parsing in `rust/src/http/client.rs`.
  - Enforced RFC 7230 §3.3.3 framing and truncation checks (`raw_body.len() < expected_len` errors; `raw_body[..expected_len]` frames body).
  - Added permanent unit regression test `test_regression_bug_007_content_length_truncation_and_framing` in `rust/src/http/client.rs`.
  - Annotated reproducer `test_bug_007` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (53/53 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-007_walkthrough.md`](resolution_BUG-007_walkthrough.md).

---

### Bug BUG-008: `snap config` Does Not Support `--global` Flag After the Key
- **Location:** [`rust/src/cli/args.rs:87-115`](../../rust/src/cli/args.rs#L87-L115)
- **Violated Contract:**
  > SPEC §7.2:
  > "`snap config [--global] contributor.id <id>` ... The `--global` flag may precede or follow the key:
  > `snap config contributor.id --global <id>` is identical to the form shown above."
- **Current Behavior:**
  In `rust/src/cli/args.rs`, the `parse_args` function only handles the case where `--global` precedes the key (`snap config --global contributor.id <value>`). The parser does not handle the case where `--global` follows the key (`snap config contributor.id --global <value>`), causing it to return `ParseError::InvalidCommandOrArguments`.
- **Expected Behavior:**
  Both `snap config --global contributor.id <value>` and `snap config contributor.id --global <value>` should parse identically with `is_global: true`.
- **Reproducer:** [`test_bug_008_config_flag_after_key_supported`](../../rust/tests/bug_reproductions.rs)
- **Resolution:**
  - Added a new condition in the `config` match arm to handle `args.len() == 4 && args[2] == "--global"`.
  - This condition checks for the pattern `snap config <key> --global <value>` and correctly parses it with `is_global: true`.
  - Fixed the reproducer test to properly call `parse_args(&args[1..])` instead of `parse_args(&args)` (the parser expects CLI tokens without argv[0]).
  - Added permanent unit regression test `test_regression_bug_008_config_flag_after_key_supported` in `rust/src/cli/args.rs`.
  - Annotated reproducer `test_bug_008` in `rust/tests/bug_reproductions.rs` as resolved (`#[ignore]`), verifying it passes when targeted.
  - Full test suite passed (55/55 unit/property tests, 28/28 acceptance suites).
  - Detailed Walkthrough: [`docs/bugs/resolution_BUG-008_walkthrough.md`](resolution_BUG-008_walkthrough.md).


