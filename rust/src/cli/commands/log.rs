use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;
use crate::core::replay::patch_result_version;
use crate::presentation::{format_log, LogRecord, PresentationMode};

/// Execute `snap log`.
pub fn cmd_log(mode: PresentationMode) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;

    let entries: Vec<LogRecord> = repo
        .patches
        .iter()
        .rev()
        .map(|patch| {
            let result_version = patch_result_version(patch);
            let escaped_message = patch
                .message
                .replace('\\', "\\\\")
                .replace('\t', "\\t")
                .replace('\n', "\\n");
            LogRecord {
                version: result_version,
                author: patch.author.to_string(),
                escaped_message,
            }
        })
        .collect();

    print!("{}", format_log(&entries, mode));
    Ok(())
}
