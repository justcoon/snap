use base64::prelude::*;

use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;
use crate::config::{resolve_contributor_id, ConfigError};
use crate::core::diff::{diff_tokens, is_text, tokenize_text};
use crate::core::patch::{Change, Patch, Repository};
use crate::core::replay::{materialize_version, patch_result_version};
use crate::core::validation::validate_repository;
use crate::core::version::MAX_REVISION;
use crate::fs::materializer::write_repository_atomic;
use crate::fs::scanner::{diff_working_tree, scan_working_tree, FileStatus};
use crate::presentation::{format_action_success, PresentationMode};

/// Maximum commit message length in UTF-8 bytes enforced by `snap commit` (§4.2).
pub const MAX_COMMIT_MESSAGE_BYTES: usize = 4096;

/// Execute `snap commit <message>`.
pub fn cmd_commit(message: String, mode: PresentationMode) -> Result<(), CliError> {
    if message.is_empty()
        || message.len() > MAX_COMMIT_MESSAGE_BYTES
        || message
            .chars()
            .any(|c| c.is_ascii_control() && c != '\t' && c != '\n')
    {
        return Err(CliError::InvalidCommitMessage);
    }

    let root = find_repository_root()?;

    // Resolve contributor identity
    let author = resolve_contributor_id(Some(&root))?.ok_or(ConfigError::MissingContributorId)?;

    let repo = load_repository(&root)?;

    let (current_tree, _warnings) = materialize_version(&repo.patches, &repo.frontier)?;
    let working_tree = scan_working_tree(&root)?;
    let diff = diff_working_tree(&working_tree, current_tree.entries());

    if diff.is_clean() {
        return Err(CliError::WorkingTreeIsClean);
    }

    // Build changes
    let mut changes = Vec::new();
    for change in diff.changes {
        match change.status {
            FileStatus::Deleted => {
                changes.push(Change::Delete { path: change.path });
            }
            FileStatus::Added | FileStatus::Modified => {
                let new_bytes = working_tree
                    .get(&change.path)
                    .map(|v| v.as_slice())
                    .unwrap_or_default();
                let old_bytes = current_tree.get(&change.path);

                let old_is_text = old_bytes.map(is_text).unwrap_or(true);
                let new_is_text = is_text(new_bytes);

                if new_is_text && old_is_text {
                    let old_tokens = if let Some(bytes) = old_bytes {
                        tokenize_text(bytes)?
                    } else {
                        Vec::new()
                    };
                    let new_tokens = tokenize_text(new_bytes)?;
                    let edit = diff_tokens(&old_tokens, &new_tokens);
                    changes.push(Change::Text {
                        path: change.path,
                        edit,
                    });
                } else {
                    let content = BASE64_STANDARD.encode(new_bytes);
                    changes.push(Change::Put {
                        path: change.path,
                        content,
                    });
                }
            }
        }
    }

    // Determine new contributor revision
    let current_rev = repo.frontier.get(&author);
    let new_rev = current_rev + 1;
    if new_rev > MAX_REVISION {
        return Err(CliError::Custom(format!(
            "revision overflow beyond maximum safe integer ({MAX_REVISION})"
        )));
    }

    let patch = Patch {
        author,
        revision: new_rev,
        base: repo.frontier.clone(),
        message,
        changes,
    };

    let result_version = patch_result_version(&patch);
    let new_frontier = repo.frontier.join(&result_version);

    let mut new_patches = repo.patches;
    new_patches.push(patch);
    new_patches.sort_by(|p1, p2| match p1.author.cmp(&p2.author) {
        std::cmp::Ordering::Equal => p1.revision.cmp(&p2.revision),
        ord => ord,
    });

    let new_repo = Repository::new(new_frontier.clone(), new_patches);
    validate_repository(&new_repo)?;

    let snap_dir = crate::fs::snap_dir(&root);
    write_repository_atomic(&snap_dir, &new_repo)?;

    print!(
        "{}",
        format_action_success("Committed", &new_frontier, mode)
    );
    Ok(())
}
