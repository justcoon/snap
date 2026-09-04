use std::fmt;

use crate::core::patch::TextEditOp;

/// Errors that can occur during text tokenization or diff application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    ContainsNulByte,
    NotUtf8,
    EditDoesNotConsumeAllTokens { expected: usize, consumed: usize },
    RetainExceedsOldTokens { available: usize, requested: usize },
    DeleteExceedsOldTokens { available: usize, requested: usize },
    EmptyInsert,
    NonCanonicalResultToken(String),
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::ContainsNulByte => write!(f, "file contains NUL byte (binary file)"),
            DiffError::NotUtf8 => write!(f, "file is not valid UTF-8 (binary file)"),
            DiffError::EditDoesNotConsumeAllTokens { expected, consumed } => {
                write!(
                    f,
                    "edit script does not consume all tokens: expected {expected}, consumed {consumed}"
                )
            }
            DiffError::RetainExceedsOldTokens {
                available,
                requested,
            } => {
                write!(
                    f,
                    "retain exceeds available tokens: available {available}, requested {requested}"
                )
            }
            DiffError::DeleteExceedsOldTokens {
                available,
                requested,
            } => {
                write!(
                    f,
                    "delete exceeds available tokens: available {available}, requested {requested}"
                )
            }
            DiffError::EmptyInsert => write!(f, "insert operation cannot be empty"),
            DiffError::NonCanonicalResultToken(t) => {
                write!(
                    f,
                    "non-canonical result token '{t}': newline invariant violated"
                )
            }
        }
    }
}

impl std::error::Error for DiffError {}

/// Check if byte content qualifies as text according to SPEC §4.4:
/// valid UTF-8 and contains no NUL bytes.
pub fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// Tokenize text bytes according to SPEC §4.4:
/// Valid UTF-8, no NUL bytes, split immediately after every LF byte retaining LF in the token.
/// Empty file produces zero tokens.
pub fn tokenize_text(bytes: &[u8]) -> Result<Vec<String>, DiffError> {
    if bytes.contains(&0) {
        return Err(DiffError::ContainsNulByte);
    }
    let s = std::str::from_utf8(bytes).map_err(|_| DiffError::NotUtf8)?;

    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut tokens = Vec::new();
    let mut start = 0;

    for (idx, ch) in s.char_indices() {
        if ch == '\n' {
            tokens.push(s[start..=idx].to_string());
            start = idx + 1;
        }
    }

    if start < s.len() {
        tokens.push(s[start..].to_string());
    }

    Ok(tokens)
}

/// Helper operation during dynamic programming diff traversal before coalescing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RawOp {
    Retain(u64),
    Delete(u64),
    Insert(String),
}

/// Compute the canonical text diff between `old_tokens` and `new_tokens` using
/// the exact recurrence relation D(i, j) and deletion-on-tie rule specified in SPEC §5.
pub fn diff_tokens(old_tokens: &[String], new_tokens: &[String]) -> Vec<TextEditOp> {
    let n = old_tokens.len();
    let m = new_tokens.len();

    // D(i, j) table of size (n + 1) x (m + 1)
    let mut d = vec![vec![0usize; m + 1]; n + 1];

    for (i, row) in d.iter_mut().enumerate().take(n + 1) {
        row[m] = n - i;
    }
    for (j, val) in d[n].iter_mut().enumerate().take(m + 1) {
        *val = m - j;
    }

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            if old_tokens[i] == new_tokens[j] {
                d[i][j] = d[i + 1][j + 1];
            } else {
                d[i][j] = 1 + std::cmp::min(d[i + 1][j], d[i][j + 1]);
            }
        }
    }

    // Walk from (0, 0)
    let mut raw_ops = Vec::new();
    let mut i = 0;
    let mut j = 0;

    while i < n && j < m {
        if old_tokens[i] == new_tokens[j] {
            raw_ops.push(RawOp::Retain(1));
            i += 1;
            j += 1;
        } else if d[i + 1][j] <= d[i][j + 1] {
            // Deletion on tie (D(i + 1, j) <= D(i, j + 1))
            raw_ops.push(RawOp::Delete(1));
            i += 1;
        } else {
            raw_ops.push(RawOp::Insert(new_tokens[j].clone()));
            j += 1;
        }
    }

    if i < n {
        raw_ops.push(RawOp::Delete((n - i) as u64));
    }
    if j < m {
        for token in &new_tokens[j..m] {
            raw_ops.push(RawOp::Insert(token.clone()));
        }
    }

    // Coalesce adjacent operations of the same kind
    let mut coalesced: Vec<TextEditOp> = Vec::new();

    for op in raw_ops {
        match (coalesced.last_mut(), op) {
            (Some(TextEditOp::Retain(prev)), RawOp::Retain(count)) => {
                *prev += count;
            }
            (Some(TextEditOp::Delete(prev)), RawOp::Delete(count)) => {
                *prev += count;
            }
            (Some(TextEditOp::Insert(tokens)), RawOp::Insert(token)) => {
                tokens.push(token);
            }
            (_, RawOp::Retain(count)) => {
                coalesced.push(TextEditOp::Retain(count));
            }
            (_, RawOp::Delete(count)) => {
                coalesced.push(TextEditOp::Delete(count));
            }
            (_, RawOp::Insert(token)) => {
                coalesced.push(TextEditOp::Insert(vec![token]));
            }
        }
    }

    coalesced
}

/// Apply an edit script to `old_tokens` according to SPEC §4.4.
/// The script MUST consume the complete old token sequence.
pub fn apply_edit(old_tokens: &[String], script: &[TextEditOp]) -> Result<Vec<String>, DiffError> {
    let mut cursor = 0;
    let mut result = Vec::new();

    for op in script {
        match op {
            TextEditOp::Retain(n) => {
                let count = *n as usize;
                if cursor + count > old_tokens.len() {
                    return Err(DiffError::RetainExceedsOldTokens {
                        available: old_tokens.len() - cursor,
                        requested: count,
                    });
                }
                result.extend_from_slice(&old_tokens[cursor..cursor + count]);
                cursor += count;
            }
            TextEditOp::Delete(n) => {
                let count = *n as usize;
                if cursor + count > old_tokens.len() {
                    return Err(DiffError::DeleteExceedsOldTokens {
                        available: old_tokens.len() - cursor,
                        requested: count,
                    });
                }
                cursor += count;
            }
            TextEditOp::Insert(tokens) => {
                if tokens.is_empty() {
                    return Err(DiffError::EmptyInsert);
                }
                result.extend(tokens.iter().cloned());
            }
        }
    }

    if cursor != old_tokens.len() {
        return Err(DiffError::EditDoesNotConsumeAllTokens {
            expected: old_tokens.len(),
            consumed: cursor,
        });
    }

    // Validate canonical token sequence invariants:
    // every token except possibly the final one ends in LF,
    // and no token contains LF before its final byte.
    for (idx, token) in result.iter().enumerate() {
        if token.is_empty() {
            return Err(DiffError::NonCanonicalResultToken(token.clone()));
        }
        let bytes = token.as_bytes();
        let last_idx = bytes.len() - 1;

        // Check LF not before final byte
        if bytes[..last_idx].contains(&b'\n') {
            return Err(DiffError::NonCanonicalResultToken(token.clone()));
        }

        // Check all except possibly the last end with LF
        if idx < result.len() - 1 && bytes[last_idx] != b'\n' {
            return Err(DiffError::NonCanonicalResultToken(token.clone()));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_c1_token_splitter_boundary_behavior() {
        // File 1: Empty byte slice
        let t1 = tokenize_text(b"").unwrap();
        assert_eq!(t1, Vec::<String>::new());

        // File 2: "line1\nline2\n"
        let t2 = tokenize_text(b"line1\nline2\n").unwrap();
        assert_eq!(t2, vec!["line1\n", "line2\n"]);

        // File 3: "line1\r\nline2\r\n"
        let t3 = tokenize_text(b"line1\r\nline2\r\n").unwrap();
        assert_eq!(t3, vec!["line1\r\n", "line2\r\n"]);

        // File 4: "line1\nunterminated"
        let t4 = tokenize_text(b"line1\nunterminated").unwrap();
        assert_eq!(t4, vec!["line1\n", "unterminated"]);

        // File 5: "binary\x00data"
        assert_eq!(
            tokenize_text(b"binary\x00data"),
            Err(DiffError::ContainsNulByte)
        );
        assert!(!is_text(b"binary\x00data"));

        // Non UTF-8 binary
        assert_eq!(
            tokenize_text(b"non-utf8 \xff\xfe data"),
            Err(DiffError::NotUtf8)
        );
        assert!(!is_text(b"non-utf8 \xff\xfe data"));
    }

    #[test]
    fn test_scenario_c2_diff_recurrence_and_deletion_on_tie() {
        // Base tokens: ["A\n", "B\n"]
        // Target tokens: ["C\n", "B\n"]
        let base = vec!["A\n".to_string(), "B\n".to_string()];
        let target = vec!["C\n".to_string(), "B\n".to_string()];
        let script = diff_tokens(&base, &target);

        // Expect: delete 1 ("A\n"), insert ["C\n"], retain 1 ("B\n")
        assert_eq!(
            script,
            vec![
                TextEditOp::Delete(1),
                TextEditOp::Insert(vec!["C\n".to_string()]),
                TextEditOp::Retain(1),
            ]
        );
        let applied = apply_edit(&base, &script).unwrap();
        assert_eq!(applied, target);

        // Ambiguous tie case: transforming ["X\n"] to ["Y\n"]
        // D(1, 0) == D(0, 1) == 1
        // Deletion-on-tie requires delete 1 before insert ["Y\n"]
        let tie_base = vec!["X\n".to_string()];
        let tie_target = vec!["Y\n".to_string()];
        let tie_script = diff_tokens(&tie_base, &tie_target);
        assert_eq!(
            tie_script,
            vec![
                TextEditOp::Delete(1),
                TextEditOp::Insert(vec!["Y\n".to_string()]),
            ]
        );
        let tie_applied = apply_edit(&tie_base, &tie_script).unwrap();
        assert_eq!(tie_applied, tie_target);

        // Golden from tests/05-diff-goldens.yaml:
        // Base: "a\nb\na\n"
        // Target: "b\na\na"
        let g_base = tokenize_text(b"a\nb\na\n").unwrap();
        let g_target = tokenize_text(b"b\na\na").unwrap();
        let g_script = diff_tokens(&g_base, &g_target);
        assert_eq!(
            g_script,
            vec![
                TextEditOp::Delete(1),
                TextEditOp::Retain(2),
                TextEditOp::Insert(vec!["a".to_string()]),
            ]
        );
        let g_applied = apply_edit(&g_base, &g_script).unwrap();
        assert_eq!(g_applied, g_target);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_tokens() -> impl Strategy<Value = Vec<String>> {
        proptest::collection::vec(
            prop_oneof![
                Just("line1\n".to_string()),
                Just("line2\n".to_string()),
                Just("line3\n".to_string()),
                Just("line4\n".to_string()),
                Just("repeated\n".to_string()),
                Just("last_no_newline".to_string()),
            ],
            0..=10,
        )
        .prop_map(|mut tokens| {
            // Ensure LF invariant: only the last token may omit LF
            for i in 0..tokens.len() {
                if i + 1 < tokens.len() && !tokens[i].ends_with('\n') {
                    tokens[i].push('\n');
                }
            }
            tokens
        })
    }

    proptest! {
        #[test]
        fn prop_diff_apply_roundtrip(a in arb_tokens(), b in arb_tokens()) {
            let script = diff_tokens(&a, &b);

            // 1. Script applied to a produces b
            let reconstructed = apply_edit(&a, &script).expect("applying valid diff must succeed");
            prop_assert_eq!(&reconstructed, &b);

            // 2. Invariant: no adjacent operations of the same kind
            for i in 1..script.len() {
                let prev = &script[i - 1];
                let curr = &script[i];
                let same_kind = matches!(
                    (prev, curr),
                    (TextEditOp::Retain(_), TextEditOp::Retain(_))
                        | (TextEditOp::Delete(_), TextEditOp::Delete(_))
                        | (TextEditOp::Insert(_), TextEditOp::Insert(_))
                );
                prop_assert!(!same_kind, "Adjacent same kind operations found in script: {:?}", script);
            }

            // 3. Invariant: sum of retain + delete counts equals a.len()
            let mut consumed_old = 0u64;
            for op in &script {
                match op {
                    TextEditOp::Retain(n) => consumed_old += n,
                    TextEditOp::Delete(n) => consumed_old += n,
                    TextEditOp::Insert(tokens) => {
                        prop_assert!(!tokens.is_empty(), "Insert operation cannot be empty");
                    }
                }
            }
            prop_assert_eq!(consumed_old, a.len() as u64);
        }
    }
}
