use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use base64::prelude::*;

use crate::core::diff::{apply_edit, diff_tokens, is_text, tokenize_text};
use crate::core::ot::transform_edit;
use crate::core::patch::{Change, Patch};
use crate::core::version::{ContributorId, Version};

/// A materialized in-memory file tree mapping tracked UTF-8 paths to file bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileTree {
    entries: BTreeMap<String, Vec<u8>>,
}

impl FileTree {
    pub fn new() -> Self {
        FileTree {
            entries: BTreeMap::new(),
        }
    }

    pub fn entries(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.entries
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.entries.get(path).map(|v| v.as_slice())
    }

    pub fn insert(&mut self, path: String, content: Vec<u8>) {
        self.entries.insert(path, content);
    }

    pub fn remove(&mut self, path: &str) -> Option<Vec<u8>> {
        self.entries.remove(path)
    }

    pub fn contains_key(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.entries.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }
}

/// An auto-resolution warning fact emitted during replay when whole-file conflict resolution occurs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResolutionWarning {
    pub path: String,
    pub reason: String,
}

impl fmt::Display for ResolutionWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "warning: auto-resolved {}: {}", self.path, self.reason)
    }
}

/// Errors that can occur during patch replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    MissingBaseDependency {
        author: ContributorId,
        revision: u64,
    },
    CausalCycleOrMissingDependency,
    TextApplicationFailed(String),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::MissingBaseDependency { author, revision } => {
                write!(
                    f,
                    "cyclic or incomplete patch history: missing patch dependency ({author}->{revision})"
                )
            }
            ReplayError::CausalCycleOrMissingDependency => {
                write!(
                    f,
                    "cyclic or incomplete patch history: history contains a causal cycle or missing dependency"
                )
            }
            ReplayError::TextApplicationFailed(msg) => {
                write!(f, "failed to apply text edit: {msg}")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// Result version of a patch: base with result[author] = revision (§4.2).
pub fn patch_result_version(patch: &Patch) -> Version {
    let mut entries = BTreeMap::new();
    for (author, rev) in patch.base.iter() {
        entries.insert(author.clone(), *rev);
    }
    entries.insert(patch.author.clone(), patch.revision);
    Version::from_map_unchecked(entries)
}

/// Compute the sequence of patches in canonical integration order for a target version V
/// according to SPEC §6.1.
pub fn canonical_integration_order<'a>(
    patches: &'a [Patch],
    target: &Version,
) -> Result<Vec<&'a Patch>, ReplayError> {
    // 1. Select every patch (c, n) where n <= V[c] (§6.1)
    let mut selected_map: BTreeMap<(ContributorId, u64), &'a Patch> = BTreeMap::new();
    for p in patches {
        if p.revision <= target.get(&p.author) {
            selected_map.insert((p.author.clone(), p.revision), p);
        }
    }

    // Verify selected set contains every selected patch's base
    for patch in selected_map.values() {
        for (base_author, base_rev) in patch.base.iter() {
            if !selected_map.contains_key(&(base_author.clone(), *base_rev)) {
                return Err(ReplayError::MissingBaseDependency {
                    author: base_author.clone(),
                    revision: *base_rev,
                });
            }
        }
    }

    let mut integrated_dots: HashSet<(ContributorId, u64)> = HashSet::new();
    let mut order: Vec<&'a Patch> = Vec::with_capacity(selected_map.len());

    // Loop until all selected patches are integrated
    while integrated_dots.len() < selected_map.len() {
        // Find ready patches
        let mut ready_patches: Vec<&'a Patch> = Vec::new();
        for ((author, rev), patch) in &selected_map {
            if integrated_dots.contains(&(author.clone(), *rev)) {
                continue;
            }
            // Ready if all base dots are integrated
            let base_ready = patch
                .base
                .iter()
                .all(|(ba, br)| integrated_dots.contains(&(ba.clone(), *br)));
            if base_ready {
                ready_patches.push(*patch);
            }
        }

        if ready_patches.is_empty() {
            return Err(ReplayError::CausalCycleOrMissingDependency);
        }

        // Choose the least ready patch by (§6.1):
        // 1. Snap order of result versions
        // 2. Unsigned UTF-8 order of author
        // 3. Numeric revision
        ready_patches.sort_by(|p1, p2| {
            let res1 = patch_result_version(p1);
            let res2 = patch_result_version(p2);
            match res1.cmp_snap_order(&res2) {
                std::cmp::Ordering::Equal => match p1.author.as_str().cmp(p2.author.as_str()) {
                    std::cmp::Ordering::Equal => p1.revision.cmp(&p2.revision),
                    ord => ord,
                },
                ord => ord,
            }
        });

        let patch = ready_patches[0];
        order.push(patch);
        integrated_dots.insert((patch.author.clone(), patch.revision));
    }

    Ok(order)
}

/// Materialize the exact canonical file tree and auto-resolution warnings
/// for a target version V from a set of patches according to SPEC §6.
pub fn materialize_version(
    patches: &[Patch],
    target: &Version,
) -> Result<(FileTree, BTreeSet<ResolutionWarning>), ReplayError> {
    let order = canonical_integration_order(patches, target)?;

    let mut current_tree = FileTree::new();
    let mut integrated_v = Version::empty();
    let mut all_warnings: BTreeSet<ResolutionWarning> = BTreeSet::new();

    // Cache of materialized base trees keyed by version
    let mut tree_cache: BTreeMap<Version, FileTree> = BTreeMap::new();
    tree_cache.insert(Version::empty(), FileTree::new());

    for patch in order {
        // Materialize exact base tree B
        let base_tree = if let Some(cached) = tree_cache.get(&patch.base) {
            cached.clone()
        } else {
            // Recompute base tree recursively
            let (b_tree, _) = materialize_version(patches, &patch.base)?;
            tree_cache.insert(patch.base.clone(), b_tree.clone());
            b_tree
        };

        // Integrate patch into current_tree
        integrate_single_patch(&mut current_tree, &base_tree, patch, &mut all_warnings)?;

        // Update integrated state
        let result_v = patch_result_version(patch);
        integrated_v = integrated_v.join(&result_v);

        tree_cache.insert(integrated_v.clone(), current_tree.clone());
    }

    Ok((current_tree, all_warnings))
}

/// Helper: compute the authored result content T of applying a single change to base content B.
fn compute_authored_result(
    base_content: Option<&[u8]>,
    change: &Change,
) -> Result<Option<Vec<u8>>, ReplayError> {
    match change {
        Change::Delete { .. } => Ok(None),
        Change::Put { content, .. } => {
            let bytes = BASE64_STANDARD
                .decode(content)
                .map_err(|e| ReplayError::TextApplicationFailed(format!("invalid base64: {e}")))?;
            Ok(Some(bytes))
        }
        Change::Text { edit, .. } => {
            let old_tokens = if let Some(bytes) = base_content {
                tokenize_text(bytes)
                    .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?
            } else {
                Vec::new()
            };
            let new_tokens = apply_edit(&old_tokens, edit)
                .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?;
            let full_text = new_tokens.join("");
            Ok(Some(full_text.into_bytes()))
        }
    }
}

/// Integrate a single patch into the current canonical tree C according to SPEC §6.2.
fn integrate_single_patch(
    current_tree: &mut FileTree,
    base_tree: &FileTree,
    patch: &Patch,
    warnings: &mut BTreeSet<ResolutionWarning>,
) -> Result<(), ReplayError> {
    let mut authored_results: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
    for change in &patch.changes {
        let p = change.path();
        let b_content = base_tree.get(p);
        let t_content = compute_authored_result(b_content, change)?;
        authored_results.insert(p.to_string(), t_content);
    }

    // Step 1: Whole-patch namespace conflict resolution (§6.2)
    // S: paths that P makes present
    let s_paths: Vec<String> = patch
        .changes
        .iter()
        .filter(|c| !matches!(c, Change::Delete { .. }))
        .map(|c| c.path().to_string())
        .collect();

    // C': C with every path that P authored as a deletion removed
    let mut c_prime = current_tree.clone();
    for change in &patch.changes {
        if matches!(change, Change::Delete { .. }) {
            c_prime.remove(change.path());
        }
    }

    let mut namespace_settled_incoming: HashSet<String> = HashSet::new();
    let mut namespace_paths_to_remove: BTreeSet<String> = BTreeSet::new();

    for p in &s_paths {
        for current_path in c_prime.keys() {
            let is_ancestor = p.starts_with(&format!("{current_path}/"));
            let is_descendant = current_path.starts_with(&format!("{p}/"));

            if is_ancestor || is_descendant {
                namespace_settled_incoming.insert(p.clone());
                namespace_paths_to_remove.insert(current_path.clone());
                warnings.insert(ResolutionWarning {
                    path: current_path.clone(),
                    reason: "namespace-wins".to_string(),
                });
            }
        }
    }

    // Remove conflicting current paths marked by namespace resolution
    for p_rem in &namespace_paths_to_remove {
        current_tree.remove(p_rem);
    }

    // Step 2: Evaluate paths changed by P (§6.2, §6.4)
    for change in &patch.changes {
        let path = change.path();
        let t_content = authored_results.get(path).cloned().flatten();

        if namespace_settled_incoming.contains(path) {
            // Settled by namespace rule: installed as authored result
            if let Some(bytes) = t_content {
                current_tree.insert(path.to_string(), bytes);
            }
            continue;
        }

        let b_content = base_tree.get(path);
        let c_content = current_tree.get(path);

        // 1. If B and C are identical, apply authored change directly
        if b_content == c_content {
            if let Some(bytes) = t_content {
                current_tree.insert(path.to_string(), bytes);
            } else {
                current_tree.remove(path);
            }
            continue;
        }

        // 2. If C and T are identical, keep C unchanged (collapses identical concurrent changes)
        if c_content == t_content.as_deref() {
            continue;
        }

        // 3. If B, C, and T are text and change is Text: line OT
        if let Change::Text { edit: p_edit, .. } = change {
            if let (Some(b_bytes), Some(c_bytes), Some(t_bytes)) =
                (b_content, c_content, t_content.as_deref())
            {
                if is_text(b_bytes) && is_text(c_bytes) && is_text(t_bytes) {
                    let b_tokens = tokenize_text(b_bytes)
                        .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?;
                    let c_tokens = tokenize_text(c_bytes)
                        .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?;

                    // Derive aggregate context edit Q = diff(B, C)
                    let q_edit = diff_tokens(&b_tokens, &c_tokens);

                    // Transform P through Q
                    let p_prime = transform_edit(p_edit, &q_edit)
                        .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?;

                    // Apply P' to C
                    let merged_tokens = apply_edit(&c_tokens, &p_prime)
                        .map_err(|e| ReplayError::TextApplicationFailed(e.to_string()))?;

                    let merged_bytes = merged_tokens.join("").into_bytes();
                    current_tree.insert(path.to_string(), merged_bytes);
                    continue;
                }
            }
        }

        // 4. Otherwise use §6.4 path-level rules:
        // Rule 1: C == T (already handled)
        // Rule 2: If T is absent, incoming delete wins (delete-wins)
        if t_content.is_none() {
            current_tree.remove(path);
            warnings.insert(ResolutionWarning {
                path: path.to_string(),
                reason: "delete-wins".to_string(),
            });
            continue;
        }

        // Rule 3: If B is present and C is absent, earlier concurrent delete wins (delete-wins)
        if b_content.is_some() && c_content.is_none() {
            current_tree.remove(path);
            warnings.insert(ResolutionWarning {
                path: path.to_string(),
                reason: "delete-wins".to_string(),
            });
            continue;
        }

        // Rule 4: If B is absent and C and T are present, incoming later create wins (later-create-wins)
        if b_content.is_none() && c_content.is_some() {
            if let Some(t_bytes) = t_content {
                current_tree.insert(path.to_string(), t_bytes);
                warnings.insert(ResolutionWarning {
                    path: path.to_string(),
                    reason: "later-create-wins".to_string(),
                });
                continue;
            }
        }

        // Rule 5: If incoming change is Put, incoming atomic replacement wins (later-put-wins)
        if matches!(change, Change::Put { .. }) {
            if let Some(t_bytes) = t_content {
                current_tree.insert(path.to_string(), t_bytes);
            }
            warnings.insert(ResolutionWarning {
                path: path.to_string(),
                reason: "later-put-wins".to_string(),
            });
            continue;
        }

        // Rule 6: Otherwise (P is text and C is non-text), incompatible current content wins (put-wins)
        warnings.insert(ResolutionWarning {
            path: path.to_string(),
            reason: "put-wins".to_string(),
        });
    }

    // Install any remaining namespace settled incoming paths
    for p in &namespace_settled_incoming {
        if let Some(Some(bytes)) = authored_results.get(p) {
            current_tree.insert(p.clone(), bytes.clone());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::patch::TextEditOp;

    #[test]
    fn test_regression_bug_005_canonical_integration_order() {
        let author_alice = ContributorId::parse("alice@example.com").unwrap();
        let author_bob = ContributorId::parse("bob@example.com").unwrap();

        let patch_bob = Patch {
            author: author_bob.clone(),
            revision: 1,
            base: Version::empty(),
            message: "root commit by bob".to_string(),
            changes: vec![Change::Put {
                path: "file.txt".to_string(),
                content: BASE64_STANDARD.encode(b"bob"),
            }],
        };

        let patch_alice = Patch {
            author: author_alice.clone(),
            revision: 1,
            base: Version::parse("(bob@example.com->1)").unwrap(),
            message: "child commit by alice".to_string(),
            changes: vec![Change::Put {
                path: "file.txt".to_string(),
                content: BASE64_STANDARD.encode(b"alice"),
            }],
        };

        let target = Version::parse("(alice@example.com->1,bob@example.com->1)").unwrap();
        // In repo.patches, Alice is first by ContributorId sort order
        let patches = vec![patch_alice.clone(), patch_bob.clone()];

        let order = canonical_integration_order(&patches, &target).unwrap();
        assert_eq!(order.len(), 2);
        // Canonical integration order must integrate Bob first (base empty), then Alice (base Bob)
        assert_eq!(order[0].author, author_bob);
        assert_eq!(order[1].author, author_alice);
    }

    #[test]
    fn test_scenario_e1_namespace_conflict_resolution() {
        // Base: empty repository
        let author_a = ContributorId::parse("alice@x").unwrap();
        let author_b = ContributorId::parse("bob@x").unwrap();

        // Branch A creates regular file "docs"
        let p_a = Patch {
            author: author_a.clone(),
            revision: 1,
            base: Version::empty(),
            message: "create docs file".to_string(),
            changes: vec![Change::Text {
                path: "docs".to_string(),
                edit: vec![TextEditOp::Insert(vec!["a docs file\n".to_string()])],
            }],
        };

        // Branch B creates file "docs/intro.txt"
        let p_b = Patch {
            author: author_b.clone(),
            revision: 1,
            base: Version::empty(),
            message: "create docs/intro.txt".to_string(),
            changes: vec![Change::Text {
                path: "docs/intro.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec!["an intro\n".to_string()])],
            }],
        };

        let target_v = Version::parse("(alice@x->1,bob@x->1)").unwrap();

        // Test with patch ordering [p_a, p_b]
        let patches1 = vec![p_a.clone(), p_b.clone()];
        let (tree1, warnings1) = materialize_version(&patches1, &target_v).unwrap();

        // Test with patch ordering [p_b, p_a] (permuting patch storage order)
        let patches2 = vec![p_b.clone(), p_a.clone()];
        let (tree2, warnings2) = materialize_version(&patches2, &target_v).unwrap();

        // Convergence: both must produce identical trees and warnings!
        assert_eq!(tree1, tree2);
        assert_eq!(warnings1, warnings2);

        // Verification of namespace-wins:
        // Canonical scheduler integrates bob@x first (bob < alice in snap order),
        // so "docs/intro.txt" is created. Then alice@x integrates "docs".
        // The conflicting current path "docs/intro.txt" is removed with warning "namespace-wins".
        assert_eq!(
            warnings1.into_iter().collect::<Vec<_>>(),
            vec![ResolutionWarning {
                path: "docs/intro.txt".to_string(),
                reason: "namespace-wins".to_string(),
            }]
        );
        assert!(tree1.contains_key("docs"));
        assert!(!tree1.contains_key("docs/intro.txt"));
    }

    #[test]
    fn test_scenario_e2_path_level_conflict_winner_rules() {
        let alice = ContributorId::parse("alice@x").unwrap();
        let bob = ContributorId::parse("bob@x").unwrap();

        // Subcase 1: Delete Wins
        // Base has "f.txt". Branch A deletes "f.txt", Branch B edits "f.txt".
        let p_init = Patch {
            author: alice.clone(),
            revision: 1,
            base: Version::empty(),
            message: "init".to_string(),
            changes: vec![Change::Text {
                path: "f.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec!["base\n".to_string()])],
            }],
        };
        let v1 = Version::parse("(alice@x->1)").unwrap();

        let p_del = Patch {
            author: alice.clone(),
            revision: 2,
            base: v1.clone(),
            message: "delete".to_string(),
            changes: vec![Change::Delete {
                path: "f.txt".to_string(),
            }],
        };
        let p_edit = Patch {
            author: bob.clone(),
            revision: 1,
            base: v1.clone(),
            message: "edit".to_string(),
            changes: vec![Change::Text {
                path: "f.txt".to_string(),
                edit: vec![
                    TextEditOp::Delete(1),
                    TextEditOp::Insert(vec!["edited\n".to_string()]),
                ],
            }],
        };

        let target_v = Version::parse("(alice@x->2,bob@x->1)").unwrap();
        let (tree, warnings) =
            materialize_version(&[p_init.clone(), p_del.clone(), p_edit.clone()], &target_v)
                .unwrap();

        assert!(!tree.contains_key("f.txt"));
        assert_eq!(
            warnings.into_iter().collect::<Vec<_>>(),
            vec![ResolutionWarning {
                path: "f.txt".to_string(),
                reason: "delete-wins".to_string(),
            }]
        );
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::core::patch::{Change, Patch, TextEditOp};
    use crate::core::version::{ContributorId, Version};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_replay_permutation_invariance_and_prefix_freedom(
            shuffle_seed in any::<u64>()
        ) {
            let alice = ContributorId::parse("alice@example.com").unwrap();
            let bob = ContributorId::parse("bob@example.com").unwrap();

            let p0 = Patch {
                author: alice.clone(),
                revision: 1,
                base: Version::empty(),
                message: "root".to_string(),
                changes: vec![Change::Text {
                    path: "docs/readme.txt".to_string(),
                    edit: vec![TextEditOp::Insert(vec!["Initial docs\n".to_string()])],
                }],
            };

            let v1 = Version::parse("(alice@example.com->1)").unwrap();
            let p1 = Patch {
                author: alice.clone(),
                revision: 2,
                base: v1.clone(),
                message: "alice change".to_string(),
                changes: vec![Change::Text {
                    path: "docs/readme.txt".to_string(),
                    edit: vec![
                        TextEditOp::Retain(1),
                        TextEditOp::Insert(vec!["Alice section\n".to_string()]),
                    ],
                }],
            };

            let p2 = Patch {
                author: bob.clone(),
                revision: 1,
                base: v1.clone(),
                message: "bob concurrent".to_string(),
                changes: vec![Change::Text {
                    path: "notes.txt".to_string(),
                    edit: vec![TextEditOp::Insert(vec!["Bob notes\n".to_string()])],
                }],
            };

            let target = Version::parse("(alice@example.com->2,bob@example.com->1)").unwrap();
            let canonical_patches = vec![p0.clone(), p1.clone(), p2.clone()];
            let (canonical_tree, canonical_warnings) =
                materialize_version(&canonical_patches, &target).expect("canonical replay must succeed");

            // Permute the input slice order using pseudo-random swap based on seed
            let mut permuted_patches = canonical_patches.clone();
            let swap_idx = (shuffle_seed % 3) as usize;
            permuted_patches.swap(0, swap_idx);

            let (permuted_tree, permuted_warnings) =
                materialize_version(&permuted_patches, &target).expect("permuted replay must succeed");

            // 1. Permutation invariance: Resulting FileTree is identical
            prop_assert_eq!(&canonical_tree, &permuted_tree);
            // 2. Permutation invariance: Warning set is identical
            prop_assert_eq!(&canonical_warnings, &permuted_warnings);

            // 3. Prefix freedom: No path in tree is a prefix of another path
            let paths: Vec<&str> = canonical_tree.keys().map(|s| s.as_str()).collect();
            let prefix_check = crate::fs::paths::check_prefix_free(paths);
            prop_assert!(prefix_check.is_ok(), "Materialized file tree must be strictly prefix-free");
        }
    }
}
