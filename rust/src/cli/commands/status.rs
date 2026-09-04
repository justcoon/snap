use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;
use crate::core::replay::materialize_version;
use crate::fs::scanner::{diff_working_tree, scan_working_tree};
use crate::presentation::{format_status, PresentationMode, StatusRow};

/// Execute `snap status`.
pub fn cmd_status(mode: PresentationMode) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;

    let (current_tree, _warnings) = materialize_version(&repo.patches, &repo.frontier)?;
    let working_tree = scan_working_tree(&root)?;
    let diff = diff_working_tree(&working_tree, current_tree.entries());

    let rows: Vec<StatusRow> = diff
        .changes
        .into_iter()
        .map(|c| StatusRow {
            path: c.path,
            status: c.status,
        })
        .collect();

    print!("{}", format_status(&repo.frontier, &rows, mode));
    Ok(())
}
