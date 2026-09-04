use std::collections::HashSet;
use std::fmt;

use base64::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::core::version::{ContributorId, Version, MAX_REVISION};

/// Errors that can occur when validating a tracked path.
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

/// A single operation in a text edit script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextEditOp {
    Retain(u64),
    Delete(u64),
    Insert(Vec<String>),
}

impl Serialize for TextEditOp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            TextEditOp::Retain(n) => map.serialize_entry("retain", n)?,
            TextEditOp::Delete(n) => map.serialize_entry("delete", n)?,
            TextEditOp::Insert(tokens) => map.serialize_entry("insert", tokens)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for TextEditOp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawEditOp {
            retain: Option<u64>,
            delete: Option<u64>,
            insert: Option<Vec<String>>,
        }

        let raw = RawEditOp::deserialize(deserializer)?;
        let count = raw.retain.is_some() as usize
            + raw.delete.is_some() as usize
            + raw.insert.is_some() as usize;

        if count != 1 {
            return Err(serde::de::Error::custom(
                "edit operation must have exactly one of 'retain', 'delete', or 'insert'",
            ));
        }

        if let Some(n) = raw.retain {
            if n == 0 {
                return Err(serde::de::Error::custom(
                    "retain count must be greater than 0",
                ));
            }
            if n > MAX_REVISION {
                return Err(serde::de::Error::custom(format!(
                    "retain count exceeds maximum safe integer ({MAX_REVISION})"
                )));
            }
            return Ok(TextEditOp::Retain(n));
        }

        if let Some(n) = raw.delete {
            if n == 0 {
                return Err(serde::de::Error::custom(
                    "delete count must be greater than 0",
                ));
            }
            if n > MAX_REVISION {
                return Err(serde::de::Error::custom(format!(
                    "delete count exceeds maximum safe integer ({MAX_REVISION})"
                )));
            }
            return Ok(TextEditOp::Delete(n));
        }

        if let Some(tokens) = raw.insert {
            if tokens.is_empty() {
                return Err(serde::de::Error::custom("insert operation cannot be empty"));
            }
            for token in &tokens {
                if token.is_empty() {
                    return Err(serde::de::Error::custom("insert token cannot be empty"));
                }
            }
            return Ok(TextEditOp::Insert(tokens));
        }

        unreachable!()
    }
}

/// A change record within a patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Change {
    #[serde(rename = "text")]
    Text { path: String, edit: Vec<TextEditOp> },
    #[serde(rename = "put")]
    Put { path: String, content: String },
    #[serde(rename = "delete")]
    Delete { path: String },
}

impl Change {
    pub fn path(&self) -> &str {
        match self {
            Change::Text { path, .. } => path,
            Change::Put { path, .. } => path,
            Change::Delete { path } => path,
        }
    }

    pub fn validate(&self) -> Result<(), PatchError> {
        validate_tracked_path(self.path()).map_err(PatchError::InvalidPath)?;

        match self {
            Change::Text { edit, .. } => {
                // Check adjacent operations of the same kind
                for i in 1..edit.len() {
                    let prev = &edit[i - 1];
                    let curr = &edit[i];
                    let same_kind = matches!(
                        (prev, curr),
                        (TextEditOp::Retain(_), TextEditOp::Retain(_))
                            | (TextEditOp::Delete(_), TextEditOp::Delete(_))
                            | (TextEditOp::Insert(_), TextEditOp::Insert(_))
                    );
                    if same_kind {
                        return Err(PatchError::AdjacentSameKindEditOps);
                    }
                }
            }
            Change::Put { content, .. } => {
                BASE64_STANDARD
                    .decode(content)
                    .map_err(|_| PatchError::InvalidBase64)?;
            }
            Change::Delete { .. } => {}
        }

        Ok(())
    }
}

/// An authored patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub author: ContributorId,
    pub revision: u64,
    pub base: Version,
    pub message: String,
    pub changes: Vec<Change>,
}

impl Patch {
    pub fn validate(&self) -> Result<(), PatchError> {
        if self.revision == 0 {
            return Err(PatchError::RevisionZero);
        }
        if self.revision > MAX_REVISION {
            return Err(PatchError::RevisionOverflow);
        }

        // revision = base[author] + 1 (§4.2)
        let expected_rev = self.base.get(&self.author) + 1;
        if self.revision != expected_rev {
            return Err(PatchError::InvalidRevisionBaseSequence {
                author: self.author.to_string(),
                revision: self.revision,
                expected: expected_rev,
            });
        }

        // Message validation
        if self.message.is_empty() {
            return Err(PatchError::EmptyMessage);
        }
        for ch in self.message.chars() {
            if ch.is_ascii_control() && ch != '\t' && ch != '\n' {
                return Err(PatchError::InvalidMessageControlChar);
            }
        }

        // Changes validation
        if self.changes.is_empty() {
            return Err(PatchError::EmptyChanges);
        }

        let mut prev_path: Option<&str> = None;
        for change in &self.changes {
            change.validate()?;
            let p = change.path();
            if let Some(prev) = prev_path {
                match p.cmp(prev) {
                    std::cmp::Ordering::Equal => {
                        return Err(PatchError::DuplicateChangePath(p.to_string()));
                    }
                    std::cmp::Ordering::Less => {
                        return Err(PatchError::UnsortedChangePaths {
                            previous: prev.to_string(),
                            current: p.to_string(),
                        });
                    }
                    std::cmp::Ordering::Greater => {}
                }
            }
            prev_path = Some(p);
        }

        Ok(())
    }
}

/// Complete repository value stored in `.snap/repository.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub format: u32,
    pub frontier: Version,
    pub patches: Vec<Patch>,
}

impl Repository {
    pub fn new(frontier: Version, patches: Vec<Patch>) -> Self {
        Repository {
            format: 1,
            frontier,
            patches,
        }
    }

    /// Serialize repository with two-space indentation and trailing LF.
    pub fn to_json_pretty(&self) -> Result<String, RepositoryError> {
        let mut json = serde_json::to_string_pretty(self).map_err(RepositoryError::Json)?;
        json.push('\n');
        Ok(json)
    }

    /// Parse repository from raw JSON bytes, strictly enforcing:
    /// - No duplicate JSON keys
    /// - No floating point numbers
    /// - Format == 1
    /// - Strict unknown field rejection
    /// - Valid patch invariants
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, RepositoryError> {
        validate_json_strict(bytes)?;

        let repo: Repository = serde_json::from_slice(bytes).map_err(RepositoryError::Json)?;
        if repo.format != 1 {
            return Err(RepositoryError::UnsupportedFormat(repo.format));
        }

        for patch in &repo.patches {
            patch.validate().map_err(RepositoryError::InvalidPatch)?;
        }

        Ok(repo)
    }
}

/// Errors that can occur when validating a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    RevisionZero,
    RevisionOverflow,
    InvalidRevisionBaseSequence {
        author: String,
        revision: u64,
        expected: u64,
    },
    EmptyMessage,
    InvalidMessageControlChar,
    EmptyChanges,
    DuplicateChangePath(String),
    UnsortedChangePaths {
        previous: String,
        current: String,
    },
    AdjacentSameKindEditOps,
    InvalidBase64,
    InvalidPath(PathError),
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchError::RevisionZero => write!(f, "revision cannot be 0"),
            PatchError::RevisionOverflow => {
                write!(f, "revision exceeds maximum safe integer ({MAX_REVISION})")
            }
            PatchError::InvalidRevisionBaseSequence {
                author,
                revision,
                expected,
            } => {
                write!(
                    f,
                    "patch for author '{author}' has revision {revision}, expected {expected} based on base version"
                )
            }
            PatchError::EmptyMessage => write!(f, "commit message cannot be empty"),
            PatchError::InvalidMessageControlChar => {
                write!(
                    f,
                    "commit message contains invalid ASCII control characters"
                )
            }
            PatchError::EmptyChanges => write!(f, "patch changes cannot be empty"),
            PatchError::DuplicateChangePath(p) => {
                write!(f, "duplicate change path '{p}' in patch")
            }
            PatchError::UnsortedChangePaths { previous, current } => {
                write!(
                    f,
                    "unsorted change paths: '{current}' must appear after '{previous}'"
                )
            }
            PatchError::AdjacentSameKindEditOps => {
                write!(f, "adjacent edit operations of the same kind are forbidden")
            }
            PatchError::InvalidBase64 => write!(f, "invalid base64 content in put change"),
            PatchError::InvalidPath(e) => write!(f, "invalid tracked path: {e}"),
        }
    }
}

impl std::error::Error for PatchError {}

/// Errors that can occur during repository JSON deserialization or validation.
#[derive(Debug)]
pub enum RepositoryError {
    StrictJson(StrictJsonError),
    Json(serde_json::Error),
    UnsupportedFormat(u32),
    InvalidPatch(PatchError),
}

impl fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryError::StrictJson(e) => write!(f, "{e}"),
            RepositoryError::Json(e) => write!(f, "{e}"),
            RepositoryError::UnsupportedFormat(fmt) => {
                write!(f, "unsupported repository format {fmt}: expected 1")
            }
            RepositoryError::InvalidPatch(e) => write!(f, "invalid patch: {e}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<StrictJsonError> for RepositoryError {
    fn from(err: StrictJsonError) -> Self {
        RepositoryError::StrictJson(err)
    }
}

/// Errors detected by the strict JSON scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictJsonError {
    DuplicateKey(String),
    FloatingPointNumberNotAllowed,
    InvalidJson(String),
}

impl fmt::Display for StrictJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StrictJsonError::DuplicateKey(k) => write!(f, "duplicate JSON key '{k}'"),
            StrictJsonError::FloatingPointNumberNotAllowed => {
                write!(f, "floating-point numbers are not permitted in Snap JSON")
            }
            StrictJsonError::InvalidJson(msg) => write!(f, "invalid JSON: {msg}"),
        }
    }
}

impl std::error::Error for StrictJsonError {}

/// Scan JSON bytes strictly to reject duplicate object keys and floating-point numbers.
pub fn validate_json_strict(bytes: &[u8]) -> Result<(), StrictJsonError> {
    let mut i = 0;
    let len = bytes.len();

    let mut object_keys_stack: Vec<HashSet<String>> = Vec::new();
    // Context stack: true for Object, false for Array
    let mut context_stack: Vec<bool> = Vec::new();
    let mut expecting_value = false;

    while i < len {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        match b {
            b'{' => {
                object_keys_stack.push(HashSet::new());
                context_stack.push(true);
                expecting_value = false;
                i += 1;
            }
            b'}' => {
                if object_keys_stack.pop().is_none() || context_stack.pop() != Some(true) {
                    return Err(StrictJsonError::InvalidJson("unmatched '}'".to_string()));
                }
                expecting_value = false;
                i += 1;
            }
            b'[' => {
                context_stack.push(false);
                expecting_value = false;
                i += 1;
            }
            b']' => {
                if context_stack.pop() != Some(false) {
                    return Err(StrictJsonError::InvalidJson("unmatched ']'".to_string()));
                }
                expecting_value = false;
                i += 1;
            }
            b':' => {
                expecting_value = true;
                i += 1;
            }
            b',' => {
                expecting_value = false;
                i += 1;
            }
            b'"' => {
                // Parse string
                i += 1;
                let mut escaped = false;
                let mut string_content = String::new();

                while i < len {
                    let c = bytes[i];
                    if escaped {
                        match c {
                            b'"' => string_content.push('"'),
                            b'\\' => string_content.push('\\'),
                            b'/' => string_content.push('/'),
                            b'b' => string_content.push('\x08'),
                            b'f' => string_content.push('\x0c'),
                            b'n' => string_content.push('\n'),
                            b'r' => string_content.push('\r'),
                            b't' => string_content.push('\t'),
                            b'u' => {
                                // 4 hex digits
                                if i + 4 >= len {
                                    return Err(StrictJsonError::InvalidJson(
                                        "incomplete unicode escape".to_string(),
                                    ));
                                }
                                let hex =
                                    std::str::from_utf8(&bytes[i + 1..=i + 4]).map_err(|_| {
                                        StrictJsonError::InvalidJson("invalid unicode".to_string())
                                    })?;
                                let code = u16::from_str_radix(hex, 16).map_err(|_| {
                                    StrictJsonError::InvalidJson(
                                        "invalid hex in unicode".to_string(),
                                    )
                                })?;
                                if let Some(ch) = char::from_u32(code as u32) {
                                    string_content.push(ch);
                                }
                                i += 4;
                            }
                            _ => string_content.push(c as char),
                        }
                        escaped = false;
                    } else if c == b'\\' {
                        escaped = true;
                    } else if c == b'"' {
                        break;
                    } else {
                        string_content.push(c as char);
                    }
                    i += 1;
                }
                if i >= len {
                    return Err(StrictJsonError::InvalidJson(
                        "unterminated string".to_string(),
                    ));
                }
                i += 1; // consume closing quote

                // Check if this string was an object key
                if let Some(true) = context_stack.last() {
                    if !expecting_value {
                        // It's a key! Check duplicates
                        let current_keys = object_keys_stack.last_mut().unwrap();
                        if current_keys.contains(&string_content) {
                            return Err(StrictJsonError::DuplicateKey(string_content));
                        }
                        current_keys.insert(string_content);
                    }
                }
            }
            b'-' | b'0'..=b'9' => {
                // Number token: inspect if it contains floating point '.' or 'e'/'E'
                while i < len
                    && (bytes[i].is_ascii_digit()
                        || bytes[i] == b'.'
                        || bytes[i] == b'-'
                        || bytes[i] == b'+'
                        || bytes[i] == b'e'
                        || bytes[i] == b'E')
                {
                    if bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'E' {
                        return Err(StrictJsonError::FloatingPointNumberNotAllowed);
                    }
                    i += 1;
                }
                expecting_value = false;
            }
            _ => {
                // true / false / null
                i += 1;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_b1_json_strictness_and_unknown_field_rejection() {
        // Unknown top-level field
        let unknown_json = br#"{
            "format": 1,
            "frontier": [],
            "patches": [],
            "extra": true
        }"#;
        assert!(Repository::from_json_slice(unknown_json).is_err());

        // Floating point number rejection
        let float_json = br#"{
            "format": 1,
            "frontier": [["alice@x", 1.0]],
            "patches": []
        }"#;
        assert_eq!(
            Repository::from_json_slice(float_json)
                .unwrap_err()
                .to_string(),
            "floating-point numbers are not permitted in Snap JSON"
        );

        // Duplicate keys rejection
        let dup_json = br#"{
            "format": 1,
            "format": 1,
            "frontier": [],
            "patches": []
        }"#;
        assert_eq!(
            Repository::from_json_slice(dup_json)
                .unwrap_err()
                .to_string(),
            "duplicate JSON key 'format'"
        );
    }

    #[test]
    fn test_tracked_path_validation() {
        assert!(validate_tracked_path("hello.txt").is_ok());
        assert!(validate_tracked_path("nested/dir/file.rs").is_ok());

        assert_eq!(validate_tracked_path(""), Err(PathError::Empty));
        assert_eq!(
            validate_tracked_path(".snap/config.json"),
            Err(PathError::SnapPrefix)
        );
        assert_eq!(validate_tracked_path("a/./b"), Err(PathError::DotSegment));
        assert_eq!(
            validate_tracked_path("a/../b"),
            Err(PathError::DotDotSegment)
        );
        assert_eq!(validate_tracked_path("a//b"), Err(PathError::EmptySegment));
        assert_eq!(
            validate_tracked_path("a\\b"),
            Err(PathError::ContainsBackslash)
        );
        assert_eq!(
            validate_tracked_path("a/\x01/b"),
            Err(PathError::ContainsControlChar)
        );
    }

    #[test]
    fn test_golden_repository_serialization() {
        let author = ContributorId::parse("a@x").unwrap();
        let p1 = Patch {
            author: author.clone(),
            revision: 1,
            base: Version::empty(),
            message: "old".to_string(),
            changes: vec![Change::Text {
                path: "repeated.txt".to_string(),
                edit: vec![TextEditOp::Insert(vec![
                    "a\n".to_string(),
                    "b\n".to_string(),
                    "a\n".to_string(),
                ])],
            }],
        };
        p1.validate().unwrap();

        let v1 = Version::parse("(a@x->1)").unwrap();
        let p2 = Patch {
            author: author.clone(),
            revision: 2,
            base: v1.clone(),
            message: "new".to_string(),
            changes: vec![
                Change::Text {
                    path: "added.txt".to_string(),
                    edit: vec![TextEditOp::Insert(vec!["new".to_string()])],
                },
                Change::Text {
                    path: "repeated.txt".to_string(),
                    edit: vec![
                        TextEditOp::Delete(1),
                        TextEditOp::Retain(2),
                        TextEditOp::Insert(vec!["a".to_string()]),
                    ],
                },
            ],
        };
        p2.validate().unwrap();

        let v2 = Version::parse("(a@x->2)").unwrap();
        let repo = Repository::new(v2, vec![p1, p2]);

        let json_str = repo.to_json_pretty().unwrap();
        assert!(json_str.ends_with('\n'));

        let parsed = Repository::from_json_slice(json_str.as_bytes()).unwrap();
        assert_eq!(parsed, repo);
    }
}
