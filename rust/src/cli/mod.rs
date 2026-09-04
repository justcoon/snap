pub mod args;
pub mod commands;
pub mod diff_format;

pub use args::{parse_args, Command, DiffTarget, ParseError};
pub use commands::{
    cmd_commit, cmd_config, cmd_diff, cmd_init, cmd_log, cmd_merge, cmd_revert, cmd_status,
    find_repository_root, CliError,
};

/// Dispatch parsed CLI arguments to command handlers.
pub fn dispatch(args: &[String]) -> Result<(), CliError> {
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
        Command::Version => {
            println!("snap 0.1.0");
            Ok(())
        }
        Command::Init { path } => cmd_init(path),
        Command::Config {
            is_global,
            key,
            value,
        } => cmd_config(is_global, &key, &value),
        Command::Status => cmd_status(),
        Command::Log => cmd_log(),
        Command::Commit { message } => cmd_commit(message),
        Command::Diff(target) => cmd_diff(target),
        Command::Revert { version } => cmd_revert(&version),
        Command::Merge { repo } => cmd_merge(&repo),
        Command::Serve(_) => Err(CliError::Custom("serve not yet implemented".to_string())),
    }
}
