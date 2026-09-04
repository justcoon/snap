# Bug Resolution Walkthrough: BUG-010

## Executive Summary
- **Bug ID:** `BUG-010`
- **Title:** `validate_repository` skips text creation validation when base is absent
- **Affected Subsystems:**
  - Repository Graph & Invariant Validation ([`rust/src/core/validation.rs`](../../rust/src/core/validation.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_010_validate_repository_text_creation_from_absent_validation`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> SPEC §4.3:
> "A text or put creation requires the path to be absent in the patch's exact base tree... A change that does not alter path existence or bytes is invalid, except that an empty text edit may create an empty file."
> SPEC §4.4:
> "The script MUST consume the complete old token sequence; there is no implicit trailing retain... An empty script is valid only when creating an empty text file."

### Root Cause
1. In `rust/src/core/validation.rs`, step 4 of `validate_repository` validates `Change::Text` operations against the base tree.
2. When `base_bytes` is `None` (path absent in base tree, i.e., text creation), the validation logic only checked the case where the path exists (`if let Some(base_val) = base_bytes`).
3. The `else` branch for text creation was completely missing, so invalid text creation operations like `Retain(5)` or `Delete(3)` passed validation even though they cannot consume tokens from an absent (empty) file.
4. This violated SPEC §4.4 which requires that text edit scripts must only contain `Insert` operations (or be empty) when creating a new file.

### Code Changes Applied
1. **[`rust/src/core/validation.rs`](../../rust/src/core/validation.rs):**
   - Added an `else` branch in the `Change::Text` validation to handle text creation (when `base_bytes` is `None`).
   - The new validation iterates through all edit operations and rejects:
     - `Retain` operations: Cannot consume tokens from an absent file.
     - `Delete` operations: Cannot remove tokens from an absent file.
     - Only `Insert` operations (or empty scripts) are valid for text creation per SPEC §4.4.
   - Returns `ValidationError::ChangeInvalidAgainstBaseTree` with appropriate error messages for invalid operations.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_010_validate_repository_text_creation_from_absent_validation ... FAILED

failures:
---- test_bug_010_validate_repository_text_creation_from_absent_validation stdout ----
thread 'test_bug_010_validate_repository_text_creation_from_absent_validation' panicked at tests/bug_reproductions.rs:529:5:
res is Err(ReplayFailed(TextApplicationFailed("retain exceeds available tokens: available 0, requested 5: consumes beyond old content")))
```

### After Fix:
```text
running 1 test
test test_bug_010_validate_repository_text_creation_from_absent_validation ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_010_text_creation_from_absent_validation` to [`rust/src/core/validation.rs`](../../rust/src/core/validation.rs).
   - Runs automatically as part of standard `cargo test` (now 57 passed; 0 failed).
2. **Active Burndown Annotation:**
   - Marked `test_bug_010` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs) with `#[ignore = "Resolved in BUG-010..."]`.
   - Explicit execution: `cargo test --test bug_reproductions -- --ignored test_bug_010` → **`ok`**.
   - Backlog burndown execution: `cargo test --test bug_reproductions` → lists `BUG-010` as resolved (`ignored`) while tracking remaining open defects (none remaining).
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean (pre-existing warnings unrelated to this fix).
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance in 22.2s).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-010`: `validate_repository` skips text creation validation when base is absent
- **Identified Defect:** Missing validation logic for text creation operations (when path is absent in base tree). Invalid operations like `Retain` and `Delete` were not rejected.
- **Remediation Applied:** Added `else` branch in `Change::Text` validation to check that text creation operations only contain `Insert` operations (or are empty), rejecting `Retain` and `Delete` operations per SPEC §4.4. Added permanent unit regression test in `validation.rs` and annotated reproducer in `bug_reproductions.rs`.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_010`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 57 unit/property tests passed.
- **Unintended Side-Effects:** None: Pure invariant enforcement strictly adhering to SPEC §4.3 and §4.4.
