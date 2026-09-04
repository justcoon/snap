use std::collections::{BTreeMap, HashSet};

use crate::cli::commands::common::{
    check_dot_collisions, find_repository_root, load_remote_repository, load_repository,
};
use crate::cli::commands::CliError;
use crate::core::patch::{Patch, Repository};
use crate::core::replay::materialize_version;
use crate::core::validation::validate_repository;
use crate::core::version::ContributorId;
use crate::fs::materializer::write_repository_atomic;
use crate::fs::scanner::{diff_working_tree, scan_working_tree};
use crate::presentation::{format_action_success, format_warning, StreamModes};

/// Execute `snap merge <repository>`.
pub fn cmd_merge(repo_source: &str, modes: StreamModes) -> Result<(), CliError> {
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
            print!(
                "{}",
                format_action_success("Merged", &joined_frontier, modes.stdout)
            );
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
        let detail = format!("auto-resolved {}: {}", w.path, w.reason);
        eprint!("{}", format_warning(&detail, modes.stderr));
    }

    // 8. Materialize merged tree and atomically replace repository.json
    crate::fs::materializer::materialize_tree(
        &root,
        current_tree.entries(),
        merged_tree.entries(),
    )?;
    let snap_dir = root.join(".snap");
    write_repository_atomic(&snap_dir, &merged_repo)?;

    print!(
        "{}",
        format_action_success("Merged", &joined_frontier, modes.stdout)
    );
    Ok(())
}
