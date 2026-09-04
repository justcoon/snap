use crate::cli::commands::CliError;
use crate::presentation::{format_version, PresentationMode};

/// Execute `snap --version`.
pub fn cmd_version(mode: PresentationMode) -> Result<(), CliError> {
    print!("{}", format_version(env!("CARGO_PKG_VERSION"), mode));
    Ok(())
}
