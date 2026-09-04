# Bug Resolution Walkthrough: BUG-001

## Executive Summary
- **Bug ID:** `BUG-001`
- **Title:** `validate_repository` does not validate patch messages for disallowed ASCII control characters
- **Affected Subsystems:**
  - Repository Graph & Invariant Validation ([`rust/src/core/validation.rs`](../../rust/src/core/validation.rs))
  - CLI Commit Handler ([`rust/src/cli/commands/commit.rs`](../../rust/src/cli/commands/commit.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_001_validate_repository_allows_control_chars_in_patch_message`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> SPEC §4.2:
> "`message` is a nonempty UTF-8 string. It may contain tab and LF but no other ASCII control character. `snap commit` limits user-supplied messages to 4096 bytes..."

### Root Cause
1. In `rust/src/core/validation.rs`, `validate_repository(&repo)` checked sorting, dot collisions, contiguous revisions, and change validity against materialized base trees, but never called `patch.validate()` on each patch in `repo.patches`.
2. In `rust/src/cli/commands/commit.rs`, `cmd_commit` checked `message.is_empty() || message.len() > MAX_COMMIT_MESSAGE_BYTES`, but did not check for ASCII control characters other than `\t` and `\n`.
3. Consequently, repositories containing patches with invalid ASCII control characters (such as `\x01` or `\r`) passed `validate_repository`, and `snap commit` permitted creating such patches.

### Code Changes Applied
1. **[`rust/src/core/validation.rs`](../../rust/src/core/validation.rs):**
   - Imported `PatchError`.
   - Added `ValidationError::InvalidPatch(PatchError)` variant and its `Display` implementation.
   - Added `patch.validate().map_err(ValidationError::InvalidPatch)?;` to the patch validation loop in `validate_repository`.
2. **[`rust/src/cli/commands/commit.rs`](../../rust/src/cli/commands/commit.rs):**
   - Added check `message.chars().any(|c| c.is_ascii_control() && c != '\t' && c != '\n')` returning `Err(CliError::InvalidCommitMessage)` before patch creation.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_001_validate_repository_allows_control_chars_in_patch_message ... FAILED

failures:
---- test_bug_001_validate_repository_allows_control_chars_in_patch_message stdout ----
thread panicked at tests/bug_reproductions.rs:35:5:
Expected validate_repository to reject patch with control characters in message, but got Ok(())
```

### After Fix:
```text
running 1 test
test test_bug_001_validate_repository_allows_control_chars_in_patch_message ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_001_control_chars_in_message_rejected` to [`rust/src/core/validation.rs`](../../rust/src/core/validation.rs).
   - Runs automatically as part of standard `cargo test` (now 47 passed; 0 failed).
2. **Active Burndown Annotation:**
   - Marked `test_bug_001` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs) with `#[ignore = "Resolved in BUG-001..."]`.
   - Explicit execution: `cargo test --test bug_reproductions -- --ignored test_bug_001` $\to$ **`ok`**.
   - Backlog burndown execution: `cargo test --test bug_reproductions` $\to$ lists `BUG-001` as resolved (`ignored`) while tracking remaining open defects.
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean.
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance in 21.2s).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-001`: `validate_repository` does not validate patch messages for ASCII control characters
- **Identified Defect:** Missing call to `patch.validate()` inside `validate_repository` and missing control character check in `cmd_commit`.
- **Remediation Applied:** Added `ValidationError::InvalidPatch(PatchError)` to `validate_repository`, invoked `patch.validate()` on all patches, guarded `cmd_commit` against ASCII control characters, added a permanent unit regression test in `validation.rs`, and annotated `test_bug_001` in `bug_reproductions.rs`.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_001`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 47 unit/property tests passed.
- **Unintended Side-Effects:** None: Pure invariant enforcement strictly adhering to SPEC §4.2.
