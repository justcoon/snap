use crate::cli::args::DiffTarget;
use crate::cli::commands::common::{
    check_dot_collisions, find_repository_root, load_remote_repository, load_repository,
};
use crate::cli::commands::CliError;
use crate::cli::diff_format::format_tree_diff;
use crate::core::replay::materialize_version;
use crate::core::version::Version;
use crate::fs::scanner::scan_working_tree;
use crate::presentation::{format_diff, PresentationMode};

/// Execute `snap diff`.
pub fn cmd_diff(target: DiffTarget, mode: PresentationMode) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let local_repo = load_repository(&root)?;

    match target {
        DiffTarget::WorkingTree => {
            let working_tree = scan_working_tree(&root)?;
            let (current_tree, _) = materialize_version(&local_repo.patches, &local_repo.frontier)?;
            let diff_output = format_tree_diff(current_tree.entries(), &working_tree)?;
            print!("{}", format_diff(&diff_output, mode));
            Ok(())
        }
        DiffTarget::Versions { old, new, repo } => {
            let old_version =
                Version::parse(&old).map_err(|e| CliError::InvalidVersion(e.to_string()))?;
            let new_version =
                Version::parse(&new).map_err(|e| CliError::InvalidVersion(e.to_string()))?;

            if !crate::core::validation::is_version_known(&local_repo, &old_version) {
                return Err(CliError::UnknownVersion(old));
            }

            let new_repo = if let Some(ref remote_src) = repo {
                let remote = load_remote_repository(remote_src)?;
                check_dot_collisions(&local_repo, &remote)?;
                remote
            } else {
                local_repo.clone()
            };

            if !crate::core::validation::is_version_known(&new_repo, &new_version) {
                return Err(CliError::UnknownVersion(new));
            }

            let (old_tree, _) = materialize_version(&local_repo.patches, &old_version)?;
            let (new_tree, _) = materialize_version(&new_repo.patches, &new_version)?;

            let diff_output = format_tree_diff(old_tree.entries(), new_tree.entries())?;
            print!("{}", format_diff(&diff_output, mode));
            Ok(())
        }
    }
}
