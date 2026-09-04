pub mod model;

use std::fmt;
use std::fs;
use std::path::Path;

use crate::core::patch::validate_json_strict;
use crate::core::version::ContributorId;
use crate::fs::materializer::atomic_replace_file;
pub use model::SnapConfig;

/// Canonical configuration key for author identity (§5.1).
pub const CONTRIBUTOR_ID_KEY: &str = "contributor.id";
/// Standard environment variable pointing to the user home directory (§5.1).
pub const ENV_HOME: &str = "HOME";

/// Errors resulting from loading, parsing, or writing configuration.
#[derive(Debug)]
pub enum ConfigError {
    InvalidJson(String),
    DuplicateKey(String),
    FloatingPointNotAllowed,
    InvalidContributorId(String),
    Io { path: String, message: String },
    MissingContributorId,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
            ConfigError::DuplicateKey(key) => write!(f, "duplicate JSON key '{key}'"),
            ConfigError::FloatingPointNotAllowed => {
                write!(f, "floating-point numbers are not allowed")
            }
            ConfigError::InvalidContributorId(id) => write!(f, "invalid contributor id: {id}"),
            ConfigError::Io { path, message } => write!(f, "cannot access '{path}': {message}"),
            ConfigError::MissingContributorId => {
                write!(
                    f,
                    "{CONTRIBUTOR_ID_KEY} is required; configure it locally or globally"
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContributorConfig {
    id: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSnapConfig {
    contributor: RawContributorConfig,
}

/// Parse and strictly validate configuration JSON content.
pub fn parse_config(content: &str) -> Result<SnapConfig, ConfigError> {
    validate_json_strict(content.as_bytes()).map_err(|e| match e {
        crate::core::patch::StrictJsonError::DuplicateKey(k) => ConfigError::DuplicateKey(k),
        crate::core::patch::StrictJsonError::FloatingPointNumberNotAllowed => {
            ConfigError::FloatingPointNotAllowed
        }
        crate::core::patch::StrictJsonError::InvalidJson(msg) => ConfigError::InvalidJson(msg),
    })?;

    let raw: RawSnapConfig =
        serde_json::from_str(content).map_err(|e| ConfigError::InvalidJson(e.to_string()))?;

    let contributor_id = ContributorId::parse(&raw.contributor.id)
        .map_err(|e| ConfigError::InvalidContributorId(e.to_string()))?;

    Ok(SnapConfig {
        contributor: model::ContributorConfig { id: contributor_id },
    })
}

/// Resolve the active contributor ID according to local-over-global precedence.
///
/// 1. If local `.snap/config.json` exists, it is read and validated.
///    Any syntax error or malformed structure in a file that is read produces an error.
/// 2. Otherwise, if `$HOME/.snapconfig.json` exists, it is read and validated.
/// 3. If neither exists, returns `Ok(None)`.
pub fn resolve_contributor_id(
    repo_root: Option<&Path>,
) -> Result<Option<ContributorId>, ConfigError> {
    // 1. Check local configuration
    if let Some(root) = repo_root {
        let local_config_path = crate::fs::local_config_path(root);
        if local_config_path.exists() {
            let content = fs::read_to_string(&local_config_path).map_err(|e| ConfigError::Io {
                path: local_config_path.display().to_string(),
                message: e.to_string(),
            })?;
            let config = parse_config(&content)?;
            return Ok(Some(config.contributor.id));
        }
    }

    // 2. Check global configuration
    if let Some(home) = std::env::var_os(ENV_HOME) {
        let global_config_path = crate::fs::global_config_path(Path::new(&home));
        if global_config_path.exists() {
            let content = fs::read_to_string(&global_config_path).map_err(|e| ConfigError::Io {
                path: global_config_path.display().to_string(),
                message: e.to_string(),
            })?;
            let config = parse_config(&content)?;
            return Ok(Some(config.contributor.id));
        }
    }

    Ok(None)
}

/// Atomically write configuration to the specified destination path.
pub fn write_config(path: &Path, id: &ContributorId) -> Result<(), ConfigError> {
    let config = SnapConfig::new(id.clone());
    let json_string = serde_json::to_string_pretty(&config)
        .map_err(|e| ConfigError::InvalidJson(e.to_string()))?;
    let mut bytes = json_string.into_bytes();
    bytes.push(b'\n');

    atomic_replace_file(path, &bytes).map_err(|e| ConfigError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let json = r#"{"contributor":{"id":"alice@example.com"}}"#;
        let config = parse_config(json).unwrap();
        assert_eq!(config.contributor.id.as_str(), "alice@example.com");
    }

    #[test]
    fn test_parse_rejects_unknown_fields() {
        let json = r#"{"contributor":{"id":"alice@example.com"},"unknown":true}"#;
        assert!(parse_config(json).is_err());
    }

    #[test]
    fn test_parse_rejects_duplicate_keys() {
        let json = r#"{"contributor":{"id":"a@x","id":"b@x"}}"#;
        assert!(matches!(
            parse_config(json),
            Err(ConfigError::DuplicateKey(_))
        ));
    }
}
