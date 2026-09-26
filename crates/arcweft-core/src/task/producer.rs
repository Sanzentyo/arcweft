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

/// First-error identity failures shared by the standalone Cut 4 substrate.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TaskIdentityError {
    #[error("a fixed runtime identity may not be all zero")]
    ZeroFixedIdentity,
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
        let bytes = *hasher.finalize().as_bytes();
        if bytes == [0; 32] {
            return Err(TaskIdentityError::ZeroFixedIdentity);
        }
        Ok(NeedProducerInstanceKey(bytes))
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct NeedProducerInstanceKey([u8; 32]);

impl NeedProducerInstanceKey {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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
    state: Need<RuntimePayload>,
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
    pub const fn state(&self) -> &Need<RuntimePayload> {
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

/// Whether an admission must emit a host task request. Rejoining an active or
/// terminal Need does not submit the task again.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeedProducerTaskDisposition {
    Ensure,
    Reuse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NeedProducerAdmission {
    launch: RuntimeNeedProducerLaunch,
    invocation: NeedProducerInvocationToken,
    disposition: NeedProducerTaskDisposition,
}

impl NeedProducerAdmission {
    #[must_use]
    pub const fn launch(&self) -> &RuntimeNeedProducerLaunch {
        &self.launch
    }

    #[must_use]
    pub const fn disposition(&self) -> NeedProducerTaskDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn invocation(&self) -> NeedProducerInvocationToken {
        self.invocation
    }

    #[must_use]
    pub fn into_launch(self) -> RuntimeNeedProducerLaunch {
        self.launch
    }
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
    pub state: Need<RuntimePayload>,
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

impl NeedProducerRegistry {
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
        let mut candidate = self.clone();
        let admission = candidate.admit_start_inner(invocation, plan, arguments)?;
        *self = candidate;
        Ok(admission)
    }

    fn admit_start_inner(
        &mut self,
        invocation: NeedProducerInvocationToken,
        plan: NeedProducerTaskPlan,
        arguments: Vec<NeedProducerRuntimeArgument>,
    ) -> Result<NeedProducerAdmission, NeedProducerAdmissionError> {
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
            let mut launch = existing.clone();
            if disposition == NeedProducerTaskDisposition::Ensure {
                let stored = self
                    .launches
                    .get_mut(&key)
                    .ok_or(NeedProducerAdmissionError::InvalidRestoredLaunch)?;
                stored.task_submitted = true;
                launch.task_submitted = true;
            }
            return Ok(NeedProducerAdmission {
                launch,
                invocation,
                disposition,
            });
        }

        if policy == TaskPolicy::JoinSameKey {
            if let Some(key) = self.joined.get(&(generation, instance_key)).copied() {
                let existing = self
                    .launches
                    .get_mut(&key)
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
                self.invocation_launches.insert(invocation, key);
                let disposition = admission_disposition(existing);
                if disposition == NeedProducerTaskDisposition::Ensure {
                    existing.task_submitted = true;
                }
                return Ok(NeedProducerAdmission {
                    launch: existing.clone(),
                    invocation,
                    disposition,
                });
            }
        }

        let ordinal = match policy {
            TaskPolicy::JoinSameKey => TaskLaunchOrdinal::JOIN,
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
                self.next_launch_ordinal.insert(counter_key, next);
                TaskLaunchOrdinal(ordinal)
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
            state: Need::NotStarted,
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
        self.launches.insert(key, launch.clone());
        self.invocation_launches.insert(invocation, key);
        if policy == TaskPolicy::JoinSameKey {
            self.joined.insert((generation, instance_key), key);
        }
        Ok(NeedProducerAdmission {
            launch,
            invocation,
            disposition: NeedProducerTaskDisposition::Ensure,
        })
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
            TaskEventKind::Ready(value) => RuntimeNeedPublication::State {
                need,
                state: Need::Ready(value.clone()),
                cursor,
            },
            TaskEventKind::Progress(progress) => RuntimeNeedPublication::State {
                need,
                state: Need::Pending(progress.clone()),
                cursor,
            },
            TaskEventKind::Cancelled => RuntimeNeedPublication::State {
                need,
                state: Need::Cancelled,
                cursor,
            },
            TaskEventKind::Failed(message) => RuntimeNeedPublication::Failed {
                need,
                cursor,
                message: message.clone(),
            },
        })
    }

    /// Reconstructs and validates one saved launch. The supplied task
    /// specification is compared against the canonical specification derived
    /// from the typed producer/request fields before it enters the live
    /// registry. Restartable active work is returned as not yet submitted so
    /// the restored owner can ensure it exactly once.
    pub fn restore_launch(
        &mut self,
        restore: NeedProducerLaunchRestore,
    ) -> Result<(), NeedProducerAdmissionError> {
        let mut candidate = self.clone();
        candidate.restore_launch_inner(restore)?;
        *self = candidate;
        Ok(())
    }

    /// Atomically rebuilds the full launch, invocation, and counter journal.
    /// The supplied launches must already have been re-projected from the
    /// verified plan and checked saved arguments by the product owner.
    pub fn restore_registry(
        &mut self,
        restore: NeedProducerRegistryRestore,
    ) -> Result<(), NeedProducerAdmissionError> {
        let mut candidate = Self::default();
        for launch in restore.launches {
            candidate.restore_launch_inner(launch)?;
        }
        for (invocation, need) in restore.invocations {
            let already_restored = candidate
                .invocation_launches
                .get(&invocation)
                .and_then(|key| candidate.launches.get(key))
                .is_some_and(|launch| launch.need == need);
            if !already_restored {
                candidate.restore_invocation_alias(invocation, &need)?;
            }
        }
        let mut invocation_frontier_keys = BTreeSet::new();
        for frontier in restore.invocation_frontiers {
            let key = (frontier.generation, frontier.fiber, frontier.producer_site);
            let Some(inferred) = candidate.next_invocation_sequence.get(&key).copied() else {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            };
            if !invocation_frontier_keys.insert(key) || frontier.next_sequence != inferred {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
        }
        let mut launch_frontier_keys = BTreeSet::new();
        for frontier in restore.launch_frontiers {
            let key = (frontier.generation, frontier.instance_key);
            let Some(inferred) = candidate.next_launch_ordinal.get(&key).copied() else {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            };
            if !launch_frontier_keys.insert(key)
                || frontier.next_ordinal != inferred
                || frontier.next_ordinal == 0
            {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
        }
        if candidate
            .next_invocation_sequence
            .keys()
            .any(|key| !invocation_frontier_keys.contains(key))
            || invocation_frontier_keys.len() != candidate.next_invocation_sequence.len()
            || candidate
                .next_launch_ordinal
                .keys()
                .any(|key| !launch_frontier_keys.contains(key))
            || launch_frontier_keys.len() != candidate.next_launch_ordinal.len()
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        *self = candidate;
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
        let Some(launch) = self
            .launches
            .values_mut()
            .find(|launch| launch.task == event.task_id)
        else {
            return Ok(false);
        };
        if launch.generation != event.generation {
            return Err(NeedProducerAdmissionError::StaleTaskGeneration);
        }
        let cursor = TaskPublicationCursor::from_event(event);
        if let Some(observed) = launch.publication {
            match observed.compare_same_source(cursor) {
                Some(Ordering::Equal) => {
                    return if task_event_matches_launch(launch, event) {
                        Ok(false)
                    } else {
                        Err(NeedProducerAdmissionError::ConflictingTaskPublication)
                    };
                }
                Some(Ordering::Greater) => {
                    return Err(NeedProducerAdmissionError::StaleTaskPublication);
                }
                Some(Ordering::Less) => {}
                None => return Err(NeedProducerAdmissionError::InvalidPublicationSource),
            }
        }
        if launch.task_terminal {
            return Err(NeedProducerAdmissionError::InvalidNeedTransition);
        }
        let state = match &event.kind {
            TaskEventKind::Ready(value) => Need::Ready(value.clone()),
            TaskEventKind::Progress(progress) => Need::Pending(progress.clone()),
            TaskEventKind::Cancelled => Need::Cancelled,
            TaskEventKind::Failed(error) => {
                launch.task_terminal = true;
                launch.task_fault = Some(error.clone());
                launch.publication = Some(cursor);
                return Ok(true);
            }
        };
        if !need_transition_is_valid(&launch.state, &state) {
            return Err(NeedProducerAdmissionError::InvalidNeedTransition);
        }
        let terminal = state.is_terminal();
        launch.state = state;
        launch.publication = Some(cursor);
        launch.task_terminal = terminal;
        Ok(true)
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
        let Some(launch) = self
            .launches
            .values_mut()
            .find(|launch| &launch.need == need)
        else {
            return Ok(None);
        };
        if launch.task_terminal || launch.state.is_terminal() || launch.task_submitted {
            return Ok(None);
        }
        if launch.plan.restart != HostRestartPolicy::Restartable {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        launch.task_submitted = true;
        Ok(Some(launch.task_spec.clone()))
    }

    fn restore_launch_inner(
        &mut self,
        restore: NeedProducerLaunchRestore,
    ) -> Result<(), NeedProducerAdmissionError> {
        let generation = restore.invocation.generation;
        let producer = restore.plan.producer_spec(&restore.arguments)?;
        let request = restore.plan.project_request(&restore.arguments)?;
        let outcome = restore.plan.outcome();
        let instance_key = producer.instance_key()?;
        let (need, task, task_key) =
            producer_runtime_ids(generation, instance_key, restore.ordinal);
        let expected_task_spec = TaskSpec::new(
            task.clone(),
            task_key,
            restore.plan.class.clone(),
            restore.plan.priority,
            restore.plan.cancel_scope.clone(),
            restore.plan.policy,
            request,
        )
        .with_outcome(outcome);
        if restore.need != need
            || restore.task != task
            || restore.task_spec != expected_task_spec
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
            )
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        let mut launch = RuntimeNeedProducerLaunch {
            generation,
            invocation: restore.invocation,
            plan: restore.plan,
            producer,
            arguments: restore.arguments,
            instance_key,
            ordinal: restore.ordinal,
            need,
            task,
            task_spec: expected_task_spec,
            state: restore.state,
            publication: restore.publication,
            task_submitted: restore.task_submitted,
            task_terminal: restore.task_terminal,
            task_fault: restore.task_fault,
        };
        let key = NeedProducerLaunchKey {
            generation,
            instance_key,
            ordinal: launch.ordinal,
        };
        if self.launches.contains_key(&key)
            || self.invocation_launches.contains_key(&launch.invocation)
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        if launch.plan.policy == TaskPolicy::JoinSameKey
            && self
                .joined
                .insert((launch.generation, instance_key), key)
                .is_some()
        {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        if launch.plan.policy == TaskPolicy::AlwaysStart {
            if launch.ordinal == TaskLaunchOrdinal::JOIN {
                return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
            }
            let counter_key = (launch.generation, instance_key);
            let next = launch
                .ordinal
                .get()
                .checked_add(1)
                .ok_or(NeedProducerAdmissionError::LaunchOrdinalExhausted)?;
            let entry = self.next_launch_ordinal.entry(counter_key).or_insert(1);
            *entry = (*entry).max(next);
        } else if launch.ordinal != TaskLaunchOrdinal::JOIN {
            return Err(NeedProducerAdmissionError::InvalidRestoredLaunch);
        }
        let invocation_key = (
            launch.generation,
            launch.invocation.fiber,
            launch.invocation.producer_site,
        );
        let invocation_next = launch
            .invocation
            .sequence
            .checked_add(1)
            .ok_or(NeedProducerAdmissionError::InvocationSequenceExhausted)?;
        let entry = self
            .next_invocation_sequence
            .entry(invocation_key)
            .or_insert(0);
        *entry = (*entry).max(invocation_next);
        if launch.plan.restart == HostRestartPolicy::Restartable && !launch.task_terminal {
            launch.task_submitted = false;
        }
        self.invocation_launches.insert(launch.invocation, key);
        self.launches.insert(key, launch);
        Ok(())
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
        (TaskEventKind::Ready(value), Need::Ready(current)) => {
            value == current && launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Progress(progress), Need::Pending(current)) => {
            progress == current && !launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Cancelled, Need::Cancelled) => {
            launch.task_terminal && launch.task_fault.is_none()
        }
        (TaskEventKind::Failed(message), _) => {
            launch.task_terminal && launch.task_fault.as_deref() == Some(message.as_str())
        }
        _ => false,
    }
}

fn need_transition_is_valid(current: &Need<RuntimePayload>, next: &Need<RuntimePayload>) -> bool {
    match current {
        Need::NotStarted => true,
        Need::Pending(_) => matches!(next, Need::Pending(_) | Need::Ready(_) | Need::Cancelled),
        Need::Ready(value) => matches!(next, Need::Ready(next_value) if value == next_value),
        Need::Cancelled => matches!(next, Need::Cancelled),
    }
}

fn restored_need_state_is_valid(
    state: &Need<RuntimePayload>,
    publication: Option<TaskPublicationCursor>,
    task_submitted: bool,
    task_terminal: bool,
    task_fault: Option<&str>,
    restart: HostRestartPolicy,
) -> bool {
    let cursor_matches_state = match state {
        Need::NotStarted => publication.is_none() || (task_terminal && task_fault.is_some()),
        Need::Pending(_) => publication.is_some() && !task_terminal && task_fault.is_none(),
        Need::Ready(_) | Need::Cancelled => {
            publication.is_some() && task_terminal && task_fault.is_none()
        }
    };
    cursor_matches_state
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
        assert_eq!(first.launch().ordinal(), TaskLaunchOrdinal::JOIN);

        let replay = registry
            .admit_start(first_token, join_plan.clone(), Vec::new())
            .expect("same accepted invocation replays");
        assert_eq!(replay.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(replay.launch().need(), first.launch().need());
        assert_eq!(replay.launch().task(), first.launch().task());

        let joined_token = registry
            .begin_invocation(generation, fiber, join_plan.site())
            .expect("next checked invocation");
        let joined = registry
            .admit_start(joined_token, join_plan.clone(), Vec::new())
            .expect("join existing producer");
        assert_eq!(joined.disposition(), NeedProducerTaskDisposition::Reuse);
        assert_eq!(joined.launch().need(), first.launch().need());
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
        assert_eq!(always_first.launch().ordinal().get(), 1);
        assert_eq!(always_second.launch().ordinal().get(), 2);
        assert_eq!(
            always_first.launch().task_spec().key,
            always_second.launch().task_spec().key
        );
        assert_ne!(always_first.launch().need(), always_second.launch().need());
        assert_ne!(always_first.launch().task(), always_second.launch().task());

        let next_generation = GenerationId::new(9);
        let next_generation_token = registry
            .begin_invocation(next_generation, fiber, always_second.launch().plan().site())
            .expect("new generation token");
        let next_generation_launch = registry
            .admit_start(
                next_generation_token,
                always_second.launch().plan().clone(),
                Vec::new(),
            )
            .expect("new generation launch");
        assert_ne!(
            next_generation_launch.launch().need(),
            always_second.launch().need()
        );
        assert_ne!(
            next_generation_launch.launch().task_spec().key,
            always_second.launch().task_spec().key
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
            .mark_task_ensured(first_launch.launch().need())
            .expect("mark one restartable ensure")
            .expect("active task needs ensure");
        assert_eq!(reissued, *first_launch.launch().task_spec());
        assert!(
            restored
                .mark_task_ensured(first_launch.launch().need())
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
