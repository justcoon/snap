# Bug Resolution Walkthrough: BUG-002

## Executive Summary
- **Bug ID:** `BUG-002`
- **Title:** `validate_repository` accepts patches containing duplicate change paths
- **Affected Subsystems:**
  - Repository Graph & Invariant Validation ([`rust/src/core/validation.rs`](../../rust/src/core/validation.rs))
  - Authored Patch Validation ([`rust/src/core/patch.rs`](../../rust/src/core/patch.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_002_validate_repository_accepts_duplicate_change_paths`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> SPEC §4.2:
> "`changes` is nonempty, sorted by path, and contains at most one change per path."

### Root Cause
1. In `rust/src/core/validation.rs`, `validate_repository(&repo)` verified repository-level properties (contiguous revisions, dot collision check, frontier reachability, base dependency closure, and change validity against materialized base trees), but did not invoke individual patch validation (`patch.validate()`).
2. While `Change` paths were verified against materialized trees, duplicate change paths within a single patch were not rejected during repository validation if each individual change operation succeeded against the base tree.
3. Consequently, repositories containing patches with duplicate change paths for the same file path bypassed repository validation.

### Code Changes Applied
1. **[`rust/src/core/validation.rs`](../../rust/src/core/validation.rs):**
   - Individual patch structural validation `patch.validate().map_err(ValidationError::InvalidPatch)?;` is executed on every patch in `repo.patches`.
   - Calling `patch.validate()` enforces:
     - `PatchError::DuplicateChangePath(p)` if multiple changes touch the same path.
     - `PatchError::UnsortedChangePaths` if change paths are not strictly lexicographically sorted.
     - `PatchError::EmptyChanges` if the changes array is empty.
     - `PatchError::TreePathsConflict` if active paths violate segment prefix-freedom.
   - Added permanent subsystem unit regression test: `test_regression_bug_002_duplicate_change_paths_rejected`.
2. **[`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs):**
   - Verified that `test_bug_002_validate_repository_accepts_duplicate_change_paths` passes.
   - Annotated with `#[ignore = "Resolved in BUG-002 (see docs/bugs/resolution_BUG-002_walkthrough.md)"]`.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_002_validate_repository_accepts_duplicate_change_paths ... FAILED

failures:
---- test_bug_002_validate_repository_accepts_duplicate_change_paths stdout ----
thread panicked at tests/bug_reproductions.rs:73:5:
Expected validate_repository to reject patch with duplicate change paths, but got Ok(())
```

### After Fix:
```text
running 1 test
test test_bug_002_validate_repository_accepts_duplicate_change_paths ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_002_duplicate_change_paths_rejected` to [`rust/src/core/validation.rs`](../../rust/src/core/validation.rs).
   - Confirmed passing in `cargo test --lib test_regression_bug_002`.
   - In-process suite: 48 passed; 0 failed.
2. **Active Burndown Annotation:**
   - Annotated `test_bug_002` with `#[ignore = "Resolved in BUG-002..."]` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs).
   - Explicit targeting: `cargo test --test bug_reproductions -- --ignored test_bug_002` $\to$ **`ok`**.
   - Backlog burndown: `cargo test --test bug_reproductions` $\to$ displays 2 ignored (`BUG-001`, `BUG-002`) and tracks remaining open bugs (`BUG-003`, `BUG-004`).
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean.
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-002`: `validate_repository` accepts patches containing duplicate change paths
- **Identified Defect:** Missing invocation of `patch.validate()` during `validate_repository` allowed patches with duplicate change paths to pass repository validation.
- **Remediation Applied:** Enforced `patch.validate()` in `validate_repository`, added permanent unit regression test `test_regression_bug_002_duplicate_change_paths_rejected` in `rust/src/core/validation.rs`, and marked `test_bug_002` as resolved in the reproducer backlog.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_002`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 48 unit/property tests passed.
- **Unintended Side-Effects:** None: Invariant strictly required by SPEC §4.2.
