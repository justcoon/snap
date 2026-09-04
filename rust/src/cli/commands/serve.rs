use crate::cli::commands::common::{find_repository_root, load_repository};
use crate::cli::commands::CliError;
use crate::http::HttpServerConfig;

/// Execute `snap --serve [port]`.
pub fn cmd_serve(port: Option<u16>) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;
    let mut config = HttpServerConfig::default();
    if let Some(port) = port {
        config.port = port;
    }
    crate::http::serve_repository(&repo, config)
}
