pub mod materializer;
pub mod paths;
pub mod scanner;

pub use materializer::{
    atomic_replace_file, materialize_tree, write_repository_atomic, MaterializeError,
};
pub use paths::{check_prefix_free, validate_tracked_path, PathError, PrefixFreeError};
pub use scanner::{
    diff_working_tree, scan_working_tree, FileStatus, ScanError, WorkingChange, WorkingTreeDiff,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scenario_f1_symlink_and_special_file_rejection() {
        let temp_dir = std::env::temp_dir().join(format!("snap_test_f1_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. Regular file
        let regular_file = temp_dir.join("a.txt");
        fs::write(&regular_file, b"content").unwrap();

        // 2. Symlink
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let symlink_path = temp_dir.join("link");
            symlink(&regular_file, &symlink_path).unwrap();

            let res = scan_working_tree(&temp_dir);
            match res {
                Err(ScanError::UnsupportedEntry(path)) => {
                    assert_eq!(path, "link");
                }
                other => panic!("expected UnsupportedEntry('link'), got {other:?}"),
            }

            fs::remove_file(&symlink_path).unwrap();
        }

        // 3. Scan without symlink succeeds
        let tree = scan_working_tree(&temp_dir).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.get("a.txt").unwrap(), b"content");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
