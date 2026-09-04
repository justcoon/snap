use std::fmt;

use crate::config::ConfigError;
use crate::core::diff::DiffError;
use crate::core::patch::RepositoryError;
use crate::core::replay::ReplayError;
use crate::core::validation::ValidationError;
use crate::core::version::ContributorIdError;
use crate::fs::materializer::MaterializeError;
use crate::fs::scanner::ScanError;

pub mod commit;
pub mod common;
pub mod config;
pub mod diff;
pub mod help;
pub mod init;
pub mod log;
pub mod merge;
pub mod revert;
pub mod serve;
pub mod status;
pub mod version;

pub use commit::{cmd_commit, MAX_COMMIT_MESSAGE_BYTES};
pub use common::{
    check_dot_collisions, find_repository_root, load_remote_repository, load_repository,
};
pub use config::cmd_config;
pub use diff::cmd_diff;
pub use help::cmd_help;
pub use init::cmd_init;
pub use log::cmd_log;
pub use merge::cmd_merge;
pub use revert::cmd_revert;
pub use serve::cmd_serve;
pub use status::cmd_status;
pub use version::cmd_version;

/// CLI operational or domain errors.
#[derive(Debug)]
pub enum CliError {
    InvalidCommandOrArguments,
    DiffUsage,
    NotASnapRepository,
    RepositoryAlreadyExists,
    CannotInitializeInsideRepository,
    WorkingTreeIsClean,
    WorkingTreeIsDirty,
    TargetTreeAlreadyCurrent,
    UnknownVersion(String),
    InvalidVersion(String),
    PatchCollision { author: String, revision: u64 },
    InvalidCommitMessage,
    InvalidPort(String),
    Config(ConfigError),
    Scan(ScanError),
    Validation(ValidationError),
    Repository(RepositoryError),
    Replay(ReplayError),
    Materialize(MaterializeError),
    Diff(DiffError),
    Io(std::io::Error),
    Custom(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::InvalidCommandOrArguments => write!(f, "invalid command or arguments"),
            CliError::DiffUsage => {
                write!(f, "usage: snap diff [<old> <new> [--repo <repository>]]")
            }
            CliError::NotASnapRepository => write!(f, "not a Snap repository"),
            CliError::RepositoryAlreadyExists => write!(f, "repository already exists"),
            CliError::CannotInitializeInsideRepository => {
                write!(f, "cannot initialize inside repository")
            }
            CliError::WorkingTreeIsClean => write!(f, "working tree is clean"),
            CliError::WorkingTreeIsDirty => write!(f, "working tree is dirty"),
            CliError::TargetTreeAlreadyCurrent => write!(f, "target tree is already current"),
            CliError::UnknownVersion(v) => write!(f, "unknown version: {v}"),
            CliError::InvalidVersion(v) => write!(f, "invalid version: {v}"),
            CliError::PatchCollision { author, revision } => {
                write!(f, "patch collision: {author} revision {revision}")
            }
            CliError::InvalidCommitMessage => write!(f, "invalid commit message"),
            CliError::InvalidPort(p) => write!(f, "invalid port: {p}"),
            CliError::Config(e) => write!(f, "{e}"),
            CliError::Scan(e) => write!(f, "{e}"),
            CliError::Validation(e) => write!(f, "{e}"),
            CliError::Repository(e) => write!(f, "{e}"),
            CliError::Replay(e) => write!(f, "{e}"),
            CliError::Materialize(e) => write!(f, "{e}"),
            CliError::Diff(e) => write!(f, "{e}"),
            CliError::Io(e) => write!(f, "{e}"),
            CliError::Custom(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ConfigError> for CliError {
    fn from(e: ConfigError) -> Self {
        CliError::Config(e)
    }
}

impl From<ScanError> for CliError {
    fn from(e: ScanError) -> Self {
        CliError::Scan(e)
    }
}

impl From<ValidationError> for CliError {
    fn from(e: ValidationError) -> Self {
        CliError::Validation(e)
    }
}

impl From<RepositoryError> for CliError {
    fn from(e: RepositoryError) -> Self {
        CliError::Repository(e)
    }
}

impl From<ContributorIdError> for CliError {
    fn from(e: ContributorIdError) -> Self {
        CliError::Custom(format!("invalid contributor id: {e}"))
    }
}

impl From<ReplayError> for CliError {
    fn from(e: ReplayError) -> Self {
        CliError::Replay(e)
    }
}

impl From<MaterializeError> for CliError {
    fn from(e: MaterializeError) -> Self {
        CliError::Materialize(e)
    }
}

impl From<DiffError> for CliError {
    fn from(e: DiffError) -> Self {
        CliError::Diff(e)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Io(e)
    }
}
