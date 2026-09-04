use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;

/// Execute `snap --serve [port]`.
pub fn cmd_serve(port: Option<u16>) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;
    crate::http::serve_repository(&repo, port)
}
