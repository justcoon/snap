pub mod args;
pub mod commands;
pub mod diff_format;

pub use args::{parse_args, Command, DiffTarget, ParseError};
pub use commands::{
    cmd_commit, cmd_config, cmd_diff, cmd_init, cmd_log, cmd_merge, cmd_revert, cmd_serve,
    cmd_status, cmd_version, find_repository_root, CliError,
};

/// Dispatch parsed CLI arguments to command handlers.
pub fn dispatch(args: &[String], modes: crate::presentation::StreamModes) -> Result<(), CliError> {
    if args.is_empty() {
        return Err(CliError::InvalidCommandOrArguments);
    }

    // args[0] is the program binary name; args[1..] are the command line tokens
    let cli_tokens = &args[1..];
    let command = parse_args(cli_tokens).map_err(|e| match e {
        ParseError::InvalidCommandOrArguments => CliError::InvalidCommandOrArguments,
        ParseError::DiffUsage => CliError::DiffUsage,
        ParseError::InvalidPort(p) => CliError::InvalidPort(p),
    })?;

    match command {
        Command::Version => cmd_version(modes.stdout),
        Command::Init { path } => cmd_init(path, modes.stdout),
        Command::Config {
            is_global,
            key,
            value,
        } => cmd_config(is_global, &key, &value),
        Command::Status => cmd_status(modes.stdout),
        Command::Log => cmd_log(modes.stdout),
        Command::Commit { message } => cmd_commit(message, modes.stdout),
        Command::Diff(target) => cmd_diff(target, modes.stdout),
        Command::Revert { version } => cmd_revert(&version, modes.stdout),
        Command::Merge { repo } => cmd_merge(&repo, modes),
        Command::Serve { port } => cmd_serve(port),
    }
}
