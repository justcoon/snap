---
name: snap-phase-workflow
description: >-
  Executes roadmap implementation phases for Snap according to plan.md and test-scenarios.md.
  Use when asked to implement a phase from plan.md, add a subsystem, or develop next roadmap milestones.
---

# Snap Phase Implementation Workflow

This skill guides the end-to-end execution of implementation phases defined in [`plan.md`](../../../plan.md) for the Snap project, adhering strictly to the contract in [`SPEC.md`](../../../SPEC.md) and the testing requirements in [`test-scenarios.md`](../../../test-scenarios.md).

## Phase Execution Checklist

Follow these systematic steps whenever executing a phase:

### 1. Requirements Alignment & Research
- **Inspect the Phase Objectives:** Locate the target phase in `plan.md` (§7: *Phased Implementation Roadmap*) and review its objectives and verification gates.
- **Consult Test Scenarios:** Find the matching domain in `test-scenarios.md` (§2: *Detailed Test Scenarios by Domain*) to identify required unit scenarios, boundary conditions, and property tests.
- **Reference the Canonical Specification:** Review the relevant sections of `SPEC.md` to guarantee all normative requirements (MUST/MUST NOT) are satisfied.

### 2. Architecture & Module Partitioning
Maintain strict separation of concerns in `rust/src/`:
- **Pure Core (`src/core/`):** Version algebra, canonical text diff, operational transformation (OT), replay engine, and repository graph validation. Must be 100% deterministic with zero filesystem or network I/O.
- **Filesystem & State (`src/fs/`, `src/config/`):** Path validation, segment prefix-freedom, working-tree scanner, atomic materializer, and local/global configuration loader.
- **HTTP Mode (`src/http/`):** Synchronous frozen-snapshot server on `127.0.0.1` and synchronous remote repository fetcher.
- **CLI & Presentation (`src/cli/`, `src/presentation/`):** Strict positional argument parsing and dual-mode formatting (byte-stable plain output vs ANSI SGR terminal styling).

### 3. Implementation & Invariant Enforcement
- **Strict Validation:** Enforce numeric bounds (e.g., JavaScript maximum safe integer `9007199254740991`), character casing, and schema invariants defensively.
- **Determinism:** Ensure outputs, warnings, and error messages are identical across runs.
- **No Extra Surface:** Do not add git-like concepts (branches, staging areas, checkout, push) not defined in `SPEC.md`.

### 4. Verification & Testing Hierarchy
Run the verification hierarchy before considering the phase complete:

1. **Compilation & Static Checks:**
   ```bash
   cd rust
   cargo check
   cargo clippy
   cargo fmt --check
   ```
   If formatting differences exist, format with `cargo fmt`.

2. **In-Process Unit & Property Tests:**
   ```bash
   cargo test
   ```
   Ensure all in-memory unit tests and `proptest` property-based tests pass.

3. **Subprocess / Binary Check:**
   ```bash
   cd ..
   ./run --lang rust --version
   ```

4. **Acceptance Suite (When Applicable):**
   For phases that touch CLI commands or repository workflows:
   ```bash
   ./verify --lang rust
   ```
   Or targeted filtering:
   ```bash
   ./verify --lang rust --filter <test-name>
   ```

### 5. User Review & Approval
Before creating any git commits:
- Present a clear summary of changes, test outputs, and verification results to the user (using `walkthrough.md` when applicable).
- Explicitly ask for the user's review and approval.
- Do NOT proceed to stage or commit changes until the user gives explicit confirmation.

### 6. Git Commit
Once approved by the user, stage only the relevant implementation and test files:
```bash
git status
git add rust/
git commit -m "Implement Phase X: <Description>"
```
