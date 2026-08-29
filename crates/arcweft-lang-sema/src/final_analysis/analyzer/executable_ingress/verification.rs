//! Independent completed-graph verification for executable ingress.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::{ItemId, TypeId},
    symbol::CallableDeclarationKey,
};

use crate::{
    entry::PreparedEntryRootCatalog, final_analysis::PreparedIncludeFlowProof,
    types::SemanticTypeDigest,
};

use super::worklist::PreparedDeclarationIngressProof;
use super::{
    Analyzer, FinalSemanticAnalysisError, PreparedExecutableDeclarationInventory,
    PreparedExecutableIngressFacts, StatementPreparationLimits, extract_declaration_adjacency,
};

/// Recompute Entry reachability from the completed typed call/Include graph.
///
/// This traversal owns fresh pending and adjacency scratch. It intentionally
/// never reads the production worklist's cache, so a cache omission or stale
/// edge cannot make the independent check agree by construction.
pub(in crate::final_analysis::analyzer) fn recompute_ingress(
    inventory: &PreparedExecutableDeclarationInventory,
    roots: &PreparedEntryRootCatalog,
    includes: &BTreeMap<arcweft_lang_hir::identity::StmtId, PreparedIncludeFlowProof>,
    analyzer: &Analyzer<'_, '_, '_>,
    limits: StatementPreparationLimits,
) -> Result<PreparedExecutableIngressFacts, FinalSemanticAnalysisError> {
    let mut facts = PreparedExecutableIngressFacts::default();
    let mut pending = BTreeMap::<CallableDeclarationKey, BTreeSet<ItemId>>::new();
    let mut adjacency = BTreeMap::<CallableDeclarationKey, Box<[CallableDeclarationKey]>>::new();
    let mut cached_edge_count = 0_u64;
    let mut work = 0_u64;

    for root in roots.roots() {
        let declaration = CallableDeclarationKey::Flow(root.target().declaration().clone());
        if inventory.get(&declaration).is_none() {
            return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
        }
        admit_recomputed_one(
            &mut facts,
            &mut pending,
            declaration,
            root.event_type(),
            root.event_digest(),
            root.entry(),
            &mut work,
            limits,
        )?;
    }

    while let Some((source, delta)) = pop_recomputed(&mut pending, &mut work, limits)? {
        let (event_type, event_digest, checked) = facts
            .declarations
            .get(&source)
            .map(|proof| (proof.event_type, proof.event_digest, proof.checked))
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
        if !checked {
            let mut edges = Vec::new();
            extract_declaration_adjacency(
                analyzer,
                &source,
                inventory,
                includes,
                |_kind| charge_recomputed_work(&mut work, limits),
                |target| {
                    edges.push(target);
                    Ok(())
                },
            )?;
            edges.sort();
            let edge_count = u64::try_from(edges.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
            let next_edge_count = cached_edge_count
                .checked_add(edge_count)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
            if next_edge_count > limits.max_edges {
                return Err(FinalSemanticAnalysisError::AccountingOverflow);
            }
            let edges = edges.into_boxed_slice();
            cached_edge_count = next_edge_count;
            let previous = adjacency.insert(source.clone(), edges);
            debug_assert!(previous.is_none());
            facts
                .declarations
                .get_mut(&source)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                .checked = true;
        }
        let edge_count = adjacency
            .get(&source)
            .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
            .len();
        for edge_index in 0..edge_count {
            let target = adjacency
                .get(&source)
                .and_then(|edges| edges.get(edge_index))
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            if inventory.get(target).is_none() {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            }
            for contributor in delta.iter().copied() {
                charge_recomputed_work(&mut work, limits)?;
                let target = target.clone();
                admit_recomputed_one(
                    &mut facts,
                    &mut pending,
                    target,
                    event_type,
                    event_digest,
                    contributor,
                    &mut work,
                    limits,
                )?;
            }
        }
    }
    Ok(facts)
}

fn pop_recomputed(
    pending: &mut BTreeMap<CallableDeclarationKey, BTreeSet<ItemId>>,
    work: &mut u64,
    limits: StatementPreparationLimits,
) -> Result<Option<(CallableDeclarationKey, Box<[ItemId]>)>, FinalSemanticAnalysisError> {
    if pending.is_empty() {
        return Ok(None);
    }
    charge_recomputed_work(work, limits)?;
    let (declaration, retained) = pending
        .pop_first()
        .expect("pending key remains present after queue-pop preflight");
    let delta = retained.into_iter().collect::<Box<[_]>>();
    Ok(Some((declaration, delta)))
}

pub(in crate::final_analysis::analyzer) fn charge_recomputed_work(
    work: &mut u64,
    limits: StatementPreparationLimits,
) -> Result<(), FinalSemanticAnalysisError> {
    charge_recomputed_work_amount(work, 1, limits)
}

fn charge_recomputed_work_amount(
    work: &mut u64,
    amount: u64,
    limits: StatementPreparationLimits,
) -> Result<(), FinalSemanticAnalysisError> {
    let next = work
        .checked_add(amount)
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    if next > limits.max_work {
        return Err(FinalSemanticAnalysisError::AccountingOverflow);
    }
    *work = next;
    Ok(())
}

pub(in crate::final_analysis::analyzer) fn require_recomputed_charge_capacity(
    work: u64,
    amount: u64,
    limits: StatementPreparationLimits,
) -> Result<(), FinalSemanticAnalysisError> {
    let next = work
        .checked_add(amount)
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    if next > limits.max_work {
        return Err(FinalSemanticAnalysisError::AccountingOverflow);
    }
    Ok(())
}

pub(in crate::final_analysis::analyzer) fn admit_recomputed_one(
    facts: &mut PreparedExecutableIngressFacts,
    pending: &mut BTreeMap<CallableDeclarationKey, BTreeSet<ItemId>>,
    declaration: CallableDeclarationKey,
    event_type: TypeId,
    event_digest: SemanticTypeDigest,
    contributor: ItemId,
    work: &mut u64,
    limits: StatementPreparationLimits,
) -> Result<(), FinalSemanticAnalysisError> {
    let new_declaration = !facts.declarations.contains_key(&declaration);
    if new_declaration
        && u64::try_from(facts.declarations.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
            >= limits.max_declarations
    {
        return Err(FinalSemanticAnalysisError::AccountingOverflow);
    }
    if let Some(proof) = facts.declarations.get(&declaration)
        && proof.event_digest != event_digest
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }

    let retained_contributor_count = facts
        .declarations
        .get(&declaration)
        .map_or(0, |proof| proof.contributors.len());
    let new_contributor = !facts
        .declarations
        .get(&declaration)
        .is_some_and(|proof| proof.contributors.contains(&contributor));
    let final_contributor_count = retained_contributor_count
        .checked_add(usize::from(new_contributor))
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    if u64::try_from(final_contributor_count)
        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
        > limits.max_entry_contributors
    {
        return Err(FinalSemanticAnalysisError::AccountingOverflow);
    }

    let checked = facts
        .declarations
        .get(&declaration)
        .is_some_and(|proof| proof.checked);
    let needs_pending = new_contributor || !checked;
    let insert_pending = needs_pending && !pending.contains_key(&declaration);
    if insert_pending
        && u64::try_from(pending.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
            >= limits.max_declarations
    {
        return Err(FinalSemanticAnalysisError::AccountingOverflow);
    }

    let new_contributor_count = u64::from(new_contributor);
    let required_work = u64::from(new_declaration)
        .checked_add(new_contributor_count)
        .and_then(|value| value.checked_add(new_contributor_count))
        .and_then(|value| value.checked_add(u64::from(insert_pending)))
        .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
    require_recomputed_charge_capacity(*work, required_work, limits)?;

    if new_declaration {
        charge_recomputed_work(work, limits)?;
        let previous = facts.declarations.insert(
            declaration.clone(),
            PreparedDeclarationIngressProof {
                event_type,
                event_digest,
                contributors: BTreeSet::new(),
                checked: false,
            },
        );
        debug_assert!(previous.is_none());
    }
    if new_contributor {
        charge_recomputed_work(work, limits)?;
        let inserted = facts
            .declarations
            .get_mut(&declaration)
            .expect("recomputed declaration row was retained after preflight")
            .contributors
            .insert(contributor);
        debug_assert!(inserted);

        charge_recomputed_work(work, limits)?;
        if insert_pending {
            let delta = BTreeSet::from([contributor]);
            charge_recomputed_work(work, limits)?;
            let previous = pending.insert(declaration, delta);
            debug_assert!(previous.is_none());
        } else {
            let delta = pending
                .get_mut(&declaration)
                .expect("pending delta exists when declaration is already queued");
            let inserted = delta.insert(contributor);
            debug_assert!(inserted);
        }
    } else if insert_pending {
        charge_recomputed_work(work, limits)?;
        let previous = pending.insert(declaration, BTreeSet::new());
        debug_assert!(previous.is_none());
    }
    Ok(())
}

pub(in crate::final_analysis::analyzer) fn same_ingress_facts(
    left: &PreparedExecutableIngressFacts,
    right: &PreparedExecutableIngressFacts,
) -> bool {
    left.declarations.len() == right.declarations.len()
        && left.declarations.iter().all(|(declaration, left)| {
            right.declarations.get(declaration).is_some_and(|right| {
                left.event_type == right.event_type
                    && left.event_digest == right.event_digest
                    && left.contributors == right.contributors
                    && left.checked == right.checked
            })
        })
}
