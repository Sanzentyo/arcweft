//! Hot-swap generation model for embedding runtimes.

use arcweft_bundle::container::BundleDigest;
use arcweft_bundle::patch::PatchCompatibility;
use arcweft_bundle::resource_codec::runtime::AdapterRequirementsSection as CompactAdapterRequirementsSection;
use arcweft_bundle::{ArcweftBundle, BundleKind as ArcweftBundleKind, BundleVirtualFile};
use arcweft_core::awbc::schema::{
    AwbcBlock, AwbcFrameLayout, AwbcFunction, AwbcInstruction, AwbcProgram, AwbcSignature,
};
use arcweft_core::bytecode::{
    BYTECODE_ABI_VERSION, BytecodeEntry, BytecodeProgram, BytecodeVerificationBudget,
    BytecodeVerificationError,
};
use arcweft_core::entry::{
    AgentBudget, AgentPolicyHash, EntryBindingIdentity, RuntimeCallableRole, RuntimeNominalTypeId,
    RuntimeStatefulEntryRoles, TypeLayoutHash as CoreTypeLayoutHash,
};
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{
    EntryRuntimeId, RuntimeCallableExecutable, RuntimeEntryKind, RuntimeEntryRoles,
    RuntimeFlowExecutable, RuntimePureHelper,
};
use arcweft_core::source::SourcePlan;
use arcweft_core::stream::StreamPlan;
use arcweft_text_model::DialogueContentCatalog;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GenerationId(pub u64);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeSlotId(pub String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateId(pub String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeLayoutHash(pub BundleDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSignature {
    pub params: BundleDigest,
    pub result: BundleDigest,
    pub effects: BundleDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeSlot {
    pub signature: RuntimeSignature,
    pub code_digest: BundleDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramGeneration {
    pub id: GenerationId,
    pub content_root: BundleDigest,
    pub dialogue_content: BundleDigest,
    pub bytecode_abi: u32,
    pub code_slots: BTreeMap<CodeSlotId, CodeSlot>,
    pub state_layouts: BTreeMap<StateId, TypeLayoutHash>,
    pub entry_compatibility: BTreeMap<EntryRuntimeId, EntryCompatibility>,
    pub adapter_requirements: BundleDigest,
}

type EntryCompatibilityMaps = (
    BTreeMap<StateId, TypeLayoutHash>,
    BTreeMap<EntryRuntimeId, EntryCompatibility>,
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryCompatibility {
    Stateful(StatefulEntryCompatibility),
    Agent(AgentEntryCompatibility),
    Existing {
        kind: RuntimeEntryKind,
        binding: EntryBindingIdentity,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatefulEntryCompatibility {
    pub kind: RuntimeEntryKind,
    pub binding: EntryBindingIdentity,
    pub state_identity: RuntimeNominalTypeId,
    pub state_layout: CoreTypeLayoutHash,
    pub event_identity: RuntimeNominalTypeId,
    pub event_layout: CoreTypeLayoutHash,
    pub initializer: RuntimeCallableRole,
    pub reducer: RuntimeCallableRole,
    pub initial_flow: arcweft_core::entry::RuntimeFlowRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentEntryCompatibility {
    pub kind: RuntimeEntryKind,
    pub binding: EntryBindingIdentity,
    pub controller: RuntimeCallableRole,
    pub policy: AgentPolicyHash,
    pub budget: AgentBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapCompatibility {
    ContentOnly,
    CodeCompatible,
    CodeGenerational,
    RestartRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwapPhase {
    Idle,
    Prepared,
    Quiescing,
    Committed,
    Retiring,
}

#[derive(Clone, Debug)]
pub struct PreparedSwap {
    pub next: Arc<ProgramGeneration>,
    pub compatibility: SwapCompatibility,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SwapError {
    #[error("patch requires a full runtime restart")]
    RestartRequired,
    #[error("hot swap is in the wrong phase: expected {expected:?}, actual {actual:?}")]
    WrongPhase {
        expected: SwapPhase,
        actual: SwapPhase,
    },
    #[error("cannot commit a hot swap while a runtime step is active")]
    RuntimeNotQuiescent,
    #[error("a hot swap is already prepared")]
    SwapAlreadyPrepared,
    #[error("no hot swap is prepared")]
    NoPreparedSwap,
}

#[derive(Debug, Error)]
pub enum GenerationBuildError {
    #[error("bundle kind `{0:?}` is not supported by the game hot-swap generation model")]
    UnsupportedBundleKind(ArcweftBundleKind),
    #[error("failed to verify bundle bytecode: {0}")]
    VerifyBytecode(#[from] BytecodeVerificationError),
    #[error("failed to verify product AWBC executable: {message}")]
    ProductAwbcVerification { message: String },
    #[error("failed to encode hot-swap generation fingerprint: {0}")]
    EncodeFingerprint(#[from] serde_json::Error),
    #[error("failed to encode Product AWBC executable identity: {message}")]
    ProductAwbcIdentity { message: String },
    #[error("failed to fingerprint compact adapter requirements: {message}")]
    AdapterRequirementFingerprint { message: String },
    #[error("failed to decode executable entry kind for `{entry}`")]
    InvalidEntryKind { entry: String },
}

#[derive(Clone, Debug)]
pub struct SwapSession {
    active: Arc<ProgramGeneration>,
    retired: Vec<Arc<ProgramGeneration>>,
    phase: SwapPhase,
    prepared: Option<PreparedSwap>,
    in_step: bool,
}

impl Default for RuntimeSignature {
    fn default() -> Self {
        Self {
            params: BundleDigest::ZERO,
            result: BundleDigest::ZERO,
            effects: BundleDigest::ZERO,
        }
    }
}

impl ProgramGeneration {
    pub fn empty(
        id: GenerationId,
        content_root: BundleDigest,
        dialogue_content: BundleDigest,
    ) -> Self {
        Self {
            id,
            content_root,
            dialogue_content,
            bytecode_abi: BYTECODE_ABI_VERSION,
            code_slots: BTreeMap::new(),
            state_layouts: BTreeMap::new(),
            entry_compatibility: BTreeMap::new(),
            adapter_requirements: BundleDigest::ZERO,
        }
    }

    pub fn from_bundle(
        id: GenerationId,
        bundle: &ArcweftBundle,
    ) -> Result<Self, GenerationBuildError> {
        if bundle.bundle_kind != ArcweftBundleKind::Game {
            return Err(GenerationBuildError::UnsupportedBundleKind(
                bundle.bundle_kind,
            ));
        }
        if let Some(product_awbc) = bundle.product_awbc() {
            product_awbc.verify_product_executable().map_err(|error| {
                GenerationBuildError::ProductAwbcVerification {
                    message: error.to_string(),
                }
            })?;
            return Self::from_verified_awbc(
                id,
                product_awbc.program(),
                content_root(bundle)?,
                adapter_requirements(bundle)?,
                dialogue_content_digest(bundle)?,
            );
        }
        bundle
            .bytecode
            .program
            .verify(BytecodeVerificationBudget::default())?;
        Self::from_verified_bytecode(
            id,
            &bundle.bytecode.program,
            content_root(bundle)?,
            adapter_requirements(bundle)?,
            dialogue_content_digest(bundle)?,
        )
    }

    pub fn from_verified_bytecode(
        id: GenerationId,
        bytecode: &BytecodeProgram,
        content_root: BundleDigest,
        adapter_requirements: BundleDigest,
        dialogue_content: BundleDigest,
    ) -> Result<Self, GenerationBuildError> {
        let (state_layouts, entry_compatibility) = bytecode_entry_compatibility(bytecode);
        Ok(Self {
            id,
            content_root,
            dialogue_content,
            bytecode_abi: bytecode.abi_version,
            code_slots: code_slots(bytecode)?,
            state_layouts,
            entry_compatibility,
            adapter_requirements,
        })
    }

    pub fn from_verified_awbc(
        id: GenerationId,
        program: &AwbcProgram,
        content_root: BundleDigest,
        adapter_requirements: BundleDigest,
        dialogue_content: BundleDigest,
    ) -> Result<Self, GenerationBuildError> {
        let (state_layouts, entry_compatibility) = awbc_entry_compatibility(program)?;
        Ok(Self {
            id,
            content_root,
            dialogue_content,
            bytecode_abi: program.header.abi_version,
            code_slots: awbc_code_slots(program)?,
            state_layouts,
            entry_compatibility,
            adapter_requirements,
        })
    }
}

impl StateId {
    #[must_use]
    pub fn for_entry_root(entry: &EntryRuntimeId) -> Self {
        Self(format!("entry-root:{}", entry.canonical_label()))
    }
}

fn bytecode_entry_compatibility(bytecode: &BytecodeProgram) -> EntryCompatibilityMaps {
    let mut state_layouts = BTreeMap::new();
    let mut entries = BTreeMap::new();
    for entry in &bytecode.entries {
        insert_entry_compatibility(
            &mut state_layouts,
            &mut entries,
            entry.id.clone(),
            entry.kind.clone(),
            entry.binding,
            &entry.roles,
        );
    }
    (state_layouts, entries)
}

fn awbc_entry_compatibility(
    program: &AwbcProgram,
) -> Result<EntryCompatibilityMaps, GenerationBuildError> {
    let mut state_layouts = BTreeMap::new();
    let mut entries = BTreeMap::new();
    for entry in &program.entries {
        let kind = entry.kind.runtime_kind(&program.strings).ok_or_else(|| {
            GenerationBuildError::InvalidEntryKind {
                entry: entry.runtime_id.canonical_label(),
            }
        })?;
        insert_entry_compatibility(
            &mut state_layouts,
            &mut entries,
            entry.runtime_id.clone(),
            kind,
            entry.binding,
            &entry.roles,
        );
    }
    Ok((state_layouts, entries))
}

fn insert_entry_compatibility(
    state_layouts: &mut BTreeMap<StateId, TypeLayoutHash>,
    entries: &mut BTreeMap<EntryRuntimeId, EntryCompatibility>,
    entry: EntryRuntimeId,
    kind: RuntimeEntryKind,
    binding: EntryBindingIdentity,
    roles: &RuntimeEntryRoles,
) {
    let compatibility = match roles {
        RuntimeEntryRoles::Stateful(roles) => {
            state_layouts.insert(
                StateId::for_entry_root(&entry),
                TypeLayoutHash(BundleDigest::from_bytes(*roles.state.layout.as_bytes())),
            );
            EntryCompatibility::Stateful(stateful_compatibility(kind, binding, roles))
        }
        RuntimeEntryRoles::Agent(roles) => EntryCompatibility::Agent(AgentEntryCompatibility {
            kind,
            binding,
            controller: roles.controller.clone(),
            policy: roles.policy,
            budget: roles.budget,
        }),
        RuntimeEntryRoles::None => EntryCompatibility::Existing { kind, binding },
    };
    entries.insert(entry, compatibility);
}

fn stateful_compatibility(
    kind: RuntimeEntryKind,
    binding: EntryBindingIdentity,
    roles: &RuntimeStatefulEntryRoles,
) -> StatefulEntryCompatibility {
    StatefulEntryCompatibility {
        kind,
        binding,
        state_identity: roles.state.identity.clone(),
        state_layout: roles.state.layout,
        event_identity: roles.event.identity.clone(),
        event_layout: roles.event.layout,
        initializer: roles.initializer.clone(),
        reducer: roles.reducer.clone(),
        initial_flow: roles.initial_flow.clone(),
    }
}

fn content_root(bundle: &ArcweftBundle) -> Result<BundleDigest, GenerationBuildError> {
    #[derive(Serialize)]
    struct ContentFingerprint<'a> {
        dialogue_content: &'a DialogueContentCatalog,
        virtual_files: Vec<&'a BundleVirtualFile>,
        image_assets: Vec<&'a arcweft_bundle::BundleImageAsset>,
        audio: serde_json::Value,
        image_objects: Vec<&'a arcweft_bundle::BundleImageObject>,
    }

    let mut virtual_files = bundle.virtual_files.iter().collect::<Vec<_>>();
    virtual_files.sort_by(|left, right| {
        left.space
            .as_str()
            .cmp(right.space.as_str())
            .then_with(|| left.path.cmp(&right.path))
    });
    let mut image_assets = bundle.image_assets.iter().collect::<Vec<_>>();
    image_assets.sort_by(|left, right| left.id.cmp(&right.id));
    let mut image_objects = bundle.image_objects.iter().collect::<Vec<_>>();
    image_objects.sort_by(|left, right| left.id.cmp(&right.id));

    digest_serde(&ContentFingerprint {
        dialogue_content: &bundle.dialogue_content,
        virtual_files,
        image_assets,
        audio: serde_json::to_value(&bundle.audio)?,
        image_objects,
    })
}

fn dialogue_content_digest(bundle: &ArcweftBundle) -> Result<BundleDigest, GenerationBuildError> {
    digest_serde(&bundle.dialogue_content)
}

fn adapter_requirements(bundle: &ArcweftBundle) -> Result<BundleDigest, GenerationBuildError> {
    CompactAdapterRequirementsSection::from_bundle(bundle)
        .and_then(|section| section.canonical_digest())
        .map_err(
            |error| GenerationBuildError::AdapterRequirementFingerprint {
                message: error.to_string(),
            },
        )
}

fn code_slots(
    bytecode: &BytecodeProgram,
) -> Result<BTreeMap<CodeSlotId, CodeSlot>, GenerationBuildError> {
    let mut slots = bytecode
        .flows
        .iter()
        .map(|flow| {
            let digest = digest_serde(flow)?;
            Ok((
                CodeSlotId(format!("flow:{}", flow.id.canonical_label())),
                CodeSlot {
                    signature: conservative_signature(digest),
                    code_digest: digest,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, GenerationBuildError>>()?;
    let tables_digest = digest_serde(&ProgramTablesFingerprint::new(bytecode));
    let tables_digest = tables_digest?;
    slots.insert(
        CodeSlotId("__program_tables".to_owned()),
        CodeSlot {
            signature: conservative_signature(tables_digest),
            code_digest: tables_digest,
        },
    );
    Ok(slots)
}

fn awbc_code_slots(
    program: &AwbcProgram,
) -> Result<BTreeMap<CodeSlotId, CodeSlot>, GenerationBuildError> {
    let mut slots = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| awbc_function_code_slot(program, index, function))
        .collect::<Result<BTreeMap<_, _>, GenerationBuildError>>()?;
    slots.insert(
        CodeSlotId("__awbc_program_data".to_owned()),
        CodeSlot {
            // The canonical executable identity captures constant pools and
            // other runtime tables referenced indirectly by instructions while
            // excluding source/display metadata. Its interface is deliberately
            // stable: table-data changes replace code at a quiescent boundary
            // rather than forcing a generational ABI transition.
            signature: RuntimeSignature::default(),
            code_digest: BundleDigest::from_bytes(
                program
                    .executable_identity()
                    .map_err(|error| GenerationBuildError::ProductAwbcIdentity {
                        message: error.to_string(),
                    })?
                    .0,
            ),
        },
    );
    Ok(slots)
}

fn awbc_function_code_slot(
    program: &AwbcProgram,
    index: usize,
    function: &AwbcFunction,
) -> Result<(CodeSlotId, CodeSlot), GenerationBuildError> {
    let signature = program.signatures.get(function.signature.index());
    let frame_layout = program.frame_layouts.get(function.frame_layout.index());
    let public_id = function
        .public_id
        .and_then(|id| program.strings.get(id.index()).map(String::as_str));
    let blocks = awbc_table_range_slice(&program.blocks, function.blocks, "blocks")?;
    let blocks = blocks
        .iter()
        .map(|block| {
            let instructions =
                awbc_table_range_slice(&program.instructions, block.instructions, "instructions")?;
            Ok(AwbcFunctionBlockFingerprint {
                block,
                instructions,
            })
        })
        .collect::<Result<Vec<_>, GenerationBuildError>>()?;
    let interface_digest = digest_serde(&AwbcFunctionInterfaceFingerprint {
        public_id,
        kind: function.kind,
        signature,
        frame_layout,
        flags: function.flags,
    })?;
    let code_digest = digest_serde(&AwbcFunctionCodeFingerprint {
        public_id,
        function,
        signature,
        frame_layout,
        blocks,
    })?;
    Ok((
        awbc_function_code_slot_id(program, index, public_id),
        CodeSlot {
            signature: conservative_signature(interface_digest),
            code_digest,
        },
    ))
}

#[derive(Serialize)]
struct AwbcFunctionInterfaceFingerprint<'a> {
    public_id: Option<&'a str>,
    kind: arcweft_core::awbc::schema::AwbcFunctionKind,
    signature: Option<&'a AwbcSignature>,
    frame_layout: Option<&'a AwbcFrameLayout>,
    flags: arcweft_core::awbc::schema::AwbcFunctionFlags,
}

#[derive(Serialize)]
struct AwbcFunctionCodeFingerprint<'a> {
    public_id: Option<&'a str>,
    function: &'a AwbcFunction,
    signature: Option<&'a AwbcSignature>,
    frame_layout: Option<&'a AwbcFrameLayout>,
    blocks: Vec<AwbcFunctionBlockFingerprint<'a>>,
}

#[derive(Serialize)]
struct AwbcFunctionBlockFingerprint<'a> {
    block: &'a AwbcBlock,
    instructions: &'a [AwbcInstruction],
}

fn awbc_table_range_slice<'a, T>(
    table: &'a [T],
    range: arcweft_core::awbc::schema::AwbcTableRange,
    table_name: &'static str,
) -> Result<&'a [T], GenerationBuildError> {
    let start = usize::try_from(range.start).map_err(|_| {
        GenerationBuildError::ProductAwbcVerification {
            message: format!("AWBC {table_name} range start does not fit usize"),
        }
    })?;
    let end = usize::try_from(range.checked_end().ok_or_else(|| {
        GenerationBuildError::ProductAwbcVerification {
            message: format!("AWBC {table_name} range overflows u32"),
        }
    })?)
    .map_err(|_| GenerationBuildError::ProductAwbcVerification {
        message: format!("AWBC {table_name} range end does not fit usize"),
    })?;
    table
        .get(start..end)
        .ok_or_else(|| GenerationBuildError::ProductAwbcVerification {
            message: format!(
                "AWBC {table_name} range {start}..{end} exceeds table length {}",
                table.len()
            ),
        })
}

fn awbc_function_code_slot_id(
    program: &AwbcProgram,
    index: usize,
    public_id: Option<&str>,
) -> CodeSlotId {
    let function =
        arcweft_core::awbc::schema::AwbcFunctionId(u32::try_from(index).unwrap_or(u32::MAX));
    let id = program.flow_identity(function).map_or_else(
        || {
            public_id.map_or_else(
                || format!("function:{index}"),
                |public_id| format!("public:{public_id}"),
            )
        },
        |flow| format!("flow:{}", flow.canonical_label()),
    );
    CodeSlotId(format!("awbc:{id}"))
}

#[derive(Serialize)]
struct ProgramTablesFingerprint<'a> {
    entries: &'a [BytecodeEntry],
    callable_executables: &'a [RuntimeCallableExecutable],
    flow_executables: &'a [RuntimeFlowExecutable],
    pure_helpers: &'a [RuntimePureHelper],
    line_task_groups: &'a [LineTaskGroup],
    stream_plans: &'a [StreamPlan],
    source_plans: &'a [SourcePlan],
}

impl<'a> ProgramTablesFingerprint<'a> {
    fn new(bytecode: &'a BytecodeProgram) -> Self {
        Self {
            entries: &bytecode.entries,
            callable_executables: &bytecode.callable_executables,
            flow_executables: &bytecode.flow_executables,
            pure_helpers: &bytecode.pure_helpers,
            line_task_groups: &bytecode.line_task_groups,
            stream_plans: &bytecode.stream_plans,
            source_plans: &bytecode.source_plans,
        }
    }
}

fn conservative_signature(digest: BundleDigest) -> RuntimeSignature {
    RuntimeSignature {
        params: digest,
        result: digest,
        effects: digest,
    }
}

fn digest_serde(value: &impl Serialize) -> Result<BundleDigest, GenerationBuildError> {
    serde_json::to_vec(value)
        .map(|bytes| BundleDigest::of(&bytes))
        .map_err(GenerationBuildError::EncodeFingerprint)
}

impl SwapCompatibility {
    pub const fn can_apply_live(self) -> bool {
        !matches!(self, Self::RestartRequired)
    }

    pub const fn requires_quiescence(self) -> bool {
        !matches!(self, Self::ContentOnly)
    }

    pub const fn keeps_old_generation(self) -> bool {
        matches!(self, Self::CodeGenerational)
    }

    pub const fn from_patch_compatibility(compatibility: PatchCompatibility) -> Self {
        match compatibility {
            PatchCompatibility::ContentOnly => Self::ContentOnly,
            PatchCompatibility::CodeCompatible => Self::CodeCompatible,
            PatchCompatibility::CodeGenerational => Self::CodeGenerational,
            PatchCompatibility::RestartRequired => Self::RestartRequired,
        }
    }

    /// Returns the equivalent bundle patch compatibility class.
    #[must_use]
    pub const fn patch_compatibility(self) -> PatchCompatibility {
        match self {
            Self::ContentOnly => PatchCompatibility::ContentOnly,
            Self::CodeCompatible => PatchCompatibility::CodeCompatible,
            Self::CodeGenerational => PatchCompatibility::CodeGenerational,
            Self::RestartRequired => PatchCompatibility::RestartRequired,
        }
    }

    /// Returns the compatibility that imposes the stricter swap policy.
    #[must_use]
    pub const fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ContentOnly => "content-only",
            Self::CodeCompatible => "code-compatible",
            Self::CodeGenerational => "code-generational",
            Self::RestartRequired => "restart-required",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::ContentOnly => 0,
            Self::CodeCompatible => 1,
            Self::CodeGenerational => 2,
            Self::RestartRequired => 3,
        }
    }
}

impl std::fmt::Display for SwapCompatibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

pub fn classify_swap(active: &ProgramGeneration, next: &ProgramGeneration) -> SwapCompatibility {
    if active.bytecode_abi != next.bytecode_abi
        || active.adapter_requirements != next.adapter_requirements
    {
        return SwapCompatibility::RestartRequired;
    }
    if active.code_slots == next.code_slots {
        return if active.dialogue_content == next.dialogue_content {
            SwapCompatibility::ContentOnly
        } else {
            // A different accepted profile/product generation may not retain
            // dialogue frames or mounted Style state from the old revision.
            SwapCompatibility::CodeCompatible
        };
    }
    let signatures_compatible = active.code_slots.iter().all(|(id, active_slot)| {
        next.code_slots
            .get(id)
            .is_some_and(|next_slot| active_slot.signature == next_slot.signature)
    });
    if signatures_compatible {
        SwapCompatibility::CodeCompatible
    } else {
        SwapCompatibility::CodeGenerational
    }
}

/// Classifies a live session against the exact entry it currently executes.
///
/// Unselected entry metadata does not create a Lang-01.2 restart reason.
pub fn classify_swap_for_entry(
    active: &ProgramGeneration,
    next: &ProgramGeneration,
    active_entry: &EntryRuntimeId,
) -> SwapCompatibility {
    let (Some(active_compatibility), Some(next_compatibility)) = (
        active.entry_compatibility.get(active_entry),
        next.entry_compatibility.get(active_entry),
    ) else {
        return SwapCompatibility::RestartRequired;
    };
    if active_compatibility != next_compatibility {
        return SwapCompatibility::RestartRequired;
    }
    if let EntryCompatibility::Stateful(compatibility) = active_compatibility {
        let state = StateId::for_entry_root(active_entry);
        let expected = TypeLayoutHash(BundleDigest::from_bytes(
            *compatibility.state_layout.as_bytes(),
        ));
        if active.state_layouts.get(&state) != Some(&expected)
            || next.state_layouts.get(&state) != Some(&expected)
        {
            return SwapCompatibility::RestartRequired;
        }
    }
    classify_swap(active, next)
}

impl SwapSession {
    pub fn new(active: Arc<ProgramGeneration>) -> Self {
        Self {
            active,
            retired: Vec::new(),
            phase: SwapPhase::Idle,
            prepared: None,
            in_step: false,
        }
    }

    pub fn active(&self) -> &Arc<ProgramGeneration> {
        &self.active
    }

    pub fn retired(&self) -> &[Arc<ProgramGeneration>] {
        &self.retired
    }

    /// Returns active plus retained retired generation ids in deterministic order.
    pub fn live_generation_ids(&self) -> BTreeSet<GenerationId> {
        let mut live = BTreeSet::new();
        live.insert(self.active.id);
        live.extend(self.retired.iter().map(|generation| generation.id));
        live
    }

    pub fn active_generation_id(&self) -> GenerationId {
        self.active.id
    }

    pub const fn phase(&self) -> SwapPhase {
        self.phase
    }

    pub fn prepare(
        &mut self,
        next: Arc<ProgramGeneration>,
    ) -> Result<SwapCompatibility, SwapError> {
        let compatibility = classify_swap(&self.active, &next);
        self.prepare_with_compatibility(next, compatibility)
    }

    pub(crate) fn prepare_with_compatibility(
        &mut self,
        next: Arc<ProgramGeneration>,
        compatibility: SwapCompatibility,
    ) -> Result<SwapCompatibility, SwapError> {
        self.expect_phase(SwapPhase::Idle)?;
        if self.prepared.is_some() {
            return Err(SwapError::SwapAlreadyPrepared);
        }
        if !compatibility.can_apply_live() {
            return Err(SwapError::RestartRequired);
        }
        self.prepared = Some(PreparedSwap {
            next,
            compatibility,
        });
        self.phase = SwapPhase::Prepared;
        Ok(compatibility)
    }

    pub fn begin_quiescence(&mut self) -> Result<(), SwapError> {
        self.expect_phase(SwapPhase::Prepared)?;
        self.phase = SwapPhase::Quiescing;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<SwapCompatibility, SwapError> {
        self.expect_phase(SwapPhase::Quiescing)?;
        if self.in_step {
            return Err(SwapError::RuntimeNotQuiescent);
        }
        let prepared = self.prepared.take().ok_or(SwapError::NoPreparedSwap)?;
        let compatibility = prepared.compatibility;
        let old = std::mem::replace(&mut self.active, prepared.next);
        self.retired.push(old);
        self.phase = SwapPhase::Committed;
        Ok(compatibility)
    }

    pub fn retire_unused(&mut self) {
        self.phase = SwapPhase::Retiring;
        self.retired
            .retain(|generation| Arc::strong_count(generation) > 1);
        if self.retired.is_empty() {
            self.phase = SwapPhase::Idle;
        }
    }

    pub fn pin_active_generation(&self) -> Arc<ProgramGeneration> {
        self.active.clone()
    }

    pub fn enter_runtime_step(&mut self) {
        self.in_step = true;
    }

    pub fn finish_runtime_step(&mut self) {
        self.in_step = false;
    }

    fn expect_phase(&self, expected: SwapPhase) -> Result<(), SwapError> {
        if self.phase == expected {
            Ok(())
        } else {
            Err(SwapError::WrongPhase {
                expected,
                actual: self.phase,
            })
        }
    }
}

#[cfg(test)]
mod tests;
