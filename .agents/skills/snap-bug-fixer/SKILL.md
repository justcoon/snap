---
name: snap-bug-fixer
description: >-
  Systematically fixes bugs, edge-case discrepancies, and specification non-compliances
  documented in bug hunting reports (e.g., docs/bugs/bug_hunting_report.md). Ensures each fix
  turns the corresponding failing reproducer test in rust/tests/bug_reproductions.rs into a pass,
  preserves 100% acceptance compliance, and updates the report status.
---

# Snap Bug Fixer Workflow

This skill defines the structured procedure for remediating bugs, edge-case divergences, and specification violations identified during bug hunting in the Snap codebase (`rust/`), using [`docs/bugs/bug_hunting_report.md`](../../../docs/bugs/bug_hunting_report.md) and [`rust/tests/bug_reproductions.rs`](../../../rust/tests/bug_reproductions.rs) as the guiding contract.

---

## 1. Core Principles

1. **Specification Canonicality:**
   [`SPEC.md`](../../../SPEC.md) is authoritative. Public behavior must conform strictly to the specification; never weaken, modify, or test around the specification to accommodate an implementation bug.
2. **Reproducer-Driven Remediation:**
   Every fix must directly turn at least one failing test case in [`rust/tests/bug_reproductions.rs`](../../../rust/tests/bug_reproductions.rs) (or domain unit test) into a passing test.
3. **Zero Regression Guarantee:**
   A bug fix must not introduce regressions or break existing test suites. The full in-process unit and property test suite (`cargo test`) and the shared language-neutral acceptance suite (`./verify --lang rust`) must continue to pass cleanly with zero failures.
4. **Scope & Architectural Purity:**
   Snap's small surface is deliberate. Fixes must be focused, minimal, and deterministic without adding unneeded concepts, extra commands, or loose error suppression.

---

## 2. Bug Fixing Execution Checklist

Follow this systematic checklist for each bug or batch of related bugs:

### Step 1: Requirements Alignment & Remediation Plan
- **Inspect Bug Report:** Read [`docs/bugs/bug_hunting_report.md`](../../../docs/bugs/bug_hunting_report.md) to understand the failure mode, affected subsystem, and relevant `SPEC.md` clauses.
- **Inspect Failing Reproducers:** Locate the corresponding reproducer test in [`rust/tests/bug_reproductions.rs`](../../../rust/tests/bug_reproductions.rs).
- **Confirm Reproducibility:** Run the reproducer test to verify failure on the current codebase:
  ```bash
  cd rust
  cargo test --test bug_reproductions -- <test_name>
  ```
- **Formulate Fix Plan:** Identify root cause in `rust/src/` and design a clean, idiomatic fix adhering to architectural boundaries.

### Step 2: Architecture & Subsystem Partitioning
Maintain strict separation of concerns across modules during bug fixes:
- **Pure Core (`src/core/`):** Version algebra, canonical diff, OT, replay engine, and repository validation (`validation.rs`). Must remain 100% deterministic with zero filesystem or network side-effects.
- **Filesystem & Config (`src/fs/`, `src/config/`):** Path validation, segment prefix-freedom, scanner, materializer, and config loader.
- **HTTP Mode (`src/http/`):** Chunked transfer decoding, header parsing, snapshot serving on loopback, and synchronous remote fetching.
- **CLI & Presentation (`src/cli/`, `src/presentation/`):** Argument parsing, exit code routing (`exit 1` to stderr with `snap: <detail>`), and dual-mode formatting.

### Step 3: Implementation & Invariant Enforcement
- **Strict Invariant Validation:**
  - Enforce character boundaries (e.g. commit message UTF-8 validity and rejection of ASCII control characters other than `\t` and `\n` per SPEC §4.2).
  - Enforce collection invariants (e.g. `patch.changes` sorted by path and unique per SPEC §4.2).
  - Enforce protocol standards (e.g. RFC 7230 §4.1 chunk extensions and mandatory chunk-data CRLF).
- **Determinism:** Ensure outputs, warnings, and error messages are identical across runs.
- **Rust Anti-Patterns to Avoid:**

| Anti-Pattern | Why It's Bad | Idiomatic Correction |
| :--- | :--- | :--- |
| **Excessive `.unwrap()` / `.expect()`** | Causes runtime crashes (panics) on unexpected errors or adversarial input. | Propagate errors gracefully using the `?` operator with typed errors (`CliError`, `ValidationError`, etc.). |
| **Aggressive `.clone()`** | Generates hidden, expensive heap allocations to bypass the borrow checker. | Pass data by reference (`&T`) or slice (`&str`, `&[T]`). |
| **"Stringly-Typed" Everything** | Uses `String` for structured data, inviting invalid states and validation bugs. | Enforce structural validity with strictly typed `enum` or `struct` definitions (`ContributorId`, `Version`, `Change`). |
| **Scattered Magic Literals & Raw Paths** | Creates maintenance friction and risk of subtle typos across modules. | Centralize in canonical constants (`REPOSITORY_FORMAT_VERSION`, `CONTRIBUTOR_ID_KEY`, `MAX_REVISION`). |
| **Ad-Hoc Loose Options** | Passing ad-hoc optional flags or scattered default constants complicates configuration. | Encapsulate options in dedicated config structs implementing `Default`. |
| **Monolithic Command Handlers** | Large files containing all command implementations increase coupling. | Keep command handlers isolated under `src/cli/commands/` with uniform `cmd_<name>` signatures. |
| **C-Style Index Loops** | Triggers runtime bounds checking on every single iteration. | Use high-performance iterators and `.enumerate()`. |
| **Holding Mutexes Across `.await`** | Freezes asynchronous tasks, leading to runtime deadlocks. | Drop the lock guard explicitly or use block scopes before `.await`. |

- **Dual-Test Strategy for Bug Fixes:**
  1. **Permanent Subsystem Regression Test:** Always add a permanent regression test named `test_regression_bug_<id>_<description>` to the affected module's `#[cfg(test)]` block (e.g. in `src/core/validation.rs` or `src/cli/commands/commit.rs`). This ensures the fix is permanently defended during standard development (`cargo test`).
  2. **Active Burndown Annotation in Reproducer Suite:** In `rust/tests/bug_reproductions.rs`, verify that the reproducer passes against the fix, then annotate the test with:
     ```rust
     #[test]
     #[ignore = "Resolved in BUG-<XXX> (see docs/bugs/resolution_BUG-<XXX>_walkthrough.md)"]
     fn test_bug_<id>_<description>() { ... }
     ```
     This allows `cargo test --test bug_reproductions` to function as an active defect burndown backlog: remaining open bugs fail as `FAILED`, while resolved bugs cleanly display as `ignored`.

### Step 4: Verification & Testing Hierarchy
Run the verification hierarchy before declaring a bug fix complete:

1. **Reproducer & Burndown Verification:**
   ```bash
   cd rust
   # Verify the fixed reproducer passes when explicitly targeted
   cargo test --test bug_reproductions -- --ignored test_bug_<id>
   # Check remaining open bugs in the burndown suite
   cargo test --test bug_reproductions
   ```

2. **Compilation & Static Checks:**
   ```bash
   cargo check
   cargo clippy --all-targets
   cargo fmt --check
   ```
   If formatting differences exist, format with `cargo fmt`.

3. **In-Process Unit & Property Tests (including permanent regression test):**
   ```bash
   cargo test
   ```
   Ensure all in-memory unit tests, property tests, and newly added permanent subsystem regression tests pass with zero regressions.

4. **Subprocess / Binary Check:**
   ```bash
   cd ..
   ./run --lang rust --version
   ```

5. **Shared Acceptance Suite:**
   Run the canonical language-neutral YAML acceptance test suite from the repository root:
   ```bash
   ./verify --lang rust
   ```
   Or targeted filtering when focusing on a specific suite:
   ```bash
   ./verify --lang rust --filter <test-name>
   ```
   All 28 acceptance suites must pass (100% compliance).

### Step 5: Bug Resolution Walkthrough & Discrepancy Analysis
Before asking for commit approval:
- **Create Resolution Walkthrough:** Create the walkthrough artifact (and synchronize the session `walkthrough.md`):
  - **Individual Bug Fix (Recommended):** [`docs/bugs/resolution_BUG-<XXX>_walkthrough.md`](../../../docs/bugs/) (e.g., `docs/bugs/resolution_BUG-001_walkthrough.md`). Preserves a persistent historical record alongside each bug fix commit.
  - **Batch Fix (Multiple/All Bugs):** [`docs/bugs/bug_resolution_walkthrough.md`](../../../docs/bugs/bug_resolution_walkthrough.md) (covers all bugs resolved in the batch).
  
  The walkthrough artifact must document:
  - **Executive Summary:** List of bug IDs addressed, affected subsystems, and verified passing test counts.
  - **Detailed Per-Bug Resolution:** Root cause, specific code changes with file links, and before/after behavior.
  - **Red-to-Green Reproducer Evidence:** Exact test commands and terminal outputs demonstrating the transition from `FAILED` to `PASSED` in `rust/tests/bug_reproductions.rs`.
  - **Full Regression Verification:** Confirmation of passing static checks, unit/property tests, and 100% acceptance suite pass (`./verify --lang rust`).
  - **Mandatory Bug Fix Discrepancy Check Section:**
    ```markdown
    ## Bug Fix Discrepancy Check
    - **Bug ID & Title:** [Bug ID (e.g. BUG-001) and description from report]
    - **Identified Defect:** [Root cause in code]
    - **Remediation Applied:** [Summary of code changes made]
    - **Failing Reproducer Status:** [Confirmed PASS in cargo test --test bug_reproductions -- <test_name>]
    - **Regression Verification:** [Confirmed 28/28 passed in ./verify --lang rust and all unit tests]
    - **Unintended Side-Effects:** [Explicitly confirm no behavioral regressions or state "None: Invariants strictly preserved."]
    ```
- **Update Bug Report:** Update [`docs/bugs/bug_hunting_report.md`](../../../docs/bugs/bug_hunting_report.md):
  - In the summary table, update the status column to `FIXED / PASSING`.
  - Add a `### Resolution` block under each resolved bug's detailed breakdown explaining the fix, modified files, and link to the reproducer test.
- **Explicit User Approval:** Present a concise summary of changes and verification results to the user. Do NOT stage or commit until the user explicitly confirms approval.

### Step 6: Git Commit
Once approved by the user, stage the implementation fixes, reproducer tests, and documentation:
- For an individual bug:
  ```bash
  git status
  git add rust/ docs/bugs/
  git commit -m "Fix BUG-<XXX>: <Short description>"
  ```
- For a batch fix:
  ```bash
  git status
  git add rust/ docs/bugs/
  git commit -m "Fix bugs BUG-001..BUG-004: Resolve bug hunting report defects"
  ```


