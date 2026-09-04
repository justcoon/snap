# Phase 6 Implementation Plan: Advanced Replay Commands (`diff`, `revert`, `merge`)

## 1. Objectives & Scope
Implement Phase 6 of the Snap implementation roadmap according to `plan.md` (§7 Phase 6), `SPEC.md` (§5, §6.4, §7.6, §7.7, §7.8), and `test-scenarios.md` (Domains C, E, K, L).

Phase 6 introduces three core commands relying on deterministic replay, file tree diffing, and working tree safety:
1. **`snap diff`**:
   - Compares current tree vs. working tree (no arguments).
   - Compares two locally known versions (`snap diff <old> <new>`).
   - Compares across repositories (`snap diff <old> <new> --repo <repository>`), resolving `old` locally and `new` remotely without importing, validating both repositories and asserting dot consistency.
   - Whole-file unified diff formatting conforming to §5 & §7.6, including `/dev/null` headers, `@@ -1,<old> +1,<new> @@` hunk headers, `\ No newline at end of file` markers, and binary file change markers.
2. **`snap revert <version>`**:
   - Verifies target version is known and valid in local history.
   - Enforces contributor configuration and clean working tree.
   - Rejects revert if target tree is already identical to current tree (`snap: target tree is already current`).
   - Authors an additive revert patch with message `revert to <version>`, materializes target files, atomically updates `repository.json`, and outputs the new version.
3. **`snap merge <repository>`**:
   - Enforces clean working tree without requiring contributor identity.
   - Resolves and validates the other repository (local path or repository.json).
   - Verifies dot consistency (rejection of duplicate dots with differing payloads).
   - Detects already-contained or identical history (silent no-op, outputting unchanged version).
   - Unions patch closures, joins frontiers, canonically replays, and calculates warning diff: `merged_warnings.difference(&local_warnings)`.
   - Atomically updates working tree and `repository.json`, emits new warnings to stderr, and prints joined version to stdout.

---

## 2. Technical Architecture & File Layout

### A. Strongly Typed CLI Grammar (`rust/src/cli/args.rs`)
- Expand `Command` enum:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum DiffTarget {
      WorkingTree,
      Versions {
          old: String,
          new: String,
          repo: Option<String>,
      },
  }

  pub enum Command {
      ...
      Diff(DiffTarget),
      Revert { version: String },
      Merge { repo: String },
      ...
  }
  ```
- Refine argument validation to enforce exact positional order:
  - `diff` -> `DiffTarget::WorkingTree`
  - `diff <old> <new>` -> `DiffTarget::Versions { old, new, repo: None }`
  - `diff <old> <new> --repo <repo>` -> `DiffTarget::Versions { old, new, repo: Some(repo) }`
  - Any deviation -> `ParseError::DiffUsage` (`snap: usage: snap diff [<old> <new> [--repo <repository>]]\n`).
  - `revert <version>` (exactly 1 argument) -> `ParseError::InvalidCommandOrArguments`.
  - `merge <repo>` (exactly 1 argument) -> `ParseError::InvalidCommandOrArguments`.

### B. Unified & Binary Diff Formatter (`rust/src/cli/diff_format.rs` or `rust/src/core/diff.rs`)
- Implement `format_tree_diff(old_tree: Option<&FileTree>, new_tree: Option<&FileTree>) -> Result<String, DiffError>`:
  - Collect and sort all keys in `old_tree` and `new_tree` (unsigned UTF-8 byte order).
  - Skip identical entries.
  - If either old or new entry is binary (`!is_text(bytes)`):
    - Output: `Binary files <a_side> and <b_side> differ\n` where absent side is `/dev/null` and present side is `a/<path>` or `b/<path>`.
  - If text:
    - Determine headers: `--- <a_path>\n+++ <b_path>\n` with `/dev/null` for absent side.
    - Tokenize non-absent sides with `tokenize_text`.
    - Hunk header: `@@ -1,<old_count> +1,<new_count> @@\n`.
    - Compute edit script with canonical DP `diff_tokens(old_tokens, new_tokens)`.
    - Render operations:
      - `Retain(n)`: emit ` <token>` for `n` tokens from `old_tokens`.
      - `Delete(n)`: emit `-<token>` for `n` tokens from `old_tokens`.
      - `Insert(tokens)`: emit `+<token>` for each token.
      - If any token lacks trailing `\n`, append `\n\ No newline at end of file\n`.

### C. History Inspection & Dot Verification Helper (`rust/src/core/validation.rs` or `rust/src/cli/commands.rs`)
- Implement `is_version_known(repo: &Repository, version: &Version) -> bool`:
  - Enforces §4.1: Every patch `(c, n)` selected by $n \le V[c]$ must exist in `repo.patches`.
  - The selected set must contain the complete base of every selected patch.
- Implement `check_dot_collisions(local: &Repository, remote: &Repository) -> Result<(), String>`:
  - If local and remote both contain dot `(author, revision)`, verify `local_patch == remote_patch`.
  - If not identical, error: `patch collision: <author> revision <rev>`.
- Refine `ValidationError::DotCollisionDifferentPayload` display format to match `patch collision: <author> revision <revision>`.

### D. Replay Commands Implementation (`rust/src/cli/commands.rs`)
- **`cmd_diff`**:
  - For `DiffTarget::WorkingTree`:
    - Find repo root, validate repository.
    - Scan working tree (error on unsupported entries).
    - Materialize `current_tree` at `repo.frontier`.
    - Format and print diff (stdout empty on clean).
  - For `DiffTarget::Versions`:
    - Parse `old` and `new` with `Version::parse` (fail with `snap: invalid version: <err>` on syntax error).
    - Validate `old` is known in local repo (fail with `snap: unknown version: <old>` if not).
    - If `--repo` specified:
      - Load and validate remote repository.
      - Assert no dot collisions between local and remote.
      - Validate `new` is known in remote repo.
      - Materialize `old_tree` from local repo and `new_tree` from remote repo.
    - Else:
      - Validate `new` is known in local repo.
      - Materialize `old_tree` and `new_tree` from local repo.
    - Format and print diff.
- **`cmd_revert`**:
  - Parse target version (syntax check).
  - Find repo root, validate local repo.
  - Check target version is known in local repo.
  - Resolve contributor configuration.
  - Scan working tree (reject dirty or unsupported entries).
  - Materialize `current_tree` and `target_tree`.
  - Reject if `current_tree == target_tree` with `snap: target tree is already current`.
  - Author additive patch with base `repo.frontier`, revision `frontier[contributor] + 1`, and message `revert to <version>`.
  - Materialize target files to disk via `materialize_tree`.
  - Atomically write updated `repository.json`.
  - Print new version to stdout.
- **`cmd_merge`**:
  - Find repo root, validate local repo.
  - Scan working tree (reject dirty or unsupported entries).
  - Materialize `local_tree` and `local_warnings` at `local_repo.frontier`.
  - Load and validate remote repository (from directory path or direct JSON path).
  - Check dot collisions.
  - Check if remote is already contained:
    `joined_frontier = local_repo.frontier.join(&remote_repo.frontier)`.
    If `joined_frontier == local_repo.frontier` (and all remote dots are present):
      Print `joined_frontier` to stdout, exit 0 without writes.
  - Union patches, sort canonical order (`author` ascending, then `revision` ascending).
  - Validate unioned repository graph.
  - Replay merged repository: `(merged_tree, merged_warnings) = materialize_version(&merged_repo.patches, &joined_frontier)`.
  - Compute new warnings: `new_warnings = merged_warnings.difference(&local_warnings)`.
  - Materialize `merged_tree` onto filesystem.
  - Atomically replace `repository.json`.
  - Print each warning in `new_warnings` to stderr.
  - Print `joined_frontier` to stdout.

---

## 3. Strict Quality & Anti-Pattern Compliance
- **Zero Panics (`.unwrap()` / `.expect()`):** All file operations, parsing, and JSON operations must propagate errors via `Result` and `?`.
- **Clippy Check:** Enforced by `-D warnings` and `[lints.clippy]` table in `Cargo.toml`.
- **POSIX Atomicity:** All disk mutations use temporary files with rename in `.snap/`.
- **Failure Safety:** No working tree or repository modifications if validation or dirty tree checks fail.

---

## 4. Verification Plan

### Automated In-Process Unit Tests
- `cargo test`:
  - Unified diff formatting tests with added, deleted, modified lines, empty files, binary files, and missing trailing newlines.
  - Version known-ness check unit tests.
  - Revert patch authoring and target tree equality checks.
  - Warning diffing logic and dot collision verification.

### Canonical Acceptance Verification Gates (`./verify --lang rust`)
- `05-diff-goldens.yaml`
- `06-binary-and-empty.yaml`
- `07-revert.yaml`
- `08-unsupported-entries.yaml`
- `09-merge-text.yaml`
- `10-merge-conflicts.yaml`
- `11-namespace-conflicts.yaml`
- `14-cli-errors.yaml`
- `16-dot-collision.yaml`
- `19-version-boundaries.yaml`
- `20-dirty-merge.yaml`
- `24-cli-grammar-matrix.yaml`
- `25-config-version-path-boundaries.yaml`
- `26-portability-and-failure-safety.yaml` (local portions)
