use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// Maximum safe integer for revisions, matching JavaScript's Number.MAX_SAFE_INTEGER (2^53 - 1).
pub const MAX_REVISION: u64 = 9007199254740991;

/// Errors that can occur when parsing or validating a contributor ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributorIdError {
    Empty,
    TooLong(usize),
    NotAscii,
    MissingAtSymbol,
    MultipleAtSymbols,
    EmptyLocalPart,
    EmptyDomainPart,
    ContainsControlChar,
    ContainsWhitespace,
    ContainsDisallowedChar(char),
    ContainsArrowSubstring,
}

impl fmt::Display for ContributorIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContributorIdError::Empty => write!(f, "contributor id cannot be empty"),
            ContributorIdError::TooLong(len) => {
                write!(f, "contributor id exceeds 254 bytes (got {len})")
            }
            ContributorIdError::NotAscii => {
                write!(f, "contributor id must contain only ASCII characters")
            }
            ContributorIdError::MissingAtSymbol => {
                write!(f, "contributor id must contain an '@' symbol")
            }
            ContributorIdError::MultipleAtSymbols => {
                write!(f, "contributor id must contain exactly one '@' symbol")
            }
            ContributorIdError::EmptyLocalPart => {
                write!(f, "contributor id must have nonempty text before '@'")
            }
            ContributorIdError::EmptyDomainPart => {
                write!(f, "contributor id must have nonempty text after '@'")
            }
            ContributorIdError::ContainsControlChar => {
                write!(f, "contributor id cannot contain control characters")
            }
            ContributorIdError::ContainsWhitespace => {
                write!(f, "contributor id cannot contain whitespace")
            }
            ContributorIdError::ContainsDisallowedChar(c) => {
                write!(f, "contributor id cannot contain '{c}'")
            }
            ContributorIdError::ContainsArrowSubstring => {
                write!(f, "contributor id cannot contain '->'")
            }
        }
    }
}

impl std::error::Error for ContributorIdError {}

/// A validated contributor identity.
///
/// An ASCII email-shaped string containing exactly one `@` with non-empty text on both sides,
/// no control characters, whitespace, `,`, `(`, `)`, or `->`, at most 254 bytes.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContributorId(String);

impl ContributorId {
    /// Parse and validate a contributor ID string.
    pub fn parse(s: &str) -> Result<Self, ContributorIdError> {
        if s.is_empty() {
            return Err(ContributorIdError::Empty);
        }
        if s.len() > 254 {
            return Err(ContributorIdError::TooLong(s.len()));
        }
        if !s.is_ascii() {
            return Err(ContributorIdError::NotAscii);
        }
        if s.contains("->") {
            return Err(ContributorIdError::ContainsArrowSubstring);
        }

        let mut at_count = 0;
        let mut at_pos = 0;

        for (idx, ch) in s.char_indices() {
            if ch.is_ascii_control() {
                return Err(ContributorIdError::ContainsControlChar);
            }
            if ch.is_ascii_whitespace() {
                return Err(ContributorIdError::ContainsWhitespace);
            }
            if ch == ',' || ch == '(' || ch == ')' {
                return Err(ContributorIdError::ContainsDisallowedChar(ch));
            }
            if ch == '@' {
                at_count += 1;
                at_pos = idx;
            }
        }

        if at_count == 0 {
            return Err(ContributorIdError::MissingAtSymbol);
        }
        if at_count > 1 {
            return Err(ContributorIdError::MultipleAtSymbols);
        }
        if at_pos == 0 {
            return Err(ContributorIdError::EmptyLocalPart);
        }
        if at_pos == s.len() - 1 {
            return Err(ContributorIdError::EmptyDomainPart);
        }

        Ok(ContributorId(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContributorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ContributorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContributorId({})", self.0)
    }
}

impl std::ops::Deref for ContributorId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for ContributorId {
    type Err = ContributorIdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ContributorId::parse(s)
    }
}

impl TryFrom<&str> for ContributorId {
    type Error = ContributorIdError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        ContributorId::parse(s)
    }
}

impl TryFrom<String> for ContributorId {
    type Error = ContributorIdError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        ContributorId::parse(&s)
    }
}

impl serde::Serialize for ContributorId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for ContributorId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ContributorId::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Errors that can occur when parsing or validating a revision number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevisionError {
    Empty,
    InvalidDigit,
    LeadingZero,
    Zero,
    Overflow,
}

impl fmt::Display for RevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RevisionError::Empty => write!(f, "revision cannot be empty"),
            RevisionError::InvalidDigit => write!(f, "revision contains non-digit characters"),
            RevisionError::LeadingZero => write!(f, "revision cannot have leading zeroes"),
            RevisionError::Zero => {
                write!(f, "revision must be a positive integer greater than zero")
            }
            RevisionError::Overflow => {
                write!(f, "revision exceeds maximum safe integer ({MAX_REVISION})")
            }
        }
    }
}

impl std::error::Error for RevisionError {}

/// Parse and validate a revision string.
///
/// Positive integer no greater than 9007199254740991, with no leading zeroes.
pub fn parse_revision(s: &str) -> Result<u64, RevisionError> {
    if s.is_empty() {
        return Err(RevisionError::Empty);
    }
    if s.starts_with('0') {
        if s == "0" {
            return Err(RevisionError::Zero);
        } else {
            return Err(RevisionError::LeadingZero);
        }
    }
    if !s.chars().all(|c| c.is_ascii_digit()) {
        return Err(RevisionError::InvalidDigit);
    }

    let val: u64 = s.parse().map_err(|_| RevisionError::Overflow)?;
    if val > MAX_REVISION {
        return Err(RevisionError::Overflow);
    }
    if val == 0 {
        return Err(RevisionError::Zero);
    }
    Ok(val)
}

/// Errors that can occur when parsing or validating a version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    MissingParentheses,
    Whitespace,
    InvalidEntryFormat(String),
    InvalidContributor(ContributorIdError),
    InvalidRevision(RevisionError),
    DuplicateContributor(String),
    NoncanonicalOrdering { previous: String, current: String },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::MissingParentheses => {
                write!(f, "version must start with '(' and end with ')'")
            }
            VersionError::Whitespace => write!(f, "version cannot contain whitespace"),
            VersionError::InvalidEntryFormat(entry) => {
                write!(f, "invalid entry format '{entry}': expected '<id>-><rev>'")
            }
            VersionError::InvalidContributor(err) => {
                write!(f, "invalid contributor id: {err}")
            }
            VersionError::InvalidRevision(err) => {
                write!(f, "invalid revision: {err}")
            }
            VersionError::DuplicateContributor(id) => {
                write!(f, "duplicate contributor id: {id}")
            }
            VersionError::NoncanonicalOrdering { previous, current } => {
                write!(
                    f,
                    "noncanonical ordering: '{current}' must appear after '{previous}'"
                )
            }
        }
    }
}

impl std::error::Error for VersionError {}

/// Four-way outcome of comparing two causal vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausalRelation {
    /// V == W
    Equal,
    /// V < W (V is strictly dominated by W)
    Before,
    /// V > W (V strictly dominates W)
    After,
    /// V || W (Neither dominates the other)
    Concurrent,
}

/// A version is a vector clock: an ordered map from ContributorId to positive revision number.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Version {
    entries: BTreeMap<ContributorId, u64>,
}

impl Version {
    /// Construct the empty version `()`.
    pub fn empty() -> Self {
        Version {
            entries: BTreeMap::new(),
        }
    }

    /// Whether this is the empty version `()`.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of active contributor counters.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Get the revision counter for a contributor, or 0 if absent.
    pub fn get(&self, contributor: &ContributorId) -> u64 {
        self.entries.get(contributor).copied().unwrap_or(0)
    }

    /// Iterator over (contributor, revision) pairs sorted by contributor ID.
    pub fn iter(&self) -> impl Iterator<Item = (&ContributorId, &u64)> {
        self.entries.iter()
    }

    /// Construct a version from an iterator of entries, validating positive revisions and bounds.
    pub fn from_entries<I>(entries: I) -> Result<Self, VersionError>
    where
        I: IntoIterator<Item = (ContributorId, u64)>,
    {
        let mut map = BTreeMap::new();
        for (id, rev) in entries {
            if rev == 0 {
                return Err(VersionError::InvalidRevision(RevisionError::Zero));
            }
            if rev > MAX_REVISION {
                return Err(VersionError::InvalidRevision(RevisionError::Overflow));
            }
            map.insert(id, rev);
        }
        Ok(Version { entries: map })
    }

    /// Parse a version from its canonical string syntax `()` or `(id1->rev1,id2->rev2)`.
    pub fn parse(s: &str) -> Result<Self, VersionError> {
        if !s.starts_with('(') || !s.ends_with(')') {
            return Err(VersionError::MissingParentheses);
        }
        if s.contains(char::is_whitespace) {
            return Err(VersionError::Whitespace);
        }

        let inner = &s[1..s.len() - 1];
        if inner.is_empty() {
            return Ok(Version::empty());
        }

        let mut entries = BTreeMap::new();
        let mut prev_id: Option<String> = None;

        for part in inner.split(',') {
            let Some((id_str, rev_str)) = part.split_once("->") else {
                return Err(VersionError::InvalidEntryFormat(part.to_string()));
            };
            if rev_str.contains("->") {
                return Err(VersionError::InvalidEntryFormat(part.to_string()));
            }

            let id = ContributorId::parse(id_str).map_err(VersionError::InvalidContributor)?;
            let rev = parse_revision(rev_str).map_err(VersionError::InvalidRevision)?;

            if let Some(prev) = &prev_id {
                match id.as_str().cmp(prev.as_str()) {
                    Ordering::Equal => {
                        return Err(VersionError::DuplicateContributor(id.to_string()));
                    }
                    Ordering::Less => {
                        return Err(VersionError::NoncanonicalOrdering {
                            previous: prev.clone(),
                            current: id.to_string(),
                        });
                    }
                    Ordering::Greater => {}
                }
            }

            prev_id = Some(id.to_string());
            entries.insert(id, rev);
        }

        Ok(Version { entries })
    }

    /// Perform a 4-way causal comparison between this version and another.
    pub fn causal_cmp(&self, other: &Self) -> CausalRelation {
        if self == other {
            return CausalRelation::Equal;
        }

        let mut has_less = false;
        let mut has_greater = false;

        // Iterate over union of contributors
        let mut self_iter = self.entries.iter().peekable();
        let mut other_iter = other.entries.iter().peekable();

        while self_iter.peek().is_some() || other_iter.peek().is_some() {
            let (v_self, v_other) = match (self_iter.peek(), other_iter.peek()) {
                (Some((k1, _)), Some((k2, _))) => match k1.cmp(k2) {
                    Ordering::Equal => {
                        let (_, r1) = self_iter.next().unwrap();
                        let (_, r2) = other_iter.next().unwrap();
                        (*r1, *r2)
                    }
                    Ordering::Less => {
                        let (_, r1) = self_iter.next().unwrap();
                        (*r1, 0)
                    }
                    Ordering::Greater => {
                        let (_, r2) = other_iter.next().unwrap();
                        (0, *r2)
                    }
                },
                (Some(_), None) => {
                    let (_, r1) = self_iter.next().unwrap();
                    (*r1, 0)
                }
                (None, Some(_)) => {
                    let (_, r2) = other_iter.next().unwrap();
                    (0, *r2)
                }
                (None, None) => unreachable!(),
            };

            if v_self < v_other {
                has_less = true;
            } else if v_self > v_other {
                has_greater = true;
            }

            if has_less && has_greater {
                return CausalRelation::Concurrent;
            }
        }

        if has_less && !has_greater {
            CausalRelation::Before
        } else if has_greater && !has_less {
            CausalRelation::After
        } else if !has_less && !has_greater {
            CausalRelation::Equal
        } else {
            CausalRelation::Concurrent
        }
    }

    /// Whether this version is causally strictly before `other`.
    pub fn is_before(&self, other: &Self) -> bool {
        self.causal_cmp(other) == CausalRelation::Before
    }

    /// Whether this version is causally strictly after `other`.
    pub fn is_after(&self, other: &Self) -> bool {
        self.causal_cmp(other) == CausalRelation::After
    }

    /// Whether this version is causally concurrent with `other`.
    pub fn is_concurrent(&self, other: &Self) -> bool {
        self.causal_cmp(other) == CausalRelation::Concurrent
    }

    /// Element-wise join (maximum) of two vector clocks.
    pub fn join(&self, other: &Self) -> Self {
        let mut entries = BTreeMap::new();

        let mut self_iter = self.entries.iter().peekable();
        let mut other_iter = other.entries.iter().peekable();

        while self_iter.peek().is_some() || other_iter.peek().is_some() {
            let (key, max_rev) = match (self_iter.peek(), other_iter.peek()) {
                (Some((k1, _)), Some((k2, _))) => match k1.cmp(k2) {
                    Ordering::Equal => {
                        let (k, r1) = self_iter.next().unwrap();
                        let (_, r2) = other_iter.next().unwrap();
                        (k.clone(), std::cmp::max(*r1, *r2))
                    }
                    Ordering::Less => {
                        let (k, r1) = self_iter.next().unwrap();
                        (k.clone(), *r1)
                    }
                    Ordering::Greater => {
                        let (k, r2) = other_iter.next().unwrap();
                        (k.clone(), *r2)
                    }
                },
                (Some(_), None) => {
                    let (k, r1) = self_iter.next().unwrap();
                    (k.clone(), *r1)
                }
                (None, Some(_)) => {
                    let (k, r2) = other_iter.next().unwrap();
                    (k.clone(), *r2)
                }
                (None, None) => unreachable!(),
            };

            if max_rev > 0 {
                entries.insert(key, max_rev);
            }
        }

        Version { entries }
    }

    /// Compare two versions using Snap total ordering (§3.4).
    ///
    /// Evaluates the sorted union of contributor IDs and lexicographically compares
    /// the counter at each ID. The first unequal counter decides.
    pub fn cmp_snap_order(&self, other: &Self) -> Ordering {
        let mut self_iter = self.entries.iter().peekable();
        let mut other_iter = other.entries.iter().peekable();

        while self_iter.peek().is_some() || other_iter.peek().is_some() {
            let (v_self, v_other) = match (self_iter.peek(), other_iter.peek()) {
                (Some((k1, _)), Some((k2, _))) => match k1.cmp(k2) {
                    Ordering::Equal => {
                        let (_, r1) = self_iter.next().unwrap();
                        let (_, r2) = other_iter.next().unwrap();
                        (*r1, *r2)
                    }
                    Ordering::Less => {
                        let (_, r1) = self_iter.next().unwrap();
                        (*r1, 0)
                    }
                    Ordering::Greater => {
                        let (_, r2) = other_iter.next().unwrap();
                        (0, *r2)
                    }
                },
                (Some(_), None) => {
                    let (_, r1) = self_iter.next().unwrap();
                    (*r1, 0)
                }
                (None, Some(_)) => {
                    let (_, r2) = other_iter.next().unwrap();
                    (0, *r2)
                }
                (None, None) => unreachable!(),
            };

            match v_self.cmp(&v_other) {
                Ordering::Equal => continue,
                ord => return ord,
            }
        }

        Ordering::Equal
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, (id, rev)) in self.entries.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{id}->{rev}")?;
        }
        write!(f, ")")
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Version({})", self)
    }
}

impl FromStr for Version {
    type Err = VersionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Version::parse(s)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_snap_order(other)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl serde::Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.entries.len()))?;
        for (id, rev) in &self.entries {
            seq.serialize_element(&(id, *rev))?;
        }
        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw_entries = Vec::<(String, u64)>::deserialize(deserializer)?;
        let mut entries = BTreeMap::new();
        let mut prev_id: Option<String> = None;

        for (id_str, rev) in raw_entries {
            if rev == 0 {
                return Err(serde::de::Error::custom(
                    "revision must be greater than zero",
                ));
            }
            if rev > MAX_REVISION {
                return Err(serde::de::Error::custom(format!(
                    "revision exceeds maximum safe integer ({MAX_REVISION})"
                )));
            }
            let id = ContributorId::parse(&id_str).map_err(serde::de::Error::custom)?;

            if let Some(prev) = &prev_id {
                match id.as_str().cmp(prev.as_str()) {
                    Ordering::Equal => {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate contributor id: {id}"
                        )));
                    }
                    Ordering::Less => {
                        return Err(serde::de::Error::custom(format!(
                            "noncanonical contributor ordering: '{id}' must appear after '{prev}'"
                        )));
                    }
                    Ordering::Greater => {}
                }
            }
            prev_id = Some(id.to_string());
            entries.insert(id, rev);
        }

        Ok(Version { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_a1_contributor_id_syntax_and_validation() {
        // Valid candidates
        let valid = [
            "alice@example.com",
            "a@b",
            "user+tag@domain.co",
            &format!("{}@{}", "a".repeat(126), "b".repeat(127)), // 126 + 1 + 127 = 254 bytes
        ];
        for candidate in valid {
            let res = ContributorId::parse(candidate);
            assert!(
                res.is_ok(),
                "Expected valid candidate '{candidate}' to parse, got {:?}",
                res.err()
            );
            assert_eq!(res.unwrap().as_str(), candidate);
        }

        // Invalid candidates
        let invalid = [
            "",
            "@",
            "@domain.com",
            "user@",
            "a@@b",
            "user@dom ain",
            "user@dom\x00ain",
            "user@dom,ain",
            "user@(x)@dom",
            "a->b@c",
            "a@b->c",
            &format!("{}@{}", "a".repeat(127), "b".repeat(127)), // 127 + 1 + 127 = 255 bytes (exceeds 254)
            "user\x1f@domain.com",                               // control char
            "user@domain\x7f.com",                               // DEL control char
            "user\t@domain.com",                                 // tab
            "user\n@domain.com",                                 // newline
            "user\r@domain.com",                                 // carriage return
            "user(name)@domain.com",                             // parens
            "user,name@domain.com",                              // comma
            "üser@domain.com",                                   // non-ASCII
        ];
        for candidate in invalid {
            assert!(
                ContributorId::parse(candidate).is_err(),
                "Expected invalid candidate '{candidate}' to fail"
            );
        }
    }

    #[test]
    fn test_scenario_a2_version_string_canonical_parser_and_formatter() {
        // Valid inputs
        let valid = [
            "()",
            "(alice@x->1)",
            "(alice@x->1,bob@y->2)",
            "(a@x->1,b@y->2,c@z->9007199254740991)",
        ];
        for input in valid {
            let v =
                Version::parse(input).unwrap_or_else(|e| panic!("Failed to parse '{input}': {e}"));
            assert_eq!(v.to_string(), input);
        }

        // Invalid inputs
        let invalid = [
            "( )",
            "(alice@x->0)",
            "(alice@x->01)",
            "(bob@y->2,alice@x->1)",       // unsorted
            "(alice@x->1,alice@x->2)",     // duplicate
            "(alice@x->9007199254740992)", // overflow (MAX_SAFE_INTEGER + 1)
            "alice@x->1",                  // missing parens
            "(alice@x->-1)",               // negative
            "(alice@x->1, bob@y->2)",      // space after comma
            "(alice@x->1 ,bob@y->2)",      // space before comma
            "(alice@x->)",                 // missing revision
            "(->1)",                       // missing contributor
            "(alice@x)",                   // missing arrow and revision
            "()extra",                     // extra chars
        ];
        for input in invalid {
            assert!(
                Version::parse(input).is_err(),
                "Expected input '{input}' to be rejected as invalid version"
            );
        }
    }

    #[test]
    fn test_scenario_a3_four_way_causal_comparison_matrix() {
        let v0 = Version::parse("()").unwrap();
        let v1 = Version::parse("(alice@x->1)").unwrap();
        let v2 = Version::parse("(alice@x->2)").unwrap();
        let v3 = Version::parse("(alice@x->1,bob@y->1)").unwrap();
        let v4 = Version::parse("(bob@y->2)").unwrap();

        // Identity
        assert_eq!(v0.causal_cmp(&v0), CausalRelation::Equal);
        assert_eq!(v1.causal_cmp(&v1), CausalRelation::Equal);
        assert_eq!(v2.causal_cmp(&v2), CausalRelation::Equal);
        assert_eq!(v3.causal_cmp(&v3), CausalRelation::Equal);
        assert_eq!(v4.causal_cmp(&v4), CausalRelation::Equal);

        // Before / After
        assert_eq!(v0.causal_cmp(&v1), CausalRelation::Before);
        assert_eq!(v1.causal_cmp(&v0), CausalRelation::After);

        assert_eq!(v1.causal_cmp(&v2), CausalRelation::Before);
        assert_eq!(v2.causal_cmp(&v1), CausalRelation::After);

        assert_eq!(v1.causal_cmp(&v3), CausalRelation::Before);
        assert_eq!(v3.causal_cmp(&v1), CausalRelation::After);

        // Concurrent: V2 vs V3
        // V2 has alice->2, bob->0
        // V3 has alice->1, bob->1
        assert_eq!(v2.causal_cmp(&v3), CausalRelation::Concurrent);
        assert_eq!(v3.causal_cmp(&v2), CausalRelation::Concurrent);

        // Concurrent: V2 vs V4
        // V2 has alice->2, bob->0
        // V4 has alice->0, bob->2
        assert_eq!(v2.causal_cmp(&v4), CausalRelation::Concurrent);
        assert_eq!(v4.causal_cmp(&v2), CausalRelation::Concurrent);

        // Helper methods
        assert!(v0.is_before(&v1));
        assert!(v1.is_after(&v0));
        assert!(v2.is_concurrent(&v3));
        assert!(!v2.is_before(&v3));
        assert!(!v2.is_after(&v3));
    }

    #[test]
    fn test_scenario_a4_snap_total_order_resolution() {
        let va = Version::parse("(alice@x->2,bob@y->1)").unwrap();
        let vb = Version::parse("(alice@x->1,bob@y->3)").unwrap();
        let vc = Version::parse("(carol@x->1)").unwrap();

        // Between VA and VB:
        // First sorted union key is alice@x: VA has 2, VB has 1 -> VB < VA
        assert_eq!(vb.cmp_snap_order(&va), Ordering::Less);
        assert_eq!(va.cmp_snap_order(&vb), Ordering::Greater);
        assert!(vb < va);

        // Between VA and VC:
        // First sorted union key is alice@x: VA has 2, VC has 0 -> VC < VA
        assert_eq!(vc.cmp_snap_order(&va), Ordering::Less);
        assert_eq!(va.cmp_snap_order(&vc), Ordering::Greater);
        assert!(vc < va);

        // Between VB and VC:
        // First sorted union key is alice@x: VB has 1, VC has 0 -> VC < VB
        assert_eq!(vc.cmp_snap_order(&vb), Ordering::Less);
        assert_eq!(vb.cmp_snap_order(&vc), Ordering::Greater);
        assert!(vc < vb);

        // Sorting: [VC, VB, VA]
        let mut list = vec![va.clone(), vc.clone(), vb.clone()];
        list.sort();
        assert_eq!(list, vec![vc, vb, va]);
    }

    #[test]
    fn test_vector_clock_join() {
        let v0 = Version::parse("()").unwrap();
        let v1 = Version::parse("(alice@x->1)").unwrap();
        let v2 = Version::parse("(alice@x->2,bob@y->1)").unwrap();
        let v3 = Version::parse("(alice@x->1,bob@y->3)").unwrap();

        // Join with empty is identity
        assert_eq!(v0.join(&v1), v1);
        assert_eq!(v1.join(&v0), v1);

        // Idempotence
        assert_eq!(v2.join(&v2), v2);

        // Commutativity
        assert_eq!(v2.join(&v3), v3.join(&v2));

        // Value: max(alice: 2, 1) = 2, max(bob: 1, 3) = 3
        let expected = Version::parse("(alice@x->2,bob@y->3)").unwrap();
        assert_eq!(v2.join(&v3), expected);

        // Join produces a causal upper bound
        assert!(v2.causal_cmp(&expected) == CausalRelation::Before);
        assert!(v3.causal_cmp(&expected) == CausalRelation::Before);
    }

    #[test]
    fn test_json_serde_roundtrip() {
        let v = Version::parse("(alice@x->1,bob@y->2)").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, r#"[["alice@x",1],["bob@y",2]]"#);

        let deserialized: Version = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, v);

        // Empty version
        let v_empty = Version::empty();
        let empty_json = serde_json::to_string(&v_empty).unwrap();
        assert_eq!(empty_json, "[]");
        let deserialized_empty: Version = serde_json::from_str(&empty_json).unwrap();
        assert_eq!(deserialized_empty, v_empty);

        // Rejection of invalid JSON
        // Unsorted
        assert!(serde_json::from_str::<Version>(r#"[["bob@y",2],["alice@x",1]]"#).is_err());
        // Duplicate
        assert!(serde_json::from_str::<Version>(r#"[["alice@x",1],["alice@x",2]]"#).is_err());
        // Zero revision
        assert!(serde_json::from_str::<Version>(r#"[["alice@x",0]]"#).is_err());
        // Overflow revision
        assert!(serde_json::from_str::<Version>(r#"[["alice@x",9007199254740992]]"#).is_err());
        // Invalid contributor
        assert!(serde_json::from_str::<Version>(r#"[["not-an-id",1]]"#).is_err());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_contributor_id() -> impl Strategy<Value = ContributorId> {
        // Generate valid email-shaped contributor IDs
        ("[a-z0-9_]{1,5}@[a-z0-9]{1,5}\\.com").prop_map(|s| ContributorId::parse(&s).unwrap())
    }

    fn arb_version() -> impl Strategy<Value = Version> {
        proptest::collection::btree_map(arb_contributor_id(), 1u64..=1000u64, 0..=5)
            .prop_map(|entries| Version { entries })
    }

    proptest! {
        #[test]
        fn prop_version_canonical_string_roundtrip(v in arb_version()) {
            let s = v.to_string();
            let parsed = Version::parse(&s).expect("valid string must parse");
            prop_assert_eq!(v, parsed);
        }

        #[test]
        fn prop_causal_cmp_antisymmetry(v1 in arb_version(), v2 in arb_version()) {
            let r1 = v1.causal_cmp(&v2);
            let r2 = v2.causal_cmp(&v1);
            match r1 {
                CausalRelation::Equal => prop_assert_eq!(r2, CausalRelation::Equal),
                CausalRelation::Before => prop_assert_eq!(r2, CausalRelation::After),
                CausalRelation::After => prop_assert_eq!(r2, CausalRelation::Before),
                CausalRelation::Concurrent => prop_assert_eq!(r2, CausalRelation::Concurrent),
            }
        }

        #[test]
        fn prop_snap_order_is_total(v1 in arb_version(), v2 in arb_version()) {
            let ord1 = v1.cmp_snap_order(&v2);
            let ord2 = v2.cmp_snap_order(&v1);
            prop_assert_eq!(ord1, ord2.reverse());
            if ord1 == Ordering::Equal {
                prop_assert_eq!(v1, v2);
            }
        }

        #[test]
        fn prop_snap_order_extends_causal_order(v1 in arb_version(), v2 in arb_version()) {
            if v1.causal_cmp(&v2) == CausalRelation::Before {
                prop_assert_eq!(v1.cmp_snap_order(&v2), Ordering::Less);
            }
            if v1.causal_cmp(&v2) == CausalRelation::After {
                prop_assert_eq!(v1.cmp_snap_order(&v2), Ordering::Greater);
            }
        }

        #[test]
        fn prop_join_properties(v1 in arb_version(), v2 in arb_version(), v3 in arb_version()) {
            // Idempotence: v1.join(v1) == v1
            prop_assert_eq!(v1.join(&v1), v1.clone());

            // Commutativity: v1.join(v2) == v2.join(v1)
            prop_assert_eq!(v1.join(&v2), v2.join(&v1));

            // Associativity: v1.join(v2).join(v3) == v1.join(v2.join(v3))
            prop_assert_eq!(v1.join(&v2).join(&v3), v1.join(&v2.join(&v3)));

            // Upper bound: v1 <= join(v1, v2)
            let joined = v1.join(&v2);
            let rel1 = v1.causal_cmp(&joined);
            prop_assert!(rel1 == CausalRelation::Before || rel1 == CausalRelation::Equal);
        }
    }
}
