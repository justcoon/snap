use base64::prelude::*;
use std::collections::BTreeSet;

use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;
use crate::config::{resolve_contributor_id, ConfigError};
use crate::core::diff::{diff_tokens, is_text, tokenize_text};
use crate::core::patch::{Change, Patch, Repository, TextEditOp};
use crate::core::replay::{materialize_version, patch_result_version, FileTree};
use crate::core::validation::validate_repository;
use crate::core::version::{Version, MAX_REVISION};
use crate::fs::materializer::write_repository_atomic;
use crate::fs::scanner::{diff_working_tree, scan_working_tree};
use crate::presentation::{format_action_success, PresentationMode};

/// Execute `snap revert <version>`.
pub fn cmd_revert(version_str: &str, mode: PresentationMode) -> Result<(), CliError> {
    let target_version =
        Version::parse(version_str).map_err(|e| CliError::InvalidVersion(e.to_string()))?;

    let root = find_repository_root()?;
    let mut repo = load_repository(&root)?;

    if !crate::core::validation::is_version_known(&repo, &target_version) {
        return Err(CliError::UnknownVersion(version_str.to_string()));
    }

    let author = resolve_contributor_id(Some(&root))?.ok_or(ConfigError::MissingContributorId)?;

    // Working tree must be clean and free of unsupported entries
    let working_tree = scan_working_tree(&root)?;
    let (current_tree, _) = materialize_version(&repo.patches, &repo.frontier)?;

    let diff = diff_working_tree(&working_tree, current_tree.entries());
    if !diff.is_clean() {
        return Err(CliError::WorkingTreeIsDirty);
    }

    // Materialize target tree
    let (target_tree, _) = materialize_version(&repo.patches, &target_version)?;

    if current_tree.entries() == target_tree.entries() {
        return Err(CliError::TargetTreeAlreadyCurrent);
    }

    // Compute changes from current_tree to target_tree
    let changes = compute_revert_changes(&current_tree, &target_tree)?;

    let current_rev = repo.frontier.get(&author);
    let new_rev = current_rev + 1;
    if new_rev > MAX_REVISION {
        return Err(CliError::Custom(format!(
            "revision overflow beyond maximum safe integer ({MAX_REVISION})"
        )));
    }

    let message = format!("revert to {target_version}");
    let patch = Patch {
        author,
        revision: new_rev,
        base: repo.frontier.clone(),
        message,
        changes,
    };

    let result_version = patch_result_version(&patch);
    let new_frontier = repo.frontier.join(&result_version);

    repo.patches.push(patch);
    repo.patches
        .sort_by(|p1, p2| match p1.author.cmp(&p2.author) {
            std::cmp::Ordering::Equal => p1.revision.cmp(&p2.revision),
            ord => ord,
        });
    let new_repo = Repository::new(new_frontier.clone(), repo.patches);
    validate_repository(&new_repo)?;

    // Materialize target tree onto working tree
    crate::fs::materializer::materialize_tree(
        &root,
        current_tree.entries(),
        target_tree.entries(),
    )?;

    // Atomically write updated repository.json
    let snap_dir = crate::fs::snap_dir(&root);
    write_repository_atomic(&snap_dir, &new_repo)?;

    print!("{}", format_action_success("Reverted", &new_frontier, mode));
    Ok(())
}

/// Compute changes to transform `current_tree` into `target_tree` per SPEC §7.7.
pub fn compute_revert_changes(
    current_tree: &FileTree,
    target_tree: &FileTree,
) -> Result<Vec<Change>, CliError> {
    let mut changes: Vec<Change> = Vec::new();
    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    for p in current_tree.keys() {
        all_paths.insert(p.as_str());
    }
    for p in target_tree.keys() {
        all_paths.insert(p.as_str());
    }

    for path in all_paths {
        let old_content = current_tree.get(path);
        let new_content = target_tree.get(path);

        match (old_content, new_content) {
            (Some(_), None) => {
                changes.push(Change::Delete {
                    path: path.to_string(),
                });
            }
            (None, Some(new_bytes)) => {
                if is_text(new_bytes) {
                    let tokens = tokenize_text(new_bytes)?;
                    // SPEC §4.4: An empty script is valid only when creating an empty text file.
                    let edit = if tokens.is_empty() {
                        Vec::new()
                    } else {
                        vec![TextEditOp::Insert(tokens)]
                    };
                    changes.push(Change::Text {
                        path: path.to_string(),
                        edit,
                    });
                } else {
                    let content = BASE64_STANDARD.encode(new_bytes);
                    changes.push(Change::Put {
                        path: path.to_string(),
                        content,
                    });
                }
            }
            (Some(old_bytes), Some(new_bytes)) => {
                if old_bytes != new_bytes {
                    if is_text(old_bytes) && is_text(new_bytes) {
                        let old_tokens = tokenize_text(old_bytes)?;
                        let new_tokens = tokenize_text(new_bytes)?;
                        let edit = diff_tokens(&old_tokens, &new_tokens);
                        changes.push(Change::Text {
                            path: path.to_string(),
                            edit,
                        });
                    } else {
                        let content = BASE64_STANDARD.encode(new_bytes);
                        changes.push(Change::Put {
                            path: path.to_string(),
                            content,
                        });
                    }
                }
            }
            (None, None) => {}
        }
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_bug_006_revert_empty_text_file() {
        let current_tree = FileTree::new();
        let mut target_tree = FileTree::new();
        target_tree.insert("empty.txt".to_string(), Vec::new());

        let changes = compute_revert_changes(&current_tree, &target_tree).unwrap();
        assert_eq!(changes.len(), 1);
        match &changes[0] {
            Change::Text { path, edit } => {
                assert_eq!(path, "empty.txt");
                assert!(
                    edit.is_empty(),
                    "Expected empty edit script for creating empty text file, got: {:?}",
                    edit
                );
            }
            other => panic!("Expected Change::Text, got: {:?}", other),
        }
    }
}
