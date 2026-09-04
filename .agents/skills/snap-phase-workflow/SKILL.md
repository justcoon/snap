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

### 1. Requirements Alignment & Implementation Plan
- **Inspect the Phase Objectives:** Locate the target phase in `plan.md` (§7: *Phased Implementation Roadmap*) and review its objectives and verification gates.
- **Consult Test Scenarios:** Find the matching domain in `test-scenarios.md` (§2: *Detailed Test Scenarios by Domain*) to identify required unit scenarios, boundary conditions, and property tests.
- **Reference the Canonical Specification:** Review the relevant sections of `SPEC.md` to guarantee all normative requirements (MUST/MUST NOT) are satisfied.
- **Create & Commit Implementation Plan:**
  - Create the detailed implementation plan in `docs/implementation/phase-<N>_implementation_plan.md` (and also update the session `implementation_plan.md` artifact for interactive review).
  - Obtain user approval on the plan before starting code modifications.

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
- **Rust Anti-Patterns to Avoid:**

| Anti-Pattern | Why It's Bad | Idiomatic Correction |
| :--- | :--- | :--- |
| **Excessive `.unwrap()` / `.expect()`** | Causes runtime crashes (panics) on unexpected errors. | Propagate errors gracefully using the `?` operator. |
| **Aggressive `.clone()`** | Generates hidden, expensive heap allocations to bypass the borrow checker. | Pass data by reference (`&T`) or slice (`&str`, `&[T]`). |
| **"Stringly-Typed" Everything** | Uses `String` for structured data, inviting invalid states and validation bugs. | Enforce structural validity with strictly typed `enum` or `struct` definitions. |
| **C-Style Index Loops** | Triggers runtime bounds checking on every single iteration. | Use high-performance iterators and `.enumerate()`. |
| **Holding Mutexes Across `.await`** | Freezes asynchronous tasks, leading to runtime deadlocks. | Drop the lock guard explicitly or use block scopes before `.await`. |


### 4. Verification & Testing Hierarchy
Run the verification hierarchy before considering the phase complete:

1. **Compilation & Static Checks:**
   ```bash
   cd rust
   cargo check
   cargo clippy --all-targets
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

### 5. Walkthrough & Discrepancy Analysis
Before asking for commit approval:
- Create `docs/implementation/phase-<N>_implementation_walkthrough.md` (and update session `walkthrough.md`).
- **Mandatory Discrepancy Check Section:** Every walkthrough MUST contain a section:
  ```markdown
  ## Plan vs. Implementation Discrepancy Check
  - **Planned Scope:** [Summary of proposed changes from phase plan]
  - **Implemented Scope:** [Summary of actual changes made]
  - **Deviations / Adjustments:** [Explicitly list any deviations, edge-case discoveries, or additions with technical rationale, or state "None: Implementation strictly adhered to the approved plan."]
  ```
- Present a clear summary of changes, test outputs, and verification results to the user.
- **Explicit User Approval:** Explicitly ask for user review and approval. Do NOT stage or commit until the user confirms.

### 6. Git Commit
Once approved by the user, stage the implementation, tests, and documentation:
```bash
git status
git add rust/ docs/implementation/
git commit -m "Implement Phase <N>: <Description>"
```
