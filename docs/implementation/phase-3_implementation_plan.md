# Phase 3: Operational Transformation & Deterministic Replay Engine

Phase 3 implements the core deterministic replay engine, the pairwise text Operational Transformation (OT) table, namespace conflict resolution (`namespace-wins`), path-level conflict policies, and full repository graph invariant validation.

## User Review Required

> [!IMPORTANT]
> - **Pairwise OT Stream Engine:** Conforms exactly to SPEC §6.3 table: `Q insert` priority, splitting counts, `P delete / Q delete` deduplication, and coalescing adjacent operations.
> - **Topological Patch Scheduler & Total Ordering:** Resolves patch integration order deterministically: ready patches are scheduled by Snap order of result version, then author ID, then numeric revision (§6.1).
> - **Deterministic Conflict Policies:** Enforces `namespace-wins` over conflicting ancestors/descendants, followed by path-level rules: `delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins`, maintaining sorted unique warning pairs.

## Proposed Changes

### Core Domain Subsystem

#### [NEW] [core/ot.rs](file:///Users/coon/workspace-zv/git/snap/rust/src/core/ot.rs)
- **`transform_edit(p: &[TextEditOp], q: &[TextEditOp]) -> Result<Vec<TextEditOp>, OtError>`**:
  - Pairwise operational transformation table for edit scripts consuming the same base tokens.
  - Handles count splitting between `Retain` and `Delete`.
  - Gives priority to `Q insert` over concurrent `P insert`.
  - Deduplicates concurrent deletes (`P delete` + `Q delete` consumes tokens and emits nothing).
  - Coalesces adjacent operations in the resulting script.

#### [NEW] [core/replay.rs](file:///Users/coon/workspace-zv/git/snap/rust/src/core/replay.rs)
- **`FileTree`**:
  - In-memory path-to-bytes map `BTreeMap<String, Vec<u8>>`.
  - Helper methods: `get`, `insert`, `remove`, `is_text`, `get_tokens`.
- **`ResolutionWarning`**:
  - Stores `path` and `reason` (`delete-wins`, `later-create-wins`, `later-put-wins`, `namespace-wins`, `put-wins`).
  - Sorted by `path` ascending, then `reason` ascending.
- **`ReplayEngine`**:
  - `materialize_version(patches: &[Patch], target: &Version) -> Result<(FileTree, Vec<ResolutionWarning>), ReplayError>`:
    - Selects causal patch closure for `target`.
    - Schedules ready patches using Snap total order (§6.1).
    - Integrates patches iteratively, applying:
      1. Whole-patch namespace conflict resolution (`namespace-wins`).
      2. Content equality shortcuts (`B == C` or `C == T`).
      3. Three-way text OT integration (`diff(B, C)` + `transform_edit`).
      4. Path-level conflict rules (`delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins`).
    - Returns finalized `FileTree` and deduplicated sorted warning pairs.

#### [NEW] [core/validation.rs](file:///Users/coon/workspace-zv/git/snap/rust/src/core/validation.rs)
- **`validate_repository(repo: &Repository) -> Result<(), ValidationError>`**:
  - Verifies format == 1.
  - Verifies patch ordering (author then numeric revision).
  - Verifies dot uniqueness (same dot with different content is corruption).
  - Verifies contiguous serial contributor revisions (1, 2, 3... without gaps).
  - Verifies complete causal base closure (every patch required by any base exists).
  - Verifies causality acyclicity.
  - Validates every patch change against its materialized exact base tree.
  - Validates replay of the declared frontier.

#### [MODIFY] [core/mod.rs](file:///Users/coon/workspace-zv/git/snap/rust/src/core/mod.rs)
- Re-export `ot`, `replay`, and `validation` modules.

---

### Tests

#### [NEW] Unit & Integration Tests in `ot.rs`, `replay.rs`, `validation.rs`
- **Scenario D.1: Pairwise OT Transformation Table**
  - All 6 matrix cases: P insert, Q insert, concurrent inserts at cursor, P delete + Q retain, P retain + Q delete, P delete + Q delete.
- **Scenario D.2 / tests/22-ot-matrix.yaml:**
  - Complex 3-way text merge convergence across association orders.
- **Scenario E.1: Namespace Conflict Resolution**
  - File-to-directory and directory-to-file collision convergence with `namespace-wins`.
- **Scenario E.2: Path-Level Conflict Winner Rules**
  - Validation of `delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins` with warning sorting.
- **Scenario B.2 & B.3: Serial Contributor & Dot Collisions**
  - Discontinuous revisions, missing base dependencies, duplicate dots with differing payloads.

## Verification Plan

### Automated Tests
1. Static checks:
   ```bash
   cd rust && cargo check && cargo clippy --all-targets && cargo fmt --check
   ```
2. Unit and integration tests:
   ```bash
   cargo test
   ```
3. Documentation:
   - Walkthrough and plan vs implementation discrepancy check in `docs/implementation/phase-3_implementation_walkthrough.md`.
