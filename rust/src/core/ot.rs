use std::collections::VecDeque;
use std::fmt;

use crate::core::patch::TextEditOp;

/// Errors that can occur during Operational Transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OtError {
    BaseTokenCountMismatch { p_remaining: u64, q_remaining: u64 },
}

impl fmt::Display for OtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OtError::BaseTokenCountMismatch {
                p_remaining,
                q_remaining,
            } => write!(
                f,
                "base token count mismatch during OT: P remaining {p_remaining}, Q remaining {q_remaining}"
            ),
        }
    }
}

impl std::error::Error for OtError {}

/// Transform incoming edit script `p` against concurrent context edit script `q`
/// according to SPEC §6.3.
///
/// Both scripts must consume the identical base token count.
/// The `Q insert` row has priority over `P insert`.
/// Returns the transformed script `P'` ready to apply onto `apply(Base, Q)`.
pub fn transform_edit(p: &[TextEditOp], q: &[TextEditOp]) -> Result<Vec<TextEditOp>, OtError> {
    let mut p_queue: VecDeque<TextEditOp> = p.iter().cloned().collect();
    let mut q_queue: VecDeque<TextEditOp> = q.iter().cloned().collect();

    let mut raw_output = Vec::new();

    while !p_queue.is_empty() || !q_queue.is_empty() {
        // 1. Q insert has top priority
        if let Some(TextEditOp::Insert(q_tokens)) = q_queue.front() {
            let len = q_tokens.len() as u64;
            raw_output.push(TextEditOp::Retain(len));
            q_queue.pop_front();
            continue;
        }

        // 2. P insert
        if let Some(TextEditOp::Insert(p_tokens)) = p_queue.front() {
            raw_output.push(TextEditOp::Insert(p_tokens.clone()));
            p_queue.pop_front();
            continue;
        }

        // 3. Both must be base-token consuming operations (Retain or Delete)
        match (p_queue.front_mut(), q_queue.front_mut()) {
            (Some(p_op), Some(q_op)) => {
                let (p_count, is_p_delete) = match p_op {
                    TextEditOp::Retain(n) => (*n, false),
                    TextEditOp::Delete(n) => (*n, true),
                    TextEditOp::Insert(tokens) => (tokens.len() as u64, false),
                };
                let (q_count, is_q_delete) = match q_op {
                    TextEditOp::Retain(n) => (*n, false),
                    TextEditOp::Delete(n) => (*n, true),
                    TextEditOp::Insert(tokens) => (tokens.len() as u64, false),
                };

                let min_count = std::cmp::min(p_count, q_count);

                // Table from §6.3:
                // P retain, Q retain -> retain(min)
                // P delete, Q retain -> delete(min)
                // P retain, Q delete -> nothing
                // P delete, Q delete -> nothing
                if !is_q_delete {
                    if is_p_delete {
                        raw_output.push(TextEditOp::Delete(min_count));
                    } else {
                        raw_output.push(TextEditOp::Retain(min_count));
                    }
                }

                // Consume min_count from P
                if p_count == min_count {
                    p_queue.pop_front();
                } else if let Some(TextEditOp::Retain(n) | TextEditOp::Delete(n)) =
                    p_queue.front_mut()
                {
                    *n -= min_count;
                }

                // Consume min_count from Q
                if q_count == min_count {
                    q_queue.pop_front();
                } else if let Some(TextEditOp::Retain(n) | TextEditOp::Delete(n)) =
                    q_queue.front_mut()
                {
                    *n -= min_count;
                }
            }
            (Some(p_op), None) => {
                let p_rem = match p_op {
                    TextEditOp::Retain(n) | TextEditOp::Delete(n) => *n,
                    TextEditOp::Insert(tokens) => tokens.len() as u64,
                };
                return Err(OtError::BaseTokenCountMismatch {
                    p_remaining: p_rem,
                    q_remaining: 0,
                });
            }
            (None, Some(q_op)) => {
                let q_rem = match q_op {
                    TextEditOp::Retain(n) | TextEditOp::Delete(n) => *n,
                    TextEditOp::Insert(tokens) => tokens.len() as u64,
                };
                return Err(OtError::BaseTokenCountMismatch {
                    p_remaining: 0,
                    q_remaining: q_rem,
                });
            }
            (None, None) => break,
        }
    }

    // Coalesce adjacent operations of the same kind
    let mut coalesced: Vec<TextEditOp> = Vec::new();

    for op in raw_output {
        match (coalesced.last_mut(), op) {
            (Some(TextEditOp::Retain(prev)), TextEditOp::Retain(count)) => {
                *prev += count;
            }
            (Some(TextEditOp::Delete(prev)), TextEditOp::Delete(count)) => {
                *prev += count;
            }
            (Some(TextEditOp::Insert(tokens)), TextEditOp::Insert(new_tokens)) => {
                tokens.extend(new_tokens);
            }
            (_, new_op) => {
                coalesced.push(new_op);
            }
        }
    }

    Ok(coalesced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::{apply_edit, diff_tokens, tokenize_text};

    #[test]
    fn test_scenario_d1_pairwise_ot_table() {
        // Base tokens: ["base\n"]
        // Case 1: P inserts while Q retains
        let p1 = vec![
            TextEditOp::Insert(vec!["P\n".to_string()]),
            TextEditOp::Retain(1),
        ];
        let q1 = vec![TextEditOp::Retain(1)];
        let p_trans1 = transform_edit(&p1, &q1).unwrap();
        assert_eq!(
            p_trans1,
            vec![
                TextEditOp::Insert(vec!["P\n".to_string()]),
                TextEditOp::Retain(1)
            ]
        );

        // Case 2: Q inserts while P retains
        let p2 = vec![TextEditOp::Retain(1)];
        let q2 = vec![
            TextEditOp::Insert(vec!["Q\n".to_string()]),
            TextEditOp::Retain(1),
        ];
        let p_trans2 = transform_edit(&p2, &q2).unwrap();
        // Emits retain(length(Q insert)) = retain(1), then retain(1) -> coalesced retain(2)
        assert_eq!(p_trans2, vec![TextEditOp::Retain(2)]);

        // Case 3: Both P and Q insert at identical cursor position
        // Q insert has priority, so transformed P emits retain(len(Q)), then P insert, then retain(1)
        let p3 = vec![
            TextEditOp::Insert(vec!["P\n".to_string()]),
            TextEditOp::Retain(1),
        ];
        let q3 = vec![
            TextEditOp::Insert(vec!["Q\n".to_string()]),
            TextEditOp::Retain(1),
        ];
        let p_trans3 = transform_edit(&p3, &q3).unwrap();
        assert_eq!(
            p_trans3,
            vec![
                TextEditOp::Retain(1),
                TextEditOp::Insert(vec!["P\n".to_string()]),
                TextEditOp::Retain(1),
            ]
        );

        // Case 4: P deletes while Q retains
        let p4 = vec![TextEditOp::Delete(1)];
        let q4 = vec![TextEditOp::Retain(1)];
        let p_trans4 = transform_edit(&p4, &q4).unwrap();
        assert_eq!(p_trans4, vec![TextEditOp::Delete(1)]);

        // Case 5: P retains while Q deletes
        let p5 = vec![TextEditOp::Retain(1)];
        let q5 = vec![TextEditOp::Delete(1)];
        let p_trans5 = transform_edit(&p5, &q5).unwrap();
        assert_eq!(p_trans5, Vec::<TextEditOp>::new());

        // Case 6: Both P and Q delete
        let p6 = vec![TextEditOp::Delete(1)];
        let q6 = vec![TextEditOp::Delete(1)];
        let p_trans6 = transform_edit(&p6, &q6).unwrap();
        assert_eq!(p_trans6, Vec::<TextEditOp>::new());
    }

    #[test]
    fn test_scenario_d2_three_way_text_ot_merge() {
        // Base: 0 through 4
        let base_tokens = tokenize_text(b"0\n1\n2\n3\n4\n").unwrap();

        // A edits: "A\n0\n3\n4\nTAIL\n"
        let a_tokens = tokenize_text(b"A\n0\n3\n4\nTAIL\n").unwrap();
        let p = diff_tokens(&base_tokens, &a_tokens);

        // B edits: "0\n1\nB\n3\n4\n"
        let b_tokens = tokenize_text(b"0\n1\nB\n3\n4\n").unwrap();
        let q = diff_tokens(&base_tokens, &b_tokens);

        // Transform P against Q
        let p_prime = transform_edit(&p, &q).unwrap();

        // Apply P' onto B's tokens
        let merged = apply_edit(&b_tokens, &p_prime).unwrap();
        let expected = tokenize_text(b"A\n0\nB\n3\n4\nTAIL\n").unwrap();
        assert_eq!(merged, expected);

        // Commutativity: Transform Q against P, apply Q' onto A's tokens
        let q_prime = transform_edit(&q, &p).unwrap();
        let merged_comm = apply_edit(&a_tokens, &q_prime).unwrap();
        assert_eq!(merged_comm, expected);
    }
}
