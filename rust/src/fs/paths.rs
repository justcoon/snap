use std::collections::HashSet;
use std::fmt;

/// Errors resulting from validating a tracked relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    ContainsControlChar,
    ContainsBackslash,
    EmptySegment,
    DotSegment,
    DotDotSegment,
    SnapPrefix,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(f, "path cannot be empty"),
            PathError::ContainsControlChar => write!(f, "path cannot contain control characters"),
            PathError::ContainsBackslash => write!(f, "path cannot contain backslashes"),
            PathError::EmptySegment => write!(f, "path cannot contain empty segments"),
            PathError::DotSegment => write!(f, "path cannot contain '.' segments"),
            PathError::DotDotSegment => write!(f, "path cannot contain '..' segments"),
            PathError::SnapPrefix => write!(f, "path first segment cannot equal '.snap'"),
        }
    }
}

impl std::error::Error for PathError {}

/// Validate a tracked relative path according to SPEC §2.
pub fn validate_tracked_path(path: &str) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.chars().any(|c| c.is_ascii_control()) {
        return Err(PathError::ContainsControlChar);
    }
    if path.contains('\\') {
        return Err(PathError::ContainsBackslash);
    }

    let segments: Vec<&str> = path.split('/').collect();
    for (idx, seg) in segments.iter().enumerate() {
        if seg.is_empty() {
            return Err(PathError::EmptySegment);
        }
        if *seg == "." {
            return Err(PathError::DotSegment);
        }
        if *seg == ".." {
            return Err(PathError::DotDotSegment);
        }
        if idx == 0 && *seg == ".snap" {
            return Err(PathError::SnapPrefix);
        }
    }

    Ok(())
}

/// Errors occurring when a collection of tracked paths is not prefix-free by segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixFreeError {
    Conflict {
        ancestor_file: String,
        descendant_path: String,
    },
    DuplicatePath(String),
}

impl fmt::Display for PrefixFreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrefixFreeError::Conflict {
                ancestor_file,
                descendant_path,
            } => write!(
                f,
                "prefix-free conflict: file '{ancestor_file}' is an ancestor of '{descendant_path}'"
            ),
            PrefixFreeError::DuplicatePath(path) => {
                write!(f, "duplicate path in tree: '{path}'")
            }
        }
    }
}

impl std::error::Error for PrefixFreeError {}

/// Verify that an iterator of paths is prefix-free by path segment.
///
/// If `a` is a file path, no `a/...` path may be present in the collection.
pub fn check_prefix_free<'a, I>(paths: I) -> Result<(), PrefixFreeError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut path_set = HashSet::new();
    let path_list: Vec<&'a str> = paths.into_iter().collect();

    for &path in &path_list {
        if !path_set.insert(path) {
            return Err(PrefixFreeError::DuplicatePath(path.to_string()));
        }
    }

    for &path in &path_list {
        let mut idx = 0;
        while let Some(slash_pos) = path[idx..].find('/') {
            let actual_slash_pos = idx + slash_pos;
            let ancestor = &path[..actual_slash_pos];
            if path_set.contains(ancestor) {
                return Err(PrefixFreeError::Conflict {
                    ancestor_file: ancestor.to_string(),
                    descendant_path: path.to_string(),
                });
            }
            idx = actual_slash_pos + 1;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_tracked_path() {
        assert!(validate_tracked_path("a").is_ok());
        assert!(validate_tracked_path("a/b.txt").is_ok());
        assert!(validate_tracked_path("nested/deep/file").is_ok());

        assert_eq!(validate_tracked_path(""), Err(PathError::Empty));
        assert_eq!(
            validate_tracked_path("a\0b"),
            Err(PathError::ContainsControlChar)
        );
        assert_eq!(
            validate_tracked_path("a\\b"),
            Err(PathError::ContainsBackslash)
        );
        assert_eq!(validate_tracked_path("/a"), Err(PathError::EmptySegment));
        assert_eq!(validate_tracked_path("a/"), Err(PathError::EmptySegment));
        assert_eq!(validate_tracked_path("a//b"), Err(PathError::EmptySegment));
        assert_eq!(validate_tracked_path("."), Err(PathError::DotSegment));
        assert_eq!(validate_tracked_path("./a"), Err(PathError::DotSegment));
        assert_eq!(validate_tracked_path("a/."), Err(PathError::DotSegment));
        assert_eq!(validate_tracked_path(".."), Err(PathError::DotDotSegment));
        assert_eq!(validate_tracked_path("../a"), Err(PathError::DotDotSegment));
        assert_eq!(validate_tracked_path(".snap"), Err(PathError::SnapPrefix));
        assert_eq!(
            validate_tracked_path(".snap/config.json"),
            Err(PathError::SnapPrefix)
        );
        // Note: a file named .snap later in path is allowed if not first segment
        assert!(validate_tracked_path("sub/.snap").is_ok());
    }

    #[test]
    fn test_check_prefix_free() {
        let valid = ["a/b.txt", "a/c.txt", "d.txt"];
        assert!(check_prefix_free(valid).is_ok());

        let invalid = ["a", "a/b.txt"];
        assert_eq!(
            check_prefix_free(invalid),
            Err(PrefixFreeError::Conflict {
                ancestor_file: "a".to_string(),
                descendant_path: "a/b.txt".to_string(),
            })
        );

        let duplicate = ["a/b.txt", "a/b.txt"];
        assert_eq!(
            check_prefix_free(duplicate),
            Err(PrefixFreeError::DuplicatePath("a/b.txt".to_string()))
        );

        // Disjoint prefixes with hyphen vs slash
        let disjoint = ["a", "a-b"];
        assert!(check_prefix_free(disjoint).is_ok());

        let multi_level = ["a", "a-b", "a/b/c"];
        assert_eq!(
            check_prefix_free(multi_level),
            Err(PrefixFreeError::Conflict {
                ancestor_file: "a".to_string(),
                descendant_path: "a/b/c".to_string(),
            })
        );
    }
}
