//! Runtime-plan lowering from one accepted final-HIR project generation.

#[path = "final_flow/control_locals.rs"]
mod control_locals;
#[path = "final_flow/line_plan.rs"]
mod line_plan;
#[path = "final_flow/value_branches.rs"]
mod value_branches;

use control_locals::ControlLocals;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::effect::{RuntimeArtifactFingerprint, RuntimeAssertionProfile};
use arcweft_core::entry::{
    FlowParameterCoordinate, RuntimeCallableId, RuntimeCallableRole, RuntimeFlowExecutable,
    RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowSchema,
};
use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeAwaitPendingObserverSeed, RuntimeBuiltinIteratorEvidenceSeed,
    RuntimeChoiceOptionSeed, RuntimeDialogueContentPlanSeedId, RuntimeDropPolicySeed,
    RuntimeEffectFieldSeed, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEvaluatedEffectSeed,
    RuntimeExprSeed, RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFunctionEffectSet, RuntimeFunctionExecutableBodySeed, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostTaskRequestTemplateSeed, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed, RuntimeLineId,
    RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePlan, RuntimePlanBuilder, RuntimeProjectCallAbiSeed,
    RuntimeProjectCallAttachedMaterializationSeed, RuntimeProjectCallAttachedPresenceSeed,
    RuntimeProjectCallDefaultCaptureSource, RuntimeProjectCallDefaultFunctionSeed,
    RuntimeProjectCallFixedMaterializationSeed, RuntimeProjectCallInputSeed,
    RuntimeProjectCallOperandSeed, RuntimeProjectCallOrdinaryMaterializationSeed,
    RuntimeProjectCallOutcomeSeed, RuntimeProjectCallPlanSeed,
    RuntimeProjectCallRestMaterializationSeed, RuntimePureHelperDeclarationSeed,
    RuntimePureHelperOrigin, RuntimePureHelperSeedId, RuntimePureInputType, RuntimePureOutputType,
    RuntimePureProgramBindingSeed, RuntimeReceiverMode, RuntimeTraitMethodDeclarationSeed,
    RuntimeTraitMethodIdentity, RuntimeTraitMethodSeedId,
};
use arcweft_core::task::{HostCapabilityId, NeedId, TaskId, TaskOutcomeContract};
use arcweft_core::value::{
    RuntimeCallArgumentMode, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};
use arcweft_lang_hir::expr::{
    HirChoiceCompactAction, HirChoiceItem, HirExprKind, HirThreadBody, HirThreadFlowItem,
    HirThreadMode,
};
use arcweft_lang_hir::identity::{ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, StmtId};
use arcweft_lang_hir::item::{
    HirEntryDeclaration, HirEntryId, HirEntryKind, HirFlowItem, HirFunctionBody, HirImplFunction,
    HirImplMember, HirItemKind, HirMethodParameter, HirMethodParameterGroup, HirMethodReceiverKind,
    HirParameter, HirParameterKind,
};
use arcweft_lang_hir::leaf::HirIdRef;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::pattern::{HirPatternBinding, HirPatternKind};
use arcweft_lang_hir::project::{HirExecutableProjectView, HirRuntimeExecutableOwner};
use arcweft_lang_hir::source_index::{
    HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
};
use arcweft_lang_hir::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirStmtKind,
    HirStmtMatchArmBody,
};
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallablePackageId, ImplMethodDeclarationId,
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
use crate::final_variant::{
    normalized_variant_binding_pattern_seed, normalized_variant_expression_seed,
};
use crate::semantic_facts::{
    RuntimeAssertionAdmission, RuntimeAwaitFact, RuntimeClosureInstanceFact,
    RuntimeClosureInstanceKey, RuntimeDialogueApplication, RuntimeDialogueEffectCaptureKey,
    RuntimeDialogueEffectProgramKey, RuntimeDialogueValueCaptureKey, RuntimeDropFadeFact,
    RuntimeDropPolicyFact, RuntimeEffectFieldFact, RuntimeEvaluatedEffect,
    RuntimeEvaluatedEffectFact, RuntimeEvaluatedEffectOperandFact, RuntimeExecutableSemanticScope,
    RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact, RuntimeNormalizedType,
    RuntimePlanSemanticFacts, RuntimeProjectCallable, RuntimeProjectFunctionExpressionPayload,
    RuntimeProjectFunctionInstanceFact, RuntimeProjectFunctionInstanceKey,
    RuntimeProjectFunctionInstanceSemanticFacts, RuntimeProjectFunctionParameterSource,
    RuntimeProjectFunctionTypeOwner, RuntimeProjectFunctionTypeProjection,
    RuntimeResolvedAttachedContent, RuntimeResolvedCall, RuntimeResolvedCallDispatch,
    RuntimeResolvedCallOperandProjection, RuntimeResolvedStaticCallTarget, RuntimeResolvedValue,
    RuntimeScopedExecutableSemanticFactView, RuntimeSemanticFactsError, RuntimeTraitIdentity,
    RuntimeTraitMethodFact, RuntimeTryBoundaryOwner, RuntimeTryCarrierFact, RuntimeTryFact,
    RuntimeTypeShape,
};

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
    /// Capture-free structured function-site code. The site carries the
    /// synthetic parameter inputs and exact HIR pattern prologue.
    FunctionSite,
    ControllerFlow(arcweft_core::plan::FlowRuntimeId),
}

/// Exact callable identity, final-HIR owner, and checked role metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeEntryCallableInput {
    callable: RuntimeProjectCallable,
    role: RuntimeCallableRole,
    body: RuntimeEntryCallableBody,
}

impl RuntimeEntryCallableInput {
    pub const fn new(
        callable: RuntimeProjectCallable,
        role: RuntimeCallableRole,
        body: RuntimeEntryCallableBody,
    ) -> Self {
        Self {
            callable,
            role,
            body,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        self.callable.declaration()
    }

    pub const fn owner(&self) -> ItemId {
        self.callable.owner()
    }

    pub const fn callable(&self) -> &RuntimeProjectCallable {
        &self.callable
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
    expected_schema: RuntimeFlowSchema,
}

impl RuntimeEntryFlowInput {
    pub const fn new(
        owner: ItemId,
        executable: RuntimeFlowExecutable,
        expected_schema: RuntimeFlowSchema,
    ) -> Self {
        Self {
            owner,
            executable,
            expected_schema,
        }
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn executable(&self) -> &RuntimeFlowExecutable {
        &self.executable
    }

    pub const fn expected_schema(&self) -> &RuntimeFlowSchema {
        &self.expected_schema
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
struct ReservedProjectFunctionDefinition<'facts> {
    instance: &'facts RuntimeProjectFunctionInstanceFact,
    site: RuntimeFunctionSiteSeedId,
}

#[derive(Clone)]
struct ReservedProjectDefaultFunctionDefinition<'facts> {
    instance: &'facts RuntimeProjectFunctionInstanceFact,
    site: RuntimeFunctionSiteSeedId,
}

#[derive(Clone)]
struct ReservedClosureDefinition<'facts> {
    closure: &'facts RuntimeClosureInstanceFact,
    site: RuntimeFunctionSiteSeedId,
}

#[derive(Clone, Default)]
struct ProjectFunctionFrameLocals {
    control: ControlLocals,
    hir: BTreeMap<LocalId, RuntimeLocalSeedId>,
    parameter_inputs: BTreeMap<(u32, u32), RuntimeLocalSeedId>,
    attached_abi: Option<RuntimeLocalSeedId>,
    /// Instance-owned source-row ANF locals.  This map is intentionally not
    /// populated from the global call inventory: a closed project instance
    /// must receive its own substituted call projection before a specialized
    /// structural payload can be lowered.
    specialized_operands: BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
}

#[derive(Clone, Copy)]
enum ProjectFunctionFrameLocal {
    Hir(LocalId),
    ParameterInput { group: u32, parameter: u32 },
    AttachedAbi,
    SpecializedOperand { owner: ExprId, source_index: u32 },
}

#[derive(Clone, Default)]
struct ClosureFrameLocals {
    control: ControlLocals,
    hir: BTreeMap<LocalId, RuntimeLocalSeedId>,
    parameter_inputs: BTreeMap<u32, RuntimeLocalSeedId>,
    capture_inputs: BTreeMap<u32, RuntimeLocalSeedId>,
    specialized_operands: BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
}

#[derive(Clone, Copy)]
enum ClosureFrameLocal {
    Hir(LocalId),
    ParameterInput { position: u32 },
    CaptureInput { position: u32 },
    SpecializedOperand { owner: ExprId, source_index: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClosureLexicalParent {
    Global,
    ProjectFunction(RuntimeProjectFunctionInstanceKey),
    Closure(RuntimeClosureInstanceKey),
}

#[derive(Clone)]
struct ReservedPureProgramDefinition {
    closure: ExprId,
    module: HirModuleId,
    body: ExprId,
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
struct PendingDialogueEffectDefinition<'facts> {
    key: RuntimeDialogueEffectProgramKey,
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    module: HirModuleId,
    site: RuntimeFunctionSiteSeedId,
    effects: RuntimeFunctionEffectSet,
    operation: RuntimeEvaluatedEffectFact,
}

type ControllerResultLocalKey = RuntimeCallableId;
type ProjectDefaultCaptureInputKey = (RuntimeProjectFunctionInstanceKey, u32);

#[derive(Clone)]
struct PendingDialogueContentDefinition<'facts> {
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    owner: ExprId,
    line: arcweft_core::plan::RuntimeLineId,
    template: arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed,
    values: Vec<(
        arcweft_core::runtime_id::RuntimeDialogueValueSlotId,
        arcweft_core::plan::RuntimeDialogueValueRole,
        RuntimeFunctionSiteSeedId,
        Box<[RuntimeExprSeed]>,
    )>,
    effect_sites: Vec<(
        arcweft_core::runtime_id::RuntimeDialogueEffectSiteId,
        RuntimeFunctionSiteSeedId,
        Box<[RuntimeExprSeed]>,
    )>,
    marks: Box<[String]>,
    effect_site_count: arcweft_core::runtime_id::RuntimeDialogueEffectSiteCount,
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
    project_function_sites:
        &'data BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    project_default_function_sites:
        &'data BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    project_function_locals:
        &'data BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    closure_sites: &'data BTreeMap<RuntimeClosureInstanceKey, RuntimeFunctionSiteSeedId>,
    closure_locals: &'data BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    trait_methods: &'data BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'data BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    dialogue_effect_sites:
        &'data BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    dialogue_value_capture_input_locals:
        &'data BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    dialogue_effect_capture_input_locals:
        &'data BTreeMap<RuntimeDialogueEffectCaptureKey, RuntimeLocalSeedId>,
    dialogue_content: &'data BTreeMap<
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
        RuntimeDialogueContentPlanSeedId,
    >,
    control: &'data ControlLocals,
    controller_result_locals: &'data BTreeMap<ControllerResultLocalKey, RuntimeLocalSeedId>,
    specialized_operand_locals: &'data BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
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
            self.trait_methods,
            self.function_sites,
            self.dialogue_effect_sites,
            (&self.control.pipes, &self.control.tries),
        )
        .with_specialized_operand_locals(self.specialized_operand_locals)
        .with_closure_sites(self.closure_sites)
    }

    fn scoped_expr_lowerer<'a>(
        &'a self,
        module: &'a HirModule,
        scope: RuntimeScopedExecutableSemanticFactView<'a>,
    ) -> Result<FinalExprLowerer<'a>, RuntimePlanLowerError> {
        let control = self.dialogue_control_locals(scope.scope())?;
        Ok(self
            .expr_lowerer(module)
            .with_locals(self.dialogue_locals(scope.scope())?)
            .with_control_locals(&control.pipes, &control.tries)
            .with_specialized_operand_locals(
                self.dialogue_specialized_operand_locals(scope.scope())?,
            )
            .with_scoped_semantics(scope))
    }

    fn dialogue_control_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&ControlLocals, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.control),
            RuntimeExecutableSemanticScope::ProjectFunction(key) => self
                .project_function_locals
                .get(key)
                .map(|frame| &frame.control)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("dialogue function frame has no control locals")
                }),
            RuntimeExecutableSemanticScope::Closure(key) => self
                .closure_locals
                .get(key)
                .map(|frame| &frame.control)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("dialogue closure frame has no control locals")
                }),
        }
    }

    fn dialogue_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&BTreeMap<LocalId, RuntimeLocalSeedId>, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.locals),
            RuntimeExecutableSemanticScope::ProjectFunction(key) => self
                .project_function_locals
                .get(key)
                .map(|locals| &locals.hir)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue project-function scope {:?} has no admitted local frame",
                        key
                    ))
                }),
            RuntimeExecutableSemanticScope::Closure(key) => self
                .closure_locals
                .get(key)
                .map(|locals| &locals.hir)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue project-closure scope {:?} has no admitted local frame",
                        key
                    ))
                }),
        }
    }

    fn dialogue_specialized_operand_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&BTreeMap<(ExprId, u32), RuntimeLocalSeedId>, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.specialized_operand_locals),
            RuntimeExecutableSemanticScope::ProjectFunction(key) => self
                .project_function_locals
                .get(key)
                .map(|locals| &locals.specialized_operands)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue project-function scope {:?} has no admitted specialized operand frame",
                        key
                    ))
                }),
            RuntimeExecutableSemanticScope::Closure(key) => self
                .closure_locals
                .get(key)
                .map(|locals| &locals.specialized_operands)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue project-closure scope {:?} has no admitted specialized operand frame",
                        key
                    ))
                }),
        }
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

    let mut type_seeds = facts
        .runtime_plan_type_seeds()
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    for callable in &entry_input.callables {
        if let Some(attached) = callable.callable().attached_content_abi() {
            attached
                .append_runtime_plan_type_seeds(&mut type_seeds)
                .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
        }
    }
    let local_facts = facts.local_declarations().collect::<Vec<_>>();
    let mut local_seeds = local_facts
        .iter()
        .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(ty.identity()))
        .collect::<Vec<_>>();
    let implicit_callable_facts = facts.implicit_callables().collect::<Vec<_>>();
    local_seeds.extend(
        implicit_callable_facts
            .iter()
            .map(|(_, callable)| RuntimeLocalDeclarationSeed::new(callable.parameter().identity())),
    );
    let controller_result_local_specs = entry_input
        .callables
        .iter()
        .filter(|callable| matches!(callable.body(), RuntimeEntryCallableBody::ControllerFlow(_)))
        .map(|callable| {
            let root = facts
                .project_function_root_for_callable(&callable.role().callable)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Entry controller `{}` has no checked project-function root",
                        callable.role().callable.as_str()
                    ))
                })?;
            let instance = facts
                .project_function_instance(root.instance())
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "Entry controller `{}` project-function root instance is absent",
                        callable.role().callable.as_str()
                    ))
                })?;
            let RuntimeTypeShape::Function { result, parameters } =
                instance.function_type().shape()
            else {
                return Err(RuntimePlanLowerError::new(format!(
                    "Entry controller `{}` root instance is not a function",
                    callable.role().callable.as_str()
                )));
            };
            if !parameters.is_empty()
                || instance.callable().attached_content_abi().is_some()
                || instance.key().group().get() != 0
            {
                return Err(RuntimePlanLowerError::new(format!(
                    "Entry controller `{}` root instance does not have the empty group-0 ABI",
                    callable.role().callable.as_str()
                )));
            }
            Ok((callable.role().callable.clone(), result.identity()))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![error])?;
    local_seeds.extend(
        controller_result_local_specs
            .iter()
            .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
    );
    let project_instance_expression_owners = facts
        .project_function_instances()
        .flat_map(|instance| instance.semantics().type_projection().iter())
        .filter_map(|projection| {
            matches!(
                projection.owner(),
                RuntimeProjectFunctionTypeOwner::Expression(_)
            )
            .then_some(projection.owner())
        })
        .filter_map(|owner| match owner {
            RuntimeProjectFunctionTypeOwner::Expression(expression) => Some(expression),
            RuntimeProjectFunctionTypeOwner::Pattern(_)
            | RuntimeProjectFunctionTypeOwner::Local(_)
            | RuntimeProjectFunctionTypeOwner::Type(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let specialized_operand_local_specs = facts
        .calls()
        .filter(|(expression, call)| {
            call.requires_specialized_operand_anf()
                && !project_instance_expression_owners.contains(expression)
        })
        .try_fold(
            Vec::new(),
            |mut specs: Vec<((ExprId, u32), RuntimeSemanticTypeId)>, (expression, call)| {
                if call.attached_content().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "specialized call {expression:?} cannot carry attached content"
                    )));
                }
                for (source_index, operand) in call.operands().iter().enumerate() {
                    let source_index = u32::try_from(source_index).map_err(|_| {
                        RuntimePlanLowerError::new(format!(
                            "specialized call {expression:?} source operand index exceeds checked limits"
                        ))
                    })?;
                    specs.push(((expression, source_index), operand.ty().identity()));
                }
                Ok(specs)
            },
        )
        .map_err(|error| vec![error])?;
    local_seeds.extend(
        specialized_operand_local_specs
            .iter()
            .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
    );
    // Generic project-function locals are deliberately absent from the shared
    // semantic local table: their checked types are open until a terminal
    // callable instance closes the substitution.  Admit one plan-local frame
    // row per instance/type-projection owner instead of reusing an open
    // declaration-local seed.
    let project_instance_local_specs = facts
        .project_function_instances()
        .map(|instance| -> Result<_, RuntimePlanLowerError> {
            let locals = instance
                .semantics()
                .type_projection()
                .iter()
                .filter_map(|projection| match projection {
                    RuntimeProjectFunctionTypeProjection::Value {
                        owner: RuntimeProjectFunctionTypeOwner::Local(local),
                        ty,
                    } => Some((ProjectFunctionFrameLocal::Hir(*local), ty.identity())),
                    RuntimeProjectFunctionTypeProjection::Value { .. }
                    | RuntimeProjectFunctionTypeProjection::SemanticOnlyExpression { .. } => None,
                })
                .collect::<Vec<_>>();
            let mut locals = locals;
            for parameter in instance.parameters() {
                let group = u32::try_from(parameter.group().get()).map_err(|_| {
                    RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} parameter group exceeds checked limits",
                        instance.key()
                    ))
                })?;
                locals.push((
                    ProjectFunctionFrameLocal::ParameterInput {
                        group,
                        parameter: parameter.parameter(),
                    },
                    parameter.binding_ty().identity(),
                ));
            }
            if let Some(attached) = instance.callable().attached_content_abi()
                && attached.group() == instance.key().group()
            {
                locals.push((
                    ProjectFunctionFrameLocal::AttachedAbi,
                    attached.binding_ty().identity(),
                ));
            }
            for expression in instance.semantics().expressions() {
                let Some(call) = instance.semantics().call(expression.owner()) else {
                    continue;
                };
                if !call.requires_specialized_operand_anf() {
                    continue;
                }
                if call.attached_content().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} specialized call {:?} carries attached content",
                        instance.key(),
                        expression.owner(),
                    )));
                }
                for (source_index, operand) in call.operands().iter().enumerate() {
                    let source_index = u32::try_from(source_index).map_err(|_| {
                        RuntimePlanLowerError::new(format!(
                            "project-function instance {:?} specialized call {:?} source operand index exceeds checked limits",
                            instance.key(),
                            expression.owner(),
                        ))
                    })?;
                    locals.push((
                        ProjectFunctionFrameLocal::SpecializedOperand {
                            owner: expression.owner(),
                            source_index,
                        },
                        operand.ty().identity(),
                    ));
                }
            }
            Ok((instance.key().clone(), locals))
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
    let (closure_instances, closure_parents) =
        collect_closure_instances(facts).map_err(|error| vec![error])?;
    let closure_local_specs = closure_instances
        .iter()
        .map(|closure| {
            let captured = closure
                .captures()
                .iter()
                .map(|capture| capture.source())
                .collect::<BTreeSet<_>>();
            let mut rows = closure
                .semantics()
                .type_projection()
                .iter()
                .filter_map(|projection| match projection {
                    RuntimeProjectFunctionTypeProjection::Value {
                        owner: RuntimeProjectFunctionTypeOwner::Local(local),
                        ty,
                    } if !captured.contains(local) => Some((
                        ClosureFrameLocal::Hir(*local),
                        ty.identity(),
                    )),
                    RuntimeProjectFunctionTypeProjection::Value { .. }
                    | RuntimeProjectFunctionTypeProjection::SemanticOnlyExpression { .. } => None,
                })
                .collect::<Vec<_>>();
            rows.extend(closure.parameters().iter().map(|parameter| {
                (
                    ClosureFrameLocal::ParameterInput {
                        position: parameter.position(),
                    },
                    parameter.ty().identity(),
                )
            }));
            rows.extend(closure.captures().iter().map(|capture| {
                (
                    ClosureFrameLocal::CaptureInput {
                        position: capture.position(),
                    },
                    capture.ty().identity(),
                )
            }));
            for expression in closure.semantics().expressions() {
                let Some(call) = closure.semantics().call(expression.owner()) else {
                    continue;
                };
                if !call.requires_specialized_operand_anf() {
                    continue;
                }
                if call.attached_content().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "project closure {:?} specialized call {:?} carries attached content",
                        closure.key(),
                        expression.owner(),
                    )));
                }
                for (source_index, operand) in call.operands().iter().enumerate() {
                    let source_index = u32::try_from(source_index).map_err(|_| {
                        RuntimePlanLowerError::new(format!(
                            "project closure {:?} specialized call {:?} source operand index exceeds checked limits",
                            closure.key(),
                            expression.owner(),
                        ))
                    })?;
                    rows.push((
                        ClosureFrameLocal::SpecializedOperand {
                            owner: expression.owner(),
                            source_index,
                        },
                        operand.ty().identity(),
                    ));
                }
            }
            Ok((closure.key().clone(), rows))
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
    let project_default_capture_input_specs = facts
        .project_function_instances()
        .flat_map(|instance| {
            instance
                .attached_default()
                .into_iter()
                .flat_map(move |default| {
                    default
                        .captures()
                        .iter()
                        .enumerate()
                        .map(move |(position, capture)| {
                            let position = u32::try_from(position).map_err(|_| {
                                RuntimePlanLowerError::new(format!(
                                    "project-function instance {:?} default capture position exceeds checked limits",
                                    instance.key()
                                ))
                            })?;
                            Ok((
                                (instance.key().clone(), position),
                                capture.binding_ty().identity(),
                            ))
                        })
                })
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
    let implicit_capture_input_local_specs = implicit_callable_facts
        .iter()
        .flat_map(|(owner, callable)| {
            callable
                .captures()
                .iter()
                .enumerate()
                .map(move |(position, capture)| {
                    let position = u32::try_from(position).map_err(|_| {
                        RuntimePlanLowerError::new(format!(
                            "implicit callable {owner:?} capture position exceeds checked limits"
                        ))
                    })?;
                    let ty = facts.local_type(*capture).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "implicit callable {owner:?} capture {capture:?} has no accepted type"
                        ))
                    })?;
                    Ok(((**owner, position), ty.identity()))
                })
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
    for (_, rows) in &project_instance_local_specs {
        local_seeds.extend(
            rows.iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
        );
    }
    for (_, rows) in &closure_local_specs {
        local_seeds.extend(
            rows.iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
        );
    }
    local_seeds.extend(
        project_default_capture_input_specs
            .iter()
            .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
    );
    local_seeds.extend(
        implicit_capture_input_local_specs
            .iter()
            .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
    );
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
    let mut admitted_locals = admission.local_ids()[local_facts.len()..].iter().cloned();
    let implicit_parameters = implicit_callable_facts
        .iter()
        .map(|(expression, _)| **expression)
        .map(|expression| {
            admitted_locals
                .next()
                .map(|local| (expression, local))
                .ok_or_else(|| {
                    vec![RuntimePlanLowerError::new(
                        "admitted implicit-callable parameter local is missing",
                    )]
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut controller_result_locals = BTreeMap::new();
    for (callable, _) in &controller_result_local_specs {
        let seed = admitted_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted Entry controller result local is missing",
            )]
        })?;
        if controller_result_locals
            .insert(callable.clone(), seed)
            .is_some()
        {
            return Err(vec![RuntimePlanLowerError::new(
                "Entry controller result local is admitted more than once",
            )]);
        }
    }
    let mut specialized_operand_locals = BTreeMap::new();
    for ((expression, source_index), _) in &specialized_operand_local_specs {
        let seed = admitted_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted specialized source-row ANF local is missing",
            )]
        })?;
        if specialized_operand_locals
            .insert((*expression, *source_index), seed)
            .is_some()
        {
            return Err(vec![RuntimePlanLowerError::new(
                "specialized source-row ANF local is admitted more than once",
            )]);
        }
    }
    let mut project_instance_locals = BTreeMap::new();
    for (key, rows) in &project_instance_local_specs {
        let mut frame = ProjectFunctionFrameLocals::default();
        for (owner, _) in rows {
            let seed = admitted_locals.next().ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "admitted project-function instance local is missing",
                )]
            })?;
            match owner {
                ProjectFunctionFrameLocal::Hir(local) => {
                    frame.hir.insert(*local, seed);
                }
                ProjectFunctionFrameLocal::ParameterInput { group, parameter } => {
                    if frame
                        .parameter_inputs
                        .insert((*group, *parameter), seed)
                        .is_some()
                    {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project-function instance repeats a parameter input local",
                        )]);
                    }
                }
                ProjectFunctionFrameLocal::AttachedAbi => {
                    if frame.attached_abi.replace(seed).is_some() {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project-function instance repeats its attached ABI local",
                        )]);
                    }
                }
                ProjectFunctionFrameLocal::SpecializedOperand {
                    owner,
                    source_index,
                } => {
                    if frame
                        .specialized_operands
                        .insert((*owner, *source_index), seed)
                        .is_some()
                    {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project-function instance repeats a specialized source-row local",
                        )]);
                    }
                }
            }
        }
        let instance = facts.project_function_instance(key).ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "project frame has no closed instance",
            )]
        })?;
        frame.control = ControlLocals::admit(
            crate::semantic_facts::RuntimeExecutableSemanticFactView::project_instance(
                instance.semantics(),
            ),
            &mut builder,
        )
        .map_err(|error| vec![error])?;
        project_instance_locals.insert(key.clone(), frame);
    }
    let mut closure_locals: BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals> =
        BTreeMap::new();
    for (key, rows) in &closure_local_specs {
        let mut frame = ClosureFrameLocals::default();
        for (owner, _) in rows {
            let seed = admitted_locals.next().ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "admitted project-closure local is missing",
                )]
            })?;
            match owner {
                ClosureFrameLocal::Hir(local) => {
                    if frame.hir.insert(*local, seed).is_some() {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project closure repeats a local declaration",
                        )]);
                    }
                }
                ClosureFrameLocal::ParameterInput { position } => {
                    if frame.parameter_inputs.insert(*position, seed).is_some() {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project closure repeats a parameter input local",
                        )]);
                    }
                }
                ClosureFrameLocal::CaptureInput { position } => {
                    if frame.capture_inputs.insert(*position, seed).is_some() {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project closure repeats a capture input local",
                        )]);
                    }
                }
                ClosureFrameLocal::SpecializedOperand {
                    owner,
                    source_index,
                } => {
                    if frame
                        .specialized_operands
                        .insert((*owner, *source_index), seed)
                        .is_some()
                    {
                        return Err(vec![RuntimePlanLowerError::new(
                            "project closure repeats a specialized source-row local",
                        )]);
                    }
                }
            }
        }
        let closure = closure_instances
            .iter()
            .find(|closure| closure.key() == key)
            .ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "project closure local spec has no closure fact",
                )]
            })?;
        let parent_hir: Option<&BTreeMap<LocalId, RuntimeLocalSeedId>> =
            match closure_parents.get(key) {
                Some(ClosureLexicalParent::Global) => Some(&locals),
                Some(ClosureLexicalParent::ProjectFunction(parent)) => {
                    project_instance_locals.get(parent).map(|frame| &frame.hir)
                }
                Some(ClosureLexicalParent::Closure(parent)) => {
                    closure_locals.get(parent).map(|frame| &frame.hir)
                }
                None => None,
            };
        let parent_hir = parent_hir.ok_or_else(|| {
            vec![RuntimePlanLowerError::new(format!(
                "project closure {:?} has no immediate lexical parent local frame",
                key
            ))]
        })?;
        for capture in closure.captures() {
            let local = parent_hir.get(&capture.source()).cloned().ok_or_else(|| {
                vec![RuntimePlanLowerError::new(format!(
                    "project closure {:?} capture source {:?} is absent from its parent frame",
                    key,
                    capture.source()
                ))]
            })?;
            if frame.hir.insert(capture.source(), local).is_some() {
                return Err(vec![RuntimePlanLowerError::new(
                    "project closure capture source collides with a local declaration",
                )]);
            }
        }
        frame.control = ControlLocals::admit(
            crate::semantic_facts::RuntimeExecutableSemanticFactView::project_instance(
                closure.semantics(),
            ),
            &mut builder,
        )
        .map_err(|error| vec![error])?;
        if closure_locals.insert(key.clone(), frame).is_some() {
            return Err(vec![RuntimePlanLowerError::new(
                "project closure local frame is admitted more than once",
            )]);
        }
    }
    let mut project_default_capture_input_locals = BTreeMap::new();
    for ((key, position), _) in &project_default_capture_input_specs {
        let seed = admitted_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted project default capture input local is missing",
            )]
        })?;
        if project_default_capture_input_locals
            .insert((key.clone(), *position), seed)
            .is_some()
        {
            return Err(vec![RuntimePlanLowerError::new(
                "project default capture input local is admitted more than once",
            )]);
        }
    }
    let mut implicit_capture_input_locals = BTreeMap::new();
    for ((owner, position), _) in &implicit_capture_input_local_specs {
        let seed = admitted_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted implicit capture input local is missing",
            )]
        })?;
        if implicit_capture_input_locals
            .insert((*owner, *position), seed)
            .is_some()
        {
            return Err(vec![RuntimePlanLowerError::new(
                "implicit capture input local is admitted more than once",
            )]);
        }
    }
    debug_assert!(admitted_locals.next().is_none());
    let mut errors = Vec::new();
    let (function_sites, function_definitions) = reserve_function_sites(
        project,
        facts,
        &locals,
        &implicit_parameters,
        &implicit_capture_input_locals,
        &mut builder,
        &mut errors,
    );
    let (project_function_sites, project_function_definitions) = reserve_project_function_sites(
        project,
        facts,
        &project_instance_locals,
        &mut builder,
        &mut errors,
    );
    let (closure_sites, closure_definitions) = reserve_closure_sites(
        project,
        facts,
        &closure_instances,
        &closure_locals,
        &mut builder,
        &mut errors,
    );
    let (project_default_function_sites, project_default_function_definitions) =
        reserve_project_default_function_sites(
            project,
            facts,
            &project_instance_locals,
            &project_default_capture_input_locals,
            &mut builder,
            &mut errors,
        );
    let pure_program_definitions = reserve_pure_programs(facts, &locals, &mut builder, &mut errors);
    let (trait_methods, trait_definitions) =
        reserve_trait_methods(project, facts, &locals, &mut builder, &mut errors);
    let empty_dialogue_effect_sites = BTreeMap::new();
    let empty_dialogue_value_capture_input_locals = BTreeMap::new();
    let empty_dialogue_effect_capture_input_locals = BTreeMap::new();
    let empty_dialogue_content = BTreeMap::new();
    let control_locals = ControlLocals::admit(
        crate::semantic_facts::RuntimeExecutableSemanticFactView::global(facts),
        &mut builder,
    )
    .map_err(|error| vec![error])?;
    let context = FinalLoweringContext {
        project,
        facts,
        locals: &locals,
        project_function_sites: &project_function_sites,
        project_default_function_sites: &project_default_function_sites,
        project_function_locals: &project_instance_locals,
        closure_sites: &closure_sites,
        closure_locals: &closure_locals,
        trait_methods: &trait_methods,
        function_sites: &function_sites,
        dialogue_effect_sites: &empty_dialogue_effect_sites,
        dialogue_value_capture_input_locals: &empty_dialogue_value_capture_input_locals,
        dialogue_effect_capture_input_locals: &empty_dialogue_effect_capture_input_locals,
        dialogue_content: &empty_dialogue_content,
        control: &control_locals,
        controller_result_locals: &controller_result_locals,
        specialized_operand_locals: &specialized_operand_locals,
    };

    // Effect callback inputs are known entirely from the checked effect
    // rows. Admit their synthetic destination locals first so the callback
    // sites can be reserved without borrowing the caller's locals as input
    // destinations.
    let dialogue_effect_capture_specs = match collect_dialogue_effect_capture_specs(&context) {
        Ok(specs) => specs,
        Err(mut capture_errors) => {
            errors.append(&mut capture_errors);
            Vec::new()
        }
    };
    let effect_admission = builder
        .admit_semantic_batch(
            [],
            dialogue_effect_capture_specs
                .iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
            [],
            [],
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut effect_local_ids = effect_admission.local_ids().iter().cloned();
    let dialogue_effect_capture_input_locals = dialogue_effect_capture_specs
        .iter()
        .map(|(key, _)| {
            effect_local_ids
                .next()
                .map(|local| (*key, local))
                .ok_or_else(|| {
                    vec![RuntimePlanLowerError::new(
                        "admitted dialogue effect capture local is missing",
                    )]
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if effect_local_ids.next().is_some() {
        return Err(vec![RuntimePlanLowerError::new(
            "admitted dialogue effect capture locals contain an unexpected row",
        )]);
    }
    let context = FinalLoweringContext {
        dialogue_effect_capture_input_locals: &dialogue_effect_capture_input_locals,
        ..context
    };
    let (dialogue_effect_sites, effect_definitions) =
        reserve_dialogue_effect_sites(&context, &mut builder, &mut errors);
    let context = FinalLoweringContext {
        dialogue_effect_sites: &dialogue_effect_sites,
        ..context
    };
    define_dialogue_effect_sites(&context, effect_definitions, &mut builder, &mut errors);
    // Value callback bodies can contain nested Content applications. Their
    // exact body lowering therefore runs only after the effect-site table is
    // available; otherwise a valid nested body would be mistaken for a
    // missing callback and silently admitted with the wrong capture ABI.
    let dialogue_value_capture_specs = match collect_dialogue_value_capture_specs(&context) {
        Ok(specs) => specs,
        Err(mut capture_errors) => {
            errors.append(&mut capture_errors);
            Vec::new()
        }
    };
    let value_admission = builder
        .admit_semantic_batch(
            [],
            dialogue_value_capture_specs
                .iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
            [],
            [],
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut value_local_ids = value_admission.local_ids().iter().cloned();
    let dialogue_value_capture_input_locals = dialogue_value_capture_specs
        .iter()
        .map(|(key, _)| {
            value_local_ids
                .next()
                .map(|local| (*key, local))
                .ok_or_else(|| {
                    vec![RuntimePlanLowerError::new(
                        "admitted dialogue value capture local is missing",
                    )]
                })
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if value_local_ids.next().is_some() {
        return Err(vec![RuntimePlanLowerError::new(
            "admitted dialogue value capture locals contain an unexpected row",
        )]);
    }
    let context = FinalLoweringContext {
        dialogue_value_capture_input_locals: &dialogue_value_capture_input_locals,
        ..context
    };
    let (dialogue_content, dialogue_assertion_sites) =
        lower_dialogue_content(&context, &dialogue_effect_sites, &mut builder, &mut errors);
    let context = FinalLoweringContext {
        dialogue_content: &dialogue_content,
        ..context
    };

    // Bodies may invoke any reserved function site or start any admitted
    // dialogue occurrence. Define them only after both inventories exist.
    define_function_sites(&context, &function_definitions, &mut builder, &mut errors);
    define_project_function_sites(
        &context,
        &project_function_definitions,
        &mut builder,
        &mut errors,
    );
    define_closure_sites(
        &context,
        &closure_definitions,
        &closure_locals,
        &mut builder,
        &mut errors,
    );
    define_project_default_function_sites(
        &context,
        &project_default_function_definitions,
        &mut builder,
        &mut errors,
    );
    define_pure_programs(
        &context,
        &pure_program_definitions,
        &mut builder,
        &mut errors,
    );
    define_trait_methods(&context, &trait_definitions, &mut builder, &mut errors);

    let mut entry_owners = collect_entry_inputs(entry_input, &mut errors);
    let mut flow_seeds = Vec::new();
    let mut flow_schemas = Vec::new();
    let mut assertion_sites = dialogue_assertion_sites;
    for item in project.items() {
        if matches!(
            item.item().kind(),
            HirItemKind::Flow(_) | HirItemKind::Entry(_)
        ) && !facts.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(item.id()))
        {
            continue;
        }
        match item.item().kind() {
            HirItemKind::Flow(flow) => {
                let Some(identity) = facts.flow(item.id()).cloned() else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "checked runtime Flow identity is missing for final-HIR item {:?}",
                        item.id()
                    )));
                    continue;
                };
                match flow_invocation_schema(item.module(), flow, &identity, facts) {
                    Ok(schema) => flow_schemas.push(schema),
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
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
    flow_schemas.extend(controller_flows.iter().map(|flow| RuntimeFlowSchema {
        flow: flow.id().clone(),
        parameters: Vec::new(),
    }));
    flow_seeds.extend(controller_flows);
    assertion_sites.extend(controller_assertions);
    let mut flow_executables =
        lower_entry_flows(project, facts, entry_input, &flow_schemas, &mut errors)?;
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
    for schema in flow_schemas {
        builder
            .push_flow_schema(schema)
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
    let mut dialogue_records = Vec::new();
    facts.visit_dialogue_applications(&mut |_, _, application| {
        dialogue_records.push(application.content().clone());
    });
    dialogue_records
        .sort_by(|left, right| (left.key(), left.text_key()).cmp(&(right.key(), right.text_key())));
    let mut dialogue_templates = Vec::new();
    facts.visit_dialogue_content_fragments(&mut |_, fragment| {
        dialogue_templates.push(fragment.template().clone());
    });
    dialogue_templates.sort_by_key(|template| template.id());
    let dialogue_content_catalog = DialogueContentCatalog::try_from_records_and_templates(
        dialogue_records,
        dialogue_templates,
    )
    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let pure_helper_count = plan.pure_helpers().len();
    Ok(RuntimePlanLowerReport {
        plan,
        stats: RuntimePlanLowerStats {
            pure_helpers: pure_helper_count,
            pure_candidate_functions_seen: 0,
            pure_candidate_lower_attempts: 0,
            ..RuntimePlanLowerStats::default()
        },
        dialogue_content_catalog,
        character_presentation_catalog: facts.character_presentation_catalog().cloned(),
        assertion_sites: assertion_sites.into_boxed_slice(),
    })
}

fn flow_invocation_schema(
    module: &HirModule,
    flow: &HirFlowItem,
    identity: &FlowRuntimeId,
    facts: &RuntimePlanSemanticFacts,
) -> Result<RuntimeFlowSchema, RuntimePlanLowerError> {
    if !flow.generic_parameters().is_empty() || !flow.where_predicates().is_empty() {
        return Err(RuntimePlanLowerError::new(format!(
            "runtime Flow {identity} cannot publish an invocation schema with open generics"
        )));
    }
    let parameters =
        flow.parameters()
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                let [local] = parameter.locals() else {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime Flow {identity} parameter {index} is not one direct binding"
                    )));
                };
                let pattern = module.resolve_pattern(parameter.pattern()).map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "runtime Flow {identity} parameter {index} pattern is unavailable: {error}"
                ))
            })?;
                let HirPatternKind::Binding(HirPatternBinding::Bound {
                    name,
                    local: pattern_local,
                }) = pattern.kind()
                else {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime Flow {identity} parameter {index} is not a fixed immutable binding"
                    )));
                };
                if pattern_local != local {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime Flow {identity} parameter {index} binding identity is inconsistent"
                    )));
                }
                let ty = facts.local_type(*local).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "runtime Flow {identity} parameter {index} has no checked semantic type"
                    ))
                })?;
                Ok(RuntimeFlowExecutableParameter {
                    coordinate: FlowParameterCoordinate::try_from_index(index)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                    name: name.as_str().to_owned(),
                    mode: RuntimeFlowParameterMode::Owned,
                    semantic_identity: ty.identity(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeFlowSchema {
        flow: identity.clone(),
        parameters,
    })
}

fn collect_closure_instances<'facts>(
    facts: &'facts RuntimePlanSemanticFacts,
) -> Result<
    (
        Vec<&'facts RuntimeClosureInstanceFact>,
        BTreeMap<RuntimeClosureInstanceKey, ClosureLexicalParent>,
    ),
    RuntimePlanLowerError,
> {
    let mut instances =
        BTreeMap::<RuntimeClosureInstanceKey, &'facts RuntimeClosureInstanceFact>::new();
    let mut parents = BTreeMap::new();
    let mut order = Vec::new();
    for closure in facts.root_closures() {
        let key = closure.key().clone();
        if instances.insert(key.clone(), closure).is_some() {
            return Err(RuntimePlanLowerError::new(
                "root closure has duplicate instance keys",
            ));
        }
        parents.insert(key.clone(), ClosureLexicalParent::Global);
        order.push(closure);
        collect_closure_instances_from_semantics(
            closure.semantics(),
            &mut instances,
            &mut order,
            ClosureLexicalParent::Closure(key),
            &mut parents,
        )?;
    }
    for instance in facts.project_function_instances() {
        collect_closure_instances_from_semantics(
            instance.semantics(),
            &mut instances,
            &mut order,
            ClosureLexicalParent::ProjectFunction(instance.key().clone()),
            &mut parents,
        )?;
    }
    Ok((order, parents))
}

fn collect_closure_instances_from_semantics<'facts>(
    semantics: &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    instances: &mut BTreeMap<RuntimeClosureInstanceKey, &'facts RuntimeClosureInstanceFact>,
    order: &mut Vec<&'facts RuntimeClosureInstanceFact>,
    parent: ClosureLexicalParent,
    parents: &mut BTreeMap<RuntimeClosureInstanceKey, ClosureLexicalParent>,
) -> Result<(), RuntimePlanLowerError> {
    for expression in semantics.expressions() {
        let RuntimeProjectFunctionExpressionPayload::Closure(closure) = expression.payload() else {
            continue;
        };
        let key = closure.key().clone();
        if let Some(previous) = instances.get(&key) {
            if *previous != closure.as_ref() {
                return Err(RuntimePlanLowerError::new(format!(
                    "project closure {:?} has conflicting closed facts",
                    key
                )));
            }
            if parents.get(&key) != Some(&parent) {
                return Err(RuntimePlanLowerError::new(format!(
                    "project closure {:?} has conflicting lexical parents",
                    key
                )));
            }
            continue;
        }
        instances.insert(key.clone(), closure.as_ref());
        parents.insert(key.clone(), parent.clone());
        order.push(closure.as_ref());
        collect_closure_instances_from_semantics(
            closure.semantics(),
            instances,
            order,
            ClosureLexicalParent::Closure(key),
            parents,
        )?;
    }
    Ok(())
}

fn reserve_function_sites(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    implicit_parameters: &BTreeMap<ExprId, RuntimeLocalSeedId>,
    implicit_capture_input_locals: &BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    Vec<ReservedFunctionSiteDefinition>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    reserve_implicit_function_sites(
        project,
        facts,
        locals,
        implicit_parameters,
        implicit_capture_input_locals,
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
    implicit_capture_input_locals: &BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
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
            .enumerate()
            .map(|(position, capture)| -> Result<_, String> {
                let binding = locals.get(capture).cloned().ok_or_else(|| {
                    format!("implicit callable {owner:?} capture {capture:?} is absent")
                })?;
                let position = u32::try_from(position).map_err(|_| {
                    format!("implicit callable {owner:?} capture position exceeds checked limits")
                })?;
                let input_local = implicit_capture_input_locals
                    .get(&(*owner, position))
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "implicit callable {owner:?} capture has no admitted synthetic input local"
                        )
                    })?;
                let ty = facts.local_type(*capture).ok_or_else(|| {
                    format!("implicit callable {owner:?} capture {capture:?} has no accepted type")
                })?;
                Ok(RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position },
                    input_local: input_local.clone(),
                    pattern: RuntimePatternSeed::new(
                        ty.identity(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: binding,
                        },
                    ),
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let parameter_input = RuntimeFunctionInputBindingSeed {
            source: RuntimeFunctionInputSource::Parameter { position: 0 },
            input_local: parameter.clone(),
            pattern: RuntimePatternSeed::new(
                callable.parameter().identity(),
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local: parameter.clone(),
                },
            ),
        };
        let declaration = captures.map(|captures| RuntimeFunctionSiteDeclarationSeed {
            inputs: captures
                .into_iter()
                .chain([parameter_input])
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            result: callable.result().identity(),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeFunctionEffectSet::empty(),
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

fn reserve_closure_sites<'facts>(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    closures: &[&'facts RuntimeClosureInstanceFact],
    closure_locals: &BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeClosureInstanceKey, RuntimeFunctionSiteSeedId>,
    Vec<ReservedClosureDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    for closure in closures {
        let key = closure.key().clone();
        let Some(locals) = closure_locals.get(&key) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} has no admitted closed local frame",
                key
            )));
            continue;
        };
        let Some(module) = module_by_id(project, closure.scope().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} body module is absent",
                key
            )));
            continue;
        };
        let pattern_lowerer = FinalPatternLowerer::new(module, facts, &locals.hir)
            .with_project_semantics(closure.semantics());
        let captures = closure
            .captures()
            .iter()
            .map(|capture| {
                let input_local = locals
                    .capture_inputs
                    .get(&capture.position())
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "project closure {:?} capture {} has no synthetic input local",
                            key,
                            capture.position()
                        ))
                    })?;
                let local = locals.hir.get(&capture.source()).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "project closure {:?} capture source {:?} has no parent local",
                        key,
                        capture.source()
                    ))
                })?;
                Ok(RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture {
                        position: capture.position(),
                    },
                    input_local,
                    pattern: RuntimePatternSeed::new(
                        capture.ty().identity(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local,
                        },
                    ),
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>();
        let parameters = closure
            .parameters()
            .iter()
            .map(|parameter| {
                let input_local = locals
                    .parameter_inputs
                    .get(&parameter.position())
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "project closure {:?} parameter {} has no synthetic input local",
                            key,
                            parameter.position()
                        ))
                    })?;
                let pattern = pattern_lowerer
                    .lower(parameter.pattern())
                    .map_err(RuntimePlanLowerError::new)?;
                if pattern.ty() != parameter.ty().identity() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "project closure {:?} parameter {} pattern type disagrees with closed ABI",
                        key,
                        parameter.position()
                    )));
                }
                Ok(RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Parameter {
                        position: parameter.position(),
                    },
                    input_local,
                    pattern,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>();
        let result = match closure.function_type().shape() {
            RuntimeTypeShape::Function { result, .. } => result.identity(),
            _ => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "project closure {:?} has no function result shape",
                    key
                )));
                continue;
            }
        };
        let effects =
            match RuntimeFunctionEffectSet::try_from_effects(closure.effects().iter().cloned()) {
                Ok(effects) => effects,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project closure {:?} effect row is invalid: {error}",
                        key
                    )));
                    continue;
                }
            };
        let declaration = captures.and_then(|captures| {
            parameters.map(|parameters| RuntimeFunctionSiteDeclarationSeed {
                inputs: captures
                    .into_iter()
                    .chain(parameters)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                result,
                body_kind: match closure.execution() {
                    crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                        RuntimeFunctionSiteBodyKind::Expression
                    }
                    crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                        RuntimeFunctionSiteBodyKind::Executable
                    }
                },
                effects,
            })
        });
        let declaration = match declaration {
            Ok(declaration) => declaration,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                if sites.insert(key.clone(), site.clone()).is_some() {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project closure {:?} function site was reserved more than once",
                        key
                    )));
                    continue;
                }
                definitions.push(ReservedClosureDefinition { closure, site });
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} site reservation failed: {error}",
                key
            ))),
        }
    }
    (sites, definitions)
}

fn reserve_project_function_sites<'facts>(
    project: HirExecutableProjectView<'_>,
    facts: &'facts RuntimePlanSemanticFacts,
    instance_locals: &BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    Vec<ReservedProjectFunctionDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    for instance in facts.project_function_instances() {
        let key = instance.key().clone();
        let Some(local_map) = instance_locals.get(&key) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} has no admitted substituted local frame",
                key
            )));
            continue;
        };
        let Some(module) = module_by_id(project, instance.callable().owner().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} owner module is absent",
                key
            )));
            continue;
        };
        let pattern_lowerer = FinalPatternLowerer::new(module, facts, &local_map.hir)
            .with_project_semantics(instance.semantics());
        let mut inputs = Vec::new();
        let mut invalid = false;
        for parameter in instance.parameters() {
            let group = match u32::try_from(parameter.group().get()) {
                Ok(group) => group,
                Err(_) => {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} parameter group exceeds checked limits",
                        key
                    )));
                    invalid = true;
                    continue;
                }
            };
            let Some(input_local) = local_map
                .parameter_inputs
                .get(&(group, parameter.parameter()))
                .cloned()
            else {
                errors.push(RuntimePlanLowerError::new(format!(
                    "project-function instance {:?} parameter ({:?}, {}) has no synthetic input local",
                    key,
                    parameter.group(),
                    parameter.parameter()
                )));
                invalid = true;
                continue;
            };
            let pattern = match pattern_lowerer.lower(parameter.pattern()) {
                Ok(pattern) => pattern,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} parameter ({:?}, {}) pattern lowering failed: {error}",
                        key,
                        parameter.group(),
                        parameter.parameter()
                    )));
                    invalid = true;
                    continue;
                }
            };
            let source = match parameter.source() {
                RuntimeProjectFunctionParameterSource::ContinuationPrefix { position } => {
                    RuntimeFunctionInputSource::Capture { position }
                }
                RuntimeProjectFunctionParameterSource::CurrentGroup { position } => {
                    RuntimeFunctionInputSource::Parameter { position }
                }
            };
            inputs.push(RuntimeFunctionInputBindingSeed {
                source,
                input_local,
                pattern,
            });
        }
        if let Some(attached) = instance.callable().attached_content_abi()
            && attached.group() == key.group()
        {
            let Some(input_local) = local_map.attached_abi.clone() else {
                errors.push(RuntimePlanLowerError::new(format!(
                    "project-function instance {:?} attached ABI has no synthetic input local",
                    key,
                )));
                continue;
            };
            let Some(binding) = local_map.hir.get(&attached.binding()).cloned() else {
                errors.push(RuntimePlanLowerError::new(format!(
                    "project-function instance {:?} attached binding {:?} is absent from its substituted frame",
                    key,
                    attached.binding()
                )));
                continue;
            };
            inputs.push(RuntimeFunctionInputBindingSeed {
                source: RuntimeFunctionInputSource::Parameter {
                    position: attached.abi_position(),
                },
                input_local,
                pattern: RuntimePatternSeed::new(
                    attached.binding_ty().identity(),
                    RuntimePatternSeedKind::Bind {
                        mutable: false,
                        local: binding,
                    },
                ),
            });
        }
        let RuntimeTypeShape::Function { result, .. } = instance.function_type().shape() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} has no function result shape",
                key
            )));
            continue;
        };
        let effects =
            match RuntimeFunctionEffectSet::try_from_effects(instance.effects().iter().cloned()) {
                Ok(effects) => effects,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} effect row is invalid: {error}",
                        key
                    )));
                    continue;
                }
            };
        let body_kind = match instance.execution() {
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                RuntimeFunctionSiteBodyKind::Expression
            }
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                RuntimeFunctionSiteBodyKind::Executable
            }
        };
        if invalid {
            continue;
        }
        let declaration = RuntimeFunctionSiteDeclarationSeed {
            inputs: inputs.into_boxed_slice(),
            result: result.identity(),
            body_kind,
            effects,
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                if sites.insert(key.clone(), site.clone()).is_some() {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} was reserved more than once",
                        key
                    )));
                    continue;
                }
                definitions.push(ReservedProjectFunctionDefinition { instance, site });
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} site reservation failed: {error}",
                key
            ))),
        }
    }
    (sites, definitions)
}

fn reserve_project_default_function_sites<'facts>(
    project: HirExecutableProjectView<'_>,
    facts: &'facts RuntimePlanSemanticFacts,
    instance_locals: &BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    capture_input_locals: &BTreeMap<ProjectDefaultCaptureInputKey, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    Vec<ReservedProjectDefaultFunctionDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    for instance in facts.project_function_instances() {
        let Some(default) = instance.attached_default() else {
            continue;
        };
        let key = instance.key().clone();
        let Some(local_map) = instance_locals.get(&key) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default has no admitted substituted local frame",
                key
            )));
            continue;
        };
        let Some(module) = module_by_id(project, default.source().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default source module is absent",
                key
            )));
            continue;
        };
        let pattern_lowerer = FinalPatternLowerer::new(module, facts, &local_map.hir)
            .with_project_semantics(instance.semantics());
        let inputs = default
            .captures()
            .iter()
            .enumerate()
            .map(|(position, capture)| {
                let position = u32::try_from(position).map_err(|_| {
                    RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} default capture position exceeds checked limits",
                        key
                    ))
                })?;
                let input_local = capture_input_locals
                    .get(&(key.clone(), position))
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "project-function instance {:?} default capture {position} has no synthetic input local",
                            key
                        ))
                    })?;
                let pattern = pattern_lowerer
                    .lower(capture.pattern())
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "project-function instance {:?} default capture {position} pattern lowering failed: {error}",
                            key
                        ))
                    })?;
                if pattern.ty() != capture.binding_ty().identity() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} default capture {position} pattern type disagrees with checked binding type",
                        key
                    )));
                }
                Ok(RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position },
                    input_local,
                    pattern,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>();
        let effects = RuntimeFunctionEffectSet::try_from_effects(default.effects().iter().cloned())
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "project-function instance {:?} default effect row is invalid: {error}",
                    key
                ))
            });
        let declaration = inputs.and_then(|inputs| {
            effects.map(|effects| RuntimeFunctionSiteDeclarationSeed {
                inputs: inputs.into_boxed_slice(),
                result: default.result().identity(),
                body_kind: match default.execution() {
                    crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                        RuntimeFunctionSiteBodyKind::Expression
                    }
                    crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                        RuntimeFunctionSiteBodyKind::Executable
                    }
                },
                effects,
            })
        });
        let declaration = match declaration {
            Ok(declaration) => declaration,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                if sites.insert(key.clone(), site.clone()).is_some() {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} default site was reserved more than once",
                        key
                    )));
                    continue;
                }
                definitions.push(ReservedProjectDefaultFunctionDefinition { instance, site });
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default site reservation failed: {error}",
                key
            ))),
        }
    }
    (sites, definitions)
}

fn reserve_pure_programs(
    facts: &RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Vec<ReservedPureProgramDefinition> {
    let mut definitions = Vec::new();
    for (_, program) in facts.pure_programs() {
        let inputs = program
            .captures()
            .iter()
            .map(|capture| {
                locals.get(&capture.local()).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "pure program {} capture {:?} has no admitted local",
                        program.program(),
                        capture.local()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let declaration = inputs.map(|inputs| RuntimePureHelperDeclarationSeed {
            name: format!("pure.program.{}", program.program()),
            input_abi: vec![RuntimePureInputType::Value; inputs.len()],
            inputs: inputs.into_boxed_slice(),
            result: program.result(),
            output_abi: RuntimePureOutputType::Value,
            scalar_eval_supported: false,
            origin: RuntimePureHelperOrigin::Inferred,
        });
        let helper = match declaration.and_then(|declaration| {
            builder
                .reserve_pure_helper_seed(declaration)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(helper) => helper,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let binding = RuntimePureProgramBindingSeed {
            program: program.program(),
            helper: helper.clone(),
        };
        if let Err(error) = builder.push_pure_program_binding_seed(&binding) {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
            continue;
        }
        definitions.push(ReservedPureProgramDefinition {
            closure: program.closure(),
            module: program.closure().module(),
            body: program.body(),
            helper,
        });
    }
    definitions
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

fn define_closure_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedClosureDefinition<'_>],
    closure_locals: &BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let closure = definition.closure;
        let Some(locals) = closure_locals.get(closure.key()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} has no admitted closed local frame",
                closure.key()
            )));
            continue;
        };
        let Some(module) = module_by_id(context.project, closure.scope().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} body module is absent",
                closure.key()
            )));
            continue;
        };
        let body = match closure.execution() {
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                context
                    .expr_lowerer(module)
                    .with_locals(&locals.hir)
                    .with_control_locals(&locals.control.pipes, &locals.control.tries)
                    .with_specialized_operand_locals(&locals.specialized_operands)
                    .with_scoped_semantics(RuntimeScopedExecutableSemanticFactView::closure(
                        closure.key(),
                        closure.semantics(),
                    ))
                    .lower_function_site_body(closure.owner(), closure.body(), BTreeMap::new())
                    .map(RuntimeFunctionSiteBodySeed::Expression)
                    .map_err(RuntimePlanLowerError::new)
            }
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                let mut flow = FinalFlowLowerer::new(
                    module,
                    context,
                    RuntimeAssertionOwner::Closure(closure.key().closure().clone()),
                )
                .with_closure(locals, closure.key(), closure.semantics());
                flow.lower_flow_value(closure.body(), RuntimeFlowValueContinuation::Return)
                    .map(|ops| {
                        let effects = RuntimeFunctionEffectSet::try_from_effects(
                            closure.effects().iter().cloned(),
                        )
                        .expect("project closure effect row was validated during reservation");
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeFunctionExecutableBodySeed {
                            effects,
                            ops: ops.into_boxed_slice(),
                        })
                    })
            }
        };
        match body.and_then(|body| {
            builder
                .define_function_site_seed(&definition.site, body)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project closure {:?} site definition failed: {error}",
                closure.key()
            ))),
        }
    }
}

fn define_project_function_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedProjectFunctionDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let instance = definition.instance;
        let Some(locals) = context.project_function_locals.get(instance.key()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} has no admitted substituted local frame",
                instance.key()
            )));
            continue;
        };
        let Some(module) = module_by_id(context.project, instance.body().scope().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} body module is absent",
                instance.key()
            )));
            continue;
        };
        let body = match instance.execution() {
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                let hir_body = HirFunctionBody::Block {
                    scope: instance.body().scope(),
                    statements: instance.body().statements().to_vec().into_boxed_slice(),
                    tail: instance.body().tail(),
                };
                context
                    .expr_lowerer(module)
                    .with_locals(&locals.hir)
                    .with_control_locals(&locals.control.pipes, &locals.control.tries)
                    .with_specialized_operand_locals(&locals.specialized_operands)
                    .with_scoped_semantics(
                        RuntimeScopedExecutableSemanticFactView::project_function(
                            instance.key(),
                            instance.semantics(),
                        ),
                    )
                    .lower_function_body(&hir_body)
                    .map(RuntimeFunctionSiteBodySeed::Expression)
                    .map_err(RuntimePlanLowerError::new)
            }
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                let declaration = match instance.callable().declaration() {
                    CallableDeclarationKey::Existing(declaration) => declaration.clone(),
                    _ => {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "project-function instance {:?} has no ordinary declaration",
                            instance.key()
                        )));
                        continue;
                    }
                };
                let mut flow = FinalFlowLowerer::new(
                    module,
                    context,
                    RuntimeAssertionOwner::Callable(declaration.clone()),
                )
                .with_project_instance(
                    locals,
                    instance.key(),
                    instance.semantics(),
                );
                let ops = match flow.lower_statement_ids_with_tail(
                    instance.body().statements(),
                    RuntimeFlowTail::Value {
                        expression: instance.body().tail(),
                        continuation: Box::new(RuntimeFlowValueContinuation::Return),
                    },
                ) {
                    Ok(ops) => ops,
                    Err(lower_errors) => {
                        errors.push(lower_errors);
                        continue;
                    }
                };
                let effects = match RuntimeFunctionEffectSet::try_from_effects(
                    instance.effects().iter().cloned(),
                ) {
                    Ok(effects) => effects,
                    Err(error) => {
                        errors.push(RuntimePlanLowerError::new(error.to_string()));
                        continue;
                    }
                };
                Ok(RuntimeFunctionSiteBodySeed::Executable(
                    RuntimeFunctionExecutableBodySeed {
                        effects,
                        ops: ops.into_boxed_slice(),
                    },
                ))
            }
        };
        match body.and_then(|body| {
            builder
                .define_function_site_seed(&definition.site, body)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} site definition failed: {error}",
                instance.key()
            ))),
        }
    }
}

fn define_project_default_function_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedProjectDefaultFunctionDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let instance = definition.instance;
        let Some(locals) = context.project_function_locals.get(instance.key()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default has no admitted substituted local frame",
                instance.key()
            )));
            continue;
        };
        let Some(default) = instance.attached_default() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default site has no checked default fact",
                instance.key()
            )));
            continue;
        };
        let Some(module) = module_by_id(context.project, default.source().module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default source module is absent",
                instance.key()
            )));
            continue;
        };
        let body = match default.execution() {
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExpressionFunctionSite => {
                context
                    .expr_lowerer(module)
                    .with_locals(&locals.hir)
                    .with_control_locals(&locals.control.pipes, &locals.control.tries)
                    .with_specialized_operand_locals(&locals.specialized_operands)
                    .with_scoped_semantics(
                        RuntimeScopedExecutableSemanticFactView::project_function(
                            instance.key(),
                            instance.semantics(),
                        ),
                    )
                    .lower_function_site_body(default.source(), default.source(), BTreeMap::new())
                    .map(RuntimeFunctionSiteBodySeed::Expression)
                    .map_err(RuntimePlanLowerError::new)
            }
            crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite => {
                let declaration = match instance.callable().declaration() {
                    CallableDeclarationKey::Existing(declaration) => declaration.clone(),
                    _ => {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "project-function instance {:?} default has no ordinary declaration",
                            instance.key()
                        )));
                        continue;
                    }
                };
                let mut flow = FinalFlowLowerer::new(
                    module,
                    context,
                    RuntimeAssertionOwner::Callable(declaration),
                )
                .with_project_instance(
                    locals,
                    instance.key(),
                    instance.semantics(),
                );
                flow.lower_flow_value(default.source(), RuntimeFlowValueContinuation::Return)
                    .map(|ops| {
                        let effects = RuntimeFunctionEffectSet::try_from_effects(
                            default.effects().iter().cloned(),
                        )
                        .expect("default site effect row was validated during reservation");
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeFunctionExecutableBodySeed {
                            effects,
                            ops: ops.into_boxed_slice(),
                        })
                    })
                    .map_err(|error| error)
            }
        };
        match body.and_then(|body| {
            builder
                .define_function_site_seed(&definition.site, body)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(()) => {}
            Err(error) => errors.push(RuntimePlanLowerError::new(format!(
                "project-function instance {:?} default site definition failed: {error}",
                instance.key()
            ))),
        }
    }
}

fn define_pure_programs(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedPureProgramDefinition],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Some(module) = module_by_id(context.project, definition.module) else {
            errors.push(RuntimePlanLowerError::new("pure program module is absent"));
            continue;
        };
        let body = context.expr_lowerer(module).lower_function_site_body(
            definition.closure,
            definition.body,
            BTreeMap::new(),
        );
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

fn collect_dialogue_value_capture_specs(
    context: &FinalLoweringContext<'_, '_>,
) -> Result<Vec<(RuntimeDialogueValueCaptureKey, RuntimeSemanticTypeId)>, Vec<RuntimePlanLowerError>>
{
    let mut errors = Vec::new();
    let mut value_specs = BTreeMap::new();
    context
        .facts
        .visit_dialogue_applications(&mut |scope, owner, application| {
            let Some(module) = module_by_id(context.project, owner.module()) else {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue application {owner:?} module is absent during capture admission"
                )));
                return;
            };
            let Some(fragment) = scope.dialogue_content_fragment_for_source(owner) else {
                errors.push(RuntimePlanLowerError::new(format!(
                "dialogue application {owner:?} content template is absent during capture admission"
            )));
                return;
            };
            let locals = match context.dialogue_locals(scope.scope()) {
                Ok(locals) => locals,
                Err(error) => {
                    errors.push(error);
                    return;
                }
            };
            let lowerer = match context.scoped_expr_lowerer(module, scope) {
                Ok(lowerer) => lowerer,
                Err(error) => {
                    errors.push(error);
                    return;
                }
            };
            for value in fragment.values() {
                let Ok(body) = lowerer.lower(value.expression()) else {
                    // The final lowering pass reports the authoritative body
                    // error.  Capture admission must not invent a replacement
                    // body or downgrade that failure to an empty capture list.
                    continue;
                };
                for (position, capture) in body.free_locals().iter().enumerate() {
                    let position = match u32::try_from(position) {
                        Ok(position) => position,
                        Err(_) => {
                            errors.push(RuntimePlanLowerError::new(format!(
                                "dialogue value {:?} capture position exceeds checked limits",
                                value.expression()
                            )));
                            continue;
                        }
                    };
                    let Some((local, ty)) = locals.iter().find_map(|(local, candidate)| {
                        (candidate == capture).then(|| (*local, scope.local_type(*local)))
                    }) else {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} capture has no accepted local authority",
                            value.expression()
                        )));
                        continue;
                    };
                    let Some(ty) = ty else {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} capture {local:?} has no accepted type",
                            value.expression()
                        )));
                        continue;
                    };
                    let key = RuntimeDialogueValueCaptureKey::new(
                        application.content().template_id(),
                        value.slot(),
                        position,
                    );
                    if value_specs.insert(key, ty.identity()).is_some() {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} repeats a capture input position",
                            value.expression()
                        )));
                    }
                }
            }
        });

    if errors.is_empty() {
        Ok(value_specs.into_iter().collect())
    } else {
        Err(errors)
    }
}

fn collect_dialogue_effect_capture_specs(
    context: &FinalLoweringContext<'_, '_>,
) -> Result<Vec<(RuntimeDialogueEffectCaptureKey, RuntimeSemanticTypeId)>, Vec<RuntimePlanLowerError>>
{
    let mut errors = Vec::new();
    let mut effect_specs = BTreeMap::new();
    context
        .facts
        .visit_dialogue_content_fragments(&mut |scope, fragment| {
            let locals = match context.dialogue_locals(scope.scope()) {
                Ok(locals) => locals,
                Err(error) => {
                    errors.push(error);
                    return;
                }
            };
            for effect in fragment.effects() {
                let program =
                    RuntimeDialogueEffectProgramKey::new(fragment.template().id(), effect.site());
                for (position, capture) in effect.captures().iter().enumerate() {
                    let position = match u32::try_from(position) {
                        Ok(position) => position,
                        Err(_) => {
                            errors.push(RuntimePlanLowerError::new(format!(
                                "dialogue effect {:?} capture position exceeds checked limits",
                                effect.site()
                            )));
                            continue;
                        }
                    };
                    if !locals.contains_key(&capture.local())
                        || scope.local_type(capture.local()).is_none()
                    {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue effect {:?} capture {:?} has no accepted local authority",
                            effect.site(),
                            capture.local()
                        )));
                        continue;
                    }
                    let key = RuntimeDialogueEffectCaptureKey::new(program, position);
                    if effect_specs.insert(key, capture.ty().identity()).is_some() {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue effect {:?} repeats a capture input position",
                            effect.site()
                        )));
                    }
                }
            }
        });

    if errors.is_empty() {
        Ok(effect_specs.into_iter().collect())
    } else {
        Err(errors)
    }
}

fn lower_dialogue_content<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    dialogue_effect_sites: &BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
        RuntimeDialogueContentPlanSeedId,
    >,
    Vec<RuntimeAssertionSite>,
) {
    context
        .facts
        .visit_dialogue_content_fragments(&mut |_, fragment| {
            let template = fragment.template();
            let slots = template
                .slots()
                .iter()
                .map(|slot| arcweft_core::plan::RuntimeDialogueContentSlotSeed {
                    slot: slot.slot(),
                    role: slot.role(),
                    semantic_type: slot.semantic_type(),
                })
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let effects = dialogue_content_effect_slot_seeds(fragment.effects());
            if let Err(error) = builder.register_dialogue_content_template_seed(
                arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed {
                    id: template.id(),
                    digest: template.digest(),
                    slots,
                    effects,
                },
            ) {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue content template {} is invalid: {error}",
                    template.id()
                )));
            }
        });
    let mut value_definitions = Vec::new();
    let mut content_definitions = Vec::new();
    context
        .facts
        .visit_dialogue_applications(&mut |scope, owner, application| {
            let Some((dialogue, pending_values)) = lower_dialogue_application(
                context,
                scope,
                owner,
                application,
                &dialogue_effect_sites,
                builder,
                errors,
            ) else {
                return;
            };
            content_definitions.push(dialogue);
            value_definitions.extend(pending_values);
        });
    for definition in value_definitions {
        if let Err(error) = builder.define_function_site_seed(&definition.site, definition.body) {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue value {:?} is invalid: {error}",
                definition.expression
            )));
        }
    }
    let mut content_handles = BTreeMap::new();
    let mut assertion_sites = Vec::new();
    for definition in content_definitions {
        let template_id = definition.template.id;
        let seed = arcweft_core::plan::RuntimeDialogueContentPlanSeed {
            line: definition.line,
            template: definition.template,
            values: definition
                .values
                .into_iter()
                .map(|(slot, role, function, captures)| {
                    arcweft_core::plan::RuntimeDialogueValueSiteSeed {
                        slot,
                        role,
                        function,
                        captures,
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            effect_sites: definition
                .effect_sites
                .into_iter()
                .map(|(site, function, captures)| {
                    arcweft_core::plan::RuntimeDialogueEffectSiteSeed {
                        site,
                        function,
                        captures,
                    }
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            marks: definition.marks.clone(),
            effect_site_count: definition.effect_site_count,
        };
        match builder.push_dialogue_content_seed(seed) {
            Ok(handle) => {
                let (group, assertions) = match line_plan::lower_dialogue_line_plan(
                    context,
                    definition.scope,
                    definition.owner,
                    &handle,
                    template_id,
                ) {
                    Ok(group) => group,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                if let Err(error) = builder
                    .push_line_task_group_seed(group)
                    .and_then(|group| builder.attach_line_task_group_seed(&handle, &group))
                {
                    errors.push(RuntimePlanLowerError::new(error.to_string()));
                    continue;
                }
                content_handles.insert(template_id, handle);
                assertion_sites.extend(assertions);
            }
            Err(error) => errors.push(RuntimePlanLowerError::new(error.to_string())),
        }
    }
    (content_handles, assertion_sites)
}

fn reserve_dialogue_effect_sites<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    Vec<PendingDialogueEffectDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    context
        .facts
        .visit_dialogue_content_fragments(&mut |scope, fragment| {
        let source = fragment.source();
        let locals = match context.dialogue_locals(scope.scope()) {
            Ok(locals) => locals,
            Err(error) => {
                errors.push(error);
                return;
            }
        };
        for effect in fragment.effects() {
            let program = RuntimeDialogueEffectProgramKey::new(
                fragment.template().id(),
                effect.site(),
            );
            let captures = effect
                .captures()
                .iter()
                .enumerate()
                .map(|(position, capture)| {
                    let position = u32::try_from(position).map_err(|_| {
                        "dialogue content effect capture position exceeds checked limits"
                            .to_owned()
                    })?;
                    let local = locals
                        .get(&capture.local())
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "dialogue content effect site {:?} capture {:?} has no admitted local",
                                effect.site(),
                                capture.local()
                            )
                        })?;
                    let input_local = context
                        .dialogue_effect_capture_input_locals
                        .get(&RuntimeDialogueEffectCaptureKey::new(program, position))
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "dialogue content effect site {:?} capture {position} has no admitted synthetic input local",
                                effect.site()
                            )
                        })?;
                    let local_ty = scope
                        .local_type(capture.local())
                        .ok_or_else(|| {
                            format!(
                                "dialogue content effect site {:?} capture {:?} has no accepted type",
                                effect.site(),
                                capture.local()
                            )
                        })?;
                    if capture.ty().identity() != local_ty.identity() {
                        return Err(format!(
                            "dialogue content effect site {:?} capture {position} type disagrees with accepted local",
                            effect.site()
                        ));
                    }
                    Ok((local, input_local, capture.ty().identity()))
                })
                .collect::<Result<Vec<_>, _>>();
            let result = effect.operation().result().clone();
            let result = if matches!(result.shape(), RuntimeTypeShape::Unit) {
                Ok(result)
            } else {
                Err(format!(
                    "dialogue content effect site {:?} operation is not a Unit expression",
                    effect.site()
                ))
            };
            let effects =
                RuntimeFunctionEffectSet::try_from_effects(effect.effects().iter().cloned())
                    .map_err(|error| error.to_string());
            let capture_inputs = captures.and_then(|captures| {
                captures
                    .into_iter()
                    .enumerate()
                    .map(|(position, (binding, input_local, ty))| {
                        let position = u32::try_from(position).map_err(|_| {
                            "dialogue content effect capture position exceeds checked limits"
                                .to_owned()
                        })?;
                        Ok(RuntimeFunctionInputBindingSeed {
                            source: RuntimeFunctionInputSource::Capture { position },
                            input_local,
                            pattern: RuntimePatternSeed::new(
                                ty,
                                RuntimePatternSeedKind::Bind {
                                    mutable: false,
                                    local: binding,
                                },
                            ),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
            });
            let declaration = capture_inputs.and_then(|captures| {
                result.and_then(|result| {
                    effects
                        .clone()
                        .map(|effects| RuntimeFunctionSiteDeclarationSeed {
                            inputs: captures.into_boxed_slice(),
                            result: result.identity(),
                            body_kind: RuntimeFunctionSiteBodyKind::Executable,
                            effects,
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
            let site = match builder.reserve_function_site_seed(declaration) {
                Ok(site) => site,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(error.to_string()));
                    continue;
                }
            };
            if sites.insert(program, site.clone()).is_some() {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue content template {} repeats effect site {:?}",
                    program.template(),
                    effect.site()
                )));
                continue;
            }
            let effects =
                RuntimeFunctionEffectSet::try_from_effects(effect.effects().iter().cloned())
                    .expect("effect set reservation was already validated");
            definitions.push(PendingDialogueEffectDefinition {
                key: program,
                scope,
                module: source.module(),
                site,
                effects,
                operation: effect.operation().clone(),
            });
        }
    });
    (sites, definitions)
}

fn define_dialogue_effect_sites<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    definitions: Vec<PendingDialogueEffectDefinition<'facts>>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Some(module) = module_by_id(context.project, definition.module) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue content effect {:?} module is absent",
                definition.key
            )));
            continue;
        };
        let scope = definition.scope;
        let expr = match context.scoped_expr_lowerer(module, scope) {
            Ok(expr) => expr,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let operation = match lower_evaluated_effect(&expr, definition.operation.effect()) {
            Ok(operation) => operation,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue content effect {:?} lowering failed: {error}",
                    definition.key
                )));
                continue;
            }
        };
        let body = RuntimeFunctionSiteBodySeed::Executable(RuntimeFunctionExecutableBodySeed {
            effects: definition.effects,
            ops: vec![RuntimeFlowOpSeed::EvaluatedEffect(operation)].into_boxed_slice(),
        });
        if let Err(error) = builder.define_function_site_seed(&definition.site, body) {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue content effect {:?} definition failed: {error}",
                definition.key
            )));
        }
    }
}

fn lower_dialogue_application<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    owner: ExprId,
    application: &RuntimeDialogueApplication,
    dialogue_effect_sites: &BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Option<(
    PendingDialogueContentDefinition<'facts>,
    Vec<PendingDialogueValueDefinition>,
)> {
    let Some(module) = module_by_id(context.project, owner.module()) else {
        errors.push(RuntimePlanLowerError::new("dialogue module is absent"));
        return None;
    };
    let Some(fragment) = scope.dialogue_content_fragment_for_source(owner) else {
        errors.push(RuntimePlanLowerError::new(format!(
            "dialogue content template {} is absent from the compiler text catalog",
            application.content().template_id()
        )));
        return None;
    };
    let template = fragment.template();
    if application.content().template_digest() != template.digest() {
        errors.push(RuntimePlanLowerError::new(format!(
            "dialogue content template {} digest disagrees with the compiler text catalog",
            template.id()
        )));
        return None;
    }
    let locals = match context.dialogue_locals(scope.scope()) {
        Ok(locals) => locals,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let lowerer = match context.scoped_expr_lowerer(module, scope) {
        Ok(lowerer) => lowerer,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let mut invalid = false;
    let mut values = Vec::new();
    let mut value_definitions = Vec::new();
    for value in fragment.values() {
        let body = match lowerer.lower(value.expression()) {
            Ok(body) => body,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error));
                invalid = true;
                continue;
            }
        };
        let captures = body.free_locals();
        let mut capture_values = Vec::with_capacity(captures.len());
        let capture_inputs = captures
            .iter()
            .enumerate()
            .map(|(position, input_local)| {
                let position = u32::try_from(position).map_err(|_| {
                    "dialogue value capture position exceeds checked limits".to_owned()
                })?;
                let (local, ty) = locals
                    .iter()
                    .find_map(|(local, candidate)| {
                        (candidate == input_local).then(|| (*local, scope.local_type(*local)))
                    })
                    .ok_or_else(|| {
                        format!(
                            "dialogue value {:?} capture has no accepted local authority",
                            value.expression()
                        )
                    })?;
                let ty = ty.ok_or_else(|| {
                    format!(
                        "dialogue value {:?} capture {local:?} has no accepted type",
                        value.expression()
                    )
                })?;
                let input_local_seed = context
                    .dialogue_value_capture_input_locals
                    .get(&RuntimeDialogueValueCaptureKey::new(
                        application.content().template_id(),
                        value.slot(),
                        position,
                    ))
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "dialogue value {:?} capture {position} has no admitted synthetic input local",
                            value.expression()
                        )
                    })?;
                capture_values.push(RuntimeExprSeed::new(
                    ty.identity(),
                    arcweft_core::plan::RuntimeExprSeedKind::Local(input_local.clone()),
                ));
                Ok(RuntimeFunctionInputBindingSeed {
                    source: RuntimeFunctionInputSource::Capture { position },
                    input_local: input_local_seed,
                    pattern: RuntimePatternSeed::new(
                        ty.identity(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: input_local.clone(),
                        },
                    ),
                })
            })
            .collect::<Result<Vec<_>, String>>();
        let declaration = capture_inputs.map(|captures| RuntimeFunctionSiteDeclarationSeed {
            inputs: captures.into_boxed_slice(),
            result: body.ty(),
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeFunctionEffectSet::empty(),
        });
        let declaration = match declaration {
            Ok(declaration) => declaration,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue value {:?} capture projection failed: {error}",
                    value.expression()
                )));
                invalid = true;
                continue;
            }
        };
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                let captures = capture_values.into_boxed_slice();
                values.push((value.slot(), value.role(), site.clone(), captures.clone()));
                value_definitions.push(PendingDialogueValueDefinition {
                    expression: value.expression(),
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
    let Some(effect_site_count) =
        arcweft_core::runtime_id::RuntimeDialogueEffectSiteCount::try_from_len(
            fragment.effects().len(),
        )
    else {
        errors.push(RuntimePlanLowerError::new(
            "dialogue effect-site count exceeds the runtime u32 admission bound",
        ));
        return None;
    };
    // The compiler/text-model template owns the complete structural mark
    // manifest, including marks nested inside Scope and Ruby bodies.  Reuse
    // that authority instead of walking only top-level nodes (which would
    // silently drop marks from recursively lowered Content emissions).
    let marks = template
        .marks()
        .iter()
        .map(|mark| mark.diagnostic_name().to_owned())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let slots = template
        .slots()
        .iter()
        .map(|slot| arcweft_core::plan::RuntimeDialogueContentSlotSeed {
            slot: slot.slot(),
            role: slot.role(),
            semantic_type: slot.semantic_type(),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let effects = dialogue_content_effect_slot_seeds(fragment.effects());
    let effect_sites = fragment
        .effects()
        .iter()
        .map(|effect| {
            dialogue_effect_sites
                .get(&RuntimeDialogueEffectProgramKey::new(
                    fragment.template().id(),
                    effect.site(),
                ))
                .cloned()
                .and_then(|function| {
                    let captures = effect
                        .captures()
                        .iter()
                        .map(|capture| {
                            locals.get(&capture.local()).cloned().map(|local| {
                                RuntimeExprSeed::new(
                                    capture.ty().identity(),
                                    arcweft_core::plan::RuntimeExprSeedKind::Local(local),
                                )
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some((effect.site(), function, captures.into_boxed_slice()))
                })
                .ok_or_else(|| {
                    format!(
                        "dialogue content effect site {:?} has no reserved callback site",
                        effect.site()
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>();
    let effect_sites = match effect_sites {
        Ok(effect_sites) => effect_sites,
        Err(error) => {
            errors.push(RuntimePlanLowerError::new(error));
            return None;
        }
    };
    Some((
        PendingDialogueContentDefinition {
            scope,
            owner,
            line: application.content().line().clone(),
            template: arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed {
                id: application.content().template_id(),
                digest: application.content().template_digest(),
                slots,
                effects,
            },
            values,
            effect_sites,
            marks,
            effect_site_count,
        },
        value_definitions,
    ))
}

fn dialogue_content_effect_slot_seeds(
    effects: &[crate::semantic_facts::RuntimeDialogueEffectProgramFact],
) -> Box<[arcweft_core::plan::RuntimeDialogueContentEffectSlotSeed]> {
    effects
        .iter()
        .map(
            |effect| arcweft_core::plan::RuntimeDialogueContentEffectSlotSeed {
                site: effect.site(),
                trigger: match effect.trigger() {
                    crate::semantic_facts::RuntimeDialogueEffectTrigger::Content => {
                        arcweft_core::plan::RuntimeDialogueContentEffectTrigger::Content
                    }
                    crate::semantic_facts::RuntimeDialogueEffectTrigger::Delay {
                        duration, ..
                    } => arcweft_core::plan::RuntimeDialogueContentEffectTrigger::Delay {
                        duration: *duration,
                    },
                },
                capture_types: effect
                    .captures()
                    .iter()
                    .map(|capture| capture.ty().identity())
                    .collect(),
            },
        )
        .collect()
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
            RuntimeEntryCallableBody::FunctionSite => {
                let Some(root) = context
                    .facts
                    .project_function_root_for_callable(&callable.role().callable)
                else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "Entry callable `{}` has no checked project-function root",
                        callable.role().callable.as_str()
                    )));
                    continue;
                };
                let Some(site) = context.project_function_sites.get(root.instance()).cloned()
                else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "Entry callable `{}` project-function root has no reserved function site",
                        callable.role().callable.as_str()
                    )));
                    continue;
                };
                executables.push(arcweft_core::plan::RuntimeCallableExecutableSeed {
                    callable: callable.role().callable.clone(),
                    contract: callable.role().contract,
                    code: arcweft_core::plan::RuntimeCallableExecutableSeedCode::FunctionSite(site),
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
    let root = context
        .facts
        .project_function_root_for_callable(&callable.role().callable)
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry controller `{}` has no checked project-function root",
                callable.role().callable.as_str()
            ))
        })?;
    let instance = context
        .facts
        .project_function_instance(root.instance())
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry controller `{}` project-function root instance is absent",
                callable.role().callable.as_str()
            ))
        })?;
    let site = context
        .project_function_sites
        .get(root.instance())
        .cloned()
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry controller `{}` project-function root has no reserved function site",
                callable.role().callable.as_str()
            ))
        })?;
    let result_ty = match instance.function_type().shape() {
        RuntimeTypeShape::Function { result, parameters }
            if parameters.is_empty()
                && instance.callable().attached_content_abi().is_none()
                && instance.key().group().get() == 0 =>
        {
            result.identity()
        }
        _ => {
            return Err(RuntimePlanLowerError::new(format!(
                "Entry controller `{}` root instance does not have the empty group-0 ABI",
                callable.role().callable.as_str()
            )));
        }
    };
    let result_local = context
        .controller_result_locals
        .get(&callable.role().callable)
        .cloned()
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Entry controller `{}` has no admitted result local",
                callable.role().callable.as_str()
            ))
        })?;
    let result_pattern = RuntimePatternSeed::new(
        result_ty,
        RuntimePatternSeedKind::Bind {
            mutable: false,
            local: result_local.clone(),
        },
    );
    let project_call = RuntimeFlowOpSeed::ProjectCall {
        plan: RuntimeProjectCallPlanSeed {
            input: RuntimeProjectCallInputSeed::Direct,
            completed_group: 0,
            operands: Box::new([]),
            ordinary: Box::new([]),
            attached: None,
            outcome: RuntimeProjectCallOutcomeSeed::Invoke {
                function_site: site,
            },
        },
        result: result_pattern,
    };
    let ops = vec![
        project_call,
        RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
            result_ty,
            arcweft_core::plan::RuntimeExprSeedKind::Local(result_local),
        )),
    ];
    let flow_executable = RuntimeFlowExecutable {
        flow: flow.clone(),
        contract: arcweft_core::entry::FlowContractHash::from_bytes(
            *callable.role().contract.as_bytes(),
        ),
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
        assertions: Vec::new(),
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

fn lower_entry_flows(
    project: HirExecutableProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    input: &RuntimeEntryLoweringInput,
    schemas: &[RuntimeFlowSchema],
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
        if schemas
            .iter()
            .find(|schema| schema.flow == flow.executable().flow)
            != Some(flow.expected_schema())
        {
            errors.push(RuntimePlanLowerError::new(format!(
                "checked Entry Flow schema `{}` does not match the sole final-HIR Flow schema",
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
    Closure(arcweft_lang_sema::callable::CheckedClosureId),
    Flow(FlowRuntimeId),
    Line(RuntimeLineId),
}

impl RuntimeAssertionOwner {
    fn label(&self) -> String {
        match self {
            Self::Callable(declaration) => declaration.qualified_name(),
            Self::Closure(closure) => format!(
                "closure@{}:{}",
                closure.expression().source().id(),
                closure.expression().range().start()
            ),
            Self::Flow(flow) => flow.canonical_label(),
            Self::Line(line) => line.canonical_label(),
        }
    }
}

struct FinalFlowLowerer<'a> {
    module: &'a HirModule,
    facts: &'a RuntimePlanSemanticFacts,
    semantic_facts: RuntimeScopedExecutableSemanticFactView<'a>,
    package: &'a CallablePackageId,
    locals: &'a BTreeMap<LocalId, RuntimeLocalSeedId>,
    trait_methods: &'a BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodSeedId>,
    function_sites: &'a BTreeMap<ExprId, RuntimeFunctionSiteSeedId>,
    closure_sites: &'a BTreeMap<RuntimeClosureInstanceKey, RuntimeFunctionSiteSeedId>,
    project_function_sites:
        &'a BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    project_default_function_sites:
        &'a BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    dialogue_effect_sites: &'a BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    dialogue_content: &'a BTreeMap<
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
        RuntimeDialogueContentPlanSeedId,
    >,
    control: &'a ControlLocals,
    specialized_operand_locals: &'a BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
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
    Branch {
        owner: ExprId,
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
            semantic_facts: RuntimeScopedExecutableSemanticFactView::global(context.facts),
            package: context.project.package(),
            locals: context.locals,
            trait_methods: context.trait_methods,
            function_sites: context.function_sites,
            closure_sites: context.closure_sites,
            project_function_sites: context.project_function_sites,
            project_default_function_sites: context.project_default_function_sites,
            dialogue_effect_sites: context.dialogue_effect_sites,
            dialogue_content: context.dialogue_content,
            control: context.control,
            specialized_operand_locals: context.specialized_operand_locals,
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

    fn with_project_instance(
        mut self,
        frame: &'a ProjectFunctionFrameLocals,
        key: &'a RuntimeProjectFunctionInstanceKey,
        semantics: &'a RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        self.locals = &frame.hir;
        self.specialized_operand_locals = &frame.specialized_operands;
        self.control = &frame.control;
        self.semantic_facts =
            RuntimeScopedExecutableSemanticFactView::project_function(key, semantics);
        self
    }

    fn with_closure(
        mut self,
        frame: &'a ClosureFrameLocals,
        key: &'a RuntimeClosureInstanceKey,
        semantics: &'a RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        self.locals = &frame.hir;
        self.specialized_operand_locals = &frame.specialized_operands;
        self.control = &frame.control;
        self.semantic_facts = RuntimeScopedExecutableSemanticFactView::closure(key, semantics);
        self
    }

    fn with_dialogue_scope(
        mut self,
        scope: RuntimeScopedExecutableSemanticFactView<'a>,
        control: &'a ControlLocals,
        locals: &'a BTreeMap<LocalId, RuntimeLocalSeedId>,
        specialized_operand_locals: &'a BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
    ) -> Self {
        self.locals = locals;
        self.specialized_operand_locals = specialized_operand_locals;
        self.semantic_facts = scope;
        self.control = control;
        self
    }

    fn expr_lowerer(&self) -> FinalExprLowerer<'_> {
        let lowerer = FinalExprLowerer::new(
            self.module,
            self.facts,
            self.locals,
            self.trait_methods,
            self.function_sites,
            self.dialogue_effect_sites,
            (&self.control.pipes, &self.control.tries),
        )
        .with_closure_sites(self.closure_sites)
        .with_specialized_operand_locals(self.specialized_operand_locals);
        lowerer.with_scoped_semantics(self.semantic_facts)
    }

    fn pattern_lowerer(&self) -> FinalPatternLowerer<'_> {
        let lowerer = FinalPatternLowerer::new(self.module, self.facts, self.locals);
        lowerer.with_semantic_facts(self.semantic_facts.facts())
    }

    fn expression_type(
        &self,
        expression: ExprId,
    ) -> Result<&RuntimeNormalizedType, RuntimePlanLowerError> {
        self.semantic_facts
            .expression_type(expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "accepted type is missing for expression {expression:?}"
                ))
            })
    }

    fn expression_children(&self, expression: ExprId) -> Result<&[ExprId], RuntimePlanLowerError> {
        self.semantic_facts
            .expression_children(expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "checked expression row is missing for {expression:?}"
                ))
            })
    }

    fn call(&self, expression: ExprId) -> Option<&RuntimeResolvedCall> {
        self.semantic_facts.call(expression)
    }

    /// Source-ordered evaluated children of an eager expression. The accepted
    /// HIR row owns child order; checked call disposition excludes static
    /// callee selectors while retaining value callees and receiver operands.
    fn evaluated_expression_children(
        &self,
        expression: ExprId,
    ) -> Result<Vec<ExprId>, RuntimePlanLowerError> {
        let callee = if let Some(call) = self.call(expression) {
            let hir = self
                .module
                .resolve_expr(expression)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            let invocation = match hir.kind() {
                HirExprKind::Call(invocation) => Some(invocation),
                HirExprKind::AttachedContentApplication(application) => {
                    application.family().invocation()
                }
                _ => None,
            };
            invocation
                .and_then(|invocation| invocation.callee().value_expression())
                .map(|callee| (callee, call.evaluates_callee(callee)))
        } else {
            None
        };
        let children = self.expression_children(expression)?;
        let mut evaluated = Vec::with_capacity(children.len());
        if let Some((callee, true)) = callee {
            if !children.contains(&callee) {
                return Err(RuntimePlanLowerError::new(format!(
                    "evaluated callee {callee:?} is absent from its checked expression graph"
                )));
            }
            evaluated.push(callee);
        }
        evaluated.extend(
            children
                .iter()
                .copied()
                .filter(|child| callee.is_none_or(|(callee, _)| *child != callee)),
        );
        Ok(evaluated)
    }

    fn value(&self, expression: ExprId) -> Option<&RuntimeResolvedValue> {
        self.semantic_facts.value(expression)
    }

    fn expression_literal(&self, expression: ExprId) -> Option<&RuntimeValue> {
        self.semantic_facts.expression_literal(expression)
    }

    fn postfix_candidate(&self, expression: ExprId) -> Option<ExprId> {
        self.semantic_facts.postfix_candidate(expression)
    }

    fn implicit_callable(
        &self,
        expression: ExprId,
    ) -> Option<&crate::semantic_facts::RuntimeImplicitCallableFact> {
        self.semantic_facts.implicit_callable(expression)
    }

    fn choice(&self, expression: ExprId) -> Option<&crate::semantic_facts::RuntimeChoiceFact> {
        self.semantic_facts.choice(expression)
    }

    fn awaited(&self, expression: ExprId) -> Option<&RuntimeAwaitFact> {
        self.semantic_facts.awaited(expression)
    }

    fn pipe(&self, expression: ExprId) -> Option<&crate::semantic_facts::RuntimePipeFact> {
        self.semantic_facts.pipe(expression)
    }

    fn tried(&self, expression: ExprId) -> Option<&RuntimeTryFact> {
        self.semantic_facts.tried(expression)
    }

    fn evaluated_effect(&self, statement: StmtId) -> Option<&RuntimeEvaluatedEffectFact> {
        self.semantic_facts.evaluated_effect(statement)
    }

    fn iteration(&self, statement: StmtId) -> Option<&RuntimeIteratorFact> {
        self.semantic_facts.iteration(statement)
    }

    fn assertion(&self, statement: StmtId) -> Option<RuntimeAssertionAdmission> {
        self.semantic_facts.assertion(statement)
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
                // A generic postfix bracket is retained in the Flow body by
                // its source application owner.  Once semantic selection has
                // chosen the Dialogue candidate, the runtime fact and content
                // handle are keyed by that candidate owner.  Resolve this
                // exact source-to-semantic relation from the accepted HIR
                // inventory instead of reinterpreting the bracket here.
                let semantic_expression =
                    self.dialogue_from_source(*expression).ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue source application {expression:?} has no accepted line"
                        ))
                    })?;
                let application = self
                    .semantic_facts
                    .dialogue_application(semantic_expression)
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue application {expression:?} has no checked projection fact"
                        ))
                    })?;
                let content = self
                    .dialogue_content
                    .get(&application.content().template_id())
                    .cloned()
                    .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue application {expression:?} has no builder-issued content handle"
                    ))
                    })?;
                let mut ops = vec![RuntimeFlowOpSeed::Dialogue {
                    content,
                    result: arcweft_core::plan::RuntimeDialogueResultTargetSeed::discard(
                        application.line_result().identity(),
                    ),
                }];
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
                let pattern = self
                    .pattern_lowerer()
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
        match kind {
            HirStmtKind::Assertion { mode, conditions } => {
                self.lower_assertion(id, *mode, conditions)
            }
            HirStmtKind::Let {
                pattern: owner,
                initializer,
                ..
            } => {
                let binding = self
                    .pattern_lowerer()
                    .lower(*owner)
                    .map_err(RuntimePlanLowerError::new)?;
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
                        expr: self
                            .expr_lowerer()
                            .lower(*initializer)
                            .map_err(RuntimePlanLowerError::new)?,
                    }])
                }
            }
            HirStmtKind::Assign { value, .. } => Ok(vec![
                self.expr_lowerer()
                    .lower_flow_assignment(id, *value)
                    .map_err(RuntimePlanLowerError::new)?,
            ]),
            HirStmtKind::LetElse {
                pattern: owner,
                initializer,
                else_body,
                ..
            } => Ok(vec![RuntimeFlowOpSeed::LetElse {
                pattern: self
                    .pattern_lowerer()
                    .lower(*owner)
                    .map_err(RuntimePlanLowerError::new)?,
                expr: self
                    .expr_lowerer()
                    .lower(*initializer)
                    .map_err(RuntimePlanLowerError::new)?,
                else_ops: self.lower_statement_ids(else_body)?,
            }]),
            HirStmtKind::Return { value } => {
                if self.contains_flow_value_expression(*value)? {
                    return self.lower_flow_value(*value, RuntimeFlowValueContinuation::Return);
                }
                if let Some(host) = self.lower_host_call(*value, None)? {
                    let result = self.expression_type(*value)?;
                    if !matches!(result.shape(), RuntimeTypeShape::Never) {
                        return Err(RuntimePlanLowerError::new(format!(
                            "return host call {value:?} must have the Never result type"
                        )));
                    }
                    Ok(vec![host])
                } else {
                    Ok(vec![RuntimeFlowOpSeed::ReturnExpr(
                        self.expr_lowerer()
                            .lower(*value)
                            .map_err(RuntimePlanLowerError::new)?,
                    )])
                }
            }
            HirStmtKind::Goto { target } => {
                if let Some(RuntimeResolvedValue::ProjectItem(item)) = self.value(*target)
                    && let Some(target) = item.flow_runtime_id()
                {
                    Ok(vec![RuntimeFlowOpSeed::Goto(target.clone())])
                } else {
                    Ok(vec![RuntimeFlowOpSeed::GotoExpr(
                        self.expr_lowerer()
                            .lower(*target)
                            .map_err(RuntimePlanLowerError::new)?,
                    )])
                }
            }
            HirStmtKind::Expression { expression: thread } => {
                if let Some(effect) = self.evaluated_effect(id) {
                    return Ok(vec![RuntimeFlowOpSeed::EvaluatedEffect(
                        lower_evaluated_effect(&self.expr_lowerer(), effect.effect())?,
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
                condition: self
                    .expr_lowerer()
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
                pattern: self
                    .pattern_lowerer()
                    .lower(branch.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: self
                    .expr_lowerer()
                    .lower(branch.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: branch
                    .guard()
                    .map(|guard| {
                        self.expr_lowerer()
                            .lower(guard)
                            .map_err(RuntimePlanLowerError::new)
                    })
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
                        pattern: self
                            .pattern_lowerer()
                            .lower(arm.pattern())
                            .map_err(RuntimePlanLowerError::new)?,
                        guard: arm
                            .guard()
                            .map(|guard| {
                                self.expr_lowerer()
                                    .lower(guard)
                                    .map_err(RuntimePlanLowerError::new)
                            })
                            .transpose()?,
                        ops,
                    });
                }
                Ok(vec![RuntimeFlowOpSeed::Match {
                    scrutinee: self
                        .expr_lowerer()
                        .lower(matched.scrutinee())
                        .map_err(RuntimePlanLowerError::new)?,
                    arms,
                }])
            }
            HirStmtKind::While(while_stmt) => Ok(vec![RuntimeFlowOpSeed::While {
                condition: self
                    .expr_lowerer()
                    .lower(while_stmt.condition())
                    .map_err(RuntimePlanLowerError::new)?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::WhileLet(while_stmt) => Ok(vec![RuntimeFlowOpSeed::WhileLet {
                pattern: self
                    .pattern_lowerer()
                    .lower(while_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                expr: self
                    .expr_lowerer()
                    .lower(while_stmt.scrutinee())
                    .map_err(RuntimePlanLowerError::new)?,
                guard: while_stmt
                    .guard()
                    .map(|guard| {
                        self.expr_lowerer()
                            .lower(guard)
                            .map_err(RuntimePlanLowerError::new)
                    })
                    .transpose()?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::For(for_stmt) => Ok(vec![RuntimeFlowOpSeed::For {
                pattern: self
                    .pattern_lowerer()
                    .lower(for_stmt.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                source: self
                    .expr_lowerer()
                    .lower(for_stmt.source())
                    .map_err(RuntimePlanLowerError::new)?,
                evidence: match self.iteration(id).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "checked iteration evidence is missing for For statement {id:?}"
                    ))
                })? {
                    RuntimeIteratorFact::Builtin(evidence) => {
                        RuntimeIteratorEvidenceSeed::Builtin(RuntimeBuiltinIteratorEvidenceSeed {
                            family: evidence.family(),
                            item: evidence.item().identity(),
                            iterator: evidence.iterator().identity(),
                            next_value: evidence.next_value().identity(),
                            step: evidence.step().identity(),
                        })
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
                        .map(|value| {
                            self.expr_lowerer()
                                .lower(value)
                                .map_err(RuntimePlanLowerError::new)
                        })
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
        if let Some(semantic_dialogue) = self.synthetic_dialogue_from_source(expression) {
            return self.contains_flow_value_expression(semantic_dialogue);
        }
        if let Some(selected) = self.postfix_candidate(expression) {
            if selected == expression {
                return Err(RuntimePlanLowerError::new(format!(
                    "postfix expression {expression:?} selected itself"
                )));
            }
            return self.contains_flow_value_expression(selected);
        }
        if self.implicit_callable(expression).is_some() {
            return Ok(false);
        }
        if self.call(expression).is_some_and(|call| {
            call.project_function().is_some()
                || matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
        }) {
            return Ok(true);
        }
        if self.call(expression).is_some_and(|call| {
            matches!(
                call.dispatch(),
                RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_))
            )
        }) {
            return Ok(true);
        }
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve flow value expression {expression:?}: {error}"
            ))
        })?;
        if matches!(
            resolved.kind(),
            HirExprKind::AttachedContentApplication(_)
                | HirExprKind::Await(_)
                | HirExprKind::Choice(_)
                | HirExprKind::Loop(_)
                | HirExprKind::Try(_)
        ) || matches!(
            resolved.kind(),
            HirExprKind::ComputationBlock(block)
                if matches!(
                    block.kind(),
                    arcweft_lang_hir::expr::HirComputationBlockKind::Result
                        | arcweft_lang_hir::expr::HirComputationBlockKind::Option
                )
        ) {
            return Ok(true);
        }
        for child in self.expression_children(expression)? {
            if self.contains_flow_value_expression(*child)? {
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
        if let Some(value) = overrides.get(&expression) {
            return self.apply_value_continuation(value.clone(), continuation);
        }
        if let Some(semantic_dialogue) = self.synthetic_dialogue_from_source(expression) {
            return self.lower_flow_value_with_overrides(
                semantic_dialogue,
                continuation,
                overrides,
            );
        }
        if let Some(selected) = self.postfix_candidate(expression) {
            if selected == expression {
                return Err(RuntimePlanLowerError::new(format!(
                    "postfix expression {expression:?} selected itself"
                )));
            }
            return self.lower_flow_value_with_overrides(selected, continuation, overrides);
        }
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve flow value expression {expression:?}: {error}"
            ))
        })?;
        if let Some(call) = self
            .call(expression)
            .filter(|call| {
                call.project_function().is_some()
                    || matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
                    || matches!(
                        call.dispatch(),
                        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(
                            _
                        ))
                    )
            })
            .cloned()
        {
            for child in self.evaluated_expression_children(expression)? {
                if !overrides.contains_key(&child) {
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
            }
            if matches!(
                call.dispatch(),
                RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_))
            ) {
                return self.lower_host_call_value(expression, continuation, overrides);
            }
            return self.lower_callable_value(expression, &call, continuation, overrides);
        }
        if self.implicit_callable(expression).is_some() {
            let value = self
                .expr_lowerer()
                .lower(expression)
                .map_err(RuntimePlanLowerError::new)?;
            return self.apply_value_continuation(value, continuation);
        }
        match resolved.kind() {
            HirExprKind::If(branch) => {
                self.lower_value_branch(expression, branch.condition(), continuation, overrides)
            }
            HirExprKind::IfLet(branch) => {
                self.lower_value_branch(expression, branch.scrutinee(), continuation, overrides)
            }
            HirExprKind::Match(branch) => {
                self.lower_value_branch(expression, branch.scrutinee(), continuation, overrides)
            }
            HirExprKind::Binary(binary)
                if matches!(
                    binary.operator(),
                    arcweft_lang_hir::expr::HirBinaryOp::And
                        | arcweft_lang_hir::expr::HirBinaryOp::Or
                        | arcweft_lang_hir::expr::HirBinaryOp::Implies
                ) =>
            {
                self.lower_value_branch(expression, binary.left(), continuation, overrides)
            }
            HirExprKind::AttachedContentApplication(application) => {
                if !matches!(
                    application.family(),
                    arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                        ..
                    }
                ) {
                    return Err(RuntimePlanLowerError::new(format!(
                        "attached content call {expression:?} cannot lower as a DialogueLine value"
                    )));
                }
                self.lower_dialogue_value(expression, continuation)
            }
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
                let flow_child = if self.contains_flow_value_expression(expression)? {
                    self.evaluated_expression_children(expression)?
                        .into_iter()
                        .find(|child| !overrides.contains_key(child))
                } else {
                    None
                };
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
                let value = self
                    .expr_lowerer()
                    .with_overrides(overrides)
                    .lower(expression)
                    .map_err(RuntimePlanLowerError::new)?;
                self.apply_value_continuation(value, continuation)
            }
        }
    }

    fn lower_callable_value(
        &mut self,
        expression: ExprId,
        call: &RuntimeResolvedCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let result_type = self.expression_type(expression)?.clone();
        let local = self
            .control
            .expression_values
            .get(&expression)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "call {expression:?} has no admitted result local"
                ))
            })?;
        let result = bind_seed(&result_type, local.clone());
        let operation = if call.project_function().is_some() {
            RuntimeFlowOpSeed::ProjectCall {
                plan: self.lower_project_call_plan(expression, call, &overrides)?,
                result,
            }
        } else {
            let (callee, args) = self
                .expr_lowerer()
                .with_overrides(overrides)
                .lower_function_application(expression, call)
                .map_err(RuntimePlanLowerError::new)?;
            RuntimeFlowOpSeed::ApplyFunction {
                callee,
                args,
                result,
            }
        };
        let mut ops = vec![operation];
        ops.extend(self.apply_value_continuation(local_seed(&result_type, local), continuation)?);
        Ok(ops)
    }
    fn lower_project_call_plan(
        &self,
        expression: ExprId,
        call: &RuntimeResolvedCall,
        overrides: &BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<RuntimeProjectCallPlanSeed, RuntimePlanLowerError> {
        let checked = call.project_function().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "project call {expression:?} has no checked project-function plan"
            ))
        })?;
        let lowerer = self.expr_lowerer().with_overrides(overrides.clone());
        let input = match checked.input() {
            crate::semantic_facts::RuntimeProjectFunctionCallInput::Direct => {
                RuntimeProjectCallInputSeed::Direct
            }
            crate::semantic_facts::RuntimeProjectFunctionCallInput::Continuation {
                callee,
                abi,
            } => RuntimeProjectCallInputSeed::Continuation {
                callee: lowerer.lower(*callee).map_err(RuntimePlanLowerError::new)?,
                expected_abi: RuntimeProjectCallAbiSeed {
                    lineage: abi.lineage(),
                    function_type: abi.function_type().identity(),
                    prefix_types: abi
                        .prefix_types()
                        .iter()
                        .map(RuntimeNormalizedType::identity)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            },
        };
        let mut operands = call
            .operands()
            .iter()
            .map(|operand| {
                let value = lowerer
                    .lower_scalar_operand_source(operand.source(), operand.ty())
                    .map_err(RuntimePlanLowerError::new)?;
                let mode = match operand.projection() {
                    RuntimeResolvedCallOperandProjection::Scalar => RuntimeCallArgumentMode::Value,
                    RuntimeResolvedCallOperandProjection::SpreadContainer(_) => {
                        RuntimeCallArgumentMode::Spread
                    }
                };
                Ok(RuntimeProjectCallOperandSeed {
                    value,
                    mode,
                    abi_position: operand.abi_position(),
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
        let attached = call
            .positioned_attached_content()
            .map(|positioned| {
                let descriptor = checked
                    .callable()
                    .attached_content_abi()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "project call {expression:?} has an attached row without a callable descriptor"
                        ))
                    })?;
                if descriptor.group() != call.completed_group() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "project call {expression:?} attached row disagrees with its checked callable group/position"
                    )));
                }
                let source_index = if let Some(source) = positioned.content().source() {
                    let source_index = u32::try_from(operands.len()).map_err(|_| {
                        RuntimePlanLowerError::new(format!(
                            "project call {expression:?} operand count exceeds checked limits"
                        ))
                    })?;
                    let value = lowerer
                        .lower_attached_content_source(source)
                        .map_err(RuntimePlanLowerError::new)?;
                    operands.push(RuntimeProjectCallOperandSeed {
                        value,
                        mode: RuntimeCallArgumentMode::Value,
                        abi_position: positioned.abi_position(),
                    });
                    Some(source_index)
                } else {
                    None
                };
                let presence = match positioned.content() {
                    RuntimeResolvedAttachedContent::Required { .. } => {
                        RuntimeProjectCallAttachedPresenceSeed::RequiredPresent
                    }
                    RuntimeResolvedAttachedContent::OptionalPresent { .. } => {
                        RuntimeProjectCallAttachedPresenceSeed::OptionalPresent
                    }
                    RuntimeResolvedAttachedContent::OptionalOmitted { .. } => {
                        RuntimeProjectCallAttachedPresenceSeed::OptionalOmitted
                    }
                    RuntimeResolvedAttachedContent::DefaultedPresent { .. } => {
                        RuntimeProjectCallAttachedPresenceSeed::DefaultedPresent
                    }
                    RuntimeResolvedAttachedContent::DefaultedOmitted { .. } => {
                        let crate::semantic_facts::RuntimeProjectFunctionCallOutcome::Invoke {
                            instance,
                        } = checked.outcome()
                        else {
                            return Err(RuntimePlanLowerError::new(format!(
                                "project call {expression:?} omitted a default before terminal invocation"
                            )));
                        };
                        let default_site = self
                            .project_default_function_sites
                            .get(instance)
                            .cloned()
                            .ok_or_else(|| {
                                RuntimePlanLowerError::new(format!(
                                    "project call {expression:?} has no reserved attached default site for {:?}",
                                    instance
                                ))
                            })?;
                        let default = self
                            .facts
                            .project_function_instance(instance)
                            .and_then(RuntimeProjectFunctionInstanceFact::attached_default)
                            .ok_or_else(|| {
                                RuntimePlanLowerError::new(format!(
                                    "project call {expression:?} has no checked attached default for {:?}",
                                    instance
                                ))
                            })?;
                        let captures = default
                            .captures()
                            .iter()
                            .map(|capture| match capture.source() {
                                RuntimeProjectFunctionParameterSource::ContinuationPrefix {
                                    position,
                                } => RuntimeProjectCallDefaultCaptureSource::ContinuationPrefix {
                                    position,
                                },
                                RuntimeProjectFunctionParameterSource::CurrentGroup {
                                    position,
                                } => RuntimeProjectCallDefaultCaptureSource::CurrentLogical {
                                    position,
                                },
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice();
                        RuntimeProjectCallAttachedPresenceSeed::DefaultedOmitted(
                            RuntimeProjectCallDefaultFunctionSeed {
                                site: default_site,
                                captures,
                            },
                        )
                    }
                };
                Ok(RuntimeProjectCallAttachedMaterializationSeed {
                    abi_ty: descriptor.abi_ty().identity(),
                    binding_ty: descriptor.binding_ty().identity(),
                    source_index,
                    presence,
                })
            })
            .transpose()?;
        let ordinary = checked
            .current_group_materialization()
            .iter()
            .map(|materialization| {
                let parameter = materialization.parameter();
                let abi_ty = materialization.abi_ty().identity();
                let binding_ty = materialization.binding_ty().identity();
                let indices = materialization.operand_indices().to_vec().into_boxed_slice();
                Ok(match materialization.kind() {
                    HirParameterKind::Fixed | HirParameterKind::ExtensionReceiver => {
                        RuntimeProjectCallOrdinaryMaterializationSeed::Fixed(
                            RuntimeProjectCallFixedMaterializationSeed {
                                parameter,
                                abi_ty,
                                binding_ty,
                                source_index: indices.first().copied().ok_or_else(|| {
                                    RuntimePlanLowerError::new(format!(
                                        "project call {expression:?} fixed parameter {parameter} has no source operand"
                                    ))
                                })?,
                            },
                        )
                    }
                    HirParameterKind::RestPositional => {
                        RuntimeProjectCallOrdinaryMaterializationSeed::Rest(
                            RuntimeProjectCallRestMaterializationSeed {
                                parameter,
                                abi_ty,
                                binding_ty,
                                source_indices: indices,
                            },
                        )
                    }
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?
            .into_boxed_slice();
        let outcome = match checked.outcome() {
            crate::semantic_facts::RuntimeProjectFunctionCallOutcome::Continue {
                abi,
                next_group,
            } => RuntimeProjectCallOutcomeSeed::Continue {
                result_abi: RuntimeProjectCallAbiSeed {
                    lineage: abi.lineage(),
                    function_type: abi.function_type().identity(),
                    prefix_types: abi
                        .prefix_types()
                        .iter()
                        .map(RuntimeNormalizedType::identity)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
                next_group: u32::try_from(next_group.get()).map_err(|_| {
                    RuntimePlanLowerError::new(format!(
                        "project call {expression:?} next group exceeds checked limits"
                    ))
                })?,
            },
            crate::semantic_facts::RuntimeProjectFunctionCallOutcome::Invoke { instance } => {
                RuntimeProjectCallOutcomeSeed::Invoke {
                    function_site: self
                        .project_function_sites
                        .get(instance)
                        .cloned()
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "project call {expression:?} has no reserved instance site for {:?}",
                                instance
                            ))
                        })?,
                }
            }
        };
        Ok(RuntimeProjectCallPlanSeed {
            input,
            completed_group: u32::try_from(call.completed_group().get()).map_err(|_| {
                RuntimePlanLowerError::new(format!(
                    "project call {expression:?} group exceeds checked limits"
                ))
            })?,
            operands: operands.into_boxed_slice(),
            ordinary,
            attached,
            outcome,
        })
    }

    fn lower_dialogue_value(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let semantic_expression = self.require_semantic_dialogue(expression)?;
        let application = self
            .semantic_facts
            .dialogue_application(semantic_expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "dialogue application {expression:?} has no checked projection fact"
                ))
            })?;
        let content = self
            .dialogue_content
            .get(&application.content().template_id())
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "dialogue application {expression:?} has no builder-issued content handle"
                ))
            })?;
        let (pattern, tail) = match continuation {
            RuntimeFlowValueContinuation::Bind { pattern, tail } => (pattern, tail),
            RuntimeFlowValueContinuation::Ignore(tail) => (
                RuntimePatternSeed::new(
                    application.line_result().identity(),
                    RuntimePatternSeedKind::Discard,
                ),
                tail,
            ),
            RuntimeFlowValueContinuation::Return
            | RuntimeFlowValueContinuation::Try { .. }
            | RuntimeFlowValueContinuation::WrapCarrier { .. }
            | RuntimeFlowValueContinuation::Compose { .. }
            | RuntimeFlowValueContinuation::Branch { .. }
            | RuntimeFlowValueContinuation::Pipe { .. } => {
                return Err(RuntimePlanLowerError::new(format!(
                    "dialogue application {expression:?} requires a direct result-pattern continuation"
                )));
            }
        };
        if pattern.ty() != application.line_result().identity() {
            return Err(RuntimePlanLowerError::new(format!(
                "dialogue application {expression:?} result pattern has the wrong accepted type"
            )));
        }
        let mut ops = vec![RuntimeFlowOpSeed::Dialogue {
            content,
            result: arcweft_core::plan::RuntimeDialogueResultTargetSeed::try_new(
                application.line_result().identity(),
                pattern,
            )
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "dialogue application {expression:?} result target rejected: {error}"
                ))
            })?,
        }];
        ops.extend(self.lower_flow_tail(tail)?);
        Ok(ops)
    }

    fn dialogue_from_source(&self, expression: ExprId) -> Option<ExprId> {
        self.facts
            .dialogue_lines()
            .and_then(|lines| lines.for_source_expr(expression))
            .map(|line| line.source().semantic_application())
    }

    fn synthetic_dialogue_from_source(&self, expression: ExprId) -> Option<ExprId> {
        self.dialogue_from_source(expression)
            .filter(|semantic| *semantic != expression)
    }

    fn require_semantic_dialogue(
        &self,
        expression: ExprId,
    ) -> Result<ExprId, RuntimePlanLowerError> {
        self.facts
            .dialogue_lines()
            .and_then(|lines| lines.for_semantic_expr(expression))
            .filter(|line| line.source().semantic_application() == expression)
            .map(|_| expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "dialogue semantic application {expression:?} has no accepted line"
                ))
            })
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
        let result = self.expression_type(owner)?;
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
        let fact = self.choice(owner).ok_or_else(|| {
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
            let label = match self.expression_literal(arm.label()) {
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
                let ty = self.expression_type(owner)?;
                (
                    RuntimePatternSeed::new(ty.identity(), RuntimePatternSeedKind::Discard),
                    tail,
                )
            }
            RuntimeFlowValueContinuation::Return
            | RuntimeFlowValueContinuation::Try { .. }
            | RuntimeFlowValueContinuation::WrapCarrier { .. }
            | RuntimeFlowValueContinuation::Compose { .. }
            | RuntimeFlowValueContinuation::Branch { .. }
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
        let fact = self.awaited(expression).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} has no checked runtime fact"
            ))
        })?;
        let locals = self
            .control
            .awaits
            .get(&expression)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "Await expression {expression:?} has no admitted continuation locals"
                ))
            })?;
        let await_op = self.lower_await_operation(expression, awaited, &fact, &locals)?;
        let payload = self.expression_type(expression)?.clone();
        let mut ops = vec![await_op];
        ops.extend(self.apply_value_continuation(
            local_seed(&payload, locals.payload),
            continuation.clone(),
        )?);
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
        let lowerer = self.expr_lowerer();
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
        let payload = self.expression_type(expression)?.clone();
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
                pattern: self
                    .pattern_lowerer()
                    .lower(checked.pattern())
                    .map_err(RuntimePlanLowerError::new)?,
                ops: self.lower_contextual_body(authored.body())?,
            });
        }
        Ok(RuntimeFlowOpSeed::Await {
            binding: Some(bind_seed(&payload, locals.payload.clone())),
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
                let boundary = self.expression_type(owner)?;
                let wrapped = normalized_variant_expression_seed(boundary, 0, Some(value))
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "carrier block {owner:?} success is invalid: {error}"
                        ))
                    })?;
                return self.apply_value_continuation(wrapped, *outer);
            }
            RuntimeFlowValueContinuation::Compose {
                owner,
                child,
                mut overrides,
                outer,
            } => {
                let ty = self.expression_type(child)?;
                let local = self
                    .control
                    .expression_values
                    .get(&child)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "evaluated expression {child:?} has no admitted value local"
                        ))
                    })?;
                let mut ops = if matches!(value.kind(), arcweft_core::plan::RuntimeExprSeedKind::Local(current) if current == &local)
                {
                    Vec::new()
                } else {
                    vec![RuntimeFlowOpSeed::Let {
                        pattern: bind_seed(ty, local.clone()),
                        expr: value,
                    }]
                };
                overrides.insert(child, local_seed(ty, local));
                ops.extend(self.lower_flow_value_with_overrides(owner, *outer, overrides)?);
                return Ok(ops);
            }
            RuntimeFlowValueContinuation::Branch {
                owner,
                overrides,
                outer,
            } => {
                return self.finish_value_branch(owner, value, *outer, overrides);
            }
            RuntimeFlowValueContinuation::Pipe {
                owner,
                right,
                mut overrides,
                outer,
            } => {
                let local = self.control.pipes.get(&owner).cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "once-only pipe {owner:?} has no admitted local"
                    ))
                })?;
                let pipe = self.pipe(owner).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "once-only pipe {owner:?} has no checked fact"
                    ))
                })?;
                let local_type = self.expression_type(pipe.left())?;
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
        let fact = self.tried(owner).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Try expression {owner:?} has no checked runtime fact"
            ))
        })?;
        let locals = self.control.tries.get(&owner).cloned().ok_or_else(|| {
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
                    normalized_variant_binding_pattern_seed(
                        fact.carrier_type(),
                        1,
                        Some(local.clone()),
                    )
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "Try expression {owner:?} residual pattern is invalid: {error}"
                        ))
                    })?,
                    Some(local_seed(residual, local)),
                )
            }
            RuntimeTryCarrierFact::Option { .. } => (
                normalized_variant_binding_pattern_seed(fact.carrier_type(), 1, None).map_err(
                    |error| {
                        RuntimePlanLowerError::new(format!(
                            "Try expression {owner:?} empty residual pattern is invalid: {error}"
                        ))
                    },
                )?,
                None,
            ),
        };
        let failure_ops = self.propagate_try_residual(&fact, failure_value)?;
        Ok(vec![RuntimeFlowOpSeed::Match {
            scrutinee: value,
            arms: vec![
                RuntimeFlowMatchArmSeed {
                    pattern: normalized_variant_binding_pattern_seed(
                        fact.carrier_type(),
                        0,
                        Some(locals.success),
                    )
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "Try expression {owner:?} success pattern is invalid: {error}"
                        ))
                    })?,
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
        let propagated = normalized_variant_expression_seed(fact.boundary_type(), 1, residual)
            .map_err(|error| {
                RuntimePlanLowerError::new(format!("Try residual is invalid: {error}"))
            })?;
        match fact.boundary() {
            RuntimeTryBoundaryOwner::Infallible => Ok(Vec::new()),
            RuntimeTryBoundaryOwner::Callable(_) => {
                Ok(vec![RuntimeFlowOpSeed::ReturnExpr(propagated)])
            }
            RuntimeTryBoundaryOwner::ExplicitFunctionSite(boundary)
            | RuntimeTryBoundaryOwner::ImplicitFunctionSite(boundary) => {
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
        let lowerer = self.expr_lowerer();
        lowerer
            .lower_host_call_target(call_id, call)
            .map(|target| target.map(|target| RuntimeFlowOpSeed::HostCall { binding, target }))
            .map_err(RuntimePlanLowerError::new)
    }

    fn lower_host_call_value(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve host-call value expression {expression:?}: {error}"
            ))
        })?;
        let HirExprKind::Call(call) = resolved.kind() else {
            return Err(RuntimePlanLowerError::new(format!(
                "checked host-call value {expression:?} is not a Call expression"
            )));
        };
        let target = self
            .expr_lowerer()
            .with_overrides(overrides)
            .lower_host_call_target(expression, call)
            .map_err(RuntimePlanLowerError::new)?
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "checked host-call value {expression:?} has no host target"
                ))
            })?;
        let result = self.expression_type(expression)?;
        if matches!(result.shape(), RuntimeTypeShape::Never) {
            return Ok(vec![RuntimeFlowOpSeed::HostCall {
                binding: None,
                target,
            }]);
        }
        let local = self
            .control
            .expression_values
            .get(&expression)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "host-call value {expression:?} has no admitted result local"
                ))
            })?;
        let mut ops = vec![RuntimeFlowOpSeed::HostCall {
            binding: Some(bind_seed(result, local.clone())),
            target,
        }];
        ops.extend(self.apply_value_continuation(local_seed(result, local), continuation)?);
        Ok(ops)
    }

    fn host_call_operand(
        &self,
        expression: arcweft_lang_hir::identity::ExprId,
    ) -> Result<
        Option<(
            arcweft_lang_hir::identity::ExprId,
            &arcweft_lang_hir::expr::HirCallInvocation,
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
        let admission = self.assertion(statement).ok_or_else(|| {
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
                RuntimeAssertionOwner::Closure(closure) => {
                    crate::assertion_lower::derive_runtime_closure_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        closure,
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
                RuntimeAssertionOwner::Line(line) => {
                    crate::assertion_lower::derive_runtime_line_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        line,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
            };
            let condition_expr = self
                .expr_lowerer()
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
    let lower = |operand: &RuntimeEvaluatedEffectOperandFact| {
        expr.lower_scalar_operand_source(operand.source(), operand.ty())
            .map_err(RuntimePlanLowerError::new)
    };
    let fields = |fields: &[RuntimeEffectFieldFact]| {
        fields
            .iter()
            .map(|field| {
                Ok(RuntimeEffectFieldSeed {
                    name: field.name().to_owned(),
                    value: lower(field.operand())?,
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
            message: lower(message)?,
            fields: fields(effect_fields)?,
        },
        RuntimeEvaluatedEffect::SignalWrite { target, value } => {
            RuntimeEvaluatedEffectSeed::SignalWrite {
                target: lower(target)?,
                value: lower(value)?,
            }
        }
        RuntimeEvaluatedEffect::MetricWrite { target, value } => {
            RuntimeEvaluatedEffectSeed::MetricWrite {
                target: lower(target)?,
                value: lower(value)?,
            }
        }
        RuntimeEvaluatedEffect::EmitEvent {
            event,
            fields: effect_fields,
        } => RuntimeEvaluatedEffectSeed::EmitEvent {
            event: lower(event)?,
            fields: fields(effect_fields)?,
        },
        RuntimeEvaluatedEffect::Panic { message } => {
            RuntimeEvaluatedEffectSeed::Panic(lower(message)?)
        }
        RuntimeEvaluatedEffect::Fail { message } => {
            RuntimeEvaluatedEffectSeed::Fail(lower(message)?)
        }
        RuntimeEvaluatedEffect::Bail { message } => {
            RuntimeEvaluatedEffectSeed::Bail(lower(message)?)
        }
        RuntimeEvaluatedEffect::Ensure { condition, message } => {
            RuntimeEvaluatedEffectSeed::Ensure {
                condition: lower(condition)?,
                message: lower(message)?,
            }
        }
        RuntimeEvaluatedEffect::Drop { target, policy } => RuntimeEvaluatedEffectSeed::Drop {
            target: lower(target)?,
            policy: match policy {
                RuntimeDropPolicyFact::Default => RuntimeDropPolicySeed::Default,
                RuntimeDropPolicyFact::Cancel => RuntimeDropPolicySeed::Cancel,
                RuntimeDropPolicyFact::Stop { fade } => RuntimeDropPolicySeed::Stop {
                    fade: match fade {
                        RuntimeDropFadeFact::Constant(value) => RuntimeExprSeed::new(
                            arcweft_core::pattern::RuntimeCheckedType::Duration
                                .semantic_identity_digest(),
                            arcweft_core::plan::RuntimeExprSeedKind::Value(RuntimeValue::Duration(
                                *value,
                            )),
                        ),
                        RuntimeDropFadeFact::Operand(operand) => lower(operand)?,
                    },
                },
                RuntimeDropPolicyFact::Finish => RuntimeDropPolicySeed::Finish,
                RuntimeDropPolicyFact::Release => RuntimeDropPolicySeed::Release,
                RuntimeDropPolicyFact::Detach => RuntimeDropPolicySeed::Detach,
            },
        },
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
        entry::{
            EntryBindingIdentity, FlowContractHash, RuntimeEntryRoles, RuntimeFlowExecutable,
            RuntimeFlowSchema,
        },
        plan::{
            EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec,
            RuntimeEntryTarget,
        },
    };
    use arcweft_lang_hir::database::HirDatabase;
    use arcweft_lang_hir::expr::HirExprKind;
    use arcweft_lang_hir::item::HirItemKind;
    use arcweft_lang_hir::lowering::{HirModuleKey, LoweringRequest};
    use arcweft_lang_hir::project::{
        HirProject, HirProjectBuilder, HirProjectModule, HirRuntimeCallCalleeDisposition,
        HirRuntimeEmissionMode, HirRuntimeExecutableOwner, HirRuntimeExpressionProjection,
        HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind, HirRuntimeSemanticReachability,
        HirRuntimeSemanticReachabilityInput, HirRuntimeValueRetention,
    };
    use arcweft_lang_hir::proof_return::HirProofReturnSemanticFactSet;
    use arcweft_lang_hir::symbol::{
        CallablePackageId, ProjectExternalDeclarations, ProjectSymbolRevision, ProjectSymbolTable,
        ProjectSymbolWorldId,
    };
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
    use arcweft_lang_syntax::incremental::SyntaxDatabase;
    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        RuntimeCheckedEntryInput, RuntimeEntryFlowInput, RuntimeEntryLoweringInput,
        lower_runtime_plan_with_stats,
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
        let mut input = complete_type_input(&project);
        input.push_flow(owner, identity.clone());
        let facts = runtime_facts(&project, input).expect("checked facts");
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
        let mut input = complete_type_input(&project);
        input.push_flow(owner, identity);
        let facts = runtime_facts(&project, input).expect("checked facts");
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
        let mut fact_input = complete_type_input(&project);
        fact_input.push_flow(flow_owner, flow.clone());
        let facts = runtime_facts(&project, fact_input).expect("checked facts");

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
            target: RuntimeEntryTarget::Flow(flow.clone()),
            roles: RuntimeEntryRoles::None,
        };
        let runtime_flow = RuntimeEntryFlowInput::new(
            flow_owner,
            RuntimeFlowExecutable {
                flow: flow.clone(),
                contract: FlowContractHash::from_bytes([8; 32]),
                controller: None,
            },
            RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            },
        );
        let input = RuntimeEntryLoweringInput::new(
            executable,
            vec![RuntimeCheckedEntryInput::new(entry_owner, runtime_entry)],
            Vec::new(),
            vec![runtime_flow],
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

    fn runtime_reachability(project: &HirProject) -> HirRuntimeSemanticReachability<'_> {
        let executable = project.executable_view().expect("executable fixture");
        let (_, module) = executable.modules().next().expect("fixture module");
        let world = ProjectSymbolWorldId::try_new(
            executable.package().clone(),
            module.provenance().source_identity().id().clone(),
            "runtime-plan-final-flow-test",
        )
        .expect("fixture reachability world");
        let revision = ProjectSymbolRevision::try_for_documents(
            executable
                .modules()
                .map(|(_, module)| module.provenance().source_identity()),
        )
        .expect("fixture reachability revision");
        let externals = ProjectExternalDeclarations::try_new(world.clone(), revision, Vec::new())
            .expect("fixture external declarations");
        let symbols = ProjectSymbolTable::link(project.view(), &externals)
            .expect("fixture symbols")
            .into_table();
        let topology = executable
            .accept_symbol_generation(&symbols)
            .expect("accepted fixture symbol generation")
            .into_evaluation_topology()
            .expect("fixture evaluation topology");
        let roots = executable
            .items()
            .filter_map(|item| {
                let kind = match item.item().kind() {
                    HirItemKind::Flow(_) => HirRuntimeReachabilityRootKind::CheckedFlow,
                    HirItemKind::Entry(_) => HirRuntimeReachabilityRootKind::CheckedEntry,
                    _ => return None,
                };
                Some(HirRuntimeReachabilityRoot::new(
                    kind,
                    HirRuntimeExecutableOwner::Item(item.id()),
                ))
            })
            .collect();
        let input = HirRuntimeSemanticReachabilityInput::try_new(
            HirRuntimeEmissionMode::CheckAll,
            world,
            revision,
            roots,
            Vec::new(),
        )
        .expect("fixture reachability input");
        executable
            .runtime_semantic_reachability(
                input,
                &topology,
                |_| None,
                |owner| retained_runtime_projection(executable, owner),
            )
            .expect("fixture reachability")
    }

    fn retained_runtime_projection(
        executable: arcweft_lang_hir::project::HirExecutableProjectView<'_>,
        owner: arcweft_lang_hir::identity::ExprId,
    ) -> Option<HirRuntimeExpressionProjection> {
        executable.modules().find_map(|(_, module)| {
            let expression = module.resolve_expr(owner).ok()?;
            Some(match expression.kind() {
                HirExprKind::Call(call) => HirRuntimeExpressionProjection::Call {
                    result: HirRuntimeValueRetention::Retain,
                    callee: if call.callee().value_expression().is_some() {
                        HirRuntimeCallCalleeDisposition::RuntimeReceiver
                    } else {
                        HirRuntimeCallCalleeDisposition::Static
                    },
                },
                HirExprKind::AttachedContentApplication(_) => {
                    HirRuntimeExpressionProjection::Structural {
                        value: HirRuntimeValueRetention::Omit,
                    }
                }
                _ => HirRuntimeExpressionProjection::Structural {
                    value: HirRuntimeValueRetention::Retain,
                },
            })
        })
    }

    fn runtime_facts(
        project: &HirProject,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<RuntimePlanSemanticFacts, crate::semantic_facts::RuntimeSemanticFactsError> {
        let executable = project.executable_view().expect("executable fixture");
        let reachability = runtime_reachability(project);
        RuntimePlanSemanticFacts::try_new(executable, &reachability, input)
    }

    fn complete_type_input(project: &HirProject) -> RuntimePlanSemanticFactInput {
        let mut input = RuntimePlanSemanticFactInput::new();
        let runtime_owners = runtime_reachability(project);
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
            .selected_expression_type_owners()
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
