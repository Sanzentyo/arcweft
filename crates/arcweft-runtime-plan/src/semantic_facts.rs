//! Accepted-generation semantic facts consumed by final-HIR runtime lowering.
//!
//! Runtime lowering is intentionally below semantic analysis in the crate
//! graph. The compiler therefore projects checked decisions into this closed
//! vocabulary and binds them to the exact executable HIR generation. Facts are
//! keyed by qualified final-HIR IDs; source-order counters, byte ranges,
//! display labels, and reconstructed paths are not accepted as identities.
//!
//! The closed fact vocabulary, staging input, immutable inventory, and atomic
//! admission remain one cohesive boundary so their families cannot drift into
//! partial schemas. HIR-owned graph traversal is delegated to HIR rather than
//! duplicated here; only runtime-plan validation and storage stay in this
//! module.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::entry::{
    RuntimeCallableId, RuntimeIdentityError, RuntimeNominalRecordShape, RuntimeNominalTypeId,
    TypeLayoutHash,
};
pub use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeCheckedRecordTypeError, RuntimeCheckedType,
    RuntimeCheckedVariantCase, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeAgentTypeProjection, RuntimeBuiltinIteratorFamily,
    RuntimeDialogueValueRole, RuntimeLineId, RuntimeNominalRecordDomainFieldSeed,
    RuntimeNominalRecordDomainSeed, RuntimePlanRecordField, RuntimePlanSequenceKind,
    RuntimePlanTypeProjection, RuntimePlanTypeSeed, RuntimeReceiverMode, RuntimeVariantCaseSeed,
    RuntimeVariantDomainSeed,
};
use arcweft_core::runtime_id::RuntimeDialogueValueSlotId;
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::value::{
    RuntimeAgentField, RuntimeIntrinsic, RuntimeNominalRecordLayout, RuntimeOpaquePersistence,
    RuntimeOpaqueValueClass, RuntimeRecordFieldId, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth,
    RuntimeValue,
};
use arcweft_id::runtime_program::RuntimePureProgramId;
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_hir::expr::{
    HirAwaitBranchKind, HirCallArgument, HirCallInvocation, HirChoiceCompactAction, HirChoiceItem,
    HirExprKind, HirPlaceholderKind, HirRecordField,
};
use arcweft_lang_hir::identity::{
    CaptureId, ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId, StmtId, TypeId,
};
use arcweft_lang_hir::item::{HirEntryMember, HirImplMember, HirItemFamily, HirItemKind};
use arcweft_lang_hir::leaf::HirName;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::pattern::{HirPatternField, HirPatternKind};
use arcweft_lang_hir::project::AcceptedDialogueLineInventory;
use arcweft_lang_hir::project::{
    HirAnalysisProjectView, HirRuntimeExecutableOwner, HirRuntimeIteratorWitnessMethodRole,
    HirRuntimeReachabilityEdge, HirRuntimeReachabilityEdgeKind, HirRuntimeReachabilityError,
    HirRuntimeReachabilityIdentity, HirRuntimeReachabilityRootKind, HirRuntimeReachabilitySite,
    HirRuntimeSemanticReachability,
};
use arcweft_lang_hir::scope::CaptureAccess;
use arcweft_lang_hir::source_index::{
    HirCallableSourceOwner, HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite,
};
use arcweft_lang_hir::stmt::{HirStmtKind, HirTrigger};
use arcweft_lang_hir::symbol::ImplMethodDeclarationId;
use arcweft_lang_hir::symbol::{
    CallableDeclarationKey, CallableDeclarationOwner, nominal::ProjectNominalDeclarationId,
};
use arcweft_text_model::DialogueContentSpec;
use thiserror::Error;

use crate::assertion_identity::RuntimeAssertionMode;

mod content;
mod evaluated_effect;
mod flow;
mod project_function;
mod type_dependencies;

pub use content::{
    RuntimeContentFragmentFact, RuntimeContentFragmentFactError, RuntimeContentFragmentId,
    RuntimeDialogueEffectCaptureFact, RuntimeDialogueEffectCaptureKey,
    RuntimeDialogueEffectProgramFact, RuntimeDialogueEffectProgramKey, RuntimeDialogueMarkFact,
    RuntimeDialogueMarkKey, RuntimeDialogueValueCaptureKey,
};
pub use evaluated_effect::{
    RuntimeDropFadeFact, RuntimeDropPolicyFact, RuntimeEffectFieldFact, RuntimeEvaluatedEffect,
    RuntimeEvaluatedEffectFact, RuntimeEvaluatedEffectOperandFact, RuntimeLogLevel,
};
pub use flow::RuntimeFlowFact;
pub use project_function::{
    RuntimeClosureCaptureFact, RuntimeClosureInstanceFact, RuntimeClosureInstanceKey,
    RuntimeClosureParameterFact, RuntimeProjectAttachedDefaultCapture,
    RuntimeProjectAttachedDefaultFunctionFact, RuntimeProjectContinuationAbi,
    RuntimeProjectFunctionBody, RuntimeProjectFunctionCallInput, RuntimeProjectFunctionCallOutcome,
    RuntimeProjectFunctionCallPlan, RuntimeProjectFunctionExecution,
    RuntimeProjectFunctionExpressionPayload, RuntimeProjectFunctionExpressionSemanticFact,
    RuntimeProjectFunctionFactError, RuntimeProjectFunctionInstanceFact,
    RuntimeProjectFunctionInstanceKey, RuntimeProjectFunctionInstanceSemanticFacts,
    RuntimeProjectFunctionParameterAbi, RuntimeProjectFunctionParameterMaterialization,
    RuntimeProjectFunctionParameterSource, RuntimeProjectFunctionPatternPayload,
    RuntimeProjectFunctionPatternSemanticFact, RuntimeProjectFunctionRootFact,
    RuntimeProjectFunctionRootRole, RuntimeProjectFunctionStatementPayload,
    RuntimeProjectFunctionStatementSemanticFact, RuntimeProjectFunctionTypeOwner,
    RuntimeProjectFunctionTypeProjection,
};

/// Stable semantic identity for a registered callable or value that is not
/// owned by one project HIR item.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeRegisteredValueId([u8; 32]);

impl RuntimeRegisteredValueId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact normalized shape of a semantic type owned by the Agent Prelude.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAgentTypeShape {
    DebugStatePath,
    ObservationFieldPath,
    Probe(Box<RuntimeNormalizedType>),
    Predicate,
    Observation,
    ObservedObject,
    BoundingBox,
    ActionName,
    ActionTarget,
    ActionResult,
    DataFormat,
    DataShape,
    EntityMetadata,
    SourceAnchor,
    ProjectGraphNeighborhood,
    ProjectGraphSymbol,
    ProjectGraphEdge,
    CaptureTarget,
    CaptureReference,
    Resource,
    RagContextPack,
    ObservedObjectId,
    Diagnostics,
    WaitError,
    ViewportPoint,
    RagError,
    SourcePosition,
    ProjectFlowControlSummary,
    ProjectGraphSummary,
    BinaryResourceBody,
    BinaryData,
}

/// Runtime-relevant shape paired with an exact semantic type identity.
///
/// `Opaque` is not an unresolved type. It is a fully resolved semantic type
/// whose runtime operations are owned by a registered producer rather than the
/// Arcweft core value algebra. Its exact identity remains in
/// [`RuntimeNormalizedType::identity`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "checked type facts preserve a direct exhaustive semantic shape; they are immutable generation-bound inputs rather than a hot runtime value representation"
)]
pub enum RuntimeTypeShape {
    Unit,
    Never,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Bytes,
    Duration,
    Progress,
    EntityReference,
    AgentValue,
    Range(Box<RuntimeNormalizedType>),
    Iterator(Box<RuntimeNormalizedType>),
    Sequence {
        kind: RuntimeSequenceKind,
        item: Box<RuntimeNormalizedType>,
    },
    Array {
        item: Box<RuntimeNormalizedType>,
        length: usize,
    },
    Map {
        key: Box<RuntimeNormalizedType>,
        value: Box<RuntimeNormalizedType>,
    },
    Need(Box<RuntimeNormalizedType>),
    Stream {
        item: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Parser {
        item: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Result {
        value: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
        value_payload: Box<RuntimeNormalizedType>,
        error_payload: Box<RuntimeNormalizedType>,
    },
    Option {
        item: Box<RuntimeNormalizedType>,
        some_payload: Box<RuntimeNormalizedType>,
    },
    BuiltinVariant {
        owner: RuntimeBuiltinVariantIdentity,
        cases: Box<[Option<RuntimeNormalizedType>]>,
    },
    ThreadHandle(Box<RuntimeNormalizedType>),
    Shared(Box<RuntimeNormalizedType>),
    Reference(Box<RuntimeNormalizedType>),
    Function {
        parameters: Box<[RuntimeNormalizedType]>,
        result: Box<RuntimeNormalizedType>,
    },
    ProjectNominal {
        nominal: RuntimeResolvedNominal,
        arguments: Box<[RuntimeNormalizedType]>,
    },
    Tuple(Box<[RuntimeNormalizedType]>),
    Record(Box<[RuntimeRecordTypeField]>),
    Choice(Box<[RuntimeNormalizedType]>),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        admission: RuntimeOpaqueTypeAdmission,
        value_class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
        arguments: Box<[RuntimeNormalizedType]>,
    },
    Agent(RuntimeAgentTypeShape),
}

/// One declaration-ordered field of an exact anonymous record runtime type.
///
/// The label is retained for runtime record mechanics and diagnostics. Exact
/// type equality is owned by declaration position plus the recursive semantic
/// type, so a diagnostic rename cannot change Match or checked-type identity.
#[derive(Clone, Debug)]
pub struct RuntimeRecordTypeField {
    diagnostic_name: String,
    ty: RuntimeNormalizedType,
}

impl PartialEq for RuntimeRecordTypeField {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
    }
}

impl Eq for RuntimeRecordTypeField {}

impl RuntimeRecordTypeField {
    pub fn new(diagnostic_name: impl Into<String>, ty: RuntimeNormalizedType) -> Self {
        Self {
            diagnostic_name: diagnostic_name.into(),
            ty,
        }
    }

    pub fn diagnostic_name(&self) -> &str {
        &self.diagnostic_name
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSequenceKind {
    Vec,
    Array,
    Slice,
    Seq,
}

/// One deterministic descent from a normalized runtime type to a checked leaf.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTypeProjectionStep {
    SequenceItem,
    ProjectNominalArgument(u32),
    TupleItem(u32),
    RecordField(u32),
    ChoiceAlternative(u32),
    OpaqueArgument(u32),
    ResultOk,
    ResultError,
    OptionItem,
    BuiltinVariantCase(u32),
    AgentProbeValue,
}

/// Typed location of the first checked-type projection failure.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeProjectionPath(Box<[RuntimeTypeProjectionStep]>);

impl RuntimeTypeProjectionPath {
    #[must_use]
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    #[must_use]
    pub fn pushed(&self, step: RuntimeTypeProjectionStep) -> Self {
        let mut steps = self.0.to_vec();
        steps.push(step);
        Self(steps.into_boxed_slice())
    }

    #[must_use]
    pub const fn steps(&self) -> &[RuntimeTypeProjectionStep] {
        &self.0
    }
}

/// Closed diagnostic category for shapes outside the checked value algebra.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeUnsupportedTypeShape {
    Range,
    Iterator,
    Map,
    Need,
    Stream,
    Parser,
    ThreadHandle,
    Shared,
    Reference,
    Function,
}

/// Invalid retained identity on a checked project nominal fact.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedNominalError {
    #[error(transparent)]
    InvalidIdentity(#[from] RuntimeIdentityError),
}

/// Failure to project a normalized semantic type into the closed runtime algebra.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCheckedTypeProjectionError {
    #[error("runtime type `{type_label}` has no opaque producer evidence")]
    MissingOpaqueProducerEvidence {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        type_label: String,
    },
    #[error("runtime type shape `{shape:?}` is not representable")]
    UnsupportedRuntimeShape {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        shape: RuntimeUnsupportedTypeShape,
    },
    #[error("project nominal runtime identity is invalid")]
    InvalidProjectNominal {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        reason: RuntimeResolvedNominalError,
    },
    #[error("builtin variant `{owner:?}` has a non-canonical payload schema")]
    InvalidBuiltinVariant {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        owner: RuntimeBuiltinVariantIdentity,
    },
    #[error("record runtime type has a non-canonical field schema")]
    InvalidRecord {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        reason: RuntimeCheckedRecordTypeError,
    },
}

/// One normalized semantic type that can be compared without source spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNormalizedType {
    identity: RuntimeSemanticTypeId,
    shape: RuntimeTypeShape,
}

/// One exact case selected directly from a normalized runtime variant type.
///
/// This borrowed view retains the exact normalized payload under the core
/// builtin case schema. Synthetic lowering paths cannot substitute a raw
/// item type for the case's structural payload type or erase its identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNormalizedVariantSelection<'a> {
    owner: &'a RuntimeNormalizedType,
    ordinal: u32,
    payload: Option<&'a RuntimeNormalizedType>,
}

impl RuntimeNormalizedVariantSelection<'_> {
    pub(crate) const fn owner(&self) -> &RuntimeNormalizedType {
        self.owner
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn payload(&self) -> Option<&RuntimeNormalizedType> {
        self.payload
    }

    pub(crate) fn single_payload_item(
        &self,
    ) -> Result<Option<&RuntimeNormalizedType>, RuntimeNormalizedVariantSelectionError> {
        match self.payload {
            Some(payload) => match payload.shape() {
                RuntimeTypeShape::Tuple(items) if items.len() == 1 => Ok(items.first()),
                RuntimeTypeShape::Tuple(items) => {
                    Err(RuntimeNormalizedVariantSelectionError::PayloadArity {
                        owner: self.owner.identity(),
                        ordinal: self.ordinal,
                        actual: items.len(),
                    })
                }
                _ => Err(RuntimeNormalizedVariantSelectionError::NonTuplePayload {
                    owner: self.owner.identity(),
                    ordinal: self.ordinal,
                }),
            },
            None => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RuntimeNormalizedVariantSelectionError {
    #[error("normalized type {owner:?} is not a closed runtime variant owner")]
    InvalidOwner { owner: RuntimeSemanticTypeId },
    #[error("normalized variant type {owner:?} disagrees with the {builtin:?} case schema")]
    InvalidCaseSchema {
        owner: RuntimeSemanticTypeId,
        builtin: RuntimeBuiltinVariantIdentity,
    },
    #[error("normalized variant type {owner:?} has {count} cases, exceeding u32 ordinals")]
    CaseCountOverflow {
        owner: RuntimeSemanticTypeId,
        count: usize,
    },
    #[error("normalized variant type {owner:?} has no case {ordinal} among {case_count} cases")]
    CaseOrdinal {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
        case_count: u32,
    },
    #[error("normalized variant type {owner:?} case {ordinal} disagrees with its declared payload")]
    PayloadMismatch {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
    },
    #[error("normalized variant type {owner:?} case {ordinal} payload is not a tuple")]
    NonTuplePayload {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
    },
    #[error(
        "normalized variant type {owner:?} case {ordinal} payload has {actual} fields instead of one"
    )]
    PayloadArity {
        owner: RuntimeSemanticTypeId,
        ordinal: u32,
        actual: usize,
    },
}

impl RuntimeNormalizedType {
    pub const fn new(identity: RuntimeSemanticTypeId, shape: RuntimeTypeShape) -> Self {
        Self { identity, shape }
    }

    pub const fn identity(&self) -> RuntimeSemanticTypeId {
        self.identity
    }

    pub const fn shape(&self) -> &RuntimeTypeShape {
        &self.shape
    }

    pub(crate) fn variant_selection(
        &self,
        ordinal: u32,
    ) -> Result<RuntimeNormalizedVariantSelection<'_>, RuntimeNormalizedVariantSelectionError> {
        match self.shape() {
            RuntimeTypeShape::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => {
                self.validate_unary_case_payload(0, value, value_payload)?;
                self.validate_unary_case_payload(1, error, error_payload)?;
                self.select_builtin_case(
                    RuntimeBuiltinVariantIdentity::Result,
                    ordinal,
                    [Some(value_payload.as_ref()), Some(error_payload.as_ref())].into_iter(),
                )
            }
            RuntimeTypeShape::Option { item, some_payload } => {
                self.validate_unary_case_payload(0, item, some_payload)?;
                self.select_builtin_case(
                    RuntimeBuiltinVariantIdentity::Option,
                    ordinal,
                    [Some(some_payload.as_ref()), None].into_iter(),
                )
            }
            RuntimeTypeShape::BuiltinVariant { owner, cases } => {
                self.select_builtin_case(*owner, ordinal, cases.iter().map(Option::as_ref))
            }
            _ => Err(RuntimeNormalizedVariantSelectionError::InvalidOwner {
                owner: self.identity(),
            }),
        }
    }

    fn validate_unary_case_payload(
        &self,
        ordinal: u32,
        item: &Self,
        payload: &Self,
    ) -> Result<(), RuntimeNormalizedVariantSelectionError> {
        // The aggregate type inventory owns each identity's definition. A
        // unary case must reference that exact item, not an equal checked shape.
        if matches!(payload.shape(), RuntimeTypeShape::Tuple(items)
            if matches!(items.as_ref(), [actual] if actual.identity() == item.identity()))
        {
            Ok(())
        } else {
            Err(RuntimeNormalizedVariantSelectionError::PayloadMismatch {
                owner: self.identity(),
                ordinal,
            })
        }
    }

    fn select_builtin_case<'a>(
        &'a self,
        builtin: RuntimeBuiltinVariantIdentity,
        ordinal: u32,
        cases: impl ExactSizeIterator<Item = Option<&'a Self>>,
    ) -> Result<RuntimeNormalizedVariantSelection<'a>, RuntimeNormalizedVariantSelectionError> {
        let schemas = builtin.cases();
        let case_count = u32::try_from(cases.len()).map_err(|_| {
            RuntimeNormalizedVariantSelectionError::CaseCountOverflow {
                owner: self.identity(),
                count: cases.len(),
            }
        })?;
        if cases.len() != schemas.len() {
            return Err(RuntimeNormalizedVariantSelectionError::InvalidCaseSchema {
                owner: self.identity(),
                builtin,
            });
        }
        if ordinal >= case_count {
            return Err(RuntimeNormalizedVariantSelectionError::CaseOrdinal {
                owner: self.identity(),
                ordinal,
                case_count,
            });
        }
        let mut selected = None;
        for ((index, payload), schema) in cases.enumerate().zip(schemas) {
            if schema.has_payload() != payload.is_some() {
                return Err(RuntimeNormalizedVariantSelectionError::InvalidCaseSchema {
                    owner: self.identity(),
                    builtin,
                });
            }
            if u32::try_from(index).ok() == Some(ordinal) {
                selected = payload;
            }
        }
        Ok(RuntimeNormalizedVariantSelection {
            owner: self,
            ordinal,
            payload: selected,
        })
    }

    pub fn checked_type(&self) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        self.checked_type_at(&RuntimeTypeProjectionPath::root())
    }

    /// Projects this accepted semantic type into the single plan-owned type
    /// graph. Child references remain semantic identities until the aggregate
    /// builder atomically rewrites the complete batch to plan-local IDs.
    pub fn runtime_plan_type_seed(
        &self,
    ) -> Result<RuntimePlanTypeSeed, RuntimeCheckedTypeProjectionError> {
        Ok(RuntimePlanTypeSeed::new(
            self.identity,
            self.runtime_plan_type_projection(),
        ))
    }

    fn runtime_plan_type_projection(&self) -> RuntimePlanTypeProjection<RuntimeSemanticTypeId> {
        let child = |ty: &RuntimeNormalizedType| ty.identity();
        match self.shape() {
            RuntimeTypeShape::Never => RuntimePlanTypeProjection::Never,
            RuntimeTypeShape::Unit => RuntimePlanTypeProjection::Unit,
            RuntimeTypeShape::Bool => RuntimePlanTypeProjection::Bool,
            RuntimeTypeShape::Signed(width) => RuntimePlanTypeProjection::Signed(*width),
            RuntimeTypeShape::Unsigned(width) => RuntimePlanTypeProjection::Unsigned(*width),
            RuntimeTypeShape::F32 => RuntimePlanTypeProjection::F32,
            RuntimeTypeShape::F64 => RuntimePlanTypeProjection::F64,
            RuntimeTypeShape::String => RuntimePlanTypeProjection::String,
            RuntimeTypeShape::Char => RuntimePlanTypeProjection::Char,
            RuntimeTypeShape::Bytes => RuntimePlanTypeProjection::Bytes,
            RuntimeTypeShape::Duration => RuntimePlanTypeProjection::Duration,
            RuntimeTypeShape::Progress => RuntimePlanTypeProjection::Progress,
            RuntimeTypeShape::EntityReference => RuntimePlanTypeProjection::EntityReference,
            RuntimeTypeShape::AgentValue => RuntimePlanTypeProjection::AgentValue,
            RuntimeTypeShape::Range(item) => RuntimePlanTypeProjection::Range(child(item)),
            RuntimeTypeShape::Iterator(item) => RuntimePlanTypeProjection::Iterator(child(item)),
            RuntimeTypeShape::Sequence { kind, item } => RuntimePlanTypeProjection::Sequence {
                kind: kind.runtime_plan_kind(),
                item: child(item),
            },
            RuntimeTypeShape::Array { item, length } => RuntimePlanTypeProjection::Array {
                item: child(item),
                length: u64::try_from(*length)
                    .expect("usize fits the u64 Arcweft runtime-plan contract"),
            },
            RuntimeTypeShape::Map { key, value } => RuntimePlanTypeProjection::Map {
                key: child(key),
                value: child(value),
            },
            RuntimeTypeShape::Need(item) => RuntimePlanTypeProjection::Need(child(item)),
            RuntimeTypeShape::Stream { item, error } => RuntimePlanTypeProjection::Stream {
                item: child(item),
                error: child(error),
            },
            RuntimeTypeShape::Parser { .. } => {
                unreachable!("unsupported Parser shape cannot enter RuntimePlan type admission")
            }
            RuntimeTypeShape::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => RuntimePlanTypeProjection::Result {
                value: child(value),
                error: child(error),
                value_payload: child(value_payload),
                error_payload: child(error_payload),
            },
            RuntimeTypeShape::Option { item, some_payload } => RuntimePlanTypeProjection::Option {
                item: child(item),
                some_payload: child(some_payload),
            },
            RuntimeTypeShape::BuiltinVariant { owner, cases } => {
                RuntimePlanTypeProjection::BuiltinVariant {
                    owner: *owner,
                    cases: cases
                        .iter()
                        .map(|payload| payload.as_ref().map(RuntimeNormalizedType::identity))
                        .collect(),
                }
            }
            RuntimeTypeShape::ThreadHandle(result) => {
                RuntimePlanTypeProjection::ThreadHandle(child(result))
            }
            RuntimeTypeShape::Shared(inner) => RuntimePlanTypeProjection::Shared(child(inner)),
            RuntimeTypeShape::Reference(inner) => {
                RuntimePlanTypeProjection::Reference(child(inner))
            }
            RuntimeTypeShape::Function { parameters, result } => {
                RuntimePlanTypeProjection::Function {
                    parameters: parameters
                        .iter()
                        .map(RuntimeNormalizedType::identity)
                        .collect(),
                    result: child(result),
                }
            }
            RuntimeTypeShape::ProjectNominal { nominal, arguments } => {
                RuntimePlanTypeProjection::ProjectNominal {
                    nominal: nominal.runtime_nominal_id(),
                    layout: nominal.layout(),
                    arguments: arguments
                        .iter()
                        .map(RuntimeNormalizedType::identity)
                        .collect(),
                }
            }
            RuntimeTypeShape::Tuple(items) => RuntimePlanTypeProjection::Tuple(
                items.iter().map(RuntimeNormalizedType::identity).collect(),
            ),
            RuntimeTypeShape::Record(fields) => RuntimePlanTypeProjection::Record(
                fields
                    .iter()
                    .map(|field| {
                        RuntimePlanRecordField::new(field.diagnostic_name(), field.ty().identity())
                    })
                    .collect(),
            ),
            RuntimeTypeShape::Choice(items) => RuntimePlanTypeProjection::Choice(
                items.iter().map(RuntimeNormalizedType::identity).collect(),
            ),
            RuntimeTypeShape::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                arguments,
            } => RuntimePlanTypeProjection::Opaque {
                producer: producer.clone(),
                admission: *admission,
                value_class: *value_class,
                persistence: *persistence,
                arguments: arguments
                    .iter()
                    .map(RuntimeNormalizedType::identity)
                    .collect(),
            },
            RuntimeTypeShape::Agent(agent) => {
                RuntimePlanTypeProjection::Agent(agent.runtime_plan_projection())
            }
        }
    }

    fn append_runtime_plan_type_seeds(
        &self,
        seeds: &mut Vec<RuntimePlanTypeSeed>,
    ) -> Result<(), RuntimeCheckedTypeProjectionError> {
        seeds.push(self.runtime_plan_type_seed()?);
        for child in self.children() {
            child.append_runtime_plan_type_seeds(seeds)?;
        }
        Ok(())
    }

    fn children(&self) -> Vec<&RuntimeNormalizedType> {
        match self.shape() {
            RuntimeTypeShape::Range(item)
            | RuntimeTypeShape::Iterator(item)
            | RuntimeTypeShape::ThreadHandle(item)
            | RuntimeTypeShape::Shared(item)
            | RuntimeTypeShape::Reference(item)
            | RuntimeTypeShape::Sequence { item, .. }
            | RuntimeTypeShape::Array { item, .. }
            | RuntimeTypeShape::Need(item)
            | RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Probe(item)) => vec![item],
            RuntimeTypeShape::Map { key, value }
            | RuntimeTypeShape::Stream {
                item: key,
                error: value,
            }
            | RuntimeTypeShape::Parser {
                item: key,
                error: value,
            } => vec![key, value],
            RuntimeTypeShape::Result {
                value: key,
                error: value,
                value_payload,
                error_payload,
            } => vec![key, value, value_payload, error_payload],
            RuntimeTypeShape::Option { item, some_payload } => vec![item, some_payload],
            RuntimeTypeShape::BuiltinVariant { cases, .. } => {
                cases.iter().filter_map(Option::as_ref).collect()
            }
            RuntimeTypeShape::Function { parameters, result } => parameters
                .iter()
                .chain(std::iter::once(result.as_ref()))
                .collect(),
            RuntimeTypeShape::ProjectNominal { arguments, .. }
            | RuntimeTypeShape::Tuple(arguments)
            | RuntimeTypeShape::Choice(arguments)
            | RuntimeTypeShape::Opaque { arguments, .. } => arguments.iter().collect(),
            RuntimeTypeShape::Record(fields) => {
                fields.iter().map(RuntimeRecordTypeField::ty).collect()
            }
            RuntimeTypeShape::Never
            | RuntimeTypeShape::Unit
            | RuntimeTypeShape::Bool
            | RuntimeTypeShape::Signed(_)
            | RuntimeTypeShape::Unsigned(_)
            | RuntimeTypeShape::F32
            | RuntimeTypeShape::F64
            | RuntimeTypeShape::String
            | RuntimeTypeShape::Char
            | RuntimeTypeShape::Bytes
            | RuntimeTypeShape::Duration
            | RuntimeTypeShape::Progress
            | RuntimeTypeShape::EntityReference
            | RuntimeTypeShape::AgentValue
            | RuntimeTypeShape::Agent(_) => Vec::new(),
        }
    }

    fn checked_type_at(
        &self,
        path: &RuntimeTypeProjectionPath,
    ) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        if let Some(checked) = self.checked_leaf_type() {
            return Ok(checked);
        }
        if let Some(shape) = unsupported_runtime_shape(self.shape()) {
            return Err(self.unsupported(path, shape));
        }
        Ok(match self.shape() {
            RuntimeTypeShape::Sequence { item, .. } => RuntimeCheckedType::Sequence(Box::new(
                item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::SequenceItem))?,
            )),
            RuntimeTypeShape::Array { item, length } => RuntimeCheckedType::Array {
                item: Box::new(
                    item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::SequenceItem))?,
                ),
                length: u64::try_from(*length)
                    .expect("usize fits the u64 Arcweft runtime-plan contract"),
            },
            RuntimeTypeShape::ProjectNominal { nominal, arguments } => {
                RuntimeCheckedType::Nominal {
                    nominal: nominal.runtime_nominal_id(),
                    semantic_identity: self.identity(),
                    layout: nominal.layout(),
                    arguments: arguments
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            argument.checked_type_at(&path.pushed(
                                RuntimeTypeProjectionStep::ProjectNominalArgument(
                                    projection_index(index),
                                ),
                            ))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            RuntimeTypeShape::Choice(alternatives) => RuntimeCheckedType::Choice(
                alternatives
                    .iter()
                    .enumerate()
                    .map(|(index, alternative)| {
                        alternative.checked_type_at(&path.pushed(
                            RuntimeTypeProjectionStep::ChoiceAlternative(projection_index(index)),
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Tuple(items) => RuntimeCheckedType::Tuple(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::TupleItem(
                            projection_index(index),
                        )))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Record(fields) => {
                let mut checked = Vec::with_capacity(fields.len());
                for (index, field) in fields.iter().enumerate() {
                    let field_path = path.pushed(RuntimeTypeProjectionStep::RecordField(
                        projection_index(index),
                    ));
                    let field_id = RuntimeRecordFieldId::try_from_zero_based_ordinal(index)
                        .map_err(|_| RuntimeCheckedTypeProjectionError::InvalidRecord {
                            semantic_identity: self.identity(),
                            path: field_path.clone(),
                            reason: RuntimeCheckedRecordTypeError::FieldOrdinalOverflow,
                        })?;
                    checked.push((
                        field_id,
                        field.diagnostic_name().to_owned(),
                        field.ty().checked_type_at(&field_path)?,
                    ));
                }
                RuntimeCheckedType::try_record(checked).map_err(|reason| {
                    RuntimeCheckedTypeProjectionError::InvalidRecord {
                        semantic_identity: self.identity(),
                        path: path.clone(),
                        reason,
                    }
                })?
            }
            RuntimeTypeShape::Result {
                value,
                error,
                value_payload,
                error_payload,
            } => {
                let value =
                    value.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::ResultOk))?;
                let error =
                    error.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::ResultError))?;
                let checked_value_payload = value_payload.checked_type_at(
                    &path.pushed(RuntimeTypeProjectionStep::BuiltinVariantCase(0)),
                )?;
                let checked_error_payload = error_payload.checked_type_at(
                    &path.pushed(RuntimeTypeProjectionStep::BuiltinVariantCase(1)),
                )?;
                if checked_value_payload != RuntimeCheckedType::Tuple(vec![value.clone()])
                    || checked_error_payload != RuntimeCheckedType::Tuple(vec![error.clone()])
                {
                    return Err(RuntimeCheckedTypeProjectionError::InvalidBuiltinVariant {
                        semantic_identity: self.identity(),
                        path: path.clone(),
                        owner: RuntimeBuiltinVariantIdentity::Result,
                    });
                }
                RuntimeCheckedType::Result {
                    ok: Box::new(value),
                    error: Box::new(error),
                }
            }
            RuntimeTypeShape::Option { item, some_payload } => {
                let item =
                    item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::OptionItem))?;
                let checked_payload = some_payload.checked_type_at(
                    &path.pushed(RuntimeTypeProjectionStep::BuiltinVariantCase(0)),
                )?;
                if checked_payload != RuntimeCheckedType::Tuple(vec![item.clone()]) {
                    return Err(RuntimeCheckedTypeProjectionError::InvalidBuiltinVariant {
                        semantic_identity: self.identity(),
                        path: path.clone(),
                        owner: RuntimeBuiltinVariantIdentity::Option,
                    });
                }
                RuntimeCheckedType::Option(Box::new(item))
            }
            RuntimeTypeShape::BuiltinVariant { owner, cases } => {
                let payloads = cases
                    .iter()
                    .enumerate()
                    .map(|(index, payload)| {
                        payload
                            .as_ref()
                            .map(|payload| {
                                payload.checked_type_at(&path.pushed(
                                    RuntimeTypeProjectionStep::BuiltinVariantCase(
                                        projection_index(index),
                                    ),
                                ))
                            })
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, RuntimeCheckedTypeProjectionError>>()?;
                RuntimeCheckedType::try_builtin_variant(*owner, payloads).map_err(|_| {
                    RuntimeCheckedTypeProjectionError::InvalidBuiltinVariant {
                        semantic_identity: self.identity(),
                        path: path.clone(),
                        owner: *owner,
                    }
                })?
            }
            RuntimeTypeShape::Opaque {
                producer,
                admission,
                value_class,
                persistence,
                ..
            } => RuntimeCheckedType::Opaque {
                owner: RuntimeOpaqueTypeOwner::with_admission(
                    producer.clone(),
                    self.identity(),
                    *admission,
                    *value_class,
                    *persistence,
                ),
            },
            RuntimeTypeShape::Never
            | RuntimeTypeShape::Unit
            | RuntimeTypeShape::Bool
            | RuntimeTypeShape::Signed(_)
            | RuntimeTypeShape::Unsigned(_)
            | RuntimeTypeShape::F32
            | RuntimeTypeShape::F64
            | RuntimeTypeShape::String
            | RuntimeTypeShape::Char
            | RuntimeTypeShape::Bytes
            | RuntimeTypeShape::Duration
            | RuntimeTypeShape::Progress
            | RuntimeTypeShape::EntityReference
            | RuntimeTypeShape::AgentValue
            | RuntimeTypeShape::Range(_)
            | RuntimeTypeShape::Iterator(_)
            | RuntimeTypeShape::Map { .. }
            | RuntimeTypeShape::Need(_)
            | RuntimeTypeShape::Stream { .. }
            | RuntimeTypeShape::Parser { .. }
            | RuntimeTypeShape::ThreadHandle(_)
            | RuntimeTypeShape::Shared(_)
            | RuntimeTypeShape::Reference(_)
            | RuntimeTypeShape::Function { .. } => {
                unreachable!("leaf and unsupported shapes returned before recursive projection")
            }
            RuntimeTypeShape::Agent(agent) => {
                RuntimeCheckedType::Agent(agent.try_project(|result| {
                    result
                        .checked_type_at(&path.pushed(RuntimeTypeProjectionStep::AgentProbeValue))
                        .map(Box::new)
                })?)
            }
        })
    }

    fn checked_leaf_type(&self) -> Option<RuntimeCheckedType> {
        match self.shape() {
            RuntimeTypeShape::Never => Some(RuntimeCheckedType::Never),
            RuntimeTypeShape::Unit => Some(RuntimeCheckedType::Unit),
            RuntimeTypeShape::Bool => Some(RuntimeCheckedType::Bool),
            RuntimeTypeShape::Signed(width) => Some(RuntimeCheckedType::Signed(*width)),
            RuntimeTypeShape::Unsigned(width) => Some(RuntimeCheckedType::Unsigned(*width)),
            RuntimeTypeShape::F32 => Some(RuntimeCheckedType::F32),
            RuntimeTypeShape::F64 => Some(RuntimeCheckedType::F64),
            RuntimeTypeShape::String => Some(RuntimeCheckedType::String),
            RuntimeTypeShape::Char => Some(RuntimeCheckedType::Char),
            RuntimeTypeShape::Bytes => Some(RuntimeCheckedType::Bytes),
            RuntimeTypeShape::Duration => Some(RuntimeCheckedType::Duration),
            RuntimeTypeShape::Progress => Some(RuntimeCheckedType::Progress),
            RuntimeTypeShape::EntityReference => Some(RuntimeCheckedType::EntityReference),
            RuntimeTypeShape::AgentValue => Some(RuntimeCheckedType::AgentValue),
            _ => None,
        }
    }

    fn unsupported(
        &self,
        path: &RuntimeTypeProjectionPath,
        shape: RuntimeUnsupportedTypeShape,
    ) -> RuntimeCheckedTypeProjectionError {
        RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: self.identity(),
            path: path.clone(),
            shape,
        }
    }
}

fn projection_index(index: usize) -> u32 {
    u32::try_from(index).expect("normalized type collections fit the u32 contract")
}

fn unsupported_runtime_shape(shape: &RuntimeTypeShape) -> Option<RuntimeUnsupportedTypeShape> {
    match shape {
        RuntimeTypeShape::Range(_) => Some(RuntimeUnsupportedTypeShape::Range),
        RuntimeTypeShape::Iterator(_) => Some(RuntimeUnsupportedTypeShape::Iterator),
        RuntimeTypeShape::Map { .. } => Some(RuntimeUnsupportedTypeShape::Map),
        RuntimeTypeShape::Need(_) => Some(RuntimeUnsupportedTypeShape::Need),
        RuntimeTypeShape::Stream { .. } => Some(RuntimeUnsupportedTypeShape::Stream),
        RuntimeTypeShape::Parser { .. } => Some(RuntimeUnsupportedTypeShape::Parser),
        RuntimeTypeShape::ThreadHandle(_) => Some(RuntimeUnsupportedTypeShape::ThreadHandle),
        RuntimeTypeShape::Shared(_) => Some(RuntimeUnsupportedTypeShape::Shared),
        RuntimeTypeShape::Reference(_) => Some(RuntimeUnsupportedTypeShape::Reference),
        RuntimeTypeShape::Function { .. } => Some(RuntimeUnsupportedTypeShape::Function),
        _ => None,
    }
}

impl RuntimeAgentTypeShape {
    fn runtime_plan_projection(&self) -> RuntimeAgentTypeProjection<RuntimeSemanticTypeId> {
        self.try_project(|value| Ok::<_, std::convert::Infallible>(value.identity()))
            .unwrap_or_else(|impossible| match impossible {})
    }

    fn try_project<R, E>(
        &self,
        mut project: impl FnMut(&RuntimeNormalizedType) -> Result<R, E>,
    ) -> Result<RuntimeAgentTypeProjection<R>, E> {
        Ok(match self {
            Self::DebugStatePath => RuntimeAgentTypeProjection::DebugStatePath,
            Self::ObservationFieldPath => RuntimeAgentTypeProjection::ObservationFieldPath,
            Self::Probe(value) => RuntimeAgentTypeProjection::Probe(project(value)?),
            Self::Predicate => RuntimeAgentTypeProjection::Predicate,
            Self::Observation => RuntimeAgentTypeProjection::Observation,
            Self::ObservedObject => RuntimeAgentTypeProjection::ObservedObject,
            Self::BoundingBox => RuntimeAgentTypeProjection::BoundingBox,
            Self::ActionName => RuntimeAgentTypeProjection::ActionName,
            Self::ActionTarget => RuntimeAgentTypeProjection::ActionTarget,
            Self::ActionResult => RuntimeAgentTypeProjection::ActionResult,
            Self::DataFormat => RuntimeAgentTypeProjection::DataFormat,
            Self::DataShape => RuntimeAgentTypeProjection::DataShape,
            Self::EntityMetadata => RuntimeAgentTypeProjection::EntityMetadata,
            Self::SourceAnchor => RuntimeAgentTypeProjection::SourceAnchor,
            Self::ProjectGraphNeighborhood => RuntimeAgentTypeProjection::ProjectGraphNeighborhood,
            Self::ProjectGraphSymbol => RuntimeAgentTypeProjection::ProjectGraphSymbol,
            Self::ProjectGraphEdge => RuntimeAgentTypeProjection::ProjectGraphEdge,
            Self::CaptureTarget => RuntimeAgentTypeProjection::CaptureTarget,
            Self::CaptureReference => RuntimeAgentTypeProjection::CaptureReference,
            Self::Resource => RuntimeAgentTypeProjection::Resource,
            Self::RagContextPack => RuntimeAgentTypeProjection::RagContextPack,
            Self::ObservedObjectId => RuntimeAgentTypeProjection::ObservedObjectId,
            Self::Diagnostics => RuntimeAgentTypeProjection::Diagnostics,
            Self::WaitError => RuntimeAgentTypeProjection::WaitError,
            Self::ViewportPoint => RuntimeAgentTypeProjection::ViewportPoint,
            Self::RagError => RuntimeAgentTypeProjection::RagError,
            Self::SourcePosition => RuntimeAgentTypeProjection::SourcePosition,
            Self::ProjectFlowControlSummary => {
                RuntimeAgentTypeProjection::ProjectFlowControlSummary
            }
            Self::ProjectGraphSummary => RuntimeAgentTypeProjection::ProjectGraphSummary,
            Self::BinaryResourceBody => RuntimeAgentTypeProjection::BinaryResourceBody,
            Self::BinaryData => RuntimeAgentTypeProjection::BinaryData,
        })
    }
}

impl RuntimeSequenceKind {
    const fn runtime_plan_kind(self) -> RuntimePlanSequenceKind {
        match self {
            Self::Vec => RuntimePlanSequenceKind::Vec,
            Self::Array => RuntimePlanSequenceKind::Array,
            Self::Slice => RuntimePlanSequenceKind::Slice,
            Self::Seq => RuntimePlanSequenceKind::Seq,
        }
    }
}

/// Exact project callable and its final-HIR owner item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallable {
    declaration: CallableDeclarationKey,
    owner: ItemId,
    source_owner: HirCallableSourceOwner,
    runtime: RuntimeCallableId,
    attached_content_abi: Option<RuntimeCallableAttachedContentAbi>,
}

impl RuntimeProjectCallable {
    pub fn try_new(
        declaration: CallableDeclarationKey,
        owner: ItemId,
        source_owner: HirCallableSourceOwner,
        runtime: RuntimeCallableId,
        attached_content_abi: Option<RuntimeCallableAttachedContentAbi>,
    ) -> Result<Self, RuntimeCallableAttachedContentAbiError> {
        if let Some(attached) = &attached_content_abi {
            attached.validate_owner(owner)?;
        }
        Ok(Self {
            declaration,
            owner,
            source_owner,
            runtime,
            attached_content_abi,
        })
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn source_owner(&self) -> HirCallableSourceOwner {
        self.source_owner
    }

    pub const fn runtime(&self) -> &RuntimeCallableId {
        &self.runtime
    }

    pub const fn attached_content_abi(&self) -> Option<&RuntimeCallableAttachedContentAbi> {
        self.attached_content_abi.as_ref()
    }
}

/// Exact declaration-side ABI for one callable-owned attached-content slot.
///
/// Generation-local HIR IDs are execution joins only. The checked default
/// coordinate/digest remain the semantic authority, so helper lowering never
/// adopts a call-site operand or reopens source/schema spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableAttachedContentAbi {
    group: arcweft_lang_sema::callable::CallableGroupIndex,
    abi_position: u32,
    presence: arcweft_lang_sema::callable::CallableParameterPresence,
    binding: LocalId,
    binding_ty: RuntimeNormalizedType,
    abi_ty: RuntimeNormalizedType,
    default: Option<RuntimeCallableAttachedContentDefault>,
}

impl RuntimeCallableAttachedContentAbi {
    pub fn try_new(
        group: arcweft_lang_sema::callable::CallableGroupIndex,
        abi_position: u32,
        presence: arcweft_lang_sema::callable::CallableParameterPresence,
        binding: LocalId,
        binding_ty: RuntimeNormalizedType,
        abi_ty: RuntimeNormalizedType,
        default: Option<RuntimeCallableAttachedContentDefault>,
    ) -> Result<Self, RuntimeCallableAttachedContentAbiError> {
        use arcweft_lang_sema::callable::CallableParameterPresence;

        let valid = match presence {
            CallableParameterPresence::Required => binding_ty == abi_ty && default.is_none(),
            CallableParameterPresence::Optional => {
                binding_ty == abi_ty
                    && matches!(abi_ty.shape(), RuntimeTypeShape::Option { .. })
                    && default.is_none()
            }
            CallableParameterPresence::Defaulted => {
                matches!(
                    abi_ty.shape(),
                    RuntimeTypeShape::Option { item, .. } if item.as_ref() == &binding_ty
                ) && default.is_some()
            }
        };
        if !valid {
            return Err(RuntimeCallableAttachedContentAbiError::InvalidPresenceShape);
        }
        Ok(Self {
            group,
            abi_position,
            presence,
            binding,
            binding_ty,
            abi_ty,
            default,
        })
    }

    fn validate_owner(&self, owner: ItemId) -> Result<(), RuntimeCallableAttachedContentAbiError> {
        if self.binding.module() != owner.module()
            || self
                .default
                .as_ref()
                .is_some_and(|default| default.source().module() != owner.module())
        {
            return Err(RuntimeCallableAttachedContentAbiError::ForeignOwner);
        }
        Ok(())
    }

    pub const fn presence(&self) -> arcweft_lang_sema::callable::CallableParameterPresence {
        self.presence
    }

    pub const fn group(&self) -> arcweft_lang_sema::callable::CallableGroupIndex {
        self.group
    }

    pub const fn abi_position(&self) -> u32 {
        self.abi_position
    }

    pub const fn binding(&self) -> LocalId {
        self.binding
    }

    pub const fn binding_ty(&self) -> &RuntimeNormalizedType {
        &self.binding_ty
    }

    pub const fn abi_ty(&self) -> &RuntimeNormalizedType {
        &self.abi_ty
    }

    pub const fn default(&self) -> Option<&RuntimeCallableAttachedContentDefault> {
        self.default.as_ref()
    }

    pub(crate) fn append_runtime_plan_type_seeds(
        &self,
        seeds: &mut Vec<RuntimePlanTypeSeed>,
    ) -> Result<(), RuntimeCheckedTypeProjectionError> {
        self.binding_ty.append_runtime_plan_type_seeds(seeds)?;
        self.abi_ty.append_runtime_plan_type_seeds(seeds)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableAttachedContentDefault {
    source: ExprId,
    coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
    digest: arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest,
}

impl RuntimeCallableAttachedContentDefault {
    pub const fn new(
        source: ExprId,
        coordinate: arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate,
        digest: arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest,
    ) -> Self {
        Self {
            source,
            coordinate,
            digest,
        }
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn coordinate(
        &self,
    ) -> &arcweft_lang_sema::semantic_coordinate::StableCheckedValueCoordinate {
        &self.coordinate
    }

    pub const fn digest(
        &self,
    ) -> arcweft_lang_sema::callable::CheckedAttachedContentDefaultExpressionDigest {
        self.digest
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCallableAttachedContentAbiError {
    #[error("runtime callable attached-content presence/type/default rows are inconsistent")]
    InvalidPresenceShape,
    #[error("runtime callable attached-content binding or default belongs to another HIR owner")]
    ForeignOwner,
}

/// Exact project nominal and its final-HIR owner item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominal {
    declaration: ProjectNominalDeclarationId,
    owner: ItemId,
    runtime_nominal: RuntimeNominalTypeId,
    identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}

impl RuntimeResolvedNominal {
    pub const fn new(
        declaration: ProjectNominalDeclarationId,
        owner: ItemId,
        runtime_nominal: RuntimeNominalTypeId,
        identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Self {
        Self {
            declaration,
            owner,
            runtime_nominal,
            identity,
            layout,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn identity(&self) -> RuntimeSemanticTypeId {
        self.identity
    }

    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    #[must_use]
    pub fn runtime_nominal_id(&self) -> RuntimeNominalTypeId {
        self.runtime_nominal.clone()
    }

    #[must_use]
    /// Projects the checked nominal owner retained by this accepted fact.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted project declaration violates the invariant
    /// that its qualified name is a valid runtime nominal identity.
    pub fn checked_type(&self) -> RuntimeCheckedType {
        RuntimeCheckedType::Nominal {
            nominal: self.runtime_nominal_id(),
            semantic_identity: self.identity,
            layout: self.layout,
            arguments: Vec::new(),
        }
    }
}

/// Complete writable-record-field decision for one final-HIR assignment.
///
/// The compiler projects this once from semantic analysis. Runtime lowerers
/// consume its local base, exact nominal layout identity, field ordinal, and
/// normalized operand types without reinterpreting HIR place syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssignmentFact {
    base: LocalId,
    nominal: RuntimeResolvedNominal,
    field: RuntimeRecordFieldId,
    field_type: RuntimeNormalizedType,
    value_type: RuntimeNormalizedType,
}

/// One source-ordered Pending observer projected from checked semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAwaitPendingObserverFact {
    pattern: PatternId,
}

impl RuntimeAwaitPendingObserverFact {
    pub const fn new(pattern: PatternId) -> Self {
        Self { pattern }
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }
}

/// Checked temporal operand and its Pending observers for one Await expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAwaitFact {
    operand: ExprId,
    observers: Box<[RuntimeAwaitPendingObserverFact]>,
}

/// Exact runtime Flow selected for one compact Choice `goto` arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChoiceGotoFact {
    arm: u32,
    target: RuntimeProjectItem,
}

impl RuntimeChoiceGotoFact {
    pub const fn new(arm: u32, target: RuntimeProjectItem) -> Self {
        Self { arm, target }
    }

    pub const fn arm(&self) -> u32 {
        self.arm
    }

    pub const fn target(&self) -> &RuntimeProjectItem {
        &self.target
    }
}

/// Generation-bound semantic additions consumed when lowering one Choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeChoiceFact {
    public_id: Option<PublicId>,
    option_ids: Box<[PublicId]>,
    gotos: Box<[RuntimeChoiceGotoFact]>,
}

impl RuntimeChoiceFact {
    pub fn new(
        public_id: Option<PublicId>,
        option_ids: impl Into<Box<[PublicId]>>,
        gotos: impl Into<Box<[RuntimeChoiceGotoFact]>>,
    ) -> Self {
        Self {
            public_id,
            option_ids: option_ids.into(),
            gotos: gotos.into(),
        }
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub fn option_ids(&self) -> &[PublicId] {
        &self.option_ids
    }

    pub fn gotos(&self) -> &[RuntimeChoiceGotoFact] {
        &self.gotos
    }

    pub fn goto_for_arm(&self, arm: u32) -> Option<&RuntimeProjectItem> {
        self.gotos
            .binary_search_by_key(&arm, RuntimeChoiceGotoFact::arm)
            .ok()
            .map(|index| self.gotos[index].target())
    }
}

impl RuntimeAwaitFact {
    pub fn new(
        operand: ExprId,
        observers: impl Into<Box<[RuntimeAwaitPendingObserverFact]>>,
    ) -> Self {
        Self {
            operand,
            observers: observers.into(),
        }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub fn observers(&self) -> &[RuntimeAwaitPendingObserverFact] {
        &self.observers
    }
}

/// Closed carrier consumed by one checked prefix Try expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTryCarrierFact {
    Result {
        success: RuntimeNormalizedType,
        residual: Box<RuntimeNormalizedType>,
    },
    Option {
        success: RuntimeNormalizedType,
    },
}

impl RuntimeTryCarrierFact {
    pub const fn success(&self) -> &RuntimeNormalizedType {
        match self {
            Self::Result { success, .. } | Self::Option { success } => success,
        }
    }

    pub fn residual(&self) -> Option<&RuntimeNormalizedType> {
        match self {
            Self::Result { residual, .. } => Some(residual.as_ref()),
            Self::Option { .. } => None,
        }
    }
}

/// Exact lexical owner that receives one checked Try residual.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAcceptedDeclarationSemanticId([u8; 32]);

impl RuntimeAcceptedDeclarationSemanticId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact lexical owner that receives one checked Try residual.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTryBoundaryOwner {
    Infallible,
    CarrierBlock(ExprId),
    ExplicitFunctionSite(ExprId),
    ImplicitFunctionSite(ExprId),
    Callable(RuntimeAcceptedDeclarationSemanticId),
}

/// Generation-bound Try carrier and propagation-boundary fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTryFact {
    operand: ExprId,
    carrier_type: RuntimeNormalizedType,
    carrier: RuntimeTryCarrierFact,
    boundary: RuntimeTryBoundaryOwner,
    boundary_type: RuntimeNormalizedType,
}

/// Generation-bound implicit callable projection for one `_` abstraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeImplicitCallableFact {
    parameter: RuntimeNormalizedType,
    result: RuntimeNormalizedType,
    placeholders: Box<[ExprId]>,
    captures: Box<[LocalId]>,
}

impl RuntimeImplicitCallableFact {
    pub const fn new(
        parameter: RuntimeNormalizedType,
        result: RuntimeNormalizedType,
        placeholders: Box<[ExprId]>,
        captures: Box<[LocalId]>,
    ) -> Self {
        Self {
            parameter,
            result,
            placeholders,
            captures,
        }
    }

    pub const fn parameter(&self) -> &RuntimeNormalizedType {
        &self.parameter
    }

    pub const fn result(&self) -> &RuntimeNormalizedType {
        &self.result
    }

    pub const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
    }

    pub const fn captures(&self) -> &[LocalId] {
        &self.captures
    }
}

/// Generation-bound once-only pipeline and its checked `^` uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePipeFact {
    left: ExprId,
    right: ExprId,
    placeholders: Box<[ExprId]>,
}

impl RuntimePipeFact {
    pub const fn new(left: ExprId, right: ExprId, placeholders: Box<[ExprId]>) -> Self {
        Self {
            left,
            right,
            placeholders,
        }
    }

    pub const fn left(&self) -> ExprId {
        self.left
    }

    pub const fn right(&self) -> ExprId {
        self.right
    }

    pub const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
    }
}

impl RuntimeTryFact {
    pub const fn new(
        operand: ExprId,
        carrier_type: RuntimeNormalizedType,
        carrier: RuntimeTryCarrierFact,
        boundary: RuntimeTryBoundaryOwner,
        boundary_type: RuntimeNormalizedType,
    ) -> Self {
        Self {
            operand,
            carrier_type,
            carrier,
            boundary,
            boundary_type,
        }
    }

    pub const fn operand(&self) -> ExprId {
        self.operand
    }

    pub const fn carrier_type(&self) -> &RuntimeNormalizedType {
        &self.carrier_type
    }

    pub const fn carrier(&self) -> &RuntimeTryCarrierFact {
        &self.carrier
    }

    pub const fn boundary(&self) -> RuntimeTryBoundaryOwner {
        self.boundary
    }

    pub const fn boundary_type(&self) -> &RuntimeNormalizedType {
        &self.boundary_type
    }
}

fn try_boundary_type_matches(fact: &RuntimeTryFact) -> bool {
    match (fact.carrier(), fact.boundary_type().shape()) {
        (
            RuntimeTryCarrierFact::Result { residual, .. },
            RuntimeTypeShape::Result { error, .. },
        ) => {
            residual.as_ref() == error.as_ref()
                || matches!(residual.shape(), RuntimeTypeShape::Never)
        }
        (RuntimeTryCarrierFact::Option { .. }, RuntimeTypeShape::Option { .. }) => true,
        _ => false,
    }
}

/// Checked iterator dispatch before plan-local type and method IDs are issued.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorFact {
    Builtin(Box<RuntimeBuiltinIteratorFact>),
    Witness(Box<RuntimeIteratorWitnessFact>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBuiltinIteratorFact {
    family: RuntimeBuiltinIteratorFamily,
    item: RuntimeNormalizedType,
    iterator: RuntimeNormalizedType,
    next_value: RuntimeNormalizedType,
    step: RuntimeNormalizedType,
}

impl RuntimeBuiltinIteratorFact {
    pub const fn new(
        family: RuntimeBuiltinIteratorFamily,
        item: RuntimeNormalizedType,
        iterator: RuntimeNormalizedType,
        next_value: RuntimeNormalizedType,
        step: RuntimeNormalizedType,
    ) -> Self {
        Self {
            family,
            item,
            iterator,
            next_value,
            step,
        }
    }

    pub const fn family(&self) -> RuntimeBuiltinIteratorFamily {
        self.family
    }

    pub const fn item(&self) -> &RuntimeNormalizedType {
        &self.item
    }

    pub const fn iterator(&self) -> &RuntimeNormalizedType {
        &self.iterator
    }

    pub const fn next_value(&self) -> &RuntimeNormalizedType {
        &self.next_value
    }

    pub const fn step(&self) -> &RuntimeNormalizedType {
        &self.step
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIteratorWitnessFact {
    item: RuntimeNormalizedType,
    iterator: RuntimeNormalizedType,
    executable: RuntimeIteratorWitnessExecutableFact,
}

impl RuntimeIteratorWitnessFact {
    pub const fn new(
        item: RuntimeNormalizedType,
        iterator: RuntimeNormalizedType,
        executable: RuntimeIteratorWitnessExecutableFact,
    ) -> Self {
        Self {
            item,
            iterator,
            executable,
        }
    }

    pub const fn item(&self) -> &RuntimeNormalizedType {
        &self.item
    }

    pub const fn iterator(&self) -> &RuntimeNormalizedType {
        &self.iterator
    }

    pub const fn executable(&self) -> &RuntimeIteratorWitnessExecutableFact {
        &self.executable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorWitnessExecutableFact {
    TraitCalls {
        into_iter: ImplMethodDeclarationId,
        next: ImplMethodDeclarationId,
    },
    IdentityIntoIterator {
        next: ImplMethodDeclarationId,
    },
}

impl RuntimeAssignmentFact {
    pub const fn new(
        base: LocalId,
        nominal: RuntimeResolvedNominal,
        field: RuntimeRecordFieldId,
        field_type: RuntimeNormalizedType,
        value_type: RuntimeNormalizedType,
    ) -> Self {
        Self {
            base,
            nominal,
            field,
            field_type,
            value_type,
        }
    }

    pub const fn base(&self) -> LocalId {
        self.base
    }

    pub const fn nominal(&self) -> &RuntimeResolvedNominal {
        &self.nominal
    }

    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    pub const fn field_type(&self) -> &RuntimeNormalizedType {
        &self.field_type
    }

    pub const fn value_type(&self) -> &RuntimeNormalizedType {
        &self.value_type
    }
}

/// One checked nominal-record fact paired with its executable field layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominalRecord {
    nominal: RuntimeResolvedNominal,
    layout: Arc<RuntimeNominalRecordLayout>,
    fields: Box<[RuntimeResolvedNominalRecordField]>,
}

/// One defining-order nominal-record field with its exact normalized type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominalRecordField {
    name: String,
    ty: RuntimeNormalizedType,
}

/// Runtime-only source retained from one C2-sealed record-expression row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRecordExpressionSource {
    Expression(ExprId),
    Binding(LocalId),
}

/// One source-ordered executable record-expression field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeRecordExpressionField {
    field: RuntimeRecordFieldId,
    source: RuntimeRecordExpressionSource,
}

impl RuntimeRecordExpressionField {
    pub const fn new(field: RuntimeRecordFieldId, source: RuntimeRecordExpressionSource) -> Self {
        Self { field, source }
    }

    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    pub const fn source(&self) -> RuntimeRecordExpressionSource {
        self.source
    }
}

/// Atomic executable projection of one nominal-record expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRecordExpressionFact {
    nominal: RuntimeResolvedNominalRecord,
    fields: Box<[RuntimeRecordExpressionField]>,
}

/// Runtime-only source retained from one C2-sealed record-pattern row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRecordPatternSource {
    Pattern(PatternId),
    Binding(LocalId),
}

/// One source-ordered executable record-pattern field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeRecordPatternField {
    field: RuntimeRecordFieldId,
    source: RuntimeRecordPatternSource,
}

impl RuntimeRecordPatternField {
    pub const fn new(field: RuntimeRecordFieldId, source: RuntimeRecordPatternSource) -> Self {
        Self { field, source }
    }

    pub const fn field(&self) -> RuntimeRecordFieldId {
        self.field
    }

    pub const fn source(&self) -> RuntimeRecordPatternSource {
        self.source
    }
}

/// Exact runtime disposition of an authored record-pattern rest row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeRecordPatternRest {
    Absent,
    Ignore,
    Binding(LocalId),
}

/// Atomic executable projection of one nominal-record pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRecordPatternFact {
    owner: RuntimeRecordPatternOwner,
    fields: Box<[RuntimeRecordPatternField]>,
    rest: RuntimeRecordPatternRest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeRecordPatternOwner {
    Nominal(RuntimeResolvedNominalRecord),
    Structural(RuntimeNormalizedType),
}

/// Invalid executable record plan projected from checked semantics.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeRecordPlanError {
    #[error("record plan contains field {field} more than once")]
    DuplicateField { field: RuntimeRecordFieldId },
    #[error("record plan references field {field} outside its nominal layout")]
    UnknownField { field: RuntimeRecordFieldId },
    #[error("complete record plan has {actual} fields, expected {expected}")]
    Incomplete { expected: usize, actual: usize },
    #[error("structural record pattern owner is not an exact checked record type")]
    InvalidStructuralOwner,
}

fn validate_runtime_record_fields(
    field_count: usize,
    fields: impl IntoIterator<Item = RuntimeRecordFieldId>,
    require_complete: bool,
) -> Result<(), RuntimeRecordPlanError> {
    let mut seen = BTreeSet::new();
    for field in fields {
        if usize::try_from(field.zero_based()).map_or(true, |ordinal| ordinal >= field_count) {
            return Err(RuntimeRecordPlanError::UnknownField { field });
        }
        if !seen.insert(field) {
            return Err(RuntimeRecordPlanError::DuplicateField { field });
        }
    }
    if require_complete && seen.len() != field_count {
        return Err(RuntimeRecordPlanError::Incomplete {
            expected: field_count,
            actual: seen.len(),
        });
    }
    Ok(())
}

impl RuntimeRecordExpressionFact {
    pub fn try_new(
        nominal: RuntimeResolvedNominalRecord,
        fields: impl Into<Box<[RuntimeRecordExpressionField]>>,
    ) -> Result<Self, RuntimeRecordPlanError> {
        let fields = fields.into();
        validate_runtime_record_fields(
            nominal.layout().len(),
            fields.iter().map(|field| field.field),
            true,
        )?;
        Ok(Self { nominal, fields })
    }

    pub const fn nominal(&self) -> &RuntimeResolvedNominalRecord {
        &self.nominal
    }

    pub const fn fields(&self) -> &[RuntimeRecordExpressionField] {
        &self.fields
    }

    fn nominal_mut(&mut self) -> &mut RuntimeResolvedNominalRecord {
        &mut self.nominal
    }
}

impl RuntimeRecordPatternFact {
    pub fn try_new(
        nominal: RuntimeResolvedNominalRecord,
        fields: impl Into<Box<[RuntimeRecordPatternField]>>,
        rest: RuntimeRecordPatternRest,
    ) -> Result<Self, RuntimeRecordPlanError> {
        let fields = fields.into();
        validate_runtime_record_fields(
            nominal.layout().len(),
            fields.iter().map(|field| field.field),
            matches!(rest, RuntimeRecordPatternRest::Absent),
        )?;
        Ok(Self {
            owner: RuntimeRecordPatternOwner::Nominal(nominal),
            fields,
            rest,
        })
    }

    pub fn try_structural(
        owner: RuntimeNormalizedType,
        fields: impl Into<Box<[RuntimeRecordPatternField]>>,
        rest: RuntimeRecordPatternRest,
    ) -> Result<Self, RuntimeRecordPlanError> {
        let RuntimeTypeShape::Record(schema) = owner.shape() else {
            return Err(RuntimeRecordPlanError::InvalidStructuralOwner);
        };
        if !matches!(owner.checked_type(), Ok(RuntimeCheckedType::Record(_))) {
            return Err(RuntimeRecordPlanError::InvalidStructuralOwner);
        }
        let fields = fields.into();
        validate_runtime_record_fields(
            schema.len(),
            fields.iter().map(|field| field.field),
            matches!(rest, RuntimeRecordPatternRest::Absent),
        )?;
        Ok(Self {
            owner: RuntimeRecordPatternOwner::Structural(owner),
            fields,
            rest,
        })
    }

    pub const fn nominal(&self) -> Option<&RuntimeResolvedNominalRecord> {
        match &self.owner {
            RuntimeRecordPatternOwner::Nominal(nominal) => Some(nominal),
            RuntimeRecordPatternOwner::Structural(_) => None,
        }
    }

    pub const fn structural(&self) -> Option<&RuntimeNormalizedType> {
        match &self.owner {
            RuntimeRecordPatternOwner::Structural(ty) => Some(ty),
            RuntimeRecordPatternOwner::Nominal(_) => None,
        }
    }

    pub const fn fields(&self) -> &[RuntimeRecordPatternField] {
        &self.fields
    }

    pub const fn rest(&self) -> RuntimeRecordPatternRest {
        self.rest
    }

    fn nominal_mut(&mut self) -> Option<&mut RuntimeResolvedNominalRecord> {
        match &mut self.owner {
            RuntimeRecordPatternOwner::Nominal(nominal) => Some(nominal),
            RuntimeRecordPatternOwner::Structural(_) => None,
        }
    }
}

/// Failure to pair a nominal fact with an executable record layout.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordFactError {
    #[error("nominal record fact has runtime identity {actual:?}, expected {expected:?}")]
    NominalIdentity {
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error("nominal record fact has a different semantic identity")]
    SemanticIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("nominal record fact has a different layout identity")]
    LayoutIdentity {
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
    #[error("project record fact requires a named record layout, received {actual:?}")]
    SourceShape { actual: RuntimeNominalRecordShape },
    #[error("nominal record fact has {actual} normalized fields, expected {expected}")]
    FieldCount { expected: usize, actual: usize },
    #[error("nominal record field {ordinal} resolved as `{actual}`, expected {expected:?}")]
    FieldName {
        ordinal: usize,
        expected: Option<String>,
        actual: String,
    },
    #[error("nominal record field `{name}` has a different checked projection")]
    FieldType { name: String },
}

impl RuntimeResolvedNominalRecord {
    /// Pairs one accepted nominal fact with its executable record layout.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted project declaration violates the invariant
    /// that its qualified name is a valid runtime nominal identity.
    pub fn try_new(
        nominal: RuntimeResolvedNominal,
        layout: Arc<RuntimeNominalRecordLayout>,
        fields: impl IntoIterator<Item = (String, RuntimeNormalizedType)>,
    ) -> Result<Self, RuntimeNominalRecordFactError> {
        let expected = nominal.runtime_nominal_id();
        if layout.nominal() != &expected {
            return Err(RuntimeNominalRecordFactError::NominalIdentity {
                expected,
                actual: layout.nominal().clone(),
            });
        }
        if layout.semantic_identity() != nominal.identity() {
            return Err(RuntimeNominalRecordFactError::SemanticIdentity {
                expected: nominal.identity(),
                actual: layout.semantic_identity(),
            });
        }
        if layout.layout() != nominal.layout() {
            return Err(RuntimeNominalRecordFactError::LayoutIdentity {
                expected: nominal.layout(),
                actual: layout.layout(),
            });
        }
        if layout.shape() != RuntimeNominalRecordShape::Record {
            return Err(RuntimeNominalRecordFactError::SourceShape {
                actual: layout.shape(),
            });
        }
        let fields = fields
            .into_iter()
            .map(|(name, ty)| RuntimeResolvedNominalRecordField { name, ty })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        if fields.len() != layout.fields().len() {
            return Err(RuntimeNominalRecordFactError::FieldCount {
                expected: layout.fields().len(),
                actual: fields.len(),
            });
        }
        for (ordinal, (field, accepted)) in fields.iter().zip(layout.fields()).enumerate() {
            if Some(field.name.as_str()) != accepted.name() {
                return Err(RuntimeNominalRecordFactError::FieldName {
                    ordinal,
                    expected: accepted.name().map(str::to_owned),
                    actual: field.name.clone(),
                });
            }
            if field.ty.checked_type().ok().as_ref() != Some(accepted.checked_type()) {
                return Err(RuntimeNominalRecordFactError::FieldType {
                    name: field.name.clone(),
                });
            }
        }
        Ok(Self {
            nominal,
            layout,
            fields,
        })
    }

    pub const fn nominal(&self) -> &RuntimeResolvedNominal {
        &self.nominal
    }

    pub const fn layout(&self) -> &Arc<RuntimeNominalRecordLayout> {
        &self.layout
    }

    pub const fn fields(&self) -> &[RuntimeResolvedNominalRecordField] {
        &self.fields
    }

    #[must_use]
    pub fn checked_type(&self) -> RuntimeCheckedType {
        self.layout.checked_type()
    }
}

impl RuntimeResolvedNominalRecordField {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// Stable entity identity paired with its closed accepted owner kind.
///
/// Retained declarations keep their exact HIR owner. Registered Characters
/// keep the externally validated Character public identity without fabricating
/// an [`ItemId`]. Runtime lowering never reconstructs either identity from
/// source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectItemOwner {
    Retained(ItemId),
    StructuralFlow {
        owner: ItemId,
        runtime: FlowRuntimeId,
    },
    ExternalCharacter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectItem {
    public_id: PublicId,
    family: DeclarationIdentityFamily,
    owner: RuntimeProjectItemOwner,
}

impl RuntimeProjectItem {
    pub fn new_retained(
        public_id: PublicId,
        family: DeclarationIdentityFamily,
        owner: ItemId,
    ) -> Self {
        Self {
            public_id,
            family,
            owner: RuntimeProjectItemOwner::Retained(owner),
        }
    }

    pub fn new_external_character(public_id: PublicId) -> Self {
        Self {
            public_id,
            family: DeclarationIdentityFamily::Character,
            owner: RuntimeProjectItemOwner::ExternalCharacter,
        }
    }

    pub fn new_structural_flow(public_id: PublicId, owner: ItemId, runtime: FlowRuntimeId) -> Self {
        Self {
            public_id,
            family: DeclarationIdentityFamily::Flow,
            owner: RuntimeProjectItemOwner::StructuralFlow { owner, runtime },
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        self.family
    }

    pub const fn owner(&self) -> &RuntimeProjectItemOwner {
        &self.owner
    }

    pub const fn retained_owner(&self) -> Option<ItemId> {
        match &self.owner {
            RuntimeProjectItemOwner::Retained(owner) => Some(*owner),
            RuntimeProjectItemOwner::StructuralFlow { .. }
            | RuntimeProjectItemOwner::ExternalCharacter => None,
        }
    }

    pub const fn flow_runtime_id(&self) -> Option<&FlowRuntimeId> {
        match &self.owner {
            RuntimeProjectItemOwner::StructuralFlow { runtime, .. } => Some(runtime),
            RuntimeProjectItemOwner::Retained(_) | RuntimeProjectItemOwner::ExternalCharacter => {
                None
            }
        }
    }
}

/// Checked meaning of one final-HIR path expression.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeResolvedValue {
    Local(LocalId),
    ProjectCallable(RuntimeProjectCallable),
    ProjectItem(RuntimeProjectItem),
    /// Checked one-way lowering of a durable `say.*` identity into the
    /// path-only runtime line domain.
    DialogueLine(RuntimeLineId),
    CharacterLook {
        character: arcweft_character::id::CharacterId,
        look: arcweft_character::id::CharacterLookId,
    },
    Intrinsic(RuntimeIntrinsic),
    Registered(RuntimeRegisteredValueId),
    Constant(RuntimeValue),
}

/// Checked projection selected for one final-HIR member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedSelect {
    Method,
    Field {
        owner: RuntimeSemanticTypeId,
        field: RuntimeRecordFieldId,
    },
    OpaqueRecord {
        owner: RuntimeSemanticTypeId,
        producer: RuntimeOpaqueTypeProducerId,
        field: RuntimeRecordFieldId,
        field_type: RuntimeSemanticTypeId,
    },
    AgentField {
        field: RuntimeAgentField,
    },
    ProgressField {
        field: arcweft_core::value::RuntimeProgressField,
    },
}

/// One source-ordered case in a complete normalized runtime variant schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNormalizedVariantCase {
    name: String,
    payload: Option<Box<RuntimeNormalizedType>>,
}

/// Source-ordered variant cases whose cardinality is representable by every
/// runtime ordinal and wire boundary.
///
/// The private row/count pair is sealed together so downstream selection and
/// diagnostics never need a lossy `usize -> u32` conversion or a saturated
/// success value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNormalizedVariantCases {
    rows: Box<[RuntimeNormalizedVariantCase]>,
    count: u32,
}

impl RuntimeNormalizedVariantCases {
    fn try_new(
        rows: Box<[RuntimeNormalizedVariantCase]>,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let count = u32::try_from(rows.len())
            .map_err(|_| RuntimeResolvedVariantError::CaseCountOverflow { count: rows.len() })?;
        Ok(Self { rows, count })
    }

    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }
}

impl std::ops::Deref for RuntimeNormalizedVariantCases {
    type Target = [RuntimeNormalizedVariantCase];

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl AsRef<[RuntimeNormalizedVariantCase]> for RuntimeNormalizedVariantCases {
    fn as_ref(&self) -> &[RuntimeNormalizedVariantCase] {
        &self.rows
    }
}

impl RuntimeNormalizedVariantCase {
    #[must_use]
    pub fn new(name: impl Into<String>, payload: Option<RuntimeNormalizedType>) -> Self {
        Self {
            name: name.into(),
            payload: payload.map(Box::new),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn payload(&self) -> Option<&RuntimeNormalizedType> {
        self.payload.as_deref()
    }

    fn checked_case(&self) -> Result<RuntimeCheckedVariantCase, RuntimeCheckedTypeProjectionError> {
        Ok(RuntimeCheckedVariantCase {
            name: self.name.clone(),
            payload: self
                .payload()
                .map(RuntimeNormalizedType::checked_type)
                .transpose()?
                .map(Box::new),
        })
    }
}

#[derive(Clone, Copy)]
struct RuntimeNormalizedVariantCaseRef<'a> {
    name: &'a str,
    payload: Option<&'a RuntimeNormalizedType>,
}

impl<'a> RuntimeNormalizedVariantCaseRef<'a> {
    fn from_case(case: &'a RuntimeNormalizedVariantCase) -> Self {
        Self {
            name: case.name(),
            payload: case.payload(),
        }
    }
}

/// Exact semantic owner of one runtime enum case.
///
/// Project, Character, and base-environment variants retain one complete
/// normalized case table. Checked cases are derived views rather than a
/// parallel payload authority. Option and Result retain their normalized type
/// arguments and expose the same internal source-ordered selection algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "variant owners retain their complete normalized semantic arguments as immutable checker evidence without adding a second indirection contract"
)]
pub enum RuntimeVariantOwner {
    Project {
        nominal: RuntimeResolvedNominal,
        arguments: Box<[RuntimeNormalizedType]>,
        cases: RuntimeNormalizedVariantCases,
    },
    CharacterNominal {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: RuntimeNormalizedVariantCases,
    },
    BuiltinClosed {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: RuntimeNormalizedVariantCases,
    },
    RuntimeBuiltin {
        identity: RuntimeSemanticTypeId,
        owner: RuntimeBuiltinVariantIdentity,
        cases: RuntimeNormalizedVariantCases,
    },
    Option {
        identity: RuntimeSemanticTypeId,
        item: RuntimeNormalizedType,
        cases: RuntimeNormalizedVariantCases,
    },
    Result {
        identity: RuntimeSemanticTypeId,
        ok: RuntimeNormalizedType,
        error: RuntimeNormalizedType,
        cases: RuntimeNormalizedVariantCases,
    },
}

impl RuntimeVariantOwner {
    fn append_normalized_types<'a>(&'a self, types: &mut Vec<&'a RuntimeNormalizedType>) {
        match self {
            Self::Project {
                arguments, cases, ..
            } => {
                types.extend(arguments.iter());
                types.extend(
                    cases
                        .iter()
                        .filter_map(RuntimeNormalizedVariantCase::payload),
                );
            }
            Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. }
            | Self::RuntimeBuiltin { cases, .. } => {
                types.extend(
                    cases
                        .iter()
                        .filter_map(RuntimeNormalizedVariantCase::payload),
                );
            }
            Self::Option { item, cases, .. } => {
                types.push(item);
                types.extend(
                    cases
                        .iter()
                        .filter_map(RuntimeNormalizedVariantCase::payload),
                );
            }
            Self::Result {
                ok, error, cases, ..
            } => {
                types.push(ok);
                types.push(error);
                types.extend(
                    cases
                        .iter()
                        .filter_map(RuntimeNormalizedVariantCase::payload),
                );
            }
        }
    }

    fn runtime_plan_domain_seed(&self) -> Option<RuntimeVariantDomainSeed> {
        let (owner, nominal, cases) = match self {
            Self::Project { nominal, cases, .. } => (
                nominal.identity(),
                nominal.runtime_nominal_id(),
                cases.as_ref(),
            ),
            Self::CharacterNominal {
                identity,
                nominal,
                cases,
            }
            | Self::BuiltinClosed {
                identity,
                nominal,
                cases,
            } => (*identity, nominal.clone(), cases.as_ref()),
            Self::RuntimeBuiltin { .. } => return None,
            Self::Option { .. } | Self::Result { .. } => return None,
        };
        Some(RuntimeVariantDomainSeed::new(
            owner,
            nominal,
            cases.iter().map(|case| {
                RuntimeVariantCaseSeed::new(
                    case.name(),
                    case.payload().map(RuntimeNormalizedType::identity),
                )
            }),
        ))
    }

    fn selected_case(
        &self,
        ordinal: u32,
    ) -> Result<RuntimeNormalizedVariantCaseRef<'_>, RuntimeResolvedVariantError> {
        let ordinal_index = usize::try_from(ordinal).ok();
        let selected = match self {
            Self::Project { cases, .. }
            | Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. } => ordinal_index
                .and_then(|ordinal| cases.get(ordinal))
                .map(RuntimeNormalizedVariantCaseRef::from_case),
            Self::RuntimeBuiltin { cases, .. } => ordinal_index
                .and_then(|ordinal| cases.get(ordinal))
                .map(RuntimeNormalizedVariantCaseRef::from_case),
            Self::Option { cases, .. } | Self::Result { cases, .. } => ordinal_index
                .and_then(|ordinal| cases.get(ordinal))
                .map(RuntimeNormalizedVariantCaseRef::from_case),
        };
        selected.ok_or(RuntimeResolvedVariantError::CaseOrdinal {
            ordinal,
            case_count: self.case_count(),
        })
    }

    fn case_count(&self) -> u32 {
        match self {
            Self::Project { cases, .. }
            | Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. }
            | Self::RuntimeBuiltin { cases, .. } => cases.count(),
            Self::Option { cases, .. } | Self::Result { cases, .. } => cases.count(),
        }
    }

    fn project_checked_type(
        &self,
    ) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        Ok(match self {
            Self::Project {
                nominal,
                arguments,
                cases,
            } => RuntimeCheckedType::Variant {
                owner: arcweft_core::pattern::RuntimeVariantIdentity::Nominal {
                    nominal: nominal.runtime_nominal_id(),
                    semantic_identity: nominal.identity(),
                },
                arguments: arguments
                    .iter()
                    .map(RuntimeNormalizedType::checked_type)
                    .collect::<Result<Vec<_>, _>>()?,
                cases: cases
                    .iter()
                    .map(RuntimeNormalizedVariantCase::checked_case)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Self::CharacterNominal {
                identity,
                nominal,
                cases,
            }
            | Self::BuiltinClosed {
                identity,
                nominal,
                cases,
            } => RuntimeCheckedType::Variant {
                owner: arcweft_core::pattern::RuntimeVariantIdentity::Nominal {
                    nominal: nominal.clone(),
                    semantic_identity: *identity,
                },
                arguments: Vec::new(),
                cases: cases
                    .iter()
                    .map(RuntimeNormalizedVariantCase::checked_case)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Self::RuntimeBuiltin {
                identity,
                owner,
                cases,
            } => {
                let payloads = cases
                    .iter()
                    .map(|case| {
                        case.payload()
                            .map(RuntimeNormalizedType::checked_type)
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                RuntimeCheckedType::try_builtin_variant(*owner, payloads).map_err(|_| {
                    RuntimeCheckedTypeProjectionError::InvalidBuiltinVariant {
                        semantic_identity: *identity,
                        path: RuntimeTypeProjectionPath::default(),
                        owner: *owner,
                    }
                })?
            }
            Self::Option {
                identity,
                item,
                cases,
            } => {
                let checked = RuntimeCheckedType::Option(Box::new(item.checked_type()?));
                validate_normalized_builtin_cases(
                    *identity,
                    RuntimeBuiltinVariantIdentity::Option,
                    cases,
                    &checked,
                )?;
                checked
            }
            Self::Result {
                identity,
                ok,
                error,
                cases,
            } => {
                let checked = RuntimeCheckedType::Result {
                    ok: Box::new(ok.checked_type()?),
                    error: Box::new(error.checked_type()?),
                };
                validate_normalized_builtin_cases(
                    *identity,
                    RuntimeBuiltinVariantIdentity::Result,
                    cases,
                    &checked,
                )?;
                checked
            }
        })
    }
}

fn validate_normalized_builtin_cases(
    semantic_identity: RuntimeSemanticTypeId,
    owner: RuntimeBuiltinVariantIdentity,
    cases: &[RuntimeNormalizedVariantCase],
    checked: &RuntimeCheckedType,
) -> Result<(), RuntimeCheckedTypeProjectionError> {
    if cases.len() != owner.cases().len()
        || cases.iter().enumerate().any(|(ordinal, case)| {
            u32::try_from(ordinal)
                .ok()
                .and_then(|ordinal| checked.variant_case(ordinal))
                .is_none_or(|expected| {
                    expected.name != case.name
                        || expected.payload.as_deref()
                            != case
                                .payload()
                                .and_then(|payload| payload.checked_type().ok())
                                .as_ref()
                })
        })
    {
        return Err(RuntimeCheckedTypeProjectionError::InvalidBuiltinVariant {
            semantic_identity,
            path: RuntimeTypeProjectionPath::root(),
            owner,
        });
    }
    Ok(())
}

/// Complete checked variant owner and its canonical selected case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedVariantSelection {
    owner: RuntimeCheckedType,
    ordinal: u32,
    case: RuntimeCheckedVariantCase,
}

impl RuntimeCheckedVariantSelection {
    #[must_use]
    pub const fn owner(&self) -> &RuntimeCheckedType {
        &self.owner
    }

    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    #[must_use]
    pub const fn case(&self) -> &RuntimeCheckedVariantCase {
        &self.case
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.case.name
    }

    #[must_use]
    pub fn payload(&self) -> Option<&RuntimeCheckedType> {
        match &self.case.payload {
            Some(payload) => Some(payload.as_ref()),
            None => None,
        }
    }
}

/// Failure to reconcile one semantically selected case with its complete owner.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedVariantError {
    #[error("variant owner checked-type projection failed")]
    CheckedTypeProjection(#[from] RuntimeCheckedTypeProjectionError),
    #[error("variant case ordinal {ordinal} is outside {case_count} cases")]
    CaseOrdinal { ordinal: u32, case_count: u32 },
    #[error("variant owner has {count} cases, exceeding the u32 runtime ordinal domain")]
    CaseCountOverflow { count: usize },
    #[error("variant case {ordinal} resolved as `{actual}`, expected `{expected}`")]
    CaseName {
        ordinal: u32,
        expected: String,
        actual: String,
    },
}

/// Checked enum case selected for a variant expression, constructor call, or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedVariant {
    owner: RuntimeVariantOwner,
    ordinal: u32,
}

impl RuntimeResolvedVariant {
    fn try_new(
        owner: RuntimeVariantOwner,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let selected = owner.selected_case(ordinal)?;
        if selected.name != selected_name {
            return Err(RuntimeResolvedVariantError::CaseName {
                ordinal,
                expected: selected.name.to_owned(),
                actual: selected_name.to_owned(),
            });
        }
        Ok(Self { owner, ordinal })
    }

    /// Retains a case directly from its accepted project enum declaration.
    pub fn project(
        owner: RuntimeResolvedNominal,
        arguments: Box<[RuntimeNormalizedType]>,
        ordinal: u32,
        selected_name: &str,
        cases: Box<[RuntimeNormalizedVariantCase]>,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let cases = RuntimeNormalizedVariantCases::try_new(cases)?;
        Self::try_new(
            RuntimeVariantOwner::Project {
                nominal: owner,
                arguments,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains a Character nominal case already admitted by checked final HIR.
    pub fn character(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let cases = RuntimeNormalizedVariantCases::try_new(cases)?;
        Self::try_new(
            RuntimeVariantOwner::CharacterNominal {
                identity,
                nominal,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains a case from one source-ordered base-environment enum schema.
    pub fn builtin_closed(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let cases = RuntimeNormalizedVariantCases::try_new(cases)?;
        Self::try_new(
            RuntimeVariantOwner::BuiltinClosed {
                identity,
                nominal,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    pub fn runtime_builtin(
        identity: RuntimeSemanticTypeId,
        owner: RuntimeBuiltinVariantIdentity,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let cases = RuntimeNormalizedVariantCases::try_new(cases)?;
        Self::try_new(
            RuntimeVariantOwner::RuntimeBuiltin {
                identity,
                owner,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains one accepted Option case after reconciling its closed name.
    pub fn option(
        identity: RuntimeSemanticTypeId,
        item: RuntimeNormalizedType,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let cases = RuntimeNormalizedVariantCases::try_new(cases)?;
        let owner = RuntimeVariantOwner::Option {
            identity,
            item,
            cases,
        };
        owner.project_checked_type()?;
        Self::try_new(owner, ordinal, selected_name)
    }

    /// Retains one accepted Result case after reconciling its closed name.
    pub fn result(
        identity: RuntimeSemanticTypeId,
        ok: RuntimeNormalizedType,
        error: RuntimeNormalizedType,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let owner = RuntimeVariantOwner::Result {
            identity,
            ok,
            error,
            cases: RuntimeNormalizedVariantCases::try_new(cases)?,
        };
        owner.project_checked_type()?;
        Self::try_new(owner, ordinal, selected_name)
    }

    pub const fn owner(&self) -> &RuntimeVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the selected name borrowed from the complete owner table.
    pub fn selected_name(&self) -> Result<&str, RuntimeResolvedVariantError> {
        self.owner
            .selected_case(self.ordinal)
            .map(|selected| selected.name)
    }

    /// Returns the selected normalized payload borrowed from its sole owner.
    pub fn selected_payload_type(
        &self,
    ) -> Result<Option<&RuntimeNormalizedType>, RuntimeResolvedVariantError> {
        self.owner
            .selected_case(self.ordinal)
            .map(|selected| selected.payload)
    }

    /// Reconciles the selected semantic case with the complete checked owner.
    pub fn checked_selection(
        &self,
    ) -> Result<RuntimeCheckedVariantSelection, RuntimeResolvedVariantError> {
        self.owner.selected_case(self.ordinal)?;
        let owner = self.owner.project_checked_type()?;
        let case =
            owner
                .variant_case(self.ordinal)
                .ok_or(RuntimeResolvedVariantError::CaseOrdinal {
                    ordinal: self.ordinal,
                    case_count: self.owner.case_count(),
                })?;
        Ok(RuntimeCheckedVariantSelection {
            owner,
            ordinal: self.ordinal,
            case,
        })
    }
}

/// Typed failure while consuming one ABI-positioned runtime call operand list.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedCallError {
    #[error("runtime call ABI position {position} is outside operand count {operand_count}")]
    AbiPositionOutOfRange { position: u32, operand_count: u32 },
    #[error("runtime call repeats ABI position {position}")]
    DuplicateAbiPosition { position: u32 },
    #[error("runtime call omits ABI position {position}")]
    MissingAbiPosition { position: u32 },
    #[error("runtime call contains duplicate operand origin")]
    DuplicateOperandOrigin,
    #[error("runtime call physical operands are not in canonical source order")]
    NonCanonicalSourceOrder,
    #[error("runtime call contains more than one receiver operand")]
    MultipleReceivers,
    #[error("runtime attached-content ABI position {actual} is not the final position {expected}")]
    NonTerminalAttachedContentPosition { expected: u32, actual: u32 },
    #[error("runtime call attached-content operand does not match the selected callable interface")]
    AttachedContentInterfaceMismatch,
    #[error("ordinary project-function call dispatch disagrees with its checked call plan")]
    ProjectFunctionPlanMismatch,
    #[error("ordinary project-function call result disagrees with its checked call outcome")]
    ProjectFunctionResultMismatch,
    #[error(
        "ordinary project-function call operands do not match its logical materialization plan"
    )]
    ProjectFunctionMaterializationMismatch,
}

/// Runtime attached-content value retained separately from ordinary authored
/// arguments while participating in the same terminal call ABI transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedAttachedContent {
    Required {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    OptionalPresent {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    OptionalOmitted {
        ty: RuntimeNormalizedType,
    },
    DefaultedPresent {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    DefaultedOmitted {
        ty: RuntimeNormalizedType,
    },
}

impl RuntimeResolvedAttachedContent {
    pub const fn source(&self) -> Option<ExprId> {
        match self {
            Self::Required { source, .. }
            | Self::OptionalPresent { source, .. }
            | Self::DefaultedPresent { source, .. } => Some(*source),
            Self::OptionalOmitted { .. } | Self::DefaultedOmitted { .. } => None,
        }
    }

    /// Returns the exact final checked ABI type. Omitted attached content
    /// retains this row so `None` can be lowered without reopening the callee
    /// schema or reconstructing a standard nominal from source spelling.
    pub const fn ty(&self) -> &RuntimeNormalizedType {
        match self {
            Self::Required { ty, .. }
            | Self::OptionalPresent { ty, .. }
            | Self::OptionalOmitted { ty }
            | Self::DefaultedPresent { ty, .. }
            | Self::DefaultedOmitted { ty } => ty,
        }
    }
}

/// Final ABI-positioned row for the one terminal attached-content operand.
/// The call-site ABI position includes physical rest expansion; the distinct
/// declaration-owned logical position remains on
/// `RuntimeCallableAttachedContentAbi`. Its group is owned once by
/// [`RuntimeResolvedCall::completed_group`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePositionedAttachedContent {
    abi_position: u32,
    content: RuntimeResolvedAttachedContent,
}

impl RuntimePositionedAttachedContent {
    pub const fn new(abi_position: u32, content: RuntimeResolvedAttachedContent) -> Self {
        Self {
            abi_position,
            content,
        }
    }

    pub const fn abi_position(&self) -> u32 {
        self.abi_position
    }

    pub const fn content(&self) -> &RuntimeResolvedAttachedContent {
        &self.content
    }

    pub fn into_content(self) -> RuntimeResolvedAttachedContent {
        self.content
    }
}

/// One compiler-selected call dispatch for an exact final-HIR call expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedCall {
    dispatch: RuntimeResolvedCallDispatch,
    completed_group: arcweft_lang_sema::callable::CallableGroupIndex,
    /// Sole source-ordered physical operand row. Every member retains its
    /// checked ABI destination; consumers evaluate this row once in order and
    /// use the positions only to install the resulting values.
    operands: Box<[RuntimeResolvedCallOperand]>,
    attached_content: Option<RuntimePositionedAttachedContent>,
    project_function: Option<RuntimeProjectFunctionCallPlan>,
    result: RuntimeCallResultShape,
}

impl RuntimeResolvedCall {
    /// Validates the ABI-position permutation without changing source order.
    pub fn try_new(
        dispatch: RuntimeResolvedCallDispatch,
        completed_group: arcweft_lang_sema::callable::CallableGroupIndex,
        operands: Vec<RuntimeResolvedCallOperand>,
        positioned_attached_content: Option<RuntimePositionedAttachedContent>,
        project_function: Option<RuntimeProjectFunctionCallPlan>,
        result: RuntimeCallResultShape,
    ) -> Result<Self, RuntimeResolvedCallError> {
        let operand_count = u32::try_from(operands.len()).map_err(|_| {
            RuntimeResolvedCallError::AbiPositionOutOfRange {
                position: u32::MAX,
                operand_count: u32::MAX,
            }
        })?;
        let mut abi_positions = vec![false; operands.len()];
        if operands
            .windows(2)
            .any(|pair| pair[0].origin() >= pair[1].origin())
        {
            return Err(RuntimeResolvedCallError::NonCanonicalSourceOrder);
        }
        let mut origins = BTreeSet::new();
        let mut receivers = 0_u8;
        for operand in &operands {
            let position = operand.abi_position();
            let index = usize::try_from(position).map_err(|_| {
                RuntimeResolvedCallError::AbiPositionOutOfRange {
                    position,
                    operand_count,
                }
            })?;
            let Some(seen) = abi_positions.get_mut(index) else {
                return Err(RuntimeResolvedCallError::AbiPositionOutOfRange {
                    position,
                    operand_count,
                });
            };
            if std::mem::replace(seen, true) {
                return Err(RuntimeResolvedCallError::DuplicateAbiPosition { position });
            }
            if !origins.insert(operand.origin().clone()) {
                return Err(RuntimeResolvedCallError::DuplicateOperandOrigin);
            }
            if matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver) {
                receivers = receivers.saturating_add(1);
            }
        }
        if receivers > 1 {
            return Err(RuntimeResolvedCallError::MultipleReceivers);
        }
        if let Some(position) = abi_positions.iter().position(|seen| !seen) {
            return Err(RuntimeResolvedCallError::MissingAbiPosition {
                position: u32::try_from(position).map_err(|_| {
                    RuntimeResolvedCallError::MissingAbiPosition { position: u32::MAX }
                })?,
            });
        }
        if let Some(positioned) = &positioned_attached_content {
            let expected = u32::try_from(operands.len()).map_err(|_| {
                RuntimeResolvedCallError::NonTerminalAttachedContentPosition {
                    expected: u32::MAX,
                    actual: positioned.abi_position(),
                }
            })?;
            if positioned.abi_position() != expected {
                return Err(
                    RuntimeResolvedCallError::NonTerminalAttachedContentPosition {
                        expected,
                        actual: positioned.abi_position(),
                    },
                );
            }
        }
        let direct_project_callable = dispatch.project_callable();
        let project_function_matches = match (&dispatch, &project_function) {
            (
                RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
                    direct,
                )),
                Some(plan),
            ) => {
                direct == plan.callable()
                    && matches!(plan.input(), RuntimeProjectFunctionCallInput::Direct)
            }
            (RuntimeResolvedCallDispatch::Value { callee }, Some(plan)) => matches!(
                plan.input(),
                RuntimeProjectFunctionCallInput::Continuation {
                    callee: expected,
                    ..
                } if expected == callee
            ),
            (
                RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
                    _,
                )),
                None,
            ) => true,
            (_, None) => true,
            (_, Some(_)) => false,
        };
        if !project_function_matches {
            return Err(RuntimeResolvedCallError::ProjectFunctionPlanMismatch);
        }
        if let Some(plan) = &project_function {
            let mut covered = vec![false; operands.len()];
            for materialization in plan.current_group_materialization() {
                let indices = materialization.operand_indices();
                let expected_group =
                    u32::try_from(materialization.group().get()).map_err(|_| {
                        RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch
                    })?;
                let expected = RuntimeCallParameterCoordinate::new(
                    expected_group,
                    materialization.parameter(),
                );
                for index in indices {
                    let index = usize::try_from(*index).map_err(|_| {
                        RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch
                    })?;
                    let Some(operand) = operands.get(index) else {
                        return Err(
                            RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch,
                        );
                    };
                    if covered[index] {
                        return Err(
                            RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch,
                        );
                    }
                    covered[index] = true;
                    let origin_matches = match materialization.kind() {
                        arcweft_lang_hir::item::HirParameterKind::ExtensionReceiver => {
                            matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver)
                        }
                        arcweft_lang_hir::item::HirParameterKind::Fixed
                        | arcweft_lang_hir::item::HirParameterKind::RestPositional => {
                            matches!(
                                operand.origin(),
                                RuntimeResolvedCallOperandOrigin::Argument { .. }
                            ) && operand.parameter() == Some(expected)
                        }
                    };
                    if !origin_matches {
                        return Err(
                            RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch,
                        );
                    }
                }
            }
            if covered.iter().any(|covered| !covered) {
                return Err(RuntimeResolvedCallError::ProjectFunctionMaterializationMismatch);
            }
        }
        if project_function.as_ref().is_some_and(|plan| {
            !matches!(
                (plan.outcome(), result),
                (
                    RuntimeProjectFunctionCallOutcome::Continue { .. },
                    RuntimeCallResultShape::PartialFunction
                ) | (
                    RuntimeProjectFunctionCallOutcome::Invoke { .. },
                    RuntimeCallResultShape::Value
                )
            )
        }) {
            return Err(RuntimeResolvedCallError::ProjectFunctionResultMismatch);
        }
        let project_attached = project_function
            .as_ref()
            .map(RuntimeProjectFunctionCallPlan::callable)
            .or(direct_project_callable)
            .and_then(RuntimeProjectCallable::attached_content_abi);
        if project_function.is_some() || direct_project_callable.is_some() {
            let interface_matches = match project_attached {
                Some(interface) if completed_group.get() < interface.group().get() => {
                    positioned_attached_content.is_none()
                }
                Some(interface) if completed_group == interface.group() => {
                    positioned_attached_content
                        .as_ref()
                        .is_some_and(|positioned| {
                            runtime_attached_content_matches_interface(
                                positioned.content(),
                                interface,
                            )
                        })
                }
                Some(_) => false,
                None => positioned_attached_content.is_none(),
            };
            if !interface_matches {
                return Err(RuntimeResolvedCallError::AttachedContentInterfaceMismatch);
            }
        }
        Ok(Self {
            dispatch,
            completed_group,
            operands: operands.into_boxed_slice(),
            attached_content: positioned_attached_content,
            project_function,
            result,
        })
    }

    pub const fn dispatch(&self) -> &RuntimeResolvedCallDispatch {
        &self.dispatch
    }

    pub const fn operands(&self) -> &[RuntimeResolvedCallOperand] {
        &self.operands
    }

    /// Whether an authored callee expression is evaluated as a runtime value.
    /// Static name/type selectors are not evaluated; a value callee or a
    /// receiver retained in the checked source operand row is evaluated.
    pub(crate) fn evaluates_callee(&self, expression: ExprId) -> bool {
        matches!(self.dispatch(), RuntimeResolvedCallDispatch::Value { callee } if *callee == expression)
            || self.operands().iter().any(|operand| {
                operand.source() == RuntimeResolvedCallOperandSource::Expression(expression)
            })
    }

    /// Derived ABI view. The stored row remains source-ordered; this iterator
    /// performs no evaluation and allocates no parallel operand inventory.
    pub fn abi_operands(&self) -> impl Iterator<Item = &RuntimeResolvedCallOperand> {
        (0..self.operands.len()).filter_map(|position| {
            let position = u32::try_from(position).ok()?;
            self.operands
                .iter()
                .find(|operand| operand.abi_position() == position)
        })
    }

    pub const fn completed_group(&self) -> arcweft_lang_sema::callable::CallableGroupIndex {
        self.completed_group
    }

    pub const fn attached_content(&self) -> Option<&RuntimeResolvedAttachedContent> {
        match &self.attached_content {
            Some(attached) => Some(attached.content()),
            None => None,
        }
    }

    pub const fn positioned_attached_content(&self) -> Option<&RuntimePositionedAttachedContent> {
        self.attached_content.as_ref()
    }

    pub const fn project_function(&self) -> Option<&RuntimeProjectFunctionCallPlan> {
        self.project_function.as_ref()
    }

    pub const fn result(&self) -> RuntimeCallResultShape {
        self.result
    }

    /// Whether this target is represented by a structural expression payload
    /// rather than the common positioned Call/Apply carrier. Such payloads
    /// must consume the shared source-order ANF materialization before they
    /// may interpret operands by semantic/ABI role.
    pub const fn requires_specialized_operand_anf(&self) -> bool {
        matches!(
            self.dispatch,
            RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::Agent(_)
                    | RuntimeResolvedStaticCallTarget::AgentProbeComparison(_)
                    | RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError
                    | RuntimeResolvedStaticCallTarget::Variant(_)
                    | RuntimeResolvedStaticCallTarget::Reduction(_)
            )
        )
    }
}

fn runtime_attached_content_matches_interface(
    content: &RuntimeResolvedAttachedContent,
    interface: &RuntimeCallableAttachedContentAbi,
) -> bool {
    use arcweft_lang_sema::callable::CallableParameterPresence;

    content.ty() == interface.abi_ty()
        && matches!(
            (interface.presence(), content),
            (
                CallableParameterPresence::Required,
                RuntimeResolvedAttachedContent::Required { .. }
            ) | (
                CallableParameterPresence::Optional,
                RuntimeResolvedAttachedContent::OptionalPresent { .. }
                    | RuntimeResolvedAttachedContent::OptionalOmitted { .. }
            ) | (
                CallableParameterPresence::Defaulted,
                RuntimeResolvedAttachedContent::DefaultedPresent { .. }
                    | RuntimeResolvedAttachedContent::DefaultedOmitted { .. }
            )
        )
}

/// Closed dispatch authority. Static targets are exhaustive and never encode
/// value-callee classification; value dispatch retains its exact HIR callee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedCallDispatch {
    Static(RuntimeResolvedStaticCallTarget),
    Value { callee: ExprId },
}

impl RuntimeResolvedCallDispatch {
    pub const fn project_callable(&self) -> Option<&RuntimeProjectCallable> {
        match self {
            Self::Static(RuntimeResolvedStaticCallTarget::Declaration(callable)) => Some(callable),
            Self::Static(RuntimeResolvedStaticCallTarget::Host(host)) => match host.owner() {
                RuntimeResolvedHostCallOwner::ExternCapability(callable) => Some(callable),
                RuntimeResolvedHostCallOwner::Agent(_) => None,
            },
            Self::Static(
                RuntimeResolvedStaticCallTarget::Intrinsic(_)
                | RuntimeResolvedStaticCallTarget::Agent(_)
                | RuntimeResolvedStaticCallTarget::AgentProbeComparison(_)
                | RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError
                | RuntimeResolvedStaticCallTarget::Variant(_)
                | RuntimeResolvedStaticCallTarget::Reduction(_)
                | RuntimeResolvedStaticCallTarget::StandardMap(_)
                | RuntimeResolvedStaticCallTarget::TraitMethod { .. }
                | RuntimeResolvedStaticCallTarget::Registered(_)
                | RuntimeResolvedStaticCallTarget::Line(_),
            )
            | Self::Value { .. } => None,
        }
    }
}

/// Closed runtime dispatch selected by the shared semantic resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeLineCallable {
    AcquireActor {
        character: arcweft_character::id::CharacterId,
    },
    ActorLook {
        character: arcweft_character::id::CharacterId,
        actor: ExprId,
        look: ExprId,
        crossfade: ExprId,
    },
    VoiceHandle,
    Schedule {
        anchor: ExprId,
        callback: ExprId,
    },
}

/// Closed runtime dispatch selected by the shared semantic resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the closed selected-target vocabulary deliberately retains exact typed owners without target-specific indirection"
)]
pub enum RuntimeResolvedStaticCallTarget {
    Intrinsic(RuntimeIntrinsic),
    Agent(crate::agent::RuntimeAgentIntrinsic),
    AgentProbeComparison(arcweft_core::value::RuntimeAgentCompareOp),
    AgentDiagnosticsHasError,
    Declaration(RuntimeProjectCallable),
    /// Typed enum case selected by the shared callable resolver.
    ///
    /// Constructor calls are values in the runtime expression algebra, not
    /// registered callables or string-selected intrinsics.
    Variant(RuntimeResolvedVariant),
    /// Core-owned `Reduction` value construction selected by semantic identity.
    Reduction(RuntimeReductionConstructor),
    /// One checked standard `map` overload with exact callback and receiver
    /// expression coordinates. The ordinary callable identity is retained by
    /// semantic analysis; this projection selects its closed executable
    /// constructor family without falling through to a registered backend.
    StandardMap(RuntimeStandardMapCall),
    TraitMethod {
        method: ImplMethodDeclarationId,
        receiver: RuntimeReceiverMode,
    },
    /// A checked line capability retained as typed operation identity. It has
    /// no ordinary runtime-call fallback and is consumed only by line-plan
    /// lowering.
    Line(RuntimeLineCallable),
    Registered(RuntimeCallableId),
    Host(RuntimeResolvedHostCall),
}

/// Closed, instantiated constructor family selected for one standard `map`
/// call. Generic arguments are read from the call's normalized operand/result
/// types; the family variant prevents a spelling or callable-digest lookup at
/// lowering time.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeStandardMapFamily {
    Vec,
    Seq,
    Array,
    Slice,
    Option,
    Result,
}

/// Exact final-HIR operands of one fully applied standard `map` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStandardMapCall {
    family: RuntimeStandardMapFamily,
    mapping: ExprId,
    receiver: ExprId,
    order: RuntimeStandardMapOperandOrder,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeStandardMapOperandOrder {
    MappingThenReceiver,
    ReceiverThenMapping,
}

impl RuntimeStandardMapCall {
    pub const fn new(
        family: RuntimeStandardMapFamily,
        mapping: ExprId,
        receiver: ExprId,
        order: RuntimeStandardMapOperandOrder,
    ) -> Self {
        Self {
            family,
            mapping,
            receiver,
            order,
        }
    }

    pub const fn family(&self) -> RuntimeStandardMapFamily {
        self.family
    }

    pub const fn mapping(&self) -> ExprId {
        self.mapping
    }

    pub const fn receiver(&self) -> ExprId {
        self.receiver
    }

    pub const fn order(&self) -> RuntimeStandardMapOperandOrder {
        self.order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedHostCall {
    owner: RuntimeResolvedHostCallOwner,
    public_id: String,
    capability: String,
    operation: String,
    contract: Option<arcweft_core::step::HostCallContractDigest>,
    mode: RuntimeHostCallMode,
    deterministic: bool,
}

impl RuntimeResolvedHostCall {
    pub fn extern_capability(
        callable: RuntimeProjectCallable,
        contract: arcweft_core::step::HostCallContractDigest,
        mode: RuntimeHostCallMode,
    ) -> Result<Self, RuntimeResolvedHostCallError> {
        let CallableDeclarationKey::Existing(declaration) = callable.declaration() else {
            return Err(RuntimeResolvedHostCallError::MissingDeclarationIdentity);
        };
        if declaration.owner() != CallableDeclarationOwner::ExternCapability {
            return Err(RuntimeResolvedHostCallError::NotExternCapability);
        }
        let capability = declaration
            .owner_path()
            .iter()
            .map(arcweft_lang_syntax::ast::module_path::ModuleSegment::as_str)
            .collect::<Vec<_>>()
            .join(".");
        if capability.is_empty() {
            return Err(RuntimeResolvedHostCallError::EmptyCapabilityPath);
        }
        let operation = declaration.name().to_owned();
        Ok(Self {
            owner: RuntimeResolvedHostCallOwner::ExternCapability(callable),
            public_id: format!("{capability}.{operation}"),
            capability,
            operation,
            contract: Some(contract),
            mode,
            deterministic: false,
        })
    }

    pub fn agent(intrinsic: crate::agent::RuntimeAgentIntrinsic) -> Option<Self> {
        let operation = intrinsic.host_operation()?;
        Some(Self {
            owner: RuntimeResolvedHostCallOwner::Agent(intrinsic),
            public_id: format!("agent.{operation}"),
            capability: "agent".to_owned(),
            operation: operation.to_owned(),
            contract: None,
            mode: RuntimeHostCallMode::Suspend,
            deterministic: false,
        })
    }

    pub const fn owner(&self) -> &RuntimeResolvedHostCallOwner {
        &self.owner
    }

    pub fn public_id(&self) -> &str {
        &self.public_id
    }

    pub fn capability(&self) -> &str {
        &self.capability
    }

    pub fn operation(&self) -> &str {
        &self.operation
    }

    pub const fn contract(&self) -> Option<arcweft_core::step::HostCallContractDigest> {
        self.contract
    }

    pub const fn mode(&self) -> RuntimeHostCallMode {
        self.mode
    }

    pub const fn deterministic(&self) -> bool {
        self.deterministic
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeResolvedHostCallError {
    #[error("extern capability host call has no declaration identity")]
    MissingDeclarationIdentity,
    #[error("host-call declaration is not owned by an extern capability")]
    NotExternCapability,
    #[error("extern capability host call has an empty capability path")]
    EmptyCapabilityPath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedHostCallOwner {
    ExternCapability(RuntimeProjectCallable),
    Agent(crate::agent::RuntimeAgentIntrinsic),
}

/// Closed core `Reduction` constructor vocabulary below semantic analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeReductionConstructor {
    Unchanged,
}

/// ABI operand origin retained by the final runtime carrier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeResolvedCallOperandOrigin {
    Receiver,
    Argument { argument: u32, slot: u32 },
}

/// Exact checked callable-schema destination of one scalar runtime operand.
/// This coordinate is retained independently from authored argument order so
/// named and positional spellings project to the same runtime authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCallParameterCoordinate {
    group: u32,
    parameter: u32,
}

impl RuntimeCallParameterCoordinate {
    pub const fn new(group: u32, parameter: u32) -> Self {
        Self { group, parameter }
    }

    pub const fn group(self) -> u32 {
        self.group
    }

    pub const fn parameter(self) -> u32 {
        self.parameter
    }
}

/// Exact final-HIR source for one runtime operand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeResolvedCallOperandSource {
    Expression(ExprId),
    CompactNumericElement { sequence: ExprId, ordinal: u32 },
}

/// Authored/runtime argument binding disposition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeResolvedCallOperandBinding {
    Positional,
    Named(String),
}

/// Accepted scalar or spread-container runtime projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedCallOperandProjection {
    Scalar,
    SpreadContainer(RuntimeResolvedSpreadContainer),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeMapKind {
    Ordered,
    Sorted,
    BTree,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedSpreadContainer {
    Vec,
    Seq,
    Slice,
    Array {
        len: usize,
    },
    MapValue {
        kind: RuntimeMapKind,
        key: RuntimeNormalizedType,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedCallOperand {
    abi_position: u32,
    origin: RuntimeResolvedCallOperandOrigin,
    source: RuntimeResolvedCallOperandSource,
    ty: RuntimeNormalizedType,
    binding: RuntimeResolvedCallOperandBinding,
    projection: RuntimeResolvedCallOperandProjection,
    parameter: Option<RuntimeCallParameterCoordinate>,
}

impl RuntimeResolvedCallOperand {
    pub fn new(
        abi_position: u32,
        origin: RuntimeResolvedCallOperandOrigin,
        source: RuntimeResolvedCallOperandSource,
        ty: RuntimeNormalizedType,
        binding: RuntimeResolvedCallOperandBinding,
        projection: RuntimeResolvedCallOperandProjection,
        parameter: Option<RuntimeCallParameterCoordinate>,
    ) -> Self {
        Self {
            abi_position,
            origin,
            source,
            ty,
            binding,
            projection,
            parameter,
        }
    }

    pub const fn abi_position(&self) -> u32 {
        self.abi_position
    }

    pub const fn origin(&self) -> &RuntimeResolvedCallOperandOrigin {
        &self.origin
    }
    pub const fn source(&self) -> RuntimeResolvedCallOperandSource {
        self.source
    }
    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
    pub const fn binding(&self) -> &RuntimeResolvedCallOperandBinding {
        &self.binding
    }
    pub const fn projection(&self) -> &RuntimeResolvedCallOperandProjection {
        &self.projection
    }

    pub const fn parameter(&self) -> Option<RuntimeCallParameterCoordinate> {
        self.parameter
    }
}

/// Whether a checked call produces its declared value or a partial function.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCallResultShape {
    Value,
    PartialFunction,
}

/// Checked assertion disposition for an exact final-HIR assertion statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAssertionAdmission {
    Discharged,
    Runtime(RuntimeAssertionMode),
    OmittedDebug,
}

/// Opaque, read-only compiler admission for one reachable `On` statement.
///
/// The compiler-to-runtime-plan staging boundary is the only constructor.
/// Callers cannot manufacture or change the admitted trigger meaning after
/// the single semantic-fact transaction has been sealed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTriggerAdmission {
    kind: RuntimeTriggerAdmissionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeTriggerAdmissionKind {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(RuntimeDialogueMarkFact),
    Select,
    Task,
    Scope,
    Expression,
}

impl RuntimeTriggerAdmission {
    const fn new(kind: RuntimeTriggerAdmissionKind) -> Self {
        Self { kind }
    }

    pub const fn input() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Input)
    }

    pub const fn event() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Event)
    }

    pub const fn signal() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Signal)
    }

    pub const fn timeout() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Timeout)
    }

    pub const fn mark(mark: RuntimeDialogueMarkFact) -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Mark(mark))
    }

    pub const fn select() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Select)
    }

    pub const fn task() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Task)
    }

    pub const fn scope() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Scope)
    }

    pub const fn expression() -> Self {
        Self::new(RuntimeTriggerAdmissionKind::Expression)
    }

    pub const fn dialogue_mark(&self) -> Option<&RuntimeDialogueMarkFact> {
        match &self.kind {
            RuntimeTriggerAdmissionKind::Mark(mark) => Some(mark),
            RuntimeTriggerAdmissionKind::Input
            | RuntimeTriggerAdmissionKind::Event
            | RuntimeTriggerAdmissionKind::Signal
            | RuntimeTriggerAdmissionKind::Timeout
            | RuntimeTriggerAdmissionKind::Select
            | RuntimeTriggerAdmissionKind::Task
            | RuntimeTriggerAdmissionKind::Scope
            | RuntimeTriggerAdmissionKind::Expression => None,
        }
    }
}

/// Checked capture metadata that is not derivable from lexical HIR alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedCapture {
    projection: arcweft_lang_hir::project::HirSelectedCapture,
    ty: RuntimeNormalizedType,
}

/// One executable dialogue application projected from checked semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueApplication {
    content: DialogueContentSpec,
    line_result: RuntimeNormalizedType,
}

impl RuntimeDialogueApplication {
    pub fn new(content: DialogueContentSpec, line_result: RuntimeNormalizedType) -> Self {
        Self {
            content,
            line_result,
        }
    }

    pub const fn content(&self) -> &DialogueContentSpec {
        &self.content
    }

    pub const fn line_result(&self) -> &RuntimeNormalizedType {
        &self.line_result
    }
}

/// Typed trigger for one effectful inline dialogue call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeDialogueEffectTrigger {
    Content,
    Delay {
        duration: arcweft_core::time::LogicalDuration,
        duration_type: RuntimeNormalizedType,
        schedule_handle_type: RuntimeNormalizedType,
    },
}

/// Accepted authored expression supplying one document-local dialogue slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDialogueValueExpression {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    expression: ExprId,
    ty: RuntimeNormalizedType,
}

impl RuntimeDialogueValueExpression {
    pub const fn new(
        slot: RuntimeDialogueValueSlotId,
        role: RuntimeDialogueValueRole,
        expression: ExprId,
        ty: RuntimeNormalizedType,
    ) -> Self {
        Self {
            slot,
            role,
            expression,
            ty,
        }
    }

    pub const fn slot(&self) -> RuntimeDialogueValueSlotId {
        self.slot
    }

    pub const fn role(&self) -> RuntimeDialogueValueRole {
        self.role
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// Trait authority selected by final semantic analysis for one executable
/// implementation method.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTraitIdentity {
    Project(ItemId),
    StandardIterator,
    StandardIntoIterator,
}

/// Generation-bound method identity consumed by final-HIR runtime lowering.
///
/// The runtime method ID is assigned deterministically by the compiler from
/// the ordered set of checked conformances. The implementation/member pair is
/// the sole body owner; no detached method catalog or source lookup is
/// retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTraitMethodFact {
    declaration: ImplMethodDeclarationId,
    implementation: ItemId,
    member: u16,
    trait_identity: RuntimeTraitIdentity,
    self_type: RuntimeNormalizedType,
}

impl RuntimeTraitMethodFact {
    pub fn new(
        declaration: ImplMethodDeclarationId,
        implementation: ItemId,
        member: u16,
        trait_identity: RuntimeTraitIdentity,
        self_type: RuntimeNormalizedType,
    ) -> Self {
        Self {
            declaration,
            implementation,
            member,
            trait_identity,
            self_type,
        }
    }

    pub const fn declaration(&self) -> &ImplMethodDeclarationId {
        &self.declaration
    }

    pub const fn implementation(&self) -> ItemId {
        self.implementation
    }

    pub const fn member(&self) -> u16 {
        self.member
    }

    pub const fn trait_identity(&self) -> &RuntimeTraitIdentity {
        &self.trait_identity
    }

    pub const fn self_type(&self) -> &RuntimeNormalizedType {
        &self.self_type
    }
}

impl RuntimeCheckedCapture {
    pub const fn new(
        projection: arcweft_lang_hir::project::HirSelectedCapture,
        ty: RuntimeNormalizedType,
    ) -> Self {
        Self { projection, ty }
    }

    pub const fn capture(&self) -> CaptureId {
        self.projection.capture()
    }

    pub const fn projection(&self) -> &arcweft_lang_hir::project::HirSelectedCapture {
        &self.projection
    }

    pub const fn source(&self) -> LocalId {
        self.projection.local()
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// One closure capture bound to a declaration-ordered domain parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePureProgramCaptureFact {
    capture: CaptureId,
    local: LocalId,
    parameter: u16,
    value_type: RuntimeSemanticTypeId,
}

impl RuntimePureProgramCaptureFact {
    #[must_use]
    pub const fn new(
        capture: CaptureId,
        local: LocalId,
        parameter: u16,
        value_type: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            capture,
            local,
            parameter,
            value_type,
        }
    }

    pub const fn capture(self) -> CaptureId {
        self.capture
    }

    pub const fn local(self) -> LocalId {
        self.local
    }

    pub const fn parameter(self) -> u16 {
        self.parameter
    }

    pub const fn value_type(self) -> RuntimeSemanticTypeId {
        self.value_type
    }
}

/// Exact mount-only deterministic program rooted at one checked closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePureProgramFact {
    program: RuntimePureProgramId,
    closure: ExprId,
    body: ExprId,
    captures: Box<[RuntimePureProgramCaptureFact]>,
    result: RuntimeSemanticTypeId,
}

impl RuntimePureProgramFact {
    #[must_use]
    pub const fn new(
        program: RuntimePureProgramId,
        closure: ExprId,
        body: ExprId,
        captures: Box<[RuntimePureProgramCaptureFact]>,
        result: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            program,
            closure,
            body,
            captures,
            result,
        }
    }

    pub const fn program(&self) -> RuntimePureProgramId {
        self.program
    }

    pub const fn closure(&self) -> ExprId {
        self.closure
    }

    pub const fn body(&self) -> ExprId {
        self.body
    }

    pub const fn captures(&self) -> &[RuntimePureProgramCaptureFact] {
        &self.captures
    }

    pub const fn result(&self) -> RuntimeSemanticTypeId {
        self.result
    }
}

/// Mutable staging owner used by semantic analysis before generation binding.
///
/// Staged facts are not executable. [`RuntimePlanSemanticFacts::try_new`]
/// validates every owner and nested project identity before publication.
#[derive(Debug)]
pub struct RuntimePlanSemanticFactInput {
    local_declarations: Vec<(LocalId, RuntimeNormalizedType)>,
    flows: Vec<(ItemId, RuntimeFlowFact)>,
    expression_types: Vec<(ExprId, RuntimeNormalizedType)>,
    pattern_types: Vec<(PatternId, RuntimeNormalizedType)>,
    expression_literals: Vec<(ExprId, RuntimeValue)>,
    pattern_literals: Vec<(PatternId, RuntimeValue)>,
    pattern_items: Vec<(PatternId, RuntimeProjectItem)>,
    values: Vec<(ExprId, RuntimeResolvedValue)>,
    selects: Vec<(ExprId, RuntimeResolvedSelect)>,
    nominal_records: Vec<(ExprId, RuntimeRecordExpressionFact)>,
    pattern_nominal_records: Vec<(PatternId, RuntimeRecordPatternFact)>,
    expression_variants: Vec<(ExprId, RuntimeResolvedVariant)>,
    pattern_variants: Vec<(PatternId, RuntimeResolvedVariant)>,
    types: Vec<(TypeId, RuntimeNormalizedType)>,
    calls: Vec<(ExprId, RuntimeResolvedCall)>,
    postfix_candidates: Vec<(ExprId, ExprId)>,
    trait_methods: Vec<RuntimeTraitMethodFact>,
    iterations: Vec<(StmtId, RuntimeIteratorFact)>,
    assertions: Vec<(StmtId, RuntimeAssertionAdmission)>,
    triggers: BTreeMap<StmtId, RuntimeTriggerAdmission>,
    assignments: Vec<(StmtId, RuntimeAssignmentFact)>,
    evaluated_effects: Vec<(StmtId, RuntimeEvaluatedEffectFact)>,
    choices: Vec<(ExprId, RuntimeChoiceFact)>,
    awaits: Vec<(ExprId, RuntimeAwaitFact)>,
    tries: Vec<(ExprId, RuntimeTryFact)>,
    implicit_callables: Vec<(ExprId, RuntimeImplicitCallableFact)>,
    pipes: Vec<(ExprId, RuntimePipeFact)>,
    captures: Vec<RuntimeCheckedCapture>,
    pure_programs: Vec<RuntimePureProgramFact>,
    project_function_instances: Vec<RuntimeProjectFunctionInstanceFact>,
    root_closures: Vec<RuntimeClosureInstanceFact>,
    project_function_roots: Vec<RuntimeProjectFunctionRootFact>,
    dialogue_applications: BTreeMap<ExprId, RuntimeDialogueApplication>,
    dialogue_content_fragments: Vec<RuntimeContentFragmentFact>,
    dialogue_lines: Option<Arc<AcceptedDialogueLineInventory>>,
    character_presentation_catalog: Option<Arc<CharacterPresentationCatalogData>>,
}

impl RuntimePlanSemanticFactInput {
    pub fn new() -> Self {
        Self {
            local_declarations: Vec::new(),
            flows: Vec::new(),
            expression_types: Vec::new(),
            pattern_types: Vec::new(),
            expression_literals: Vec::new(),
            pattern_literals: Vec::new(),
            pattern_items: Vec::new(),
            values: Vec::new(),
            selects: Vec::new(),
            nominal_records: Vec::new(),
            pattern_nominal_records: Vec::new(),
            expression_variants: Vec::new(),
            pattern_variants: Vec::new(),
            types: Vec::new(),
            calls: Vec::new(),
            postfix_candidates: Vec::new(),
            trait_methods: Vec::new(),
            iterations: Vec::new(),
            assertions: Vec::new(),
            triggers: BTreeMap::new(),
            assignments: Vec::new(),
            evaluated_effects: Vec::new(),
            choices: Vec::new(),
            awaits: Vec::new(),
            tries: Vec::new(),
            implicit_callables: Vec::new(),
            pipes: Vec::new(),
            captures: Vec::new(),
            pure_programs: Vec::new(),
            project_function_instances: Vec::new(),
            root_closures: Vec::new(),
            project_function_roots: Vec::new(),
            dialogue_applications: BTreeMap::new(),
            dialogue_content_fragments: Vec::new(),
            dialogue_lines: None,
            character_presentation_catalog: None,
        }
    }

    /// Appends one runtime-domain HIR local and its exact normalized type in
    /// canonical project order. Final plan-local identity issuance belongs
    /// exclusively to [`arcweft_core::plan::RuntimePlanBuilder`].
    pub fn push_local_declaration(&mut self, owner: LocalId, ty: RuntimeNormalizedType) {
        self.local_declarations.push((owner, ty));
    }

    pub fn push_flow(&mut self, owner: ItemId, flow: RuntimeFlowFact) {
        self.flows.push((owner, flow));
    }

    /// Stages the accepted normalized type of one selected runtime-domain
    /// final-HIR expression.
    pub fn push_expression_type(&mut self, owner: ExprId, ty: RuntimeNormalizedType) {
        self.expression_types.push((owner, ty));
    }

    /// Stages the accepted normalized type of one runtime-domain final-HIR
    /// pattern.
    pub fn push_pattern_type(&mut self, owner: PatternId, ty: RuntimeNormalizedType) {
        self.pattern_types.push((owner, ty));
    }

    pub fn push_expression_literal(&mut self, owner: ExprId, value: RuntimeValue) {
        self.expression_literals.push((owner, value));
    }

    pub fn push_pattern_literal(&mut self, owner: PatternId, value: RuntimeValue) {
        self.pattern_literals.push((owner, value));
    }

    pub fn push_pattern_item(&mut self, owner: PatternId, item: RuntimeProjectItem) {
        self.pattern_items.push((owner, item));
    }

    pub fn push_value(&mut self, owner: ExprId, value: RuntimeResolvedValue) {
        self.values.push((owner, value));
    }

    pub fn push_select(&mut self, owner: ExprId, select: RuntimeResolvedSelect) {
        self.selects.push((owner, select));
    }

    pub fn push_nominal_record(&mut self, owner: ExprId, record: RuntimeRecordExpressionFact) {
        self.nominal_records.push((owner, record));
    }

    pub fn push_pattern_nominal_record(
        &mut self,
        owner: PatternId,
        record: RuntimeRecordPatternFact,
    ) {
        self.pattern_nominal_records.push((owner, record));
    }

    pub fn push_expression_variant(&mut self, owner: ExprId, variant: RuntimeResolvedVariant) {
        self.expression_variants.push((owner, variant));
    }

    pub fn push_pattern_variant(&mut self, owner: PatternId, variant: RuntimeResolvedVariant) {
        self.pattern_variants.push((owner, variant));
    }

    pub fn push_type(&mut self, owner: TypeId, ty: RuntimeNormalizedType) {
        self.types.push((owner, ty));
    }

    pub fn push_call(&mut self, owner: ExprId, call: RuntimeResolvedCall) {
        self.calls.push((owner, call));
    }

    /// Stages the exact semantic winner for one immutable postfix ambiguity.
    pub fn push_postfix_candidate(&mut self, owner: ExprId, candidate: ExprId) {
        self.postfix_candidates.push((owner, candidate));
    }

    pub fn push_iteration(&mut self, owner: StmtId, evidence: RuntimeIteratorFact) {
        self.iterations.push((owner, evidence));
    }

    pub fn push_trait_method(&mut self, method: RuntimeTraitMethodFact) {
        self.trait_methods.push(method);
    }

    pub fn push_assertion(&mut self, owner: StmtId, admission: RuntimeAssertionAdmission) {
        self.assertions.push((owner, admission));
    }

    pub fn push_input_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Input)
    }

    pub fn push_event_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Event)
    }

    pub fn push_signal_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Signal)
    }

    pub fn push_timeout_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Timeout)
    }

    pub fn push_mark_trigger(
        &mut self,
        owner: StmtId,
        mark: RuntimeDialogueMarkFact,
    ) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Mark(mark))
    }

    pub fn push_select_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Select)
    }

    pub fn push_task_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Task)
    }

    pub fn push_scope_trigger(&mut self, owner: StmtId) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Scope)
    }

    pub fn push_expression_trigger(
        &mut self,
        owner: StmtId,
    ) -> Result<(), RuntimeSemanticFactsError> {
        self.insert_trigger(owner, RuntimeTriggerAdmissionKind::Expression)
    }

    fn insert_trigger(
        &mut self,
        owner: StmtId,
        kind: RuntimeTriggerAdmissionKind,
    ) -> Result<(), RuntimeSemanticFactsError> {
        match self.triggers.entry(owner) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(RuntimeTriggerAdmission::new(kind));
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(RuntimeSemanticFactsError::DuplicateFact {
                    family: RuntimeSemanticFactFamily::Trigger,
                })
            }
        }
    }

    /// Stages the sole checked writable place for one assignment statement.
    pub fn push_assignment(&mut self, owner: StmtId, assignment: RuntimeAssignmentFact) {
        self.assignments.push((owner, assignment));
    }

    pub fn push_evaluated_effect(&mut self, owner: StmtId, effect: RuntimeEvaluatedEffectFact) {
        self.evaluated_effects.push((owner, effect));
    }

    pub fn push_await(&mut self, owner: ExprId, fact: RuntimeAwaitFact) {
        self.awaits.push((owner, fact));
    }

    pub fn push_choice(&mut self, owner: ExprId, fact: RuntimeChoiceFact) {
        self.choices.push((owner, fact));
    }

    pub fn push_try(&mut self, owner: ExprId, fact: RuntimeTryFact) {
        self.tries.push((owner, fact));
    }

    pub fn push_implicit_callable(&mut self, owner: ExprId, fact: RuntimeImplicitCallableFact) {
        self.implicit_callables.push((owner, fact));
    }

    pub fn push_pipe(&mut self, owner: ExprId, fact: RuntimePipeFact) {
        self.pipes.push((owner, fact));
    }

    pub fn push_capture(&mut self, capture: RuntimeCheckedCapture) {
        self.captures.push(capture);
    }

    pub fn push_pure_program(&mut self, program: RuntimePureProgramFact) {
        self.pure_programs.push(program);
    }

    /// Stages one fully closed ordinary project-function instance. Equal keys
    /// must carry equal facts; publication rejects conflicting projections.
    pub fn push_project_function_instance(&mut self, instance: RuntimeProjectFunctionInstanceFact) {
        self.project_function_instances.push(instance);
    }

    pub fn push_root_closure(&mut self, closure: RuntimeClosureInstanceFact) {
        self.root_closures.push(closure);
    }

    pub fn push_project_function_root(&mut self, root: RuntimeProjectFunctionRootFact) {
        self.project_function_roots.push(root);
    }

    /// Stages the complete dialogue projection for the same single
    /// `RuntimePlanSemanticFacts::try_new` transaction as all other facts.
    pub fn attach_dialogue_projection(
        &mut self,
        catalog: Option<Arc<CharacterPresentationCatalogData>>,
        applications: BTreeMap<ExprId, RuntimeDialogueApplication>,
        fragments: Vec<RuntimeContentFragmentFact>,
        dialogue_lines: Arc<AcceptedDialogueLineInventory>,
    ) -> Result<(), RuntimeSemanticFactsError> {
        if !self.dialogue_applications.is_empty()
            || !self.dialogue_content_fragments.is_empty()
            || self.dialogue_lines.is_some()
            || self.character_presentation_catalog.is_some()
        {
            return Err(RuntimeSemanticFactsError::DuplicateFact {
                family: RuntimeSemanticFactFamily::DialogueApplication,
            });
        }
        let mut has_instance_dialogue = false;
        for (scope, semantics) in instance_semantic_roots(
            self.project_function_instances.iter(),
            self.root_closures.iter(),
        ) {
            semantics
                .visit_dialogue_applications(scope, &mut |_, _, _| has_instance_dialogue = true);
        }
        if (applications.is_empty() && !has_instance_dialogue) != catalog.is_none() {
            return Err(RuntimeSemanticFactsError::DialogueCatalogPresenceMismatch);
        }
        self.dialogue_applications = applications;
        self.dialogue_content_fragments = fragments;
        self.dialogue_lines = Some(dialogue_lines);
        self.character_presentation_catalog = catalog;
        Ok(())
    }
}

impl Default for RuntimePlanSemanticFactInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable semantic fact set bound to one exact executable project generation.
#[derive(Clone, Debug)]
pub struct RuntimePlanSemanticFacts {
    reachability: HirRuntimeReachabilityIdentity,
    view_value_reachability: Option<HirRuntimeReachabilityIdentity>,
    runtime_owners: BTreeSet<HirRuntimeExecutableOwner>,
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    local_declaration_order: Box<[LocalId]>,
    local_declarations: BTreeMap<LocalId, RuntimeNormalizedType>,
    flows: BTreeMap<ItemId, RuntimeFlowFact>,
    expression_types: BTreeMap<ExprId, RuntimeNormalizedType>,
    expression_children: BTreeMap<ExprId, Box<[ExprId]>>,
    pattern_types: BTreeMap<PatternId, RuntimeNormalizedType>,
    expression_literals: BTreeMap<ExprId, RuntimeValue>,
    pattern_literals: BTreeMap<PatternId, RuntimeValue>,
    pattern_items: BTreeMap<PatternId, RuntimeProjectItem>,
    values: BTreeMap<ExprId, RuntimeResolvedValue>,
    selects: BTreeMap<ExprId, RuntimeResolvedSelect>,
    nominal_records: BTreeMap<ExprId, RuntimeRecordExpressionFact>,
    pattern_nominal_records: BTreeMap<PatternId, RuntimeRecordPatternFact>,
    expression_variants: BTreeMap<ExprId, RuntimeResolvedVariant>,
    pattern_variants: BTreeMap<PatternId, RuntimeResolvedVariant>,
    types: BTreeMap<TypeId, RuntimeNormalizedType>,
    calls: BTreeMap<ExprId, RuntimeResolvedCall>,
    postfix_candidates: BTreeMap<ExprId, ExprId>,
    trait_methods: BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodFact>,
    iterations: BTreeMap<StmtId, RuntimeIteratorFact>,
    assertions: BTreeMap<StmtId, RuntimeAssertionAdmission>,
    triggers: BTreeMap<StmtId, RuntimeTriggerAdmission>,
    assignments: BTreeMap<StmtId, RuntimeAssignmentFact>,
    evaluated_effects: BTreeMap<StmtId, RuntimeEvaluatedEffectFact>,
    choices: BTreeMap<ExprId, RuntimeChoiceFact>,
    awaits: BTreeMap<ExprId, RuntimeAwaitFact>,
    tries: BTreeMap<ExprId, RuntimeTryFact>,
    implicit_callables: BTreeMap<ExprId, RuntimeImplicitCallableFact>,
    pipes: BTreeMap<ExprId, RuntimePipeFact>,
    captures: BTreeMap<CaptureId, RuntimeCheckedCapture>,
    pure_programs: BTreeMap<RuntimePureProgramId, RuntimePureProgramFact>,
    project_function_instances:
        BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeProjectFunctionInstanceFact>,
    root_closures: BTreeMap<ExprId, RuntimeClosureInstanceFact>,
    project_function_roots:
        BTreeMap<(ItemId, RuntimeProjectFunctionRootRole), RuntimeProjectFunctionRootFact>,
    dialogue_applications: BTreeMap<ExprId, RuntimeDialogueApplication>,
    dialogue_content_fragments: Vec<RuntimeContentFragmentFact>,
    dialogue_lines: Option<Arc<AcceptedDialogueLineInventory>>,
    character_presentation_catalog: Option<Arc<CharacterPresentationCatalogData>>,
}

/// Sole executable semantic-fact view selected before lowering one body.
///
/// A closed project-function instance never falls back to the global open
/// catalog. Keeping the mode choice in this owner type prevents individual
/// expression/pattern/flow helpers from drifting into family-specific lookup
/// rules.
#[derive(Clone, Copy)]
pub enum RuntimeExecutableSemanticFactView<'facts> {
    Global(&'facts RuntimePlanSemanticFacts),
    ProjectInstance(&'facts RuntimeProjectFunctionInstanceSemanticFacts),
}

/// Exact lexical executable that owns one selected semantic-fact view.
/// Nested closures retain their own closed key instead of being collapsed to
/// the root project instance or reconstructed from a raw expression owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeExecutableSemanticScope<'facts> {
    Global,
    ProjectFunction(&'facts RuntimeProjectFunctionInstanceKey),
    Closure(&'facts RuntimeClosureInstanceKey),
}

/// One lexical scope and the only semantic catalog valid while lowering it.
#[derive(Clone, Copy)]
pub struct RuntimeScopedExecutableSemanticFactView<'facts> {
    scope: RuntimeExecutableSemanticScope<'facts>,
    facts: RuntimeExecutableSemanticFactView<'facts>,
}

impl<'facts> RuntimeScopedExecutableSemanticFactView<'facts> {
    pub const fn global(facts: &'facts RuntimePlanSemanticFacts) -> Self {
        Self {
            scope: RuntimeExecutableSemanticScope::Global,
            facts: RuntimeExecutableSemanticFactView::Global(facts),
        }
    }

    pub const fn project_function(
        key: &'facts RuntimeProjectFunctionInstanceKey,
        facts: &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        Self {
            scope: RuntimeExecutableSemanticScope::ProjectFunction(key),
            facts: RuntimeExecutableSemanticFactView::ProjectInstance(facts),
        }
    }

    pub const fn closure(
        key: &'facts RuntimeClosureInstanceKey,
        facts: &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        Self {
            scope: RuntimeExecutableSemanticScope::Closure(key),
            facts: RuntimeExecutableSemanticFactView::ProjectInstance(facts),
        }
    }

    pub const fn scope(self) -> RuntimeExecutableSemanticScope<'facts> {
        self.scope
    }

    pub const fn facts(self) -> RuntimeExecutableSemanticFactView<'facts> {
        self.facts
    }

    pub const fn is_project_instance(self) -> bool {
        self.facts.is_project_instance()
    }

    pub fn expression_type(self, owner: ExprId) -> Option<&'facts RuntimeNormalizedType> {
        self.facts.expression_type(owner)
    }

    pub fn expression_children(self, owner: ExprId) -> Option<&'facts [ExprId]> {
        self.facts.expression_children(owner)
    }

    pub fn expression_literal(self, owner: ExprId) -> Option<&'facts RuntimeValue> {
        self.facts.expression_literal(owner)
    }

    pub fn expression_variant(self, owner: ExprId) -> Option<&'facts RuntimeResolvedVariant> {
        self.facts.expression_variant(owner)
    }

    pub fn value(self, owner: ExprId) -> Option<&'facts RuntimeResolvedValue> {
        self.facts.value(owner)
    }

    pub fn call(self, owner: ExprId) -> Option<&'facts RuntimeResolvedCall> {
        self.facts.call(owner)
    }

    pub fn select(self, owner: ExprId) -> Option<&'facts RuntimeResolvedSelect> {
        self.facts.select(owner)
    }

    pub fn nominal_record(self, owner: ExprId) -> Option<&'facts RuntimeRecordExpressionFact> {
        self.facts.nominal_record(owner)
    }

    pub fn postfix_candidate(self, owner: ExprId) -> Option<ExprId> {
        self.facts.postfix_candidate(owner)
    }

    pub fn implicit_callable(self, owner: ExprId) -> Option<&'facts RuntimeImplicitCallableFact> {
        self.facts.implicit_callable(owner)
    }

    pub fn pipe(self, owner: ExprId) -> Option<&'facts RuntimePipeFact> {
        self.facts.pipe(owner)
    }

    pub fn choice(self, owner: ExprId) -> Option<&'facts RuntimeChoiceFact> {
        self.facts.choice(owner)
    }

    pub fn awaited(self, owner: ExprId) -> Option<&'facts RuntimeAwaitFact> {
        self.facts.awaited(owner)
    }

    pub fn tried(self, owner: ExprId) -> Option<&'facts RuntimeTryFact> {
        self.facts.tried(owner)
    }

    pub fn assignment(self, owner: StmtId) -> Option<&'facts RuntimeAssignmentFact> {
        self.facts.assignment(owner)
    }

    pub fn evaluated_effect(self, owner: StmtId) -> Option<&'facts RuntimeEvaluatedEffectFact> {
        self.facts.evaluated_effect(owner)
    }

    pub fn iteration(self, owner: StmtId) -> Option<&'facts RuntimeIteratorFact> {
        self.facts.iteration(owner)
    }

    pub fn assertion(self, owner: StmtId) -> Option<RuntimeAssertionAdmission> {
        self.facts.assertion(owner)
    }

    pub fn trigger(self, owner: StmtId) -> Option<&'facts RuntimeTriggerAdmission> {
        self.facts.trigger(owner)
    }

    pub fn local_type(self, owner: LocalId) -> Option<&'facts RuntimeNormalizedType> {
        self.facts.local_type(owner)
    }

    pub fn dialogue_application(self, owner: ExprId) -> Option<&'facts RuntimeDialogueApplication> {
        self.facts.dialogue_application(owner)
    }

    pub fn dialogue_content_fragment_for_source(
        self,
        source: ExprId,
    ) -> Option<&'facts RuntimeContentFragmentFact> {
        self.facts.dialogue_content_fragment_for_source(source)
    }

    pub fn closure_instance(self, owner: ExprId) -> Option<&'facts RuntimeClosureInstanceFact> {
        self.facts.closure_instance(owner)
    }
}

impl<'facts> RuntimeExecutableSemanticFactView<'facts> {
    pub(crate) fn visit_runtime_expression_types(
        self,
        visitor: &mut impl FnMut(ExprId, &'facts RuntimeNormalizedType),
    ) {
        match self {
            Self::Global(facts) => {
                for (owner, ty) in &facts.expression_types {
                    visitor(*owner, ty);
                }
            }
            Self::ProjectInstance(facts) => {
                for projection in facts.type_projection() {
                    if let RuntimeProjectFunctionTypeProjection::Value {
                        owner: RuntimeProjectFunctionTypeOwner::Expression(owner),
                        ty,
                    } = projection
                    {
                        visitor(*owner, ty);
                    }
                }
            }
        }
    }

    pub const fn global(facts: &'facts RuntimePlanSemanticFacts) -> Self {
        Self::Global(facts)
    }

    pub const fn project_instance(
        facts: &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    ) -> Self {
        Self::ProjectInstance(facts)
    }

    pub const fn is_project_instance(self) -> bool {
        matches!(self, Self::ProjectInstance(_))
    }

    pub fn expression_type(self, owner: ExprId) -> Option<&'facts RuntimeNormalizedType> {
        match self {
            Self::Global(facts) => facts.expression_type(owner),
            Self::ProjectInstance(facts) => facts.expression_type(owner),
        }
    }

    pub fn pattern_type(self, owner: PatternId) -> Option<&'facts RuntimeNormalizedType> {
        match self {
            Self::Global(facts) => facts.pattern_type(owner),
            Self::ProjectInstance(facts) => facts.pattern_type(owner),
        }
    }

    pub fn local_type(self, owner: LocalId) -> Option<&'facts RuntimeNormalizedType> {
        match self {
            Self::Global(facts) => facts.local_type(owner),
            Self::ProjectInstance(facts) => facts.local_type(owner),
        }
    }

    pub fn ty(self, owner: TypeId) -> Option<&'facts RuntimeNormalizedType> {
        match self {
            Self::Global(facts) => facts.ty(owner),
            Self::ProjectInstance(facts) => facts.source_type(owner),
        }
    }

    /// Returns `None` only when `owner` is absent from the selected catalog.
    /// A present leaf expression returns `Some(&[])`.
    pub fn expression_children(self, owner: ExprId) -> Option<&'facts [ExprId]> {
        match self {
            Self::Global(facts) => facts
                .expression_children
                .get(&owner)
                .map(|children| children.as_ref()),
            Self::ProjectInstance(facts) => facts.expression_children(owner),
        }
    }

    pub fn expression_literal(self, owner: ExprId) -> Option<&'facts RuntimeValue> {
        match self {
            Self::Global(facts) => facts.expression_literal(owner),
            Self::ProjectInstance(facts) => facts.expression_literal(owner),
        }
    }

    pub fn pattern_literal(self, owner: PatternId) -> Option<&'facts RuntimeValue> {
        match self {
            Self::Global(facts) => facts.pattern_literal(owner),
            Self::ProjectInstance(facts) => facts.pattern_literal(owner),
        }
    }

    pub fn pattern_item(self, owner: PatternId) -> Option<&'facts RuntimeProjectItem> {
        match self {
            Self::Global(facts) => facts.pattern_item(owner),
            Self::ProjectInstance(facts) => facts.pattern_item(owner),
        }
    }

    pub fn value(self, owner: ExprId) -> Option<&'facts RuntimeResolvedValue> {
        match self {
            Self::Global(facts) => facts.value(owner),
            Self::ProjectInstance(facts) => facts.value(owner),
        }
    }

    pub fn select(self, owner: ExprId) -> Option<&'facts RuntimeResolvedSelect> {
        match self {
            Self::Global(facts) => facts.select(owner),
            Self::ProjectInstance(facts) => facts.select(owner),
        }
    }

    pub fn nominal_record(self, owner: ExprId) -> Option<&'facts RuntimeRecordExpressionFact> {
        match self {
            Self::Global(facts) => facts.nominal_record(owner),
            Self::ProjectInstance(facts) => facts.nominal_record(owner),
        }
    }

    pub fn pattern_nominal_record(
        self,
        owner: PatternId,
    ) -> Option<&'facts RuntimeRecordPatternFact> {
        match self {
            Self::Global(facts) => facts.pattern_nominal_record(owner),
            Self::ProjectInstance(facts) => facts.pattern_nominal_record(owner),
        }
    }

    pub fn expression_variant(self, owner: ExprId) -> Option<&'facts RuntimeResolvedVariant> {
        match self {
            Self::Global(facts) => facts.expression_variant(owner),
            Self::ProjectInstance(facts) => facts.expression_variant(owner),
        }
    }

    pub fn pattern_variant(self, owner: PatternId) -> Option<&'facts RuntimeResolvedVariant> {
        match self {
            Self::Global(facts) => facts.pattern_variant(owner),
            Self::ProjectInstance(facts) => facts.pattern_variant(owner),
        }
    }

    pub fn call(self, owner: ExprId) -> Option<&'facts RuntimeResolvedCall> {
        match self {
            Self::Global(facts) => facts.call(owner),
            Self::ProjectInstance(facts) => facts.call(owner),
        }
    }

    pub fn postfix_candidate(self, owner: ExprId) -> Option<ExprId> {
        match self {
            Self::Global(facts) => facts.postfix_candidate(owner),
            Self::ProjectInstance(facts) => facts.postfix_candidate(owner),
        }
    }

    pub fn iteration(self, owner: StmtId) -> Option<&'facts RuntimeIteratorFact> {
        match self {
            Self::Global(facts) => facts.iteration(owner),
            Self::ProjectInstance(facts) => facts.iteration(owner),
        }
    }

    pub fn assertion(self, owner: StmtId) -> Option<RuntimeAssertionAdmission> {
        match self {
            Self::Global(facts) => facts.assertion(owner),
            Self::ProjectInstance(facts) => facts.assertion(owner),
        }
    }

    pub fn trigger(self, owner: StmtId) -> Option<&'facts RuntimeTriggerAdmission> {
        match self {
            Self::Global(facts) => facts.trigger(owner),
            Self::ProjectInstance(facts) => facts.trigger(owner),
        }
    }

    pub fn assignment(self, owner: StmtId) -> Option<&'facts RuntimeAssignmentFact> {
        match self {
            Self::Global(facts) => facts.assignment(owner),
            Self::ProjectInstance(facts) => facts.assignment(owner),
        }
    }

    pub fn evaluated_effect(self, owner: StmtId) -> Option<&'facts RuntimeEvaluatedEffectFact> {
        match self {
            Self::Global(facts) => facts.evaluated_effect(owner),
            Self::ProjectInstance(facts) => facts.evaluated_effect(owner),
        }
    }

    pub fn awaited(self, owner: ExprId) -> Option<&'facts RuntimeAwaitFact> {
        match self {
            Self::Global(facts) => facts.awaited(owner),
            Self::ProjectInstance(facts) => facts.awaited(owner),
        }
    }

    pub fn choice(self, owner: ExprId) -> Option<&'facts RuntimeChoiceFact> {
        match self {
            Self::Global(facts) => facts.choice(owner),
            Self::ProjectInstance(facts) => facts.choice(owner),
        }
    }

    pub fn tried(self, owner: ExprId) -> Option<&'facts RuntimeTryFact> {
        match self {
            Self::Global(facts) => facts.tried(owner),
            Self::ProjectInstance(facts) => facts.tried(owner),
        }
    }

    pub fn implicit_callable(self, owner: ExprId) -> Option<&'facts RuntimeImplicitCallableFact> {
        match self {
            Self::Global(facts) => facts.implicit_callable(owner),
            Self::ProjectInstance(facts) => facts.implicit_callable(owner),
        }
    }

    pub fn pipe(self, owner: ExprId) -> Option<&'facts RuntimePipeFact> {
        match self {
            Self::Global(facts) => facts.pipe(owner),
            Self::ProjectInstance(facts) => facts.pipe(owner),
        }
    }

    pub fn capture(self, owner: CaptureId) -> Option<&'facts RuntimeCheckedCapture> {
        match self {
            Self::Global(facts) => facts.capture(owner),
            Self::ProjectInstance(facts) => facts.capture(owner),
        }
    }

    pub fn dialogue_application(self, owner: ExprId) -> Option<&'facts RuntimeDialogueApplication> {
        match self {
            Self::Global(facts) => facts.dialogue_application(owner),
            Self::ProjectInstance(facts) => facts.dialogue_application(owner),
        }
    }

    /// Selects a closed closure only from a closed project-instance catalog.
    /// Returning `None` in global mode is intentional: ordinary global
    /// closures use the generation-global FunctionSite authority instead.
    pub fn closure_instance(self, owner: ExprId) -> Option<&'facts RuntimeClosureInstanceFact> {
        match self {
            Self::Global(facts) => facts.root_closure(owner),
            Self::ProjectInstance(facts) => facts.closure_instance(owner),
        }
    }

    pub fn dialogue_content_fragment_for_source(
        self,
        source: ExprId,
    ) -> Option<&'facts RuntimeContentFragmentFact> {
        match self {
            Self::Global(facts) => facts.dialogue_content_fragment_for_source(source),
            Self::ProjectInstance(facts) => facts.dialogue_content_fragment_for_source(source),
        }
    }
}

#[derive(Clone, Copy)]
struct RuntimeSemanticOwnerSet<'a> {
    runtime: &'a HirRuntimeSemanticReachability<'a>,
    view_values: Option<&'a HirRuntimeSemanticReachability<'a>>,
}

/// Borrowed complete executable catalogs. Every consumer keeps the root's
/// exact lexical scope while visiting its nested closure catalogs.
fn instance_semantic_roots<'facts>(
    functions: impl Iterator<Item = &'facts RuntimeProjectFunctionInstanceFact> + 'facts,
    closures: impl Iterator<Item = &'facts RuntimeClosureInstanceFact> + 'facts,
) -> impl Iterator<
    Item = (
        RuntimeScopedExecutableSemanticFactView<'facts>,
        &'facts RuntimeProjectFunctionInstanceSemanticFacts,
    ),
> + 'facts {
    functions
        .map(|instance| {
            (
                RuntimeScopedExecutableSemanticFactView::project_function(
                    instance.key(),
                    instance.semantics(),
                ),
                instance.semantics(),
            )
        })
        .chain(closures.map(|closure| {
            (
                RuntimeScopedExecutableSemanticFactView::closure(
                    closure.key(),
                    closure.semantics(),
                ),
                closure.semantics(),
            )
        }))
}

impl<'a> RuntimeSemanticOwnerSet<'a> {
    const fn runtime_only(runtime: &'a HirRuntimeSemanticReachability<'a>) -> Self {
        Self {
            runtime,
            view_values: None,
        }
    }

    const fn with_view_values(
        runtime: &'a HirRuntimeSemanticReachability<'a>,
        view_values: &'a HirRuntimeSemanticReachability<'a>,
    ) -> Self {
        Self {
            runtime,
            view_values: Some(view_values),
        }
    }

    fn contains_expression(self, owner: ExprId) -> bool {
        self.runtime.contains_expression(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_expression(owner))
    }

    fn contains_pattern(self, owner: PatternId) -> bool {
        self.runtime.contains_pattern(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_pattern(owner))
    }

    fn contains_statement(self, owner: StmtId) -> bool {
        self.runtime.contains_statement(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_statement(owner))
    }

    fn contains_type(self, owner: TypeId) -> bool {
        self.runtime.contains_type(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_type(owner))
    }

    fn contains_local(self, owner: LocalId) -> bool {
        self.runtime.contains_local(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_local(owner))
    }

    fn contains_capture(self, owner: CaptureId) -> bool {
        self.runtime.contains_capture(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_capture(owner))
    }

    fn contains_runtime_owner(self, owner: &HirRuntimeExecutableOwner) -> bool {
        self.runtime.contains_runtime_owner(owner)
            || self
                .view_values
                .is_some_and(|owners| owners.contains_runtime_owner(owner))
    }

    fn executable_owners(
        self,
        owner: &HirRuntimeExecutableOwner,
    ) -> Option<&'a arcweft_lang_hir::project::HirRuntimeExecutableSemanticOwners> {
        self.runtime.executable_owners(owner).or_else(|| {
            self.view_values
                .and_then(|owners| owners.executable_owners(owner))
        })
    }

    fn expressions(self) -> BTreeSet<ExprId> {
        self.runtime
            .expressions()
            .chain(
                self.view_values
                    .into_iter()
                    .flat_map(HirRuntimeSemanticReachability::expressions),
            )
            .collect()
    }

    fn locals(self) -> BTreeSet<LocalId> {
        self.runtime
            .locals()
            .chain(
                self.view_values
                    .into_iter()
                    .flat_map(|owners| owners.locals()),
            )
            .collect()
    }

    fn reachable_executables(self) -> BTreeSet<HirRuntimeExecutableOwner> {
        self.runtime
            .reachable_executables()
            .chain(
                self.view_values
                    .into_iter()
                    .flat_map(HirRuntimeSemanticReachability::reachable_executables),
            )
            .cloned()
            .collect()
    }

    fn edges_from(
        self,
        source: HirRuntimeReachabilitySite,
    ) -> impl Iterator<Item = &'a HirRuntimeReachabilityEdge> + 'a {
        self.runtime.edge_from(source).chain(
            self.view_values
                .into_iter()
                .flat_map(move |owners| owners.edge_from(source)),
        )
    }

    fn selected_expression_type_owners(
        self,
    ) -> Result<BTreeSet<ExprId>, RuntimeSemanticFactsError> {
        let mut owners = self
            .runtime
            .selected_expression_type_owners()
            .map_err(|error| {
                RuntimeSemanticFactsError::RuntimeReachability(HirRuntimeReachabilityError::from(
                    error,
                ))
            })?;
        if let Some(view_values) = self.view_values {
            owners.extend(
                view_values
                    .selected_expression_type_owners()
                    .map_err(|error| {
                        RuntimeSemanticFactsError::RuntimeReachability(
                            HirRuntimeReachabilityError::from(error),
                        )
                    })?,
            );
        }
        Ok(owners)
    }

    fn selected_expression_children(
        self,
    ) -> Result<BTreeMap<ExprId, Box<[ExprId]>>, RuntimeSemanticFactsError> {
        let mut children: BTreeMap<ExprId, Box<[ExprId]>> = BTreeMap::new();
        // Each lexical partition owns the runtime child relation, including
        // an empty row for a closure value whose body executes in another
        // partition. The aggregate structural edge map omits those value rows.
        for reachability in std::iter::once(self.runtime).chain(self.view_values) {
            for executable in reachability.reachable_executables() {
                let owners = reachability
                    .executable_owners(executable)
                    .ok_or(RuntimeSemanticFactsError::ReachabilityMismatch)?;
                for owner in owners.expressions() {
                    let row = owners.expression_children(owner);
                    if children
                        .insert(owner, Box::from(row))
                        .is_some_and(|previous| previous.as_ref() != row)
                    {
                        return Err(RuntimeSemanticFactsError::ReachabilityMismatch);
                    }
                }
            }
        }
        Ok(children)
    }

    fn patterns(self) -> BTreeSet<PatternId> {
        self.runtime
            .patterns()
            .chain(
                self.view_values
                    .into_iter()
                    .flat_map(HirRuntimeSemanticReachability::patterns),
            )
            .collect()
    }
}

impl RuntimePlanSemanticFacts {
    pub const fn reachability(&self) -> &HirRuntimeReachabilityIdentity {
        &self.reachability
    }

    /// Reports whether one executable owner was admitted by the sole checked
    /// reachability closure used to construct these facts.
    pub fn contains_runtime_owner(&self, owner: &HirRuntimeExecutableOwner) -> bool {
        self.runtime_owners.contains(owner)
    }

    /// Validates every staged fact against the exact accepted executable module leases.
    #[allow(
        clippy::too_many_lines,
        reason = "fact publication validates every family and cross-owner identity in one all-or-nothing accepted-generation transaction"
    )]
    pub fn try_new(
        project: HirAnalysisProjectView<'_>,
        runtime_owners: &HirRuntimeSemanticReachability<'_>,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        Self::try_new_with_owner_set(
            project,
            RuntimeSemanticOwnerSet::runtime_only(runtime_owners),
            input,
        )
    }

    pub fn try_new_with_view_value_programs(
        project: HirAnalysisProjectView<'_>,
        runtime_owners: &HirRuntimeSemanticReachability<'_>,
        view_value_owners: &HirRuntimeSemanticReachability<'_>,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        if runtime_owners
            .roots()
            .any(|root| root.kind() == HirRuntimeReachabilityRootKind::CheckedViewValueProgram)
            || view_value_owners
                .roots()
                .any(|root| root.kind() != HirRuntimeReachabilityRootKind::CheckedViewValueProgram)
        {
            return Err(RuntimeSemanticFactsError::ReachabilityMismatch);
        }
        Self::try_new_with_owner_set(
            project,
            RuntimeSemanticOwnerSet::with_view_values(runtime_owners, view_value_owners),
            input,
        )
    }

    fn try_new_with_owner_set(
        project: HirAnalysisProjectView<'_>,
        runtime_owners: RuntimeSemanticOwnerSet<'_>,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        let supplied_snapshots = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect::<Box<[_]>>();
        if supplied_snapshots.as_ref() != runtime_owners.runtime.identity().module_snapshots()
            || runtime_owners.view_values.is_some_and(|owners| {
                supplied_snapshots.as_ref() != owners.identity().module_snapshots()
                    || owners.identity().symbol_world()
                        != runtime_owners.runtime.identity().symbol_world()
                    || owners.identity().symbol_revision()
                        != runtime_owners.runtime.identity().symbol_revision()
            })
        {
            return Err(RuntimeSemanticFactsError::ReachabilityMismatch);
        }
        let modules = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let snapshots = modules
            .iter()
            .map(|(id, module)| (*id, module.snapshot_id()))
            .collect();

        let project_function_instances = collect_unique(
            input
                .project_function_instances
                .into_iter()
                .map(|instance| (instance.key().clone(), instance)),
            RuntimeSemanticFactFamily::ProjectFunctionInstance,
        )?;
        for instance in project_function_instances.values() {
            validate_project_function_instance(&modules, runtime_owners, instance)?;
        }
        let root_closures = collect_unique(
            input
                .root_closures
                .into_iter()
                .map(|closure| (closure.owner(), closure)),
            RuntimeSemanticFactFamily::ClosureInstance,
        )?;
        if root_closures
            .values()
            .any(|closure| closure.key().enclosing_instance().is_some())
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
        let project_function_roots = collect_unique(
            input
                .project_function_roots
                .into_iter()
                .map(|root| ((root.entry(), root.role()), root)),
            RuntimeSemanticFactFamily::ProjectFunctionRoot,
        )?;
        let mut root_instances_by_callable = BTreeMap::new();
        for root in project_function_roots.values() {
            validate_project_function_root(
                &modules,
                runtime_owners,
                &project_function_instances,
                root,
            )?;
            match root_instances_by_callable
                .insert(root.instance().callable().clone(), root.instance().clone())
            {
                Some(previous) if previous != *root.instance() => {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionRoot);
                }
                Some(_) | None => {}
            }
        }
        let mut instance_expression_owners = BTreeSet::new();
        let mut instance_pattern_owners = BTreeSet::new();
        let mut instance_local_owners = BTreeSet::new();
        let mut instance_type_owners = BTreeSet::new();
        let mut instance_capture_owners = BTreeSet::new();
        let mut instance_statement_owners = BTreeSet::new();
        for instance in project_function_instances.values() {
            instance.visit_type_projections(&mut |projection| match projection.owner() {
                RuntimeProjectFunctionTypeOwner::Expression(owner) => {
                    instance_expression_owners.insert(owner);
                }
                RuntimeProjectFunctionTypeOwner::Pattern(owner) => {
                    instance_pattern_owners.insert(owner);
                }
                RuntimeProjectFunctionTypeOwner::Local(owner) => {
                    instance_local_owners.insert(owner);
                }
                RuntimeProjectFunctionTypeOwner::Type(owner) => {
                    instance_type_owners.insert(owner);
                }
            });
            instance.visit_captures(&mut |capture| {
                instance_capture_owners.insert(capture.capture());
            });
            instance.visit_statement_owners(&mut |statement| {
                instance_statement_owners.insert(statement);
            });
        }
        for closure in root_closures.values() {
            closure.semantics().visit_type_projections(
                &mut |projection| match projection.owner() {
                    RuntimeProjectFunctionTypeOwner::Expression(owner) => {
                        instance_expression_owners.insert(owner);
                    }
                    RuntimeProjectFunctionTypeOwner::Pattern(owner) => {
                        instance_pattern_owners.insert(owner);
                    }
                    RuntimeProjectFunctionTypeOwner::Local(owner) => {
                        instance_local_owners.insert(owner);
                    }
                    RuntimeProjectFunctionTypeOwner::Type(owner) => {
                        instance_type_owners.insert(owner);
                    }
                },
            );
            closure.semantics().visit_captures(&mut |capture| {
                instance_capture_owners.insert(capture.capture());
            });
            closure
                .semantics()
                .visit_statement_owners(&mut |statement| {
                    instance_statement_owners.insert(statement);
                });
        }
        let expected_local_declarations = runtime_owners
            .locals()
            .into_iter()
            .filter(|owner| !instance_local_owners.contains(owner))
            .collect::<Vec<_>>();

        let expression_types = collect_unique(
            input.expression_types,
            RuntimeSemanticFactFamily::ExpressionType,
        )?;
        for (owner, ty) in &expression_types {
            if instance_expression_owners.contains(owner) {
                return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                    family: RuntimeSemanticFactFamily::ExpressionType,
                });
            }
            resolve_expr(&modules, *owner)?;
            require_runtime_expression_owner(
                runtime_owners,
                *owner,
                RuntimeSemanticFactFamily::ExpressionType,
            )?;
            validate_normalized_type(&modules, ty)?;
        }
        let mut expression_children = runtime_owners.selected_expression_children()?;
        expression_children.retain(|owner, _| !instance_expression_owners.contains(owner));
        for (owner, children) in &expression_children {
            require_runtime_expression_owner(
                runtime_owners,
                *owner,
                RuntimeSemanticFactFamily::ExpressionChildren,
            )?;
            if !runtime_owners.contains_expression(*owner)
                || children
                    .iter()
                    .any(|child| !runtime_owners.contains_expression(*child))
            {
                return Err(RuntimeSemanticFactsError::InactiveExpressionFact {
                    expression: *owner,
                    family: RuntimeSemanticFactFamily::ExpressionChildren,
                });
            }
        }

        let pattern_types =
            collect_unique(input.pattern_types, RuntimeSemanticFactFamily::PatternType)?;
        for (owner, ty) in &pattern_types {
            if instance_pattern_owners.contains(owner) {
                return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                    family: RuntimeSemanticFactFamily::PatternType,
                });
            }
            resolve_pattern(&modules, *owner)?;
            require_runtime_pattern_owner(
                runtime_owners,
                *owner,
                RuntimeSemanticFactFamily::PatternType,
            )?;
            validate_normalized_type(&modules, ty)?;
        }

        let local_declarations = collect_unique(
            input.local_declarations.iter().cloned(),
            RuntimeSemanticFactFamily::LocalDeclaration,
        )?;
        let expected_local_set = expected_local_declarations
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if let Some((local, _)) = input
            .local_declarations
            .iter()
            .find(|(local, _)| !expected_local_set.contains(local))
        {
            return Err(RuntimeSemanticFactsError::ExtraLocalDeclaration { local: *local });
        }
        if let Some(local) = expected_local_declarations
            .iter()
            .find(|local| !local_declarations.contains_key(local))
        {
            return Err(RuntimeSemanticFactsError::MissingLocalDeclaration { local: *local });
        }
        for ((owner, ty), expected_owner) in input
            .local_declarations
            .iter()
            .zip(&expected_local_declarations)
        {
            if owner != expected_owner {
                return Err(
                    RuntimeSemanticFactsError::NonCanonicalLocalDeclarationOrder {
                        expected: *expected_owner,
                        actual: *owner,
                    },
                );
            }
            validate_normalized_type(&modules, ty)?;
        }

        let flows = collect_unique(input.flows, RuntimeSemanticFactFamily::FlowIdentity)?;
        for item in flows.keys() {
            let resolved = resolve_item(&modules, *item)?;
            if !matches!(resolved.kind(), HirItemKind::Flow(_)) {
                return Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: *item,
                    actual: resolved.kind().family(),
                });
            }
            if !runtime_owners.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(*item)) {
                return Err(RuntimeSemanticFactsError::OwnerOutsideReachability {
                    owner: HirRuntimeExecutableOwner::Item(*item),
                });
            }
        }

        let expression_literals = collect_unique(
            input.expression_literals,
            RuntimeSemanticFactFamily::ExpressionLiteral,
        )?;
        for expression in expression_literals.keys() {
            require_expr_family(
                &modules,
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::ExpressionLiteral,
                |kind| {
                    matches!(
                        kind,
                        HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_)
                    )
                },
            )?;
        }

        let pattern_literals = collect_unique(
            input.pattern_literals,
            RuntimeSemanticFactFamily::PatternLiteral,
        )?;
        for pattern in pattern_literals.keys() {
            require_pattern_family(
                &modules,
                runtime_owners,
                *pattern,
                RuntimeSemanticFactFamily::PatternLiteral,
                |kind| matches!(kind, HirPatternKind::Literal(_)),
            )?;
        }

        let pattern_items =
            collect_unique(input.pattern_items, RuntimeSemanticFactFamily::PatternItem)?;
        for (pattern, item) in &pattern_items {
            require_pattern_family(
                &modules,
                runtime_owners,
                *pattern,
                RuntimeSemanticFactFamily::PatternItem,
                |kind| matches!(kind, HirPatternKind::EntityReference(_)),
            )?;
            validate_project_item(&modules, item)?;
        }

        let values = collect_unique(input.values, RuntimeSemanticFactFamily::Value)?;
        for (expression, value) in &values {
            require_expr_family(
                &modules,
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Value,
                |kind| {
                    matches!(
                        kind,
                        HirExprKind::Path(_)
                            | HirExprKind::EntityReference(_)
                            | HirExprKind::ShortVariant(_)
                    )
                },
            )?;
            validate_resolved_value(&modules, runtime_owners, value)?;
            match (resolve_expr(&modules, *expression)?, value) {
                (
                    HirExprKind::Path(_),
                    RuntimeResolvedValue::ProjectItem(_) | RuntimeResolvedValue::DialogueLine(_),
                )
                | (
                    HirExprKind::EntityReference(_),
                    RuntimeResolvedValue::Local(_)
                    | RuntimeResolvedValue::ProjectCallable(_)
                    | RuntimeResolvedValue::Intrinsic(_)
                    | RuntimeResolvedValue::Registered(_)
                    | RuntimeResolvedValue::Constant(_),
                ) => {
                    return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                        expression: *expression,
                        expected: RuntimeSemanticFactFamily::Value,
                    });
                }
                (
                    HirExprKind::EntityReference(_),
                    RuntimeResolvedValue::ProjectItem(_) | RuntimeResolvedValue::DialogueLine(_),
                )
                | (HirExprKind::Path(_), _) => {}
                (HirExprKind::ShortVariant(_), RuntimeResolvedValue::CharacterLook { .. }) => {}
                _ => unreachable!("value fact family was checked immediately above"),
            }
        }

        let selects = collect_unique(input.selects, RuntimeSemanticFactFamily::Select)?;
        for (expression, select) in &selects {
            require_expr_family(
                &modules,
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Select,
                |kind| matches!(kind, HirExprKind::Select(_)),
            )?;
            validate_select(&modules, select)?;
        }

        let mut nominal_records = collect_unique(
            input.nominal_records,
            RuntimeSemanticFactFamily::NominalRecord,
        )?;
        for (expression, record) in &nominal_records {
            require_expr_family(
                &modules,
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::NominalRecord,
                |kind| matches!(kind, HirExprKind::Record(_) | HirExprKind::RecordLiteral(_)),
            )?;
            validate_record_expression_fact(&modules, *expression, record)?;
        }

        let mut pattern_nominal_records = collect_unique(
            input.pattern_nominal_records,
            RuntimeSemanticFactFamily::PatternNominalRecord,
        )?;
        for (pattern, record) in &pattern_nominal_records {
            require_pattern_family(
                &modules,
                runtime_owners,
                *pattern,
                RuntimeSemanticFactFamily::PatternNominalRecord,
                |kind| matches!(kind, HirPatternKind::Record { .. }),
            )?;
            validate_record_pattern_fact(&modules, *pattern, record)?;
            if record
                .structural()
                .is_some_and(|owner| pattern_types.get(pattern) != Some(owner))
            {
                return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                    pattern: *pattern,
                    expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                });
            }
        }

        let mut nominal_layouts = BTreeMap::new();
        for record in nominal_records.values_mut() {
            intern_nominal_record_layout(record.nominal_mut(), &mut nominal_layouts)?;
        }
        for record in pattern_nominal_records.values_mut() {
            if let Some(nominal) = record.nominal_mut() {
                intern_nominal_record_layout(nominal, &mut nominal_layouts)?;
            }
        }

        let expression_variants = collect_unique(
            input.expression_variants,
            RuntimeSemanticFactFamily::ExpressionVariant,
        )?;
        for (expression, variant) in &expression_variants {
            require_expr_family(
                &modules,
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::ExpressionVariant,
                |kind| matches!(kind, HirExprKind::ShortVariant(_) | HirExprKind::Path(_)),
            )?;
            validate_variant(&modules, variant)?;
        }

        let pattern_variants = collect_unique(
            input.pattern_variants,
            RuntimeSemanticFactFamily::PatternVariant,
        )?;
        for (pattern, variant) in &pattern_variants {
            require_pattern_family(
                &modules,
                runtime_owners,
                *pattern,
                RuntimeSemanticFactFamily::PatternVariant,
                |kind| matches!(kind, HirPatternKind::Variant(_)),
            )?;
            validate_variant(&modules, variant)?;
        }

        let types = collect_unique(input.types, RuntimeSemanticFactFamily::Type)?;
        for (owner, ty) in &types {
            if instance_type_owners.contains(owner) {
                return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                    family: RuntimeSemanticFactFamily::Type,
                });
            }
            let hir_type = module_for(&modules, owner.module())?
                .resolve_type(*owner)
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedType { ty: *owner })?;
            require_runtime_type_owner(runtime_owners, *owner)?;
            if hir_type.is_poisoned() {
                return Err(RuntimeSemanticFactsError::PoisonedType { ty: *owner });
            }
            validate_normalized_type(&modules, ty)?;
        }

        let calls = collect_unique(input.calls, RuntimeSemanticFactFamily::Call)?;
        let mut invoked_project_function_instances = project_function_roots
            .values()
            .map(|root| root.instance().clone())
            .collect::<BTreeSet<_>>();
        for (expression, call) in &calls {
            if instance_expression_owners.contains(expression) {
                return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                    family: RuntimeSemanticFactFamily::Call,
                });
            }
            let kind = resolve_expr(&modules, *expression)?;
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Call,
            )?;
            let (hir_call, attached_body) = match kind {
                HirExprKind::Call(hir_call) => (hir_call, None),
                HirExprKind::AttachedContentApplication(application) => {
                    let Some(invocation) = application.family().invocation() else {
                        return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                            expression: *expression,
                            expected: RuntimeSemanticFactFamily::Call,
                        });
                    };
                    (invocation, Some(application.body_presence()))
                }
                _ => {
                    return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                        expression: *expression,
                        expected: RuntimeSemanticFactFamily::Call,
                    });
                }
            };
            validate_call(
                &modules,
                &expression_types,
                *expression,
                hir_call,
                attached_body,
                call,
            )?;
            validate_project_function_call_materialization(&modules, call)?;
            if let Some(instance) = validate_project_function_instance_reference(
                *expression,
                call,
                expression_types.get(expression),
                &project_function_instances,
            )? {
                invoked_project_function_instances.insert(instance);
            }
            let executable = match call.dispatch() {
                RuntimeResolvedCallDispatch::Static(
                    RuntimeResolvedStaticCallTarget::Declaration(callable),
                ) => Some(HirRuntimeExecutableOwner::Item(callable.owner())),
                RuntimeResolvedCallDispatch::Static(
                    RuntimeResolvedStaticCallTarget::TraitMethod { method, .. },
                ) => Some(HirRuntimeExecutableOwner::ImplMethod(method.clone())),
                RuntimeResolvedCallDispatch::Static(
                    RuntimeResolvedStaticCallTarget::Intrinsic(_)
                    | RuntimeResolvedStaticCallTarget::Agent(_)
                    | RuntimeResolvedStaticCallTarget::AgentProbeComparison(_)
                    | RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError
                    | RuntimeResolvedStaticCallTarget::Variant(_)
                    | RuntimeResolvedStaticCallTarget::Reduction(_)
                    | RuntimeResolvedStaticCallTarget::StandardMap(_)
                    | RuntimeResolvedStaticCallTarget::Line(_)
                    | RuntimeResolvedStaticCallTarget::Registered(_)
                    | RuntimeResolvedStaticCallTarget::Host(_),
                )
                | RuntimeResolvedCallDispatch::Value { .. } => None,
            };
            if let Some(owner) = executable
                && !runtime_owners.contains_runtime_owner(&owner)
            {
                return Err(RuntimeSemanticFactsError::OwnerOutsideReachability { owner });
            }
        }
        for instance in project_function_instances.values() {
            let mut expression_types = BTreeMap::new();
            instance.visit_type_projections(&mut |projection| {
                if let (RuntimeProjectFunctionTypeOwner::Expression(owner), Some(ty)) =
                    (projection.owner(), projection.ty())
                {
                    expression_types.insert(owner, ty);
                }
            });
            let mut closed_calls = Vec::new();
            instance.visit_calls(&mut |owner, call| closed_calls.push((owner, call)));
            for (owner, call) in closed_calls {
                if let Some(invoked) = validate_project_function_instance_reference(
                    owner,
                    call,
                    expression_types.get(&owner).copied(),
                    &project_function_instances,
                )? {
                    invoked_project_function_instances.insert(invoked);
                }
            }
        }
        for closure in root_closures.values() {
            let mut closed_calls = Vec::new();
            closure
                .semantics()
                .visit_calls(&mut |owner, call| closed_calls.push((owner, call)));
            for (owner, call) in closed_calls {
                let mut expected_type = None;
                closure
                    .semantics()
                    .visit_type_projections(&mut |projection| {
                        if projection.owner() == RuntimeProjectFunctionTypeOwner::Expression(owner)
                        {
                            expected_type = projection.ty();
                        }
                    });
                if let Some(invoked) = validate_project_function_instance_reference(
                    owner,
                    call,
                    expected_type,
                    &project_function_instances,
                )? {
                    invoked_project_function_instances.insert(invoked);
                }
            }
        }
        if project_function_instances
            .keys()
            .any(|key| !invoked_project_function_instances.contains(key))
        {
            return Err(RuntimeSemanticFactsError::UnreferencedProjectFunctionInstance);
        }

        let choices = collect_unique(input.choices, RuntimeSemanticFactFamily::Choice)?;
        for (expression, fact) in &choices {
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Choice,
            )?;
            let HirExprKind::Choice(choice) = resolve_expr(&modules, *expression)? else {
                return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                    expression: *expression,
                });
            };
            if fact.option_ids().len() != choice.body().items().len()
                || fact
                    .public_id()
                    .is_some_and(|id| id.as_str().split('.').next() != Some("choice"))
                || fact
                    .option_ids()
                    .iter()
                    .any(|id| id.as_str().split('.').next() != Some("choice"))
            {
                return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                    expression: *expression,
                });
            }
            let mut previous = None;
            for goto in fact.gotos() {
                if previous.is_some_and(|previous| previous >= goto.arm()) {
                    return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                        expression: *expression,
                    });
                }
                let index = usize::try_from(goto.arm()).map_err(|_| {
                    RuntimeSemanticFactsError::InvalidChoiceFact {
                        expression: *expression,
                    }
                })?;
                if !matches!(
                    choice.body().items().get(index),
                    Some(HirChoiceItem::CompactArm(arm))
                        if matches!(arm.action(), HirChoiceCompactAction::Goto(_))
                ) || goto.target().family() != DeclarationIdentityFamily::Flow
                {
                    return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                        expression: *expression,
                    });
                }
                validate_project_item(&modules, goto.target())?;
                let RuntimeProjectItemOwner::StructuralFlow { owner, runtime } =
                    goto.target().owner()
                else {
                    return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                        expression: *expression,
                    });
                };
                if flows.get(owner).map(RuntimeFlowFact::identity) != Some(runtime) {
                    return Err(RuntimeSemanticFactsError::InvalidChoiceFact {
                        expression: *expression,
                    });
                }
                previous = Some(goto.arm());
            }
        }

        let awaits = collect_unique(input.awaits, RuntimeSemanticFactFamily::Await)?;
        for (expression, fact) in &awaits {
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Await,
            )?;
            let HirExprKind::Await(awaited) = resolve_expr(&modules, *expression)? else {
                return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                    expression: *expression,
                });
            };
            if awaited.operand() != fact.operand()
                || fact.observers().len() != awaited.branches().len()
            {
                return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                    expression: *expression,
                });
            }
            let Some(operand) = expression_types.get(&fact.operand()) else {
                return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                    expression: *expression,
                });
            };
            let RuntimeTypeShape::Need(item) = operand.shape() else {
                return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                    expression: *expression,
                });
            };
            if expression_types.get(expression) != Some(item.as_ref()) {
                return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                    expression: *expression,
                });
            }
            for (authored, checked) in awaited.branches().iter().zip(fact.observers()) {
                if authored.kind() != HirAwaitBranchKind::Pending
                    || authored.pattern() != Some(checked.pattern())
                    || !matches!(
                        pattern_types
                            .get(&checked.pattern())
                            .map(RuntimeNormalizedType::shape),
                        Some(RuntimeTypeShape::Progress)
                    )
                {
                    return Err(RuntimeSemanticFactsError::InvalidAwaitFact {
                        expression: *expression,
                    });
                }
            }
        }

        let implicit_callables = collect_unique(
            input.implicit_callables,
            RuntimeSemanticFactFamily::ImplicitCallable,
        )?;
        for (expression, fact) in &implicit_callables {
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::ImplicitCallable,
            )?;
            let Some(expression_type) = expression_types.get(expression) else {
                return Err(RuntimeSemanticFactsError::InvalidImplicitCallableFact {
                    expression: *expression,
                });
            };
            let RuntimeTypeShape::Function { parameters, result } = expression_type.shape() else {
                return Err(RuntimeSemanticFactsError::InvalidImplicitCallableFact {
                    expression: *expression,
                });
            };
            if parameters.len() != 1
                || &parameters[0] != fact.parameter()
                || result.as_ref() != fact.result()
                || fact.placeholders().is_empty()
                || !all_unique(fact.placeholders())
                || !all_unique(fact.captures())
            {
                return Err(RuntimeSemanticFactsError::InvalidImplicitCallableFact {
                    expression: *expression,
                });
            }
            for placeholder in fact.placeholders() {
                if !matches!(
                    resolve_expr(&modules, *placeholder)?,
                    HirExprKind::Placeholder(HirPlaceholderKind::PartialApplication)
                ) || (*placeholder != *expression
                    && expression_types.get(placeholder) != Some(fact.parameter()))
                {
                    return Err(RuntimeSemanticFactsError::InvalidImplicitCallableFact {
                        expression: *expression,
                    });
                }
            }
            for capture in fact.captures() {
                if !local_declarations.contains_key(capture) {
                    return Err(RuntimeSemanticFactsError::InvalidImplicitCallableFact {
                        expression: *expression,
                    });
                }
            }
        }

        let pipes = collect_unique(input.pipes, RuntimeSemanticFactFamily::Pipe)?;
        for (expression, fact) in &pipes {
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Pipe,
            )?;
            let HirExprKind::Pipe(pipe) = resolve_expr(&modules, *expression)? else {
                return Err(RuntimeSemanticFactsError::InvalidPipeFact {
                    expression: *expression,
                });
            };
            let Some(left_type) = expression_types.get(&fact.left()) else {
                return Err(RuntimeSemanticFactsError::InvalidPipeFact {
                    expression: *expression,
                });
            };
            if pipe.left() != fact.left()
                || pipe.right() != fact.right()
                || !all_unique(fact.placeholders())
            {
                return Err(RuntimeSemanticFactsError::InvalidPipeFact {
                    expression: *expression,
                });
            }
            for placeholder in fact.placeholders() {
                if !matches!(
                    resolve_expr(&modules, *placeholder)?,
                    HirExprKind::Placeholder(HirPlaceholderKind::PipeLeft)
                ) || expression_types.get(placeholder) != Some(left_type)
                {
                    return Err(RuntimeSemanticFactsError::InvalidPipeFact {
                        expression: *expression,
                    });
                }
            }
        }

        let try_facts = collect_unique(input.tries, RuntimeSemanticFactFamily::Try)?;
        for (expression, fact) in &try_facts {
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::Try,
            )?;
            let HirExprKind::Try(tried) = resolve_expr(&modules, *expression)? else {
                return Err(RuntimeSemanticFactsError::InvalidTryFact {
                    expression: *expression,
                });
            };
            let result_matches = match fact.boundary() {
                RuntimeTryBoundaryOwner::ImplicitFunctionSite(boundary) => implicit_callables
                    .get(&boundary)
                    .is_some_and(|callable| callable.result() == fact.carrier().success()),
                RuntimeTryBoundaryOwner::Infallible
                | RuntimeTryBoundaryOwner::CarrierBlock(_)
                | RuntimeTryBoundaryOwner::ExplicitFunctionSite(_)
                | RuntimeTryBoundaryOwner::Callable(_) => {
                    expression_types.get(expression) == Some(fact.carrier().success())
                }
            };
            if tried.operand() != fact.operand() || !result_matches {
                return Err(RuntimeSemanticFactsError::InvalidTryFact {
                    expression: *expression,
                });
            }
            let Some(operand) = expression_types.get(&fact.operand()) else {
                return Err(RuntimeSemanticFactsError::InvalidTryFact {
                    expression: *expression,
                });
            };
            if operand != fact.carrier_type() {
                return Err(RuntimeSemanticFactsError::InvalidTryFact {
                    expression: *expression,
                });
            }
            let carrier_matches = match (fact.carrier(), operand.shape()) {
                (
                    RuntimeTryCarrierFact::Result { success, residual },
                    RuntimeTypeShape::Result { value, error, .. },
                ) => success == value.as_ref() && residual.as_ref() == error.as_ref(),
                (
                    RuntimeTryCarrierFact::Option { success },
                    RuntimeTypeShape::Option { item, .. },
                ) => success == item.as_ref(),
                _ => false,
            };
            if !carrier_matches || !try_boundary_type_matches(fact) {
                return Err(RuntimeSemanticFactsError::InvalidTryFact {
                    expression: *expression,
                });
            }
            match fact.boundary() {
                RuntimeTryBoundaryOwner::Infallible => {
                    if !matches!(
                        fact.carrier(),
                            RuntimeTryCarrierFact::Result { residual, .. }
                                if matches!(residual.shape(), RuntimeTypeShape::Never)
                    ) {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    }
                }
                RuntimeTryBoundaryOwner::CarrierBlock(boundary) => {
                    let HirExprKind::ComputationBlock(block) = resolve_expr(&modules, boundary)?
                    else {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    };
                    let family_matches = matches!(
                        (block.kind(), fact.carrier()),
                        (
                            arcweft_lang_hir::expr::HirComputationBlockKind::Result,
                            RuntimeTryCarrierFact::Result { .. }
                        ) | (
                            arcweft_lang_hir::expr::HirComputationBlockKind::Option,
                            RuntimeTryCarrierFact::Option { .. }
                        )
                    );
                    if !family_matches
                        || expression_types.get(&boundary) != Some(fact.boundary_type())
                    {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    }
                }
                RuntimeTryBoundaryOwner::ExplicitFunctionSite(boundary) => {
                    let kind = resolve_expr(&modules, boundary)?;
                    let valid_owner = matches!(kind, HirExprKind::Closure(_));
                    let boundary_result =
                        expression_types
                            .get(&boundary)
                            .and_then(|ty| match ty.shape() {
                                RuntimeTypeShape::Function { result, .. } => Some(result.as_ref()),
                                _ => None,
                            });
                    if !valid_owner || boundary_result != Some(fact.boundary_type()) {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    }
                }
                RuntimeTryBoundaryOwner::ImplicitFunctionSite(boundary) => {
                    if implicit_callables
                        .get(&boundary)
                        .is_none_or(|callable| callable.result() != fact.boundary_type())
                    {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    }
                }
                RuntimeTryBoundaryOwner::Callable(boundary) => {
                    if !boundary.as_bytes().iter().any(|byte| *byte != 0) {
                        return Err(RuntimeSemanticFactsError::InvalidTryFact {
                            expression: *expression,
                        });
                    }
                }
            }
        }

        let postfix_candidates = collect_unique(
            input.postfix_candidates,
            RuntimeSemanticFactFamily::PostfixCandidate,
        )?;
        for (expression, candidate) in &postfix_candidates {
            let kind = resolve_expr(&modules, *expression)?;
            require_runtime_expression_owner(
                runtime_owners,
                *expression,
                RuntimeSemanticFactFamily::PostfixCandidate,
            )?;
            resolve_expr(&modules, *candidate)?;
            require_runtime_expression_owner(
                runtime_owners,
                *candidate,
                RuntimeSemanticFactFamily::PostfixCandidate,
            )?;
            let HirExprKind::PostfixBracket(postfix) = kind else {
                return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                    expression: *expression,
                    expected: RuntimeSemanticFactFamily::PostfixCandidate,
                });
            };
            let arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                index,
                dialogue,
            } = postfix.candidates()
            else {
                return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                    expression: *expression,
                    expected: RuntimeSemanticFactFamily::PostfixCandidate,
                });
            };
            if candidate != index && candidate != dialogue {
                return Err(RuntimeSemanticFactsError::WrongPostfixCandidate {
                    expression: *expression,
                    candidate: *candidate,
                });
            }
        }

        for expression in runtime_owners
            .expressions()
            .into_iter()
            .filter(|owner| !instance_expression_owners.contains(owner))
        {
            if matches!(
                resolve_expr(&modules, expression)?,
                HirExprKind::PostfixBracket(_)
            ) && !postfix_candidates.contains_key(&expression)
            {
                return Err(RuntimeSemanticFactsError::MissingPostfixCandidate { expression });
            }
        }
        validate_complete_expression_types(
            runtime_owners,
            &expression_types,
            &instance_expression_owners,
        )?;
        validate_complete_pattern_types(runtime_owners, &pattern_types, &instance_pattern_owners)?;

        let trait_methods = collect_unique(
            input
                .trait_methods
                .into_iter()
                .map(|method| (method.declaration().clone(), method)),
            RuntimeSemanticFactFamily::TraitMethod,
        )?;
        for method in trait_methods.values() {
            validate_trait_method(&modules, method)?;
            let owner = HirRuntimeExecutableOwner::ImplMethod(method.declaration().clone());
            if !runtime_owners.contains_runtime_owner(&owner) {
                return Err(RuntimeSemanticFactsError::OwnerOutsideReachability { owner });
            }
        }

        let iterations = collect_unique(input.iterations, RuntimeSemanticFactFamily::Iteration)?;
        if iterations
            .keys()
            .any(|owner| instance_statement_owners.contains(owner))
        {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Iteration,
            });
        }
        for (statement, evidence) in &iterations {
            require_stmt_family(
                &modules,
                runtime_owners,
                *statement,
                RuntimeSemanticFactFamily::Iteration,
                |kind| matches!(kind, HirStmtKind::For(_)),
            )?;
            match evidence {
                RuntimeIteratorFact::Builtin(builtin) => {
                    for ty in [
                        builtin.item(),
                        builtin.iterator(),
                        builtin.next_value(),
                        builtin.step(),
                    ] {
                        validate_normalized_type(&modules, ty)?;
                    }
                }
                RuntimeIteratorFact::Witness(witness) => {
                    validate_normalized_type(&modules, witness.item())?;
                    validate_normalized_type(&modules, witness.iterator())?;
                }
            }
            validate_iterator_witness_method_edges(
                runtime_owners,
                *statement,
                evidence,
                &trait_methods,
            )?;
        }

        let assertions = collect_unique(input.assertions, RuntimeSemanticFactFamily::Assertion)?;
        if assertions
            .keys()
            .any(|owner| instance_statement_owners.contains(owner))
        {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Assertion,
            });
        }
        for statement in assertions.keys() {
            require_stmt_family(
                &modules,
                runtime_owners,
                *statement,
                RuntimeSemanticFactFamily::Assertion,
                |kind| matches!(kind, HirStmtKind::Assertion { .. }),
            )?;
        }

        let triggers = input.triggers;
        let expected_triggers = modules
            .values()
            .flat_map(|module| module.statements().map(|(statement, _)| statement))
            .filter(|statement| runtime_owners.contains_statement(*statement))
            .filter(|statement| !instance_statement_owners.contains(statement))
            .filter(|statement| {
                matches!(
                    resolve_stmt(&modules, *statement),
                    Ok(HirStmtKind::On { .. })
                )
            })
            .collect::<BTreeSet<_>>();
        if triggers
            .keys()
            .any(|owner| instance_statement_owners.contains(owner))
        {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Trigger,
            });
        }
        for statement in triggers.keys() {
            // HIR participates only in the reachable `On` owner-set check.
            // The checked trigger variant and payload were selected by the
            // compiler's exhaustive projection and are never reclassified
            // from `HirTrigger` here.
            require_stmt_family(
                &modules,
                runtime_owners,
                *statement,
                RuntimeSemanticFactFamily::Trigger,
                |kind| matches!(kind, HirStmtKind::On { .. }),
            )?;
        }
        if let Some(statement) = expected_triggers
            .iter()
            .find(|statement| !triggers.contains_key(statement))
        {
            return Err(RuntimeSemanticFactsError::MissingTriggerFact {
                statement: *statement,
            });
        }

        let evaluated_effects = collect_unique(
            input.evaluated_effects,
            RuntimeSemanticFactFamily::EvaluatedEffect,
        )?;
        if evaluated_effects
            .keys()
            .any(|owner| instance_statement_owners.contains(owner))
        {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::EvaluatedEffect,
            });
        }
        for (statement, effect) in &evaluated_effects {
            require_stmt_family(
                &modules,
                runtime_owners,
                *statement,
                RuntimeSemanticFactFamily::EvaluatedEffect,
                |kind| matches!(kind, HirStmtKind::Expression { .. }),
            )?;
            evaluated_effect::validate_evaluated_effect(
                &modules,
                &expression_types,
                &calls,
                *statement,
                effect,
            )?;
        }

        let assignments = collect_unique(input.assignments, RuntimeSemanticFactFamily::Assignment)?;
        let expected_assignments = modules
            .values()
            .flat_map(|module| module.statements().map(|(statement, _)| statement))
            .filter(|statement| runtime_owners.contains_statement(*statement))
            .filter(|statement| !instance_statement_owners.contains(statement))
            .filter(|statement| {
                matches!(
                    resolve_stmt(&modules, *statement),
                    Ok(HirStmtKind::Assign { .. })
                )
            })
            .collect::<BTreeSet<_>>();
        if assignments
            .keys()
            .any(|owner| instance_statement_owners.contains(owner))
        {
            return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                family: RuntimeSemanticFactFamily::Assignment,
            });
        }
        if let Some(statement) = expected_assignments
            .iter()
            .find(|statement| !assignments.contains_key(statement))
        {
            return Err(RuntimeSemanticFactsError::MissingAssignmentFact {
                statement: *statement,
            });
        }
        for (statement, assignment) in &assignments {
            require_stmt_family(
                &modules,
                runtime_owners,
                *statement,
                RuntimeSemanticFactFamily::Assignment,
                |kind| matches!(kind, HirStmtKind::Assign { .. }),
            )?;
            validate_assignment(
                &modules,
                &local_declarations,
                &expression_types,
                &values,
                &selects,
                *statement,
                assignment,
            )?;
        }

        let mut captures = BTreeMap::new();
        for checked in input.captures {
            let id = checked.capture();
            if instance_capture_owners.contains(&id) {
                return Err(RuntimeSemanticFactsError::InstanceOwnedGlobalFact {
                    family: RuntimeSemanticFactFamily::Capture,
                });
            }
            let capture = module_for(&modules, id.module())?
                .resolve_capture(id)
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedCapture { capture: id })?;
            require_runtime_capture_owner(runtime_owners, id)?;
            if runtime_owners
                .executable_owners(&HirRuntimeExecutableOwner::Closure(capture.closure()))
                .is_none_or(|owners| !owners.capture_plan().contains(checked.projection()))
            {
                return Err(RuntimeSemanticFactsError::InvalidCaptureProjection { capture: id });
            }
            module_for(&modules, capture.local().module())?
                .resolve_local(capture.local())
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedLocal {
                    local: capture.local(),
                })?;
            require_runtime_local_reference(runtime_owners, capture.local())?;
            validate_normalized_type(&modules, checked.ty())?;
            if captures.insert(id, checked).is_some() {
                return Err(RuntimeSemanticFactsError::DuplicateFact {
                    family: RuntimeSemanticFactFamily::Capture,
                });
            }
        }

        let pure_programs = validate_pure_programs(
            &modules,
            runtime_owners,
            &expression_types,
            &local_declarations,
            &captures,
            input.pure_programs,
        )?;
        let dialogue_applications = input.dialogue_applications;
        let dialogue_content_fragments = input.dialogue_content_fragments;
        let mut fragment_ids = BTreeSet::new();
        let mut fragment_sources = BTreeSet::new();
        let mut fragment_templates = BTreeSet::new();
        for fragment in &dialogue_content_fragments {
            if !fragment_ids.insert(fragment.id())
                || !fragment_sources.insert(fragment.source())
                || !fragment_templates.insert(fragment.template().id())
            {
                return Err(RuntimeSemanticFactsError::DuplicateContentFragment {
                    expression: fragment.source(),
                });
            }
            require_runtime_expression_owner(
                runtime_owners,
                fragment.source(),
                RuntimeSemanticFactFamily::DialogueApplication,
            )?;
            for value in fragment.values() {
                require_runtime_expression_owner(
                    runtime_owners,
                    value.expression(),
                    RuntimeSemanticFactFamily::DialogueApplication,
                )?;
                validate_normalized_type(&modules, value.ty())?;
                if expression_types.get(&value.expression()) != Some(value.ty()) {
                    return Err(RuntimeSemanticFactsError::InvalidContentFragment {
                        expression: fragment.source(),
                    });
                }
            }
            for effect in fragment.effects() {
                if !evaluated_effect::validate_evaluated_effect_site(&modules, effect.operation())
                    || !evaluated_effect::validate_evaluated_effect_operation(
                        &modules,
                        &expression_types,
                        &calls,
                        effect.operation().application_site(),
                        effect.operation().effect(),
                    )
                {
                    return Err(RuntimeSemanticFactsError::InvalidContentFragment {
                        expression: fragment.source(),
                    });
                }
                for capture in effect.captures() {
                    validate_normalized_type(&modules, capture.ty())?;
                    if local_declarations.get(&capture.local()) != Some(capture.ty()) {
                        return Err(RuntimeSemanticFactsError::InvalidContentFragment {
                            expression: fragment.source(),
                        });
                    }
                }
            }
        }
        let mut global_mark_facts = BTreeMap::new();
        for fragment in &dialogue_content_fragments {
            for mark in fragment.marks() {
                if global_mark_facts
                    .insert(mark.coordinate().clone(), mark.key())
                    .is_some()
                {
                    return Err(RuntimeSemanticFactsError::InvalidContentFragment {
                        expression: fragment.source(),
                    });
                }
            }
        }
        for (statement, admission) in &triggers {
            let hir = resolve_stmt(&modules, *statement)?;
            match (hir, admission.dialogue_mark()) {
                (
                    HirStmtKind::On {
                        trigger: HirTrigger::Mark(source),
                        ..
                    },
                    Some(mark),
                ) if source.ordinal() == mark.coordinate().ordinal()
                    && global_mark_facts.get(mark.coordinate()) == Some(&mark.key()) => {}
                (
                    HirStmtKind::On {
                        trigger: HirTrigger::Mark(_),
                        ..
                    },
                    _,
                )
                | (_, Some(_)) => {
                    return Err(RuntimeSemanticFactsError::InvalidTriggerFact {
                        statement: *statement,
                    });
                }
                (_, None) => {}
            }
        }
        let mut duplicate_instance_template = None;
        for (scope, semantics) in
            instance_semantic_roots(project_function_instances.values(), root_closures.values())
        {
            semantics.visit_content_fragments(scope, &mut |_, fragment| {
                if !fragment_templates.insert(fragment.template().id())
                    && duplicate_instance_template.is_none()
                {
                    duplicate_instance_template = Some(fragment.source());
                }
            });
        }
        if let Some(expression) = duplicate_instance_template {
            return Err(RuntimeSemanticFactsError::DuplicateContentFragment { expression });
        }
        let dialogue_lines = input.dialogue_lines;
        let character_presentation_catalog = input.character_presentation_catalog;
        let mut has_instance_dialogue = false;
        for (scope, semantics) in
            instance_semantic_roots(project_function_instances.values(), root_closures.values())
        {
            semantics
                .visit_dialogue_applications(scope, &mut |_, _, _| has_instance_dialogue = true);
        }
        if (dialogue_applications.is_empty() && !has_instance_dialogue)
            != character_presentation_catalog.is_none()
        {
            return Err(RuntimeSemanticFactsError::DialogueCatalogPresenceMismatch);
        }

        let facts = Self {
            reachability: runtime_owners.runtime.identity().clone(),
            view_value_reachability: runtime_owners
                .view_values
                .map(|owners| owners.identity().clone()),
            runtime_owners: runtime_owners.reachable_executables(),
            snapshots,
            local_declaration_order: expected_local_declarations.into_boxed_slice(),
            local_declarations,
            flows,
            expression_types,
            expression_children,
            pattern_types,
            expression_literals,
            pattern_literals,
            pattern_items,
            values,
            selects,
            nominal_records,
            pattern_nominal_records,
            expression_variants,
            pattern_variants,
            types,
            calls,
            postfix_candidates,
            trait_methods,
            iterations,
            assertions,
            triggers,
            assignments,
            evaluated_effects,
            choices,
            awaits,
            tries: try_facts,
            implicit_callables,
            pipes,
            captures,
            pure_programs,
            project_function_instances,
            root_closures,
            project_function_roots,
            dialogue_applications,
            dialogue_content_fragments,
            dialogue_lines,
            character_presentation_catalog,
        };
        for owner in facts.expression_types.keys() {
            if matches!(resolve_expr(&modules, *owner)?, HirExprKind::Closure(_))
                && !facts.is_pure_program_closure(*owner)
                && !facts.root_closures.contains_key(owner)
            {
                return Err(RuntimeSemanticFactsError::MissingClosureInstance {
                    expression: *owner,
                });
            }
        }
        for closure in facts.root_closures.values() {
            let callable = RuntimeCallableId::from_checked_digest(
                closure
                    .key()
                    .closure()
                    .owner()
                    .semantic_digest()
                    .into_bytes(),
            );
            validate_closure_instance(
                &modules,
                runtime_owners,
                None,
                &callable,
                RuntimeExecutableSemanticFactView::Global(&facts),
                closure.owner(),
                closure,
            )?;
        }
        if let Some(catalog) = facts.character_presentation_catalog.as_ref() {
            let dialogue_owners = RuntimeSemanticOwnerSet::runtime_only(runtime_owners.runtime);
            for (owner, application) in &facts.dialogue_applications {
                facts.validate_dialogue_application(
                    &modules,
                    dialogue_owners,
                    catalog,
                    *owner,
                    application,
                )?;
            }
            let lines = facts
                .dialogue_lines
                .as_deref()
                .ok_or(RuntimeSemanticFactsError::DialogueCatalogPresenceMismatch)?;
            for (_, semantics) in instance_semantic_roots(
                facts.project_function_instances.values(),
                facts.root_closures.values(),
            ) {
                validate_project_instance_dialogue_applications(
                    &modules, catalog, lines, semantics,
                )?;
            }
        }
        Ok(facts)
    }

    fn validate_dialogue_application(
        &self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        runtime_owners: RuntimeSemanticOwnerSet<'_>,
        catalog: &CharacterPresentationCatalogData,
        owner: ExprId,
        application: &RuntimeDialogueApplication,
    ) -> Result<(), RuntimeSemanticFactsError> {
        let fragment = self
            .dialogue_content_fragments
            .iter()
            .find(|fragment| fragment.template().id() == application.content().template_id())
            .ok_or(RuntimeSemanticFactsError::DialogueTemplateMismatch { expression: owner })?;
        require_expr_family(
            modules,
            runtime_owners,
            owner,
            RuntimeSemanticFactFamily::DialogueApplication,
            |kind| {
                matches!(
                    kind,
                    HirExprKind::AttachedContentApplication(application)
                        if matches!(
                            application.family(),
                            arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::DialogueLine {
                                ..
                            }
                        )
                )
            },
        )?;
        let accepted = self
            .dialogue_lines
            .as_ref()
            .and_then(|lines| lines.for_semantic_expr(owner))
            .ok_or(RuntimeSemanticFactsError::DialogueLineMismatch { expression: owner })?;
        let accepted_runtime_line = RuntimeLineId::from_source_entity_body(accepted.id().as_str())
            .map_err(|_| RuntimeSemanticFactsError::DialogueLineMismatch { expression: owner })?;
        if &accepted_runtime_line != application.content().line()
            || accepted.text_key().as_str() != application.content().text_key().as_str()
        {
            return Err(RuntimeSemanticFactsError::DialogueLineMismatch { expression: owner });
        }
        validate_normalized_type(modules, application.line_result())?;
        if application.content().character().semantic_digest() != catalog.semantic_digest()
            || application.content().character().locale_policy_digest()
                != catalog.locale_policy_digest()
        {
            return Err(RuntimeSemanticFactsError::DialogueCharacterPlanMismatch {
                expression: owner,
            });
        }
        if let arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence::Exact(
            character,
        ) = application.content().character().target()
            && catalog.record(character).is_err()
        {
            return Err(RuntimeSemanticFactsError::DialogueCharacterPlanMismatch {
                expression: owner,
            });
        }
        for (index, value) in fragment.values().iter().enumerate() {
            let expected = RuntimeDialogueValueSlotId::from_zero_based(index).ok_or(
                RuntimeSemanticFactsError::TooManyDialogueValueSlots { expression: owner },
            )?;
            if value.slot() != expected {
                return Err(RuntimeSemanticFactsError::NonCanonicalDialogueValueSlot {
                    expression: owner,
                    expected,
                    actual: value.slot(),
                });
            }
            resolve_expr(modules, value.expression())?;
            require_runtime_expression_owner(
                runtime_owners,
                value.expression(),
                RuntimeSemanticFactFamily::DialogueApplication,
            )?;
            if self.expression_type(value.expression()) != Some(value.ty()) {
                return Err(RuntimeSemanticFactsError::MissingDialogueValueType {
                    dialogue: owner,
                    value: value.expression(),
                });
            }
        }
        for (index, effect) in fragment.effects().iter().enumerate() {
            let expected =
                arcweft_core::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                    .ok_or(RuntimeSemanticFactsError::TooManyDialogueValueSlots {
                        expression: owner,
                    })?;
            let trigger_valid = match effect.trigger() {
                RuntimeDialogueEffectTrigger::Content => true,
                RuntimeDialogueEffectTrigger::Delay {
                    duration_type,
                    schedule_handle_type,
                    ..
                } => {
                    matches!(duration_type.shape(), RuntimeTypeShape::Duration)
                        && validate_normalized_type(modules, duration_type).is_ok()
                        && validate_normalized_type(modules, schedule_handle_type).is_ok()
                }
            };
            if effect.site() != expected
                || !trigger_valid
                || !self.expression_reaches(owner, effect.operation().site_root())
                || !evaluated_effect::validate_evaluated_effect_site(modules, effect.operation())
                || !evaluated_effect::validate_evaluated_effect_operation(
                    modules,
                    &self.expression_types,
                    &self.calls,
                    effect.operation().application_site(),
                    effect.operation().effect(),
                )
            {
                return Err(RuntimeSemanticFactsError::InvalidDialogueEffectSite {
                    dialogue: owner,
                    site: effect.site(),
                });
            }
        }
        Ok(())
    }

    fn expression_reaches(&self, owner: ExprId, target: ExprId) -> bool {
        let mut pending = self.expression_children(owner).to_vec();
        let mut visited = BTreeSet::new();
        while let Some(expression) = pending.pop() {
            if expression == target {
                return true;
            }
            if visited.insert(expression) {
                pending.extend_from_slice(self.expression_children(expression));
            }
        }
        false
    }

    /// Revalidates that the facts are consumed by the exact generation that
    /// admitted them. Stable IDs surviving a reload do not make stale facts valid.
    pub fn validate_generation(
        &self,
        project: HirAnalysisProjectView<'_>,
    ) -> Result<(), RuntimeSemanticFactsError> {
        let actual = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect::<BTreeMap<_, _>>();
        if actual == self.snapshots {
            Ok(())
        } else {
            Err(RuntimeSemanticFactsError::WrongProjectGeneration)
        }
    }

    pub fn expression_literal(&self, expression: ExprId) -> Option<&RuntimeValue> {
        self.expression_literals.get(&expression)
    }

    /// Returns the sole accepted normalized type of one selected runtime-domain
    /// final-HIR expression.
    pub fn expression_type(&self, expression: ExprId) -> Option<&RuntimeNormalizedType> {
        self.expression_types.get(&expression)
    }

    /// Returns the sole accepted normalized type of one runtime-domain
    /// final-HIR pattern.
    pub fn pattern_type(&self, pattern: PatternId) -> Option<&RuntimeNormalizedType> {
        self.pattern_types.get(&pattern)
    }

    /// Sole accepted normalized semantic type of one runtime-domain final-HIR
    /// local.
    pub fn local_type(&self, local: LocalId) -> Option<&RuntimeNormalizedType> {
        self.local_declarations.get(&local)
    }

    /// Runtime-domain locals in canonical final-HIR inventory order.
    ///
    /// # Panics
    ///
    /// Panics only if accepted local-order metadata is inconsistent with the
    /// corresponding local fact map.
    pub fn local_declarations(
        &self,
    ) -> impl ExactSizeIterator<Item = (LocalId, &RuntimeNormalizedType)> {
        self.local_declaration_order.iter().map(|local| {
            (
                *local,
                self.local_declarations
                    .get(local)
                    .expect("accepted local order and fact map remain correlated"),
            )
        })
    }

    /// Complete recursive semantic type batch required by the aggregate plan
    /// builder. Duplicate identities remain in traversal order so the sole
    /// interner can reject inconsistent projections atomically.
    pub fn runtime_plan_type_seeds(
        &self,
    ) -> Result<Vec<RuntimePlanTypeSeed>, RuntimeCheckedTypeProjectionError> {
        let mut seeds = Vec::new();
        for ty in self.all_normalized_type_roots() {
            ty.append_runtime_plan_type_seeds(&mut seeds)?;
        }
        Ok(seeds)
    }

    /// Complete plan-owned nominal-record schemas. Repeated owners are
    /// retained so the sole builder can reject conflicting projections.
    pub fn runtime_plan_nominal_record_domain_seeds(&self) -> Vec<RuntimeNominalRecordDomainSeed> {
        let project = |record: &RuntimeResolvedNominalRecord| {
            RuntimeNominalRecordDomainSeed::new(
                record.nominal().identity(),
                record.fields().iter().map(|field| {
                    RuntimeNominalRecordDomainFieldSeed::new(field.name(), field.ty().identity())
                }),
            )
        };
        let mut domains = self
            .nominal_records
            .values()
            .map(|record| project(record.nominal()))
            .chain(
                self.pattern_nominal_records
                    .values()
                    .filter_map(|record| record.nominal().map(&project)),
            )
            .collect::<Vec<_>>();
        for (_, semantics) in instance_semantic_roots(
            self.project_function_instances.values(),
            self.root_closures.values(),
        ) {
            semantics.visit_catalogs(&mut |catalog| {
                domains.extend(catalog.expressions().iter().filter_map(|row| {
                    catalog
                        .nominal_record(row.owner())
                        .map(|record| project(record.nominal()))
                }));
                domains.extend(catalog.patterns().iter().filter_map(|row| {
                    catalog
                        .pattern_nominal_record(row.owner())
                        .and_then(RuntimeRecordPatternFact::nominal)
                        .map(&project)
                }));
            });
        }
        domains
    }

    /// Complete non-Option/Result variant schemas. Repeated owners remain in
    /// the batch for exact builder-level conflict validation.
    pub fn runtime_plan_variant_domain_seeds(&self) -> Vec<RuntimeVariantDomainSeed> {
        let mut domains = self
            .expression_variants
            .values()
            .chain(self.pattern_variants.values())
            .chain(
                self.calls
                    .values()
                    .filter_map(|call| match call.dispatch() {
                        RuntimeResolvedCallDispatch::Static(
                            RuntimeResolvedStaticCallTarget::Variant(variant),
                        ) => Some(variant),
                        _ => None,
                    }),
            )
            .filter_map(|variant| variant.owner().runtime_plan_domain_seed())
            .collect::<Vec<_>>();
        for (_, semantics) in instance_semantic_roots(
            self.project_function_instances.values(),
            self.root_closures.values(),
        ) {
            semantics.visit_catalogs(&mut |catalog| {
                for row in catalog.expressions() {
                    let variant = match row.payload() {
                        RuntimeProjectFunctionExpressionPayload::Variant(variant) => Some(variant),
                        RuntimeProjectFunctionExpressionPayload::Call(call) => {
                            match call.dispatch() {
                                RuntimeResolvedCallDispatch::Static(
                                    RuntimeResolvedStaticCallTarget::Variant(variant),
                                ) => Some(variant),
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    domains.extend(
                        variant.and_then(|variant| variant.owner().runtime_plan_domain_seed()),
                    );
                }
                domains.extend(catalog.patterns().iter().filter_map(|row| {
                    catalog
                        .pattern_variant(row.owner())
                        .and_then(|variant| variant.owner().runtime_plan_domain_seed())
                }));
            });
        }
        domains
    }

    fn all_normalized_type_roots(&self) -> Vec<&RuntimeNormalizedType> {
        let mut roots = Vec::new();
        roots.extend(self.local_declarations.values());
        roots.extend(self.expression_types.values());
        roots.extend(self.pattern_types.values());
        roots.extend(self.types.values());
        roots.extend(self.captures.values().map(RuntimeCheckedCapture::ty));
        for call in self.calls.values() {
            call.append_normalized_types(&mut roots);
        }
        for value in self.values.values() {
            value.append_normalized_types(&mut roots);
        }
        for instance in self.project_function_instances.values() {
            instance.append_normalized_types(&mut roots);
        }
        for closure in self.root_closures.values() {
            closure.append_normalized_types(&mut roots);
        }
        roots.extend(
            self.dialogue_applications
                .values()
                .map(RuntimeDialogueApplication::line_result),
        );
        for fragment in &self.dialogue_content_fragments {
            fragment.append_normalized_types(&mut roots);
        }
        for effect in self.evaluated_effects.values() {
            effect
                .effect()
                .visit_operand_types(&mut |ty| roots.push(ty));
        }
        for record in self.nominal_records.values() {
            record.append_normalized_types(&mut roots);
        }
        for record in self.pattern_nominal_records.values() {
            record.append_normalized_types(&mut roots);
        }
        for assignment in self.assignments.values() {
            roots.extend([assignment.field_type(), assignment.value_type()]);
        }
        for tried in self.tries.values() {
            tried.append_normalized_types(&mut roots);
        }
        for callable in self.implicit_callables.values() {
            roots.extend([callable.parameter(), callable.result()]);
        }
        roots.extend(
            self.trait_methods
                .values()
                .map(RuntimeTraitMethodFact::self_type),
        );
        for iteration in self.iterations.values() {
            iteration.append_normalized_types(&mut roots);
        }
        for variant in self
            .expression_variants
            .values()
            .chain(self.pattern_variants.values())
        {
            variant.owner().append_normalized_types(&mut roots);
        }
        roots
    }

    /// Compiler-admitted identity and closed effects for one exact final-HIR Flow item.
    pub fn flow(&self, item: ItemId) -> Option<&RuntimeFlowFact> {
        self.flows.get(&item)
    }

    pub fn pattern_literal(&self, pattern: PatternId) -> Option<&RuntimeValue> {
        self.pattern_literals.get(&pattern)
    }

    pub fn pattern_item(&self, pattern: PatternId) -> Option<&RuntimeProjectItem> {
        self.pattern_items.get(&pattern)
    }

    pub fn value(&self, expression: ExprId) -> Option<&RuntimeResolvedValue> {
        self.values.get(&expression)
    }

    pub fn select(&self, expression: ExprId) -> Option<&RuntimeResolvedSelect> {
        self.selects.get(&expression)
    }

    pub fn nominal_record(&self, expression: ExprId) -> Option<&RuntimeRecordExpressionFact> {
        self.nominal_records.get(&expression)
    }

    pub fn pattern_nominal_record(&self, pattern: PatternId) -> Option<&RuntimeRecordPatternFact> {
        self.pattern_nominal_records.get(&pattern)
    }

    pub fn expression_variant(&self, expression: ExprId) -> Option<&RuntimeResolvedVariant> {
        self.expression_variants.get(&expression)
    }

    pub fn pattern_variant(&self, pattern: PatternId) -> Option<&RuntimeResolvedVariant> {
        self.pattern_variants.get(&pattern)
    }

    pub fn ty(&self, ty: TypeId) -> Option<&RuntimeNormalizedType> {
        self.types.get(&ty)
    }

    pub fn call(&self, expression: ExprId) -> Option<&RuntimeResolvedCall> {
        self.calls.get(&expression)
    }

    /// Iterates accepted runtime call facts in canonical expression identity order.
    pub fn calls(&self) -> impl ExactSizeIterator<Item = (ExprId, &RuntimeResolvedCall)> {
        self.calls.iter().map(|(owner, call)| (*owner, call))
    }

    pub fn project_function_instance(
        &self,
        key: &RuntimeProjectFunctionInstanceKey,
    ) -> Option<&RuntimeProjectFunctionInstanceFact> {
        self.project_function_instances.get(key)
    }

    pub fn project_function_instances(
        &self,
    ) -> impl ExactSizeIterator<Item = &RuntimeProjectFunctionInstanceFact> {
        self.project_function_instances.values()
    }

    pub fn root_closure(&self, owner: ExprId) -> Option<&RuntimeClosureInstanceFact> {
        self.root_closures.get(&owner)
    }

    pub fn root_closures(&self) -> impl ExactSizeIterator<Item = &RuntimeClosureInstanceFact> {
        self.root_closures.values()
    }

    pub fn project_function_roots(
        &self,
    ) -> impl ExactSizeIterator<Item = &RuntimeProjectFunctionRootFact> {
        self.project_function_roots.values()
    }

    /// Returns the checked non-call ingress for one runtime callable. Multiple
    /// Entry declarations may share the same row, but admission guarantees
    /// that they all name the same closed instance key.
    pub fn project_function_root_for_callable(
        &self,
        callable: &RuntimeCallableId,
    ) -> Option<&RuntimeProjectFunctionRootFact> {
        self.project_function_roots
            .values()
            .find(|root| root.instance().callable() == callable)
    }

    /// Returns the sole checked candidate selected for one postfix root.
    pub fn postfix_candidate(&self, expression: ExprId) -> Option<ExprId> {
        self.postfix_candidates.get(&expression).copied()
    }

    /// Returns the accepted owning children retained for runtime lowering.
    pub fn expression_children(&self, expression: ExprId) -> &[ExprId] {
        self.expression_children
            .get(&expression)
            .map_or(&[], Box::as_ref)
    }

    pub fn iteration(&self, statement: StmtId) -> Option<&RuntimeIteratorFact> {
        self.iterations.get(&statement)
    }

    pub fn trait_methods(&self) -> impl ExactSizeIterator<Item = &RuntimeTraitMethodFact> {
        self.trait_methods.values()
    }

    pub fn assertion(&self, statement: StmtId) -> Option<RuntimeAssertionAdmission> {
        self.assertions.get(&statement).copied()
    }

    pub(crate) fn trigger(&self, statement: StmtId) -> Option<&RuntimeTriggerAdmission> {
        self.triggers.get(&statement)
    }

    /// Returns the sole compiler-admitted writable place for an assignment.
    pub fn assignment(&self, statement: StmtId) -> Option<&RuntimeAssignmentFact> {
        self.assignments.get(&statement)
    }

    pub fn evaluated_effect(&self, statement: StmtId) -> Option<&RuntimeEvaluatedEffectFact> {
        self.evaluated_effects.get(&statement)
    }

    pub fn awaited(&self, expression: ExprId) -> Option<&RuntimeAwaitFact> {
        self.awaits.get(&expression)
    }

    pub fn choice(&self, expression: ExprId) -> Option<&RuntimeChoiceFact> {
        self.choices.get(&expression)
    }

    pub fn awaits(&self) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimeAwaitFact)> {
        self.awaits.iter()
    }

    pub fn tried(&self, expression: ExprId) -> Option<&RuntimeTryFact> {
        self.tries.get(&expression)
    }

    pub fn tries(&self) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimeTryFact)> {
        self.tries.iter()
    }

    pub fn implicit_callable(&self, expression: ExprId) -> Option<&RuntimeImplicitCallableFact> {
        self.implicit_callables.get(&expression)
    }

    pub fn implicit_callables(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimeImplicitCallableFact)> {
        self.implicit_callables.iter()
    }

    pub fn pipe(&self, expression: ExprId) -> Option<&RuntimePipeFact> {
        self.pipes.get(&expression)
    }

    pub fn pipes(&self) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimePipeFact)> {
        self.pipes.iter()
    }

    pub fn capture(&self, capture: CaptureId) -> Option<&RuntimeCheckedCapture> {
        self.captures.get(&capture)
    }

    pub fn pure_programs(
        &self,
    ) -> impl ExactSizeIterator<Item = (&RuntimePureProgramId, &RuntimePureProgramFact)> {
        self.pure_programs.iter()
    }

    pub fn is_pure_program_closure(&self, closure: ExprId) -> bool {
        self.pure_programs
            .values()
            .any(|program| program.closure() == closure)
    }

    pub const fn view_value_reachability(&self) -> Option<&HirRuntimeReachabilityIdentity> {
        self.view_value_reachability.as_ref()
    }

    pub fn dialogue_application(&self, expression: ExprId) -> Option<&RuntimeDialogueApplication> {
        self.dialogue_applications.get(&expression)
    }

    pub fn dialogue_applications(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimeDialogueApplication)> {
        self.dialogue_applications.iter()
    }

    /// Visits every global or closed-instance dialogue application together
    /// with its exact lexical semantic scope and catalog.
    pub fn visit_dialogue_applications<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            ExprId,
            &'facts RuntimeDialogueApplication,
        ),
    ) {
        let global = RuntimeScopedExecutableSemanticFactView::global(self);
        for (owner, application) in &self.dialogue_applications {
            visitor(global, *owner, application);
        }
        for (scope, semantics) in instance_semantic_roots(
            self.project_function_instances.values(),
            self.root_closures.values(),
        ) {
            semantics.visit_dialogue_applications(scope, visitor);
        }
    }

    /// Generation-global immutable template rows. Use
    /// [`Self::visit_dialogue_content_fragments`] for the complete plan-wide
    /// catalog, including closed project instances and nested closures.
    pub fn dialogue_content_fragments(&self) -> &[RuntimeContentFragmentFact] {
        &self.dialogue_content_fragments
    }

    pub fn visit_dialogue_content_fragments<'facts>(
        &'facts self,
        visitor: &mut impl FnMut(
            RuntimeScopedExecutableSemanticFactView<'facts>,
            &'facts RuntimeContentFragmentFact,
        ),
    ) {
        let global = RuntimeScopedExecutableSemanticFactView::global(self);
        for fragment in &self.dialogue_content_fragments {
            visitor(global, fragment);
        }
        for (scope, semantics) in instance_semantic_roots(
            self.project_function_instances.values(),
            self.root_closures.values(),
        ) {
            semantics.visit_content_fragments(scope, visitor);
        }
    }

    pub fn dialogue_content_fragment(
        &self,
        template: arcweft_core::runtime_id::RuntimeDialogueContentTemplateId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.dialogue_content_fragments
            .iter()
            .find(|fragment| fragment.template().id() == template)
            .or_else(|| {
                instance_semantic_roots(
                    self.project_function_instances.values(),
                    self.root_closures.values(),
                )
                .find_map(|(_, semantics)| semantics.dialogue_content_fragment(template))
            })
    }

    pub fn dialogue_content_fragment_for_source(
        &self,
        source: ExprId,
    ) -> Option<&RuntimeContentFragmentFact> {
        self.dialogue_content_fragments
            .iter()
            .find(|fragment| fragment.source() == source)
    }

    /// Accepted line identities supplied by the final semantic analysis.
    pub fn dialogue_lines(&self) -> Option<&AcceptedDialogueLineInventory> {
        self.dialogue_lines.as_deref()
    }

    pub const fn character_presentation_catalog(
        &self,
    ) -> Option<&Arc<CharacterPresentationCatalogData>> {
        self.character_presentation_catalog.as_ref()
    }
}

fn validate_pure_programs(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owners: RuntimeSemanticOwnerSet<'_>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    local_declarations: &BTreeMap<LocalId, RuntimeNormalizedType>,
    checked_captures: &BTreeMap<CaptureId, RuntimeCheckedCapture>,
    staged: Vec<RuntimePureProgramFact>,
) -> Result<BTreeMap<RuntimePureProgramId, RuntimePureProgramFact>, RuntimeSemanticFactsError> {
    let expected_closures = owners
        .view_values
        .into_iter()
        .flat_map(HirRuntimeSemanticReachability::roots)
        .filter_map(|root| match root.owner() {
            HirRuntimeExecutableOwner::Closure(closure)
                if root.kind() == HirRuntimeReachabilityRootKind::CheckedViewValueProgram =>
            {
                Some(*closure)
            }
            HirRuntimeExecutableOwner::Item(_)
            | HirRuntimeExecutableOwner::ImplMethod(_)
            | HirRuntimeExecutableOwner::Closure(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let mut programs = BTreeMap::new();
    let mut closures = BTreeSet::new();
    for fact in staged {
        let program = fact.program();
        if programs.contains_key(&program) {
            return Err(RuntimeSemanticFactsError::DuplicateFact {
                family: RuntimeSemanticFactFamily::PureProgram,
            });
        }
        if !expected_closures.contains(&fact.closure()) || !closures.insert(fact.closure()) {
            return Err(RuntimeSemanticFactsError::InvalidPureProgram { program });
        }
        let HirExprKind::Closure(closure) = resolve_expr(modules, fact.closure())? else {
            return Err(RuntimeSemanticFactsError::InvalidPureProgram { program });
        };
        let expected_captures = owners
            .executable_owners(&HirRuntimeExecutableOwner::Closure(fact.closure()))
            .ok_or(RuntimeSemanticFactsError::InvalidPureProgram { program })?
            .capture_plan();
        let Some(RuntimeTypeShape::Function { parameters, result }) = expression_types
            .get(&fact.closure())
            .map(RuntimeNormalizedType::shape)
        else {
            return Err(RuntimeSemanticFactsError::InvalidPureProgram { program });
        };
        if !parameters.is_empty()
            || !closure.parameters().is_empty()
            || closure.body() != fact.body()
            || result.identity() != fact.result()
            || expression_types
                .get(&fact.body())
                .is_none_or(|body| body.identity() != fact.result())
            || expected_captures.len() != fact.captures().len()
        {
            return Err(RuntimeSemanticFactsError::InvalidPureProgram { program });
        }
        let module = module_for(modules, fact.closure().module())?;
        let mut parameters = BTreeSet::new();
        let mut locals = BTreeSet::new();
        for (expected_capture, capture) in expected_captures
            .iter()
            .copied()
            .zip(fact.captures().iter().copied())
        {
            let hir_capture = module.resolve_capture(capture.capture()).map_err(|_| {
                RuntimeSemanticFactsError::UnresolvedCapture {
                    capture: capture.capture(),
                }
            })?;
            let checked = checked_captures.get(&capture.capture());
            let local_type = local_declarations.get(&capture.local());
            if expected_capture.capture() != capture.capture()
                || hir_capture.closure() != fact.closure()
                || hir_capture.local() != capture.local()
                || expected_capture.local() != capture.local()
                || expected_capture.mode() != CaptureAccess::Read
                || !parameters.insert(capture.parameter())
                || !locals.insert(capture.local())
                || checked.is_none_or(|checked| {
                    checked.capture() != capture.capture()
                        || checked.ty().identity() != capture.value_type()
                })
                || local_type.is_none_or(|ty| ty.identity() != capture.value_type())
            {
                return Err(RuntimeSemanticFactsError::InvalidPureProgram { program });
            }
        }
        programs.insert(program, fact);
    }
    if let Some(closure) = expected_closures
        .iter()
        .find(|closure| !closures.contains(closure))
    {
        return Err(RuntimeSemanticFactsError::MissingPureProgram { closure: *closure });
    }
    Ok(programs)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSemanticFactsError {
    #[error("runtime capture {capture:?} differs from its selected lexical projection")]
    InvalidCaptureProjection { capture: CaptureId },
    #[error("runtime semantic facts and reachability belong to different generations")]
    ReachabilityMismatch,
    #[error("runtime semantic fact owner is outside the accepted reachability closure")]
    OwnerOutsideReachability { owner: HirRuntimeExecutableOwner },
    #[error("runtime semantic facts are bound to a different accepted HIR generation")]
    WrongProjectGeneration,
    #[error("runtime semantic facts contain more than one {family:?} fact for the same HIR ID")]
    DuplicateFact { family: RuntimeSemanticFactFamily },
    #[error("runtime pure program {program} does not match its checked View closure root")]
    InvalidPureProgram { program: RuntimePureProgramId },
    #[error("checked View closure root {closure:?} has no exact runtime pure-program fact")]
    MissingPureProgram { closure: ExprId },
    #[error("accepted runtime semantic facts omit expression type {expression:?}")]
    MissingExpressionType { expression: ExprId },
    #[error(
        "accepted runtime semantic facts contain a {family:?} fact for inactive expression {expression:?}"
    )]
    InactiveExpressionFact {
        expression: ExprId,
        family: RuntimeSemanticFactFamily,
    },
    #[error("accepted runtime semantic facts omit pattern type {pattern:?}")]
    MissingPatternType { pattern: PatternId },
    #[error(
        "accepted runtime semantic facts contain a {family:?} fact for inactive pattern {pattern:?}"
    )]
    InactivePatternFact {
        pattern: PatternId,
        family: RuntimeSemanticFactFamily,
    },
    #[error("accepted runtime semantic facts omit an assignment fact for {statement:?}")]
    MissingAssignmentFact { statement: StmtId },
    #[error("accepted runtime semantic facts omit a Trigger fact for {statement:?}")]
    MissingTriggerFact { statement: StmtId },
    #[error("Trigger fact for {statement:?} does not match its checked runtime authority")]
    InvalidTriggerFact { statement: StmtId },
    #[error("assignment fact for {statement:?} does not match its checked direct record field")]
    InvalidAssignmentFact { statement: StmtId },
    #[error("evaluated-effect fact for {statement:?} does not match its selected call")]
    InvalidEvaluatedEffectFact { statement: StmtId },
    #[error("Await fact for {expression:?} does not match its checked expression")]
    InvalidAwaitFact { expression: ExprId },
    #[error("Choice fact for {expression:?} does not match its checked expression")]
    InvalidChoiceFact { expression: ExprId },
    #[error("Try fact for {expression:?} does not match its checked carrier boundary")]
    InvalidTryFact { expression: ExprId },
    #[error("implicit callable fact for {expression:?} does not match its checked abstraction")]
    InvalidImplicitCallableFact { expression: ExprId },
    #[error("pipe fact for {expression:?} does not match its checked once-only pipeline")]
    InvalidPipeFact { expression: ExprId },
    #[error("postfix expression {expression:?} has no accepted candidate fact")]
    MissingPostfixCandidate { expression: ExprId },
    #[error("runtime semantic fact references unknown HIR module {module:?}")]
    UnknownModule { module: HirModuleId },
    #[error("runtime semantic fact references unresolved item {item:?}")]
    UnresolvedItem { item: ItemId },
    #[error("runtime semantic fact references unresolved local {local:?}")]
    UnresolvedLocal { local: LocalId },
    #[error("runtime semantic fact references presentation-owned local {local:?}")]
    InactiveLocalReference { local: LocalId },
    #[error("accepted runtime semantic facts omit runtime-domain local declaration {local:?}")]
    MissingLocalDeclaration { local: LocalId },
    #[error("accepted runtime semantic facts contain extra local declaration {local:?}")]
    ExtraLocalDeclaration { local: LocalId },
    #[error(
        "runtime local declarations are not in canonical project order: expected {expected:?}, observed {actual:?}"
    )]
    NonCanonicalLocalDeclarationOrder { expected: LocalId, actual: LocalId },
    #[error("runtime semantic fact references unresolved expression {expression:?}")]
    UnresolvedExpression { expression: ExprId },
    #[error("accepted runtime semantic facts omit a complete closure instance for {expression:?}")]
    MissingClosureInstance { expression: ExprId },
    #[error("runtime semantic fact references unresolved statement {statement:?}")]
    UnresolvedStatement { statement: StmtId },
    #[error(
        "accepted runtime semantic facts contain a {family:?} fact for inactive statement {statement:?}"
    )]
    InactiveStatementFact {
        statement: StmtId,
        family: RuntimeSemanticFactFamily,
    },
    #[error("runtime semantic fact references unresolved pattern {pattern:?}")]
    UnresolvedPattern { pattern: PatternId },
    #[error("runtime semantic fact references unresolved type {ty:?}")]
    UnresolvedType { ty: TypeId },
    #[error("accepted runtime semantic facts contain a fact for inactive type {ty:?}")]
    InactiveTypeFact { ty: TypeId },
    #[error("runtime semantic fact references poisoned type {ty:?}")]
    PoisonedType { ty: TypeId },
    #[error("runtime semantic fact references unresolved capture {capture:?}")]
    UnresolvedCapture { capture: CaptureId },
    #[error("accepted runtime semantic facts contain a fact for inactive capture {capture:?}")]
    InactiveCaptureFact { capture: CaptureId },
    #[error(transparent)]
    RuntimeReachability(#[from] HirRuntimeReachabilityError),
    #[error("expression {expression:?} cannot own a {expected:?} runtime semantic fact")]
    WrongExpressionFamily {
        expression: ExprId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("statement {statement:?} cannot own a {expected:?} runtime semantic fact")]
    WrongStatementFamily {
        statement: StmtId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("pattern {pattern:?} cannot own a {expected:?} runtime semantic fact")]
    WrongPatternFamily {
        pattern: PatternId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("runtime semantic fact item {item:?} has incompatible HIR family {actual:?}")]
    WrongItemFamily { item: ItemId, actual: HirItemFamily },
    #[error("runtime project callable does not match its exact final-HIR source owner")]
    InvalidCallableSourceOwner,
    #[error("runtime project callable attached-content ABI does not match final HIR")]
    InvalidCallableAttachedContentAbi,
    #[error("runtime project-function instance does not match its exact final-HIR function body")]
    InvalidProjectFunctionInstance,
    #[error("runtime project-function call {expression:?} has no exact closed instance fact")]
    MissingProjectFunctionInstance { expression: ExprId },
    #[error(
        "runtime project-function instance is not referenced by any checked Invoke outcome or non-call root"
    )]
    UnreferencedProjectFunctionInstance,
    #[error("runtime project-function non-call root does not match its checked HIR ingress")]
    InvalidProjectFunctionRoot,
    #[error("ordinary project-function call is missing its checked continuation/instance plan")]
    MissingProjectFunctionCallPlan,
    #[error("closed project-function instance owner was also published in global {family:?} facts")]
    InstanceOwnedGlobalFact { family: RuntimeSemanticFactFamily },
    #[error("runtime nominal-record fact item {item:?} is not a struct")]
    WrongNominalRecordItemFamily { item: ItemId },
    #[error("runtime nominal-record layout catalog contains conflicting descriptors")]
    ConflictingNominalRecordLayout {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    #[error("runtime nominal layout for `{nominal:?}` on {item:?} is unresolved")]
    UnresolvedNominalLayout {
        item: ItemId,
        nominal: RuntimeNominalTypeId,
    },
    #[error("runtime variant fact does not match its typed owner and source-ordered ordinal")]
    WrongVariantIdentity,
    #[error("runtime project item does not match its typed owner and public ID")]
    WrongProjectItemIdentity,
    #[error(
        "runtime call fact references authored argument {ordinal}, but the call has {count} arguments"
    )]
    InvalidCallArgumentOrdinal { ordinal: u32, count: usize },
    #[error("runtime call fact repeats one argument projection")]
    DuplicateCallArgument,
    #[error("runtime function-value call fact is attached to a call without a value callee")]
    MissingFunctionValueCallee,
    #[error(
        "Reduction.unchanged runtime call requires exactly one authored argument and a value result"
    )]
    InvalidReductionConstructorCall,
    #[error("Agent runtime call arguments do not match the selected intrinsic family")]
    InvalidAgentCallArguments,
    #[error("postfix expression {expression:?} does not own selected candidate {candidate:?}")]
    WrongPostfixCandidate {
        expression: ExprId,
        candidate: ExprId,
    },
    #[error("expression {expression:?} has a runtime call disposition but is not a Call")]
    InvalidRuntimeCallDisposition { expression: ExprId },
    #[error("selected runtime call {expression:?} requires a runtime receiver but has none")]
    MissingRuntimeCallReceiver { expression: ExprId },
    #[error("runtime trait method fact does not match its final-HIR implementation member")]
    InvalidTraitMethodIdentity,
    #[error("iterator witness method edge does not match its checked statement role")]
    InvalidIteratorWitnessMethodEdge {
        statement: StmtId,
        role: HirRuntimeIteratorWitnessMethodRole,
    },
    #[error("dialogue projection and Character presentation catalog presence disagree")]
    DialogueCatalogPresenceMismatch,
    #[error("dialogue application {expression:?} does not match its accepted line identity")]
    DialogueLineMismatch { expression: ExprId },
    #[error("dialogue application {expression:?} has no matching runtime content fragment")]
    DialogueTemplateMismatch { expression: ExprId },
    #[error("runtime content fragment source {expression:?} is duplicated")]
    DuplicateContentFragment { expression: ExprId },
    #[error("runtime content fragment source {expression:?} does not match its checked programs")]
    InvalidContentFragment { expression: ExprId },
    #[error("dialogue application {expression:?} has too many value slots")]
    TooManyDialogueValueSlots { expression: ExprId },
    #[error(
        "dialogue application {expression:?} has non-canonical value slot {actual:?}, expected {expected:?}"
    )]
    NonCanonicalDialogueValueSlot {
        expression: ExprId,
        expected: RuntimeDialogueValueSlotId,
        actual: RuntimeDialogueValueSlotId,
    },
    #[error("dialogue {dialogue:?} value expression {value:?} has no accepted type")]
    MissingDialogueValueType { dialogue: ExprId, value: ExprId },
    #[error("dialogue {dialogue:?} effect site {site:?} does not match its checked operation")]
    InvalidDialogueEffectSite {
        dialogue: ExprId,
        site: arcweft_core::runtime_id::RuntimeDialogueEffectSiteId,
    },
    #[error("dialogue application {expression:?} carries stale or unknown Character evidence")]
    DialogueCharacterPlanMismatch { expression: ExprId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSemanticFactFamily {
    LocalDeclaration,
    FlowIdentity,
    ExpressionType,
    ExpressionChildren,
    PatternType,
    ExpressionLiteral,
    PatternLiteral,
    PatternItem,
    Value,
    Select,
    NominalRecord,
    PatternNominalRecord,
    ExpressionVariant,
    PatternVariant,
    Type,
    Call,
    PostfixCandidate,
    TraitMethod,
    Iteration,
    Assertion,
    Trigger,
    Assignment,
    EvaluatedEffect,
    Choice,
    Await,
    Try,
    ImplicitCallable,
    Pipe,
    Capture,
    PureProgram,
    ProjectFunctionInstance,
    ClosureInstance,
    ProjectFunctionRoot,
    DialogueApplication,
}

fn validate_complete_expression_types(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    instance_owned: &BTreeSet<ExprId>,
) -> Result<(), RuntimeSemanticFactsError> {
    let mut accepted = runtime_owners.selected_expression_type_owners()?;
    accepted.retain(|owner| !instance_owned.contains(owner));

    if let Some(expression) = accepted
        .iter()
        .find(|owner| !expression_types.contains_key(owner))
    {
        return Err(RuntimeSemanticFactsError::MissingExpressionType {
            expression: *expression,
        });
    }
    if let Some(expression) = expression_types
        .keys()
        .find(|owner| !accepted.contains(owner))
    {
        return Err(RuntimeSemanticFactsError::InactiveExpressionFact {
            expression: *expression,
            family: RuntimeSemanticFactFamily::ExpressionType,
        });
    }
    Ok(())
}

/// Final semantic publication owns a fact for every runtime-domain final-HIR
/// pattern, including patterns retained inside bounded candidate HIR. If
/// candidate rollback leaves one without a type, semantic analysis fails
/// before this projection can be constructed. Presentation-owned patterns do
/// not enter this table.
fn validate_complete_pattern_types(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    pattern_types: &BTreeMap<PatternId, RuntimeNormalizedType>,
    instance_owned: &BTreeSet<PatternId>,
) -> Result<(), RuntimeSemanticFactsError> {
    for pattern in runtime_owners
        .patterns()
        .into_iter()
        .filter(|pattern| !instance_owned.contains(pattern))
    {
        if !pattern_types.contains_key(&pattern) {
            return Err(RuntimeSemanticFactsError::MissingPatternType { pattern });
        }
    }
    if let Some(pattern) = pattern_types
        .keys()
        .find(|owner| !runtime_owners.contains_pattern(**owner) || instance_owned.contains(*owner))
    {
        return Err(RuntimeSemanticFactsError::InactivePatternFact {
            pattern: *pattern,
            family: RuntimeSemanticFactFamily::PatternType,
        });
    }
    Ok(())
}

fn require_runtime_expression_owner(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    expression: ExprId,
    family: RuntimeSemanticFactFamily,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_expression(expression) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactiveExpressionFact { expression, family })
    }
}

fn require_runtime_statement_owner(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    statement: StmtId,
    family: RuntimeSemanticFactFamily,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_statement(statement) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactiveStatementFact { statement, family })
    }
}

fn require_runtime_pattern_owner(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    pattern: PatternId,
    family: RuntimeSemanticFactFamily,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_pattern(pattern) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactivePatternFact { pattern, family })
    }
}

fn require_runtime_type_owner(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    ty: TypeId,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_type(ty) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactiveTypeFact { ty })
    }
}

fn require_runtime_capture_owner(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    capture: CaptureId,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_capture(capture) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactiveCaptureFact { capture })
    }
}

fn require_runtime_local_reference(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    local: LocalId,
) -> Result<(), RuntimeSemanticFactsError> {
    if runtime_owners.contains_local(local) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::InactiveLocalReference { local })
    }
}

fn collect_unique<K: Ord, V>(
    values: impl IntoIterator<Item = (K, V)>,
    family: RuntimeSemanticFactFamily,
) -> Result<BTreeMap<K, V>, RuntimeSemanticFactsError> {
    let mut result = BTreeMap::new();
    for (key, value) in values {
        if result.insert(key, value).is_some() {
            return Err(RuntimeSemanticFactsError::DuplicateFact { family });
        }
    }
    Ok(result)
}

fn all_unique<T: Copy + Ord>(values: &[T]) -> bool {
    values.iter().copied().collect::<BTreeSet<_>>().len() == values.len()
}

fn module_for<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: HirModuleId,
) -> Result<&'project HirModule, RuntimeSemanticFactsError> {
    modules
        .get(&id)
        .copied()
        .ok_or(RuntimeSemanticFactsError::UnknownModule { module: id })
}

fn resolve_expr<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: ExprId,
) -> Result<&'project HirExprKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_expr(id)
        .map(arcweft_lang_hir::expr::HirExpr::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedExpression { expression: id })
}

fn resolve_stmt<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: StmtId,
) -> Result<&'project HirStmtKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_stmt(id)
        .map(arcweft_lang_hir::stmt::HirStmt::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedStatement { statement: id })
}

fn resolve_pattern<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: PatternId,
) -> Result<&'project HirPatternKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_pattern(id)
        .map(arcweft_lang_hir::pattern::HirPattern::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedPattern { pattern: id })
}

fn require_expr_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    expression: ExprId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirExprKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    let kind = resolve_expr(modules, expression)?;
    require_runtime_expression_owner(runtime_owners, expression, expected)?;
    if predicate(kind) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongExpressionFamily {
            expression,
            expected,
        })
    }
}

fn require_stmt_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    statement: StmtId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirStmtKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    let kind = resolve_stmt(modules, statement)?;
    require_runtime_statement_owner(runtime_owners, statement, expected)?;
    if predicate(kind) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongStatementFamily {
            statement,
            expected,
        })
    }
}

fn require_pattern_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    pattern: PatternId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirPatternKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    let kind = resolve_pattern(modules, pattern)?;
    require_runtime_pattern_owner(runtime_owners, pattern, expected)?;
    if predicate(kind) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongPatternFamily { pattern, expected })
    }
}

fn validate_resolved_value(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    value: &RuntimeResolvedValue,
) -> Result<(), RuntimeSemanticFactsError> {
    match value {
        RuntimeResolvedValue::Local(local) => {
            module_for(modules, local.module())?
                .resolve_local(*local)
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedLocal { local: *local })?;
            require_runtime_local_reference(runtime_owners, *local)
        }
        RuntimeResolvedValue::ProjectCallable(callable) => validate_callable(modules, callable),
        RuntimeResolvedValue::ProjectItem(item) => validate_project_item(modules, item),
        RuntimeResolvedValue::DialogueLine(_)
        | RuntimeResolvedValue::CharacterLook { .. }
        | RuntimeResolvedValue::Intrinsic(_)
        | RuntimeResolvedValue::Registered(_)
        | RuntimeResolvedValue::Constant(_) => Ok(()),
    }
}

fn validate_project_item(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    item: &RuntimeProjectItem,
) -> Result<(), RuntimeSemanticFactsError> {
    item.family()
        .validate_public_id(item.public_id())
        .map_err(|_| RuntimeSemanticFactsError::WrongProjectItemIdentity)?;
    match item.owner() {
        RuntimeProjectItemOwner::ExternalCharacter => (item.family()
            == DeclarationIdentityFamily::Character)
            .then_some(())
            .ok_or(RuntimeSemanticFactsError::WrongProjectItemIdentity),
        RuntimeProjectItemOwner::StructuralFlow { owner, .. } => (item.family()
            == DeclarationIdentityFamily::Flow
            && matches!(resolve_item(modules, *owner)?.kind(), HirItemKind::Flow(_)))
        .then_some(())
        .ok_or(RuntimeSemanticFactsError::WrongProjectItemIdentity),
        RuntimeProjectItemOwner::Retained(owner) => {
            let actual = resolve_item(modules, *owner)?.kind().family();
            let expected = match item.family() {
                DeclarationIdentityFamily::Character => HirItemFamily::Character,
                DeclarationIdentityFamily::View => HirItemFamily::View,
                DeclarationIdentityFamily::Action => HirItemFamily::Action,
                DeclarationIdentityFamily::Activity => HirItemFamily::Activity,
                DeclarationIdentityFamily::Signal => HirItemFamily::Signal,
                DeclarationIdentityFamily::Metric => HirItemFamily::Metric,
                DeclarationIdentityFamily::Layer => HirItemFamily::Layer,
                DeclarationIdentityFamily::Asset
                | DeclarationIdentityFamily::Flow
                | DeclarationIdentityFamily::Proof
                | DeclarationIdentityFamily::Style => {
                    return Err(RuntimeSemanticFactsError::WrongProjectItemIdentity);
                }
            };
            if actual == expected {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: *owner,
                    actual,
                })
            }
        }
    }
}

fn validate_assignment(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    local_declarations: &BTreeMap<LocalId, RuntimeNormalizedType>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    values: &BTreeMap<ExprId, RuntimeResolvedValue>,
    selects: &BTreeMap<ExprId, RuntimeResolvedSelect>,
    statement: StmtId,
    assignment: &RuntimeAssignmentFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let HirStmtKind::Assign { target, value } = resolve_stmt(modules, statement)? else {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    };
    let HirExprKind::Select(select) = resolve_expr(modules, *target)? else {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    };
    if !matches!(
        resolve_expr(modules, select.target())?,
        HirExprKind::Path(_)
    ) || values.get(&select.target()) != Some(&RuntimeResolvedValue::Local(assignment.base()))
    {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    }
    let RuntimeResolvedSelect::Field { owner, field } = selects
        .get(target)
        .ok_or(RuntimeSemanticFactsError::InvalidAssignmentFact { statement })?
    else {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    };
    if *owner != assignment.nominal().identity()
        || *field != assignment.field()
        || expression_types.get(target) != Some(assignment.field_type())
        || expression_types.get(value) != Some(assignment.value_type())
        || assignment.field_type() != assignment.value_type()
    {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    }
    let local = local_declarations
        .get(&assignment.base())
        .ok_or(RuntimeSemanticFactsError::InvalidAssignmentFact { statement })?;
    if local.checked_type().ok().as_ref() != Some(&assignment.nominal().checked_type()) {
        return Err(RuntimeSemanticFactsError::InvalidAssignmentFact { statement });
    }
    validate_nominal(modules, assignment.nominal())?;
    validate_normalized_type(modules, assignment.field_type())?;
    validate_normalized_type(modules, assignment.value_type())?;
    Ok(())
}

fn validate_select(
    _modules: &BTreeMap<HirModuleId, &HirModule>,
    select: &RuntimeResolvedSelect,
) -> Result<(), RuntimeSemanticFactsError> {
    match select {
        RuntimeResolvedSelect::Method
        | RuntimeResolvedSelect::AgentField { .. }
        | RuntimeResolvedSelect::ProgressField { .. }
        | RuntimeResolvedSelect::Field { .. }
        | RuntimeResolvedSelect::OpaqueRecord { .. } => Ok(()),
    }
}

fn validate_variant(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    variant: &RuntimeResolvedVariant,
) -> Result<(), RuntimeSemanticFactsError> {
    variant
        .checked_selection()
        .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)?;
    let selected_name = variant
        .selected_name()
        .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)?;
    match variant.owner() {
        RuntimeVariantOwner::Project {
            nominal,
            arguments,
            cases,
        } => {
            validate_nominal(modules, nominal)?;
            for argument in arguments {
                validate_normalized_type(modules, argument)?;
            }
            validate_normalized_variant_payloads(modules, cases)?;
            let HirItemKind::Enum(declaration) = resolve_item(modules, nominal.owner())?.kind()
            else {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            };
            if declaration.variants().len() != cases.len()
                || declaration.variants().iter().zip(cases.iter()).any(
                    |(declaration, normalized)| {
                        declaration.name().resolved().map(HirName::as_str)
                            != Some(normalized.name())
                            || declaration.payload().is_some() != normalized.payload().is_some()
                    },
                )
            {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            }
            let selected = usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| declaration.variants().get(ordinal))
                .and_then(|selected| selected.name().resolved())
                .ok_or(RuntimeSemanticFactsError::WrongVariantIdentity)?;
            if selected.as_str() == selected_name {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeVariantOwner::CharacterNominal { nominal, cases, .. } => {
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name().is_empty()
                        || case.payload().is_some()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name() == case.name())
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name() != selected_name)
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::BuiltinClosed { nominal, cases, .. } => {
            validate_normalized_variant_payloads(modules, cases)?;
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name().is_empty()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name() == case.name())
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name() != selected_name)
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::RuntimeBuiltin { owner, cases, .. } => {
            validate_normalized_variant_payloads(modules, cases)?;
            if cases.len() != owner.cases().len()
                || cases.iter().zip(owner.cases()).any(|(case, schema)| {
                    case.name() != schema.name() || case.payload().is_some() != schema.has_payload()
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name() != selected_name)
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::Option { item, cases, .. } => {
            validate_normalized_type(modules, item)?;
            validate_normalized_variant_payloads(modules, cases)?;
            if matches!(
                (variant.ordinal(), selected_name),
                (0, "Some") | (1, "None")
            ) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeVariantOwner::Result {
            ok, error, cases, ..
        } => {
            validate_normalized_type(modules, ok)?;
            validate_normalized_type(modules, error)?;
            validate_normalized_variant_payloads(modules, cases)?;
            if matches!((variant.ordinal(), selected_name), (0, "Ok") | (1, "Err")) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
    }
}

fn validate_normalized_variant_payloads(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    cases: &[RuntimeNormalizedVariantCase],
) -> Result<(), RuntimeSemanticFactsError> {
    for case in cases {
        if let Some(payload) = case.payload() {
            validate_normalized_type(modules, payload)?;
        }
    }
    Ok(())
}

fn validate_normalized_type(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    ty: &RuntimeNormalizedType,
) -> Result<(), RuntimeSemanticFactsError> {
    match ty.shape() {
        RuntimeTypeShape::Range(item)
        | RuntimeTypeShape::Iterator(item)
        | RuntimeTypeShape::Sequence { item, .. }
        | RuntimeTypeShape::Array { item, .. }
        | RuntimeTypeShape::ThreadHandle(item)
        | RuntimeTypeShape::Shared(item)
        | RuntimeTypeShape::Reference(item)
        | RuntimeTypeShape::Need(item) => validate_normalized_type(modules, item),
        RuntimeTypeShape::Agent(RuntimeAgentTypeShape::Probe(value)) => {
            validate_normalized_type(modules, value)
        }
        RuntimeTypeShape::Map { key, value }
        | RuntimeTypeShape::Stream {
            item: key,
            error: value,
        }
        | RuntimeTypeShape::Parser {
            item: key,
            error: value,
        } => {
            validate_normalized_type(modules, key)?;
            validate_normalized_type(modules, value)
        }
        RuntimeTypeShape::Option { item, some_payload } => {
            validate_normalized_type(modules, item)?;
            validate_normalized_type(modules, some_payload)?;
            if normalized_tuple_payload_matches(some_payload, std::slice::from_ref(&item.as_ref()))
            {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeTypeShape::Result {
            value,
            error,
            value_payload,
            error_payload,
        } => {
            validate_normalized_type(modules, value)?;
            validate_normalized_type(modules, error)?;
            validate_normalized_type(modules, value_payload)?;
            validate_normalized_type(modules, error_payload)?;
            if normalized_tuple_payload_matches(
                value_payload,
                std::slice::from_ref(&value.as_ref()),
            ) && normalized_tuple_payload_matches(
                error_payload,
                std::slice::from_ref(&error.as_ref()),
            ) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeTypeShape::BuiltinVariant { cases, .. } => {
            for payload in cases.iter().flatten() {
                validate_normalized_type(modules, payload)?;
            }
            ty.checked_type()
                .map(|_| ())
                .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)
        }
        RuntimeTypeShape::Function { parameters, result } => {
            for parameter in parameters {
                validate_normalized_type(modules, parameter)?;
            }
            validate_normalized_type(modules, result)
        }
        RuntimeTypeShape::ProjectNominal { nominal, arguments } => {
            validate_nominal(modules, nominal)?;
            for argument in arguments {
                validate_normalized_type(modules, argument)?;
            }
            Ok(())
        }
        RuntimeTypeShape::Opaque { arguments, .. } => {
            for argument in arguments {
                validate_normalized_type(modules, argument)?;
            }
            Ok(())
        }
        RuntimeTypeShape::Tuple(items) | RuntimeTypeShape::Choice(items) => {
            for item in items {
                validate_normalized_type(modules, item)?;
            }
            Ok(())
        }
        RuntimeTypeShape::Record(fields) => {
            for field in fields {
                validate_normalized_type(modules, field.ty())?;
            }
            ty.checked_type()
                .map(|_| ())
                .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)
        }
        RuntimeTypeShape::Unit
        | RuntimeTypeShape::Never
        | RuntimeTypeShape::Bool
        | RuntimeTypeShape::Signed(_)
        | RuntimeTypeShape::Unsigned(_)
        | RuntimeTypeShape::F32
        | RuntimeTypeShape::F64
        | RuntimeTypeShape::String
        | RuntimeTypeShape::Char
        | RuntimeTypeShape::Bytes
        | RuntimeTypeShape::Duration
        | RuntimeTypeShape::Progress
        | RuntimeTypeShape::EntityReference
        | RuntimeTypeShape::AgentValue
        | RuntimeTypeShape::Agent(
            RuntimeAgentTypeShape::DebugStatePath
            | RuntimeAgentTypeShape::ObservationFieldPath
            | RuntimeAgentTypeShape::Predicate
            | RuntimeAgentTypeShape::Observation
            | RuntimeAgentTypeShape::ObservedObject
            | RuntimeAgentTypeShape::BoundingBox
            | RuntimeAgentTypeShape::ActionName
            | RuntimeAgentTypeShape::ActionTarget
            | RuntimeAgentTypeShape::ActionResult
            | RuntimeAgentTypeShape::DataFormat
            | RuntimeAgentTypeShape::DataShape
            | RuntimeAgentTypeShape::EntityMetadata
            | RuntimeAgentTypeShape::SourceAnchor
            | RuntimeAgentTypeShape::ProjectGraphNeighborhood
            | RuntimeAgentTypeShape::ProjectGraphSymbol
            | RuntimeAgentTypeShape::ProjectGraphEdge
            | RuntimeAgentTypeShape::CaptureTarget
            | RuntimeAgentTypeShape::CaptureReference
            | RuntimeAgentTypeShape::Resource
            | RuntimeAgentTypeShape::RagContextPack
            | RuntimeAgentTypeShape::ObservedObjectId
            | RuntimeAgentTypeShape::Diagnostics
            | RuntimeAgentTypeShape::WaitError
            | RuntimeAgentTypeShape::ViewportPoint
            | RuntimeAgentTypeShape::RagError
            | RuntimeAgentTypeShape::SourcePosition
            | RuntimeAgentTypeShape::ProjectFlowControlSummary
            | RuntimeAgentTypeShape::ProjectGraphSummary
            | RuntimeAgentTypeShape::BinaryResourceBody
            | RuntimeAgentTypeShape::BinaryData,
        ) => Ok(()),
    }
}

fn validate_call(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
    expression: ExprId,
    hir_call: &HirCallInvocation,
    attached_body: Option<arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence>,
    call: &RuntimeResolvedCall,
) -> Result<(), RuntimeSemanticFactsError> {
    use arcweft_lang_hir::dialogue_application::HirAttachedContentBodyPresence;

    let attached_matches = match call.attached_content() {
        Some(RuntimeResolvedAttachedContent::Required { source, ty }) => {
            *source == expression
                && expression_types.get(source) == Some(ty)
                && attached_body == Some(HirAttachedContentBodyPresence::Present)
        }
        Some(RuntimeResolvedAttachedContent::OptionalPresent { source, ty })
        | Some(RuntimeResolvedAttachedContent::DefaultedPresent { source, ty }) => {
            *source == expression
                && matches!(
                    (ty.shape(), expression_types.get(source)),
                    (RuntimeTypeShape::Option { item, .. }, Some(source_ty))
                        if item.as_ref() == source_ty
                )
                && attached_body == Some(HirAttachedContentBodyPresence::Present)
        }
        Some(RuntimeResolvedAttachedContent::OptionalOmitted { ty })
        | Some(RuntimeResolvedAttachedContent::DefaultedOmitted { ty }) => {
            validate_normalized_type(modules, ty).is_ok()
                && matches!(ty.shape(), RuntimeTypeShape::Option { .. })
                && attached_body != Some(HirAttachedContentBodyPresence::Present)
        }
        None => attached_body != Some(HirAttachedContentBodyPresence::Present),
    };
    if !attached_matches {
        return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression });
    }
    if let Some(plan) = call.project_function() {
        validate_callable(modules, plan.callable())?;
        if let Some(function_type) = plan.input().function_type() {
            validate_normalized_type(modules, function_type)?;
            let Some(callee) = plan.input().callee() else {
                return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                    expression,
                });
            };
            if expression_types.get(&callee) != Some(function_type) {
                return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                    expression,
                });
            }
        }
        if let Some(function_type) = plan.outcome().function_type() {
            validate_normalized_type(modules, function_type)?;
            if expression_types.get(&expression) != Some(function_type) {
                return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                    expression,
                });
            }
        }
    } else if matches!(
        call.dispatch(),
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
            callable
        )) if callable.declaration().owner() == CallableDeclarationOwner::Function
    ) {
        return Err(RuntimeSemanticFactsError::MissingProjectFunctionCallPlan);
    }
    match call.dispatch() {
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
            callable,
        )) => validate_callable(modules, callable)?,
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Host(host)) => {
            if let RuntimeResolvedHostCallOwner::ExternCapability(callable) = host.owner() {
                validate_callable(modules, callable)?;
            }
        }
        RuntimeResolvedCallDispatch::Value { callee } => {
            if !matches!(
                hir_call.callee(),
                arcweft_lang_hir::expr::HirCallCallee::Value { value } if value == callee
            ) {
                return Err(RuntimeSemanticFactsError::MissingFunctionValueCallee);
            }
        }
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Variant(variant)) => {
            validate_variant(modules, variant)?;
        }
        RuntimeResolvedCallDispatch::Static(
            RuntimeResolvedStaticCallTarget::Agent(_)
            | RuntimeResolvedStaticCallTarget::AgentProbeComparison(_)
            | RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError
            | RuntimeResolvedStaticCallTarget::Reduction(_)
            | RuntimeResolvedStaticCallTarget::Intrinsic(_)
            | RuntimeResolvedStaticCallTarget::TraitMethod { .. }
            | RuntimeResolvedStaticCallTarget::Registered(_),
        ) => {}
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::StandardMap(map)) => {
            if !runtime_standard_map_matches_operands(call, map, expression_types.get(&expression))
            {
                return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                    expression,
                });
            }
        }
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Line(line)) => {
            if !runtime_line_callable_matches_operands(call, line) {
                return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                    expression,
                });
            }
        }
    }

    let module = modules
        .get(&expression.module())
        .copied()
        .ok_or(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression })?;
    let count = hir_call.arguments().len();
    let mut origins = BTreeSet::new();
    let expected_receiver = module
        .resolve_call_value_receiver(hir_call)
        .map_err(|_| RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression })?;
    for operand in call.operands() {
        if !origins.insert(operand.origin().clone()) {
            return Err(RuntimeSemanticFactsError::DuplicateCallArgument);
        }
        if !runtime_call_projection_matches_type(operand) {
            return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression });
        }
        match operand.origin() {
            RuntimeResolvedCallOperandOrigin::Receiver => {
                let RuntimeResolvedCallOperandSource::Expression(source) = operand.source() else {
                    return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                        expression,
                    });
                };
                if expected_receiver != Some(source)
                    || !matches!(
                        operand.binding(),
                        RuntimeResolvedCallOperandBinding::Positional
                    )
                    || !matches!(
                        operand.projection(),
                        RuntimeResolvedCallOperandProjection::Scalar
                    )
                    || operand.parameter().is_some()
                    || expression_types.get(&source) != Some(operand.ty())
                {
                    return Err(RuntimeSemanticFactsError::MissingRuntimeCallReceiver {
                        expression,
                    });
                }
            }
            RuntimeResolvedCallOperandOrigin::Argument { argument, .. } => {
                let ordinal = usize::try_from(*argument).map_err(|_| {
                    RuntimeSemanticFactsError::InvalidCallArgumentOrdinal {
                        ordinal: *argument,
                        count,
                    }
                })?;
                let authored = hir_call.arguments().get(ordinal).ok_or(
                    RuntimeSemanticFactsError::InvalidCallArgumentOrdinal {
                        ordinal: *argument,
                        count,
                    },
                )?;
                match operand.source() {
                    RuntimeResolvedCallOperandSource::Expression(source) => {
                        if source != authored.value()
                            || expression_types.get(&source) != Some(operand.ty())
                        {
                            return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                                expression,
                            });
                        }
                    }
                    RuntimeResolvedCallOperandSource::CompactNumericElement {
                        sequence,
                        ordinal,
                    } => {
                        if sequence != authored.value() || !expression_types.contains_key(&sequence)
                        {
                            return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                                expression,
                            });
                        }
                        let sequence_expression = module.resolve_expr(sequence).map_err(|_| {
                            RuntimeSemanticFactsError::InvalidRuntimeCallDisposition { expression }
                        })?;
                        let HirExprKind::NumericBracketSequence(sequence) =
                            sequence_expression.kind()
                        else {
                            return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                                expression,
                            });
                        };
                        if usize::try_from(ordinal)
                            .map_or(true, |ordinal| ordinal >= sequence.elements().len())
                        {
                            return Err(RuntimeSemanticFactsError::InvalidCallArgumentOrdinal {
                                ordinal,
                                count: sequence.elements().len(),
                            });
                        }
                    }
                }
                let binding_matches = match authored {
                    HirCallArgument::Positional { .. } => matches!(
                        operand.binding(),
                        RuntimeResolvedCallOperandBinding::Positional
                    ),
                    HirCallArgument::Named { .. } => matches!(
                        operand.binding(),
                        RuntimeResolvedCallOperandBinding::Named(name)
                            if authored.resolved_name().is_some_and(|resolved| resolved.as_str() == name)
                    ),
                    HirCallArgument::Spread { .. } => matches!(
                        operand.binding(),
                        RuntimeResolvedCallOperandBinding::Positional
                    ),
                };
                if !binding_matches {
                    return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                        expression,
                    });
                }
                if matches!(
                    operand.projection(),
                    RuntimeResolvedCallOperandProjection::SpreadContainer(_)
                ) && !matches!(authored, HirCallArgument::Spread { .. })
                {
                    return Err(RuntimeSemanticFactsError::InvalidRuntimeCallDisposition {
                        expression,
                    });
                }
            }
        }
    }
    match call.dispatch() {
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Reduction(_))
            if (!matches!(
                call.operands(),
                [RuntimeResolvedCallOperand {
                    origin: RuntimeResolvedCallOperandOrigin::Argument {
                        argument: 0,
                        slot: 0
                    },
                    binding: RuntimeResolvedCallOperandBinding::Positional,
                    projection: RuntimeResolvedCallOperandProjection::Scalar,
                    ..
                }]
            ) || call.result() != RuntimeCallResultShape::Value) =>
        {
            return Err(RuntimeSemanticFactsError::InvalidReductionConstructorCall);
        }
        RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Agent(_))
            if call.operands().iter().any(|operand| {
                matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver)
            }) =>
        {
            return Err(RuntimeSemanticFactsError::InvalidAgentCallArguments);
        }
        RuntimeResolvedCallDispatch::Static(
            RuntimeResolvedStaticCallTarget::AgentProbeComparison(_),
        ) if !matches!(
            call.operands(),
            [
                RuntimeResolvedCallOperand {
                    origin: RuntimeResolvedCallOperandOrigin::Receiver,
                    ..
                },
                RuntimeResolvedCallOperand {
                    origin: RuntimeResolvedCallOperandOrigin::Argument { argument: 0, .. },
                    ..
                }
            ]
        ) || call.result() != RuntimeCallResultShape::Value =>
        {
            return Err(RuntimeSemanticFactsError::InvalidAgentCallArguments);
        }
        RuntimeResolvedCallDispatch::Static(
            RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError,
        ) if !matches!(
            call.operands(),
            [RuntimeResolvedCallOperand {
                origin: RuntimeResolvedCallOperandOrigin::Receiver,
                ..
            }]
        ) || call.result() != RuntimeCallResultShape::Value =>
        {
            return Err(RuntimeSemanticFactsError::InvalidAgentCallArguments);
        }
        _ => {}
    }
    Ok(())
}

fn runtime_standard_map_matches_operands(
    call: &RuntimeResolvedCall,
    map: &RuntimeStandardMapCall,
    result: Option<&RuntimeNormalizedType>,
) -> bool {
    if call.result() != RuntimeCallResultShape::Value {
        return false;
    }
    let mut mappings = call
        .operands()
        .iter()
        .filter(|operand| operand.parameter() == Some(RuntimeCallParameterCoordinate::new(0, 0)));
    let Some(mapping) = mappings.next() else {
        return false;
    };
    if mappings.next().is_some()
        || mapping.source() != RuntimeResolvedCallOperandSource::Expression(map.mapping())
        || !matches!(
            mapping.projection(),
            RuntimeResolvedCallOperandProjection::Scalar
        )
    {
        return false;
    }
    let mut receivers = call.operands().iter().filter(|operand| {
        matches!(operand.origin(), RuntimeResolvedCallOperandOrigin::Receiver)
            || operand.parameter() == Some(RuntimeCallParameterCoordinate::new(1, 0))
    });
    let Some(receiver) = receivers.next() else {
        return false;
    };
    if receivers.next().is_some()
        || receiver.source() != RuntimeResolvedCallOperandSource::Expression(map.receiver())
        || !matches!(
            receiver.projection(),
            RuntimeResolvedCallOperandProjection::Scalar
        )
    {
        return false;
    }
    match map.order() {
        RuntimeStandardMapOperandOrder::ReceiverThenMapping
            if !matches!(
                receiver.origin(),
                RuntimeResolvedCallOperandOrigin::Receiver
            ) =>
        {
            return false;
        }
        RuntimeStandardMapOperandOrder::MappingThenReceiver
            if matches!(
                receiver.origin(),
                RuntimeResolvedCallOperandOrigin::Receiver
            ) =>
        {
            return false;
        }
        RuntimeStandardMapOperandOrder::MappingThenReceiver
        | RuntimeStandardMapOperandOrder::ReceiverThenMapping => {}
    }
    let Some(result) = result else {
        return false;
    };
    let RuntimeTypeShape::Function {
        parameters,
        result: mapping_result,
    } = mapping.ty().shape()
    else {
        return false;
    };
    let [mapping_input] = parameters.as_ref() else {
        return false;
    };
    let Some((receiver_input, result_output)) =
        standard_map_item_types(map.family(), receiver.ty().shape(), result.shape())
    else {
        return false;
    };
    mapping_input == receiver_input && mapping_result.as_ref() == result_output
}

fn standard_map_item_types<'a>(
    family: RuntimeStandardMapFamily,
    receiver: &'a RuntimeTypeShape,
    result: &'a RuntimeTypeShape,
) -> Option<(&'a RuntimeNormalizedType, &'a RuntimeNormalizedType)> {
    match (family, receiver, result) {
        (
            RuntimeStandardMapFamily::Vec,
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item: input,
            },
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item: output,
            },
        )
        | (
            RuntimeStandardMapFamily::Seq,
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Seq,
                item: input,
            },
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Seq,
                item: output,
            },
        )
        | (
            RuntimeStandardMapFamily::Slice,
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Slice,
                item: input,
            },
            RuntimeTypeShape::Sequence {
                kind: RuntimeSequenceKind::Vec,
                item: output,
            },
        )
        | (
            RuntimeStandardMapFamily::Option,
            RuntimeTypeShape::Option { item: input, .. },
            RuntimeTypeShape::Option { item: output, .. },
        ) => Some((input, output)),
        (
            RuntimeStandardMapFamily::Array,
            RuntimeTypeShape::Array {
                item: input,
                length: input_length,
            },
            RuntimeTypeShape::Array {
                item: output,
                length: output_length,
            },
        ) if input_length == output_length => Some((input, output)),
        (
            RuntimeStandardMapFamily::Result,
            RuntimeTypeShape::Result {
                value: input,
                error: input_error,
                ..
            },
            RuntimeTypeShape::Result {
                value: output,
                error: output_error,
                ..
            },
        ) if input_error == output_error => Some((input, output)),
        _ => None,
    }
}

fn runtime_line_callable_matches_operands(
    call: &RuntimeResolvedCall,
    line: &RuntimeLineCallable,
) -> bool {
    let exact_operand =
        |source: ExprId, parameter: Option<RuntimeCallParameterCoordinate>, receiver: bool| {
            let mut operands = call.operands().iter().filter(|operand| {
                matches!(
                    (receiver, operand.origin()),
                    (true, RuntimeResolvedCallOperandOrigin::Receiver)
                        | (false, RuntimeResolvedCallOperandOrigin::Argument { .. })
                ) && operand.parameter() == parameter
                    && operand.source() == RuntimeResolvedCallOperandSource::Expression(source)
                    && matches!(
                        operand.projection(),
                        RuntimeResolvedCallOperandProjection::Scalar
                    )
            });
            operands.next().is_some() && operands.next().is_none()
        };
    match line {
        RuntimeLineCallable::ActorLook {
            character: _,
            actor,
            look,
            crossfade,
        } => {
            exact_operand(*actor, None, true)
                && exact_operand(
                    *look,
                    Some(RuntimeCallParameterCoordinate::new(0, 0)),
                    false,
                )
                && exact_operand(
                    *crossfade,
                    Some(RuntimeCallParameterCoordinate::new(0, 1)),
                    false,
                )
        }
        RuntimeLineCallable::Schedule { callback, .. } => exact_operand(
            *callback,
            Some(RuntimeCallParameterCoordinate::new(1, 0)),
            false,
        ),
        RuntimeLineCallable::AcquireActor { .. } | RuntimeLineCallable::VoiceHandle => true,
    }
}

fn runtime_call_projection_matches_type(operand: &RuntimeResolvedCallOperand) -> bool {
    match operand.projection() {
        RuntimeResolvedCallOperandProjection::Scalar => true,
        RuntimeResolvedCallOperandProjection::SpreadContainer(container) => {
            match (container, operand.ty().shape()) {
                (
                    RuntimeResolvedSpreadContainer::Vec,
                    RuntimeTypeShape::Sequence {
                        kind: RuntimeSequenceKind::Vec,
                        ..
                    },
                )
                | (
                    RuntimeResolvedSpreadContainer::Seq,
                    RuntimeTypeShape::Sequence {
                        kind: RuntimeSequenceKind::Seq,
                        ..
                    },
                )
                | (
                    RuntimeResolvedSpreadContainer::Slice,
                    RuntimeTypeShape::Sequence {
                        kind: RuntimeSequenceKind::Slice,
                        ..
                    },
                ) => true,
                (
                    RuntimeResolvedSpreadContainer::Array { len },
                    RuntimeTypeShape::Array { length, .. },
                ) => len == length,
                (
                    RuntimeResolvedSpreadContainer::MapValue { key, .. },
                    RuntimeTypeShape::Map { key: actual, .. },
                ) => key == actual.as_ref(),
                _ => false,
            }
        }
    }
}

fn validate_callable(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    callable: &RuntimeProjectCallable,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, callable.owner())?;
    let valid_family = matches!(
        (
            callable.declaration().owner(),
            callable.source_owner(),
            item.kind()
        ),
        (
            CallableDeclarationOwner::Function,
            HirCallableSourceOwner::Item,
            HirItemKind::Function(_)
        ) | (
            CallableDeclarationOwner::ExternCapability,
            HirCallableSourceOwner::ExternCapabilityFunction { .. },
            HirItemKind::ExternCapability(_)
        ) | (
            CallableDeclarationOwner::View,
            HirCallableSourceOwner::ViewItem,
            HirItemKind::View(_)
        ) | (
            CallableDeclarationOwner::Predicate,
            HirCallableSourceOwner::Item,
            HirItemKind::Predicate(_)
        ) | (
            CallableDeclarationOwner::Proof,
            HirCallableSourceOwner::Item,
            HirItemKind::Proof(_)
        ) | (
            CallableDeclarationOwner::TraitRequirement,
            HirCallableSourceOwner::TraitFunction { .. },
            HirItemKind::Trait(_)
        ) | (
            CallableDeclarationOwner::TraitImplementation
                | CallableDeclarationOwner::InherentMethod,
            HirCallableSourceOwner::ImplFunction { .. },
            HirItemKind::Impl(_)
        )
    );
    if !valid_family {
        let valid_item_family = matches!(
            (callable.declaration().owner(), item.kind()),
            (CallableDeclarationOwner::Function, HirItemKind::Function(_))
                | (
                    CallableDeclarationOwner::ExternCapability,
                    HirItemKind::ExternCapability(_)
                )
                | (CallableDeclarationOwner::View, HirItemKind::View(_))
                | (
                    CallableDeclarationOwner::Predicate,
                    HirItemKind::Predicate(_)
                )
                | (CallableDeclarationOwner::Proof, HirItemKind::Proof(_))
                | (
                    CallableDeclarationOwner::TraitRequirement,
                    HirItemKind::Trait(_)
                )
                | (
                    CallableDeclarationOwner::TraitImplementation
                        | CallableDeclarationOwner::InherentMethod,
                    HirItemKind::Impl(_)
                )
        );
        if valid_item_family {
            return Err(RuntimeSemanticFactsError::InvalidCallableSourceOwner);
        }
        return Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: callable.owner(),
            actual: item.kind().family(),
        });
    }

    let hir_attached = item
        .kind()
        .callable_attached_content_interface(callable.source_owner());
    let attached_matches = match (callable.attached_content_abi(), hir_attached) {
        (None, None) => true,
        (Some(runtime), Some(hir_interface)) => {
            use arcweft_lang_hir::item::HirAttachedContentPresence;
            use arcweft_lang_sema::callable::CallableParameterPresence;

            let hir = hir_interface.parameter();
            runtime.binding() == hir.binding()
                && u32::try_from(runtime.group().get()).ok() == Some(hir_interface.group())
                && runtime.abi_position() == hir_interface.abi_position()
                && match (runtime.presence(), hir.presence(), runtime.default()) {
                    (
                        CallableParameterPresence::Required,
                        HirAttachedContentPresence::Required,
                        None,
                    )
                    | (
                        CallableParameterPresence::Optional,
                        HirAttachedContentPresence::Optional,
                        None,
                    ) => true,
                    (
                        CallableParameterPresence::Defaulted,
                        HirAttachedContentPresence::Defaulted { value },
                        Some(default),
                    ) => default.source() == value,
                    _ => false,
                }
        }
        (None, Some(_)) | (Some(_), None) => false,
    };
    if !attached_matches {
        return Err(RuntimeSemanticFactsError::InvalidCallableAttachedContentAbi);
    }
    Ok(())
}

fn validate_project_function_instance(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    instance: &RuntimeProjectFunctionInstanceFact,
) -> Result<(), RuntimeSemanticFactsError> {
    validate_callable(modules, instance.callable())?;
    if instance.key().callable() != instance.callable().runtime() {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let module = module_for(modules, instance.callable().owner().module())?;
    let item = module
        .resolve_item(instance.callable().owner())
        .map_err(|_| RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    let HirItemKind::Function(function) = item.kind() else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    function
        .parameter_groups()
        .get(instance.key().group().get())
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    let expected_parameters = function
        .parameter_groups()
        .iter()
        .enumerate()
        .take(instance.key().group().get() + 1)
        .flat_map(|(group, parameters)| {
            parameters
                .parameters()
                .iter()
                .enumerate()
                .map(move |(parameter, row)| (group, parameter, row))
        })
        .collect::<Vec<_>>();
    if expected_parameters.len() != instance.parameters().len()
        || expected_parameters.iter().zip(instance.parameters()).any(
            |((group, position, hir), runtime)| {
                arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(*group).ok()
                    != Some(runtime.group())
                    || u32::try_from(*position).ok() != Some(runtime.parameter())
                    || hir.pattern() != runtime.pattern()
                    || hir.ty() != runtime.source_type()
                    || hir.kind() != runtime.kind()
                    || hir.locals() != runtime.bindings()
            },
        )
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let arcweft_lang_hir::item::HirFunctionBody::Block {
        scope,
        statements,
        tail,
    } = function.body()
    else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    if instance.body().scope() != *scope
        || instance.body().statements() != statements.as_ref()
        || instance.body().tail() != *tail
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    validate_normalized_type(modules, instance.function_type())?;
    for parameter in instance.parameters() {
        validate_normalized_type(modules, parameter.abi_ty())?;
        validate_normalized_type(modules, parameter.binding_ty())?;
    }
    for projection in instance.type_projection() {
        if let Some(ty) = projection.ty() {
            validate_normalized_type(modules, ty)?;
        }
    }
    if let Some(default) = instance.attached_default() {
        validate_normalized_type(modules, default.result())?;
        if default.source().module() != instance.callable().owner().module()
            || !runtime_owners
                .executable_owners(&HirRuntimeExecutableOwner::Item(
                    instance.callable().owner(),
                ))
                .is_some_and(|owners| owners.expressions().any(|owner| owner == default.source()))
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
        for capture in default.captures() {
            validate_normalized_type(modules, capture.binding_ty())?;
            if !capture
                .used_locals()
                .iter()
                .all(|local| capture.bindings().contains(local))
            {
                return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
            }
        }
    }

    let exact_owners = runtime_owners
        .executable_owners(&HirRuntimeExecutableOwner::Item(
            instance.callable().owner(),
        ))
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    if instance.semantics().partition().reachability() != runtime_owners.runtime.identity()
        || instance.semantics().partition().executable()
            != &HirRuntimeExecutableOwner::Item(instance.callable().owner())
        || instance.semantics().expressions().iter().any(|fact| {
            !exact_owners
                .expressions()
                .any(|owner| owner == fact.owner())
        })
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    if !exact_owners
        .expressions()
        .any(|owner| owner == instance.body().tail())
        || instance
            .body()
            .statements()
            .iter()
            .any(|statement| !exact_owners.statements().any(|owner| owner == *statement))
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let expected_type_owners = exact_owners
        .expressions()
        .map(RuntimeProjectFunctionTypeOwner::Expression)
        .chain(
            exact_owners
                .patterns()
                .map(RuntimeProjectFunctionTypeOwner::Pattern),
        )
        .chain(
            exact_owners
                .locals()
                .map(RuntimeProjectFunctionTypeOwner::Local),
        )
        .chain(
            exact_owners
                .types()
                .map(RuntimeProjectFunctionTypeOwner::Type),
        )
        .collect::<BTreeSet<_>>();
    let actual_type_owners = instance
        .type_projection()
        .iter()
        .map(RuntimeProjectFunctionTypeProjection::owner)
        .collect::<BTreeSet<_>>();
    if expected_type_owners != actual_type_owners {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let projected_types = instance
        .type_projection()
        .iter()
        .filter_map(|projection| projection.ty().map(|ty| (projection.owner(), ty)))
        .collect::<BTreeMap<_, _>>();
    let instance_expression_types = instance
        .type_projection()
        .iter()
        .filter_map(|projection| match (projection.owner(), projection.ty()) {
            (RuntimeProjectFunctionTypeOwner::Expression(owner), Some(ty)) => {
                Some((owner, ty.clone()))
            }
            (RuntimeProjectFunctionTypeOwner::Expression(_), None)
            | (RuntimeProjectFunctionTypeOwner::Pattern(_), _)
            | (RuntimeProjectFunctionTypeOwner::Local(_), _)
            | (RuntimeProjectFunctionTypeOwner::Type(_), _) => None,
        })
        .collect::<BTreeMap<_, _>>();
    for projection in instance.semantics().expressions() {
        let RuntimeProjectFunctionExpressionPayload::Call(call) = projection.payload() else {
            continue;
        };
        let expression = projection.owner();
        let kind = resolve_expr(modules, expression)?;
        let (hir_call, attached_body) = match kind {
            HirExprKind::Call(hir_call) => (hir_call, None),
            HirExprKind::AttachedContentApplication(application) => {
                let Some(invocation) = application.family().invocation() else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                (invocation, Some(application.body_presence()))
            }
            _ => return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance),
        };
        validate_call(
            modules,
            &instance_expression_types,
            expression,
            hir_call,
            attached_body,
            call,
        )?;
        validate_project_function_call_materialization(modules, call)?;
        let executable = match call.dispatch() {
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::Declaration(
                callable,
            )) => Some(HirRuntimeExecutableOwner::Item(callable.owner())),
            RuntimeResolvedCallDispatch::Static(RuntimeResolvedStaticCallTarget::TraitMethod {
                method,
                ..
            }) => Some(HirRuntimeExecutableOwner::ImplMethod(method.clone())),
            RuntimeResolvedCallDispatch::Static(
                RuntimeResolvedStaticCallTarget::Intrinsic(_)
                | RuntimeResolvedStaticCallTarget::Agent(_)
                | RuntimeResolvedStaticCallTarget::AgentProbeComparison(_)
                | RuntimeResolvedStaticCallTarget::AgentDiagnosticsHasError
                | RuntimeResolvedStaticCallTarget::Variant(_)
                | RuntimeResolvedStaticCallTarget::Reduction(_)
                | RuntimeResolvedStaticCallTarget::StandardMap(_)
                | RuntimeResolvedStaticCallTarget::Line(_)
                | RuntimeResolvedStaticCallTarget::Registered(_)
                | RuntimeResolvedStaticCallTarget::Host(_),
            )
            | RuntimeResolvedCallDispatch::Value { .. } => None,
        };
        if let Some(owner) = executable
            && !runtime_owners.contains_runtime_owner(&owner)
        {
            return Err(RuntimeSemanticFactsError::OwnerOutsideReachability { owner });
        }
    }
    if !projected_types.contains_key(&RuntimeProjectFunctionTypeOwner::Expression(
        instance.body().tail(),
    )) || instance.parameters().iter().any(|parameter| {
        projected_types.get(&RuntimeProjectFunctionTypeOwner::Pattern(
            parameter.pattern(),
        )) != Some(&parameter.binding_ty())
            || projected_types.get(&RuntimeProjectFunctionTypeOwner::Type(
                parameter.source_type(),
            )) != Some(&parameter.abi_ty())
    }) {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    if let Some(default) = instance.attached_default()
        && (projected_types.get(&RuntimeProjectFunctionTypeOwner::Expression(
            default.source(),
        )) != Some(&default.result())
            || default.captures().iter().any(|capture| {
                projected_types.get(&RuntimeProjectFunctionTypeOwner::Pattern(capture.pattern()))
                    != Some(&capture.binding_ty())
                    || capture.used_locals().iter().any(|local| {
                        !projected_types
                            .contains_key(&RuntimeProjectFunctionTypeOwner::Local(*local))
                    })
            }))
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    validate_project_function_semantic_catalog(
        modules,
        runtime_owners,
        Some(instance.key()),
        instance.callable().runtime(),
        instance.semantics(),
        None,
    )?;
    Ok(())
}

fn validate_project_function_root(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    instances: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeProjectFunctionInstanceFact>,
    root: &RuntimeProjectFunctionRootFact,
) -> Result<(), RuntimeSemanticFactsError> {
    if !runtime_owners.contains_runtime_owner(&HirRuntimeExecutableOwner::Item(root.entry())) {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionRoot);
    }
    let module = module_for(modules, root.entry().module())?;
    let item = module
        .resolve_item(root.entry())
        .map_err(|_| RuntimeSemanticFactsError::InvalidProjectFunctionRoot)?;
    let HirItemKind::Entry(entry) = item.kind() else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionRoot);
    };
    if entry.has_structural_recovery()
        || instances.get(root.instance()).is_none_or(|instance| {
            instance.key() != root.instance()
                || instance.callable().runtime() != root.instance().callable()
        })
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionRoot);
    }
    let matching_members = entry
        .members()
        .iter()
        .filter(|member| match (root.role(), member) {
            (
                RuntimeProjectFunctionRootRole::EntryInitializer,
                HirEntryMember::Initializer(binding),
            )
            | (RuntimeProjectFunctionRootRole::EntryReducer, HirEntryMember::Reducer(binding))
            | (
                RuntimeProjectFunctionRootRole::EntryController,
                HirEntryMember::Controller(binding),
            ) => !binding.has_recovery(),
            _ => false,
        })
        .count();
    if matching_members != 1 {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionRoot);
    }
    Ok(())
}

fn validate_project_function_semantic_catalog(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    parent_key: Option<&RuntimeProjectFunctionInstanceKey>,
    callable: &RuntimeCallableId,
    semantics: &RuntimeProjectFunctionInstanceSemanticFacts,
    outer: Option<RuntimeExecutableSemanticFactView<'_>>,
) -> Result<(), RuntimeSemanticFactsError> {
    let partition = semantics.partition();
    if partition.reachability() != runtime_owners.runtime.identity() {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let exact = runtime_owners
        .executable_owners(partition.executable())
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    if partition
        .expressions()
        .iter()
        .map(|row| row.owner())
        .ne(exact.expressions())
        || partition
            .patterns()
            .iter()
            .map(|row| row.owner())
            .ne(exact.patterns())
        || partition
            .statements()
            .iter()
            .map(|row| row.owner())
            .ne(exact.statements())
        || partition.locals().iter().copied().ne(exact.locals())
        || partition.types().iter().copied().ne(exact.types())
        || partition.captures().iter().copied().ne(exact.captures())
        || partition
            .expressions()
            .iter()
            .any(|row| row.children() != exact.expression_children(row.owner()))
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }

    let expression_types = semantics
        .type_projection()
        .iter()
        .filter_map(|projection| match (projection.owner(), projection.ty()) {
            (RuntimeProjectFunctionTypeOwner::Expression(owner), Some(ty)) => {
                Some((owner, ty.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let pattern_types = semantics
        .type_projection()
        .iter()
        .filter_map(|projection| match (projection.owner(), projection.ty()) {
            (RuntimeProjectFunctionTypeOwner::Pattern(owner), Some(ty)) => {
                Some((owner, ty.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let local_types = semantics
        .type_projection()
        .iter()
        .filter_map(|projection| match (projection.owner(), projection.ty()) {
            (RuntimeProjectFunctionTypeOwner::Local(owner), Some(ty)) => Some((owner, ty.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let calls = semantics
        .expressions()
        .iter()
        .filter_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::Call(call) => {
                Some((row.owner(), call.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let values = semantics
        .expressions()
        .iter()
        .filter_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::Value(value) => {
                Some((row.owner(), value.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let selects = semantics
        .expressions()
        .iter()
        .filter_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::Select(select) => {
                Some((row.owner(), select.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();

    for projection in semantics.type_projection() {
        if let Some(ty) = projection.ty() {
            validate_normalized_type(modules, ty)?;
        }
    }
    for capture in semantics.captures() {
        if !exact.capture_plan().contains(capture.projection()) {
            return Err(RuntimeSemanticFactsError::InvalidCaptureProjection {
                capture: capture.capture(),
            });
        }
        validate_normalized_type(modules, capture.ty())?;
    }

    for row in semantics.expressions() {
        let owner = row.owner();
        let hir = resolve_expr(modules, owner)?;
        match row.payload() {
            RuntimeProjectFunctionExpressionPayload::Structural
            | RuntimeProjectFunctionExpressionPayload::Consumed => {}
            RuntimeProjectFunctionExpressionPayload::Literal(_) => {
                if !matches!(
                    hir,
                    HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_)
                ) || !expression_types.contains_key(&owner)
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::Value(value) => {
                if !matches!(
                    hir,
                    HirExprKind::Path(_)
                        | HirExprKind::EntityReference(_)
                        | HirExprKind::ShortVariant(_)
                ) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_resolved_value(modules, runtime_owners, value)?;
            }
            RuntimeProjectFunctionExpressionPayload::Select(select) => {
                if !matches!(hir, HirExprKind::Select(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_select(modules, select)?;
            }
            RuntimeProjectFunctionExpressionPayload::NominalRecord(record) => {
                if !matches!(hir, HirExprKind::Record(_) | HirExprKind::RecordLiteral(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_record_expression_fact(modules, owner, record)?;
            }
            RuntimeProjectFunctionExpressionPayload::Variant(variant) => {
                if !matches!(hir, HirExprKind::ShortVariant(_) | HirExprKind::Path(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_variant(modules, variant)?;
            }
            RuntimeProjectFunctionExpressionPayload::Call(call) => {
                let (hir_call, attached_body) = match hir {
                    HirExprKind::Call(hir_call) => (hir_call, None),
                    HirExprKind::AttachedContentApplication(application) => {
                        let Some(invocation) = application.family().invocation() else {
                            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                        };
                        (invocation, Some(application.body_presence()))
                    }
                    _ => return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance),
                };
                validate_call(
                    modules,
                    &expression_types,
                    owner,
                    hir_call,
                    attached_body,
                    call,
                )?;
                validate_project_function_call_materialization(modules, call)?;
            }
            RuntimeProjectFunctionExpressionPayload::PostfixCandidate(candidate) => {
                let HirExprKind::PostfixBracket(postfix) = hir else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                let arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                    index,
                    dialogue,
                } = postfix.candidates()
                else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if candidate != index && candidate != dialogue {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::Await(fact) => {
                let HirExprKind::Await(awaited) = hir else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if awaited.operand() != fact.operand()
                    || fact.observers().len() != awaited.branches().len()
                    || !matches!(
                        expression_types
                            .get(&fact.operand())
                            .map(RuntimeNormalizedType::shape),
                        Some(RuntimeTypeShape::Need(_))
                    )
                    || awaited
                        .branches()
                        .iter()
                        .zip(fact.observers())
                        .any(|(authored, checked)| {
                            authored.pattern() != Some(checked.pattern())
                                || !matches!(
                                    pattern_types
                                        .get(&checked.pattern())
                                        .map(RuntimeNormalizedType::shape),
                                    Some(RuntimeTypeShape::Progress)
                                )
                        })
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::Choice(fact) => {
                let HirExprKind::Choice(choice) = hir else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if fact.option_ids().len() != choice.body().items().len() {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                for goto in fact.gotos() {
                    validate_project_item(modules, goto.target())?;
                }
            }
            RuntimeProjectFunctionExpressionPayload::Try(fact) => {
                let HirExprKind::Try(tried) = hir else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if tried.operand() != fact.operand()
                    || expression_types.get(&fact.operand()) != Some(fact.carrier_type())
                    || !try_boundary_type_matches(fact)
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::ImplicitCallable {
                callable: fact,
                tried,
                pipe,
            } => {
                let Some(RuntimeTypeShape::Function { parameters, result }) = expression_types
                    .get(&owner)
                    .map(RuntimeNormalizedType::shape)
                else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if parameters.len() != 1
                    || parameters.first() != Some(fact.parameter())
                    || result.as_ref() != fact.result()
                    || fact.placeholders().is_empty()
                    || fact
                        .captures()
                        .iter()
                        .any(|capture| !local_types.contains_key(capture))
                    || tried
                        .as_ref()
                        .is_some_and(|tried| !try_boundary_type_matches(tried))
                    || pipe.as_ref().is_some_and(|pipe| {
                        pipe.placeholders().is_empty()
                            || !row.children().contains(&pipe.left())
                            || !row.children().contains(&pipe.right())
                    })
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::Pipe(pipe) => {
                let HirExprKind::Pipe(hir) = hir else {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                };
                if hir.left() != pipe.left()
                    || hir.right() != pipe.right()
                    || pipe.placeholders().is_empty()
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                application,
                fragments,
            } => {
                let root = fragments.iter().find(|fragment| fragment.source() == owner);
                if !matches!(hir, HirExprKind::AttachedContentApplication(_))
                    || root.is_none_or(|fragment| {
                        fragment.template().id() != application.content().template_id()
                            || fragment.template().digest()
                                != application.content().template_digest()
                    })
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_normalized_type(modules, application.line_result())?;
                validate_project_instance_fragments(modules, semantics, fragments)?;
            }
            RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments } => {
                if !matches!(hir, HirExprKind::AttachedContentApplication(_))
                    || fragments.iter().any(|fragment| fragment.source() != owner)
                {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_project_instance_fragments(modules, semantics, fragments)?;
            }
            RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                validate_closure_instance(
                    modules,
                    runtime_owners,
                    parent_key,
                    callable,
                    RuntimeExecutableSemanticFactView::ProjectInstance(semantics),
                    owner,
                    closure,
                )?;
            }
        }
    }

    for row in semantics.patterns() {
        let hir = resolve_pattern(modules, row.owner())?;
        match row.payload() {
            RuntimeProjectFunctionPatternPayload::Structural => {}
            RuntimeProjectFunctionPatternPayload::Literal(_) => {
                if !matches!(hir, HirPatternKind::Literal(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
            RuntimeProjectFunctionPatternPayload::Entity(item) => {
                if !matches!(hir, HirPatternKind::EntityReference(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_project_item(modules, item)?;
            }
            RuntimeProjectFunctionPatternPayload::NominalRecord(record) => {
                if !matches!(hir, HirPatternKind::Record { .. }) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_record_pattern_fact(modules, row.owner(), record)?;
            }
            RuntimeProjectFunctionPatternPayload::Variant(variant) => {
                if !matches!(hir, HirPatternKind::Variant(_)) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_variant(modules, variant)?;
            }
            RuntimeProjectFunctionPatternPayload::TypedBinding => {
                if !matches!(hir, HirPatternKind::TypedBinding { .. }) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
            }
        }
    }

    let mark_facts = semantics
        .expressions()
        .iter()
        .flat_map(|row| match row.payload() {
            RuntimeProjectFunctionExpressionPayload::DialogueApplication { fragments, .. }
            | RuntimeProjectFunctionExpressionPayload::ContentApplication { fragments } => {
                fragments.as_ref()
            }
            _ => &[],
        })
        .flat_map(RuntimeContentFragmentFact::marks)
        .map(|mark| (mark.coordinate().clone(), mark.key()))
        .collect::<BTreeMap<_, _>>();
    for row in semantics.statements() {
        match row.payload() {
            RuntimeProjectFunctionStatementPayload::Assignment(fact) => {
                validate_assignment(
                    modules,
                    &local_types,
                    &expression_types,
                    &values,
                    &selects,
                    row.owner(),
                    fact,
                )?;
            }
            RuntimeProjectFunctionStatementPayload::EvaluatedEffect(fact) => {
                evaluated_effect::validate_evaluated_effect(
                    modules,
                    &expression_types,
                    &calls,
                    row.owner(),
                    fact,
                )?;
            }
            RuntimeProjectFunctionStatementPayload::Iteration(fact) => match fact {
                RuntimeIteratorFact::Builtin(fact) => {
                    for ty in [fact.item(), fact.iterator(), fact.next_value(), fact.step()] {
                        validate_normalized_type(modules, ty)?;
                    }
                }
                RuntimeIteratorFact::Witness(fact) => {
                    validate_normalized_type(modules, fact.item())?;
                    validate_normalized_type(modules, fact.iterator())?;
                }
            },
            RuntimeProjectFunctionStatementPayload::Trigger(admission) => {
                let hir = resolve_stmt(modules, row.owner())?;
                match (hir, admission.dialogue_mark()) {
                    (
                        HirStmtKind::On {
                            trigger: HirTrigger::Mark(source),
                            ..
                        },
                        Some(mark),
                    ) if source.ordinal() == mark.coordinate().ordinal()
                        && mark_facts.get(mark.coordinate()) == Some(&mark.key()) => {}
                    (
                        HirStmtKind::On {
                            trigger: HirTrigger::Mark(_),
                            ..
                        },
                        _,
                    )
                    | (_, Some(_)) => {
                        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                    }
                    (_, None) => {}
                }
            }
            RuntimeProjectFunctionStatementPayload::Structural
            | RuntimeProjectFunctionStatementPayload::Assertion(_)
            | RuntimeProjectFunctionStatementPayload::Defer
            | RuntimeProjectFunctionStatementPayload::ControlTransfer
            | RuntimeProjectFunctionStatementPayload::UnsafeAudit
            | RuntimeProjectFunctionStatementPayload::Select
            | RuntimeProjectFunctionStatementPayload::SourceLocale
            | RuntimeProjectFunctionStatementPayload::Scope
            | RuntimeProjectFunctionStatementPayload::Include
            | RuntimeProjectFunctionStatementPayload::Suspension
            | RuntimeProjectFunctionStatementPayload::Yield => {}
        }
    }
    if outer.is_some_and(|outer| {
        semantics
            .captures()
            .iter()
            .any(|capture| outer.capture(capture.capture()).is_some())
    }) {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    Ok(())
}

fn validate_project_instance_fragments(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    semantics: &RuntimeProjectFunctionInstanceSemanticFacts,
    fragments: &[RuntimeContentFragmentFact],
) -> Result<(), RuntimeSemanticFactsError> {
    let mut sources = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let expression_types = semantics
        .expressions()
        .iter()
        .filter_map(|row| {
            semantics
                .expression_type(row.owner())
                .cloned()
                .map(|ty| (row.owner(), ty))
        })
        .collect::<BTreeMap<_, _>>();
    let calls = semantics
        .expressions()
        .iter()
        .filter_map(|row| {
            semantics
                .call(row.owner())
                .cloned()
                .map(|call| (row.owner(), call))
        })
        .collect::<BTreeMap<_, _>>();
    for fragment in fragments {
        if !sources.insert(fragment.source())
            || !ids.insert(fragment.id())
            || semantics.expression(fragment.source()).is_none()
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
        for value in fragment.values() {
            if semantics.expression_type(value.expression()) != Some(value.ty()) {
                return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
            }
            validate_normalized_type(modules, value.ty())?;
        }
        for effect in fragment.effects() {
            let trigger_valid = match effect.trigger() {
                RuntimeDialogueEffectTrigger::Content => true,
                RuntimeDialogueEffectTrigger::Delay {
                    duration_type,
                    schedule_handle_type,
                    ..
                } => {
                    matches!(duration_type.shape(), RuntimeTypeShape::Duration)
                        && validate_normalized_type(modules, duration_type).is_ok()
                        && validate_normalized_type(modules, schedule_handle_type).is_ok()
                }
            };
            if !trigger_valid
                || semantics
                    .expression(effect.operation().site_root())
                    .is_none()
                || !evaluated_effect::validate_evaluated_effect_site(modules, effect.operation())
                || !evaluated_effect::validate_evaluated_effect_operation(
                    modules,
                    &expression_types,
                    &calls,
                    effect.operation().application_site(),
                    effect.operation().effect(),
                )
            {
                return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
            }
            for capture in effect.captures() {
                if semantics.local_type(capture.local()) != Some(capture.ty()) {
                    return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
                }
                validate_normalized_type(modules, capture.ty())?;
            }
        }
    }
    Ok(())
}

fn validate_project_instance_dialogue_applications(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    catalog: &CharacterPresentationCatalogData,
    lines: &AcceptedDialogueLineInventory,
    semantics: &RuntimeProjectFunctionInstanceSemanticFacts,
) -> Result<(), RuntimeSemanticFactsError> {
    for row in semantics.expressions() {
        match row.payload() {
            RuntimeProjectFunctionExpressionPayload::DialogueApplication {
                application,
                fragments,
            } => {
                let owner = row.owner();
                let fragment = fragments
                    .iter()
                    .find(|fragment| fragment.source() == owner)
                    .ok_or(RuntimeSemanticFactsError::DialogueTemplateMismatch {
                        expression: owner,
                    })?;
                let accepted = lines
                    .for_semantic_expr(owner)
                    .ok_or(RuntimeSemanticFactsError::DialogueLineMismatch { expression: owner })?;
                let runtime_line = RuntimeLineId::from_source_entity_body(accepted.id().as_str())
                    .map_err(|_| {
                    RuntimeSemanticFactsError::DialogueLineMismatch { expression: owner }
                })?;
                if fragment.template().id() != application.content().template_id()
                    || fragment.template().digest() != application.content().template_digest()
                    || &runtime_line != application.content().line()
                    || accepted.text_key().as_str() != application.content().text_key().as_str()
                    || application.content().character().semantic_digest()
                        != catalog.semantic_digest()
                    || application.content().character().locale_policy_digest()
                        != catalog.locale_policy_digest()
                {
                    return Err(RuntimeSemanticFactsError::DialogueTemplateMismatch {
                        expression: owner,
                    });
                }
                if let arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence::Exact(
                    character,
                ) = application.content().character().target()
                    && catalog.record(character).is_err()
                {
                    return Err(RuntimeSemanticFactsError::DialogueCharacterPlanMismatch {
                        expression: owner,
                    });
                }
                validate_normalized_type(modules, application.line_result())?;
            }
            RuntimeProjectFunctionExpressionPayload::Closure(closure) => {
                validate_project_instance_dialogue_applications(
                    modules,
                    catalog,
                    lines,
                    closure.semantics(),
                )?;
            }
            RuntimeProjectFunctionExpressionPayload::Structural
            | RuntimeProjectFunctionExpressionPayload::Consumed
            | RuntimeProjectFunctionExpressionPayload::Literal(_)
            | RuntimeProjectFunctionExpressionPayload::Value(_)
            | RuntimeProjectFunctionExpressionPayload::Select(_)
            | RuntimeProjectFunctionExpressionPayload::NominalRecord(_)
            | RuntimeProjectFunctionExpressionPayload::Variant(_)
            | RuntimeProjectFunctionExpressionPayload::Call(_)
            | RuntimeProjectFunctionExpressionPayload::PostfixCandidate(_)
            | RuntimeProjectFunctionExpressionPayload::Await(_)
            | RuntimeProjectFunctionExpressionPayload::Choice(_)
            | RuntimeProjectFunctionExpressionPayload::Try(_)
            | RuntimeProjectFunctionExpressionPayload::ImplicitCallable { .. }
            | RuntimeProjectFunctionExpressionPayload::Pipe(_)
            | RuntimeProjectFunctionExpressionPayload::ContentApplication { .. } => {}
        }
    }
    Ok(())
}

fn validate_closure_instance(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    parent_key: Option<&RuntimeProjectFunctionInstanceKey>,
    callable: &RuntimeCallableId,
    outer: RuntimeExecutableSemanticFactView<'_>,
    owner: ExprId,
    closure: &RuntimeClosureInstanceFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let arcweft_lang_sema::callable::CheckedCallableContext::Project {
        world, revision, ..
    } = closure.key().closure().owner().context()
    else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    if world != runtime_owners.runtime.identity().symbol_world()
        || *revision != runtime_owners.runtime.identity().symbol_revision()
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let HirExprKind::Closure(hir) = resolve_expr(modules, owner)? else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    let expected_captures = runtime_owners
        .executable_owners(&HirRuntimeExecutableOwner::Closure(owner))
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?
        .capture_plan();
    let module = module_for(modules, owner.module())?;
    let source = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner,
                role: HirExprSourceRole::Whole,
            },
        )
        .ok()
        .and_then(|lookup| match lookup.presence() {
            HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span),
            HirSourcePresence::Present(HirSourceSite::Insertion(_))
            | HirSourcePresence::AbsentOptional => None,
        })
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    if closure.owner() != owner
        || closure.key().enclosing_instance() != parent_key
        || closure.key().closure().expression() != source
        || RuntimeCallableId::from_checked_digest(
            closure
                .key()
                .closure()
                .owner()
                .semantic_digest()
                .into_bytes(),
        ) != *callable
        || closure.scope() != hir.scope()
        || closure.body() != hir.body()
        || closure.parameters().len() != hir.parameters().len()
        || closure.captures().len() != expected_captures.len()
        || outer.expression_type(owner) != Some(closure.function_type())
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    for (position, (runtime, hir)) in closure
        .parameters()
        .iter()
        .zip(hir.parameters())
        .enumerate()
    {
        if u32::try_from(position).ok() != Some(runtime.position())
            || runtime.pattern() != hir.pattern()
            || closure.semantics().pattern_type(hir.pattern()) != Some(runtime.ty())
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
    }
    for (position, (runtime, capture)) in
        closure.captures().iter().zip(expected_captures).enumerate()
    {
        let checked = module
            .resolve_capture(capture.capture())
            .map_err(|_| RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
        if u32::try_from(position).ok() != Some(runtime.position())
            || runtime.capture() != capture.capture()
            || runtime.source() != checked.local()
            || runtime.source() != capture.local()
            || outer.local_type(runtime.source()) != Some(runtime.ty())
            || closure
                .semantics()
                .capture(runtime.capture())
                .is_none_or(|capture| capture.ty() != runtime.ty())
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
    }
    validate_project_function_semantic_catalog(
        modules,
        runtime_owners,
        parent_key,
        callable,
        closure.semantics(),
        Some(outer),
    )
}

fn validate_project_function_call_materialization(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    call: &RuntimeResolvedCall,
) -> Result<(), RuntimeSemanticFactsError> {
    let Some(plan) = call.project_function() else {
        return Ok(());
    };
    let module = module_for(modules, plan.callable().owner().module())?;
    let item = module
        .resolve_item(plan.callable().owner())
        .map_err(|_| RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    let HirItemKind::Function(function) = item.kind() else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    let group = function
        .parameter_groups()
        .get(call.completed_group().get())
        .ok_or(RuntimeSemanticFactsError::InvalidProjectFunctionInstance)?;
    if group.parameters().len() != plan.current_group_materialization().len() {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    for (parameter_index, (hir, runtime)) in group
        .parameters()
        .iter()
        .zip(plan.current_group_materialization())
        .enumerate()
    {
        if runtime.group() != call.completed_group()
            || u32::try_from(parameter_index).ok() != Some(runtime.parameter())
            || runtime.kind() != hir.kind()
            || hir.default().is_some()
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
        if runtime
            .operand_indices()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
        }
        validate_normalized_type(modules, runtime.abi_ty())?;
        validate_normalized_type(modules, runtime.binding_ty())?;
    }
    Ok(())
}

fn validate_project_function_instance_reference(
    expression: ExprId,
    call: &RuntimeResolvedCall,
    expression_type: Option<&RuntimeNormalizedType>,
    instances: &BTreeMap<RuntimeProjectFunctionInstanceKey, RuntimeProjectFunctionInstanceFact>,
) -> Result<Option<RuntimeProjectFunctionInstanceKey>, RuntimeSemanticFactsError> {
    let Some(plan) = call.project_function() else {
        return Ok(None);
    };
    let Some(instance) = plan.outcome().instance() else {
        return Ok(None);
    };
    let Some(fact) = instances.get(instance) else {
        return Err(RuntimeSemanticFactsError::MissingProjectFunctionInstance { expression });
    };
    if fact.callable() != plan.callable()
        || plan
            .input()
            .function_type()
            .is_some_and(|function_type| function_type != fact.function_type())
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let input_prefix_types = plan
        .input()
        .continuation_abi()
        .map_or(&[][..], RuntimeProjectContinuationAbi::prefix_types);
    let instance_prefix_types = fact
        .parameters()
        .iter()
        .filter_map(|parameter| {
            matches!(
                parameter.source(),
                RuntimeProjectFunctionParameterSource::ContinuationPrefix { .. }
            )
            .then_some(parameter.binding_ty())
        })
        .collect::<Vec<_>>();
    let instance_current_group_parameters = fact
        .parameters()
        .iter()
        .filter_map(|parameter| {
            matches!(
                parameter.source(),
                RuntimeProjectFunctionParameterSource::CurrentGroup { .. }
            )
            .then_some(parameter)
        })
        .collect::<Vec<_>>();
    if input_prefix_types.len() != instance_prefix_types.len()
        || input_prefix_types
            .iter()
            .zip(instance_prefix_types)
            .any(|(actual, expected)| actual != expected)
        || plan.current_group_materialization().len() != instance_current_group_parameters.len()
        || plan
            .current_group_materialization()
            .iter()
            .zip(instance_current_group_parameters)
            .any(|(actual, expected)| {
                actual.group() != expected.group()
                    || actual.parameter() != expected.parameter()
                    || actual.kind() != expected.kind()
                    || actual.abi_ty() != expected.abi_ty()
                    || actual.binding_ty() != expected.binding_ty()
            })
    {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    let RuntimeTypeShape::Function { result, .. } = fact.function_type().shape() else {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    };
    if expression_type != Some(result.as_ref()) {
        return Err(RuntimeSemanticFactsError::InvalidProjectFunctionInstance);
    }
    Ok(Some(instance.clone()))
}

fn normalized_tuple_payload_matches(
    payload: &RuntimeNormalizedType,
    expected_fields: &[&RuntimeNormalizedType],
) -> bool {
    let RuntimeTypeShape::Tuple(fields) = payload.shape() else {
        return false;
    };
    fields.len() == expected_fields.len()
        && fields
            .iter()
            .zip(expected_fields)
            .all(|(field, expected)| field == *expected)
}

fn validate_nominal(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    nominal: &RuntimeResolvedNominal,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, nominal.owner())?;
    let valid = matches!(
        (nominal.declaration().kind(), item.kind()),
        (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Struct,
            HirItemKind::Struct(_)
        ) | (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Enum,
            HirItemKind::Enum(_)
        ) | (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::TypeAlias,
            HirItemKind::TypeAlias(_)
        )
    );
    if valid {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: nominal.owner(),
            actual: item.kind().family(),
        })
    }
}

fn validate_nominal_record(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    record: &RuntimeResolvedNominalRecord,
) -> Result<(), RuntimeSemanticFactsError> {
    let nominal = record.nominal();
    let arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Struct =
        nominal.declaration().kind()
    else {
        return Err(RuntimeSemanticFactsError::WrongNominalRecordItemFamily {
            item: nominal.owner(),
        });
    };
    validate_nominal(modules, nominal)
}

fn validate_record_expression_fact(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: ExprId,
    fact: &RuntimeRecordExpressionFact,
) -> Result<(), RuntimeSemanticFactsError> {
    validate_nominal_record(modules, fact.nominal())?;
    let expression = resolve_expr(modules, owner)?;
    let authored = match expression {
        HirExprKind::Record(record) => record.fields(),
        HirExprKind::RecordLiteral(record) => record.fields(),
        _ => {
            return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                expression: owner,
                expected: RuntimeSemanticFactFamily::NominalRecord,
            });
        }
    };
    if authored.len() != fact.fields().len() {
        return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
            expression: owner,
            expected: RuntimeSemanticFactFamily::NominalRecord,
        });
    }
    let sources_match = authored
        .iter()
        .zip(fact.fields())
        .all(|(authored, checked)| {
            matches!(
                (authored, checked.source()),
                (
                    HirRecordField::Explicit { value, .. },
                    RuntimeRecordExpressionSource::Expression(checked)
                ) if *value == checked
            ) || matches!(
                (authored, checked.source()),
                (
                    HirRecordField::Shorthand { local, .. },
                    RuntimeRecordExpressionSource::Binding(checked)
                ) if *local == checked
            )
        });
    sources_match
        .then_some(())
        .ok_or(RuntimeSemanticFactsError::WrongExpressionFamily {
            expression: owner,
            expected: RuntimeSemanticFactFamily::NominalRecord,
        })
}

fn validate_record_pattern_fact(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    owner: PatternId,
    fact: &RuntimeRecordPatternFact,
) -> Result<(), RuntimeSemanticFactsError> {
    match (fact.nominal(), fact.structural()) {
        (Some(nominal), None) => validate_nominal_record(modules, nominal)?,
        (None, Some(structural)) => validate_normalized_type(modules, structural)?,
        _ => {
            return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                pattern: owner,
                expected: RuntimeSemanticFactFamily::PatternNominalRecord,
            });
        }
    }
    let HirPatternKind::Record { fields, .. } = resolve_pattern(modules, owner)? else {
        return Err(RuntimeSemanticFactsError::WrongPatternFamily {
            pattern: owner,
            expected: RuntimeSemanticFactFamily::PatternNominalRecord,
        });
    };
    let mut checked = fact.fields().iter();
    let mut rest = None;
    for authored in fields {
        match authored {
            HirPatternField::Explicit { pattern, .. } => {
                let Some(projected) = checked.next() else {
                    return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                        pattern: owner,
                        expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                    });
                };
                if projected.source() != RuntimeRecordPatternSource::Pattern(*pattern) {
                    return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                        pattern: owner,
                        expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                    });
                }
            }
            HirPatternField::Shorthand { local, .. } => {
                let Some(projected) = checked.next() else {
                    return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                        pattern: owner,
                        expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                    });
                };
                if projected.source() != RuntimeRecordPatternSource::Binding(*local) {
                    return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                        pattern: owner,
                        expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                    });
                }
            }
            HirPatternField::Rest { binding } if rest.is_none() => {
                rest = Some(match binding {
                    Some(local) => RuntimeRecordPatternRest::Binding(*local),
                    None => RuntimeRecordPatternRest::Ignore,
                });
            }
            HirPatternField::Rest { .. } | HirPatternField::Invalid { .. } => {
                return Err(RuntimeSemanticFactsError::WrongPatternFamily {
                    pattern: owner,
                    expected: RuntimeSemanticFactFamily::PatternNominalRecord,
                });
            }
        }
    }
    if checked.next().is_some() || fact.rest() != rest.unwrap_or(RuntimeRecordPatternRest::Absent) {
        return Err(RuntimeSemanticFactsError::WrongPatternFamily {
            pattern: owner,
            expected: RuntimeSemanticFactFamily::PatternNominalRecord,
        });
    }
    Ok(())
}

fn intern_nominal_record_layout(
    record: &mut RuntimeResolvedNominalRecord,
    layouts: &mut BTreeMap<
        (RuntimeNominalTypeId, RuntimeSemanticTypeId, TypeLayoutHash),
        Arc<RuntimeNominalRecordLayout>,
    >,
) -> Result<(), RuntimeSemanticFactsError> {
    let layout = Arc::clone(record.layout());
    let key = (
        layout.nominal().clone(),
        layout.semantic_identity(),
        layout.layout(),
    );
    match layouts.get(&key) {
        Some(previous) if previous.as_ref() != layout.as_ref() => {
            return Err(RuntimeSemanticFactsError::ConflictingNominalRecordLayout {
                nominal: key.0,
                semantic_identity: key.1,
                layout: key.2,
            });
        }
        Some(previous) => record.layout = Arc::clone(previous),
        None => {
            layouts.insert(key, layout);
        }
    }
    Ok(())
}

fn resolve_item<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: ItemId,
) -> Result<&'project arcweft_lang_hir::item::HirItem, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_item(id)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedItem { item: id })
}

fn validate_iterator_witness_method_edges(
    runtime_owners: RuntimeSemanticOwnerSet<'_>,
    statement: StmtId,
    iteration: &RuntimeIteratorFact,
    methods: &BTreeMap<ImplMethodDeclarationId, RuntimeTraitMethodFact>,
) -> Result<(), RuntimeSemanticFactsError> {
    let expected = match iteration {
        RuntimeIteratorFact::Builtin(_) => BTreeMap::new(),
        RuntimeIteratorFact::Witness(witness) => match witness.executable() {
            RuntimeIteratorWitnessExecutableFact::TraitCalls { into_iter, next } => {
                BTreeMap::from([
                    (
                        HirRuntimeIteratorWitnessMethodRole::IntoIterator,
                        into_iter.clone(),
                    ),
                    (
                        HirRuntimeIteratorWitnessMethodRole::IteratorNext,
                        next.clone(),
                    ),
                ])
            }
            RuntimeIteratorWitnessExecutableFact::IdentityIntoIterator { next } => {
                BTreeMap::from([(
                    HirRuntimeIteratorWitnessMethodRole::IteratorNext,
                    next.clone(),
                )])
            }
        },
    };
    let source = HirRuntimeReachabilitySite::Statement(statement);
    let mut actual = BTreeMap::new();
    for edge in runtime_owners.edges_from(source) {
        let HirRuntimeReachabilityEdgeKind::CheckedIteratorWitnessMethod {
            role,
            implementation,
            member,
            method,
        } = edge.kind()
        else {
            continue;
        };
        let row = (*implementation, *member, method.clone());
        if actual
            .insert(*role, row.clone())
            .is_some_and(|existing| existing != row)
        {
            return Err(
                RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge {
                    statement,
                    role: *role,
                },
            );
        }
    }
    if let Some(role) = expected.iter().find_map(|(role, expected_method)| {
        actual
            .get(role)
            .is_none_or(|(_, _, actual_method)| actual_method != expected_method)
            .then_some(*role)
    }) {
        return Err(
            RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge { statement, role },
        );
    }
    if let Some(role) = actual
        .keys()
        .copied()
        .find(|role| !expected.contains_key(role))
    {
        return Err(
            RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge { statement, role },
        );
    }
    for (role, method) in expected {
        let Some((implementation, member, actual_method)) = actual.get(&role) else {
            return Err(
                RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge { statement, role },
            );
        };
        let expected_trait = match role {
            HirRuntimeIteratorWitnessMethodRole::IntoIterator => {
                RuntimeTraitIdentity::StandardIntoIterator
            }
            HirRuntimeIteratorWitnessMethodRole::IteratorNext => {
                RuntimeTraitIdentity::StandardIterator
            }
        };
        if actual_method != &method
            || methods.get(&method).is_none_or(|fact| {
                fact.implementation() != *implementation
                    || fact.member() != *member
                    || fact.trait_identity() != &expected_trait
            })
        {
            return Err(
                RuntimeSemanticFactsError::InvalidIteratorWitnessMethodEdge { statement, role },
            );
        }
    }
    Ok(())
}

fn validate_trait_method(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    method: &RuntimeTraitMethodFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, method.implementation())?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: method.implementation(),
            actual: item.kind().family(),
        });
    };
    let Some(HirImplMember::Function(function)) =
        implementation.members().get(usize::from(method.member()))
    else {
        return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
    };
    let name = function
        .name()
        .resolved()
        .ok_or(RuntimeSemanticFactsError::InvalidTraitMethodIdentity)?;
    if method.declaration().method().as_str() != name.as_str() || function.body().is_none() {
        return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
    }
    match method.trait_identity() {
        RuntimeTraitIdentity::Project(trait_item) => {
            let trait_owner = *trait_item;
            let trait_item = resolve_item(modules, trait_owner)?;
            if !matches!(trait_item.kind(), HirItemKind::Trait(_)) {
                return Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: trait_owner,
                    actual: trait_item.kind().family(),
                });
            }
        }
        RuntimeTraitIdentity::StandardIterator | RuntimeTraitIdentity::StandardIntoIterator => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "semantic_facts/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "semantic_facts/variant_selection_tests.rs"]
mod variant_selection_tests;
