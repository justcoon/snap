# Walkthrough — Phase 3: Operational Transformation & Deterministic Replay Engine

Phase 3 of [plan.md](../../plan.md) is complete. The pairwise text Operational Transformation (OT) table, deterministic topological patch replay scheduler, whole-patch namespace conflict resolution (`namespace-wins`), path-level conflict policies (`delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins`), and full repository graph validator have been implemented and verified.

## Changes Made

### 1. Operational Transformation (OT) Engine ([`src/core/ot.rs`](../../rust/src/core/ot.rs))
- **`transform_edit`**:
  - Implemented pairwise transformation table according to `SPEC.md §6.3`.
  - Processed `Q insert` with strict priority over concurrent `P insert`, generating `Retain(length(Q insert))`.
  - Implemented numeric count splitting for `Retain` and `Delete` operations consuming shared base tokens.
  - Handled deduplication of concurrent deletes (`P delete` + `Q delete` consumes base tokens and emits nothing).
  - Coalesced adjacent operations of the same kind.

### 2. Deterministic Replay Engine ([`src/core/replay.rs`](../../rust/src/core/replay.rs))
- **`FileTree`**:
  - In-memory path/byte map `BTreeMap<String, Vec<u8>>`.
- **`ResolutionWarning`**:
  - Auto-resolution warning fact `(<path>, <reason>)` sorted lexicographically by path ascending, then reason ascending.
- **`materialize_version`**:
  - Selects the causal patch closure for target version $V$ where $n \le V[c]$ (§6.1).
  - Iteratively schedules ready patches using **Snap total order** of their result versions, then author ID, then numeric revision.
  - Implemented whole-patch **namespace conflict resolution**: detects conflicting ancestor/descendant paths against $C'$, marks conflicting current paths for removal with `namespace-wins`, and installs incoming paths as authored results.
  - Implemented path-level conflict policies:
    - Content equality shortcuts (`B == C` direct application; `C == T` no-op).
    - Three-way text OT integration (`diff(B, C)` + `transform_edit(P, Q)`).
    - Whole-file conflict policies: `delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins`.

### 3. Repository Graph Validation ([`src/core/validation.rs`](../../rust/src/core/validation.rs))
- **`validate_repository`**:
  - Verifies format == 1.
  - Enforces canonical patch sorting (author ascending, then revision ascending).
  - Enforces contiguous serial contributor revisions ($1, 2, \dots, N$ per contributor).
  - Enforces dot uniqueness (differing payloads for the same dot report corruption).
  - Verifies complete causal base closure (every dot in any patch base is present in history).
  - Verifies $revision = base[author] + 1$.
  - Checks for absence of unreachable patches outside the frontier's causal closure.
  - Validates every change against its exact materialized base tree (creation requires absent path; edit/replacement/deletion requires present path; changes must alter bytes).
  - Verifies successful deterministic replay of the declared frontier.

### 4. Module Exports ([`src/core/mod.rs`](../../rust/src/core/mod.rs))
- Re-exported `ot`, `replay`, and `validation` types and functions.

---

## Verification Results

### Automated Tests
1. **Unit & Integration Tests (`cargo test`):**
   ```text
   running 23 tests
   test core::diff::tests::test_scenario_c1_token_splitter_boundary_behavior ... ok
   test core::patch::tests::test_tracked_path_validation ... ok
   test core::diff::tests::test_scenario_c2_diff_recurrence_and_deletion_on_tie ... ok
   test core::ot::tests::test_scenario_d2_three_way_text_ot_merge ... ok
   test core::ot::tests::test_scenario_d1_pairwise_ot_table ... ok
   test core::validation::tests::test_scenario_b2_patch_continuity_and_serial_contributor ... ok
   test core::validation::tests::test_scenario_b3_dot_collision_detection ... ok
   test core::patch::tests::test_scenario_b1_json_strictness_and_unknown_field_rejection ... ok
   test core::replay::tests::test_scenario_e2_path_level_conflict_winner_rules ... ok
   test core::version::tests::test_json_serde_roundtrip ... ok
   test core::version::tests::test_scenario_a1_contributor_id_syntax_and_validation ... ok
   test core::replay::tests::test_scenario_e1_namespace_conflict_resolution ... ok
   test core::version::tests::test_scenario_a3_four_way_causal_comparison_matrix ... ok
   test core::version::tests::test_scenario_a2_version_string_canonical_parser_and_formatter ... ok
   test core::version::tests::test_scenario_a4_snap_total_order_resolution ... ok
   test core::patch::tests::test_golden_repository_serialization ... ok
   test core::version::tests::test_vector_clock_join ... ok
   test core::version::property_tests::prop_version_canonical_string_roundtrip ... ok
   test core::diff::property_tests::prop_diff_apply_roundtrip ... ok
   test core::version::property_tests::prop_snap_order_extends_causal_order ... ok
   test core::version::property_tests::prop_causal_cmp_antisymmetry ... ok
   test core::version::property_tests::prop_snap_order_is_total ... ok
   test core::version::property_tests::prop_join_properties ... ok

   test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
   ```

2. **Linter & Code Formatter:**
   ```bash
   cargo fmt --check && cargo clippy --all-targets
   ```
   Passes cleanly with 0 warnings.

3. **Subprocess / Binary Check:**
   ```bash
   ./run --lang rust --version
   ```
   Outputs `snap 0.1.0`.

---

## Plan vs. Implementation Discrepancy Check
- **Planned Scope:**
  - Implement pairwise text OT stream engine.
  - Implement topological patch scheduler by base closure and Snap order.
  - Implement namespace collision resolution (`namespace-wins`) and path-level conflict rules (`delete-wins`, `later-create-wins`, `later-put-wins`, `put-wins`).
  - Implement full repository validation (acyclicity, dot uniqueness, serial continuity, base tree validity).
- **Implemented Scope:**
  - All planned components and algorithms were implemented and verified with unit and integration tests.
- **Deviations / Adjustments:** None in architecture or behavior. In code quality: Conducted an audit of `.unwrap()` and `unreachable!()` across the codebase, replacing remaining occurrences in production code (`validation.rs`, `patch.rs`, `ot.rs`) with safe pattern matching and structured error propagation to guarantee panic-free production execution per `rust/AGENTS.md`.
