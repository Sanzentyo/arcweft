//! Entry-rooted executable declaration preparation.
//!
//! Event locals can participate in call selection, while selected project
//! calls determine which Event type reaches the callee. This module owns the
//! one deterministic, move-only worklist that interleaves those operations.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::StmtId,
    project::{HirSemanticPathOwnerId, HirSemanticPathRoot},
    source_index::{HirSourceQuery, HirStmtSourceRole},
    stmt::HirStmtKind,
    symbol::{CallableDeclarationKey, CallableDeclarationOwner, ResolvedProjectSymbol},
};

use crate::{
    entry::PreparedEntryRootCatalog,
    final_analysis::{
        PreparedEventScrutineeProof, PreparedIncludeFlowProof, PreparedStatementScrutineeProof,
    },
};

#[cfg(test)]
use crate::types::SemanticTypeDigest;

use super::{
    Analyzer, FinalSemanticAnalysisError, HirCallArgument, HirExprKind, HirModule,
    preparation::simple_binding_source,
};

#[path = "executable_ingress/inventory.rs"]
mod inventory;
#[cfg(test)]
#[path = "executable_ingress/tests.rs"]
mod tests;
#[path = "executable_ingress/verification.rs"]
mod verification;
#[path = "executable_ingress/worklist.rs"]
mod worklist;

pub(in crate::final_analysis::analyzer) use inventory::{
    AdjacencyWorkKind, PreparedExecutableDeclaration, PreparedExecutableDeclarationInventory,
    executable_body_path, extract_declaration_adjacency,
};
#[cfg(test)]
pub(in crate::final_analysis::analyzer) use verification::{
    admit_recomputed_one, charge_recomputed_work, require_recomputed_charge_capacity,
};
pub(in crate::final_analysis::analyzer) use verification::{recompute_ingress, same_ingress_facts};
pub(crate) use worklist::PreparedExecutableIngressFacts;
pub(in crate::final_analysis::analyzer) use worklist::{
    PreparedExecutableIngressWorklist, StatementPreparationLimits,
};
#[derive(Debug)]
pub(crate) struct PreparedExecutableIngressSeal {
    facts: PreparedExecutableIngressFacts,
    roots: PreparedEntryRootCatalog,
    includes: BTreeMap<StmtId, PreparedIncludeFlowProof>,
    events: BTreeMap<StmtId, PreparedEventScrutineeProof>,
    scrutinees: BTreeMap<StmtId, PreparedStatementScrutineeProof>,
}

/// Entry-owned half of the completed ingress transaction.
#[derive(Debug)]
pub(crate) struct PreparedEntryIngressSeal {
    facts: PreparedExecutableIngressFacts,
    roots: PreparedEntryRootCatalog,
    events: BTreeMap<StmtId, PreparedEventScrutineeProof>,
}

/// Statement-owned half of the completed ingress transaction.
#[derive(Debug)]
pub(crate) struct PreparedStatementIngressSeal {
    includes: BTreeMap<StmtId, PreparedIncludeFlowProof>,
    scrutinees: BTreeMap<StmtId, PreparedStatementScrutineeProof>,
}

impl PreparedExecutableIngressSeal {
    #[cfg(test)]
    pub(crate) fn empty_for_call_free_fixture() -> Self {
        Self {
            facts: PreparedExecutableIngressFacts::default(),
            roots: PreparedEntryRootCatalog::default(),
            includes: BTreeMap::new(),
            events: BTreeMap::new(),
            scrutinees: BTreeMap::new(),
        }
    }

    pub(crate) fn into_phase_seals(
        self,
    ) -> (PreparedEntryIngressSeal, PreparedStatementIngressSeal) {
        (
            PreparedEntryIngressSeal {
                facts: self.facts,
                roots: self.roots,
                events: self.events,
            },
            PreparedStatementIngressSeal {
                includes: self.includes,
                scrutinees: self.scrutinees,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn replace_event_digest_for_test(
        &mut self,
        statement: StmtId,
        replacement: SemanticTypeDigest,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let proof =
            self.events
                .remove(&statement)
                .ok_or(FinalSemanticAnalysisError::MissingFact {
                    family: super::SemanticFactFamily::Statement,
                })?;
        let (retained_statement, retained_digest, contributors) = proof.into_parts();
        if retained_statement != statement || retained_digest == replacement {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let previous = self.events.insert(
            statement,
            PreparedEventScrutineeProof::new(statement, replacement, contributors),
        );
        debug_assert!(previous.is_none());
        Ok(())
    }
}

impl PreparedEntryIngressSeal {
    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedExecutableIngressFacts,
        PreparedEntryRootCatalog,
        BTreeMap<StmtId, PreparedEventScrutineeProof>,
    ) {
        (self.facts, self.roots, self.events)
    }
}

impl PreparedStatementIngressSeal {
    pub(crate) fn into_parts(
        self,
    ) -> (
        BTreeMap<StmtId, PreparedIncludeFlowProof>,
        BTreeMap<StmtId, PreparedStatementScrutineeProof>,
    ) {
        (self.includes, self.scrutinees)
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn is_executable_declaration_body_owner(
        &self,
        owner: HirSemanticPathOwnerId,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let location = self
            .topology
            .semantic_path(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        Ok({
            matches!(location.root(), HirSemanticPathRoot::Declaration(_))
                && executable_body_path(location.path().steps())
        })
    }

    /// Prepares Include edges without publishing a parallel target catalog.
    fn prepare_include_flow_proofs(
        &self,
        inventory: &PreparedExecutableDeclarationInventory,
    ) -> Result<BTreeMap<StmtId, PreparedIncludeFlowProof>, FinalSemanticAnalysisError> {
        let mut proofs = BTreeMap::new();
        for declaration in inventory.values() {
            let module = self.module(declaration.module)?;
            for owner in &declaration.statements {
                let statement = module
                    .resolve_stmt(*owner)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                let HirStmtKind::Include(include) = statement.kind() else {
                    continue;
                };
                let reference = include
                    .target()
                    .as_resolved()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                let source = super::statements::source_span(
                    module,
                    HirSourceQuery::Stmt {
                        owner: *owner,
                        role: HirStmtSourceRole::Whole,
                    },
                )?;
                let resolved = self
                    .symbols
                    .resolve_entity_reference(module.key().path(), reference, source)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
                let ResolvedProjectSymbol::StructuralCallable(symbol) = resolved else {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                };
                if symbol.owner() != CallableDeclarationOwner::Flow {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                }
                let CallableDeclarationKey::Flow(target) = symbol.declaration() else {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                };
                let target_key = CallableDeclarationKey::Flow(target.clone());
                if inventory.get(&target_key).is_none() {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                }
                let proof = PreparedIncludeFlowProof::new(
                    *owner,
                    declaration.declaration.clone(),
                    target.clone(),
                );
                if proofs.insert(*owner, proof).is_some() {
                    return Err(FinalSemanticAnalysisError::DuplicateFact {
                        family: super::SemanticFactFamily::Statement,
                    });
                }
            }
        }
        Ok(proofs)
    }

    fn complete_ingress_declaration(
        &mut self,
        declaration: &PreparedExecutableDeclaration,
        ingress: &PreparedExecutableIngressFacts,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.module(declaration.module)?
            .resolve_item(declaration.item)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        super::statement_scrutinee::seed_declaration_scrutinees(
            &self.modules,
            &self.types,
            self.symbols,
            self.catalogs.world.environment().typecheck_env(),
            self.catalogs.world.environment().statement_ingress(),
            self.executable,
            self.topology.as_ref(),
            ingress,
            &declaration.declaration,
            &declaration.statements,
            &mut self.facts,
        )?;
        self.complete_declaration_statements(declaration)?;
        self.validate_declaration_body_result(&declaration.declaration)?;
        self.complete_declaration_expression_roots(declaration)?;
        super::statement_scrutinee::validate_declaration_scrutinees(
            &self.modules,
            &self.types,
            self.catalogs.world.environment().statement_ingress(),
            self.executable,
            self.topology.as_ref(),
            ingress,
            &declaration.declaration,
            &declaration.statements,
            &self.facts,
        )?;
        self.finalize_declaration_locals(declaration)
    }

    fn complete_declaration_statements(
        &mut self,
        declaration: &PreparedExecutableDeclaration,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let statements = {
            let module = self.module(declaration.module)?;
            declaration
                .statements
                .iter()
                .map(|owner| {
                    module
                        .resolve_stmt(*owner)
                        .map(|statement| (*owner, statement.kind().clone()))
                        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        for (owner, statement) in statements {
            self.control.check()?;
            let module = self
                .modules
                .get(&owner.module())
                .copied()
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            for request in super::statement_scrutinee::expression_requests_for_statement(
                module, owner, &statement,
            )? {
                let expected = request.expected();
                self.check_expression_published(request.owner(), expected.as_ref())?;
            }
            super::statement_scrutinee::seed_dynamic_scrutinee(
                module,
                &self.types,
                self.symbols,
                self.catalogs.world.environment().typecheck_env(),
                owner,
                &statement,
                &mut self.facts,
            )?;
            if let Some((pattern, initializer, annotation)) = simple_binding_source(&statement) {
                self.infer_simple_statement_binding(owner, pattern, initializer, annotation)?;
            } else {
                self.infer_control_statement_bindings(owner, statement)?;
            }
        }
        Ok(())
    }

    fn complete_declaration_expression_roots(
        &mut self,
        declaration: &PreparedExecutableDeclaration,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut contextual_arguments = BTreeSet::new();
        for owner in &declaration.expressions {
            let expression = self
                .module(owner.module())?
                .resolve_expr(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            if let HirExprKind::Call(call) = expression.kind() {
                contextual_arguments.extend(call.arguments().iter().map(HirCallArgument::value));
            }
        }
        let expression_set = declaration
            .expressions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let children = declaration
            .expressions
            .iter()
            .map(|owner| {
                self.module(owner.module())?
                    .resolve_expr(*owner)
                    .map(|expression| expression.kind().direct_expression_children())
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .filter(|owner| expression_set.contains(owner))
            .collect::<BTreeSet<_>>();
        for owner in declaration
            .expressions
            .iter()
            .copied()
            .filter(|owner| !children.contains(owner))
        {
            self.check_expression_published(owner, None)?;
        }
        for owner in contextual_arguments {
            if !self.facts.expressions().contains_key(&owner) {
                self.check_expression_published(owner, None)?;
            }
        }
        Ok(())
    }

    fn finalize_declaration_locals(
        &mut self,
        declaration: &PreparedExecutableDeclaration,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let module = self.module(declaration.module)?;
        let view = self
            .topology
            .declaration(&declaration.declaration)
            .map_err(|_| FinalSemanticAnalysisError::InvalidCallableOwner)?;
        let locals = module
            .locals()
            .filter_map(|(owner, local)| {
                view.paths().local(owner).and_then(|path| {
                    executable_body_path(path.steps()).then(|| (owner, local.clone()))
                })
            })
            .collect::<Vec<_>>();
        for (owner, local) in locals {
            if local.is_poisoned() {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            }
            if self.facts.locals().contains_key(&owner) {
                continue;
            }
            let inferred = local
                .pattern()
                .and_then(|pattern| self.pattern_type_hint(module, pattern))
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner })?;
            self.facts
                .set_local_type(owner, inferred)
                .map_err(FinalSemanticAnalysisError::from)?;
        }
        Ok(())
    }

    pub(super) fn complete_contextual_declarations(
        &mut self,
        roots: PreparedEntryRootCatalog,
    ) -> Result<PreparedExecutableIngressSeal, FinalSemanticAnalysisError> {
        let inventory = PreparedExecutableDeclarationInventory::build(self)?;
        let includes = self.prepare_include_flow_proofs(&inventory)?;
        let limits = StatementPreparationLimits::production(&inventory, &roots, includes.len())?;
        let mut worklist =
            PreparedExecutableIngressWorklist::new(&inventory, roots, includes, limits)?;
        super::statement_scrutinee::seed_non_event_scrutinees(
            &self.modules,
            &self.types,
            self.symbols,
            self.catalogs.world.environment().typecheck_env(),
            self.catalogs.world.environment().statement_ingress(),
            self.executable,
            self.topology.as_ref(),
            &worklist.facts,
            &inventory,
            &mut self.facts,
        )?;
        self.drive_ingress_worklist(&inventory, &mut worklist)?;
        self.reject_unreached_event_declarations(&inventory, &worklist.facts)?;
        self.complete_unreached_declarations(&inventory, &mut worklist)?;
        let recomputed = recompute_ingress(
            &inventory,
            &worklist.roots,
            &worklist.includes,
            self,
            worklist.limits,
        )?;
        if !same_ingress_facts(&worklist.facts, &recomputed) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let scrutinees =
            super::statement_scrutinee::prepare_scrutinee_proofs(&self.modules, &inventory)?;
        let events = self.prepare_event_scrutinee_proofs(&inventory, &worklist.facts)?;
        Ok(PreparedExecutableIngressSeal {
            facts: worklist.facts,
            roots: worklist.roots,
            includes: worklist.includes,
            events,
            scrutinees,
        })
    }

    fn drive_ingress_worklist(
        &mut self,
        inventory: &PreparedExecutableDeclarationInventory,
        worklist: &mut PreparedExecutableIngressWorklist,
    ) -> Result<(), FinalSemanticAnalysisError> {
        while let Some((declaration, contributors)) = worklist.pop_pending()? {
            let checked = worklist
                .facts
                .declarations
                .get(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                .checked;
            if !checked {
                let row = inventory
                    .get(&declaration)
                    .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
                worklist.charge(
                    u64::try_from(row.statements.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                self.complete_ingress_declaration(row, &worklist.facts)?;
                worklist
                    .facts
                    .declarations
                    .get_mut(&declaration)
                    .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                    .checked = true;
                self.cache_declaration_adjacency(&declaration, inventory, worklist)?;
            }
            if worklist.adjacency(&declaration).is_none() {
                return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
            }
            let event_type = worklist
                .facts
                .declarations
                .get(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                .event_type;
            let event_digest = worklist
                .facts
                .event_digest(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
            let edge_count = worklist
                .adjacency(&declaration)
                .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                .len();
            for edge_index in 0..edge_count {
                for contributor in contributors.iter().copied() {
                    worklist.charge(1)?;
                    let target = worklist
                        .adjacency(&declaration)
                        .and_then(|edges| edges.get(edge_index))
                        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?
                        .clone();
                    worklist.admit_one(target, event_type, event_digest, contributor)?;
                }
            }
        }
        Ok(())
    }

    fn cache_declaration_adjacency(
        &self,
        declaration: &CallableDeclarationKey,
        inventory: &PreparedExecutableDeclarationInventory,
        worklist: &mut PreparedExecutableIngressWorklist,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut targets = Vec::new();
        let includes = std::mem::take(&mut worklist.includes);
        let extracted = extract_declaration_adjacency(
            self,
            declaration,
            inventory,
            &includes,
            |_kind: AdjacencyWorkKind| worklist.charge(1),
            |target| {
                targets.push(target);
                Ok(())
            },
        );
        worklist.includes = includes;
        extracted?;
        targets.sort();
        worklist.cache_precharged_adjacency(declaration.clone(), targets.into_boxed_slice())
    }

    fn reject_unreached_event_declarations(
        &self,
        inventory: &PreparedExecutableDeclarationInventory,
        facts: &PreparedExecutableIngressFacts,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for declaration in inventory.values() {
            if declaration.contains_event_scrutinee(&self.modules)?
                && !facts.declarations.contains_key(&declaration.declaration)
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
        Ok(())
    }

    fn complete_unreached_declarations(
        &mut self,
        inventory: &PreparedExecutableDeclarationInventory,
        worklist: &mut PreparedExecutableIngressWorklist,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for declaration in inventory.values() {
            let already_checked = worklist
                .facts
                .declarations
                .get(&declaration.declaration)
                .is_some_and(|proof| proof.checked);
            if !already_checked {
                worklist.charge(
                    u64::try_from(declaration.statements.len())
                        .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
                )?;
                self.complete_ingress_declaration(declaration, &worklist.facts)?;
                if let Some(proof) = worklist
                    .facts
                    .declarations
                    .get_mut(&declaration.declaration)
                {
                    proof.checked = true;
                }
            }
            if worklist.adjacency(&declaration.declaration).is_none() {
                self.cache_declaration_adjacency(&declaration.declaration, inventory, worklist)?;
            }
        }
        Ok(())
    }

    fn prepare_event_scrutinee_proofs(
        &self,
        inventory: &PreparedExecutableDeclarationInventory,
        facts: &PreparedExecutableIngressFacts,
    ) -> Result<BTreeMap<StmtId, PreparedEventScrutineeProof>, FinalSemanticAnalysisError> {
        let mut events = BTreeMap::new();
        for declaration in inventory.values() {
            let Some(proof) = facts.declarations.get(&declaration.declaration) else {
                continue;
            };
            let module = self.module(declaration.module)?;
            for statement in &declaration.statements {
                let kind = module
                    .resolve_stmt(*statement)
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .kind();
                if !super::statement_scrutinee::has_event_scrutinee(kind) {
                    continue;
                }
                let event = PreparedEventScrutineeProof::new(
                    *statement,
                    proof.event_digest,
                    proof
                        .contributors
                        .iter()
                        .copied()
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );
                if events.insert(*statement, event).is_some() {
                    return Err(FinalSemanticAnalysisError::DuplicateFact {
                        family: super::SemanticFactFamily::Statement,
                    });
                }
            }
        }
        Ok(events)
    }
}
