# Implementation Plan - Phase 4: Filesystem Scanner, Path Validation & Materializer

Snap requires safe, deterministic, and atomic filesystem operations. This phase builds the filesystem layer (`rust/src/fs/`) responsible for tracked path validation, prefix-freedom checks, working tree scanning with clean/dirty detection and symlink rejection, and atomic file materialization with failure safety.

## User Review Required

> [!IMPORTANT]
> - **Symlink & Special Entry Policy:** All non-regular filesystem entries (symlinks, FIFOs, sockets, devices) encountered during scanning MUST immediately fail with `ScanError::UnsupportedEntry("<relpath>")` without following symlinks.
> - **Atomic Replacement Safety:** Repository metadata (`repository.json`) is always written via a same-directory temporary file followed by an atomic `rename` operation to prevent metadata corruption during aborted writes.
> - **Zero Panics / No `unwrap()`:** Production code in `src/fs/` will strictly propagate all I/O errors and adhere to the Clippy anti-pattern rules established in Phase 3.

## Proposed Changes

### Filesystem Layer (`rust/src/fs/`)

#### [NEW] [`paths.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/fs/paths.rs)
- Relocate / unify `validate_tracked_path` and `PathError`.
- Implement `check_prefix_free` to verify that a set or slice of paths contains no file-directory segment collisions (i.e. if `a` is present, `a/...` cannot be present).
- Implement unsigned lexicographic UTF-8 path ordering utilities.

#### [NEW] [`scanner.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/fs/scanner.rs)
- Implement `ScanError` with `UnsupportedEntry(String)`, `IoError(String)`, and `InvalidPath(String)`.
- Implement `scan_working_tree(repo_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, ScanError>`:
  - Traverses directory recursively using `fs::symlink_metadata` (never following symlinks).
  - Skips root `.snap` directory.
  - Ignores empty directories.
  - Rejects any symlink, FIFO, socket, or device with `ScanError::UnsupportedEntry(rel_path)`.
  - Reads regular file bytes and normalizes relative paths with `/`.
- Implement `diff_working_tree`:
  - Compares scanned working tree against target/current tree.
  - Generates sorted list of changes: `Added` (`A`), `Modified` (`M`), and `Deleted` (`D`).
  - Identifies clean state when byte contents match exactly.

#### [NEW] [`materializer.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/fs/materializer.rs)
- Implement `MaterializeError` covering I/O failures, serialization errors, and atomic rename failures.
- Implement `atomic_replace_file(dest_path: &Path, content: &[u8]) -> Result<(), MaterializeError>`:
  - Creates a temporary file in the same parent directory (`dest_path.with_extension("tmp.<rand>")`).
  - Writes and flushes bytes.
  - Atomically renames temporary file over `dest_path`.
  - Cleans up temporary file on failure, leaving target untouched.
- Implement `materialize_tree(repo_root: &Path, current_tree: &BTreeMap<String, Vec<u8>>, target_tree: &BTreeMap<String, Vec<u8>>) -> Result<(), MaterializeError>`:
  - Deletes removed files.
  - Removes obstructing files that block required directories.
  - Creates missing directories.
  - Writes updated and added files.
  - Cleans up newly empty intermediate directories.
- Implement `write_repository_atomic(snap_dir: &Path, repo: &Repository) -> Result<(), MaterializeError>`.

#### [NEW] [`mod.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/fs/mod.rs)
- Re-exports `paths`, `scanner`, and `materializer` types.

#### [MODIFY] [`main.rs`](file:///Users/coon/workspace-zv/git/snap/rust/src/main.rs)
- Declare `pub mod fs;`.

---

## Verification Plan

### Automated Tests
1. **Compilation & Static Checks:**
   ```bash
   cd rust
   cargo check
   cargo clippy --all-targets -- -D warnings
   cargo fmt --check
   ```
2. **Unit & Integration Tests (`cargo test`):**
   - `test_scenario_f1_symlink_and_special_file_rejection`: creates symlinks and fifos, verifies exact unsupported entry error without following.
   - `test_scenario_f2_working_tree_clean_vs_dirty_detection`: asserts timestamp modification is clean, byte change is `M`, file addition is `A`, file deletion is `D`.
   - `test_scenario_f3_atomic_metadata_replacement_safety`: tests atomic replace leaves original file intact upon write failure.
   - `test_path_validation_and_prefix_freedom`: tests path syntax and segment prefix-freedom enforcement.
   - `test_materialize_tree_lifecycle`: verifies file creation, removal, and cleanup of empty directories.
3. **Subprocess / Binary Check:**
   ```bash
   ./run --lang rust --version
   ```
