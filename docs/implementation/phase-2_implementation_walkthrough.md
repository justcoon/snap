# Walkthrough — Phase 2: Repository Data Model & Canonical Text Diff

Phase 2 of [plan.md](../../plan.md) is complete. The repository data structures, authored patch schema, change operations, LF-preserving text tokenizer, and canonical dynamic programming diff recurrence `D(i, j)` have been implemented and validated against the requirements in [SPEC.md](../../SPEC.md) and [test-scenarios.md](../../test-scenarios.md).

## Changes Made

### 1. Repository & Patch Schema ([`src/core/patch.rs`](../../rust/src/core/patch.rs))
- **`TextEditOp`**: Strongly typed operations (`Retain`, `Delete`, `Insert`) serialized to single-key JSON objects. Invariants enforce positive safe integers, non-empty insert strings, and forbid adjacent same-kind operations.
- **`Change`**: `Text`, `Put` (standard padded RFC 4648 base64), and `Delete`. Validates tracked path syntax via `validate_tracked_path`.
- **`Patch`**: Authored patch with `revision == base.get(&author) + 1` validation, message control character constraints, and sorted unique change paths.
- **`Repository`**: Stores `format: 1`, `frontier: Version`, and `patches: Vec<Patch>`. Pretty-printed two-space JSON serializer and `validate_json_strict` scanner for duplicate JSON key and float rejection.

### 2. Tokenization & Canonical Dynamic Programming Diff ([`src/core/diff.rs`](../../rust/src/core/diff.rs))
- **`tokenize_text` & `is_text`**: LF-preserving token splitting; binary rejection on NUL byte (`\0`) or non-UTF-8 bytes.
- **`diff_tokens`**: Canonical DP recurrence `D(i, j)` with deletion-on-tie ($D(i+1, j) \le D(i, j+1)$) and operation coalescing.
- **`apply_edit`**: Verified exact token sequence consumption and result token invariants.

### 3. Verification Results
- **Unit & Property Tests (`cargo test`)**: 17 passed; 0 failed.
- **Linter & Formatter (`cargo clippy`, `cargo fmt --check`)**: 0 warnings.
- **CLI Subprocess (`./run --lang rust --version`)**: Outputs `snap 0.1.0`.

---

## Plan vs. Implementation Discrepancy Check
- **Planned Scope:**
  - Define strongly typed structures for patches, edit scripts, `put`, and `delete` changes.
  - Implement JSON serialization with strict validation (two-space indentation, unique keys, safe integer limits).
  - Implement UTF-8 tokenization with newline retention.
  - Implement canonical dynamic programming diff recurrence `D(i, j)` with deletion-on-tie.
- **Implemented Scope:**
  - Fully implemented `TextEditOp`, `Change`, `Patch`, `Repository`, `validate_tracked_path`, `tokenize_text`, `diff_tokens`, and `apply_edit`.
- **Deviations / Adjustments:**
  - **Custom `validate_json_strict` Scanner:** To strictly enforce the requirement of rejecting duplicate JSON object keys (such as duplicate keys at nested levels) and floating-point numbers (`1.0`) without pulling in heavy or non-standard external parser crates, a zero-dependency streaming JSON validator was implemented in `src/core/patch.rs`. This directly satisfies Scenario B.1 and test harness expectations (`tests/25-config-version-path-boundaries.yaml`).
