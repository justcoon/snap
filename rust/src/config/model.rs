use serde::{Deserialize, Serialize};

use crate::core::version::ContributorId;

/// Contributor configuration section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContributorConfig {
    pub id: ContributorId,
}

/// Root Snap configuration structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapConfig {
    pub contributor: ContributorConfig,
}

impl SnapConfig {
    pub fn new(id: ContributorId) -> Self {
        Self {
            contributor: ContributorConfig { id },
        }
    }
}
