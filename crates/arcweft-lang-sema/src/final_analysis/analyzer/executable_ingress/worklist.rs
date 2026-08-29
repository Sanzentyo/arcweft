//! Entry-rooted worklist state and checked contributor admission.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::{ItemId, StmtId, TypeId},
    symbol::CallableDeclarationKey,
};

use crate::{
    entry::{PreparedEntryRootCatalog, PreparedEntryRootSeed},
    final_analysis::PreparedIncludeFlowProof,
    types::SemanticTypeDigest,
};

use super::{FinalSemanticAnalysisError, PreparedExecutableDeclarationInventory};

/// Immutable proof state for one declaration reached from stateful Entries.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) struct PreparedDeclarationIngressProof {
    pub(in crate::final_analysis::analyzer) event_type: TypeId,
    pub(in crate::final_analysis::analyzer) event_digest: SemanticTypeDigest,
    pub(in crate::final_analysis::analyzer) contributors: BTreeSet<ItemId>,
    pub(in crate::final_analysis::analyzer) checked: bool,
}

/// Single private declaration→Event authority under construction.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct PreparedExecutableIngressFacts {
    pub(super) declarations: BTreeMap<CallableDeclarationKey, PreparedDeclarationIngressProof>,
}

impl PreparedExecutableIngressFacts {
    pub(crate) fn event_type(&self, declaration: &CallableDeclarationKey) -> Option<TypeId> {
        self.declarations
            .get(declaration)
            .map(|proof| proof.event_type)
    }

    pub(crate) fn event_digest(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<SemanticTypeDigest> {
        self.declarations
            .get(declaration)
            .map(|proof| proof.event_digest)
    }

    #[cfg(test)]
    pub(super) fn contributors(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<&BTreeSet<ItemId>> {
        self.declarations
            .get(declaration)
            .map(|proof| &proof.contributors)
    }
}

/// Checked operational bounds for one ingress transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) struct StatementPreparationLimits {
    pub(in crate::final_analysis::analyzer) max_declarations: u64,
    pub(in crate::final_analysis::analyzer) max_edges: u64,
    pub(in crate::final_analysis::analyzer) max_entry_contributors: u64,
    pub(in crate::final_analysis::analyzer) max_contextual_statements: u64,
    pub(in crate::final_analysis::analyzer) max_work: u64,
}

impl StatementPreparationLimits {
    pub(crate) fn production(
        inventory: &PreparedExecutableDeclarationInventory,
        roots: &PreparedEntryRootCatalog,
        include_count: usize,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let max_declarations = u64::try_from(inventory.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let max_entry_contributors = u64::try_from(roots.roots().len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let max_contextual_statements = inventory.values().try_fold(0_u64, |count, row| {
            count
                .checked_add(
                    u64::try_from(row.statements.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
        })?;
        let max_call_candidate_inspections = inventory.values().try_fold(0_u64, |count, row| {
            count
                .checked_add(
                    u64::try_from(row.expressions.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
        })?;
        let include_edges = u64::try_from(include_count)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let max_edges = max_call_candidate_inspections
            .checked_add(include_edges)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let max_contributor_pairs = max_declarations
            .checked_mul(max_entry_contributors)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        let max_edge_contributor_visits = max_edges
            .checked_mul(max_entry_contributors)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        // One bound covers every unit charged by either traversal:
        // declaration rows (D), contextual statement visits (S), call
        // candidate inspections (C), direct Include lookups over statement
        // IDs (J=S), retained adjacency edges (E), fact contributor writes
        // (D*K), pending-delta writes (D*K), queue pushes/pops (D*K each),
        // and edge/contributor propagation visits (E*K). Each traversal owns
        // its own counter.
        let work_units = [
            max_declarations,
            max_contextual_statements,
            max_call_candidate_inspections,
            max_contextual_statements,
            max_edges,
            max_contributor_pairs,
            max_contributor_pairs,
            max_contributor_pairs,
            max_contributor_pairs,
            max_edge_contributor_visits,
        ];
        let max_work = work_units.into_iter().try_fold(0_u64, |total, unit| {
            total
                .checked_add(unit)
                .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
        })?;
        Ok(Self {
            max_declarations,
            max_edges,
            max_entry_contributors,
            max_contextual_statements,
            max_work,
        })
    }

    #[cfg(test)]
    pub(super) const fn for_test(
        max_declarations: u64,
        max_edges: u64,
        max_entry_contributors: u64,
        max_contextual_statements: u64,
        max_work: u64,
    ) -> Self {
        Self {
            max_declarations,
            max_edges,
            max_entry_contributors,
            max_contextual_statements,
            max_work,
        }
    }
}

/// Move-only ingress worklist. No intermediate state can enter a report.
pub(in crate::final_analysis::analyzer) struct PreparedExecutableIngressWorklist {
    pub(super) facts: PreparedExecutableIngressFacts,
    pub(super) pending: BTreeMap<CallableDeclarationKey, BTreeSet<ItemId>>,
    pub(super) adjacency: BTreeMap<CallableDeclarationKey, Box<[CallableDeclarationKey]>>,
    pub(super) cached_edge_count: u64,
    pub(super) roots: PreparedEntryRootCatalog,
    pub(super) includes: BTreeMap<StmtId, PreparedIncludeFlowProof>,
    pub(super) limits: StatementPreparationLimits,
    pub(super) work: u64,
}

impl PreparedExecutableIngressWorklist {
    pub(super) fn new(
        inventory: &PreparedExecutableDeclarationInventory,
        roots: PreparedEntryRootCatalog,
        includes: BTreeMap<StmtId, PreparedIncludeFlowProof>,
        limits: StatementPreparationLimits,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        if u64::try_from(inventory.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
            > limits.max_declarations
            || u64::try_from(roots.roots().len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                > limits.max_entry_contributors
            || u64::try_from(includes.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                > limits.max_edges
            || inventory.values().try_fold(0_u64, |count, declaration| {
                count
                    .checked_add(
                        u64::try_from(declaration.statements.len())
                            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                    )
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
            })? > limits.max_contextual_statements
        {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }
        let mut worklist = Self {
            facts: PreparedExecutableIngressFacts::default(),
            pending: BTreeMap::new(),
            adjacency: BTreeMap::new(),
            cached_edge_count: 0,
            roots,
            includes,
            limits,
            work: 0,
        };
        let seeds = worklist
            .roots
            .roots()
            .map(|seed: &PreparedEntryRootSeed| {
                (
                    CallableDeclarationKey::Flow(seed.target().declaration().clone()),
                    seed.event_type(),
                    seed.event_digest(),
                    seed.entry(),
                )
            })
            .collect::<Vec<_>>();
        for (declaration, event_type, event_digest, entry) in seeds {
            if inventory.get(&declaration).is_none() {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            }
            worklist.admit_one(declaration, event_type, event_digest, entry)?;
        }
        Ok(worklist)
    }

    pub(super) fn charge(&mut self, amount: u64) -> Result<(), FinalSemanticAnalysisError> {
        let next = self
            .work
            .checked_add(amount)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        if next > self.limits.max_work {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }
        self.work = next;
        Ok(())
    }

    pub(super) fn require_charge_capacity(
        &self,
        amount: u64,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let next = self
            .work
            .checked_add(amount)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        if next > self.limits.max_work {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }
        Ok(())
    }

    pub(super) fn admit_one(
        &mut self,
        declaration: CallableDeclarationKey,
        event_type: TypeId,
        event_digest: SemanticTypeDigest,
        contributor: ItemId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let new_declaration = !self.facts.declarations.contains_key(&declaration);
        if new_declaration {
            if u64::try_from(self.facts.declarations.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                >= self.limits.max_declarations
            {
                return Err(FinalSemanticAnalysisError::AccountingOverflow);
            }
        }
        if let Some(proof) = self.facts.declarations.get(&declaration)
            && proof.event_digest != event_digest
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }

        let retained_contributor_count = self
            .facts
            .declarations
            .get(&declaration)
            .map_or(0, |proof| proof.contributors.len());
        let new_contributor = !self
            .facts
            .declarations
            .get(&declaration)
            .is_some_and(|proof| proof.contributors.contains(&contributor));
        let final_contributor_count = retained_contributor_count
            .checked_add(usize::from(new_contributor))
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        if u64::try_from(final_contributor_count)
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
            > self.limits.max_entry_contributors
        {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }

        let checked = self
            .facts
            .declarations
            .get(&declaration)
            .is_some_and(|proof| proof.checked);
        let needs_pending = new_contributor || !checked;
        let insert_pending = needs_pending && !self.pending.contains_key(&declaration);
        if needs_pending && !self.pending.contains_key(&declaration) {
            if u64::try_from(self.pending.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                >= self.limits.max_declarations
            {
                return Err(FinalSemanticAnalysisError::AccountingOverflow);
            }
        }

        let new_contributor_count = u64::from(new_contributor);
        let required_work = u64::from(new_declaration)
            .checked_add(new_contributor_count)
            .and_then(|work| work.checked_add(new_contributor_count))
            .and_then(|work| work.checked_add(u64::from(insert_pending)))
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        self.require_charge_capacity(required_work)?;

        if new_declaration {
            self.charge(1)?;
            let previous = self.facts.declarations.insert(
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
            self.charge(1)?;
            let inserted = self
                .facts
                .declarations
                .get_mut(&declaration)
                .expect("the declaration row was retained or inserted after complete preflight")
                .contributors
                .insert(contributor);
            debug_assert!(inserted);

            self.charge(1)?;
            if insert_pending {
                let delta = BTreeSet::from([contributor]);
                self.charge(1)?;
                let previous = self.pending.insert(declaration, delta);
                debug_assert!(previous.is_none());
            } else {
                let delta = self
                    .pending
                    .get_mut(&declaration)
                    .expect("pending delta exists when a declaration is already queued");
                let inserted = delta.insert(contributor);
                debug_assert!(inserted);
            }
        } else if insert_pending {
            self.charge(1)?;
            let previous = self.pending.insert(declaration, BTreeSet::new());
            debug_assert!(previous.is_none());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn cache_adjacency(
        &mut self,
        declaration: CallableDeclarationKey,
        adjacency: Box<[CallableDeclarationKey]>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.adjacency.contains_key(&declaration) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let edge_count = u64::try_from(adjacency.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let next_edge_count = self
            .cached_edge_count
            .checked_add(edge_count)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        if next_edge_count > self.limits.max_edges {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }
        self.require_charge_capacity(edge_count)?;
        self.charge(edge_count)?;
        self.insert_precharged_adjacency(declaration, adjacency, next_edge_count)
    }

    fn reserve_adjacency_edges(&self, amount: u64) -> Result<u64, FinalSemanticAnalysisError> {
        let next = self
            .cached_edge_count
            .checked_add(amount)
            .ok_or(FinalSemanticAnalysisError::AccountingOverflow)?;
        if next > self.limits.max_edges {
            return Err(FinalSemanticAnalysisError::AccountingOverflow);
        }
        Ok(next)
    }

    pub(super) fn cache_precharged_adjacency(
        &mut self,
        declaration: CallableDeclarationKey,
        adjacency: Box<[CallableDeclarationKey]>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if self.adjacency.contains_key(&declaration) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let edge_count = u64::try_from(adjacency.len())
            .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
        let next_edge_count = self.reserve_adjacency_edges(edge_count)?;
        self.insert_precharged_adjacency(declaration, adjacency, next_edge_count)
    }

    fn insert_precharged_adjacency(
        &mut self,
        declaration: CallableDeclarationKey,
        adjacency: Box<[CallableDeclarationKey]>,
        next_edge_count: u64,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.cached_edge_count = next_edge_count;
        let previous = self.adjacency.insert(declaration, adjacency);
        debug_assert!(previous.is_none());
        Ok(())
    }

    pub(super) fn adjacency(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<&[CallableDeclarationKey]> {
        self.adjacency.get(declaration).map(Box::as_ref)
    }

    pub(super) fn pop_pending(
        &mut self,
    ) -> Result<Option<(CallableDeclarationKey, Box<[ItemId]>)>, FinalSemanticAnalysisError> {
        if self.pending.is_empty() {
            return Ok(None);
        }
        self.require_charge_capacity(1)?;
        self.charge(1)?;
        let (declaration, retained) = self
            .pending
            .pop_first()
            .expect("pending key remains present after queue preflight");
        let delta = retained.into_iter().collect::<Box<[_]>>();
        Ok(Some((declaration, delta)))
    }
}
