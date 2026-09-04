# Phase 2: Repository Data Model & Canonical Text Diff

Phase 2 establishes the data structures and serialization invariants for Snap's repository envelope, authored patches, and change operations, alongside the deterministic LF-preserving text tokenizer and canonical dynamic programming diff recurrence.

## Objectives & Invariants

- **Strict JSON Schema & Duplicate Key Rejection:** Repository JSON serialization will use exact two-space indentation and trailing LF. Deserialization will strictly reject unknown fields, non-integer numbers (e.g. floats like `1.0`), duplicate JSON keys, and invalid paths.
- **Canonical Dynamic Programming Diff:** The diff engine strictly follows the specification's recurrence relation `D(i, j)` with the `D(i + 1, j) <= D(i, j + 1)` deletion-on-tie rule, guaranteeing bit-for-bit identical edit scripts across all implementations.

## Proposed Changes

### Core Domain Subsystem
- **`src/core/patch.rs`**:
  - `TextEditOp`: `Retain(u64)`, `Delete(u64)`, `Insert(Vec<String>)`.
  - `Change`: `Text { path, edit }`, `Put { path, content }`, `Delete { path }`.
  - `Patch`: `author`, `revision`, `base`, `message`, `changes`. Invariant: `revision == base.get(&author) + 1`.
  - `Repository`: `format: 1`, `frontier: Version`, `patches: Vec<Patch>`. Custom strict JSON parser rejecting duplicate keys and floats.
  - `validate_tracked_path`: Enforce non-empty relative UTF-8 paths without control characters, backslashes, empty/dot segments, or `.snap` prefix.
- **`src/core/diff.rs`**:
  - `tokenize_text` & `is_text`: LF-preserving tokenization, binary file rejection (NUL bytes / invalid UTF-8).
  - `diff_tokens`: Canonical DP recurrence `D(i, j)` with deletion-on-tie prioritization and adjacent operation coalescing.
  - `apply_edit`: Application of edit scripts with complete token consumption and result canonical invariant validation.
- **`src/core/mod.rs`**:
  - Re-export new types and functions.

### Tests
- **Unit Tests**: Scenarios B.1, C.1, C.2 from `test-scenarios.md`.
- **Property-based Tests**: Generative round-trip diff application (`apply_edit(a, diff_tokens(a, b)) == b`), operation coalescing, token count invariants.
