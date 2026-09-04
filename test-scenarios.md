# Snap (Rust) — Concrete Test Scenarios Specification

This document defines the comprehensive test scenarios to be implemented in Rust across in-process unit tests, subsystem integration tests, CLI subprocess tests, and generative property-based tests.

Each scenario details the **Setup (Preconditions & Fixtures)**, **Invocation (Execution)**, and **Expected Invariants & Assertions**.

---

## 1. Test Harness Architecture & Test Framework in Rust

The Rust test suite is organized into three distinct tiers using standard Cargo test targets:

1. **Unit & Property Tests (`rust/src/**/tests.rs` or `#[cfg(test)]` modules):**
   - In-memory execution using Rust’s built-in test runner.
   - Property-based testing powered by the `proptest` crate.
   - Zero filesystem and zero network dependencies.

2. **Integration Tests (`rust/tests/*.rs`):**
   - Exercised against temporary filesystem sandboxes managed via `tempfile::TempDir`.
   - Evaluates multi-patch replay, filesystem materialization, configuration loading, and atomic mutations.

3. **Subprocess & TTY Emulation Tests (`rust/tests/cli_*.rs`):**
   - Executes the compiled `snap` binary (`target/debug/snap`) with controlled process environments (`SNAP_COLOR`, `NO_COLOR`, `HOME`).
   - Mocks and asserts TTY vs non-TTY stream behavior on stdout and stderr independently.

---

## 2. Detailed Test Scenarios by Domain

### Domain A: Version Algebra & Contributor Identity

#### Scenario A.1: Contributor ID Syntax & Validation Boundaries
- **Target:** Contributor email validation parser.
- **Classification:** Unit Test.
- **Setup:**
  - Valid candidates: `alice@example.com`, `a@b`, `user+tag@domain.co`, 254-byte string with `@` in the middle.
  - Invalid candidates: empty string, `@`, `@domain.com`, `user@`, `a@@b`, `user@dom ain`, `user@dom\x00ain`, `user@dom,ain`, `user@(x)@dom`, `a->b@c`, `a@b->c`, string exceeding 254 bytes.
- **Invocation:** Parse each candidate through the contributor ID constructor.
- **Expected Invariants & Assertions:**
  - Valid candidates return successfully, preserving exact ASCII character casing without normalization.
  - Invalid candidates return explicit validation errors identifying the constraint violation.

#### Scenario A.2: Version String Canonical Parser & Formatter
- **Target:** Vector clock parser and string display.
- **Classification:** Unit Test.
- **Setup:**
  - Valid inputs: `()`, `(alice@x->1)`, `(alice@x->1,bob@y->2)`, `(a@x->1,b@y->2,c@z->9007199254740991)`.
  - Invalid inputs: `( )`, `(alice@x->0)`, `(alice@x->01)`, `(bob@y->2,alice@x->1)` (unsorted), `(alice@x->1,alice@x->2)` (duplicate), `(alice@x->9007199254740992)` (overflow), `alice@x->1` (missing parentheses), `(alice@x->-1)`.
- **Invocation:** Parse each string into a version structure, then re-serialize to string.
- **Expected Invariants & Assertions:**
  - Valid inputs parse successfully. Serializing them returns the exact canonical input string byte-for-byte.
  - Unsorted contributors, leading zeroes, explicit zero revisions, overflow beyond JavaScript maximum safe integer (`9007199254740991`), and duplicate IDs are rejected.

#### Scenario A.3: Four-Way Causal Comparison Matrix
- **Target:** Version comparison logic.
- **Classification:** Unit Test.
- **Setup:**
  - Version `V0`: `()`
  - Version `V1`: `(alice@x->1)`
  - Version `V2`: `(alice@x->2)`
  - Version `V3`: `(alice@x->1,bob@y->1)`
  - Version `V4`: `(bob@y->2)`
- **Invocation:** Perform pairwise comparisons across all pairs.
- **Expected Invariants & Assertions:**
  - `V0 < V1`, `V1 < V2`, `V1 < V3`.
  - `V2 || V3` (concurrent: `V2` has higher alice revision, but `V3` has higher bob revision).
  - `V2 || V4` (concurrent: alice vs bob).
  - Antisymmetry: `V < W` strictly implies `W > V`.
  - Concurrency symmetry: `V || W` strictly implies `W || V`.
  - Identity: `V == V` returns Equal.

#### Scenario A.4: Snap Total Order Resolution
- **Target:** Deterministic total ordering for concurrent versions.
- **Classification:** Unit Test.
- **Setup:**
  - Version `VA`: `(alice@x->2,bob@y->1)`
  - Version `VB`: `(alice@x->1,bob@y->3)`
  - Version `VC`: `(carol@x->1)`
- **Invocation:** Sort versions using Snap order.
- **Expected Invariants & Assertions:**
  - Sorted union of contributor IDs is evaluated lexicographically.
  - First differing counter determines order: `alice@x` has counter 2 in `VA` and 1 in `VB`; therefore `VB < VA` in Snap order.
  - For `VA` and `VC`, the first union key `alice@x` is absent (0) in `VC` and present (2) in `VA`; therefore `VC < VA`.

---

### Domain B: Repository Schema & Graph Invariant Validation

#### Scenario B.1: JSON Schema Strictness & Unknown Field Rejection
- **Target:** Repository JSON deserializer.
- **Classification:** Unit Test.
- **Setup:**
  - Repository JSON with unknown top-level field `{"format": 1, "extra": true, ...}`.
  - Patch JSON with floating point number `{"revision": 1.0}` or string `{"revision": "1"}`.
  - Patch JSON containing duplicate keys `{"message": "a", "message": "b"}`.
- **Invocation:** Attempt deserialization from raw JSON bytes.
- **Expected Invariants & Assertions:**
  - Deserializer rejects unknown fields, non-integer numbers, and duplicate keys with descriptive error messages.

#### Scenario B.2: Patch Continuity & Serial Contributor Invariant
- **Target:** Repository graph validator.
- **Classification:** Unit Test.
- **Setup:**
  - Patch graph where contributor `alice@x` has revision 1 and revision 3, but revision 2 is missing.
  - Patch graph where contributor `alice@x` revision 2 declares a base that does not contain `alice@x->1`.
- **Invocation:** Run complete graph validation.
- **Expected Invariants & Assertions:**
  - Validator fails: non-contiguous revisions violate the serial contributor rule.
  - Missing prerequisite patch base violates causal closure.

#### Scenario B.3: Dot Collision Detection
- **Target:** Repository import and validator.
- **Classification:** Unit Test.
- **Setup:**
  - Existing patch: author `alice@x`, revision 1, message "first commit", change adds `file.txt`.
  - Incoming patch: author `alice@x`, revision 1, message "different commit", change adds `other.txt`.
- **Invocation:** Validate repository containing both or merge candidate repository.
- **Expected Invariants & Assertions:**
  - Validator marks history as corrupt: identical `(author, revision)` dot with structurally differing patch values is rejected.

---

### Domain C: Tokenization & Canonical Text Diff

#### Scenario C.1: Token Splitter Boundary Behavior
- **Target:** LF-preserving text tokenizer.
- **Classification:** Unit Test.
- **Setup:**
  - File 1: Empty byte slice `""`.
  - File 2: `"line1\nline2\n"`.
  - File 3: `"line1\r\nline2\r\n"`.
  - File 4: `"line1\nunterminated"`.
  - File 5: `"binary\x00data"`.
- **Invocation:** Run tokenization and binary detection.
- **Expected Invariants & Assertions:**
  - File 1 produces zero tokens.
  - File 2 produces `["line1\n", "line2\n"]`.
  - File 3 produces `["line1\r\n", "line2\r\n"]`.
  - File 4 produces `["line1\n", "unterminated"]`.
  - File 5 is flagged as non-text (binary) due to NUL byte.

#### Scenario C.2: Diff Recurrence & Deletion-on-Tie Tie-Breaker
- **Target:** Canonical token diff dynamic programming engine.
- **Classification:** Unit Test.
- **Setup:**
  - Base tokens: `["A\n", "B\n"]`.
  - Target tokens: `["C\n", "B\n"]`.
  - Ambiguous tie case: transforming `["X\n"]` to `["Y\n"]` where inserting or deleting first has equal minimal distance `D(1, 0) == D(0, 1)`.
- **Invocation:** Generate edit scripts using the canonical recurrence.
- **Expected Invariants & Assertions:**
  - For ambiguous cost ties (`D(i + 1, j) <= D(i, j + 1)`), deletion operation (`delete 1`) is emitted before insertion (`insert ["Y\n"]`).
  - Adjacent operations of the same kind are coalesced into a single operation.
  - Applying the emitted edit script to base tokens produces target tokens exactly.

---

### Domain D: Operational Transformation (OT) Engine

#### Scenario D.1: Pairwise OT Transformation Table
- **Target:** Text edit transform engine.
- **Classification:** Unit Test.
- **Setup:**
  - Incoming edit `P` and concurrent context edit `Q` consuming the same base tokens.
  - Case 1: `P` inserts at cursor while `Q` retains.
  - Case 2: `Q` inserts at cursor while `P` retains.
  - Case 3: Both `P` and `Q` insert at the identical cursor position.
  - Case 4: `P` deletes while `Q` retains.
  - Case 5: `P` retains while `Q` deletes.
  - Case 6: Both `P` and `Q` delete the identical base tokens.
- **Invocation:** Execute `transform(P, Q)`.
- **Expected Invariants & Assertions:**
  - Case 1: Transformed `P` retains the exact insertion.
  - Case 2: Transformed `P` emits `retain(length(Q_insert))` to shift past `Q`'s insert.
  - Case 3: `Q insert` takes priority; transformed `P` emits `retain(length(Q_insert))` followed by `P insert`.
  - Case 4: Transformed `P` emits `delete`.
  - Case 5: Transformed `P` consumes base tokens and emits nothing.
  - Case 6: Duplicate deletion consumes tokens and emits nothing (no duplicate delete).

#### Scenario D.2: Three-Way Concurrent Text Edits
- **Target:** Multi-contributor text merge via replay.
- **Classification:** Integration Test.
- **Setup:**
  - Base repository contains `story.txt` with lines 1 through 10.
  - Contributor A inserts a line between lines 2 and 3.
  - Contributor B modifies line 5.
  - Contributor C appends a line after line 10.
- **Invocation:** Merge all three branches in differing topological orders.
- **Expected Invariants & Assertions:**
  - Replay integrates all changes deterministically.
  - Resulting file contains all three non-conflicting edits in canonical order.
  - Emits zero warnings.

---

### Domain E: Deterministic Replay & Conflict Policies

#### Scenario E.1: Namespace Conflict Resolution (`namespace-wins`)
- **Target:** Multi-path namespace resolver during patch integration.
- **Classification:** Integration Test.
- **Setup:**
  - Base version has empty repository.
  - Branch A creates regular file `docs`.
  - Branch B creates file `docs/intro.txt` (making `docs` a directory).
- **Invocation:** Merge Branch B into Branch A, and separately merge Branch A into Branch B.
- **Expected Invariants & Assertions:**
  - Canonical integration order evaluates the patches.
  - The patch making paths present overrides conflicting ancestral/descendant paths.
  - Conflicting current path is removed with warning: `warning: auto-resolved docs: namespace-wins`.
  - Replayed filesystem produces prefix-free hierarchy: both merge directions converge to the identical tree.

#### Scenario E.2: Path-Level Conflict Winner Rules
- **Target:** Resolution of concurrent file edits and deletions.
- **Classification:** Unit / Integration Test.
- **Setup:**
  - Subcase 1 (Delete Wins): Branch A deletes `f.txt`, Branch B edits `f.txt`.
  - Subcase 2 (Later Create Wins): Branch A creates `f.txt` with "hello", Branch B creates `f.txt` with "world".
  - Subcase 3 (Later Put Wins): Branch A edits `f.txt` as text, Branch B overwrites `f.txt` with binary content via `put`.
  - Subcase 4 (Put Wins): Current tree has binary `f.bin`, incoming patch applies text edit to `f.bin`.
- **Invocation:** Replay each pair of concurrent patches.
- **Expected Invariants & Assertions:**
  - Subcase 1: `f.txt` is absent, emits `delete-wins`.
  - Subcase 2: Canonically later create wins, emits `later-create-wins`.
  - Subcase 3: Incoming binary replacement wins, emits `later-put-wins`.
  - Subcase 4: Existing binary content is preserved, emits `put-wins`.
  - Warnings are logged in sorted order: path ascending, then reason ascending.

---

### Domain F: Filesystem Scanning & Safe Materialization

#### Scenario F.1: Path Invariant Enforcer & Symlink Rejection
- **Target:** Working tree scanner and path validator.
- **Classification:** Integration Test.
- **Setup:**
  - Repository contains regular files `a/b.txt`.
  - Test creates a symlink `link -> a/b.txt` in the working directory.
  - Test creates a FIFO or socket entry if supported by OS.
- **Invocation:** Run `snap status`.
- **Expected Invariants & Assertions:**
  - Command fails with exit code 1.
  - Standard error reports unsupported filesystem entry without following the symlink.

#### Scenario F.2: Working Tree Clean vs Dirty Detection
- **Target:** Tree scanner.
- **Classification:** Integration Test.
- **Setup:**
  - Clean working tree matching repository frontier.
  - State 1: Touch existing file modifying its timestamp without changing bytes.
  - State 2: Change 1 byte in existing file.
  - State 3: Add new untracked regular file.
  - State 4: Remove tracked file.
- **Invocation:** Query working tree status for each state.
- **Expected Invariants & Assertions:**
  - State 1 is reported as clean (timestamps are ignored, only byte contents matter).
  - State 2 is dirty (`M` / modified).
  - State 3 is dirty (`A` / added).
  - State 4 is dirty (`D` / deleted).

#### Scenario F.3: Atomic Metadata Replacement Failure Safety
- **Target:** Atomic file materializer.
- **Classification:** Integration Test.
- **Setup:**
  - Valid repository with existing `repository.json`.
  - Trigger commit or merge with a read-only filesystem or restricted permissions on `.snap/repository.json` temporary destination.
- **Invocation:** Attempt mutation.
- **Expected Invariants & Assertions:**
  - Target files are not corrupted.
  - Original `repository.json` remains untouched.
  - Command exits with error.

---

### Domain G: Configuration Hierarchy & Identity

#### Scenario G.1: Local Over Global Precedence
- **Target:** Configuration resolver.
- **Classification:** Integration Test.
- **Setup:**
  - Set environment `HOME` to custom directory containing `.snapconfig.json` with `contributor.id = "global@example.com"`.
  - Create local repository with `.snap/config.json` containing `contributor.id = "local@example.com"`.
- **Invocation:** Author a commit with `snap commit "message"`.
- **Expected Invariants & Assertions:**
  - Created patch author is `local@example.com`.
  - Local configuration overrides global configuration.

#### Scenario G.2: Missing Contributor ID Enforcement
- **Target:** Contributor validation in authoring commands.
- **Classification:** Integration Test.
- **Setup:**
  - Fresh repository initialized without local config and empty `HOME` directory.
- **Invocation:**
  - Run `snap status` (read-only).
  - Run `snap commit "test"` (authoring).
  - Run `snap revert ()` (authoring).
- **Expected Invariants & Assertions:**
  - `snap status` succeeds without requiring contributor configuration.
  - `snap commit` and `snap revert` abort immediately with:
    `snap: contributor.id is required; configure it locally or globally`.
  - Exit code is 1; no repository mutation occurs.

---

### Domain H: CLI Dispatch & Grammar Enforcement

#### Scenario H.1: Strict Positional Grammar Verification
- **Target:** CLI argument scanner.
- **Classification:** Subprocess / Integration Test.
- **Setup:**
  - Invocation 1: `snap commit` (missing message argument).
  - Invocation 2: `snap commit msg extra` (unexpected extra operand).
  - Invocation 3: `snap --global config contributor.id a@b` (flag placed in non-standard leading position).
  - Invocation 4: `snap status --verbose` (unsupported option).
- **Invocation:** Execute binary with each argument vector.
- **Expected Invariants & Assertions:**
  - All invocations fail before inspecting repository state.
  - Standard error prints exact error format `snap: <detail>`.
  - Exit code is 1.

#### Scenario H.2: Repository Discovery by Upward Traversal
- **Target:** Directory walker.
- **Classification:** Integration Test.
- **Setup:**
  - Initialize repository at `/tmp/sandbox/repo`.
  - Create deep nested subdirectory structure `/tmp/sandbox/repo/a/b/c/d`.
- **Invocation:** Run `snap status` from `/tmp/sandbox/repo/a/b/c/d`.
- **Expected Invariants & Assertions:**
  - Discovers parent repository `/tmp/sandbox/repo/.snap`.
  - Paths in status output are displayed relative to repository root (`a/b/c/d/...`).
  - Running outside any repository outputs error stating repository was not found.

---

### Domain I: Terminal Presentation & Color Negotiation

#### Scenario I.1: Stream-Independent TTY & SNAP_COLOR Evaluation
- **Target:** Presentation negotiator.
- **Classification:** Unit & Subprocess Test.
- **Setup:**
  - Test Matrix across stdout and stderr:
    1. Stream is non-TTY (pipe/file redirect), `SNAP_COLOR` unset.
    2. Stream is TTY, `SNAP_COLOR` unset, `NO_COLOR` unset.
    3. Stream is TTY, `SNAP_COLOR` unset, `NO_COLOR=1` set.
    4. Stream is non-TTY, `SNAP_COLOR=always` set, `NO_COLOR=1` set.
    5. Stream is TTY, `SNAP_COLOR=never` set.
    6. `SNAP_COLOR=invalid_value`.
- **Invocation:** Run `snap status` and invalid command across each configuration.
- **Expected Invariants & Assertions:**
  - Case 1: Plain mode (no ANSI escape sequences).
  - Case 2: Terminal mode (ANSI escape sequences present).
  - Case 3: Plain mode (`NO_COLOR` disables terminal mode in `auto` mode).
  - Case 4: Terminal mode (`SNAP_COLOR=always` overrides `NO_COLOR` and redirection).
  - Case 5: Plain mode (`SNAP_COLOR=never` disables terminal mode even on TTY).
  - Case 6: Aborts before execution with plain error:
    `snap: SNAP_COLOR must be auto, always, or never` and exit code 1.

#### Scenario I.2: Exact ANSI Formatting Golden Suite
- **Target:** Terminal formatters.
- **Classification:** Unit Test.
- **Setup:**
  - Command outputs:
    - Successful `commit` output.
    - Dirty `status` output with added, modified, deleted paths.
    - Diff output with hunk headers, added lines, deleted lines, no-newline markers.
    - Warning line and error line.
- **Invocation:** Format using terminal mode helper.
- **Expected Invariants & Assertions:**
  - Validates exact ANSI SGR strings according to `S(n, text)` definition:
    - `✓` formatted with `\x1b[32m✓\x1b[0m`.
    - Labels formatted with `\x1b[1m<label>\x1b[0m`.
    - Versions formatted with `\x1b[36m<version>\x1b[0m`.
    - Diff additions formatted with `\x1b[32m+...\x1b[0m`, deletions with `\x1b[31m-...\x1b[0m`.

---

### Domain J: HTTP Repository Service & Client Fetcher

#### Scenario J.1: Snapshot Isolation in Read-Only HTTP Server
- **Target:** `snap --serve [port]`.
- **Classification:** Integration / Subprocess Test.
- **Setup:**
  - Repository initialized with version `(alice@x->1)`.
  - Start `snap --serve 0` as a child process.
  - Read dynamic server URL from child process stdout: `http://127.0.0.1:<port>/repository.json`.
- **Invocation:**
  - Perform HTTP `GET /repository.json` -> verify payload.
  - Perform HTTP `HEAD /repository.json` -> verify headers with empty body.
  - Perform HTTP `POST /repository.json` -> verify status 405 (`Allow: GET, HEAD`).
  - Perform HTTP `GET /invalid` -> verify status 404.
  - Concurrently modify the underlying `.snap/repository.json` on disk to version `(alice@x->2)`.
  - Perform second HTTP `GET /repository.json`.
  - Send `SIGTERM` to child server process.
- **Expected Invariants & Assertions:**
  - Second GET returns the initial version `(alice@x->1)` (snapshot isolation).
  - Server exits with code 0 upon receiving `SIGTERM`.

#### Scenario J.2: Remote Diff & Merge Fetching Over HTTP
- **Target:** HTTP client resolver.
- **Classification:** Integration Test.
- **Setup:**
  - Local repository A.
  - Local repository B served via `snap --serve 0`.
- **Invocation:**
  - Run `snap diff () (b@x->1) --repo http://127.0.0.1:<port>/repository.json` in repository A.
  - Run `snap merge http://127.0.0.1:<port>/repository.json` in repository A.
- **Expected Invariants & Assertions:**
  - Diff renders cross-repository changes without mutating local repository.
  - Merge successfully imports patches from HTTP endpoint and joins frontiers.

---

### Domain K: Multi-Repository Synchronization & Convergence

#### Scenario K.1: Merge Commutativity & Associativity
- **Target:** Replay and merge integration.
- **Classification:** Integration Test.
- **Setup:**
  - Common ancestor repository with initial content.
  - Three divergent clones authored by Alice, Bob, and Carol containing concurrent edits, creates, and deletes.
- **Invocation:**
  - Branch Merge Order 1: Merge Bob into Alice, then Carol into Alice.
  - Branch Merge Order 2: Merge Carol into Bob, then Bob into Alice.
  - Branch Merge Order 3: Merge Alice and Carol into Bob.
- **Expected Invariants & Assertions:**
  - All merged repositories arrive at bit-for-bit identical working trees.
  - All repositories have identical frontiers and identical patch sets.
  - Re-merging an already merged repository is a complete no-op emitting zero warnings.

---

### Domain L: Fault Tolerance & State Protection

#### Scenario L.1: Refusal of Revert & Merge on Dirty Working Tree
- **Target:** Pre-mutation working tree check.
- **Classification:** Integration Test.
- **Setup:**
  - Repository with uncommitted modification in `dirty.txt`.
- **Invocation:**
  - Run `snap revert (alice@x->1)`.
  - Run `snap merge ../other-repo`.
- **Expected Invariants & Assertions:**
  - Commands refuse execution immediately.
  - Working tree files remain untouched.
  - Repository metadata is not updated.
  - Exit code is 1.

---

## 3. Property-Based Testing Scenarios (Generative Specifications)

Property-based tests will use `proptest` to generate thousands of pseudo-random, structured inputs to uncover edge cases and prove invariants.

### Property Test 1: Version Clock Algebraic Lattice Laws
- **Generators:**
  - Random valid contributor IDs (ASCII strings with `@`, varying lengths).
  - Random positive revisions up to safe integer bound `9007199254740991`.
  - Version clocks $U$, $V$, $W$ containing between 0 and 10 components.
- **Invariants Checked:**
  - **Idempotence of Join:** $\text{join}(V, V) == V$.
  - **Commutativity of Join:** $\text{join}(V, W) == \text{join}(W, V)$.
  - **Associativity of Join:** $\text{join}(\text{join}(U, V), W) == \text{join}(U, \text{join}(V, W))$.
  - **Strict Monotonicity:** If $V < W$, then $\text{join}(V, W) == W$.
  - **Trichotomy & Concurrency:** Exactly one relation holds between $V$ and $W$: $V == W$, $V < W$, $V > W$, or $V \parallel W$.
  - **Snap Order Totality:** For any two versions $V \neq W$, either $V <_{\text{snap}} W$ or $W <_{\text{snap}} V$.

### Property Test 2: Canonical Text Diff Invertibility & Minimality
- **Generators:**
  - Arbitrary token sequences $A$ and $B$ composed of generated Unicode lines with/without LF.
- **Invariants Checked:**
  - **Correctness:** Applying $\text{diff}(A, B)$ to $A$ produces $B$ exactly.
  - **Identity:** $\text{diff}(A, A)$ produces an edit script containing only `retain(length(A))`.
  - **Coalescing:** No two adjacent edit operations in the emitted script have the same operation variant.

### Property Test 3: Operational Transformation Concurrency Invariants
- **Generators:**
  - Base token sequence $S$.
  - Independent valid edit scripts $P$ and $Q$ authored against base $S$.
- **Invariants Checked:**
  - **Base Length Parity:** Transformed script $P'$ consumes exactly the output tokens of $Q$.
  - **Insertion Preservation:** Every token inserted by $P$ is present in the final transformed output.
  - **No Duplicate Deletion:** If both $P$ and $Q$ delete base token index $k$, token $k$ is deleted exactly once without index misalignment.

### Property Test 4: Replay Determinism on Random Causal Patch DAGs
- **Generators:**
  - Generate valid acyclic causal patch graphs with up to 5 contributors and 20 total patches.
  - Ensure every patch satisfies `revision = base[author] + 1` and base closure.
- **Invariants Checked:**
  - **Permutation Invariance:** Replaying any random topological ordering of the patch set produces the exact same file tree and the exact same warning list.
  - **Prefix Freedom:** The materialized file tree never contains both a file path `a` and any descendant `a/...`.

---

## 4. Platform & Environment-Specific Scenarios

### Scenario ENV.1: PTY & Terminal Auto-Detection Matrix
- **Scope:** Verify the matrix of `auto` mode handling when stdout is a TTY and stderr is a pipe, and vice versa.
- **Execution:**
  - Use pseudo-terminal (PTY) helpers to spawn candidate subprocesses with split stream allocations.
  - Verify that standard output receives ANSI escapes while redirected standard error remains plain.

### Scenario ENV.2: Cross-Platform File Paths & Case Preservation
- **Scope:** macOS / Linux filesystem compatibility.
- **Execution:**
  - Assert that file paths with distinct ASCII casing (`File.txt` vs `file.txt`) are tracked as distinct paths without normalization.
  - Verify rejection of backslashes (`\`) on both Unix and Windows environments.

---

## 5. Traceability Matrix to Public YAML Acceptance Tests

| Domain | Functional Focus | Canonical YAML Test Suites in `tests/` |
|---|---|---|
| **Domain A** | Version algebra, parsing, join, Snap order | `19-version-boundaries.yaml`, `21-version-algebra.yaml`, `25-config-version-path-boundaries.yaml` |
| **Domain B** | Schema, serial contributor, closures | `15-repository-validation.yaml`, `16-dot-collision.yaml`, `23-strict-validation-matrix.yaml` |
| **Domain C** | Text diff, tokenization, binary detection | `05-diff-goldens.yaml`, `06-binary-and-empty.yaml` |
| **Domain D** | OT matrix, 3-way concurrent text edits | `22-ot-matrix.yaml` |
| **Domain E** | Replay engine, namespace & path rules | `09-merge-text.yaml`, `10-merge-conflicts.yaml`, `11-namespace-conflicts.yaml`, `17-concurrent-creates.yaml` |
| **Domain F** | Scanner, unsupported entries, atomic write | `01-init.yaml`, `02-init-paths.yaml`, `08-unsupported-entries.yaml`, `26-portability-and-failure-safety.yaml` |
| **Domain G** | Configuration hierarchy | `03-configuration.yaml` |
| **Domain H** | CLI grammar, commands, errors | `04-commit-status-log.yaml`, `14-cli-errors.yaml`, `24-cli-grammar-matrix.yaml` |
| **Domain I** | ANSI presentation, color negotiation | `28-terminal-presentation.yaml` |
| **Domain J** | HTTP server snapshot & client | `12-http-server.yaml`, `13-http-client.yaml` |
| **Domain K** | Multi-repo convergence & 3-way merges | `18-three-way-convergence.yaml`, `20-dirty-merge.yaml`, `27-history-canonicality.yaml` |
| **Domain L** | Fault tolerance & dirty tree protection | `07-revert.yaml`, `20-dirty-merge.yaml`, `26-portability-and-failure-safety.yaml` |
