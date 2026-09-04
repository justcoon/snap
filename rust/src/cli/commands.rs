use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::args::DiffTarget;
use crate::cli::diff_format::format_tree_diff;
use crate::config::{resolve_contributor_id, write_config, ConfigError};
use crate::core::diff::{diff_tokens, is_text, tokenize_text, DiffError};
use crate::core::patch::{Change, Patch, Repository, RepositoryError, TextEditOp};
use crate::core::replay::{materialize_version, patch_result_version, ReplayError};
use crate::core::validation::{validate_repository, ValidationError};
use crate::core::version::{ContributorId, ContributorIdError, Version, MAX_REVISION};
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
    let bytes = fs::read(&repo_file)?;
    let repo = Repository::from_json_slice(&bytes)?;
    validate_repository(&repo)?;
    Ok(repo)
}

/// Load and strictly validate a remote repository from a path or file.
pub fn load_remote_repository(source: &str) -> Result<Repository, CliError> {
    let path = Path::new(source);
    let target_file = if path.join(".snap").join("repository.json").is_file() {
        path.join(".snap").join("repository.json")
    } else if path.join("repository.json").is_file() {
        path.join("repository.json")
    } else if path.is_file() {
        path.to_path_buf()
    } else {
        return Err(CliError::Custom(format!(
            "cannot read repository from '{source}'"
        )));
    };

    let bytes = fs::read(&target_file)?;
    let repo = Repository::from_json_slice(&bytes)?;
    validate_repository(&repo)?;
    Ok(repo)
}

/// Compare common dots across two repositories and fail if payloads differ (§3.5, §7.6).
pub fn check_dot_collisions(local: &Repository, remote: &Repository) -> Result<(), CliError> {
    let mut local_map = std::collections::BTreeMap::new();
    for p in &local.patches {
        local_map.insert((&p.author, p.revision), p);
    }
    for p in &remote.patches {
        if let Some(local_p) = local_map.get(&(&p.author, p.revision)) {
            if *local_p != p {
                return Err(CliError::PatchCollision {
                    author: p.author.to_string(),
                    revision: p.revision,
                });
            }
        }
    }
    Ok(())
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
    new_patches.sort_by(|p1, p2| match p1.author.cmp(&p2.author) {
        std::cmp::Ordering::Equal => p1.revision.cmp(&p2.revision),
        ord => ord,
    });

    let new_repo = Repository::new(new_frontier.clone(), new_patches);
    validate_repository(&new_repo)?;

    let snap_dir = root.join(".snap");
    write_repository_atomic(&snap_dir, &new_repo)?;

    println!("{new_frontier}");
    Ok(())
}

use base64::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Execute `snap diff`.
pub fn cmd_diff(target: DiffTarget) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let local_repo = load_repository(&root)?;

    match target {
        DiffTarget::WorkingTree => {
            let working_tree = scan_working_tree(&root)?;
            let (current_tree, _) = materialize_version(&local_repo.patches, &local_repo.frontier)?;
            let diff_output = format_tree_diff(current_tree.entries(), &working_tree)?;
            print!("{diff_output}");
            Ok(())
        }
        DiffTarget::Versions { old, new, repo } => {
            let old_version =
                Version::parse(&old).map_err(|e| CliError::InvalidVersion(e.to_string()))?;
            let new_version =
                Version::parse(&new).map_err(|e| CliError::InvalidVersion(e.to_string()))?;

            if !crate::core::validation::is_version_known(&local_repo, &old_version) {
                return Err(CliError::UnknownVersion(old));
            }

            let new_repo = if let Some(ref remote_src) = repo {
                let remote = load_remote_repository(remote_src)?;
                check_dot_collisions(&local_repo, &remote)?;
                remote
            } else {
                local_repo.clone()
            };

            if !crate::core::validation::is_version_known(&new_repo, &new_version) {
                return Err(CliError::UnknownVersion(new));
            }

            let (old_tree, _) = materialize_version(&local_repo.patches, &old_version)?;
            let (new_tree, _) = materialize_version(&new_repo.patches, &new_version)?;

            let diff_output = format_tree_diff(old_tree.entries(), new_tree.entries())?;
            print!("{diff_output}");
            Ok(())
        }
    }
}

/// Execute `snap revert <version>`.
pub fn cmd_revert(version_str: &str) -> Result<(), CliError> {
    let target_version =
        Version::parse(version_str).map_err(|e| CliError::InvalidVersion(e.to_string()))?;

    let root = find_repository_root()?;
    let mut repo = load_repository(&root)?;

    if !crate::core::validation::is_version_known(&repo, &target_version) {
        return Err(CliError::UnknownVersion(version_str.to_string()));
    }

    let author = resolve_contributor_id(Some(&root))?.ok_or(ConfigError::MissingContributorId)?;

    // Working tree must be clean and free of unsupported entries
    let working_tree = scan_working_tree(&root)?;
    let (current_tree, _) = materialize_version(&repo.patches, &repo.frontier)?;

    let diff = diff_working_tree(&working_tree, current_tree.entries());
    if !diff.is_clean() {
        return Err(CliError::WorkingTreeIsDirty);
    }

    // Materialize target tree
    let (target_tree, _) = materialize_version(&repo.patches, &target_version)?;

    if current_tree.entries() == target_tree.entries() {
        return Err(CliError::TargetTreeAlreadyCurrent);
    }

    // Compute changes from current_tree to target_tree
    let mut changes: Vec<Change> = Vec::new();
    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    for p in current_tree.keys() {
        all_paths.insert(p.as_str());
    }
    for p in target_tree.keys() {
        all_paths.insert(p.as_str());
    }

    for path in all_paths {
        let old_content = current_tree.get(path);
        let new_content = target_tree.get(path);

        match (old_content, new_content) {
            (Some(_), None) => {
                changes.push(Change::Delete {
                    path: path.to_string(),
                });
            }
            (None, Some(new_bytes)) => {
                if is_text(new_bytes) {
                    let tokens = tokenize_text(new_bytes)?;
                    changes.push(Change::Text {
                        path: path.to_string(),
                        edit: vec![TextEditOp::Insert(tokens)],
                    });
                } else {
                    let content = BASE64_STANDARD.encode(new_bytes);
                    changes.push(Change::Put {
                        path: path.to_string(),
                        content,
                    });
                }
            }
            (Some(old_bytes), Some(new_bytes)) => {
                if old_bytes != new_bytes {
                    if is_text(old_bytes) && is_text(new_bytes) {
                        let old_tokens = tokenize_text(old_bytes)?;
                        let new_tokens = tokenize_text(new_bytes)?;
                        let edit = diff_tokens(&old_tokens, &new_tokens);
                        changes.push(Change::Text {
                            path: path.to_string(),
                            edit,
                        });
                    } else {
                        let content = BASE64_STANDARD.encode(new_bytes);
                        changes.push(Change::Put {
                            path: path.to_string(),
                            content,
                        });
                    }
                }
            }
            (None, None) => {}
        }
    }

    let current_rev = repo.frontier.get(&author);
    let new_rev = current_rev + 1;
    if new_rev > MAX_REVISION {
        return Err(CliError::Custom(format!(
            "revision overflow beyond maximum safe integer ({MAX_REVISION})"
        )));
    }

    let message = format!("revert to {target_version}");
    let patch = Patch {
        author,
        revision: new_rev,
        base: repo.frontier.clone(),
        message,
        changes,
    };

    let result_version = patch_result_version(&patch);
    let new_frontier = repo.frontier.join(&result_version);

    repo.patches.push(patch);
    repo.patches
        .sort_by(|p1, p2| match p1.author.cmp(&p2.author) {
            std::cmp::Ordering::Equal => p1.revision.cmp(&p2.revision),
            ord => ord,
        });
    let new_repo = Repository::new(new_frontier.clone(), repo.patches);
    validate_repository(&new_repo)?;

    // Materialize target tree onto working tree
    crate::fs::materializer::materialize_tree(
        &root,
        current_tree.entries(),
        target_tree.entries(),
    )?;

    // Atomically write updated repository.json
    let snap_dir = root.join(".snap");
    write_repository_atomic(&snap_dir, &new_repo)?;

    println!("{new_frontier}");
    Ok(())
}

/// Execute `snap merge <repository>`.
pub fn cmd_merge(repo_source: &str) -> Result<(), CliError> {
    let root = find_repository_root()?;
    let local_repo = load_repository(&root)?;

    // 1. Scan working tree: must be clean and free of unsupported entries
    let working_tree = scan_working_tree(&root)?;
    let (current_tree, local_warnings) =
        materialize_version(&local_repo.patches, &local_repo.frontier)?;

    let diff = diff_working_tree(&working_tree, current_tree.entries());
    if !diff.is_clean() {
        return Err(CliError::WorkingTreeIsDirty);
    }

    // 2. Load and validate other repository
    let remote_repo = load_remote_repository(repo_source)?;

    // 3. Compare common dots for patch collisions
    check_dot_collisions(&local_repo, &remote_repo)?;

    // 4. Check if other repository is already contained or equal
    let joined_frontier = local_repo.frontier.join(&remote_repo.frontier);
    if joined_frontier == local_repo.frontier {
        let mut all_present = true;
        let local_dots: HashSet<_> = local_repo
            .patches
            .iter()
            .map(|p| (&p.author, p.revision))
            .collect();
        for p in &remote_repo.patches {
            if !local_dots.contains(&(&p.author, p.revision)) {
                all_present = false;
                break;
            }
        }
        if all_present {
            // No-op: silent stderr, prints unchanged version to stdout
            println!("{joined_frontier}");
            return Ok(());
        }
    }

    // 5. Union patch sets (§4.1: sorted by author ascending, then numeric revision)
    let mut unioned_map: BTreeMap<(ContributorId, u64), Patch> = BTreeMap::new();
    for p in local_repo.patches {
        unioned_map.insert((p.author.clone(), p.revision), p);
    }
    for p in remote_repo.patches {
        unioned_map.insert((p.author.clone(), p.revision), p);
    }
    let unioned_patches: Vec<Patch> = unioned_map.into_values().collect();

    let merged_repo = Repository::new(joined_frontier.clone(), unioned_patches);
    validate_repository(&merged_repo)?;

    // 6. Canonically replay merged repository
    let (merged_tree, merged_warnings) =
        materialize_version(&merged_repo.patches, &joined_frontier)?;

    // 7. Calculate warning diff: new warnings emitted during merge (§6.4)
    let new_warnings: Vec<_> = merged_warnings.difference(&local_warnings).collect();
    for w in new_warnings {
        eprintln!("{w}");
    }

    // 8. Materialize merged tree and atomically replace repository.json
    crate::fs::materializer::materialize_tree(
        &root,
        current_tree.entries(),
        merged_tree.entries(),
    )?;
    let snap_dir = root.join(".snap");
    write_repository_atomic(&snap_dir, &merged_repo)?;

    println!("{joined_frontier}");
    Ok(())
}
