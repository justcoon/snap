# Bug Resolution Walkthrough: BUG-009

## Executive Summary
- **Bug ID:** `BUG-009`
- **Title:** `validate_repository` does not check that authored result trees are prefix-free
- **Affected Subsystems:**
  - Repository Graph & Invariant Validation ([`rust/src/core/validation.rs`](../../rust/src/core/validation.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_009_validate_repository_authored_result_prefix_free`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> SPEC §2:
> "Every tracked tree is prefix-free by path segment: if `a` is a file, no `a/...` path is present. This is validated for every patch's authored result and enforced during concurrent replay by §6.4."
> SPEC §4.5:
> "5. The authored result tree of every patch (the tree resulting from applying its changes to its base tree) is prefix-free by path segment."

### Root Cause
1. In `rust/src/core/validation.rs`, `validate_repository` checked `check_prefix_free` only on `patch.changes` in isolation during step 4.
2. The validation never verified that the authored result tree (applying `patch.changes` to the materialized `base_tree`) is prefix-free.
3. Consequently, a patch creating a file "dir" when "dir/file.txt" exists in the base tree was accepted, resulting in both "dir" (file) and "dir/file.txt" (nested file) coexisting in the authored tree, violating the prefix-free invariant.

### Code Changes Applied
1. **[`rust/src/core/validation.rs`](../../rust/src/core/validation.rs):**
   - Added a new validation step (step 5) that materializes each patch's authored result tree by applying its changes to its base tree.
   - For each patch, the code now:
     - Materializes the base tree via `materialize_version(&repo.patches, &patch.base)`.
     - Applies all patch changes to the base tree to construct the authored result tree.
     - Calls `crate::fs::paths::check_prefix_free` on the authored tree's paths.
     - Returns `ValidationError::ChangeInvalidAgainstBaseTree` if the authored tree is not prefix-free.
   - This ensures that patches cannot create prefix conflicts between newly created paths and existing paths in the base tree.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_009_validate_repository_authored_result_prefix_free ... FAILED

failures:
---- test_bug_009_validate_repository_authored_result_prefix_free stdout ----
thread panicked at tests/bug_reproductions.rs:490:5:
Expected validate_repository to reject patch whose authored result tree is not prefix-free, but got Ok(())
```

### After Fix:
```text
running 1 test
test test_bug_009_validate_repository_authored_result_prefix_free ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_009_authored_result_prefix_free` to [`rust/src/core/validation.rs`](../../rust/src/core/validation.rs).
   - Runs automatically as part of standard `cargo test` (now 57 passed; 0 failed).
2. **Active Burndown Annotation:**
   - Marked `test_bug_009` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs) with `#[ignore = "Resolved in BUG-009..."]`.
   - Explicit execution: `cargo test --test bug_reproductions -- --ignored test_bug_009` → **`ok`**.
   - Backlog burndown execution: `cargo test --test bug_reproductions` → lists `BUG-009` as resolved (`ignored`) while tracking remaining open defects (none remaining).
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean (pre-existing warnings unrelated to this fix).
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance in 22.2s).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-009`: `validate_repository` does not check that authored result trees are prefix-free
- **Identified Defect:** Missing validation step to verify that each patch's authored result tree (after applying changes to base tree) is prefix-free by path segment.
- **Remediation Applied:** Added step 5 to `validate_repository` that materializes each patch's authored result tree and validates prefix-freeness using `check_prefix_free`. Added permanent unit regression test in `validation.rs` and annotated reproducer in `bug_reproductions.rs`.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_009`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 57 unit/property tests passed.
- **Unintended Side-Effects:** None: Pure invariant enforcement strictly adhering to SPEC §2 and §4.5.
