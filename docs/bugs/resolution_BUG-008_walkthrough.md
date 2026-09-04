# Bug Resolution Walkthrough: BUG-008

## Executive Summary
- **Bug ID:** `BUG-008`
- **Title:** `snap config` does not support `--global` flag after the key
- **Affected Subsystems:**
  - CLI Argument Parsing ([`rust/src/cli/args.rs`](../../rust/src/cli/args.rs))
- **Status:** 🟢 `FIXED / PASSING`
- **Reproducer:** [`rust/tests/bug_reproductions.rs:test_bug_008_config_flag_after_key_supported`](../../rust/tests/bug_reproductions.rs)

---

## Detailed Root Cause & Remediation

### Violated Contract
> SPEC §7.2:
> "`snap config [--global] contributor.id <id>` ... The `--global` flag may precede or follow the key:
> `snap config contributor.id --global <id>` is identical to the form shown above."

### Root Cause
1. In `rust/src/cli/args.rs`, the `parse_args` function only handled the case where `--global` precedes the key (`snap config --global contributor.id <value>`).
2. The parser did not handle the case where `--global` follows the key (`snap config contributor.id --global <value>`), causing it to return `ParseError::InvalidCommandOrArguments`.
3. This violated the SPEC requirement that both flag positions should be equivalent.

### Code Changes Applied
1. **[`rust/src/cli/args.rs`](../../rust/src/cli/args.rs):**
   - Added a new condition in the `config` match arm to handle `args.len() == 4 && args[2] == "--global"`.
   - This condition checks for the pattern `snap config <key> --global <value>` and correctly parses it with `is_global: true`.
2. **[`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs):**
   - Fixed the test to properly call `parse_args(&args[1..])` instead of `parse_args(&args)` (the parser expects CLI tokens without argv[0]).
   - Annotated the test with `#[ignore = "Resolved in BUG-008..."]` to mark it as resolved in the burndown backlog.
3. **[`rust/src/cli/args.rs`](../../rust/src/cli/args.rs):**
   - Added permanent regression test `test_regression_bug_008_config_flag_after_key_supported` to the module's test suite.

---

## Red-to-Green Reproducer Evidence

### Before Fix:
```text
running 1 test
test test_bug_008_config_flag_after_key_supported ... FAILED

failures:
---- test_bug_008_config_flag_after_key_supported stdout ----
thread panicked at tests/bug_reproductions.rs:432:5:
Expected 'snap config contributor.id --global alice@example.com' to succeed, but got: Err(InvalidCommandOrArguments)
```

### After Fix:
```text
running 1 test
test test_bug_008_config_flag_after_key_supported ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s
```

---

## Full Regression Verification & Dual-Test Strategy

1. **Permanent Subsystem Regression Test:**
   - Added `test_regression_bug_008_config_flag_after_key_supported` to [`rust/src/cli/args.rs`](../../rust/src/cli/args.rs).
   - Runs automatically as part of standard `cargo test` (now 55 passed; 0 failed).
2. **Active Burndown Annotation:**
   - Marked `test_bug_008` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs) with `#[ignore = "Resolved in BUG-008..."]`.
   - Explicit execution: `cargo test --test bug_reproductions -- --ignored test_bug_008` → **`ok`**.
   - Backlog burndown execution: `cargo test --test bug_reproductions` → lists `BUG-008` as resolved (`ignored`) while tracking remaining open defects (BUG-009, BUG-010).
3. **Static Analysis & Formatting:**
   - `cargo check`: Clean.
   - `cargo clippy --all-targets`: Clean (pre-existing warnings unrelated to this fix).
   - `cargo fmt --check`: Clean.
4. **Shared Acceptance Suite:**
   - `./verify --lang rust`: 28/28 test suites passed (100% compliance in 22.3s).

---

## Bug Fix Discrepancy Check

- **Bug ID & Title:** `BUG-008`: `snap config` does not support `--global` flag after the key
- **Identified Defect:** Missing parsing logic for `snap config <key> --global <value>` pattern in `parse_args`.
- **Remediation Applied:** Added conditional branch to handle `--global` flag after the key, fixed reproducer test to properly exclude argv[0], added permanent unit regression test in `args.rs`, and annotated `test_bug_008` in `bug_reproductions.rs`.
- **Failing Reproducer Status:** Confirmed `PASSED` in `cargo test --test bug_reproductions -- --ignored test_bug_008`.
- **Regression Verification:** Confirmed 28/28 passed in `./verify --lang rust` and all 55 unit/property tests passed.
- **Unintended Side-Effects:** None: Pure argument parsing extension strictly adhering to SPEC §7.2.
