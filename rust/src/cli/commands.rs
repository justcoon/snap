use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use base64::prelude::*;

use crate::config::{resolve_contributor_id, write_config, ConfigError};
use crate::core::diff::{diff_tokens, is_text, tokenize_text, DiffError};
use crate::core::patch::{Change, Patch, Repository};
use crate::core::replay::{materialize_version, patch_result_version, ReplayError};
use crate::core::validation::{validate_repository, ValidationError};
use crate::core::version::{ContributorId, Version, MAX_REVISION};
use crate::fs::materializer::{write_repository_atomic, MaterializeError};
use crate::fs::scanner::{diff_working_tree, scan_working_tree, FileStatus, ScanError};

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
    InvalidCommitMessage,
    InvalidPort(String),
    Config(ConfigError),
    Scan(ScanError),
    Validation(ValidationError),
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
            CliError::DiffUsage => write!(f, "usage: snap diff [--repo <repo>] [<old> <new>]"),
            CliError::NotASnapRepository => write!(f, "not a Snap repository"),
            CliError::RepositoryAlreadyExists => write!(f, "repository already exists"),
            CliError::CannotInitializeInsideRepository => {
                write!(f, "cannot initialize inside repository")
            }
            CliError::WorkingTreeIsClean => write!(f, "working tree is clean"),
            CliError::WorkingTreeIsDirty => write!(f, "working tree is dirty"),
            CliError::InvalidCommitMessage => write!(f, "invalid commit message"),
            CliError::InvalidPort(p) => write!(f, "invalid port: {p}"),
            CliError::Config(e) => write!(f, "{e}"),
            CliError::Scan(e) => write!(f, "{e}"),
            CliError::Validation(e) => write!(f, "{e}"),
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

/// Locate the nearest repository root directory containing `.snap/repository.json`.
pub fn find_repository_root() -> Result<PathBuf, CliError> {
    let mut curr = std::env::current_dir()?;
    loop {
        if curr.join(".snap").join("repository.json").is_file() {
            return Ok(curr);
        }
        if let Some(parent) = curr.parent() {
            curr = parent.to_path_buf();
        } else {
            break;
        }
    }
    Err(CliError::NotASnapRepository)
}

/// Load and strictly validate the repository at `repo_root`.
pub fn load_repository(repo_root: &Path) -> Result<Repository, CliError> {
    let repo_file = repo_root.join(".snap").join("repository.json");
    let content = fs::read_to_string(&repo_file)?;
    let repo: Repository = serde_json::from_str(&content)
        .map_err(|e| CliError::Custom(format!("invalid repository.json: {e}")))?;
    validate_repository(&repo)?;
    Ok(repo)
}

/// Execute `snap init [path]`.
pub fn cmd_init(target_path: Option<String>) -> Result<(), CliError> {
    let cwd = std::env::current_dir()?;
    let target_dir = if let Some(ref p) = target_path {
        cwd.join(p)
    } else {
        cwd
    };

    // Check if target directory already has a .snap directory
    if target_dir.join(".snap").exists() {
        return Err(CliError::RepositoryAlreadyExists);
    }

    // Check if target directory is inside an existing repository
    let mut check_dir = if target_dir.exists() {
        target_dir.clone()
    } else if let Some(parent) = target_dir.parent() {
        parent.to_path_buf()
    } else {
        target_dir.clone()
    };

    loop {
        if check_dir.join(".snap").join("repository.json").is_file() {
            return Err(CliError::CannotInitializeInsideRepository);
        }
        if let Some(parent) = check_dir.parent() {
            check_dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Create target and .snap directory
    fs::create_dir_all(&target_dir)?;
    let snap_dir = target_dir.join(".snap");
    fs::create_dir_all(&snap_dir)?;

    // Create empty repository
    let empty_repo = Repository::new(Version::empty(), Vec::new());
    write_repository_atomic(&snap_dir, &empty_repo)?;

    println!("()");
    Ok(())
}

/// Execute `snap config [--global] contributor.id <id>`.
pub fn cmd_config(is_global: bool, key: &str, value: &str) -> Result<(), CliError> {
    if key != "contributor.id" {
        return Err(CliError::InvalidCommandOrArguments);
    }

    let contributor_id = ContributorId::parse(value)
        .map_err(|_| ConfigError::InvalidContributorId(value.to_string()))?;

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

/// Execute `snap status`.
pub fn cmd_status() -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;

    let (current_tree, _warnings) = materialize_version(&repo.patches, &repo.frontier)?;
    let working_tree = scan_working_tree(&root)?;
    let diff = diff_working_tree(&working_tree, current_tree.entries());

    println!("version {}", repo.frontier);
    for change in diff.changes {
        println!("{} {}", change.status.symbol(), change.path);
    }

    Ok(())
}

/// Execute `snap log`.
pub fn cmd_log() -> Result<(), CliError> {
    let root = find_repository_root()?;
    let repo = load_repository(&root)?;

    for patch in repo.patches.iter().rev() {
        let result_version = patch_result_version(patch);
        let escaped_message = patch
            .message
            .replace('\\', "\\\\")
            .replace('\t', "\\t")
            .replace('\n', "\\n");
        println!("{result_version}\t{}\t{escaped_message}", patch.author);
    }

    Ok(())
}

/// Execute `snap commit <message>`.
pub fn cmd_commit(message: String) -> Result<(), CliError> {
    if message.is_empty() || message.len() > 4096 {
        return Err(CliError::InvalidCommitMessage);
    }

    let root = find_repository_root()?;

    // Resolve contributor identity
    let author = resolve_contributor_id(Some(&root))?.ok_or(ConfigError::MissingContributorId)?;

    let repo = load_repository(&root)?;

    let (current_tree, _warnings) = materialize_version(&repo.patches, &repo.frontier)?;
    let working_tree = scan_working_tree(&root)?;
    let diff = diff_working_tree(&working_tree, current_tree.entries());

    if diff.is_clean() {
        return Err(CliError::WorkingTreeIsClean);
    }

    // Build changes
    let mut changes = Vec::new();
    for change in diff.changes {
        match change.status {
            FileStatus::Deleted => {
                changes.push(Change::Delete { path: change.path });
            }
            FileStatus::Added | FileStatus::Modified => {
                let new_bytes = working_tree
                    .get(&change.path)
                    .map(|v| v.as_slice())
                    .unwrap_or_default();
                let old_bytes = current_tree.get(&change.path);

                let old_is_text = old_bytes.map(is_text).unwrap_or(true);
                let new_is_text = is_text(new_bytes);

                if new_is_text && old_is_text {
                    let old_tokens = if let Some(bytes) = old_bytes {
                        tokenize_text(bytes)?
                    } else {
                        Vec::new()
                    };
                    let new_tokens = tokenize_text(new_bytes)?;
                    let edit = diff_tokens(&old_tokens, &new_tokens);
                    changes.push(Change::Text {
                        path: change.path,
                        edit,
                    });
                } else {
                    let content = BASE64_STANDARD.encode(new_bytes);
                    changes.push(Change::Put {
                        path: change.path,
                        content,
                    });
                }
            }
        }
    }

    // Determine new contributor revision
    let current_rev = repo.frontier.get(&author);
    let new_rev = current_rev + 1;
    if new_rev > MAX_REVISION {
        return Err(CliError::Custom(format!(
            "revision overflow beyond maximum safe integer ({MAX_REVISION})"
        )));
    }

    let patch = Patch {
        author,
        revision: new_rev,
        base: repo.frontier.clone(),
        message,
        changes,
    };

    let result_version = patch_result_version(&patch);
    let new_frontier = repo.frontier.join(&result_version);

    let mut new_patches = repo.patches;
    new_patches.push(patch);

    let new_repo = Repository::new(new_frontier.clone(), new_patches);
    validate_repository(&new_repo)?;

    let snap_dir = root.join(".snap");
    write_repository_atomic(&snap_dir, &new_repo)?;

    println!("{new_frontier}");
    Ok(())
}

/// Check if a target version is known in the repository.
pub fn is_version_known(repo: &Repository, target: &Version) -> bool {
    if target.is_empty() {
        return true;
    }
    if &repo.frontier == target {
        return true;
    }
    for patch in &repo.patches {
        let res_v = patch_result_version(patch);
        if &res_v == target || &patch.base == target {
            return true;
        }
    }
    false
}

/// Execute `snap revert <version>`.
pub fn cmd_revert(version_str: &str) -> Result<(), CliError> {
    let target_version = Version::parse(version_str)
        .map_err(|_| CliError::Custom(format!("invalid version: {version_str}")))?;

    let root = find_repository_root()?;
    let repo = load_repository(&root)?;

    if !is_version_known(&repo, &target_version) {
        return Err(CliError::Custom(format!("unknown version: {version_str}")));
    }

    Err(CliError::Custom("revert not yet implemented".to_string()))
}
