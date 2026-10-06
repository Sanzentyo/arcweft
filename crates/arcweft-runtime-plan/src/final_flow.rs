//! Runtime-plan lowering from one accepted final-HIR project generation.

#[path = "final_flow/callable_states.rs"]
mod callable_states;

#[path = "final_flow/control_locals.rs"]
mod control_locals;
#[path = "final_flow/defer.rs"]
mod defer;
#[path = "final_flow/format_attempt.rs"]
mod format_attempt;
#[path = "final_flow/line_plan.rs"]
mod line_plan;
#[path = "final_flow/rust_defaults.rs"]
mod rust_defaults;
#[path = "final_flow/scopes.rs"]
mod scopes;
#[path = "final_flow/trait_method.rs"]
mod trait_method;
#[path = "final_flow/value_branches.rs"]
mod value_branches;

use control_locals::ControlLocals;
use trait_method::{define_trait_methods, reserve_trait_methods};

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
    RuntimeCallableParameterSeed, RuntimeChoiceOptionSeed, RuntimeDeferOwner,
    RuntimeDialogueContentPlanSeedId, RuntimeDropPolicySeed, RuntimeEffectFieldSeed,
    RuntimeEffectSet, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEvaluatedEffectSeed,
    RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFlowMatchArmSeed,
    RuntimeFlowMatchGuardSeed, RuntimeFlowOpSeed, RuntimeFlowSeed, RuntimeFormatAttemptSeedId,
    RuntimeFunctionInputBindingSeed, RuntimeFunctionInputOwnershipRequirement,
    RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
    RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed, RuntimeLineId,
    RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed, RuntimeLocalSeedId, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlan, RuntimePlanBuilder,
    RuntimeProjectCallAttachedMaterializationSeed, RuntimeProjectCallAttachedPresenceSeed,
    RuntimeProjectCallFixedMaterializationSeed, RuntimeProjectCallOperandSeed,
    RuntimeProjectCallOrdinaryMaterializationSeed, RuntimeProjectCallPlanSeed,
    RuntimeProjectCallRestMaterializationSeed, RuntimePureInputType, RuntimePureOutputType,
    RuntimePureProgramBindingSeed, RuntimeReceiverMode, RuntimeTraitMethodDeclarationSeed,
    RuntimeTraitMethodIdentity, RuntimeTraitMethodSeedId,
};
use arcweft_core::runtime_id::RuntimeDeferSiteId;
use arcweft_core::value::{
    RuntimeCallArgumentMode, RuntimeFmtParameterId, RuntimeIntrinsic, RuntimeLocalReadMode,
    RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
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
use arcweft_lang_hir::project::{HirAnalysisProjectView, HirRuntimeExecutableOwner};
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
use arcweft_lang_sema::final_analysis::CheckedLocalReadMode;
use arcweft_source::SourceSpan;
use arcweft_text_model::{DialogueContentCatalog, DialogueContentFragmentTemplate};

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
    RuntimeAssertionAdmission, RuntimeAwaitFact, RuntimeCallResultShape,
    RuntimeClosureInstanceFact, RuntimeClosureInstanceKey, RuntimeDialogueApplication,
    RuntimeDialogueApplicationTarget, RuntimeDialogueEffectCaptureKey,
    RuntimeDialogueEffectOperationFact, RuntimeDialogueEffectProgramKey,
    RuntimeDialogueValueCaptureKey, RuntimeDropFadeFact, RuntimeDropPolicyFact,
    RuntimeEffectFieldFact, RuntimeEvaluatedEffect, RuntimeEvaluatedEffectFact,
    RuntimeEvaluatedEffectOperandFact, RuntimeExecutableSemanticScope,
    RuntimeImplicitCallableSiteKey, RuntimeIteratorFact, RuntimeIteratorWitnessExecutableFact,
    RuntimeLineCallable, RuntimeNormalizedType, RuntimePlanSemanticFacts, RuntimeProjectCallable,
    RuntimeProjectFunctionExpressionPayload, RuntimeProjectFunctionInstanceFact,
    RuntimeProjectFunctionInstanceKey, RuntimeProjectFunctionInstanceSemanticFacts,
    RuntimeProjectFunctionParameterSource, RuntimeProjectFunctionTypeOwner,
    RuntimeProjectFunctionTypeProjection, RuntimeResolvedAttachedContent, RuntimeResolvedCall,
    RuntimeResolvedCallDispatch, RuntimeResolvedCallOperandOrigin,
    RuntimeResolvedCallOperandProjection, RuntimeResolvedCallOperandSource,
    RuntimeResolvedStaticCallTarget, RuntimeResolvedValue, RuntimeScopeContinuation,
    RuntimeScopeFact, RuntimeScopeOwner, RuntimeScopedExecutableSemanticFactView,
    RuntimeSemanticFactsError, RuntimeTraitIdentity, RuntimeTraitMethodFact,
    RuntimeTraitMethodInstanceKey, RuntimeTryBoundaryOwner, RuntimeTryCarrierFact, RuntimeTryFact,
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
        project: HirAnalysisProjectView<'_>,
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

    pub fn empty(project: HirAnalysisProjectView<'_>) -> Self {
        Self::new(project, Vec::new(), Vec::new(), Vec::new())
    }

    fn validate_generation(&self, project: HirAnalysisProjectView<'_>) -> bool {
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
    pub character_dialogue_generation: Option<
        Arc<arcweft_dialogue::CharacterDialogueGenerationDeclaration<RuntimeSemanticTypeId>>,
    >,
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
struct ReservedFunctionSiteDefinition<'facts> {
    effects: RuntimeEffectSet,
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    owner: ExprId,
    module: HirModuleId,
    body: ExprId,
    site: RuntimeFunctionSiteSeedId,
    implicit_parameter: RuntimeLocalSeedId,
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

impl ProjectFunctionFrameLocals {
    fn admit_catalog(
        semantics: &RuntimeProjectFunctionInstanceSemanticFacts,
        program: &crate::semantic_facts::RuntimePureProgramFact,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Self, RuntimePlanLowerError> {
        let context = program.function_type().identity();
        let declaration = |ty: &crate::semantic_facts::RuntimeNormalizedType| {
            if ty.scope().is_root() {
                RuntimeLocalDeclarationSeed::new(ty.identity())
            } else {
                RuntimeLocalDeclarationSeed::in_function(ty.identity(), context)
            }
        };
        let mut rows = semantics
            .type_projection()
            .iter()
            .filter_map(|projection| match projection {
                RuntimeProjectFunctionTypeProjection::Local {
                    owner: local, ty, ..
                } => Some((ProjectFunctionFrameLocal::Hir(*local), declaration(ty))),
                _ => None,
            })
            .collect::<Vec<_>>();
        for expression in semantics.expressions() {
            let Some(call) = semantics.call(expression.owner()) else {
                continue;
            };
            if !call.requires_specialized_operand_anf() {
                continue;
            }
            for (position, operand) in call.operands().iter().enumerate() {
                let source_index = u32::try_from(position).map_err(|_| {
                    RuntimePlanLowerError::new(
                        "program source operand coordinate exceeds checked limits",
                    )
                })?;
                rows.push((
                    ProjectFunctionFrameLocal::SpecializedOperand {
                        owner: expression.owner(),
                        source_index,
                    },
                    declaration(operand.ty()),
                ));
            }
        }
        let admitted = builder
            .admit_type_batch([], rows.iter().map(|(_, declaration)| *declaration))
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let mut frame = Self::default();
        for ((owner, _), local) in rows.into_iter().zip(admitted.local_ids()) {
            match owner {
                ProjectFunctionFrameLocal::Hir(owner) => {
                    frame.hir.insert(owner, local.clone());
                }
                ProjectFunctionFrameLocal::SpecializedOperand {
                    owner,
                    source_index,
                } => {
                    frame
                        .specialized_operands
                        .insert((owner, source_index), local.clone());
                }
                ProjectFunctionFrameLocal::ParameterInput { .. }
                | ProjectFunctionFrameLocal::AttachedAbi => {
                    return Err(RuntimePlanLowerError::new(
                        "program catalog contains a declaration parameter input",
                    ));
                }
            }
        }
        frame.control = ControlLocals::admit(
            crate::semantic_facts::RuntimeExecutableSemanticFactView::project_instance(semantics),
            Some(program.function_type()),
            builder,
        )?;
        Ok(frame)
    }
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
    Program(arcweft_id::runtime_program::RuntimePureProgramId),
    ProjectFunction(RuntimeProjectFunctionInstanceKey),
    TraitMethod(RuntimeTraitMethodInstanceKey),
    Closure(RuntimeClosureInstanceKey),
}

#[derive(Clone)]
struct ReservedPureProgramDefinition<'facts> {
    program: &'facts crate::semantic_facts::RuntimePureProgramFact,
    scope: RuntimeScopedExecutableSemanticFactView<'facts>,
    parameter_inputs: Box<[RuntimeLocalSeedId]>,
    site: RuntimeFunctionSiteSeedId,
    body_kind: RuntimeFunctionSiteBodyKind,
    iteration_output: Option<RuntimeLocalSeedId>,
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
    effects: RuntimeEffectSet,
    operation: RuntimeDialogueEffectOperationFact,
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
        RuntimeSemanticTypeId,
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
    project: HirAnalysisProjectView<'project>,
    facts: &'data RuntimePlanSemanticFacts,
    locals: &'data BTreeMap<LocalId, RuntimeLocalSeedId>,
    project_callable_states: &'data callable_states::ProjectCallableStates,
    callable_sources: &'data callable_states::ProjectCallableSourceStates,
    callable_specializations: &'data callable_states::CallableSpecializationSeeds,
    callable_applications: &'data callable_states::ProjectCallableApplicationStates,
    callable_specialization_targets: &'data callable_states::CallableSpecializationTargetStates,
    project_function_sites:
        &'data BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeFunctionSiteSeedId>,
    project_function_locals:
        &'data BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    program_locals: &'data BTreeMap<
        arcweft_id::runtime_program::RuntimePureProgramId,
        ProjectFunctionFrameLocals,
    >,
    closure_sites: &'data BTreeMap<RuntimeClosureInstanceKey, RuntimeFunctionSiteSeedId>,
    closure_locals: &'data BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    trait_methods: &'data BTreeMap<RuntimeTraitMethodInstanceKey, RuntimeTraitMethodSeedId>,
    format_attempts: &'data BTreeMap<
        crate::semantic_facts::RuntimeFormatTemplateKey,
        RuntimeFormatAttemptSeedId,
    >,
    trait_method_locals: &'data BTreeMap<RuntimeTraitMethodInstanceKey, ProjectFunctionFrameLocals>,
    function_sites: &'data BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeFunctionSiteSeedId>,
    defer_sites: &'data BTreeMap<StmtId, RuntimeDeferSiteId>,
    dialogue_effect_sites:
        &'data BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    dialogue_value_capture_input_locals:
        &'data BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    dialogue_value_result_locals:
        &'data BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    dialogue_value_project_source_locals:
        &'data BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    format_operand_source_locals: &'data BTreeMap<
        (
            crate::semantic_facts::RuntimeFormatTemplateKey,
            RuntimeFmtParameterId,
        ),
        RuntimeLocalSeedId,
    >,
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

#[derive(Clone)]
pub(crate) struct ScopeLocalSeeds {
    pub(crate) carrier: RuntimeLocalSeedId,
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
        .with_scope_locals(&self.control.scopes)
        .with_specialized_operand_locals(self.specialized_operand_locals)
        .with_closure_sites(self.closure_sites)
        .with_format_attempts(self.format_attempts)
        .with_project_callable_states(self.project_callable_states)
        .with_callable_sources(self.callable_sources)
        .with_callable_specializations(self.callable_specializations)
    }

    fn scoped_expr_lowerer<'a>(
        &'a self,
        module: &'a HirModule,
        scope: RuntimeScopedExecutableSemanticFactView<'a>,
    ) -> Result<FinalExprLowerer<'a>, RuntimePlanLowerError> {
        let control = self.executable_control_locals(scope.scope())?;
        Ok(self
            .expr_lowerer(module)
            .with_locals(self.executable_locals(scope.scope())?)
            .with_control_locals(&control.pipes, &control.tries)
            .with_scope_locals(&control.scopes)
            .with_specialized_operand_locals(
                self.executable_specialized_operand_locals(scope.scope())?,
            )
            .with_scoped_semantics(scope))
    }

    fn executable_control_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&ControlLocals, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.control),
            RuntimeExecutableSemanticScope::Program(program) => self
                .program_locals
                .get(&program)
                .map(|frame| &frame.control)
                .ok_or_else(|| RuntimePlanLowerError::new("program has no control local frame")),
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
            RuntimeExecutableSemanticScope::TraitMethod(key) => self
                .trait_method_locals
                .get(key)
                .map(|frame| &frame.control)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("dialogue trait-method frame has no control locals")
                }),
        }
    }

    fn executable_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&BTreeMap<LocalId, RuntimeLocalSeedId>, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.locals),
            RuntimeExecutableSemanticScope::Program(program) => self
                .program_locals
                .get(&program)
                .map(|frame| &frame.hir)
                .ok_or_else(|| RuntimePlanLowerError::new("program has no admitted local frame")),
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
            RuntimeExecutableSemanticScope::TraitMethod(key) => self
                .trait_method_locals
                .get(key)
                .map(|frame| &frame.hir)
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue trait-method scope {key:?} has no admitted local frame"
                    ))
                }),
        }
    }

    fn executable_specialized_operand_locals(
        &self,
        scope: RuntimeExecutableSemanticScope<'_>,
    ) -> Result<&BTreeMap<(ExprId, u32), RuntimeLocalSeedId>, RuntimePlanLowerError> {
        match scope {
            RuntimeExecutableSemanticScope::Global => Ok(self.specialized_operand_locals),
            RuntimeExecutableSemanticScope::Program(program) => self.program_locals.get(&program)
                .map(|frame| &frame.specialized_operands)
                .ok_or_else(|| RuntimePlanLowerError::new("program has no specialized operand frame")),
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
            RuntimeExecutableSemanticScope::TraitMethod(key) => self
                .trait_method_locals
                .get(key)
                .map(|frame| &frame.specialized_operands)
                .ok_or_else(|| RuntimePlanLowerError::new(format!("dialogue trait-method scope {key:?} has no admitted specialized operand frame"))),
        }
    }
}

/// Lowers one exact accepted HIR generation and its checked semantic facts.
#[allow(
    clippy::too_many_lines,
    reason = "this function is the single transactional authority switch that validates and publishes one complete runtime plan"
)]
pub fn lower_runtime_plan_with_stats(
    project: HirAnalysisProjectView<'_>,
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
        .map(|(local, ty)| match facts.local_context(*local) {
            Some(context) => {
                RuntimeLocalDeclarationSeed::in_function(ty.identity(), context.identity())
            }
            None => RuntimeLocalDeclarationSeed::new(ty.identity()),
        })
        .collect::<Vec<_>>();
    let mut implicit_callable_facts = Vec::new();
    facts.visit_scoped_implicit_callables(&mut |scope, owner, callable| {
        implicit_callable_facts.push((
            RuntimeImplicitCallableSiteKey::new(scope.scope(), owner),
            scope,
            owner,
            callable,
        ));
    });
    let mut seen_implicit_sites = BTreeSet::new();
    if let Some((key, _, _, _)) = implicit_callable_facts
        .iter()
        .find(|(key, _, _, _)| !seen_implicit_sites.insert(key.clone()))
    {
        return Err(vec![RuntimePlanLowerError::new(format!(
            "implicit callable site {key:?} was selected more than once"
        ))]);
    }
    local_seeds.extend(implicit_callable_facts.iter().map(|(_, _, _, callable)| {
        RuntimeLocalDeclarationSeed::new(callable.parameter().identity())
    }));
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
            let RuntimeTypeShape::Function {
                result, parameters, ..
            } = instance.function_type().shape()
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
    let mut project_instance_expression_owners = facts
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
    project_instance_expression_owners.extend(
        facts
            .trait_methods()
            .filter_map(RuntimeTraitMethodFact::closed_semantics)
            .flat_map(|semantics| semantics.expressions().iter().map(|row| row.owner())),
    );
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
                    RuntimeProjectFunctionTypeProjection::Local {
                        owner: local,
                        ty, ..
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
    let trait_method_local_specs = facts
        .trait_methods()
        .filter_map(|method| method.closed_semantics().map(|semantics| (method.key(), semantics)))
        .map(|(key, semantics)| -> Result<_, RuntimePlanLowerError> {
            let mut rows = semantics.type_projection().iter().filter_map(|projection| {
                match projection {
                    RuntimeProjectFunctionTypeProjection::Local {
                        owner: local, ty, ..
                    } => Some((ProjectFunctionFrameLocal::Hir(*local), ty.identity())),
                    _ => None,
                }
            }).collect::<Vec<_>>();
            for expression in semantics.expressions() {
                let Some(call) = semantics.call(expression.owner()) else { continue };
                if !call.requires_specialized_operand_anf() { continue; }
                if call.attached_content().is_some() {
                    return Err(RuntimePlanLowerError::new(format!(
                        "trait-method instance {key:?} specialized call {:?} carries attached content",
                        expression.owner(),
                    )));
                }
                for (index, operand) in call.operands().iter().enumerate() {
                    let source_index = u32::try_from(index).map_err(|_| RuntimePlanLowerError::new(
                        "trait-method specialized source operand index exceeds checked limits"
                    ))?;
                    rows.push((
                        ProjectFunctionFrameLocal::SpecializedOperand { owner: expression.owner(), source_index },
                        operand.ty().identity(),
                    ));
                }
            }
            Ok((key.clone(), rows))
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
    let (closure_instances, closure_parents, line_schedule_callbacks) =
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
                    RuntimeProjectFunctionTypeProjection::Local {
                        owner: local,
                        ty, ..
                    } if !captured.contains(local) => Some((
                        ClosureFrameLocal::Hir(*local),
                        ty.identity(),
                    )),
                    RuntimeProjectFunctionTypeProjection::Local { .. }
                    | RuntimeProjectFunctionTypeProjection::Value { .. }
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
        .flat_map(|(key, scope, owner, callable)| {
            callable.captures().iter().map(move |capture| {
                let position = capture.position();
                let ty = scope.local_type(capture.local()).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "implicit callable {owner:?} capture {capture:?} has no accepted type"
                    ))
                })?;
                Ok(((key.clone(), position), ty.identity()))
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
    for (_, rows) in &trait_method_local_specs {
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
    let nominal_schema = facts
        .runtime_plan_nominal_schema()
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let admission = builder
        .admit_semantic_batch(
            type_seeds,
            local_seeds,
            facts.runtime_plan_nominal_record_domain_seeds(),
            facts.runtime_plan_variant_domain_seeds(),
            &nominal_schema,
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
        .map(|(key, _, _, _)| key.clone())
        .map(|key| {
            admitted_locals
                .next()
                .map(|local| (key, local))
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
            None,
            &mut builder,
        )
        .map_err(|error| vec![error])?;
        project_instance_locals.insert(key.clone(), frame);
    }
    let mut trait_method_locals = BTreeMap::new();
    for (key, rows) in &trait_method_local_specs {
        let mut frame = ProjectFunctionFrameLocals::default();
        for (owner, _) in rows {
            let seed = admitted_locals.next().ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "admitted trait-method instance local is missing",
                )]
            })?;
            match owner {
                ProjectFunctionFrameLocal::Hir(local) => {
                    if frame.hir.insert(*local, seed).is_some() {
                        return Err(vec![RuntimePlanLowerError::new(
                            "trait-method instance repeats a HIR local",
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
                            "trait-method instance repeats a specialized operand local",
                        )]);
                    }
                }
                ProjectFunctionFrameLocal::ParameterInput { .. }
                | ProjectFunctionFrameLocal::AttachedAbi => {
                    return Err(vec![RuntimePlanLowerError::new(
                        "trait-method instance has an ordinary-function local role",
                    )]);
                }
            }
        }
        let semantics = facts
            .trait_methods()
            .find(|method| method.key() == key)
            .and_then(RuntimeTraitMethodFact::closed_semantics)
            .ok_or_else(|| {
                vec![RuntimePlanLowerError::new(
                    "trait-method frame has no closed semantic facts",
                )]
            })?;
        frame.control = ControlLocals::admit(
            crate::semantic_facts::RuntimeExecutableSemanticFactView::project_instance(semantics),
            None,
            &mut builder,
        )
        .map_err(|error| vec![error])?;
        if trait_method_locals.insert(key.clone(), frame).is_some() {
            return Err(vec![RuntimePlanLowerError::new(
                "trait-method instance repeats its local frame",
            )]);
        }
    }
    let program_locals = facts
        .pure_program_semantics()
        .map(|(program, semantics)| {
            let contract = facts
                .pure_programs()
                .find_map(|(_, fact)| (fact.program() == *program).then_some(fact))
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("program frame has no function contract")
                })?;
            Ok((
                *program,
                ProjectFunctionFrameLocals::admit_catalog(semantics, contract, &mut builder)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimePlanLowerError>>()
        .map_err(|error| vec![error])?;
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
                Some(ClosureLexicalParent::Program(program)) => {
                    program_locals.get(program).map(|frame| &frame.hir)
                }
                Some(ClosureLexicalParent::ProjectFunction(parent)) => {
                    project_instance_locals.get(parent).map(|frame| &frame.hir)
                }
                Some(ClosureLexicalParent::TraitMethod(parent)) => {
                    trait_method_locals.get(parent).map(|frame| &frame.hir)
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
            None,
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
    for ((key, position), _) in &implicit_capture_input_local_specs {
        let seed = admitted_locals.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted implicit capture input local is missing",
            )]
        })?;
        if implicit_capture_input_locals
            .insert((key.clone(), *position), seed)
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
        &project_instance_locals,
        &closure_locals,
        &trait_method_locals,
        &program_locals,
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
        &line_schedule_callbacks,
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
    if !errors.is_empty() {
        return Err(errors);
    }
    let (
        project_callable_states,
        callable_sources,
        callable_specializations,
        callable_applications,
        callable_specialization_targets,
    ) = callable_states::materialize(
        facts,
        &project_function_sites,
        &project_default_function_sites,
        &mut builder,
        &mut errors,
    );
    rust_defaults::lower(facts, &mut builder, &mut errors);
    let (trait_methods, trait_definitions) = reserve_trait_methods(
        project,
        facts,
        &locals,
        &trait_method_locals,
        &mut builder,
        &mut errors,
    );
    let empty_dialogue_effect_sites = BTreeMap::new();
    let empty_defer_sites = BTreeMap::new();
    let empty_dialogue_value_capture_input_locals = BTreeMap::new();
    let empty_dialogue_value_result_locals = BTreeMap::new();
    let empty_dialogue_value_project_source_locals = BTreeMap::new();
    let empty_format_attempts = BTreeMap::new();
    let empty_format_operand_source_locals = BTreeMap::new();
    let empty_dialogue_effect_capture_input_locals = BTreeMap::new();
    let empty_dialogue_content = BTreeMap::new();
    let control_locals = ControlLocals::admit(
        crate::semantic_facts::RuntimeExecutableSemanticFactView::global(facts),
        None,
        &mut builder,
    )
    .map_err(|error| vec![error])?;
    let context = FinalLoweringContext {
        project,
        facts,
        locals: &locals,
        project_callable_states: &project_callable_states,
        callable_sources: &callable_sources,
        callable_specializations: &callable_specializations,
        callable_applications: &callable_applications,
        callable_specialization_targets: &callable_specialization_targets,
        project_function_sites: &project_function_sites,
        project_function_locals: &project_instance_locals,
        program_locals: &program_locals,
        closure_sites: &closure_sites,
        closure_locals: &closure_locals,
        trait_methods: &trait_methods,
        format_attempts: &empty_format_attempts,
        trait_method_locals: &trait_method_locals,
        function_sites: &function_sites,
        defer_sites: &empty_defer_sites,
        dialogue_effect_sites: &empty_dialogue_effect_sites,
        dialogue_value_capture_input_locals: &empty_dialogue_value_capture_input_locals,
        dialogue_value_result_locals: &empty_dialogue_value_result_locals,
        dialogue_value_project_source_locals: &empty_dialogue_value_project_source_locals,
        format_operand_source_locals: &empty_format_operand_source_locals,
        dialogue_effect_capture_input_locals: &empty_dialogue_effect_capture_input_locals,
        dialogue_content: &empty_dialogue_content,
        control: &control_locals,
        controller_result_locals: &controller_result_locals,
        specialized_operand_locals: &specialized_operand_locals,
    };

    let pure_program_definitions = reserve_pure_programs(&context, &mut builder, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

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
        .admit_type_batch(
            [],
            dialogue_effect_capture_specs
                .iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
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
    let mut dialogue_effect_assertion_sites = Vec::new();
    define_dialogue_effect_sites(
        &context,
        effect_definitions,
        &mut builder,
        &mut errors,
        &mut dialogue_effect_assertion_sites,
    );
    // Dialogue value sites capture caller-computed slot results. Admit a typed
    // callback input and caller result local per slot before reserving their
    // identity callbacks.
    let dialogue_value_capture_specs = match collect_dialogue_value_capture_specs(&context) {
        Ok(specs) => specs,
        Err(mut capture_errors) => {
            errors.append(&mut capture_errors);
            Vec::new()
        }
    };
    let value_admission = builder
        .admit_type_batch(
            [],
            dialogue_value_capture_specs.iter().flat_map(|(_, ty)| {
                [
                    RuntimeLocalDeclarationSeed::new(*ty),
                    RuntimeLocalDeclarationSeed::new(*ty),
                ]
            }),
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut value_local_ids = value_admission.local_ids().iter().cloned();
    let mut dialogue_value_capture_input_locals = BTreeMap::new();
    let mut dialogue_value_result_locals = BTreeMap::new();
    for (key, _) in &dialogue_value_capture_specs {
        let input_local = value_local_ids.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted dialogue value capture input local is missing",
            )]
        })?;
        let result_local = value_local_ids.next().ok_or_else(|| {
            vec![RuntimePlanLowerError::new(
                "admitted dialogue value caller result local is missing",
            )]
        })?;
        dialogue_value_capture_input_locals.insert(*key, input_local);
        dialogue_value_result_locals.insert(*key, result_local);
    }
    if value_local_ids.next().is_some() {
        return Err(vec![RuntimePlanLowerError::new(
            "admitted dialogue value locals contain an unexpected row",
        )]);
    }
    let mut project_source_specs = Vec::new();
    context
        .facts
        .visit_dialogue_content_fragments(&mut |_, fragment| {
            for value in fragment.values() {
                let Some(project) = value.project_display() else {
                    continue;
                };
                project_source_specs.push((
                    RuntimeDialogueValueCaptureKey::new(fragment.template().id(), value.slot(), 0),
                    project.source_type().identity(),
                ));
            }
        });
    let source_admission = builder
        .admit_type_batch(
            [],
            project_source_specs
                .iter()
                .map(|(_, ty)| RuntimeLocalDeclarationSeed::new(*ty)),
        )
        .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    let mut dialogue_value_project_source_locals = BTreeMap::new();
    for ((key, _), local) in project_source_specs
        .iter()
        .zip(source_admission.local_ids())
    {
        if dialogue_value_project_source_locals
            .insert(*key, local.clone())
            .is_some()
        {
            return Err(vec![RuntimePlanLowerError::new(
                "project dialogue value repeats its source local",
            )]);
        }
    }
    let context = FinalLoweringContext {
        dialogue_value_capture_input_locals: &dialogue_value_capture_input_locals,
        dialogue_value_result_locals: &dialogue_value_result_locals,
        dialogue_value_project_source_locals: &dialogue_value_project_source_locals,
        ..context
    };
    let (defer_sites, defer_definitions) =
        defer::reserve_global_defer_sites(&context, &mut builder, &mut errors);
    let context = FinalLoweringContext {
        defer_sites: &defer_sites,
        ..context
    };
    let (dialogue_content, mut dialogue_assertion_sites, plain_text_context_template) =
        lower_dialogue_content(&context, &dialogue_effect_sites, &mut builder, &mut errors);
    dialogue_assertion_sites.extend(dialogue_effect_assertion_sites);
    let context = FinalLoweringContext {
        dialogue_content: &dialogue_content,
        ..context
    };
    let (format_attempts, format_operand_source_locals) =
        format_attempt::reserve(&context, &mut builder).map_err(|error| vec![error])?;
    let context = FinalLoweringContext {
        format_attempts: &format_attempts,
        format_operand_source_locals: &format_operand_source_locals,
        ..context
    };

    // Bodies may invoke any reserved function site or start any admitted
    // dialogue occurrence. Define them only after both inventories exist.
    define_function_sites(&context, &function_definitions, &mut builder, &mut errors);
    dialogue_assertion_sites.extend(defer::define_global_defer_sites(
        &context,
        &defer_definitions,
        &mut builder,
        &mut errors,
    ));
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
                let Some(flow_fact) = facts.flow(item.id()) else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "checked runtime Flow identity is missing for final-HIR item {:?}",
                        item.id()
                    )));
                    continue;
                };
                let identity = flow_fact.identity().clone();
                match flow_invocation_schema(
                    item.module(),
                    flow,
                    &identity,
                    flow_fact.definition(),
                    facts,
                ) {
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
                        flow_seeds.push(RuntimeFlowSeed::new(
                            flow_fact
                                .definition()
                                .definition_identity()
                                .runtime_identity(),
                            identity,
                            params,
                            flow_fact.effects().clone(),
                            ops,
                        ));
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
    let mut plan = builder
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
    dialogue_templates.extend(facts.format_templates().map(|fact| fact.template().clone()));
    if let Some(template) = plain_text_context_template {
        dialogue_templates.push(template);
    }
    dialogue_templates.sort_by_key(|template| template.id());
    let dialogue_content_catalog = DialogueContentCatalog::try_from_records_and_templates(
        dialogue_records,
        dialogue_templates,
    )
    .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])?;
    if let Some(identity) = plan.dialogue_content().plain_text_context_template() {
        let path = format!("dialogue.template.{}", identity.id());
        let canonical = DialogueContentFragmentTemplate::plain_text_context(identity.id())
            .map_err(|error| {
                vec![RuntimePlanLowerError::new(format!(
                    "plain-text context template factory failed: {error}"
                ))]
            })?;
        let Some(manifest) = plan.dialogue_content().template(identity.id()) else {
            return Err(vec![RuntimePlanLowerError::new(format!(
                "{path} context pointer has no RuntimePlan manifest"
            ))]);
        };
        let Some(template) = dialogue_content_catalog.find_template(identity.id()) else {
            return Err(vec![RuntimePlanLowerError::new(format!(
                "{path} context pointer has no text-model template"
            ))]);
        };
        if manifest.digest() != identity.digest()
            || template.digest() != identity.digest()
            || template != &canonical
        {
            return Err(vec![RuntimePlanLowerError::new(format!(
                "{path} context template disagrees with its canonical text-model identity or body"
            ))]);
        }
        let proof = arcweft_core::value::RuntimeDialoguePlainTextContextTemplateProof::
            try_from_validated_ref(identity, canonical.digest())
            .map_err(|error| {
                vec![RuntimePlanLowerError::new(format!(
                    "{path} context proof is invalid: {error}"
                ))]
            })?;
        plan.accept_plain_text_context_template_proof(proof)
            .map_err(|error| {
                vec![RuntimePlanLowerError::new(format!(
                    "{path} context proof cannot be attached to the RuntimePlan: {error}"
                ))]
            })?;
    }
    let pure_helper_count = plan.pure_helpers().len();
    let character_dialogue_generation = facts
        .character_dialogue_generation()
        .map(|declaration| {
            declaration
                .try_map_type_refs(|ty| Ok::<_, std::convert::Infallible>(ty.identity()))
                .map(Arc::new)
                .map_err(|error| vec![RuntimePlanLowerError::new(error.to_string())])
        })
        .transpose()?;
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
        character_dialogue_generation,
        assertion_sites: assertion_sites.into_boxed_slice(),
    })
}

fn flow_invocation_schema(
    module: &HirModule,
    flow: &HirFlowItem,
    identity: &FlowRuntimeId,
    definition: &arcweft_lang_sema::final_analysis::CheckedExecutionInputAbi,
    facts: &RuntimePlanSemanticFacts,
) -> Result<RuntimeFlowSchema, RuntimePlanLowerError> {
    if !flow.generic_parameters().is_empty() || !flow.where_predicates().is_empty() {
        return Err(RuntimePlanLowerError::new(format!(
            "runtime Flow {identity} cannot publish an invocation schema with open generics"
        )));
    }
    if definition.parameters().len() != flow.parameters().len() {
        return Err(RuntimePlanLowerError::new(format!(
            "runtime Flow {identity} has a different accepted formal arity"
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
                let formal = &definition.parameters()[index];
                if formal.pattern() != Some(parameter.pattern()) || formal.bindings() != [*local]
                    || !matches!(formal.origin(), arcweft_lang_sema::final_analysis::CheckedExecutionParameterOrigin::Declaration(position)
                        if position.group().get() == 0 && position.parameter().get() == index)
                {
                    return Err(RuntimePlanLowerError::new(format!(
                        "runtime Flow {identity} parameter {index} disagrees with its accepted formal"
                    )));
                }
                Ok(RuntimeFlowExecutableParameter {
                    identity: formal.identity().runtime_identity(),
                    coordinate: FlowParameterCoordinate::try_from_index(index)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                    name: name.as_str().to_owned(),
                    mode: RuntimeFlowParameterMode::Owned,
                    passing: formal.passing(),
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
        BTreeSet<RuntimeClosureInstanceKey>,
    ),
    RuntimePlanLowerError,
> {
    let mut instances =
        BTreeMap::<RuntimeClosureInstanceKey, &'facts RuntimeClosureInstanceFact>::new();
    let mut parents = BTreeMap::new();
    let mut line_schedule_callbacks = BTreeSet::new();
    let mut order = Vec::new();
    for (_, call) in facts.calls() {
        let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(
            RuntimeLineCallable::Schedule { callback, .. },
        )) = call.dispatch()
        else {
            continue;
        };
        if let Some(closure) = facts.root_closure(*callback) {
            line_schedule_callbacks.insert(closure.key().clone());
        }
    }
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
            &mut line_schedule_callbacks,
        )?;
    }
    for instance in facts.project_function_instances() {
        collect_closure_instances_from_semantics(
            instance.semantics(),
            &mut instances,
            &mut order,
            ClosureLexicalParent::ProjectFunction(instance.key().clone()),
            &mut parents,
            &mut line_schedule_callbacks,
        )?;
    }
    for method in facts.trait_methods() {
        let Some(semantics) = method.closed_semantics() else {
            continue;
        };
        collect_closure_instances_from_semantics(
            semantics,
            &mut instances,
            &mut order,
            ClosureLexicalParent::TraitMethod(method.key().clone()),
            &mut parents,
            &mut line_schedule_callbacks,
        )?;
    }
    for (program, semantics) in facts.pure_program_semantics() {
        collect_closure_instances_from_semantics(
            semantics,
            &mut instances,
            &mut order,
            ClosureLexicalParent::Program(*program),
            &mut parents,
            &mut line_schedule_callbacks,
        )?;
    }
    Ok((order, parents, line_schedule_callbacks))
}

fn collect_closure_instances_from_semantics<'facts>(
    semantics: &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    instances: &mut BTreeMap<RuntimeClosureInstanceKey, &'facts RuntimeClosureInstanceFact>,
    order: &mut Vec<&'facts RuntimeClosureInstanceFact>,
    parent: ClosureLexicalParent,
    parents: &mut BTreeMap<RuntimeClosureInstanceKey, ClosureLexicalParent>,
    line_schedule_callbacks: &mut BTreeSet<RuntimeClosureInstanceKey>,
) -> Result<(), RuntimePlanLowerError> {
    for expression in semantics.expressions() {
        if let Some(call) = semantics.call(expression.owner())
            && let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(
                RuntimeLineCallable::Schedule { callback, .. },
            )) = call.dispatch()
            && let Some(closure) = semantics.closure_instance(*callback)
        {
            line_schedule_callbacks.insert(closure.key().clone());
        }
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
            line_schedule_callbacks,
        )?;
    }
    Ok(())
}

fn reserve_function_sites<'facts>(
    project: HirAnalysisProjectView<'_>,
    facts: &'facts RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    project_locals: &BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    closure_locals: &BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    trait_locals: &BTreeMap<RuntimeTraitMethodInstanceKey, ProjectFunctionFrameLocals>,
    program_locals: &BTreeMap<
        arcweft_id::runtime_program::RuntimePureProgramId,
        ProjectFunctionFrameLocals,
    >,
    implicit_parameters: &BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeLocalSeedId>,
    implicit_capture_input_locals: &BTreeMap<
        (RuntimeImplicitCallableSiteKey, u32),
        RuntimeLocalSeedId,
    >,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> (
    BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeFunctionSiteSeedId>,
    Vec<ReservedFunctionSiteDefinition<'facts>>,
) {
    let mut sites = BTreeMap::new();
    let mut definitions = Vec::new();
    reserve_implicit_function_sites(
        project,
        facts,
        locals,
        project_locals,
        closure_locals,
        trait_locals,
        program_locals,
        implicit_parameters,
        implicit_capture_input_locals,
        builder,
        errors,
        (&mut sites, &mut definitions),
    );
    (sites, definitions)
}
fn reserve_implicit_function_sites<'facts>(
    project: HirAnalysisProjectView<'_>,
    facts: &'facts RuntimePlanSemanticFacts,
    locals: &BTreeMap<LocalId, RuntimeLocalSeedId>,
    project_locals: &BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectFunctionFrameLocals>,
    closure_locals: &BTreeMap<RuntimeClosureInstanceKey, ClosureFrameLocals>,
    trait_locals: &BTreeMap<RuntimeTraitMethodInstanceKey, ProjectFunctionFrameLocals>,
    program_locals: &BTreeMap<
        arcweft_id::runtime_program::RuntimePureProgramId,
        ProjectFunctionFrameLocals,
    >,
    implicit_parameters: &BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeLocalSeedId>,
    implicit_capture_input_locals: &BTreeMap<
        (RuntimeImplicitCallableSiteKey, u32),
        RuntimeLocalSeedId,
    >,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
    output: (
        &mut BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeFunctionSiteSeedId>,
        &mut Vec<ReservedFunctionSiteDefinition<'facts>>,
    ),
) {
    let (sites, definitions) = output;
    let mut callables = Vec::new();
    facts.visit_scoped_implicit_callables(&mut |scope, owner, callable| {
        callables.push((scope, owner, callable));
    });
    for (scope, owner, callable) in callables {
        let key = RuntimeImplicitCallableSiteKey::new(scope.scope(), owner);
        let selected_locals = match scope.scope() {
            RuntimeExecutableSemanticScope::Global => Some(locals),
            RuntimeExecutableSemanticScope::Program(program) => {
                program_locals.get(&program).map(|frame| &frame.hir)
            }
            RuntimeExecutableSemanticScope::ProjectFunction(instance) => {
                project_locals.get(instance).map(|frame| &frame.hir)
            }
            RuntimeExecutableSemanticScope::Closure(instance) => {
                closure_locals.get(instance).map(|frame| &frame.hir)
            }
            RuntimeExecutableSemanticScope::TraitMethod(instance) => {
                trait_locals.get(instance).map(|frame| &frame.hir)
            }
        };
        let Some(selected_locals) = selected_locals else {
            errors.push(RuntimePlanLowerError::new(format!(
                "implicit callable {owner:?} has no admitted lexical local frame"
            )));
            continue;
        };
        let Some(module) = module_by_id(project, owner.module()) else {
            errors.push(RuntimePlanLowerError::new(format!(
                "implicit callable {owner:?} module is absent"
            )));
            continue;
        };
        let Some(parameter) = implicit_parameters.get(&key).cloned() else {
            errors.push(RuntimePlanLowerError::new(format!(
                "implicit callable {owner:?} parameter local is absent"
            )));
            continue;
        };
        let parameter_ownership = if let Some(requirement) =
            scope.checked_synthetic_copy_requirement(callable.identity())
        {
            if requirement.ty().as_bytes() != callable.parameter().identity().as_bytes() {
                errors.push(RuntimePlanLowerError::new(format!(
                    "implicit callable {owner:?} Copy ingress requirement disagrees with its parameter type"
                )));
                continue;
            }
            RuntimeFunctionInputOwnershipRequirement::Unrestricted
        } else {
            RuntimeFunctionInputOwnershipRequirement::Owned
        };
        let captures = callable
            .captures()
            .iter()
            .map(|capture| -> Result<_, String> {
                let binding = selected_locals.get(&capture.local()).cloned().ok_or_else(|| {
                    format!("implicit callable {owner:?} capture {capture:?} is absent")
                })?;
                let position = capture.position();
                let input_local = implicit_capture_input_locals
                    .get(&(key.clone(), position))
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "implicit callable {owner:?} capture has no admitted synthetic input local"
                        )
                    })?;
                let ty = scope.local_type(capture.local()).ok_or_else(|| {
                    format!("implicit callable {owner:?} capture {capture:?} has no accepted type")
                })?;
                Ok(RuntimeFunctionInputBindingSeed {
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Transferred(capture.transfer().runtime_capture_mode().expect("accepted capture is a value transfer")),
                    origin: capture.origin().runtime_input_origin().map_err(|error| error.to_string())?,
                    ownership: match capture.transfer().mode() { arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Copy => RuntimeFunctionInputOwnershipRequirement::Unrestricted, arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Move => RuntimeFunctionInputOwnershipRequirement::Owned, arcweft_lang_sema::final_analysis::CheckedLocalReadMode::Borrow => unreachable!("accepted implicit capture is a value transfer") },
                    unrestricted_bindings: Box::new([]),
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
            transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
            origin: callable.formal().runtime_input_origin(),
            ownership: parameter_ownership,
            unrestricted_bindings: Box::new([]),
            source: RuntimeFunctionInputSource::Parameter {
                position: 0,
                passing: callable.formal().passing(),
            },
            input_local: parameter.clone(),
            pattern: RuntimePatternSeed::new(
                callable.parameter().identity(),
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local: parameter.clone(),
                },
            ),
        };
        let Some(RuntimeTypeShape::Function { contract, .. }) = scope
            .expression_type(owner)
            .map(RuntimeNormalizedType::shape)
        else {
            errors.push(RuntimePlanLowerError::new(
                "implicit body has no closed function contract",
            ));
            continue;
        };
        if contract.invocation().variables().next().is_some() {
            errors.push(RuntimePlanLowerError::new(
                "implicit body has an open invocation effect row",
            ));
            continue;
        }
        let effects =
            match RuntimeEffectSet::try_from_effects(contract.invocation().constant_effects()) {
                Ok(effects) => effects,
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(error.to_string()));
                    continue;
                }
            };
        let declaration = captures.map(|captures| RuntimeFunctionSiteDeclarationSeed {
            definition: callable.definition_identity().runtime_identity(),
            role: arcweft_core::plan::RuntimeFunctionSemanticRole::Closure,
            function_type: None,
            inputs: captures
                .into_iter()
                .chain([parameter_input])
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            result: callable.result().identity(),
            body_kind: if callable.control() == arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible
                && effects.is_empty() {
                RuntimeFunctionSiteBodyKind::Expression
            } else { RuntimeFunctionSiteBodyKind::Executable },
            effects: effects.clone(),
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
                sites.insert(key, site.clone());
                definitions.push(ReservedFunctionSiteDefinition {
                    effects,
                    scope,
                    owner,
                    module: module.module_id(),
                    body: owner,
                    site,
                    implicit_parameter: parameter,
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
    project: HirAnalysisProjectView<'_>,
    facts: &RuntimePlanSemanticFacts,
    closures: &[&'facts RuntimeClosureInstanceFact],
    line_schedule_callbacks: &BTreeSet<RuntimeClosureInstanceKey>,
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
        if line_schedule_callbacks.contains(&key) {
            continue;
        }
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
                let ownership = match capture.transfer().mode()
                {
                    CheckedLocalReadMode::Copy => {
                        RuntimeFunctionInputOwnershipRequirement::Unrestricted
                    }
                    CheckedLocalReadMode::Move => RuntimeFunctionInputOwnershipRequirement::Owned,
                    CheckedLocalReadMode::Borrow => {
                        return Err(RuntimePlanLowerError::new(format!(
                            "project closure {:?} capture source {:?} has an invalid borrowed value role",
                            key,
                            capture.source()
                        )));
                    }
                };
                Ok(RuntimeFunctionInputBindingSeed {
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Transferred(capture.transfer().runtime_capture_mode().expect("accepted capture is a value transfer")),
                    origin: capture.origin().runtime_input_origin().map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                    ownership,
                    unrestricted_bindings: Box::new([]),
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
                let unrestricted_bindings = closure
                    .semantics()
                    .local_uses()
                    .copy_requirements()
                    .into_iter()
                    .filter(|requirement| matches!(
                        requirement.owner(),
                        arcweft_lang_sema::final_analysis::CheckedLocalCopyIngressOwner::Closure {
                            closure: owner,
                            parameter: position,
                        } if *owner == closure.owner() && *position == parameter.position()
                    ))
                    .map(|requirement| {
                        if closure
                            .semantics()
                            .local_type(requirement.local())
                            .is_none_or(|ty| ty.identity().as_bytes() != requirement.ty().as_bytes())
                        {
                            return Err(RuntimePlanLowerError::new(format!(
                                "project closure {:?} Copy ingress local {:?} has another type",
                                key,
                                requirement.local()
                            )));
                        }
                        locals.hir.get(&requirement.local()).cloned().ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "project closure {:?} Copy ingress local {:?} has no admitted binding",
                                key,
                                requirement.local()
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(RuntimeFunctionInputBindingSeed {
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                    origin: parameter.definition().runtime_input_origin(),
                    ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                    unrestricted_bindings: unrestricted_bindings.into_boxed_slice(),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: parameter.position(),
                        passing: parameter.passing(),
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
        let effects = match RuntimeEffectSet::try_from_effects(closure.effects().iter().cloned()) {
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
                definition: closure.definition_identity().runtime_identity(),
                role: arcweft_core::plan::RuntimeFunctionSemanticRole::Closure,
                function_type: None,
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
    project: HirAnalysisProjectView<'_>,
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
                    RuntimeFunctionInputSource::CapturedParameter {
                        position,
                        passing: parameter.passing(),
                    }
                }
                RuntimeProjectFunctionParameterSource::CurrentGroup { position } => {
                    RuntimeFunctionInputSource::Parameter {
                        position,
                        passing: parameter.passing(),
                    }
                }
            };
            let mut unrestricted_bindings = Vec::new();
            for local in parameter.bindings() {
                let Some(requirement) = instance.semantics().local_uses().copy_requirement(*local)
                else {
                    continue;
                };
                if !matches!(
                    requirement.owner(),
                    arcweft_lang_sema::final_analysis::CheckedLocalCopyIngressOwner::Declaration {
                        declaration,
                        parameter: arcweft_lang_sema::final_analysis::CheckedIngressParameterCoordinate::Parameter {
                            group: required_group,
                            parameter: required_parameter,
                        },
                    } if declaration == instance.callable().declaration()
                        && *required_group == group
                        && *required_parameter == parameter.parameter()
                ) || instance
                    .semantics()
                    .local_type(*local)
                    .is_none_or(|ty| ty.identity().as_bytes() != requirement.ty().as_bytes())
                {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} has a mismatched Copy ingress requirement for {local:?}",
                        key
                    )));
                    invalid = true;
                    continue;
                }
                let Some(seed) = local_map.hir.get(local).cloned() else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} Copy ingress binding {local:?} has no admitted local",
                        key
                    )));
                    invalid = true;
                    continue;
                };
                unrestricted_bindings.push(seed);
            }
            inputs.push(RuntimeFunctionInputBindingSeed {
                transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                origin: parameter.definition().runtime_input_origin(),
                ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                unrestricted_bindings: unrestricted_bindings.into_boxed_slice(),
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
            let unrestricted_bindings: Box<[RuntimeLocalSeedId]> = if let Some(requirement) =
                instance
                    .semantics()
                    .local_uses()
                    .copy_requirement(attached.binding())
            {
                if !matches!(
                    requirement.owner(),
                    arcweft_lang_sema::final_analysis::CheckedLocalCopyIngressOwner::Declaration {
                        declaration,
                        parameter: arcweft_lang_sema::final_analysis::CheckedIngressParameterCoordinate::AttachedContent,
                    } if declaration == instance.callable().declaration()
                ) || requirement.ty().as_bytes() != attached.binding_ty().identity().as_bytes()
                {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "project-function instance {:?} attached Copy ingress requirement disagrees with its binding",
                        key
                    )));
                    invalid = true;
                    Box::new([])
                } else {
                    Box::new([binding.clone()])
                }
            } else {
                Box::new([])
            };
            let Some(formal) = instance.definition().parameters().iter().find(|parameter|
                matches!(parameter.origin(), arcweft_lang_sema::final_analysis::CheckedExecutionParameterOrigin::AttachedContent)) else {
                errors.push(RuntimePlanLowerError::new("attached input has no accepted whole formal"));
                continue;
            };
            inputs.push(RuntimeFunctionInputBindingSeed {
                transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                origin: formal.runtime_input_origin(),
                ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                unrestricted_bindings,
                source: RuntimeFunctionInputSource::Parameter {
                    position: attached.abi_position(),
                    passing: formal.passing(),
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
        let effects = match RuntimeEffectSet::try_from_effects(instance.effects().iter().cloned()) {
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
            definition: instance.definition_identity().runtime_identity(),
            role: arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
            function_type: None,
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
    project: HirAnalysisProjectView<'_>,
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
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                    origin: capture.formal().definition().runtime_input_origin(),
                    ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
                    unrestricted_bindings: Box::new([]),
                    source: RuntimeFunctionInputSource::CapturedParameter { position, passing: capture.formal().passing() },
                    input_local,
                    pattern,
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>();
        let effects = RuntimeEffectSet::try_from_effects(default.effects().iter().cloned())
            .map_err(|error| {
                RuntimePlanLowerError::new(format!(
                    "project-function instance {:?} default effect row is invalid: {error}",
                    key
                ))
            });
        let declaration = inputs.and_then(|inputs| {
            effects.map(|effects| RuntimeFunctionSiteDeclarationSeed {
                definition: default.definition_identity().runtime_identity(),
                role: arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
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

fn reserve_pure_programs<'facts>(
    context: &FinalLoweringContext<'_, 'facts>,
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) -> Vec<ReservedPureProgramDefinition<'facts>> {
    let mut definitions = Vec::new();
    let project = context.project;
    let facts = context.facts;
    for (_, program) in facts.pure_programs() {
        let scope = match facts.program_scope(program) {
            Ok(scope) => scope,
            Err(error) => {
                errors.push(RuntimePlanLowerError::new(error.to_string()));
                continue;
            }
        };
        let locals = match context.executable_locals(scope.scope()) {
            Ok(locals) => locals,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let inputs = program
            .free_inputs()
            .enumerate()
            .map(|(position, (input, ty))| {
                let local = locals
                    .get(&input.binding().local())
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "pure program {} capture {:?} has no admitted local",
                            program.program(),
                            input.binding().local()
                        ))
                    })?;
                let position = u32::try_from(position).map_err(|_| {
                    RuntimePlanLowerError::new("pure program input position exceeds u32")
                })?;
                Ok(RuntimeFunctionInputBindingSeed {
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::ExternalBinding,
                    origin: input
                        .binding()
                        .origin()
                        .runtime_input_origin()
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?,
                    source: RuntimeFunctionInputSource::Capture { position },
                    input_local: local.clone(),
                    pattern: RuntimePatternSeed::new(
                        ty.identity(),
                        RuntimePatternSeedKind::Bind {
                            mutable: program
                                .admission()
                                .input_abi()
                                .binding_outputs()
                                .iter()
                                .any(|output| output.local() == input.binding().local()),
                            local: local.clone(),
                        },
                    ),
                    ownership: if input.copy_requirement().is_some()
                        || input.copy_evidence().is_some()
                    {
                        RuntimeFunctionInputOwnershipRequirement::Unrestricted
                    } else {
                        RuntimeFunctionInputOwnershipRequirement::Owned
                    },
                    unrestricted_bindings: if input.copy_requirement().is_some()
                        || input.copy_evidence().is_some()
                    {
                        Box::new([local.clone()])
                    } else {
                        Box::new([])
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>();
        let mut parameter_inputs = Vec::new();
        let parameters = program
            .parameters()
            .enumerate()
            .map(|(position, (parameter, ty))| {
                let declaration = if ty.scope().is_root() {
                    RuntimeLocalDeclarationSeed::new(ty.identity())
                } else {
                    RuntimeLocalDeclarationSeed::in_function(ty.identity(), program.function_type().identity())
                };
                let admitted = builder
                    .admit_type_batch([], [declaration])
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                let input_local = admitted.local_ids().first().cloned().ok_or_else(|| {
                    RuntimePlanLowerError::new("program parameter input local is absent")
                })?;
                parameter_inputs.push(input_local.clone());
                let pattern = match parameter.pattern() {
                    Some(pattern) => {
                        let module = module_by_id(project, pattern.module()).ok_or_else(|| {
                            RuntimePlanLowerError::new("program parameter module is absent")
                        })?;
                        FinalPatternLowerer::new(module, facts, locals)
                            .with_semantic_facts(scope.facts())
                            .lower(pattern)
                            .map_err(RuntimePlanLowerError::new)?
                    }
                    None => RuntimePatternSeed::new(
                        ty.identity(),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: match parameter.bindings() {
                                [] => input_local.clone(),
                                [binding] => locals.get(binding).cloned().ok_or_else(|| {
                                    RuntimePlanLowerError::new("program parameter binding is absent")
                                })?,
                                _ => return Err(RuntimePlanLowerError::new(
                                    "program parameter with multiple bindings requires a pattern",
                                )),
                            },
                        },
                    ),
                };
                let unrestricted_bindings = program.admission().input_abi().inputs().iter()
                    .filter(|input| matches!(input.role(), arcweft_lang_sema::final_analysis::CheckedExecutionInputRole::Parameter(origin) if origin == parameter.origin())
                        && input.copy_requirement().is_some())
                    .map(|input| locals.get(&input.binding().local()).cloned()
                        .ok_or_else(|| RuntimePlanLowerError::new("program Copy ingress binding is absent")))
                    .collect::<Result<Vec<_>, _>>()?;
                let ownership = if let arcweft_lang_sema::final_analysis::CheckedExecutionParameterOrigin::Implicit(callable) = parameter.origin()
                    && let Some(requirement) = scope.checked_synthetic_copy_requirement(*callable)
                {
                    if requirement.ty().as_bytes() != ty.identity().as_bytes() {
                        return Err(RuntimePlanLowerError::new("program synthetic Copy ingress type disagrees with its formal"));
                    }
                    RuntimeFunctionInputOwnershipRequirement::Unrestricted
                } else { RuntimeFunctionInputOwnershipRequirement::Owned };
                Ok(RuntimeFunctionInputBindingSeed {
                    transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Formal,
                    origin: parameter.runtime_input_origin(),
                    source: RuntimeFunctionInputSource::Parameter {
                        position: u32::try_from(position).map_err(|_| {
                            RuntimePlanLowerError::new("program parameter position exceeds u32")
                        })?,
                        passing: parameter.passing(),
                    },
                    input_local,
                    pattern,
                    ownership,
                    unrestricted_bindings: unrestricted_bindings.into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>();
        let inputs = inputs.and_then(|mut inputs| {
            inputs.extend(parameters?);
            Ok(inputs)
        });
        let mut body_kind = program.body_kind();
        if let Some(semantics) = facts
            .pure_program_semantics()
            .find_map(|(id, semantics)| (*id == program.program()).then_some(semantics))
        {
            semantics.visit_calls(&mut |_, call| {
                if matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
                    || call.project_function().and_then(|call| call.outcome().instance()).is_some_and(|key| facts.project_function_instance(key).is_some_and(|instance| instance.execution() == crate::semantic_facts::RuntimeProjectFunctionExecution::ExecutableFunctionSite))
                {
                    body_kind = RuntimeFunctionSiteBodyKind::Executable;
                }
            });
        }
        let declaration = inputs.map(|inputs| RuntimeFunctionSiteDeclarationSeed {
            definition: program.definition_identity().runtime_identity(),
            role: program.semantic_role(),
            function_type: Some(program.function_type().identity()),
            inputs: inputs.into_boxed_slice(),
            result: program.result().identity(),
            body_kind,
            effects: RuntimeEffectSet::empty(),
        });
        let site = match declaration.and_then(|declaration| {
            builder
                .reserve_function_site_seed(declaration)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(site) => site,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let binding = RuntimePureProgramBindingSeed {
            program: program.program(),
            site: site.clone(),
        };
        if let Err(error) = builder.push_pure_program_binding_seed(&binding) {
            errors.push(RuntimePlanLowerError::new(error.to_string()));
            continue;
        }
        let iteration_output = if matches!(
            program.source(),
            arcweft_lang_sema::final_analysis::CheckedExecutionSource::ExportIteration(_)
        ) {
            match builder.admit_type_batch(
                [],
                [RuntimeLocalDeclarationSeed::new(
                    program.result().identity(),
                )],
            ) {
                Ok(admitted) => admitted.local_ids().first().cloned(),
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(error.to_string()));
                    continue;
                }
            }
        } else {
            None
        };
        definitions.push(ReservedPureProgramDefinition {
            program,
            scope,
            parameter_inputs: parameter_inputs.into_boxed_slice(),
            site,
            body_kind,
            iteration_output,
        });
    }
    definitions
}

fn define_function_sites(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedFunctionSiteDefinition<'_>],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    for definition in definitions {
        let Some(module) = module_by_id(context.project, definition.module) else {
            errors.push(RuntimePlanLowerError::new("closure module is absent"));
            continue;
        };
        let lowerer = match context.scoped_expr_lowerer(module, definition.scope) {
            Ok(lowerer) => lowerer,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let body = (|| {
            let callable = definition
                .scope
                .implicit_callable(definition.body)
                .ok_or_else(|| {
                    "implicit callable fact is absent in its lexical instance".to_owned()
                })?;
            let overrides = callable
                    .placeholders()
                    .iter()
                    .map(|placeholder| {
                        let use_row = definition
                            .scope
                            .checked_synthetic_use(*placeholder)
                            .ok_or_else(|| {
                                format!("checked implicit parameter use is missing for {placeholder:?}")
                            })?;
                        if use_row.owner()
                            != arcweft_lang_sema::final_analysis::CheckedSyntheticUseOwner::ImplicitParameter(
                                callable.identity(),
                            )
                        {
                            return Err(format!(
                                "checked implicit parameter use {placeholder:?} has another owner"
                            ));
                        }
                        Ok((
                            *placeholder,
                            RuntimeExprSeed::new(
                                callable.parameter().identity(),
                                arcweft_core::plan::RuntimeExprSeedKind::Local(
                                    RuntimeLocalReadSeed::new(
                                        definition.implicit_parameter.clone(),
                                        crate::final_expr::runtime_local_read_mode(use_row.mode()),
                                    ),
                                ),
                            ),
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, String>>()?;
            if callable.control() == arcweft_lang_sema::final_analysis::CheckedExecutableControlRole::ExpressionCompatible
                && definition.effects.is_empty() {
                lowerer.lower_implicit_callable_body(definition.owner, overrides)
                    .map(RuntimeFunctionSiteBodySeed::Expression)
            } else {
                let mut flow = FinalFlowLowerer::new(
                    module, context, RuntimeAssertionOwner::ImplicitCallable(callable.identity()),
                ).with_executable_scope(
                    definition.scope,
                    context.executable_control_locals(definition.scope.scope()).map_err(|error| error.to_string())?,
                    context.executable_locals(definition.scope.scope()).map_err(|error| error.to_string())?,
                    context.executable_specialized_operand_locals(definition.scope.scope()).map_err(|error| error.to_string())?,
                );
                flow.expression_overrides = overrides;
                flow.implicit_body_root = Some(definition.owner);
                flow.lower_flow_value(definition.body, RuntimeFlowValueContinuation::Return)
                    .map(|ops| RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                        effects: definition.effects.clone(), ops: ops.into_boxed_slice(),
                    })).map_err(|error| error.to_string())
            }
        })();
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
                    .with_scope_locals(&locals.control.scopes)
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
                        let effects =
                            RuntimeEffectSet::try_from_effects(closure.effects().iter().cloned())
                                .expect(
                                    "project closure effect row was validated during reservation",
                                );
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
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
                    .with_scope_locals(&locals.control.scopes)
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
                        overrides: BTreeMap::new(),
                    },
                ) {
                    Ok(ops) => ops,
                    Err(lower_errors) => {
                        errors.push(lower_errors);
                        continue;
                    }
                };
                let effects =
                    match RuntimeEffectSet::try_from_effects(instance.effects().iter().cloned()) {
                        Ok(effects) => effects,
                        Err(error) => {
                            errors.push(RuntimePlanLowerError::new(error.to_string()));
                            continue;
                        }
                    };
                Ok(RuntimeFunctionSiteBodySeed::Executable(
                    RuntimeExecutableBodySeed {
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
                    .with_scope_locals(&locals.control.scopes)
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
                        let effects =
                            RuntimeEffectSet::try_from_effects(default.effects().iter().cloned())
                                .expect("default site effect row was validated during reservation");
                        RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
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

fn exported_program_bindings(
    context: &FinalLoweringContext<'_, '_>,
    scope: RuntimeScopedExecutableSemanticFactView<'_>,
    bindings: &[arcweft_lang_sema::final_analysis::CheckedExecutableCapture],
    result: &crate::semantic_facts::RuntimeNormalizedType,
) -> Result<RuntimeExprSeed, RuntimePlanLowerError> {
    let fields = match result.shape() {
        RuntimeTypeShape::Unit if bindings.is_empty() => {
            return Ok(RuntimeExprSeed::new(
                result.identity(),
                RuntimeExprSeedKind::Value(RuntimeValue::Unit),
            ));
        }
        RuntimeTypeShape::Tuple(types) if types.len() == bindings.len() && !types.is_empty() => {
            types
        }
        _ => {
            return Err(RuntimePlanLowerError::new(
                "owned program outputs disagree with result ABI",
            ));
        }
    };
    let locals = context.executable_locals(scope.scope())?;
    let values = bindings
        .iter()
        .zip(fields)
        .map(|(binding, ty)| {
            let local = locals
                .get(&binding.local())
                .ok_or_else(|| RuntimePlanLowerError::new("owned output has no admitted local"))?;
            Ok(RuntimeExprSeed::new(
                ty.identity(),
                RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                    local.clone(),
                    RuntimeLocalReadMode::Move,
                )),
            ))
        })
        .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
    Ok(RuntimeExprSeed::new(
        result.identity(),
        RuntimeExprSeedKind::Tuple(values.into_boxed_slice()),
    ))
}

fn define_pure_programs(
    context: &FinalLoweringContext<'_, '_>,
    definitions: &[ReservedPureProgramDefinition<'_>],
    builder: &mut RuntimePlanBuilder,
    errors: &mut Vec<RuntimePlanLowerError>,
) {
    use arcweft_lang_hir::body_edges::{HirBodyChild, HirBodyChildRole, HirBodyKind};
    use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};
    for definition in definitions {
        let program = definition.program;
        let abi = program.admission().input_abi();
        let body = (|| -> Result<RuntimeFunctionSiteBodySeed, RuntimePlanLowerError> {
            let module_id = match program.source() {
                CheckedExecutionSource::ExportBinding(owner)
                | CheckedExecutionSource::ExportIteration(owner) => owner.module(),
                CheckedExecutionSource::SelectMatch(owner) => owner.module(),
                CheckedExecutionSource::EvaluateValue(owner)
                | CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(
                    owner,
                ))
                | CheckedExecutionSource::ExportMutation(
                    CheckedExecutionBodyOwner::CallableValue(owner),
                ) => owner.module(),
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    ..
                })
                | CheckedExecutionSource::ExportMutation(
                    CheckedExecutionBodyOwner::Declaration { declaration, .. },
                ) => abi
                    .hir_topology()
                    .declaration(declaration)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                    .body()
                    .source_item()
                    .module(),
            };
            let module = module_by_id(context.project, module_id)
                .ok_or_else(|| RuntimePlanLowerError::new("program body module is absent"))?;
            let lowerer = context.scoped_expr_lowerer(module, definition.scope)?;
            let mut flow = FinalFlowLowerer::new(
                module,
                context,
                RuntimeAssertionOwner::Program(program.program()),
            )
            .with_executable_scope(
                definition.scope,
                context.executable_control_locals(definition.scope.scope())?,
                context.executable_locals(definition.scope.scope())?,
                context.executable_specialized_operand_locals(definition.scope.scope())?,
            );
            let expression_compatible =
                definition.body_kind == RuntimeFunctionSiteBodyKind::Expression;
            if matches!(program.source(), CheckedExecutionSource::ExportMutation(_)) {
                flow.mutation_return = Some(RuntimeMutationReturn::try_new(
                    context,
                    definition.scope,
                    abi.binding_outputs(),
                    program.result(),
                )?);
            }
            let ops = match program.source() {
                CheckedExecutionSource::ExportIteration(statement) => {
                    let row = module
                        .resolve_stmt(*statement)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                    let HirStmtKind::For(iteration) = row.kind() else {
                        return Err(RuntimePlanLowerError::new(
                            "iteration program has no For owner",
                        ));
                    };
                    let RuntimeTypeShape::Sequence { item, .. } = program.result().shape() else {
                        return Err(RuntimePlanLowerError::new(
                            "iteration program has no sequence result",
                        ));
                    };
                    let output = definition.iteration_output.as_ref().ok_or_else(|| {
                        RuntimePlanLowerError::new("iteration result local was not admitted")
                    })?;
                    let value = exported_program_bindings(
                        context,
                        definition.scope,
                        abi.binding_outputs(),
                        item,
                    )?;
                    let unit =
                        arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest();
                    let append = RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(unit, RuntimePatternSeedKind::Discard),
                        expr: RuntimeExprSeed::new(
                            unit,
                            RuntimeExprSeedKind::SequencePush {
                                place: arcweft_core::plan::RuntimeMutablePlaceSeed::Local(
                                    output.clone(),
                                ),
                                value: Box::new(value),
                            },
                        ),
                    };
                    let continuation =
                        flow.lower_iteration_continuation(*statement, iteration, vec![append])?;
                    let mut ops = vec![RuntimeFlowOpSeed::Let {
                        pattern: RuntimePatternSeed::new(
                            program.result().identity(),
                            RuntimePatternSeedKind::Bind {
                                mutable: true,
                                local: output.clone(),
                            },
                        ),
                        expr: RuntimeExprSeed::new(
                            program.result().identity(),
                            RuntimeExprSeedKind::BracketSeq(Box::new([])),
                        ),
                    }];
                    ops.extend(flow.lower_flow_value(iteration.source(), continuation)?);
                    ops.push(RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        program.result().identity(),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            output.clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    )));
                    ops
                }
                CheckedExecutionSource::ExportBinding(statement) => {
                    let result = exported_program_bindings(
                        context,
                        definition.scope,
                        abi.binding_outputs(),
                        program.result(),
                    )?;
                    flow.lower_statement_ids_with_tail(
                        &[*statement],
                        RuntimeFlowTail::PreparedOps(
                            vec![RuntimeFlowOpSeed::ReturnExpr(result)].into_boxed_slice(),
                        ),
                    )?
                }
                CheckedExecutionSource::SelectMatch(owner) => {
                    let matched = owner
                        .resolve(module)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                    let selection = abi.match_selection().ok_or_else(|| {
                        RuntimePlanLowerError::new("selector has no issued Match output layout")
                    })?;
                    let RuntimeTypeShape::Tuple(result_fields) = program.result().shape() else {
                        return Err(RuntimePlanLowerError::new("selector has no tuple result"));
                    };
                    let [tag, payload] = result_fields.as_ref() else {
                        return Err(RuntimePlanLowerError::new(
                            "selector has no tagged output contract",
                        ));
                    };
                    if selection.outputs().len() != matched.arms().len() {
                        return Err(RuntimePlanLowerError::new(
                            "selector case inventory disagrees with ABI",
                        ));
                    }
                    let payloads = match payload.shape() {
                        RuntimeTypeShape::Choice(alternatives) => alternatives
                            .iter()
                            .map(|ty| (ty.identity(), ty))
                            .collect::<BTreeMap<_, _>>(),
                        _ => BTreeMap::from([(payload.identity(), payload)]),
                    };
                    let mut arms = Vec::new();
                    for (index, arm) in matched.arms().enumerate() {
                        let payload_type = selection.payload_type(index).ok_or_else(|| {
                            RuntimePlanLowerError::new("selector output case is absent")
                        })?;
                        let identity = arcweft_id::RuntimeSemanticTypeId::from_semantic_digest(
                            *abi.environment()
                                .semantic_type_identity(&payload_type)
                                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                                .as_bytes(),
                        );
                        let payload_type = payloads.get(&identity).ok_or_else(|| {
                            RuntimePlanLowerError::new(
                                "selector output type is absent from its Choice",
                            )
                        })?;
                        let value = exported_program_bindings(
                            context,
                            definition.scope,
                            &selection.outputs()[index],
                            payload_type,
                        )?;
                        let ordinal = u32::try_from(index).map_err(|_| {
                            RuntimePlanLowerError::new("selector arm ordinal exceeds u32")
                        })?;
                        let values = vec![
                            RuntimeExprSeed::new(
                                tag.identity(),
                                RuntimeExprSeedKind::Value(RuntimeValue::UInt(
                                    arcweft_core::value::RuntimeUInt::U32(ordinal),
                                )),
                            ),
                            value,
                        ];
                        arms.push(RuntimeFlowMatchArmSeed {
                            pattern: flow
                                .pattern_lowerer()
                                .lower(arm.pattern())
                                .map_err(RuntimePlanLowerError::new)?,
                            guard: arm
                                .guard()
                                .map(|guard| {
                                    flow.lower_match_guard(
                                        guard,
                                        matched.scrutinee(),
                                        BTreeMap::new(),
                                    )
                                })
                                .transpose()?,
                            ops: vec![RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                                program.result().identity(),
                                RuntimeExprSeedKind::Tuple(values.into_boxed_slice()),
                            ))],
                        });
                    }
                    flow.lower_flow_value(
                        matched.scrutinee(),
                        RuntimeFlowValueContinuation::Match { arms },
                    )?
                }
                CheckedExecutionSource::EvaluateValue(owner) => {
                    if expression_compatible {
                        return lowerer
                            .lower(*owner)
                            .map(RuntimeFunctionSiteBodySeed::Expression)
                            .map_err(RuntimePlanLowerError::new);
                    }
                    flow.lower_flow_value(*owner, RuntimeFlowValueContinuation::Return)?
                }
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::CallableValue(
                    owner,
                ))
                | CheckedExecutionSource::ExportMutation(
                    CheckedExecutionBodyOwner::CallableValue(owner),
                ) => {
                    let expression = module
                        .resolve_expr(*owner)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                    if let HirExprKind::Closure(closure) = expression.kind() {
                        if expression_compatible {
                            return lowerer
                                .lower_function_site_body(*owner, closure.body(), BTreeMap::new())
                                .map(RuntimeFunctionSiteBodySeed::Expression)
                                .map_err(RuntimePlanLowerError::new);
                        }
                        flow.lower_flow_value(closure.body(), RuntimeFlowValueContinuation::Return)?
                    } else {
                        let callable =
                            definition.scope.implicit_callable(*owner).ok_or_else(|| {
                                RuntimePlanLowerError::new("program implicit body is absent")
                            })?;
                        let input = definition.parameter_inputs.first().ok_or_else(|| {
                            RuntimePlanLowerError::new("program implicit input is absent")
                        })?;
                        let overrides = callable
                            .placeholders()
                            .iter()
                            .map(|placeholder| {
                                let use_row = definition
                                    .scope
                                    .checked_synthetic_use(*placeholder)
                                    .ok_or_else(|| {
                                        RuntimePlanLowerError::new(
                                            "program synthetic input use is absent",
                                        )
                                    })?;
                                if use_row.owner() != arcweft_lang_sema::final_analysis::CheckedSyntheticUseOwner::ImplicitParameter(callable.identity()) {
                                    return Err(RuntimePlanLowerError::new("program synthetic use belongs to another callable"));
                                }
                                Ok((
                                    *placeholder,
                                    RuntimeExprSeed::new(
                                        callable.parameter().identity(),
                                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                                            input.clone(),
                                            crate::final_expr::runtime_local_read_mode(
                                                use_row.mode(),
                                            ),
                                        )),
                                    ),
                                ))
                            })
                            .collect::<Result<BTreeMap<_, _>, RuntimePlanLowerError>>()?;
                        if expression_compatible {
                            return lowerer
                                .lower_implicit_callable_body(*owner, overrides)
                                .map(RuntimeFunctionSiteBodySeed::Expression)
                                .map_err(RuntimePlanLowerError::new);
                        }
                        flow.expression_overrides = overrides;
                        flow.implicit_body_root = Some(*owner);
                        flow.lower_flow_value(*owner, RuntimeFlowValueContinuation::Return)?
                    }
                }
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role,
                })
                | CheckedExecutionSource::ExportMutation(
                    CheckedExecutionBodyOwner::Declaration { declaration, role },
                ) => {
                    let declaration = abi
                        .hir_topology()
                        .declaration(declaration)
                        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                    let root = declaration
                        .body()
                        .roots()
                        .iter()
                        .find(|root| root.role() == *role)
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new("program declaration body root is absent")
                        })?;
                    let projection = root.projection();
                    if expression_compatible {
                        return lowerer
                            .lower_body_projection(projection)
                            .map(RuntimeFunctionSiteBodySeed::Expression)
                            .map_err(RuntimePlanLowerError::new);
                    }
                    if projection.kind() == HirBodyKind::Expression {
                        let [edge] = projection.children() else {
                            return Err(RuntimePlanLowerError::new(
                                "expression body has an invalid root arity",
                            ));
                        };
                        let HirBodyChild::Expression(expression) = edge.child() else {
                            return Err(RuntimePlanLowerError::new(
                                "expression body has a statement root",
                            ));
                        };
                        flow.lower_flow_value(expression, RuntimeFlowValueContinuation::Return)?
                    } else {
                        let value_result = if matches!(
                            program.source(),
                            CheckedExecutionSource::ExportMutation(_)
                        ) {
                            let RuntimeTypeShape::Tuple(fields) = program.result().shape() else {
                                return Err(RuntimePlanLowerError::new(
                                    "mutation declaration has no result tuple",
                                ));
                            };
                            fields.first().ok_or_else(|| {
                                RuntimePlanLowerError::new("mutation declaration has no body value")
                            })?
                        } else {
                            program.result()
                        };
                        let mut ops = if matches!(value_result.shape(), RuntimeTypeShape::Unit) {
                            flow.apply_value_continuation(
                                RuntimeExprSeed::new(
                                    value_result.identity(),
                                    RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                                ),
                                RuntimeFlowValueContinuation::Return,
                            )?
                        } else {
                            Vec::new()
                        };
                        for edge in projection.children().iter().rev() {
                            let tail = RuntimeFlowTail::PreparedOps(ops.into_boxed_slice());
                            ops = match edge.child() {
                                HirBodyChild::Statement(statement) => {
                                    flow.lower_statement_ids_with_tail(&[statement], tail)?
                                }
                                HirBodyChild::Expression(expression) => {
                                    let continuation = if edge.role() == HirBodyChildRole::Tail {
                                        RuntimeFlowValueContinuation::Return
                                    } else {
                                        RuntimeFlowValueContinuation::Ignore(tail)
                                    };
                                    flow.lower_flow_value(expression, continuation)?
                                }
                            };
                        }
                        ops
                    }
                }
            };
            Ok(RuntimeFunctionSiteBodySeed::Executable(
                RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: ops.into_boxed_slice(),
                },
            ))
        })();
        match body.and_then(|body| {
            builder
                .define_function_site_seed(&definition.site, body)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))
        }) {
            Ok(()) => {}
            Err(error) => errors.push(error),
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
            let Some(fragment) = scope.dialogue_content_fragment_for_source(owner) else {
                errors.push(RuntimePlanLowerError::new(format!(
                "dialogue application {owner:?} content template is absent during capture admission"
            )));
                return;
            };
            for value in fragment.values() {
                let key = RuntimeDialogueValueCaptureKey::new(
                    application.content().template_id(),
                    value.slot(),
                    0,
                );
                if value_specs.insert(key, value.ty().identity()).is_some() {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "dialogue value {:?} repeats its slot result capture",
                        value.expression()
                    )));
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
            let locals = match context.executable_locals(scope.scope()) {
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
    Option<DialogueContentFragmentTemplate>,
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
    for fact in context.facts.format_templates() {
        let template = fact.template();
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
        if let Err(error) = builder.register_dialogue_content_template_seed(
            arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed {
                id: template.id(),
                digest: template.digest(),
                slots,
                effects: Box::default(),
            },
        ) {
            errors.push(RuntimePlanLowerError::new(format!(
                "formatter content template {} is invalid: {error}",
                template.id()
            )));
        }
    }
    let mut template_ids = BTreeSet::new();
    context
        .facts
        .visit_dialogue_content_fragments(&mut |_, fragment| {
            template_ids.insert(fragment.template().id());
        });
    template_ids.extend(
        context
            .facts
            .format_templates()
            .map(|fact| fact.template().id()),
    );
    let plain_text_context_template = if needs_plain_text_context_template(context.facts) {
        match arcweft_core::runtime_id::RuntimeDialogueContentTemplateId::from_zero_based(
            template_ids.len(),
        ) {
            Some(id) => match DialogueContentFragmentTemplate::plain_text_context(id) {
                Ok(template) => {
                    let seed = arcweft_core::plan::RuntimeDialogueContentTemplateManifestSeed {
                        id: template.id(),
                        digest: template.digest(),
                        slots: template
                            .slots()
                            .iter()
                            .map(|slot| arcweft_core::plan::RuntimeDialogueContentSlotSeed {
                                slot: slot.slot(),
                                role: slot.role(),
                                semantic_type: slot.semantic_type(),
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        effects: Box::new([]),
                    };
                    match builder.register_plain_text_context_template_seed(seed) {
                        Ok(identity)
                            if identity.id() == template.id()
                                && identity.digest() == template.digest() =>
                        {
                            Some(template)
                        }
                        Ok(_) => {
                            errors.push(RuntimePlanLowerError::new(
                                "plain-text context template registration changed its text-model identity or digest",
                            ));
                            None
                        }
                        Err(error) => {
                            errors.push(RuntimePlanLowerError::new(format!(
                                "plain-text context template {id} is invalid: {error}"
                            )));
                            None
                        }
                    }
                }
                Err(error) => {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "plain-text context template {id} is invalid: {error}"
                    )));
                    None
                }
            },
            None => {
                errors.push(RuntimePlanLowerError::new(
                    "plain-text context template identity space is exhausted",
                ));
                None
            }
        }
    } else {
        None
    };
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
                .map(|(site, function, callable_type, captures)| {
                    arcweft_core::plan::RuntimeDialogueEffectSiteSeed {
                        site,
                        function,
                        callable_type,
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
    (
        content_handles,
        assertion_sites,
        plain_text_context_template,
    )
}

fn needs_plain_text_context_template(facts: &RuntimePlanSemanticFacts) -> bool {
    let is_context_call = |call: &RuntimeResolvedCall| {
        matches!(
            call.dispatch(),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Intrinsic(
                RuntimeIntrinsic::StdOptionContext
                    | RuntimeIntrinsic::StdOptionWithContext
                    | RuntimeIntrinsic::StdResultContext
                    | RuntimeIntrinsic::StdResultWithContext
            ))
        )
    };
    if facts.calls().any(|(_, call)| is_context_call(call)) {
        return true;
    }
    if facts.project_function_instances().any(|instance| {
        let mut found = false;
        instance.visit_calls(&mut |_, call| found |= is_context_call(call));
        found
    }) {
        return true;
    }
    facts.root_closures().any(|closure| {
        let mut found = false;
        closure
            .semantics()
            .visit_calls(&mut |_, call| found |= is_context_call(call));
        found
    })
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
        let locals = match context.executable_locals(scope.scope()) {
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
                    Ok((local, input_local, capture.ty().identity(), capture.input_ownership(), capture.origin().runtime_input_origin().map_err(|error| error.to_string())?, arcweft_core::plan::RuntimeFunctionInputTransfer::Transferred(capture.transfer().runtime_capture_mode().expect("accepted effect capture is a value transfer"))))
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
                RuntimeEffectSet::try_from_effects(effect.effects().iter().cloned())
                    .map_err(|error| error.to_string());
            let capture_inputs = captures.and_then(|captures| {
                captures
                    .into_iter()
                    .enumerate()
                    .map(|(position, (binding, input_local, ty, ownership, origin, transfer))| {
                        let position = u32::try_from(position).map_err(|_| {
                            "dialogue content effect capture position exceeds checked limits"
                                .to_owned()
                        })?;
                        Ok(RuntimeFunctionInputBindingSeed {
                            transfer,
                            origin,
                            ownership,
                            unrestricted_bindings: if matches!(ownership, RuntimeFunctionInputOwnershipRequirement::Unrestricted) {
                                Box::new([binding.clone()])
                            } else {
                                Box::new([])
                            },
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
                            definition: effect.definition_identity().runtime_identity(),
                            role: arcweft_core::plan::RuntimeFunctionSemanticRole::Effect,
                            function_type: None,
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
                RuntimeEffectSet::try_from_effects(effect.effects().iter().cloned())
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
    assertions: &mut Vec<RuntimeAssertionSite>,
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
        let ops = match &definition.operation {
            RuntimeDialogueEffectOperationFact::EvaluatedEffect(effect) => {
                let expr = match context.scoped_expr_lowerer(module, scope) {
                    Ok(expr) => expr,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                match lower_evaluated_effect(&expr, effect.effect()) {
                    Ok(operation) => vec![RuntimeFlowOpSeed::EvaluatedEffect(operation)],
                    Err(error) => {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue content effect {:?} lowering failed: {error}",
                            definition.key
                        )));
                        continue;
                    }
                }
            }
            RuntimeDialogueEffectOperationFact::OrdinaryCall {
                root,
                application,
                result,
            } => {
                let control = match context.executable_control_locals(scope.scope()) {
                    Ok(control) => control,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let locals = match context.executable_locals(scope.scope()) {
                    Ok(locals) => locals,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let specialized_operand_locals =
                    match context.executable_specialized_operand_locals(scope.scope()) {
                        Ok(locals) => locals,
                        Err(error) => {
                            errors.push(error);
                            continue;
                        }
                    };
                let mut flow = FinalFlowLowerer::new(
                    module,
                    context,
                    RuntimeAssertionOwner::DialogueEffect(definition.key),
                )
                .with_executable_scope(
                    scope,
                    &control,
                    &locals,
                    &specialized_operand_locals,
                );
                let Some(call) = flow.call(*application) else {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "dialogue content effect {:?} application has no accepted call fact",
                        definition.key
                    )));
                    continue;
                };
                let application_type = match flow.expression_source_type(*application) {
                    Ok(ty) => ty,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                let root_type = match flow.expression_source_type(*root) {
                    Ok(ty) => ty,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                if !matches!(call.result(), RuntimeCallResultShape::Value)
                    || application_type.identity() != result.identity()
                    || root_type.identity() != result.identity()
                {
                    errors.push(RuntimePlanLowerError::new(format!(
                        "dialogue content effect {:?} ordinary call result disagrees with its accepted operation",
                        definition.key
                    )));
                    continue;
                }
                let ops = match flow.lower_flow_value(*root, RuntimeFlowValueContinuation::Return) {
                    Ok(ops) => ops,
                    Err(error) => {
                        errors.push(RuntimePlanLowerError::new(format!(
                            "dialogue content effect {:?} ordinary call lowering failed: {error}",
                            definition.key
                        )));
                        continue;
                    }
                };
                assertions.extend(flow.into_assertion_sites());
                ops
            }
        };
        let body = RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
            effects: definition.effects,
            ops: ops.into_boxed_slice(),
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
    let locals = match context.executable_locals(scope.scope()) {
        Ok(locals) => locals,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let mut invalid = false;
    let mut values = Vec::new();
    let mut value_definitions = Vec::new();
    // The Dialogue runtime still consumes typed value sites. Each site is an
    // identity callback over the caller's ANF result local; source expressions
    // are lowered once in the owning Flow body.
    for value in fragment.values() {
        let expression_type = match scope.expression_type(value.expression()) {
            Some(ty) if ty.identity() == value.source_type().identity() => value.ty().identity(),
            Some(_) => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue value {:?} source type disagrees with its accepted projection",
                    value.expression()
                )));
                invalid = true;
                continue;
            }
            None => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue value {:?} has no accepted source type",
                    value.expression()
                )));
                invalid = true;
                continue;
            }
        };
        let input_local = match context
            .dialogue_value_capture_input_locals
            .get(&RuntimeDialogueValueCaptureKey::new(
                application.content().template_id(),
                value.slot(),
                0,
            ))
            .cloned()
        {
            Some(local) => local,
            None => {
                errors.push(RuntimePlanLowerError::new(format!(
                    "dialogue value {:?} has no admitted result input local",
                    value.expression()
                )));
                invalid = true;
                continue;
            }
        };
        let Some(result_local) = context
            .dialogue_value_result_locals
            .get(&RuntimeDialogueValueCaptureKey::new(
                application.content().template_id(),
                value.slot(),
                0,
            ))
            .cloned()
        else {
            errors.push(RuntimePlanLowerError::new(format!(
                "dialogue value {:?} has no admitted caller result local",
                value.expression()
            )));
            invalid = true;
            continue;
        };
        let capture_inputs = [RuntimeFunctionInputBindingSeed {
            transfer: arcweft_core::plan::RuntimeFunctionInputTransfer::Transferred(
                arcweft_core::plan::RuntimeFunctionCaptureMode::Move,
            ),
            origin: arcweft_core::plan::RuntimeFunctionInputOrigin::EvaluatedResult(
                fragment
                    .value_callback_definition(value)
                    .expect("iterated accepted fragment slot"),
            ),
            ownership: RuntimeFunctionInputOwnershipRequirement::Owned,
            unrestricted_bindings: Box::new([]),
            source: RuntimeFunctionInputSource::Capture { position: 0 },
            input_local: input_local.clone(),
            pattern: RuntimePatternSeed::new(
                expression_type,
                RuntimePatternSeedKind::Bind {
                    mutable: false,
                    local: result_local.clone(),
                },
            ),
        }];
        let declaration = RuntimeFunctionSiteDeclarationSeed {
            definition: fragment
                .value_callback_definition(value)
                .expect("iterated accepted fragment slot"),
            role: arcweft_core::plan::RuntimeFunctionSemanticRole::Dialogue,
            function_type: None,
            inputs: capture_inputs.into(),
            result: expression_type,
            body_kind: RuntimeFunctionSiteBodyKind::Expression,
            effects: RuntimeEffectSet::empty(),
        };
        let body = RuntimeExprSeed::new(
            expression_type,
            arcweft_core::plan::RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                result_local.clone(),
                RuntimeLocalReadMode::Move,
            )),
        );
        let capture_values = vec![RuntimeExprSeed::new(
            expression_type,
            arcweft_core::plan::RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                result_local,
                RuntimeLocalReadMode::Move,
            )),
        )]
        .into_boxed_slice();
        match builder.reserve_function_site_seed(declaration) {
            Ok(site) => {
                values.push((
                    value.slot(),
                    value.role(),
                    site.clone(),
                    capture_values.clone(),
                ));
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
                            locals.get(&capture.local()).cloned().and_then(|local| {
                                let checked = capture.transfer();
                                (checked.local() == capture.local()).then(|| {
                                    RuntimeExprSeed::new(
                                        capture.ty().identity(),
                                        arcweft_core::plan::RuntimeExprSeedKind::Local(
                                            RuntimeLocalReadSeed::new(
                                                local,
                                                crate::final_expr::runtime_local_read_mode(
                                                    checked.mode(),
                                                ),
                                            ),
                                        ),
                                    )
                                })
                            })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some((
                        effect.site(),
                        function,
                        effect.callable_type().identity(),
                        captures.into_boxed_slice(),
                    ))
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
    let state = context
        .project_callable_states
        .get(root.instance())
        .and_then(|states| states.first())
        .cloned()
        .ok_or_else(|| RuntimePlanLowerError::new("entry controller callable state is absent"))?;
    let result_ty = match instance.function_type().shape() {
        RuntimeTypeShape::Function {
            result, parameters, ..
        } if parameters.is_empty()
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
            callee: RuntimeExprSeed::new(
                instance.callable_type().identity(),
                arcweft_core::plan::RuntimeExprSeedKind::MakeCallable {
                    state: state.clone(),
                    captures: Box::new([]),
                },
            ),
            state,
            completed_group: 0,
            operands: Box::new([]),
            ordinary: Box::new([]),
            attached: None,
        },
        result: result_pattern,
    };
    let ops = vec![
        project_call,
        RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
            result_ty,
            arcweft_core::plan::RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                result_local,
                RuntimeLocalReadMode::Move,
            )),
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
    let effects = RuntimeEffectSet::try_from_effects(instance.effects().iter().cloned())
        .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
    Ok(LoweredControllerCallable {
        flow: RuntimeFlowSeed::new(
            instance.definition_identity().runtime_identity(),
            flow.clone(),
            [],
            effects,
            ops,
        ),
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

fn local_seed(
    ty: &RuntimeNormalizedType,
    local: RuntimeLocalSeedId,
    mode: RuntimeLocalReadMode,
) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        ty.identity(),
        arcweft_core::plan::RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(local, mode)),
    )
}

fn module_by_id(
    project: HirAnalysisProjectView<'_>,
    expected: HirModuleId,
) -> Option<&Arc<HirModule>> {
    project
        .modules()
        .find_map(|(_, module)| (module.module_id() == expected).then_some(module))
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
    project: HirAnalysisProjectView<'_>,
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
            || facts
                .flow(flow.owner())
                .map(crate::semantic_facts::RuntimeFlowFact::identity)
                != Some(&flow.executable().flow)
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
    Program(arcweft_id::runtime_program::RuntimePureProgramId),
    ImplicitCallable(arcweft_lang_sema::final_analysis::CheckedImplicitCallableIdentity),
    Callable(CallableDeclarationId),
    Closure(arcweft_lang_sema::callable::CheckedClosureId),
    DialogueEffect(RuntimeDialogueEffectProgramKey),
    Flow(FlowRuntimeId),
    Line(RuntimeLineId),
    Defer(StmtId),
}

impl RuntimeAssertionOwner {
    fn label(&self) -> String {
        match self {
            Self::Program(program) => format!("program@{program}"),
            Self::ImplicitCallable(identity) => format!("implicit-callable@{identity:?}"),
            Self::Callable(declaration) => declaration.qualified_name(),
            Self::Closure(closure) => format!(
                "closure@{}:{}",
                closure.expression().source().id(),
                closure.expression().range().start()
            ),
            Self::DialogueEffect(program) => {
                format!("dialogue-effect@{}:{}", program.template(), program.site())
            }
            Self::Flow(flow) => flow.canonical_label(),
            Self::Line(line) => line.canonical_label(),
            Self::Defer(statement) => format!("defer@{statement:?}"),
        }
    }
}

/// Frame-owned projection of the accepted mutation output ABI. Every root
/// return publishes the body value first, then transfers each initialized
/// updated binding. No caller place is retained by the executable frame.
struct RuntimeMutationReturn {
    result: RuntimeSemanticTypeId,
    outputs: Box<[RuntimeExprSeed]>,
}

impl RuntimeMutationReturn {
    fn try_new(
        context: &FinalLoweringContext<'_, '_>,
        scope: RuntimeScopedExecutableSemanticFactView<'_>,
        bindings: &[arcweft_lang_sema::final_analysis::CheckedExecutableCapture],
        result: &crate::semantic_facts::RuntimeNormalizedType,
    ) -> Result<Self, RuntimePlanLowerError> {
        let RuntimeTypeShape::Tuple(fields) = result.shape() else {
            return Err(RuntimePlanLowerError::new(
                "mutation result has no output tuple",
            ));
        };
        if fields.len() != bindings.len() + 1 {
            return Err(RuntimePlanLowerError::new(
                "mutation output arity differs from its ABI",
            ));
        }
        let locals = context.executable_locals(scope.scope())?;
        let outputs = bindings
            .iter()
            .zip(fields.iter().skip(1))
            .map(|(binding, ty)| {
                let local = locals.get(&binding.local()).ok_or_else(|| {
                    RuntimePlanLowerError::new("mutation output has no admitted local")
                })?;
                Ok(RuntimeExprSeed::new(
                    ty.identity(),
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        local.clone(),
                        RuntimeLocalReadMode::Move,
                    )),
                ))
            })
            .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?
            .into_boxed_slice();
        Ok(Self {
            result: result.identity(),
            outputs,
        })
    }

    fn publish(&self, value: RuntimeExprSeed) -> RuntimeExprSeed {
        RuntimeExprSeed::new(
            self.result,
            RuntimeExprSeedKind::Tuple(
                std::iter::once(value)
                    .chain(self.outputs.iter().cloned())
                    .collect(),
            ),
        )
    }
}

struct FinalFlowLowerer<'a> {
    expression_overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    mutation_return: Option<RuntimeMutationReturn>,
    implicit_body_root: Option<ExprId>,
    module: &'a HirModule,
    facts: &'a RuntimePlanSemanticFacts,
    semantic_facts: RuntimeScopedExecutableSemanticFactView<'a>,
    package: &'a CallablePackageId,
    locals: &'a BTreeMap<LocalId, RuntimeLocalSeedId>,
    trait_methods: &'a BTreeMap<RuntimeTraitMethodInstanceKey, RuntimeTraitMethodSeedId>,
    format_attempts:
        &'a BTreeMap<crate::semantic_facts::RuntimeFormatTemplateKey, RuntimeFormatAttemptSeedId>,
    format_operand_source_locals: &'a BTreeMap<
        (
            crate::semantic_facts::RuntimeFormatTemplateKey,
            RuntimeFmtParameterId,
        ),
        RuntimeLocalSeedId,
    >,
    function_sites: &'a BTreeMap<RuntimeImplicitCallableSiteKey, RuntimeFunctionSiteSeedId>,
    defer_sites: &'a BTreeMap<StmtId, RuntimeDeferSiteId>,
    closure_sites: &'a BTreeMap<RuntimeClosureInstanceKey, RuntimeFunctionSiteSeedId>,
    project_callable_states: &'a callable_states::ProjectCallableStates,
    callable_sources: &'a callable_states::ProjectCallableSourceStates,
    callable_specializations: &'a callable_states::CallableSpecializationSeeds,
    callable_applications: &'a callable_states::ProjectCallableApplicationStates,
    callable_specialization_targets: &'a callable_states::CallableSpecializationTargetStates,
    dialogue_effect_sites: &'a BTreeMap<RuntimeDialogueEffectProgramKey, RuntimeFunctionSiteSeedId>,
    dialogue_value_result_locals: &'a BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    dialogue_value_project_source_locals:
        &'a BTreeMap<RuntimeDialogueValueCaptureKey, RuntimeLocalSeedId>,
    dialogue_content: &'a BTreeMap<
        arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
        RuntimeDialogueContentPlanSeedId,
    >,
    control: &'a ControlLocals,
    specialized_operand_locals: &'a BTreeMap<(ExprId, u32), RuntimeLocalSeedId>,
    carrier_continuations: BTreeMap<ExprId, RuntimeFlowValueContinuation>,
    result_selection_application: Option<ExprId>,
    result_selection_emitted: bool,
    scope_continuations: Vec<scopes::ScopeContinuationFrame>,
    assertion_owner: RuntimeAssertionOwner,
    assertion_ordinal: u32,
    assertion_sites: Vec<RuntimeAssertionSite>,
    flow_tail_worklists: Vec<FlowTailWorklist>,
}

#[derive(Clone)]
enum RuntimeFlowValueContinuation {
    For {
        pattern: RuntimePatternSeed,
        evidence: RuntimeIteratorEvidenceSeed,
        body: Vec<RuntimeFlowOpSeed>,
    },
    Match {
        arms: Vec<RuntimeFlowMatchArmSeed>,
    },
    If {
        then_ops: Vec<RuntimeFlowOpSeed>,
        else_ops: Vec<RuntimeFlowOpSeed>,
    },
    Specialize {
        owner: ExprId,
        outer: Box<Self>,
    },
    Bind {
        pattern: RuntimePatternSeed,
        tail: RuntimeFlowTail,
    },
    Assign {
        statement: StmtId,
    },
    Return,
    ExitScope {
        owner: ExprId,
        outer: Box<Self>,
    },
    ScopeSuccess {
        owner: RuntimeScopeOwner,
    },
    Ignore(RuntimeFlowTail),
    Try {
        owner: ExprId,
        outer: Box<Self>,
    },
    WrapCarrier {
        owner: ExprId,
        outer: Box<Self>,
    },
    Await {
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
    PreparedOps(Box<[RuntimeFlowOpSeed]>),
    StatementsWithTail {
        statements: Arc<[StmtId]>,
        next: usize,
        tail: Box<RuntimeFlowTail>,
    },
    ThreadItems {
        items: Arc<[HirThreadFlowItem]>,
        next: usize,
        tail: Box<Self>,
    },
    Value {
        expression: ExprId,
        continuation: Box<RuntimeFlowValueContinuation>,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    },
    SourceValue {
        expression: ExprId,
        continuation: Box<RuntimeFlowValueContinuation>,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    },
    ContinueValue {
        value: RuntimeExprSeed,
        continuation: Box<RuntimeFlowValueContinuation>,
    },
}

/// Deferred continuation work keeps long source-order tails off the native
/// Rust call stack, including expression operands and their value continuations.
/// Jobs are drained depth-first while the owning lexical
/// scope/carrier frames are still active, then their result trees are spliced
/// into the exact continuation holes that scheduled them.
#[derive(Default)]
struct FlowTailWorklist {
    next_id: usize,
    pending: Vec<FlowTailJob>,
    resolved: BTreeMap<usize, Vec<RuntimeFlowOpSeed>>,
}

#[derive(Clone)]
struct FlowTailJob {
    id: usize,
    tail: RuntimeFlowTail,
    scope_continuations: Vec<scopes::ScopeContinuationFrame>,
    carrier_continuations: BTreeMap<ExprId, RuntimeFlowValueContinuation>,
}

struct FlowTailFrame {
    id: Option<usize>,
    ops: Vec<RuntimeFlowOpSeed>,
    children: Vec<FlowTailJob>,
    next_child: usize,
}

fn resolve_flow_tail_holes(
    ops: Vec<RuntimeFlowOpSeed>,
    child_ids: &[usize],
    child_cursor: &mut usize,
    resolved: &mut BTreeMap<usize, Vec<RuntimeFlowOpSeed>>,
) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
    let mut output = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            RuntimeFlowOpSeed::Noop => {
                let id = child_ids.get(*child_cursor).copied().ok_or_else(|| {
                    RuntimePlanLowerError::new(
                        "flow continuation contains an unowned internal Noop hole",
                    )
                })?;
                *child_cursor += 1;
                output.extend(resolved.remove(&id).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "flow continuation job {id} was not resolved before its parent"
                    ))
                })?);
            }
            RuntimeFlowOpSeed::LetElse {
                pattern,
                expr,
                else_ops,
            } => output.push(RuntimeFlowOpSeed::LetElse {
                pattern,
                expr,
                else_ops: resolve_flow_tail_holes(else_ops, child_ids, child_cursor, resolved)?,
            }),
            RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt,
                parameter,
                body,
                value,
            } => output.push(RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt,
                parameter,
                body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
                value,
            }),
            RuntimeFlowOpSeed::Await {
                binding,
                target,
                observers,
            } => {
                let mut resolved_observers = Vec::with_capacity(observers.len());
                for observer in observers {
                    resolved_observers.push(RuntimeAwaitPendingObserverSeed {
                        pattern: observer.pattern,
                        ops: resolve_flow_tail_holes(
                            observer.ops,
                            child_ids,
                            child_cursor,
                            resolved,
                        )?,
                    });
                }
                output.push(RuntimeFlowOpSeed::Await {
                    binding,
                    target,
                    observers: resolved_observers,
                });
            }
            RuntimeFlowOpSeed::If {
                condition,
                then_ops,
                else_ops,
            } => output.push(RuntimeFlowOpSeed::If {
                condition,
                then_ops: resolve_flow_tail_holes(then_ops, child_ids, child_cursor, resolved)?,
                else_ops: resolve_flow_tail_holes(else_ops, child_ids, child_cursor, resolved)?,
            }),
            RuntimeFlowOpSeed::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => output.push(RuntimeFlowOpSeed::IfLet {
                pattern,
                expr,
                guard,
                then_ops: resolve_flow_tail_holes(then_ops, child_ids, child_cursor, resolved)?,
                else_ops: resolve_flow_tail_holes(else_ops, child_ids, child_cursor, resolved)?,
            }),
            RuntimeFlowOpSeed::Match { scrutinee, arms } => {
                let mut resolved_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    resolved_arms.push(RuntimeFlowMatchArmSeed {
                        pattern: arm.pattern,
                        guard: arm.guard,
                        ops: resolve_flow_tail_holes(arm.ops, child_ids, child_cursor, resolved)?,
                    });
                }
                output.push(RuntimeFlowOpSeed::Match {
                    scrutinee,
                    arms: resolved_arms,
                });
            }
            RuntimeFlowOpSeed::Loop { result, body } => {
                output.push(RuntimeFlowOpSeed::Loop {
                    result,
                    body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
                });
            }
            RuntimeFlowOpSeed::While { condition, body } => {
                output.push(RuntimeFlowOpSeed::While {
                    condition,
                    body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
                });
            }
            RuntimeFlowOpSeed::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => output.push(RuntimeFlowOpSeed::WhileLet {
                pattern,
                expr,
                guard,
                body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
            }),
            RuntimeFlowOpSeed::For {
                pattern,
                source,
                evidence,
                body,
            } => output.push(RuntimeFlowOpSeed::For {
                pattern,
                source,
                evidence,
                body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
            }),
            RuntimeFlowOpSeed::Thread {
                name,
                producer,
                captures,
                body,
            } => {
                output.push(RuntimeFlowOpSeed::Thread {
                    name,
                    producer,
                    captures,
                    body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
                });
            }
            RuntimeFlowOpSeed::Scope { identity, body } => {
                output.push(RuntimeFlowOpSeed::Scope {
                    identity,
                    body: resolve_flow_tail_holes(body, child_ids, child_cursor, resolved)?,
                });
            }
            terminal_or_leaf => output.push(terminal_or_leaf),
        }
    }
    Ok(output)
}

impl<'a> FinalFlowLowerer<'a> {
    fn new(
        module: &'a HirModule,
        context: &'a FinalLoweringContext<'_, '_>,
        assertion_owner: RuntimeAssertionOwner,
    ) -> Self {
        Self {
            module,
            expression_overrides: BTreeMap::new(),
            mutation_return: None,
            implicit_body_root: None,
            facts: context.facts,
            semantic_facts: RuntimeScopedExecutableSemanticFactView::global(context.facts),
            package: context.project.package(),
            locals: context.locals,
            trait_methods: context.trait_methods,
            format_attempts: context.format_attempts,
            format_operand_source_locals: context.format_operand_source_locals,
            function_sites: context.function_sites,
            defer_sites: context.defer_sites,
            closure_sites: context.closure_sites,
            project_callable_states: context.project_callable_states,
            callable_sources: context.callable_sources,
            callable_specializations: context.callable_specializations,
            callable_applications: context.callable_applications,
            callable_specialization_targets: context.callable_specialization_targets,
            dialogue_effect_sites: context.dialogue_effect_sites,
            dialogue_value_result_locals: context.dialogue_value_result_locals,
            dialogue_value_project_source_locals: context.dialogue_value_project_source_locals,
            dialogue_content: context.dialogue_content,
            control: context.control,
            specialized_operand_locals: context.specialized_operand_locals,
            carrier_continuations: BTreeMap::new(),
            result_selection_application: None,
            result_selection_emitted: false,
            scope_continuations: Vec::new(),
            assertion_owner,
            assertion_ordinal: 0,
            assertion_sites: Vec::new(),
            flow_tail_worklists: Vec::new(),
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

    fn with_executable_scope(
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
        .with_format_attempts(self.format_attempts)
        .with_project_callable_states(self.project_callable_states)
        .with_callable_sources(self.callable_sources)
        .with_callable_specializations(self.callable_specializations)
        .with_scope_locals(&self.control.scopes)
        .with_specialized_operand_locals(self.specialized_operand_locals);
        lowerer
            .with_scoped_semantics(self.semantic_facts)
            .with_overrides(self.expression_overrides.clone())
            .with_implicit_body_root(self.implicit_body_root)
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
    fn expression_source_type(
        &self,
        expression: ExprId,
    ) -> Result<&RuntimeNormalizedType, RuntimePlanLowerError> {
        self.semantic_facts
            .expression_source_type(expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "accepted source type is missing for expression {expression:?}"
                ))
            })
    }

    fn expression_children(&self, expression: ExprId) -> Result<&[ExprId], RuntimePlanLowerError> {
        self.semantic_facts
            .expression_children(expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "checked expression row is missing for {expression:?} in executable {:?}",
                    self.semantic_facts.scope()
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
        let place_receiver = self
            .call(expression)
            .and_then(RuntimeResolvedCall::mutation)
            .map(crate::semantic_facts::RuntimeResolvedCallMutation::source);
        let evaluated_call_target = if let Some(call) = self.call(expression) {
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
            if let Some(invocation) = invocation {
                let callee = invocation.callee().value_expression();
                let receiver = self
                    .module
                    .resolve_call_value_receiver(invocation)
                    .map_err(|error| {
                        RuntimePlanLowerError::new(format!(
                            "cannot resolve runtime receiver for call {expression:?}: {error}"
                        ))
                    })?;
                callee.and_then(|callee| {
                    if call.evaluates_callee(callee) {
                        Some((callee, callee))
                    } else {
                        receiver
                            .filter(|receiver| {
                                call.operands().iter().any(|operand| {
                                    matches!(
                                        (operand.origin(), operand.source()),
                                        (
                                            RuntimeResolvedCallOperandOrigin::Receiver,
                                            RuntimeResolvedCallOperandSource::Expression(source)
                                        ) if source == *receiver
                                    )
                                })
                            })
                            .map(|receiver| (callee, receiver))
                    }
                })
            } else {
                None
            }
        } else {
            None
        };
        let children = self.expression_children(expression)?;
        let mut evaluated = Vec::with_capacity(children.len());
        let selector = evaluated_call_target.map(|(selector, target)| {
            if selector == target && !children.contains(&selector) {
                return Err(RuntimePlanLowerError::new(format!(
                    "evaluated call selector {selector:?} is absent from its checked expression graph"
                )));
            }
            // A selected mutable receiver is an address. Its operand must
            // never be composed as a value before evaluating the RHS.
            if Some(target) != place_receiver {
                evaluated.push(target);
            }
            Ok(selector)
        }).transpose()?;
        evaluated.extend(
            children
                .iter()
                .copied()
                .filter(|child| Some(*child) != selector && Some(*child) != place_receiver),
        );
        Ok(evaluated)
    }

    fn value_expression_children(
        &self,
        expression: ExprId,
    ) -> Result<Vec<ExprId>, RuntimePlanLowerError> {
        if self.call(expression).is_some() {
            self.evaluated_expression_children(expression)
        } else {
            Ok(self.expression_children(expression)?.to_vec())
        }
    }

    fn evaluated_effect_children(
        &self,
        application: ExprId,
    ) -> Result<Vec<ExprId>, RuntimePlanLowerError> {
        let hir = self
            .module
            .resolve_expr(application)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let HirExprKind::Call(invocation) = hir.kind() else {
            return Err(RuntimePlanLowerError::new(format!(
                "checked effect application {application:?} is not a Call expression"
            )));
        };
        let static_callee = invocation.callee().value_expression();
        Ok(self
            .expression_children(application)?
            .iter()
            .copied()
            .filter(|child| Some(*child) != static_callee)
            .collect())
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

    fn evaluated_effect(&self, expression: ExprId) -> Option<&RuntimeEvaluatedEffectFact> {
        self.semantic_facts.evaluated_effect(expression)
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
        self_type: RuntimeSemanticTypeId,
    ) -> Result<RuntimeTraitMethodSeedId, RuntimePlanLowerError> {
        let key = RuntimeTraitMethodInstanceKey::new(declaration.clone(), self_type);
        self.trait_methods.get(&key).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "For statement {statement:?} refers to an unreserved trait method instance {key:?}"
            ))
        })
    }

    fn lower_body(
        &mut self,
        body: &HirThreadBody,
    ) -> Result<Vec<RuntimeFlowOpSeed>, Vec<RuntimePlanLowerError>> {
        let lowered = self.lower_thread_items(body.items());
        lowered.map_err(|error| vec![error])
    }

    fn lower_thread_items(
        &mut self,
        items: &[HirThreadFlowItem],
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_thread_items_with_tail(items, RuntimeFlowTail::None)
    }

    fn lower_thread_items_with_tail(
        &mut self,
        items: &[HirThreadFlowItem],
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_owned_flow_tail(RuntimeFlowTail::ThreadItems {
            items: Arc::from(items),
            next: 0,
            tail: Box::new(tail),
        })
    }

    fn lower_thread_items_with_tail_inline(
        &mut self,
        items: Arc<[HirThreadFlowItem]>,
        next_index: usize,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let Some(item) = items.get(next_index).cloned() else {
            return self.lower_flow_tail(tail);
        };
        let next = if next_index + 1 < items.len() {
            RuntimeFlowTail::ThreadItems {
                items: Arc::clone(&items),
                next: next_index + 1,
                tail: Box::new(tail),
            }
        } else {
            tail
        };
        self.lower_thread_item(&item, next)
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
                return self.lower_dialogue_value(
                    semantic_expression,
                    RuntimeFlowValueContinuation::Ignore(tail),
                    BTreeMap::new(),
                );
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
            HirStmtKind::Scope(scope) => self.lower_scope_statement(id, scope.body(), tail),
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
            HirStmtKind::Defer { .. } => Err(RuntimePlanLowerError::new(format!(
                "defer {id:?} requires scope-owned runtime registration"
            ))),
            HirStmtKind::Out { value, .. } => {
                let application = self.result_selection_application.ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "out {id:?} has no line-result selection owner"
                    ))
                })?;
                let admitted = self
                    .semantic_facts
                    .dialogue_application(application)
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "out {id:?} has no checked dialogue application"
                        ))
                    })?;
                if admitted.result_output_application(id) != Some(application)
                    || self.semantic_facts.expression_type(*value) != Some(admitted.line_result())
                {
                    return Err(RuntimePlanLowerError::new(format!(
                        "out {id:?} does not select its checked dialogue result"
                    )));
                }
                self.result_selection_emitted = true;
                Ok(vec![RuntimeFlowOpSeed::SelectDialogueResult {
                    value: self
                        .expr_lowerer()
                        .lower(*value)
                        .map_err(RuntimePlanLowerError::new)?,
                }])
            }
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
            HirStmtKind::Assign { value, .. } => {
                if self.contains_flow_value_expression(*value)? {
                    self.lower_flow_value(
                        *value,
                        RuntimeFlowValueContinuation::Assign { statement: id },
                    )
                } else {
                    Ok(vec![
                        self.expr_lowerer()
                            .lower_flow_assignment(id, *value)
                            .map_err(RuntimePlanLowerError::new)?,
                    ])
                }
            }
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
                    let value = self
                        .expr_lowerer()
                        .lower(*value)
                        .map_err(RuntimePlanLowerError::new)?;
                    self.apply_value_continuation(value, RuntimeFlowValueContinuation::Return)
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
                if let HirExprKind::Thread(thread_expr) = thread_expr.kind() {
                    if thread_expr.mode() == HirThreadMode::Detached {
                        return Err(RuntimePlanLowerError::new(format!(
                            "detached Thread expression {thread:?} requires typed runtime ownership metadata"
                        )));
                    }
                    let body = self.lower_body_as_one_error(thread_expr.body())?;
                    let captures = RuntimeFlowOpSeed::Scope {
                        identity: arcweft_core::scope::RuntimeScopeIdentity::Anonymous,
                        body: body.clone(),
                    }
                    .free_locals()
                    .into_vec();
                    let admission =
                        self.semantic_facts
                            .thread_producer(*thread)
                            .ok_or_else(|| {
                                RuntimePlanLowerError::new(
                                    "Thread has no accepted producer definition",
                                )
                            })?;
                    let payload = arcweft_core::pattern::RuntimeCheckedType::Unit;
                    let identity = payload.semantic_identity_digest();
                    let mut contract = blake3::Hasher::new();
                    contract.update(b"arcweft.thread.producer-contract.v1\0");
                    contract.update(identity.as_bytes());
                    let producer = arcweft_core::plan::RuntimeNeedProducerTemplateSeed {
                        family: arcweft_core::task::NeedProducerFamily::StructuredTaskPlan,
                        contract: arcweft_core::task::NeedProducerContractDigest::from_bytes(
                            *contract.finalize().as_bytes(),
                        ),
                        plan: admission.plan(),
                        producer_site: admission.site(),
                        payload_type: arcweft_core::task::RuntimeTypeSemanticDigest::from_bytes(
                            *identity.as_bytes(),
                        ),
                        class: arcweft_core::task::TaskClass::Cpu,
                        priority: arcweft_core::task::TaskPriority(0),
                        cancel_scope: arcweft_core::task::CancelScopeId("flow".into()),
                        policy: arcweft_core::task::TaskPolicy::AlwaysStart,
                        outcome: arcweft_core::task::TaskOutcomeContract::new(payload),
                        request: arcweft_core::plan::RuntimeHostTaskRequestTemplateSeed {
                            capability: arcweft_core::task::HostCapabilityId("flow_thread".into()),
                            operation: "run_child".into(),
                            args: Vec::new(),
                        },
                        debug_label: thread_expr
                            .name()
                            .map_or("anonymous", |name| name.as_str())
                            .to_owned(),
                    };
                    return Ok(vec![RuntimeFlowOpSeed::Thread {
                        name: thread_expr.name().map(|name| name.as_str().to_owned()),
                        producer,
                        captures,
                        body,
                    }]);
                }
                let ty = self.expression_source_type(*thread)?;
                if !matches!(ty.shape(), RuntimeTypeShape::Unit) {
                    return Err(RuntimePlanLowerError::new(format!(
                        "expression statement {id:?} has a non-Unit value without an explicit binding"
                    )));
                }
                let pattern =
                    RuntimePatternSeed::new(ty.identity(), RuntimePatternSeedKind::Discard);
                let expr = self
                    .expr_lowerer()
                    .lower_source(*thread)
                    .map_err(RuntimePlanLowerError::new)?;
                Ok(vec![RuntimeFlowOpSeed::Let { pattern, expr }])
            }
            HirStmtKind::Choice { choice } => self.lower_flow_value(
                *choice,
                RuntimeFlowValueContinuation::Ignore(RuntimeFlowTail::None),
            ),
            HirStmtKind::If(branch) => {
                let then_ops = self.lower_contextual_body(branch.then_body())?;
                let else_ops = branch
                    .else_branch()
                    .map(|branch| self.lower_else_branch(branch))
                    .transpose()?
                    .unwrap_or_default();
                self.lower_flow_value(
                    branch.condition(),
                    RuntimeFlowValueContinuation::If { then_ops, else_ops },
                )
            }
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
                            .lower_guard(guard)
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
                            self.lower_owned_flow_tail(RuntimeFlowTail::Value {
                                expression: *expression,
                                continuation: Box::new(RuntimeFlowValueContinuation::Ignore(
                                    RuntimeFlowTail::None,
                                )),
                                overrides: BTreeMap::new(),
                            })?
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
                                self.lower_match_guard(guard, matched.scrutinee(), BTreeMap::new())
                            })
                            .transpose()?,
                        ops,
                    });
                }
                self.lower_flow_value(
                    matched.scrutinee(),
                    RuntimeFlowValueContinuation::Match { arms },
                )
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
                            .lower_guard(guard)
                            .map_err(RuntimePlanLowerError::new)
                    })
                    .transpose()?,
                body: self.lower_contextual_body(while_stmt.body())?,
            }]),
            HirStmtKind::For(for_stmt) => {
                let body = self.lower_contextual_body(for_stmt.body())?;
                let body = if let Some(key) = for_stmt.key() {
                    let mut key_ops = self.lower_owned_flow_tail(RuntimeFlowTail::Value {
                        expression: key,
                        continuation: Box::new(RuntimeFlowValueContinuation::Bind {
                            pattern: RuntimePatternSeed::new(
                                self.expression_type(key)?.identity(),
                                RuntimePatternSeedKind::Discard,
                            ),
                            tail: RuntimeFlowTail::None,
                        }),
                        overrides: BTreeMap::new(),
                    })?;
                    key_ops.extend(body);
                    key_ops
                } else {
                    body
                };
                let continuation = self.lower_iteration_continuation(id, for_stmt, body)?;
                self.lower_flow_value(for_stmt.source(), continuation)
            }
            HirStmtKind::Scope(scope) => {
                self.lower_scope_statement(id, scope.body(), RuntimeFlowTail::None)
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

    fn lower_iteration_continuation(
        &self,
        id: StmtId,
        for_stmt: &arcweft_lang_hir::stmt::HirForStmt,
        body: Vec<RuntimeFlowOpSeed>,
    ) -> Result<RuntimeFlowValueContinuation, RuntimePlanLowerError> {
        Ok(RuntimeFlowValueContinuation::For {
            pattern: self
                .pattern_lowerer()
                .lower(for_stmt.pattern())
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
                    let source_type = self
                        .semantic_facts
                        .expression_type(for_stmt.source())
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "For statement {id:?} has no checked source type"
                            ))
                        })?;
                    let executable = match witness.executable() {
                        RuntimeIteratorWitnessExecutableFact::TraitCalls { into_iter, next } => {
                            RuntimeIteratorWitnessExecutableSeed::TraitCalls {
                                into_iter: self.trait_method(
                                    into_iter,
                                    id,
                                    source_type.identity(),
                                )?,
                                next: self.trait_method(next, id, witness.iterator().identity())?,
                            }
                        }
                        RuntimeIteratorWitnessExecutableFact::IdentityIntoIterator { next } => {
                            RuntimeIteratorWitnessExecutableSeed::IdentityIntoIterator {
                                next: self.trait_method(next, id, witness.iterator().identity())?,
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
            body,
        })
    }

    fn lower_defer_registration(
        &self,
        statement: StmtId,
        owner: RuntimeDeferOwner,
    ) -> Result<RuntimeFlowOpSeed, RuntimePlanLowerError> {
        let fact = self.semantic_facts.defer(statement).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "defer {statement:?} has no selected checked body fact"
            ))
        })?;
        let site = *self.defer_sites.get(&statement).ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "defer {statement:?} has no reserved executable site"
            ))
        })?;
        let captures = fact
            .captures()
            .iter()
            .map(|capture| {
                self.locals
                    .get(&capture.local())
                    .cloned()
                    .and_then(|local| {
                        let checked = capture.transfer();
                        (checked.local() == capture.local()).then(|| {
                            RuntimeExprSeed::new(
                                capture.ty().identity(),
                                arcweft_core::plan::RuntimeExprSeedKind::Local(
                                    RuntimeLocalReadSeed::new(
                                        local,
                                        crate::final_expr::runtime_local_read_mode(checked.mode()),
                                    ),
                                ),
                            )
                        })
                    })
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "defer {statement:?} capture {:?} has no admitted local",
                            capture.local()
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimeFlowOpSeed::RegisterDefer {
            site,
            outcome: fact.outcome(),
            captures,
            owner,
        })
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
        if self.implicit_body_root != Some(expression)
            && self.implicit_callable(expression).is_some()
        {
            return Ok(false);
        }
        if self.evaluated_effect(expression).is_some() {
            return Ok(true);
        }
        if self.call(expression).is_some_and(|call| {
            call.project_function().is_some()
                || matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
                || call.need_producer().is_some()
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
                | HirExprKind::Block(_)
                | HirExprKind::NamedBlock(_)
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
        for child in self.value_expression_children(expression)? {
            if self.contains_flow_value_expression(child)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn lower_match_guard(
        &mut self,
        owner: ExprId,
        scrutinee: ExprId,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<RuntimeFlowMatchGuardSeed, RuntimePlanLowerError> {
        let ty = self.expression_type(owner)?.clone();
        let candidate = self
            .control
            .expression_values
            .get(&scrutinee)
            .cloned()
            .ok_or_else(|| RuntimePlanLowerError::new("guard has no admitted candidate local"))?;
        let copy_locals = self
            .expr_lowerer()
            .guard_copy_locals(owner)
            .map_err(RuntimePlanLowerError::new)?
            .into_boxed_slice();
        let (ops, condition) = if matches!(ty.shape(), RuntimeTypeShape::Never) {
            (
                self.lower_owned_flow_tail(RuntimeFlowTail::Value {
                    expression: owner,
                    continuation: Box::new(RuntimeFlowValueContinuation::Ignore(
                        RuntimeFlowTail::None,
                    )),
                    overrides,
                })?,
                None,
            )
        } else {
            let result = self
                .control
                .guard_values
                .get(&owner)
                .cloned()
                .ok_or_else(|| RuntimePlanLowerError::new("guard has no admitted result local"))?;
            let ops = self.lower_owned_flow_tail(RuntimeFlowTail::Value {
                expression: owner,
                continuation: Box::new(RuntimeFlowValueContinuation::Bind {
                    pattern: bind_seed(&ty, result.clone()),
                    tail: RuntimeFlowTail::None,
                }),
                overrides,
            })?;
            (
                ops,
                Some(local_seed(&ty, result, RuntimeLocalReadMode::Move)),
            )
        };
        Ok(RuntimeFlowMatchGuardSeed {
            candidate,
            result: ty.identity(),
            condition,
            copy_locals,
            ops,
        })
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
        self.lower_flow_tail(RuntimeFlowTail::Value {
            expression,
            continuation: Box::new(continuation),
            overrides,
        })
    }

    fn lower_flow_value_with_overrides_inline(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        if let Some(value) = overrides.get(&expression) {
            return self.apply_value_continuation(value.clone(), continuation);
        }
        let continuation = if self
            .semantic_facts
            .expression_specialization(expression)
            .is_some()
        {
            RuntimeFlowValueContinuation::Specialize {
                owner: expression,
                outer: Box::new(continuation),
            }
        } else {
            continuation
        };
        self.lower_flow_value_source_with_overrides(expression, continuation, overrides)
    }

    fn is_format_attempt_call(&self, expression: ExprId) -> bool {
        let Some(call) = self.call(expression) else {
            return false;
        };
        let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(formatted)) =
            call.dispatch()
        else {
            return false;
        };
        let key = crate::semantic_facts::RuntimeFormatTemplateKey::for_call(
            self.semantic_facts.scope(),
            formatted,
        );
        self.format_attempts.contains_key(&key)
    }

    // Keep the selected call clone off the recursive source-lowering frame:
    // even unrelated callable expressions may nest deeply in that path.
    #[inline(never)]
    fn lower_selected_format_attempt(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let call = self.call(expression).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new("selected formatter attempt has no checked call")
        })?;
        let RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Format(formatted)) =
            call.dispatch()
        else {
            return Err(RuntimePlanLowerError::new(
                "selected formatter attempt has no format dispatch",
            ));
        };
        self.lower_format_attempt_call(expression, &call, formatted, continuation, overrides)
    }

    fn lower_flow_value_source_with_overrides(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_flow_tail(RuntimeFlowTail::SourceValue {
            expression,
            continuation: Box::new(continuation),
            overrides,
        })
    }

    fn lower_flow_value_source_with_overrides_inline(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
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
        if let Some(effect) = self.evaluated_effect(expression).cloned() {
            if let HirExprKind::Pipe(pipe) = self
                .module
                .resolve_expr(expression)
                .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?
                .kind()
                && !overrides.contains_key(&pipe.left())
            {
                let inherited = overrides.clone();
                return self.lower_flow_value_with_overrides(
                    pipe.left(),
                    RuntimeFlowValueContinuation::Pipe {
                        owner: expression,
                        right: expression,
                        overrides: inherited,
                        outer: Box::new(continuation),
                    },
                    overrides,
                );
            }
            for child in self.evaluated_effect_children(effect.application_site())? {
                if !overrides.contains_key(&child) && self.expression_literal(child).is_none() {
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
            let operation = lower_evaluated_effect(
                &self.expr_lowerer().with_overrides(overrides),
                effect.effect(),
            )?;
            let mut ops = vec![RuntimeFlowOpSeed::EvaluatedEffect(operation)];
            match effect.result().shape() {
                RuntimeTypeShape::Unit => {
                    let unit = RuntimeExprSeed::new(
                        effect.result().identity(),
                        arcweft_core::plan::RuntimeExprSeedKind::Value(RuntimeValue::Unit),
                    );
                    ops.extend(self.apply_value_continuation(unit, continuation)?);
                }
                RuntimeTypeShape::Never => {}
                other => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "checked evaluated effect {expression:?} has unsupported value result {other:?}"
                    )));
                }
            }
            return Ok(ops);
        }
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve flow value expression {expression:?}: {error}"
            ))
        })?;
        if self.is_format_attempt_call(expression) {
            return self.lower_selected_format_attempt(expression, continuation, overrides);
        }
        if self.call(expression).is_some_and(|call| {
            call.project_function().is_some()
                || matches!(call.dispatch(), RuntimeResolvedCallDispatch::Value { .. })
                || matches!(
                    call.dispatch(),
                    RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_))
                )
                || call.need_producer().is_some()
        }) {
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
            return self.lower_selected_callable_or_need_value(expression, continuation, overrides);
        }
        if self.implicit_body_root != Some(expression)
            && self.implicit_callable(expression).is_some()
        {
            let value = self
                .expr_lowerer()
                .lower_source(expression)
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
                self.lower_dialogue_value(expression, continuation, overrides)
            }
            HirExprKind::Try(operation) => self.lower_flow_value_with_overrides(
                operation.operand(),
                RuntimeFlowValueContinuation::Try {
                    owner: expression,
                    outer: Box::new(continuation),
                },
                overrides,
            ),
            HirExprKind::Await(awaited) => self.lower_flow_value_with_overrides(
                awaited.operand(),
                RuntimeFlowValueContinuation::Await {
                    owner: expression,
                    outer: Box::new(continuation),
                },
                overrides,
            ),
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
                let fact = self
                    .semantic_facts
                    .expression_scope(expression)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "Scope expression {expression:?} has no checked lexical identity"
                        ))
                    })?;
                if fact.continuation().is_some() {
                    return self.lower_scope_value(
                        expression,
                        &fact,
                        block.statements(),
                        block.tail(),
                        continuation,
                    );
                }
                let mut ops = vec![RuntimeFlowOpSeed::EnterScope {
                    identity: fact.identity().clone(),
                }];
                ops.extend(self.lower_value_block(
                    block.statements(),
                    block.tail(),
                    RuntimeFlowValueContinuation::ExitScope {
                        owner: expression,
                        outer: Box::new(continuation),
                    },
                )?);
                if matches!(
                    self.expression_type(expression)?.shape(),
                    RuntimeTypeShape::Never
                ) {
                    // A Never tail has no value continuation. Retain a balanced
                    // lexical fallthrough for resumable terminal operations;
                    // a real return/goto unwinds before reaching this marker.
                    ops.push(RuntimeFlowOpSeed::ExitScope);
                }
                Ok(ops)
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
                    .lower_source(expression)
                    .map_err(RuntimePlanLowerError::new)?;
                self.apply_value_continuation(value, continuation)
            }
        }
    }

    // A selected call can carry large checked application evidence. Keep its
    // owned clone off the recursive expression-lowering frame while child
    // calls are resolved through Compose continuations.
    #[inline(never)]
    fn lower_selected_callable_or_need_value(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let call = self.call(expression).cloned().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "selected callable expression {expression:?} has no checked call"
            ))
        })?;
        if call.need_producer().is_some() {
            return self.lower_need_producer_value(expression, &call, continuation, overrides);
        }
        if matches!(
            call.dispatch(),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(_))
        ) {
            return self.lower_host_call_value(expression, continuation, overrides);
        }
        self.lower_callable_value(expression, &call, continuation, overrides)
    }

    fn lower_callable_value(
        &mut self,
        expression: ExprId,
        call: &RuntimeResolvedCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let result_type = self.expression_source_type(expression)?.clone();
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
        ops.extend(self.apply_value_continuation(
            local_seed(&result_type, local, RuntimeLocalReadMode::Move),
            continuation,
        )?);
        Ok(ops)
    }

    fn lower_need_producer_value(
        &mut self,
        expression: ExprId,
        call: &RuntimeResolvedCall,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let producer = call.need_producer().ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "Need producer call {expression:?} has no selected runtime plan"
            ))
        })?;
        let result_type = self.expression_source_type(expression)?.clone();
        if result_type != *producer.need_type() {
            return Err(RuntimePlanLowerError::new(format!(
                "Need producer call {expression:?} result differs from its selected instantiated Need<T>"
            )));
        }
        let local = self
            .control
            .expression_values
            .get(&expression)
            .cloned()
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "Need producer call {expression:?} has no admitted result local"
                ))
            })?;
        let plan = producer.plan().clone();
        if call.operands().len() != plan.argument_types().len() {
            return Err(RuntimePlanLowerError::new(format!(
                "Need producer call {expression:?} source row differs from its selected argument signature"
            )));
        }
        let lowerer = self.expr_lowerer().with_overrides(overrides);
        let arguments = call
            .operands()
            .iter()
            .zip(plan.argument_types())
            .map(|(operand, expected)| {
                if operand.ty().identity() != *expected
                    || !matches!(
                        operand.projection(),
                        RuntimeResolvedCallOperandProjection::Scalar
                    )
                {
                    return Err(RuntimePlanLowerError::new(format!(
                        "Need producer call {expression:?} has an unchecked runtime argument"
                    )));
                }
                lowerer
                    .lower_scalar_operand_source(operand.source(), operand.ty())
                    .map_err(RuntimePlanLowerError::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut ops = vec![RuntimeFlowOpSeed::StartNeedProducer {
            binding: bind_seed(&result_type, local.clone()),
            target: arcweft_core::plan::RuntimeNeedProducerStartTargetSeed { plan, arguments },
        }];
        ops.extend(self.apply_value_continuation(
            local_seed(&result_type, local, RuntimeLocalReadMode::Move),
            continuation,
        )?);
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
        let state = if let Some(specialization) = checked.input_specialization() {
            self.callable_specialization_targets
                .get(&(specialization.key().clone(), specialization.source_digest()))
                .cloned()
        } else if let Some(instance) = checked.outcome().callable_instance() {
            self.project_callable_states
                .get(instance)
                .and_then(|states| states.get(call.completed_group().get()))
                .cloned()
        } else if let crate::semantic_facts::RuntimeProjectFunctionCallOutcome::Continue {
            target: crate::semantic_facts::RuntimeProjectCallableValueTarget::Source(source),
            ..
        } = checked.outcome()
        {
            self.callable_applications
                .get(&(source.clone(), checked.application_type().identity()))
                .cloned()
        } else {
            None
        }
        .ok_or_else(|| {
            RuntimePlanLowerError::new(format!(
                "project callable {expression:?} has no admitted group state"
            ))
        })?;
        let mut callee = match checked.input() {
            crate::semantic_facts::RuntimeProjectFunctionCallInput::Direct => RuntimeExprSeed::new(
                checked.application_type().identity(),
                arcweft_core::plan::RuntimeExprSeedKind::MakeCallable {
                    state: state.clone(),
                    captures: Box::new([]),
                },
            ),
            crate::semantic_facts::RuntimeProjectFunctionCallInput::Value { callee }
            | crate::semantic_facts::RuntimeProjectFunctionCallInput::Continuation {
                callee, ..
            } => lowerer.lower(*callee).map_err(RuntimePlanLowerError::new)?,
        };
        if let Some(specialization) = checked.input_specialization() {
            let seed = self
                .callable_specializations
                .get(specialization.key())
                .cloned()
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("project call input specialization is absent")
                })?;
            callee = RuntimeExprSeed::new(
                checked.application_type().identity(),
                arcweft_core::plan::RuntimeExprSeedKind::SpecializeCallable {
                    value: Box::new(callee),
                    specialization: seed,
                },
            );
        }
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
                    RuntimeResolvedAttachedContent::DefaultedOmitted { .. } => RuntimeProjectCallAttachedPresenceSeed::DefaultedOmitted,
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
        Ok(RuntimeProjectCallPlanSeed {
            callee,
            state,
            completed_group: u32::try_from(call.completed_group().get()).map_err(|_| {
                RuntimePlanLowerError::new(format!(
                    "project call {expression:?} group exceeds checked limits"
                ))
            })?,
            operands: operands.into_boxed_slice(),
            ordinary,
            attached,
        })
    }

    fn lower_dialogue_value(
        &mut self,
        expression: ExprId,
        continuation: RuntimeFlowValueContinuation,
        overrides: BTreeMap<ExprId, RuntimeExprSeed>,
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
        let fragment = self
            .semantic_facts
            .dialogue_content_fragment_for_source(semantic_expression)
            .ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "dialogue application {expression:?} has no checked content fragment"
                ))
            })?;
        if fragment.template().id() != application.content().template_id() {
            return Err(RuntimePlanLowerError::new(format!(
                "dialogue application {expression:?} content fragment disagrees with its accepted template"
            )));
        }
        let target_expression = application.target().expression();
        let Some(target) = overrides.get(&target_expression).cloned() else {
            // The target can contain an awaited call, branch, or other Flow
            // value. Complete its continuation before any content is started.
            return self.lower_flow_value_with_overrides(
                target_expression,
                RuntimeFlowValueContinuation::Compose {
                    owner: semantic_expression,
                    child: target_expression,
                    overrides,
                    outer: Box::new(continuation),
                },
                BTreeMap::new(),
            );
        };
        let target = match application.target() {
            RuntimeDialogueApplicationTarget::CharacterReference { dialogue_type, .. } => {
                RuntimeExprSeed::new(
                    dialogue_type.identity(),
                    arcweft_core::plan::RuntimeExprSeedKind::CharacterDialogue {
                        operation:
                            arcweft_interaction_model::dialogue::CharacterDialogueOperation::Factory,
                        target: Box::new(target),
                        fields: Box::new([]),
                    },
                )
            }
            RuntimeDialogueApplicationTarget::CharacterDialogue { .. } => target,
        };
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
            other => {
                let ty = application.line_result();
                let local = self
                    .control
                    .expression_values
                    .get(&expression)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue application {expression:?} has no admitted result local"
                        ))
                    })?;
                (
                    bind_seed(ty, local.clone()),
                    RuntimeFlowTail::ContinueValue {
                        value: local_seed(ty, local, RuntimeLocalReadMode::Move),
                        continuation: Box::new(other),
                    },
                )
            }
        };
        if pattern.ty() != application.line_result().identity() {
            return Err(RuntimePlanLowerError::new(format!(
                "dialogue application {expression:?} result pattern has the wrong accepted type"
            )));
        }
        // Slot expressions execute eagerly in canonical template order after
        // the target has completed. Dialogue value sites below read these
        // result locals and never lower the source expressions again.
        let mut ops = Vec::new();
        for value in fragment.values() {
            let value_type = self.expression_type(value.expression())?.clone();
            if value_type.identity() != value.source_type().identity() {
                return Err(RuntimePlanLowerError::new(format!(
                    "dialogue value {:?} source type disagrees with its accepted projection",
                    value.expression()
                )));
            }
            let local = self
                .dialogue_value_result_locals
                .get(&RuntimeDialogueValueCaptureKey::new(
                    application.content().template_id(),
                    value.slot(),
                    0,
                ))
                .cloned()
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "dialogue value {:?} has no admitted caller result local",
                        value.expression()
                    ))
                })?;
            if let Some(project) = value.project_display() {
                let (body, source) = if self.contains_flow_value_expression(value.expression())? {
                    let key = RuntimeDialogueValueCaptureKey::new(
                        application.content().template_id(),
                        value.slot(),
                        0,
                    );
                    let source_local = self
                        .dialogue_value_project_source_locals
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new(format!(
                                "dialogue value {:?} has no admitted project source local",
                                value.expression()
                            ))
                        })?;
                    let body = self.lower_flow_value(
                        value.expression(),
                        RuntimeFlowValueContinuation::Bind {
                            pattern: bind_seed(&value_type, source_local.clone()),
                            tail: RuntimeFlowTail::None,
                        },
                    )?;
                    (
                        body,
                        local_seed(&value_type, source_local, RuntimeLocalReadMode::Move),
                    )
                } else {
                    (
                        Vec::new(),
                        self.expr_lowerer()
                            .lower_source(value.expression())
                            .map_err(RuntimePlanLowerError::new)?,
                    )
                };
                let method = self
                    .trait_methods
                    .get(project.method())
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} has no selected DisplayText method",
                            value.expression()
                        ))
                    })?;
                let template = self
                    .facts
                    .format_template(project.template())
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} has no accepted formatter template",
                            value.expression()
                        ))
                    })?;
                let attempt = self
                    .format_attempts
                    .get(project.template())
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "dialogue value {:?} has no builder-issued formatter attempt",
                            value.expression()
                        ))
                    })?;
                ops.push(RuntimeFlowOpSeed::FormatOperandAttempt {
                    attempt: attempt.clone(),
                    parameter: RuntimeFmtParameterId::Value,
                    body,
                    value: source,
                });
                ops.push(RuntimeFlowOpSeed::Let {
                    pattern: bind_seed(value.ty(), local),
                    expr: RuntimeExprSeed::new(
                        value.ty().identity(),
                        RuntimeExprSeedKind::FormatContent {
                            template: template.template().id(),
                            attempt: Some(attempt),
                            operands: Box::new([]),
                            project_method: Some(method),
                            project_option: false,
                        },
                    ),
                });
            } else {
                ops.extend(self.lower_flow_value(
                    value.expression(),
                    RuntimeFlowValueContinuation::Bind {
                        pattern: bind_seed(&value_type, local),
                        tail: RuntimeFlowTail::None,
                    },
                )?);
            }
        }
        ops.push(RuntimeFlowOpSeed::Dialogue {
            target,
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
        });
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
                let ty = self.expression_source_type(owner)?;
                (
                    RuntimePatternSeed::new(ty.identity(), RuntimePatternSeedKind::Discard),
                    tail,
                )
            }
            other => {
                let ty = self.expression_source_type(owner)?;
                let local = self
                    .control
                    .expression_values
                    .get(&owner)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "Loop expression {owner:?} has no admitted result local"
                        ))
                    })?;
                (
                    bind_seed(ty, local.clone()),
                    RuntimeFlowTail::ContinueValue {
                        value: local_seed(ty, local, RuntimeLocalReadMode::Move),
                        continuation: Box::new(other),
                    },
                )
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
                overrides: BTreeMap::new(),
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
            overrides: BTreeMap::new(),
        };
        let lowered = self.lower_statement_ids_with_tail(block.statements(), tail);
        self.carrier_continuations.remove(&expression);
        lowered
    }

    fn lower_await_continuation(
        &mut self,
        expression: ExprId,
        source: RuntimeExprSeed,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let resolved = self.module.resolve_expr(expression).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "cannot resolve Await expression {expression:?}: {error}"
            ))
        })?;
        let HirExprKind::Await(awaited) = resolved.kind() else {
            return Err(RuntimePlanLowerError::new(format!(
                "Await continuation owner {expression:?} is not an Await expression"
            )));
        };
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
        let source_type = self.expression_type(awaited.operand())?;
        let RuntimeTypeShape::Need(item_type) = source_type.shape() else {
            return Err(RuntimePlanLowerError::new(format!(
                "Await operand {:?} is not a checked Need<T> value",
                awaited.operand()
            )));
        };
        let payload = self.expression_source_type(expression)?.clone();
        if source.ty() != source_type.identity() || payload.identity() != item_type.identity() {
            return Err(RuntimePlanLowerError::new(format!(
                "Await expression {expression:?} source or result type differs from its checked Need<T> contract"
            )));
        }
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
        let await_op = RuntimeFlowOpSeed::Await {
            binding: Some(bind_seed(&payload, locals.payload.clone())),
            target: arcweft_core::plan::RuntimeAwaitTargetSeed { source },
            observers,
        };
        let mut ops = vec![await_op];
        ops.extend(self.apply_value_continuation(
            local_seed(&payload, locals.payload, RuntimeLocalReadMode::Move),
            continuation,
        )?);
        Ok(ops)
    }

    fn apply_value_continuation(
        &mut self,
        value: RuntimeExprSeed,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.lower_flow_tail(RuntimeFlowTail::ContinueValue {
            value,
            continuation: Box::new(continuation),
        })
    }

    fn apply_value_continuation_inline(
        &mut self,
        value: RuntimeExprSeed,
        continuation: RuntimeFlowValueContinuation,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        Ok(match continuation {
            RuntimeFlowValueContinuation::For {
                pattern,
                evidence,
                body,
            } => vec![RuntimeFlowOpSeed::For {
                pattern,
                source: value,
                evidence,
                body,
            }],
            RuntimeFlowValueContinuation::Match { arms } => {
                vec![RuntimeFlowOpSeed::Match {
                    scrutinee: value,
                    arms,
                }]
            }
            RuntimeFlowValueContinuation::If { then_ops, else_ops } => {
                vec![RuntimeFlowOpSeed::If {
                    condition: value,
                    then_ops,
                    else_ops,
                }]
            }
            RuntimeFlowValueContinuation::Specialize { owner, outer } => {
                let value = self
                    .expr_lowerer()
                    .specialize_result(owner, value)
                    .map_err(RuntimePlanLowerError::new)?;
                return self.apply_value_continuation(value, *outer);
            }
            RuntimeFlowValueContinuation::Bind { pattern, tail } => {
                let mut ops = vec![RuntimeFlowOpSeed::Let {
                    pattern,
                    expr: value,
                }];
                ops.extend(self.lower_flow_tail(tail)?);
                ops
            }
            RuntimeFlowValueContinuation::Assign { statement } => vec![
                self.expr_lowerer()
                    .lower_flow_assignment_value(statement, value)
                    .map_err(RuntimePlanLowerError::new)?,
            ],
            RuntimeFlowValueContinuation::Return => {
                vec![RuntimeFlowOpSeed::ReturnExpr(match &self.mutation_return {
                    Some(publication) => publication.publish(value),
                    None => value,
                })]
            }
            RuntimeFlowValueContinuation::ScopeSuccess { owner } => {
                return self.complete_scope_success(owner, value);
            }
            RuntimeFlowValueContinuation::ExitScope { owner, outer } => {
                let ty = self.expression_source_type(owner)?;
                let local = self
                    .control
                    .expression_values
                    .get(&owner)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "Scope expression {owner:?} has no admitted result local"
                        ))
                    })?;
                let result = local_seed(ty, local.clone(), RuntimeLocalReadMode::Move);
                let mut ops = vec![RuntimeFlowOpSeed::ExitScopeBind {
                    pattern: bind_seed(ty, local),
                    expr: value,
                }];
                ops.extend(self.apply_value_continuation(result, *outer)?);
                ops
            }
            RuntimeFlowValueContinuation::Ignore(tail) => {
                let mut ops = vec![RuntimeFlowOpSeed::Let {
                    pattern: RuntimePatternSeed::new(value.ty(), RuntimePatternSeedKind::Discard),
                    expr: value,
                }];
                ops.extend(self.lower_flow_tail(tail)?);
                ops
            }
            RuntimeFlowValueContinuation::Try { owner, outer } => {
                return self.lower_try_continuation(owner, value, *outer);
            }
            RuntimeFlowValueContinuation::Await { owner, outer } => {
                return self.lower_await_continuation(owner, value, *outer);
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
                    .expression_final_values
                    .get(&child)
                    .or_else(|| self.control.expression_values.get(&child))
                    .cloned()
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(format!(
                            "evaluated expression {child:?} has no admitted value local"
                        ))
                    })?;
                let mut ops = if matches!(value.kind(), arcweft_core::plan::RuntimeExprSeedKind::Local(current) if current.local() == &local)
                {
                    Vec::new()
                } else {
                    vec![RuntimeFlowOpSeed::Let {
                        pattern: bind_seed(ty, local.clone()),
                        expr: value,
                    }]
                };
                overrides.insert(child, local_seed(ty, local, RuntimeLocalReadMode::Move));
                ops.extend(self.lower_flow_value_source_with_overrides(owner, *outer, overrides)?);
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
                let replacement = local_seed(local_type, local.clone(), RuntimeLocalReadMode::Move);
                let placeholder_uses = pipe
                    .placeholders()
                    .iter()
                    .map(|placeholder| {
                        let use_row = self
                            .semantic_facts
                            .checked_synthetic_use(*placeholder)
                            .ok_or_else(|| {
                                RuntimePlanLowerError::new(format!(
                                    "checked pipe use is missing for {placeholder:?}"
                                ))
                            })?;
                        if use_row.owner()
                            != arcweft_lang_sema::final_analysis::CheckedSyntheticUseOwner::Pipe(
                                pipe.binding_identity(),
                            )
                        {
                            return Err(RuntimePlanLowerError::new(format!(
                                "checked pipe use {placeholder:?} has another owner"
                            )));
                        }
                        Ok((
                            *placeholder,
                            local_seed(
                                local_type,
                                local.clone(),
                                crate::final_expr::runtime_local_read_mode(use_row.mode()),
                            ),
                        ))
                    })
                    .collect::<Result<Vec<_>, RuntimePlanLowerError>>()?;
                overrides.extend(placeholder_uses);
                if right == owner {
                    overrides.insert(pipe.left(), replacement);
                }
                let mut ops = vec![RuntimeFlowOpSeed::Let {
                    pattern: bind_seed(local_type, local),
                    expr: value,
                }];
                ops.extend(if right == owner {
                    self.lower_flow_value_source_with_overrides(right, *outer, overrides)?
                } else {
                    self.lower_flow_value_with_overrides(right, *outer, overrides)?
                });
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
        let success = local_seed(
            fact.carrier().success(),
            locals.success.clone(),
            RuntimeLocalReadMode::Move,
        );
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
                    Some(local_seed(residual, local, RuntimeLocalReadMode::Move)),
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
        self.propagate_scope_residual(fact.boundary(), fact.boundary_type(), residual)
    }

    fn propagate_scope_residual(
        &mut self,
        boundary: RuntimeTryBoundaryOwner,
        boundary_type: &RuntimeNormalizedType,
        residual: Option<RuntimeExprSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        if let Some(frame) = self
            .scope_continuations
            .iter()
            .rev()
            .find(|frame| frame.fact.boundary() == boundary)
            .cloned()
        {
            let carrier =
                normalized_variant_expression_seed(frame.fact.carrier_type(), 1, residual)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
            return self.complete_scope_carrier(frame.owner, carrier);
        }
        let propagated =
            normalized_variant_expression_seed(boundary_type, 1, residual).map_err(|error| {
                RuntimePlanLowerError::new(format!("Try residual is invalid: {error}"))
            })?;
        match boundary {
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

    // A completed fragment may be embedded in another continuation. Resolve
    // its jobs here so its construction holes never escape to the parent's tree.
    fn lower_owned_flow_tail(
        &mut self,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let prior_worklist_depth = self.flow_tail_worklists.len();
        self.flow_tail_worklists.push(FlowTailWorklist::default());
        let result = self
            .lower_flow_tail_inline(tail)
            .and_then(|ops| self.resolve_flow_tail_worklist(ops));
        self.flow_tail_worklists.truncate(prior_worklist_depth);
        result
    }

    fn lower_flow_tail(
        &mut self,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        if matches!(tail, RuntimeFlowTail::None) {
            return Ok(Vec::new());
        }
        let Some(worklist) = self.flow_tail_worklists.last_mut() else {
            return self.lower_owned_flow_tail(tail);
        };
        let id = worklist.next_id;
        worklist.next_id = id
            .checked_add(1)
            .ok_or_else(|| RuntimePlanLowerError::new("flow continuation worklist id overflow"))?;
        worklist.pending.push(FlowTailJob {
            id,
            tail,
            scope_continuations: self.scope_continuations.clone(),
            carrier_continuations: self.carrier_continuations.clone(),
        });
        // Noop is a private construction hole while this lowerer owns an
        // active worklist. resolve_flow_tail_worklist replaces every such hole
        // before the seed can leave final-flow lowering.
        Ok(vec![RuntimeFlowOpSeed::Noop])
    }

    fn lower_flow_tail_inline(
        &mut self,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        match tail {
            RuntimeFlowTail::None => Ok(Vec::new()),
            RuntimeFlowTail::PreparedOps(ops) => Ok(ops.into_vec()),
            RuntimeFlowTail::StatementsWithTail {
                statements,
                next,
                tail,
            } => self.lower_statement_ids_with_tail_inline(statements, next, *tail),
            RuntimeFlowTail::ThreadItems { items, next, tail } => {
                self.lower_thread_items_with_tail_inline(items, next, *tail)
            }
            RuntimeFlowTail::Value {
                expression,
                continuation,
                overrides,
            } => self.lower_flow_value_with_overrides_inline(expression, *continuation, overrides),
            RuntimeFlowTail::SourceValue {
                expression,
                continuation,
                overrides,
            } => self.lower_flow_value_source_with_overrides_inline(
                expression,
                *continuation,
                overrides,
            ),
            RuntimeFlowTail::ContinueValue {
                value,
                continuation,
            } => self.apply_value_continuation_inline(value, *continuation),
        }
    }

    fn resolve_flow_tail_worklist(
        &mut self,
        root_ops: Vec<RuntimeFlowOpSeed>,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let root_children = std::mem::take(
            &mut self
                .flow_tail_worklists
                .last_mut()
                .ok_or_else(|| RuntimePlanLowerError::new("flow worklist frame is absent"))?
                .pending,
        );
        let mut frames = vec![FlowTailFrame {
            id: None,
            ops: root_ops,
            children: root_children,
            next_child: 0,
        }];

        loop {
            let next_child = frames.last_mut().and_then(|frame| {
                let job = frame.children.get(frame.next_child).cloned()?;
                frame.next_child += 1;
                Some(job)
            });
            if let Some(job) = next_child {
                let previous_scopes =
                    std::mem::replace(&mut self.scope_continuations, job.scope_continuations);
                let previous_carriers =
                    std::mem::replace(&mut self.carrier_continuations, job.carrier_continuations);
                let lowered = self.lower_flow_tail_inline(job.tail);
                self.scope_continuations = previous_scopes;
                self.carrier_continuations = previous_carriers;
                let child_ops = lowered?;
                let child_jobs = std::mem::take(
                    &mut self
                        .flow_tail_worklists
                        .last_mut()
                        .ok_or_else(|| {
                            RuntimePlanLowerError::new("flow worklist frame disappeared")
                        })?
                        .pending,
                );
                frames.push(FlowTailFrame {
                    id: Some(job.id),
                    ops: child_ops,
                    children: child_jobs,
                    next_child: 0,
                });
                continue;
            }

            let frame = frames
                .pop()
                .ok_or_else(|| RuntimePlanLowerError::new("flow worklist frame underflow"))?;
            let child_ids = frame
                .children
                .iter()
                .map(|child| child.id)
                .collect::<Vec<_>>();
            let worklist = self
                .flow_tail_worklists
                .last_mut()
                .ok_or_else(|| RuntimePlanLowerError::new("flow worklist frame disappeared"))?;
            let mut child_cursor = 0;
            let resolved = resolve_flow_tail_holes(
                frame.ops,
                &child_ids,
                &mut child_cursor,
                &mut worklist.resolved,
            )?;
            if child_cursor != child_ids.len() {
                return Err(RuntimePlanLowerError::new(
                    "flow continuation worklist did not consume every child job",
                ));
            }
            if let Some(id) = frame.id {
                worklist.resolved.insert(id, resolved);
            } else {
                if !worklist.pending.is_empty() || !worklist.resolved.is_empty() {
                    return Err(RuntimePlanLowerError::new(
                        "flow continuation worklist retained unresolved internal jobs",
                    ));
                }
                return Ok(resolved);
            }
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
        ops.extend(self.apply_value_continuation(
            local_seed(result, local, RuntimeLocalReadMode::Move),
            continuation,
        )?);
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
                RuntimeAssertionOwner::Program(program) => {
                    crate::assertion_lower::derive_runtime_program_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        *program,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
                RuntimeAssertionOwner::ImplicitCallable(identity) => {
                    crate::assertion_lower::derive_runtime_implicit_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        *identity,
                        ordinal,
                        condition_index,
                        profile,
                    )
                }
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
                RuntimeAssertionOwner::DialogueEffect(program) => {
                    crate::assertion_lower::derive_runtime_dialogue_effect_assertion_guard(
                        self.package,
                        self.module.key().path(),
                        *program,
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
                RuntimeAssertionOwner::Defer(statement) => {
                    return Err(RuntimePlanLowerError::new(format!(
                        "defer {statement:?} assertion has no stable executable guard owner"
                    )));
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
        self.lower_owned_flow_tail(RuntimeFlowTail::StatementsWithTail {
            statements: Arc::from(statements),
            next: 0,
            tail: Box::new(tail),
        })
    }

    fn lower_statement_ids_with_tail_inline(
        &mut self,
        statements: Arc<[StmtId]>,
        next_index: usize,
        tail: RuntimeFlowTail,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        let Some(statement) = statements.get(next_index).copied() else {
            return self.lower_flow_tail(tail);
        };
        let kind = self.resolve_statement(statement)?.kind().clone();
        let next = if next_index + 1 < statements.len() {
            RuntimeFlowTail::StatementsWithTail {
                statements: Arc::clone(&statements),
                next: next_index + 1,
                tail: Box::new(tail),
            }
        } else {
            tail
        };
        self.lower_statement_with_tail(statement, &kind, next)
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

    fn lower_cancel_body(
        &mut self,
        application: ExprId,
        body: &HirThreadBody,
    ) -> Result<Vec<RuntimeFlowOpSeed>, RuntimePlanLowerError> {
        self.begin_dialogue_result_selection(application)?;
        let lowered = self.lower_body_as_one_error(body);
        self.finish_dialogue_result_selection(application)?;
        lowered
    }

    fn begin_dialogue_result_selection(
        &mut self,
        application: ExprId,
    ) -> Result<(), RuntimePlanLowerError> {
        if self.result_selection_application.is_some() {
            return Err(RuntimePlanLowerError::new(
                "nested dialogue result selection owner is ambiguous",
            ));
        }
        self.result_selection_application = Some(application);
        self.result_selection_emitted = false;
        Ok(())
    }

    fn finish_dialogue_result_selection(
        &mut self,
        application: ExprId,
    ) -> Result<bool, RuntimePlanLowerError> {
        if self.result_selection_application.take() != Some(application) {
            return Err(RuntimePlanLowerError::new(
                "dialogue result selection owner changed while lowering its body",
            ));
        }
        Ok(std::mem::take(&mut self.result_selection_emitted))
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
        HirSelectedCallExpressionDisposition, HirSelectedCallExpressionInventory,
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
        let executable = project.analysis_view().expect("executable fixture");
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = complete_type_input(&project);
        input.push_flow(
            owner,
            accepted_flow_fixture(&project, owner, identity.clone()),
        );
        let facts = runtime_facts(&project, input).expect("checked facts");
        let entry_input = RuntimeEntryLoweringInput::empty(executable);
        let report = lower_runtime_plan_with_stats(executable, &facts, &entry_input)
            .expect("empty Flow lowers");
        assert_eq!(report.plan.flows().len(), 1);
        assert_eq!(report.plan.flows()[0].id, identity);
        assert!(report.plan.flows()[0].body().ops().is_empty());
    }

    #[test]
    fn thread_expression_statement_lowers_through_the_sole_expression_owner() {
        let label = "thread-expression-statement";
        let source = "flow opening {\n    thread {\n    }\n}\n";
        let project = project_fixture(label, source);
        let executable = project.analysis_view().expect("executable fixture");
        let (_, module) = executable.modules().next().unwrap();
        let document = Arc::new(
            SourceDocument::try_new(
                module.provenance().source_identity().id().clone(),
                SourceName::path(format!("runtime-plan-final-flow-{label}.arcw")),
                source,
            )
            .unwrap(),
        );
        let registration = arcweft_lang_sema::registration::ProjectRegistrationFacts::try_new(
            ProjectSymbolWorldId::try_new(
                executable.package().clone(),
                document.identity().id().clone(),
                "runtime-plan-final-flow-test",
            )
            .unwrap(),
            vec![document],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let registered = arcweft_lang_sema::registration::CharacterRegistrar::register(
            arcweft_lang_sema::registration::CharacterRegistrationRequest::new(
                Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
                project.view(),
                &registration,
                None,
            ),
        )
        .unwrap();
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let analysis = arcweft_lang_sema::final_analysis::analyze_final_project(
            executable,
            registered.symbols(),
            arcweft_lang_sema::final_analysis::FinalSemanticCatalogs::production(&registered),
            arcweft_lang_sema::final_analysis::FinalSemanticAnalysisControl::new(&cancellation),
        )
        .unwrap();
        let owner = executable
            .items()
            .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
            .map(arcweft_lang_hir::project::HirProjectItemRef::id)
            .expect("Flow item");
        let identity = FlowRuntimeId::canonical("opening").expect("runtime Flow identity");
        let mut input = complete_type_input(&project);
        input.attach_checked_local_uses(Arc::clone(analysis.checked_local_uses()));
        for (owner, _) in analysis.expressions() {
            let expression = module.resolve_expr(owner).unwrap();
            if matches!(expression.kind(), HirExprKind::Thread(_)) {
                input.push_thread_producer(
                    owner,
                    analysis
                        .checked_thread_producer_admission(executable, registered.symbols(), owner)
                        .unwrap(),
                );
            }
        }
        input.push_flow(owner, accepted_flow_fixture(&project, owner, identity));
        let facts = runtime_facts(&project, input).expect("checked facts");
        let partition = analysis
            .execution_projection()
            .runtime_fact_partition(
                &runtime_reachability(&project),
                &HirRuntimeExecutableOwner::Item(owner),
            )
            .unwrap();
        let projected_types = partition.expressions().iter().map(|row| {
            let owner = crate::semantic_facts::RuntimeProjectFunctionTypeOwner::Expression(row.owner());
            if row.has_runtime_type() {
                crate::semantic_facts::RuntimeProjectFunctionTypeProjection::Value {
                    owner,
                    ty: RuntimeNormalizedType::new(
                        arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
                        RuntimeTypeShape::Unit,
                    ),
                }
            } else {
                crate::semantic_facts::RuntimeProjectFunctionTypeProjection::SemanticOnlyExpression { owner: row.owner() }
            }
        }).collect::<Box<[_]>>();
        let rows = partition
            .expressions()
            .iter()
            .map(|row| {
                assert_eq!(
                    row.producer_kind(),
                    Some(arcweft_lang_sema::CheckedExpressionProducerKind::Thread)
                );
                crate::semantic_facts::RuntimeProjectFunctionExpressionSemanticFact::new(
                    row.owner(),
                    row.children().into(),
                    crate::semantic_facts::RuntimeProjectFunctionExpressionPayload::Structural,
                )
            })
            .collect::<Box<[_]>>();
        let statements = partition
            .statements()
            .iter()
            .map(|row| {
                assert_eq!(row.family(), arcweft_lang_sema::final_analysis::CheckedExecutableRuntimeStatementFactFamily::Structural);
                crate::semantic_facts::RuntimeProjectFunctionStatementSemanticFact::new(
                    row.owner(),
                    crate::semantic_facts::RuntimeProjectFunctionStatementPayload::Structural,
                )
            })
            .collect::<Box<[_]>>();
        let authority = arcweft_lang_sema::final_analysis::CheckedLocalUseAuthority::Global(
            Arc::clone(analysis.checked_local_uses()),
        );
        assert!(matches!(
            crate::semantic_facts::RuntimeProjectFunctionInstanceSemanticFacts::try_new(
                partition.clone(),
                authority.clone(),
                projected_types.clone(),
                rows.clone(),
                Box::new([]),
                statements.clone(),
                Box::new([]),
            ),
            Err(crate::semantic_facts::RuntimeProjectFunctionFactError::NonCanonicalSemanticFacts)
        ));
        let proved_rows = rows
            .into_vec()
            .into_iter()
            .map(|row| {
                let producer = facts.thread_producer(row.owner()).unwrap().clone();
                row.with_producer(producer)
            })
            .collect();
        crate::semantic_facts::RuntimeProjectFunctionInstanceSemanticFacts::try_new(
            partition,
            authority,
            projected_types,
            proved_rows,
            Box::new([]),
            statements,
            Box::new([]),
        )
        .expect("the identical partition admits only after its required producers are restored");
        let report = lower_runtime_plan_with_stats(
            executable,
            &facts,
            &RuntimeEntryLoweringInput::empty(executable),
        )
        .expect("Thread expression statement lowers");

        let [FlowOp::Thread { name, body, .. }] = report.plan.flows()[0].body().ops() else {
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
        let executable = project.analysis_view().expect("executable fixture");
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
        fact_input.push_flow(
            flow_owner,
            accepted_flow_fixture(&project, flow_owner, flow.clone()),
        );
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
        let executable = project.analysis_view().expect("executable fixture");
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
                |owner| selected_call_inventory(executable, owner),
                |owner| retained_runtime_projection(executable, owner),
            )
            .expect("fixture reachability")
    }

    fn retained_runtime_projection(
        executable: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
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

    fn selected_call_inventory(
        executable: arcweft_lang_hir::project::HirAnalysisProjectView<'_>,
        owner: arcweft_lang_hir::identity::ExprId,
    ) -> Option<HirSelectedCallExpressionDisposition> {
        executable.modules().find_map(|(_, module)| {
            let expression = module.resolve_expr(owner).ok()?;
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            Some(HirSelectedCallExpressionDisposition::Callable(
                HirSelectedCallExpressionInventory::new(
                    call.arguments()
                        .iter()
                        .map(|argument| argument.value())
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    call.callee().value_expression(),
                ),
            ))
        })
    }

    fn accepted_flow_fixture(
        project: &arcweft_lang_hir::project::HirProject,
        owner: arcweft_lang_hir::identity::ItemId,
        identity: arcweft_core::plan::FlowRuntimeId,
    ) -> crate::semantic_facts::RuntimeFlowFact {
        use arcweft_lang_sema::{final_analysis::*, registration::*};
        let executable = project.analysis_view().unwrap();
        let documents = executable
            .modules()
            .map(|(_, module)| Arc::clone(module.provenance().document()))
            .collect::<Vec<_>>();
        let world = ProjectSymbolWorldId::try_new(
            executable.package().clone(),
            documents[0].identity().id().clone(),
            "runtime-plan-final-flow-test",
        )
        .unwrap();
        let registration =
            ProjectRegistrationFacts::try_new(world, documents, vec![], vec![], vec![]).unwrap();
        let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(arcweft_lang_sema::env::TypeCheckEnv::standard()),
            project.view(),
            &registration,
            None,
        ))
        .unwrap();
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let analysis = analyze_final_project(
            executable,
            registered.symbols(),
            FinalSemanticCatalogs::production(&registered),
            FinalSemanticAnalysisControl::new(&cancellation),
        )
        .unwrap();
        let definition = Arc::new(
            analysis
                .checked_flow_execution_definition(executable, registered.symbols(), owner)
                .unwrap(),
        );
        crate::semantic_facts::RuntimeFlowFact::try_new(identity, definition).unwrap()
    }

    fn runtime_facts(
        project: &HirProject,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<RuntimePlanSemanticFacts, crate::semantic_facts::RuntimeSemanticFactsError> {
        let executable = project.analysis_view().expect("executable fixture");
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
                super::super::semantic_facts::tests::fixture_local_origin(project, owner),
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
