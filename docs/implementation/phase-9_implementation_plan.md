# Phase 9 Implementation Plan: End-to-End Hardening & Acceptance Suite Conformance

## 1. Objectives & Scope
Implement Phase 9 of the Snap implementation roadmap according to [`plan.md`](../../plan.md) (§7 Phase 9), [`SPEC.md`](../../SPEC.md), and [`test-scenarios.md`](../../test-scenarios.md) (Domain A through Domain L, §3 Property Tests, and §4 Platform Scenarios).

Phase 9 represents the final hardening, comprehensive verification, and acceptance conformance gate for the Rust implementation of Snap:
1. **Acceptance Suite 100% Pass Conformance**:
   - Verify all 28 language-neutral YAML acceptance suites pass cleanly with `./verify --lang rust`.
   - Ensure deterministic, byte-stable plain outputs and exact exit codes across all commands.
2. **Hardening & Environment Constants**:
   - Unify remaining environment variable names into canonical constants:
     - `ENV_SNAP_COLOR = "SNAP_COLOR"` (in `src/presentation/mod.rs`)
     - `ENV_NO_COLOR = "NO_COLOR"` (in `src/presentation/mod.rs`)
     - `ENV_HOME = "HOME"` (in `src/config/mod.rs`)
3. **Property-Based Testing Expansion (`proptest`)**:
   - Implement Property Test 3 from `test-scenarios.md` §3: *Operational Transformation Concurrency Invariants* (in `src/core/ot.rs` `property_tests`).
     - Base length parity: Transformed edit scripts preserve target length invariants.
     - Insertion preservation: Inserted tokens are preserved across concurrent edits.
     - No duplicate deletions or index drift when both sides delete identical tokens.
   - Implement Property Test 4 from `test-scenarios.md` §3: *Replay Determinism on Random Causal Patch DAGs* (in `src/core/replay.rs` `property_tests`).
     - Permutation invariance: Any valid topological order of a patch set produces the exact same file tree and warning list.
     - Prefix freedom: The materialized tree never contains conflicting file/directory transitions.
4. **Static Verification & Linting**:
   - Run `cargo clippy --all-targets -- -D warnings` with zero warnings allowed.
   - Run `cargo fmt --check` with zero formatting differences.
   - Verify `cargo test` executes all in-memory unit and property tests successfully.

---

## 2. Proposed Changes & Subsystems

### A. Environment Variable Constants
- [`rust/src/presentation/mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/presentation/mod.rs):
  - Add `pub const ENV_SNAP_COLOR: &str = "SNAP_COLOR";`
  - Add `pub const ENV_NO_COLOR: &str = "NO_COLOR";`
  - Use in `current_stream_modes()`.
- [`rust/src/config/mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/config/mod.rs):
  - Add `pub const ENV_HOME: &str = "HOME";`
  - Use in `resolve_contributor_id()`.
- [`rust/src/cli/commands/config.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/cli/commands/config.rs):
  - Use `ENV_HOME` from `crate::config`.

### B. Property-Based Testing Expansion
- [`rust/src/core/ot.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/core/ot.rs):
  - Add `mod property_tests` powered by `proptest`:
    - Generator for base token sequences and pairs of independent valid edit scripts.
    - Assert `transform_edit(p, q)` satisfies base length parity and token conservation.
- [`rust/src/core/replay.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/core/replay.rs):
  - Add `mod property_tests` powered by `proptest`:
    - Generator for acyclic causal patch DAGs.
    - Assert topological order permutations produce identical materialized `FileTree` results.

---

## 3. Verification Hierarchy

1. **Static Analysis & Formatting**:
   ```bash
   cd rust
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   ```
2. **In-Process Unit & Property Tests**:
   ```bash
   cargo test
   ```
3. **Direct CLI Execution**:
   ```bash
   cd ..
   ./run --lang rust --version
   ```
4. **Full Shared Acceptance Suite**:
   ```bash
   ./verify --lang rust
   ```
   Ensuring all 28 acceptance test suites pass 100%.
