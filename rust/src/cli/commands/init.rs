use crate::cli::commands::CliError;
use crate::core::patch::Repository;
use crate::core::version::Version;
use crate::fs::materializer::write_repository_atomic;
use crate::fs::{repo_file_path, snap_dir};
use crate::presentation::{format_action_success, PresentationMode};
use std::fs;

/// Execute `snap init [path]`.
pub fn cmd_init(target_path: Option<String>, mode: PresentationMode) -> Result<(), CliError> {
    let cwd = std::env::current_dir()?;
    let target_dir = if let Some(ref p) = target_path {
        cwd.join(p)
    } else {
        cwd
    };

    // Check if target directory already has a .snap directory
    if snap_dir(&target_dir).exists() {
        return Err(CliError::RepositoryAlreadyExists);
    }

    // Check if target directory is inside an existing repository
    let mut check_dir = if target_dir.exists() {
        target_dir.clone()
    } else if let Some(parent) = target_dir.parent() {
        parent.to_path_buf()
    } else {
        target_dir.clone()
    };

    loop {
        if repo_file_path(&check_dir).is_file() {
            return Err(CliError::CannotInitializeInsideRepository);
        }
        if let Some(parent) = check_dir.parent() {
            check_dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Create target and .snap directory
    fs::create_dir_all(&target_dir)?;
    let snap_dir = snap_dir(&target_dir);
    fs::create_dir_all(&snap_dir)?;

    // Create empty repository
    let empty_repo = Repository::new(Version::empty(), Vec::new());
    write_repository_atomic(&snap_dir, &empty_repo)?;

    print!(
        "{}",
        format_action_success("Initialized repository", &Version::empty(), mode)
    );
    Ok(())
}
