# Bug Resolution Walkthrough: BUG-006

## Executive Summary
- **Bug ID:** `BUG-006`
- **Title:** `snap revert` emits invalid empty `TextEditOp::Insert([])` when restoring an empty text file
- **Subsystem:** CLI Revert Command (`rust/src/cli/commands/revert.rs`)
- **SPEC Clause Violated:** SPEC §4.4 ("`{"insert": [s...]}` inserts one or more nonempty text tokens... An empty script is valid only when creating an empty text file."), SPEC §7.7 (`snap revert <version>`)
- **Reproducer:** `test_bug_006_revert_empty_text_file_from_absent` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs)
- **Status:** 🟢 `FIXED`

---

## Root Cause Analysis
In `rust/src/cli/commands/revert.rs`, when generating patch changes transforming `current_tree` into `target_tree`, the absent-to-present transition `(None, Some(new_bytes))` executed:
```rust
if is_text(new_bytes) {
    let tokens = tokenize_text(new_bytes)?;
    changes.push(Change::Text {
        path: path.to_string(),
        edit: vec![TextEditOp::Insert(tokens)],
    });
}
```
When `new_bytes` is empty (`b""`), `tokenize_text(b"")` yields an empty vector `vec![]`. Wrapping `tokens` unconditionally into `TextEditOp::Insert(tokens)` created `TextEditOp::Insert(vec![])`.

During patch replay and validation (`validate_repository(&new_repo)`), empty insert operations are rejected by `DiffError::EmptyInsert` / `PatchError::InvalidChange("insert operation cannot be empty")`. As a result, running `snap revert` back to any commit creating or restoring an empty text file failed with exit code 1.

Per SPEC §4.4:
> "An empty script is valid only when creating an empty text file."

The edit script for creating an empty text file from absent must be empty (`edit: vec![]`), not `vec![TextEditOp::Insert(vec![])]`.

---

## Changes Made

### 1. Extracted Pure `compute_revert_changes` with Empty File Fix
**File:** [`rust/src/cli/commands/revert.rs`](../../rust/src/cli/commands/revert.rs)
- Extracted `compute_revert_changes(current_tree: &FileTree, target_tree: &FileTree) -> Result<Vec<Change>, CliError>`.
- In the `(None, Some(new_bytes))` branch, inspected `tokens.is_empty()`:
  ```rust
  let edit = if tokens.is_empty() {
      Vec::new()
  } else {
      vec![TextEditOp::Insert(tokens)]
  };
  ```
  Generating an empty edit script (`vec![]`) when restoring an empty text file.

### 2. Added Permanent Subsystem Regression Test
**File:** [`rust/src/cli/commands/revert.rs`](../../rust/src/cli/commands/revert.rs)
- Added `test_regression_bug_006_revert_empty_text_file` verifying that transforming an empty tree to a tree with an empty text file produces `Change::Text { path, edit: vec![] }`.

---

## Verification & Active Burndown

### 1. Failing Reproducer to Passing Transition
```console
$ cargo test --test bug_reproductions -- --ignored test_bug_006
running 1 test
test test_bug_006_revert_empty_text_file_from_absent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.46s
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
test test_bug_007_http_client_content_length_truncation_rejected ... FAILED

test result: FAILED. 0 passed; 1 failed; 6 ignored; finished in 0.00s
```

### 3. Unit & Property Tests
```console
$ cargo test --lib
test result: ok. 52 passed; 0 failed; 0 ignored; finished in 0.21s
```

---

## Bug Fix Discrepancy Check
- **Bug ID & Title:** `BUG-006`: `snap revert` emits invalid empty `TextEditOp::Insert([])` when restoring an empty text file
- **Identified Defect:** `cmd_revert` unconditionally constructed `TextEditOp::Insert(tokens)` where `tokens` is empty for `b""`, triggering `DiffError::EmptyInsert` on replay validation.
- **Remediation Applied:** Generated an empty edit script (`edit: vec![]`) when `tokens.is_empty()`, matching SPEC §4.4.
- **Failing Reproducer Status:** Confirmed `test_bug_006_revert_empty_text_file_from_absent` now passes cleanly (`1 passed`).
- **Regression Verification:** All 52 in-process unit/property tests and all 28 acceptance test suites pass with 100% compliance.
