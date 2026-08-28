mod construction;
mod dialogue_content;
pub mod entry_inventory;
mod function_sites;
pub mod generation_contract;
mod local_declarations;
#[cfg(test)]
pub(crate) use local_declarations::RuntimeLocalDeclarationTableBuilder;
mod nominal_record_domains;
mod type_kind;
mod type_table;
mod variant_domains;

pub use construction::{
    RuntimeAgentExprSeed, RuntimeAudioCommandSeed, RuntimeAwaitManyTargetSeed,
    RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed, RuntimeBuiltinIteratorEvidenceSeed,
    RuntimeCallArgumentSeed, RuntimeCallableExecutableSeed, RuntimeCallableExecutableSeedCode,
    RuntimeChoiceOptionSeed, RuntimeDialogueContentPlanSeed, RuntimeDialogueContentPlanSeedId,
    RuntimeDialogueEffectSiteSeedId, RuntimeDialogueMarkSeedId, RuntimeDialogueResultTargetSeed,
    RuntimeDialogueResultTargetSeedError, RuntimeDialogueValueSiteSeed, RuntimeDropPolicySeed,
    RuntimeEffectFieldSeed, RuntimeEvaluatedEffectSeed, RuntimeExprMatchArmSeed, RuntimeExprSeed,
    RuntimeExprSeedKind, RuntimeFieldProjectionSeed, RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed,
    RuntimeFlowSeed, RuntimeFunctionSiteDeclarationSeed, RuntimeFunctionSiteSeedId,
    RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeHostTaskRequestTemplateSeed,
    RuntimeIteratorEvidenceSeed, RuntimeIteratorWitnessEvidenceSeed,
    RuntimeIteratorWitnessExecutableSeed, RuntimeLineEffectSeed, RuntimeLineHandleSiteSeed,
    RuntimeLineOperationSeed, RuntimeLineTaskCancelRuleSeed, RuntimeLineTaskGroupSeed,
    RuntimeLineTaskGroupSeedId, RuntimeLineTaskNodeSeed, RuntimeLineTaskNodeSeedId,
    RuntimeLineTaskTriggerSeed, RuntimeLocalDeclarationSeed, RuntimeLocalSeedId,
    RuntimeNominalRecordFieldSeed, RuntimePatternRestSeed, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlanBuildError, RuntimePlanBuilder,
    RuntimePlanSemanticAdmission, RuntimePlanTable, RuntimePureHelperDeclarationSeed,
    RuntimePureHelperSeed, RuntimePureHelperSeedId, RuntimePureProgramBindingSeed,
    RuntimeRecordFieldSeedId, RuntimeRecordPatternFieldSeed, RuntimeScheduledCaptureSeed,
    RuntimeStreamMatchArmSeed, RuntimeStreamOpSeed, RuntimeStreamPlanSeed,
    RuntimeTraitMethodDeclarationSeed, RuntimeTraitMethodSeed, RuntimeTraitMethodSeedId,
};
pub use dialogue_content::{
    RuntimeDialogueContentPlan, RuntimeDialogueContentPlanTable,
    RuntimeDialogueContentPlanTableError, RuntimeDialogueMark, RuntimeDialogueValueRole,
    RuntimeDialogueValueSite,
};
pub use function_sites::{RuntimeFunctionSite, RuntimeFunctionSiteError, RuntimeFunctionSiteTable};
pub use generation_contract::{
    CharacterDialogueRuntimeCustomFieldDigest, RuntimeCharacterCatalogDigest,
    RuntimeGenerationIdentity, RuntimeProducerRootId, RuntimeProjectRootId,
    RuntimeViewCatalogDigest, RuntimeViewId,
};
pub use local_declarations::{
    RuntimeLocalDeclaration, RuntimeLocalDeclarationTable, RuntimeLocalDeclarationTableError,
};
pub use nominal_record_domains::{
    RuntimeNominalRecordDomain, RuntimeNominalRecordDomainError, RuntimeNominalRecordDomainField,
    RuntimeNominalRecordDomainFieldSeed, RuntimeNominalRecordDomainSeed,
    RuntimeNominalRecordDomainTable,
};
pub use type_kind::{
    RuntimeAgentOperationalType, RuntimeAgentTypeProjection, RuntimeOperationalType,
    RuntimePlanRecordField, RuntimePlanSequenceKind, RuntimePlanTypeClass,
    RuntimePlanTypeProjection,
};
pub use type_table::{
    MAX_RUNTIME_PLAN_TYPE_DEPTH, RuntimePlanTypeDeclaration, RuntimePlanTypeResolutionError,
    RuntimePlanTypeSeed, RuntimePlanTypeTable, RuntimePlanTypeTableError,
};
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
use crate::runtime_id::{
    RuntimeDialogueValueSlotId, RuntimeIdError, RuntimeIdFamily, RuntimeIdPath,
    RuntimeLocalDeclarationId, RuntimePlanTypeId, RuntimePublicLabel,
};
use crate::step::RuntimeHostCallMode;
use crate::stream::StreamPlan;
use crate::task::{AwaitManyTarget, AwaitTarget, NeedId, RuntimeHostArgumentTemplate, TaskId};

struct CheckedTypeTraversal<'a> {
    memo: &'a mut BTreeMap<RuntimePlanTypeId, Option<RuntimeCheckedType>>,
    visiting: &'a mut BTreeSet<RuntimePlanTypeId>,
}
use crate::value::{
    RuntimeExpr, RuntimeIterator, RuntimeLocalBinding, RuntimePayload, RuntimeRecordFieldId,
};
pub use entry_inventory::{
    EntryRuntimeId, RouteCaptureCoordinate, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlowInvocation, RuntimeFlowInvocationError, RuntimeHttpMethod, RuntimePlanError,
    RuntimeRouteBinding, RuntimeRouteBindingSource, RuntimeRoutePath, RuntimeRoutePathError,
    RuntimeRoutePathSegment, RuntimeRouteSpec,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePlan {
    pub(crate) artifact: Option<crate::effect::RuntimeArtifactFingerprint>,
    pub(crate) type_table: RuntimePlanTypeTable,
    pub(crate) local_declarations: RuntimeLocalDeclarationTable,
    pub(crate) nominal_record_domains: RuntimeNominalRecordDomainTable,
    pub(crate) variant_domains: RuntimeVariantDomainTable,
    pub(crate) function_sites: RuntimeFunctionSiteTable,
    pub(crate) dialogue_content: RuntimeDialogueContentPlanTable,
    pub(crate) entries: Vec<RuntimeEntrySpec>,
    pub(crate) callable_executables: Vec<RuntimeCallableExecutable>,
    pub(crate) flow_schemas: Vec<RuntimeFlowSchema>,
    pub(crate) flow_executables: Vec<RuntimeFlowExecutable>,
    pub(crate) flows: Vec<RuntimeFlow>,
    pub(crate) pure_helpers: Vec<RuntimePureHelper>,
    pub(crate) pure_programs: Vec<RuntimePureProgramBinding>,
    pub(crate) trait_methods: Vec<RuntimeTraitMethod>,
    pub(crate) line_task_groups: Vec<LineTaskGroup>,
    pub(crate) stream_plans: Vec<StreamPlan>,
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
        &self.type_table
    }

    #[must_use]
    pub const fn local_declarations(&self) -> &RuntimeLocalDeclarationTable {
        &self.local_declarations
    }

    #[must_use]
    pub const fn nominal_record_domains(&self) -> &RuntimeNominalRecordDomainTable {
        &self.nominal_record_domains
    }

    #[must_use]
    pub const fn variant_domains(&self) -> &RuntimeVariantDomainTable {
        &self.variant_domains
    }

    #[must_use]
    pub const fn function_sites(&self) -> &RuntimeFunctionSiteTable {
        &self.function_sites
    }

    #[must_use]
    pub const fn dialogue_content(&self) -> &RuntimeDialogueContentPlanTable {
        &self.dialogue_content
    }

    #[must_use]
    pub fn entries(&self) -> &[RuntimeEntrySpec] {
        &self.entries
    }

    #[must_use]
    pub fn callable_executables(&self) -> &[RuntimeCallableExecutable] {
        &self.callable_executables
    }

    #[must_use]
    pub fn flow_executables(&self) -> &[RuntimeFlowExecutable] {
        &self.flow_executables
    }

    #[must_use]
    pub fn flow_schemas(&self) -> &[RuntimeFlowSchema] {
        &self.flow_schemas
    }

    #[must_use]
    pub fn flows(&self) -> &[RuntimeFlow] {
        &self.flows
    }

    #[must_use]
    pub fn pure_helpers(&self) -> &[RuntimePureHelper] {
        &self.pure_helpers
    }

    #[must_use]
    pub fn pure_programs(&self) -> &[RuntimePureProgramBinding] {
        &self.pure_programs
    }

    #[must_use]
    pub fn trait_methods(&self) -> &[RuntimeTraitMethod] {
        &self.trait_methods
    }

    #[must_use]
    pub fn line_task_groups(&self) -> &[LineTaskGroup] {
        &self.line_task_groups
    }

    #[must_use]
    pub fn stream_plans(&self) -> &[StreamPlan] {
        &self.stream_plans
    }

    /// Derives the complete checked predicate in plan context. Nominal enum
    /// cases come from the owner-keyed variant domain rather than a copied
    /// type-row sidecar.
    pub fn checked_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        self.checked_type_inner(ty, &mut BTreeMap::new(), &mut BTreeSet::new())
    }

    /// Derives the final execution class from the complete plan-owned graph.
    pub fn type_class(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeClass, RuntimePlanTypeResolutionError> {
        self.type_table.class(ty)
    }

    fn checked_type_inner(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        if let Some(checked) = memo.get(&ty) {
            return Ok(checked.clone());
        }
        if !visiting.insert(ty) {
            return Err(RuntimePlanTypeResolutionError::CheckedProjectionCycle { ty });
        }
        let declaration = self
            .type_table
            .get(ty)
            .ok_or(RuntimePlanTypeResolutionError::UnknownType { ty })?;
        let checked = match declaration.projection() {
            RuntimePlanTypeProjection::Never => Some(RuntimeCheckedType::Never),
            RuntimePlanTypeProjection::Unit => Some(RuntimeCheckedType::Unit),
            RuntimePlanTypeProjection::Bool => Some(RuntimeCheckedType::Bool),
            RuntimePlanTypeProjection::Signed(width) => Some(RuntimeCheckedType::Signed(*width)),
            RuntimePlanTypeProjection::Unsigned(width) => {
                Some(RuntimeCheckedType::Unsigned(*width))
            }
            RuntimePlanTypeProjection::F32 => Some(RuntimeCheckedType::F32),
            RuntimePlanTypeProjection::F64 => Some(RuntimeCheckedType::F64),
            RuntimePlanTypeProjection::String => Some(RuntimeCheckedType::String),
            RuntimePlanTypeProjection::Char => Some(RuntimeCheckedType::Char),
            RuntimePlanTypeProjection::Bytes => Some(RuntimeCheckedType::Bytes),
            RuntimePlanTypeProjection::Duration => Some(RuntimeCheckedType::Duration),
            RuntimePlanTypeProjection::Progress => Some(RuntimeCheckedType::Progress),
            RuntimePlanTypeProjection::EntityReference => Some(RuntimeCheckedType::EntityReference),
            RuntimePlanTypeProjection::AgentValue => Some(RuntimeCheckedType::AgentValue),
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. } => self
                .checked_type_inner(*item, memo, visiting)?
                .map(|item| RuntimeCheckedType::Sequence(Box::new(item))),
            RuntimePlanTypeProjection::ProjectNominal {
                nominal,
                layout,
                arguments,
            } => self.checked_nominal_or_variant(
                ty,
                nominal,
                declaration.semantic_identity(),
                *layout,
                arguments,
                CheckedTypeTraversal { memo, visiting },
            )?,
            RuntimePlanTypeProjection::Tuple(items) => self
                .checked_children(items, memo, visiting)?
                .map(RuntimeCheckedType::Tuple),
            RuntimePlanTypeProjection::Record(fields) => {
                self.checked_record_type(ty, fields, memo, visiting)?
            }
            RuntimePlanTypeProjection::Choice(items) => self
                .checked_children(items, memo, visiting)?
                .map(RuntimeCheckedType::Choice),
            RuntimePlanTypeProjection::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => self.checked_result_type(
                ty,
                *value,
                *error,
                *value_payload,
                *error_payload,
                memo,
                visiting,
            )?,
            RuntimePlanTypeProjection::Option { item, some_payload } => {
                self.checked_option_type(ty, *item, *some_payload, memo, visiting)?
            }
            RuntimePlanTypeProjection::BuiltinVariant { owner, cases } => {
                self.checked_builtin_variant_type(*owner, cases, memo, visiting)?
            }
            RuntimePlanTypeProjection::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => {
                if self.variant_domains.get(ty).is_some() {
                    self.checked_variant(
                        ty,
                        declaration.semantic_identity(),
                        arguments,
                        memo,
                        visiting,
                    )?
                } else {
                    Some(RuntimeCheckedType::Opaque {
                        owner: RuntimeOpaqueTypeOwner::with_admission(
                            producer.clone(),
                            declaration.semantic_identity(),
                            *admission,
                            *value_class,
                            *persistence,
                        ),
                    })
                }
            }
            RuntimePlanTypeProjection::Agent(agent) => checked_agent_type(agent),
            RuntimePlanTypeProjection::Range(_)
            | RuntimePlanTypeProjection::Iterator(_)
            | RuntimePlanTypeProjection::Map { .. }
            | RuntimePlanTypeProjection::Need(_)
            | RuntimePlanTypeProjection::Stream { .. }
            | RuntimePlanTypeProjection::ThreadHandle(_)
            | RuntimePlanTypeProjection::Shared(_)
            | RuntimePlanTypeProjection::Reference(_)
            | RuntimePlanTypeProjection::Function { .. } => None,
        };
        visiting.remove(&ty);
        memo.insert(ty, checked.clone());
        Ok(checked)
    }

    fn checked_record_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        fields: &[RuntimePlanRecordField<crate::runtime_id::RuntimePlanTypeId>],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let mut checked = Vec::with_capacity(fields.len());
        for (ordinal, field) in fields.iter().enumerate() {
            let Some(field_ty) = self.checked_type_inner(*field.ty(), memo, visiting)? else {
                return Ok(None);
            };
            let field_id =
                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).map_err(|_| {
                    RuntimePlanTypeResolutionError::InvalidCheckedRecord {
                        ty,
                        source: RuntimeCheckedRecordTypeError::FieldOrdinalOverflow,
                    }
                })?;
            checked.push((field_id, field.diagnostic_name().to_owned(), field_ty));
        }
        RuntimeCheckedType::try_record(checked)
            .map(Some)
            .map_err(|source| RuntimePlanTypeResolutionError::InvalidCheckedRecord { ty, source })
    }

    fn checked_result_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        value: crate::runtime_id::RuntimePlanTypeId,
        error: crate::runtime_id::RuntimePlanTypeId,
        value_payload: crate::runtime_id::RuntimePlanTypeId,
        error_payload: crate::runtime_id::RuntimePlanTypeId,
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let Some(value) = self.checked_type_inner(value, memo, visiting)? else {
            return Ok(None);
        };
        let Some(error) = self.checked_type_inner(error, memo, visiting)? else {
            return Ok(None);
        };
        let Some(value_payload) = self.checked_type_inner(value_payload, memo, visiting)? else {
            return Ok(None);
        };
        let Some(error_payload) = self.checked_type_inner(error_payload, memo, visiting)? else {
            return Ok(None);
        };
        if value_payload != RuntimeCheckedType::Tuple(vec![value.clone()]) {
            return Err(
                RuntimePlanTypeResolutionError::InvalidBuiltinVariantPayload { ty, ordinal: 0 },
            );
        }
        if error_payload != RuntimeCheckedType::Tuple(vec![error.clone()]) {
            return Err(
                RuntimePlanTypeResolutionError::InvalidBuiltinVariantPayload { ty, ordinal: 1 },
            );
        }
        Ok(Some(RuntimeCheckedType::Result {
            ok: Box::new(value),
            error: Box::new(error),
        }))
    }

    fn checked_option_type(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        item: crate::runtime_id::RuntimePlanTypeId,
        some_payload: crate::runtime_id::RuntimePlanTypeId,
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let Some(item) = self.checked_type_inner(item, memo, visiting)? else {
            return Ok(None);
        };
        let Some(some_payload) = self.checked_type_inner(some_payload, memo, visiting)? else {
            return Ok(None);
        };
        if some_payload != RuntimeCheckedType::Tuple(vec![item.clone()]) {
            return Err(
                RuntimePlanTypeResolutionError::InvalidBuiltinVariantPayload { ty, ordinal: 0 },
            );
        }
        Ok(Some(RuntimeCheckedType::Option(Box::new(item))))
    }

    fn checked_builtin_variant_type(
        &self,
        owner: RuntimeBuiltinVariantIdentity,
        cases: &[Option<crate::runtime_id::RuntimePlanTypeId>],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let mut checked_cases = Vec::with_capacity(cases.len());
        for (schema, payload) in owner.cases().iter().zip(cases) {
            let payload = match payload {
                Some(payload) => {
                    let Some(payload) = self.checked_type_inner(*payload, memo, visiting)? else {
                        return Ok(None);
                    };
                    Some(Box::new(payload))
                }
                None => None,
            };
            checked_cases.push(RuntimeCheckedVariantCase {
                name: schema.name().to_owned(),
                payload,
            });
        }
        Ok(Some(RuntimeCheckedType::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Builtin(owner),
            arguments: Vec::new(),
            cases: checked_cases,
        }))
    }

    fn checked_nominal_or_variant(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        arguments: &[crate::runtime_id::RuntimePlanTypeId],
        traversal: CheckedTypeTraversal<'_>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let CheckedTypeTraversal { memo, visiting } = traversal;
        if self.variant_domains.get(ty).is_some() {
            return self.checked_variant(ty, semantic_identity, arguments, memo, visiting);
        }
        let Some(arguments) = self.checked_children(arguments, memo, visiting)? else {
            return Ok(None);
        };
        Ok(Some(RuntimeCheckedType::Nominal {
            nominal: nominal.clone(),
            semantic_identity,
            layout,
            arguments,
        }))
    }

    fn checked_variant(
        &self,
        ty: crate::runtime_id::RuntimePlanTypeId,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
        arguments: &[crate::runtime_id::RuntimePlanTypeId],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<RuntimeCheckedType>, RuntimePlanTypeResolutionError> {
        let Some(domain) = self.variant_domains.get(ty) else {
            return Ok(None);
        };
        let Some(arguments) = self.checked_children(arguments, memo, visiting)? else {
            return Ok(None);
        };
        let mut cases = Vec::with_capacity(domain.cases().len());
        for case in domain.cases() {
            let payload = match case.payload() {
                Some(payload) => {
                    let Some(payload) = self.checked_type_inner(payload, memo, visiting)? else {
                        return Ok(None);
                    };
                    Some(Box::new(payload))
                }
                None => None,
            };
            cases.push(RuntimeCheckedVariantCase {
                name: case.name().to_owned(),
                payload,
            });
        }
        Ok(Some(RuntimeCheckedType::Variant {
            owner: crate::pattern::RuntimeVariantIdentity::Nominal {
                nominal: domain.nominal().clone(),
                semantic_identity,
            },
            arguments,
            cases,
        }))
    }

    fn checked_children(
        &self,
        children: &[crate::runtime_id::RuntimePlanTypeId],
        memo: &mut BTreeMap<crate::runtime_id::RuntimePlanTypeId, Option<RuntimeCheckedType>>,
        visiting: &mut BTreeSet<crate::runtime_id::RuntimePlanTypeId>,
    ) -> Result<Option<Vec<RuntimeCheckedType>>, RuntimePlanTypeResolutionError> {
        let mut checked = Vec::with_capacity(children.len());
        for child in children {
            let Some(child) = self.checked_type_inner(*child, memo, visiting)? else {
                return Ok(None);
            };
            checked.push(child);
        }
        Ok(Some(checked))
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

fn checked_agent_type(
    agent: &RuntimeAgentTypeProjection<crate::runtime_id::RuntimePlanTypeId>,
) -> Option<RuntimeCheckedType> {
    (!matches!(agent, RuntimeAgentTypeProjection::Probe(_)))
        .then(|| RuntimeCheckedType::Agent(agent.operational_type()))
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
    pub params: Box<[RuntimeLocalDeclarationId]>,
    pub ops: Vec<FlowOp>,
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
        let public_label = Self::from_source_entity_body(public_id)?.public_label;
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
    pub id: RuntimePureHelperId,
    pub name: String,
    pub input_locals: Box<[RuntimeLocalDeclarationId]>,
    pub input_types: Vec<RuntimePureInputType>,
    pub output_type: RuntimePureOutputType,
    pub expr: RuntimeExpr,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

/// Exact stable program identity mapped to its deterministic runtime helper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePureProgramBinding {
    program: arcweft_id::runtime_program::RuntimePureProgramId,
    helper: RuntimePureHelperId,
    input_types: Box<[RuntimeSemanticTypeId]>,
    result_type: RuntimeSemanticTypeId,
}

impl RuntimePureProgramBinding {
    #[must_use]
    pub(crate) fn new(
        program: arcweft_id::runtime_program::RuntimePureProgramId,
        helper: RuntimePureHelperId,
        input_types: impl Into<Box<[RuntimeSemanticTypeId]>>,
        result_type: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            program,
            helper,
            input_types: input_types.into(),
            result_type,
        }
    }

    #[must_use]
    pub const fn program(&self) -> arcweft_id::runtime_program::RuntimePureProgramId {
        self.program
    }

    #[must_use]
    pub const fn helper(&self) -> RuntimePureHelperId {
        self.helper
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
    pub id: RuntimeTraitMethodId,
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub input_locals: Box<[RuntimeLocalDeclarationId]>,
    pub input_types: Vec<RuntimePureInputType>,
    pub output_type: RuntimePureOutputType,
    pub body: RuntimeExpr,
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

/// Runtime identifier for a lowered stream transform.

#[derive(Clone, Debug, PartialEq)]
pub enum FlowOp {
    Bind(Vec<RuntimeLocalBinding>),
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    LetElse {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
        else_ops: Vec<FlowOp>,
    },
    AssignNominalField {
        base: RuntimeLocalDeclarationId,
        field: crate::value::RuntimeRecordFieldId,
        value: RuntimeExpr,
    },
    LineOperation {
        binding: Option<RuntimePattern>,
        operation: RuntimeLineOperation,
    },
    CommitDialogueResult {
        value: RuntimeExpr,
    },
    Dialogue {
        content: crate::runtime_id::RuntimeDialogueContentPlanId,
        result: RuntimeDialogueResultTarget,
    },
    Choice {
        id: Option<String>,
        options: Vec<ChoiceRuntimeOption>,
    },
    Await {
        binding: Option<RuntimePattern>,
        target: AwaitTarget,
        observers: Vec<RuntimeAwaitPendingObserver>,
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
        body: Vec<FlowOp>,
    },
    Scope(Vec<FlowOp>),
    LetScope {
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
    RegisterCleanup {
        key: String,
        effect: LineEffectRequest,
    },
    CancelCleanup {
        key: String,
    },
    EnterScope,
    ExitScope,
    /// Engine-only fallthrough marker for one Pending observer body.
    CompleteAwaitObserver,
    ExitScopeBind {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    Noop,
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
        actor: RuntimeExpr,
        look: RuntimeExpr,
        crossfade: RuntimeExpr,
    },
    VoiceHandle {
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    },
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
    pub guard: Option<RuntimeExpr>,
    pub ops: Vec<FlowOp>,
}

/// One source-ordered typed observer for a Pending publication.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitPendingObserver {
    pub pattern: RuntimePattern,
    pub ops: Vec<FlowOp>,
}

pub(crate) type RuntimeMatchSelection = Option<(Vec<RuntimeLocalBinding>, Vec<FlowOp>)>;

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
        task: TaskId,
    },
    AwaitReady {
        need: NeedId,
        value: RuntimePayload,
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
    pub value: crate::value::RuntimeValue,
}

impl RuntimePlan {
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty() && self.line_task_groups.is_empty() && self.stream_plans.is_empty()
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
        FlowRuntimeId::resolve_runtime_target(value, self.flows.iter().map(|flow| &flow.id))
            .cloned()
    }
}
