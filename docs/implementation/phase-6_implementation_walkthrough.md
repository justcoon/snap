# Phase 6 Implementation Walkthrough: Advanced Replay Commands (`diff`, `revert`, `merge`)

## 1. Executive Summary
Phase 6 implements the advanced replay-based commands of Snap in Rust: `diff`, `revert`, and `merge`.
All implementations adhere strictly to `SPEC.md` (§5, §6.4, §7.6, §7.7, §7.8), `plan.md` (Phase 6), and `test-scenarios.md` (Domains C, E, K, L).

All 6 canonical verification gate acceptance test suites (`05-diff-goldens.yaml`, `06-binary-and-empty.yaml`, `07-revert.yaml`, `09-merge-text.yaml`, `10-merge-conflicts.yaml`, `11-namespace-conflicts.yaml`) as well as `08`, `14`, `16`, `17`, `18`, `19`, `20`, `21`, `22`, `24`, `25`, and `27` pass with 100% compliance.

---

## 2. Key Changes & File Layout

### A. Strongly Typed CLI Grammar & Dispatch
- **`rust/src/cli/args.rs`:**
  - Added `DiffTarget` enum: `WorkingTree` and `Versions { old: String, new: String, repo: Option<String> }`.
  - Refined `Command` enum:
    - `Command::Diff(DiffTarget)`
    - `Command::Revert { version: String }`
    - `Command::Merge { repo: String }`
  - Enforced exact positional syntax matching the CLI grammar matrix.
- **`rust/src/cli/mod.rs`:**
  - Exposed `diff_format` module and dispatched `Diff`, `Revert`, and `Merge` commands.

### B. Unified & Binary Diff Formatter
- **`rust/src/cli/diff_format.rs`:**
  - Implemented `format_tree_diff(&old_tree, &new_tree) -> Result<String, DiffError>`:
    - Sorts all paths in unsigned UTF-8 byte order.
    - Emits `Binary files <a_side> and <b_side> differ\n` when either side is binary, substituting `/dev/null` for absent sides.
    - Emits unified text diffs with whole-file headers `--- a/<path>\n+++ b/<path>\n` (or `/dev/null`).
    - Hunk headers formatted as `@@ -1,<old_count> +1,<new_count> @@\n`.
    - Handles missing trailing newlines with `\ No newline at end of file\n` markers.

### C. Repository Validation & History Inspection
- **`rust/src/core/validation.rs`:**
  - Updated `DotCollisionDifferentPayload` error message to match `repository corruption: patch collision: {author} revision {revision}`.
  - Implemented `is_version_known(repo: &Repository, version: &Version) -> bool` enforcing §4.1: verifies all revisions $1..=V[c]$ exist in `repo.patches` and their base dependencies are closed within $V$.

### D. Advanced Command Implementations
- **`rust/src/cli/commands.rs`:**
  - **`cmd_diff`:**
    - Handles `DiffTarget::WorkingTree`: scans working tree, materializes current tree, and outputs unified diff (empty on clean).
    - Handles `DiffTarget::Versions`: verifies `old` and `new` versions are known, loads remote repository if `--repo` is specified while verifying dot consistency, materializes trees, and prints unified diff.
  - **`cmd_revert`:**
    - Verifies target version is valid and known locally.
    - Validates contributor identity and clean working tree.
    - Guards against identical target tree (`snap: target tree is already current`).
    - Authors an additive revert patch with message `revert to <version>`, materializes target files, updates `repository.json` atomically, and outputs the new version.
  - **`cmd_merge`:**
    - Enforces clean working tree without requiring contributor configuration.
    - Loads and strictly validates remote repository.
    - Checks common dots for collisions (`snap: patch collision: <author> revision <rev>`).
    - Detects already-contained history as an idempotent no-op (silent stderr, outputting unchanged version).
    - Unions patch closures, sorts canonical order (`author` ascending, then `revision` ascending), and joins frontiers.
    - Replays merged history canonically, diffs warnings against pre-merge local warnings (`merged_warnings.difference(&local_warnings)`), materializes merged tree, atomically updates `repository.json`, emits warnings to stderr, and prints joined version to stdout.
  - **Patch Canonical Ordering:**
    - Ensured `cmd_commit` and `cmd_revert` sort patches by author and numeric revision.
- **`rust/src/config/mod.rs`:**
  - Parsed `RawContributorConfig` with `ContributorId::parse` to emit `ConfigError::InvalidContributorId` matching `^snap: invalid contributor id: .+\n$`.

---

## 3. Discrepancy Check (Plan vs. Implementation)

| Area | Approved Plan | Final Implementation | Discrepancy / Resolution |
| :--- | :--- | :--- | :--- |
| **`DiffTarget` grammar** | `WorkingTree` and `Versions { old, new, repo }` | Implemented in `args.rs` | None. |
| **Diff Formatting** | Unified diff with `/dev/null`, hunk headers, `\ No newline at end of file` | Implemented in `diff_format.rs` | None. Matches golden diff fixtures byte-for-byte. |
| **Revert Patch** | Additive patch with `revert to <version>` message | Implemented in `commands.rs` | None. Never moves frontier backward. |
| **Revert No-Op Error** | `snap: target tree is already current` | Implemented in `commands.rs` | None. Exits 1 with exact error. |
| **Merge Idempotence** | If contained or equal: no-op, silent stderr | Implemented in `commands.rs` | None. |
| **Merge Warning Diff** | `merged_warnings.difference(&local_warnings)` | Implemented in `commands.rs` | None. Emits only newly introduced auto-resolutions sorted by path and reason. |
| **Dot Collisions** | Compare common dots across repos, error if unequal | Implemented in `check_dot_collisions` | None. Rejects with `snap: patch collision: <author> revision <rev>`. |
| **Canonical Patch Sort** | Patches sorted by author ascending, then revision | Implemented in `cmd_commit`, `cmd_revert`, and `cmd_merge` | Corrected initial commit insertion to ensure canonical sorting invariant. |

---

## 4. Verification Results

### A. In-Process Toolchain Checks
```bash
cargo check && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
- **`cargo check`:** Clean, 0 errors.
- **`cargo fmt --check`:** Clean, 100% compliant with standard rustfmt.
- **`cargo clippy`:** Clean, 0 warnings with `-D warnings` and custom anti-pattern lints.
- **`cargo test`:** 38 passed, 0 failed.

### B. Canonical Acceptance Verification Gates (`./verify --lang rust`)
| Suite | Domain | Status |
| :--- | :--- | :--- |
| `05-diff-goldens.yaml` | Diff formatting, repeated lines, missing newlines | **PASSED** |
| `06-binary-and-empty.yaml` | Binary file diffs, empty file versioning | **PASSED** |
| `07-revert.yaml` | Additive revert, file-directory transitions, target equality guard | **PASSED** |
| `08-unsupported-entries.yaml` | Working tree symlink and FIFO rejection | **PASSED** |
| `09-merge-text.yaml` | Concurrent text edits convergence and idempotence | **PASSED** |
| `10-merge-conflicts.yaml` | Path conflict winner rules and sorted warnings | **PASSED** |
| `11-namespace-conflicts.yaml` | File vs directory collision convergence (`namespace-wins`) | **PASSED** |
| `14-cli-errors.yaml` | Grammar errors, diff usage, unknown version | **PASSED** |
| `16-dot-collision.yaml` | Cross-repository dot collision detection | **PASSED** |
| `17-concurrent-creates.yaml` | Concurrent creates resolution | **PASSED** |
| `18-three-way-convergence.yaml` | Three-way text convergence and merge associativity | **PASSED** |
| `19-version-boundaries.yaml` | Known causal frontiers and revert to empty tree | **PASSED** |
| `20-dirty-merge.yaml` | Refusal of merge on dirty or unsupported working trees | **PASSED** |
| `21-version-algebra.yaml` | Vector clock operations and Snap total order | **PASSED** |
| `22-ot-matrix.yaml` | Operational transformation table | **PASSED** |
| `24-cli-grammar-matrix.yaml` | Strict CLI grammar matrix | **PASSED** |
| `25-config-version-path-boundaries.yaml` | Strict boundary validation | **PASSED** |
| `27-history-canonicality.yaml` | Repository schema and patch sequence validation | **PASSED** |

22 out of 28 acceptance test suites in the repository pass (the remaining 6 belong to future Phase 7 HTTP server/client, Phase 8 ANSI presentation, and Phase 9 schema error wording).
All Phase 6 gates pass 100%.

---

## 5. Next Steps & User Confirmation
Phase 6 implementation and verification are complete. As per our development guidelines, I will not create a git commit without your explicit approval.

Please confirm if you would like me to proceed with committing the Phase 6 changes.
