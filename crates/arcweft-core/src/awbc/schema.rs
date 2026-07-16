use crate::entry::{
    EntryBindingIdentity, RuntimeCallableRole, RuntimeEntryRoles, RuntimeFlowExecutable,
};
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};
use serde::{Deserialize, Serialize};

/// Canonical AWBC executable ABI implemented by this schema.
pub const AWBC_ABI_VERSION: u32 = 1;
/// Canonical binary codec version used inside an `AWBC` product section.
///
/// Version 7 adds first-class closure allocation/application opcodes.
/// V6 readers cannot skip the new canonical instruction payloads.
pub const AWBC_CODEC_VERSION: u16 = 7;
/// Magic at the beginning of a standalone canonical AWBC payload.
pub const AWBC_MAGIC: [u8; 8] = *b"AWBC\r\n\x1a\n";

macro_rules! awbc_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            Deserialize,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
        )]
        #[repr(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

awbc_id!(AwbcStringId, "Index into the canonical UTF-8 string table.");
awbc_id!(AwbcTypeId, "Index into the runtime type table.");
awbc_id!(AwbcConstantId, "Index into the constant table.");
awbc_id!(AwbcEffectSetId, "Index into the effect-set table.");
awbc_id!(AwbcSignatureId, "Index into the callable-signature table.");
awbc_id!(AwbcFrameLayoutId, "Index into the frame-layout table.");
awbc_id!(AwbcFunctionId, "Index into the function table.");
awbc_id!(AwbcBlockId, "Index into the global basic-block table.");
awbc_id!(
    AwbcInstructionId,
    "Index into the global instruction table."
);
awbc_id!(AwbcRegisterId, "Function-local frame register/slot index.");
awbc_id!(AwbcScopeId, "Function-local lexical scope index.");
awbc_id!(AwbcResumePointId, "Index into the resume-point table.");
awbc_id!(AwbcPatternId, "Index into the executable pattern table.");
awbc_id!(AwbcMatchArmId, "Index into the match-arm table.");
awbc_id!(AwbcChoiceId, "Index into the choice table.");
awbc_id!(AwbcChoiceOptionId, "Index into the choice-option table.");
awbc_id!(AwbcIntrinsicId, "Index into the intrinsic-call table.");
awbc_id!(AwbcHostCallId, "Index into the host-call ABI table.");
awbc_id!(AwbcTaskPlanId, "Index into the host-task plan table.");
awbc_id!(
    AwbcAudioCommandId,
    "Index into the typed audio-command payload table."
);
awbc_id!(AwbcEffectPlanId, "Index into the effect plan table.");
awbc_id!(AwbcContentUnitId, "Index into the content-unit table.");
awbc_id!(AwbcLineTaskGroupId, "Index into the line-task-group table.");
awbc_id!(AwbcLineTaskNodeId, "Index into the line-task-node table.");
awbc_id!(AwbcStreamPlanId, "Index into the stream-plan table.");
awbc_id!(AwbcSourcePlanId, "Index into the source-plan table.");
awbc_id!(AwbcPureHelperId, "Index into the pure-helper table.");
awbc_id!(
    AwbcTraitMethodId,
    "Index into the trait-method callable table."
);
awbc_id!(AwbcDisplayMapId, "Index into the display-map table.");
awbc_id!(AwbcSourceMapId, "Index into the source-map table.");
awbc_id!(AwbcResourceId, "Index into the resource-reference table.");
awbc_id!(AwbcEntryId, "Index into the public entrypoint table.");

/// Half-open range into a table. The field using the range determines its item type.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcTableRange {
    pub start: u32,
    pub len: u32,
}

impl AwbcTableRange {
    pub const fn new(start: u32, len: u32) -> Self {
        Self { start, len }
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub const fn checked_end(self) -> Option<u32> {
        self.start.checked_add(self.len)
    }
}

/// Fixed-size digest stored as data. Hashing policy belongs to the producer/consumer.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct AwbcDigest(pub [u8; 32]);

/// Canonical executable payload. All identifiers are indices into these tables.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcProgram {
    pub header: AwbcHeader,
    pub strings: Vec<String>,
    pub runtime_types: Vec<AwbcRuntimeType>,
    pub constants: Vec<AwbcConstant>,
    pub effect_sets: Vec<AwbcEffectSet>,
    pub signatures: Vec<AwbcSignature>,
    pub frame_layouts: Vec<AwbcFrameLayout>,
    pub functions: Vec<AwbcFunction>,
    pub blocks: Vec<AwbcBlock>,
    pub instructions: Vec<AwbcInstruction>,
    pub resume_points: Vec<AwbcResumePoint>,
    pub patterns: Vec<AwbcPattern>,
    pub match_arms: Vec<AwbcMatchArm>,
    pub intrinsics: Vec<AwbcIntrinsic>,
    pub host_calls: Vec<AwbcHostCall>,
    pub task_plans: Vec<AwbcTaskPlan>,
    pub audio_commands: Vec<AwbcAudioCommand>,
    pub effect_plans: Vec<AwbcEffectPlan>,
    pub choices: Vec<AwbcChoice>,
    pub choice_options: Vec<AwbcChoiceOption>,
    pub content_units: Vec<AwbcContentUnit>,
    pub line_task_groups: Vec<AwbcLineTaskGroup>,
    pub line_task_nodes: Vec<AwbcLineTaskNode>,
    pub stream_plans: Vec<AwbcStreamPlan>,
    pub source_plans: Vec<AwbcSourcePlan>,
    pub pure_helpers: Vec<AwbcPureHelper>,
    pub trait_methods: Vec<AwbcTraitMethod>,
    pub display_map: Vec<AwbcDisplayMapEntry>,
    pub source_map: Vec<AwbcSourceMapEntry>,
    pub resources: Vec<AwbcResourceRef>,
    pub callable_executables: Vec<AwbcCallableExecutable>,
    pub flow_executables: Vec<AwbcFlowExecutable>,
    pub entries: Vec<AwbcEntry>,
}

impl Default for AwbcProgram {
    fn default() -> Self {
        Self {
            header: AwbcHeader::default(),
            strings: Vec::new(),
            runtime_types: vec![AwbcRuntimeType::Unit, AwbcRuntimeType::Dynamic],
            constants: Vec::new(),
            effect_sets: vec![AwbcEffectSet::default()],
            signatures: Vec::new(),
            frame_layouts: Vec::new(),
            functions: Vec::new(),
            blocks: Vec::new(),
            instructions: Vec::new(),
            resume_points: Vec::new(),
            patterns: Vec::new(),
            match_arms: Vec::new(),
            intrinsics: Vec::new(),
            host_calls: Vec::new(),
            task_plans: Vec::new(),
            audio_commands: Vec::new(),
            effect_plans: Vec::new(),
            choices: Vec::new(),
            choice_options: Vec::new(),
            content_units: Vec::new(),
            line_task_groups: Vec::new(),
            line_task_nodes: Vec::new(),
            stream_plans: Vec::new(),
            source_plans: Vec::new(),
            pure_helpers: Vec::new(),
            trait_methods: Vec::new(),
            display_map: Vec::new(),
            source_map: Vec::new(),
            resources: Vec::new(),
            callable_executables: Vec::new(),
            flow_executables: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl AwbcProgram {
    /// Sorts and deduplicates the canonical string table while preserving every
    /// existing `AwbcStringId` reference.
    ///
    /// AWBC verification requires the UTF-8 string table to be canonical.
    /// Producers may discover strings while walking higher-level plans in
    /// semantic order; this method performs the final owned-table normalization
    /// before verification or encoding.
    pub fn canonicalize_string_table(&mut self) {
        let mut strings = self.strings.clone();
        strings.sort();
        strings.dedup();
        if strings == self.strings {
            return;
        }

        let remap = self
            .strings
            .iter()
            .map(|value| {
                let index = strings
                    .binary_search(value)
                    .unwrap_or_else(|insertion_index| insertion_index);
                u32::try_from(index).unwrap_or(u32::MAX)
            })
            .collect::<Vec<_>>();
        self.strings = strings;
        remap_program_strings(self, &remap);
    }

    pub(super) fn retain_referenced_strings(&mut self) {
        let mut referenced = vec![false; self.strings.len()];
        visit_program_strings(self, &mut |id| {
            if let Some(referenced) = referenced.get_mut(id.index()) {
                *referenced = true;
            }
        });

        let mut remap = vec![0_u32; self.strings.len()];
        let mut strings = Vec::with_capacity(referenced.iter().filter(|used| **used).count());
        for (old_index, (value, used)) in self.strings.iter().zip(referenced).enumerate() {
            if !used {
                continue;
            }
            let new_index = u32::try_from(strings.len()).unwrap_or(u32::MAX);
            remap[old_index] = new_index;
            strings.push(value.clone());
        }
        self.strings = strings;
        remap_program_strings(self, &remap);
    }
}

fn visit_string_id(id: &mut AwbcStringId, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    visitor(id);
}

fn visit_optional_string_id(
    id: &mut Option<AwbcStringId>,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    if let Some(id) = id {
        visit_string_id(id, visitor);
    }
}

fn visit_program_strings(program: &mut AwbcProgram, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    for ty in &mut program.runtime_types {
        visit_runtime_type_strings(ty, visitor);
    }
    for constant in &mut program.constants {
        visit_constant_strings(constant, visitor);
    }
    for effect_set in &mut program.effect_sets {
        for effect in &mut effect_set.effects {
            visit_string_id(effect, visitor);
        }
    }
    for layout in &mut program.frame_layouts {
        for slot in &mut layout.slots {
            visit_optional_string_id(&mut slot.name, visitor);
        }
    }
    for function in &mut program.functions {
        visit_optional_string_id(&mut function.public_id, visitor);
    }
    for instruction in &mut program.instructions {
        visit_instruction_strings(instruction, visitor);
    }
    for block in &mut program.blocks {
        visit_terminator_strings(&mut block.terminator, visitor);
    }
    for pattern in &mut program.patterns {
        visit_pattern_strings(pattern, visitor);
    }
    for intrinsic in &mut program.intrinsics {
        visit_string_id(&mut intrinsic.public_id, visitor);
    }
    for call in &mut program.host_calls {
        visit_string_id(&mut call.public_id, visitor);
        visit_string_id(&mut call.capability, visitor);
        visit_string_id(&mut call.operation, visitor);
    }
    for task in &mut program.task_plans {
        visit_string_id(&mut task.public_id, visitor);
        visit_string_id(&mut task.need_id, visitor);
        visit_string_id(&mut task.capability, visitor);
        visit_string_id(&mut task.operation, visitor);
        visit_string_id(&mut task.cancel_scope, visitor);
        for argument in &mut task.arguments {
            visit_optional_string_id(&mut argument.name, visitor);
        }
    }
    for effect in &mut program.effect_plans {
        visit_optional_string_id(&mut effect.capability, visitor);
    }
    for choice in &mut program.choices {
        visit_optional_string_id(&mut choice.public_id, visitor);
    }
    for option in &mut program.choice_options {
        visit_optional_string_id(&mut option.public_id, visitor);
        visit_string_id(&mut option.label, visitor);
    }
    for content in &mut program.content_units {
        visit_string_id(&mut content.public_id, visitor);
    }
    for group in &mut program.line_task_groups {
        for option in &mut group.options {
            visit_string_id(&mut option.name, visitor);
        }
        for handler in &mut group.cancel_handlers {
            visit_string_id(&mut handler.trigger, visitor);
        }
    }
    for node in &mut program.line_task_nodes {
        visit_line_task_node_strings(node, visitor);
    }
    for stream in &mut program.stream_plans {
        visit_string_id(&mut stream.public_id, visitor);
    }
    for source in &mut program.source_plans {
        visit_string_id(&mut source.public_id, visitor);
    }
    for helper in &mut program.pure_helpers {
        visit_string_id(&mut helper.public_id, visitor);
    }
    for method in &mut program.trait_methods {
        visit_string_id(&mut method.public_id, visitor);
    }
    for display in &mut program.display_map {
        visit_string_id(&mut display.display_key, visitor);
    }
    for source in &mut program.source_map {
        visit_string_id(&mut source.source_file, visitor);
        visit_optional_string_id(&mut source.anchor, visitor);
    }
    for resource in &mut program.resources {
        visit_string_id(&mut resource.public_id, visitor);
        visit_string_id(&mut resource.kind, visitor);
    }
    for entry in &mut program.entries {
        visit_string_id(&mut entry.public_id, visitor);
        visit_entry_kind_strings(&mut entry.kind, visitor);
        visit_entry_target_strings(&mut entry.target, visitor);
    }
}

fn remap_program_strings(program: &mut AwbcProgram, remap: &[u32]) {
    visit_program_strings(program, &mut |id| {
        if let Some(index) = remap.get(id.index()).copied() {
            id.0 = index;
        }
    });
}

fn visit_runtime_type_strings(
    ty: &mut AwbcRuntimeType,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    match ty {
        AwbcRuntimeType::Record { public_id, fields } => {
            visit_optional_string_id(public_id, visitor);
            for field in fields {
                visit_string_id(&mut field.name, visitor);
            }
        }
        AwbcRuntimeType::Variant { public_id, cases } => {
            visit_optional_string_id(public_id, visitor);
            for case in cases {
                visit_string_id(&mut case.name, visitor);
            }
        }
        AwbcRuntimeType::Unit
        | AwbcRuntimeType::Bool
        | AwbcRuntimeType::Int(_)
        | AwbcRuntimeType::UInt(_)
        | AwbcRuntimeType::F32
        | AwbcRuntimeType::F64
        | AwbcRuntimeType::String
        | AwbcRuntimeType::Char
        | AwbcRuntimeType::Duration
        | AwbcRuntimeType::EntityRef
        | AwbcRuntimeType::Tuple(_)
        | AwbcRuntimeType::Sequence(_)
        | AwbcRuntimeType::MatrixF32
        | AwbcRuntimeType::MatrixF64
        | AwbcRuntimeType::TensorF32
        | AwbcRuntimeType::TensorF64
        | AwbcRuntimeType::TaskHandle
        | AwbcRuntimeType::NeedHandle
        | AwbcRuntimeType::Dynamic => {}
    }
}

fn visit_constant_strings(constant: &mut AwbcConstant, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    match constant {
        AwbcConstant::String(id) | AwbcConstant::EntityRef(id) => visit_string_id(id, visitor),
        AwbcConstant::Unit
        | AwbcConstant::Bool(_)
        | AwbcConstant::Int { .. }
        | AwbcConstant::UInt { .. }
        | AwbcConstant::F32Bits(_)
        | AwbcConstant::F64Bits(_)
        | AwbcConstant::Char(_)
        | AwbcConstant::DurationNanos(_)
        | AwbcConstant::Tuple(_)
        | AwbcConstant::Sequence(_)
        | AwbcConstant::Range { .. }
        | AwbcConstant::Bytes(_)
        | AwbcConstant::TensorF32 { .. }
        | AwbcConstant::TensorF64 { .. } => {}
        AwbcConstant::Variant { case_name, .. } => visit_string_id(case_name, visitor),
        AwbcConstant::Record { field_names, .. } => {
            for field_name in field_names {
                visit_string_id(field_name, visitor);
            }
        }
    }
}

fn visit_instruction_strings(
    instruction: &mut AwbcInstruction,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    match instruction {
        AwbcInstruction::ProjectField { field, .. }
        | AwbcInstruction::AssignField { field, .. } => visit_string_id(field, visitor),
        AwbcInstruction::RegisterCleanup { key, .. } | AwbcInstruction::CancelCleanup { key } => {
            visit_string_id(key, visitor);
        }
        AwbcInstruction::MakeFunction {
            params,
            capture_names,
            ..
        } => {
            for param in params {
                visit_string_id(param, visitor);
            }
            for capture_name in capture_names {
                visit_string_id(capture_name, visitor);
            }
        }
        AwbcInstruction::MakeRecord { field_names, .. } => {
            for field_name in field_names {
                visit_string_id(field_name, visitor);
            }
        }
        AwbcInstruction::MakeVariant { case_name, .. } => visit_string_id(case_name, visitor),
        _ => {}
    }
}

fn visit_terminator_strings(
    terminator: &mut AwbcTerminator,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    if let AwbcTerminator::Trap { message, .. } = terminator {
        visit_optional_string_id(message, visitor);
    }
}

fn visit_pattern_strings(pattern: &mut AwbcPattern, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    match pattern {
        AwbcPattern::Entity(entity) => visit_string_id(entity, visitor),
        AwbcPattern::Variant { case_name, .. } => visit_string_id(case_name, visitor),
        AwbcPattern::Bind { .. }
        | AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Tuple(_)
        | AwbcPattern::Record { .. }
        | AwbcPattern::Sequence { .. }
        | AwbcPattern::Whole { .. } => {}
    }
}

fn visit_line_task_node_strings(
    node: &mut AwbcLineTaskNode,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    if let AwbcLineTaskNode::Child { trigger, .. } = node {
        visit_line_task_trigger_strings(trigger, visitor);
    }
}

fn visit_line_task_trigger_strings(
    trigger: &mut AwbcLineTaskTrigger,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    if let AwbcLineTaskTrigger::Mark(id) = trigger {
        visit_string_id(id, visitor);
    }
}

fn visit_entry_kind_strings(kind: &mut AwbcEntryKind, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    if let AwbcEntryKind::Custom(id) = kind {
        visit_string_id(id, visitor);
    }
}

fn visit_entry_target_strings(
    target: &mut AwbcEntryTarget,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    if let AwbcEntryTarget::Routes(routes) = target {
        for route in routes {
            visit_string_id(&mut route.method, visitor);
            visit_string_id(&mut route.path, visitor);
            for binding in &mut route.bindings {
                visit_route_binding_source_strings(&mut binding.source, visitor);
            }
        }
    }
}

fn visit_route_binding_source_strings(
    source: &mut AwbcRouteBindingSource,
    visitor: &mut dyn FnMut(&mut AwbcStringId),
) {
    match source {
        AwbcRouteBindingSource::PathParameter(id) => visit_string_id(id, visitor),
    }
}

/// Program-level ABI facts included in semantic and code-cache identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcHeader {
    pub abi_version: u32,
    pub minimum_runtime_abi: u32,
    pub feature_bits: u64,
    pub runtime_layout_digest: AwbcDigest,
    pub host_abi_digest: AwbcDigest,
}

impl Default for AwbcHeader {
    fn default() -> Self {
        Self {
            abi_version: AWBC_ABI_VERSION,
            minimum_runtime_abi: AWBC_ABI_VERSION,
            feature_bits: 0,
            runtime_layout_digest: AwbcDigest::default(),
            host_abi_digest: AwbcDigest::default(),
        }
    }
}

/// Runtime value layout visible to both the compact VM and compiled regions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcRuntimeType {
    Unit,
    Bool,
    Int(AwbcSignedIntKind),
    UInt(AwbcUnsignedIntKind),
    F32,
    F64,
    String,
    Char,
    Duration,
    EntityRef,
    Tuple(Vec<AwbcTypeId>),
    Sequence(AwbcTypeId),
    Record {
        public_id: Option<AwbcStringId>,
        fields: Vec<AwbcRecordField>,
    },
    Variant {
        public_id: Option<AwbcStringId>,
        cases: Vec<AwbcVariantCase>,
    },
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
    TaskHandle,
    NeedHandle,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcSignedIntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcUnsignedIntKind {
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRecordField {
    pub name: AwbcStringId,
    pub ty: AwbcTypeId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcVariantCase {
    pub name: AwbcStringId,
    pub payload: Option<AwbcTypeId>,
}

/// Canonical constant pool entry. Floating-point constants store exact bits.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcConstant {
    Unit,
    Bool(bool),
    Int {
        kind: AwbcSignedIntKind,
        bits: [u8; 16],
    },
    UInt {
        kind: AwbcUnsignedIntKind,
        bits: [u8; 16],
    },
    F32Bits(u32),
    F64Bits(u64),
    String(AwbcStringId),
    Char(u32),
    DurationNanos(u64),
    EntityRef(AwbcStringId),
    Tuple(Vec<AwbcConstantId>),
    Sequence(Vec<AwbcConstantId>),
    Record {
        ty: AwbcTypeId,
        field_names: Vec<AwbcStringId>,
        fields: Vec<AwbcConstantId>,
    },
    Variant {
        ty: AwbcTypeId,
        case: u32,
        case_name: AwbcStringId,
        payload: Option<AwbcConstantId>,
    },
    Range {
        start: Option<AwbcConstantId>,
        end: Option<AwbcConstantId>,
        inclusive: bool,
    },
    Bytes(Vec<u8>),
    TensorF32 {
        shape: Vec<u32>,
        values: Vec<u32>,
    },
    TensorF64 {
        shape: Vec<u32>,
        values: Vec<u64>,
    },
}

/// Sorted set of stable effect identifiers.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcEffectSet {
    pub effects: Vec<AwbcStringId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcSignature {
    pub params: Vec<AwbcTypeId>,
    pub result: Option<AwbcTypeId>,
    pub effects: AwbcEffectSetId,
}

/// Register frame layout. Registers, parameters, locals, and temporaries use one
/// stable slot vector so VM and compiled code exchange state without projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFrameLayout {
    pub slots: Vec<AwbcFrameSlot>,
    pub max_scope_depth: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFrameSlot {
    pub name: Option<AwbcStringId>,
    pub ty: AwbcTypeId,
    pub role: AwbcFrameSlotRole,
    pub scope_depth: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcFrameSlotRole {
    Parameter,
    Local,
    Temporary,
    ReturnValue,
    RuntimeState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFunction {
    pub public_id: Option<AwbcStringId>,
    pub kind: AwbcFunctionKind,
    pub signature: AwbcSignatureId,
    pub frame_layout: AwbcFrameLayoutId,
    pub blocks: AwbcTableRange,
    pub entry_block: AwbcBlockId,
    pub flags: AwbcFunctionFlags,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcFunctionKind {
    Flow,
    PureHelper,
    TraitMethod,
    StreamTransform,
    SourceOpen,
    SourceHandler,
    LineTask,
    Synthetic,
}

impl AwbcFunctionKind {
    #[must_use]
    pub const fn is_flow(self) -> bool {
        matches!(self, Self::Flow)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFunctionFlags(pub u32);

impl AwbcFunctionFlags {
    pub const MAY_SUSPEND: u32 = 1 << 0;
    pub const MAY_ALLOCATE: u32 = 1 << 1;
    pub const DETERMINISTIC: u32 = 1 << 2;
    pub const HAS_DYNAMIC_TARGET: u32 = 1 << 3;

    pub const fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcBlock {
    pub owner: AwbcFunctionId,
    pub instructions: AwbcTableRange,
    pub terminator: AwbcTerminator,
    pub safe_point: AwbcSafePointKind,
    pub source_map: Option<AwbcSourceMapId>,
}

/// Stable opcode values. `encoded` and `from_encoded` are the sole mapping.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcOpcode {
    Nop,
    LoadConst,
    Move,
    Clear,
    EnterScope,
    ExitScope,
    BindPattern,
    TestPattern,
    MakeTuple,
    MakeSequence,
    RepeatSequence,
    SequenceLen,
    SequenceGet,
    SequenceSlice,
    SequencePush,
    MakeRecord,
    MakeVariant,
    ProjectTuple,
    ProjectRecord,
    ProjectField,
    Unary,
    Binary,
    CallPureHelper,
    CallIntrinsic,
    EnsureContent,
    EmitEffect,
    StartTask,
    SpawnFiber,
    StreamYield,
    StreamClose,
    SourceClose,
    Drop,
    SourceYield,
    AssignField,
    CallTraitMethod,
    RegisterCleanup,
    CancelCleanup,
    MakeFunction,
    ApplyFunction,
    Jump,
    Branch,
    Match,
    CallFunction,
    GotoStatic,
    GotoDynamic,
    Dialogue,
    Choice,
    Await,
    AwaitMany,
    HostCall,
    Return,
    Trap,
    BudgetYield,
    Unreachable,
}

impl AwbcOpcode {
    pub const fn encoded(self) -> u8 {
        match self {
            Self::Nop => 0x00,
            Self::LoadConst => 0x01,
            Self::Move => 0x02,
            Self::Clear => 0x03,
            Self::EnterScope => 0x04,
            Self::ExitScope => 0x05,
            Self::BindPattern => 0x06,
            Self::TestPattern => 0x07,
            Self::MakeTuple => 0x08,
            Self::MakeSequence => 0x09,
            Self::RepeatSequence => 0x0a,
            Self::SequenceLen => 0x0b,
            Self::SequenceGet => 0x0c,
            Self::SequenceSlice => 0x0d,
            Self::SequencePush => 0x0e,
            Self::MakeRecord => 0x0f,
            Self::MakeVariant => 0x10,
            Self::ProjectTuple => 0x11,
            Self::ProjectRecord => 0x12,
            Self::ProjectField => 0x13,
            Self::Unary => 0x14,
            Self::Binary => 0x15,
            Self::CallPureHelper => 0x16,
            Self::CallIntrinsic => 0x17,
            Self::EnsureContent => 0x18,
            Self::EmitEffect => 0x19,
            Self::StartTask => 0x1a,
            Self::SpawnFiber => 0x1b,
            Self::StreamYield => 0x1c,
            Self::StreamClose => 0x1d,
            Self::SourceClose => 0x1e,
            Self::Drop => 0x1f,
            Self::SourceYield => 0x20,
            Self::AssignField => 0x21,
            Self::CallTraitMethod => 0x22,
            Self::RegisterCleanup => 0x23,
            Self::CancelCleanup => 0x24,
            Self::MakeFunction => 0x25,
            Self::ApplyFunction => 0x26,
            Self::Jump => 0x80,
            Self::Branch => 0x81,
            Self::Match => 0x82,
            Self::CallFunction => 0x83,
            Self::GotoStatic => 0x84,
            Self::GotoDynamic => 0x85,
            Self::Dialogue => 0x86,
            Self::Choice => 0x87,
            Self::Await => 0x88,
            Self::AwaitMany => 0x89,
            Self::HostCall => 0x8a,
            Self::Return => 0x8b,
            Self::Trap => 0x8c,
            Self::BudgetYield => 0x8d,
            Self::Unreachable => 0x8e,
        }
    }

    pub const fn from_encoded(value: u8) -> Option<Self> {
        Some(match value {
            0x00 => Self::Nop,
            0x01 => Self::LoadConst,
            0x02 => Self::Move,
            0x03 => Self::Clear,
            0x04 => Self::EnterScope,
            0x05 => Self::ExitScope,
            0x06 => Self::BindPattern,
            0x07 => Self::TestPattern,
            0x08 => Self::MakeTuple,
            0x09 => Self::MakeSequence,
            0x0a => Self::RepeatSequence,
            0x0b => Self::SequenceLen,
            0x0c => Self::SequenceGet,
            0x0d => Self::SequenceSlice,
            0x0e => Self::SequencePush,
            0x0f => Self::MakeRecord,
            0x10 => Self::MakeVariant,
            0x11 => Self::ProjectTuple,
            0x12 => Self::ProjectRecord,
            0x13 => Self::ProjectField,
            0x14 => Self::Unary,
            0x15 => Self::Binary,
            0x16 => Self::CallPureHelper,
            0x17 => Self::CallIntrinsic,
            0x18 => Self::EnsureContent,
            0x19 => Self::EmitEffect,
            0x1a => Self::StartTask,
            0x1b => Self::SpawnFiber,
            0x1c => Self::StreamYield,
            0x1d => Self::StreamClose,
            0x1e => Self::SourceClose,
            0x1f => Self::Drop,
            0x20 => Self::SourceYield,
            0x21 => Self::AssignField,
            0x22 => Self::CallTraitMethod,
            0x23 => Self::RegisterCleanup,
            0x24 => Self::CancelCleanup,
            0x25 => Self::MakeFunction,
            0x26 => Self::ApplyFunction,
            0x80 => Self::Jump,
            0x81 => Self::Branch,
            0x82 => Self::Match,
            0x83 => Self::CallFunction,
            0x84 => Self::GotoStatic,
            0x85 => Self::GotoDynamic,
            0x86 => Self::Dialogue,
            0x87 => Self::Choice,
            0x88 => Self::Await,
            0x89 => Self::AwaitMany,
            0x8a => Self::HostCall,
            0x8b => Self::Return,
            0x8c => Self::Trap,
            0x8d => Self::BudgetYield,
            0x8e => Self::Unreachable,
            _ => return None,
        })
    }

    pub const fn is_terminator(self) -> bool {
        self.encoded() >= 0x80
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcInstruction {
    Nop,
    LoadConst {
        dst: AwbcRegisterId,
        constant: AwbcConstantId,
    },
    Move {
        dst: AwbcRegisterId,
        src: AwbcRegisterId,
    },
    Clear {
        register: AwbcRegisterId,
    },
    EnterScope {
        scope: AwbcScopeId,
    },
    ExitScope {
        scope: AwbcScopeId,
    },
    BindPattern {
        pattern: AwbcPatternId,
        value: AwbcRegisterId,
        mode: AwbcBindMode,
    },
    TestPattern {
        dst: AwbcRegisterId,
        pattern: AwbcPatternId,
        value: AwbcRegisterId,
    },
    MakeTuple {
        dst: AwbcRegisterId,
        items: Vec<AwbcRegisterId>,
    },
    MakeSequence {
        dst: AwbcRegisterId,
        items: Vec<AwbcRegisterId>,
    },
    RepeatSequence {
        dst: AwbcRegisterId,
        value: AwbcRegisterId,
        len: AwbcRegisterId,
    },
    SequenceLen {
        dst: AwbcRegisterId,
        sequence: AwbcRegisterId,
    },
    SequenceGet {
        dst: AwbcRegisterId,
        sequence: AwbcRegisterId,
        index: AwbcRegisterId,
    },
    SequenceSlice {
        dst: AwbcRegisterId,
        sequence: AwbcRegisterId,
        start: AwbcRegisterId,
    },
    SequencePush {
        sequence: AwbcRegisterId,
        value: AwbcRegisterId,
    },
    MakeRecord {
        dst: AwbcRegisterId,
        ty: AwbcTypeId,
        field_names: Vec<AwbcStringId>,
        fields: Vec<AwbcRegisterId>,
    },
    MakeVariant {
        dst: AwbcRegisterId,
        ty: AwbcTypeId,
        case: u32,
        case_name: AwbcStringId,
        payload: Option<AwbcRegisterId>,
    },
    ProjectTuple {
        dst: AwbcRegisterId,
        target: AwbcRegisterId,
        ordinal: u32,
    },
    ProjectRecord {
        dst: AwbcRegisterId,
        target: AwbcRegisterId,
        ordinal: u32,
    },
    ProjectField {
        dst: AwbcRegisterId,
        target: AwbcRegisterId,
        field: AwbcStringId,
    },
    Unary {
        dst: AwbcRegisterId,
        op: AwbcUnaryOp,
        src: AwbcRegisterId,
    },
    Binary {
        dst: AwbcRegisterId,
        op: AwbcBinaryOp,
        lhs: AwbcRegisterId,
        rhs: AwbcRegisterId,
    },
    CallPureHelper {
        dst: AwbcRegisterId,
        helper: AwbcPureHelperId,
        args: Vec<AwbcRegisterId>,
    },
    CallIntrinsic {
        dst: Option<AwbcRegisterId>,
        intrinsic: AwbcIntrinsicId,
        args: Vec<AwbcRegisterId>,
    },
    EnsureContent {
        content: AwbcContentUnitId,
    },
    EmitEffect {
        effect: AwbcEffectPlanId,
        args: Vec<AwbcRegisterId>,
    },
    StartTask {
        dst: AwbcRegisterId,
        plan: AwbcTaskPlanId,
        args: Vec<AwbcRegisterId>,
    },
    SpawnFiber {
        dst: Option<AwbcRegisterId>,
        function: AwbcFunctionId,
        args: Vec<AwbcRegisterId>,
    },
    StreamYield {
        stream: AwbcStreamPlanId,
        value: AwbcRegisterId,
    },
    StreamClose {
        stream: AwbcStreamPlanId,
    },
    SourceClose {
        source: AwbcSourcePlanId,
    },
    Drop {
        register: AwbcRegisterId,
    },
    SourceYield {
        source: AwbcSourcePlanId,
        value: AwbcRegisterId,
    },
    AssignField {
        target: AwbcRegisterId,
        field: AwbcStringId,
        value: AwbcRegisterId,
    },
    CallTraitMethod {
        dst: AwbcRegisterId,
        method: AwbcTraitMethodId,
        receiver: AwbcRegisterId,
        args: Vec<AwbcRegisterId>,
        receiver_out: Option<AwbcRegisterId>,
    },
    RegisterCleanup {
        key: AwbcStringId,
        effect: AwbcEffectPlanId,
        args: Vec<AwbcRegisterId>,
    },
    CancelCleanup {
        key: AwbcStringId,
    },
    MakeFunction {
        dst: AwbcRegisterId,
        function: AwbcFunctionId,
        params: Vec<AwbcStringId>,
        capture_names: Vec<AwbcStringId>,
        captures: Vec<AwbcRegisterId>,
    },
    ApplyFunction {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        args: Vec<AwbcRegisterId>,
    },
}

impl AwbcInstruction {
    pub const fn opcode(&self) -> AwbcOpcode {
        match self {
            Self::Nop => AwbcOpcode::Nop,
            Self::LoadConst { .. } => AwbcOpcode::LoadConst,
            Self::Move { .. } => AwbcOpcode::Move,
            Self::Clear { .. } => AwbcOpcode::Clear,
            Self::EnterScope { .. } => AwbcOpcode::EnterScope,
            Self::ExitScope { .. } => AwbcOpcode::ExitScope,
            Self::BindPattern { .. } => AwbcOpcode::BindPattern,
            Self::TestPattern { .. } => AwbcOpcode::TestPattern,
            Self::MakeTuple { .. } => AwbcOpcode::MakeTuple,
            Self::MakeSequence { .. } => AwbcOpcode::MakeSequence,
            Self::RepeatSequence { .. } => AwbcOpcode::RepeatSequence,
            Self::SequenceLen { .. } => AwbcOpcode::SequenceLen,
            Self::SequenceGet { .. } => AwbcOpcode::SequenceGet,
            Self::SequenceSlice { .. } => AwbcOpcode::SequenceSlice,
            Self::SequencePush { .. } => AwbcOpcode::SequencePush,
            Self::MakeRecord { .. } => AwbcOpcode::MakeRecord,
            Self::MakeVariant { .. } => AwbcOpcode::MakeVariant,
            Self::ProjectTuple { .. } => AwbcOpcode::ProjectTuple,
            Self::ProjectRecord { .. } => AwbcOpcode::ProjectRecord,
            Self::ProjectField { .. } => AwbcOpcode::ProjectField,
            Self::Unary { .. } => AwbcOpcode::Unary,
            Self::Binary { .. } => AwbcOpcode::Binary,
            Self::CallPureHelper { .. } => AwbcOpcode::CallPureHelper,
            Self::CallIntrinsic { .. } => AwbcOpcode::CallIntrinsic,
            Self::EnsureContent { .. } => AwbcOpcode::EnsureContent,
            Self::EmitEffect { .. } => AwbcOpcode::EmitEffect,
            Self::StartTask { .. } => AwbcOpcode::StartTask,
            Self::SpawnFiber { .. } => AwbcOpcode::SpawnFiber,
            Self::StreamYield { .. } => AwbcOpcode::StreamYield,
            Self::StreamClose { .. } => AwbcOpcode::StreamClose,
            Self::SourceClose { .. } => AwbcOpcode::SourceClose,
            Self::Drop { .. } => AwbcOpcode::Drop,
            Self::SourceYield { .. } => AwbcOpcode::SourceYield,
            Self::AssignField { .. } => AwbcOpcode::AssignField,
            Self::CallTraitMethod { .. } => AwbcOpcode::CallTraitMethod,
            Self::RegisterCleanup { .. } => AwbcOpcode::RegisterCleanup,
            Self::CancelCleanup { .. } => AwbcOpcode::CancelCleanup,
            Self::MakeFunction { .. } => AwbcOpcode::MakeFunction,
            Self::ApplyFunction { .. } => AwbcOpcode::ApplyFunction,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcTraitMethod {
    pub public_id: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub function: AwbcFunctionId,
    pub receiver: AwbcTraitReceiverMode,
    pub receiver_state_slot: Option<AwbcRegisterId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcTraitReceiverMode {
    Owned,
    SharedRef,
    MutRef,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcBindMode {
    Declare,
    Assign,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcUnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcBinaryOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    And,
    Or,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcTerminator {
    Jump {
        target: AwbcBlockId,
    },
    Branch {
        condition: AwbcRegisterId,
        then_block: AwbcBlockId,
        else_block: AwbcBlockId,
    },
    Match {
        scrutinee: AwbcRegisterId,
        arms: AwbcTableRange,
        default: AwbcBlockId,
    },
    CallFunction {
        function: AwbcFunctionId,
        args: Vec<AwbcRegisterId>,
        dst: Option<AwbcRegisterId>,
        resume: AwbcResumePointId,
    },
    GotoStatic {
        function: AwbcFunctionId,
        args: Vec<AwbcRegisterId>,
    },
    GotoDynamic {
        target: AwbcRegisterId,
        args: Vec<AwbcRegisterId>,
    },
    Dialogue {
        content: AwbcContentUnitId,
        line_task_group: AwbcLineTaskGroupId,
        resume: AwbcResumePointId,
    },
    Choice {
        choice: AwbcChoiceId,
        dst: AwbcRegisterId,
        resume: AwbcResumePointId,
    },
    Await {
        task: AwbcRegisterId,
        binding: Option<AwbcPatternId>,
        resume: AwbcResumePointId,
    },
    AwaitMany {
        plan: AwbcTaskPlanId,
        source: AwbcRegisterId,
        binding: Option<AwbcPatternId>,
        resume: AwbcResumePointId,
    },
    HostCall {
        call: AwbcHostCallId,
        args: Vec<AwbcRegisterId>,
        dst: Option<AwbcRegisterId>,
        resume: AwbcResumePointId,
    },
    Return {
        value: Option<AwbcRegisterId>,
    },
    Trap {
        code: AwbcTrapCode,
        message: Option<AwbcStringId>,
    },
    BudgetYield {
        resume: AwbcResumePointId,
    },
    Unreachable,
}

impl AwbcTerminator {
    pub const fn opcode(&self) -> AwbcOpcode {
        match self {
            Self::Jump { .. } => AwbcOpcode::Jump,
            Self::Branch { .. } => AwbcOpcode::Branch,
            Self::Match { .. } => AwbcOpcode::Match,
            Self::CallFunction { .. } => AwbcOpcode::CallFunction,
            Self::GotoStatic { .. } => AwbcOpcode::GotoStatic,
            Self::GotoDynamic { .. } => AwbcOpcode::GotoDynamic,
            Self::Dialogue { .. } => AwbcOpcode::Dialogue,
            Self::Choice { .. } => AwbcOpcode::Choice,
            Self::Await { .. } => AwbcOpcode::Await,
            Self::AwaitMany { .. } => AwbcOpcode::AwaitMany,
            Self::HostCall { .. } => AwbcOpcode::HostCall,
            Self::Return { .. } => AwbcOpcode::Return,
            Self::Trap { .. } => AwbcOpcode::Trap,
            Self::BudgetYield { .. } => AwbcOpcode::BudgetYield,
            Self::Unreachable => AwbcOpcode::Unreachable,
        }
    }

    pub const fn resume_point(&self) -> Option<AwbcResumePointId> {
        match self {
            Self::CallFunction { resume, .. }
            | Self::Dialogue { resume, .. }
            | Self::Choice { resume, .. }
            | Self::Await { resume, .. }
            | Self::AwaitMany { resume, .. }
            | Self::HostCall { resume, .. }
            | Self::BudgetYield { resume } => Some(*resume),
            Self::Jump { .. }
            | Self::Branch { .. }
            | Self::Match { .. }
            | Self::GotoStatic { .. }
            | Self::GotoDynamic { .. }
            | Self::Return { .. }
            | Self::Trap { .. }
            | Self::Unreachable => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcSafePointKind {
    FlowEntry,
    CallableBoundary,
    Dialogue,
    Choice,
    Await,
    AwaitMany,
    HostCall,
    LoopBackedge,
    BudgetYield,
    Return,
    Trap,
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcResumePoint {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    pub frame_layout: AwbcFrameLayoutId,
    pub kind: AwbcSafePointKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcTrapCode {
    TypeMismatch,
    UninitializedRegister,
    InvalidIndex,
    DivisionByZero,
    PatternMismatch,
    MissingDynamicTarget,
    HostAbiMismatch,
    CapabilityDenied,
    ExplicitPanic,
    InternalInvariant,
}

/// Executable pattern graph. Child references must be acyclic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcPattern {
    Bind {
        target: AwbcRegisterId,
        mutable: bool,
        expected: Option<AwbcTypeId>,
    },
    Discard,
    Literal(AwbcConstantId),
    Entity(AwbcStringId),
    Tuple(Vec<AwbcPatternId>),
    Record {
        ty: Option<AwbcTypeId>,
        fields: Vec<AwbcRecordPatternField>,
        rest: bool,
    },
    Sequence {
        items: Vec<AwbcPatternId>,
        rest: Option<AwbcRegisterId>,
    },
    Variant {
        ty: Option<AwbcTypeId>,
        case: u32,
        case_name: AwbcStringId,
        payload: Option<AwbcPatternId>,
    },
    Whole {
        target: AwbcRegisterId,
        inner: AwbcPatternId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRecordPatternField {
    pub field: u32,
    pub pattern: AwbcPatternId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcMatchArm {
    pub pattern: AwbcPatternId,
    pub guard: Option<AwbcFunctionId>,
    pub target: AwbcBlockId,
}

/// Pure, deterministic intrinsic resolved by the runtime's typed registry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcIntrinsic {
    pub public_id: AwbcStringId,
    pub registry_code: u32,
    pub signature: AwbcSignatureId,
    pub revision: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcHostCall {
    pub public_id: AwbcStringId,
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub mode: AwbcHostCallMode,
    pub deterministic: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcHostCallMode {
    Immediate,
    Suspend,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcTaskPlan {
    pub public_id: AwbcStringId,
    /// Stable need identifier reported at the shared runtime boundary.
    pub need_id: AwbcStringId,
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub class: AwbcTaskClass,
    pub priority: i32,
    pub cancel_scope: AwbcStringId,
    pub policy: AwbcTaskPolicy,
    pub arguments: Vec<AwbcTaskArgument>,
    pub many: Option<AwbcAwaitManyPolicy>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcTaskClass {
    LocalView,
    Io,
    Cpu,
    GpuPrepare,
    ShaderCompile,
    WasmCall,
    AssetDecode,
    AudioDecode,
    AudioRender,
    TtsSynthesis,
    BgmPrecompose,
    Lsp,
    Background,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcTaskPolicy {
    JoinSameKey,
    AlwaysStart,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcTaskArgument {
    pub name: Option<AwbcStringId>,
    pub spread: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcAwaitManyPolicy {
    pub item_binding: AwbcRegisterId,
    pub limit: u32,
}

/// Effect-local argument index used by typed AWBC audio payloads.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct AwbcAudioArg(pub u32);

impl AwbcAudioArg {
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One evaluated value consumed by a typed AWBC audio command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AwbcAudioValueRef {
    /// Runtime value passed through `AwbcInstruction::EmitEffect.args`.
    Arg(AwbcAudioArg),
    /// Canonical constant used by context-free literal line-task effects.
    Const(AwbcConstantId),
}

/// Canonical typed AWBC representation of `RuntimeAudioCommand`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AwbcAudioCommand {
    Play {
        voice: AwbcAudioValueRef,
        resource: AwbcAudioValueRef,
        bus: AwbcAudioValueRef,
        gain_db_milli: AwbcAudioValueRef,
        pan_milli: AwbcAudioValueRef,
        loop_mode: AudioLoopMode,
        start_frame: AwbcAudioValueRef,
        fade_in_millis: AwbcAudioValueRef,
    },
    Stop {
        voice: AwbcAudioValueRef,
        fade_out_millis: AwbcAudioValueRef,
    },
    StopAll {
        fade_out_millis: AwbcAudioValueRef,
    },
    SetVoiceGain {
        voice: AwbcAudioValueRef,
        gain_db_milli: AwbcAudioValueRef,
        transition_millis: AwbcAudioValueRef,
    },
    SetVoicePan {
        voice: AwbcAudioValueRef,
        pan_milli: AwbcAudioValueRef,
        transition_millis: AwbcAudioValueRef,
    },
    SetBusGain {
        bus: AwbcAudioValueRef,
        gain_db_milli: AwbcAudioValueRef,
        transition_millis: AwbcAudioValueRef,
    },
    SetBusMute {
        bus: AwbcAudioValueRef,
        muted: AwbcAudioValueRef,
    },
    SetEffectEnabled {
        bus: AwbcAudioValueRef,
        effect: AwbcAudioValueRef,
        enabled: AwbcAudioValueRef,
    },
    SetEffectParameter {
        bus: AwbcAudioValueRef,
        effect: AwbcAudioValueRef,
        parameter: AudioEffectParameterKind,
        value: AwbcAudioValueRef,
        transition_millis: AwbcAudioValueRef,
    },
    ApplySnapshot {
        snapshot: AwbcAudioValueRef,
        transition_millis: AwbcAudioValueRef,
    },
    RequestMicrophone {
        capture: AwbcAudioValueRef,
        constraints: MicrophoneConstraints,
    },
    StopMicrophone {
        capture: AwbcAudioValueRef,
    },
    SetCaptureMonitor {
        capture: AwbcAudioValueRef,
        bus: Option<AwbcAudioValueRef>,
        gain_db_milli: AwbcAudioValueRef,
    },
}

impl AwbcAudioCommand {
    /// Returns all payload value references in canonical field order.
    #[must_use]
    pub fn value_refs(&self) -> Vec<AwbcAudioValueRef> {
        match self {
            Self::Play {
                voice,
                resource,
                bus,
                gain_db_milli,
                pan_milli,
                start_frame,
                fade_in_millis,
                ..
            } => vec![
                *voice,
                *resource,
                *bus,
                *gain_db_milli,
                *pan_milli,
                *start_frame,
                *fade_in_millis,
            ],
            Self::Stop {
                voice,
                fade_out_millis,
            } => vec![*voice, *fade_out_millis],
            Self::StopAll { fade_out_millis } => vec![*fade_out_millis],
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            } => vec![*voice, *gain_db_milli, *transition_millis],
            Self::SetVoicePan {
                voice,
                pan_milli,
                transition_millis,
            } => vec![*voice, *pan_milli, *transition_millis],
            Self::SetBusGain {
                bus,
                gain_db_milli,
                transition_millis,
            } => vec![*bus, *gain_db_milli, *transition_millis],
            Self::SetBusMute { bus, muted } => vec![*bus, *muted],
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => vec![*bus, *effect, *enabled],
            Self::SetEffectParameter {
                bus,
                effect,
                value,
                transition_millis,
                ..
            } => vec![*bus, *effect, *value, *transition_millis],
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => vec![*snapshot, *transition_millis],
            Self::RequestMicrophone { capture, .. } | Self::StopMicrophone { capture } => {
                vec![*capture]
            }
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => bus.map_or_else(
                || vec![*capture, *gain_db_milli],
                |bus| vec![*capture, bus, *gain_db_milli],
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcEffectPlan {
    pub kind: AwbcEffectKind,
    pub signature: AwbcSignatureId,
    pub capability: Option<AwbcStringId>,
    pub audio: Option<AwbcAudioCommandId>,
    pub static_args: Vec<AwbcConstantId>,
    pub resources: Vec<AwbcResourceAccess>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcEffectKind {
    RegisterHandle,
    DropHandle,
    Wait,
    Audio,
    Call,
    Log,
    SignalWrite,
    MetricWrite,
    EmitEvent,
    Out,
    Return,
    Goto,
    Panic,
    Fail,
    Bail,
    Ensure,
    Assert,
    Close,
    Select,
    Break,
    Continue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcResourceAccess {
    pub resource: AwbcResourceId,
    pub mode: AwbcResourceAccessMode,
    pub conflict: AwbcConflictPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcResourceAccessMode {
    Read,
    Write,
    Drop,
    Append,
    Control,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcConflictPolicy {
    Error,
    Append,
    LastWriterWins { priority: i32 },
    MergePatch,
    Reduce { op: AwbcReduceOp },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcReduceOp {
    Sum,
    Min,
    Max,
    And,
    Or,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcChoice {
    pub public_id: Option<AwbcStringId>,
    pub options: AwbcTableRange,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcChoiceOption {
    pub public_id: Option<AwbcStringId>,
    pub label: AwbcStringId,
    pub condition: Option<AwbcFunctionId>,
    pub target: Option<AwbcFunctionId>,
    pub out_effect: Option<AwbcEffectPlanId>,
    pub effects: Vec<AwbcEffectPlanId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcContentUnit {
    pub public_id: AwbcStringId,
    pub line_task_group: Option<AwbcLineTaskGroupId>,
    pub display: Option<AwbcDisplayMapId>,
    pub source: Option<AwbcSourceMapId>,
    pub resources: Vec<AwbcResourceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineTaskGroup {
    pub root: AwbcLineTaskNodeId,
    pub options: Vec<AwbcLineOption>,
    pub bindings: Option<AwbcFunctionId>,
    pub out: Option<AwbcFunctionId>,
    pub cancel_handlers: Vec<AwbcLineCancelHandler>,
    pub cleanup: AwbcLineCleanupPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineOption {
    pub name: AwbcStringId,
    pub value: AwbcConstantId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineCancelHandler {
    pub trigger: AwbcStringId,
    pub function: AwbcFunctionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineCleanupPolicy {
    pub child_tasks: AwbcChildCleanup,
    pub presentation: AwbcPresentationCleanup,
    pub audio: AwbcAudioCleanup,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcChildCleanup {
    CancelAndJoin,
    Detach,
    Finish,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcPresentationCleanup {
    DropRegistered,
    KeepRegistered,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcAudioCleanup {
    StopRegistered,
    FadeRegistered,
    KeepRegistered,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcLineTaskNode {
    Sequence(Vec<AwbcLineTaskNodeId>),
    Start(Vec<AwbcLineTaskNodeId>),
    Parallel {
        policy: AwbcParallelPolicy,
        children: Vec<AwbcLineTaskNodeId>,
    },
    Child {
        task: AwbcTaskPlanId,
        trigger: AwbcLineTaskTrigger,
        join: AwbcChildJoinPolicy,
        cancel: AwbcChildCancelPolicy,
        scope: AwbcLineTaskNodeId,
    },
    Effect(AwbcEffectPlanId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcParallelPolicy {
    JoinAll,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcLineTaskTrigger {
    Immediate,
    Mark(AwbcStringId),
    DelayNanos(u64),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcChildJoinPolicy {
    Join,
    Detached,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcChildCancelPolicy {
    CancelAndJoin,
    Finish,
    Detach,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcStreamPlan {
    pub public_id: AwbcStringId,
    pub item_type: AwbcTypeId,
    pub error_type: AwbcTypeId,
    pub transform: AwbcFunctionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcSourcePlan {
    pub public_id: AwbcStringId,
    pub item_type: AwbcTypeId,
    pub error_type: AwbcTypeId,
    pub open: AwbcFunctionId,
    pub policy: AwbcSourcePolicy,
    pub handlers: Vec<AwbcSourceHandler>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcSourceHandler {
    pub kind: AwbcSourceEventKind,
    pub pattern: Option<AwbcPatternId>,
    pub function: AwbcFunctionId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcSourceEventKind {
    Item,
    Error,
    Progress,
    Disconnected,
    PermissionRevoked,
    End,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcSourcePolicy {
    pub backpressure: AwbcBackpressurePolicy,
    pub replay: AwbcReplayPolicy,
    pub privacy: AwbcPrivacyPolicy,
    pub max_queue: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcBackpressurePolicy {
    LatestOnly,
    BoundedQueue {
        capacity: u32,
        overflow: AwbcOverflowPolicy,
    },
    BlockingNotAllowed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcOverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcReplayPolicy {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcPrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcPureHelper {
    pub public_id: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub function: AwbcFunctionId,
    pub scalar_eval_supported: bool,
    pub origin: AwbcPureHelperOrigin,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcPureHelperOrigin {
    Annotated,
    Inferred,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcDisplayMapEntry {
    pub content: AwbcContentUnitId,
    pub display_key: AwbcStringId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcSourceMapEntry {
    pub location: AwbcCodeLocation,
    pub source_file: AwbcStringId,
    pub start: u32,
    pub end: u32,
    pub anchor: Option<AwbcStringId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcCodeLocation {
    Instruction(AwbcInstructionId),
    Block(AwbcBlockId),
    ResumePoint(AwbcResumePointId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcResourceRef {
    pub public_id: AwbcStringId,
    pub kind: AwbcStringId,
    pub digest: AwbcDigest,
    pub decoded_len: u64,
    pub residency: AwbcResourceResidency,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcResourceResidency {
    Startup,
    OnDemand,
    Streaming,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcEntry {
    /// Canonical runtime lookup identity. `public_id` is presentation/debug
    /// text and must never be parsed back into semantic identity.
    pub runtime_id: crate::plan::EntryRuntimeId,
    pub binding: EntryBindingIdentity,
    pub public_id: AwbcStringId,
    pub kind: AwbcEntryKind,
    pub signature: AwbcSignatureId,
    pub target: AwbcEntryTarget,
    pub roles: RuntimeEntryRoles,
}

/// Exact semantic callable role mapped to one Product AWBC function slot.
///
/// Root transactions retain `role` and ask the Product evaluator to resolve
/// this table. The dense function id never becomes semantic identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcCallableExecutable {
    pub role: RuntimeCallableRole,
    pub function: AwbcFunctionId,
}

/// Exact semantic flow contract mapped to one Product AWBC function slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFlowExecutable {
    pub metadata: RuntimeFlowExecutable,
    pub function: AwbcFunctionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcEntryKind {
    Game,
    Editor,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Agent,
    Custom(AwbcStringId),
}

impl AwbcEntryKind {
    #[must_use]
    pub fn runtime_kind(&self, strings: &[String]) -> Option<crate::plan::RuntimeEntryKind> {
        Some(match self {
            Self::Game => crate::plan::RuntimeEntryKind::Game,
            Self::Editor => crate::plan::RuntimeEntryKind::Editor,
            Self::Cli => crate::plan::RuntimeEntryKind::Cli,
            Self::Server => crate::plan::RuntimeEntryKind::Server,
            Self::Activity => crate::plan::RuntimeEntryKind::Activity,
            Self::Test => crate::plan::RuntimeEntryKind::Test,
            Self::Bench => crate::plan::RuntimeEntryKind::Bench,
            Self::Agent => crate::plan::RuntimeEntryKind::Agent,
            Self::Custom(value) => {
                crate::plan::RuntimeEntryKind::Custom(strings.get(value.index())?.clone())
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcEntryTarget {
    Function(AwbcFunctionId),
    Routes(Vec<AwbcRoute>),
}

impl AwbcEntryTarget {
    /// Returns the single function selected by this entry target, if one exists.
    #[must_use]
    pub const fn function(&self) -> Option<AwbcFunctionId> {
        match self {
            Self::Function(function) => Some(*function),
            Self::Routes(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRoute {
    pub method: AwbcStringId,
    pub path: AwbcStringId,
    pub target: AwbcFunctionId,
    pub bindings: Vec<AwbcRouteBinding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRouteBinding {
    pub register: AwbcRegisterId,
    pub source: AwbcRouteBindingSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcRouteBindingSource {
    PathParameter(AwbcStringId),
}
