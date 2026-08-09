//! Immutable accepted semantic report and publication transaction.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_lang_hir::{
    item::{HirItemKind, HirVisibility},
    scope::HirScopeKind,
    source_index::{
        HirDeclarationSourceRole, HirExprSourceRole, HirItemSourceRole, HirSourcePresence,
        HirSourceQuery, HirSourceSite,
    },
};
use arcweft_source::{Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceSpan};

#[cfg(test)]
use std::sync::atomic::AtomicBool;

use super::{
    CallTargetFacts, CaptureId, CheckedBinding, CheckedCallableCatalog, CheckedExpression,
    CheckedItem, CheckedPattern, CheckedStatement, ExprId, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, FinalSemanticAnalysisWork,
    HirExecutableProjectView, HirModule, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId,
    ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId, SemanticFactFamily, StmtId,
    TypeId, TypeKind, TypeResolutionReport,
    validation::{
        SemanticFactInventory, accepted_type_owners, collect_unique, collect_work,
        validate_bindings, validate_calls, validate_complete_inventory, validate_expressions,
        validate_items, validate_patterns, validate_physical_candidate_argument_evaluations,
        validate_statements, validate_symbol_generation, validate_types,
    },
};

#[cfg(test)]
use super::PhysicalCandidateArgumentEvaluation;

/// Immutable semantic analysis bound to one exact accepted HIR generation.
#[derive(Clone, Debug)]
pub struct FinalSemanticAnalysis {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    symbol_world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    checked_callables: Arc<CheckedCallableCatalog>,
    types: BTreeMap<TypeId, TypeKind>,
    type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
    locals: BTreeMap<LocalId, CheckedBinding>,
    captures: BTreeMap<CaptureId, CheckedBinding>,
    expressions: BTreeMap<ExprId, CheckedExpression>,
    patterns: BTreeMap<PatternId, CheckedPattern>,
    statements: BTreeMap<StmtId, CheckedStatement>,
    items: BTreeMap<ItemId, CheckedItem>,
    calls: BTreeMap<ExprId, CallTargetFacts>,
    diagnostics: Arc<[Diagnostic]>,
    #[cfg(test)]
    physical_candidate_argument_evaluations:
        BTreeMap<ExprId, Arc<[PhysicalCandidateArgumentEvaluation]>>,
    work: FinalSemanticAnalysisWork,
}

impl FinalSemanticAnalysis {
    /// Validates and publishes a complete semantic generation.
    #[cfg(test)]
    pub(crate) fn try_new(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let cancellation = AtomicBool::new(false);
        Self::try_new_with_control(
            project,
            symbols,
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
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        Self::try_new_with_control_and_type_resolutions(
            project,
            symbols,
            checked_callables,
            input,
            BTreeMap::new(),
            control,
        )
    }

    /// Publishes the semantic type products created by the sole production
    /// nominal resolver with the same accepted generation as their flattened
    /// type facts. Manual fact fixtures deliberately use the constructor above
    /// and therefore cannot fabricate nominal-reference evidence.
    pub(super) fn try_new_with_control_and_type_resolutions(
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        checked_callables: Arc<CheckedCallableCatalog>,
        input: FinalSemanticAnalysisInput,
        type_resolutions: BTreeMap<TypeId, TypeResolutionReport>,
        control: FinalSemanticAnalysisControl<'_>,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        control.check()?;
        validate_symbol_generation(project, symbols)?;
        checked_callables
            .validate_project_generation(symbols.world(), *symbols.revision())
            .map_err(|_| FinalSemanticAnalysisError::CatalogGenerationMismatch)?;
        let (modules, snapshots) = project_generation_maps(project);
        let dialogue_lines = project.dialogue_lines();

        let types = collect_unique(input.types, SemanticFactFamily::Type)?;
        let locals = collect_unique(input.locals, SemanticFactFamily::Local)?;
        control.check()?;
        let captures = collect_unique(input.captures, SemanticFactFamily::Capture)?;
        control.check()?;
        let expressions = collect_unique(input.expressions, SemanticFactFamily::Expression)?;
        control.check()?;
        let patterns = collect_unique(input.patterns, SemanticFactFamily::Pattern)?;
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
        let physical_candidate_argument_evaluations = input.physical_candidate_argument_evaluations;
        control.check()?;

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
        validate_complete_inventory(&modules, inventory, &type_resolutions)?;
        control.check()?;
        validate_types(&modules, &types, &calls, &type_resolutions)?;
        control.check()?;
        validate_bindings(&modules, &locals, &captures)?;
        control.check()?;
        validate_expressions(
            symbols,
            &modules,
            dialogue_lines,
            &expressions,
            &calls,
            &type_resolutions,
        )?;
        control.check()?;
        validate_patterns(symbols, &modules, &patterns)?;
        control.check()?;
        validate_statements(&modules, &statements)?;
        control.check()?;
        validate_items(&modules, &items)?;
        control.check()?;
        validate_calls(symbols, &modules, &expressions, &calls, &type_resolutions)?;
        validate_physical_candidate_argument_evaluations(
            &modules,
            &physical_candidate_argument_evaluations,
        )?;
        let work = collect_work(inventory)?;
        let diagnostics = collect_final_diagnostics(&modules, &types, &expressions, &items)?;
        control.check()?;
        Ok(Self {
            snapshots,
            symbol_world: symbols.world().clone(),
            symbol_revision: *symbols.revision(),
            checked_callables,
            types,
            type_resolutions,
            locals,
            captures,
            expressions,
            patterns,
            statements,
            items,
            calls,
            diagnostics: diagnostics.into(),
            #[cfg(test)]
            physical_candidate_argument_evaluations,
            work,
        })
    }

    /// Rejects reuse with any missing, foreign, or stale module generation.
    pub fn validate_generation(
        &self,
        project: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), FinalSemanticAnalysisError> {
        validate_symbol_generation(project, symbols)?;
        self.checked_callables
            .validate_project_generation(symbols.world(), *symbols.revision())
            .map_err(|_| FinalSemanticAnalysisError::CatalogGenerationMismatch)?;
        let actual = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect::<BTreeMap<_, _>>();
        if actual == self.snapshots
            && symbols.world() == &self.symbol_world
            && symbols.revision() == &self.symbol_revision
        {
            Ok(())
        } else {
            Err(FinalSemanticAnalysisError::GenerationMismatch)
        }
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
        if symbols.world() != &self.symbol_world
            || symbols.revision() != &self.symbol_revision
            || self.snapshots.get(&module.module_id()) != Some(&module.snapshot_id())
        {
            return Err(FinalSemanticAnalysisError::GenerationMismatch);
        }
        Ok(())
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

fn project_generation_maps(
    project: HirExecutableProjectView<'_>,
) -> (
    BTreeMap<HirModuleId, &HirModule>,
    BTreeMap<HirModuleId, HirSnapshotId>,
) {
    let modules = project
        .modules()
        .map(|(_, module)| (module.module_id(), module.as_ref()))
        .collect::<BTreeMap<_, _>>();
    let snapshots = modules
        .iter()
        .map(|(id, module)| (*id, module.snapshot_id()))
        .collect();
    (modules, snapshots)
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
