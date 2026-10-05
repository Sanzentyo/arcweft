// Typed Need producer plan, admission journal, publication, and restore authority.
// The selected plan and its launch registry remain together because they share
// canonical TaskSpec, Need/Task identity, restart policy, and publication state.
use super::*;
use crate::entry::RuntimeValueDigest;
use std::collections::BTreeSet;

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
    const fn semantic_tag(self) -> u8 {
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
}

/// Complete typed producer contract used as the sole source of its instance
/// identity.  The individual semantic fields are intentionally not exposed as
/// an alternate task/Need identity authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NeedProducerSpec {
    family: NeedProducerFamily,
    contract: NeedProducerContractDigest,
    plan: TaskPlanSemanticDigest,
    producer_site: NeedProducerSiteDigest,
    payload_type: RuntimeTypeSemanticDigest,
    arguments: RuntimeValueDigest,
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
        }
    }

    /// Derives the fixed producer-instance identity from this complete spec.
    pub fn instance_key(&self) -> Result<NeedProducerInstanceKey, TaskIdentityError> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.need.producer-instance.v1\0");
        hasher.update(&[self.family.semantic_tag()]);
        hasher.update(self.contract.as_bytes());
        hasher.update(self.plan.as_bytes());
        hasher.update(self.producer_site.as_bytes());
        hasher.update(self.payload_type.as_bytes());
        hasher.update(self.arguments.as_bytes());
        NeedProducerInstanceKey::try_from_bytes(*hasher.finalize().as_bytes())
    }

    pub const fn family(&self) -> NeedProducerFamily {
        self.family
    }

    pub const fn contract(&self) -> NeedProducerContractDigest {
        self.contract
    }

    pub const fn plan(&self) -> TaskPlanSemanticDigest {
        self.plan
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

/// One checked, source-ordered argument evaluated for a Need producer call.
/// Names are retained because extern requests preserve manifest argument
/// bindings; spreads are not representable in this closed runtime row.
#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerRuntimeArgument {
    pub name: Option<String>,
    pub value: RuntimeValue,
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
}

/// Complete static selected contract for one host-backed Need producer.
/// Construction computes its semantic digest from typed fields, so native and
/// AWBC consumers cannot supply a competing digest or include their local
/// instruction/plan index in identity.
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
    semantic_digest: TaskPlanSemanticDigest,
}

impl NeedProducerTaskPlan {
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
        let semantic_digest = need_producer_task_plan_digest(
            contract,
            &request,
            &argument_types,
            payload_type,
            policy,
            restart,
            &class,
            priority,
            &cancel_scope,
        )?;
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
            semantic_digest,
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
    pub const fn semantic_digest(&self) -> TaskPlanSemanticDigest {
        self.semantic_digest
    }

    #[must_use]
    pub fn argument_name(&self, index: usize) -> Option<Option<&str>> {
        match &self.request {
            NeedProducerRequestProjection::AssetLoad { argument_name, .. } => {
                (index == 0).then_some(Some(argument_name.as_str()))
            }
            NeedProducerRequestProjection::ExternCapability { argument_names, .. } => {
                argument_names.get(index).map(|name| name.as_deref())
            }
        }
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

    #[must_use]
    pub fn producer_spec(
        &self,
        arguments: &[NeedProducerRuntimeArgument],
    ) -> Result<NeedProducerSpec, NeedProducerAdmissionError> {
        let digest = need_producer_arguments_digest(arguments)?;
        Ok(NeedProducerSpec::new(
            NeedProducerFamily::SelectedCallable,
            self.contract,
            self.semantic_digest,
            self.site,
            RuntimeTypeSemanticDigest::from_bytes(*self.payload_type.as_bytes()),
            digest,
        ))
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
                    || arguments[0]
                        .name
                        .as_deref()
                        .is_some_and(|name| name != argument_name)
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
    fn request_matches(
        &self,
        arguments: &[NeedProducerRuntimeArgument],
        actual: &HostTaskRequest,
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
                    || arguments[0]
                        .name
                        .as_deref()
                        .is_some_and(|name| name != argument_name)
                {
                    return Err(NeedProducerRequestError::ArgumentBindingMismatch);
                }
                let RuntimeValue::EntityRef(reference) = &arguments[0].value else {
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
                    && positional
                        .zip(args)
                        .all(|(argument, saved)| argument.value == *saved.value())
                    && named.zip(named_args).all(|(argument, saved)| {
                        argument.name.as_deref() == Some(saved.name.as_str())
                            && argument.value == *saved.value.value()
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

#[allow(clippy::too_many_arguments)]
fn need_producer_task_plan_digest(
    contract: NeedProducerContractDigest,
    request: &NeedProducerRequestProjection,
    argument_types: &[RuntimeSemanticTypeId],
    payload_type: RuntimeSemanticTypeId,
    policy: TaskPolicy,
    restart: HostRestartPolicy,
    class: &TaskClass,
    priority: TaskPriority,
    cancel_scope: &CancelScopeId,
) -> Result<TaskPlanSemanticDigest, NeedProducerPlanError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.need.producer-task-plan.v1\0");
    hasher.update(contract.as_bytes());
    match request {
        NeedProducerRequestProjection::AssetLoad {
            kind,
            argument_name,
        } => {
            hasher.update(&[
                0,
                match kind {
                    AssetLoadKind::Image => 0,
                    AssetLoadKind::Voice => 1,
                },
            ]);
            write_digest_string(&mut hasher, argument_name);
        }
        NeedProducerRequestProjection::ExternCapability {
            capability,
            operation,
            contract,
            argument_names,
        } => {
            hasher.update(&[1]);
            write_digest_string(&mut hasher, &capability.0);
            write_digest_string(&mut hasher, operation);
            hasher.update(contract.as_bytes());
            hasher.update(
                &u32::try_from(argument_names.len())
                    .map_err(|_| NeedProducerPlanError::ArgumentBindingCountOverflow)?
                    .to_le_bytes(),
            );
            for name in argument_names.iter() {
                match name {
                    Some(name) => {
                        hasher.update(&[1]);
                        write_digest_string(&mut hasher, name);
                    }
                    None => {
                        hasher.update(&[0]);
                    }
                }
            }
        }
    }
    hasher.update(
        &u32::try_from(argument_types.len())
            .map_err(|_| NeedProducerPlanError::ArgumentBindingCountOverflow)?
            .to_le_bytes(),
    );
    for argument_type in argument_types {
        hasher.update(argument_type.as_bytes());
    }
    hasher.update(payload_type.as_bytes());
    hasher.update(&[match policy {
        TaskPolicy::JoinSameKey => 0,
        TaskPolicy::AlwaysStart => 1,
    }]);
    hasher.update(&[match restart {
        HostRestartPolicy::MustBeQuiescent => 0,
        HostRestartPolicy::Restartable => 1,
    }]);
    hasher.update(&[task_class_semantic_tag(class)]);
    hasher.update(&priority.0.to_le_bytes());
    write_digest_string(&mut hasher, &cancel_scope.0);
    Ok(TaskPlanSemanticDigest::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
}

fn task_class_semantic_tag(class: &TaskClass) -> u8 {
    match class {
        TaskClass::LocalView => 0,
        TaskClass::Io => 1,
        TaskClass::Cpu => 2,
        TaskClass::GpuPrepare => 3,
        TaskClass::ShaderCompile => 4,
        TaskClass::WasmCall => 5,
        TaskClass::AssetDecode => 6,
        TaskClass::AudioDecode => 7,
        TaskClass::AudioRender => 8,
        TaskClass::TtsSynthesis => 9,
        TaskClass::BgmPrecompose => 10,
        TaskClass::Lsp => 11,
        TaskClass::Background => 12,
    }
}

fn write_digest_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(u64::try_from(value.len()).unwrap_or(u64::MAX)).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn need_producer_arguments_digest(
    arguments: &[NeedProducerRuntimeArgument],
) -> Result<RuntimeValueDigest, NeedProducerAdmissionError> {
    const MAX_ARGUMENT_BYTES: usize = 16 * 1024 * 1024;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.need.producer.arguments.v1\0");
    hasher.update(
        &u32::try_from(arguments.len())
            .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?
            .to_le_bytes(),
    );
    let mut encoded_bytes = 0_usize;
    for argument in arguments {
        match &argument.name {
            Some(name) => {
                hasher.update(&[1]);
                write_digest_string(&mut hasher, name);
            }
            None => {
                hasher.update(&[0]);
            }
        }
        let bytes = argument
            .value
            .try_canonical_bytes(MAX_ARGUMENT_BYTES)
            .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?;
        encoded_bytes = encoded_bytes
            .checked_add(bytes.len())
            .ok_or(NeedProducerAdmissionError::InvalidProducerArguments)?;
        if encoded_bytes > MAX_ARGUMENT_BYTES {
            return Err(NeedProducerAdmissionError::InvalidProducerArguments);
        }
        hasher.update(
            &u64::try_from(bytes.len())
                .map_err(|_| NeedProducerAdmissionError::InvalidProducerArguments)?
                .to_le_bytes(),
        );
        hasher.update(&bytes);
    }
    Ok(RuntimeValueDigest::from_bytes(
        *hasher.finalize().as_bytes(),
    ))
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
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNeedProducerLaunch {
    generation: GenerationId,
    invocation: NeedProducerInvocationToken,
    plan: NeedProducerTaskPlan,
    producer: NeedProducerSpec,
    arguments: Vec<NeedProducerRuntimeArgument>,
    instance_key: NeedProducerInstanceKey,
    ordinal: TaskLaunchOrdinal,
    need: NeedId,
    task: TaskId,
    task_spec: TaskSpec,
    state: RuntimeNeedProducerState,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
    task_fault: Option<String>,
}

impl RuntimeNeedProducerLaunch {
    #[must_use]
    pub const fn generation(&self) -> GenerationId {
        self.generation
    }

    #[must_use]
    pub const fn invocation(&self) -> NeedProducerInvocationToken {
        self.invocation
    }

    #[must_use]
    pub const fn producer(&self) -> &NeedProducerSpec {
        &self.producer
    }

    #[must_use]
    pub const fn plan(&self) -> &NeedProducerTaskPlan {
        &self.plan
    }

    #[must_use]
    pub fn arguments(&self) -> &[NeedProducerRuntimeArgument] {
        &self.arguments
    }

    #[must_use]
    pub const fn instance_key(&self) -> NeedProducerInstanceKey {
        self.instance_key
    }

    #[must_use]
    pub const fn ordinal(&self) -> TaskLaunchOrdinal {
        self.ordinal
    }

    #[must_use]
    pub const fn need(&self) -> &NeedId {
        &self.need
    }

    #[must_use]
    pub const fn task(&self) -> &TaskId {
        &self.task
    }

    #[must_use]
    pub const fn task_spec(&self) -> &TaskSpec {
        &self.task_spec
    }

    #[must_use]
    pub const fn restart(&self) -> HostRestartPolicy {
        self.plan.restart
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
    pub fn task_fault(&self) -> Option<&str> {
        self.task_fault.as_deref()
    }
}

/// The producer's terminal Ready metadata remains after an AlwaysStart
/// payload moves to its one Await consumer. JoinSameKey retains a checked
/// unrestricted Ready payload for later joiners.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeNeedProducerState {
    NotStarted,
    Pending(Progress),
    Ready(RuntimePayload),
    ReadyTransferred,
    Cancelled,
}

impl RuntimeNeedProducerState {
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Ready(_) | Self::ReadyTransferred | Self::Cancelled
        )
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
    need: NeedId,
    task_spec: TaskSpec,
    invocation: NeedProducerInvocationToken,
    disposition: NeedProducerTaskDisposition,
}

impl NeedProducerAdmission {
    #[must_use]
    pub const fn need(&self) -> &NeedId {
        &self.need
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
        next_ordinal: Option<((GenerationId, NeedProducerInstanceKey), u64)>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeedProducerRegistry {
    launches: BTreeMap<NeedProducerLaunchKey, RuntimeNeedProducerLaunch>,
    joined: BTreeMap<(GenerationId, NeedProducerInstanceKey), NeedProducerLaunchKey>,
    invocation_launches: BTreeMap<NeedProducerInvocationToken, NeedProducerLaunchKey>,
    next_launch_ordinal: BTreeMap<(GenerationId, NeedProducerInstanceKey), u64>,
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
    #[error("borrowed task Ready publication requires a recursively unrestricted payload")]
    AffineBorrowedTaskPublication,
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
    publication: TaskPublicationCursor,
}

pub struct NeedProducerReadyTakeProof {
    key: NeedProducerLaunchKey,
    publication: TaskPublicationCursor,
    policy: TaskPolicy,
}

pub struct NeedProducerTaskEnsuredProof {
    key: Option<NeedProducerLaunchKey>,
    task_spec: Option<TaskSpec>,
}

impl NeedProducerTaskEnsuredProof {
    #[must_use]
    pub const fn task_spec(&self) -> Option<&TaskSpec> {
        self.task_spec.as_ref()
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
#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerLaunchRestore {
    pub invocation: NeedProducerInvocationToken,
    pub plan: NeedProducerTaskPlan,
    pub arguments: Vec<NeedProducerRuntimeArgument>,
    pub ordinal: TaskLaunchOrdinal,
    pub need: NeedId,
    pub task: TaskId,
    pub task_spec: TaskSpec,
    pub state: RuntimeNeedProducerState,
    pub publication: Option<TaskPublicationCursor>,
    pub task_submitted: bool,
    pub task_terminal: bool,
    pub task_fault: Option<String>,
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
    pub generation: GenerationId,
    pub need_id: NeedId,
    pub task_id: TaskId,
    pub task_spec: TaskSpec,
    pub restart: HostRestartPolicy,
    pub publication: Option<TaskPublicationCursor>,
    pub needs_reensure: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NeedProducerRegistryRestore {
    pub launches: Vec<NeedProducerLaunchRestore>,
    pub invocations: Vec<(NeedProducerInvocationToken, NeedId)>,
    pub invocation_frontiers: Vec<NeedProducerInvocationFrontier>,
    pub launch_frontiers: Vec<NeedProducerLaunchFrontier>,
}

/// In-memory rollback representation of a producer registry. Every runtime
/// argument and Ready payload is inert until the prior live owner is gone.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NeedProducerRegistryRollbackImage {
    launches: Vec<NeedProducerLaunchRollbackImage>,
    invocations: Vec<(NeedProducerInvocationToken, NeedId)>,
    invocation_frontiers: Vec<NeedProducerInvocationFrontier>,
    launch_frontiers: Vec<NeedProducerLaunchFrontier>,
}

#[derive(Clone, Debug, PartialEq)]
struct NeedProducerLaunchRollbackImage {
    invocation: NeedProducerInvocationToken,
    plan: NeedProducerTaskPlan,
    arguments: Vec<(Option<String>, crate::value::AwbcRuntimeValueSnapshot)>,
    ordinal: TaskLaunchOrdinal,
    need: NeedId,
    task: TaskId,
    state: NeedProducerStateRollbackImage,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
    task_fault: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum NeedProducerStateRollbackImage {
    NotStarted,
    Pending(Progress),
    Ready(crate::value::AwbcRuntimeValueSnapshot),
    ReadyTransferred,
    Cancelled,
}

struct ValidatedRestoreLaunch {
    producer: NeedProducerSpec,
    instance_key: NeedProducerInstanceKey,
    key: NeedProducerLaunchKey,
    invocation_key: (
        GenerationId,
        RuntimePersistentFiberId,
        NeedProducerSiteDigest,
    ),
    invocation_next: u64,
    launch_next: Option<((GenerationId, NeedProducerInstanceKey), u64)>,
}

fn validate_restore_launch(
    restore: &NeedProducerLaunchRestore,
) -> Result<ValidatedRestoreLaunch, NeedProducerAdmissionError> {
    if restore
        .arguments
        .iter()
        .any(|argument| !argument.value.ownership().permits_copy())
    {
        return Err(NeedProducerAdmissionError::AffineHostProducerArgument);
    }
    let generation = restore.invocation.generation;
    let producer = restore.plan.producer_spec(&restore.arguments)?;
    let instance_key = producer.instance_key()?;
    let (need, task, task_key) = producer_runtime_ids(generation, instance_key, restore.ordinal);
    let saved_spec = &restore.task_spec;
    if restore.need != need
        || restore.task != task
        || saved_spec.id != task
        || saved_spec.key != task_key
        || saved_spec.class != restore.plan.class
        || saved_spec.priority != restore.plan.priority
        || saved_spec.cancel_scope != restore.plan.cancel_scope
        || saved_spec.policy != restore.plan.policy
        || saved_spec.outcome != restore.plan.outcome()
        || saved_spec.debug_label != saved_spec.request.debug_label()
        || !restore
            .plan
            .request_matches(&restore.arguments, &saved_spec.request)?
        || restore.invocation.producer_site != restore.plan.site
        || (restore.plan.policy == TaskPolicy::JoinSameKey
            && restore.ordinal != TaskLaunchOrdinal::JOIN)
        || (restore.plan.policy == TaskPolicy::AlwaysStart
            && restore.ordinal == TaskLaunchOrdinal::JOIN)
        || !restored_need_state_is_valid(
            &restore.state,
            restore.publication,
            restore.task_submitted,
            restore.task_terminal,
            restore.task_fault.as_deref(),
            restore.plan.restart,
            restore.plan.policy,
        )
    {
        return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
    }
    let launch_next = if restore.plan.policy == TaskPolicy::AlwaysStart {
        Some((
            (generation, instance_key),
            restore
                .ordinal
                .get()
                .checked_add(1)
                .ok_or(NeedProducerAdmissionError::LaunchOrdinalExhausted)?,
        ))
    } else {
        None
    };
    let invocation_next = restore
        .invocation
        .sequence
        .checked_add(1)
        .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
    Ok(ValidatedRestoreLaunch {
        producer,
        instance_key,
        key: NeedProducerLaunchKey {
            generation,
            instance_key,
            ordinal: restore.ordinal,
        },
        invocation_key: (
            generation,
            restore.invocation.fiber,
            restore.invocation.producer_site,
        ),
        invocation_next,
        launch_next,
    })
}

impl NeedProducerRegistry {
    pub(crate) fn inert_rollback_image(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NeedProducerRegistryRollbackImage, String> {
        let image = |value: &RuntimeValue| {
            crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
                .map_err(|error| error.to_string())
        };
        Ok(NeedProducerRegistryRollbackImage {
            launches: self
                .launches
                .values()
                .map(|launch| {
                    Ok(NeedProducerLaunchRollbackImage {
                        invocation: launch.invocation,
                        plan: launch.plan.clone(),
                        arguments: launch
                            .arguments
                            .iter()
                            .map(|argument| Ok((argument.name.clone(), image(&argument.value)?)))
                            .collect::<Result<_, String>>()?,
                        ordinal: launch.ordinal,
                        need: launch.need.clone(),
                        task: launch.task.clone(),
                        state: match &launch.state {
                            RuntimeNeedProducerState::NotStarted => {
                                NeedProducerStateRollbackImage::NotStarted
                            }
                            RuntimeNeedProducerState::Pending(progress) => {
                                NeedProducerStateRollbackImage::Pending(progress.clone())
                            }
                            RuntimeNeedProducerState::Ready(value) => {
                                NeedProducerStateRollbackImage::Ready(image(value.value())?)
                            }
                            RuntimeNeedProducerState::ReadyTransferred => {
                                NeedProducerStateRollbackImage::ReadyTransferred
                            }
                            RuntimeNeedProducerState::Cancelled => {
                                NeedProducerStateRollbackImage::Cancelled
                            }
                        },
                        publication: launch.publication,
                        task_submitted: launch.task_submitted,
                        task_terminal: launch.task_terminal,
                        task_fault: launch.task_fault.clone(),
                    })
                })
                .collect::<Result<_, String>>()?,
            invocations: self
                .invocation_launches
                .iter()
                .map(|(invocation, key)| (*invocation, self.launches[key].need.clone()))
                .collect(),
            invocation_frontiers: self
                .next_invocation_sequence
                .iter()
                .map(|((generation, fiber, producer_site), next_sequence)| {
                    NeedProducerInvocationFrontier {
                        generation: *generation,
                        fiber: *fiber,
                        producer_site: *producer_site,
                        next_sequence: *next_sequence,
                    }
                })
                .collect(),
            launch_frontiers: self
                .next_launch_ordinal
                .iter()
                .map(
                    |((generation, instance_key), next_ordinal)| NeedProducerLaunchFrontier {
                        generation: *generation,
                        instance_key: *instance_key,
                        next_ordinal: *next_ordinal,
                    },
                )
                .collect(),
        })
    }

    pub(crate) fn from_rollback_image(
        image: NeedProducerRegistryRollbackImage,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let value = |image: crate::value::AwbcRuntimeValueSnapshot| {
            image
                .into_runtime_value_for_program(owner)
                .map_err(|error| error.to_string())
        };
        let launches = image
            .launches
            .into_iter()
            .map(|launch| {
                let arguments = launch
                    .arguments
                    .into_iter()
                    .map(|(name, saved)| {
                        Ok(NeedProducerRuntimeArgument {
                            name,
                            value: value(saved)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if arguments
                    .iter()
                    .any(|argument| !argument.value.ownership().permits_copy())
                {
                    return Err(NeedProducerAdmissionError::AffineHostProducerArgument.to_string());
                }
                let producer = launch
                    .plan
                    .producer_spec(&arguments)
                    .map_err(|error| error.to_string())?;
                let instance_key = producer.instance_key().map_err(|error| error.to_string())?;
                let (_, _, task_key) = producer_runtime_ids(
                    launch.invocation.generation,
                    instance_key,
                    launch.ordinal,
                );
                let request = launch
                    .plan
                    .project_request(&arguments)
                    .map_err(|error| error.to_string())?;
                let task_spec = TaskSpec::new(
                    launch.task.clone(),
                    task_key,
                    launch.plan.class.clone(),
                    launch.plan.priority,
                    launch.plan.cancel_scope.clone(),
                    launch.plan.policy,
                    request,
                )
                .with_outcome(launch.plan.outcome());
                let state = match launch.state {
                    NeedProducerStateRollbackImage::NotStarted => {
                        RuntimeNeedProducerState::NotStarted
                    }
                    NeedProducerStateRollbackImage::Pending(progress) => {
                        RuntimeNeedProducerState::Pending(progress)
                    }
                    NeedProducerStateRollbackImage::Ready(saved) => {
                        RuntimeNeedProducerState::Ready(RuntimePayload(value(saved)?))
                    }
                    NeedProducerStateRollbackImage::ReadyTransferred => {
                        RuntimeNeedProducerState::ReadyTransferred
                    }
                    NeedProducerStateRollbackImage::Cancelled => {
                        RuntimeNeedProducerState::Cancelled
                    }
                };
                Ok(NeedProducerLaunchRestore {
                    invocation: launch.invocation,
                    plan: launch.plan,
                    arguments,
                    ordinal: launch.ordinal,
                    need: launch.need,
                    task: launch.task,
                    task_spec,
                    state,
                    publication: launch.publication,
                    task_submitted: launch.task_submitted,
                    task_terminal: launch.task_terminal,
                    task_fault: launch.task_fault,
                })
            })
            .collect::<Result<_, String>>()?;
        let mut registry = Self::default();
        registry
            .restore_registry_with_policy(
                NeedProducerRegistryRestore {
                    launches,
                    invocations: image.invocations,
                    invocation_frontiers: image.invocation_frontiers,
                    launch_frontiers: image.launch_frontiers,
                },
                NeedProducerRestorePolicy::Rollback,
            )
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
        plan: NeedProducerTaskPlan,
        arguments: Vec<NeedProducerRuntimeArgument>,
    ) -> Result<NeedProducerAdmission, NeedProducerAdmissionError> {
        let (admission, decision) = self.prepare_start_decision(invocation, plan, arguments)?;
        self.commit_start_decision(decision);
        Ok(admission)
    }

    /// Prepares one producer visit while preserving the current registry and
    /// every live Ready payload. The proof holds only metadata and host-safe
    /// arguments; binding/quota checks can run before `commit_start_visit`.
    pub fn inspect_start_visit(
        &self,
        generation: GenerationId,
        fiber: RuntimePersistentFiberId,
        plan: NeedProducerTaskPlan,
        arguments: Vec<NeedProducerRuntimeArgument>,
    ) -> Result<NeedProducerStartProof, NeedProducerAdmissionError> {
        let frontier = (generation, fiber, plan.site);
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
            producer_site: plan.site,
            sequence: observed_sequence,
        };
        let (admission, decision) = self.prepare_start_decision(invocation, plan, arguments)?;
        Ok(NeedProducerStartProof {
            frontier,
            observed_sequence,
            next_sequence,
            admission,
            decision,
        })
    }

    pub fn commit_start_visit(&mut self, proof: NeedProducerStartProof) -> NeedProducerAdmission {
        assert_eq!(
            self.next_invocation_sequence
                .get(&proof.frontier)
                .copied()
                .unwrap_or(0),
            proof.observed_sequence,
            "prepared Need producer visit requires an unchanged invocation frontier"
        );
        self.commit_start_decision(proof.decision);
        self.next_invocation_sequence
            .insert(proof.frontier, proof.next_sequence);
        proof.admission
    }

    fn prepare_start_decision(
        &self,
        invocation: NeedProducerInvocationToken,
        plan: NeedProducerTaskPlan,
        arguments: Vec<NeedProducerRuntimeArgument>,
    ) -> Result<(NeedProducerAdmission, NeedProducerStartDecision), NeedProducerAdmissionError>
    {
        if arguments
            .iter()
            .any(|argument| !argument.value.ownership().permits_copy())
        {
            return Err(NeedProducerAdmissionError::AffineHostProducerArgument);
        }
        if invocation.producer_site != plan.site {
            return Err(NeedProducerAdmissionError::InvocationSiteMismatch);
        }
        let generation = invocation.generation;
        let producer = plan.producer_spec(&arguments)?;
        let request = plan.project_request(&arguments)?;
        let policy = plan.policy;
        let outcome = plan.outcome();
        let class = plan.class.clone();
        let priority = plan.priority;
        let cancel_scope = plan.cancel_scope.clone();
        let instance_key = producer.instance_key()?;

        if let Some(key) = self.invocation_launches.get(&invocation).copied() {
            let existing = self
                .launches
                .get(&key)
                .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
            if !launch_matches_plan(
                existing,
                generation,
                &plan,
                &producer,
                instance_key,
                &arguments,
                &request,
            ) {
                return Err(NeedProducerAdmissionError::InvocationSpecificationConflict);
            }
            let disposition = admission_disposition(existing);
            return Ok((
                NeedProducerAdmission {
                    need: existing.need.clone(),
                    task_spec: existing.task_spec.clone(),
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
                if !launch_matches_plan(
                    existing,
                    generation,
                    &plan,
                    &producer,
                    instance_key,
                    &arguments,
                    &request,
                ) {
                    return Err(NeedProducerAdmissionError::JoinSpecificationConflict);
                }
                let disposition = admission_disposition(existing);
                return Ok((
                    NeedProducerAdmission {
                        need: existing.need.clone(),
                        task_spec: existing.task_spec.clone(),
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

        let (ordinal, next_ordinal) = match policy {
            TaskPolicy::JoinSameKey => (TaskLaunchOrdinal::JOIN, None),
            TaskPolicy::AlwaysStart => {
                let counter_key = (generation, instance_key);
                let ordinal = self
                    .next_launch_ordinal
                    .get(&counter_key)
                    .copied()
                    .unwrap_or(1);
                let next = ordinal
                    .checked_add(1)
                    .ok_or(NeedProducerAdmissionError::LaunchOrdinalExhausted)?;
                (TaskLaunchOrdinal(ordinal), Some((counter_key, next)))
            }
        };
        let key = NeedProducerLaunchKey {
            generation,
            instance_key,
            ordinal,
        };
        let (need, task, task_key) = producer_runtime_ids(generation, instance_key, ordinal);
        let task_spec = TaskSpec::new(
            task.clone(),
            task_key,
            class,
            priority,
            cancel_scope,
            policy,
            request,
        )
        .with_outcome(outcome);
        let launch = RuntimeNeedProducerLaunch {
            generation,
            invocation,
            plan,
            producer,
            arguments,
            instance_key,
            ordinal,
            need,
            task,
            task_spec,
            state: RuntimeNeedProducerState::NotStarted,
            publication: None,
            task_submitted: true,
            task_terminal: false,
            task_fault: None,
        };
        if let Some(existing) = self.launches.get(&key) {
            if existing != &launch {
                return Err(NeedProducerAdmissionError::LaunchSpecificationConflict);
            }
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        let admission = NeedProducerAdmission {
            need: launch.need.clone(),
            task_spec: launch.task_spec.clone(),
            invocation,
            disposition: NeedProducerTaskDisposition::Ensure,
        };
        Ok((
            admission,
            NeedProducerStartDecision::New {
                key,
                launch,
                invocation,
                next_ordinal,
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
                next_ordinal,
            } => {
                if let Some((counter_key, next)) = next_ordinal {
                    self.next_launch_ordinal.insert(counter_key, next);
                }
                if launch.plan.policy == TaskPolicy::JoinSameKey {
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

    /// Projects active Restartable producer work in stable identity order.
    /// Restored but not yet re-ensured rows remain visible with that state.
    pub fn restartable_dispatches(&self) -> Vec<RuntimeNeedProducerDispatch> {
        self.launches
            .values()
            .filter(|launch| {
                launch.plan.restart == HostRestartPolicy::Restartable
                    && !launch.task_terminal
                    && !launch.state.is_terminal()
            })
            .map(|launch| RuntimeNeedProducerDispatch {
                generation: launch.generation,
                need_id: launch.need.clone(),
                task_id: launch.task.clone(),
                task_spec: launch.task_spec.clone(),
                restart: launch.plan.restart,
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
                launch.restart() == HostRestartPolicy::MustBeQuiescent
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
            .find(|launch| &launch.task == task)
            .map(|launch| launch.generation)
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
        self.next_launch_ordinal
            .iter()
            .map(
                |((generation, instance_key), next_ordinal)| NeedProducerLaunchFrontier {
                    generation: *generation,
                    instance_key: *instance_key,
                    next_ordinal: *next_ordinal,
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
        let key = (generation, producer.instance_key()?);
        Ok(self.next_launch_ordinal.get(&key).copied().unwrap_or(1))
    }

    #[must_use]
    pub fn launch_for_need(&self, need: &NeedId) -> Option<&RuntimeNeedProducerLaunch> {
        self.launches.values().find(|launch| &launch.need == need)
    }

    #[must_use]
    pub fn need_for_task(&self, task: &TaskId) -> Option<&NeedId> {
        self.launches
            .values()
            .find(|launch| &launch.task == task)
            .map(|launch| &launch.need)
    }

    #[must_use]
    pub fn publication_for_task_event(&self, event: &TaskEvent) -> Option<RuntimeNeedPublication> {
        let need = self.need_for_task(&event.task_id)?.clone();
        let cursor = TaskPublicationCursor::from_event(event);
        Some(match &event.kind {
            TaskEventKind::Ready(_) | TaskEventKind::Progress(_) | TaskEventKind::Cancelled => {
                RuntimeNeedPublication::Producer { need, cursor }
            }
            TaskEventKind::Failed(message) => RuntimeNeedPublication::Failed {
                need,
                cursor,
                message: message.clone(),
            },
        })
    }

    /// Atomically rebuilds the full launch, invocation, and counter journal.
    /// The supplied launches must already have been re-projected from the
    /// verified plan and checked saved arguments by the product owner.
    pub fn restore_registry(
        &mut self,
        restore: NeedProducerRegistryRestore,
    ) -> Result<(), NeedProducerAdmissionError> {
        self.restore_registry_with_policy(restore, NeedProducerRestorePolicy::Resume)
    }

    pub(crate) fn restore_registry_with_policy(
        &mut self,
        restore: NeedProducerRegistryRestore,
        policy: NeedProducerRestorePolicy,
    ) -> Result<(), NeedProducerAdmissionError> {
        if !self.launches.is_empty()
            || !self.joined.is_empty()
            || !self.invocation_launches.is_empty()
            || !self.next_launch_ordinal.is_empty()
            || !self.next_invocation_sequence.is_empty()
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        Self::validate_restore(&restore)?;
        let mut candidate = Self::default();
        for launch in restore.launches {
            candidate.insert_validated_restore_launch(launch, policy);
        }
        for (invocation, need) in restore.invocations {
            let already_restored = candidate
                .invocation_launches
                .get(&invocation)
                .and_then(|key| candidate.launches.get(key))
                .is_some_and(|launch| launch.need == need);
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
    ) -> Result<(), NeedProducerAdmissionError> {
        let mut launches = BTreeMap::new();
        let mut joined = BTreeSet::new();
        let mut invocations = BTreeMap::new();
        let mut next_launch_ordinal = BTreeMap::new();
        let mut next_invocation_sequence = BTreeMap::new();
        for launch in &restore.launches {
            let checked = validate_restore_launch(launch)?;
            if launches.insert(checked.key, launch).is_some()
                || invocations.insert(launch.invocation, checked.key).is_some()
                || (launch.plan.policy == TaskPolicy::JoinSameKey
                    && !joined.insert((launch.invocation.generation, checked.instance_key)))
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            if let Some((key, next)) = checked.launch_next {
                let entry = next_launch_ordinal.entry(key).or_insert(1);
                *entry = (*entry).max(next);
            }
            let entry = next_invocation_sequence
                .entry(checked.invocation_key)
                .or_insert(0);
            *entry = (*entry).max(checked.invocation_next);
        }
        for (invocation, need) in &restore.invocations {
            if let Some(key) = invocations.get(invocation) {
                if launches.get(key).is_none_or(|launch| &launch.need != need) {
                    return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
                }
                continue;
            }
            let Some((key, launch)) = launches.iter().find(|(_, launch)| &launch.need == need)
            else {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            };
            if invocation.generation != launch.invocation.generation
                || invocation.producer_site != launch.plan.site
                || launch.plan.policy != TaskPolicy::JoinSameKey
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
            .values_mut()
            .find(|launch| launch.need == *publication.need())
            .is_some()
        {
            return Err(NeedProducerAdmissionError::InvalidPublicationSource);
        }
        Ok(false)
    }

    /// Projects one host TaskEvent into the launch's Need state. A failed task
    /// remains an infrastructure fault and is deliberately not represented as
    /// a typed `Ready(T)` value.
    pub fn publish_task_event(
        &mut self,
        event: &TaskEvent,
    ) -> Result<bool, NeedProducerAdmissionError> {
        if let TaskEventKind::Ready(value) = &event.kind
            && !value.value().ownership().permits_copy()
        {
            return Err(NeedProducerAdmissionError::AffineBorrowedTaskPublication);
        }
        match self.publish_task_event_owned(event.clone()) {
            Ok(NeedProducerOwnedTaskEventDisposition::Published) => Ok(true),
            Ok(
                NeedProducerOwnedTaskEventDisposition::Duplicate(_)
                | NeedProducerOwnedTaskEventDisposition::NotLocal(_),
            ) => Ok(false),
            Err(error) => Err(error.into_parts().0),
        }
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
        let Some(key) = self
            .launches
            .iter()
            .find_map(|(key, launch)| (launch.task == event.task_id).then_some(*key))
        else {
            return Ok(NeedProducerOwnedTaskEventDisposition::NotLocal(event));
        };
        let launch = &self.launches[&key];
        let reject = |reason, event| NeedProducerOwnedTaskEventError { reason, event };
        if launch.generation != event.generation {
            return Err(reject(
                NeedProducerAdmissionError::StaleTaskGeneration,
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
        if launch.task_terminal {
            return Err(reject(
                NeedProducerAdmissionError::InvalidNeedTransition,
                event,
            ));
        }
        if let TaskEventKind::Ready(value) = &event.kind
            && launch.plan.policy == TaskPolicy::JoinSameKey
            && !value.value().ownership().permits_copy()
        {
            return Err(reject(
                NeedProducerAdmissionError::AffineJoinedReadyPublication,
                event,
            ));
        }
        if !matches!(event.kind, TaskEventKind::Failed(_))
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
            TaskEventKind::Failed(error) => {
                launch.task_terminal = true;
                launch.task_fault = Some(error);
            }
        }
        launch.publication = Some(cursor);
        Ok(NeedProducerOwnedTaskEventDisposition::Published)
    }

    #[must_use]
    pub fn ready_for_need(&self, need: &NeedId) -> Option<&RuntimePayload> {
        let launch = self.launch_for_need(need)?;
        match launch.state() {
            RuntimeNeedProducerState::Ready(value) => Some(value),
            _ => None,
        }
    }

    /// Supplies one owned Ready to Await. AlwaysStart transfers the sole
    /// payload and retains terminal cursor metadata; JoinSameKey copies only
    /// after publication proved the complete value unrestricted.
    pub fn take_ready_for_need(
        &mut self,
        need: &NeedId,
    ) -> Result<RuntimePayload, NeedProducerAdmissionError> {
        let proof = self.inspect_ready_take_for_need(need)?;
        Ok(self.take_ready_for_need_prepared(proof))
    }

    pub fn inspect_ready_take_for_need(
        &self,
        need: &NeedId,
    ) -> Result<NeedProducerReadyTakeProof, NeedProducerAdmissionError> {
        let (key, launch) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.need == need)
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        let RuntimeNeedProducerState::Ready(value) = &launch.state else {
            return Err(NeedProducerAdmissionError::ReadyUnavailable);
        };
        if launch.plan.policy == TaskPolicy::JoinSameKey
            && !value.value().ownership().permits_copy()
        {
            return Err(NeedProducerAdmissionError::AffineJoinedReadyPublication);
        }
        let publication = launch
            .publication
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        Ok(NeedProducerReadyTakeProof {
            key: *key,
            publication,
            policy: launch.plan.policy,
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
            launch.publication == Some(proof.publication) && launch.plan.policy == proof.policy,
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
    pub fn restore_ready_for_need(
        &mut self,
        need: &NeedId,
        value: RuntimePayload,
    ) -> Result<(), NeedProducerReadyRestoreError> {
        let proof = match self.inspect_ready_restore_for_need(need) {
            Ok(proof) => proof,
            Err(reason) => return Err(NeedProducerReadyRestoreError { reason, value }),
        };
        self.restore_ready_for_need_prepared(proof, value);
        Ok(())
    }

    pub fn inspect_ready_restore_for_need(
        &self,
        need: &NeedId,
    ) -> Result<NeedProducerReadyRestoreProof, NeedProducerAdmissionError> {
        let (key, launch) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.need == need)
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        if launch.plan.policy != TaskPolicy::AlwaysStart
            || !matches!(launch.state, RuntimeNeedProducerState::ReadyTransferred)
            || !launch.task_terminal
        {
            return Err(NeedProducerAdmissionError::ReadyUnavailable);
        }
        let publication = launch
            .publication
            .ok_or(NeedProducerAdmissionError::ReadyUnavailable)?;
        Ok(NeedProducerReadyRestoreProof {
            key: *key,
            publication,
        })
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
            launch.publication == Some(proof.publication)
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
            launch.plan.restart == HostRestartPolicy::Restartable
                && !launch.task_terminal
                && !launch.state.is_terminal()
                && !launch.task_submitted
        })
    }

    pub fn mark_task_ensured(
        &mut self,
        need: &NeedId,
    ) -> Result<Option<TaskSpec>, NeedProducerAdmissionError> {
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
            .find(|(_, launch)| &launch.need == need)
        else {
            return Ok(NeedProducerTaskEnsuredProof {
                key: None,
                task_spec: None,
            });
        };
        if launch.task_terminal || launch.state.is_terminal() || launch.task_submitted {
            return Ok(NeedProducerTaskEnsuredProof {
                key: None,
                task_spec: None,
            });
        }
        if launch.plan.restart != HostRestartPolicy::Restartable {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        Ok(NeedProducerTaskEnsuredProof {
            key: Some(*key),
            task_spec: Some(launch.task_spec.clone()),
        })
    }

    pub fn mark_task_ensured_prepared(
        &mut self,
        proof: NeedProducerTaskEnsuredProof,
    ) -> Option<TaskSpec> {
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
        proof.task_spec
    }

    fn insert_validated_restore_launch(
        &mut self,
        restore: NeedProducerLaunchRestore,
        policy: NeedProducerRestorePolicy,
    ) {
        // `restore_registry` validates the entire borrowed journal before the
        // first owner moves into this candidate.
        let checked = validate_restore_launch(&restore)
            .expect("borrowed Need registry preflight established this launch");
        let generation = restore.invocation.generation;
        let mut launch = RuntimeNeedProducerLaunch {
            generation,
            invocation: restore.invocation,
            plan: restore.plan,
            producer: checked.producer,
            arguments: restore.arguments,
            instance_key: checked.instance_key,
            ordinal: restore.ordinal,
            need: restore.need,
            task: restore.task,
            task_spec: restore.task_spec,
            state: restore.state,
            publication: restore.publication,
            task_submitted: restore.task_submitted,
            task_terminal: restore.task_terminal,
            task_fault: restore.task_fault,
        };
        let key = checked.key;
        assert!(!self.launches.contains_key(&key));
        assert!(!self.invocation_launches.contains_key(&launch.invocation));
        if launch.plan.policy == TaskPolicy::JoinSameKey
            && self
                .joined
                .insert((launch.generation, checked.instance_key), key)
                .is_some()
        {
            unreachable!("borrowed Need registry preflight checked joined uniqueness");
        }
        if let Some((counter_key, next)) = checked.launch_next {
            let entry = self.next_launch_ordinal.entry(counter_key).or_insert(1);
            *entry = (*entry).max(next);
        }
        let entry = self
            .next_invocation_sequence
            .entry(checked.invocation_key)
            .or_insert(0);
        *entry = (*entry).max(checked.invocation_next);
        if policy == NeedProducerRestorePolicy::Resume
            && launch.plan.restart == HostRestartPolicy::Restartable
            && !launch.task_terminal
        {
            launch.task_submitted = false;
        }
        self.invocation_launches.insert(launch.invocation, key);
        self.launches.insert(key, launch);
    }

    /// Restores a committed invocation alias for a previously restored
    /// JoinSameKey launch. AlwaysStart launch identities have exactly one
    /// invocation token each.
    pub fn restore_invocation_alias(
        &mut self,
        invocation: NeedProducerInvocationToken,
        need: &NeedId,
    ) -> Result<(), NeedProducerAdmissionError> {
        let Some((key, launch)) = self
            .launches
            .iter()
            .find(|(_, launch)| &launch.need == need)
        else {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        };
        if invocation.generation != launch.generation
            || invocation.producer_site != launch.plan.site
            || launch.plan.policy != TaskPolicy::JoinSameKey
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
    match (&event.kind, &launch.state) {
        (TaskEventKind::Ready(value), RuntimeNeedProducerState::Ready(current)) => {
            value == current && launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Progress(progress), RuntimeNeedProducerState::Pending(current)) => {
            progress == current && !launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Cancelled, RuntimeNeedProducerState::Cancelled) => {
            launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Failed(message), _) => {
            launch.task_terminal && launch.task_fault.as_deref() == Some(message.as_str())
        }
        _ => false,
    }
}

fn restored_need_state_is_valid(
    state: &RuntimeNeedProducerState,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
    task_fault: Option<&str>,
    restart: HostRestartPolicy,
    policy: TaskPolicy,
) -> bool {
    let cursor_matches_state = match state {
        RuntimeNeedProducerState::NotStarted => {
            publication.is_none() || (task_terminal && task_fault.is_some())
        }
        RuntimeNeedProducerState::Pending(_) => {
            publication.is_some() && !task_terminal && task_fault.is_none()
        }
        RuntimeNeedProducerState::Ready(_)
        | RuntimeNeedProducerState::ReadyTransferred
        | RuntimeNeedProducerState::Cancelled => {
            publication.is_some() && task_terminal && task_fault.is_none()
        }
    };
    cursor_matches_state
        && (!matches!(state, RuntimeNeedProducerState::ReadyTransferred)
            || policy == TaskPolicy::AlwaysStart)
        && (!matches!(state, RuntimeNeedProducerState::Ready(value) if policy == TaskPolicy::JoinSameKey && !value.value().ownership().permits_copy()))
        && task_fault.is_none_or(|_| task_terminal && !state.is_terminal())
        && (task_submitted || (!task_terminal && restart == HostRestartPolicy::Restartable))
        && (restart != HostRestartPolicy::MustBeQuiescent || task_terminal)
}

#[allow(clippy::too_many_arguments)]
fn launch_matches_plan(
    launch: &RuntimeNeedProducerLaunch,
    generation: GenerationId,
    plan: &NeedProducerTaskPlan,
    producer: &NeedProducerSpec,
    instance_key: NeedProducerInstanceKey,
    arguments: &[NeedProducerRuntimeArgument],
    request: &HostTaskRequest,
) -> bool {
    let (need, task, task_key) = producer_runtime_ids(generation, instance_key, launch.ordinal);
    let expected_task_spec = TaskSpec::new(
        task.clone(),
        task_key,
        plan.class.clone(),
        plan.priority,
        plan.cancel_scope.clone(),
        plan.policy,
        request.clone(),
    )
    .with_outcome(plan.outcome());
    launch.generation == generation
        && launch.plan == *plan
        && &launch.producer == producer
        && launch.arguments == arguments
        && launch.instance_key == instance_key
        && launch.need == need
        && launch.task == task
        && launch.task_spec == expected_task_spec
}

fn producer_runtime_ids(
    generation: GenerationId,
    instance_key: NeedProducerInstanceKey,
    ordinal: TaskLaunchOrdinal,
) -> (NeedId, TaskId, TaskKey) {
    let mut key_hasher = blake3::Hasher::new();
    key_hasher.update(b"arcweft.need.producer-task-key.v1\0");
    key_hasher.update(&generation.get().to_le_bytes());
    key_hasher.update(instance_key.as_bytes());
    let key_digest = *key_hasher.finalize().as_bytes();

    let mut launch_hasher = blake3::Hasher::new();
    launch_hasher.update(b"arcweft.need.producer-launch.v1\0");
    launch_hasher.update(&generation.get().to_le_bytes());
    launch_hasher.update(instance_key.as_bytes());
    launch_hasher.update(&ordinal.get().to_le_bytes());
    let launch_digest = *launch_hasher.finalize().as_bytes();
    let key = format!("producer.v1.{}", digest_hex(&key_digest));
    let launch = digest_hex(&launch_digest);
    (
        NeedId(format!("need.v1.{launch}")),
        TaskId(format!("task.v1.{launch}")),
        TaskKey(key),
    )
}

fn digest_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

/// Sans-I/O protocol implemented by the accepted upper View product owner.
/// Core validates only the typed request projection and never copies View
/// catalog rows or depends on `arcweft-view`.
pub trait ViewTaskPlanAuthority {
    fn validate_view_task_plan(
        &self,
        request: ViewTaskPlanValidation<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;
}

#[derive(Clone, Copy, Debug)]
pub struct ViewTaskPlanValidation<'a> {
    pub generation: GenerationId,
    pub producer: &'a NeedProducerSpec,
    pub outcome: &'a TaskOutcomeContract,
    pub request: &'a HostTaskRequest,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewTaskPlanValidationError {
    #[error("View task-plan validation rejected the generation")]
    GenerationMismatch,
    #[error("View task-plan validation rejected the producer")]
    ProducerMismatch,
    #[error("View task-plan validation rejected the outcome")]
    OutcomeMismatch,
    #[error("View task-plan validation rejected the Host request")]
    RequestMismatch,
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::sync::Arc;

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

    fn restore_launch(launch: &RuntimeNeedProducerLaunch) -> NeedProducerLaunchRestore {
        NeedProducerLaunchRestore {
            invocation: launch.invocation(),
            plan: launch.plan().clone(),
            arguments: launch.arguments().to_vec(),
            ordinal: launch.ordinal(),
            need: launch.need().clone(),
            task: launch.task().clone(),
            task_spec: launch.task_spec().clone(),
            state: launch.state().clone(),
            publication: launch.publication(),
            task_submitted: launch.task_submitted(),
            task_terminal: launch.task_terminal(),
            task_fault: launch.task_fault().map(str::to_owned),
        }
    }

    #[test]
    fn generation_zero_and_join_ordinal_are_valid_values() {
        assert_eq!(GenerationId::new(0).get(), 0);
        assert_eq!(TaskLaunchOrdinal::JOIN.get(), 0);
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
    fn producer_task_plan_seals_task_contract_but_site_remains_distinct() {
        let base = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        let another_site = producer_plan(
            3,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        assert_eq!(base.semantic_digest(), another_site.semantic_digest());
        assert_ne!(
            base.producer_spec(&[])
                .expect("producer spec")
                .instance_key()
                .expect("site-specific key"),
            another_site
                .producer_spec(&[])
                .expect("producer spec")
                .instance_key()
                .expect("site-specific key")
        );
        let changed_policy = producer_plan(
            1,
            2,
            TaskPolicy::AlwaysStart,
            HostRestartPolicy::Restartable,
            TaskClass::AssetDecode,
        );
        assert_ne!(base.semantic_digest(), changed_policy.semantic_digest());
    }

    #[test]
    fn rollback_image_restores_one_affine_ready_without_changing_dispatch_state() {
        let owner = RuntimeProgramOwner::Plan(Arc::new(
            crate::plan::RuntimePlanBuilder::new()
                .finish()
                .expect("empty owner plan"),
        ));
        let generation = GenerationId::new(3);
        let fiber = RuntimePersistentFiberId::from_allocated(7);
        let mut registry = NeedProducerRegistry::default();
        let proof = registry
            .inspect_start_visit(
                generation,
                fiber,
                producer_plan(
                    2,
                    3,
                    TaskPolicy::AlwaysStart,
                    HostRestartPolicy::Restartable,
                    TaskClass::Io,
                ),
                Vec::new(),
            )
            .expect("producer start preflight");
        let admission = registry.commit_start_visit(proof);
        let need = admission.need().clone();
        let task = admission.task_spec().id.clone();
        let ensured = registry
            .inspect_task_ensured(&need)
            .expect("task ensured preflight");
        registry.mark_task_ensured_prepared(ensured);
        let ready = RuntimeValue::Need(NeedId("need.inner.affine".to_owned()));
        assert!(!ready.ownership().permits_copy());
        let event = TaskEvent {
            generation,
            logical_epoch: LogicalEpoch(1),
            task_id: task,
            sequence: TaskSequence(1),
            publication_revision: TaskPublicationRevision::FIRST,
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
            restored.take_ready_for_need(&need).unwrap().into_value(),
            RuntimeValue::Need(NeedId("need.inner.affine".to_owned()))
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
        let join_plan = producer_plan(
            1,
            2,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let mut registry = NeedProducerRegistry::default();
        let first_token = registry
            .begin_invocation(generation, fiber, join_plan.site())
            .expect("first token");
        let first = registry
            .admit_start(first_token, join_plan.clone(), Vec::new())
            .expect("first join producer");
        assert_eq!(first.disposition(), NeedProducerTaskDisposition::Ensure);
        assert_eq!(
            registry.launch_for_need(first.need()).unwrap().ordinal(),
            TaskLaunchOrdinal::JOIN
        );

        let replay = registry
            .admit_start(first_token, join_plan.clone(), Vec::new())
            .expect("same accepted invocation replays");
        assert_eq!(replay.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(replay.need(), first.need());
        assert_eq!(replay.task_spec().id, first.task_spec().id);

        let joined_token = registry
            .begin_invocation(generation, fiber, join_plan.site())
            .expect("next checked invocation");
        let joined = registry
            .admit_start(joined_token, join_plan.clone(), Vec::new())
            .expect("join existing producer");
        assert_eq!(joined.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(joined.need(), first.need());
        assert_eq!(registry.invocations().count(), 2);

        let changed_plan = producer_plan(
            1,
            4,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        assert_eq!(
            registry.admit_start(first_token, changed_plan, Vec::new()),
            Err(NeedProducerAdmissionError::InvocationSpecificationConflict)
        );

        let always_plan = producer_plan(
            2,
            3,
            TaskPolicy::AlwaysStart,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let always_first_token = registry
            .begin_invocation(generation, fiber, always_plan.site())
            .expect("AlwaysStart token 1");
        let always_first = registry
            .admit_start(always_first_token, always_plan.clone(), Vec::new())
            .expect("AlwaysStart launch 1");
        let always_second_token = registry
            .begin_invocation(generation, fiber, always_plan.site())
            .expect("AlwaysStart token 2");
        let always_second = registry
            .admit_start(always_second_token, always_plan, Vec::new())
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
        assert_eq!(always_first.task_spec().key, always_second.task_spec().key);
        assert_ne!(always_first.need(), always_second.need());
        assert_ne!(always_first.task_spec().id, always_second.task_spec().id);

        let next_generation = GenerationId::new(9);
        let always_plan = registry
            .launch_for_need(always_second.need())
            .unwrap()
            .plan()
            .clone();
        let next_generation_token = registry
            .begin_invocation(next_generation, fiber, always_plan.site())
            .expect("new generation token");
        let next_generation_launch = registry
            .admit_start(next_generation_token, always_plan, Vec::new())
            .expect("new generation launch");
        assert_ne!(next_generation_launch.need(), always_second.need());
        assert_ne!(
            next_generation_launch.task_spec().key,
            always_second.task_spec().key
        );
    }

    #[test]
    fn need_producer_restore_preserves_aliases_and_reensures_once() {
        let generation = GenerationId::new(11);
        let fiber = RuntimePersistentFiberId::from_allocated(23);
        let plan = producer_plan(
            5,
            6,
            TaskPolicy::JoinSameKey,
            HostRestartPolicy::Restartable,
            TaskClass::Io,
        );
        let mut registry = NeedProducerRegistry::default();
        let first = registry
            .begin_invocation(generation, fiber, plan.site())
            .expect("first invocation");
        let first_launch = registry
            .admit_start(first, plan.clone(), Vec::new())
            .expect("first launch");
        let second = registry
            .begin_invocation(generation, fiber, plan.site())
            .expect("join invocation");
        registry
            .admit_start(second, plan, Vec::new())
            .expect("join launch");

        let snapshot = NeedProducerRegistryRestore {
            launches: registry.launches().map(restore_launch).collect(),
            invocations: registry
                .invocations()
                .map(|(token, launch)| (token, launch.need().clone()))
                .collect(),
            invocation_frontiers: registry.invocation_frontiers(),
            launch_frontiers: registry.launch_frontiers(),
        };
        let mut restored = NeedProducerRegistry::default();
        restored
            .restore_registry(snapshot)
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
        assert_eq!(reissued, *first_launch.task_spec());
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
            let plan = producer_plan(7, 8, policy, HostRestartPolicy::Restartable, TaskClass::Io);
            let mut registry = NeedProducerRegistry::default();
            let invocation = registry
                .begin_invocation(generation, fiber, plan.site())
                .expect("producer invocation");
            registry
                .admit_start(invocation, plan, Vec::new())
                .expect("producer launch");
            let mut snapshot = NeedProducerRegistryRestore {
                launches: registry.launches().map(restore_launch).collect(),
                invocations: registry
                    .invocations()
                    .map(|(token, launch)| (token, launch.need().clone()))
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
                restored.restore_registry(snapshot),
                Err(NeedProducerAdmissionError::InvalidRestoredLaunch)
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
            let plan = producer_plan(9, 10, policy, HostRestartPolicy::Restartable, TaskClass::Io);
            let mut registry = NeedProducerRegistry::default();
            let invocation = registry
                .begin_invocation(generation, fiber, plan.site())
                .expect("producer invocation");
            registry
                .admit_start(invocation, plan, Vec::new())
                .expect("producer launch");
            let mut snapshot = NeedProducerRegistryRestore {
                launches: registry.launches().map(restore_launch).collect(),
                invocations: registry
                    .invocations()
                    .map(|(token, launch)| (token, launch.need().clone()))
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
                restored.restore_registry(snapshot),
                Err(NeedProducerAdmissionError::InvalidRestoredLaunch)
            );
            assert_eq!(restored.launches().count(), 0);
        }
    }
}
