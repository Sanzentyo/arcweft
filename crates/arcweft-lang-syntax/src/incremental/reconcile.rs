//! Deterministic parent-local syntax identity reconciliation.

use std::collections::HashMap;

use super::shape::ShapeNode;
use super::{ParseFailure, SyntaxIdentityMap, SyntaxNodeId};
use crate::cst::SyntaxNode;

pub(super) fn allocate_initial(
    root: &ShapeNode,
    allocate: &mut impl FnMut() -> Result<SyntaxNodeId, ParseFailure>,
) -> Result<SyntaxIdentityMap, ParseFailure> {
    let mut identities = HashMap::new();
    allocate_subtree(root, &mut identities, allocate)?;
    Ok(SyntaxIdentityMap::new(identities))
}

pub(super) fn reconcile(
    old_root: &ShapeNode,
    new_root: &ShapeNode,
    old_identities: &SyntaxIdentityMap,
    allocate: &mut impl FnMut() -> Result<SyntaxNodeId, ParseFailure>,
) -> Result<SyntaxIdentityMap, ParseFailure> {
    let root_id = old_identities
        .id_for(old_root.syntax())
        .ok_or(ParseFailure::InternalInvariant)?;
    let mut identities = HashMap::new();
    reconcile_matched_node(
        old_root,
        new_root,
        root_id,
        old_identities,
        &mut identities,
        allocate,
    )?;
    Ok(SyntaxIdentityMap::new(identities))
}

fn reconcile_matched_node(
    old: &ShapeNode,
    new: &ShapeNode,
    retained_id: SyntaxNodeId,
    old_identities: &SyntaxIdentityMap,
    identities: &mut HashMap<SyntaxNode, SyntaxNodeId>,
    allocate: &mut impl FnMut() -> Result<SyntaxNodeId, ParseFailure>,
) -> Result<(), ParseFailure> {
    identities.insert(new.syntax().clone(), retained_id);

    let old_children = old.children();
    let new_children = new.children();
    let mut old_matched = vec![false; old_children.len()];
    let mut new_to_old = vec![None; new_children.len()];

    match_unique_full_shapes(
        old_children,
        new_children,
        &mut old_matched,
        &mut new_to_old,
    );
    match_remaining_own_shapes(
        old_children,
        new_children,
        old_identities,
        &mut old_matched,
        &mut new_to_old,
    )?;

    for (new_index, new_child) in new_children.iter().enumerate() {
        if let Some(old_index) = new_to_old[new_index] {
            let old_child = &old_children[old_index];
            let child_id = old_identities
                .id_for(old_child.syntax())
                .ok_or(ParseFailure::InternalInvariant)?;
            reconcile_matched_node(
                old_child,
                new_child,
                child_id,
                old_identities,
                identities,
                allocate,
            )?;
        } else {
            allocate_subtree(new_child, identities, allocate)?;
        }
    }
    Ok(())
}

fn match_unique_full_shapes(
    old: &[ShapeNode],
    new: &[ShapeNode],
    old_matched: &mut [bool],
    new_to_old: &mut [Option<usize>],
) {
    let mut old_by_digest = HashMap::<[u8; 32], Vec<usize>>::new();
    let mut new_by_digest = HashMap::<[u8; 32], Vec<usize>>::new();
    for (index, child) in old.iter().enumerate() {
        old_by_digest
            .entry(*child.digest())
            .or_default()
            .push(index);
    }
    for (index, child) in new.iter().enumerate() {
        new_by_digest
            .entry(*child.digest())
            .or_default()
            .push(index);
    }

    for (digest, old_indices) in old_by_digest {
        let Some(new_indices) = new_by_digest.get(&digest) else {
            continue;
        };
        if old_indices.len() > 1
            && old_indices[1..]
                .iter()
                .all(|&index| full_shape_equals(&old[old_indices[0]], &old[index]))
        {
            continue;
        }
        for &old_index in &old_indices {
            let old_child = &old[old_index];
            if old_indices
                .iter()
                .filter(|&&candidate| full_shape_equals(old_child, &old[candidate]))
                .count()
                != 1
            {
                continue;
            }
            let mut candidates = new_indices
                .iter()
                .copied()
                .filter(|&candidate| full_shape_equals(old_child, &new[candidate]));
            let Some(new_index) = candidates.next() else {
                continue;
            };
            if candidates.next().is_none() && new_to_old[new_index].is_none() {
                old_matched[old_index] = true;
                new_to_old[new_index] = Some(old_index);
            }
        }
    }
}

fn match_remaining_own_shapes(
    old: &[ShapeNode],
    new: &[ShapeNode],
    old_identities: &SyntaxIdentityMap,
    old_matched: &mut [bool],
    new_to_old: &mut [Option<usize>],
) -> Result<(), ParseFailure> {
    let remaining_old = old
        .iter()
        .enumerate()
        .filter(|(index, _)| !old_matched[*index])
        .collect::<Vec<_>>();
    let remaining_new = new
        .iter()
        .enumerate()
        .filter(|(index, _)| new_to_old[*index].is_none())
        .collect::<Vec<_>>();
    for (old_position, new_position) in
        stable_lcs_matches(&remaining_old, &remaining_new, old_identities)?
    {
        let old_ordinal = remaining_old[old_position].0;
        let new_ordinal = remaining_new[new_position].0;
        old_matched[old_ordinal] = true;
        new_to_old[new_ordinal] = Some(old_ordinal);
    }
    Ok(())
}

type IndexedShape<'a> = (usize, &'a ShapeNode);

fn stable_lcs_matches(
    old: &[IndexedShape<'_>],
    new: &[IndexedShape<'_>],
    old_identities: &SyntaxIdentityMap,
) -> Result<Vec<(usize, usize)>, ParseFailure> {
    let mut matches = Vec::new();
    let mut old_start = 0;
    let mut new_start = 0;
    while old_start < old.len() && new_start < new.len() {
        let old_tail = &old[old_start..];
        let new_tail = &new[new_start..];
        if all_one_own_shape(old_tail, new_tail) {
            match_one_own_shape(
                old_tail,
                new_tail,
                old_identities,
                old_start,
                new_start,
                &mut matches,
            )?;
            break;
        }

        let Some((old_position, new_position)) =
            first_normative_match(old_tail, new_tail, old_identities)?
        else {
            break;
        };
        matches.push((old_start + old_position, new_start + new_position));
        old_start += old_position + 1;
        new_start += new_position + 1;
    }
    Ok(matches)
}

fn first_normative_match(
    old: &[IndexedShape<'_>],
    new: &[IndexedShape<'_>],
    old_identities: &SyntaxIdentityMap,
) -> Result<Option<(usize, usize)>, ParseFailure> {
    type Candidate = (usize, SyntaxNodeId, usize, usize, usize);

    let mut below = vec![0_usize; new.len() + 1];
    let mut current = vec![0_usize; new.len() + 1];
    let mut best_by_length = vec![None::<Candidate>; old.len().min(new.len()) + 1];
    for old_position in (0..old.len()).rev() {
        current[new.len()] = 0;
        let (old_ordinal, old_node) = old[old_position];
        let old_id = old_identities
            .id_for(old_node.syntax())
            .ok_or(ParseFailure::InternalInvariant)?;
        for new_position in (0..new.len()).rev() {
            if old_node.own() == new[new_position].1.own() {
                let chain_length = 1 + below[new_position + 1];
                current[new_position] = chain_length;
                let candidate = (
                    old_ordinal.abs_diff(new[new_position].0),
                    old_id,
                    new[new_position].0,
                    old_position,
                    new_position,
                );
                let best = &mut best_by_length[chain_length];
                if best.is_none_or(|existing| candidate < existing) {
                    *best = Some(candidate);
                }
            } else {
                current[new_position] = below[new_position].max(current[new_position + 1]);
            }
        }
        core::mem::swap(&mut below, &mut current);
    }

    let needed = below[0];
    Ok(best_by_length[needed]
        .map(|(_, _, _, old_position, new_position)| (old_position, new_position)))
}

fn all_one_own_shape(old: &[IndexedShape<'_>], new: &[IndexedShape<'_>]) -> bool {
    let own = old[0].1.own();
    old.iter().all(|(_, node)| node.own() == own) && new.iter().all(|(_, node)| node.own() == own)
}

fn match_one_own_shape(
    old: &[IndexedShape<'_>],
    new: &[IndexedShape<'_>],
    old_identities: &SyntaxIdentityMap,
    old_base: usize,
    new_base: usize,
    matches: &mut Vec<(usize, usize)>,
) -> Result<(), ParseFailure> {
    let mut old_start = 0;
    let mut new_start = 0;
    while old_start < old.len() && new_start < new.len() {
        if old.len() - old_start <= new.len() - new_start {
            let last_new = new.len() - (old.len() - old_start);
            let selected = nearest_new_ordinal(new, new_start, last_new, old[old_start].0)
                .ok_or(ParseFailure::InternalInvariant)?;
            matches.push((old_base + old_start, new_base + selected));
            old_start += 1;
            new_start = selected + 1;
        } else {
            let last_old = old.len() - (new.len() - new_start);
            let selected =
                nearest_old_ordinal(old, old_start, last_old, new[new_start].0, old_identities)?;
            matches.push((old_base + selected, new_base + new_start));
            old_start = selected + 1;
            new_start += 1;
        }
    }
    Ok(())
}

fn nearest_new_ordinal(
    new: &[IndexedShape<'_>],
    start: usize,
    end: usize,
    old_ordinal: usize,
) -> Option<usize> {
    let candidates = &new[start..=end];
    let split = candidates.partition_point(|(ordinal, _)| *ordinal < old_ordinal);
    [
        split.checked_sub(1),
        (split < candidates.len()).then_some(split),
    ]
    .into_iter()
    .flatten()
    .map(|position| start + position)
    .min_by_key(|&position| {
        (
            old_ordinal.abs_diff(new[position].0),
            new[position].0,
            position,
        )
    })
}

fn nearest_old_ordinal(
    old: &[IndexedShape<'_>],
    start: usize,
    end: usize,
    new_ordinal: usize,
    old_identities: &SyntaxIdentityMap,
) -> Result<usize, ParseFailure> {
    let candidates = &old[start..=end];
    let split = candidates.partition_point(|(ordinal, _)| *ordinal < new_ordinal);
    [
        split.checked_sub(1),
        (split < candidates.len()).then_some(split),
    ]
    .into_iter()
    .flatten()
    .map(|position| {
        let position = start + position;
        let old_id = old_identities
            .id_for(old[position].1.syntax())
            .ok_or(ParseFailure::InternalInvariant)?;
        Ok((
            (old[position].0.abs_diff(new_ordinal), old_id, new_ordinal),
            position,
        ))
    })
    .collect::<Result<Vec<_>, ParseFailure>>()?
    .into_iter()
    .min_by_key(|(score, _)| *score)
    .map(|(_, position)| position)
    .ok_or(ParseFailure::InternalInvariant)
}

fn allocate_subtree(
    node: &ShapeNode,
    identities: &mut HashMap<SyntaxNode, SyntaxNodeId>,
    allocate: &mut impl FnMut() -> Result<SyntaxNodeId, ParseFailure>,
) -> Result<(), ParseFailure> {
    identities.insert(node.syntax().clone(), allocate()?);
    for child in node.children() {
        allocate_subtree(child, identities, allocate)?;
    }
    Ok(())
}

fn full_shape_equals(left: &ShapeNode, right: &ShapeNode) -> bool {
    left.digest() == right.digest() && left.exactly_equals(right)
}

#[cfg(test)]
mod tests {
    use super::{IndexedShape, allocate_initial, reconcile, stable_lcs_matches};
    use crate::cst::parse_cst;
    use crate::incremental::shape::ShapeNode;
    use crate::incremental::{ParseFailure, SyntaxNodeId};
    use core::num::NonZeroU64;

    #[test]
    fn large_repeated_sibling_runs_reconcile_with_bounded_storage() {
        const SIBLINGS: usize = 4_096;
        let old = ShapeNode::from_syntax(parse_cst(&"value\n".repeat(SIBLINGS)));
        let new = ShapeNode::from_syntax(parse_cst(&"value\n".repeat(SIBLINGS + 1)));
        let mut next = 1_u64;
        let mut allocate = || {
            let slot = NonZeroU64::new(next)
                .map(SyntaxNodeId)
                .ok_or(ParseFailure::InternalInvariant)?;
            next = next.checked_add(1).ok_or(ParseFailure::InternalInvariant)?;
            Ok(slot)
        };
        let old_identities = allocate_initial(&old, &mut allocate).expect("initial identities");

        let reconciled =
            reconcile(&old, &new, &old_identities, &mut allocate).expect("reconciled identities");

        assert_eq!(reconciled.len(), SIBLINGS + 2);
        for (old_child, new_child) in old.children().iter().zip(new.children()) {
            assert_eq!(
                old_identities.id_for(old_child.syntax()),
                reconciled.id_for(new_child.syntax())
            );
        }
    }

    #[test]
    fn linear_space_lcs_matches_the_normative_tie_break_on_small_sequences() {
        let sequences = (0..=5)
            .flat_map(|length| {
                (0..(1_usize << length)).map(move |bits| {
                    let source = (0..length)
                        .map(|index| {
                            if bits & (1 << index) == 0 {
                                "a\n"
                            } else {
                                "b\n"
                            }
                        })
                        .collect::<String>();
                    ShapeNode::from_syntax(parse_cst(&source))
                })
            })
            .collect::<Vec<_>>();

        for old in &sequences {
            let mut next = 1_u64;
            let mut allocate = || {
                let slot = NonZeroU64::new(next)
                    .map(SyntaxNodeId)
                    .ok_or(ParseFailure::InternalInvariant)?;
                next = next.checked_add(1).ok_or(ParseFailure::InternalInvariant)?;
                Ok(slot)
            };
            let old_identities = allocate_initial(old, &mut allocate).expect("old identities");
            let old_children = old.children().iter().enumerate().collect::<Vec<_>>();
            for new in &sequences {
                let new_children = new.children().iter().enumerate().collect::<Vec<_>>();
                let actual = stable_lcs_matches(&old_children, &new_children, &old_identities)
                    .expect("linear-space LCS");
                let expected = normative_lcs_matches(&old_children, &new_children, &old_identities);
                assert_eq!(
                    actual,
                    expected,
                    "old={:?}, new={:?}",
                    own_labels(&old_children),
                    own_labels(&new_children)
                );
            }
        }
    }

    fn normative_lcs_matches(
        old: &[IndexedShape<'_>],
        new: &[IndexedShape<'_>],
        old_identities: &super::super::SyntaxIdentityMap,
    ) -> Vec<(usize, usize)> {
        let mut lengths = vec![vec![0_usize; new.len() + 1]; old.len() + 1];
        for old_index in (0..old.len()).rev() {
            for new_index in (0..new.len()).rev() {
                lengths[old_index][new_index] = if old[old_index].1.own() == new[new_index].1.own()
                {
                    1 + lengths[old_index + 1][new_index + 1]
                } else {
                    lengths[old_index + 1][new_index].max(lengths[old_index][new_index + 1])
                };
            }
        }
        let mut matches = Vec::new();
        let mut old_start = 0;
        let mut new_start = 0;
        let mut needed = lengths[0][0];
        while needed > 0 {
            let mut best = None;
            for old_position in old_start..old.len() {
                for new_position in new_start..new.len() {
                    if old[old_position].1.own() != new[new_position].1.own()
                        || 1 + lengths[old_position + 1][new_position + 1] != needed
                    {
                        continue;
                    }
                    let old_id = old_identities
                        .id_for(old[old_position].1.syntax())
                        .expect("old identity");
                    let score = (
                        old[old_position].0.abs_diff(new[new_position].0),
                        old_id,
                        new[new_position].0,
                        old_position,
                        new_position,
                    );
                    if best.is_none_or(|current| score < current) {
                        best = Some(score);
                    }
                }
            }
            let (_, _, _, old_position, new_position) = best.expect("LCS match");
            matches.push((old_position, new_position));
            old_start = old_position + 1;
            new_start = new_position + 1;
            needed -= 1;
        }
        matches
    }

    fn own_labels(nodes: &[IndexedShape<'_>]) -> Vec<String> {
        nodes
            .iter()
            .map(|(_, node)| node.syntax().text().to_string())
            .collect()
    }
}
