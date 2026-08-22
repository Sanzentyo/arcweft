//! Normative Rust-shaped design for Lang-01.5.1.1.2.1.1.1.1.1.1.1.3.
//!
//! This file is design material, not a standalone crate and not a production
//! patch. Existing Arcweft names remain on their legitimate current owners.
//! Ellipses denote already accepted fields outside this child request, never an
//! optional legacy route.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

// -------------------------------------------------------------------------
// arcweft-core::plan::task_semantic — opaque digests and static row
// -------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPlanSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeExecutableSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProducerFunctionSemanticDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskRequestTemplateDigest([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlEffectContractDigest([u8; 32]);

impl TaskPlanSemanticDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    // Private to the legitimate owner module. There is deliberately no public
    // `from_bytes`, `TryFrom<[u8; 32]>`, serde decoder, or ZERO.
    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

impl RuntimeExecutableSemanticDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    // Private to the legitimate owner module. There is deliberately no public
    // `from_bytes`, `TryFrom<[u8; 32]>`, serde decoder, or ZERO.
    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

impl ProducerFunctionSemanticDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    // Private to the legitimate owner module. There is deliberately no public
    // `from_bytes`, `TryFrom<[u8; 32]>`, serde decoder, or ZERO.
    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

impl TaskRequestTemplateDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    // Private to the legitimate owner module. There is deliberately no public
    // `from_bytes`, `TryFrom<[u8; 32]>`, serde decoder, or ZERO.
    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

impl ControlEffectContractDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    // Private to the legitimate owner module. There is deliberately no public
    // `from_bytes`, `TryFrom<[u8; 32]>`, serde decoder, or ZERO.
    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTaskPlanBuildCoordinate {
    owner: RuntimePlanConstructionToken,
    ordinal: u32,
}

impl RuntimeTaskPlanBuildCoordinate {
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct RuntimePlanConstructionToken(NonZeroU32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlan {
    producer_function: RuntimeFunctionSiteId,
    family: NeedProducerFamily,
    class: TaskClass,
    request_template: RuntimeHostTaskRequestTemplate,
    control_effect: RuntimeControlEffectContractId,
    binding: RuntimeTaskSemanticBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTaskSemanticBinding {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout {
        contract: NeedTimeoutContractDigest,
    },
    Line {
        plan: LinePlanSemanticDigest,
    },
}

impl RuntimeTaskPlan {
    #[must_use]
    pub const fn producer_function(&self) -> RuntimeFunctionSiteId {
        self.producer_function
    }

    #[must_use]
    pub const fn family(&self) -> NeedProducerFamily {
        self.family
    }

    #[must_use]
    pub const fn class(&self) -> TaskClass {
        self.class
    }

    #[must_use]
    pub const fn request_template(&self) -> &RuntimeHostTaskRequestTemplate {
        &self.request_template
    }

    #[must_use]
    pub const fn control_effect(&self) -> RuntimeControlEffectContractId {
        self.control_effect
    }

    #[must_use]
    pub const fn binding(&self) -> &RuntimeTaskSemanticBinding {
        &self.binding
    }
}

impl NeedProducerFamily {
    /// This exhaustive inherent match is the sole structured-plan family/
    /// binding authority. `AwbcTaskPlan` belongs to the existing AWBC owner.
    pub fn validate_runtime_task_binding(
        self,
        binding: &RuntimeTaskSemanticBinding,
    ) -> Result<(), RuntimeTaskPlanValidationError> {
        use NeedProducerFamily as Family;
        use RuntimeTaskSemanticBinding as Binding;

        match (self, binding) {
            (Family::StructuredTaskPlan, Binding::Ordinary)
            | (Family::HostAdapterTask, Binding::Ordinary)
            | (Family::MakeNeedHandle, Binding::Ordinary)
            | (Family::ViewMatchSubscription, Binding::View)
            | (Family::AwaitManyBase, Binding::AwaitManyBase)
            | (Family::AwaitManyChild, Binding::AwaitManyChild)
            | (Family::Timeout, Binding::Timeout { .. })
            | (Family::LineTask, Binding::Line { .. }) => Ok(()),
            (Family::AwbcTaskPlan, _) => {
                Err(RuntimeTaskPlanValidationError::AwbcPlanInStructuredOwner)
            }
            _ => Err(RuntimeTaskPlanValidationError::FamilyBindingMismatch {
                family: self,
                binding: binding.kind(),
            }),
        }
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

impl RuntimeTaskSemanticBinding {
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

    #[must_use]
    pub const fn semantic_tag(&self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::View => 1,
            Self::AwaitManyBase => 2,
            Self::AwaitManyChild => 3,
            Self::Timeout { .. } => 4,
            Self::Line { .. } => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlanIndex(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTaskPlanTable {
    rows: Box<[SealedRuntimeTaskPlanRow]>,
    by_digest: BTreeMap<TaskPlanSemanticDigest, RuntimeTaskPlanIndex>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SealedRuntimeTaskPlanRow {
    plan: RuntimeTaskPlan,
    digest: TaskPlanSemanticDigest,
}

impl RuntimeTaskPlanTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn by_index(
        &self,
        index: RuntimeTaskPlanIndex,
    ) -> Option<(&RuntimeTaskPlan, TaskPlanSemanticDigest)> {
        self.rows
            .get(index.0 as usize)
            .map(|row| (&row.plan, row.digest))
    }

    #[must_use]
    pub fn by_digest(
        &self,
        digest: TaskPlanSemanticDigest,
    ) -> Option<&RuntimeTaskPlan> {
        self.by_digest
            .get(&digest)
            .and_then(|index| self.rows.get(index.0 as usize))
            .map(|row| &row.plan)
    }
}

// -------------------------------------------------------------------------
// Unforgeable borrowed base and sole View protocol
// -------------------------------------------------------------------------

/// Minted only by `RuntimePlanSemanticEncoder::seal_task_plans`.
///
/// No fields are public. The type deliberately implements neither Clone, Copy,
/// Debug, Serialize, nor Deserialize.
pub struct RuntimeTaskPlanDigestBase<'a> {
    issuer: &'a RuntimePlanSemanticSealIssuer,
    coordinate: RuntimeTaskPlanBuildCoordinate,
    plan: &'a RuntimeTaskPlan,
    executable: RuntimeExecutableSemanticDigest,
    producer_function: ProducerFunctionSemanticDigest,
    request_template: TaskRequestTemplateDigest,
    control_effect: ControlEffectContractDigest,
}

/// One-use View request. It owns the non-Clone base and cannot be reconstructed
/// from fields outside the core seal owner.
pub struct ViewTaskPlanDigestRequest<'a> {
    base: RuntimeTaskPlanDigestBase<'a>,
}

struct RuntimePlanSemanticSealIssuer {
    owner: RuntimePlanConstructionToken,
}

impl RuntimeTaskPlanDigestBase<'_> {
    #[must_use]
    pub const fn coordinate(&self) -> RuntimeTaskPlanBuildCoordinate {
        self.coordinate
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
    pub const fn producer_function_semantic_digest(
        &self,
    ) -> ProducerFunctionSemanticDigest {
        self.producer_function
    }

    #[must_use]
    pub const fn family(&self) -> NeedProducerFamily {
        self.plan.family
    }

    #[must_use]
    pub const fn task_class(&self) -> TaskClass {
        self.plan.class
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

impl ViewTaskPlanDigestRequest<'_> {
    #[must_use]
    pub const fn base(&self) -> &RuntimeTaskPlanDigestBase<'_> {
        &self.base
    }

    /// Capability-gated finalization for the sole authority implementation.
    ///
    /// This is not a raw digest constructor: it can be called only by consuming
    /// an owner-minted, one-use request. The method accepts a completed BLAKE3
    /// state, not View fields or a caller-provided byte sink. The production
    /// authority is required to seed and update the state with the exact
    /// version-one transcript before calling this method.
    #[must_use]
    pub fn finish_authority_transcript(
        self,
        hasher: blake3::Hasher,
    ) -> TaskPlanSemanticDigest {
        let _issuer = self.base.issuer;
        TaskPlanSemanticDigest::from_hasher_output(hasher.finalize())
    }
}

pub trait ViewTaskPlanAuthority {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewTaskPlanValidationError {
    StaleAuthority,
    MissingBinding {
        coordinate: RuntimeTaskPlanBuildCoordinate,
    },
    CoordinateOwnerMismatch,
    ExpectedViewMarker,
    ExpectedViewFamily,
    ProgramMismatch,
    SiteMismatch,
    AdmissionMismatch,
    WorkLimitExceeded,
    ArithmeticOverflow,
}

// -------------------------------------------------------------------------
// Semantic encoder, exact limits, and common seal path
// -------------------------------------------------------------------------

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

struct RuntimePlanSemanticEncoder<'a> {
    image: &'a UnsealedRuntimePlanImage,
    limits: RuntimeTaskPlanSealLimits,
    meter: SemanticWorkMeter,
    issuer: RuntimePlanSemanticSealIssuer,
}

struct SemanticWorkMeter {
    work: u64,
    transcript_bytes: u64,
}

struct UnsealedRuntimePlanImage {
    // Existing complete core tables in canonical order.
    type_table: RuntimePlanTypeTable,
    local_declarations: RuntimeLocalDeclarationTable,
    nominal_record_domains: RuntimeNominalRecordDomainTable,
    variant_domains: RuntimeVariantDomainTable,
    function_sites: RuntimeFunctionSiteTable,
    dialogue_content: RuntimeDialogueContentPlanTable,
    entries: Box<[RuntimeEntrySpec]>,
    callable_executables: Box<[RuntimeCallableExecutable]>,
    flow_executables: Box<[RuntimeFlowExecutable]>,
    flows: Box<[RuntimeFlow]>,
    pure_helpers: Box<[RuntimePureHelper]>,
    trait_methods: Box<[RuntimeTraitMethod]>,
    line_task_groups: Box<[LineTaskGroup]>,
    stream_plans: Box<[StreamPlan]>,
    task_plans: Box<[RuntimeTaskPlan]>,
    expected_task_plan_keys: Option<Box<[ExpectedTaskPlanKey]>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedTaskPlanKey([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTaskPlanSealError {
    ArithmeticOverflow,
    LimitExceeded {
        limit: RuntimeTaskPlanLimitKind,
        actual: u64,
        allowed: u64,
    },
    Structural(RuntimeTaskPlanValidationError),
    UnknownProducerFunction {
        coordinate: RuntimeTaskPlanBuildCoordinate,
    },
    InvalidRequestTemplate {
        coordinate: RuntimeTaskPlanBuildCoordinate,
        source: TaskRequestTemplateSemanticError,
    },
    InvalidControlEffectContract {
        coordinate: RuntimeTaskPlanBuildCoordinate,
        source: ControlEffectContractSemanticError,
    },
    MissingViewTaskPlanAuthority {
        coordinate: RuntimeTaskPlanBuildCoordinate,
    },
    View {
        coordinate: RuntimeTaskPlanBuildCoordinate,
        source: ViewTaskPlanValidationError,
    },
    ExpectedKeyCountMismatch {
        rows: usize,
        keys: usize,
    },
    ExpectedKeyMismatch {
        coordinate: RuntimeTaskPlanBuildCoordinate,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    DuplicateSemanticDigest {
        digest: TaskPlanSemanticDigest,
        first: RuntimeTaskPlanBuildCoordinate,
        second: RuntimeTaskPlanBuildCoordinate,
        first_binding: RuntimeTaskSemanticBindingKind,
        second_binding: RuntimeTaskSemanticBindingKind,
    },
    FinalCrossReference(RuntimePlanError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTaskPlanLimitKind {
    TaskPlanRows,
    ExecutableRows,
    ChildrenPerRow,
    FunctionRoles,
    RequestRoles,
    ControlEffectRows,
    ViewBindings,
    TranscriptBytes,
    SemanticWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTaskPlanValidationError {
    AwbcPlanInStructuredOwner,
    FamilyBindingMismatch {
        family: NeedProducerFamily,
        binding: RuntimeTaskSemanticBindingKind,
    },
    ForeignBuildCoordinate,
    NonCanonicalCoordinate {
        expected: u32,
        actual: u32,
    },
    UnknownControlEffectContract,
    InvalidLinePlan,
    InvalidTimeoutContract,
}

impl RuntimePlanSemanticEncoder<'_> {
    fn executable_semantic_digest(
        &mut self,
        child_rows: &[ResolvedRuntimeTaskPlanChildren],
    ) -> Result<RuntimeExecutableSemanticDigest, RuntimeTaskPlanSealError> {
        // Exact transcript in TRANSCRIPTS.md. No completed task-plan key is an
        // input. All task references use construction coordinates.
        unimplemented!()
    }

    fn resolve_task_children(
        &mut self,
    ) -> Result<Box<[ResolvedRuntimeTaskPlanChildren]>, RuntimeTaskPlanSealError> {
        unimplemented!()
    }

    fn seal_task_plans(
        mut self,
        authority: Option<&dyn ViewTaskPlanAuthority>,
    ) -> Result<RuntimeTaskPlanTable, RuntimeTaskPlanSealError> {
        let children = self.resolve_task_children()?;
        let executable = self.executable_semantic_digest(&children)?;
        let rows = self
            .image
            .task_plans
            .iter()
            .zip(children.iter())
            .enumerate()
            .map(|(ordinal, (plan, children))| {
                let coordinate = RuntimeTaskPlanBuildCoordinate {
                    owner: self.issuer.owner,
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| RuntimeTaskPlanSealError::ArithmeticOverflow)?,
                };
                let base = RuntimeTaskPlanDigestBase {
                    issuer: &self.issuer,
                    coordinate,
                    plan,
                    executable,
                    producer_function: children.producer_function,
                    request_template: children.request_template,
                    control_effect: children.control_effect,
                };
                let digest = match &plan.binding {
                    RuntimeTaskSemanticBinding::View => {
                        let authority = authority.ok_or(
                            RuntimeTaskPlanSealError::MissingViewTaskPlanAuthority {
                                coordinate,
                            },
                        )?;
                        authority
                            .task_plan_semantic_digest(ViewTaskPlanDigestRequest { base })
                            .map_err(|source| RuntimeTaskPlanSealError::View {
                                coordinate,
                                source,
                            })?
                    }
                    _ => seal_core_task_plan(base)?,
                };
                Ok(SealedRuntimeTaskPlanRow {
                    plan: plan.clone(),
                    digest,
                })
            })
            .collect::<Result<Vec<_>, RuntimeTaskPlanSealError>>()?;

        verify_expected_keys(self.image.expected_task_plan_keys.as_deref(), &rows)?;
        RuntimeTaskPlanTable::try_from_sealed_rows(rows, self.issuer.owner)
    }
}

#[derive(Clone, Copy)]
struct ResolvedRuntimeTaskPlanChildren {
    producer_function: ProducerFunctionSemanticDigest,
    request_template: TaskRequestTemplateDigest,
    control_effect: ControlEffectContractDigest,
}

fn seal_core_task_plan(
    base: RuntimeTaskPlanDigestBase<'_>,
) -> Result<TaskPlanSemanticDigest, RuntimeTaskPlanSealError> {
    // Writes the accepted seven-role prefix plus the closed non-View binding.
    // The View marker is unreachable here.
    unimplemented!()
}

impl RuntimeTaskPlanTable {
    fn try_from_sealed_rows(
        rows: Vec<SealedRuntimeTaskPlanRow>,
        owner: RuntimePlanConstructionToken,
    ) -> Result<Self, RuntimeTaskPlanSealError> {
        // Source-order insertion; the second occurrence is the duplicate error.
        unimplemented!()
    }
}

fn verify_expected_keys(
    expected: Option<&[ExpectedTaskPlanKey]>,
    rows: &[SealedRuntimeTaskPlanRow],
) -> Result<(), RuntimeTaskPlanSealError> {
    unimplemented!()
}

// Inherent semantic behavior belongs to the actual owners, not extension traits.
impl RuntimeHostTaskRequestTemplate {
    fn semantic_digest(
        &self,
        context: &RuntimePlanSemanticContext<'_>,
    ) -> Result<TaskRequestTemplateDigest, TaskRequestTemplateSemanticError> {
        unimplemented!()
    }
}

impl RuntimeControlEffectContract {
    fn semantic_digest(
        &self,
        context: &RuntimePlanSemanticContext<'_>,
    ) -> Result<ControlEffectContractDigest, ControlEffectContractSemanticError> {
        unimplemented!()
    }
}

struct RuntimePlanSemanticContext<'a> {
    image: &'a UnsealedRuntimePlanImage,
    meter: &'a mut SemanticWorkMeter,
}

// -------------------------------------------------------------------------
// RuntimePlanBuilder and private decode use the exact same sealer
// -------------------------------------------------------------------------

impl RuntimePlanBuilder {
    pub fn push_runtime_task_plan(
        &mut self,
        plan: RuntimeTaskPlan,
    ) -> Result<RuntimeTaskPlanBuildCoordinate, RuntimePlanBuildError> {
        unimplemented!()
    }

    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError> {
        self.finish_inner(None, RuntimeTaskPlanSealLimits::default())
    }

    pub fn finish_with_view_task_plan_authority(
        self,
        authority: &dyn ViewTaskPlanAuthority,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimePlan, RuntimePlanBuildError> {
        self.finish_inner(Some(authority), limits)
    }

    fn finish_inner(
        self,
        authority: Option<&dyn ViewTaskPlanAuthority>,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimePlan, RuntimePlanBuildError> {
        let image = self.into_unsealed_image()?;
        let task_plans = RuntimePlanSemanticEncoder::new(&image, limits)?
            .seal_task_plans(authority)?;
        RuntimePlan::try_from_sealed_image(image, task_plans)
            .map_err(RuntimePlanBuildError::from)
    }
}

struct DecodedRuntimePlanImage {
    image: UnsealedRuntimePlanImage,
    coordinate_owner: RuntimePlanConstructionToken,
}

impl DecodedRuntimePlanImage {
    /// Resolves a stored source-order ordinal to an owner-bound coordinate. It
    /// cannot mint an out-of-range or foreign coordinate.
    pub fn task_plan_coordinate(
        &self,
        ordinal: u32,
    ) -> Result<RuntimeTaskPlanBuildCoordinate, RuntimePlanDecodeError> {
        let index = usize::try_from(ordinal)
            .map_err(|_| RuntimePlanDecodeError::ArithmeticOverflow)?;
        self.image
            .task_plans
            .get(index)
            .ok_or(RuntimePlanDecodeError::UnknownTaskPlanCoordinate { ordinal })?;
        Ok(RuntimeTaskPlanBuildCoordinate {
            owner: self.coordinate_owner,
            ordinal,
        })
    }

    fn seal(
        self,
        authority: Option<&dyn ViewTaskPlanAuthority>,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimePlan, RuntimePlanDecodeError> {
        let task_plans = RuntimePlanSemanticEncoder::new_with_owner(
            &self.image,
            limits,
            self.coordinate_owner,
        )?
        .seal_task_plans(authority)?;
        RuntimePlan::try_from_sealed_image(self.image, task_plans)
            .map_err(RuntimePlanDecodeError::from)
    }
}

pub struct RuntimePlan {
    // Existing immutable fields.
    task_plans: RuntimeTaskPlanTable,
}

impl RuntimePlan {
    #[must_use]
    pub const fn task_plans(&self) -> &RuntimeTaskPlanTable {
        &self.task_plans
    }
}

// -------------------------------------------------------------------------
// arcweft-bundle::resource_codec::view::validated — actual upper owner
// -------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ValidatedViewTaskPlanBinding {
    coordinate: RuntimeTaskPlanBuildCoordinate,
    program_id: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    admission: CheckedViewMatchAdmissionDigest,
}

#[derive(Clone, Debug)]
pub struct ValidatedViewProgramResource {
    resource: ViewProgramResource,
    program_id: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    source_set_revision: SourceSetRevision,
    task_plan_bindings:
        BTreeMap<RuntimeTaskPlanBuildCoordinate, ValidatedViewTaskPlanBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewTaskPlanBindingLimits {
    pub rows: u32,
    pub semantic_work: u64,
}

impl Default for ViewTaskPlanBindingLimits {
    fn default() -> Self {
        Self {
            rows: 65_536,
            semantic_work: 4_194_304,
        }
    }
}

pub struct ValidatedViewTaskPlanBindingInput<'a> {
    pub coordinate: RuntimeTaskPlanBuildCoordinate,
    pub compiler: &'a CompilerLocalViewMatchCatalogRow,
}

impl ValidatedViewProgramResource {
    pub fn try_with_task_plan_bindings(
        resource: ViewProgramResource,
        source_map: &SourceMapSection,
        inputs: impl IntoIterator<Item = ValidatedViewTaskPlanBindingInput<'_>>,
        product_limits: ViewProductValidationLimits,
        binding_limits: ViewTaskPlanBindingLimits,
    ) -> Result<Self, ViewProductValidationError> {
        // Existing complete-product validation runs first. Binding validation
        // then checks current program/revision/site/admission and exact coverage.
        unimplemented!()
    }

    #[must_use]
    pub fn task_plan_binding(
        &self,
        coordinate: RuntimeTaskPlanBuildCoordinate,
    ) -> Option<&ValidatedViewTaskPlanBinding> {
        self.task_plan_bindings.get(&coordinate)
    }
}

impl ViewTaskPlanAuthority for ValidatedViewProgramResource {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError> {
        self.validate_current_stamp()?;
        let base = request.base();
        let binding = self
            .task_plan_binding(base.coordinate())
            .ok_or(ViewTaskPlanValidationError::MissingBinding {
                coordinate: base.coordinate(),
            })?;
        binding.validate_request(base, self)?;

        let mut hasher = blake3::Hasher::new();
        write_task_plan_base_prefix(&mut hasher, base);
        hasher.update(&[1]);
        write_bounded_string(&mut hasher, binding.program_id.as_str())?;
        hasher.update(binding.site.as_bytes());
        hasher.update(binding.admission.as_bytes());
        Ok(request.finish_authority_transcript(hasher))
    }
}

impl ValidatedViewTaskPlanBinding {
    fn validate_request(
        &self,
        base: &RuntimeTaskPlanDigestBase<'_>,
        owner: &ValidatedViewProgramResource,
    ) -> Result<(), ViewTaskPlanValidationError> {
        unimplemented!()
    }
}

// -------------------------------------------------------------------------
// Purpose-built bundle codec staging and atomic publication
// -------------------------------------------------------------------------

struct DecodedValidatedViewTaskPlanBindingImage {
    coordinate_ordinal: u32,
    program_id: ViewProgramId,
    accepted_revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    admission: CheckedViewMatchAdmissionDigest,
}

struct DecodedBundleImage {
    runtime_plan: DecodedRuntimePlanImage,
    view_program: Option<DecodedViewProgramImage>,
}

impl DecodedBundleImage {
    fn validate_and_publish(
        self,
        limits: BundleValidationLimits,
    ) -> Result<ValidatedRuntimeBundle, BundleValidationError> {
        let view = self
            .view_program
            .map(|image| image.validate_against_plan(&self.runtime_plan, limits.view))
            .transpose()?;
        let plan = self.runtime_plan.seal(
            view.as_ref().map(|program| program as &dyn ViewTaskPlanAuthority),
            limits.task_plan,
        )?;
        ValidatedRuntimeBundle::try_new_after_complete_validation(plan, view)
    }
}

// -------------------------------------------------------------------------
// Existing owner placeholders used only to make roles unambiguous here.
// -------------------------------------------------------------------------

pub struct RuntimeFunctionSiteId;
pub struct RuntimeHostTaskRequestTemplate;
pub struct RuntimeControlEffectContractId;
pub struct RuntimeControlEffectContract;
pub struct RuntimePlanBuilder;
pub struct RuntimePlanTypeTable;
pub struct RuntimeLocalDeclarationTable;
pub struct RuntimeNominalRecordDomainTable;
pub struct RuntimeVariantDomainTable;
pub struct RuntimeFunctionSiteTable;
pub struct RuntimeDialogueContentPlanTable;
pub struct RuntimeEntrySpec;
pub struct RuntimeCallableExecutable;
pub struct RuntimeFlowExecutable;
pub struct RuntimeFlow;
pub struct RuntimePureHelper;
pub struct RuntimeTraitMethod;
pub struct LineTaskGroup;
pub struct StreamPlan;
pub struct NeedTimeoutContractDigest;
pub struct LinePlanSemanticDigest;
pub struct ViewProgramId;
pub struct AcceptedViewProgramRevision;
pub struct ViewMatchSiteId;
pub struct CheckedViewMatchAdmissionDigest;
pub struct ViewProgramResource;
pub struct SourceSetRevision;
pub struct SourceMapSection;
pub struct CompilerLocalViewMatchCatalogRow;
pub struct ViewProductValidationLimits;
pub struct DecodedViewProgramImage;
pub struct BundleValidationLimits;
pub struct ValidatedRuntimeBundle;
pub struct RuntimePlanError;
pub struct RuntimePlanBuildError;
pub struct RuntimePlanDecodeError;
pub struct TaskRequestTemplateSemanticError;
pub struct ControlEffectContractSemanticError;
pub struct ViewProductValidationError;
pub struct BundleValidationError;
pub struct TaskClass;
pub struct NeedProducerFamily;

impl ViewProgramId {
    pub fn as_str(&self) -> &str {
        unimplemented!()
    }
}
impl ViewMatchSiteId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        unimplemented!()
    }
}
impl CheckedViewMatchAdmissionDigest {
    pub fn as_bytes(&self) -> &[u8; 32] {
        unimplemented!()
    }
}
