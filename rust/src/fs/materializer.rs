use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::patch::Repository;

/// Errors encountered during filesystem materialization or atomic metadata replacement.
#[derive(Debug)]
pub enum MaterializeError {
    Io { path: String, source: io::Error },
    Serialization(serde_json::Error),
}

impl fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaterializeError::Io { path, source } => {
                write!(f, "filesystem error at '{path}': {source}")
            }
            MaterializeError::Serialization(e) => {
                write!(f, "serialization error: {e}")
            }
        }
    }
}

impl std::error::Error for MaterializeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MaterializeError::Io { source, .. } => Some(source),
            MaterializeError::Serialization(e) => Some(e),
        }
    }
}

/// Atomically replace `dest_path` with `content` using a temporary file in the same directory.
///
/// If write or rename fails, the temporary file is deleted and `dest_path` remains untouched.
pub fn atomic_replace_file(dest_path: &Path, content: &[u8]) -> Result<(), MaterializeError> {
    let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));

    // Ensure parent directory exists
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| MaterializeError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    let file_name = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let pid = std::process::id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let temp_name = format!(".{file_name}.tmp.{pid}.{timestamp}");
    let temp_path = parent.join(temp_name);

    if let Err(e) = fs::write(&temp_path, content) {
        let _ = fs::remove_file(&temp_path);
        return Err(MaterializeError::Io {
            path: temp_path.display().to_string(),
            source: e,
        });
    }

    if let Err(e) = fs::rename(&temp_path, dest_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(MaterializeError::Io {
            path: dest_path.display().to_string(),
            source: e,
        });
    }

    Ok(())
}

/// Atomically write `.snap/repository.json`.
pub fn write_repository_atomic(snap_dir: &Path, repo: &Repository) -> Result<(), MaterializeError> {
    let json_string =
        serde_json::to_string_pretty(repo).map_err(MaterializeError::Serialization)?;
    let mut bytes = json_string.into_bytes();
    bytes.push(b'\n');

    let repo_file = snap_dir.join(crate::fs::paths::REPOSITORY_FILE);
    atomic_replace_file(&repo_file, &bytes)
}

/// Materialize `target_tree` onto the working filesystem rooted at `repo_root`.
///
/// Conforms to SPEC §6.2:
/// - Removes deleted paths.
/// - Removes regular files that block required directories.
/// - Creates required directories.
/// - Writes target files.
/// - Cleans up newly empty directories.
pub fn materialize_tree(
    repo_root: &Path,
    current_tree: &BTreeMap<String, Vec<u8>>,
    target_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<(), MaterializeError> {
    let mut candidate_dirs_to_clean: HashSet<PathBuf> = HashSet::new();

    // 1. Remove paths present in current_tree but absent in target_tree
    for path in current_tree.keys() {
        if !target_tree.contains_key(path) {
            let full_path = repo_root.join(path);
            if full_path.exists() {
                fs::remove_file(&full_path).map_err(|e| MaterializeError::Io {
                    path: full_path.display().to_string(),
                    source: e,
                })?;
                if let Some(parent) = full_path.parent() {
                    candidate_dirs_to_clean.insert(parent.to_path_buf());
                }
            }
        }
    }

    // 2. Remove files that block required directories
    for target_path in target_tree.keys() {
        let mut idx = 0;
        while let Some(slash_pos) = target_path[idx..].find('/') {
            let actual_slash_pos = idx + slash_pos;
            let ancestor_rel = &target_path[..actual_slash_pos];
            let ancestor_full = repo_root.join(ancestor_rel);
            if ancestor_full.is_file() {
                fs::remove_file(&ancestor_full).map_err(|e| MaterializeError::Io {
                    path: ancestor_full.display().to_string(),
                    source: e,
                })?;
                if let Some(parent) = ancestor_full.parent() {
                    candidate_dirs_to_clean.insert(parent.to_path_buf());
                }
            }
            idx = actual_slash_pos + 1;
        }
    }

    // 3. Write target files
    for (rel_path, target_bytes) in target_tree {
        let full_path = repo_root.join(rel_path);

        // If a directory currently occupies this path, remove it
        if full_path.is_dir() {
            fs::remove_dir_all(&full_path).map_err(|e| MaterializeError::Io {
                path: full_path.display().to_string(),
                source: e,
            })?;
        }

        // If file already exists with identical bytes, skip rewrite
        if full_path.is_file() {
            if let Ok(existing_bytes) = fs::read(&full_path) {
                if &existing_bytes == target_bytes {
                    continue;
                }
            }
        }

        // Ensure parent directories exist
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| MaterializeError::Io {
                    path: parent.display().to_string(),
                    source: e,
                })?;
            }
        }

        fs::write(&full_path, target_bytes).map_err(|e| MaterializeError::Io {
            path: full_path.display().to_string(),
            source: e,
        })?;
    }

    // 4. Remove empty directories bottom-up
    clean_empty_dirs(repo_root, candidate_dirs_to_clean);

    Ok(())
}

fn clean_empty_dirs(repo_root: &Path, candidate_dirs: HashSet<PathBuf>) {
    let mut sorted_dirs: Vec<PathBuf> = candidate_dirs.into_iter().collect();
    // Sort deepest first (longest path string first)
    sorted_dirs.sort_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));

    for dir in sorted_dirs {
        let mut curr = dir;
        while curr != repo_root && curr.starts_with(repo_root) {
            // Try to remove directory if empty
            if fs::remove_dir(&curr).is_ok() {
                if let Some(parent) = curr.parent() {
                    curr = parent.to_path_buf();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_f3_atomic_metadata_replacement_safety() {
        let temp_dir =
            std::env::temp_dir().join(format!("snap_test_atomic_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let target_file = temp_dir.join("repository.json");
        fs::write(&target_file, b"initial content").unwrap();

        // 1. Successful atomic replace
        atomic_replace_file(&target_file, b"new content").unwrap();
        assert_eq!(fs::read(&target_file).unwrap(), b"new content");

        // 2. Failure simulation: destination in an invalid path
        let blocker_file = temp_dir.join("blocker_file");
        fs::write(&blocker_file, b"i am a file").unwrap();
        let invalid_dest = blocker_file.join("cannot_create_here.json");
        let res = atomic_replace_file(&invalid_dest, b"data");
        assert!(res.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_materialize_tree_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("snap_test_mat_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let mut tree1 = BTreeMap::new();
        tree1.insert("dir1/file1.txt".to_string(), b"hello\n".to_vec());
        tree1.insert("dir2/file2.txt".to_string(), b"world\n".to_vec());

        // Materialize tree1 from empty
        materialize_tree(&temp_dir, &BTreeMap::new(), &tree1).unwrap();
        assert_eq!(
            fs::read(temp_dir.join("dir1/file1.txt")).unwrap(),
            b"hello\n"
        );
        assert_eq!(
            fs::read(temp_dir.join("dir2/file2.txt")).unwrap(),
            b"world\n"
        );

        // Transition to tree2:
        // - dir1/file1.txt is removed (making dir1 empty)
        // - dir2 is replaced by regular file "dir2" (file blocking directory)
        let mut tree2 = BTreeMap::new();
        tree2.insert("dir2".to_string(), b"i am now a file\n".to_vec());

        materialize_tree(&temp_dir, &tree1, &tree2).unwrap();

        // dir1 and dir1/file1.txt must not exist
        assert!(!temp_dir.join("dir1").exists());
        assert!(!temp_dir.join("dir1/file1.txt").exists());

        // dir2 is now a regular file
        assert!(temp_dir.join("dir2").is_file());
        assert_eq!(
            fs::read(temp_dir.join("dir2")).unwrap(),
            b"i am now a file\n"
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
