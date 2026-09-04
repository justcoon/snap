use std::collections::{BTreeMap, BTreeSet};

use crate::core::diff::{diff_tokens, is_text, tokenize_text, DiffError};
use crate::core::patch::TextEditOp;

/// Format the unified diff between an old file tree and a new file tree according to SPEC §5 and §7.6.
pub fn format_tree_diff(
    old_tree: &BTreeMap<String, Vec<u8>>,
    new_tree: &BTreeMap<String, Vec<u8>>,
) -> Result<String, DiffError> {
    let mut all_paths: BTreeSet<&str> = BTreeSet::new();
    for p in old_tree.keys() {
        all_paths.insert(p.as_str());
    }
    for p in new_tree.keys() {
        all_paths.insert(p.as_str());
    }

    let mut output = String::new();

    for path in all_paths {
        let old_opt = old_tree.get(path);
        let new_opt = new_tree.get(path);

        // Skip identical paths
        if old_opt == new_opt {
            continue;
        }

        let old_is_binary = old_opt.is_some_and(|b| !is_text(b));
        let new_is_binary = new_opt.is_some_and(|b| !is_text(b));

        if old_is_binary || new_is_binary {
            let old_label = if old_opt.is_some() {
                format!("a/{path}")
            } else {
                "/dev/null".to_string()
            };
            let new_label = if new_opt.is_some() {
                format!("b/{path}")
            } else {
                "/dev/null".to_string()
            };
            output.push_str(&format!(
                "Binary files {old_label} and {new_label} differ\n"
            ));
            continue;
        }

        // Both are text (or one is absent and the other is text)
        let old_tokens = match old_opt {
            Some(bytes) => tokenize_text(bytes)?,
            None => Vec::new(),
        };
        let new_tokens = match new_opt {
            Some(bytes) => tokenize_text(bytes)?,
            None => Vec::new(),
        };

        // File headers
        if old_opt.is_some() {
            output.push_str(&format!("--- a/{path}\n"));
        } else {
            output.push_str("--- /dev/null\n");
        }

        if new_opt.is_some() {
            output.push_str(&format!("+++ b/{path}\n"));
        } else {
            output.push_str("+++ /dev/null\n");
        }

        output.push_str(&format!(
            "@@ -1,{} +1,{} @@\n",
            old_tokens.len(),
            new_tokens.len()
        ));

        let edit = diff_tokens(&old_tokens, &new_tokens);
        let mut old_idx = 0;

        for op in edit {
            match op {
                TextEditOp::Retain(count) => {
                    for _ in 0..count {
                        let token = &old_tokens[old_idx];
                        format_diff_line(&mut output, ' ', token);
                        old_idx += 1;
                    }
                }
                TextEditOp::Delete(count) => {
                    for _ in 0..count {
                        let token = &old_tokens[old_idx];
                        format_diff_line(&mut output, '-', token);
                        old_idx += 1;
                    }
                }
                TextEditOp::Insert(tokens) => {
                    for token in &tokens {
                        format_diff_line(&mut output, '+', token);
                    }
                }
            }
        }
    }

    Ok(output)
}

fn format_diff_line(output: &mut String, prefix: char, token: &str) {
    output.push(prefix);
    output.push_str(token);
    if !token.ends_with('\n') {
        output.push('\n');
        output.push_str("\\ No newline at end of file\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tree_diff_repeated_and_missing_newline() {
        let mut old_tree = BTreeMap::new();
        old_tree.insert("repeated.txt".to_string(), b"a\nb\na\n".to_vec());

        let mut new_tree = BTreeMap::new();
        new_tree.insert("repeated.txt".to_string(), b"b\na\na".to_vec());
        new_tree.insert("added.txt".to_string(), b"new".to_vec());

        let diff = format_tree_diff(&old_tree, &new_tree).unwrap();
        let expected = "\
--- /dev/null
+++ b/added.txt
@@ -1,0 +1,1 @@
+new
\\ No newline at end of file
--- a/repeated.txt
+++ b/repeated.txt
@@ -1,3 +1,3 @@
-a
 b
 a
+a
\\ No newline at end of file
";
        assert_eq!(diff, expected);
    }

    #[test]
    fn test_format_tree_diff_binary_and_empty() {
        let old_tree = BTreeMap::new();

        let mut new_tree = BTreeMap::new();
        new_tree.insert("data.bin".to_string(), vec![0x00, 0xFF]);
        new_tree.insert("empty".to_string(), Vec::new());

        let diff = format_tree_diff(&old_tree, &new_tree).unwrap();
        let expected = "\
Binary files /dev/null and b/data.bin differ
--- /dev/null
+++ b/empty
@@ -1,0 +1,0 @@
";
        assert_eq!(diff, expected);
    }
}
