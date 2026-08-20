//! Runtime-plan lowering from one accepted final-HIR project generation.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::effect::{RuntimeArtifactFingerprint, RuntimeAssertionProfile};
use arcweft_core::entry::{RuntimeCallableId, RuntimeCallableRole, RuntimeFlowExecutable};
use arcweft_core::line_task::{ChildCancelPolicy, ChildJoinPolicy, LineCleanupPolicy};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeAwaitPendingObserverSeed, RuntimeChoiceOptionSeed,
    RuntimeDialogueContentPlanSeedId, RuntimeEffectFieldSeed, RuntimeEntryKind, RuntimeEntrySpec,
    RuntimeEvaluatedEffectSeed, RuntimeExprSeed, RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed,
    RuntimeFlowSeed, RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostTaskRequestTemplateSeed, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed,
    RuntimeLineTaskGroupSeed, RuntimeLineTaskNodeSeed, RuntimeLineTaskTriggerSeed,
    RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePlan, RuntimePlanBuilder, RuntimePureHelperDeclarationSeed, RuntimePureHelperOrigin,
    RuntimePureHelperSeedId, RuntimePureInputType, RuntimePureOutputType, RuntimeReceiverMode,
    RuntimeTraitMethodDeclarationSeed, RuntimeTraitMethodIdentity, RuntimeTraitMethodSeedId,
};
use arcweft_core::task::{HostCapabilityId, NeedId, TaskId, TaskOutcomeContract, TaskPriority};
use arcweft_core::time::LogicalDuration;
use arcweft_core::value::{RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue};
use arcweft_lang_hir::expr::{
    HirChoiceCompactAction, HirChoiceItem, HirExprKind, HirThreadBody, HirThreadFlowItem,
    HirThreadMode,
};
use arcweft_lang_hir::identity::{ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, StmtId};
use arcweft_lang_hir::item::{
    HirEntryDeclaration, HirEntryId, HirEntryKind, HirFunctionBody, HirFunctionItem,
    HirFunctionParameterGroup, HirImplFunction, HirImplMember, HirItemKind, HirMethodParameter,
    HirMethodParameterGroup, HirMethodReceiverKind, HirParameter, HirParameterKind,
};
use arcweft_lang_hir::leaf::HirIdRef;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::project::HirExecutableProjectView;
use arcweft_lang_hir::source_index::{
    HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
};
use arcweft_lang_hir::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirStmtKind,
    HirStmtMatchArmBody,
};
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallablePackageId,
    ImplMethodDeclarationId,
};
use arcweft_source::SourceSpan;
use arcweft_text_model::DialogueContentCatalog;

use crate::assertion_identity::{
    AssertionConditionIndex, AssertionPresentation, RuntimeAssertionInventory,
    RuntimeAssertionMode, RuntimeAssertionSite,
};
use crate::errors::RuntimePlanLowerError;
use crate::final_expr::FinalExprLowerer;
use crate::final_pattern::FinalPatternLowerer;
use crate::semantic_facts::{
    RuntimeAssertionAdmission, RuntimeAwaitFact, RuntimeDialogueApplication,
    RuntimeDialogueEffectTrigger, RuntimeEvaluatedEffect, RuntimeIteratorFact,
    RuntimeIteratorWitnessExecutableFact, RuntimeNormalizedType, RuntimePlanSemanticFacts,
    RuntimeResolvedCallTarget, RuntimeResolvedValue, RuntimeSemanticFactsError,
    RuntimeTraitIdentity, RuntimeTraitMethodFact, RuntimeTryBoundaryOwner, RuntimeTryCarrierFact,
    RuntimeTryFact, RuntimeTypeShape,
};
use arcweft_text_model::{RichTextControl, RichTextNode};

/// Final-HIR owner and checked runtime Entry metadata admitted by semantic analysis.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCheckedEntryInput {
    owner: ItemId,
    entry: RuntimeEntrySpec,
}

impl RuntimeCheckedEntryInput {
    pub const fn new(owner: ItemId, entry: RuntimeEntrySpec) -> Self {
        Self { owner, entry }
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn entry(&self) -> &RuntimeEntrySpec {
        &self.entry
    }
}

/// Runtime body family selected for one checked ordinary-function Entry role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeEntryCallableBody {
    PureHelper,
    ControllerFlow(arcweft_core::plan::FlowRuntimeId),
}

/// Exact callable identity, final-HIR owner, and checked role metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntryCallableInput {
    declaration: CallableDeclarationKey,
    owner: ItemId,
    role: RuntimeCallableRole,
    body: RuntimeEntryCallableBody,
}

impl RuntimeEntryCallableInput {
    pub const fn new(
        declaration: CallableDeclarationKey,
        owner: ItemId,
        role: RuntimeCallableRole,
        body: RuntimeEntryCallableBody,
    ) -> Self {
        Self {
            declaration,
            owner,
            role,
            body,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn role(&self) -> &RuntimeCallableRole {
        &self.role
    }

    pub const fn body(&self) -> &RuntimeEntryCallableBody {
        &self.body
    }
}

/// Exact final-HIR Flow owner and checked executable role metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntryFlowInput {
    owner: ItemId,
    executable: RuntimeFlowExecutable,
}

impl RuntimeEntryFlowInput {
    pub const fn new(owner: ItemId, executable: RuntimeFlowExecutable) -> Self {
        Self { owner, executable }
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn executable(&self) -> &RuntimeFlowExecutable {
        &self.executable
    }
}

/// Generation-bound checked Entry projection consumed by final runtime lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEntryLoweringInput {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    entries: Vec<RuntimeCheckedEntryInput>,
    callables: Vec<RuntimeEntryCallableInput>,
    flows: Vec<RuntimeEntryFlowInput>,
}

impl RuntimeEntryLoweringInput {
    pub fn new(
        project: HirExecutableProjectView<'_>,
        entries: Vec<RuntimeCheckedEntryInput>,
        callables: Vec<RuntimeEntryCallableInput>,
        flows: Vec<RuntimeEntryFlowInput>,
    ) -> Self {
        let snapshots = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect();
        Self {
            snapshots,
            entries,
            callables,
            flows,
        }
    }

    pub fn empty(project: HirExecutableProjectView<'_>) -> Self {
        Self::new(project, Vec::new(), Vec::new(), Vec::new())
    }

    fn validate_generation(&self, project: HirExecutableProjectView<'_>) -> bool {
        self.snapshots
            == project
                .modules()
                .map(|(_, module)| (module.module_id(), module.snapshot_id()))
                .collect()
    }
}

/// Runtime-plan lowering result plus lowering-time counters.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlanLowerReport {
    pub plan: RuntimePlan,
    pub stats: RuntimePlanLowerStats,
    pub dialogue_content_catalog: DialogueContentCatalog,
    pub character_presentation_catalog: Option<Arc<CharacterPresentationCatalogData>>,
    assertion_sites: Box<[RuntimeAssertionSite]>,
}

impl RuntimePlanLowerReport {
    /// Binds the fresh-session assertion sites to the exact completed
    /// runtime-plan artifact identity.
    ///
    /// The fingerprint is copied from the existing runtime-plan `ArtifactKey`
    /// by the compiler/cache owner after plan construction. It never
    /// participates in guard derivation and the returned inventory is never
    /// serialized.
    ///
    /// # Panics
    ///
    /// Panics only if this already-validated report contains duplicate guard
    /// identities, which would violate its construction invariant.
    pub fn bind_assertion_inventory(
        &self,
        artifact: RuntimeArtifactFingerprint,
    ) -> RuntimeAssertionInventory {
        RuntimeAssertionInventory::try_new(artifact, self.assertion_sites.iter().cloned())
            .expect("runtime-plan lowering validated unique assertion guards")
    }

    /// Number of runtime-capable assertion conditions retained for a fresh
    /// compiler session. Debug assertions omitted by the selected profile and
    /// proof-only assertions are absent.
    pub const fn assertion_site_count(&self) -> usize {
        self.assertion_sites.len()
    }
}

/// Runtime-plan counters retained by compiler and profile output.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimePlanLowerStats {
    pub pure_helpers: usize,
    pub pure_candidate_functions_seen: usize,
    pub pure_candidate_lower_attempts: usize,
    pub pure_candidate_lower_failures_inferred: usize,
    pub pure_expr_lowered_nodes: usize,
    pub pure_expr_cloned_nodes: usize,
    pub pure_rewrite_expr_visits: usize,
    pub optimized_flows: usize,
    pub optimized_op_slices: usize,
    pub local_use_tail_scans: usize,
    pub local_use_scan_ops: usize,
    pub sequence_map_sum_fusions: usize,
    pub map_sum_fusions: usize,
    pub sequence_source_inlines: usize,
    pub pure_call_exprs: usize,
}

#[derive(Clone)]
struct ReservedFunctionSiteDefinition {
    owner: ExprId,
    module: HirModuleId,
    body: ExprId,
    site: RuntimeFunctionSiteSeedId,
    implicit_parameter: Option<RuntimeLocalSeedId>,
}

#[derive(Clone)]
struct ReservedPureHelperDefinition {
    owner: ItemId,
    helper: RuntimePureHelperSeedId,
}

#[derive(Clone)]
struct ReservedTraitMethodDefinition {
    checked: RuntimeTraitMethodFact,
    method: RuntimeTraitMethodSeedId,
}

#[derive(Clone)]
struct PendingDialogueValueDefinition {
    expression: ExprId,
    site: RuntimeFunctionSiteSeedId,
    body: RuntimeExprSeed,
}

#[derive(Clone)]
struct PendingDialogueContentDefinition {
    owner: ExprId,
    line: arcweft_core::plan::RuntimeLineId,
    values: Vec<(
        arcweft_core::runtime_id::RuntimeDialogueValueSlotId,
        arcweft_core::plan::RuntimeDialogueValueRole,
        RuntimeFunctionSiteSeedId,
    )>,
    marks: Box<[String]>,
    effects: Vec<PendingDialogueEffectDefinition>,
}

#[derive(Clone)]
struct PendingDialogueEffectDefinition {
    trigger: RuntimeDialogueEffectTrigger,
    target: arcweft_core::plan::RuntimeHostCallTargetSeed,
}

struct LoweredControllerCallable {
    flow: RuntimeFlowSeed,
    flow_executable: RuntimeFlowExecutable,
    executable: arcweft_core::plan::RuntimeCallableExecutableSeed,
    assertions: Vec<RuntimeAssertionSite>,
}

struct FinalLoweringContext<'project, 'data> {
    project: HirExecutableProjectView<'project>,
    facts: &'data RuntimePlanSemanticFacts,
    locals: &'data BTreeMap<LocalId, RuntimeLocalSeedId>,
    pure_helpers: &'data BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    trait_methods: &'data BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'data BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    dialogue_content: &'data BTreeMap<ExprId, RuntimeDialogueContentPlanSeedId>,
    await_locals: &'data BTreeMap<ExprId, AwaitLocalSeeds>,
    try_locals: &'data BTreeMap<ExprId, TryLocalSeeds>,
    pipe_locals: &'data BTreeMap<ExprId, RuntimeLocalSeedId>,
}

#[derive(Clone)]
struct AwaitLocalSeeds {
    payload: RuntimeLocalSeedId,
}

#[derive(Clone)]
pub(crate) struct TryLocalSeeds {
    pub(crate) success: RuntimeLocalSeedId,
    pub(crate) residual: Option<RuntimeLocalSeedId>,
}

impl FinalLoweringContext<'_, '_> {
    fn expr_lowerer<'a>(&'a self, module: &'a HirModule) -> FinalExprLowerer<'a> {
        FinalExprLowerer::new(
            module,
            self.facts,
            self.locals,
            self.pure_helpers,
            self.trait_methods,
            self.function_sites,
            (self.pipe_locals, self.try_locals),
        )
    }
}

/// Lowers one exact accepted HIR generation and its checked semantic facts.
#[allow(
    clippy::too_many_lines,
    reason = "this function is the single transactional authority switch that validates and publishes one complete runtime plan"
)]
pub fn lower_runtime_plan_with_stats(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    entry_input: &RuntimeEntryLoweringInput,
) -> Result<RuntimePlanLowerReport, Vec<RuntimePlanLowerError>> {
    facts
        .validate_generation(project)
        .map_err(|error| vec![semantic_fact_error(&error)])?;
    if !entry_input.validate_generation(project) {
        return Err(vec![RuntimePlanLowerError::new(
            "checked runtime Entry input belongs to a different accepted HIR generation",
        )]);
    }

    let type_seeds = facts
        .runtime_plan_type_seeds()
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let local_facts = facts.local_declarations().collect::<Vec<_>>();
    let mut local_seeds = local_facts
        .iter()
        .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(ty.identity()))
        .collect::<Vec<_>>();
    let await_facts = facts.awaits().collect::<Vec<_>>();
    for (expression, _) in &await_facts {
        let payload = facts.expression_type(**expression).ok_or_else(|| {
            vec![RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no accepted payload type"
            ))]
        })?;
        local_seeds.push(RuntimeLocalDeclarationSeed::new(payload.identity()));
    }
    let try_facts = facts.tries().collect::<Vec<_>>();
    for (_, tried) in &try_facts {
        local_seeds.push(RuntimeLocalDeclarationSeed::new(
            tried.carrier().success().identity(),
        ));
        if let Some(residual) = tried.carrier().residual() {
            local_seeds.push(RuntimeLocalDeclarationSeed::new(residual.identity()));
        }
    }
    let implicit_callable_facts = facts.implicit_callables().collect::<Vec<_>>();
    for (_, callable) in &implicit_callable_facts {
        local_seeds.push(RuntimeLocalDeclarationSeed::new(
            callable.parameter().identity(),
        ));
    }
    let pipe_facts = facts.pipes().collect::<Vec<_>>();
    for (_, pipe) in &pipe_facts {
        let left = facts.expression_type(pipe.left()).ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "checked pipe left type is missing during local admission",
            )]
        })?;
        local_seeds.push(RuntimeLocalDeclarationSeed::new(left.identity()));
    }
    let mut builder = RuntimePlanBuilder::new();
    let admission = builder
        .admit_semantic_batch(
            type_seeds,
            local_seeds,
            facts.runtime_plan_nominal_record_domain_seeds(),
            facts.runtime_plan_variant_domain_seeds(),
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let locals = local_facts
        .iter()
        .map(|(local, _)| *local)
        .zip(admission.local_ids().iter().cloned())
        .collect::<BTreeMap<_, _>>();
    let await_locals = await_facts
        .iter()
        .zip(admission.local_ids()[local_facts.len()..].iter())
        .map(|((expression, _), locals)| {
            (
                **expression,
                AwaitLocalSeeds {
                    payload: locals.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut admitted_try_locals = admission.local_ids()[local_facts.len() + await_facts.len()..]
        .iter()
        .cloned();
    let mut try_locals = BTreeMap::new();
    for (expression, tried) in &try_facts {
        let success = admitted_try_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted Try success local is missing",
            )]
        })?;
        let residual = if tried.carrier().residual().is_some() {
            Some(admitted_try_locals.next().ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "admitted Result Try residual local is missing",
                )]
            })?)
        } else {
            None
        };
        try_locals.insert(**expression, TryLocalSeeds { success, residual });
    }
    let implicit_parameters = implicit_callable_facts
        .iter()
        .map(|(expression, _)| **expression)
        .map(|expression| {
            admitted_try_locals
                .next()
                .map(|local| (expression, local))
                .ok_or_else(|| {
                    vec![RuntimePlanLowerError::new(
                        "admitted implicit-callable parameter local is missing",
                    )]
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let pipe_locals = pipe_facts
        .iter()
        .map(|(expression, _)| **expression)
        .map(|expression| {
            admitted_try_locals
                .next()
                .map(|local| (expression, local))
                .ok_or_else(|| {
                    vec![RuntimePlanLowerError::new(
                        "admitted once-only pipe local is missing",
                    )]
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    debug_assert!(admitted_try_locals.next().is_none());
    let mut errors = Vec::new();
    let (function_sites, function_definitions) = reserve_function_sites(
        project,
        facts,
        &locals,
        &implicit_parameters,
        &mut builder,
        &mut errors,
    );
    let (pure_helpers, pure_definitions) = reserve_entry_pure_helpers(
        project,
        facts,
        entry_input,
        &locals,
        &mut builder,
        &mut errors,
    );
    let (pure_helpers, pure_definitions) = reserve_called_project_helpers(
        project,
        facts,
        &locals,
        pure_helpers,
        pure_definitions,
        &mut builder,
        &mut errors,
    );
    let (trait_methods, trait_definitions) =
        reserve_trait_methods(project, facts, &locals, &mut builder, &mut errors);
    let empty_dialogue_content = BTreeMap::new();
    let context = FinalLoweringContext {
        project,
        facts,
        locals: &locals,
        pure_helpers: &pure_helpers,
        trait_methods: &trait_methods,
        function_sites: &function_sites,
        dialogue_content: &empty_dialogue_content,
        await_locals: &await_locals,
        try_locals: &try_locals,
        pipe_locals: &pipe_locals,
    };

    define_function_sites(&context, &function_definitions, &mut builder, &mut errors);
    define_pure_helpers(&context, &pure_definitions, &mut builder, &mut errors);
    define_trait_methods(&context, &trait_definitions, &mut builder, &mut errors);
    let dialogue_content = lower_dialogue_content(&context, &mut builder, &mut errors);
    let context = FinalLoweringContext {
        dialogue_content: &dialogue_content,
        ..context
    };

    let mut entry_owners = collect_entry_inputs(entry_input, &mut errors);
    let mut flow_seeds = Vec::new();
    let mut assertion_sites = Vec::new();
    for item in project.items() {
        match item.item().kind() {
            HirItemKind::Flow(flow) => {
                let Some(identity) = facts.flow(item.id()).cloned() else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "checked runtime Flow identity is missing for final-HIR item {:?}",
                        item.id()
                    )));
                    continue;
                };
                let params = flow
                    .parameters()
                    .iter()
                    .flat_map(HirParameter::locals)
                    .map(|local| {
                        locals.get(local).cloned().ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "runtime Flow {identity} parameter {local:?} has no admitted local"
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>();
                let params = match params {
                    Ok(params) => params,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let mut lowerer = FinalFlowLowerer::new(
                    item.module(),
                    &context,
                    RuntimeAssertionOwner::Flow(identity.clone()),
                );
                match lowerer.lower_body(flow.body()) {
                    Ok(ops) => {
                        assertion_sites.extend(lowerer.into_assertion_sites());
                        flow_seeds.push(RuntimeFlowSeed::new(identity, params, ops));
                    }
                    Err(mut item_errors) => errors.append(&mut item_errors),
                }
            }
            HirItemKind::Entry(entry) => match entry_owners.remove(&item.id()) {
                Some(input) => {
                    if let Err(error) = validate_entry_input(entry, input.entry()) {
                        errors.push(error);
                    }
                }
                None => errors.push(RuntimePlanLowerError::new(format!(
                    "final-HIR Entry item {:?} is absent from the checked runtime Entry input",
                    item.id()
                ))),
            },
            HirItemKind::Error(_) => errors.push(RuntimePlanLowerError::new(format!(
                "recovered final-HIR item {:?} cannot enter runtime-plan lowering",
                item.id()
            ))),
            HirItemKind::Module(_)
            | HirItemKind::Use(_)
            | HirItemKind::Function(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Trait(_)
            | HirItemKind::Impl(_)
            | HirItemKind::Enum(_)
            | HirItemKind::Struct(_)
            | HirItemKind::TypeAlias(_)
            | HirItemKind::Resource(_)
            | HirItemKind::Character(_)
            | HirItemKind::View(_)
            | HirItemKind::Action(_)
            | HirItemKind::Activity(_)
            | HirItemKind::Signal(_)
            | HirItemKind::Metric(_)
            | HirItemKind::Layer(_)
            | HirItemKind::ExternCapability(_)
            | HirItemKind::Test(_)
            | HirItemKind::Bench(_)
            | HirItemKind::Style(_) => {}
        }
    }
    for owner in entry_owners.keys() {
        errors.push(RuntimePlanLowerError::new(format!(
            "checked runtime Entry input references non-Entry or stale owner {owner:?}"
        )));
    }
    let (controller_flows, controller_executables, callable_executables, controller_assertions) =
        lower_entry_callables(&context, entry_input, &mut errors);
    flow_seeds.extend(controller_flows);
    assertion_sites.extend(controller_assertions);
    let mut flow_executables = lower_entry_flows(project, facts, entry_input, &mut errors)?;
    flow_executables.extend(controller_executables);
    validate_unique_assertion_guards(&assertion_sites)?;
    if !errors.is_empty() {
        return Err(errors);
    }

    for input in &entry_input.entries {
        builder
            .push_entry(input.entry.clone())
            .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    }
    for executable in callable_executables {
        builder
            .push_callable_executable_seed(executable)
            .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    }
    for executable in flow_executables {
        builder
            .push_flow_executable(executable)
            .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    }
    for flow in flow_seeds {
        builder
            .push_flow_seed(flow)
            .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    }
    let plan = builder
        .finish()
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut dialogue_records = facts
        .dialogue_applications()
        .map(|(_, application)| application.content().clone())
        .collect::<Vec<_>>();
    dialogue_records.sort_by(|left, right| {
        (left.line(), left.text_key()).cmp(&(right.line(), right.text_key()))
    });
    let dialogue_content_catalog = DialogueContentCatalog::try_from_records(dialogue_records)
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    Ok(RuntimePlanLowerReport {
        plan,
        stats: RuntimePlanLowerStats {
            pure_helpers: pure_helpers.len(),
            pure_candidate_functions_seen: pure_helpers.len(),
            pure_candidate_lower_attempts: pure_helpers.len(),
            ..RuntimePlanLowerStats::default()
        },
        dialogue_content_catalog,
        character_presentation_catalog: facts.character_presentation_catalog().cloned(),
        assertion_sites: assertion_sites.into_boxed_slice(),
    })
}

fn reserve_function_sites(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    implicit_parameters: &BTreeMap<ExprId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    Vec<ReservedFunctionSiteDefinition>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    for (_, module) in project.modules() {
        for (owner, expression) in module.expressions() {
            let HirExprKind::Closure(closure) = expression.kind() else {
                continue;
            };
            let pattern_lowerer = FinalPatternLowerer::new(module, facts, locals);
            let params = closure
                .parameters()
                .iter()
                .map(|parameter| pattern_lowerer.lower(parameter.pattern()))
                .collect::<Result<Vec<_>, _>>()
                .map(|patterns| {
                    patterns
                        .iter()
                        .flat_map(|pattern| pattern.binding_locals().into_vec())
                        .collect::<Vec<_>>()
                });
            let captures = closure
                .captures()
                .iter()
                .map(|capture| {
                    module
                        .resolve_capture(*capture)
                        .map_err(|error| error.to_string())
                        .and_then(|capture| {
                            locals.get(&capture.local()).cloned().ok_or_else(|| {
                                format!(
                                    "closure {owner:?} capture {:?} has no admitted local",
                                    capture.local()
                                )
                            })
                        })
                })
                .collect::<Result<Vec<_>, _>>();
            let result = facts
                .expression_type(owner)
                .and_then(|ty| match ty.shape() {
                    RuntimeTypeShape::Function { result, .. } => Some(result.identity()),
                    _ => None,
                })
                .ok_or_else(|| format!("closure {owner:?} has no accepted function result"));
            let declaration = params.and_then(|params| {
                captures.and_then(|captures| {
                    result.map(|result| RuntimeFunctionSiteDeclarationSeed {
                        params: params.into_boxed_slice(),
                        captures: captures.into_boxed_slice(),
                        result,
                    })
                })
            });
            let declaration = match declaration {
                Ok(declaration) => declaration,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(error));
                    continue;
                }
            };
            match builder.reserve_function_site_seed(declaration) {
                Ok(site) => {
                    sites.insert(owner, site.clone());
                    definitions.push(ReservedFunctionSiteDefinition {
                        owner,
                        module: module.module_id(),
                        body: closure.body(),
                        site,
                        implicit_parameter: None,
                    });
                }
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(error.to_string()));
                    break;
                }
            }
        }
    }
    reserve_implicit_function_sites(
        project,
        facts,
        locals,
        implicit_parameters,
        builder,
        errors,
        (&mut sites, &mut definitions),
    );
    (sites, definitions)
}

fn reserve_implicit_function_sites(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    implicit_parameters: &BTreeMap<ExprId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
    output: (
        &mut BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
        &mut Vec<ReservedFunctionSiteDefinition>,
    ),
) {
    let (sites, definitions) = output;
    for (owner, callable) in facts.implicit_callables() {
        let Some(module) = module_by_id(project, owner.module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "implicit callable {owner:?} module is absent"
            )));
            continue;
        };
        let Some(parameter) = implicit_parameters.get(owner).cloned() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "implicit callable {owner:?} parameter local is absent"
            )));
            continue;
        };
        let captures = callable
            .captures()
            .iter()
            .map(|capture| {
                locals.get(capture).cloned().ok_or_else(|| {
                    format!("implicit callable {owner:?} capture {capture:?} is absent")
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let declaration = captures.map(|captures| RuntimeFunctionSiteDeclarationSeed {
            params: Box::new([parameter.clone()]),
            captures: captures.into_boxed_slice(),
            result: callable.result().identity(),
        });
        let declaration = match declaration {
            Ok(declaration) => declaration,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error));
                continue;
            }
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                sites.insert(*owner, site.clone());
                definitions.push(ReservedFunctionSiteDefinition {
                    owner: *owner,
                    module: module.module_id(),
                    body: *owner,
                    site,
                    implicit_parameter: Some(parameter),
                });
            }
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                break;
            }
        }
    }
}

fn reserve_entry_pure_helpers(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    input: &RuntimeEntryLoweringInput,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    Vec<ReservedPureHelperDefinition>,
) {
    let mut by_callable = BTreeMap::<RuntimeCallableId, &RuntimeEntryCallableInput>::new();
    for callable in &input.callables {
        let identity = callable.role().callable.clone();
        if let Some(previous) = by_callable.insert(identity.clone(), callable)
            && previous != callable
        {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry callable `{}` has conflicting owners or body roles",
                identity.as_str()
            )));
        }
    }
    let mut helpers = BTreeMap::new();
    let mut definitions = Vec::new();
    for (identity, callable) in by_callable {
        if !matches!(callable.body(), RuntimeEntryCallableBody::PureHelper) {
            continue;
        }
        let Some(item) = project.items().find(|item| item.id() == callable.owner()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry callable `{}` has a stale owner",
                identity.as_str()
            )));
            continue;
        };
        let HirItemKind::Function(function) = item.item().kind() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry callable `{}` is not a function",
                identity.as_str()
            )));
            continue;
        };
        if let Err(error) =
            validate_callable_owner(project.package(), item.module_path(), function, callable)
        {
            errors.push(error);
            continue;
        }
        match pure_helper_declaration(item.module(), facts, locals, function, identity.as_str())
            .and_then(|declaration| {
                builder
                    .reserve_pure_helper_seed(declaration)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
            }) {
            Ok(helper) => {
                helpers.insert(identity, helper.clone());
                definitions.push(ReservedPureHelperDefinition {
                    owner: callable.owner(),
                    helper,
                });
            }
            Err(error) => errors.push(error),
        }
    }
    (helpers, definitions)
}

fn reserve_called_project_helpers(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    mut helpers: BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    mut definitions: Vec<ReservedPureHelperDefinition>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    Vec<ReservedPureHelperDefinition>,
) {
    let mut helpers_by_owner = definitions
        .iter()
        .map(|definition| (definition.owner, definition.helper.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut called = BTreeMap::<ItemId, (RuntimeCallableId, &CallableDeclarationKey)>::new();
    for (_, call) in facts.calls() {
        let RuntimeResolvedCallTarget::Declaration(callable) = call.target() else {
            continue;
        };
        if callable.declaration().owner() != CallableDeclarationOwner::Function {
            continue;
        }
        match called.entry(callable.owner()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((callable.runtime().clone(), callable.declaration()));
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get().1 != callable.declaration() =>
            {
                errors.push(RuntimePlanLowerError::new(format!(
                    "project function owner {:?} has conflicting accepted callable declarations",
                    callable.owner()
                )));
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
        if let Some(helper) = helpers_by_owner.get(&callable.owner()).cloned() {
            helpers.insert(callable.runtime().clone(), helper);
        }
    }

    for (owner, (runtime, declaration)) in called {
        if helpers.contains_key(&runtime) {
            continue;
        }
        let Some(item) = project.items().find(|item| item.id() == owner) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "called project function {owner:?} has a stale final-HIR owner"
            )));
            continue;
        };
        let HirItemKind::Function(function) = item.item().kind() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "called project callable {owner:?} is not a function"
            )));
            continue;
        };
        if function
            .effect_clauses()
            .iter()
            .any(|clause| !clause.operands().is_empty())
        {
            // Effectful ordinary functions are not pure-helper declarations.
            // Their calls are lowered by flow/effect owners when reachable;
            // unrelated effectful declarations must not poison plan creation.
            continue;
        }
        if let Err(error) = validate_project_callable_owner(
            project.package(),
            item.module_path(),
            function,
            declaration,
        ) {
            errors.push(error);
            continue;
        }
        let name = function
            .name()
            .resolved()
            .map_or_else(|| runtime.as_str(), |name| name.as_str());
        let declaration = pure_helper_declaration(item.module(), facts, locals, function, name)
            .and_then(|declaration| {
                builder
                    .reserve_pure_helper_seed(declaration)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
            });
        match declaration {
            Ok(helper) => {
                helpers.insert(runtime, helper.clone());
                helpers_by_owner.insert(owner, helper.clone());
                definitions.push(ReservedPureHelperDefinition { owner, helper });
            }
            Err(error) => errors.push(error),
        }
    }
    (helpers, definitions)
}

fn pure_helper_declaration(
    module: &HirModule,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    function: &HirFunctionItem,
    name: &str,
) -> Result<RuntimePureHelperDeclarationSeed, RuntimePlanLowerError> {
    if !function.generic_parameters().is_empty() {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry pure helper `{name}` cannot retain unbound generic parameters"
        )));
    }
    let mut inputs = Vec::new();
    let mut input_abi = Vec::new();
    for parameter in function
        .parameter_groups()
        .iter()
        .flat_map(HirFunctionParameterGroup::parameters)
    {
        if parameter.kind() != HirParameterKind::Fixed
            || parameter.default().is_some()
            || parameter.locals().len() != 1
        {
            return Err(RuntimePlanLowerError::new(format!(
                "Entry pure helper `{name}` requires fixed single-binding parameters"
            )));
        }
        inputs.push(
            locals
                .get(&parameter.locals()[0])
                .cloned()
                .ok_or_else(|| RuntimePlanLowerError::new("pure helper local is not admitted"))?,
        );
        let ty = facts.ty(parameter.ty()).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry pure helper `{name}` is missing a parameter type fact"
            ))
        })?;
        input_abi.push(runtime_input_type(ty.shape()));
    }
    let body = function_body_expression(function.body())?;
    let result = facts.expression_type(body).ok_or_else(|| {
        RuntimePlanLowerError::new(format!(
            "Entry pure helper `{name}` body has no accepted runtime type"
        ))
    })?;
    let output_abi = function
        .return_type()
        .and_then(|ty| facts.ty(ty))
        .map_or(RuntimePureOutputType::Value, |ty| {
            runtime_output_type(ty.shape())
        });
    let scalar_eval_supported = output_abi != RuntimePureOutputType::Value
        && input_abi
            .iter()
            .all(|input| *input != RuntimePureInputType::Value);
    let _ = module;
    Ok(RuntimePureHelperDeclarationSeed {
        name: name.to_owned(),
        inputs: inputs.into_boxed_slice(),
        input_abi,
        result: result.identity(),
        output_abi,
        scalar_eval_supported,
        origin: RuntimePureHelperOrigin::Inferred,
    })
}

fn reserve_trait_methods(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    Vec<ReservedTraitMethodDefinition>,
) {
    let mut methods = BTreeMap::new();
    let mut definitions = Vec::new();
    for (position, checked) in facts.trait_methods().enumerate() {
        match trait_method_declaration(project, facts, locals, checked, position).and_then(
            |declaration| {
                builder
                    .reserve_trait_method_seed(declaration)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
            },
        ) {
            Ok(method) => {
                methods.insert(checked.declaration().clone(), method.clone());
                definitions.push(ReservedTraitMethodDefinition {
                    checked: checked.clone(),
                    method,
                });
            }
            Err(error) => errors.push(error),
        }
    }
    (methods, definitions)
}

fn trait_method_declaration(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    checked: &RuntimeTraitMethodFact,
    witness: usize,
) -> Result<RuntimeTraitMethodDeclarationSeed, RuntimePlanLowerError> {
    let (module, function) = resolve_trait_method(project, checked)?;
    let method_name = function
        .name()
        .resolved()
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no resolved name"))?;
    let mut receiver = None;
    let mut inputs = Vec::new();
    let mut input_abi = Vec::new();
    for parameter in function
        .parameter_groups()
        .iter()
        .flat_map(HirMethodParameterGroup::parameters)
    {
        let (local, abi) = match parameter {
            HirMethodParameter::Receiver(parameter) => {
                if receiver.is_some() {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method has more than one receiver",
                    ));
                }
                receiver = Some(match parameter.kind() {
                    HirMethodReceiverKind::Owned => RuntimeReceiverMode::Owned,
                    HirMethodReceiverKind::SharedReference => RuntimeReceiverMode::SharedRef,
                    HirMethodReceiverKind::MutableReference => RuntimeReceiverMode::MutRef,
                });
                (parameter.locals()[0], RuntimePureInputType::Value)
            }
            HirMethodParameter::Typed(parameter) => {
                if parameter.kind() != HirParameterKind::Fixed
                    || parameter.default().is_some()
                    || parameter.locals().len() != 1
                {
                    return Err(RuntimePlanLowerError::new(
                        "runtime trait method requires fixed single-binding parameters",
                    ));
                }
                let ty = facts.ty(parameter.ty()).ok_or_else(|| {
                    RuntimePlanLowerError::new("runtime trait parameter type fact is missing")
                })?;
                (parameter.locals()[0], runtime_input_type(ty.shape()))
            }
        };
        inputs.push(
            locals
                .get(&local)
                .cloned()
                .ok_or_else(|| RuntimePlanLowerError::new("trait method local is not admitted"))?,
        );
        input_abi.push(abi);
    }
    let receiver = receiver
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method requires a receiver"))?;
    let body = function_body_expression(
        function
            .body()
            .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method has no body"))?,
    )?;
    let result = facts.expression_type(body).ok_or_else(|| {
        RuntimePlanLowerError::new("runtime trait method body has no accepted runtime type")
    })?;
    let output_abi = function
        .return_type()
        .and_then(|ty| facts.ty(ty))
        .map_or(RuntimePureOutputType::Value, |ty| {
            runtime_output_type(ty.shape())
        });
    let impl_id = project
        .items()
        .position(|item| item.id() == checked.implementation())
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait Impl owner is absent"))?;
    let (trait_id, trait_name) = lower_runtime_trait_identity(project, checked.trait_identity())?;
    let _ = module;
    Ok(RuntimeTraitMethodDeclarationSeed {
        identity: RuntimeTraitMethodIdentity {
            impl_id,
            trait_id,
            witness: Some(witness),
            trait_name,
            self_type: semantic_type_label(checked.self_type()),
            method_name: method_name.as_str().to_owned(),
            monomorph_label: format!(
                "{}::{}",
                semantic_type_label(checked.self_type()),
                method_name.as_str()
            ),
        },
        receiver,
        inputs: inputs.into_boxed_slice(),
        input_abi,
        result: result.identity(),
        output_abi,
    })
}

fn semantic_type_label(ty: &crate::semantic_facts::RuntimeNormalizedType) -> String {
    let mut label = String::with_capacity(64);
    for byte in ty.identity().as_bytes() {
        use std::fmt::Write as _;
        write!(&mut label, "{byte:02x}").expect("writing to String cannot fail");
    }
    label
}

fn resolve_trait_method<'a>(
    project: HirExecutableProjectView<'a>,
    checked: &RuntimeTraitMethodFact,
) -> Result<(&'a HirModule, &'a HirImplFunction), RuntimePlanLowerError> {
    let module = project
        .modules()
        .find_map(|(_, module)| {
            (module.module_id() == checked.implementation().module()).then_some(module)
        })
        .ok_or_else(|| RuntimePlanLowerError::new("runtime trait method module is absent"))?;
    let item = module
        .resolve_item(checked.implementation())
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method owner is not an Impl",
        ));
    };
    let Some(HirImplMember::Function(function)) =
        implementation.members().get(usize::from(checked.member()))
    else {
        return Err(RuntimePlanLowerError::new(
            "checked runtime trait method member is not a function",
        ));
    };
    Ok((module, function))
}

fn define_function_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedFunctionSiteDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Some(module) = module_by_id(context.project, definition.module) else {
            errors.push(RuntimePlanLowerError::new("closure module is absent"));
            continue;
        };
        let lowerer = context.expr_lowerer(module);
        let body = definition.implicit_parameter.as_ref().map_or_else(
            || lowerer.lower_function_site_body(definition.owner, definition.body, BTreeMap::new()),
            |parameter| {
                let callable = context
                    .facts
                    .implicit_callable(definition.body)
                    .ok_or_else(|| "implicit callable fact is absent".to_owned())?;
                let value = RuntimeExprSeed::new(
                    callable.parameter().identity(),
                    arcweft_core::plan::RuntimeExprSeedKind::Local(parameter.clone()),
                );
                let overrides = callable
                    .placeholders()
                    .iter()
                    .map(|placeholder| (*placeholder, value.clone()))
                    .collect();
                lowerer.lower_function_site_body(definition.owner, definition.body, overrides)
            },
        );
        match body.and_then(|body| {
            builder
                .define_function_site_seed(&definition.site, body)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(error)),
        }
    }
}

fn define_pure_helpers(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedPureHelperDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Some(item) = context
            .project
            .items()
            .find(|item| item.id() == definition.owner)
        else {
            errors.push(RuntimePlanLowerError::new("pure helper owner is absent"));
            continue;
        };
        let HirItemKind::Function(function) = item.item().kind() else {
            errors.push(RuntimePlanLowerError::new(
                "pure helper owner is not a function",
            ));
            continue;
        };
        let body = context
            .expr_lowerer(item.module())
            .lower_function_body(function.body());
        match body.and_then(|body| {
            builder
                .define_pure_helper_seed(&definition.helper, body)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(error)),
        }
    }
}

fn define_trait_methods(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedTraitMethodDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Ok((module, function)) = resolve_trait_method(context.project, &definition.checked)
        else {
            errors.push(RuntimePlanLowerError::new("trait method owner is absent"));
            continue;
        };
        let Some(body_owner) = function.body() else {
            errors.push(RuntimePlanLowerError::new(
                "runtime trait method has no body",
            ));
            continue;
        };
        let body = context.expr_lowerer(module).lower_function_body(body_owner);
        match body.and_then(|body| {
            builder
                .define_trait_method_seed(&definition.method, body)
                .map_err(|error| error.to_string())
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(error)),
        }
    }
}

fn lower_dialogue_content(
    context: &FinalLoweringContext<'_, '_>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> BTreeMap<ExprId, RuntimeDialogueContentPlanSeedId> {
    let mut value_definitions = Vec::new();
    let mut content_definitions = Vec::new();
    for (owner, application) in context.facts.dialogue_applications() {
        let Some((dialogue, pending_values)) =
            lower_dialogue_application(context, *owner, application, builder, errors)
        else {
            continue;
        };
        content_definitions.push(dialogue);
        value_definitions.extend(pending_values);
    }
    for definition in value_definitions {
        if let Err(error) = builder.define_function_site_seed(&definition.site, definition.body) {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue value {:?} is invalid: {error}",
                definition.expression
            )));
        }
    }
    let mut content_handles = BTreeMap::new();
    for definition in content_definitions {
        let line_task_id = definition.line.public_label().into_string();
        let seed = arcweft_core::plan::RuntimeDialogueContentPlanSeed {
            line: definition.line,
            values: definition
                .values
                .into_iter()
                .map(
                    |(slot, role, function)| arcweft_core::plan::RuntimeDialogueValueSiteSeed {
                        slot,
                        role,
                        function,
                    },
                )
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            marks: definition.marks.clone(),
        };
        match builder.push_dialogue_content_seed(seed) {
            Ok(handle) => {
                if let Err(error) = attach_dialogue_effects(
                    builder,
                    definition.owner,
                    &line_task_id,
                    &definition.marks,
                    definition.effects,
                    &handle,
                ) {
                    errors.push(RuntimePlanLowerError::new(error));
                    continue;
                }
                content_handles.insert(definition.owner, handle);
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(error.to_string())),
        }
    }
    content_handles
}

fn lower_dialogue_application(
    context: &FinalLoweringContext<'_, '_>,
    owner: ExprId,
    application: &RuntimeDialogueApplication,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Option<(
    PendingDialogueContentDefinition,
    Vec<PendingDialogueValueDefinition>,
)> {
    let Some(module) = module_by_id(context.project, owner.module()) else {
        errors.push(RuntimePlanLowerError::new("dialogue module is absent"));
        return None;
    };
    let lowerer = context.expr_lowerer(module);
    let effects = lower_dialogue_effects(module, &lowerer, application, errors);
    let mut invalid = effects.is_none();
    let mut values = Vec::new();
    let mut value_definitions = Vec::new();
    for value in application.values() {
        let body = match lowerer.lower(value.expression) {
            Ok(body) => body,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error));
                invalid = true;
                continue;
            }
        };
        let declaration = RuntimeFunctionSiteDeclarationSeed {
            params: Box::new([]),
            captures: body.free_locals(),
            result: body.ty(),
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                values.push((value.slot, value.role, site.clone()));
                value_definitions.push(PendingDialogueValueDefinition {
                    expression: value.expression,
                    site,
                    body,
                });
            }
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                invalid = true;
            }
        }
    }
    if invalid {
        return None;
    }
    let effects = effects?;
    let marks = application
        .content()
        .content()
        .nodes
        .iter()
        .filter_map(|node| match node {
            RichTextNode::Control {
                control: RichTextControl::Mark { name },
            } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    Some((
        PendingDialogueContentDefinition {
            owner,
            line: application.content().line().clone(),
            values,
            marks,
            effects,
        },
        value_definitions,
    ))
}

fn lower_dialogue_effects(
    module: &HirModule,
    lowerer: &FinalExprLowerer<'_>,
    application: &RuntimeDialogueApplication,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Option<Vec<PendingDialogueEffectDefinition>> {
    let mut effects = Vec::with_capacity(application.effects().len());
    let mut valid = true;
    for effect in application.effects() {
        let expression = match module.resolve_expr(effect.expression) {
            Ok(expression) => expression,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "cannot resolve dialogue effect {:?}: {error}",
                    effect.expression
                )));
                valid = false;
                continue;
            }
        };
        let HirExprKind::Call(call) = expression.kind() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue effect {:?} is not a checked Call expression",
                effect.expression
            )));
            valid = false;
            continue;
        };
        match lowerer.lower_host_call_target(effect.expression, call) {
            Ok(Some(target)) => effects.push(PendingDialogueEffectDefinition {
                trigger: effect.trigger.clone(),
                target,
            }),
            Ok(None) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue effect {:?} is not a typed host call",
                    effect.expression
                )));
                valid = false;
            }
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error));
                valid = false;
            }
        }
    }
    valid.then_some(effects)
}

fn attach_dialogue_effects(
    builder: &mut RuntimePlanBuilder,
    owner: ExprId,
    line_task_id: &str,
    marks: &[String],
    effects: Vec<PendingDialogueEffectDefinition>,
    handle: &RuntimeDialogueContentPlanSeedId,
) -> Result<(), String> {
    let mut children = Vec::with_capacity(effects.len());
    for (ordinal, effect) in effects.into_iter().enumerate() {
        let trigger = match effect.trigger {
            RuntimeDialogueEffectTrigger::Mark(label) => marks
                .iter()
                .position(|mark| mark == &label)
                .and_then(|index| handle.mark(index))
                .map(RuntimeLineTaskTriggerSeed::Mark)
                .ok_or_else(|| {
                    format!("dialogue application {owner:?} effect mark `{label}` is absent")
                })?,
            RuntimeDialogueEffectTrigger::DelayMillis(millis) => millis
                .checked_mul(1_000_000)
                .map(LogicalDuration::from_nanos)
                .map(RuntimeLineTaskTriggerSeed::Delay)
                .ok_or_else(|| {
                    format!("dialogue application {owner:?} delay {millis}ms overflows nanoseconds")
                })?,
        };
        children.push(RuntimeLineTaskNodeSeed::Child {
            id: TaskId(format!("{line_task_id}.effect.{ordinal}")),
            key: None,
            name: None,
            trigger,
            priority: TaskPriority::default(),
            join_policy: ChildJoinPolicy::Join,
            cancel_policy: ChildCancelPolicy::CancelAndJoin,
            scope: Box::new(RuntimeLineTaskNodeSeed::Action(vec![
                RuntimeFlowOpSeed::HostCall {
                    binding: None,
                    target: effect.target,
                },
            ])),
        });
    }
    if children.is_empty() {
        return Ok(());
    }
    let group = RuntimeLineTaskGroupSeed {
        root: RuntimeLineTaskNodeSeed::Start(children),
        cancel_rules: Box::new([]),
        cleanup_completed: Vec::new(),
        cleanup_cancelled: Vec::new(),
        cleanup_failed: Vec::new(),
        cleanup_policy: LineCleanupPolicy::default(),
    };
    builder
        .push_line_task_group_seed(group)
        .and_then(|group| builder.attach_line_task_group_seed(handle, &group))
        .map_err(|error| error.to_string())
}

fn lower_entry_callables(
    context: &FinalLoweringContext<'_, '_>,
    input: &RuntimeEntryLoweringInput,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    Vec<RuntimeFlowSeed>,
    Vec<RuntimeFlowExecutable>,
    Vec<arcweft_core::plan::RuntimeCallableExecutableSeed>,
    Vec<RuntimeAssertionSite>,
) {
    let mut flows = Vec::new();
    let mut flow_executables = Vec::new();
    let mut executables = Vec::new();
    let mut assertions = Vec::new();
    let mut admitted = BTreeMap::new();
    for callable in &input.callables {
        match admitted.insert(callable.role().callable.clone(), callable) {
            Some(previous) if previous != callable => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "Entry callable `{}` has conflicting executable projections",
                    callable.role().callable.as_str()
                )));
                continue;
            }
            Some(_) => continue,
            None => {}
        }
        match callable.body() {
            RuntimeEntryCallableBody::PureHelper => {
                let Some(helper) = context.pure_helpers.get(&callable.role().callable).cloned()
                else {
                    continue;
                };
                executables.push(arcweft_core::plan::RuntimeCallableExecutableSeed {
                    callable: callable.role().callable.clone(),
                    contract: callable.role().contract,
                    code: arcweft_core::plan::RuntimeCallableExecutableSeedCode::PureHelper(helper),
                });
            }
            RuntimeEntryCallableBody::ControllerFlow(flow) => {
                match lower_controller_callable(context, callable, flow) {
                    Ok(lowered) => {
                        flows.push(lowered.flow);
                        flow_executables.push(lowered.flow_executable);
                        executables.push(lowered.executable);
                        assertions.extend(lowered.assertions);
                    }
                    Err(error) => errors.push(error),
                }
            }
        }
    }
    (flows, flow_executables, executables, assertions)
}

fn lower_controller_callable(
    context: &FinalLoweringContext<'_, '_>,
    callable: &RuntimeEntryCallableInput,
    flow: &FlowRuntimeId,
) -> Result<LoweredControllerCallable, RuntimePlanLowerError> {
    let item = context
        .project
        .items()
        .find(|item| item.id() == callable.owner())
        .ok_or_else(|| RuntimePlanLowerError::new("controller owner is absent"))?;
    let HirItemKind::Function(function) = item.item().kind() else {
        return Err(RuntimePlanLowerError::new(
            "controller owner is not a function",
        ));
    };
    if function
        .parameter_groups()
        .iter()
        .any(|group| !group.parameters().is_empty())
    {
        return Err(RuntimePlanLowerError::new(
            "Agent controller ordinary function must not accept parameters",
        ));
    }
    let HirFunctionBody::Block {
        statements, tail, ..
    } = function.body()
    else {
        return Err(RuntimePlanLowerError::new(
            "recovered Agent controller body cannot enter runtime lowering",
        ));
    };
    let CallableDeclarationKey::Existing(declaration) = callable.declaration() else {
        return Err(RuntimePlanLowerError::new(
            "Agent controller has no ordinary declaration identity",
        ));
    };
    let mut lowerer = FinalFlowLowerer::new(
        item.module(),
        context,
        RuntimeAssertionOwner::Callable(declaration.clone()),
    );
    let mut ops = lowerer.lower_statement_ids(statements)?;
    let tail = context
        .expr_lowerer(item.module())
        .lower(*tail)
        .map_err(RuntimePlanLowerError::new)?;
    ops.push(RuntimeFlowOpSeed::ReturnExpr(tail));
    let assertions = lowerer.into_assertion_sites();
    let flow_executable = RuntimeFlowExecutable {
        flow: flow.clone(),
        contract: arcweft_core::entry::FlowContractHash::from_bytes(
            *callable.role().contract.as_bytes(),
        ),
        parameters: Vec::new(),
        controller: Some(callable.role().clone()),
    };
    let executable = arcweft_core::plan::RuntimeCallableExecutableSeed {
        callable: callable.role().callable.clone(),
        contract: callable.role().contract,
        code: arcweft_core::plan::RuntimeCallableExecutableSeedCode::ControllerFlow(flow.clone()),
    };
    Ok(LoweredControllerCallable {
        flow: RuntimeFlowSeed::new(flow.clone(), [], ops),
        flow_executable,
        executable,
        assertions,
    })
}

fn function_body_expression(body: &HirFunctionBody) -> Result<ExprId, RuntimePlanLowerError> {
    match body {
        HirFunctionBody::Block { tail, .. } => Ok(*tail),
        HirFunctionBody::Error(expression) => Err(RuntimePlanLowerError::new(format!(
            "recovered ordinary-function body {expression:?} cannot enter runtime lowering"
        ))),
    }
}

fn bind_seed(ty: &RuntimeNormalizedType, local: RuntimeLocalSeedId) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        ty.identity(),
        RuntimePatternSeedKind::Bind {
            local,
            mutable: false,
        },
    )
}

fn local_seed(ty: &RuntimeNormalizedType, local: RuntimeLocalSeedId) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty.identity(),
        arcweft_core::plan::RuntimeExprSeedKind::Local(local),
    )
}

fn variant_bind_seed(
    result: &RuntimeNormalizedType,
    ordinal: u32,
    payload: &RuntimeNormalizedType,
    local: RuntimeLocalSeedId,
) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        result.identity(),
        RuntimePatternSeedKind::Variant {
            ordinal,
            payload: Some(Box::new(bind_seed(payload, local))),
        },
    )
}

fn variant_empty_seed(result: &RuntimeNormalizedType, ordinal: u32) -> RuntimePatternSeed {
    RuntimePatternSeed::new(
        result.identity(),
        RuntimePatternSeedKind::Variant {
            ordinal,
            payload: None,
        },
    )
}

fn module_by_id(
    project: HirExecutableProjectView<'_>,
    expected: HirModuleId,
) -> Option<&Arc<HirModule>> {
    project
        .modules()
        .find_map(|(_, module)| (module.module_id() == expected).then_some(module))
}

fn lower_runtime_trait_identity(
    project: HirExecutableProjectView<'_>,
    identity: &RuntimeTraitIdentity,
) -> Result<(Option<usize>, Option<String>), RuntimePlanLowerError> {
    Ok(match identity {
        RuntimeTraitIdentity::Project(owner) => {
            let (position, trait_owner) = project
                .items()
                .enumerate()
                .find(|(_, item)| item.id() == *owner)
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait owner is absent"))?;
            let trait_item = trait_owner
                .module()
                .resolve_item(*owner)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let HirItemKind::Trait(trait_item) = trait_item.kind() else {
                return Err(RuntimePlanLowerError::new(
                    "runtime Trait identity does not own a Trait item",
                ));
            };
            let name = trait_item
                .name()
                .resolved()
                .ok_or_else(|| RuntimePlanLowerError::new("runtime Trait has no resolved name"))?;
            (Some(position), Some(name.as_str().to_owned()))
        }
        RuntimeTraitIdentity::StandardIterator => (None, Some("Iterator".to_owned())),
        RuntimeTraitIdentity::StandardIntoIterator => (None, Some("IntoIterator".to_owned())),
    })
}

fn collect_entry_inputs<'input>(
    input: &'input RuntimeEntryLoweringInput,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> BTreeMap<ItemId, &'input RuntimeCheckedEntryInput> {
    let mut entries = BTreeMap::new();
    for entry in &input.entries {
        if entries.insert(entry.owner(), entry).is_some() {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked runtime Entry input repeats final-HIR owner {:?}",
                entry.owner()
            )));
        }
    }
    entries
}

fn validate_entry_input(
    hir: &HirEntryDeclaration,
    runtime: &RuntimeEntrySpec,
) -> Result<(), RuntimePlanLowerError> {
    let expected_kind = match hir.kind() {
        HirEntryKind::Game => RuntimeEntryKind::Game,
        HirEntryKind::Editor => RuntimeEntryKind::Editor,
        HirEntryKind::Cli => RuntimeEntryKind::Cli,
        HirEntryKind::Server => RuntimeEntryKind::Server,
        HirEntryKind::Activity => RuntimeEntryKind::Activity,
        HirEntryKind::Test => RuntimeEntryKind::Test,
        HirEntryKind::Bench => RuntimeEntryKind::Bench,
        HirEntryKind::Agent => RuntimeEntryKind::Agent,
        HirEntryKind::Custom(name) => RuntimeEntryKind::Custom(name.as_str().to_owned()),
        HirEntryKind::Recovered(_) => {
            return Err(RuntimePlanLowerError::new(
                "recovered final-HIR Entry kind cannot enter runtime lowering",
            ));
        }
    };
    if expected_kind != runtime.kind {
        return Err(RuntimePlanLowerError::new(format!(
            "checked runtime Entry `{}` has kind `{}`, but its final-HIR owner has kind `{}`",
            runtime.id,
            runtime.kind.as_str(),
            expected_kind.as_str()
        )));
    }
    let HirEntryId::Authored { value, .. } = hir.id() else {
        return Err(RuntimePlanLowerError::new(
            "final-HIR Entry owner has no authored semantic identity",
        ));
    };
    let Some(HirIdRef::Absolute(source_id)) = value.as_resolved() else {
        return Err(RuntimePlanLowerError::new(
            "final-HIR Entry owner does not retain one absolute semantic identity",
        ));
    };
    if source_id.as_str() != runtime.id.public_label().as_str() {
        return Err(RuntimePlanLowerError::new(format!(
            "checked runtime Entry identity `{}` does not match final-HIR owner `{}`",
            runtime.id,
            source_id.as_str()
        )));
    }
    Ok(())
}

fn validate_callable_owner(
    package: &CallablePackageId,
    module_path: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    function: &HirFunctionItem,
    input: &RuntimeEntryCallableInput,
) -> Result<(), RuntimePlanLowerError> {
    let CallableDeclarationKey::Existing(declaration) = input.declaration() else {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` does not use an ordinary declaration identity",
            input.role().callable.as_str()
        )));
    };
    let Some(name) = function.name().resolved() else {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` has a recovered final-HIR function name",
            input.role().callable.as_str()
        )));
    };
    if declaration.owner() != CallableDeclarationOwner::Function
        || declaration.package() != package
        || declaration.module() != module_path
        || declaration.name() != name.as_str()
        || RuntimeCallableId::try_new(declaration.to_string()).as_ref()
            != Ok(&input.role().callable)
    {
        return Err(RuntimePlanLowerError::new(format!(
            "Entry callable `{}` identity does not match its exact final-HIR function owner",
            input.role().callable.as_str()
        )));
    }
    Ok(())
}

fn validate_project_callable_owner(
    package: &CallablePackageId,
    module_path: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
    function: &HirFunctionItem,
    declaration: &CallableDeclarationKey,
) -> Result<(), RuntimePlanLowerError> {
    let CallableDeclarationKey::Existing(declaration) = declaration else {
        return Err(RuntimePlanLowerError::new(
            "called project function does not use an ordinary declaration identity",
        ));
    };
    let Some(name) = function.name().resolved() else {
        return Err(RuntimePlanLowerError::new(
            "called project function has a recovered final-HIR name",
        ));
    };
    if declaration.owner() != CallableDeclarationOwner::Function
        || declaration.package() != package
        || declaration.module() != module_path
        || declaration.name() != name.as_str()
    {
        return Err(RuntimePlanLowerError::new(format!(
            "called project function `{}` does not match its accepted declaration identity",
            name.as_str()
        )));
    }
    Ok(())
}

fn lower_entry_flows(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    input: &RuntimeEntryLoweringInput,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Result<Vec<RuntimeFlowExecutable>, Vec<RuntimePlanLowerError>> {
    let mut by_runtime = BTreeMap::new();
    let mut owners = BTreeSet::new();
    for flow in &input.flows {
        if !owners.insert(flow.owner()) {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow input repeats final-HIR owner {:?}",
                flow.owner()
            )));
            continue;
        }
        let Some(item) = project.items().find(|item| item.id() == flow.owner()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow input references a foreign or stale owner {:?}",
                flow.owner()
            )));
            continue;
        };
        if !matches!(item.item().kind(), HirItemKind::Flow(_))
            || facts.flow(flow.owner()) != Some(&flow.executable().flow)
        {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow executable `{}` does not match its exact final-HIR Flow owner",
                flow.executable().flow
            )));
            continue;
        }
        match by_runtime.insert(flow.executable().flow.clone(), flow.executable().clone()) {
            Some(previous) if previous != *flow.executable() => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "checked Entry Flow executable `{}` has conflicting role metadata",
                    flow.executable().flow
                )));
            }
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(by_runtime.into_values().collect())
    } else {
        Err(std::mem::take(errors))
    }
}

const fn runtime_input_type(shape: &RuntimeTypeShape) -> RuntimePureInputType {
    match shape {
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8) => RuntimePureInputType::I8,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16) => RuntimePureInputType::I16,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32) => RuntimePureInputType::I32,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64) => RuntimePureInputType::I64,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128) => RuntimePureInputType::I128,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize) => RuntimePureInputType::ISize,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8) => RuntimePureInputType::U8,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16) => RuntimePureInputType::U16,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32) => RuntimePureInputType::U32,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64) => RuntimePureInputType::U64,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128) => RuntimePureInputType::U128,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize) => RuntimePureInputType::USize,
        RuntimeTypeShape::F32 => RuntimePureInputType::F32,
        RuntimeTypeShape::F64 => RuntimePureInputType::F64,
        _ => RuntimePureInputType::Value,
    }
}

const fn runtime_output_type(shape: &RuntimeTypeShape) -> RuntimePureOutputType {
    match shape {
        RuntimeTypeShape::Bool => RuntimePureOutputType::Bool,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I8) => RuntimePureOutputType::I8,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I16) => RuntimePureOutputType::I16,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I32) => RuntimePureOutputType::I32,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I64) => RuntimePureOutputType::I64,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::I128) => RuntimePureOutputType::I128,
        RuntimeTypeShape::Signed(RuntimeSignedIntWidth::ISize) => RuntimePureOutputType::ISize,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U8) => RuntimePureOutputType::U8,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U16) => RuntimePureOutputType::U16,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U32) => RuntimePureOutputType::U32,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U64) => RuntimePureOutputType::U64,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::U128) => RuntimePureOutputType::U128,
        RuntimeTypeShape::Unsigned(RuntimeUnsignedIntWidth::USize) => RuntimePureOutputType::USize,
        RuntimeTypeShape::F32 => RuntimePureOutputType::F32,
        RuntimeTypeShape::F64 => RuntimePureOutputType::F64,
        _ => RuntimePureOutputType::Value,
    }
}

fn semantic_fact_error(error: &RuntimeSemanticFactsError) -> RuntimePlanLowerError {
    RuntimePlanLowerError::new(format!(
        "runtime semantic facts do not match the accepted HIR generation: {error}"
    ))
}

fn validate_unique_assertion_guards(
    sites: &[RuntimeAssertionSite],
) -> Result<(), Vec<RuntimePlanLowerError>> {
    let mut guards = BTreeSet::new();
    let mut errors = Vec::new();
    for site in sites {
        if !guards.insert(site.guard()) {
            errors.push(RuntimePlanLowerError::new(format!(
                "runtime assertion guard collision for {:?}",
                site.guard()
            )));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

enum RuntimeAssertionOwner {
    Callable(CallableDeclarationId),
    Flow(FlowRuntimeId),
}

impl RuntimeAssertionOwner {
    fn label(&self) -> String {
        match self {
            Self::Callable(declaration) => declaration.qualified_name(),
            Self::Flow(flow) => flow.canonical_label(),
        }
    }
}

struct FinalFlowLowerer<'a> {
    module: &'a HirModule,
    facts: &'a RuntimePlanSemanticFacts,
    package: &'a CallablePackageId,
    locals: &'a BTreeMap<LocalId, RuntimeLocalSeedId>,
    pure_helpers: &'a BTreeMap<RuntimeCallableId, RuntimePureHelperSeedId>,
    trait_methods: &'a BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'a BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    dialogue_content: &'a BTreeMap<ExprId, RuntimeDialogueContentPlanSeedId>,
    await_locals: &'a BTreeMap<ExprId, AwaitLocalSeeds>,
    try_locals: &'a BTreeMap<ExprId, TryLocalSeeds>,
    pipe_locals: &'a BTreeMap<ExprId, RuntimeLocalSeedId>,
    carrier_continuations: BTreeMap<ExprId, RuntimeFlowValueContinuation>,
    assertion_owner: RuntimeAssertionOwner,
    assertion_ordinal: u32,
    await_ordinal: u32,
    assertion_sites: Vec<RuntimeAssertionSite>,
}

#[derive(Clone)]
enum RuntimeFlowValueContinuation {
    Bind {
        pattern: RuntimePatternSeed,
        tail: RuntimeFlowTail,
    },
    Return,
    Ignore(RuntimeFlowTail),
    Try {
        owner: ExprId,
        outer: Box<Self>,
    },
    WrapCarrier {
        owner: ExprId,
        outer: Box<Self>,
    },
    Compose {
        owner: ExprId,
        child: ExprId,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
        outer: Box<Self>,
    },
    Pipe {
        owner: ExprId,
        right: ExprId,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
        outer: Box<Self>,
    },
}

#[derive(Clone, Default)]
enum RuntimeFlowTail {
    #[default]
    None,
    StatementsWithTail {
        statements: Box<[StmtId]>,
        tail: Box<RuntimeFlowTail>,
    },
    ThreadItems(Box<[HirThreadFlowItem]>),
    Value {
        expression: ExprId,
        continuation: Box<RuntimeFlowValueContinuation>,
    },
}

impl<'a> FinalFlowLowerer<'a> {
    fn new(
        module: &'a HirModule,
        context: &'a FinalLoweringContext<'_, '_>,
        assertion_owner: RuntimeAssertionOwner,
    ) -> Self {
        Self {
            module,
            facts: context.facts,
            package: context.project.package(),
            locals: context.locals,
            pure_helpers: context.pure_helpers,
            trait_methods: context.trait_methods,
            function_sites: context.function_sites,
            dialogue_content: context.dialogue_content,
            await_locals: context.await_locals,
            try_locals: context.try_locals,
            pipe_locals: context.pipe_locals,
            carrier_continuations: BTreeMap::new(),
            assertion_owner,
            assertion_ordinal: 0,
            await_ordinal: 0,
            assertion_sites: Vec::new(),
        }
    }

    fn into_assertion_sites(self) -> Vec<RuntimeAssertionSite> {
        self.assertion_sites
    }

    fn expr_lowerer(&self) -> FinalExprLowerer<'_> {
        FinalExprLowerer::new(
            self.module,
            self.facts,
            self.locals,
            self.pure_helpers,
            self.trait_methods,
            self.function_sites,
            (self.pipe_locals, self.try_locals),
        )
    }

    fn trait_method(
        &self,
        declaration: &ImplMethodDeclarationId,
        statement: StmtId,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanLowerError> {
        self.trait_methods.get(declaration).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "For statement {statement:?} refers to an unreserved trait method"
            ))
        })
    }

    fn lower_body(
        &mut self,
        body: &HirThreadBody,
    ) -> Result<Vec<RuntimeFlowOpSeed>, Vec<RuntimePlanLowerError>> {
        self.lower_thread_items(body.items())
            .map_err(|error| vec![error])
    }

    fn lower_thread_items(
        &mut self,
        items: &[HirThreadFlowItem],
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let Some((item, tail)) = items.split_first() else {
            return Ok(Vec::new());
        };
        self.lower_thread_item(
            item,
            RuntimeFlowTail::ThreadItems(tail.to_vec().into_boxed_slice()),
        )
    }

    fn lower_thread_item(
        &mut self,
        item: &HirThreadFlowItem,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let statement = match item {
            HirThreadFlowItem::DialogueApplication(expression) => {
                self.facts
                    .dialogue_application(*expression)
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue application {expression:?} has no checked projection fact"
                        ))
                    })?;
                let content = self.dialogue_content.get(expression).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue application {expression:?} has no builder-issued content handle"
                    ))
                })?;
                let mut ops = vec![RuntimeFlowOpSeed::Dialogue { content }];
                ops.extend(self.lower_flow_tail(tail)?);
                return Ok(ops);
            }
            HirThreadFlowItem::Statement(statement)
            | HirThreadFlowItem::Choice(statement)
            | HirThreadFlowItem::If(statement)
            | HirThreadFlowItem::IfLet(statement)
            | HirThreadFlowItem::Match(statement)
            | HirThreadFlowItem::While(statement)
            | HirThreadFlowItem::WhileLet(statement)
            | HirThreadFlowItem::For(statement)
            | HirThreadFlowItem::Select(statement)
            | HirThreadFlowItem::SourceLocale(statement)
            | HirThreadFlowItem::Scope(statement)
            | HirThreadFlowItem::Include(statement)
            | HirThreadFlowItem::Error(statement) => *statement,
        };
        let kind = self.resolve_statement(statement)?.kind().clone();
        if !thread_item_matches_kind(item, &kind) {
            return Err(RuntimePlanLowerError::new(format!(
                "final-HIR thread item family does not match statement {statement:?} payload {kind:?}"
            )));
        }
        self.lower_statement_with_tail(statement, &kind, tail)
    }

    fn resolve_statement(
        &self,
        statement: StmtId,
    ) -> Result<&arcweft_lang_hir::stmt::HirStmt, RuntimePlanLowerError> {
        self.module.resolve_stmt(statement).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve final-HIR flow statement {statement:?}: {error}"
            ))
        })
    }

    fn lower_statement_with_tail(
        &mut self,
        id: StmtId,
        kind: &HirStmtKind,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        match kind {
            HirStmtKind::Let {
                pattern: owner,
                initializer,
                ..
            } if self.contains_flow_value_expression(*initializer)? => {
                let pattern = FinalPatternLowerer::new(self.module, self.facts, self.locals)
                    .lower(*owner)
                    .map_err(RuntimePlanLowerError::new)?;
                self.lower_flow_value(
                    *initializer,
                    RuntimeFlowValueContinuation::Bind { pattern, tail },
                )
            }
            HirStmtKind::Expression { expression }
                if self.contains_flow_value_expression(*expression)? =>
            {
                self.lower_flow_value(*expression, RuntimeFlowValueContinuation::Ignore(tail))
            }
            HirStmtKind::Choice { choice } => {
                self.lower_flow_value(*choice, RuntimeFlowValueContinuation::Ignore(tail))
            }
            HirStmtKind::Return { .. }
            | HirStmtKind::Goto { .. }
            | HirStmtKind::Break { .. }
            | HirStmtKind::Continue { .. } => self.lower_statement(id, kind),
            _ => {
                let mut ops = self.lower_statement(id, kind)?;
                ops.extend(self.lower_flow_tail(tail)?);
                Ok(ops)
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the final-HIR statement match is intentionally exhaustive so unsupported execution families cannot fall through to a second reader"
    )]
    fn lower_statement(
        &mut self,
        id: StmtId,
        kind: &HirStmtKind,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let expr = FinalExprLowerer::new(
            self.module,
            self.facts,
            self.locals,
            self.pure_helpers,
            self.trait_methods,
            self.function_sites,
            (self.pipe_locals, self.try_locals),
        );
        let pattern = FinalPatternLowerer::new(self.module, self.facts, self.locals);
        match kind {
            HirStmtKind::Assertion { mode, conditions } => {
                self.lower_assertion(id, *mode, conditions)
            }
            HirStmtKind::Let {
                pattern: owner,
                initializer,
                ..
            } => {
                let binding = pattern.lower(*owner).map_err(RuntimePlanLowerError::new)?;
                if self.contains_flow_value_expression(*initializer)? {
                    return self.lower_flow_value(
                        *initializer,
                        RuntimeFlowValueContinuation::Bind {
                            pattern: binding,
                            tail: RuntimeFlowTail::None,
                        },
                    );
                }
                if let Some(host) = self.lower_host_call(*initializer, Some(binding.clone()))? {
                    Ok(vec![host])
                } else {
                    Ok(vec![RuntimeFlowOpSeed::Let {
                        pattern: binding,
                        expr: expr
                            .lower(*initializer)
                            .map_err(RuntimePlanLowerError::new)?,
                    }])
                }
            }
            HirStmtKind::Assign { value, .. } => Ok(vec![
                expr.lower_flow_assignment(id, *value)
                    .map_err(RuntimePlanLowerError::new)?,
            ]),
            HirStmtKind::LetElse {
                pattern: owner,
                initializer,
                else_body,
                ..
            } => Ok(vec![RuntimeFlowOpSeed::LetElse {
                pattern: pattern.lower(*owner).map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(*initializer)
                    .map_err(RuntimePlanLowerError::new)?,
                else_ops: self.lower_statement_ids(else_body)?,
            }]),
            HirStmtKind::Return { value } => {
                if self.contains_flow_value_expression(*value)? {
                    return self.lower_flow_value(*value, RuntimeFlowValueContinuation::Return);
                }
                if let Some(host) = self.lower_host_call(*value, None)? {
                    let result = self.facts.expression_type(*value).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "return host call {value:?} has no accepted result type"
                        ))
                    })?;
                    if !matches!(result.shape(), RuntimeTypeShape::Never) {
                        return Err(RuntimePlanLowerError::new(format!(
                            "return host call {value:?} must have the Never result type"
                        )));
                    }
                    Ok(vec![host])
                } else {
                    Ok(vec![RuntimeFlowOpSeed::ReturnExpr(
                        expr.lower(*value).map_err(RuntimePlanLowerError::new)?,
                    )])
                }
            }
            HirStmtKind::Goto { target } => {
                if let Some(RuntimeResolvedValue::ProjectItem(item)) = self.facts.value(*target)
                    && let Some(target) = item.flow_runtime_id()
                {
                    Ok(vec![RuntimeFlowOpSeed::Goto(target.clone())])
                } else {
                    Ok(vec![RuntimeFlowOpSeed::GotoExpr(
                        expr.lower(*target).map_err(RuntimePlanLowerError::new)?,
                    )])
                }
            }
            HirStmtKind::Expression { expression: thread } => {
                if let Some(effect) = self.facts.evaluated_effect(id) {
                    return Ok(vec![RuntimeFlowOpSeed::EvaluatedEffect(
                        lower_evaluated_effect(&expr, effect.effect())?,
                    )]);
                }
                if self.contains_flow_value_expression(*thread)? {
                    return self.lower_flow_value(
                        *thread,
                        RuntimeFlowValueContinuation::Ignore(RuntimeFlowTail::None),
                    );
                }
                if let Some(host) = self.lower_host_call(*thread, None)? {
                    return Ok(vec![host]);
                }
                let thread_expr = self.module.resolve_expr(*thread).map_err(|error| {
                    RuntimePlanLowerError::new(format!(
                        "cannot resolve final-HIR Thread expression {thread:?}: {error}"
                    ))
                })?;
                let HirExprKind::Thread(thread_expr) = thread_expr.kind() else {
                    return Err(RuntimePlanLowerError::new(format!(
                        "expression statement {id:?} references non-Thread expression {thread:?} without a checked effect disposition"
                    )));
                };
                if thread_expr.mode() == HirThreadMode::Detached {
                    return Err(RuntimePlanLowerError::new(format!(
                        "detached Thread expression {thread:?} requires typed runtime ownership metadata"
                    )));
                }
                Ok(vec![RuntimeFlowOpSeed::Thread {
                    name: thread_expr.name().map(|name| name.as_str().to_owned()),
                    body: self.lower_body_as_one_error(thread_expr.body())?,
                }])
            }
            HirStmtKind::Choice { choice } => self.lower_flow_value(
                *choice,
                RuntimeFlowValueContinuation::Ignore(RuntimeFlowTail::None),
            ),
            HirStmtKind::If(branch) => Ok(vec![RuntimeFlowOpSeed::If {
                condition: expr
                    .lower(branch.condition())
                    .map_err(RuntimePlanLowerError::new)?,
                then_ops: self.lower_contextual_body(branch.then_body())?,
                else_ops: branch
                    .else_branch()
                    .map(|branch| self.lower_else_branch(branch))
                    .transpose()?
                    .unwrap_or_default(),
            }]),
            HirStmtKind::IfLet(branch) => Ok(vec![RuntimeFlowOpSeed::IfLet {
                pattern: pattern
                    .lower(branch.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(branch.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: branch
                    .guard()
                    .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                    .transpose()?,
                then_ops: self.lower_contextual_body(branch.then_body())?,
                else_ops: branch
                    .else_branch()
                    .map(|branch| self.lower_else_branch(branch))
                    .transpose()?
                    .unwrap_or_default(),
            }]),
            HirStmtKind::Match(matched) => {
                let mut arms = Vec::with_capacity(matched.arms().len());
                for arm in matched.arms() {
                    let ops = match arm.body() {
                        HirStmtMatchArmBody::Body(body) => self.lower_contextual_body(body)?,
                        HirStmtMatchArmBody::Expression(expression) => {
                            return Err(RuntimePlanLowerError::new(format!(
                                "flow match expression arm {expression:?} requires an explicit effect/value disposition"
                            )));
                        }
                    };
                    arms.push(RuntimeFlowMatchArmSeed {
                        pattern: pattern
                            .lower(arm.pattern())
                            .map_err(RuntimePlanLowerError::new)?,
                        guard: arm
                            .guard()
                            .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                            .transpose()?,
                        ops,
                    });
                }
                Ok(vec![RuntimeFlowOpSeed::Match {
                    scrutinee: expr
                        .lower(matched.scrutinee())
                        .map_err(RuntimePlanLowerError::new)?,
                    arms,
                }])
            }
            HirStmtKind::While(while_stmt) => Ok(vec![RuntimeFlowOpSeed::While {
                condition: expr
                    .lower(while_stmt.condition())
                    .map_err(RuntimePlanLowerError::new)?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::WhileLet(while_stmt) => Ok(vec![RuntimeFlowOpSeed::WhileLet {
                pattern: pattern
                    .lower(while_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: expr
                    .lower(while_stmt.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: while_stmt
                    .guard()
                    .map(|guard| expr.lower(guard).map_err(RuntimePlanLowerError::new))
                    .transpose()?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::For(for_stmt) => Ok(vec![RuntimeFlowOpSeed::For {
                pattern: pattern
                    .lower(for_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                source: expr
                    .lower(for_stmt.source())
                    .map_err(RuntimePlanLowerError::new)?,
                evidence: match self.facts.iteration(id).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "checked iteration evidence is missing for For statement {id:?}"
                    ))
                })? {
                    RuntimeIteratorFact::Builtin(evidence) => {
                        RuntimeIteratorEvidenceSeed::Builtin(evidence)
                    }
                    RuntimeIteratorFact::Witness(witness) => {
                        let executable = match witness.executable() {
                            RuntimeIteratorWitnessExecutableFact::TraitCalls {
                                into_iter,
                                next,
                            } => RuntimeIteratorWitnessExecutableSeed::TraitCalls {
                                into_iter: self.trait_method(into_iter, id)?,
                                next: self.trait_method(next, id)?,
                            },
                            RuntimeIteratorWitnessExecutableFact::IdentityIntoIterator { next } => {
                                RuntimeIteratorWitnessExecutableSeed::IdentityIntoIterator {
                                    next: self.trait_method(next, id)?,
                                }
                            }
                        };
                        RuntimeIteratorEvidenceSeed::Witness(RuntimeIteratorWitnessEvidenceSeed {
                            item: witness.item().identity(),
                            iterator: witness.iterator().identity(),
                            executable,
                        })
                    }
                },
                body: self.lower_contextual_body(for_stmt.body())?,
            }]),
            HirStmtKind::Scope(scope) => {
                if scope.name().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "named Scope {id:?} requires a typed runtime scope identity"
                    )));
                }
                Ok(vec![RuntimeFlowOpSeed::Scope(
                    self.lower_contextual_body(scope.body())?,
                )])
            }
            HirStmtKind::Break { label, value } if label.is_none() => {
                Ok(vec![RuntimeFlowOpSeed::Break(
                    value
                        .map(|value| expr.lower(value).map_err(RuntimePlanLowerError::new))
                        .transpose()?,
                )])
            }
            HirStmtKind::Continue { label } if label.is_none() => {
                Ok(vec![RuntimeFlowOpSeed::Continue])
            }
            HirStmtKind::Error => Err(RuntimePlanLowerError::new(format!(
                "recovered statement {id:?} cannot enter runtime-plan lowering"
            ))),
            unsupported => Err(RuntimePlanLowerError::new(format!(
                "final-HIR statement {id:?} family {unsupported:?} has no checked core projection"
            ))),
        }
    }

    fn contains_flow_value_expression(
        &self,
        expression: ExprId,
    ) -> Result<bool, RuntimePlanLowerError> {
        if self.facts.implicit_callable(expression).is_some() {
            return Ok(false);
        }
        let expression = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve flow value expression {expression:?}: {error}"
            ))
        })?;
        if matches!(
            expression.kind(),
            HirExprKind::Await(_)
                | HirExprKind::Choice(_)
                | HirExprKind::Loop(_)
                | HirExprKind::Try(_)
        ) || matches!(
            expression.kind(),
            HirExprKind::ComputationBlock(block)
                if matches!(
                    block.kind(),
                    arcweft_lang_hir::expr::HirComputationBlockKind::Result
                        | arcweft_lang_hir::expr::HirComputationBlockKind::Option
                )
        ) {
            return Ok(true);
        }
        for child in expression.kind().direct_expression_children() {
            if self.contains_flow_value_expression(child)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn lower_flow_value(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_flow_value_with_overrides(expression, continuation, BTreeMap::new())
    }

    fn lower_flow_value_with_overrides(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve flow value expression {expression:?}: {error}"
            ))
        })?;
        if self.facts.implicit_callable(expression).is_some() {
            let value = self
                .expr_lowerer()
                .lower(expression)
                .map_err(RuntimePlanLowerError::new)?;
            return self.apply_value_continuation(value, continuation);
        }
        match resolved.kind() {
            HirExprKind::Try(operation) => self.lower_flow_value_with_overrides(
                operation.operand(),
                RuntimeFlowValueContinuation::Try {
                    owner: expression,
                    outer: Box::new(continuation),
                },
                overrides,
            ),
            HirExprKind::Await(awaited) => {
                self.lower_await_value(expression, awaited, &continuation)
            }
            HirExprKind::Choice(choice) => {
                self.lower_choice_value(expression, choice, continuation)
            }
            HirExprKind::Pipe(pipe) => {
                self.lower_pipe_value(expression, pipe, continuation, overrides)
            }
            HirExprKind::Block(block) => {
                self.lower_value_block(block.statements(), block.tail(), continuation)
            }
            HirExprKind::NamedBlock(block) => {
                self.lower_value_block(block.statements(), block.tail(), continuation)
            }
            HirExprKind::ComputationBlock(block)
                if matches!(
                    block.kind(),
                    arcweft_lang_hir::expr::HirComputationBlockKind::Result
                        | arcweft_lang_hir::expr::HirComputationBlockKind::Option
                ) =>
            {
                self.lower_carrier_block(expression, block, continuation)
            }
            HirExprKind::Loop(loop_expression) => {
                self.lower_loop_value(expression, loop_expression, continuation)
            }
            _ => {
                let mut flow_child = None;
                for child in resolved.kind().direct_expression_children() {
                    if !overrides.contains_key(&child)
                        && self.contains_flow_value_expression(child)?
                    {
                        flow_child = Some(child);
                        break;
                    }
                }
                if let Some(child) = flow_child {
                    return self.lower_flow_value_with_overrides(
                        child,
                        RuntimeFlowValueContinuation::Compose {
                            owner: expression,
                            child,
                            overrides,
                            outer: Box::new(continuation),
                        },
                        BTreeMap::new(),
                    );
                }
                let value = FinalExprLowerer::new(
                    self.module,
                    self.facts,
                    self.locals,
                    self.pure_helpers,
                    self.trait_methods,
                    self.function_sites,
                    (self.pipe_locals, self.try_locals),
                )
                .with_overrides(overrides)
                .lower(expression)
                .map_err(RuntimePlanLowerError::new)?;
                self.apply_value_continuation(value, continuation)
            }
        }
    }

    fn lower_pipe_value(
        &mut self,
        owner: ExprId,
        pipe: &arcweft_lang_hir::expr::HirPipeExpr,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let inherited = overrides.clone();
        self.lower_flow_value_with_overrides(
            pipe.left(),
            RuntimeFlowValueContinuation::Pipe {
                owner,
                right: pipe.right(),
                overrides: inherited,
                outer: Box::new(continuation),
            },
            overrides,
        )
    }

    fn lower_choice_value(
        &mut self,
        owner: ExprId,
        choice: &arcweft_lang_hir::expr::HirChoiceExpr,
        _continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let result = self.facts.expression_type(owner).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Choice expression {owner:?} has no accepted result type"
            ))
        })?;
        if !matches!(result.shape(), RuntimeTypeShape::Never) {
            return Err(RuntimePlanLowerError::new(format!(
                "value-producing Choice expression {owner:?} requires typed runtime result ownership"
            )));
        }
        if choice.plan().is_some() {
            return Err(RuntimePlanLowerError::new(format!(
                "Choice expression {owner:?} lifecycle plan requires typed runtime ownership"
            )));
        }
        let fact = self.facts.choice(owner).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Choice expression {owner:?} has no checked runtime fact"
            ))
        })?;
        let id = fact.public_id().map(|id| id.as_str().to_owned());
        let mut options = Vec::with_capacity(choice.body().items().len());
        for (index, item) in choice.body().items().iter().enumerate() {
            let HirChoiceItem::CompactArm(arm) = item else {
                return Err(RuntimePlanLowerError::new(format!(
                    "Choice expression {owner:?} contains a candidate family without a typed core projection"
                )));
            };
            if arm.condition().is_some() {
                return Err(RuntimePlanLowerError::new(format!(
                    "Choice expression {owner:?} compact enabled state requires typed runtime ownership"
                )));
            }
            if !matches!(arm.action(), HirChoiceCompactAction::Goto(_)) {
                return Err(RuntimePlanLowerError::new(format!(
                    "Choice expression {owner:?} contains a non-goto compact action"
                )));
            }
            let arm_index = u32::try_from(index).map_err(|_| {
                RuntimePlanLowerError::new(format!(
                    "Choice expression {owner:?} contains too many compact arms"
                ))
            })?;
            let target = fact
                .goto_for_arm(arm_index)
                .and_then(crate::semantic_facts::RuntimeProjectItem::flow_runtime_id)
                .cloned()
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Choice expression {owner:?} arm {index} has no checked Flow target"
                    ))
                })?;
            let label = match self.facts.expression_literal(arm.label()) {
                Some(RuntimeValue::String(label)) => label.clone(),
                _ => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "Choice expression {owner:?} arm {index} label is not a checked static string"
                    )));
                }
            };
            options.push(RuntimeChoiceOptionSeed {
                id: Some(
                    fact.option_ids()
                        .get(index)
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "Choice expression {owner:?} arm {index} has no checked option identity"
                            ))
                        })?
                        .as_str()
                        .to_owned(),
                ),
                label,
                target: Some(target),
                out: None,
                effects: Vec::new(),
            });
        }
        Ok(vec![RuntimeFlowOpSeed::Choice { id, options }])
    }

    fn lower_loop_value(
        &mut self,
        owner: ExprId,
        expression: &arcweft_lang_hir::expr::HirLoopExpr,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let (result, tail) = match continuation {
            RuntimeFlowValueContinuation::Bind { pattern, tail } => (pattern, tail),
            RuntimeFlowValueContinuation::Ignore(tail) => {
                let ty = self.facts.expression_type(owner).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Loop expression {owner:?} has no accepted result type"
                    ))
                })?;
                (
                    RuntimePatternSeed::new(ty.identity(), RuntimePatternSeedKind::Discard),
                    tail,
                )
            }
            RuntimeFlowValueContinuation::Return
            | RuntimeFlowValueContinuation::Try { .. }
            | RuntimeFlowValueContinuation::WrapCarrier { .. }
            | RuntimeFlowValueContinuation::Compose { .. }
            | RuntimeFlowValueContinuation::Pipe { .. } => {
                return Err(RuntimePlanLowerError::new(format!(
                    "Loop expression {owner:?} requires a continuation result local"
                )));
            }
        };
        let mut ops = vec![RuntimeFlowOpSeed::Loop {
            result: Some(result),
            body: self.lower_statement_ids(expression.statements())?,
        }];
        ops.extend(self.lower_flow_tail(tail)?);
        Ok(ops)
    }

    fn lower_value_block(
        &mut self,
        statements: &[StmtId],
        tail: ExprId,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_statement_ids_with_tail(
            statements,
            RuntimeFlowTail::Value {
                expression: tail,
                continuation: Box::new(continuation),
            },
        )
    }

    fn lower_carrier_block(
        &mut self,
        expression: ExprId,
        block: &arcweft_lang_hir::expr::HirComputationBlockExpr,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        if self
            .carrier_continuations
            .insert(expression, continuation.clone())
            .is_some()
        {
            return Err(RuntimePlanLowerError::new(format!(
                "carrier block {expression:?} was entered more than once during lowering"
            )));
        }
        let tail = RuntimeFlowTail::Value {
            expression: block.tail(),
            continuation: Box::new(RuntimeFlowValueContinuation::WrapCarrier {
                owner: expression,
                outer: Box::new(continuation),
            }),
        };
        let lowered = self.lower_statement_ids_with_tail(block.statements(), tail);
        self.carrier_continuations.remove(&expression);
        lowered
    }

    fn lower_await_value(
        &mut self,
        expression: ExprId,
        awaited: &arcweft_lang_hir::expr::HirAwaitExpr,
        continuation: &RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let fact = self.facts.awaited(expression).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no checked runtime fact"
            ))
        })?;
        let locals = self.await_locals.get(&expression).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no admitted continuation locals"
            ))
        })?;
        let await_op = self.lower_await_operation(expression, awaited, &fact, &locals)?;
        let payload = self.facts.expression_type(expression).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no accepted payload type"
            ))
        })?;
        let mut ops = vec![await_op];
        ops.extend(
            self.apply_value_continuation(
                local_seed(payload, locals.payload),
                continuation.clone(),
            )?,
        );
        Ok(ops)
    }

    fn lower_await_operation(
        &mut self,
        expression: ExprId,
        awaited: &arcweft_lang_hir::expr::HirAwaitExpr,
        fact: &RuntimeAwaitFact,
        locals: &AwaitLocalSeeds,
    ) -> Result<RuntimeFlowOpSeed, RuntimePlanLowerError> {
        let operand = self
            .module
            .resolve_expr(awaited.operand())
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "cannot resolve Await operand {:?}: {error}",
                    awaited.operand()
                ))
            })?;
        let HirExprKind::Call(call) = operand.kind() else {
            return Err(RuntimePlanLowerError::new(format!(
                "Await operand {:?} is not a checked host call",
                awaited.operand()
            )));
        };
        let lowerer = FinalExprLowerer::new(
            self.module,
            self.facts,
            self.locals,
            self.pure_helpers,
            self.trait_methods,
            self.function_sites,
            (self.pipe_locals, self.try_locals),
        );
        let target = lowerer
            .lower_host_call_target(awaited.operand(), call)
            .map_err(RuntimePlanLowerError::new)?
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "Await operand {:?} is not a typed host call",
                    awaited.operand()
                ))
            })?;
        let ordinal = self.await_ordinal;
        self.await_ordinal = self
            .await_ordinal
            .checked_add(1)
            .ok_or_else(|| RuntimePlanLowerError::new("runtime Await ordinal overflow"))?;
        let owner = self.assertion_owner.label();
        let task = TaskId(format!("{owner}.await.{ordinal}"));
        let need = NeedId(format!("{owner}.need.{ordinal}"));
        let payload = self.facts.expression_type(expression).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no accepted payload type"
            ))
        })?;
        if awaited.branches().len() != fact.observers().len() {
            return Err(RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has {} authored observers but {} checked observers",
                awaited.branches().len(),
                fact.observers().len()
            )));
        }
        let mut observers = Vec::with_capacity(fact.observers().len());
        for (authored, checked) in awaited.branches().iter().zip(fact.observers()) {
            observers.push(RuntimeAwaitPendingObserverSeed {
                pattern: FinalPatternLowerer::new(self.module, self.facts, self.locals)
                    .lower(checked.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                ops: self.lower_contextual_body(authored.body())?,
            });
        }
        Ok(RuntimeFlowOpSeed::Await {
            binding: Some(bind_seed(payload, locals.payload.clone())),
            target: arcweft_core::plan::RuntimeAwaitTargetSeed {
                need,
                task,
                outcome: TaskOutcomeContract::new(
                    payload
                        .checked_type()
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                ),
                request: RuntimeHostTaskRequestTemplateSeed {
                    capability: HostCapabilityId(target.capability),
                    operation: target.operation,
                    args: target.args,
                },
            },
            observers,
        })
    }

    fn apply_value_continuation(
        &mut self,
        value: RuntimeExprSeed,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        Ok(match continuation {
            RuntimeFlowValueContinuation::Bind { pattern, tail } => {
                let mut ops = vec![RuntimeFlowOpSeed::Let {
                    pattern,
                    expr: value,
                }];
                ops.extend(self.lower_flow_tail(tail)?);
                ops
            }
            RuntimeFlowValueContinuation::Return => vec![RuntimeFlowOpSeed::ReturnExpr(value)],
            RuntimeFlowValueContinuation::Ignore(tail) => self.lower_flow_tail(tail)?,
            RuntimeFlowValueContinuation::Try { owner, outer } => {
                return self.lower_try_continuation(owner, value, *outer);
            }
            RuntimeFlowValueContinuation::WrapCarrier { owner, outer } => {
                let boundary = self.facts.expression_type(owner).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "carrier block {owner:?} has no accepted result type"
                    ))
                })?;
                let wrapped = RuntimeExprSeed::new(
                    boundary.identity(),
                    arcweft_core::plan::RuntimeExprSeedKind::Variant {
                        ordinal: 0,
                        payload: Some(Box::new(value)),
                    },
                );
                return self.apply_value_continuation(wrapped, *outer);
            }
            RuntimeFlowValueContinuation::Compose {
                owner,
                child,
                mut overrides,
                outer,
            } => {
                overrides.insert(child, value);
                return self.lower_flow_value_with_overrides(owner, *outer, overrides);
            }
            RuntimeFlowValueContinuation::Pipe {
                owner,
                right,
                mut overrides,
                outer,
            } => {
                let local = self.pipe_locals.get(&owner).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "once-only pipe {owner:?} has no admitted local"
                    ))
                })?;
                let pipe = self.facts.pipe(owner).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "once-only pipe {owner:?} has no checked fact"
                    ))
                })?;
                let local_type = self.facts.expression_type(pipe.left()).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "once-only pipe {owner:?} has no checked left type"
                    ))
                })?;
                let replacement = local_seed(local_type, local.clone());
                overrides.extend(
                    pipe.placeholders()
                        .iter()
                        .map(|placeholder| (*placeholder, replacement.clone())),
                );
                let mut ops = vec![RuntimeFlowOpSeed::Let {
                    pattern: bind_seed(local_type, local),
                    expr: value,
                }];
                ops.extend(self.lower_flow_value_with_overrides(right, *outer, overrides)?);
                return Ok(ops);
            }
        })
    }

    fn lower_try_continuation(
        &mut self,
        owner: ExprId,
        value: RuntimeExprSeed,
        outer: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let fact = self.facts.tried(owner).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Try expression {owner:?} has no checked runtime fact"
            ))
        })?;
        let locals = self.try_locals.get(&owner).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Try expression {owner:?} has no admitted continuation locals"
            ))
        })?;
        let success = local_seed(fact.carrier().success(), locals.success.clone());
        let success_ops = self.apply_value_continuation(success, outer)?;
        let (failure_pattern, failure_value) = match fact.carrier() {
            RuntimeTryCarrierFact::Result { residual, .. } => {
                let local = locals.residual.ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Result Try expression {owner:?} has no residual local"
                    ))
                })?;
                (
                    variant_bind_seed(fact.carrier_type(), 1, residual, local.clone()),
                    Some(local_seed(residual, local)),
                )
            }
            RuntimeTryCarrierFact::Option { .. } => {
                (variant_empty_seed(fact.carrier_type(), 1), None)
            }
        };
        let failure_ops = self.propagate_try_residual(&fact, failure_value)?;
        Ok(vec![RuntimeFlowOpSeed::Match {
            scrutinee: value,
            arms: vec![
                RuntimeFlowMatchArmSeed {
                    pattern: variant_bind_seed(
                        fact.carrier_type(),
                        0,
                        fact.carrier().success(),
                        locals.success,
                    ),
                    guard: None,
                    ops: success_ops,
                },
                RuntimeFlowMatchArmSeed {
                    pattern: failure_pattern,
                    guard: None,
                    ops: failure_ops,
                },
            ],
        }])
    }

    fn propagate_try_residual(
        &mut self,
        fact: &RuntimeTryFact,
        residual: Option<RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let propagated = RuntimeExprSeed::new(
            fact.boundary_type().identity(),
            arcweft_core::plan::RuntimeExprSeedKind::Variant {
                ordinal: 1,
                payload: residual.map(Box::new),
            },
        );
        match fact.boundary() {
            RuntimeTryBoundaryOwner::Infallible => Ok(Vec::new()),
            RuntimeTryBoundaryOwner::Callable(_) => {
                Ok(vec![RuntimeFlowOpSeed::ReturnExpr(propagated)])
            }
            RuntimeTryBoundaryOwner::FunctionSite(boundary) => {
                Err(RuntimePlanLowerError::new(format!(
                    "Try residual for function site {boundary:?} reached Flow continuation lowering"
                )))
            }
            RuntimeTryBoundaryOwner::CarrierBlock(boundary) => {
                let continuation = self
                    .carrier_continuations
                    .get(&boundary)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "Try residual targets inactive carrier block {boundary:?}"
                        ))
                    })?;
                self.apply_value_continuation(propagated, continuation)
            }
        }
    }

    fn lower_flow_tail(
        &mut self,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        match tail {
            RuntimeFlowTail::None => Ok(Vec::new()),
            RuntimeFlowTail::StatementsWithTail { statements, tail } => {
                self.lower_statement_ids_with_tail(&statements, *tail)
            }
            RuntimeFlowTail::ThreadItems(items) => self.lower_thread_items(&items),
            RuntimeFlowTail::Value {
                expression,
                continuation,
            } => self.lower_flow_value(expression, *continuation),
        }
    }

    fn lower_host_call(
        &mut self,
        expression: arcweft_lang_hir::identity::ExprId,
        binding: Option<RuntimePatternSeed>,
    ) -> Result<Option<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let Some((call_id, call)) = self.host_call_operand(expression)? else {
            return Ok(None);
        };
        let lowerer = FinalExprLowerer::new(
            self.module,
            self.facts,
            self.locals,
            self.pure_helpers,
            self.trait_methods,
            self.function_sites,
            (self.pipe_locals, self.try_locals),
        );
        lowerer
            .lower_host_call_target(call_id, call)
            .map(|target| target.map(|target| RuntimeFlowOpSeed::HostCall { binding, target }))
            .map_err(RuntimePlanLowerError::new)
    }

    fn host_call_operand(
        &self,
        expression: arcweft_lang_hir::identity::ExprId,
    ) -> Result<
        Option<(
            arcweft_lang_hir::identity::ExprId,
            &arcweft_lang_hir::expr::HirCallExpr,
        )>,
        RuntimePlanLowerError,
    > {
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve possible host expression {expression:?}: {error}"
            ))
        })?;
        match resolved.kind() {
            HirExprKind::Call(call) => Ok(Some((expression, call))),
            HirExprKind::Try(propagation) => self.host_call_operand(propagation.operand()),
            _ => Ok(None),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "assertion admission, ordered condition projection, guard derivation, and site publication form one identity-preserving transaction"
    )]
    fn lower_assertion(
        &mut self,
        statement: StmtId,
        hir_mode: HirAssertionMode,
        conditions: &[arcweft_lang_hir::identity::ExprId],
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let ordinal = self.assertion_ordinal;
        self.assertion_ordinal = self.assertion_ordinal.checked_add(1).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "runtime assertion ordinal overflow in declaration {}",
                self.assertion_owner.label()
            ))
        })?;

        let source_mode = hir_mode.resolved().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "recovered assertion mode for statement {statement:?} cannot enter runtime lowering"
            ))
        })?;
        let admission = self.facts.assertion(statement).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "checked assertion admission is missing for statement {statement:?}"
            ))
        })?;

        let runtime_mode = match admission {
            RuntimeAssertionAdmission::Discharged => {
                if source_mode != arcweft_lang_syntax::assertion::AssertionMode::Prove {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime assertion {statement:?} is discharged despite source mode {source_mode:?}"
                    )));
                }
                return Ok(Vec::new());
            }
            RuntimeAssertionAdmission::OmittedDebug => {
                if source_mode != arcweft_lang_syntax::assertion::AssertionMode::Debug {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime assertion {statement:?} is omitted despite source mode {source_mode:?}"
                    )));
                }
                return Ok(Vec::new());
            }
            RuntimeAssertionAdmission::Runtime(mode) => mode,
        };
        let source_runtime_mode = RuntimeAssertionMode::try_from_assertion_mode(source_mode)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        if source_runtime_mode != runtime_mode {
            return Err(RuntimePlanLowerError::new(format!(
                "checked runtime assertion mode {runtime_mode:?} does not match source mode {source_mode:?} for {statement:?}"
            )));
        }
        if !(1..=64).contains(&conditions.len()) {
            return Err(RuntimePlanLowerError::new(format!(
                "runtime assertion {statement:?} has invalid condition count {}",
                conditions.len()
            )));
        }

        let profile = match runtime_mode {
            RuntimeAssertionMode::Check => RuntimeAssertionProfile::Always,
            RuntimeAssertionMode::Debug => RuntimeAssertionProfile::DebugOnly,
        };
        let statement_span = self.source_span(
            &HirSourceQuery::Stmt {
                owner: statement,
                role: HirStmtSourceRole::Whole,
            },
            "assertion statement",
        )?;
        let mut ops = Vec::with_capacity(conditions.len());
        let mut sites = Vec::with_capacity(conditions.len());
        for (index, condition) in conditions.iter().copied().enumerate() {
            let condition_index = AssertionConditionIndex::try_new(index, conditions.len())
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let condition_span = self.source_span(
                &HirSourceQuery::Expr {
                    owner: condition,
                    role: HirExprSourceRole::Whole,
                },
                "assertion condition",
            )?;
            let range = condition_span.range();
            let condition_label = self
                .module
                .provenance()
                .document()
                .text()
                .get(range.start()..range.end())
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "assertion condition {condition:?} source span is outside its accepted document"
                    ))
                })?;
            let guard = match &self.assertion_owner {
                RuntimeAssertionOwner::Callable(declaration) => {
                    crate::assertion_lower::derive_runtime_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        declaration,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
                RuntimeAssertionOwner::Flow(flow) => {
                    crate::assertion_lower::derive_runtime_flow_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        flow,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
            };
            let condition_expr = FinalExprLowerer::new(
                self.module,
                self.facts,
                self.locals,
                self.pure_helpers,
                self.trait_methods,
                self.function_sites,
                (self.pipe_locals, self.try_locals),
            )
            .lower(condition)
            .map_err(RuntimePlanLowerError::new)?;
            let mode_label = match runtime_mode {
                RuntimeAssertionMode::Check => "check",
                RuntimeAssertionMode::Debug => "debug",
            };
            let message = format!("assert.{mode_label} condition {index} failed");
            ops.push(RuntimeFlowOpSeed::EvaluatedEffect(
                RuntimeEvaluatedEffectSeed::Assert {
                    guard,
                    condition: condition_expr,
                    message,
                    profile,
                },
            ));
            sites.push(RuntimeAssertionSite::new(
                guard,
                statement,
                condition_index,
                runtime_mode,
                condition_span,
                AssertionPresentation::new(
                    statement_span.clone(),
                    Arc::<str>::from(condition_label),
                ),
            ));
        }
        self.assertion_sites.extend(sites);
        Ok(ops)
    }

    fn source_span(
        &self,
        query: &HirSourceQuery,
        role: &str,
    ) -> Result<SourceSpan, RuntimePlanLowerError> {
        let lookup = self
            .module
            .source_site(self.module.provenance().source_identity(), query.clone())
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "cannot resolve exact final-HIR {role} source for {query:?}: {error}"
                ))
            })?;
        match lookup.presence() {
            HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
            HirSourcePresence::Present(HirSourceSite::Insertion(_))
            | HirSourcePresence::AbsentOptional => Err(RuntimePlanLowerError::new(format!(
                "executable final-HIR {role} for {query:?} has no authored source span"
            ))),
        }
    }

    fn lower_statement_ids(
        &mut self,
        statements: &[StmtId],
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_statement_ids_with_tail(statements, RuntimeFlowTail::None)
    }

    fn lower_statement_ids_with_tail(
        &mut self,
        statements: &[StmtId],
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let Some((statement, remaining)) = statements.split_first() else {
            return self.lower_flow_tail(tail);
        };
        let kind = self.resolve_statement(*statement)?.kind().clone();
        let next = if remaining.is_empty() {
            tail
        } else {
            RuntimeFlowTail::StatementsWithTail {
                statements: remaining.to_vec().into_boxed_slice(),
                tail: Box::new(tail),
            }
        };
        self.lower_statement_with_tail(*statement, &kind, next)
    }

    fn lower_contextual_body(
        &mut self,
        body: &HirContextualStmtBody,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        match body {
            HirContextualStmtBody::Ordinary { statements, .. } => {
                self.lower_statement_ids(statements)
            }
            HirContextualStmtBody::Thread(body) => self.lower_body_as_one_error(body),
        }
    }

    fn lower_else_branch(
        &mut self,
        branch: &HirConditionalElseBranch,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        match branch {
            HirConditionalElseBranch::Body(body) => self.lower_contextual_body(body),
            HirConditionalElseBranch::ElseIf(statement) => {
                let kind = self.resolve_statement(*statement)?.kind().clone();
                self.lower_statement(*statement, &kind)
            }
        }
    }

    fn lower_body_as_one_error(
        &mut self,
        body: &HirThreadBody,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_body(body).map_err(|errors| {
            RuntimePlanLowerError::new(
                errors
                    .into_iter()
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })
    }
}

fn lower_evaluated_effect(
    expr: &FinalExprLowerer<'_>,
    effect: &RuntimeEvaluatedEffect,
) -> Result<RuntimeEvaluatedEffectSeed, RuntimePlanLowerError> {
    let lower = |expression| expr.lower(expression).map_err(RuntimePlanLowerError::new);
    let fields = |fields: &[crate::semantic_facts::RuntimeEffectFieldFact]| {
        fields
            .iter()
            .map(|field| {
                Ok(RuntimeEffectFieldSeed {
                    name: field.name().to_owned(),
                    value: lower(field.value())?,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
    };
    Ok(match effect {
        RuntimeEvaluatedEffect::Log {
            level,
            message,
            fields: effect_fields,
        } => RuntimeEvaluatedEffectSeed::Log {
            level: level.as_str().to_owned(),
            message: lower(*message)?,
            fields: fields(effect_fields)?,
        },
        RuntimeEvaluatedEffect::SignalWrite { target, value } => {
            RuntimeEvaluatedEffectSeed::SignalWrite {
                target: lower(*target)?,
                value: lower(*value)?,
            }
        }
        RuntimeEvaluatedEffect::MetricWrite { target, value } => {
            RuntimeEvaluatedEffectSeed::MetricWrite {
                target: lower(*target)?,
                value: lower(*value)?,
            }
        }
        RuntimeEvaluatedEffect::EmitEvent {
            event,
            fields: effect_fields,
        } => RuntimeEvaluatedEffectSeed::EmitEvent {
            event: lower(*event)?,
            fields: fields(effect_fields)?,
        },
        RuntimeEvaluatedEffect::Panic { message } => {
            RuntimeEvaluatedEffectSeed::Panic(lower(*message)?)
        }
        RuntimeEvaluatedEffect::Fail { message } => {
            RuntimeEvaluatedEffectSeed::Fail(lower(*message)?)
        }
        RuntimeEvaluatedEffect::Bail { message } => {
            RuntimeEvaluatedEffectSeed::Bail(lower(*message)?)
        }
        RuntimeEvaluatedEffect::Ensure { condition, message } => {
            RuntimeEvaluatedEffectSeed::Ensure {
                condition: lower(*condition)?,
                message: lower(*message)?,
            }
        }
    })
}

fn thread_item_matches_kind(item: &HirThreadFlowItem, kind: &HirStmtKind) -> bool {
    match item {
        HirThreadFlowItem::DialogueApplication(_) => false,
        HirThreadFlowItem::Statement(_) => !matches!(
            kind,
            HirStmtKind::Choice { .. }
                | HirStmtKind::If(_)
                | HirStmtKind::IfLet(_)
                | HirStmtKind::Match(_)
                | HirStmtKind::While(_)
                | HirStmtKind::WhileLet(_)
                | HirStmtKind::For(_)
                | HirStmtKind::Select(_)
                | HirStmtKind::SourceLocale(_)
                | HirStmtKind::Scope(_)
                | HirStmtKind::Include(_)
                | HirStmtKind::Error
        ),
        HirThreadFlowItem::Choice(_) => matches!(kind, HirStmtKind::Choice { .. }),
        HirThreadFlowItem::If(_) => matches!(kind, HirStmtKind::If(_)),
        HirThreadFlowItem::IfLet(_) => matches!(kind, HirStmtKind::IfLet(_)),
        HirThreadFlowItem::Match(_) => matches!(kind, HirStmtKind::Match(_)),
        HirThreadFlowItem::While(_) => matches!(kind, HirStmtKind::While(_)),
        HirThreadFlowItem::WhileLet(_) => matches!(kind, HirStmtKind::WhileLet(_)),
        HirThreadFlowItem::For(_) => matches!(kind, HirStmtKind::For(_)),
        HirThreadFlowItem::Select(_) => matches!(kind, HirStmtKind::Select(_)),
        HirThreadFlowItem::SourceLocale(_) => matches!(kind, HirStmtKind::SourceLocale(_)),
        HirThreadFlowItem::Scope(_) => matches!(kind, HirStmtKind::Scope(_)),
        HirThreadFlowItem::Include(_) => matches!(kind, HirStmtKind::Include(_)),
        HirThreadFlowItem::Error(_) => matches!(kind, HirStmtKind::Error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_core::{
        entry::{EntryBindingIdentity, RuntimeEntryRoles},
        plan::{
            EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
            RuntimeEntryTarget,
        },
    };
    use arcweft_lang_hir::database::HirDatabase;
    use arcweft_lang_hir::item::HirItemKind;
    use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
    use arcweft_lang_hir::project::{
        HirProject, HirProjectBuilder, HirProjectModule, HirRuntimeExpressionTypeDisposition,
    };
    use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
    use arcweft_lang_hir::symbol::{
        CallablePackageId, ProjectSymbolRevision, ProjectSymbolWorldId,
    };
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
    use arcweft_lang_syntax::incremental::SyntaxDatabase;
    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        RuntimeCheckedEntryInput, RuntimeEntryLoweringInput, lower_runtime_plan_with_stats,
    };
    use crate::semantic_facts::{
        RuntimeNormalizedType, RuntimePlanSemanticFactInput, RuntimePlanSemanticFacts,
        RuntimeSemanticTypeId, RuntimeTypeShape,
    };

    #[test]
    fn empty_flow_lowers_only_with_its_checked_core_identity() {
        let project = project_fixture("empty-flow", "flow opening {}\n");
        let executable = project.executable_view().expect("executable fixture");
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = complete_type_input(executable);
        input.push_flow(owner, identity.clone());
        let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("checked facts");
        let entry_input = RuntimeEntryLoweringInput::empty(executable);
        let report = lower_runtime_plan_with_stats(executable, &facts, &entry_input)
            .expect("empty Flow lowers");
        assert_eq!(report.plan.flows().len(), 1);
        assert_eq!(report.plan.flows()[0].id, identity);
        assert!(report.plan.flows()[0].ops.is_empty());
    }

    #[test]
    fn thread_expression_statement_lowers_through_the_sole_expression_owner() {
        let project = project_fixture(
            "thread-expression-statement",
            "flow opening {\n    thread {\n    }\n}\n",
        );
        let executable = project.executable_view().expect("executable fixture");
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = complete_type_input(executable);
        input.push_flow(owner, identity);
        let facts = RuntimePlanSemanticFacts::try_new(executable, input).expect("checked facts");
        let report = lower_runtime_plan_with_stats(
            executable,
            &facts,
            &RuntimeEntryLoweringInput::empty(executable),
        )
        .expect("Thread expression statement lowers");

        let [FlowOp::Thread { name, body }] = report.plan.flows()[0].ops.as_slice() else {
            panic!("ordinary expression statement must project its typed Thread payload")
        };
        assert!(name.is_none());
        assert!(body.is_empty());
    }

    #[test]
    fn final_entry_requires_and_consumes_its_exact_checked_hir_owner() {
        let project = project_fixture(
            "checked-entry-owner",
            "flow @flow.main main {}\nentry cli @entry.cli.main { goto @flow.main }\n",
        );
        let executable = project.executable_view().expect("executable fixture");
        let flow_owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let entry_owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Entry(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Entry item");
        let flow =
            FlowRuntimeId::from_source_entity_body("flow.main").expect("runtime Flow identity");
        let mut fact_input = complete_type_input(executable);
        fact_input.push_flow(flow_owner, flow.clone());
        let facts =
            RuntimePlanSemanticFacts::try_new(executable, fact_input).expect("checked facts");

        let missing = RuntimeEntryLoweringInput::empty(executable);
        let errors = lower_runtime_plan_with_stats(executable, &facts, &missing)
            .expect_err("an Entry cannot be silently omitted from the checked input");
        assert!(errors.iter().any(|error| {
            error
                .to_string()
                .contains("absent from the checked runtime Entry input")
        }));

        let runtime_entry = RuntimeEntrySpec {
            id: EntryRuntimeId::from_source_entity_body("entry.cli.main")
                .expect("runtime Entry identity"),
            kind: RuntimeEntryKind::Cli,
            binding: EntryBindingIdentity::from_bytes([7; 32]),
            target: RuntimeEntryTarget::Flow(flow),
            roles: RuntimeEntryRoles::None,
        };
        let input = RuntimeEntryLoweringInput::new(
            executable,
            vec![RuntimeCheckedEntryInput::new(entry_owner, runtime_entry)],
            Vec::new(),
            Vec::new(),
        );
        let report = lower_runtime_plan_with_stats(executable, &facts, &input)
            .expect("exact checked Entry owner lowers");
        assert_eq!(report.plan.entries().len(), 1);
        assert_eq!(report.plan.flows().len(), 1);
    }

    fn project_fixture(label: &str, source: &str) -> HirProject {
        let package = CallablePackageId::try_new(format!("runtime-plan-final-flow-{label}"))
            .expect("fixture package");
        let path = CanonicalModulePath::crate_root();
        let source_name = SourceName::path(format!("runtime-plan-final-flow-{label}.arcw"));
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("arcweft-test://runtime-plan/flow/{label}"))
                    .expect("fixture document ID"),
                source_name.clone(),
                source,
            )
            .expect("fixture document"),
        );
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let parsed = syntax
            .parse_initial(
                SourceSnapshotId::initial(source_name),
                document,
                arcweft_lang_syntax::parser::ParseOptions::default(),
            )
            .expect("attached fixture parse");
        let key = HirModuleKey::new(
            package.clone(),
            path.clone(),
            parsed.document().identity().clone(),
        );
        let mut database = HirDatabase::try_new().expect("HIR database");
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            parsed.document().identity().id().clone(),
            "runtime-plan-final-flow-test",
        )
        .expect("fixture symbol world");
        let revision = ProjectSymbolRevision::try_for_documents([parsed.document().identity()])
            .expect("fixture symbol revision");
        let transaction = database
            .stage_proof_return_project(
                [LoweringRequest::try_new(key, &parsed).expect("lower request")],
                world,
                revision,
                [parsed.document().identity()],
                arcweft_lang_hir::lowering::HirLoweringControl::new(),
            )
            .expect("final HIR project stages");
        let facts = HirProofReturnSemanticFactSet::try_new(
            Arc::clone(transaction.generation()),
            transaction.headers().cloned(),
            [],
        )
        .expect("runtime-plan fixture has no authored Proof return headers");
        let mut outputs = transaction
            .publish_with_semantic_facts(&mut database, facts)
            .expect("final HIR project publishes");
        let module = outputs
            .pop()
            .expect("one runtime-plan fixture module")
            .into_module();
        assert!(outputs.is_empty());
        let project_module = HirProjectModule::try_new(
            &database,
            &package,
            &path,
            parsed.document().identity(),
            module,
        )
        .expect("accepted module lease");
        let mut builder = HirProjectBuilder::new(&database, package);
        builder
            .insert_module(project_module)
            .expect("module insertion");
        builder.finish().expect("fixture project")
    }

    fn complete_type_input(
        project: arcweft_lang_hir::project::HirExecutableProjectView<'_>,
    ) -> RuntimePlanSemanticFactInput {
        let mut input = RuntimePlanSemanticFactInput::new();
        let runtime_owners = project
            .runtime_semantic_owner_inventory()
            .expect("runtime semantic owner inventory");
        for owner in runtime_owners.locals() {
            input.push_local_declaration(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
        }
        for owner in runtime_owners.patterns() {
            input.push_pattern_type(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
        }
        for owner in runtime_owners
            .selected_expression_type_owners(
                |_| None,
                |_| HirRuntimeExpressionTypeDisposition::Retain,
            )
            .expect("postfix-free runtime expression-type fixture")
        {
            input.push_expression_type(
                owner,
                RuntimeNormalizedType::new(
                    RuntimeSemanticTypeId::from_bytes([0x11; 32]),
                    RuntimeTypeShape::Unit,
                ),
            );
        }
        input
    }
}
