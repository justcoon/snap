use std::path::Path;

use crate::cli::commands::common::find_repository_root;
use crate::cli::commands::CliError;
use crate::config::write_config;
use crate::core::version::ContributorId;

/// Execute `snap config [--global] contributor.id <id>`.
pub fn cmd_config(is_global: bool, key: &str, value: &str) -> Result<(), CliError> {
    if key != "contributor.id" {
        return Err(CliError::InvalidCommandOrArguments);
    }

    let contributor_id = ContributorId::parse(value)?;

    if is_global {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| CliError::Custom("HOME environment variable not set".to_string()))?;
        let global_config = Path::new(&home).join(".snapconfig.json");
        write_config(&global_config, &contributor_id)?;
    } else {
        let root = find_repository_root()?;
        let local_config = root.join(".snap").join("config.json");
        write_config(&local_config, &contributor_id)?;
    }

    Ok(())
}
