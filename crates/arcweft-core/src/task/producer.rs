// Typed Need producer plan, admission journal, publication, and restore authority.
// The selected plan and its launch registry remain together because they share
// canonical TaskSpec, Need/Task identity, restart policy, and publication state.
use super::*;
use crate::entry::RuntimeValueDigest;
use std::collections::BTreeSet;

mod source;
mod start;
pub(crate) use source::RuntimeTaskProducerSourceError;
pub(crate) use source::RuntimeTaskTemplateArguments;
pub use start::RuntimeNeedProducerStart;

/// Source-independent ordinal assigned to one launch of a producer instance.
/// Join uses the zero ordinal; positive `AlwaysStart` candidates remain journal
/// authority and are not exposed as raw constructors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct TaskLaunchOrdinal(u64);

impl TaskLaunchOrdinal {
    pub const JOIN: Self = Self(0);

    pub(crate) const fn try_for_policy(
        policy: TaskPolicy,
        value: u64,
    ) -> Result<Self, TaskIdentityError> {
        match (policy, value) {
            (TaskPolicy::JoinSameKey, 0) | (TaskPolicy::AlwaysStart, 1..) => Ok(Self(value)),
            (TaskPolicy::JoinSameKey, _) => Err(TaskIdentityError::NonZeroJoinOrdinal),
            (TaskPolicy::AlwaysStart, 0) => Err(TaskIdentityError::ZeroAlwaysStartOrdinal),
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

macro_rules! semantic_digest {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

semantic_digest!(NeedProducerContractDigest);
semantic_digest!(NeedProducerSiteDigest);
semantic_digest!(TaskPlanSemanticDigest);
semantic_digest!(RuntimeTypeSemanticDigest);
semantic_digest!(NeedTimeoutContractDigest);

/// Untrusted codec assertion. Even all-zero bytes are ordinary input data;
/// only an already sealed original table can supply the corresponding key.
/// A completed key may be projected into this inert role, never the reverse.
///
/// ```compile_fail
/// use arcweft_core::task::{ExpectedTaskPlanKey, TaskPlanSemanticDigest};
/// fn untrusted(expected: ExpectedTaskPlanKey) -> TaskPlanSemanticDigest {
///     expected.into()
/// }
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ExpectedTaskPlanKey([u8; 32]);

impl ExpectedTaskPlanKey {
    pub(crate) const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub(crate) const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<TaskPlanSemanticDigest> for ExpectedTaskPlanKey {
    fn from(key: TaskPlanSemanticDigest) -> Self {
        Self(*key.as_bytes())
    }
}

impl std::borrow::Borrow<[u8; 32]> for TaskPlanSemanticDigest {
    fn borrow(&self) -> &[u8; 32] {
        self.as_bytes()
    }
}

impl TaskPlanSemanticDigest {
    /// The version-one domain shared by Core and its upper View authority.
    pub const DOMAIN: &'static [u8] = b"arcweft.task.plan-semantic.v1\0";

    pub(crate) fn from_view_authority_transcript(request: ViewTaskPlanDigestRequest<'_>) -> Self {
        request.complete_transcript(|hasher| Self(*hasher.finalize().as_bytes()))
    }
    /// Only the semantic owner turns a completed, checked transcript into a key.
    pub(crate) fn from_semantic_transcript(
        transcript: super::semantic::TaskSemanticEncoder<'_>,
    ) -> Result<Self, super::semantic::TaskSemanticEncodingError> {
        Ok(Self(*transcript.finish()?.as_bytes()))
    }
}

/// Typed operation selected by one callable contract that produces a
/// `Need<Result<_, _>>` for a host asset.
///
/// The operation is an execution identity. The request's payload identity is
/// still derived from the selected callable's instantiated result type.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum NeedProducerOperation {
    AssetLoad { kind: AssetLoadKind },
}

/// Asset load family retained in the selected producer contract and request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AssetLoadKind {
    Image,
    Voice,
}

impl AssetLoadKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Voice => "voice",
        }
    }
}

/// Closed producer family used by the canonical instance-key transcript.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum NeedProducerFamily {
    StructuredTaskPlan,
    AwbcTaskPlan,
    ViewMatchSubscription,
    AwaitManyBase,
    AwaitManyChild,
    Timeout,
    LineTask,
    HostAdapterTask,
    MakeNeedHandle,
    SelectedCallable,
}

impl NeedProducerFamily {
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::StructuredTaskPlan => 0,
            Self::AwbcTaskPlan => 1,
            Self::ViewMatchSubscription => 2,
            Self::AwaitManyBase => 3,
            Self::AwaitManyChild => 4,
            Self::Timeout => 5,
            Self::LineTask => 6,
            Self::HostAdapterTask => 7,
            Self::MakeNeedHandle => 8,
            Self::SelectedCallable => 9,
        }
    }

    pub const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::StructuredTaskPlan),
            1 => Some(Self::AwbcTaskPlan),
            2 => Some(Self::ViewMatchSubscription),
            3 => Some(Self::AwaitManyBase),
            4 => Some(Self::AwaitManyChild),
            5 => Some(Self::Timeout),
            6 => Some(Self::LineTask),
            7 => Some(Self::HostAdapterTask),
            8 => Some(Self::MakeNeedHandle),
            9 => Some(Self::SelectedCallable),
            _ => None,
        }
    }
}

/// Complete typed producer contract used as the sole source of its instance
/// identity.  The individual semantic fields are intentionally not exposed as
/// an alternate task/Need identity authority.
/// Live producers and inert plan assertions share this field grammar. Only
/// the issued-key specialization can derive a producer instance identity.
///
/// ```compile_fail
/// use arcweft_core::task::NeedProducerSpec;
/// fn untrusted(spec: &NeedProducerSpec<[u8; 32]>) {
///     let _ = spec.instance_key();
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NeedProducerSpec<Plan = TaskPlanSemanticDigest> {
    family: NeedProducerFamily,
    contract: NeedProducerContractDigest,
    plan: Plan,
    producer_site: NeedProducerSiteDigest,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
    // Only Source admission supplies this live declaration solution. A
    // digest-only Serde specification cannot construct an invocation binding.
    #[serde(skip)]
    binding: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
}

impl NeedProducerSpec {
    #[must_use]
    pub const fn new(
        family: NeedProducerFamily,
        contract: NeedProducerContractDigest,
        plan: TaskPlanSemanticDigest,
        producer_site: NeedProducerSiteDigest,
        payload_type: RuntimeTypeSemanticDigest,
        arguments: RuntimeValueDigest,
    ) -> Self {
        Self {
            family,
            contract,
            plan,
            producer_site,
            payload_type,
            arguments,
            binding: None,
        }
    }

    /// Derives the fixed producer-instance identity from this complete spec.
    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        self.instance_key_with_plan(self.plan)
    }

    pub const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
    }
}

impl<Plan> NeedProducerSpec<Plan> {
    fn instance_key_with_plan(
        &self,
        plan: TaskPlanSemanticDigest,
    ) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.need.producer-instance.v1\0");
        hasher.update(&[self.family.semantic_tag()]);
        hasher.update(self.contract.as_bytes());
        hasher.update(plan.as_bytes());
        hasher.update(self.producer_site.as_bytes());
        hasher.update(self.payload_type.as_bytes());
        hasher.update(self.arguments.as_bytes());
        match &self.binding {
            None => {
                hasher.update(&[0]);
            }
            Some(binding) => {
                hasher.update(&[1]);
                hasher.update(b"arcweft.need.producer-invocation.v1\0");
                hasher.update(binding.context().as_bytes());
                let rows = binding.source_rows();
                let count = u64::try_from(rows.len())
                    .map_err(|_| TaskIdentityError::InvocationLengthOverflow)?;
                hasher.update(&count.to_le_bytes());
                for row in rows {
                    let count = u64::try_from(row.len())
                        .map_err(|_| TaskIdentityError::InvocationLengthOverflow)?;
                    hasher.update(&count.to_le_bytes());
                    for effect in row.iter() {
                        let bytes = effect.as_str().as_bytes();
                        let length = u64::try_from(bytes.len())
                            .map_err(|_| TaskIdentityError::InvocationLengthOverflow)?;
                        hasher.update(&length.to_le_bytes());
                        hasher.update(bytes);
                    }
                }
            }
        }
        NeedProducerInstanceKey::try_from_bytes(*hasher.finalize().as_bytes())
    }

    pub(crate) fn binding(
        &self,
    ) -> Option<&crate::program_types::RuntimeFunctionEffectInstantiation> {
        self.binding.as_ref()
    }

    pub const fn family(&self) -> NeedProducerFamily {
        self.family
    }

    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }

    pub const fn producer_site(&self) -> NeedProducerSiteDigest {
        self.producer_site
    }

    pub const fn payload_type(&self) -> RuntimeTypeSemanticDigest {
        self.payload_type
    }

    pub const fn arguments(&self) -> RuntimeValueDigest {
        self.arguments
    }
}

impl NeedProducerSpec<crate::plan::task_source::RuntimeTaskSource> {
    pub const fn source(&self) -> &crate::plan::task_source::RuntimeTaskSource {
        &self.plan
    }

    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        self.instance_key_with_plan(self.plan.task_plan().key())
    }

    /// The retained lease proves admission; its pointer is not an instance
    /// transcript field. A joined launch keeps its already accepted lease.
    pub(crate) fn same_instance_contract(&self, other: &Self) -> bool {
        self.family == other.family
            && self.contract == other.contract
            && self.plan.task_plan().key() == other.plan.task_plan().key()
            && self.producer_site == other.producer_site
            && self.payload_type == other.payload_type
            && self.arguments == other.arguments
            && self.binding == other.binding
    }
}

/// Fixed identity of a complete producer spec.  It has no public raw-byte
/// constructor; only `NeedProducerSpec::instance_key` can issue it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct NeedProducerInstanceKey([u8; 32]);

impl NeedProducerInstanceKey {
    pub(crate) fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, TaskIdentityError> {
        if bytes == [0; 32] {
            Err(TaskIdentityError::Zero {
                kind: TaskIdentityKind::NeedProducerInstance,
            })
        } else {
            Ok(Self(bytes))
        }
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for NeedProducerInstanceKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes = <[u8; 32]>::deserialize(deserializer)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

/// Checked request family selected by the callable or manifest contract.
/// Runtime arguments are evaluated separately in their checked source order;
/// this projection identifies how those values become one exact Host request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NeedProducerRequestProjection {
    AssetLoad {
        kind: AssetLoadKind,
        argument_name: String,
    },
    ExternCapability {
        capability: HostCapabilityId,
        operation: String,
        contract: crate::step::HostCallContractDigest,
        argument_names: Box<[Option<String>]>,
    },
}

/// Borrowed scalar binding grammar and its canonical request name. This is
/// shared by declared requests and bounded source decoding before allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeedProducerArgumentBinding<'a> {
    Positional,
    Named(&'a str),
    NamedOrPositional(&'a str),
}

impl<'a> NeedProducerArgumentBinding<'a> {
    #[must_use]
    pub fn accepts_source_name(self, actual: Option<&str>) -> bool {
        match self {
            Self::Positional => actual.is_none(),
            Self::Named(expected) => actual == Some(expected),
            Self::NamedOrPositional(expected) => actual.is_none_or(|actual| actual == expected),
        }
    }

    #[must_use]
    pub const fn canonical_name(self) -> Option<&'a str> {
        match self {
            Self::Positional => None,
            Self::Named(name) | Self::NamedOrPositional(name) => Some(name),
        }
    }
}

impl NeedProducerRequestProjection {
    /// Selects one original source role without copying its binding name.
    #[must_use]
    pub fn argument_binding(&self, index: usize) -> Option<NeedProducerArgumentBinding<'_>> {
        match self {
            Self::AssetLoad { argument_name, .. } => (index == 0).then_some(
                NeedProducerArgumentBinding::NamedOrPositional(argument_name),
            ),
            Self::ExternCapability { argument_names, .. } => {
                argument_names.get(index).map(|name| match name {
                    Some(name) => NeedProducerArgumentBinding::Named(name),
                    None => NeedProducerArgumentBinding::Positional,
                })
            }
        }
    }
}

/// One checked, source-ordered argument evaluated for a Need producer call.
/// Names use the canonical request binding after source admission; source
/// order is preserved and spreads are absent from this scalar runtime row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "V: crate::value::InertRuntimeValueStorage",
        deserialize = "V: crate::value::InertRuntimeValueStorage"
    )
)]
pub struct NeedProducerRuntimeArgument<V = RuntimeValue> {
    pub name: Option<String>,
    pub value: V,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum NeedProducerPlanError {
    #[error("external Need producer request has an empty capability or operation")]
    EmptyExternalRequestIdentity,
    #[error("external Need producer contract differs from its selected manifest contract")]
    ExternalContractMismatch,
    #[error("Need producer argument binding count exceeds the version-one plan contract")]
    ArgumentBindingCountOverflow,
    #[error("Need producer argument type count differs from its selected request signature")]
    ArgumentTypeCountMismatch,
    #[error("Need producer request string length exceeds the version-one semantic transcript")]
    RequestStringLengthOverflow,
    #[error("Need producer semantic transcript arithmetic overflow")]
    TranscriptArithmeticOverflow,
    #[error("Need producer semantic work limit exceeded")]
    SemanticWorkLimit,
    #[error("producer transcript owner rejected semantic input")]
    OwnerRejected,
    #[error("Need producer semantic transcript byte limit exceeded")]
    TranscriptByteLimit,
}

/// Complete static selected contract for one host-backed Need producer.
/// This is request/control metadata inside the actual original endpoint. It
/// cannot issue a Task key or live producer; the common seal and Source own
/// those authorities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeedProducerTaskPlan {
    contract: NeedProducerContractDigest,
    site: NeedProducerSiteDigest,
    request: NeedProducerRequestProjection,
    argument_types: Box<[RuntimeSemanticTypeId]>,
    payload_type: RuntimeSemanticTypeId,
    policy: TaskPolicy,
    restart: HostRestartPolicy,
    class: TaskClass,
    priority: TaskPriority,
    cancel_scope: CancelScopeId,
}

impl NeedProducerTaskPlan {
    fn admit_retained_metadata(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata(&(
            self.contract,
            self.site,
            self.payload_type,
            self.policy,
            self.restart,
            &self.class,
            self.priority,
            &self.cancel_scope,
        ))?;
        admission.validate_metadata(&self.argument_types)?;
        match &self.request {
            NeedProducerRequestProjection::AssetLoad {
                kind,
                argument_name,
            } => {
                admission.validate_metadata(&(kind.as_str(), argument_name))?;
            }
            NeedProducerRequestProjection::ExternCapability {
                capability,
                operation,
                contract,
                argument_names,
            } => {
                admission.validate_metadata(&(capability, operation, contract, argument_names))?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        contract: NeedProducerContractDigest,
        site: NeedProducerSiteDigest,
        request: NeedProducerRequestProjection,
        argument_types: Box<[RuntimeSemanticTypeId]>,
        payload_type: RuntimeSemanticTypeId,
        policy: TaskPolicy,
        restart: HostRestartPolicy,
        class: TaskClass,
        priority: TaskPriority,
        cancel_scope: CancelScopeId,
    ) -> Result<Self, NeedProducerPlanError> {
        if let NeedProducerRequestProjection::ExternCapability {
            capability,
            operation,
            contract: request_contract,
            argument_names,
        } = &request
        {
            if capability.0.is_empty() || operation.is_empty() {
                return Err(NeedProducerPlanError::EmptyExternalRequestIdentity);
            }
            if request_contract.as_bytes() != contract.as_bytes() {
                return Err(NeedProducerPlanError::ExternalContractMismatch);
            }
            if argument_names.iter().flatten().any(String::is_empty) {
                return Err(NeedProducerPlanError::EmptyExternalRequestIdentity);
            }
            u32::try_from(argument_names.len())
                .map_err(|_| NeedProducerPlanError::ArgumentBindingCountOverflow)?;
        }
        if let NeedProducerRequestProjection::AssetLoad { argument_name, .. } = &request
            && argument_name.is_empty()
        {
            return Err(NeedProducerPlanError::EmptyExternalRequestIdentity);
        }
        let expected_argument_count = match &request {
            NeedProducerRequestProjection::AssetLoad { .. } => 1,
            NeedProducerRequestProjection::ExternCapability { argument_names, .. } => {
                argument_names.len()
            }
        };
        if argument_types.len() != expected_argument_count {
            return Err(NeedProducerPlanError::ArgumentTypeCountMismatch);
        }
        Ok(Self {
            contract,
            site,
            request,
            argument_types,
            payload_type,
            policy,
            restart,
            class,
            priority,
            cancel_scope,
        })
    }

    #[must_use]
    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }

    #[must_use]
    pub const fn site(&self) -> NeedProducerSiteDigest {
        self.site
    }

    #[must_use]
    pub const fn request(&self) -> &NeedProducerRequestProjection {
        &self.request
    }

    #[must_use]
    pub fn argument_types(&self) -> &[RuntimeSemanticTypeId] {
        &self.argument_types
    }

    #[must_use]
    pub const fn payload_type(&self) -> RuntimeSemanticTypeId {
        self.payload_type
    }

    #[must_use]
    pub const fn policy(&self) -> TaskPolicy {
        self.policy
    }

    #[must_use]
    pub const fn restart(&self) -> HostRestartPolicy {
        self.restart
    }

    #[must_use]
    pub const fn class(&self) -> &TaskClass {
        &self.class
    }

    #[must_use]
    pub const fn priority(&self) -> TaskPriority {
        self.priority
    }

    #[must_use]
    pub const fn cancel_scope(&self) -> &CancelScopeId {
        &self.cancel_scope
    }

    #[must_use]
    pub fn argument_name(&self, index: usize) -> Option<Option<&str>> {
        self.request
            .argument_binding(index)
            .map(NeedProducerArgumentBinding::canonical_name)
    }

    #[must_use]
    pub fn argument_count(&self) -> usize {
        match &self.request {
            NeedProducerRequestProjection::AssetLoad { .. } => 1,
            NeedProducerRequestProjection::ExternCapability { argument_names, .. } => {
                argument_names.len()
            }
        }
    }

    pub fn project_request(
        &self,
        arguments: &[NeedProducerRuntimeArgument],
    ) -> Result<HostTaskRequest, NeedProducerRequestError> {
        match &self.request {
            NeedProducerRequestProjection::AssetLoad {
                kind,
                argument_name,
            } => {
                if arguments.len() != 1
                    || !NeedProducerArgumentBinding::NamedOrPositional(argument_name)
                        .accepts_source_name(arguments[0].name.as_deref())
                {
                    return Err(NeedProducerRequestError::ArgumentBindingMismatch);
                }
                let RuntimeValue::EntityRef(reference) = &arguments[0].value else {
                    return Err(NeedProducerRequestError::ExpectedEntityReference);
                };
                Ok(HostTaskRequest::AssetLoad(AssetRequest {
                    id: reference.runtime_label(),
                    kind: kind.as_str().to_owned(),
                }))
            }
            NeedProducerRequestProjection::ExternCapability {
                capability,
                operation,
                contract,
                argument_names,
            } => {
                if argument_names.len() != arguments.len()
                    || arguments.iter().zip(argument_names.iter()).any(
                        |(argument, expected)| match (&argument.name, expected) {
                            (None, _) => false,
                            (Some(actual), Some(expected)) => actual != expected,
                            (Some(_), None) => true,
                        },
                    )
                {
                    return Err(NeedProducerRequestError::ArgumentBindingMismatch);
                }
                Ok(
                    HostTaskRequest::custom_with_named_args_and_manifest_contract(
                        capability.0.clone(),
                        operation.clone(),
                        arguments
                            .iter()
                            .filter(|argument| argument.name.is_none())
                            .map(|argument| RuntimePayload::new(argument.value.clone())),
                        arguments.iter().filter_map(|argument| {
                            Some((
                                argument.name.clone()?,
                                RuntimePayload::new(argument.value.clone()),
                            ))
                        }),
                        *contract,
                    ),
                )
            }
        }
    }

    /// Checks a saved host request against the selected arguments without
    /// constructing a second RuntimeValue owner during snapshot validation.
    fn request_matches<V: crate::value::RuntimeStoredValue, P: crate::value::RuntimeStoredValue>(
        &self,
        arguments: &[NeedProducerRuntimeArgument<V>],
        actual: &HostTaskRequest<P>,
    ) -> Result<bool, NeedProducerRequestError> {
        match (&self.request, actual) {
            (
                NeedProducerRequestProjection::AssetLoad {
                    kind,
                    argument_name,
                },
                HostTaskRequest::AssetLoad(saved),
            ) => {
                if arguments.len() != 1
                    || !NeedProducerArgumentBinding::NamedOrPositional(argument_name)
                        .accepts_source_name(arguments[0].name.as_deref())
                {
                    return Err(NeedProducerRequestError::ArgumentBindingMismatch);
                }
                let crate::value::RuntimeValueView::Scalar(
                    crate::value::RuntimeScalarView::EntityRef(reference),
                ) = arguments[0].value.borrowed_view()
                else {
                    return Err(NeedProducerRequestError::ExpectedEntityReference);
                };
                Ok(saved.id == reference.runtime_label() && saved.kind == kind.as_str())
            }
            (
                NeedProducerRequestProjection::ExternCapability {
                    capability,
                    operation,
                    contract,
                    argument_names,
                },
                HostTaskRequest::Custom {
                    capability: saved_capability,
                    operation: saved_operation,
                    args,
                    named_args,
                    manifest_contract,
                },
            ) => {
                if argument_names.len() != arguments.len()
                    || arguments.iter().zip(argument_names.iter()).any(
                        |(argument, expected)| match (&argument.name, expected) {
                            (None, _) => false,
                            (Some(actual), Some(expected)) => actual != expected,
                            (Some(_), None) => true,
                        },
                    )
                {
                    return Err(NeedProducerRequestError::ArgumentBindingMismatch);
                }
                let positional = arguments.iter().filter(|argument| argument.name.is_none());
                let named = arguments.iter().filter(|argument| argument.name.is_some());
                Ok(saved_capability == capability
                    && saved_operation == operation
                    && *manifest_contract == Some(*contract)
                    && args.len() == positional.clone().count()
                    && named_args.len() == named.clone().count()
                    && positional.zip(args).all(|(argument, saved)| {
                        crate::value::runtime_value_views_equal(
                            argument.value.borrowed_view(),
                            saved.borrowed_view(),
                        )
                    })
                    && named.zip(named_args).all(|(argument, saved)| {
                        argument.name.as_deref() == Some(saved.name.as_str())
                            && crate::value::runtime_value_views_equal(
                                argument.value.borrowed_view(),
                                saved.value.borrowed_view(),
                            )
                    }))
            }
            _ => Ok(false),
        }
    }

    #[must_use]
    pub const fn outcome(&self) -> TaskOutcomeContract {
        TaskOutcomeContract::program(self.payload_type)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum NeedProducerRequestError {
    #[error("Need producer arguments do not match the selected request binding")]
    ArgumentBindingMismatch,
    #[error("selected AssetLoad producer requires an EntityRef argument")]
    ExpectedEntityReference,
}

impl NeedProducerTaskPlan {
    /// Inert endpoint metadata for the original source's control contract.
    /// This excludes task keys and issues no completed semantic proof.
    pub(crate) fn encode_control_metadata(
        &self,
        encoder: &mut super::semantic::TaskSemanticEncoder<'_>,
    ) {
        encoder.digest(self.contract.as_bytes());
        encoder.digest(self.site.as_bytes());
        encoder.digest(self.payload_type.as_bytes());
        encoder.tag(self.class.semantic_tag());
        encoder.ordinal(self.priority.0.cast_unsigned());
        encoder.string(&self.cancel_scope.0);
        encoder.tag(self.policy.semantic_tag());
        self.encode_request_schema(encoder);
        if let NeedProducerRequestProjection::ExternCapability { contract, .. } = &self.request {
            encoder.digest(contract.as_bytes());
        }
    }

    fn encode_request_schema(&self, encoder: &mut super::semantic::TaskSemanticEncoder<'_>) {
        match &self.request {
            NeedProducerRequestProjection::AssetLoad {
                kind,
                argument_name,
            } => {
                encoder.tag(0);
                encoder.tag(match kind {
                    AssetLoadKind::Image => 0,
                    AssetLoadKind::Voice => 1,
                });
                encoder.string(argument_name);
            }
            NeedProducerRequestProjection::ExternCapability {
                capability,
                operation,
                argument_names,
                ..
            } => {
                encoder.tag(1);
                encoder.string(&capability.0);
                encoder.string(operation);
                encoder.count(argument_names.len());
                for name in argument_names.iter() {
                    if encoder.status().is_err() {
                        return;
                    }
                    encoder.enter_element();
                    encoder.enter_role();
                    match name {
                        Some(name) => {
                            encoder.tag(1);
                            encoder.string(name);
                        }
                        None => {
                            encoder.tag(0);
                        }
                    }
                }
            }
        }
        encoder.count(self.argument_types.len());
        for argument_type in &self.argument_types {
            if encoder.status().is_err() {
                return;
            }
            encoder.enter_element();
            encoder.enter_role();
            encoder.digest(argument_type.as_bytes());
        }
        encoder.tag(match self.restart {
            HostRestartPolicy::MustBeQuiescent => 0,
            HostRestartPolicy::Restartable => 1,
        });
    }
}

impl TaskClass {
    pub(crate) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::LocalView),
            1 => Some(Self::Io),
            2 => Some(Self::Cpu),
            3 => Some(Self::GpuPrepare),
            4 => Some(Self::ShaderCompile),
            5 => Some(Self::WasmCall),
            6 => Some(Self::AssetDecode),
            7 => Some(Self::AudioDecode),
            8 => Some(Self::AudioRender),
            9 => Some(Self::TtsSynthesis),
            10 => Some(Self::BgmPrecompose),
            11 => Some(Self::Lsp),
            12 => Some(Self::Background),
            _ => None,
        }
    }

    /// Canonical version-one class tag in the shared task-plan transcript.
    #[must_use]
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::LocalView => 0,
            Self::Io => 1,
            Self::Cpu => 2,
            Self::GpuPrepare => 3,
            Self::ShaderCompile => 4,
            Self::WasmCall => 5,
            Self::AssetDecode => 6,
            Self::AudioDecode => 7,
            Self::AudioRender => 8,
            Self::TtsSynthesis => 9,
            Self::BgmPrecompose => 10,
            Self::Lsp => 11,
            Self::Background => 12,
        }
    }
}

fn need_producer_arguments_digest<'value, V: crate::value::RuntimeStoredValue + 'value>(
    arguments: impl Iterator<Item = &'value V>,
    max_encoded_bytes: usize,
) -> Result<RuntimeValueDigest, crate::entry::RuntimeSchemaError> {
    let values: Vec<_> = arguments.map(V::borrowed_view).collect();
    crate::entry::schema::canonical_runtime_value_view_digest(
        crate::value::RuntimeValueView::Tuple(crate::value::RuntimeTupleView::Views(&values)),
        max_encoded_bytes,
    )
}

/// Opaque, registry-issued identity of one accepted producer-start
/// instruction. The registry journals these before issuing a task so replaying
/// the same accepted instruction returns the original launch, while a later
/// visit to the same source site gets a new token.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct NeedProducerInvocationToken {
    generation: GenerationId,
    fiber: RuntimePersistentFiberId,
    producer_site: NeedProducerSiteDigest,
    sequence: u64,
}

impl NeedProducerInvocationToken {
    #[must_use]
    pub const fn generation(self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn fiber(self) -> RuntimePersistentFiberId {
        self.fiber
    }

    #[must_use]
    pub const fn producer_site(self) -> NeedProducerSiteDigest {
        self.producer_site
    }

    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// One admitted typed producer launch. Its Need and task identities are
/// derived together from the generation, the complete producer instance key,
/// and one launch ordinal. The exact task specification is retained so native
/// and Product execution share the same request and outcome authority.
#[derive(Debug, PartialEq)]
pub struct RuntimeNeedProducerLaunch {
    correlation: TaskCorrelation,
    origin: RuntimeTaskLaunchOrigin,
    state: RuntimeNeedProducerState,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
}

#[derive(Debug, PartialEq)]
enum RuntimeTaskLaunchOrigin {
    Need {
        invocation: NeedProducerInvocationToken,
        start: RuntimeNeedProducerStart,
    },
    Program {
        launch: super::ProgramTaskLaunch,
        retirement: super::ProgramTaskRetirementPhase,
    },
}

impl RuntimeNeedProducerLaunch {
    fn program_terminal(&self) -> Option<super::ProgramTaskTerminal> {
        if !self.task_terminal || !matches!(self.origin, RuntimeTaskLaunchOrigin::Program { .. }) {
            return None;
        }
        Some(match &self.state {
            RuntimeNeedProducerState::Ready(_) | RuntimeNeedProducerState::ReadyTransferred => {
                super::ProgramTaskTerminal::Completed
            }
            RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                super::ProgramTaskTerminal::Failed(failure.clone())
            }
            RuntimeNeedProducerState::Cancelled => super::ProgramTaskTerminal::Cancelled,
            RuntimeNeedProducerState::NotStarted | RuntimeNeedProducerState::Pending(_) => {
                return None;
            }
        })
    }

    fn need_start(&self) -> Option<&RuntimeNeedProducerStart> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => Some(start),
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    fn ready_origin(&self) -> Option<NeedProducerReadyOrigin> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { .. } => {
                self.publication.map(NeedProducerReadyOrigin::Host)
            }
            RuntimeTaskLaunchOrigin::Program { .. } if self.task_terminal => {
                Some(NeedProducerReadyOrigin::Program)
            }
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    pub fn execution_kind(&self) -> super::TaskExecutionKind {
        self.task_spec().request.kind()
    }
    pub const fn correlation(&self) -> TaskCorrelation {
        self.correlation
    }
    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.correlation.generation
    }

    #[must_use]
    pub const fn invocation(&self) -> Option<NeedProducerInvocationToken> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { invocation, .. } => Some(*invocation),
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    #[must_use]
    pub const fn producer(&self) -> Option<&NeedProducerSpec<crate::plan::RuntimeTaskSource>> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => Some(&start.producer),
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    #[must_use]
    pub fn source(&self) -> &crate::plan::RuntimeTaskSource {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => start.source(),
            RuntimeTaskLaunchOrigin::Program { launch, .. } => launch.source(),
        }
    }

    pub fn payload_type(&self) -> RuntimeSemanticTypeId {
        match self.task_spec().outcome {
            TaskOutcomeContract::Program { payload } => payload,
            TaskOutcomeContract::Standalone { .. } => {
                unreachable!("a selected original Source has a program-owned Ready contract")
            }
        }
    }

    #[must_use]
    pub fn arguments(&self) -> Option<&[NeedProducerRuntimeArgument]> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => Some(&start.arguments),
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    #[must_use]
    pub const fn instance_key(&self) -> NeedProducerInstanceKey {
        self.correlation.producer
    }

    #[must_use]
    pub const fn ordinal(&self) -> TaskLaunchOrdinal {
        self.correlation.launch_ordinal
    }

    #[must_use]
    pub const fn need(&self) -> &NeedId {
        &self.correlation.need
    }

    #[must_use]
    pub const fn task(&self) -> &TaskId {
        &self.correlation.task_id
    }

    #[must_use]
    pub fn task_spec(&self) -> &TaskSpec {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => &start.task_spec,
            RuntimeTaskLaunchOrigin::Program { launch, .. } => launch.spec(),
        }
    }

    #[must_use]
    pub const fn restart(&self) -> Option<HostRestartPolicy> {
        match &self.origin {
            RuntimeTaskLaunchOrigin::Need { start, .. } => Some(start.restart),
            RuntimeTaskLaunchOrigin::Program { .. } => None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &RuntimeNeedProducerState {
        &self.state
    }

    #[must_use]
    pub const fn publication(&self) -> Option<TaskPublicationCursor> {
        self.publication
    }

    #[must_use]
    pub const fn task_submitted(&self) -> bool {
        self.task_submitted
    }

    #[must_use]
    pub const fn task_terminal(&self) -> bool {
        self.task_terminal
    }

    #[must_use]
    pub fn task_fault(&self) -> Option<&super::RuntimeTaskFailure> {
        match &self.state {
            RuntimeNeedProducerState::InfrastructureFailure(failure) => Some(failure),
            _ => None,
        }
    }
}

/// The producer's terminal Ready metadata remains after an AlwaysStart
/// payload moves to its one Await consumer. JoinSameKey retains a checked
/// unrestricted Ready payload for later joiners.
/// Retained Ready owns its payload until it moves to the selected waiter.
/// Only the inert storage specialization supports cloning and decoding.
///
/// ```compile_fail
/// use arcweft_core::task::RuntimeNeedProducerState;
/// fn clone_live(value: &RuntimeNeedProducerState) -> RuntimeNeedProducerState {
///     value.clone()
/// }
/// ```
///
/// ```compile_fail
/// use arcweft_core::task::RuntimeNeedProducerState;
/// fn decode_live<'a, D: serde::Deserializer<'a>>(decoder: D) {
///     let _ = <RuntimeNeedProducerState as serde::Deserialize>::deserialize(decoder);
/// }
/// ```
#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(
        serialize = "P: Serialize",
        deserialize = "P: crate::value::InertRuntimeValueStorage"
    )
)]
pub enum RuntimeNeedProducerState<P = RuntimePayload> {
    NotStarted,
    Pending(Progress),
    Ready(P),
    ReadyTransferred,
    InfrastructureFailure(super::RuntimeTaskFailure),
    Cancelled,
}

impl<P> RuntimeNeedProducerState<P> {
    pub(crate) fn admit_retained<Q, S>(
        &self,
        spec: &TaskSpec<Q, TaskPlanSemanticDigest, S>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        P: crate::value::RuntimeStoredValue,
    {
        match self {
            Self::Ready(value) => {
                admission.admit_composite(P::admission_origin(), |owner, resources| {
                    let view = value.borrowed_view();
                    if spec.policy == TaskPolicy::JoinSameKey {
                        view.admit_ownership(
                            crate::value::ownership::RuntimeValueOwnershipRequirement::Unrestricted,
                            resources,
                            0,
                        )?;
                    } else {
                        resources.validate_view(view, 0, None)?;
                    }
                    crate::value::AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                        view, owner, resources,
                    )?;
                    spec.validate_admitted_value_view(view, owner, resources)?;
                    Ok(())
                })
            }
            Self::Pending(progress) => admission.validate_metadata(progress),
            Self::InfrastructureFailure(failure) => admission.validate_metadata(failure),
            Self::NotStarted | Self::ReadyTransferred | Self::Cancelled => Ok(()),
        }
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Ready(_)
                | Self::ReadyTransferred
                | Self::InfrastructureFailure(_)
                | Self::Cancelled
        )
    }
}

impl Clone for RuntimeNeedProducerState<crate::value::AwbcRuntimeValueSnapshot> {
    fn clone(&self) -> Self {
        match self {
            Self::NotStarted => Self::NotStarted,
            Self::Pending(progress) => Self::Pending(progress.clone()),
            Self::Ready(value) => Self::Ready(value.clone()),
            Self::ReadyTransferred => Self::ReadyTransferred,
            Self::InfrastructureFailure(failure) => Self::InfrastructureFailure(failure.clone()),
            Self::Cancelled => Self::Cancelled,
        }
    }
}

impl RuntimeNeedProducerState<crate::value::AwbcRuntimeValueSnapshot> {
    fn from_admitted_live(
        state: &RuntimeNeedProducerState,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match state {
            RuntimeNeedProducerState::NotStarted => Self::NotStarted,
            RuntimeNeedProducerState::Pending(progress) => Self::Pending(progress.clone()),
            RuntimeNeedProducerState::Ready(value) => Self::Ready(
                crate::value::AwbcRuntimeValueSnapshot::from_admitted_runtime_value(
                    value.value(),
                    owner,
                )?,
            ),
            RuntimeNeedProducerState::ReadyTransferred => Self::ReadyTransferred,
            RuntimeNeedProducerState::InfrastructureFailure(failure) => {
                Self::InfrastructureFailure(failure.clone())
            }
            RuntimeNeedProducerState::Cancelled => Self::Cancelled,
        })
    }

    fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeNeedProducerState, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(match self {
            Self::NotStarted => RuntimeNeedProducerState::NotStarted,
            Self::Pending(progress) => RuntimeNeedProducerState::Pending(progress),
            Self::Ready(value) => RuntimeNeedProducerState::Ready(RuntimePayload(
                value.into_admitted_runtime_value(owner)?,
            )),
            Self::ReadyTransferred => RuntimeNeedProducerState::ReadyTransferred,
            Self::InfrastructureFailure(failure) => {
                RuntimeNeedProducerState::InfrastructureFailure(failure)
            }
            Self::Cancelled => RuntimeNeedProducerState::Cancelled,
        })
    }
}

/// Whether an admission must emit a host task request. Rejoining an active or
/// terminal Need does not submit the task again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeedProducerTaskDisposition {
    Ensure,
    Reuse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerAdmission {
    correlation: TaskCorrelation,
    task_spec: TaskSpec,
    invocation: NeedProducerInvocationToken,
    disposition: NeedProducerTaskDisposition,
}

impl NeedProducerAdmission {
    #[must_use]
    pub const fn need(&self) -> &NeedId {
        &self.correlation.need
    }

    pub const fn correlation(&self) -> TaskCorrelation {
        self.correlation
    }

    #[must_use]
    pub const fn task_spec(&self) -> &TaskSpec {
        &self.task_spec
    }

    #[must_use]
    pub const fn disposition(&self) -> NeedProducerTaskDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn invocation(&self) -> NeedProducerInvocationToken {
        self.invocation
    }
}

/// Sealed, metadata-only start decision. The caller can inspect the Need and
/// task request before committing any registry mutation or moving an affine
/// value elsewhere in the runtime.
pub struct NeedProducerStartProof {
    frontier: (
        GenerationId,
        RuntimePersistentFiberId,
        NeedProducerSiteDigest,
    ),
    observed_sequence: u64,
    next_sequence: u64,
    admission: NeedProducerAdmission,
    decision: NeedProducerStartDecision,
}

impl NeedProducerStartProof {
    #[must_use]
    pub const fn admission(&self) -> &NeedProducerAdmission {
        &self.admission
    }
}

enum NeedProducerStartDecision {
    Replay {
        key: NeedProducerLaunchKey,
        mark_submitted: bool,
    },
    Join {
        key: NeedProducerLaunchKey,
        invocation: NeedProducerInvocationToken,
        mark_submitted: bool,
    },
    New {
        key: NeedProducerLaunchKey,
        launch: RuntimeNeedProducerLaunch,
        invocation: NeedProducerInvocationToken,
    },
}

#[derive(Debug, Default, PartialEq)]
pub struct NeedProducerRegistry {
    launches: BTreeMap<NeedProducerLaunchKey, RuntimeNeedProducerLaunch>,
    joined: BTreeMap<(GenerationId, NeedProducerInstanceKey), NeedProducerLaunchKey>,
    invocation_launches: BTreeMap<NeedProducerInvocationToken, NeedProducerLaunchKey>,
    task_journal: super::TaskAdmissionJournal,
    next_invocation_sequence: BTreeMap<
        (
            GenerationId,
            RuntimePersistentFiberId,
            NeedProducerSiteDigest,
        ),
        u64,
    >,
}

/// Resume reissues restartable work; rollback preserves the accepted host
/// submission frontier because no external execution was undone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NeedProducerRestorePolicy {
    Resume,
    Rollback,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NeedProducerLaunchKey {
    generation: GenerationId,
    instance_key: NeedProducerInstanceKey,
    ordinal: TaskLaunchOrdinal,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum NeedProducerAdmissionError {
    #[error(transparent)]
    Admission(#[from] crate::value::AwbcRuntimeValueSnapshotError),
    #[error("the original task-plan namespace has not completed sealing")]
    UnsealedTaskPlan,
    #[error(transparent)]
    TaskEnsure(#[from] TaskEnsureError),
    #[error("producer instance identity could not be issued: {0}")]
    Identity(#[from] TaskIdentityError),
    #[error(transparent)]
    Plan(#[from] NeedProducerPlanError),
    #[error(transparent)]
    Request(#[from] NeedProducerRequestError),
    #[error("evaluated Need producer arguments have no canonical persistent digest")]
    InvalidProducerArguments,
    #[error("host Need producer arguments must be recursively unrestricted")]
    AffineHostProducerArgument,
    #[error("JoinSameKey Ready must be recursively unrestricted for joined or later observers")]
    AffineJoinedReadyPublication,
    #[error("producer Need has no Ready payload available for this Await")]
    ReadyUnavailable,
    #[error("producer invocation counter is exhausted")]
    InvocationSequenceExhausted,
    #[error("AlwaysStart producer launch ordinal is exhausted")]
    LaunchOrdinalExhausted,
    #[error("producer invocation token does not match the selected producer site")]
    InvocationSiteMismatch,
    #[error("producer invocation token does not match the selected generation")]
    InvocationGenerationMismatch,
    #[error("producer invocation token was reused with a different complete launch contract")]
    InvocationSpecificationConflict,
    #[error("joined producer identity was reused with a different complete launch contract")]
    JoinSpecificationConflict,
    #[error("producer launch identity was reused with a different complete launch contract")]
    LaunchSpecificationConflict,
    #[error("producer launch record is not consistent with its canonical identity")]
    InvalidRestoredLaunch,
    #[error("producer Need state transition is not monotone")]
    InvalidNeedTransition,
    #[error("Need publication source does not match its selected producer authority")]
    InvalidPublicationSource,
    #[error("task event generation does not match the selected Need producer generation")]
    StaleTaskGeneration,
    #[error("task publication revision regressed within its dispatch")]
    StaleTaskPublication,
    #[error("task publication revision was reused with a conflicting payload")]
    ConflictingTaskPublication,
    #[error(transparent)]
    HostReadyOwnership(#[from] RuntimeHostPayloadOwnershipError),
}

/// An owned task publication either transfers its payload to the selected
/// producer or returns the unchanged event to its caller.
#[derive(Debug, PartialEq)]
pub enum NeedProducerOwnedTaskEventDisposition {
    Published,
    Duplicate(TaskEvent),
    NotLocal(TaskEvent),
}

#[derive(Debug, Error, PartialEq)]
#[error("{reason}")]
pub struct NeedProducerOwnedTaskEventError {
    reason: NeedProducerAdmissionError,
    event: TaskEvent,
}

#[derive(Debug, Error, PartialEq)]
#[error("{reason}")]
pub struct NeedProducerReadyRestoreError {
    reason: NeedProducerAdmissionError,
    value: RuntimePayload,
}

pub struct NeedProducerReadyRestoreProof {
    key: NeedProducerLaunchKey,
    origin: NeedProducerReadyOrigin,
}

pub struct NeedProducerReadyTakeProof {
    key: NeedProducerLaunchKey,
    origin: NeedProducerReadyOrigin,
    policy: TaskPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NeedProducerReadyOrigin {
    Host(TaskPublicationCursor),
    Program,
}

pub struct NeedProducerTaskEnsuredProof {
    key: Option<NeedProducerLaunchKey>,
    submission: Option<TaskSubmission>,
}

impl NeedProducerTaskEnsuredProof {
    #[must_use]
    pub const fn task_spec(&self) -> Option<&TaskSpec> {
        match &self.submission {
            Some(submission) => Some(submission.spec()),
            None => None,
        }
    }
}

impl NeedProducerReadyRestoreError {
    pub fn into_parts(self) -> (NeedProducerAdmissionError, RuntimePayload) {
        (self.reason, self.value)
    }
}

impl NeedProducerOwnedTaskEventError {
    pub fn into_parts(self) -> (NeedProducerAdmissionError, TaskEvent) {
        (self.reason, self.event)
    }
}

/// Typed, decoded input for restoring a producer launch. AWBC decodes any
/// embedded runtime values through its owner-aware save-value schema before
/// constructing this value. The registry rebuilds all IDs and the canonical
/// TaskSpec, then compares the saved TaskSpec before accepting it.
#[derive(Debug, PartialEq)]
pub struct NeedProducerLaunchRestore<
    V = RuntimeValue,
    P = RuntimePayload,
    S = crate::plan::RuntimeTaskSource,
> {
    pub origin: NeedProducerLaunchOriginRestore<V, P, S>,
    pub correlation: TaskCorrelation,
    pub state: RuntimeNeedProducerState<P>,
    pub publication: Option<TaskPublicationCursor>,
    pub task_submitted: bool,
    pub task_terminal: bool,
}

#[derive(Debug, PartialEq)]
pub enum NeedProducerLaunchOriginRestore<
    V = RuntimeValue,
    P = RuntimePayload,
    S = crate::plan::RuntimeTaskSource,
> {
    Need {
        invocation: NeedProducerInvocationToken,
        start: RuntimeNeedProducerStart<V, P, S>,
    },
    /// Its accepted specification and Source are the enclosing journal row.
    Program {
        retirement: super::ProgramTaskRetirementPhase,
    },
}

pub(crate) type NeedProducerLaunchSaveSnapshot = NeedProducerLaunchRestore<
    crate::value::AwbcRuntimeValueSnapshot,
    crate::value::AwbcRuntimeValueSnapshot,
>;

impl Clone for NeedProducerLaunchSaveSnapshot {
    fn clone(&self) -> Self {
        Self {
            origin: match &self.origin {
                NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                    NeedProducerLaunchOriginRestore::Need {
                        invocation: *invocation,
                        start: start.clone(),
                    }
                }
                NeedProducerLaunchOriginRestore::Program { retirement } => {
                    NeedProducerLaunchOriginRestore::Program {
                        retirement: *retirement,
                    }
                }
            },
            correlation: self.correlation,
            state: self.state.clone(),
            publication: self.publication,
            task_submitted: self.task_submitted,
            task_terminal: self.task_terminal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeedProducerInvocationFrontier {
    pub generation: GenerationId,
    pub fiber: RuntimePersistentFiberId,
    pub producer_site: NeedProducerSiteDigest,
    pub next_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeedProducerLaunchFrontier {
    pub generation: GenerationId,
    pub instance_key: NeedProducerInstanceKey,
    pub next_ordinal: u64,
}

/// Exact host dispatch information for an active Restartable Need producer.
/// Hosts use this projection to retain the owning program generation and to
/// reconstruct the same dispatch after Product snapshot restore.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedProducerDispatch {
    pub submission: TaskSubmission,
    pub restart: HostRestartPolicy,
    pub publication: Option<TaskPublicationCursor>,
    pub needs_reensure: bool,
}

#[derive(Debug, PartialEq)]
pub struct NeedProducerRegistryRestore<
    V = RuntimeValue,
    P = RuntimePayload,
    S = crate::plan::RuntimeTaskSource,
> {
    pub task_admissions: Vec<TaskSubmission<P, TaskPlanSemanticDigest, S>>,
    pub launches: Vec<NeedProducerLaunchRestore<V, P, S>>,
    pub invocations: Vec<(NeedProducerInvocationToken, TaskCorrelation)>,
    pub invocation_frontiers: Vec<NeedProducerInvocationFrontier>,
    pub launch_frontiers: Vec<NeedProducerLaunchFrontier>,
}

impl<V, P, S> Default for NeedProducerRegistryRestore<V, P, S> {
    fn default() -> Self {
        Self {
            task_admissions: Vec::new(),
            launches: Vec::new(),
            invocations: Vec::new(),
            invocation_frontiers: Vec::new(),
            launch_frontiers: Vec::new(),
        }
    }
}

pub(crate) type NeedProducerRegistrySaveSnapshot = NeedProducerRegistryRestore<
    crate::value::AwbcRuntimeValueSnapshot,
    crate::value::AwbcRuntimeValueSnapshot,
>;

impl Clone for NeedProducerRegistrySaveSnapshot {
    fn clone(&self) -> Self {
        Self {
            task_admissions: self.task_admissions.clone(),
            launches: self.launches.clone(),
            invocations: self.invocations.clone(),
            invocation_frontiers: self.invocation_frontiers.clone(),
            launch_frontiers: self.launch_frontiers.clone(),
        }
    }
}

impl<V, P, S: super::TaskExecutionSource + PartialEq> NeedProducerRegistryRestore<V, P, S> {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
        P: crate::value::RuntimeStoredValue + PartialEq,
    {
        admission.validate_metadata_collection(self.task_admissions.len())?;
        admission.validate_metadata_collection(self.launches.len())?;
        let sources = self
            .launches
            .iter()
            .filter_map(|launch| match &launch.origin {
                NeedProducerLaunchOriginRestore::Need { start, .. } => {
                    Some((launch.correlation.task_id, start.source()))
                }
                NeedProducerLaunchOriginRestore::Program { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        for row in &self.task_admissions {
            if let Some(source) = sources.get(&row.task_id()) {
                admission.with_program_owner(source.program_owner(), |admission| {
                    row.admit_retained(admission)
                })?;
            } else {
                match &row.spec().request {
                    super::TaskRequest::Program(source) => {
                        source.with_owner(admission, |admission| row.admit_retained(admission))?
                    }
                    super::TaskRequest::Host(_) => row.admit_retained(admission)?,
                }
            }
        }
        for launch in &self.launches {
            let spec = self.task_spec(launch.correlation).ok_or_else(|| {
                crate::value::AwbcRuntimeValueSnapshotError::Message {
                    message: "restored task state has no exact accepted journal row".into(),
                }
            })?;
            launch.admit_retained(spec, admission)?;
        }
        admission.validate_metadata(&self.invocations)?;
        admission.validate_metadata_collection(self.invocation_frontiers.len())?;
        for frontier in &self.invocation_frontiers {
            admission.validate_metadata(&(
                frontier.generation,
                frontier.fiber,
                frontier.producer_site,
                frontier.next_sequence,
            ))?;
        }
        admission.validate_metadata_collection(self.launch_frontiers.len())?;
        for frontier in &self.launch_frontiers {
            admission.validate_metadata(&(
                frontier.generation,
                frontier.instance_key,
                frontier.next_ordinal,
            ))?;
        }
        if let Err(error) = NeedProducerRegistry::validate_restore_journal(self) {
            let error = crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: error.to_string(),
            };
            admission.retain_failure(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn task_spec(
        &self,
        correlation: TaskCorrelation,
    ) -> Option<&TaskSpec<P, TaskPlanSemanticDigest, S>> {
        let mut matches = self
            .task_admissions
            .iter()
            .filter(|row| row.task_id() == correlation.task_id);
        let row = matches.next()?;
        (row.handle().correlation == correlation && matches.next().is_none()).then_some(row.spec())
    }
}

impl<V, P, S: super::TaskExecutionSource> NeedProducerLaunchRestore<V, P, S> {
    fn admit_retained(
        &self,
        spec: &TaskSpec<P, TaskPlanSemanticDigest, S>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        V: crate::value::RuntimeStoredValue,
        P: crate::value::RuntimeStoredValue,
    {
        admission.validate_metadata(&(
            self.correlation,
            self.publication,
            self.task_submitted,
            self.task_terminal,
        ))?;
        match &self.origin {
            NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                admission.validate_metadata(invocation)?;
                start.admit_retained(admission)?;
                admission.with_program_owner(start.source().program_owner(), |admission| {
                    self.state.admit_retained(spec, admission)
                })
            }
            NeedProducerLaunchOriginRestore::Program { retirement } => {
                admission.validate_metadata(retirement)?;
                let super::TaskRequest::Program(source) = &spec.request else {
                    return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "program task state refers to a Host journal row".into(),
                    });
                };
                source.with_owner(admission, |admission| {
                    self.state.admit_retained(spec, admission)
                })
            }
        }
    }
}
impl NeedProducerRegistrySaveSnapshot {
    pub(crate) fn visit_saved_value_roots<E>(
        &self,
        visit: &mut impl FnMut(&crate::value::AwbcRuntimeValueSnapshot) -> Result<(), E>,
    ) -> Result<(), E> {
        for row in &self.task_admissions {
            for value in row.spec().request.payloads() {
                visit(value)?;
            }
        }
        for launch in &self.launches {
            if let NeedProducerLaunchOriginRestore::Need { start, .. } = &launch.origin {
                for argument in start.arguments() {
                    visit(&argument.value)?;
                }
                for value in start.task_spec().request.payloads() {
                    visit(value)?;
                }
            }
            if let RuntimeNeedProducerState::Ready(value) = &launch.state {
                visit(value)?;
            }
        }
        Ok(())
    }

    pub(crate) fn into_admitted_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NeedProducerRegistryRestore, crate::value::AwbcRuntimeValueSnapshotError> {
        let sources = self
            .launches
            .iter()
            .filter_map(|launch| match &launch.origin {
                NeedProducerLaunchOriginRestore::Need { start, .. } => {
                    Some((launch.correlation.task_id, start.source()))
                }
                NeedProducerLaunchOriginRestore::Program { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        let task_admissions: Vec<TaskSubmission> = self
            .task_admissions
            .into_iter()
            .map(|row| {
                let (spec, handle) = row.into_parts();
                let original = match &spec.request {
                    super::TaskRequest::Program(source) => Some(source.program_owner()),
                    super::TaskRequest::Host(_) => sources
                        .get(&handle.correlation.task_id)
                        .map(|source| source.program_owner()),
                };
                Ok(TaskSubmission {
                    spec: spec.into_admitted_live(original.as_ref().unwrap_or(owner), None)?,
                    handle,
                })
            })
            .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?;
        let program_owners = task_admissions
            .iter()
            .filter_map(|row: &TaskSubmission| match &row.spec().request {
                super::TaskRequest::Program(source) => {
                    Some((row.task_id(), source.program_owner()))
                }
                super::TaskRequest::Host(_) => None,
            })
            .collect::<BTreeMap<_, _>>();
        Ok(NeedProducerRegistryRestore {
            task_admissions,
            launches: self
                .launches
                .into_iter()
                .map(|launch| {
                    let (origin, original) = match launch.origin {
                        NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                            let original = start.source().program_owner();
                            (NeedProducerLaunchOriginRestore::Need { invocation, start: start.into_admitted_live()? }, original)
                        }
                        NeedProducerLaunchOriginRestore::Program { retirement } => {
                            let original = program_owners.get(&launch.correlation.task_id).ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
                                message: "program task state lost its admitted executable journal row".into(),
                            })?;
                            (NeedProducerLaunchOriginRestore::Program { retirement }, original.clone())
                        }
                    };
                    Ok(NeedProducerLaunchRestore {
                        origin,
                        correlation: launch.correlation,
                        state: launch.state.into_admitted_live(&original)?,
                        publication: launch.publication,
                        task_submitted: launch.task_submitted,
                        task_terminal: launch.task_terminal,
                    })
                })
                .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
            invocations: self.invocations,
            invocation_frontiers: self.invocation_frontiers,
            launch_frontiers: self.launch_frontiers,
        })
    }
}

/// Inert rollback and Product persistence share the owning registry grammar.
pub(crate) type NeedProducerRegistryRollbackImage = NeedProducerRegistrySaveSnapshot;

struct ValidatedRestoreLaunch {
    instance_key: NeedProducerInstanceKey,
    key: NeedProducerLaunchKey,
    invocation_frontier: Option<(
        (
            GenerationId,
            RuntimePersistentFiberId,
            NeedProducerSiteDigest,
        ),
        u64,
    )>,
    launch_next: Option<((GenerationId, NeedProducerInstanceKey), u64)>,
}

fn validate_restore_launch<
    V: crate::value::RuntimeStoredValue,
    P: crate::value::RuntimeStoredValue + PartialEq,
    S: super::TaskExecutionSource + PartialEq,
>(
    restore: &NeedProducerLaunchRestore<V, P, S>,
    saved_spec: &TaskSpec<P, TaskPlanSemanticDigest, S>,
) -> Result<ValidatedRestoreLaunch, NeedProducerAdmissionError> {
    let generation = saved_spec.generation;
    let correlation = saved_spec.correlation(restore.correlation.launch_ordinal)?;
    if restore.correlation != correlation {
        return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
    }
    let instance_key = correlation.producer;
    let invocation_frontier = match &restore.origin {
        NeedProducerLaunchOriginRestore::Need { invocation, start } => {
            if invocation.generation != generation
                || saved_spec.producer != super::NeedProducerInstance::try_from(&start.producer)?
                || invocation.producer_site != start.producer.producer_site()
                || !saved_spec.same_join_contract(&start.task_spec)
                || !restored_need_state_is_valid(
                    &restore.state,
                    restore.publication,
                    restore.task_submitted,
                    restore.task_terminal,
                    start.restart,
                    saved_spec.policy,
                )
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            Some((
                (generation, invocation.fiber, invocation.producer_site),
                invocation
                    .sequence
                    .checked_add(1)
                    .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?,
            ))
        }
        NeedProducerLaunchOriginRestore::Program { retirement } => {
            let terminal_matches = match &restore.state {
                RuntimeNeedProducerState::NotStarted => false,
                RuntimeNeedProducerState::Pending(_) => {
                    !restore.task_terminal
                        && *retirement == super::ProgramTaskRetirementPhase::Executing
                }
                RuntimeNeedProducerState::Ready(_)
                | RuntimeNeedProducerState::ReadyTransferred
                | RuntimeNeedProducerState::InfrastructureFailure(_)
                | RuntimeNeedProducerState::Cancelled => {
                    restore.task_terminal
                        && *retirement != super::ProgramTaskRetirementPhase::Executing
                }
            };
            if !matches!(&saved_spec.request, super::TaskRequest::Program(_))
                || !restore.task_submitted
                || !terminal_matches
                || (restore.publication.is_some()
                    != (*retirement == super::ProgramTaskRetirementPhase::Acknowledged))
                || (matches!(&restore.state, RuntimeNeedProducerState::ReadyTransferred)
                    && saved_spec.policy != TaskPolicy::AlwaysStart)
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            None
        }
    };
    let launch_next = if saved_spec.policy == TaskPolicy::AlwaysStart {
        Some((
            (generation, instance_key),
            restore
                .correlation
                .launch_ordinal
                .get()
                .checked_add(1)
                .ok_or(NeedProducerAdmissionError::LaunchOrdinalExhausted)?,
        ))
    } else {
        None
    };
    Ok(ValidatedRestoreLaunch {
        instance_key,
        key: NeedProducerLaunchKey {
            generation,
            instance_key,
            ordinal: correlation.launch_ordinal,
        },
        invocation_frontier,
        launch_next,
    })
}
impl NeedProducerRegistry {
    pub(crate) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.validate_metadata_collection(self.task_journal.accepted_len())?;
        admission.validate_metadata_collection(self.launches.len())?;
        for (spec, handle) in self.task_journal.borrowed_submissions() {
            if let Some(launch) = self.launch_for_correlation(&handle.correlation) {
                admission.with_program_owner(launch.source().program_owner(), |admission| {
                    spec.admit_retained(admission)
                })?;
            } else {
                spec.admit_retained(admission)?;
            }
            admission.validate_metadata(&handle)?;
        }
        for launch in self.launches.values() {
            admission.validate_metadata(&(
                launch.invocation(),
                launch.correlation,
                launch.publication,
                launch.task_submitted,
                launch.task_terminal,
            ))?;
            match &launch.origin {
                RuntimeTaskLaunchOrigin::Need { start, .. } => start.admit_retained(admission)?,
                RuntimeTaskLaunchOrigin::Program { retirement, .. } => {
                    admission.validate_metadata(retirement)?
                }
            }
            admission.with_program_owner(launch.source().program_owner(), |admission| {
                launch.state.admit_retained(launch.task_spec(), admission)
            })?;
        }
        admission.validate_metadata_collection(self.invocation_launches.len())?;
        for (invocation, launch) in self.invocations() {
            admission.validate_metadata(&(invocation, launch.correlation))?;
        }
        admission.validate_metadata(&self.next_invocation_sequence)?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn snapshot_retained(
        &self,
        owner: &RuntimeProgramOwner,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<NeedProducerRegistrySaveSnapshot, crate::value::AwbcRuntimeValueSnapshotError> {
        self.admit_retained(admission)?;
        self.snapshot_admitted(owner)
    }

    pub(crate) fn snapshot_admitted(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NeedProducerRegistrySaveSnapshot, crate::value::AwbcRuntimeValueSnapshotError> {
        Ok(NeedProducerRegistrySaveSnapshot {
            task_admissions: self
                .task_journal
                .borrowed_submissions()
                .map(|(spec, handle)| {
                    let original = self
                        .launch_for_correlation(&handle.correlation)
                        .map(|launch| launch.source().program_owner());
                    Ok(TaskSubmission {
                        spec: super::TaskSpec::<crate::value::AwbcRuntimeValueSnapshot>::from_admitted_live(
                            spec,
                            original.as_ref().unwrap_or(owner),
                        )?,
                        handle,
                    })
                })
                .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
            launches: self
                .launches
                .values()
                .map(|launch| {
                    let original = launch.source().program_owner();
                    Ok(NeedProducerLaunchSaveSnapshot {
                        origin: match &launch.origin {
                            RuntimeTaskLaunchOrigin::Need { invocation, start } => NeedProducerLaunchOriginRestore::Need {
                                invocation: *invocation,
                                start: start.snapshot_admitted()?,
                            },
                            RuntimeTaskLaunchOrigin::Program { retirement, .. } => NeedProducerLaunchOriginRestore::Program { retirement: *retirement },
                        },
                        correlation: launch.correlation,
                        state: RuntimeNeedProducerState::from_admitted_live(
                            &launch.state,
                            &original,
                        )?,
                        publication: launch.publication,
                        task_submitted: launch.task_submitted,
                        task_terminal: launch.task_terminal,
                    })
                })
                .collect::<Result<_, crate::value::AwbcRuntimeValueSnapshotError>>()?,
            invocations: self
                .invocations()
                .map(|(invocation, launch)| (invocation, launch.correlation()))
                .collect(),
            invocation_frontiers: self.invocation_frontiers(),
            launch_frontiers: self.launch_frontiers(),
        })
    }

    #[cfg(test)]
    pub(crate) fn inert_rollback_image(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NeedProducerRegistryRollbackImage, String> {
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        self.snapshot_retained(owner, &mut admission)
            .map_err(|error| error.to_string())
    }

    #[cfg(test)]
    pub(crate) fn from_rollback_image(
        image: NeedProducerRegistryRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        image
            .admit_retained(
                &mut crate::value::RuntimeRetainedValueAdmission::for_program(
                    owner.clone(),
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                ),
            )
            .map_err(|error| error.to_string())?;
        Self::from_admitted_rollback_image(image, owner)
    }

    pub(crate) fn from_admitted_rollback_image(
        image: NeedProducerRegistryRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let restore = image
            .into_admitted_live(owner)
            .map_err(|error| error.to_string())?;
        let mut registry = Self::default();
        registry
            .restore_admitted_registry_with_policy(restore, NeedProducerRestorePolicy::Rollback)
            .map_err(|error| error.to_string())?;
        Ok(registry)
    }
    /// Begins one checked producer-site visit. Call this inside the same
    /// candidate transaction that admits the start and advances its cursor.
    /// An uncommitted candidate therefore reissues the same token on retry.
    pub fn begin_invocation(
        &mut self,
        generation: GenerationId,
        fiber: RuntimePersistentFiberId,
        producer_site: NeedProducerSiteDigest,
    ) -> Result<NeedProducerInvocationToken, NeedProducerAdmissionError> {
        let key = (generation, fiber, producer_site);
        let sequence = self
            .next_invocation_sequence
            .get(&key)
            .copied()
            .unwrap_or(0);
        let next = sequence
            .checked_add(1)
            .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
        self.next_invocation_sequence.insert(key, next);
        Ok(NeedProducerInvocationToken {
            generation,
            fiber,
            producer_site,
            sequence,
        })
    }

    /// Admits one selected producer plan with its exact evaluated source-order
    /// arguments. The registry derives the producer digest, TaskOutcome, and
    /// typed Host request from the sealed plan; callers cannot supply a
    /// parallel request/outcome identity.
    pub fn admit_start(
        &mut self,
        invocation: NeedProducerInvocationToken,
        start: RuntimeNeedProducerStart,
    ) -> Result<NeedProducerAdmission, NeedProducerAdmissionError> {
        let (admission, decision) = self.prepare_start_decision(invocation, start)?;
        if matches!(&decision, NeedProducerStartDecision::New { .. }) {
            let handle = self.task_journal.ensure_task(admission.task_spec.clone())?;
            assert_eq!(
                handle.correlation, admission.correlation,
                "unchanged preflight must commit the prepared journal receipt"
            );
        }
        self.commit_start_decision(decision);
        Ok(admission)
    }

    /// Prepares one producer visit while preserving the current registry and
    /// every live Ready payload. The proof holds only metadata and host-safe
    /// arguments; binding/quota checks can run before `commit_start_visit`.
    pub fn inspect_start_visit(
        &self,
        fiber: RuntimePersistentFiberId,
        start: RuntimeNeedProducerStart,
    ) -> Result<NeedProducerStartProof, NeedProducerAdmissionError> {
        let generation = start.task_spec.generation;
        let site = start.producer.producer_site();
        let frontier = (generation, fiber, site);
        let observed_sequence = self
            .next_invocation_sequence
            .get(&frontier)
            .copied()
            .unwrap_or(0);
        let next_sequence = observed_sequence
            .checked_add(1)
            .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
        let invocation = NeedProducerInvocationToken {
            generation,
            fiber,
            producer_site: site,
            sequence: observed_sequence,
        };
        let (admission, decision) = self.prepare_start_decision(invocation, start)?;
        Ok(NeedProducerStartProof {
            frontier,
            observed_sequence,
            next_sequence,
            admission,
            decision,
        })
    }

    pub fn commit_start_visit(&mut self, proof: NeedProducerStartProof) -> NeedProducerAdmission {
        self.commit_start_visit_with(proof, |_| Ok::<_, std::convert::Infallible>(()))
            .expect("infallible metadata publication")
            .0
    }

    pub(crate) fn commit_start_visit_with<T, E>(
        &mut self,
        proof: NeedProducerStartProof,
        publish: impl FnOnce(super::RuntimeNeedHandle) -> Result<T, E>,
    ) -> Result<(NeedProducerAdmission, T), E> {
        assert_eq!(
            self.next_invocation_sequence
                .get(&proof.frontier)
                .copied()
                .unwrap_or(0),
            proof.observed_sequence,
            "prepared Need producer visit requires an unchanged invocation frontier"
        );
        let task_spec = proof.admission.task_spec.clone();
        let ordinal = match &proof.decision {
            NeedProducerStartDecision::New { launch, .. } => launch.correlation.launch_ordinal,
            NeedProducerStartDecision::Replay { key, .. }
            | NeedProducerStartDecision::Join { key, .. } => key.ordinal,
        };
        let correlation = task_spec
            .correlation(ordinal)
            .expect("start preflight checked correlation");
        let value = match &proof.decision {
            NeedProducerStartDecision::New { .. } => {
                let prepared = self
                    .task_journal
                    .inspect_task(task_spec.clone())
                    .expect("unchanged start frontier preserves journal admission");
                assert_eq!(prepared.correlation(), correlation);
                self.task_journal.with_admission(prepared, |handle| {
                    let need =
                        super::RuntimeNeedHandle::try_from_accepted_launch(task_spec, handle)
                            .expect("actual accepted receipt matches the prepared specification");
                    publish(need)
                })?
            }
            _ => {
                let handle = TaskHandle { correlation };
                assert!(self.task_journal.accepted_spec(handle).is_some());
                publish(
                    super::RuntimeNeedHandle::try_from_accepted_launch(task_spec, handle)
                        .expect("existing journal receipt matches its complete specification"),
                )?
            }
        };
        self.commit_start_decision(proof.decision);
        self.next_invocation_sequence
            .insert(proof.frontier, proof.next_sequence);
        Ok((proof.admission, value))
    }

    fn prepare_start_decision(
        &self,
        invocation: NeedProducerInvocationToken,
        start: RuntimeNeedProducerStart,
    ) -> Result<(NeedProducerAdmission, NeedProducerStartDecision), NeedProducerAdmissionError>
    {
        let producer = &start.producer;
        let arguments = &start.arguments;
        let task_spec = &start.task_spec;
        let restart = start.restart;
        if invocation.producer_site != producer.producer_site() {
            return Err(NeedProducerAdmissionError::InvocationSiteMismatch);
        }
        let generation = invocation.generation;
        if generation != task_spec.generation {
            return Err(NeedProducerAdmissionError::InvocationGenerationMismatch);
        }
        let policy = task_spec.policy;
        let instance_key = producer.instance_key()?;

        if let Some(key) = self.invocation_launches.get(&invocation).copied() {
            let existing = self
                .launches
                .get(&key)
                .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
            if !launch_matches_start(
                existing,
                generation,
                &task_spec,
                &producer,
                instance_key,
                &arguments,
                restart,
            ) {
                return Err(NeedProducerAdmissionError::InvocationSpecificationConflict);
            }
            let disposition = admission_disposition(existing);
            return Ok((
                NeedProducerAdmission {
                    correlation: existing.correlation,
                    task_spec: existing.task_spec().clone(),
                    invocation,
                    disposition,
                },
                NeedProducerStartDecision::Replay {
                    key,
                    mark_submitted: disposition == NeedProducerTaskDisposition::Ensure,
                },
            ));
        }

        if policy == TaskPolicy::JoinSameKey {
            if let Some(key) = self.joined.get(&(generation, instance_key)).copied() {
                let existing = self
                    .launches
                    .get(&key)
                    .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
                if !launch_matches_start(
                    existing,
                    generation,
                    &task_spec,
                    &producer,
                    instance_key,
                    &arguments,
                    restart,
                ) {
                    return Err(NeedProducerAdmissionError::JoinSpecificationConflict);
                }
                let disposition = admission_disposition(existing);
                return Ok((
                    NeedProducerAdmission {
                        correlation: existing.correlation,
                        task_spec: existing.task_spec().clone(),
                        invocation,
                        disposition,
                    },
                    NeedProducerStartDecision::Join {
                        key,
                        invocation,
                        mark_submitted: disposition == NeedProducerTaskDisposition::Ensure,
                    },
                ));
            }
        }

        let task_admission = self.task_journal.inspect_task(task_spec.clone())?;
        let correlation = task_admission.correlation();
        let ordinal = correlation.launch_ordinal;
        let key = NeedProducerLaunchKey {
            generation,
            instance_key,
            ordinal,
        };
        let launch = RuntimeNeedProducerLaunch {
            correlation,
            origin: RuntimeTaskLaunchOrigin::Need { invocation, start },
            state: RuntimeNeedProducerState::NotStarted,
            publication: None,
            task_submitted: true,
            task_terminal: false,
        };
        if let Some(existing) = self.launches.get(&key) {
            if existing != &launch {
                return Err(NeedProducerAdmissionError::LaunchSpecificationConflict);
            }
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        let admission = NeedProducerAdmission {
            correlation: launch.correlation,
            task_spec: launch.task_spec().clone(),
            invocation,
            disposition: NeedProducerTaskDisposition::Ensure,
        };
        Ok((
            admission,
            NeedProducerStartDecision::New {
                key,
                launch,
                invocation,
            },
        ))
    }

    fn commit_start_decision(&mut self, decision: NeedProducerStartDecision) {
        match decision {
            NeedProducerStartDecision::Replay {
                key,
                mark_submitted,
            } => {
                if mark_submitted {
                    self.launches
                        .get_mut(&key)
                        .expect("prepared replay launch exists")
                        .task_submitted = true;
                }
            }
            NeedProducerStartDecision::Join {
                key,
                invocation,
                mark_submitted,
            } => {
                self.invocation_launches.insert(invocation, key);
                if mark_submitted {
                    self.launches
                        .get_mut(&key)
                        .expect("prepared joined launch exists")
                        .task_submitted = true;
                }
            }
            NeedProducerStartDecision::New {
                key,
                launch,
                invocation,
            } => {
                if launch.task_spec().policy == TaskPolicy::JoinSameKey {
                    self.joined.insert((key.generation, key.instance_key), key);
                }
                self.launches.insert(key, launch);
                self.invocation_launches.insert(invocation, key);
            }
        }
    }

    /// Returns accepted launches in deterministic identity order for Product
    /// snapshot construction and exact plan-based restore validation.
    pub fn launches(&self) -> impl Iterator<Item = &RuntimeNeedProducerLaunch> {
        self.launches.values()
    }

    pub(crate) fn submission_for_admission(
        &self,
        admission: &NeedProducerAdmission,
    ) -> TaskSubmission {
        let submission = self
            .task_journal
            .submission(TaskHandle {
                correlation: admission.correlation,
            })
            .expect("committed producer admission belongs to its task journal");
        assert!(submission.spec().same_join_contract(&admission.task_spec));
        submission
    }

    pub fn ensure_task(&mut self, spec: TaskSpec) -> Result<TaskSubmission, TaskEnsureError> {
        let handle = self.task_journal.ensure_task(spec)?;
        Ok(self
            .task_journal
            .submission(handle)
            .expect("accepted receipt belongs to this journal"))
    }

    /// Moves one actual local body outcome into the common task state. A
    /// retirement proof carries no second copy of the returned value.
    pub(crate) fn complete_program_task(
        &mut self,
        launch: &super::ProgramTaskLaunch,
        outcome: super::ProgramTaskOutcome,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<super::ProgramTaskCompletion, super::ProgramTaskRetirementRejection> {
        let correlation = launch.handle().correlation;
        let key = NeedProducerLaunchKey {
            generation: correlation.generation,
            instance_key: correlation.producer,
            ordinal: correlation.launch_ordinal,
        };
        let prepared = (|| {
            let existing = self
                .launches
                .get(&key)
                .ok_or(NeedProducerAdmissionError::InvalidPublicationSource)?;
            if existing.correlation != correlation
                || !matches!(&existing.origin, RuntimeTaskLaunchOrigin::Program { launch: accepted, .. } if accepted == launch)
            {
                return Err(NeedProducerAdmissionError::InvalidPublicationSource);
            }
            if existing.task_terminal
                || !matches!(existing.state, RuntimeNeedProducerState::Pending(_))
                || !matches!(
                    &existing.origin,
                    RuntimeTaskLaunchOrigin::Program {
                        retirement: super::ProgramTaskRetirementPhase::Executing,
                        ..
                    }
                )
            {
                return Err(NeedProducerAdmissionError::InvalidNeedTransition);
            }
            match &outcome {
                super::ProgramTaskOutcome::Returned(value) => {
                    launch.admit_result(value, admission)?;
                    if launch.spec().policy == TaskPolicy::JoinSameKey
                        && !value.ownership().permits_copy()
                    {
                        return Err(NeedProducerAdmissionError::AffineJoinedReadyPublication);
                    }
                }
                super::ProgramTaskOutcome::Failed(failure) => {
                    admission.validate_metadata(failure)?
                }
                super::ProgramTaskOutcome::Cancelled => (),
            }
            Ok(())
        })();
        if let Err(reason) = prepared {
            return Err(super::ProgramTaskRetirementRejection { reason, outcome });
        }
        let (state, terminal) = match outcome {
            super::ProgramTaskOutcome::Returned(value) => (
                RuntimeNeedProducerState::Ready(value.into()),
                super::ProgramTaskTerminal::Completed,
            ),
            super::ProgramTaskOutcome::Failed(failure) => (
                RuntimeNeedProducerState::InfrastructureFailure(failure.clone()),
                super::ProgramTaskTerminal::Failed(failure),
            ),
            super::ProgramTaskOutcome::Cancelled => (
                RuntimeNeedProducerState::Cancelled,
                super::ProgramTaskTerminal::Cancelled,
            ),
        };
        let existing = self
            .launches
            .get_mut(&key)
            .expect("the validated local task remains registered");
        existing.state = state;
        existing.task_terminal = true;
        let RuntimeTaskLaunchOrigin::Program { retirement, .. } = &mut existing.origin else {
            unreachable!("retirement preflight selected an actual Program launch")
        };
        *retirement = super::ProgramTaskRetirementPhase::Unacknowledged;
        Ok(super::ProgramTaskCompletion::retired(
            launch.clone(),
            terminal,
        ))
    }

    pub(crate) fn accepted_task_spec(&self, handle: super::TaskHandle) -> Option<&TaskSpec> {
        self.task_journal.accepted_spec(handle)
    }

    pub(crate) fn program_launch_for_handle(
        &self,
        handle: super::TaskHandle,
    ) -> Option<&super::ProgramTaskLaunch> {
        let state = self.launch_for_correlation(&handle.correlation)?;
        match &state.origin {
            RuntimeTaskLaunchOrigin::Program {
                launch,
                retirement: super::ProgramTaskRetirementPhase::Executing,
            } if !state.task_terminal
                && matches!(state.state, RuntimeNeedProducerState::Pending(_)) =>
            {
                Some(launch)
            }
            _ => None,
        }
    }

    pub fn task_admissions(&self) -> impl Iterator<Item = TaskSubmission> + '_ {
        self.task_journal.submissions()
    }

    pub(crate) fn task_spec(&self, correlation: TaskCorrelation) -> Option<&TaskSpec> {
        self.task_journal
            .accepted_spec(super::TaskHandle { correlation })
    }

    pub(crate) fn ensure_program_task_with_binding<T, E>(
        &mut self,
        spec: TaskSpec,
        bind: impl FnOnce(&TaskSubmission) -> Result<T, E>,
    ) -> Result<(TaskSubmission, super::ProgramTaskLaunch, T), E>
    where
        E: From<TaskEnsureError>,
    {
        if !matches!(&spec.request, super::TaskRequest::Program(_)) {
            return Err(TaskEnsureError::ProgramSourceMismatch.into());
        }
        spec.validate_task_descriptor()?;
        let prepared = self.task_journal.inspect_task(spec.clone())?;
        let launches = &mut self.launches;
        self.task_journal.with_admission(prepared, |handle| {
            let submission = TaskSubmission::try_from_accepted(spec, handle)?;
            let launch = super::ProgramTaskLaunch::retain(&submission)?;
            let correlation = handle.correlation;
            let key = NeedProducerLaunchKey {
                generation: correlation.generation,
                instance_key: correlation.producer,
                ordinal: correlation.launch_ordinal,
            };
            if launches.contains_key(&key) {
                return Err(TaskEnsureError::TaskIdSpecificationConflict {
                    task_id: correlation.task_id,
                }
                .into());
            }
            // Binding observes the actual accepted receipt. Rejection or
            // unwinding precedes launch publication, and the journal guard
            // restores the admission and ordinal frontier.
            let binding = bind(&submission)?;
            launches.insert(
                key,
                RuntimeNeedProducerLaunch {
                    correlation,
                    origin: RuntimeTaskLaunchOrigin::Program {
                        launch: launch.clone(),
                        retirement: super::ProgramTaskRetirementPhase::Executing,
                    },
                    state: RuntimeNeedProducerState::Pending(
                        Progress::new(0.0).expect("zero is valid initial program progress"),
                    ),
                    publication: None,
                    task_submitted: true,
                    task_terminal: false,
                },
            );
            Ok((submission, launch, binding))
        })
    }

    /// Projects active Restartable producer work in stable identity order.
    /// Restored but not yet re-ensured rows remain visible with that state.
    pub fn restartable_dispatches(&self) -> Vec<RuntimeNeedProducerDispatch> {
        self.launches
            .values()
            .filter(|launch| {
                launch.restart() == Some(HostRestartPolicy::Restartable)
                    && !launch.task_terminal
                    && !launch.state.is_terminal()
            })
            .map(|launch| RuntimeNeedProducerDispatch {
                submission: self
                    .task_journal
                    .submission(super::TaskHandle {
                        correlation: launch.correlation,
                    })
                    .expect("active launch retains its accepted journal receipt"),
                restart: launch
                    .restart()
                    .expect("the selected active dispatch has a host restart policy"),
                publication: launch.publication,
                needs_reensure: !launch.task_submitted,
            })
            .collect()
    }

    /// Returns active producer Needs whose selected restart policy requires
    /// the host task to finish before a session save can proceed.
    #[must_use]
    pub fn quiescence_blocking_needs(&self) -> Vec<NeedId> {
        self.launches
            .values()
            .filter(|launch| {
                launch.restart() == Some(HostRestartPolicy::MustBeQuiescent)
                    && !launch.task_terminal()
                    && !launch.state().is_terminal()
                    && launch.task_fault().is_none()
            })
            .map(|launch| launch.need().clone())
            .collect()
    }

    #[must_use]
    pub fn generation_for_task(&self, task: &TaskId) -> Option<GenerationId> {
        self.launches
            .values()
            .find(|launch| &launch.correlation.task_id == task)
            .map(|launch| launch.correlation.generation)
    }

    /// Returns every accepted invocation, including JoinSameKey aliases, in
    /// token order for Product journal snapshots.
    pub fn invocations(
        &self,
    ) -> impl Iterator<Item = (NeedProducerInvocationToken, &RuntimeNeedProducerLaunch)> {
        self.invocation_launches
            .iter()
            .filter_map(|(invocation, key)| {
                self.launches.get(key).map(|launch| (*invocation, launch))
            })
    }

    pub fn invocation_frontiers(&self) -> Vec<NeedProducerInvocationFrontier> {
        self.next_invocation_sequence
            .iter()
            .map(|((generation, fiber, producer_site), next_sequence)| {
                NeedProducerInvocationFrontier {
                    generation: *generation,
                    fiber: *fiber,
                    producer_site: *producer_site,
                    next_sequence: *next_sequence,
                }
            })
            .collect()
    }

    pub fn launch_frontiers(&self) -> Vec<NeedProducerLaunchFrontier> {
        self.task_journal
            .frontiers()
            .map(
                |((generation, instance_key), next_ordinal)| NeedProducerLaunchFrontier {
                    generation,
                    instance_key,
                    next_ordinal,
                },
            )
            .collect()
    }

    /// Returns the next invocation sequence required to continue one site's
    /// journal after a Product snapshot restore.
    #[must_use]
    pub fn next_invocation_sequence(
        &self,
        generation: GenerationId,
        fiber: RuntimePersistentFiberId,
        producer_site: NeedProducerSiteDigest,
    ) -> u64 {
        self.next_invocation_sequence
            .get(&(generation, fiber, producer_site))
            .copied()
            .unwrap_or(0)
    }

    /// Returns the next AlwaysStart ordinal required after restore. JoinSameKey
    /// always remains on the zero ordinal.
    #[must_use]
    pub fn next_launch_ordinal(
        &self,
        generation: GenerationId,
        producer: &NeedProducerSpec,
    ) -> Result<u64, NeedProducerAdmissionError> {
        Ok(self
            .task_journal
            .next_ordinal(generation, producer.instance_key()?))
    }

    #[must_use]
    pub fn launch_for_need(&self, need: &NeedId) -> Option<&RuntimeNeedProducerLaunch> {
        self.launches
            .values()
            .find(|launch| &launch.correlation.need == need)
    }

    #[must_use]
    pub fn launch_for_correlation(
        &self,
        correlation: &super::TaskCorrelation,
    ) -> Option<&RuntimeNeedProducerLaunch> {
        let key = NeedProducerLaunchKey {
            generation: correlation.generation,
            instance_key: correlation.producer,
            ordinal: correlation.launch_ordinal,
        };
        self.launches
            .get(&key)
            .filter(|launch| launch.correlation == *correlation)
    }

    #[must_use]
    pub(crate) fn launch_for_handle(
        &self,
        handle: &super::RuntimeNeedHandle,
    ) -> Option<&RuntimeNeedProducerLaunch> {
        let correlation = handle.correlation();
        self.launch_for_correlation(&correlation)
            .filter(|launch| handle.matches_spec(launch.task_spec()))
    }

    pub fn need_for_task(&self, task: &TaskId) -> Option<&NeedId> {
        self.launches
            .values()
            .find(|launch| &launch.correlation.task_id == task)
            .map(|launch| &launch.correlation.need)
    }

    #[must_use]
    pub fn publication_for_task_event(&self, event: &TaskEvent) -> Option<RuntimeNeedPublication> {
        let correlation = event.correlation;
        self.launches
            .values()
            .find(|launch| launch.correlation == correlation)?;
        let cursor = TaskPublicationCursor::from_event(event);
        Some(match &event.kind {
            TaskEventKind::ProgramTerminal(_) => return None,
            TaskEventKind::Ready(_) | TaskEventKind::Progress(_) | TaskEventKind::Cancelled => {
                RuntimeNeedPublication::Producer {
                    correlation,
                    cursor,
                }
            }
            TaskEventKind::InfrastructureFailure(failure) => {
                RuntimeNeedPublication::InfrastructureFailure {
                    correlation,
                    cursor,
                    failure: failure.clone(),
                }
            }
        })
    }

    /// Reissues only delivery metadata after resume. The result and executable
    /// body remain in their existing local owners and are never run again.
    pub(crate) fn take_program_retirement_replays(&mut self) -> Vec<super::ProgramTaskCompletion> {
        let mut reports = Vec::new();
        for launch in self.launches.values_mut() {
            let Some(terminal) = launch.program_terminal() else {
                continue;
            };
            if let RuntimeTaskLaunchOrigin::Program { launch, retirement } = &mut launch.origin
                && *retirement == super::ProgramTaskRetirementPhase::ReplayPending
            {
                reports.push(super::ProgramTaskCompletion::retired(
                    launch.clone(),
                    terminal,
                ));
                *retirement = super::ProgramTaskRetirementPhase::Unacknowledged;
            }
        }
        reports
    }

    /// Atomically rebuilds the full launch, invocation, and counter journal.
    /// The supplied launches must already have been re-projected from the
    /// verified plan and checked saved arguments by the product owner.
    pub fn restore_registry(
        &mut self,
        restore: NeedProducerRegistryRestore,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), NeedProducerAdmissionError> {
        restore.admit_retained(admission)?;
        self.restore_admitted_registry_with_policy(restore, NeedProducerRestorePolicy::Resume)
    }

    pub(crate) fn restore_admitted_registry_with_policy(
        &mut self,
        restore: NeedProducerRegistryRestore,
        policy: NeedProducerRestorePolicy,
    ) -> Result<(), NeedProducerAdmissionError> {
        if !self.launches.is_empty()
            || !self.joined.is_empty()
            || !self.invocation_launches.is_empty()
            || !self.task_journal.is_empty()
            || !self.next_invocation_sequence.is_empty()
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        Self::validate_restore_journal(&restore)?;
        let mut candidate = Self::default();
        for row in restore.task_admissions {
            let (spec, handle) = row.into_parts();
            candidate
                .task_journal
                .restore_admitted_accepted(spec, handle)?;
        }
        for launch in restore.launches {
            candidate.insert_validated_restore_launch(launch, policy);
        }
        for (invocation, need) in restore.invocations {
            let already_restored = candidate
                .invocation_launches
                .get(&invocation)
                .and_then(|key| candidate.launches.get(key))
                .is_some_and(|launch| launch.correlation == need);
            if !already_restored {
                candidate
                    .restore_invocation_alias(invocation, &need)
                    .expect("borrowed Need registry preflight established this alias");
            }
        }
        *self = candidate;
        Ok(())
    }

    /// Validates a saved producer journal by borrowing its payloads. In
    /// particular, an AlwaysStart Ready may be affine, so snapshot validation
    /// must not copy it into a temporary live registry.
    pub fn validate_restore(
        restore: &NeedProducerRegistryRestore,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<(), NeedProducerAdmissionError> {
        restore.admit_retained(admission)?;
        Ok(())
    }
    fn validate_restore_journal<
        V: crate::value::RuntimeStoredValue,
        P: crate::value::RuntimeStoredValue + PartialEq,
        S: super::TaskExecutionSource + PartialEq,
    >(
        restore: &NeedProducerRegistryRestore<V, P, S>,
    ) -> Result<(), NeedProducerAdmissionError> {
        let mut launches = BTreeMap::new();
        let mut joined = BTreeSet::new();
        let mut invocations = BTreeMap::new();
        let mut next_launch_ordinal =
            super::TaskAdmissionJournal::validate_submission_journal(&restore.task_admissions)?;
        let task_admissions = restore
            .task_admissions
            .iter()
            .map(|row| (row.task_id(), row))
            .collect::<BTreeMap<_, _>>();
        let mut next_invocation_sequence = BTreeMap::new();
        for launch in &restore.launches {
            let row = task_admissions
                .get(&launch.correlation.task_id)
                .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
            if row.handle().correlation != launch.correlation {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            let checked = validate_restore_launch(launch, row.spec())?;
            if launches.insert(checked.key, launch).is_some() {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            if let NeedProducerLaunchOriginRestore::Need { invocation, .. } = &launch.origin {
                if invocations.insert(*invocation, checked.key).is_some()
                    || (row.spec().policy == TaskPolicy::JoinSameKey
                        && !joined.insert((invocation.generation, checked.instance_key)))
                {
                    return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
                }
            }
            if let Some((key, next)) = checked.launch_next {
                let entry = next_launch_ordinal.entry(key).or_insert(1);
                *entry = (*entry).max(next);
            }
            if let Some((key, next)) = checked.invocation_frontier {
                let entry = next_invocation_sequence.entry(key).or_insert(0);
                *entry = (*entry).max(next);
            }
        }
        for (invocation, need) in &restore.invocations {
            if let Some(key) = invocations.get(invocation) {
                if launches
                    .get(key)
                    .is_none_or(|launch| &launch.correlation != need)
                {
                    return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
                }
                continue;
            }
            let Some((key, launch)) = launches
                .iter()
                .find(|(_, launch)| &launch.correlation == need)
            else {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            };
            let NeedProducerLaunchOriginRestore::Need {
                invocation: original,
                start,
            } = &launch.origin
            else {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            };
            if invocation.generation != original.generation
                || invocation.producer_site != start.producer.producer_site()
                || start.task_spec.policy != TaskPolicy::JoinSameKey
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            let next = invocation
                .sequence
                .checked_add(1)
                .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
            let frontier = (
                invocation.generation,
                invocation.fiber,
                invocation.producer_site,
            );
            let entry = next_invocation_sequence.entry(frontier).or_insert(0);
            *entry = (*entry).max(next);
            invocations.insert(*invocation, *key);
        }
        let mut invocation_frontiers = BTreeSet::new();
        for frontier in &restore.invocation_frontiers {
            let key = (frontier.generation, frontier.fiber, frontier.producer_site);
            if !invocation_frontiers.insert(key)
                || next_invocation_sequence.get(&key) != Some(&frontier.next_sequence)
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
        }
        let mut launch_frontiers = BTreeSet::new();
        for frontier in &restore.launch_frontiers {
            let key = (frontier.generation, frontier.instance_key);
            if !launch_frontiers.insert(key)
                || frontier.next_ordinal == 0
                || next_launch_ordinal.get(&key) != Some(&frontier.next_ordinal)
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
        }
        if invocation_frontiers.len() != next_invocation_sequence.len()
            || launch_frontiers.len() != next_launch_ordinal.len()
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        Ok(())
    }

    /// Publishes the next accepted state for one producer-owned Need. Callers
    /// perform the selected payload type check before this state transition.
    pub fn publish_need_state(
        &mut self,
        publication: &RuntimeNeedState,
    ) -> Result<bool, NeedProducerAdmissionError> {
        if self
            .launches
            .values()
            .find(|launch| launch.correlation == publication.correlation)
            .is_some()
        {
            return Err(NeedProducerAdmissionError::InvalidPublicationSource);
        }
        Ok(false)
    }

    /// Transfers a Ready payload into its producer launch after all cursor and
    /// transition checks. A rejected or unclaimed event keeps its sole owner.
    pub fn publish_task_event_owned(
        &mut self,
        event: TaskEvent,
    ) -> Result<NeedProducerOwnedTaskEventDisposition, NeedProducerOwnedTaskEventError> {
        if let Err(error) = event.inspect_host_ready_ownership() {
            return Err(NeedProducerOwnedTaskEventError {
                reason: NeedProducerAdmissionError::HostReadyOwnership(error),
                event,
            });
        }
        let Some(key) = self.launches.iter().find_map(|(key, launch)| {
            (launch.correlation.task_id == event.correlation.task_id).then_some(*key)
        }) else {
            return Ok(NeedProducerOwnedTaskEventDisposition::NotLocal(event));
        };
        let launch = &self.launches[&key];
        let reject = |reason, event| NeedProducerOwnedTaskEventError { reason, event };
        if launch.correlation != event.correlation {
            return Err(reject(
                NeedProducerAdmissionError::StaleTaskGeneration,
                event,
            ));
        }
        let program = matches!(&launch.origin, RuntimeTaskLaunchOrigin::Program { .. });
        if program != matches!(&event.kind, TaskEventKind::ProgramTerminal(_)) {
            return Err(reject(
                NeedProducerAdmissionError::InvalidPublicationSource,
                event,
            ));
        }
        let cursor = TaskPublicationCursor::from_event(&event);
        if let Some(observed) = launch.publication {
            match observed.compare_same_source(cursor) {
                Some(Ordering::Equal) => {
                    return if task_event_matches_launch(launch, &event)
                        && !matches!(
                            &event.kind,
                            TaskEventKind::Ready(value) if !value.value().ownership().permits_copy()
                        ) {
                        Ok(NeedProducerOwnedTaskEventDisposition::Duplicate(event))
                    } else {
                        Err(reject(
                            NeedProducerAdmissionError::ConflictingTaskPublication,
                            event,
                        ))
                    };
                }
                Some(Ordering::Greater) => {
                    return Err(reject(
                        NeedProducerAdmissionError::StaleTaskPublication,
                        event,
                    ));
                }
                Some(Ordering::Less) => {}
                None => {
                    return Err(reject(
                        NeedProducerAdmissionError::InvalidPublicationSource,
                        event,
                    ));
                }
            }
        }
        if program {
            if !task_event_matches_launch(launch, &event) {
                return Err(reject(
                    NeedProducerAdmissionError::InvalidNeedTransition,
                    event,
                ));
            }
            let launch = self.launches.get_mut(&key).expect("selected launch exists");
            let RuntimeTaskLaunchOrigin::Program { retirement, .. } = &mut launch.origin else {
                unreachable!("the checked launch retains its Program origin")
            };
            *retirement = super::ProgramTaskRetirementPhase::Acknowledged;
            launch.publication = Some(cursor);
            return Ok(NeedProducerOwnedTaskEventDisposition::Published);
        }
        if launch.task_terminal {
            return Err(reject(
                NeedProducerAdmissionError::InvalidNeedTransition,
                event,
            ));
        }
        if let TaskEventKind::Ready(value) = &event.kind
            && launch.task_spec().policy == TaskPolicy::JoinSameKey
            && !value.value().ownership().permits_copy()
        {
            return Err(reject(
                NeedProducerAdmissionError::AffineJoinedReadyPublication,
                event,
            ));
        }
        if !matches!(event.kind, TaskEventKind::InfrastructureFailure(_))
            && !matches!(
                launch.state,
                RuntimeNeedProducerState::NotStarted | RuntimeNeedProducerState::Pending(_)
            )
        {
            return Err(reject(
                NeedProducerAdmissionError::InvalidNeedTransition,
                event,
            ));
        }

        let launch = self.launches.get_mut(&key).expect("selected launch exists");
        match event.kind {
            TaskEventKind::ProgramTerminal(_) => unreachable!(
                "Program acknowledgements were handled without replacing the local result"
            ),
            TaskEventKind::Ready(value) => {
                launch.state = RuntimeNeedProducerState::Ready(value);
                launch.task_terminal = true;
            }
            TaskEventKind::Progress(progress) => {
                launch.state = RuntimeNeedProducerState::Pending(progress);
            }
            TaskEventKind::Cancelled => {
                launch.state = RuntimeNeedProducerState::Cancelled;
                launch.task_terminal = true;
            }
            TaskEventKind::InfrastructureFailure(error) => {
                launch.task_terminal = true;
                launch.state = RuntimeNeedProducerState::InfrastructureFailure(error);
            }
        }
        launch.publication = Some(cursor);
        Ok(NeedProducerOwnedTaskEventDisposition::Published)
    }

    #[must_use]
    pub fn ready_for_correlation(
        &self,
        correlation: &super::TaskCorrelation,
    ) -> Option<&RuntimePayload> {
        let launch = self.launch_for_correlation(correlation)?;
        match launch.state() {
            RuntimeNeedProducerState::Ready(value) => Some(value),
            _ => None,
        }
    }

    /// Supplies one owned Ready to Await. AlwaysStart transfers the sole
    /// payload and retains terminal cursor metadata; JoinSameKey copies only
    /// after publication proved the complete value unrestricted.
    pub fn take_ready_for_correlation(
        &mut self,
        correlation: &super::TaskCorrelation,
    ) -> Result<RuntimePayload, NeedProducerAdmissionError> {
        let proof = self.inspect_ready_take_for_correlation(correlation)?;
        Ok(self.take_ready_for_need_prepared(proof))
    }

    pub fn inspect_ready_take_for_correlation(
        &self,
        correlation: &super::TaskCorrelation,
    ) -> Result<NeedProducerReadyTakeProof, NeedProducerAdmissionError> {
        let (key, launch) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.correlation == correlation)
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        let RuntimeNeedProducerState::Ready(value) = &launch.state else {
            return Err(NeedProducerAdmissionError::ReadyUnavailable);
        };
        if launch.task_spec().policy == TaskPolicy::JoinSameKey
            && !value.value().ownership().permits_copy()
        {
            return Err(NeedProducerAdmissionError::AffineJoinedReadyPublication);
        }
        let origin = launch
            .ready_origin()
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        Ok(NeedProducerReadyTakeProof {
            key: *key,
            origin,
            policy: launch.task_spec().policy,
        })
    }

    pub fn take_ready_for_need_prepared(
        &mut self,
        proof: NeedProducerReadyTakeProof,
    ) -> RuntimePayload {
        let launch = self
            .launches
            .get_mut(&proof.key)
            .expect("prepared Ready take launch exists");
        assert!(
            launch.ready_origin() == Some(proof.origin)
                && launch.task_spec().policy == proof.policy,
            "prepared Ready take requires unchanged producer cursor and policy"
        );
        if proof.policy == TaskPolicy::JoinSameKey {
            let RuntimeNeedProducerState::Ready(value) = &launch.state else {
                unreachable!("prepared joined Ready remains present")
            };
            assert!(value.value().ownership().permits_copy());
            return value.clone();
        }
        let previous = std::mem::replace(
            &mut launch.state,
            RuntimeNeedProducerState::ReadyTransferred,
        );
        let RuntimeNeedProducerState::Ready(value) = previous else {
            unreachable!("prepared AlwaysStart Ready remains present")
        };
        value
    }

    /// Returns a temporarily transferred AlwaysStart Ready to its exact
    /// producer during an aborted Await transaction. Failure returns the
    /// untouched payload to the caller.
    pub fn restore_ready_for_correlation(
        &mut self,
        correlation: &super::TaskCorrelation,
        value: RuntimePayload,
    ) -> Result<(), NeedProducerReadyRestoreError> {
        let proof = match self.inspect_ready_restore_for_correlation(correlation) {
            Ok(proof) => proof,
            Err(reason) => return Err(NeedProducerReadyRestoreError { reason, value }),
        };
        self.restore_ready_for_need_prepared(proof, value);
        Ok(())
    }

    pub fn inspect_ready_restore_for_correlation(
        &self,
        correlation: &super::TaskCorrelation,
    ) -> Result<NeedProducerReadyRestoreProof, NeedProducerAdmissionError> {
        let (key, launch) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.correlation == correlation)
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        if launch.task_spec().policy != TaskPolicy::AlwaysStart
            || !matches!(launch.state, RuntimeNeedProducerState::ReadyTransferred)
            || !launch.task_terminal
        {
            return Err(NeedProducerAdmissionError::ReadyUnavailable);
        }
        let origin = launch
            .ready_origin()
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        Ok(NeedProducerReadyRestoreProof { key: *key, origin })
    }

    pub fn restore_ready_for_need_prepared(
        &mut self,
        proof: NeedProducerReadyRestoreProof,
        value: RuntimePayload,
    ) {
        let launch = self
            .launches
            .get_mut(&proof.key)
            .expect("prepared Ready restore launch exists");
        assert!(
            launch.ready_origin() == Some(proof.origin)
                && matches!(launch.state, RuntimeNeedProducerState::ReadyTransferred),
            "prepared Ready restore requires unchanged producer state"
        );
        launch.state = RuntimeNeedProducerState::Ready(value);
    }

    /// Lists active Restartable launches whose host request must be ensured
    /// after Product restore. `mark_task_ensured` should be committed with the
    /// corresponding Product step output.
    pub fn pending_reensure(&self) -> impl Iterator<Item = &RuntimeNeedProducerLaunch> {
        self.launches.values().filter(|launch| {
            launch.restart() == Some(HostRestartPolicy::Restartable)
                && !launch.task_terminal
                && !launch.state.is_terminal()
                && !launch.task_submitted
        })
    }

    pub fn mark_task_ensured(
        &mut self,
        need: &NeedId,
    ) -> Result<Option<TaskSubmission>, NeedProducerAdmissionError> {
        let proof = self.inspect_task_ensured(need)?;
        Ok(self.mark_task_ensured_prepared(proof))
    }

    pub fn inspect_task_ensured(
        &self,
        need: &NeedId,
    ) -> Result<NeedProducerTaskEnsuredProof, NeedProducerAdmissionError> {
        let Some((key, launch)) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.correlation.need == need)
        else {
            return Ok(NeedProducerTaskEnsuredProof {
                key: None,
                submission: None,
            });
        };
        if launch.task_terminal || launch.state.is_terminal() || launch.task_submitted {
            return Ok(NeedProducerTaskEnsuredProof {
                key: None,
                submission: None,
            });
        }
        if launch.restart() != Some(HostRestartPolicy::Restartable) {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        Ok(NeedProducerTaskEnsuredProof {
            key: Some(*key),
            submission: Some(
                self.task_journal
                    .submission(TaskHandle {
                        correlation: launch.correlation,
                    })
                    .expect("restored launch retains its accepted journal receipt"),
            ),
        })
    }

    pub fn mark_task_ensured_prepared(
        &mut self,
        proof: NeedProducerTaskEnsuredProof,
    ) -> Option<TaskSubmission> {
        if let Some(key) = proof.key {
            let launch = self
                .launches
                .get_mut(&key)
                .expect("prepared reensure launch exists");
            assert!(
                !launch.task_terminal && !launch.state.is_terminal() && !launch.task_submitted,
                "prepared reensure requires unchanged producer state"
            );
            launch.task_submitted = true;
        }
        proof.submission
    }

    fn insert_validated_restore_launch(
        &mut self,
        restore: NeedProducerLaunchRestore,
        policy: NeedProducerRestorePolicy,
    ) {
        let receipt = self
            .task_journal
            .submission(TaskHandle {
                correlation: restore.correlation,
            })
            .expect("borrowed restore preflight authenticated the journal row");
        let checked = validate_restore_launch(&restore, receipt.spec())
            .expect("borrowed registry preflight established this launch");
        let origin = match restore.origin {
            NeedProducerLaunchOriginRestore::Need { invocation, start } => {
                RuntimeTaskLaunchOrigin::Need { invocation, start }
            }
            NeedProducerLaunchOriginRestore::Program { retirement } => {
                let launch = super::ProgramTaskLaunch::retain(&receipt)
                    .expect("the validated program row retains only immutable metadata");
                let retirement = if policy == NeedProducerRestorePolicy::Resume
                    && retirement == super::ProgramTaskRetirementPhase::Unacknowledged
                {
                    super::ProgramTaskRetirementPhase::ReplayPending
                } else {
                    retirement
                };
                RuntimeTaskLaunchOrigin::Program { launch, retirement }
            }
        };
        let mut launch = RuntimeNeedProducerLaunch {
            correlation: restore.correlation,
            origin,
            state: restore.state,
            publication: restore.publication,
            task_submitted: restore.task_submitted,
            task_terminal: restore.task_terminal,
        };
        let key = checked.key;
        assert!(!self.launches.contains_key(&key));
        assert!(receipt.spec().same_join_contract(launch.task_spec()));
        if let Some(invocation) = launch.invocation() {
            assert!(!self.invocation_launches.contains_key(&invocation));
            if launch.task_spec().policy == TaskPolicy::JoinSameKey
                && self
                    .joined
                    .insert((launch.correlation.generation, checked.instance_key), key)
                    .is_some()
            {
                unreachable!("borrowed registry preflight checked joined uniqueness");
            }
            self.invocation_launches.insert(invocation, key);
        }
        if let Some((frontier, next)) = checked.invocation_frontier {
            let entry = self.next_invocation_sequence.entry(frontier).or_insert(0);
            *entry = (*entry).max(next);
        }
        if policy == NeedProducerRestorePolicy::Resume
            && launch.restart() == Some(HostRestartPolicy::Restartable)
            && !launch.task_terminal
        {
            launch.task_submitted = false;
        }
        self.launches.insert(key, launch);
    }
    /// Restores a committed invocation alias for a previously restored
    /// JoinSameKey launch. AlwaysStart launch identities have exactly one
    /// invocation token each.
    pub fn restore_invocation_alias(
        &mut self,
        invocation: NeedProducerInvocationToken,
        correlation: &TaskCorrelation,
    ) -> Result<(), NeedProducerAdmissionError> {
        let Some((key, launch)) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.correlation == correlation)
        else {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        };
        let start = launch
            .need_start()
            .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
        if invocation.generation != launch.correlation.generation
            || invocation.producer_site != start.producer.producer_site()
            || launch.task_spec().policy != TaskPolicy::JoinSameKey
            || self.invocation_launches.contains_key(&invocation)
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        let key = *key;
        let invocation_key = (
            invocation.generation,
            invocation.fiber,
            invocation.producer_site,
        );
        let next = invocation
            .sequence
            .checked_add(1)
            .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
        let frontier = self
            .next_invocation_sequence
            .entry(invocation_key)
            .or_insert(0);
        *frontier = (*frontier).max(next);
        self.invocation_launches.insert(invocation, key);
        Ok(())
    }
}

fn admission_disposition(launch: &RuntimeNeedProducerLaunch) -> NeedProducerTaskDisposition {
    if launch.task_terminal || launch.state.is_terminal() || launch.task_submitted {
        NeedProducerTaskDisposition::Reuse
    } else {
        NeedProducerTaskDisposition::Ensure
    }
}

fn task_event_matches_launch(launch: &RuntimeNeedProducerLaunch, event: &TaskEvent) -> bool {
    if launch.correlation != event.correlation {
        return false;
    }
    match (&event.kind, &launch.state) {
        (TaskEventKind::ProgramTerminal(status), _) => {
            launch.program_terminal().as_ref() == Some(status)
        }
        (TaskEventKind::Ready(value), RuntimeNeedProducerState::Ready(current)) => {
            value == current && launch.task_terminal
        }
        (TaskEventKind::Progress(progress), RuntimeNeedProducerState::Pending(current)) => {
            progress == current && !launch.task_terminal
        }
        (TaskEventKind::Cancelled, RuntimeNeedProducerState::Cancelled) => launch.task_terminal,
        (
            TaskEventKind::InfrastructureFailure(failure),
            RuntimeNeedProducerState::InfrastructureFailure(current),
        ) => failure == current && launch.task_terminal,
        _ => false,
    }
}

fn restored_need_state_is_valid<P: crate::value::RuntimeStoredValue>(
    state: &RuntimeNeedProducerState<P>,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
    restart: HostRestartPolicy,
    policy: TaskPolicy,
) -> bool {
    let cursor_matches_state = match state {
        RuntimeNeedProducerState::NotStarted => publication.is_none() && !task_terminal,
        RuntimeNeedProducerState::Pending(_) => publication.is_some() && !task_terminal,
        RuntimeNeedProducerState::Ready(_)
        | RuntimeNeedProducerState::ReadyTransferred
        | RuntimeNeedProducerState::InfrastructureFailure(_)
        | RuntimeNeedProducerState::Cancelled => publication.is_some() && task_terminal,
    };
    cursor_matches_state
        && (!matches!(state, RuntimeNeedProducerState::ReadyTransferred)
            || policy == TaskPolicy::AlwaysStart)
        && (task_submitted || (!task_terminal && restart == HostRestartPolicy::Restartable))
        && (restart != HostRestartPolicy::MustBeQuiescent || task_terminal)
}
#[allow(clippy::too_many_arguments)]
fn launch_matches_start(
    launch: &RuntimeNeedProducerLaunch,
    generation: GenerationId,
    task_spec: &TaskSpec,
    producer: &NeedProducerSpec<crate::plan::RuntimeTaskSource>,
    instance_key: NeedProducerInstanceKey,
    arguments: &[NeedProducerRuntimeArgument],
    restart: HostRestartPolicy,
) -> bool {
    let RuntimeTaskLaunchOrigin::Need { start, .. } = &launch.origin else {
        return false;
    };
    let Ok(correlation) = task_spec.correlation(launch.correlation.launch_ordinal) else {
        return false;
    };
    launch.correlation.generation == generation
        && start.restart == restart
        && start.producer.same_instance_contract(producer)
        && start.arguments == arguments
        && launch.correlation.producer == instance_key
        && launch.correlation.need == correlation.need
        && launch.correlation.task_id == correlation.task_id
        && &start.task_spec == task_spec
}

pub use crate::plan::{
    RuntimeTaskPlanDigestBase, ViewTaskPlanAuthority, ViewTaskPlanDigestRequest,
    ViewTaskPlanValidationError,
};

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn source_control_metadata_binds_the_actual_launch_definition() {
        fn identity(plan: &NeedProducerTaskPlan) -> blake3::Hash {
            let mut meter = super::super::semantic::TaskSemanticMeter::new(100_000, 1_000_000);
            let mut encoder = super::super::semantic::TaskSemanticEncoder::new(
                b"arcweft.task.source-control.v1\0",
                &mut meter,
            );
            plan.encode_control_metadata(&mut encoder);
            encoder.finish().unwrap()
        }
        let base = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        let changes: [fn(&mut NeedProducerTaskPlan); 10] = [
            |plan| plan.site = NeedProducerSiteDigest::from_bytes([3; 32]),
            |plan| {
                plan.contract = NeedProducerContractDigest::from_bytes([4; 32]);
                let NeedProducerRequestProjection::ExternCapability { contract, .. } =
                    &mut plan.request
                else {
                    unreachable!()
                };
                *contract = crate::step::HostCallContractDigest::from_bytes([4; 32]);
            },
            |plan| plan.payload_type = RuntimeSemanticTypeId::from_bytes([5; 32]),
            |plan| plan.policy = TaskPolicy::AlwaysStart,
            |plan| plan.restart = HostRestartPolicy::MustBeQuiescent,
            |plan| plan.class = TaskClass::Io,
            |plan| plan.priority = TaskPriority(5),
            |plan| plan.cancel_scope.0.push_str("/child"),
            |plan| {
                let NeedProducerRequestProjection::ExternCapability { operation, .. } =
                    &mut plan.request
                else {
                    unreachable!()
                };
                operation.push_str("_other");
            },
            |plan| {
                let NeedProducerRequestProjection::ExternCapability { argument_names, .. } =
                    &mut plan.request
                else {
                    unreachable!()
                };
                *argument_names = Box::new([Some("path".to_owned())]);
                plan.argument_types = Box::new([RuntimeSemanticTypeId::from_bytes([6; 32])]);
            },
        ];
        let expected = identity(&base);
        for (ordinal, mutate) in changes.into_iter().enumerate() {
            let mut variant = base.clone();
            mutate(&mut variant);
            assert_ne!(
                identity(&variant),
                expected,
                "source metadata mutation {ordinal}"
            );
        }
    }

    #[test]
    fn source_control_metadata_cannot_reset_its_parent_budget() {
        let plan = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        let mut meter = super::super::semantic::TaskSemanticMeter::new(0, 1_000_000);
        let mut encoder = super::super::semantic::TaskSemanticEncoder::new(
            b"arcweft.task.source-control.v1\0",
            &mut meter,
        );
        plan.encode_control_metadata(&mut encoder);
        assert_eq!(
            encoder.finish(),
            Err(super::super::semantic::TaskSemanticEncodingError::SemanticWork)
        );
        assert_eq!(
            meter.status(),
            Err(super::super::semantic::TaskSemanticEncodingError::SemanticWork)
        );
    }

    fn producer_plan(
        site: u8,
        contract: u8,
        policy: TaskPolicy,
        restart: HostRestartPolicy,
        class: TaskClass,
    ) -> NeedProducerTaskPlan {
        let host_contract = crate::step::HostCallContractDigest::from_bytes([contract; 32]);
        NeedProducerTaskPlan::try_new(
            NeedProducerContractDigest::from_bytes([contract; 32]),
            NeedProducerSiteDigest::from_bytes([site; 32]),
            NeedProducerRequestProjection::ExternCapability {
                capability: HostCapabilityId("asset".to_owned()),
                operation: "image".to_owned(),
                contract: host_contract,
                argument_names: Box::new([]),
            },
            Box::new([]),
            RuntimeSemanticTypeId::from_bytes([9; 32]),
            policy,
            restart,
            class,
            TaskPriority(4),
            CancelScopeId("flow".to_owned()),
        )
        .expect("typed producer plan")
    }

    fn registry_plan(
        site: u8,
        contract: u8,
        policy: TaskPolicy,
        restart: HostRestartPolicy,
        class: TaskClass,
    ) -> Arc<crate::plan::RuntimePlan> {
        source_fixture(producer_plan(site, contract, policy, restart, class))
    }

    fn source_fixture(metadata: NeedProducerTaskPlan) -> Arc<crate::plan::RuntimePlan> {
        let mut builder = crate::plan::RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [crate::plan::RuntimePlanTypeSeed::new(
                    metadata.payload_type(),
                    crate::plan::RuntimePlanTypeProjection::Bool,
                )],
                [],
            )
            .unwrap();
        crate::tests::selected_producer_plan(builder, metadata, Vec::new())
    }

    fn registry_site(plan: &crate::plan::RuntimePlan) -> NeedProducerSiteDigest {
        let crate::plan::FlowOp::StartNeedProducer { target, .. } = &plan
            .function_sites()
            .iter()
            .next()
            .unwrap()
            .body()
            .executable()
            .unwrap()
            .ops()[0]
        else {
            unreachable!()
        };
        target.plan().site()
    }

    fn start(
        plan: &Arc<crate::plan::RuntimePlan>,
        generation: GenerationId,
    ) -> RuntimeNeedProducerStart {
        crate::tests::selected_producer_start(plan, generation, Vec::new())
    }

    fn restore_admission(
        plan: &Arc<crate::plan::RuntimePlan>,
    ) -> crate::value::RuntimeRetainedValueAdmission {
        crate::value::RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Plan(Arc::clone(plan)),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Tests the schema fragment nested in original control metadata. This is
    /// deliberately a raw test transcript hash, never a live Task plan key.
    fn request_schema_hash(
        plan: &NeedProducerTaskPlan,
    ) -> Result<blake3::Hash, super::super::semantic::TaskSemanticEncodingError> {
        let mut meter = super::super::semantic::TaskSemanticMeter::new(100_000, 1_000_000);
        let mut encoder = super::super::semantic::TaskSemanticEncoder::new(
            b"arcweft.task.request-schema.v1\0",
            &mut meter,
        );
        plan.encode_request_schema(&mut encoder);
        encoder.finish()
    }

    #[test]
    fn generation_zero_and_join_ordinal_are_valid_values() {
        assert_eq!(GenerationId::new(0).get(), 0);
        assert_eq!(TaskLaunchOrdinal::JOIN.get(), 0);
    }

    fn restore_launch(launch: &RuntimeNeedProducerLaunch) -> NeedProducerLaunchRestore {
        assert!(matches!(
            launch.state(),
            RuntimeNeedProducerState::NotStarted
        ));
        let owner = launch.source().program_owner();
        let mut admission = crate::value::RuntimeRetainedValueAdmission::for_program(
            owner.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let original = launch
            .need_start()
            .expect("the fixture restores a Need producer");
        original.admit_retained(&mut admission).unwrap();
        let start = original
            .snapshot_admitted()
            .unwrap()
            .into_admitted_live()
            .unwrap();
        NeedProducerLaunchRestore {
            origin: NeedProducerLaunchOriginRestore::Need {
                invocation: launch.invocation().unwrap(),
                start,
            },
            correlation: launch.correlation(),
            state: RuntimeNeedProducerState::NotStarted,
            publication: launch.publication(),
            task_submitted: launch.task_submitted(),
            task_terminal: launch.task_terminal(),
        }
    }

    #[test]
    fn producer_instance_key_commits_every_typed_spec_field() {
        let base = NeedProducerSpec::new(
            NeedProducerFamily::StructuredTaskPlan,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([7; 32]),
            RuntimeTypeSemanticDigest::from_bytes([3; 32]),
            RuntimeValueDigest::from_bytes([4; 32]),
        );
        let changed = NeedProducerSpec::new(
            NeedProducerFamily::StructuredTaskPlan,
            NeedProducerContractDigest::from_bytes([1; 32]),
            TaskPlanSemanticDigest::from_bytes([2; 32]),
            NeedProducerSiteDigest::from_bytes([8; 32]),
            RuntimeTypeSemanticDigest::from_bytes([3; 32]),
            RuntimeValueDigest::from_bytes([4; 32]),
        );
        assert_ne!(
            base.instance_key().expect("base key").as_bytes(),
            changed.instance_key().expect("changed key").as_bytes()
        );
    }

    #[test]
    fn producer_request_transcript_has_exact_version_one_byte_grammar() {
        let plan = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        let mut transcript = b"arcweft.task.request-schema.v1\0".to_vec();
        transcript.push(1);
        transcript.extend_from_slice(&5_u32.to_le_bytes());
        transcript.extend_from_slice(b"asset");
        transcript.extend_from_slice(&5_u32.to_le_bytes());
        transcript.extend_from_slice(b"image");
        transcript.extend_from_slice(&0_u32.to_le_bytes());
        transcript.extend_from_slice(&0_u32.to_le_bytes());
        transcript.push(1);
        assert_eq!(
            request_schema_hash(&plan).unwrap().as_bytes(),
            blake3::hash(&transcript).as_bytes()
        );
    }

    #[test]
    fn producer_request_transcript_commits_ordered_optional_names_and_types() {
        let mut plan = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        if let NeedProducerRequestProjection::ExternCapability { argument_names, .. } =
            &mut plan.request
        {
            *argument_names = vec![Some("é".to_owned()), None].into_boxed_slice();
        } else {
            panic!("fixture is an external producer");
        }
        plan.argument_types = vec![
            RuntimeSemanticTypeId::from_bytes([7; 32]),
            RuntimeSemanticTypeId::from_bytes([8; 32]),
        ]
        .into_boxed_slice();
        let mut transcript = b"arcweft.task.request-schema.v1\0".to_vec();
        transcript.push(1);
        transcript.extend_from_slice(&5_u32.to_le_bytes());
        transcript.extend_from_slice(b"asset");
        transcript.extend_from_slice(&5_u32.to_le_bytes());
        transcript.extend_from_slice(b"image");
        transcript.extend_from_slice(&2_u32.to_le_bytes());
        transcript.push(1);
        transcript.extend_from_slice(&2_u32.to_le_bytes());
        transcript.extend_from_slice("é".as_bytes());
        transcript.push(0);
        transcript.extend_from_slice(&2_u32.to_le_bytes());
        transcript.extend_from_slice(&[7; 32]);
        transcript.extend_from_slice(&[8; 32]);
        transcript.push(1);
        assert_eq!(
            request_schema_hash(&plan).unwrap().as_bytes(),
            blake3::hash(&transcript).as_bytes()
        );
        let before = request_schema_hash(&plan).unwrap();
        plan.argument_types.reverse();
        assert_ne!(before, request_schema_hash(&plan).unwrap());
        plan.argument_types.reverse();
        if let NeedProducerRequestProjection::ExternCapability { argument_names, .. } =
            &mut plan.request
        {
            argument_names.reverse();
        }
        assert_ne!(before, request_schema_hash(&plan).unwrap());
    }

    #[test]
    fn asset_request_transcript_uses_the_same_checked_string_grammar() {
        let mut plan = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::MustBeQuiescent,
            TaskClass::AssetDecode,
        );
        plan.request = NeedProducerRequestProjection::AssetLoad {
            kind: AssetLoadKind::Voice,
            argument_name: "clip".to_owned(),
        };
        plan.argument_types = vec![RuntimeSemanticTypeId::from_bytes([7; 32])].into_boxed_slice();
        let mut transcript = b"arcweft.task.request-schema.v1\0".to_vec();
        transcript.extend_from_slice(&[0, 1]);
        transcript.extend_from_slice(&4_u32.to_le_bytes());
        transcript.extend_from_slice(b"clip");
        transcript.extend_from_slice(&1_u32.to_le_bytes());
        transcript.extend_from_slice(&[7; 32]);
        transcript.push(0);
        assert_eq!(
            request_schema_hash(&plan).unwrap().as_bytes(),
            blake3::hash(&transcript).as_bytes()
        );
        plan.request = NeedProducerRequestProjection::AssetLoad {
            kind: AssetLoadKind::Image,
            argument_name: "clip".to_owned(),
        };
        assert_ne!(
            request_schema_hash(&plan).unwrap().as_bytes(),
            blake3::hash(&transcript).as_bytes()
        );
    }

    #[test]
    fn selected_instance_and_plan_commit_original_control_metadata() {
        let base = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let original = source_fixture(base.clone());
        let expected = start(&original, GenerationId::new(0));
        let changes: [fn(&mut NeedProducerTaskPlan); 7] = [
            |plan| plan.site = NeedProducerSiteDigest::from_bytes([3; 32]),
            |plan| plan.policy = TaskPolicy::AlwaysStart,
            |plan| plan.restart = HostRestartPolicy::MustBeQuiescent,
            |plan| plan.priority = TaskPriority(12),
            |plan| plan.cancel_scope = CancelScopeId("another scope".into()),
            |plan| plan.class = TaskClass::Cpu,
            |plan| plan.payload_type = RuntimeSemanticTypeId::from_bytes([10; 32]),
        ];
        for (index, change) in changes.into_iter().enumerate() {
            let mut metadata = base.clone();
            change(&mut metadata);
            let program = source_fixture(metadata);
            let changed = start(&program, GenerationId::new(0));
            assert_ne!(
                expected.source().task_plan().key(),
                changed.source().task_plan().key(),
                "control field {index}"
            );
            assert_ne!(
                expected.producer().instance_key().unwrap(),
                changed.producer().instance_key().unwrap(),
                "instance field {index}"
            );
        }
    }

    #[test]
    fn selected_retained_start_uses_the_original_bank_and_keeps_its_first_failure() {
        let plan = registry_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let input = start(&plan, GenerationId::new(7));
        let mut measured = restore_admission(&plan);
        input.admit_retained(&mut measured).unwrap();
        let work = measured.work().consumed();
        assert!(work > 1);
        let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
        let bank = |maximum| {
            crate::value::RuntimeRetainedValueAdmission::for_program(
                owner.clone(),
                crate::entry::RuntimeSchemaLimits {
                    max_validation_work: maximum,
                    ..crate::entry::RuntimeSchemaLimits::engine_default()
                },
            )
        };
        let mut exact = bank(work);
        input.admit_retained(&mut exact).unwrap();
        assert_eq!(exact.work().consumed(), work);
        assert!(
            exact.try_into_state().is_ok(),
            "Source must release its temporary work alias"
        );
        for prefix in [false, true] {
            let mut short = bank(if prefix { work } else { work - 1 });
            if prefix {
                short.validate_metadata(&17_u64).unwrap();
            }
            let leases = Arc::strong_count(&plan);
            let error = input.admit_retained(&mut short).unwrap_err();
            let consumed = short.work().consumed();
            assert_eq!(consumed, if prefix { work + 1 } else { work });
            assert_eq!(short.status(), Err(error.clone()));
            assert_eq!(input.admit_retained(&mut short), Err(error));
            assert_eq!(short.work().consumed(), consumed);
            assert_eq!(Arc::strong_count(&plan), leases);
        }
    }

    #[test]
    fn selected_ready_retains_its_original_type_graph_across_program_replacement() {
        let original = registry_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let metadata = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let mut builder = crate::plan::RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [crate::plan::RuntimePlanTypeSeed::new(
                    metadata.payload_type(),
                    crate::plan::RuntimePlanTypeProjection::String,
                )],
                [],
            )
            .unwrap();
        let replacement = crate::tests::selected_producer_plan(builder, metadata, Vec::new());
        let current = RuntimeProgramOwner::Plan(replacement);
        let mut registry = NeedProducerRegistry::default();
        let proof = registry
            .inspect_start_visit(
                RuntimePersistentFiberId::from_allocated(3),
                start(&original, GenerationId::new(7)),
            )
            .unwrap();
        let admitted = registry.commit_start_visit(proof);
        let event = TaskEvent {
            correlation: admitted.correlation(),
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(1),
                sequence: TaskSequence(1),
            },
            kind: TaskEventKind::Ready(RuntimePayload(RuntimeValue::Bool(true))),
        };
        assert!(matches!(
            registry.publish_task_event_owned(event),
            Ok(NeedProducerOwnedTaskEventDisposition::Published)
        ));
        let mut bank = crate::value::RuntimeRetainedValueAdmission::for_program(
            current.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let image = registry.snapshot_retained(&current, &mut bank).unwrap();
        assert!(bank.program_owner().unwrap().same_program(&current));
        let work = bank.work().consumed();
        image.admit_retained(&mut bank).unwrap();
        assert!(bank.work().consumed() > work);
        assert!(bank.program_owner().unwrap().same_program(&current));
        let mut forged = image.clone();
        forged.launches[0].state = RuntimeNeedProducerState::Ready(
            crate::value::AwbcRuntimeValueSnapshot::String("replacement-only payload".into()),
        );
        let mut failed = crate::value::RuntimeRetainedValueAdmission::for_program(
            current.clone(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
        );
        let error = forged.admit_retained(&mut failed).unwrap_err();
        assert_eq!(failed.status(), Err(error));
        assert!(failed.program_owner().unwrap().same_program(&current));
        let mut restored = NeedProducerRegistry::default();
        restored
            .restore_admitted_registry_with_policy(
                image.into_admitted_live(&current).unwrap(),
                NeedProducerRestorePolicy::Rollback,
            )
            .unwrap();
        let launch = restored
            .launch_for_correlation(&admitted.correlation())
            .unwrap();
        assert!(launch.source().belongs_to(&original));
        assert!(
            matches!(launch.state(), RuntimeNeedProducerState::Ready(value) if value.value() == &RuntimeValue::Bool(true))
        );
    }

    #[test]
    fn producer_arguments_use_ordered_tuple_identity_without_binding_names() {
        let max_argument_bytes =
            crate::entry::RuntimeSchemaLimits::engine_default().platform_encoded_bytes();
        let values = [
            RuntimeValue::Bool(true),
            RuntimeValue::String("value".into()),
        ];
        let named: Vec<_> = values
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, value)| NeedProducerRuntimeArgument {
                name: Some(format!("arg{index}")),
                value,
            })
            .collect();
        let positional: Vec<_> = values
            .iter()
            .cloned()
            .map(|value| NeedProducerRuntimeArgument { name: None, value })
            .collect();
        let digest = need_producer_arguments_digest(
            named.iter().map(|argument| &argument.value),
            max_argument_bytes,
        )
        .unwrap();
        assert_eq!(
            digest,
            need_producer_arguments_digest(
                positional.iter().map(|argument| &argument.value),
                max_argument_bytes
            )
            .unwrap()
        );
        assert_eq!(
            digest,
            RuntimeValue::Tuple(values.to_vec())
                .try_digest(1024)
                .unwrap()
        );
        let reversed: Vec<_> = positional.into_iter().rev().collect();
        assert_ne!(
            digest,
            need_producer_arguments_digest(
                reversed.iter().map(|argument| &argument.value),
                max_argument_bytes
            )
            .unwrap()
        );
        assert_eq!(
            need_producer_arguments_digest(std::iter::empty::<&RuntimeValue>(), max_argument_bytes)
                .unwrap(),
            RuntimeValue::Tuple(Vec::new()).try_digest(1024).unwrap()
        );
        assert_ne!(
            need_producer_arguments_digest(std::iter::empty::<&RuntimeValue>(), max_argument_bytes)
                .unwrap()
                .as_bytes(),
            &[0; 32]
        );
    }

    #[test]
    fn borrowed_tuple_digest_enforces_one_complete_transcript_budget() {
        let values = [
            RuntimeValue::Bool(true),
            RuntimeValue::String("value".into()),
        ];
        let tuple = RuntimeValue::Tuple(values.to_vec());
        let encoded = tuple.try_canonical_bytes(1024).unwrap();
        let borrowed: Vec<_> = values.iter().collect();
        assert_eq!(
            crate::entry::schema::canonical_runtime_tuple_digest(&borrowed, encoded.len()).unwrap(),
            tuple.try_digest(encoded.len()).unwrap()
        );
        assert!(
            crate::entry::schema::canonical_runtime_tuple_digest(&borrowed, encoded.len() - 1)
                .is_err()
        );
    }

    #[test]
    fn rollback_image_restores_one_affine_ready_without_changing_dispatch_state() {
        let mut builder = crate::plan::RuntimePlanBuilder::new();
        let boolean = crate::pattern::RuntimeCheckedType::Bool.semantic_identity_digest();
        builder
            .admit_semantic_batch(
                [
                    crate::plan::RuntimePlanTypeSeed::new(
                        boolean,
                        crate::plan::RuntimePlanTypeProjection::Bool,
                    ),
                    crate::plan::RuntimePlanTypeSeed::new(
                        RuntimeSemanticTypeId::from_bytes([9; 32]),
                        crate::plan::RuntimePlanTypeProjection::Need(boolean),
                    ),
                ],
                [],
                [],
                [],
                &crate::entry::RuntimeNominalSchemaGraph::try_new(
                    vec![],
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .expect("empty nominal authority"),
            )
            .expect("original Need<Bool> outcome rows");
        let plan = crate::tests::selected_producer_plan(
            builder,
            producer_plan(
                2,
                3,
                TaskPolicy::AlwaysStart,
                HostRestartPolicy::Restartable,
                TaskClass::Io,
            ),
            Vec::new(),
        );
        let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
        let generation = GenerationId::new(3);
        let fiber = RuntimePersistentFiberId::from_allocated(7);
        let mut registry = NeedProducerRegistry::default();
        let proof = registry
            .inspect_start_visit(fiber, start(&plan, generation))
            .expect("producer start preflight");
        let admission = registry.commit_start_visit(proof);
        let need = admission.need().clone();
        let correlation = admission.correlation();
        let ensured = registry
            .inspect_task_ensured(&need)
            .expect("task ensured preflight");
        registry.mark_task_ensured_prepared(ensured);
        let inner = || {
            crate::tests::reusable_need_with_outcome(
                "need.inner.affine",
                super::super::TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::Bool),
            )
        };
        let ready = RuntimeValue::NeedHandle(inner());
        assert!(!ready.ownership().permits_copy());
        let event = TaskEvent {
            correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(1),
                sequence: TaskSequence(1),
            },
            kind: TaskEventKind::Ready(RuntimePayload(ready)),
        };
        assert!(matches!(
            registry.publish_task_event_owned(event),
            Ok(NeedProducerOwnedTaskEventDisposition::Published)
        ));
        let image = registry
            .inert_rollback_image(&owner)
            .expect("inert registry image");
        drop(registry);
        let mut restored = NeedProducerRegistry::from_rollback_image(image, &owner)
            .expect("exact registry restore");
        assert!(restored.launch_for_need(&need).unwrap().task_submitted());
        assert_eq!(
            restored
                .take_ready_for_correlation(&correlation)
                .unwrap()
                .into_value(),
            RuntimeValue::NeedHandle(inner())
        );
        assert!(matches!(
            restored.launch_for_need(&need).unwrap().state(),
            RuntimeNeedProducerState::ReadyTransferred
        ));
    }

    #[test]
    fn need_producer_registry_replays_join_and_advances_always_start_per_instance() {
        let generation = GenerationId::new(8);
        let fiber = RuntimePersistentFiberId::from_allocated(17);
        let join_plan = registry_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let mut registry = NeedProducerRegistry::default();
        let first_token = registry
            .begin_invocation(generation, fiber, registry_site(&join_plan))
            .expect("first token");
        let first = registry
            .admit_start(first_token, start(&join_plan, first_token.generation))
            .expect("first join producer");
        assert_eq!(first.disposition(), NeedProducerTaskDisposition::Ensure);
        assert_eq!(
            registry.launch_for_need(first.need()).unwrap().ordinal(),
            TaskLaunchOrdinal::JOIN
        );

        let replay = registry
            .admit_start(first_token, start(&join_plan, first_token.generation))
            .expect("same accepted invocation replays");
        assert_eq!(replay.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(replay.need(), first.need());
        assert_eq!(replay.correlation().task_id, first.correlation().task_id);

        let joined_token = registry
            .begin_invocation(generation, fiber, registry_site(&join_plan))
            .expect("next checked invocation");
        let joined = registry
            .admit_start(joined_token, start(&join_plan, joined_token.generation))
            .expect("join existing producer");
        assert_eq!(joined.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(joined.need(), first.need());
        assert_eq!(registry.invocations().count(), 2);

        let changed_plan = registry_plan(
            1,
            4,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        assert_eq!(
            registry.admit_start(first_token, start(&changed_plan, first_token.generation)),
            Err(NeedProducerAdmissionError::InvocationSpecificationConflict)
        );

        let always_plan = registry_plan(
            2,
            3,
            TaskPolicy::AlwaysStart,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let always_first_token = registry
            .begin_invocation(generation, fiber, registry_site(&always_plan))
            .expect("AlwaysStart token 1");
        let always_first = registry
            .admit_start(
                always_first_token,
                start(&always_plan, always_first_token.generation),
            )
            .expect("AlwaysStart launch 1");
        let always_second_token = registry
            .begin_invocation(generation, fiber, registry_site(&always_plan))
            .expect("AlwaysStart token 2");
        let always_second = registry
            .admit_start(
                always_second_token,
                start(&always_plan, always_second_token.generation),
            )
            .expect("AlwaysStart launch 2");
        assert_eq!(
            registry
                .launch_for_need(always_first.need())
                .unwrap()
                .ordinal()
                .get(),
            1
        );
        assert_eq!(
            registry
                .launch_for_need(always_second.need())
                .unwrap()
                .ordinal()
                .get(),
            2
        );
        assert_eq!(
            always_first.correlation().task_key,
            always_second.correlation().task_key
        );
        assert_ne!(always_first.need(), always_second.need());
        assert_ne!(
            always_first.correlation().task_id,
            always_second.correlation().task_id
        );

        let next_generation = GenerationId::new(9);
        assert!(
            registry
                .launch_for_need(always_second.need())
                .unwrap()
                .source()
                .belongs_to(&always_plan)
        );
        let next_generation_token = registry
            .begin_invocation(next_generation, fiber, registry_site(&always_plan))
            .expect("new generation token");
        let next_generation_launch = registry
            .admit_start(
                next_generation_token,
                start(&always_plan, next_generation_token.generation),
            )
            .expect("new generation launch");
        assert_ne!(next_generation_launch.need(), always_second.need());
        assert_ne!(
            next_generation_launch.correlation().task_key,
            always_second.correlation().task_key
        );
    }

    #[test]
    fn need_producer_restore_preserves_aliases_and_reensures_once() {
        let generation = GenerationId::new(11);
        let fiber = RuntimePersistentFiberId::from_allocated(23);
        let plan = registry_plan(
            5,
            6,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let mut registry = NeedProducerRegistry::default();
        let first = registry
            .begin_invocation(generation, fiber, registry_site(&plan))
            .expect("first invocation");
        let first_launch = registry
            .admit_start(first, start(&plan, first.generation))
            .expect("first launch");
        let second = registry
            .begin_invocation(generation, fiber, registry_site(&plan))
            .expect("join invocation");
        registry
            .admit_start(second, start(&plan, second.generation))
            .expect("join launch");

        let snapshot = NeedProducerRegistryRestore {
            task_admissions: registry.task_admissions().collect(),
            launches: registry.launches().map(restore_launch).collect(),
            invocations: registry
                .invocations()
                .map(|(token, launch)| (token, launch.correlation()))
                .collect(),
            invocation_frontiers: registry.invocation_frontiers(),
            launch_frontiers: registry.launch_frontiers(),
        };
        let mut restored = NeedProducerRegistry::default();
        restored
            .restore_registry(snapshot, &mut restore_admission(&plan))
            .expect("restore full producer journal");
        assert_eq!(restored.invocations().count(), 2);
        assert_eq!(
            restored.next_invocation_sequence(
                generation,
                fiber,
                NeedProducerSiteDigest::from_bytes([5; 32])
            ),
            2
        );
        assert_eq!(restored.pending_reensure().count(), 1);
        let reissued = restored
            .mark_task_ensured(first_launch.need())
            .expect("mark one restartable ensure")
            .expect("active task needs ensure");
        assert_eq!(reissued.spec(), first_launch.task_spec());
        assert_eq!(reissued.handle().correlation, first_launch.correlation());
        assert!(
            restored
                .mark_task_ensured(first_launch.need())
                .expect("duplicate ensure is harmless")
                .is_none()
        );
        assert_eq!(restored.pending_reensure().count(), 0);
    }

    #[test]
    fn need_producer_restore_rejects_missing_frontiers_atomically() {
        for (policy, omit_invocations) in [
            (TaskPolicy::JoinSameKey, true),
            (TaskPolicy::AlwaysStart, false),
        ] {
            let generation = GenerationId::new(12);
            let fiber = RuntimePersistentFiberId::from_allocated(24);
            let plan = registry_plan(7, 8, policy, HostRestartPolicy::Restartable, TaskClass::Io);
            let mut registry = NeedProducerRegistry::default();
            let invocation = registry
                .begin_invocation(generation, fiber, registry_site(&plan))
                .expect("producer invocation");
            registry
                .admit_start(invocation, start(&plan, invocation.generation))
                .expect("producer launch");
            let mut snapshot = NeedProducerRegistryRestore {
                task_admissions: registry.task_admissions().collect(),
                launches: registry.launches().map(restore_launch).collect(),
                invocations: registry
                    .invocations()
                    .map(|(token, launch)| (token, launch.correlation()))
                    .collect(),
                invocation_frontiers: registry.invocation_frontiers(),
                launch_frontiers: registry.launch_frontiers(),
            };
            if omit_invocations {
                snapshot.invocation_frontiers.clear();
            } else {
                snapshot.launch_frontiers.clear();
            }

            let mut restored = NeedProducerRegistry::default();
            assert_eq!(
                restored.restore_registry(snapshot, &mut restore_admission(&plan)),
                Err(NeedProducerAdmissionError::Admission(
                    crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: NeedProducerAdmissionError::InvalidRestoredLaunch.to_string(),
                    },
                )),
            );
            assert_eq!(restored.launches().count(), 0);
        }
    }

    #[test]
    fn need_producer_restore_rejects_future_frontiers_atomically() {
        for (policy, tamper_invocation) in [
            (TaskPolicy::JoinSameKey, true),
            (TaskPolicy::AlwaysStart, false),
        ] {
            let generation = GenerationId::new(13);
            let fiber = RuntimePersistentFiberId::from_allocated(25);
            let plan = registry_plan(9, 10, policy, HostRestartPolicy::Restartable, TaskClass::Io);
            let mut registry = NeedProducerRegistry::default();
            let invocation = registry
                .begin_invocation(generation, fiber, registry_site(&plan))
                .expect("producer invocation");
            registry
                .admit_start(invocation, start(&plan, invocation.generation))
                .expect("producer launch");
            let mut snapshot = NeedProducerRegistryRestore {
                task_admissions: registry.task_admissions().collect(),
                launches: registry.launches().map(restore_launch).collect(),
                invocations: registry
                    .invocations()
                    .map(|(token, launch)| (token, launch.correlation()))
                    .collect(),
                invocation_frontiers: registry.invocation_frontiers(),
                launch_frontiers: registry.launch_frontiers(),
            };
            if tamper_invocation {
                snapshot.invocation_frontiers[0].next_sequence += 1;
            } else {
                snapshot.launch_frontiers[0].next_ordinal += 1;
            }

            let mut restored = NeedProducerRegistry::default();
            assert_eq!(
                restored.restore_registry(snapshot, &mut restore_admission(&plan)),
                Err(NeedProducerAdmissionError::Admission(
                    crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: NeedProducerAdmissionError::InvalidRestoredLaunch.to_string(),
                    },
                )),
            );
            assert_eq!(restored.launches().count(), 0);
        }
    }
}
