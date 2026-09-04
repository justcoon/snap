---
name: snap-bug-hunter
description: >-
  Guides the identification of bugs, edge-case discrepancies, and specification violations in the
  Rust Snap implementation. Spawns or orchestrates an adversarial testing workflow requiring proof of
  existence via failing test cases for every bug found (minimum of 3), followed by a high-level summary report.
---

# Snap Bug Hunter Workflow

This skill defines the structured workflow for systematically uncovering bugs, boundary condition failures, and deviations from [`SPEC.md`](../../../SPEC.md) in the Snap Rust codebase (`rust/`).

Every identified bug must be proven to exist by adding a corresponding reproducible failing test case before any fix is attempted, targeting a minimum of 3 distinct bugs.

---

## 1. Adversarial Bug Hunting Strategy

Conduct targeted static analysis, boundary testing, and adversarial scenario generation across the following high-risk domains:

### Domain 1: Text Tokenization & Canonical Diff
- **Unterminated line tokens:** Text files ending without trailing newlines (`\n`).
- **Carriage returns (`\r`):** Lone CRs, mixed CRLF, or CR within tokens.
- **Diff Tie-Breaking:** Recurrence $D(i, j)$ tie-breaking (deletions over insertions on equal distance) with repeated lines.
- **Empty file handling:** Transition from empty to single-line or binary file.

### Domain 2: Version Algebra & Contributor Identities
- **Revision limits & parsing:** Numerical boundaries around JavaScript safe integer (`9007199254740991`), leading signs (e.g. `+1`), negative values, or zero revisions.
- **Contributor ID syntax:** Disallowed characters (`,`, `(`, `)`, `->`), ASCII whitespace, multi-byte UTF-8, case preservation, and 254-byte length limits.
- **Frontier comparison & join:** Concurrent frontiers with missing components, antisymmetry, and total Snap order tie-breaking.

### Domain 3: Operational Transformation (OT) & Replay Engine
- **Cursor Integration Order:** Concurrent insertions by both $P$ and $Q$ at the identical cursor position (priority of $Q$-insert vs $P$-insert).
- **Adjacent Edit Operations:** Coalescing rules after transformation.
- **Namespace Conflict Resolution:** Concurrent replacement of directory by file or file by directory (`namespace-wins`), ensuring exact prefix-free cleanup in `current_tree`.
- **Delete-Wins vs Later-Create-Wins:** Interactions between concurrent deletes, puts, and text changes on identical paths.

### Domain 4: Path Validation & Filesystem Safety
- **Path normalizations:** Trailing slashes, empty segments (`//`), `.` or `..` segments, backslashes, and control characters.
- **Prefix Freedom:** Segment prefix conflicts when multiple changes transition a file into a directory structure within the same patch.
- **Symlink & Special File Rejection:** Ensuring symlinks are never followed and trigger immediate clean errors.

### Domain 5: CLI Grammar & Positional Arguments
- **Option placement & extra arguments:** Rejecting extra positional tokens, duplicate flags, misplaced options (e.g., `--global` at end of command), and unknown flags.
- **Exit codes & channel routing:** Expected errors exit 1 to `stderr` with `snap: <detail>`, success exits 0 to `stdout`.

### Domain 6: HTTP Snapshot Server & Client
- **Chunked transfer encoding:** Parsing hex chunk sizes, uppercase hex, trailing CRLF, and chunk trailers.
- **Connection timeouts & headers:** Handling malformed HTTP headers, non-200 responses, redirects (must not follow), and large payloads.

---

## 2. Bug Identification & Proof-of-Existence Workflow

Follow this procedure for each candidate bug:

```
+-------------------------------------------------------------+
| 1. Formulate Hypothesis & Inspect SPEC.md                   |
+-----------------------------+-------------------------------+
                              |
                              v
+-------------------------------------------------------------+
| 2. Write Failing Reproducer Test Case in rust/              |
|    - Add to appropriate #[cfg(test)] module or integration  |
|    - Test name pattern: test_bug_<domain>_<description>     |
+-----------------------------+-------------------------------+
                              |
                              v
+-------------------------------------------------------------+
| 3. Execute cargo test to Prove Failure                      |
|    - Must FAIL on current codebase                          |
|    - Captures exact error / panic / divergence              |
+-----------------------------+-------------------------------+
                              |
                              v
+-------------------------------------------------------------+
| 4. Document Bug & Add to Summary Table                      |
|    - Minimum of 3 distinct bugs required                     |
+-------------------------------------------------------------+
```

### Invariant Rules for Bug Proofs
1. **No Spec Mutations:** Do not alter [`SPEC.md`](../../../SPEC.md) to make a bug pass. The specification is canonical.
2. **Proof via Code:** A bug report without a runnable, failing test case is invalid. Every bug must be accompanied by an added test in the test suite.
3. **Reproducibility:** Tests must execute in-process via `cargo test` without relying on external network dependencies or ambient machine state.

---

## 3. Subagent Prompt Template

When delegating this task to a subagent, use the following structured prompt:

```markdown
You are an adversarial bug hunter specializing in Rust, systems programming, and formal specifications.

Your objective is to discover and prove the existence of at least 3 distinct bugs or specification non-compliances in the Snap Rust implementation (`rust/`).

For every bug you identify:
1. Cross-reference the relevant clause in `SPEC.md`.
2. Add a standalone, reproducible test case to the Rust test suite in `rust/` (e.g., in the relevant `tests` module) named `test_bug_<description>`.
3. Run `cargo test` and verify that the test fails against the current implementation, proving the bug exists.
4. Document:
   - Bug description and affected file/function.
   - Exact SPEC.md requirement violated.
   - Failing test name and failure output.

Do not stop until you have uncovered and proven a minimum of 3 distinct, genuine bugs.
Finally, provide a high-level summary table of all bugs found and their reproducing test cases.
```

---

---

## 4. Storage Locations & Summary Report Format

### Designated Storage Locations
1. **Summary & Detailed Reports:**
   - Primary file: [`docs/bugs/bug_hunting_report.md`](../../../docs/bugs/bug_hunting_report.md)
   - Multi-run / historical reports: `docs/bugs/report_<YYYY-MM-DD>.md`
2. **Reproducer Test Cases:**
   - Dedicated integration test suite: `rust/tests/bug_reproductions.rs`
   - In-process domain tests: Co-located in `rust/src/<subsystem>/` named `test_bug_<description>`

---

### Report Document Template (`docs/bugs/bug_hunting_report.md`)

Once the hunting phase completes and at least 3 failing tests are established, save the final report to `docs/bugs/bug_hunting_report.md` using this template:

```markdown
# Snap Bug Hunting Report

## Executive Summary
- **Report Date:** <YYYY-MM-DD>
- **Total Bugs Identified & Proven:** <count> (Minimum: 3)
- **Primary Subsystems Affected:** <list of subsystems>
- **Reproducer Suite Location:** `rust/tests/bug_reproductions.rs`
- **Failing Tests Count:** <count>

## Proven Bugs & Corresponding Failing Test Cases

| Bug ID | Title | Subsystem / File | SPEC.md Reference | Failing Test Case | Failure Mode / Output |
|---|---|---|---|---|---|
| `BUG-001` | <Title> | `<file>:<fn>` | §<X.Y> | `test_bug_001_<name>` | `<assertion failure or panic>` |
| `BUG-002` | <Title> | `<file>:<fn>` | §<X.Y> | `test_bug_002_<name>` | `<assertion failure or panic>` |
| `BUG-003` | <Title> | `<file>:<fn>` | §<X.Y> | `test_bug_003_<name>` | `<assertion failure or panic>` |

## Detailed Breakdown & Root Cause Analysis

### Bug BUG-001: <Title>
- **Location:** `rust/src/<path>`
- **Violated Contract:** [Quote relevant text from SPEC.md]
- **Current Behavior:** [Describe what the code currently does]
- **Expected Behavior:** [Describe what the specification requires]
- **Reproducer:** [Link to test case in `rust/tests/bug_reproductions.rs`]

[Repeat for each bug: BUG-002, BUG-003, ...]
```
