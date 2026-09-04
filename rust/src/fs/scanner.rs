use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::fs::paths::{validate_tracked_path, PathError};

/// Errors encountered when scanning a working tree.
#[derive(Debug)]
pub enum ScanError {
    UnsupportedEntry(String),
    InvalidPath { path: String, source: PathError },
    Io { path: String, source: io::Error },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::UnsupportedEntry(path) => {
                write!(f, "unsupported working tree entry: {path}")
            }
            ScanError::InvalidPath { path, source } => {
                write!(f, "invalid tracked path '{path}': {source}")
            }
            ScanError::Io { path, source } => {
                write!(f, "filesystem error at '{path}': {source}")
            }
        }
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScanError::UnsupportedEntry(_) => None,
            ScanError::InvalidPath { source, .. } => Some(source),
            ScanError::Io { source, .. } => Some(source),
        }
    }
}

/// Status of a file relative to a reference target tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

impl FileStatus {
    /// Return standard single-character status symbol ('A', 'M', 'D').
    pub fn symbol(&self) -> char {
        match self {
            FileStatus::Added => 'A',
            FileStatus::Modified => 'M',
            FileStatus::Deleted => 'D',
        }
    }
}

/// A changed file entry in the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingChange {
    pub path: String,
    pub status: FileStatus,
}

/// The set of differences between the working tree and a target tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingTreeDiff {
    pub changes: Vec<WorkingChange>,
}

impl WorkingTreeDiff {
    /// Returns true if the working tree has no differences against the target tree.
    pub fn is_clean(&self) -> bool {
        self.changes.is_empty()
    }
}

/// Recursively scan the working tree below `repo_root`.
///
/// Returns a map of tracked relative paths to their raw byte content.
///
/// Rules per SPEC §2 & §7.10:
/// - Skips root `.snap/` directory and its contents.
/// - Empty directories are ignored.
/// - Rejects symlinks and other non-regular entries immediately with `ScanError::UnsupportedEntry`.
/// - Never follows symlinks (uses `symlink_metadata`).
pub fn scan_working_tree(repo_root: &Path) -> Result<BTreeMap<String, Vec<u8>>, ScanError> {
    let mut tree = BTreeMap::new();
    scan_dir_recursive(repo_root, "", &mut tree)?;
    Ok(tree)
}

fn scan_dir_recursive(
    dir_path: &Path,
    rel_prefix: &str,
    tree: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), ScanError> {
    let entries = fs::read_dir(dir_path).map_err(|e| ScanError::Io {
        path: dir_path.display().to_string(),
        source: e,
    })?;

    // Sort directory entries by file name for deterministic traversal
    let mut sorted_entries = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| ScanError::Io {
            path: dir_path.display().to_string(),
            source: e,
        })?;
        sorted_entries.push(entry);
    }
    sorted_entries.sort_by_key(|e| e.file_name());

    for entry in sorted_entries {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();

        // Skip root .snap directory
        if rel_prefix.is_empty() && name_str == crate::fs::paths::SNAP_DIR {
            continue;
        }

        let rel_path = if rel_prefix.is_empty() {
            name_str.to_string()
        } else {
            format!("{rel_prefix}/{name_str}")
        };

        let full_path = entry.path();
        let symlink_meta = fs::symlink_metadata(&full_path).map_err(|e| ScanError::Io {
            path: full_path.display().to_string(),
            source: e,
        })?;

        let file_type = symlink_meta.file_type();

        if file_type.is_symlink() {
            return Err(ScanError::UnsupportedEntry(rel_path));
        }

        if file_type.is_dir() {
            scan_dir_recursive(&full_path, &rel_path, tree)?;
        } else if file_type.is_file() {
            validate_tracked_path(&rel_path).map_err(|e| ScanError::InvalidPath {
                path: rel_path.clone(),
                source: e,
            })?;

            let bytes = fs::read(&full_path).map_err(|e| ScanError::Io {
                path: full_path.display().to_string(),
                source: e,
            })?;

            tree.insert(rel_path, bytes);
        } else {
            // FIFOs, sockets, block/char devices, etc.
            return Err(ScanError::UnsupportedEntry(rel_path));
        }
    }

    Ok(())
}

/// Compute the difference between a scanned working tree and a target tree.
///
/// Changes are returned in sorted order by path.
pub fn diff_working_tree(
    working_tree: &BTreeMap<String, Vec<u8>>,
    target_tree: &BTreeMap<String, Vec<u8>>,
) -> WorkingTreeDiff {
    let mut all_paths: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for p in working_tree.keys() {
        all_paths.insert(p.as_str());
    }
    for p in target_tree.keys() {
        all_paths.insert(p.as_str());
    }

    let mut changes = Vec::new();

    for path in all_paths {
        let in_working = working_tree.get(path);
        let in_target = target_tree.get(path);

        match (in_working, in_target) {
            (Some(_), None) => {
                changes.push(WorkingChange {
                    path: path.to_string(),
                    status: FileStatus::Added,
                });
            }
            (None, Some(_)) => {
                changes.push(WorkingChange {
                    path: path.to_string(),
                    status: FileStatus::Deleted,
                });
            }
            (Some(w_bytes), Some(t_bytes)) => {
                if w_bytes != t_bytes {
                    changes.push(WorkingChange {
                        path: path.to_string(),
                        status: FileStatus::Modified,
                    });
                }
            }
            (None, None) => {}
        }
    }

    WorkingTreeDiff { changes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_f2_working_tree_clean_vs_dirty_detection() {
        let mut target_tree = BTreeMap::new();
        target_tree.insert("f1.txt".to_string(), b"hello\n".to_vec());
        target_tree.insert("f2.txt".to_string(), b"world\n".to_vec());

        // State 0: Identical trees -> clean
        let working_tree = target_tree.clone();
        let diff0 = diff_working_tree(&working_tree, &target_tree);
        assert!(diff0.is_clean());
        assert_eq!(diff0.changes.len(), 0);

        // State 1: Touch / identical bytes -> clean
        let mut working_tree1 = target_tree.clone();
        working_tree1.insert("f1.txt".to_string(), b"hello\n".to_vec());
        let diff1 = diff_working_tree(&working_tree1, &target_tree);
        assert!(diff1.is_clean());

        // State 2: 1-byte change -> Modified (M)
        let mut working_tree2 = target_tree.clone();
        working_tree2.insert("f1.txt".to_string(), b"Hello\n".to_vec());
        let diff2 = diff_working_tree(&working_tree2, &target_tree);
        assert!(!diff2.is_clean());
        assert_eq!(
            diff2.changes,
            vec![WorkingChange {
                path: "f1.txt".to_string(),
                status: FileStatus::Modified,
            }]
        );

        // State 3: Add new file -> Added (A)
        let mut working_tree3 = target_tree.clone();
        working_tree3.insert("f3.txt".to_string(), b"new\n".to_vec());
        let diff3 = diff_working_tree(&working_tree3, &target_tree);
        assert!(!diff3.is_clean());
        assert_eq!(
            diff3.changes,
            vec![WorkingChange {
                path: "f3.txt".to_string(),
                status: FileStatus::Added,
            }]
        );

        // State 4: Remove tracked file -> Deleted (D)
        let mut working_tree4 = target_tree.clone();
        working_tree4.remove("f2.txt");
        let diff4 = diff_working_tree(&working_tree4, &target_tree);
        assert!(!diff4.is_clean());
        assert_eq!(
            diff4.changes,
            vec![WorkingChange {
                path: "f2.txt".to_string(),
                status: FileStatus::Deleted,
            }]
        );
    }
}
