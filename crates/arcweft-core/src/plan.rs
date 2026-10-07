pub(crate) mod body_semantic;
mod callable_specialization;
mod callable_states;
mod construction;
pub(crate) use construction::task_coordinates::{
    RuntimeTaskPlanBuildCoordinate, RuntimeTaskPlanCoordinateOwner,
};
mod control_effect;
mod dialogue_content;
pub mod entry_inventory;
mod executable_body;
mod flow_ops;
pub(crate) use flow_ops::{RuntimeFlowTreeEvent, try_visit_ops_events};
mod format_attempt;
mod function_inputs;
mod function_sites;
mod inventory;
pub use inventory::RuntimePlanInventory;
pub mod generation_contract;
mod local_declarations;
#[cfg(test)]
pub(crate) use local_declarations::RuntimeLocalDeclarationTableBuilder;
mod nominal_record_domains;
mod project_call;
mod task_semantic;
mod type_kind;
mod type_scope;
mod type_table;
mod value_admission;
mod variant_case;
mod variant_domains;

pub use callable_specialization::{
    RuntimeCallableSpecializationContext, RuntimeCallableSpecializationDefinition,
    RuntimeCallableSpecializationError, RuntimeCallableSpecializationState,
    RuntimeFunctionSpecializationArguments,
};
pub use callable_states::{
    RuntimeCallableAttachedContract, RuntimeCallableDefault, RuntimeCallableInputSource,
    RuntimeCallableParameterCoordinate, RuntimeCallableParameterInput,
    RuntimeCallableParameterKind, RuntimeCallablePartialTransition, RuntimeCallablePosition,
    RuntimeCallableRetainedInput, RuntimeCallableRetainedRole, RuntimeCallableState,
    RuntimeCallableStateDefinition, RuntimeCallableStateError, RuntimeCallableStateTable,
    RuntimeCallableTransition,
};
pub use construction::{
    RuntimeAgentExprSeed, RuntimeAssignmentSeed, RuntimeAudioCommandSeed,
    RuntimeAwaitManyTargetSeed, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed,
    RuntimeBorrowedLocalSeed, RuntimeBuiltinIteratorEvidenceSeed, RuntimeCallArgumentSeed,
    RuntimeCallableExecutableSeed, RuntimeCallableExecutableSeedCode, RuntimeCallableParameterSeed,
    RuntimeCallableSpecializationSeed, RuntimeCallableSpecializationSeedId,
    RuntimeCallableStateSeed, RuntimeCallableStateSeedId, RuntimeChoiceOptionSeed,
    RuntimeDialogueContentEffectBindingSeed, RuntimeDialogueContentEffectSlotSeed,
    RuntimeDialogueContentPlanSeed, RuntimeDialogueContentPlanSeedId,
    RuntimeDialogueContentSlotSeed, RuntimeDialogueContentTemplateManifestSeed,
    RuntimeDialogueEffectSiteSeed, RuntimeDialogueMarkSeedId, RuntimeDialogueResultTargetSeed,
    RuntimeDialogueResultTargetSeedError, RuntimeDialogueValueSiteSeed, RuntimeDropPolicySeed,
    RuntimeEffectFieldSeed, RuntimeEvaluatedEffectSeed, RuntimeExecutableBodySeed,
    RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFieldProjectionSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowMatchGuardSeed, RuntimeFlowOpSeed, RuntimeFlowSeed,
    RuntimeFormatAttemptDeclarationSeed, RuntimeFormatAttemptOperandSeed,
    RuntimeFormatAttemptSeedId, RuntimeFormatContentOperandSeed, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeHostTaskRequestTemplateSeed,
    RuntimeIteratorEvidenceSeed, RuntimeIteratorWitnessEvidenceSeed,
    RuntimeIteratorWitnessExecutableSeed, RuntimeLineEffectSeed, RuntimeLineHandleSiteSeed,
    RuntimeLineOperationSeed, RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskGroupSeed,
    RuntimeLineTaskGroupSeedId, RuntimeLineTaskNodeSeed, RuntimeLineTaskNodeSeedId,
    RuntimeLineTaskTriggerSeed, RuntimeLocalDeclarationSeed, RuntimeLocalReadSeed,
    RuntimeLocalSeedId, RuntimeMutablePlaceSeed, RuntimeNeedProducerStartTargetSeed,
    RuntimeNeedProducerTemplateSeed, RuntimeNominalRecordFieldSeed, RuntimePatternRestSeed,
    RuntimePatternSeed, RuntimePatternSeedKind, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanNominalSchemaError, RuntimePlanSchemaComponent, RuntimePlanSemanticAdmission,
    RuntimePlanTable, RuntimePureHelperDeclarationSeed, RuntimePureHelperSeed,
    RuntimePureHelperSeedId, RuntimePureProgramBindingSeed, RuntimeRecordFieldSeedId,
    RuntimeRecordPatternFieldSeed, RuntimeScheduledCaptureSeed, RuntimeStreamMatchArmSeed,
    RuntimeStreamOpSeed, RuntimeStreamPlanSeed, RuntimeTraitMethodDeclarationSeed,
    RuntimeTraitMethodSeed, RuntimeTraitMethodSeedId,
};
pub use construction::{RuntimeControlEffectContractSeed, RuntimeControlEffectContractSeedId};
pub use control_effect::{
    ControlEffectContractDigest, RuntimeControlEffectCancellation, RuntimeControlEffectCardinality,
    RuntimeControlEffectContract, RuntimeControlEffectContractDefinition,
    RuntimeControlEffectContractError, RuntimeControlEffectContractId,
    RuntimeControlEffectContractTable, RuntimeControlEffectIdentity, RuntimeControlEffectKind,
    RuntimeControlEffectOrdering, RuntimeControlEffectRow, RuntimeControlEffectTerminalBehavior,
    RuntimeTaskControlMode,
};
pub use dialogue_content::{
    RuntimeDialogueContentApplicationKey, RuntimeDialogueContentEffectSlot,
    RuntimeDialogueContentEffectTrigger, RuntimeDialogueContentPlan,
    RuntimeDialogueContentPlanTable, RuntimeDialogueContentPlanTableError,
    RuntimeDialogueContentSlot, RuntimeDialogueContentTemplateManifest,
    RuntimeDialogueContentTemplateManifestError, RuntimeDialogueContentTemplateManifestTable,
    RuntimeDialogueEffectSite, RuntimeDialogueMark, RuntimeDialogueValueRole,
    RuntimeDialogueValueSite,
};
pub use executable_body::{RuntimeEffectSet, RuntimeEffectSetError, RuntimeExecutableBody};
pub use flow_ops::{RuntimeFlowBodyRole, RuntimeFlowOwnedBodies, RuntimeFlowValueRole};
pub use format_attempt::{
    RuntimeFormatAttempt, RuntimeFormatAttemptOperand, RuntimeFormatAttemptTable,
};
pub use function_sites::{
    RuntimeFunctionCaptureMode, RuntimeFunctionDefinitionIdentity, RuntimeFunctionInputBinding,
    RuntimeFunctionInputOrigin, RuntimeFunctionInputOwnershipRequirement,
    RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole, RuntimeFunctionSite,
    RuntimeFunctionSiteBody, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteError,
    RuntimeFunctionSiteTable, RuntimeGeneratedFunctionRole,
};
pub use generation_contract::{
    CharacterDialogueRuntimeCustomFieldDigest, RuntimeCharacterCatalogDigest,
    RuntimeGenerationIdentity, RuntimeProducerRootId, RuntimeProjectRootId,
    RuntimeViewCatalogDigest, RuntimeViewId,
};
pub use local_declarations::{
    RuntimeGeneratedLocalOrigin, RuntimeLocalDeclaration, RuntimeLocalDeclarationTable,
    RuntimeLocalDeclarationTableError, RuntimeLocalOrigin,
};
pub use nominal_record_domains::{
    RuntimeNominalRecordDomain, RuntimeNominalRecordDomainError, RuntimeNominalRecordDomainField,
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed,
    RuntimeNominalRecordDomainTable,
};
pub use project_call::{
    RuntimeProjectCallAttachedMaterialization, RuntimeProjectCallAttachedMaterializationSeed,
    RuntimeProjectCallAttachedPresence, RuntimeProjectCallAttachedPresenceSeed,
    RuntimeProjectCallFixedMaterialization, RuntimeProjectCallFixedMaterializationSeed,
    RuntimeProjectCallOperand, RuntimeProjectCallOperandSeed,
    RuntimeProjectCallOrdinaryMaterialization, RuntimeProjectCallOrdinaryMaterializationSeed,
    RuntimeProjectCallPlan, RuntimeProjectCallPlanError, RuntimeProjectCallPlanSeed,
    RuntimeProjectCallRestMaterialization, RuntimeProjectCallRestMaterializationSeed,
    RuntimeProjectCallSite, RuntimeProjectCallSiteTable, RuntimeProjectCallSiteTableError,
};
pub use task_semantic::RuntimeTaskPlanSealLimits;
pub use type_kind::{
    RuntimeAgentOperationalType, RuntimeAgentTypeProjection, RuntimeOperationalType,
    RuntimePlanRecordField, RuntimePlanSequenceKind, RuntimePlanTypeClass,
    RuntimePlanTypeProjection,
};
pub use type_scope::{
    RuntimeArrayLength, RuntimeBoundConstReference, RuntimeBoundEffectReference,
    RuntimeBoundTypeReference, RuntimeFunctionTypeContract, RuntimeTypeBinder, RuntimeTypeScope,
    RuntimeTypeScopeError,
};
pub use type_table::{
    MAX_RUNTIME_PLAN_TYPE_DEPTH, RuntimePlanTypeDeclaration, RuntimePlanTypeResolutionError,
    RuntimePlanTypeSeed, RuntimePlanTypeTable, RuntimePlanTypeTableError,
};
pub use value_admission::RuntimePlanValueAdmissionError;
pub use variant_case::{RuntimePlanVariantCase, RuntimePlanVariantCaseError};
pub use variant_domains::{
    RuntimeVariantCase, RuntimeVariantCaseSeed, RuntimeVariantDomain, RuntimeVariantDomainError,
    RuntimeVariantDomainSeed, RuntimeVariantDomainTable,
};

use crate::effect::{LineEffectRequest, RuntimeEffectExpr};
pub use crate::entry::{
    AgentBudget, AgentPolicyHash, CallableContractHash, EntryBindingIdentity, FlowContractHash,
    FlowParameterCoordinate, RuntimeAgentEntryRoles, RuntimeCallableExecutable,
    RuntimeCallableExecutableCode, RuntimeCallableId, RuntimeCallableRole,
    RuntimeCommandConstructorId, RuntimeCommandContract, RuntimeCommandPolicy,
    RuntimeCommandTargetId, RuntimeEntryRoles, RuntimeFlowExecutable,
    RuntimeFlowExecutableParameter, RuntimeFlowParameterMode, RuntimeFlowRole, RuntimeFlowSchema,
    RuntimeNominalRole, RuntimeNominalTypeId, RuntimeSchemaField, RuntimeSchemaLimits,
    RuntimeSchemaVariant, RuntimeStatefulEntryRoles, RuntimeTypeSchema, RuntimeValueDigest,
    TypeLayoutHash,
};
use crate::line_task::{LineOutRequest, LineTaskGroup};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeCheckedRecordTypeError, RuntimeCheckedType,
    RuntimeCheckedVariantCase, RuntimeOpaqueTypeOwner, RuntimePattern, RuntimeSemanticTypeId,
};
pub use crate::runtime_id::RuntimeProjectCallSiteId;
use crate::runtime_id::{
    RuntimeDialogueValueSlotId, RuntimeIdError, RuntimeIdFamily, RuntimeIdPath,
    RuntimeLocalDeclarationId, RuntimePlanTypeId, RuntimePublicLabel,
};
use crate::step::RuntimeHostCallMode;
use crate::stream::StreamPlan;
use crate::task::{
    AwaitManyTarget, NeedId, NeedProducerTaskPlan, RuntimeHostArgumentTemplate, TaskId,
};

use crate::value::{RuntimeExpr, RuntimeIterator, RuntimeLocalBinding, RuntimePayload};
pub use entry_inventory::{
    EntryRuntimeId, RouteCaptureCoordinate, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlowInvocation, RuntimeFlowInvocationError, RuntimeHttpMethod, RuntimePlanError,
    RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRoutePath, RuntimeRoutePathError,
    RuntimeRoutePathSegment, RuntimeRouteSpec,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlan {
    pub(crate) artifact: Option<crate::effect::RuntimeArtifactFingerprint>,
    pub(crate) inventory: RuntimePlanInventory,
}

impl std::ops::Deref for RuntimePlan {
    type Target = RuntimePlanInventory;

    fn deref(&self) -> &Self::Target {
        &self.inventory
    }
}

/// Failure to resolve the plan-owned semantic type of a runtime value.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanValueTypeError {
    #[error("runtime value references unknown plan type {ty}")]
    UnknownType {
        ty: crate::runtime_id::RuntimePlanTypeId,
    },
}

impl RuntimePlan {
    #[must_use]
    pub const fn control_effect_contracts(&self) -> &RuntimeControlEffectContractTable {
        self.inventory.control_effect_contracts()
    }

    #[must_use]
    pub const fn callable_states(&self) -> &RuntimeCallableStateTable {
        self.inventory.callable_states()
    }

    #[must_use]
    pub const fn callable_specializations(
        &self,
    ) -> &[RuntimeCallableSpecializationDefinition<
        RuntimePlanTypeId,
        crate::runtime_id::RuntimeCallableStateId,
    >] {
        self.inventory.callable_specializations()
    }

    /// Binds the in-memory plan to the exact accepted persisted artifact.
    /// Rebinding to a different generation is rejected rather than silently
    /// changing every dialogue-handle identity.
    pub fn bind_artifact(
        &mut self,
        artifact: crate::effect::RuntimeArtifactFingerprint,
    ) -> Result<(), RuntimePlanArtifactError> {
        match self.artifact {
            Some(existing) if existing != artifact => {
                Err(RuntimePlanArtifactError::AlreadyBound { existing, artifact })
            }
            Some(_) => Ok(()),
            None => {
                self.artifact = Some(artifact);
                Ok(())
            }
        }
    }

    #[must_use]
    pub const fn artifact(&self) -> Option<crate::effect::RuntimeArtifactFingerprint> {
        self.artifact
    }

    #[must_use]
    pub const fn type_table(&self) -> &RuntimePlanTypeTable {
        self.inventory.type_table()
    }

    #[must_use]
    pub const fn local_declarations(&self) -> &RuntimeLocalDeclarationTable {
        self.inventory.local_declarations()
    }

    #[must_use]
    pub const fn nominal_record_domains(&self) -> &RuntimeNominalRecordDomainTable {
        self.inventory.nominal_record_domains()
    }

    #[must_use]
    pub const fn variant_domains(&self) -> &RuntimeVariantDomainTable {
        self.inventory.variant_domains()
    }

    #[must_use]
    pub const fn function_sites(&self) -> &RuntimeFunctionSiteTable {
        self.inventory.function_sites()
    }

    #[must_use]
    pub const fn format_attempts(&self) -> &RuntimeFormatAttemptTable {
        self.inventory.format_attempts()
    }

    #[must_use]
    pub fn format_attempt(
        &self,
        id: crate::runtime_id::RuntimeFormatAttemptId,
    ) -> Option<&RuntimeFormatAttempt> {
        self.inventory.format_attempt(id)
    }

    #[must_use]
    pub fn defer_function_site(
        &self,
        site: crate::runtime_id::RuntimeDeferSiteId,
    ) -> Option<crate::runtime_id::RuntimeFunctionSiteId> {
        self.inventory.defer_function_site(site)
    }

    #[must_use]
    pub fn defer_sites(&self) -> &[crate::runtime_id::RuntimeFunctionSiteId] {
        self.inventory.defer_sites()
    }

    #[must_use]
    pub const fn project_call_sites(&self) -> &RuntimeProjectCallSiteTable {
        self.inventory.project_call_sites()
    }

    #[must_use]
    pub const fn dialogue_content(&self) -> &RuntimeDialogueContentPlanTable {
        self.inventory.dialogue_content()
    }

    #[must_use]
    pub const fn dialogue_content_templates(&self) -> &RuntimeDialogueContentTemplateManifestTable {
        self.inventory.dialogue_content_templates()
    }

    /// Records the non-serialized plain-text Content proof after a
    /// RuntimePlan/catalog owner join has validated the canonical template.
    pub fn accept_plain_text_context_template_proof(
        &mut self,
        proof: crate::value::RuntimeDialoguePlainTextContextTemplateProof,
    ) -> Result<(), RuntimeDialogueContentPlanTableError> {
        self.inventory
            .dialogue_content
            .accept_plain_text_context_template_proof(proof)
    }

    #[must_use]
    pub fn entries(&self) -> &[RuntimeEntrySpec] {
        self.inventory.entries()
    }

    #[must_use]
    pub fn callable_executables(&self) -> &[RuntimeCallableExecutable] {
        self.inventory.callable_executables()
    }

    #[must_use]
    pub fn flow_executables(&self) -> &[RuntimeFlowExecutable] {
        self.inventory.flow_executables()
    }

    #[must_use]
    pub fn flow_schemas(&self) -> &[RuntimeFlowSchema] {
        self.inventory.flow_schemas()
    }

    #[must_use]
    pub fn flows(&self) -> &[RuntimeFlow] {
        self.inventory.flows()
    }

    #[must_use]
    pub fn pure_helpers(&self) -> &[RuntimePureHelper] {
        self.inventory.pure_helpers()
    }

    #[must_use]
    pub fn pure_programs(&self) -> &[RuntimePureProgramBinding] {
        self.inventory.pure_programs()
    }

    #[must_use]
    pub fn trait_methods(&self) -> &[RuntimeTraitMethod] {
        self.inventory.trait_methods()
    }

    #[must_use]
    pub fn line_task_groups(&self) -> &[LineTaskGroup] {
        self.inventory.line_task_groups()
    }

    #[must_use]
    pub fn stream_plans(&self) -> &[StreamPlan] {
        self.inventory.stream_plans()
    }

    /// Derives the complete checked predicate in plan context. Nominal enum
    /// cases come from the owner-keyed variant domain rather than a copied
    /// type-row sidecar.
    pub fn checked_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        self.inventory.checked_type(ty)
    }

    /// Derives the final execution class from the complete plan-owned graph.
    pub fn type_class(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeClass, RuntimePlanTypeResolutionError> {
        self.inventory.type_class(ty)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanArtifactError {
    #[error("runtime plan is already bound to artifact {existing:?}, not {artifact:?}")]
    AlreadyBound {
        existing: crate::effect::RuntimeArtifactFingerprint,
        artifact: crate::effect::RuntimeArtifactFingerprint,
    },
}

/// Runtime identifier for a lowered flow.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct FlowRuntimeId {
    path: RuntimeIdPath,
    public_label: RuntimePublicLabel,
}

/// Dynamic runtime Flow target lookup failure.
///
/// Runtime-authored text may select an accepted manual canonical identity
/// exactly, or select one checked/generated declaration through its unique
/// public label. It never reconstructs a checked/generated semantic identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeFlowTargetError {
    #[error(transparent)]
    Invalid(#[from] RuntimeIdError),
    #[error("runtime Flow target `{target}` is not present in the accepted plan")]
    Missing { target: String },
    #[error("runtime Flow target `{target}` matches {matches} accepted declarations")]
    Ambiguous { target: String, matches: usize },
}

/// Runtime identifier for a lowered dialogue line.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeLineId {
    path: RuntimeIdPath,
}

/// Lowered flow program.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFlow {
    pub id: FlowRuntimeId,
    /// Derived invocation index; the immutable function row remains authority.
    pub params: Box<[RuntimeLocalDeclarationId]>,
    function_site: crate::runtime_id::RuntimeFunctionSiteId,
    function: std::sync::Arc<RuntimeFunctionSite>,
}

impl RuntimeFlow {
    pub(crate) fn matches_parameter_contract(&self, schema: &RuntimeFlowSchema) -> bool {
        schema.flow == self.id
            && self.function.inputs().len() == schema.parameters.len()
            && self
                .function
                .inputs()
                .iter()
                .zip(&schema.parameters)
                .all(|(input, formal)| {
                    input.origin() == RuntimeFunctionInputOrigin::Parameter(formal.identity)
                        && input.source()
                            == RuntimeFunctionInputSource::Parameter {
                                position: formal.coordinate.position(),
                                passing: formal.passing,
                            }
                        && input.ownership() == RuntimeFunctionInputOwnershipRequirement::Owned
                        && formal.mode == crate::entry::RuntimeFlowParameterMode::Owned
                })
    }

    #[must_use]
    pub const fn function_site(&self) -> crate::runtime_id::RuntimeFunctionSiteId {
        self.function_site
    }

    #[must_use]
    pub fn function(&self) -> &RuntimeFunctionSite {
        &self.function
    }

    #[must_use]
    pub fn definition(&self) -> RuntimeFunctionDefinitionIdentity {
        self.function.definition()
    }

    /// # Panics
    /// Panics only if the admitted Flow ownership invariant was corrupted.
    #[must_use]
    pub fn body(&self) -> &RuntimeExecutableBody {
        self.function
            .body()
            .executable()
            .expect("admitted Flow owns an executable function row")
    }
}

impl FlowRuntimeId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Flow, value).map(Self::from_runtime_path)
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Flow,
            value,
            RuntimeIdFamily::flow_source_families(),
        )
        .map(Self::from_runtime_path)
    }

    pub fn from_runtime_target_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::flow_source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    /// Projects one accepted structural Flow declaration into a one-way
    /// runtime identity while retaining its separately selected public label.
    pub fn from_checked_declaration_digest(
        digest: [u8; 32],
        public_id: &str,
    ) -> Result<Self, RuntimeIdError> {
        let public_label = Self::from_source_entity_body(public_id)?.public_label;
        Ok(Self {
            path: RuntimeIdPath::for_checked_flow_declaration(digest),
            public_label,
        })
    }

    pub(crate) fn from_runtime_contract(
        identity: &str,
        public_id: &str,
    ) -> Result<Self, RuntimeIdError> {
        let path = RuntimeIdPath::from_runtime_contract_str(RuntimeIdFamily::Flow, identity)?;
        // The contract stores diagnostic text separately from its identity.
        // Generated controller labels are valid runtime labels even though
        // their reserved segments cannot be authored as source Flow names.
        let public_label = RuntimePublicLabel::new(public_id);
        Ok(Self { path, public_label })
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        self.public_label.clone()
    }

    #[must_use]
    pub(crate) const fn public_label_ref(&self) -> &RuntimePublicLabel {
        &self.public_label
    }

    /// Selects one exact accepted Flow identity for a runtime-authored target.
    ///
    /// Canonical identities admitted by the public/manual `RuntimePlan`
    /// boundary remain exact. Public labels select only when exactly one
    /// accepted declaration owns that label; checked/generated semantic
    /// identity is never reconstructed from runtime-authored text.
    pub fn resolve_runtime_target<'a>(
        value: &str,
        candidates: impl IntoIterator<Item = &'a Self>,
    ) -> Result<&'a Self, RuntimeFlowTargetError> {
        let projected = Self::from_runtime_target_value(value)?;
        let public_label = projected.public_label();
        let mut public_match = None;
        let mut public_matches = 0_usize;
        for candidate in candidates {
            if *candidate == projected {
                return Ok(candidate);
            }
            if candidate.public_label() == public_label {
                public_matches = public_matches.saturating_add(1);
                public_match.get_or_insert(candidate);
            }
        }
        match (public_match, public_matches) {
            (Some(candidate), 1) => Ok(candidate),
            (None, _) => Err(RuntimeFlowTargetError::Missing {
                target: value.to_owned(),
            }),
            (Some(_), matches) => Err(RuntimeFlowTargetError::Ambiguous {
                target: value.to_owned(),
                matches,
            }),
        }
    }

    fn from_runtime_path(path: RuntimeIdPath) -> Self {
        let public_label = RuntimePublicLabel::for_family(RuntimeIdFamily::Flow, &path);
        Self { path, public_label }
    }
}

impl fmt::Display for FlowRuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

impl RuntimeLineId {
    pub fn canonical(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_canonical_str(RuntimeIdFamily::Line, value).map(|path| Self { path })
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        RuntimeIdPath::from_source_entity_body(
            RuntimeIdFamily::Line,
            value,
            RuntimeIdFamily::Line.source_families(),
        )
        .map(|path| Self { path })
    }

    pub fn from_runtime_line_value(value: &str) -> Result<Self, RuntimeIdError> {
        let Some((family, _)) = value.split_once('.') else {
            return Self::canonical(value);
        };
        if RuntimeIdFamily::Line.source_families().contains(&family) {
            Self::from_source_entity_body(value)
        } else {
            Self::canonical(value)
        }
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }

    #[must_use]
    pub fn canonical_label(&self) -> String {
        self.path.label()
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Line, &self.path)
    }
}

impl fmt::Display for RuntimeLineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.fmt(f)
    }
}

/// Runtime identifier for a lowered deterministic pure helper.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimePureHelperId(pub usize);

/// Lowered deterministic pure helper callable from runtime expressions.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePureHelper {
    pub definition: RuntimeFunctionDefinitionIdentity,
    pub id: RuntimePureHelperId,
    pub name: String,
    pub inputs: Box<[RuntimeCallableParameter]>,
    pub output_type: RuntimePureOutputType,
    pub expr: RuntimeExpr,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

/// Exact stable program identity mapped to its plan-owned function frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePureProgramBinding {
    program: arcweft_id::runtime_program::RuntimePureProgramId,
    site: crate::runtime_id::RuntimeFunctionSiteId,
    function_type: Option<RuntimeSemanticTypeId>,
    input_types: Box<[RuntimeSemanticTypeId]>,
    result_type: RuntimeSemanticTypeId,
}

impl RuntimePureProgramBinding {
    #[must_use]
    pub(crate) fn new(
        program: arcweft_id::runtime_program::RuntimePureProgramId,
        site: crate::runtime_id::RuntimeFunctionSiteId,
        function_type: Option<RuntimeSemanticTypeId>,
        input_types: impl Into<Box<[RuntimeSemanticTypeId]>>,
        result_type: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            program,
            site,
            function_type,
            input_types: input_types.into(),
            result_type,
        }
    }

    #[must_use]
    pub const fn program(&self) -> arcweft_id::runtime_program::RuntimePureProgramId {
        self.program
    }

    #[must_use]
    pub const fn site(&self) -> crate::runtime_id::RuntimeFunctionSiteId {
        self.site
    }

    /// Quantified frame owner; absence denotes the fixed root-scope ABI.
    #[must_use]
    pub const fn function_type(&self) -> Option<RuntimeSemanticTypeId> {
        self.function_type
    }

    /// Ordered semantic input identities retained across the Value ABI.
    #[must_use]
    pub const fn input_types(&self) -> &[RuntimeSemanticTypeId] {
        &self.input_types
    }

    /// Exact semantic result identity retained across the Value ABI.
    #[must_use]
    pub const fn result_type(&self) -> RuntimeSemanticTypeId {
        self.result_type
    }
}

/// Runtime identifier for a lowered trait/impl method body.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeTraitMethodId(pub usize);

/// Receiver ownership mode selected by the surface method signature.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeReceiverMode {
    Owned,
    SharedRef,
    MutRef,
}

/// Stable identity of a concrete trait method selected through a sema witness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeTraitMethodIdentity {
    pub impl_id: usize,
    pub trait_id: Option<usize>,
    pub witness: Option<usize>,
    pub trait_name: Option<String>,
    pub self_type: String,
    pub method_name: String,
    pub monomorph_label: String,
}

/// Lowered deterministic trait/impl method body callable by runtime dispatch.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTraitMethod {
    pub definition: RuntimeFunctionDefinitionIdentity,
    pub id: RuntimeTraitMethodId,
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub inputs: Box<[RuntimeCallableParameter]>,
    pub output_type: RuntimePureOutputType,
    pub body: RuntimeExpr,
}

/// One directly bound callable formal keeps its frame local, static passing
/// and physical representation together. A method receiver is its first row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuntimeCallableParameter {
    identity: RuntimeFunctionParameterIdentity,
    local: RuntimeLocalDeclarationId,
    passing: RuntimeFunctionParameterPassing,
    abi: RuntimePureInputType,
}

impl RuntimeCallableParameter {
    pub(crate) const fn new(
        identity: RuntimeFunctionParameterIdentity,
        local: RuntimeLocalDeclarationId,
        passing: RuntimeFunctionParameterPassing,
        abi: RuntimePureInputType,
    ) -> Self {
        Self {
            identity,
            local,
            passing,
            abi,
        }
    }

    pub const fn identity(self) -> RuntimeFunctionParameterIdentity {
        self.identity
    }
    pub const fn local(self) -> RuntimeLocalDeclarationId {
        self.local
    }
    pub const fn passing(self) -> RuntimeFunctionParameterPassing {
        self.passing
    }
    pub const fn abi(self) -> RuntimePureInputType {
        self.abi
    }
}

/// Runtime pure helper input representation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureInputType {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    Value,
}

/// Runtime pure helper output representation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureOutputType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    Value,
}

/// Source of a runtime pure helper candidate.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimePureHelperOrigin {
    Annotated,
    Inferred,
}

/// Serializable evidence that a `for` source was resolved through the standard
/// `IntoIterator` / `Iterator` contract before runtime lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorEvidence {
    Builtin(RuntimeBuiltinIteratorEvidence),
    Witness(RuntimeIteratorWitnessEvidence),
}

/// Built-in iterator family selected by checked language semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RuntimeBuiltinIteratorFamily {
    Range,
    Seq,
    Stream,
    Vec,
    Array,
    Slice,
    TupleHomogeneous,
}

/// Plan-owned ABI rows for one built-in iterator execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBuiltinIteratorEvidence {
    pub family: RuntimeBuiltinIteratorFamily,
    pub item: RuntimePlanTypeId,
    pub iterator: RuntimePlanTypeId,
    pub next_value: RuntimePlanTypeId,
    pub step: RuntimePlanTypeId,
}

/// Lowered witness-backed iterator evidence.
///
/// Runtime dispatch can execute trait-call witnesses; AWBC lowering still
/// requires a typed trait-method table before it can consume them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIteratorWitnessEvidence {
    pub item: RuntimePlanTypeId,
    pub iterator: RuntimePlanTypeId,
    pub executable: RuntimeIteratorWitnessExecutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorWitnessExecutable {
    TraitCalls {
        into_iter: RuntimeTraitMethodId,
        next: RuntimeTraitMethodId,
    },
    IdentityIntoIterator {
        next: RuntimeTraitMethodId,
    },
}

impl RuntimeIteratorEvidence {
    #[must_use]
    pub const fn builtin(evidence: RuntimeBuiltinIteratorEvidence) -> Self {
        Self::Builtin(evidence)
    }
}

/// The runtime scope that owns a reached deferred registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeDeferOwner {
    /// Unwinds with the current lexical fiber scope.
    CurrentScope,
    /// Survives activation and unwinds after the line's joined children.
    LineRoot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlowOp {
    Bind(Vec<RuntimeLocalBinding>),
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    /// Runs one checked source-ordered formatter operand body inside the
    /// owning attempt's recoverable failure boundary.
    FormatOperandAttempt {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
        body: Vec<FlowOp>,
        value: RuntimeExpr,
    },
    /// Engine-only completion marker appended after a formatter operand body.
    /// It never appears in a finished `RuntimePlan`.
    CompleteFormatOperand {
        attempt: crate::runtime_id::RuntimeFormatAttemptId,
        parameter: crate::value::RuntimeFmtParameterId,
        value: RuntimeExpr,
    },
    LetElse {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        else_ops: Vec<FlowOp>,
    },
    Assign {
        place: crate::value::RuntimeAssignment,
        value: RuntimeExpr,
    },
    LineOperation {
        binding: Option<RuntimePattern>,
        operation: RuntimeLineOperation,
    },
    CommitDialogueResult {
        value: RuntimeExpr,
    },
    /// Selects the alternate result from the owning cancellation handler.
    SelectDialogueResult {
        value: RuntimeExpr,
    },
    Dialogue {
        target: RuntimeExpr,
        content: crate::runtime_id::RuntimeDialogueContentPlanId,
        result: RuntimeDialogueResultTarget,
    },
    Choice {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    Await {
        binding: Option<RuntimePattern>,
        target: RuntimeNeedAwaitTarget,
        observers: Vec<RuntimeAwaitPendingObserver>,
    },
    StartNeedProducer {
        binding: RuntimePattern,
        target: RuntimeNeedProducerStartTarget,
    },
    AwaitMany {
        binding: Option<RuntimePattern>,
        target: AwaitManyTarget,
        pending: Vec<LineEffectRequest>,
    },
    HostCall {
        binding: Option<RuntimePattern>,
        target: RuntimeHostCallTarget,
    },
    /// Typed project-function control transfer. The site catalog row owns the
    /// result pattern together with the reusable ABI/materialization plan;
    /// this operation carries only the checked site identity.
    ProjectCall {
        site: crate::runtime_id::RuntimeProjectCallSiteId,
    },
    /// Applies a function value in the current fiber, retaining its return
    /// binding while an executable body is running or suspended.
    ApplyGroup {
        callee: RuntimeExpr,
        args: Vec<crate::value::RuntimeCallArgument>,
        result: RuntimePattern,
    },
    If {
        condition: RuntimeExpr,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    IfLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        then_ops: Vec<FlowOp>,
        else_ops: Vec<FlowOp>,
    },
    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<RuntimeMatchArm>,
    },
    Loop {
        result: Option<RuntimePattern>,
        body: Vec<FlowOp>,
    },
    LoopNext {
        body: Arc<[FlowOp]>,
    },
    While {
        condition: RuntimeExpr,
        body: Vec<FlowOp>,
    },
    WhileNext {
        condition: RuntimeExpr,
        body: Arc<[FlowOp]>,
    },
    WhileLet {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Vec<FlowOp>,
    },
    WhileLetNext {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        guard: Option<RuntimeExpr>,
        body: Arc<[FlowOp]>,
    },
    For {
        pattern: RuntimePattern,
        source: RuntimeExpr,
        evidence: RuntimeIteratorEvidence,
        body: Vec<FlowOp>,
    },
    ForNext {
        pattern: RuntimePattern,
        iterator: RuntimeIterator,
        evidence: RuntimeIteratorEvidence,
        body: Arc<[FlowOp]>,
    },
    Thread {
        name: Option<String>,
        producer: crate::task::NeedProducerTemplate,
        captures: Vec<RuntimeLocalDeclarationId>,
        body: Vec<FlowOp>,
    },
    Scope {
        identity: crate::scope::RuntimeScopeIdentity,
        body: Vec<FlowOp>,
    },
    LetScope {
        identity: crate::scope::RuntimeScopeIdentity,
        pattern: RuntimePattern,
        ops: Vec<FlowOp>,
        value: RuntimeExpr,
    },
    Break(Option<RuntimeExpr>),
    Continue,
    Goto(FlowRuntimeId),
    GotoExpr(RuntimeExpr),
    Return(String),
    ReturnExpr(RuntimeExpr),
    Effect(LineEffectRequest),
    EvaluatedEffect(RuntimeEffectExpr),
    /// Registers a reached deferred body and its evaluated captures with the
    /// owning runtime scope. Repeated executions of one site remain distinct.
    RegisterDefer {
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: Vec<RuntimeExpr>,
        owner: RuntimeDeferOwner,
    },
    RegisterCleanup {
        key: String,
        effect: LineEffectRequest,
    },
    CancelCleanup {
        key: String,
    },
    EnterScope {
        identity: crate::scope::RuntimeScopeIdentity,
    },
    ExitScope,
    /// Engine-only fallthrough marker for one Pending observer body.
    CompleteAwaitObserver,
    ExitScopeBind {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    Noop,
}

/// Runtime source of a general typed `Need<T>` Await. The executor evaluates
/// this once and retains the resulting handle across observer fallthrough.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedAwaitTarget {
    source: RuntimeExpr,
}

impl RuntimeNeedAwaitTarget {
    #[must_use]
    pub const fn new(source: RuntimeExpr) -> Self {
        Self { source }
    }

    #[must_use]
    pub const fn source(&self) -> &RuntimeExpr {
        &self.source
    }
}

/// Checked producer plan and already source-ordered argument expressions.
/// Core construction validates the argument types before this enters a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedProducerStartTarget {
    plan: NeedProducerTaskPlan,
    arguments: Box<[crate::task::RuntimeHostArgumentTemplate]>,
}

impl RuntimeNeedProducerStartTarget {
    pub fn try_new(
        plan: NeedProducerTaskPlan,
        arguments: Vec<crate::task::RuntimeHostArgumentTemplate>,
    ) -> Result<Self, RuntimeNeedProducerStartTargetError> {
        if arguments.len() != plan.argument_count() {
            return Err(RuntimeNeedProducerStartTargetError::ArgumentCountMismatch);
        }
        for (ordinal, argument) in arguments.iter().enumerate() {
            let binding_matches = match (plan.request(), argument) {
                (
                    crate::task::NeedProducerRequestProjection::AssetLoad { .. },
                    crate::task::RuntimeHostArgumentTemplate::Positional(..),
                ) => true,
                (
                    crate::task::NeedProducerRequestProjection::AssetLoad { argument_name, .. },
                    crate::task::RuntimeHostArgumentTemplate::Named(_, named),
                ) => named.name == *argument_name,
                (
                    crate::task::NeedProducerRequestProjection::ExternCapability {
                        argument_names,
                        ..
                    },
                    crate::task::RuntimeHostArgumentTemplate::Positional(..),
                ) => matches!(argument_names.get(ordinal), Some(None)),
                (
                    crate::task::NeedProducerRequestProjection::ExternCapability {
                        argument_names,
                        ..
                    },
                    crate::task::RuntimeHostArgumentTemplate::Named(_, named),
                ) => {
                    argument_names.get(ordinal).and_then(Option::as_deref)
                        == Some(named.name.as_str())
                }
                (_, crate::task::RuntimeHostArgumentTemplate::Spread(..)) => false,
            };
            if !binding_matches {
                return Err(
                    RuntimeNeedProducerStartTargetError::ArgumentBindingMismatch { ordinal },
                );
            }
        }
        Ok(Self {
            plan,
            arguments: arguments.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn plan(&self) -> &NeedProducerTaskPlan {
        &self.plan
    }

    #[must_use]
    pub fn arguments(&self) -> &[crate::task::RuntimeHostArgumentTemplate] {
        &self.arguments
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum RuntimeNeedProducerStartTargetError {
    #[error("Need producer start arguments do not match the selected request signature")]
    ArgumentCountMismatch,
    #[error("Need producer argument {ordinal} binding differs from its selected scalar request")]
    ArgumentBindingMismatch { ordinal: usize },
}

/// Typed operation executable only inside its owning dialogue activation.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineOperation {
    AcquireActor {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
        character: arcweft_character::id::CharacterId,
        scope: crate::line_task::RuntimeLineHandleScope,
    },
    Schedule {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
        delay: RuntimeExpr,
        child: crate::runtime_id::RuntimeLineTaskNodeId,
        captures: Box<[RuntimeScheduledCapture]>,
    },
    ActorLook {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
        character: arcweft_character::id::CharacterId,
        actor: RuntimeBorrowedLocal,
        look: RuntimeExpr,
        crossfade: RuntimeExpr,
    },
    VoiceHandle {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    },
}

/// A selected line-operation receiver borrowed from its live local slot.
/// This role cannot be constructed as a general value expression or captured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeBorrowedLocal {
    local: RuntimeLocalDeclarationId,
}

impl RuntimeBorrowedLocal {
    pub(crate) const fn new(local: RuntimeLocalDeclarationId) -> Self {
        Self { local }
    }

    #[must_use]
    pub const fn local(self) -> RuntimeLocalDeclarationId {
        self.local
    }
}

/// One exact callback-local/value pair captured at a scheduled source site.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeScheduledCapture {
    local: RuntimeLocalDeclarationId,
    value: RuntimeExpr,
}

impl RuntimeScheduledCapture {
    pub(crate) const fn new(local: RuntimeLocalDeclarationId, value: RuntimeExpr) -> Self {
        Self { local, value }
    }

    #[must_use]
    pub const fn local(&self) -> RuntimeLocalDeclarationId {
        self.local
    }

    #[must_use]
    pub const fn value(&self) -> &RuntimeExpr {
        &self.value
    }
}

impl RuntimeLineOperation {
    #[must_use]
    pub const fn site(&self) -> crate::runtime_id::RuntimeLineHandleSiteId {
        match self {
            Self::AcquireActor { site, .. }
            | Self::Schedule { site, .. }
            | Self::ActorLook { site, .. }
            | Self::VoiceHandle { site } => *site,
        }
    }
}

/// Sole parent binding target for one dialogue result value.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueResultTarget {
    ty: RuntimePlanTypeId,
    pattern: RuntimePattern,
}

impl RuntimeDialogueResultTarget {
    pub(crate) const fn new(ty: RuntimePlanTypeId, pattern: RuntimePattern) -> Self {
        Self { ty, pattern }
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimePlanTypeId {
        self.ty
    }

    #[must_use]
    pub const fn pattern(&self) -> &RuntimePattern {
        &self.pattern
    }
}

/// Direct host-call request surface for runtime-step hosts.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHostCallTarget {
    pub producer: crate::task::HostCallProducerDefinition,
    pub public_id: String,
    pub capability: String,
    pub operation: String,
    pub contract: Option<crate::step::HostCallContractDigest>,
    pub args: Vec<RuntimeHostArgumentTemplate>,
    pub result: RuntimePlanTypeId,
    pub mode: RuntimeHostCallMode,
    pub deterministic: bool,
}

/// One executable `match` arm in the runtime flow model.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeMatchArm {
    pub pattern: RuntimePattern,
    pub guard: Option<RuntimeMatchGuard>,
    pub ops: Vec<FlowOp>,
}

/// Shared flow guard: existing operations compute one final Bool condition.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeMatchGuard {
    pub candidate: crate::runtime_id::RuntimeLocalDeclarationId,
    pub condition: Option<RuntimeExpr>,
    pub copy_locals: Box<[crate::runtime_id::RuntimeLocalDeclarationId]>,
    pub ops: Vec<FlowOp>,
}

/// One source-ordered typed observer for a Pending publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitPendingObserver {
    pub pattern: RuntimePattern,
    pub ops: Vec<FlowOp>,
}

/// Runtime choice option visible to adapters and selectable from `RuntimeStepInput`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceRuntimeOption {
    pub id: Option<String>,
    pub label: String,
    pub target: Option<FlowRuntimeId>,
    pub out: Option<LineOutRequest>,
    pub effects: Vec<LineEffectRequest>,
}

/// Replay-observable flow event emitted by the core runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowEvent {
    DialogueLine {
        activation: crate::runtime_id::DialogueActivationId,
        line: RuntimeLineId,
        template: crate::runtime_id::RuntimeDialogueContentTemplateId,
        target: crate::value::RuntimeOpaqueValue,
        values: Box<[RuntimeDialogueValueBinding]>,
    },
    LineCancelled {
        trigger: String,
    },
    ChoicePresented {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    ChoiceSelected {
        id: Option<String>,
        option: String,
    },
    AwaitStarted {
        need: NeedId,
        task: Option<TaskId>,
    },
    AwaitReady {
        need: NeedId,
        /// An observation copy when the payload is unrestricted. An affine
        /// Ready value remains with its sole runtime consumer.
        value: Option<RuntimePayload>,
    },
    AwaitProgress {
        need: NeedId,
        progress: arcweft_need::Progress,
    },
    Goto {
        target: FlowRuntimeId,
    },
    Return {
        value: String,
    },
    Done,
}

/// One evaluated value supplied to a document-local dialogue slot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeDialogueValueBinding {
    pub slot: RuntimeDialogueValueSlotId,
    pub role: RuntimeDialogueValueRole,
    pub value: crate::value::RuntimeValue,
}

impl RuntimePlan {
    pub fn is_empty(&self) -> bool {
        self.inventory.is_empty()
    }

    /// Resolves one dynamic target against the exact accepted Flow inventory.
    ///
    /// A legacy canonical runtime ID still selects itself exactly. Otherwise
    /// the validated public label must identify one and only one accepted Flow;
    /// duplicate module-local labels are a terminal ambiguity.
    pub fn resolve_flow_target_value(
        &self,
        value: &str,
    ) -> Result<FlowRuntimeId, RuntimeFlowTargetError> {
        self.inventory.resolve_flow_target_value(value)
    }
}

impl RuntimeLineOperation {
    /// Direct owned expressions; borrowed receiver metadata is not a value read.
    pub(crate) fn argument_exprs(&self) -> Vec<&RuntimeExpr> {
        match self {
            Self::AcquireActor { .. } | Self::VoiceHandle { .. } => Vec::new(),
            Self::Schedule {
                delay, captures, ..
            } => std::iter::once(delay)
                .chain(captures.iter().map(RuntimeScheduledCapture::value))
                .collect(),
            Self::ActorLook {
                look, crossfade, ..
            } => vec![look, crossfade],
        }
    }
}
