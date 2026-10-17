//! Validation policy for the common static task semantic seal.
//! These bounds are not semantic digest inputs.

use super::body_semantic::task_image::RuntimeTaskPlan as SourceRuntimeTaskPlan;
use super::{
    ControlEffectContractDigest, ProducerFunctionSemanticDigest, RuntimeTaskPlanBuildCoordinate,
    TaskRequestTemplateDigest,
};
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};
use crate::task::{NeedProducerFamily, TaskClass, TaskPlanSemanticDigest};
use std::cell::Cell;

/// Static source definition owned by the executable task table. Preparation
/// uses original Line group coordinates; completed rows use issued Line proofs.
/// Both phases have exactly this one grammar.
#[derive(Debug, PartialEq)]
pub struct RuntimeTaskPlan<
    Line = RuntimeLineTaskPlanBinding,
    Function = crate::runtime_id::RuntimeFunctionSiteId,
    Request = super::RuntimeTaskRequestTemplate,
    Control = super::RuntimeControlEffectContractId,
> {
    pub(crate) producer_function: Function,
    pub(crate) family: NeedProducerFamily,
    pub(crate) class: TaskClass,
    pub(crate) request_template: Request,
    pub(crate) control_effect: Control,
    pub(crate) binding: RuntimeTaskSemanticBinding<Line>,
}

/// Original builder capabilities and semantic types use the same six-field
/// definition grammar as the completed row. A seed carries no computed key.
pub type RuntimeTaskPlanSeed = RuntimeTaskPlan<
    super::RuntimeLineTaskGroupSeedId,
    super::RuntimeFunctionSiteSeedId,
    super::RuntimeTaskRequestTemplateSeed,
    super::RuntimeControlEffectContractSeedId,
>;

/// Construction phase whose Q metadata comes from the original function ABI
/// inside the consuming seal, under its unchanged parent budget.
pub type RuntimeTaskPlanFunctionRootSeed = RuntimeTaskPlan<
    super::RuntimeLineTaskGroupSeedId,
    super::RuntimeFunctionSiteSeedId,
    (),
    super::RuntimeControlEffectContractSeedId,
>;

impl RuntimeTaskPlanFunctionRootSeed {
    #[must_use]
    pub fn new(
        producer_function: super::RuntimeFunctionSiteSeedId,
        family: NeedProducerFamily,
        class: TaskClass,
        control_effect: super::RuntimeControlEffectContractSeedId,
        binding: RuntimeTaskSemanticBinding<super::RuntimeLineTaskGroupSeedId>,
    ) -> Self {
        Self {
            producer_function,
            family,
            class,
            request_template: (),
            control_effect,
            binding,
        }
    }
}

impl RuntimeTaskPlanSeed {
    #[must_use]
    pub fn new(
        producer_function: super::RuntimeFunctionSiteSeedId,
        family: NeedProducerFamily,
        class: TaskClass,
        request_template: super::RuntimeTaskRequestTemplateSeed,
        control_effect: super::RuntimeControlEffectContractSeedId,
        binding: RuntimeTaskSemanticBinding<super::RuntimeLineTaskGroupSeedId>,
    ) -> Self {
        Self {
            producer_function,
            family,
            class,
            request_template,
            control_effect,
            binding,
        }
    }
}

impl<Line> RuntimeTaskPlan<Line> {
    #[must_use]
    pub const fn producer_function(&self) -> crate::runtime_id::RuntimeFunctionSiteId {
        self.producer_function
    }
    #[must_use]
    pub const fn family(&self) -> NeedProducerFamily {
        self.family
    }
    #[must_use]
    pub const fn class(&self) -> &TaskClass {
        &self.class
    }
    #[must_use]
    pub const fn request_template(&self) -> &super::RuntimeTaskRequestTemplate {
        &self.request_template
    }
    #[must_use]
    pub const fn control_effect(&self) -> super::RuntimeControlEffectContractId {
        self.control_effect
    }
    #[must_use]
    pub const fn binding(&self) -> &RuntimeTaskSemanticBinding<Line> {
        &self.binding
    }
}

/// Original Line source and its completed semantic proof. Only the common
/// task seal constructs this binding; codecs retain the group coordinate and
/// recompute its proof from the actual original inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLineTaskPlanBinding {
    group: crate::runtime_id::RuntimeLineTaskGroupId,
    semantic: crate::line_task::LinePlanSemanticDigest,
}

impl RuntimeLineTaskPlanBinding {
    pub(super) const fn from_original_group(
        group: crate::runtime_id::RuntimeLineTaskGroupId,
        semantic: crate::line_task::LinePlanSemanticDigest,
    ) -> Self {
        Self { group, semantic }
    }

    #[must_use]
    pub const fn group(&self) -> crate::runtime_id::RuntimeLineTaskGroupId {
        self.group
    }

    #[must_use]
    pub const fn semantic_digest(&self) -> crate::line_task::LinePlanSemanticDigest {
        self.semantic
    }
}

/// Closed binding algebra. Published Line bindings retain both the original
/// group coordinate and its completed proof, never a caller-provided hash.
#[derive(Debug, PartialEq)]
pub enum RuntimeTaskSemanticBinding<Line = RuntimeLineTaskPlanBinding> {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout {
        contract: crate::task::NeedTimeoutContractDigest,
    },
    Line {
        plan: Line,
    },
}

impl<Line> RuntimeTaskSemanticBinding<Line> {
    pub(crate) fn try_complete_line<Completed, E>(
        &self,
        complete: impl FnOnce(&Line) -> Result<Completed, E>,
    ) -> Result<RuntimeTaskSemanticBinding<Completed>, E> {
        Ok(match self {
            Self::Ordinary => RuntimeTaskSemanticBinding::Ordinary,
            Self::View => RuntimeTaskSemanticBinding::View,
            Self::AwaitManyBase => RuntimeTaskSemanticBinding::AwaitManyBase,
            Self::AwaitManyChild => RuntimeTaskSemanticBinding::AwaitManyChild,
            Self::Timeout { contract } => RuntimeTaskSemanticBinding::Timeout {
                contract: *contract,
            },
            Self::Line { plan } => RuntimeTaskSemanticBinding::Line {
                plan: complete(plan)?,
            },
        })
    }
    #[must_use]
    pub const fn kind(&self) -> RuntimeTaskSemanticBindingKind {
        match self {
            Self::Ordinary => RuntimeTaskSemanticBindingKind::Ordinary,
            Self::View => RuntimeTaskSemanticBindingKind::View,
            Self::AwaitManyBase => RuntimeTaskSemanticBindingKind::AwaitManyBase,
            Self::AwaitManyChild => RuntimeTaskSemanticBindingKind::AwaitManyChild,
            Self::Timeout { .. } => RuntimeTaskSemanticBindingKind::Timeout,
            Self::Line { .. } => RuntimeTaskSemanticBindingKind::Line,
        }
    }

    pub(crate) const fn semantic_tag(&self) -> u8 {
        self.kind().semantic_tag()
    }
}

/// Dense index issued only after the complete task table has been sealed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTaskPlanIndex(pub(super) u32);

/// The sole completed task inventory. Expected wire keys have no entry here.
#[derive(Debug, PartialEq)]
pub struct RuntimeTaskPlanTable {
    pub(super) rows: Box<[SealedRuntimeTaskPlanRow]>,
    pub(super) by_digest: std::collections::BTreeMap<TaskPlanSemanticDigest, RuntimeTaskPlanIndex>,
}

/// Borrowed proof that a task definition and its key belong to one completed
/// original table. Equal keys in another program do not transfer ownership.
///
/// ```compile_fail
/// fn encode<T: serde::Serialize>() {}
/// encode::<arcweft_core::plan::RuntimeTaskPlanRef<'static>>();
/// ```
/// ```compile_fail
/// fn decode<T: serde::de::DeserializeOwned>() {}
/// decode::<arcweft_core::plan::RuntimeTaskPlanRef<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct RuntimeTaskPlanRef<'plan> {
    table: &'plan RuntimeTaskPlanTable,
    row: &'plan SealedRuntimeTaskPlanRow,
}

impl<'plan> RuntimeTaskPlanRef<'plan> {
    #[must_use]
    pub const fn definition(self) -> &'plan RuntimeTaskPlan {
        &self.row.plan
    }

    #[must_use]
    pub const fn key(self) -> TaskPlanSemanticDigest {
        self.row.digest
    }

    #[must_use]
    pub fn belongs_to(self, program: &super::RuntimePlan) -> bool {
        std::ptr::eq(self.table, program.task_plans())
    }
}

impl std::fmt::Debug for RuntimeTaskPlanRef<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeTaskPlanRef")
            .field("key", &self.row.digest)
            .field("family", &self.row.plan.family)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RuntimeTaskPlanRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.table, other.table) && std::ptr::eq(self.row, other.row)
    }
}

impl Eq for RuntimeTaskPlanRef<'_> {}

/// One immutable result shared by the original construction namespace and
/// its completed image. Source references never carry a copied task table.
#[derive(Debug, PartialEq)]
pub(in crate::plan) struct RuntimeTaskProgramSemantics {
    pub(in crate::plan) executable: RuntimeExecutableSemanticDigest,
    pub(in crate::plan) task_plans: RuntimeTaskPlanTable,
}

#[derive(Debug, PartialEq)]
pub(super) struct SealedRuntimeTaskPlanRow {
    pub(super) plan: RuntimeTaskPlan,
    pub(super) digest: TaskPlanSemanticDigest,
}

impl RuntimeTaskPlanTable {
    /// Resolves data against the actual issued key set. This never constructs
    /// a digest from assertion bytes or treats an assertion as a hash input.
    pub(crate) fn by_expected_key(
        &self,
        expected: &crate::task::ExpectedTaskPlanKey,
    ) -> Option<RuntimeTaskPlanRef<'_>> {
        self.by_digest
            .get(expected.bytes())
            .and_then(|index| self.by_index(*index))
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    #[must_use]
    pub fn by_index(&self, index: RuntimeTaskPlanIndex) -> Option<RuntimeTaskPlanRef<'_>> {
        self.rows
            .get(index.0 as usize)
            .map(|row| RuntimeTaskPlanRef { table: self, row })
    }
    #[must_use]
    pub fn by_digest(&self, digest: TaskPlanSemanticDigest) -> Option<&RuntimeTaskPlan> {
        self.by_digest
            .get(&digest)
            .and_then(|index| self.by_index(*index))
            .map(RuntimeTaskPlanRef::definition)
    }
}

/// Issued by the complete E0..E14 transcript over the original candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeExecutableSemanticDigest([u8; 32]);

impl RuntimeExecutableSemanticDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub(crate) fn from_semantic_transcript(
        transcript: TaskSemanticEncoder<'_>,
    ) -> Result<Self, TaskSemanticEncodingError> {
        Ok(Self(*transcript.finish()?.as_bytes()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTaskSemanticBindingKind {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout,
    Line,
}

impl RuntimeTaskSemanticBindingKind {
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::View => 1,
            Self::AwaitManyBase => 2,
            Self::AwaitManyChild => 3,
            Self::Timeout => 4,
            Self::Line => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeTaskPlanValidationError {
    #[error("AWBC task plans belong to the AWBC owner")]
    AwbcPlanInStructuredOwner,
    #[error("task family {family:?} cannot use {binding:?} binding")]
    FamilyBindingMismatch {
        family: NeedProducerFamily,
        binding: RuntimeTaskSemanticBindingKind,
    },
}

impl NeedProducerFamily {
    /// One family law applies to both original and completed Line storage.
    pub fn validate_runtime_task_binding<Line>(
        self,
        binding: &RuntimeTaskSemanticBinding<Line>,
    ) -> Result<(), RuntimeTaskPlanValidationError> {
        use RuntimeTaskSemanticBinding as Binding;
        match (self, binding) {
            (
                Self::StructuredTaskPlan
                | Self::HostAdapterTask
                | Self::MakeNeedHandle
                | Self::SelectedCallable,
                Binding::Ordinary,
            )
            | (Self::ViewMatchSubscription, Binding::View)
            | (Self::AwaitManyBase, Binding::AwaitManyBase)
            | (Self::AwaitManyChild, Binding::AwaitManyChild)
            | (Self::Timeout, Binding::Timeout { .. })
            | (Self::LineTask, Binding::Line { .. }) => Ok(()),
            (Self::AwbcTaskPlan, _) => {
                Err(RuntimeTaskPlanValidationError::AwbcPlanInStructuredOwner)
            }
            (family, binding) => Err(RuntimeTaskPlanValidationError::FamilyBindingMismatch {
                family,
                binding: binding.kind(),
            }),
        }
    }
}

/// Read-only evidence borrowed from one core seal invocation. It cannot be
/// cloned, decoded, or assembled by an upper-layer caller.
///
/// ```compile_fail
/// use arcweft_core::plan::RuntimeTaskPlanDigestBase;
/// fn copy_base(base: &RuntimeTaskPlanDigestBase<'_>) {
///     let _: RuntimeTaskPlanDigestBase<'_> = base.clone();
/// }
/// ```
pub struct RuntimeTaskPlanDigestBase<'a> {
    issuer: &'a ViewTaskPlanSealInvocation,
    plan: &'a SourceRuntimeTaskPlan,
    executable: RuntimeExecutableSemanticDigest,
    producer_function: ProducerFunctionSemanticDigest,
    request_template: TaskRequestTemplateDigest,
    control_effect: ControlEffectContractDigest,
}

impl RuntimeTaskPlanDigestBase<'_> {
    #[must_use]
    pub const fn coordinate(&self) -> &RuntimeTaskPlanBuildCoordinate {
        &self.issuer.coordinate
    }
    #[must_use]
    pub const fn plan_owner_tag(&self) -> u8 {
        0
    }
    #[must_use]
    pub const fn executable_semantic_digest(&self) -> RuntimeExecutableSemanticDigest {
        self.executable
    }
    #[must_use]
    pub const fn producer_function_semantic_digest(&self) -> ProducerFunctionSemanticDigest {
        self.producer_function
    }
    #[must_use]
    pub const fn family(&self) -> NeedProducerFamily {
        self.plan.family
    }
    #[must_use]
    pub const fn task_class(&self) -> &TaskClass {
        &self.plan.class
    }
    #[must_use]
    pub const fn request_template_digest(&self) -> TaskRequestTemplateDigest {
        self.request_template
    }
    #[must_use]
    pub const fn control_effect_contract_digest(&self) -> ControlEffectContractDigest {
        self.control_effect
    }
    #[must_use]
    pub const fn binding_marker(&self) -> RuntimeTaskSemanticBindingKind {
        self.plan.binding.kind()
    }
}

/// Owning this request is the sole upper-layer authority to complete one key.
///
/// ```compile_fail
/// use arcweft_core::task::ViewTaskPlanDigestRequest;
/// fn duplicate(request: ViewTaskPlanDigestRequest<'_>) {
///     let _ = request.clone();
/// }
/// ```
///
/// ```compile_fail
/// use arcweft_core::task::ViewTaskPlanDigestRequest;
/// fn finalize_twice(request: ViewTaskPlanDigestRequest<'_>) {
///     let _ = request.finish_authority_transcript(blake3::Hasher::new());
///     let _ = request.finish_authority_transcript(blake3::Hasher::new());
/// }
/// ```
pub struct ViewTaskPlanDigestRequest<'a> {
    base: RuntimeTaskPlanDigestBase<'a>,
}

impl ViewTaskPlanDigestRequest<'_> {
    #[must_use]
    pub const fn base(&self) -> &RuntimeTaskPlanDigestBase<'_> {
        &self.base
    }

    /// Consumes the capability after the accepted View owner writes the exact
    /// version-one transcript. Core verifies this invocation's issuance record.
    #[must_use]
    pub fn finish_authority_transcript(self, hasher: blake3::Hasher) -> TaskPlanSemanticDigest {
        TaskPlanSemanticDigest::from_view_authority_transcript(self, hasher)
    }

    pub(crate) fn record_completion(self, key: TaskPlanSemanticDigest, bytes: u64) {
        self.base.issuer.completed.set(Some((key, bytes)));
    }
}

pub trait ViewTaskPlanAuthority {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ViewTaskPlanValidationError {
    #[error("View task-plan authority is already bound to its original construction")]
    AuthorityAlreadyBound,
    #[error("View task-plan bindings do not cover the exact original source set")]
    BindingSetMismatch,
    #[error("View task-plan binding occurs more than once")]
    DuplicateBinding,
    #[error("View task-plan authority is stale")]
    StaleAuthority,
    #[error("View task-plan binding is missing for {coordinate:?}")]
    MissingBinding {
        coordinate: RuntimeTaskPlanBuildCoordinate,
    },
    #[error("View task-plan coordinate belongs to another candidate")]
    CoordinateOwnerMismatch,
    #[error("View task-plan request does not have the View marker")]
    ExpectedViewMarker,
    #[error("View task-plan request does not have the View family")]
    ExpectedViewFamily,
    #[error("View task-plan program does not match its accepted owner")]
    ProgramMismatch,
    #[error("View task-plan site does not match its original source")]
    SiteMismatch,
    #[error("View task-plan admission does not match its original source")]
    AdmissionMismatch,
    #[error("View task-plan semantic work limit exceeded")]
    WorkLimitExceeded,
    #[error("View task-plan transcript arithmetic overflow")]
    ArithmeticOverflow,
    #[error("View task-plan authority did not complete this request's capability")]
    InvalidCompletion,
}

/// One record per call, borrowed by exactly one non-Clone request. A valid key
/// from a different call cannot substitute for consuming this capability.
pub(crate) struct ViewTaskPlanSealInvocation {
    coordinate: RuntimeTaskPlanBuildCoordinate,
    completed: Cell<Option<(TaskPlanSemanticDigest, u64)>>,
}

impl ViewTaskPlanSealInvocation {
    pub(crate) const fn new(coordinate: RuntimeTaskPlanBuildCoordinate) -> Self {
        Self {
            coordinate,
            completed: Cell::new(None),
        }
    }

    pub(crate) fn request<'a>(
        &'a self,
        plan: &'a SourceRuntimeTaskPlan,
        executable: RuntimeExecutableSemanticDigest,
        producer_function: ProducerFunctionSemanticDigest,
        request_template: TaskRequestTemplateDigest,
        control_effect: ControlEffectContractDigest,
    ) -> ViewTaskPlanDigestRequest<'a> {
        ViewTaskPlanDigestRequest {
            base: RuntimeTaskPlanDigestBase {
                issuer: self,
                plan,
                executable,
                producer_function,
                request_template,
                control_effect,
            },
        }
    }

    pub(crate) fn verify(
        &self,
        returned: TaskPlanSemanticDigest,
        core_prefix_bytes: u64,
    ) -> Result<u64, ViewTaskPlanValidationError> {
        match self.completed.get() {
            Some((issued, bytes)) if issued == returned && bytes > core_prefix_bytes => {
                Ok(bytes - core_prefix_bytes)
            }
            _ => Err(ViewTaskPlanValidationError::InvalidCompletion),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlanSealLimits {
    pub max_task_plan_rows: u32,
    pub max_executable_rows: u32,
    pub max_children_per_row: u32,
    pub max_function_roles: u32,
    pub max_request_roles: u32,
    pub max_control_effect_rows: u32,
    pub max_view_bindings: u32,
    pub max_transcript_bytes: u64,
    pub max_semantic_work: u64,
}

impl Default for RuntimeTaskPlanSealLimits {
    fn default() -> Self {
        Self {
            max_task_plan_rows: 65_536,
            max_executable_rows: 1_048_576,
            max_children_per_row: 65_536,
            max_function_roles: 65_536,
            max_request_roles: 65_536,
            max_control_effect_rows: 65_536,
            max_view_bindings: 65_536,
            max_transcript_bytes: 67_108_864,
            max_semantic_work: 4_194_304,
        }
    }
}
