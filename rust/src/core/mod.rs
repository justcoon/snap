pub mod version;

pub use version::{
    parse_revision, CausalRelation, ContributorId, ContributorIdError, RevisionError, Version,
    VersionError, MAX_REVISION,
};
