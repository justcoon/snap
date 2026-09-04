use crate::cli::commands::CliError;
use crate::presentation::PresentationMode;

/// Execute `snap help`.
pub fn cmd_help(mode: PresentationMode) -> Result<(), CliError> {
    let help_text = format_help(mode);
    print!("{}", help_text);
    Ok(())
}

fn format_help(mode: PresentationMode) -> String {
    match mode {
        PresentationMode::Plain => format_plain_help(),
        PresentationMode::Terminal => format_terminal_help(),
    }
}

fn format_plain_help() -> String {
    format!(
        r#"Snap - A small local version control system

Usage:
  snap <command> [arguments]

Commands:
  init [path]              Initialize a new repository
  config [--global] contributor.id <id>
                           Configure contributor ID
  status                   Show working tree status
  log                      Show patch history
  commit <message>         Record changes to the repository
  diff [<old> <new> [--repo <repository>]]
                           Compare versions or working tree
  revert <version>         Revert to a previous version
  merge <repository>       Merge another repository
  --serve [port]           Start HTTP server
  --version                Show version information
  help                     Show this help message
"#
    )
}

fn format_terminal_help() -> String {
    format!(
        r#"{bold}Snap{reset} - A small local version control system

{bold}Usage:{reset}
  snap {cyan}<command>{reset} [arguments]

{bold}Commands:{reset}
  {cyan}init{reset} [path]              Initialize a new repository
  {cyan}config{reset} [--global] contributor.id <id>
                           Configure contributor ID
  {cyan}status{reset}                   Show working tree status
  {cyan}log{reset}                      Show patch history
  {cyan}commit{reset} {cyan}<message>{reset}         Record changes to the repository
  {cyan}diff{reset} [{cyan}<old>{reset} {cyan}<new>{reset} [--repo {cyan}<repository>{reset}]
                           Compare versions or working tree
  {cyan}revert{reset} {cyan}<version>{reset}         Revert to a previous version
  {cyan}merge{reset} {cyan}<repository>{reset}       Merge another repository
  {cyan}--serve{reset} [port]           Start HTTP server
  {cyan}--version{reset}                Show version information
  {cyan}help{reset}                     Show this help message
"#,
        bold = "\x1b[1m",
        reset = "\x1b[0m",
        cyan = "\x1b[36m"
    )
}
