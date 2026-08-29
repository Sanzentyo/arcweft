//! Complete executable declaration inventory and typed adjacency extraction.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::{ExprId, HirModuleId, ItemId, StmtId},
    project::HirSemanticPathStep,
    symbol::CallableDeclarationKey,
};

use crate::callable::CallableCandidateId;
use crate::final_analysis::PreparedIncludeFlowProof;

use super::{Analyzer, FinalSemanticAnalysisError, HirModule};

#[derive(Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) struct PreparedExecutableDeclaration {
    pub(super) declaration: CallableDeclarationKey,
    pub(super) module: HirModuleId,
    pub(super) item: ItemId,
    pub(super) statements: Box<[StmtId]>,
    pub(super) expressions: Box<[ExprId]>,
}

impl PreparedExecutableDeclaration {
    pub(in crate::final_analysis::analyzer) const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub(in crate::final_analysis::analyzer) const fn statements(&self) -> &[StmtId] {
        &self.statements
    }

    pub(in crate::final_analysis::analyzer) fn contains_event_scrutinee(
        &self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let module = modules
            .get(&self.module)
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        self.statements.iter().try_fold(false, |found, owner| {
            let statement = module
                .resolve_stmt(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            Ok(found || super::super::statement_scrutinee::has_event_scrutinee(statement.kind()))
        })
    }
}

/// Sole declaration/body inventory used by Event reachability.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) struct PreparedExecutableDeclarationInventory {
    pub(super) declarations: BTreeMap<CallableDeclarationKey, PreparedExecutableDeclaration>,
}

impl PreparedExecutableDeclarationInventory {
    pub(in crate::final_analysis::analyzer) fn build(
        analyzer: &Analyzer<'_, '_, '_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut declarations = BTreeMap::new();
        for module_topology in analyzer.topology.modules() {
            let module = analyzer.module(module_topology.module())?;
            for entry in module_topology.entries() {
                let Some(body) = entry.body() else {
                    continue;
                };
                if body.roots().is_empty() {
                    continue;
                }
                let declaration = body.declaration().clone();
                let mut statements = module
                    .statements()
                    .filter_map(|(owner, _)| {
                        body.paths().statement(owner).and_then(|path| {
                            executable_body_path(path.steps()).then_some((path.steps(), owner))
                        })
                    })
                    .collect::<Vec<_>>();
                statements.sort_by(|(left, _), (right, _)| left.cmp(right));
                let statements = statements
                    .into_iter()
                    .map(|(_, owner)| owner)
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let expressions = module
                    .expressions()
                    .filter_map(|(owner, _)| {
                        body.paths()
                            .expression(owner)
                            .and_then(|path| executable_body_path(path.steps()).then_some(owner))
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                let row = PreparedExecutableDeclaration {
                    declaration: declaration.clone(),
                    module: module.module_id(),
                    item: body.source_item(),
                    statements,
                    expressions,
                };
                if declarations.insert(declaration, row).is_some() {
                    return Err(FinalSemanticAnalysisError::InvalidCallableOwner);
                }
            }
        }
        Ok(Self { declarations })
    }

    pub(in crate::final_analysis::analyzer) fn get(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Option<&PreparedExecutableDeclaration> {
        self.declarations.get(declaration)
    }

    pub(in crate::final_analysis::analyzer) fn values(
        &self,
    ) -> impl ExactSizeIterator<Item = &PreparedExecutableDeclaration> {
        self.declarations.values()
    }

    pub(in crate::final_analysis::analyzer) fn len(&self) -> usize {
        self.declarations.len()
    }
}

pub(in crate::final_analysis::analyzer) fn executable_body_path(
    steps: &[HirSemanticPathStep],
) -> bool {
    matches!(steps.first(), Some(HirSemanticPathStep::DeclarationBody(_)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::final_analysis::analyzer) enum AdjacencyWorkKind {
    CallCandidate,
    IncludeCandidate,
    RetainedEdge,
}

/// Visits the completed declaration's selected project-call and Include edges
/// exactly once for one ingress traversal. The visitor receives one value per
/// typed outgoing edge, including duplicate calls to the same declaration;
/// callers charge before retaining each edge and propagate each
/// `(edge, contributor)` pair independently.
pub(in crate::final_analysis::analyzer) fn extract_declaration_adjacency(
    analyzer: &Analyzer<'_, '_, '_>,
    source: &CallableDeclarationKey,
    inventory: &PreparedExecutableDeclarationInventory,
    includes: &BTreeMap<arcweft_lang_hir::identity::StmtId, PreparedIncludeFlowProof>,
    mut charge: impl FnMut(AdjacencyWorkKind) -> Result<(), FinalSemanticAnalysisError>,
    mut visit: impl FnMut(CallableDeclarationKey) -> Result<(), FinalSemanticAnalysisError>,
) -> Result<(), FinalSemanticAnalysisError> {
    let declaration = inventory
        .get(source)
        .ok_or(FinalSemanticAnalysisError::InvalidCallableOwner)?;
    let graph = analyzer
        .facts
        .prepared_calls()
        .map_err(FinalSemanticAnalysisError::from)?;
    for owner in &declaration.expressions {
        charge(AdjacencyWorkKind::CallCandidate)?;
        let Some(fact) = analyzer.facts.expressions().get(owner) else {
            // Structural HIR carriers intentionally publish no expression
            // fact. A checked call site, when present, is still required to
            // resolve to one exact live prepared-graph node below.
            continue;
        };
        let Some(site) = fact.checked_call_site(*owner) else {
            continue;
        };
        let target = graph
            .project_site_payload(
                site,
                |prefix| match prefix.application().selected().id() {
                    CallableCandidateId::Project(target) => Some(target.clone()),
                    _ => None,
                },
                |_unselected| None,
            )
            .ok_or(FinalSemanticAnalysisError::CandidateFactTransaction {
                violation:
                    crate::final_analysis::CandidateFactTransactionViolation::PreparedCallGraph(
                        crate::callable::CallConstraintInvariant::MissingOrStalePreparedNode.into(),
                    ),
            })?;
        let Some(target) = target else {
            continue;
        };
        if inventory.get(&target).is_some() {
            charge(AdjacencyWorkKind::RetainedEdge)?;
            visit(target)?;
        }
    }
    for owner in &declaration.statements {
        charge(AdjacencyWorkKind::IncludeCandidate)?;
        let Some(proof) = includes.get(owner) else {
            continue;
        };
        if proof.source() != source {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        charge(AdjacencyWorkKind::RetainedEdge)?;
        visit(CallableDeclarationKey::Flow(proof.target().clone()))?;
    }
    Ok(())
}
