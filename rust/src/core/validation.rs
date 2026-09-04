use std::collections::BTreeMap;
use std::fmt;

use crate::core::patch::{Change, Patch, Repository};
use crate::core::replay::{materialize_version, ReplayError};
use crate::core::version::ContributorId;

/// Errors that can occur during full repository graph and schema validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    UnsupportedFormat(u32),
    UnsortedPatches {
        previous: String,
        current: String,
    },
    NonContiguousRevision {
        author: ContributorId,
        expected: u64,
        got: u64,
    },
    DotCollisionDifferentPayload {
        author: ContributorId,
        revision: u64,
    },
    MissingBasePatch {
        author: ContributorId,
        revision: u64,
    },
    InvalidRevisionBaseRelation {
        author: ContributorId,
        revision: u64,
        expected: u64,
    },
    UnreachablePatch {
        author: ContributorId,
        revision: u64,
    },
    ChangeInvalidAgainstBaseTree {
        path: String,
        reason: String,
    },
    ReplayFailed(ReplayError),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::UnsupportedFormat(fmt) => {
                write!(f, "unsupported repository format {fmt}: expected 1")
            }
            ValidationError::UnsortedPatches { previous, current } => {
                write!(
                    f,
                    "patches are not canonically sorted: '{current}' must appear after '{previous}'"
                )
            }
            ValidationError::NonContiguousRevision {
                author,
                expected,
                got,
            } => {
                write!(
                    f,
                    "non-contiguous revision for author '{author}': expected {expected}, got {got}"
                )
            }
            ValidationError::DotCollisionDifferentPayload { author, revision } => {
                write!(
                    f,
                    "repository corruption: dot collision at ({author}, {revision}) with different patch payloads"
                )
            }
            ValidationError::MissingBasePatch { author, revision } => {
                write!(
                    f,
                    "missing prerequisite patch ({author}->{revision}) in causal base"
                )
            }
            ValidationError::InvalidRevisionBaseRelation {
                author,
                revision,
                expected,
            } => {
                write!(
                    f,
                    "patch for author '{author}' has revision {revision}, expected {expected} based on base version"
                )
            }
            ValidationError::UnreachablePatch { author, revision } => {
                write!(
                    f,
                    "unreachable patch ({author}->{revision}) is not in causal closure of frontier"
                )
            }
            ValidationError::ChangeInvalidAgainstBaseTree { path, reason } => {
                write!(
                    f,
                    "change for path '{path}' is invalid against base tree: {reason}"
                )
            }
            ValidationError::ReplayFailed(e) => write!(f, "replay failed: {e}"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Perform complete validation of a repository graph according to SPEC §4.1, §4.5.
pub fn validate_repository(repo: &Repository) -> Result<(), ValidationError> {
    if repo.format != 1 {
        return Err(ValidationError::UnsupportedFormat(repo.format));
    }

    // 1. Validate sorting, dot uniqueness, and contiguous revisions
    let mut author_revisions: BTreeMap<ContributorId, u64> = BTreeMap::new();
    let mut known_dots: BTreeMap<(ContributorId, u64), &Patch> = BTreeMap::new();

    let mut prev_dot: Option<(ContributorId, u64)> = None;

    for patch in &repo.patches {
        let dot = (patch.author.clone(), patch.revision);

        // Check canonical sort order: author ascending, then revision ascending
        if let Some((prev_author, prev_rev)) = &prev_dot {
            match patch.author.as_str().cmp(prev_author.as_str()) {
                std::cmp::Ordering::Less => {
                    return Err(ValidationError::UnsortedPatches {
                        previous: format!("{prev_author}->{prev_rev}"),
                        current: format!("{}->{}", patch.author, patch.revision),
                    });
                }
                std::cmp::Ordering::Equal => {
                    if patch.revision <= *prev_rev {
                        if patch.revision == *prev_rev {
                            // Check if payload differs
                            if let Some(existing) = known_dots.get(&dot) {
                                if *existing != patch {
                                    return Err(ValidationError::DotCollisionDifferentPayload {
                                        author: patch.author.clone(),
                                        revision: patch.revision,
                                    });
                                }
                            }
                        }
                        return Err(ValidationError::UnsortedPatches {
                            previous: format!("{prev_author}->{prev_rev}"),
                            current: format!("{}->{}", patch.author, patch.revision),
                        });
                    }
                }
                std::cmp::Ordering::Greater => {}
            }
        }

        // Check contiguous revisions
        let current_max = author_revisions.entry(patch.author.clone()).or_insert(0);
        if patch.revision != *current_max + 1 {
            return Err(ValidationError::NonContiguousRevision {
                author: patch.author.clone(),
                expected: *current_max + 1,
                got: patch.revision,
            });
        }
        *current_max = patch.revision;

        known_dots.insert(dot.clone(), patch);
        prev_dot = Some(dot);
    }

    // 2. Validate revision = base[author] + 1 and base closure completeness
    for patch in &repo.patches {
        let expected_rev = patch.base.get(&patch.author) + 1;
        if patch.revision != expected_rev {
            return Err(ValidationError::InvalidRevisionBaseRelation {
                author: patch.author.clone(),
                revision: patch.revision,
                expected: expected_rev,
            });
        }

        for (base_author, base_rev) in patch.base.iter() {
            if !known_dots.contains_key(&(base_author.clone(), *base_rev)) {
                return Err(ValidationError::MissingBasePatch {
                    author: base_author.clone(),
                    revision: *base_rev,
                });
            }
        }
    }

    // 3. Validate causal closure of frontier: no unreachable patches (§4.1)
    for patch in &repo.patches {
        let frontier_rev = repo.frontier.get(&patch.author);
        if patch.revision > frontier_rev {
            return Err(ValidationError::UnreachablePatch {
                author: patch.author.clone(),
                revision: patch.revision,
            });
        }
    }

    // 4. Validate every change against its exact materialized base tree (§4.3)
    // A text or put creation requires the path to be absent in the patch's exact base tree.
    // An edit, replacement, or delete requires it to be present.
    // A change that does not alter path existence or bytes is invalid, except that an empty text
    // edit may create an empty file.
    for patch in &repo.patches {
        let (base_tree, _) = materialize_version(&repo.patches, &patch.base)
            .map_err(ValidationError::ReplayFailed)?;

        for change in &patch.changes {
            let path = change.path();
            let base_bytes = base_tree.get(path);

            match change {
                Change::Text { edit, .. } => {
                    if let Some(base_val) = base_bytes {
                        // Edit: path must be present in base tree (already true)
                        // Must alter bytes
                        let old_tokens =
                            crate::core::diff::tokenize_text(base_val).map_err(|e| {
                                ValidationError::ChangeInvalidAgainstBaseTree {
                                    path: path.to_string(),
                                    reason: e.to_string(),
                                }
                            })?;
                        let new_tokens =
                            crate::core::diff::apply_edit(&old_tokens, edit).map_err(|e| {
                                ValidationError::ChangeInvalidAgainstBaseTree {
                                    path: path.to_string(),
                                    reason: e.to_string(),
                                }
                            })?;
                        let new_bytes = new_tokens.join("").into_bytes();
                        if new_bytes == base_val {
                            return Err(ValidationError::ChangeInvalidAgainstBaseTree {
                                path: path.to_string(),
                                reason: "text edit does not alter file content".to_string(),
                            });
                        }
                    }
                }
                Change::Put { content, .. } => {
                    use base64::prelude::*;
                    let new_bytes = match BASE64_STANDARD.decode(content) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            return Err(ValidationError::ChangeInvalidAgainstBaseTree {
                                path: path.to_string(),
                                reason: format!("invalid base64 content: {e}"),
                            });
                        }
                    };
                    if let Some(existing) = base_bytes {
                        if existing == new_bytes {
                            return Err(ValidationError::ChangeInvalidAgainstBaseTree {
                                path: path.to_string(),
                                reason: "put change does not alter file content".to_string(),
                            });
                        }
                    }
                }
                Change::Delete { .. } => {
                    if base_bytes.is_none() {
                        return Err(ValidationError::ChangeInvalidAgainstBaseTree {
                            path: path.to_string(),
                            reason: "deleted path was not present in base tree".to_string(),
                        });
                    }
                }
            }
        }
    }

    // 5. Replay of declared frontier
    materialize_version(&repo.patches, &repo.frontier).map_err(ValidationError::ReplayFailed)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patch::TextEditOp;
    use crate::core::version::Version;

    #[test]
    fn test_scenario_b2_patch_continuity_and_serial_contributor() {
        let alice = ContributorId::parse("alice@x").unwrap();

        // Contributor alice has rev 1 and rev 3, but missing rev 2
        let p1 = Patch {
            author: alice.clone(),
            revision: 1,
            base: Version::empty(),
            message: "rev 1".to_string(),
            changes: vec![Change::Text {
                path: "f.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec!["1\n".to_string()])],
            }],
        };
        let p3 = Patch {
            author: alice.clone(),
            revision: 3,
            base: Version::parse("(alice@x->2)").unwrap(),
            message: "rev 3".to_string(),
            changes: vec![Change::Text {
                path: "f.txt".to_string(),
                edit: vec![
                    TextEditOp::Delete(1),
                    TextEditOp::Insert(vec!["3\n".to_string()]),
                ],
            }],
        };

        let repo = Repository::new(Version::parse("(alice@x->3)").unwrap(), vec![p1, p3]);
        let err = validate_repository(&repo).unwrap_err();
        assert!(matches!(err, ValidationError::NonContiguousRevision { .. }));
    }

    #[test]
    fn test_scenario_b3_dot_collision_detection() {
        let alice = ContributorId::parse("alice@x").unwrap();

        let p1 = Patch {
            author: alice.clone(),
            revision: 1,
            base: Version::empty(),
            message: "first commit".to_string(),
            changes: vec![Change::Text {
                path: "file.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec!["1\n".to_string()])],
            }],
        };
        let p1_conflict = Patch {
            author: alice.clone(),
            revision: 1,
            base: Version::empty(),
            message: "different commit".to_string(),
            changes: vec![Change::Text {
                path: "other.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec!["other\n".to_string()])],
            }],
        };

        let repo = Repository::new(
            Version::parse("(alice@x->1)").unwrap(),
            vec![p1, p1_conflict],
        );
        let err = validate_repository(&repo).unwrap_err();
        assert!(matches!(
            err,
            ValidationError::DotCollisionDifferentPayload { .. }
                | ValidationError::UnsortedPatches { .. }
        ));
    }
}
