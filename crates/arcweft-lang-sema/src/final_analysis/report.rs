//! Immutable accepted semantic report and publication transaction.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    item::{HirItemKind, HirVisibility},
    project::HirProjectEvaluationTopology,
    scope::HirScopeKind,
    source_index::{
        HirDeclarationSourceRole, HirExprSourceRole, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};
use thiserror::Error;

#[cfg(test)]
use std::sync::atomic::AtomicBool;

use super::PhysicalCandidateArgumentEvaluation;
use super::match_edges;
use super::{
    CallTargetFacts, CaptureId, CheckedBinding, CheckedCallableCatalog, CheckedExpression,
    CheckedItem, CheckedPattern, CheckedStatement, ExprId, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, FinalSemanticAnalysisWork,
    FinalSemanticProjectError, HirExecutableProjectView, HirModule, HirModuleId, ItemId, LocalId,
    PatternId, ProjectSymbolTable, SemanticFactFamily, StmtId, TypeId, TypeKind,
    TypeResolutionReport,
    validation::{
        SemanticFactInventory, accepted_type_owners, collect_unique, collect_work,
        validate_bindings, validate_calls, validate_complete_inventory, validate_expressions,
        validate_items, validate_patterns, validate_physical_candidate_argument_evaluations,
        validate_statements, validate_types,
    },
};
use crate::callable::{CheckedCallCalleeExecution, CheckedCallReceiverProjection, CheckedCallSite};
use crate::entry::CheckedEntryCatalog;
use crate::semantic_coordinate::AcceptedSemanticRootCatalog;

use super::nominal_schema::RuntimeNominalProjectionCatalog;
use super::nominal_semantic::{ProjectNominalSemanticCatalog, ProjectNominalSemanticDefinition};
use super::semantic_shapes::AcceptedSemanticShapeCatalog;

/// Immutable semantic analysis bound to one exact accepted HIR generation.
#[derive(Clone, Debug)]
pub struct FinalSemanticAnalysis {
    checked_callables: Arc<CheckedCallableCatalog>,
    accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    checked_entries: CheckedEntryCatalog,
    project_nominals: ProjectNominalSemanticCatalog,
    semantic_shapes: AcceptedSemanticShapeCatalog,
    runtime_nominals: RuntimeNominalProjectionCatalog,
    types: BTreeMap<TypeId, TypeKind>,
    type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    locals: BTreeMap<LocalId, CheckedBinding>,
    captures: BTreeMap<CaptureId, CheckedBinding>,
    expressions: BTreeMap<ExprId, CheckedExpression>,
    patterns: BTreeMap<PatternId, CheckedPattern>,
    statements: BTreeMap<StmtId, CheckedStatement>,
    items: BTreeMap<ItemId, CheckedItem>,
    calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) edge_facts: BTreeMap<
        ExprId,
        Result<super::CheckedExpressionEdgeFact, super::CheckedExpressionEdgeError>,
    >,
    diagnostics: Arc<[Diagnostic]>,
    #[cfg(test)]
    physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    work: FinalSemanticAnalysisWork,
}

/// Typed runtime-emission disposition for one checked expression.
///
/// Structural expressions do not participate in the callable application
/// graph. Ordinary final-HIR calls carry the selected callee disposition
/// derived from their sealed call application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedExpressionRuntimeDisposition {
    Structural,
    Call(CheckedCallRuntimeCalleeDisposition),
}

/// Runtime callee handling selected by one sealed ordinary call application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallRuntimeCalleeDisposition {
    Static,
    RuntimeReceiver,
}

/// Failure to join one checked expression's call resolution with its exact
/// final call fact.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CheckedExpressionRuntimeDispositionError {
    #[error("checked expression {owner:?} is absent from the final analysis")]
    MissingExpression { owner: ExprId },
    #[error("checked HirCall expression {owner:?} has no call fact")]
    MissingCallFacts { owner: ExprId },
    #[error("checked HirCall expression {owner:?} has no selected call application")]
    UnselectedCall { owner: ExprId },
    #[error(
        "checked HirCall expression {owner:?} has a mismatched call fact site: expected {expected:?}, actual {actual:?}"
    )]
    CallSiteMismatch {
        owner: ExprId,
        expected: CheckedCallSite,
        actual: CheckedCallSite,
    },
}

/// Complete, unpublished semantic generation awaiting the consuming Entry and
/// runtime-nominal seal. This owner is deliberately non-Clone.
pub(crate) struct FinalSemanticAnalysisDraft {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) ingress: super::PreparedExecutableIngressSeal,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
}

/// Disjoint moved draft state used while the nominal context borrows only the
/// accepted type map.
pub(crate) struct FinalSemanticAnalysisDraftParts {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) ingress: super::PreparedExecutableIngressSeal,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
}

impl FinalSemanticAnalysisDraft {
    pub(crate) fn into_parts(self) -> FinalSemanticAnalysisDraftParts {
        let Self {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            physical_candidate_argument_evaluations,
        } = self;
        FinalSemanticAnalysisDraftParts {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            physical_candidate_argument_evaluations,
        }
    }
}

/// Post-Entry unpublished state. The full executable ingress worklist has
/// already been split and consumed; only the affine statement half remains.
pub(crate) struct FinalSemanticAnalysisPostEntryDraft {
    pub(super) checked_callables: Arc<CheckedCallableCatalog>,
    pub(super) accepted_roots: Arc<AcceptedSemanticRootCatalog>,
    pub(super) types: BTreeMap<TypeId, TypeKind>,
    pub(super) type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    pub(super) locals: BTreeMap<LocalId, CheckedBinding>,
    pub(super) captures: BTreeMap<CaptureId, CheckedBinding>,
    pub(super) expressions: BTreeMap<ExprId, super::PreparedExpressionFact>,
    pub(super) patterns: BTreeMap<PatternId, super::PreparedPatternFact>,
    pub(super) statements: BTreeMap<StmtId, super::PreparedStatementPayload>,
    pub(super) items: BTreeMap<ItemId, CheckedItem>,
    pub(super) calls: BTreeMap<ExprId, CallTargetFacts>,
    pub(super) callable_joins: super::match_edges::PreparedCallableJoins,
    pub(super) selected_expressions: super::match_edges::CheckedSelectedExpressionGraph,
    pub(super) structural_edges: super::match_edges::CheckedStructuralEdgeDraft,
    pub(super) statement_ingress: super::PreparedStatementIngressSeal,
    pub(super) physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
}

impl FinalSemanticAnalysisDraftParts {
    pub(crate) fn into_post_entry(
        self,
    ) -> (
        super::PreparedEntryIngressSeal,
        FinalSemanticAnalysisPostEntryDraft,
    ) {
        let FinalSemanticAnalysisDraftParts {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            physical_candidate_argument_evaluations,
        } = self;
        let (entry_ingress, statement_ingress) = ingress.into_phase_seals();
        (
            entry_ingress,
            FinalSemanticAnalysisPostEntryDraft {
                checked_callables,
                accepted_roots,
                types,
                type_resolutions,
                locals,
                captures,
                expressions,
                patterns,
                statements,
                items,
                calls,
                callable_joins,
                selected_expressions,
                structural_edges,
                statement_ingress,
                physical_candidate_argument_evaluations,
            },
        )
    }
}

impl FinalSemanticAnalysisPostEntryDraft {
    pub(crate) fn seal(
        self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_entries: CheckedEntryCatalog,
        project_nominals: ProjectNominalSemanticCatalog,
        semantic_shapes: AcceptedSemanticShapeCatalog,
        runtime_nominals: RuntimeNominalProjectionCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
        let Self {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions: prepared_expressions,
            patterns: prepared_patterns,
            statements: prepared_statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            statement_ingress,
            physical_candidate_argument_evaluations,
        } = self;
        control.check()?;
        let expressions = collect_sealed_expressions(prepared_expressions)?;
        let patterns = collect_sealed_patterns(prepared_patterns)?;
        let evaluation_topology = Arc::clone(accepted_roots.topology());
        let modules = project_generation_modules(project);
        let dialogue_lines = project.dialogue_lines();
        let completed = {
            let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
                accepted_roots.as_ref(),
                &structural_edges,
            );
            let payloads = super::statement_seal::CheckedStatementSeal::new(
                prepared_statements,
                statement_ingress,
                &locals,
                checked_callables.as_ref(),
                &coordinates,
                project,
            );
            super::statement_effects::seal_statement_effects(
                super::statement_effects::StatementEffectSealInput {
                    modules: &modules,
                    topology: evaluation_topology.as_ref(),
                    selected: &selected_expressions,
                    calls: &calls,
                    callables: checked_callables.as_ref(),
                    expressions,
                    payloads,
                    control,
                },
            )?
        };
        let (expressions, statements) = completed.into_parts();
        validate_checked_entry_references(&expressions, &checked_entries)?;

        let type_owners = if type_resolutions.is_empty() {
            None
        } else {
            Some(accepted_type_owners(&modules, &expressions, &calls)?)
        };
        validate_type_resolution_reports(
            &modules,
            type_owners.as_ref(),
            &types,
            &type_resolutions,
        )?;
        control.check()?;

        let inventory = SemanticFactInventory {
            types: &types,
            locals: &locals,
            captures: &captures,
            expressions: &expressions,
            patterns: &patterns,
            statements: &statements,
            items: &items,
            calls: &calls,
        };
        validate_complete_inventory(
            evaluation_topology.as_ref(),
            &modules,
            &selected_expressions,
            inventory,
            &type_resolutions,
        )?;
        control.check()?;
        validate_types(&modules, &types)?;
        control.check()?;
        validate_bindings(&modules, &locals, &captures)?;
        control.check()?;
        validate_expressions(
            symbols,
            &evaluation_topology,
            &modules,
            dialogue_lines,
            &expressions,
            &calls,
        )?;
        control.check()?;
        validate_patterns(symbols, &modules, &types, &patterns)?;
        control.check()?;
        validate_statements(&modules, &locals, &statements, &calls)?;
        control.check()?;
        validate_items(&modules, &items)?;
        control.check()?;
        validate_calls(symbols, &modules, &expressions, &calls)?;
        validate_physical_candidate_argument_evaluations(
            &modules,
            &physical_candidate_argument_evaluations,
        )?;
        let work = collect_work(inventory)?;
        let diagnostics = collect_final_diagnostics(&modules, &types, &expressions, &items)?;
        let (edge_facts, unconsumed_callable_joins) =
            structural_edges.into_final_facts(&calls, callable_joins);
        if !unconsumed_callable_joins.is_empty() {
            return Err(FinalSemanticAnalysisError::CheckedCallableCatalog);
        }
        control.check()?;
        Ok(FinalSemanticAnalysis {
            checked_callables,
            accepted_roots,
            checked_entries,
            project_nominals,
            semantic_shapes,
            runtime_nominals,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            edge_facts,
            diagnostics: diagnostics.into(),
            #[cfg(test)]
            physical_candidate_argument_evaluations,
            work,
        })
    }
}

fn validate_checked_entry_references(
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    entries: &CheckedEntryCatalog,
) -> Result<(), FinalSemanticAnalysisError> {
    for expression in expressions.values() {
        let super::CheckedExpressionResolution::Value(super::CheckedValueResolution::Entry(
            reference,
        )) = expression.resolution()
        else {
            continue;
        };
        let binding = entries
            .get_public(reference.diagnostic_public_id())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        if binding.source_item() != reference.lookup_owner()
            || binding.binding_digest() != reference.binding()
            || expression.ty().semantic_identity_digest() != reference.value_type()
            || expression.ty() != &TypeKind::entity_ref(crate::types::EntityKind::Entry)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    Ok(())
}

impl FinalSemanticAnalysis {
    #[cfg(test)]
    pub(super) const fn accepted_types(&self) -> &BTreeMap<TypeId, TypeKind> {
        &self.types
    }

    #[allow(
        dead_code,
        reason = "used only by the crate-private Cut 2 ownership classifier until Cut 5 publication"
    )]
    pub(crate) fn matches_symbol_lease(&self, symbols: &ProjectSymbolTable) -> bool {
        symbols.world() == self.hir_generation().symbol_world()
            && *symbols.revision() == self.hir_generation().symbol_revision()
    }

    /// Validates and publishes a complete semantic generation.
    #[cfg(test)]
    pub(crate) fn try_new(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let cancellation = AtomicBool::new(false);
        Self::try_new_with_control(
            project,
            symbols,
            topology,
            checked_callables,
            input,
            FinalSemanticAnalysisControl::new(&cancellation),
        )
    }

    /// Validates and publishes a complete semantic generation while observing
    /// caller-owned cancellation at every publication phase boundary.
    #[cfg(test)]
    pub(crate) fn try_new_with_control(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        Self::try_new_with_control_and_type_resolutions(
            project,
            symbols,
            topology,
            checked_callables,
            input,
            BTreeMap::new(),
            control,
        )
    }

    /// Test-only publication keeps a single topology lease without exposing a
    /// constructor that can mint accepted roots in production.
    #[cfg(test)]
    pub(crate) fn try_new_with_control_and_type_resolutions(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        topology: Arc<HirProjectEvaluationTopology>,
        checked_callables: Arc<CheckedCallableCatalog>,
        mut input: FinalSemanticAnalysisInput,
        type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        control.check()?;
        let mut item_facts = BTreeMap::new();
        for (item, fact) in &input.items {
            if item_facts.insert(*item, fact).is_some() {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: SemanticFactFamily::Item,
                });
            }
        }
        let accepted_roots = Arc::new(AcceptedSemanticRootCatalog::seal(
            Arc::clone(&topology),
            &checked_callables,
            &item_facts,
        )?);
        let modules = project_generation_modules(project);
        let prepared_expressions = collect_unique(
            input.expressions.iter().cloned(),
            SemanticFactFamily::Expression,
        )?;
        let expressions = prepared_expressions;
        let selected_expressions =
            match_edges::CheckedSelectedExpressionGraph::seal_call_free_fixture(
                project,
                Arc::clone(&topology),
                &expressions,
            )?;
        input.set_structural_edges(match_edges::CheckedStructuralEdgeDraft::seal(
            &selected_expressions,
            &modules,
            &expressions,
        ))?;
        input.set_selected_expressions(selected_expressions)?;
        Self::try_new_with_control_and_type_resolutions_and_catalog(
            project,
            symbols,
            checked_callables,
            input,
            type_resolutions,
            accepted_roots,
            AcceptedSemanticShapeCatalog::default(),
            control,
        )
        .map_err(FinalSemanticProjectError::into_semantic_fixture_error)
    }

    /// Publishes the semantic type products created by the sole production
    /// nominal resolver with the same accepted generation as their flattened
    /// type facts. Manual fact fixtures deliberately use the constructor above
    /// and therefore cannot fabricate nominal-reference evidence.
    pub(super) fn try_new_with_control_and_type_resolutions_and_catalog(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_callables: Arc<CheckedCallableCatalog>,
        mut input: FinalSemanticAnalysisInput,
        type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
        accepted_roots: Arc<AcceptedSemanticRootCatalog>,
        semantic_shapes: AcceptedSemanticShapeCatalog,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticProjectError> {
        control.check()?;
        let observed = project
            .accept_symbol_generation(symbols)
            .map_err(|_| FinalSemanticAnalysisError::SymbolGenerationMismatch)?;
        if !accepted_roots
            .topology()
            .generation()
            .same_generation(observed.generation().as_ref())
        {
            return Err(FinalSemanticAnalysisError::GenerationMismatch.into());
        }
        let callable_generation = checked_callables
            .hir_generation()
            .ok_or(FinalSemanticAnalysisError::CatalogGenerationMismatch)?;
        if !Arc::ptr_eq(accepted_roots.topology().generation(), callable_generation) {
            return Err(FinalSemanticAnalysisError::CatalogGenerationMismatch.into());
        }
        let evaluation_topology = Arc::clone(accepted_roots.topology());
        let types = collect_unique(input.types, SemanticFactFamily::Type)?;
        let locals = collect_unique(input.locals, SemanticFactFamily::Local)?;
        control.check()?;
        let captures = collect_unique(input.captures, SemanticFactFamily::Capture)?;
        control.check()?;
        let prepared_expressions =
            collect_unique(input.expressions, SemanticFactFamily::Expression)?;
        let selected_expressions = input
            .selected_expressions
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if !Arc::ptr_eq(selected_expressions.topology(), &evaluation_topology) {
            return Err(FinalSemanticAnalysisError::GenerationMismatch.into());
        }
        let structural_edges = input
            .structural_edges
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let ingress = input
            .ingress_seal
            .take()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        control.check()?;
        let prepared_patterns = collect_unique(input.patterns, SemanticFactFamily::Pattern)?;
        control.check()?;
        let statements = collect_unique(input.statements, SemanticFactFamily::Statement)?;
        control.check()?;
        let items = collect_unique(input.items, SemanticFactFamily::Item)?;
        control.check()?;
        let calls = collect_unique(
            input
                .calls
                .into_iter()
                .map(|call| (call.expression(), call)),
            SemanticFactFamily::Call,
        )?;
        let callable_joins = input.callable_joins;
        match_edges::validate_callable_join_inventory(&calls, &callable_joins)
            .map_err(|error| FinalSemanticAnalysisError::CheckedCallableJoin(Box::new(error)))?;
        let physical_candidate_argument_evaluations = input.physical_candidate_argument_evaluations;
        control.check()?;
        let draft = FinalSemanticAnalysisDraft {
            checked_callables,
            accepted_roots,
            types,
            type_resolutions,
            locals,
            captures,
            expressions: prepared_expressions,
            patterns: prepared_patterns,
            statements,
            items,
            calls,
            callable_joins,
            selected_expressions,
            structural_edges,
            ingress,
            physical_candidate_argument_evaluations,
        };
        super::nominal_schema::seal_runtime_nominal_draft(
            draft,
            project,
            symbols,
            semantic_shapes,
            control,
        )
    }

    /// Rejects reuse with any missing, foreign, or stale module generation.
    pub fn validate_generation(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let observed = project
            .accept_symbol_generation(symbols)
            .map_err(|_| FinalSemanticAnalysisError::SymbolGenerationMismatch)?;
        self.hir_generation()
            .same_generation(observed.generation().as_ref())
            .then_some(())
            .ok_or(FinalSemanticAnalysisError::GenerationMismatch)
    }

    /// Validates one module-scoped query against this report's exact accepted
    /// snapshot and symbol generation. This does not publish or reconstruct a
    /// partial analysis; it is only a lease check for consumers that already
    /// hold the immutable project report.
    pub fn validate_module_generation(
        &self,
        module: &HirModule,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.hir_generation()
            .validate_module_lease(module, symbols)
            .map_err(|_| FinalSemanticAnalysisError::GenerationMismatch)
    }

    pub fn hir_generation(&self) -> &Arc<arcweft_lang_hir::project::AcceptedHirProjectGeneration> {
        self.accepted_roots.topology().generation()
    }

    pub fn validate_registered_callable_authority(
        &self,
        registered: &crate::callable::RegisteredCallableCatalog,
    ) -> Result<(), crate::callable::CheckedCallableLookupError> {
        self.checked_callables
            .validate_registered_authority(registered, self.hir_generation().as_ref())
    }

    pub fn hir_topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        self.accepted_roots.topology()
    }

    /// Sole checked Entry catalog accepted by this semantic generation.
    pub const fn checked_entries(&self) -> &CheckedEntryCatalog {
        &self.checked_entries
    }

    pub(crate) const fn runtime_nominals(&self) -> &RuntimeNominalProjectionCatalog {
        &self.runtime_nominals
    }

    /// Layout-free accepted semantics for one exact project nominal type.
    pub(crate) fn project_nominal_semantic(
        &self,
        semantic_type: crate::types::SemanticTypeDigest,
    ) -> Option<&ProjectNominalSemanticDefinition> {
        self.project_nominals.get(semantic_type)
    }

    /// Complete layout-free checked variant owner for one project semantic type.
    pub fn project_variant_owner(
        &self,
        semantic_type: crate::types::SemanticTypeDigest,
    ) -> Option<super::CheckedVariantOwner> {
        let definition = self.project_nominals.get(semantic_type)?;
        let cases = definition.cases()?;
        super::CheckedVariantOwner::try_project_shapes(
            definition.nominal().clone(),
            cases.iter().map(|case| {
                (
                    case.payload().clone(),
                    Some(case.diagnostic_name().to_owned()),
                )
            }),
        )
    }

    pub(crate) const fn semantic_shapes(&self) -> &AcceptedSemanticShapeCatalog {
        &self.semantic_shapes
    }

    pub fn ty(&self, owner: TypeId) -> Option<&TypeKind> {
        self.types.get(&owner)
    }

    /// Complete nominal-resolution product for one exact final-HIR type root.
    ///
    /// Production reports contain one row for every structural type root.
    /// Hand-built test fixtures intentionally return `None` instead of
    /// inventing source or alias evidence.
    pub fn type_resolution(&self, owner: TypeId) -> Option<&TypeResolutionReport> {
        self.type_resolutions.get(&owner)
    }

    pub fn type_resolutions(
        &self,
    ) -> impl ExactSizeIterator<Item = (TypeId, &TypeResolutionReport)> {
        self.type_resolutions
            .iter()
            .map(|(owner, report)| (*owner, report))
    }

    pub fn local(&self, owner: LocalId) -> Option<&CheckedBinding> {
        self.locals.get(&owner)
    }

    pub fn capture(&self, owner: CaptureId) -> Option<&CheckedBinding> {
        self.captures.get(&owner)
    }

    pub fn expression(&self, owner: ExprId) -> Option<&CheckedExpression> {
        self.expressions.get(&owner)
    }

    /// Returns the sole typed runtime-emission disposition for one checked
    /// expression. Only a checked final-HIR Call joins the call-fact ledger;
    /// dialogue, view, style, and other structural expressions remain
    /// structural at this boundary.
    pub fn runtime_expression_disposition(
        &self,
        owner: ExprId,
    ) -> Result<CheckedExpressionRuntimeDisposition, CheckedExpressionRuntimeDispositionError> {
        let expression = self
            .expression(owner)
            .ok_or(CheckedExpressionRuntimeDispositionError::MissingExpression { owner })?;
        let Some(site) = expression.resolution().checked_call_site(owner) else {
            return Ok(CheckedExpressionRuntimeDisposition::Structural);
        };
        let CheckedCallSite::HirCall(call) = site else {
            return Ok(CheckedExpressionRuntimeDisposition::Structural);
        };
        let expected = CheckedCallSite::HirCall(owner);
        if call != owner {
            return Err(CheckedExpressionRuntimeDispositionError::CallSiteMismatch {
                owner,
                expected,
                actual: site,
            });
        }
        let facts = self
            .call(owner)
            .ok_or(CheckedExpressionRuntimeDispositionError::MissingCallFacts { owner })?;
        let actual = facts.outcome().site();
        if actual != expected {
            return Err(CheckedExpressionRuntimeDispositionError::CallSiteMismatch {
                owner,
                expected,
                actual,
            });
        }
        let application = facts
            .selected_application()
            .ok_or(CheckedExpressionRuntimeDispositionError::UnselectedCall { owner })?;
        let application_site = application.core().site();
        if application_site != expected {
            return Err(CheckedExpressionRuntimeDispositionError::CallSiteMismatch {
                owner,
                expected,
                actual: application_site,
            });
        }
        let callee = if matches!(
            application.core().callee(),
            CheckedCallCalleeExecution::Value { .. }
        ) || matches!(
            application.core().execution().receiver(),
            CheckedCallReceiverProjection::Operand { .. }
        ) {
            CheckedCallRuntimeCalleeDisposition::RuntimeReceiver
        } else {
            CheckedCallRuntimeCalleeDisposition::Static
        };
        Ok(CheckedExpressionRuntimeDisposition::Call(callee))
    }

    pub fn pattern(&self, owner: PatternId) -> Option<&CheckedPattern> {
        self.patterns.get(&owner)
    }

    pub fn statement(&self, owner: StmtId) -> Option<&CheckedStatement> {
        self.statements.get(&owner)
    }

    pub fn item(&self, owner: ItemId) -> Option<&CheckedItem> {
        self.items.get(&owner)
    }

    pub fn call(&self, owner: ExprId) -> Option<&CallTargetFacts> {
        self.calls.get(&owner)
    }

    /// Sole immutable checked callable/effect authority accepted with this
    /// semantic generation.
    pub const fn checked_callables(&self) -> &Arc<CheckedCallableCatalog> {
        &self.checked_callables
    }

    /// Sole accepted-root authority retained by this immutable report.
    pub(crate) const fn accepted_root_catalog(&self) -> &Arc<AcceptedSemanticRootCatalog> {
        &self.accepted_roots
    }

    pub fn types(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeKind)> {
        self.types.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn locals(&self) -> impl ExactSizeIterator<Item = (LocalId, &CheckedBinding)> {
        self.locals.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn captures(&self) -> impl ExactSizeIterator<Item = (CaptureId, &CheckedBinding)> {
        self.captures.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn expressions(&self) -> impl ExactSizeIterator<Item = (ExprId, &CheckedExpression)> {
        self.expressions.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn patterns(&self) -> impl ExactSizeIterator<Item = (PatternId, &CheckedPattern)> {
        self.patterns.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn statements(&self) -> impl ExactSizeIterator<Item = (StmtId, &CheckedStatement)> {
        self.statements.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn items(&self) -> impl ExactSizeIterator<Item = (ItemId, &CheckedItem)> {
        self.items.iter().map(|(id, fact)| (*id, fact))
    }

    pub fn calls(&self) -> impl ExactSizeIterator<Item = (ExprId, &CallTargetFacts)> {
        self.calls.iter().map(|(id, fact)| (*id, fact))
    }

    /// Ordered per-root operational trace used by the sema acceptance matrix.
    /// This remains crate-owned and is not projected into language tooling.
    #[cfg(test)]
    pub(crate) fn physical_candidate_argument_evaluations(
        &self,
    ) -> impl Iterator<Item = &PhysicalCandidateArgumentEvaluation> {
        self.physical_candidate_argument_evaluations
            .values()
            .flat_map(|evaluations| evaluations.iter())
    }

    pub const fn work(&self) -> FinalSemanticAnalysisWork {
        self.work
    }

    /// Diagnostics remain owned by their exact call facts; this projection
    /// does not copy them into a positional side table.
    pub fn call_diagnostics(&self) -> impl Iterator<Item = &crate::callable::CallableDiagnostic> {
        self.calls
            .values()
            .flat_map(|call| call.diagnostics().iter())
    }

    /// Source-backed warnings accepted with this exact semantic generation.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

fn collect_sealed_expressions(
    prepared: BTreeMap<ExprId, super::PreparedExpressionFact>,
) -> Result<BTreeMap<ExprId, CheckedExpression>, FinalSemanticAnalysisError> {
    prepared
        .into_iter()
        .map(|(owner, fact)| {
            fact.into_complete()
                .map(|fact| (owner, fact))
                .map_err(|_| FinalSemanticAnalysisError::UnsealedPreparedC2Owner)
        })
        .collect()
}

fn collect_sealed_patterns(
    prepared: BTreeMap<PatternId, super::PreparedPatternFact>,
) -> Result<BTreeMap<PatternId, CheckedPattern>, FinalSemanticAnalysisError> {
    prepared
        .into_iter()
        .map(|(owner, fact)| {
            fact.into_complete()
                .map(|fact| (owner, fact))
                .map_err(|_| FinalSemanticAnalysisError::UnsealedPreparedC2Owner)
        })
        .collect()
}

fn project_generation_modules(
    project: HirExecutableProjectView<'_>,
) -> BTreeMap<HirModuleId, &HirModule> {
    project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect()
}

fn collect_final_diagnostics(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &BTreeMap<ExprId, CheckedExpression>,
    items: &BTreeMap<ItemId, CheckedItem>,
) -> Result<Vec<Diagnostic>, FinalSemanticAnalysisError> {
    let mut diagnostics = Vec::new();
    for (owner, checked) in expressions {
        if checked.type_selection() != super::CheckedTypeSelection::DefaultNumericFallback {
            continue;
        }
        let module = resolve_module(modules, owner.module())?;
        let expression = module
            .resolve_expr(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if !scope_is_nested_in_closure(module, expression.scope())? {
            continue;
        }
        let span = source_span(
            module,
            HirSourceQuery::Expr {
                owner: *owner,
                role: HirExprSourceRole::Whole,
            },
        )?;
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                format!(
                    "unsuffixed numeric literal inside inferred closure body defaults to {}; add a suffix or closure return type to make the contract explicit",
                    checked.ty().source_label()
                ),
            )
            .with_code("sema.numeric.fallback_in_inferred_closure")
            .with_label(DiagnosticLabel::primary(
                span,
                Some("default numeric type selected here".to_owned()),
            )),
        );
    }
    for (owner, checked) in items {
        if !matches!(checked.role(), super::CheckedItemRole::TypeAlias) {
            continue;
        }
        let module = resolve_module(modules, owner.module())?;
        let item = module
            .resolve_item(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirItemKind::TypeAlias(alias) = item.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if item.prefix().visibility() != Some(HirVisibility::Public)
            || !matches!(types.get(&alias.target()), Some(TypeKind::Choice(_)))
        {
            continue;
        }
        let name = alias
            .name()
            .resolved()
            .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
        let span = source_span(
            module,
            HirSourceQuery::Item {
                owner: *owner,
                role: HirItemSourceRole::Declaration(HirDeclarationSourceRole::Name),
            },
        )?;
        let ty = types
            .get(&alias.target())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                format!(
                    "public type alias `{}` exposes anonymous sum `{}`; public ABI and save data are more stable with a nominal enum",
                    name.as_str(),
                    ty.source_label()
                ),
            )
            .with_code("sema.public_abi.anonymous_sum")
            .with_label(DiagnosticLabel::primary(
                span,
                Some("public anonymous sum type".to_owned()),
            )),
        );
    }
    Ok(diagnostics)
}

fn scope_is_nested_in_closure(
    module: &HirModule,
    mut scope: arcweft_lang_hir::identity::ScopeId,
) -> Result<bool, FinalSemanticAnalysisError> {
    loop {
        let current = module
            .resolve_scope(scope)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if current.kind() == HirScopeKind::Closure {
            return Ok(true);
        }
        let Some(parent) = current.parent() else {
            return Ok(false);
        };
        scope = parent;
    }
}

fn resolve_module<'a>(
    modules: &BTreeMap<HirModuleId, &'a HirModule>,
    owner: HirModuleId,
) -> Result<&'a HirModule, FinalSemanticAnalysisError> {
    modules
        .get(&owner)
        .copied()
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)
}

fn source_span(
    module: &HirModule,
    query: HirSourceQuery,
) -> Result<SourceSpan, FinalSemanticAnalysisError> {
    let lookup = module
        .source_site(module.provenance().source_identity(), query)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}

fn validate_type_resolution_reports(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    accepted_owners: Option<&BTreeSet<TypeId>>,
    types: &BTreeMap<TypeId, TypeKind>,
    reports: &BTreeMap<TypeId, TypeResolutionReport>,
) -> Result<(), FinalSemanticAnalysisError> {
    if reports.is_empty() {
        return Ok(());
    }
    let all_nodes = accepted_owners.cloned().unwrap_or_else(|| {
        modules
            .values()
            .flat_map(|module| module.types().map(|(owner, _)| owner))
            .collect::<BTreeSet<_>>()
    });
    let mut node_facts = BTreeMap::new();
    for (owner, report) in reports {
        let product = report.outcome().product();
        if product.root() != *owner {
            return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner: *owner });
        }
        for node in product.nodes() {
            if node.is_contextual_alias_target() {
                continue;
            }
            if !all_nodes.contains(&node.node()) {
                return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch {
                    owner: node.node(),
                });
            }
            let recovered = node.recovered().cloned();
            merge_type_resolution_fact(&mut node_facts, node.node(), &recovered)?;
        }
    }
    let covered = node_facts.keys().copied().collect::<BTreeSet<_>>();
    let recovered = node_facts
        .into_iter()
        .filter_map(|(owner, ty)| ty.map(|ty| (owner, ty)))
        .collect::<BTreeMap<_, _>>();
    if covered != all_nodes || recovered != *types {
        let owner = all_nodes
            .iter()
            .find(|owner| !covered.contains(owner) || recovered.get(owner) != types.get(owner))
            .copied()
            .or_else(|| {
                types
                    .keys()
                    .find(|owner| !recovered.contains_key(owner))
                    .copied()
            })
            .unwrap_or_else(|| *reports.keys().next().expect("non-empty report inventory"));
        return Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner });
    }
    Ok(())
}

pub(super) fn merge_type_resolution_fact<T: Clone + Eq>(
    facts: &mut BTreeMap<TypeId, T>,
    owner: TypeId,
    fact: &T,
) -> Result<(), FinalSemanticAnalysisError> {
    match facts.entry(owner) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(fact.clone());
            Ok(())
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == fact => Ok(()),
        std::collections::btree_map::Entry::Occupied(_) => {
            Err(FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner })
        }
    }
}
