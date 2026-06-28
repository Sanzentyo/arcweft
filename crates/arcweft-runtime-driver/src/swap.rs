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
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{FlowRuntimeId, RuntimePureHelper};
use arcweft_core::source::SourcePlan;
use arcweft_core::stream::StreamPlan;
use arcweft_render_text::LineDisplayCatalog;
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
    pub bytecode_abi: u32,
    pub code_slots: BTreeMap<CodeSlotId, CodeSlot>,
    pub state_layouts: BTreeMap<StateId, TypeLayoutHash>,
    pub adapter_requirements: BundleDigest,
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
    #[error("failed to fingerprint compact adapter requirements: {message}")]
    AdapterRequirementFingerprint { message: String },
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
    pub fn empty(id: GenerationId, content_root: BundleDigest) -> Self {
        Self {
            id,
            content_root,
            bytecode_abi: BYTECODE_ABI_VERSION,
            code_slots: BTreeMap::new(),
            state_layouts: BTreeMap::new(),
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
        )
    }

    pub fn from_verified_bytecode(
        id: GenerationId,
        bytecode: &BytecodeProgram,
        content_root: BundleDigest,
        adapter_requirements: BundleDigest,
    ) -> Result<Self, GenerationBuildError> {
        Ok(Self {
            id,
            content_root,
            bytecode_abi: bytecode.abi_version,
            code_slots: code_slots(bytecode)?,
            state_layouts: BTreeMap::new(),
            adapter_requirements,
        })
    }

    pub fn from_verified_awbc(
        id: GenerationId,
        program: &AwbcProgram,
        content_root: BundleDigest,
        adapter_requirements: BundleDigest,
    ) -> Result<Self, GenerationBuildError> {
        Ok(Self {
            id,
            content_root,
            bytecode_abi: program.header.abi_version,
            code_slots: awbc_code_slots(program)?,
            state_layouts: BTreeMap::new(),
            adapter_requirements,
        })
    }
}

fn content_root(bundle: &ArcweftBundle) -> Result<BundleDigest, GenerationBuildError> {
    #[derive(Serialize)]
    struct ContentFingerprint<'a> {
        display: &'a LineDisplayCatalog,
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
        display: &bundle.display,
        virtual_files,
        image_assets,
        audio: serde_json::to_value(&bundle.audio)?,
        image_objects,
    })
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
                CodeSlotId(flow.id.0.clone()),
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
    program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| awbc_function_code_slot(program, index, function))
        .collect()
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
        awbc_function_code_slot_id(index, public_id),
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

fn awbc_function_code_slot_id(index: usize, public_id: Option<&str>) -> CodeSlotId {
    let id = public_id.map_or_else(|| format!("function.{index}"), str::to_owned);
    CodeSlotId(format!("awbc:{id}"))
}

#[derive(Serialize)]
struct ProgramTablesFingerprint<'a> {
    entry_flow: &'a Option<FlowRuntimeId>,
    entries: &'a [BytecodeEntry],
    pure_helpers: &'a [RuntimePureHelper],
    line_task_groups: &'a [LineTaskGroup],
    stream_plans: &'a [StreamPlan],
    source_plans: &'a [SourcePlan],
}

impl<'a> ProgramTablesFingerprint<'a> {
    fn new(bytecode: &'a BytecodeProgram) -> Self {
        Self {
            entry_flow: &bytecode.entry_flow,
            entries: &bytecode.entries,
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

    pub const fn label(self) -> &'static str {
        match self {
            Self::ContentOnly => "content-only",
            Self::CodeCompatible => "code-compatible",
            Self::CodeGenerational => "code-generational",
            Self::RestartRequired => "restart-required",
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
        || active.state_layouts != next.state_layouts
    {
        return SwapCompatibility::RestartRequired;
    }
    if active.code_slots == next.code_slots {
        return SwapCompatibility::ContentOnly;
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

    pub fn prepare_with_compatibility(
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
mod tests {
    use super::*;
    use arcweft_bundle::{
        BundleLaunchKind, BundleManifest, BundleRuntimeSummary, BundleSource,
        BundleVirtualFileSpace,
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
        AwbcStringId, AwbcTableRange, AwbcTerminator, AwbcTrapCode,
    };
    use arcweft_core::bytecode::{
        BYTECODE_ABI_VERSION, BytecodeEntry, BytecodeFlow, BytecodeInstruction,
    };
    use arcweft_core::plan::{
        EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget,
    };

    fn digest(value: &[u8]) -> BundleDigest {
        BundleDigest::of(value)
    }

    fn generation(id: u64, code: &'static [u8], content: &'static [u8]) -> Arc<ProgramGeneration> {
        Arc::new(ProgramGeneration {
            id: GenerationId(id),
            content_root: digest(content),
            bytecode_abi: BYTECODE_ABI_VERSION,
            code_slots: BTreeMap::from([(
                CodeSlotId("main".to_owned()),
                CodeSlot {
                    signature: RuntimeSignature {
                        params: digest(b"params"),
                        result: digest(b"result"),
                        effects: digest(b"effects"),
                    },
                    code_digest: digest(code),
                },
            )]),
            state_layouts: BTreeMap::from([(
                StateId("save.main".to_owned()),
                TypeLayoutHash(digest(b"state-layout")),
            )]),
            adapter_requirements: digest(b"adapter"),
        })
    }

    #[test]
    fn content_only_swap_does_not_require_quiescence_semantically() {
        let active = generation(1, b"code", b"old-content");
        let next = generation(2, b"code", b"new-content");

        let compatibility = classify_swap(&active, &next);

        assert_eq!(compatibility, SwapCompatibility::ContentOnly);
        assert!(compatibility.can_apply_live());
        assert!(!compatibility.requires_quiescence());
        assert_eq!(compatibility.label(), "content-only");
    }

    #[test]
    fn compatible_code_swap_commits_between_steps_and_retires_after_pins_drop() {
        let active = generation(1, b"old-code", b"content");
        let mut session = SwapSession::new(active);
        let fiber_pin = session.pin_active_generation();
        let next = generation(2, b"new-code", b"content");

        assert_eq!(
            session.prepare(next).expect("prepare"),
            SwapCompatibility::CodeCompatible
        );
        session.begin_quiescence().expect("quiesce");
        session.enter_runtime_step();
        assert_eq!(session.commit(), Err(SwapError::RuntimeNotQuiescent));
        session.finish_runtime_step();
        assert_eq!(
            session.commit().expect("commit"),
            SwapCompatibility::CodeCompatible
        );

        session.retire_unused();
        assert_eq!(session.phase(), SwapPhase::Retiring);
        assert_eq!(session.retired().len(), 1);
        drop(fiber_pin);
        session.retire_unused();
        assert_eq!(session.phase(), SwapPhase::Idle);
        assert!(session.retired().is_empty());
    }

    #[test]
    fn state_layout_change_requires_restart() {
        let active = generation(1, b"code", b"content");
        let mut next = (*generation(2, b"new-code", b"content")).clone();
        next.state_layouts.insert(
            StateId("save.main".to_owned()),
            TypeLayoutHash(digest(b"changed-layout")),
        );

        assert_eq!(
            classify_swap(&active, &next),
            SwapCompatibility::RestartRequired
        );
    }

    #[test]
    fn missing_active_code_signature_is_generational() {
        let active = generation(1, b"code", b"content");
        let mut next = (*generation(2, b"new-code", b"content")).clone();
        next.code_slots.clear();

        assert_eq!(
            classify_swap(&active, &next),
            SwapCompatibility::CodeGenerational
        );
    }

    #[test]
    fn generation_from_bundle_classifies_content_only_when_only_content_changes() {
        let active = ProgramGeneration::from_bundle(
            GenerationId(1),
            &test_bundle(
                test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
                b"old",
            ),
        )
        .expect("active generation");
        let next = ProgramGeneration::from_bundle(
            GenerationId(2),
            &test_bundle(
                test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
                b"new",
            ),
        )
        .expect("next generation");

        assert_ne!(active.content_root, next.content_root);
        assert_eq!(active.code_slots, next.code_slots);
        assert_eq!(
            classify_swap(&active, &next),
            SwapCompatibility::ContentOnly
        );
    }

    #[test]
    fn generation_from_bundle_treats_structured_bytecode_change_as_generational() {
        let active = ProgramGeneration::from_bundle(
            GenerationId(1),
            &test_bundle(
                test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Noop)]),
                b"asset",
            ),
        )
        .expect("active generation");
        let next = ProgramGeneration::from_bundle(
            GenerationId(2),
            &test_bundle(
                test_bytecode(vec![BytecodeInstruction::Flow(FlowOp::Return(
                    "done".to_owned(),
                ))]),
                b"asset",
            ),
        )
        .expect("next generation");

        assert_eq!(
            classify_swap(&active, &next),
            SwapCompatibility::CodeGenerational
        );
    }

    #[test]
    fn generation_from_bundle_uses_product_awbc_function_identity() {
        let active = ProgramGeneration::from_bundle(
            GenerationId(1),
            &test_bundle(BytecodeProgram::default(), b"asset")
                .with_product_awbc(test_awbc_program("revision-a")),
        )
        .expect("active AWBC generation");
        let next = ProgramGeneration::from_bundle(
            GenerationId(2),
            &test_bundle(BytecodeProgram::default(), b"asset")
                .with_product_awbc(test_awbc_program("revision-b")),
        )
        .expect("next AWBC generation");

        assert_eq!(active.content_root, next.content_root);
        assert_ne!(active.code_slots, next.code_slots);
        assert_eq!(
            classify_swap(&active, &next),
            SwapCompatibility::CodeCompatible
        );
    }

    #[test]
    fn generation_from_bundle_rejects_unverified_bytecode() {
        let mut bytecode = test_bytecode(Vec::new());
        bytecode.abi_version = BYTECODE_ABI_VERSION + 1;

        let error =
            ProgramGeneration::from_bundle(GenerationId(1), &test_bundle(bytecode, b"asset"))
                .expect_err("unsupported ABI should reject generation");

        assert!(matches!(
            error,
            GenerationBuildError::VerifyBytecode(BytecodeVerificationError::UnsupportedAbi { .. })
        ));
    }

    fn test_bundle(bytecode: BytecodeProgram, asset_bytes: &[u8]) -> ArcweftBundle {
        let stats = bytecode.stats();
        ArcweftBundle::new(
            BundleManifest {
                source_label: "test.arcw".to_owned(),
                profile_id: None,
                profile_kind: Some(BundleLaunchKind::Game),
                entry: Some("entry.main".to_owned()),
                adapter: Some("test".to_owned()),
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: bytecode.entry_flow.as_ref().map(|flow| flow.0.clone()),
                    flows: stats.flows,
                    bytecode_instructions: stats.instructions,
                    line_task_groups: stats.line_task_groups,
                    stream_plans: stats.stream_plans,
                    source_plans: stats.source_plans,
                },
            },
            BundleSource {
                label: "test.arcw".to_owned(),
                text: "flow main { return \"ok\" }".to_owned(),
            },
            bytecode,
            LineDisplayCatalog::default(),
        )
        .with_virtual_files([BundleVirtualFile {
            space: BundleVirtualFileSpace::Asset,
            path: "asset.bin".to_owned(),
            bytes: asset_bytes.to_vec(),
        }])
    }

    fn test_awbc_program(revision: &str) -> AwbcProgram {
        let trap_code = if revision == "revision-a" {
            AwbcTrapCode::ExplicitPanic
        } else {
            AwbcTrapCode::InternalInvariant
        };
        AwbcProgram {
            strings: vec!["entry.main".to_owned(), revision.to_owned()],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![AwbcFunction {
                public_id: Some(AwbcStringId(0)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Trap {
                    code: trap_code,
                    message: None,
                },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            }],
            entries: vec![AwbcEntry {
                public_id: AwbcStringId(0),
                kind: AwbcEntryKind::Game,
                signature: AwbcSignatureId(0),
                target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
            }],
            ..AwbcProgram::default()
        }
    }

    fn test_bytecode(instructions: Vec<BytecodeInstruction>) -> BytecodeProgram {
        BytecodeProgram {
            abi_version: BYTECODE_ABI_VERSION,
            runtime_layout: arcweft_core::bytecode::BytecodeRuntimeLayout::current(),
            entry_flow: Some(FlowRuntimeId("flow.main".to_owned())),
            entries: vec![BytecodeEntry {
                id: EntryRuntimeId("entry.main".to_owned()),
                kind: RuntimeEntryKind::Game,
                target: RuntimeEntryTarget::Flow(FlowRuntimeId("flow.main".to_owned())),
            }],
            flows: vec![BytecodeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                instructions,
            }],
            pure_helpers: Vec::new(),
            line_task_groups: Vec::new(),
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
        }
    }
}
