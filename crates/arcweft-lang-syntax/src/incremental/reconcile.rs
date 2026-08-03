//! Deterministic attached-grammar syntax identity reconciliation.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{ParseFailure, SyntaxInvariantFailure};
use crate::attachment::{GrammarIdentityMap, SyntaxNodeId as AttachedSyntaxNodeId};
use crate::grammar::build::GrammarEventPath;
use crate::grammar::kinds::SyntaxRoleClass;
use crate::incremental::shape::GrammarShapeNode;

pub(super) fn allocate_initial_grammar(
    root: &GrammarShapeNode,
    allocate: &mut impl FnMut() -> Result<AttachedSyntaxNodeId, ParseFailure>,
) -> Result<GrammarIdentityMap, ParseFailure> {
    let mut identities = HashMap::new();
    allocate_grammar_subtree(root, &mut identities, allocate)?;
    Ok(GrammarIdentityMap::new(identities))
}

pub(super) fn reconcile_grammar(
    old_root: &GrammarShapeNode,
    new_root: &GrammarShapeNode,
    old_identities: &GrammarIdentityMap,
    allocate: &mut impl FnMut() -> Result<AttachedSyntaxNodeId, ParseFailure>,
) -> Result<GrammarIdentityMap, ParseFailure> {
    let root_id = old_identities
        .id_for_path(old_root.path())
        .ok_or(SyntaxInvariantFailure::IdentityMapMismatch)?;
    let mut identities = HashMap::new();
    reconcile_grammar_node(
        old_root,
        new_root,
        root_id,
        old_identities,
        &mut identities,
        allocate,
    )?;
    Ok(GrammarIdentityMap::new(identities))
}

fn reconcile_grammar_node(
    old: &GrammarShapeNode,
    new: &GrammarShapeNode,
    retained_id: AttachedSyntaxNodeId,
    old_identities: &GrammarIdentityMap,
    identities: &mut HashMap<GrammarEventPath, AttachedSyntaxNodeId>,
    allocate: &mut impl FnMut() -> Result<AttachedSyntaxNodeId, ParseFailure>,
) -> Result<(), ParseFailure> {
    identities.insert(new.path().clone(), retained_id);

    let old_children = old.children();
    let new_children = new.children();
    let mut old_matched = vec![false; old_children.len()];
    let mut new_to_old = vec![None; new_children.len()];
    let roles = old_children
        .iter()
        .chain(new_children)
        .map(GrammarShapeNode::role_class)
        .collect::<BTreeSet<_>>();

    for role in roles {
        let old_bucket = old_children
            .iter()
            .enumerate()
            .filter(|(_, node)| node.role_class() == role)
            .collect::<Vec<_>>();
        let new_bucket = new_children
            .iter()
            .enumerate()
            .filter(|(_, node)| node.role_class() == role)
            .collect::<Vec<_>>();
        match_unique_grammar_shapes(&old_bucket, &new_bucket, &mut old_matched, &mut new_to_old);
        match_remaining_grammar_shapes(
            &old_bucket,
            &new_bucket,
            old_identities,
            &mut old_matched,
            &mut new_to_old,
        )?;
    }

    let bridges = replacement_bridges(old_children, new_children, &old_matched, &new_to_old);
    for (new_index, new_child) in new_children.iter().enumerate() {
        if let Some(old_index) = new_to_old[new_index] {
            let old_child = &old_children[old_index];
            let child_id = old_identities
                .id_for_path(old_child.path())
                .ok_or(SyntaxInvariantFailure::IdentityMapMismatch)?;
            reconcile_grammar_node(
                old_child,
                new_child,
                child_id,
                old_identities,
                identities,
                allocate,
            )?;
        } else if let Some(&old_index) = bridges.get(&new_index) {
            let fresh = allocate()?;
            reconcile_grammar_node(
                &old_children[old_index],
                new_child,
                fresh,
                old_identities,
                identities,
                allocate,
            )?;
        } else {
            allocate_grammar_subtree(new_child, identities, allocate)?;
        }
    }
    Ok(())
}

type IndexedGrammarShape<'a> = (usize, &'a GrammarShapeNode);

fn match_unique_grammar_shapes(
    old: &[IndexedGrammarShape<'_>],
    new: &[IndexedGrammarShape<'_>],
    old_matched: &mut [bool],
    new_to_old: &mut [Option<usize>],
) {
    let old_nodes = old.iter().copied().collect::<HashMap<_, _>>();
    let new_nodes = new.iter().copied().collect::<HashMap<_, _>>();
    let mut old_by_digest = HashMap::<[u8; 32], Vec<usize>>::new();
    let mut new_by_digest = HashMap::<[u8; 32], Vec<usize>>::new();
    for &(index, child) in old {
        old_by_digest
            .entry(*child.digest())
            .or_default()
            .push(index);
    }
    for &(index, child) in new {
        new_by_digest
            .entry(*child.digest())
            .or_default()
            .push(index);
    }

    for (digest, old_indices) in old_by_digest {
        let Some(new_indices) = new_by_digest.get(&digest) else {
            continue;
        };
        for &old_index in &old_indices {
            let Some(&old_child) = old_nodes.get(&old_index) else {
                continue;
            };
            if old_indices
                .iter()
                .filter(|&&candidate| {
                    old_nodes
                        .get(&candidate)
                        .is_some_and(|candidate| grammar_full_shape_equals(old_child, candidate))
                })
                .count()
                != 1
            {
                continue;
            }
            let candidates = new_indices
                .iter()
                .copied()
                .filter(|&candidate| {
                    new_nodes
                        .get(&candidate)
                        .is_some_and(|candidate| grammar_full_shape_equals(old_child, candidate))
                })
                .collect::<Vec<_>>();
            if let [new_index] = candidates.as_slice()
                && new_to_old[*new_index].is_none()
            {
                old_matched[old_index] = true;
                new_to_old[*new_index] = Some(old_index);
            }
        }
    }
}

fn match_remaining_grammar_shapes(
    old: &[IndexedGrammarShape<'_>],
    new: &[IndexedGrammarShape<'_>],
    old_identities: &GrammarIdentityMap,
    old_matched: &mut [bool],
    new_to_old: &mut [Option<usize>],
) -> Result<(), ParseFailure> {
    let remaining_old = old
        .iter()
        .copied()
        .filter(|(index, _)| !old_matched[*index])
        .collect::<Vec<_>>();
    let remaining_new = new
        .iter()
        .copied()
        .filter(|(index, _)| new_to_old[*index].is_none())
        .collect::<Vec<_>>();
    for (old_position, new_position) in
        stable_grammar_lcs(&remaining_old, &remaining_new, old_identities)?
    {
        let old_ordinal = remaining_old[old_position].0;
        let new_ordinal = remaining_new[new_position].0;
        old_matched[old_ordinal] = true;
        new_to_old[new_ordinal] = Some(old_ordinal);
    }
    Ok(())
}

fn stable_grammar_lcs(
    old: &[IndexedGrammarShape<'_>],
    new: &[IndexedGrammarShape<'_>],
    old_identities: &GrammarIdentityMap,
) -> Result<Vec<(usize, usize)>, ParseFailure> {
    let mut matches = Vec::new();
    let mut old_start = 0;
    let mut new_start = 0;
    while old_start < old.len() && new_start < new.len() {
        let Some((old_position, new_position)) =
            first_grammar_match(&old[old_start..], &new[new_start..], old_identities)?
        else {
            break;
        };
        matches.push((old_start + old_position, new_start + new_position));
        old_start += old_position + 1;
        new_start += new_position + 1;
    }
    Ok(matches)
}

fn first_grammar_match(
    old: &[IndexedGrammarShape<'_>],
    new: &[IndexedGrammarShape<'_>],
    old_identities: &GrammarIdentityMap,
) -> Result<Option<(usize, usize)>, ParseFailure> {
    type Candidate = (usize, core::num::NonZeroU64, usize, usize, usize);

    let mut below = vec![0_usize; new.len() + 1];
    let mut current = vec![0_usize; new.len() + 1];
    let mut best_by_length = vec![None::<Candidate>; old.len().min(new.len()) + 1];
    for old_position in (0..old.len()).rev() {
        current[new.len()] = 0;
        let (old_ordinal, old_node) = old[old_position];
        let old_id = old_identities
            .id_for_path(old_node.path())
            .ok_or(SyntaxInvariantFailure::IdentityMapMismatch)?;
        for new_position in (0..new.len()).rev() {
            if old_node.own() == new[new_position].1.own() {
                let chain_length = 1 + below[new_position + 1];
                current[new_position] = chain_length;
                let candidate = (
                    old_ordinal.abs_diff(new[new_position].0),
                    old_id.slot(),
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

    Ok(best_by_length[below[0]]
        .map(|(_, _, _, old_position, new_position)| (old_position, new_position)))
}

fn replacement_bridges(
    old: &[GrammarShapeNode],
    new: &[GrammarShapeNode],
    old_matched: &[bool],
    new_to_old: &[Option<usize>],
) -> BTreeMap<usize, usize> {
    let roles = old
        .iter()
        .chain(new)
        .map(GrammarShapeNode::role_class)
        .collect::<BTreeSet<_>>();
    let mut bridges = BTreeMap::new();
    for role in roles {
        let old_candidates = old
            .iter()
            .enumerate()
            .filter(|(index, node)| !old_matched[*index] && node.role_class() == role)
            .collect::<Vec<_>>();
        let new_candidates = new
            .iter()
            .enumerate()
            .filter(|(index, node)| new_to_old[*index].is_none() && node.role_class() == role)
            .collect::<Vec<_>>();
        if let ([(old_index, old_node)], [(new_index, new_node)]) =
            (old_candidates.as_slice(), new_candidates.as_slice())
            && child_role_partitions_are_unambiguous(old_node, new_node)
        {
            bridges.insert(*new_index, *old_index);
        }
    }
    bridges
}

fn child_role_partitions_are_unambiguous(old: &GrammarShapeNode, new: &GrammarShapeNode) -> bool {
    fn counts(node: &GrammarShapeNode) -> BTreeMap<SyntaxRoleClass, usize> {
        let mut counts = BTreeMap::new();
        for child in node.children() {
            *counts.entry(child.role_class()).or_default() += 1;
        }
        counts
    }

    let old_counts = counts(old);
    let new_counts = counts(new);
    old_counts == new_counts && old_counts.values().all(|count| *count <= 1)
}

fn allocate_grammar_subtree(
    node: &GrammarShapeNode,
    identities: &mut HashMap<GrammarEventPath, AttachedSyntaxNodeId>,
    allocate: &mut impl FnMut() -> Result<AttachedSyntaxNodeId, ParseFailure>,
) -> Result<(), ParseFailure> {
    identities.insert(node.path().clone(), allocate()?);
    for child in node.children() {
        allocate_grammar_subtree(child, identities, allocate)?;
    }
    Ok(())
}

fn grammar_full_shape_equals(left: &GrammarShapeNode, right: &GrammarShapeNode) -> bool {
    left.digest() == right.digest() && left.exactly_equals(right)
}
