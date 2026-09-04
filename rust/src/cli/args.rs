/// Target specification for the `diff` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffTarget {
    WorkingTree,
    Versions {
        old: String,
        new: String,
        repo: Option<String>,
    },
}

/// Strongly typed command parsed from CLI arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Version,
    Init {
        path: Option<String>,
    },
    Config {
        is_global: bool,
        key: String,
        value: String,
    },
    Status,
    Log,
    Commit {
        message: String,
    },
    Diff(DiffTarget),
    Revert {
        version: String,
    },
    Merge {
        repo: String,
    },
    Serve {
        port: Option<u16>,
    },
}

/// Errors occurring during CLI argument parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidCommandOrArguments,
    DiffUsage,
    InvalidPort(String),
}

/// Parse command-line arguments (excluding argv[0] program name).
pub fn parse_args(args: &[String]) -> Result<Command, ParseError> {
    if args.is_empty() {
        return Err(ParseError::InvalidCommandOrArguments);
    }

    match args[0].as_str() {
        "--version" => {
            if args.len() == 1 {
                Ok(Command::Version)
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "init" => match args.len() {
            1 => Ok(Command::Init { path: None }),
            2 => {
                let target = &args[1];
                if target.starts_with('-') {
                    Err(ParseError::InvalidCommandOrArguments)
                } else {
                    Ok(Command::Init {
                        path: Some(target.clone()),
                    })
                }
            }
            _ => Err(ParseError::InvalidCommandOrArguments),
        },
        "config" => {
            if args.len() == 4 && args[1] == "--global" {
                if args[2] != "contributor.id" {
                    return Err(ParseError::InvalidCommandOrArguments);
                }
                Ok(Command::Config {
                    is_global: true,
                    key: args[2].clone(),
                    value: args[3].clone(),
                })
            } else if args.len() == 3 && !args[1].starts_with('-') {
                if args[1] != "contributor.id" {
                    return Err(ParseError::InvalidCommandOrArguments);
                }
                Ok(Command::Config {
                    is_global: false,
                    key: args[1].clone(),
                    value: args[2].clone(),
                })
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "status" => {
            if args.len() == 1 {
                Ok(Command::Status)
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "log" => {
            if args.len() == 1 {
                Ok(Command::Log)
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "commit" => {
            if args.len() == 2 {
                Ok(Command::Commit {
                    message: args[1].clone(),
                })
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "diff" => {
            let rest = &args[1..];
            if rest.is_empty() {
                Ok(Command::Diff(DiffTarget::WorkingTree))
            } else if rest.len() == 2 && !rest[0].starts_with('-') && !rest[1].starts_with('-') {
                Ok(Command::Diff(DiffTarget::Versions {
                    old: rest[0].clone(),
                    new: rest[1].clone(),
                    repo: None,
                }))
            } else if rest.len() == 4
                && !rest[0].starts_with('-')
                && !rest[1].starts_with('-')
                && rest[2] == "--repo"
                && !rest[3].starts_with('-')
            {
                Ok(Command::Diff(DiffTarget::Versions {
                    old: rest[0].clone(),
                    new: rest[1].clone(),
                    repo: Some(rest[3].clone()),
                }))
            } else {
                Err(ParseError::DiffUsage)
            }
        }
        "revert" => {
            if args.len() == 2 && !args[1].starts_with('-') {
                Ok(Command::Revert {
                    version: args[1].clone(),
                })
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "merge" => {
            if args.len() == 2 && !args[1].starts_with('-') {
                Ok(Command::Merge {
                    repo: args[1].clone(),
                })
            } else {
                Err(ParseError::InvalidCommandOrArguments)
            }
        }
        "--serve" => match args.len() {
            1 => Ok(Command::Serve { port: None }),
            2 => {
                let port_str = &args[1];
                if port_str.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(port) = port_str.parse::<u32>() {
                        if port <= 65535 {
                            Ok(Command::Serve {
                                port: Some(port as u16),
                            })
                        } else {
                            Err(ParseError::InvalidPort(port_str.clone()))
                        }
                    } else {
                        Err(ParseError::InvalidPort(port_str.clone()))
                    }
                } else {
                    Err(ParseError::InvalidPort(port_str.clone()))
                }
            }
            _ => Err(ParseError::InvalidCommandOrArguments),
        },
        _ => Err(ParseError::InvalidCommandOrArguments),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_args(&to_args(&["--version"])), Ok(Command::Version));
        assert_eq!(
            parse_args(&to_args(&["--version", "extra"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
    }

    #[test]
    fn test_parse_init() {
        assert_eq!(
            parse_args(&to_args(&["init"])),
            Ok(Command::Init { path: None })
        );
        assert_eq!(
            parse_args(&to_args(&["init", "my_repo"])),
            Ok(Command::Init {
                path: Some("my_repo".to_string())
            })
        );
        assert_eq!(
            parse_args(&to_args(&["init", "a", "b"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
        assert_eq!(
            parse_args(&to_args(&["init", "--flag"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
    }

    #[test]
    fn test_parse_config() {
        assert_eq!(
            parse_args(&to_args(&["config", "contributor.id", "a@x"])),
            Ok(Command::Config {
                is_global: false,
                key: "contributor.id".to_string(),
                value: "a@x".to_string(),
            })
        );
        assert_eq!(
            parse_args(&to_args(&["config", "--global", "contributor.id", "a@x"])),
            Ok(Command::Config {
                is_global: true,
                key: "contributor.id".to_string(),
                value: "a@x".to_string(),
            })
        );
        assert_eq!(
            parse_args(&to_args(&["config", "contributor.id", "a@x", "--global"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
        assert_eq!(
            parse_args(&to_args(&[
                "config",
                "--global",
                "--global",
                "contributor.id",
                "a@x"
            ])),
            Err(ParseError::InvalidCommandOrArguments)
        );
    }

    #[test]
    fn test_parse_commit() {
        assert_eq!(
            parse_args(&to_args(&["commit", "initial"])),
            Ok(Command::Commit {
                message: "initial".to_string()
            })
        );
        assert_eq!(
            parse_args(&to_args(&["commit"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
        assert_eq!(
            parse_args(&to_args(&["commit", "a", "b"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
    }

    #[test]
    fn test_parse_serve() {
        assert_eq!(
            parse_args(&to_args(&["--serve"])),
            Ok(Command::Serve { port: None })
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "0"])),
            Ok(Command::Serve { port: Some(0) })
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "8765"])),
            Ok(Command::Serve { port: Some(8765) })
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "65535"])),
            Ok(Command::Serve { port: Some(65535) })
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "65536"])),
            Err(ParseError::InvalidPort("65536".to_string()))
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "abc"])),
            Err(ParseError::InvalidPort("abc".to_string()))
        );
        assert_eq!(
            parse_args(&to_args(&["--serve", "8765", "extra"])),
            Err(ParseError::InvalidCommandOrArguments)
        );
    }
}
