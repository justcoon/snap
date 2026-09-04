# Phase 4 Implementation Walkthrough: Filesystem Scanner, Path Validation & Materializer

## Overview

Phase 4 implemented the complete filesystem subsystem (`rust/src/fs/`) for Snap, fulfilling all objectives of [`plan.md`](../../plan.md) and [`SPEC.md`](../../SPEC.md). This layer provides safe, deterministic, and atomic operations covering tracked path validation, prefix-freedom enforcement, working tree scanning with symlink rejection, and atomic working tree materialization with failure rollback safety.

---

## Changes Implemented

### 1. Tracked Path & Segment Prefix-Freedom (`rust/src/fs/paths.rs`)
- **Path Syntax Validation (`validate_tracked_path`):** Validates UTF-8 relative paths, strictly rejecting empty paths, ASCII control characters (including NUL), backslashes, empty segments, `.` or `..` segments, and `.snap` first-segment prefixes.
- **Segment Prefix-Freedom (`check_prefix_free`):** Enforces that no tracked file path is a proper ancestor segment of another path (e.g. if `a` is a file, `a/b` cannot be present). Uses a segment-prefix set check that avoids ASCII sorting pitfalls (such as `a-b` sorting between `a` and `a/b`).
- **Path Ordering:** Standard byte-wise unsigned UTF-8 lexicographic ordering.

### 2. Working Tree Scanner & Status Diff (`rust/src/fs/scanner.rs`)
- **Symlink & Unsupported Entry Rejection (`scan_working_tree`):**
  - Traverses directory hierarchy using `fs::symlink_metadata` (never following symlinks).
  - Immediately terminates with `ScanError::UnsupportedEntry(rel_path)` on symlinks, FIFOs, sockets, block/char devices.
  - Automatically skips the root `.snap/` directory and ignores empty directories.
  - Returns a sorted map of normalized relative paths to raw byte contents (`BTreeMap<String, Vec<u8>>`).
- **Working Tree Diffing (`diff_working_tree`):**
  - Compares working tree against a reference target tree (`current_tree`).
  - Correctly reports `Added` (`A`), `Modified` (`M`), and `Deleted` (`D`) changes in path-sorted order.
  - Recognizes unchanged byte contents as clean even if filesystem timestamps (`mtime`) have changed.

### 3. Atomic Materialization & File Replacement (`rust/src/fs/materializer.rs`)
- **Atomic File Replacement (`atomic_replace_file`):**
  - Creates a temporary file in the same parent directory (`.{filename}.tmp.{pid}.{timestamp}`).
  - Writes and flushes byte content.
  - Atomically renames temporary file over destination path via POSIX `rename`.
  - Cleans up temporary file upon write or rename failure, leaving destination intact.
- **Repository Metadata Serializer (`write_repository_atomic`):**
  - Atomically writes `.snap/repository.json` with two-space indented formatting and trailing newline.
- **Working Tree Materializer (`materialize_tree`):**
  - Deletes paths removed in target tree.
  - Removes obstructing regular files that block directories required by target files.
  - Creates required parent directories.
  - Writes/updates target files.
  - Recursively removes newly empty intermediate directories.

### 4. Module Exports & Integration (`rust/src/fs/mod.rs` & `rust/src/main.rs`)
- Clean re-exports of core types.
- Exposed `pub mod fs;` in `main.rs`.

---

## Verification Results

### 1. In-Process Unit & Integration Tests
```bash
cargo test
```
```text
running 29 tests
test core::diff::tests::test_scenario_c1_token_splitter_boundary_behavior ... ok
test core::patch::tests::test_tracked_path_validation ... ok
test core::diff::tests::test_scenario_c2_diff_recurrence_and_deletion_on_tie ... ok
test core::ot::tests::test_scenario_d2_three_way_text_ot_merge ... ok
test core::ot::tests::test_scenario_d1_pairwise_ot_table ... ok
test core::validation::tests::test_scenario_b3_dot_collision_detection ... ok
test core::validation::tests::test_scenario_b2_patch_continuity_and_serial_contributor ... ok
test core::patch::tests::test_scenario_b1_json_strictness_and_unknown_field_rejection ... ok
test core::version::tests::test_json_serde_roundtrip ... ok
test core::replay::tests::test_scenario_e2_path_level_conflict_winner_rules ... ok
test core::version::tests::test_scenario_a1_contributor_id_syntax_and_validation ... ok
test core::replay::tests::test_scenario_e1_namespace_conflict_resolution ... ok
test core::version::tests::test_scenario_a3_four_way_causal_comparison_matrix ... ok
test core::version::tests::test_scenario_a2_version_string_canonical_parser_and_formatter ... ok
test core::version::tests::test_scenario_a4_snap_total_order_resolution ... ok
test core::version::tests::test_vector_clock_join ... ok
test core::patch::tests::test_golden_repository_serialization ... ok
test fs::paths::tests::test_validate_tracked_path ... ok
test fs::paths::tests::test_check_prefix_free ... ok
test fs::scanner::tests::test_scenario_f2_working_tree_clean_vs_dirty_detection ... ok
test fs::tests::test_scenario_f1_symlink_and_special_file_rejection ... ok
test fs::materializer::tests::test_scenario_f3_atomic_metadata_replacement_safety ... ok
test fs::materializer::tests::test_materialize_tree_lifecycle ... ok
test core::version::property_tests::prop_version_canonical_string_roundtrip ... ok
test core::diff::property_tests::prop_diff_apply_roundtrip ... ok
test core::version::property_tests::prop_snap_order_extends_causal_order ... ok
test core::version::property_tests::prop_snap_order_is_total ... ok
test core::version::property_tests::prop_causal_cmp_antisymmetry ... ok
test core::version::property_tests::prop_join_properties ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
```

### 2. Static Analysis & Formatter
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```
Clean pass with 0 warnings.

### 3. Binary Check
```bash
./run --lang rust --version
```
Outputs `snap 0.1.0`.

---

## Plan vs. Implementation Discrepancy Check
- **Planned Scope:**
  - UTF-8 path validation & segment prefix-freedom invariant checking.
  - Working tree scanner with clean/dirty detection and symlink/unsupported entry rejection.
  - Atomic filesystem materialization and safe metadata replacement.
- **Implemented Scope:**
  - All planned components in `rust/src/fs/{paths,scanner,materializer,mod}.rs` were implemented and verified.
- **Deviations / Adjustments:** None: Implementation strictly adhered to the approved plan.
