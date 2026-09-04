# Bug Resolution Walkthrough: BUG-005

## Executive Summary
- **Bug ID:** `BUG-005`
- **Title:** `snap log` traverses `repo.patches` in reverse author order instead of reverse canonical integration order
- **Subsystem:** Core Replay & CLI Log Command (`rust/src/core/replay.rs`, `rust/src/cli/commands/log.rs`)
- **SPEC Clause Violated:** SPEC §7.4 ("Prints patches in reverse canonical integration order, one tab-separated line each")
- **Reproducer:** `test_bug_005_log_reverse_canonical_integration_order` in [`rust/tests/bug_reproductions.rs`](../../rust/tests/bug_reproductions.rs)
- **Status:** 🟢 `FIXED`

---

## Root Cause Analysis
In `rust/src/cli/commands/log.rs`, log records were generated using:
```rust
repo.patches.iter().rev()
```
According to SPEC §4.1, `repo.patches` is stored sorted lexicographically by contributor ID (`author`), then numeric `revision`.

When multiple contributors collaborate in a repository, the lexicographical ordering of their IDs often differs from the causal integration order of their commits. For example, if contributor `Bob` (`bob@example.com`) authors root commit 1, and contributor `Alice` (`alice@example.com`) authors child commit 1 with `(bob@example.com->1)` as its base:
- In `repo.patches`, Alice appears at index 0 and Bob appears at index 1 (sorted by author string).
- Reversing `repo.patches` yields `[Bob, Alice]`.
- As a result, `snap log` printed Bob's root commit first and Alice's dependent child commit second, inverting causal history.

Per SPEC §6.1 and §7.4, `snap log` must compute the canonical integration sequence (least ready patch by Snap order of result versions, author, revision starting from the empty tree) and traverse the sequence in exact reverse.

---

## Changes Made

### 1. Extracted Pure `canonical_integration_order` in Replay Engine
**File:** [`rust/src/core/replay.rs`](../../rust/src/core/replay.rs)
- Implemented `canonical_integration_order<'a>(patches: &'a [Patch], target: &Version) -> Result<Vec<&'a Patch>, ReplayError>` implementing SPEC §6.1's exact integration tie-breaking rules.
- Refactored `materialize_version` to consume `canonical_integration_order`, eliminating duplication and ensuring consistent integration behavior across replay and presentation.

### 2. Updated `cmd_log` to Traverse Reverse Canonical Integration Order
**File:** [`rust/src/cli/commands/log.rs`](../../rust/src/cli/commands/log.rs)
- Replaced `repo.patches.iter().rev()` with:
  ```rust
  let ordered_patches =
      canonical_integration_order(&repo.patches, &repo.frontier).map_err(CliError::Replay)?;
  let entries: Vec<LogRecord> = ordered_patches
      .iter()
      .rev()
      ...
  ```

### 3. Added Permanent Subsystem Regression Test
**File:** [`rust/src/core/replay.rs`](../../rust/src/core/replay.rs)
- Added `test_regression_bug_005_canonical_integration_order` verifying that child commits dependent on parent commits author-sorted later are strictly sequenced after their dependencies in canonical integration order.

---

## Verification & Active Burndown

### 1. Failing Reproducer to Passing Transition
```console
$ cargo test --test bug_reproductions -- --ignored test_bug_005
running 1 test
test test_bug_005_log_reverse_canonical_integration_order ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.34s
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
test test_bug_007_http_client_content_length_truncation_rejected ... FAILED
test test_bug_006_revert_empty_text_file_from_absent ... FAILED

test result: FAILED. 0 passed; 2 failed; 5 ignored; finished in 0.49s
```

### 3. Unit & Property Tests
```console
$ cargo test --lib
test result: ok. 51 passed; 0 failed; 0 ignored; finished in 0.21s
```

### 4. Shared Acceptance Suite Execution
```console
$ ./verify --lang rust
snap tests — candidate=/var/folders/7f/s_dm8hkd2z78nfn6r6trdw200000gn/T/snap-rust.R7u6Ae, 28 case(s)
  ✓ init creates an empty repository 736ms
  ✓ initialization preserves files and rejects nested or existing repositories 234ms
  ✓ local and global contributor configuration have strict precedence 482ms
  ✓ commit status and log expose exact deterministic history 502ms
  ✓ diff renders canonical repeated-line edits and missing final newlines 400ms
  ✓ binary and empty files are versioned byte exactly 283ms
  ✓ revert is additive and restores file-directory transitions 533ms
  ✓ working tree scans reject symlinks and special files without mutation 299ms
  ✓ local merge converges concurrent text changes and is idempotent 652ms
  ✓ merge applies every whole-file conflict rule with sorted warnings 602ms
  ✓ canonical namespace winners replace conflicting files in both directions 812ms
  ✓ server exposes one immutable repository snapshot and exits on SIGTERM 413ms
  ✓ HTTP merge and diff use one exact validated GET without redirects 546ms
  ✓ command grammar and common failures use stable exit channels 489ms
  ✓ repository reader rejects malformed schemas histories paths and edits 590ms
  ✓ cross-repository dot collisions fail before changing local state 298ms
  ✓ concurrent creates choose the canonical later value independent of merge direction 521ms
  ✓ three-way text history converges across different merge association orders 1306ms
  ✓ CLI versions are canonical known causal frontiers 528ms
  ✓ merge refuses dirty and unsupported working trees without importing history 359ms
  ✓ vector clocks use causal closure componentwise join and canonical Snap order 664ms
  ✓ text OT covers overlapping deletes split counts insert priority and trailing inserts 1537ms
  ✓ repository validation rejects every malformed layer before mutation 756ms
  ✓ every command rejects unknown misplaced duplicate and extra arguments 986ms
  ✓ configuration versions paths and text use their exact canonical boundaries 1059ms
  ✓ local exchange preserves text bytes and malformed remotes never mutate 749ms
  ✓ patch histories require exact schemas canonical order and valid base transitions 454ms
  ✓ terminal presentation is colorful readable and explicitly controllable 1956ms

28 passed in 18743ms
```

---

## Bug Fix Discrepancy Check
- **Bug ID & Title:** `BUG-005`: `snap log` traverses `repo.patches` in reverse author order instead of reverse canonical integration order
- **Identified Defect:** `cmd_log` directly called `repo.patches.iter().rev()`, reversing `repo.patches` storage order (author, revision) rather than canonical integration order.
- **Remediation Applied:** Introduced `canonical_integration_order` in `core::replay` and updated `cmd_log` to iterate its result in reverse (`ordered_patches.iter().rev()`).
- **Failing Reproducer Status:** Confirmed `test_bug_005_log_reverse_canonical_integration_order` now passes cleanly (`1 passed`).
- **Regression Verification:** All 51 in-process unit/property tests and all 28 acceptance test suites pass with 100% compliance.
