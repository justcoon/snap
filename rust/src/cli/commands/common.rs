use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::commands::CliError;
use crate::core::patch::Repository;
use crate::core::validation::validate_repository;
use crate::fs::{repo_file_path, REPOSITORY_FILE};

/// Locate the nearest repository root directory containing `.snap/repository.json`.
pub fn find_repository_root() -> Result<PathBuf, CliError> {
    let mut curr = std::env::current_dir()?;
    loop {
        if repo_file_path(&curr).is_file() {
            return Ok(curr);
        }
        if let Some(parent) = curr.parent() {
            curr = parent.to_path_buf();
        } else {
            break;
        }
    }
    Err(CliError::NotASnapRepository)
}

/// Load and strictly validate the repository at `repo_root`.
pub fn load_repository(repo_root: &Path) -> Result<Repository, CliError> {
    let repo_file = repo_file_path(repo_root);
    let bytes = fs::read(&repo_file)?;
    let repo = Repository::from_json_slice(&bytes)?;
    validate_repository(&repo)?;
    Ok(repo)
}

/// Load and strictly validate a remote repository from a path, file, or HTTP URL.
pub fn load_remote_repository(source: &str) -> Result<Repository, CliError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return crate::http::fetch_repository(source);
    }

    let path = Path::new(source);
    let target_file = if repo_file_path(path).is_file() {
        repo_file_path(path)
    } else if path.join(REPOSITORY_FILE).is_file() {
        path.join(REPOSITORY_FILE)
    } else if path.is_file() {
        path.to_path_buf()
    } else {
        return Err(CliError::Custom(format!(
            "cannot read repository from '{source}'"
        )));
    };

    let bytes = fs::read(&target_file)?;
    let repo = Repository::from_json_slice(&bytes)?;
    validate_repository(&repo)?;
    Ok(repo)
}

/// Compare common dots across two repositories and fail if payloads differ (§3.5, §7.6).
pub fn check_dot_collisions(local: &Repository, remote: &Repository) -> Result<(), CliError> {
    let mut local_map = std::collections::BTreeMap::new();
    for p in &local.patches {
        local_map.insert((&p.author, p.revision), p);
    }
    for p in &remote.patches {
        if let Some(local_p) = local_map.get(&(&p.author, p.revision)) {
            if *local_p != p {
                return Err(CliError::PatchCollision {
                    author: p.author.to_string(),
                    revision: p.revision,
                });
            }
        }
    }
    Ok(())
}
