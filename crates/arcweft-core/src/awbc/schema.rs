use crate::entry::{
    EntryBindingIdentity, RuntimeCallableRole, RuntimeEntryRoles, RuntimeFlowExecutable,
};
use crate::pattern::{
    RuntimeCheckedType, RuntimeOpaqueTypeAdmission, RuntimeSemanticTypeId,
    RuntimeSemanticTypeIdentityEncoder,
};
use crate::plan::{FlowRuntimeId, RuntimeAgentOperationalType, RuntimeFlowTargetError};
use crate::runtime_id::{
    RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId, RuntimeDialogueValueSlotId,
    RuntimeLocalDeclarationId,
};
use crate::value::{RuntimeAgentConstructor, RuntimeEntityReference, RuntimeHandleKind};
use arcweft_character::id::CharacterId;
use arcweft_interaction_model::audio::{
    AudioEffectParameterKind, AudioLoopMode, MicrophoneConstraints,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Defines a closed AWBC-owned one-byte enum together with its sole numeric
/// inventory, allocation-free decoder, and numeric structured-data contract.
macro_rules! awbc_u8_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident = $tag:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(u8)]
        pub enum $name {
            $($variant = $tag),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            const DECODE: [Option<Self>; 256] = {
                let mut table = [None; 256];
                let mut index = 0;
                while index < Self::ALL.len() {
                    let value = Self::ALL[index];
                    let encoded = value as u8 as usize;
                    assert!(table[encoded].is_none(), concat!("duplicate ", stringify!($name), " tag"));
                    table[encoded] = Some(value);
                    index += 1;
                }
                table
            };

            #[must_use]
            pub const fn encoded(self) -> u8 {
                self as u8
            }

            #[must_use]
            pub const fn from_encoded(encoded: u8) -> Option<Self> {
                Self::DECODE[encoded as usize]
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_u8(self.encoded())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let encoded = u8::deserialize(deserializer)?;
                Self::from_encoded(encoded).ok_or_else(|| {
                    serde::de::Error::custom(format_args!(
                        "unknown {} tag {encoded}",
                        stringify!($name)
                    ))
                })
            }
        }
    };
}

/// Canonical AWBC executable ABI implemented by this schema.
pub const AWBC_ABI_VERSION: u32 = 1;
/// Canonical binary codec version used inside an `AWBC` product section.
///
/// This codec persists the exact semantic Flow-to-function binding table.
pub const AWBC_CODEC_VERSION: u16 = 1;
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
awbc_id!(
    AwbcLineOperationId,
    "Index into the typed line-operation table."
);
awbc_id!(
    AwbcLineHandleSiteId,
    "Index into one line-task group's typed handle-site table."
);
awbc_id!(AwbcStreamPlanId, "Index into the stream-plan table.");
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
    pub line_operations: Vec<AwbcLineOperation>,
    pub stream_plans: Vec<AwbcStreamPlan>,
    pub pure_helpers: Vec<AwbcPureHelper>,
    pub pure_programs: Vec<AwbcPureProgramBinding>,
    pub trait_methods: Vec<AwbcTraitMethod>,
    pub display_map: Vec<AwbcDisplayMapEntry>,
    pub source_map: Vec<AwbcSourceMapEntry>,
    pub resources: Vec<AwbcResourceRef>,
    pub callable_executables: Vec<AwbcCallableExecutable>,
    pub flow_bindings: Vec<AwbcFlowBinding>,
    pub flow_executables: Vec<AwbcFlowExecutable>,
    pub entries: Vec<AwbcEntry>,
}

impl Default for AwbcProgram {
    fn default() -> Self {
        Self {
            header: AwbcHeader::default(),
            strings: Vec::new(),
            runtime_types: vec![AwbcRuntimeType::unit(), AwbcRuntimeType::dynamic()],
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
            line_operations: Vec::new(),
            stream_plans: Vec::new(),
            pure_helpers: Vec::new(),
            pure_programs: Vec::new(),
            trait_methods: Vec::new(),
            display_map: Vec::new(),
            source_map: Vec::new(),
            resources: Vec::new(),
            callable_executables: Vec::new(),
            flow_bindings: Vec::new(),
            flow_executables: Vec::new(),
            entries: Vec::new(),
        }
    }
}

impl AwbcProgram {
    /// Returns the Product function selected by one exact semantic Flow ID.
    #[must_use]
    pub fn flow_function(&self, flow: &FlowRuntimeId) -> Option<AwbcFunctionId> {
        self.flow_bindings
            .iter()
            .find(|binding| binding.flow == *flow)
            .map(|binding| binding.function)
    }

    /// Returns the exact semantic Flow ID owned by one Product function.
    #[must_use]
    pub fn flow_identity(&self, function: AwbcFunctionId) -> Option<&FlowRuntimeId> {
        self.flow_bindings
            .iter()
            .find(|binding| binding.function == function)
            .map(|binding| &binding.flow)
    }

    /// Resolves one stable domain-owned pure program to its exact helper row.
    #[must_use]
    pub fn pure_program_helper(
        &self,
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    ) -> Option<AwbcPureHelperId> {
        self.pure_programs
            .iter()
            .find(|binding| binding.program == program)
            .map(|binding| binding.helper)
    }

    /// Resolves the complete semantic signature sealed for one pure program.
    #[must_use]
    pub fn pure_program_binding(
        &self,
        program: arcweft_id::runtime_program::RuntimePureProgramId,
    ) -> Option<&AwbcPureProgramBinding> {
        self.pure_programs
            .iter()
            .find(|binding| binding.program == program)
    }

    /// Resolves runtime-authored target text through the persisted accepted
    /// Flow inventory, never through function display strings.
    ///
    /// # Panics
    ///
    /// Panics only if the internally selected Flow identity has lost the
    /// function binding from the same accepted AWBC inventory.
    pub fn resolve_flow_target_value(
        &self,
        value: &str,
    ) -> Result<(&FlowRuntimeId, AwbcFunctionId), RuntimeFlowTargetError> {
        let flow = FlowRuntimeId::resolve_runtime_target(
            value,
            self.flow_bindings.iter().map(|binding| &binding.flow),
        )?;
        let function = self
            .flow_function(flow)
            .expect("selected AWBC Flow identity must retain its function binding");
        Ok((flow, function))
    }

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
    for call in &mut program.host_calls {
        visit_string_id(&mut call.public_id, visitor);
        visit_string_id(&mut call.capability, visitor);
        visit_string_id(&mut call.operation, visitor);
        for argument in &mut call.arguments {
            visit_optional_string_id(&mut argument.name, visitor);
        }
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
        for mark in &mut content.marks {
            visit_string_id(&mut mark.label, visitor);
        }
    }
    for stream in &mut program.stream_plans {
        visit_string_id(&mut stream.public_id, visitor);
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
    match &mut ty.shape {
        AwbcRuntimeTypeShape::Record { public_id, fields } => {
            visit_optional_string_id(public_id, visitor);
            for field in fields {
                visit_string_id(&mut field.name, visitor);
            }
        }
        AwbcRuntimeTypeShape::Variant { owner, cases, .. } => {
            if let AwbcVariantIdentity::Nominal { public_id, .. } = owner {
                visit_string_id(public_id, visitor);
            }
            for case in cases {
                visit_string_id(&mut case.name, visitor);
            }
        }
        AwbcRuntimeTypeShape::Nominal { public_id, .. } => visit_string_id(public_id, visitor),
        AwbcRuntimeTypeShape::NominalRecord {
            public_id, fields, ..
        } => {
            visit_string_id(public_id, visitor);
            for field in fields {
                visit_string_id(&mut field.name, visitor);
            }
        }
        AwbcRuntimeTypeShape::Opaque { producer, .. } => visit_string_id(producer, visitor),
        AwbcRuntimeTypeShape::Unit
        | AwbcRuntimeTypeShape::Bool
        | AwbcRuntimeTypeShape::Int(_)
        | AwbcRuntimeTypeShape::UInt(_)
        | AwbcRuntimeTypeShape::Bytes
        | AwbcRuntimeTypeShape::Never
        | AwbcRuntimeTypeShape::F32
        | AwbcRuntimeTypeShape::F64
        | AwbcRuntimeTypeShape::String
        | AwbcRuntimeTypeShape::Char
        | AwbcRuntimeTypeShape::Duration
        | AwbcRuntimeTypeShape::Progress
        | AwbcRuntimeTypeShape::EntityRef
        | AwbcRuntimeTypeShape::AgentValue
        | AwbcRuntimeTypeShape::Tuple(_)
        | AwbcRuntimeTypeShape::Sequence(_)
        | AwbcRuntimeTypeShape::Choice(_)
        | AwbcRuntimeTypeShape::MatrixF32
        | AwbcRuntimeTypeShape::MatrixF64
        | AwbcRuntimeTypeShape::TensorF32
        | AwbcRuntimeTypeShape::TensorF64
        | AwbcRuntimeTypeShape::Range(_)
        | AwbcRuntimeTypeShape::Iterator(_)
        | AwbcRuntimeTypeShape::Array { .. }
        | AwbcRuntimeTypeShape::Map { .. }
        | AwbcRuntimeTypeShape::Need(_)
        | AwbcRuntimeTypeShape::Task(_)
        | AwbcRuntimeTypeShape::Stream { .. }
        | AwbcRuntimeTypeShape::Shared(_)
        | AwbcRuntimeTypeShape::Reference(_)
        | AwbcRuntimeTypeShape::Function { .. }
        | AwbcRuntimeTypeShape::Agent(_)
        | AwbcRuntimeTypeShape::Dynamic => {}
    }
}

fn visit_constant_strings(constant: &mut AwbcConstant, visitor: &mut dyn FnMut(&mut AwbcStringId)) {
    match constant {
        AwbcConstant::String(id) => visit_string_id(id, visitor),
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
        | AwbcConstant::EntityRef(_)
        | AwbcConstant::Bytes(_)
        | AwbcConstant::TensorF32 { .. }
        | AwbcConstant::TensorF64 { .. }
        | AwbcConstant::Opaque { .. } => {}
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
        AwbcInstruction::ProjectField {
            field: AwbcFieldProjection::Named(field),
            ..
        } => visit_string_id(field, visitor),
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
        AwbcPattern::Variant { case_name, .. } => visit_string_id(case_name, visitor),
        AwbcPattern::Bind { .. }
        | AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Entity(_)
        | AwbcPattern::Tuple(_)
        | AwbcPattern::Record { .. }
        | AwbcPattern::Sequence { .. }
        | AwbcPattern::Whole { .. } => {}
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
            for segment in &mut route.segments {
                if let AwbcRouteSegment::Literal(literal) = segment {
                    visit_string_id(literal, visitor);
                }
            }
        }
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
/// One total semantic type owner in the AWBC runtime-type table.
///
/// The semantic identity and executable shape are inseparable. Every
/// instruction signature therefore retains the checked identity even when its
/// operational ABI is structurally primitive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRuntimeType {
    semantic_identity: RuntimeSemanticTypeId,
    shape: AwbcRuntimeTypeShape,
}

impl AwbcRuntimeType {
    #[must_use]
    pub const fn new(
        semantic_identity: RuntimeSemanticTypeId,
        shape: AwbcRuntimeTypeShape,
    ) -> Self {
        Self {
            semantic_identity,
            shape,
        }
    }

    #[must_use]
    pub fn unit() -> Self {
        Self::new(
            RuntimeCheckedType::Unit.semantic_identity_digest(),
            AwbcRuntimeTypeShape::Unit,
        )
    }

    #[must_use]
    pub fn dynamic() -> Self {
        Self::new(
            AwbcSyntheticRuntimeTypeKind::Dynamic.semantic_identity(),
            AwbcRuntimeTypeShape::Dynamic,
        )
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        self.semantic_identity
    }

    #[must_use]
    pub const fn shape(&self) -> &AwbcRuntimeTypeShape {
        &self.shape
    }
}

/// Closed identity owner for executable AWBC shapes that have no checked
/// language type. Plan-backed types never use this domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AwbcSyntheticRuntimeTypeKind {
    Dynamic,
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
}

impl AwbcSyntheticRuntimeTypeKind {
    /// Returns the collision-separated semantic identity for this complete
    /// synthetic runtime shape.
    #[must_use]
    pub fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        match self {
            Self::Dynamic => encoder.write_tag(0xff00),
            Self::MatrixF32 => encoder.write_tag(0xff01),
            Self::MatrixF64 => encoder.write_tag(0xff02),
            Self::TensorF32 => encoder.write_tag(0xff03),
            Self::TensorF64 => encoder.write_tag(0xff04),
        }
        encoder.finish()
    }
}

/// Closed structural identity owner for runtime-only container shapes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AwbcStructuralRuntimeTypeKind {
    Record {
        public_id: Option<String>,
        fields: Vec<(String, RuntimeSemanticTypeId)>,
    },
}

impl AwbcStructuralRuntimeTypeKind {
    #[must_use]
    pub fn semantic_identity(&self) -> RuntimeSemanticTypeId {
        let mut encoder = RuntimeSemanticTypeIdentityEncoder::new();
        match self {
            Self::Record { public_id, fields } => {
                encoder.write_tag(0xff20);
                match public_id {
                    Some(public_id) => {
                        encoder.write_u8(1);
                        encoder.write_str(public_id);
                    }
                    None => encoder.write_u8(0),
                }
                encoder.write_len(fields.len());
                for (name, semantic_identity) in fields {
                    encoder.write_str(name);
                    encoder.write_bytes(semantic_identity.as_bytes());
                }
            }
        }
        encoder.finish()
    }
}

/// Closed executable shape owned by one [`AwbcRuntimeType`] row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcRuntimeTypeShape {
    Unit,
    Bool,
    Int(AwbcSignedIntKind),
    UInt(AwbcUnsignedIntKind),
    F32,
    F64,
    String,
    Char,
    Duration,
    Progress,
    EntityRef,
    Tuple(Vec<AwbcTypeId>),
    Sequence(AwbcTypeId),
    Record {
        public_id: Option<AwbcStringId>,
        fields: Vec<AwbcRecordField>,
    },
    Variant {
        owner: AwbcVariantIdentity,
        /// Ordered project-nominal generic arguments. Built-in variants keep
        /// this empty because their payload edges already own the parameters.
        arguments: Vec<AwbcTypeId>,
        cases: Vec<AwbcVariantCase>,
    },
    /// One of several closed structural alternatives.
    Choice(Vec<AwbcTypeId>),
    /// Checked nominal identity shared by project and standard runtime types.
    Nominal {
        public_id: AwbcStringId,
        layout: [u8; 32],
        arguments: Vec<AwbcTypeId>,
    },
    /// Executable nominal-record descriptor in defining field order.
    NominalRecord {
        public_id: AwbcStringId,
        layout: [u8; 32],
        arguments: Vec<AwbcTypeId>,
        fields: Vec<AwbcRecordField>,
    },
    /// Opaque checked-type identity and its producer-owned admission rule.
    Opaque {
        producer: AwbcStringId,
        admission: RuntimeOpaqueTypeAdmission,
        value_class: crate::value::RuntimeOpaqueValueClass,
        persistence: crate::value::RuntimeOpaquePersistence,
        /// Generic arguments in the exact checked type graph.
        arguments: Vec<AwbcTypeId>,
    },
    /// Canonical byte-buffer checked type, distinct from `Sequence<U8>`.
    Bytes,
    /// Uninhabited checked type, distinct from an authored empty choice.
    Never,
    /// Recursive closed JSON-like value algebra owned by Agent protocol.
    AgentValue,
    /// Closed Agent runtime family. Exact semantic identity remains in the
    /// runtime-plan facts; this type selects the executable value carrier.
    Agent(AwbcAgentTypeShape),
    MatrixF32,
    MatrixF64,
    TensorF32,
    TensorF64,
    Range(AwbcTypeId),
    Iterator(AwbcTypeId),
    Array {
        item: AwbcTypeId,
        length: u64,
    },
    Map {
        key: AwbcTypeId,
        value: AwbcTypeId,
    },
    Need(AwbcTypeId),
    Task(AwbcTypeId),
    Stream {
        item: AwbcTypeId,
        error: AwbcTypeId,
    },
    Shared(AwbcTypeId),
    Reference(AwbcTypeId),
    Function {
        parameters: Vec<AwbcTypeId>,
        result: AwbcTypeId,
    },
    Dynamic,
}

/// Exact Agent runtime type graph retained by an AWBC row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcAgentTypeShape {
    Leaf(RuntimeAgentOperationalType),
    Probe(AwbcTypeId),
}

impl AwbcAgentTypeShape {
    #[must_use]
    pub const fn operational_type(&self) -> RuntimeAgentOperationalType {
        match self {
            Self::Leaf(value) => *value,
            Self::Probe(_) => RuntimeAgentOperationalType::Probe,
        }
    }
}

/// Closed semantic owner for an AWBC variant type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcVariantIdentity {
    Nominal { public_id: AwbcStringId },
    Builtin(crate::pattern::RuntimeBuiltinVariantIdentity),
}

awbc_u8_enum! {
    pub enum AwbcSignedIntKind {
        I8 = 0,
        I16 = 1,
        I32 = 2,
        I64 = 3,
        I128 = 4,
        ISize = 5,
    }
}

awbc_u8_enum! {
    pub enum AwbcUnsignedIntKind {
        U8 = 0,
        U16 = 1,
        U32 = 2,
        U64 = 3,
        U128 = 4,
        USize = 5,
    }
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
    EntityRef(RuntimeEntityReference),
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
    /// Opaque payload stored behind its checked opaque runtime type.
    Opaque {
        ty: AwbcTypeId,
        payload: AwbcConstantId,
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

awbc_u8_enum! {
    pub enum AwbcFrameSlotRole {
        Parameter = 0,
        Local = 1,
        Temporary = 2,
        ReturnValue = 3,
        RuntimeState = 4,
    }
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AwbcFunctionKind {
    Flow = 0,
    Ordinary = 1,
    PureHelper = 2,
    TraitMethod = 3,
    Synthetic = 4,
    GeneratorProducer = 5,
    StreamTransform = 6,
    LineActivation = 7,
    LineTask = 8,
}

impl AwbcFunctionKind {
    pub const ALL: &'static [Self] = &[
        Self::Flow,
        Self::Ordinary,
        Self::PureHelper,
        Self::TraitMethod,
        Self::Synthetic,
        Self::GeneratorProducer,
        Self::StreamTransform,
        Self::LineActivation,
        Self::LineTask,
    ];

    const DECODE: [Option<Self>; 256] = {
        let mut table = [None; 256];
        let mut index = 0;
        while index < Self::ALL.len() {
            let kind = Self::ALL[index];
            let encoded = kind as u8 as usize;
            assert!(table[encoded].is_none(), "duplicate AWBC function kind");
            table[encoded] = Some(kind);
            index += 1;
        }
        table
    };

    #[must_use]
    pub const fn encoded(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_encoded(encoded: u8) -> Option<Self> {
        Self::DECODE[encoded as usize]
    }

    #[must_use]
    pub const fn is_flow(self) -> bool {
        matches!(self, Self::Flow)
    }
}

impl Serialize for AwbcFunctionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.encoded())
    }
}

impl<'de> Deserialize<'de> for AwbcFunctionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = u8::deserialize(deserializer)?;
        Self::from_encoded(encoded).ok_or_else(|| {
            serde::de::Error::custom(format_args!("unknown AWBC function kind {encoded}"))
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AwbcFunctionFlag {
    Deterministic = 0,
    MayAllocate = 1,
    MaySuspend = 2,
    HasDynamicTarget = 3,
    NeedProducer = 4,
    OwnsStreamProducer = 5,
}

impl AwbcFunctionFlag {
    pub const ALL: &'static [Self] = &[
        Self::Deterministic,
        Self::MayAllocate,
        Self::MaySuspend,
        Self::HasDynamicTarget,
        Self::NeedProducer,
        Self::OwnsStreamProducer,
    ];

    #[must_use]
    pub const fn mask(self) -> u32 {
        1_u32 << (self as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AwbcFunctionFlags {
    bits: u32,
}

impl AwbcFunctionFlags {
    pub const KNOWN_MASK: u32 = {
        let mut mask = 0;
        let mut index = 0;
        while index < AwbcFunctionFlag::ALL.len() {
            mask |= AwbcFunctionFlag::ALL[index].mask();
            index += 1;
        }
        mask
    };

    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub const fn with(self, flag: AwbcFunctionFlag) -> Self {
        Self {
            bits: self.bits | flag.mask(),
        }
    }

    pub const fn try_from_bits(bits: u32) -> Result<Self, AwbcFunctionFlagsError> {
        if bits & !Self::KNOWN_MASK == 0 {
            Ok(Self { bits })
        } else {
            Err(AwbcFunctionFlagsError { bits })
        }
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.bits
    }

    #[must_use]
    pub const fn contains(self, flag: AwbcFunctionFlag) -> bool {
        self.bits & flag.mask() != 0
    }

    pub const fn validate_for_kind(
        self,
        kind: AwbcFunctionKind,
    ) -> Result<(), AwbcFunctionRoleError> {
        let need = self.contains(AwbcFunctionFlag::NeedProducer);
        let stream = self.contains(AwbcFunctionFlag::OwnsStreamProducer);
        if need && stream {
            return Err(AwbcFunctionRoleError::ConflictingProducerRoles);
        }
        if need {
            if !matches!(kind, AwbcFunctionKind::Synthetic) {
                return Err(AwbcFunctionRoleError::NeedProducerKind { actual: kind });
            }
            if !self.contains(AwbcFunctionFlag::Deterministic)
                || !self.contains(AwbcFunctionFlag::MayAllocate)
                || self.contains(AwbcFunctionFlag::MaySuspend)
                || self.contains(AwbcFunctionFlag::HasDynamicTarget)
            {
                return Err(AwbcFunctionRoleError::NeedProducerFlags);
            }
        }
        if matches!(kind, AwbcFunctionKind::GeneratorProducer) {
            if !stream || !self.contains(AwbcFunctionFlag::MaySuspend) {
                return Err(AwbcFunctionRoleError::StreamProducerFlags);
            }
        } else if stream {
            return Err(AwbcFunctionRoleError::StreamProducerKind { actual: kind });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
#[error("AWBC function flags contain unknown bits: {bits:#x}")]
pub struct AwbcFunctionFlagsError {
    bits: u32,
}

#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum AwbcFunctionRoleError {
    #[error("AWBC function cannot own both Need and Stream producer roles")]
    ConflictingProducerRoles,
    #[error("AWBC Need producer role requires Synthetic kind, found {actual:?}")]
    NeedProducerKind { actual: AwbcFunctionKind },
    #[error(
        "AWBC Need producer requires deterministic, allocating, non-suspending, static-target flags"
    )]
    NeedProducerFlags,
    #[error("AWBC Stream producer role requires GeneratorProducer kind, found {actual:?}")]
    StreamProducerKind { actual: AwbcFunctionKind },
    #[error("AWBC GeneratorProducer requires MaySuspend and OwnsStreamProducer flags")]
    StreamProducerFlags,
}

impl Serialize for AwbcFunctionFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.bits)
    }
}

impl<'de> Deserialize<'de> for AwbcFunctionFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from_bits(u32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

awbc_u8_enum! {
    /// Typed role of one value supplied to a dialogue content slot.
    pub enum AwbcDialogueValueRole {
        Interpolation = 0,
        Condition = 1,
    }
}

/// Register-backed value supplied at a dialogue safe point.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcDialogueValueBinding {
    pub slot: RuntimeDialogueValueSlotId,
    pub role: AwbcDialogueValueRole,
    pub value: AwbcRegisterId,
}

/// Parent-frame publication target for one typed dialogue result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcDialogueResultTarget {
    pub ty: AwbcTypeId,
    pub pattern: AwbcPatternId,
    pub destination: AwbcRegisterId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcBlock {
    pub owner: AwbcFunctionId,
    pub instructions: AwbcTableRange,
    pub terminator: AwbcTerminator,
    pub safe_point: AwbcSafePointKind,
    pub source_map: Option<AwbcSourceMapId>,
}

/// Top-level execution class of one AWBC opcode.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AwbcOpcodeClass {
    Instruction,
    Terminator,
}

/// Semantic instruction family owned by the opcode inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AwbcOpcodeFamily {
    Value,
    CallTask,
    StreamLine,
    Ownership,
    Terminator,
}

impl AwbcOpcodeFamily {
    #[must_use]
    pub const fn class(self) -> AwbcOpcodeClass {
        match self {
            Self::Value | Self::CallTask | Self::StreamLine | Self::Ownership => {
                AwbcOpcodeClass::Instruction
            }
            Self::Terminator => AwbcOpcodeClass::Terminator,
        }
    }
}

/// Stable opcode values. The discriminants are the canonical AWBC v1 wire
/// representation; [`Self::ALL`] and the compile-time decode table are the
/// sole inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AwbcOpcode {
    Nop = 0x00,
    LoadConst = 0x01,
    MakeTuple = 0x02,
    MakeSequence = 0x03,
    RepeatSequence = 0x04,
    MakeRecord = 0x05,
    MakeVariant = 0x06,
    MakeFunction = 0x07,
    MakeAgent = 0x08,
    MakeReductionUnchanged = 0x09,
    SequenceLen = 0x0a,
    SequenceGet = 0x0b,
    SequenceSlice = 0x0c,
    SequencePush = 0x0d,
    ProjectTuple = 0x0e,
    ProjectRecord = 0x0f,
    ProjectField = 0x10,
    AssignRecordField = 0x11,
    TestPattern = 0x12,
    Unary = 0x13,
    Binary = 0x14,
    CallPureHelper = 0x20,
    CallIntrinsic = 0x21,
    CallTraitMethod = 0x22,
    ApplyFunction = 0x23,
    EnsureContent = 0x24,
    EmitEffect = 0x25,
    StartTask = 0x26,
    SpawnFiber = 0x27,
    StreamYield = 0x32,
    StreamClose = 0x34,
    ExecuteLineOperation = 0x35,
    CommitDialogueResult = 0x36,
    Move = 0x40,
    CopyValue = 0x41,
    Clear = 0x42,
    Drop = 0x43,
    EnterScope = 0x44,
    ExitScope = 0x45,
    BindPattern = 0x46,
    RegisterCleanup = 0x47,
    CancelCleanup = 0x48,
    Jump = 0x80,
    Branch = 0x81,
    Match = 0x82,
    CallFunction = 0x83,
    GotoStatic = 0x84,
    GotoDynamic = 0x85,
    Return = 0x86,
    HostCall = 0x88,
    Await = 0x89,
    AwaitMany = 0x8a,
    BudgetYield = 0x8b,
    Dialogue = 0x98,
    Choice = 0x99,
    Trap = 0xa0,
    Unreachable = 0xa1,
}

impl AwbcOpcode {
    pub const ALL: &'static [Self] = &[
        Self::Nop,
        Self::LoadConst,
        Self::MakeTuple,
        Self::MakeSequence,
        Self::RepeatSequence,
        Self::MakeRecord,
        Self::MakeVariant,
        Self::MakeFunction,
        Self::MakeAgent,
        Self::MakeReductionUnchanged,
        Self::SequenceLen,
        Self::SequenceGet,
        Self::SequenceSlice,
        Self::SequencePush,
        Self::ProjectTuple,
        Self::ProjectRecord,
        Self::ProjectField,
        Self::AssignRecordField,
        Self::TestPattern,
        Self::Unary,
        Self::Binary,
        Self::CallPureHelper,
        Self::CallIntrinsic,
        Self::CallTraitMethod,
        Self::ApplyFunction,
        Self::EnsureContent,
        Self::EmitEffect,
        Self::StartTask,
        Self::SpawnFiber,
        Self::StreamYield,
        Self::StreamClose,
        Self::ExecuteLineOperation,
        Self::CommitDialogueResult,
        Self::Move,
        Self::CopyValue,
        Self::Clear,
        Self::Drop,
        Self::EnterScope,
        Self::ExitScope,
        Self::BindPattern,
        Self::RegisterCleanup,
        Self::CancelCleanup,
        Self::Jump,
        Self::Branch,
        Self::Match,
        Self::CallFunction,
        Self::GotoStatic,
        Self::GotoDynamic,
        Self::Return,
        Self::HostCall,
        Self::Await,
        Self::AwaitMany,
        Self::BudgetYield,
        Self::Dialogue,
        Self::Choice,
        Self::Trap,
        Self::Unreachable,
    ];

    const DECODE: [Option<Self>; 256] = Self::build_decode();

    const fn build_decode() -> [Option<Self>; 256] {
        let mut table = [None; 256];
        let mut index = 0;
        while index < Self::ALL.len() {
            let opcode = Self::ALL[index];
            let encoded = opcode as u8 as usize;
            assert!(
                table[encoded].is_none(),
                "duplicate AWBC opcode discriminant"
            );
            table[encoded] = Some(opcode);
            index += 1;
        }
        table
    }

    #[must_use]
    pub const fn encoded(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_encoded(value: u8) -> Option<Self> {
        Self::DECODE[value as usize]
    }

    #[must_use]
    pub const fn family(self) -> AwbcOpcodeFamily {
        match self {
            Self::Nop
            | Self::LoadConst
            | Self::MakeTuple
            | Self::MakeSequence
            | Self::RepeatSequence
            | Self::MakeRecord
            | Self::MakeVariant
            | Self::MakeFunction
            | Self::MakeAgent
            | Self::MakeReductionUnchanged
            | Self::SequenceLen
            | Self::SequenceGet
            | Self::SequenceSlice
            | Self::SequencePush
            | Self::ProjectTuple
            | Self::ProjectRecord
            | Self::ProjectField
            | Self::AssignRecordField
            | Self::TestPattern
            | Self::Unary
            | Self::Binary => AwbcOpcodeFamily::Value,
            Self::CallPureHelper
            | Self::CallIntrinsic
            | Self::CallTraitMethod
            | Self::ApplyFunction
            | Self::EnsureContent
            | Self::EmitEffect
            | Self::StartTask
            | Self::SpawnFiber => AwbcOpcodeFamily::CallTask,
            Self::StreamYield
            | Self::StreamClose
            | Self::ExecuteLineOperation
            | Self::CommitDialogueResult => AwbcOpcodeFamily::StreamLine,
            Self::Move
            | Self::CopyValue
            | Self::Clear
            | Self::Drop
            | Self::EnterScope
            | Self::ExitScope
            | Self::BindPattern
            | Self::RegisterCleanup
            | Self::CancelCleanup => AwbcOpcodeFamily::Ownership,
            Self::Jump
            | Self::Branch
            | Self::Match
            | Self::CallFunction
            | Self::GotoStatic
            | Self::GotoDynamic
            | Self::Return
            | Self::HostCall
            | Self::Await
            | Self::AwaitMany
            | Self::BudgetYield
            | Self::Dialogue
            | Self::Choice
            | Self::Trap
            | Self::Unreachable => AwbcOpcodeFamily::Terminator,
        }
    }

    #[must_use]
    pub const fn class(self) -> AwbcOpcodeClass {
        self.family().class()
    }
}

impl Serialize for AwbcOpcode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.encoded())
    }
}

impl<'de> Deserialize<'de> for AwbcOpcode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = u8::deserialize(deserializer)?;
        Self::from_encoded(encoded).ok_or_else(|| {
            serde::de::Error::custom(format_args!("unknown AWBC opcode {encoded:#04x}"))
        })
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
    CopyValue {
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
        field: AwbcFieldProjection,
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
    ExecuteLineOperation {
        dst: AwbcRegisterId,
        operation: AwbcLineOperationId,
        args: Vec<AwbcRegisterId>,
    },
    CommitDialogueResult {
        source: AwbcRegisterId,
    },
    Drop {
        register: AwbcRegisterId,
        policy: AwbcDropPolicy,
    },
    AssignRecordField {
        target: AwbcRegisterId,
        field: u32,
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
    MakeAgent {
        dst: AwbcRegisterId,
        constructor: RuntimeAgentConstructor,
        operands: Vec<AwbcRegisterId>,
    },
    /// Constructs the admitted unchanged result for `Reduction<State>`.
    MakeReductionUnchanged {
        dst: AwbcRegisterId,
        ty: AwbcTypeId,
        state: AwbcRegisterId,
    },
}

/// Closed, typed policy operand of the affine `Drop` instruction.
///
/// The optional fade payload belongs only to `Stop`; representing it in the
/// enum payload prevents invalid kind/register combinations from entering the
/// accepted AWBC instruction stream.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcDropPolicy {
    Default,
    Cancel,
    Stop { fade: AwbcRegisterId },
    Finish,
    Release,
    Detach,
}

/// Typed field coordinate consumed by the one generic `ProjectField` opcode.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcFieldProjection {
    Named(AwbcStringId),
    OpaqueRecord {
        owner: AwbcTypeId,
        field: u32,
        field_type: AwbcTypeId,
    },
}

impl AwbcInstruction {
    pub const fn opcode(&self) -> AwbcOpcode {
        match self {
            Self::Nop => AwbcOpcode::Nop,
            Self::LoadConst { .. } => AwbcOpcode::LoadConst,
            Self::Move { .. } => AwbcOpcode::Move,
            Self::CopyValue { .. } => AwbcOpcode::CopyValue,
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
            Self::ExecuteLineOperation { .. } => AwbcOpcode::ExecuteLineOperation,
            Self::CommitDialogueResult { .. } => AwbcOpcode::CommitDialogueResult,
            Self::Drop { .. } => AwbcOpcode::Drop,
            Self::AssignRecordField { .. } => AwbcOpcode::AssignRecordField,
            Self::CallTraitMethod { .. } => AwbcOpcode::CallTraitMethod,
            Self::RegisterCleanup { .. } => AwbcOpcode::RegisterCleanup,
            Self::CancelCleanup { .. } => AwbcOpcode::CancelCleanup,
            Self::MakeFunction { .. } => AwbcOpcode::MakeFunction,
            Self::ApplyFunction { .. } => AwbcOpcode::ApplyFunction,
            Self::MakeAgent { .. } => AwbcOpcode::MakeAgent,
            Self::MakeReductionUnchanged { .. } => AwbcOpcode::MakeReductionUnchanged,
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

awbc_u8_enum! {
    pub enum AwbcTraitReceiverMode {
        Owned = 0,
        SharedRef = 1,
        MutRef = 2,
    }
}

awbc_u8_enum! {
    pub enum AwbcBindMode {
        Declare = 0,
        Assign = 1,
    }
}

awbc_u8_enum! {
    pub enum AwbcUnaryOp {
        Not = 0,
        Neg = 1,
    }
}

awbc_u8_enum! {
    pub enum AwbcBinaryOp {
        Eq = 0,
        Ne = 1,
        Lt = 2,
        Le = 3,
        Gt = 4,
        Ge = 5,
        Add = 6,
        Sub = 7,
        Mul = 8,
        Div = 9,
        And = 10,
        Or = 11,
    }
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
        values: Vec<AwbcDialogueValueBinding>,
        line_task_captures: Vec<AwbcRegisterId>,
        result: AwbcDialogueResultTarget,
        resume: AwbcResumePointId,
    },
    Choice {
        choice: AwbcChoiceId,
        dst: AwbcRegisterId,
        resume: AwbcResumePointId,
    },
    Await {
        handle: AwbcRegisterId,
        binding: Option<AwbcPatternId>,
        observer: Option<AwbcAwaitObserverResume>,
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

/// Progress destination and observer dispatcher for one AWBC Await site.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcAwaitObserverResume {
    pub destination: AwbcRegisterId,
    pub resume: AwbcResumePointId,
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

awbc_u8_enum! {
    pub enum AwbcSafePointKind {
        FlowEntry = 0,
        CallableBoundary = 1,
        Dialogue = 2,
        Choice = 3,
        Await = 4,
        AwaitMany = 5,
        HostCall = 6,
        LoopBackedge = 7,
        BudgetYield = 8,
        Return = 9,
        Trap = 10,
        None = 11,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcResumePoint {
    pub function: AwbcFunctionId,
    pub block: AwbcBlockId,
    pub frame_layout: AwbcFrameLayoutId,
    pub kind: AwbcSafePointKind,
}

awbc_u8_enum! {
    pub enum AwbcTrapCode {
        TypeMismatch = 0,
        UninitializedRegister = 1,
        InvalidIndex = 2,
        DivisionByZero = 3,
        PatternMismatch = 4,
        MissingDynamicTarget = 5,
        HostAbiMismatch = 6,
        CapabilityDenied = 7,
        ExplicitPanic = 8,
        InternalInvariant = 9,
    }
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
    Entity(RuntimeEntityReference),
    Tuple(Vec<AwbcPatternId>),
    Record {
        ty: Option<AwbcTypeId>,
        fields: Vec<AwbcRecordPatternField>,
        rest: AwbcPatternRest,
    },
    Sequence {
        items: Vec<AwbcPatternId>,
        rest: AwbcPatternRest,
    },
    Variant {
        ty: AwbcTypeId,
        case: u32,
        case_name: AwbcStringId,
        payload: Option<AwbcPatternId>,
    },
    Whole {
        target: AwbcRegisterId,
        inner: AwbcPatternId,
    },
}

/// Exact, open, or binding remainder semantics shared by AWBC patterns.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcPatternRest {
    Exact,
    Ignore,
    Bind(AwbcRegisterId),
}

impl AwbcPatternRest {
    #[must_use]
    pub const fn accepts_len(self, required: usize, actual: usize) -> bool {
        match self {
            Self::Exact => required == actual,
            Self::Ignore | Self::Bind(_) => required <= actual,
        }
    }
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
    pub identity: crate::value::RuntimeCallTarget,
    pub signature: AwbcSignatureId,
    pub revision: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcHostCall {
    pub public_id: AwbcStringId,
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub contract: Option<crate::step::HostCallContractDigest>,
    pub signature: AwbcSignatureId,
    pub mode: AwbcHostCallMode,
    pub deterministic: bool,
    pub arguments: Vec<AwbcHostArgument>,
}

awbc_u8_enum! {
    pub enum AwbcHostCallMode {
        Immediate = 0,
        Suspend = 1,
    }
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
    /// Checked type of the unary temporal Ready payload.
    pub payload_type: AwbcTypeId,
    pub arguments: Vec<AwbcHostArgument>,
    pub many: Option<AwbcAwaitManyPolicy>,
}

awbc_u8_enum! {
    pub enum AwbcTaskClass {
        LocalView = 0,
        Io = 1,
        Cpu = 2,
        GpuPrepare = 3,
        ShaderCompile = 4,
        WasmCall = 5,
        AssetDecode = 6,
        AudioDecode = 7,
        AudioRender = 8,
        TtsSynthesis = 9,
        BgmPrecompose = 10,
        Lsp = 11,
        Background = 12,
    }
}

awbc_u8_enum! {
    pub enum AwbcTaskPolicy {
        JoinSameKey = 0,
        AlwaysStart = 1,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcHostArgument {
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

awbc_u8_enum! {
    pub enum AwbcEffectKind {
        Wait = 0,
        Audio = 1,
        Call = 2,
        Log = 3,
        SignalWrite = 4,
        MetricWrite = 5,
        EmitEvent = 6,
        Out = 7,
        Return = 8,
        Goto = 9,
        Panic = 10,
        Fail = 11,
        Bail = 12,
        Ensure = 13,
        Assert = 14,
        Close = 15,
        Select = 16,
        Break = 17,
        Continue = 18,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcResourceAccess {
    pub resource: AwbcResourceId,
    pub mode: AwbcResourceAccessMode,
    pub conflict: AwbcConflictPolicy,
}

awbc_u8_enum! {
    pub enum AwbcResourceAccessMode {
        Read = 0,
        Write = 1,
        Drop = 2,
        Append = 3,
        Control = 4,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcConflictPolicy {
    Error,
    Append,
    LastWriterWins { priority: i32 },
    MergePatch,
    Reduce { op: AwbcReduceOp },
}

awbc_u8_enum! {
    pub enum AwbcReduceOp {
        Sum = 0,
        Min = 1,
        Max = 2,
        And = 3,
        Or = 4,
    }
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
    pub marks: Vec<AwbcDialogueMark>,
    pub effect_site_count: u32,
    pub line_task_group: Option<AwbcLineTaskGroupId>,
    pub display: Option<AwbcDisplayMapId>,
    pub source: Option<AwbcSourceMapId>,
    pub resources: Vec<AwbcResourceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcDialogueMark {
    pub id: RuntimeDialogueMarkId,
    pub label: AwbcStringId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineTaskGroup {
    pub captures: Vec<RuntimeLocalDeclarationId>,
    pub activation: AwbcFunctionId,
    pub result_type: AwbcTypeId,
    pub handle_sites: Vec<AwbcLineHandleSite>,
    pub root: AwbcLineTaskNodeId,
    pub nodes: AwbcTableRange,
    pub cancel_handlers: Vec<AwbcLineCancelHandler>,
    pub cleanup_completed: Option<AwbcFunctionId>,
    pub cleanup_cancelled: Option<AwbcFunctionId>,
    pub cleanup_failed: Option<AwbcFunctionId>,
    pub cleanup: AwbcLineCleanupPolicy,
}

/// One dense typed handle-producing site owned by an AWBC line-task group.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineHandleSite {
    pub source_ordinal: u32,
    pub kind: RuntimeHandleKind,
    pub result_type: AwbcTypeId,
    pub character: Option<CharacterId>,
    pub scheduled_child: Option<AwbcLineTaskNodeId>,
}

/// Exact destination-local and type coordinate for one scheduled callback
/// capture. Values remain instruction operands; this row is the sealed child
/// binding ABI and prevents runtime position/type guessing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineScheduledCapture {
    pub local: RuntimeLocalDeclarationId,
    pub ty: AwbcTypeId,
}

/// Static typed identity and ABI of one executable line operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcLineOperation {
    AcquireActor {
        group: AwbcLineTaskGroupId,
        site: AwbcLineHandleSiteId,
        character: CharacterId,
        scope: crate::line_task::RuntimeLineHandleScope,
        result_type: AwbcTypeId,
    },
    Schedule {
        group: AwbcLineTaskGroupId,
        site: AwbcLineHandleSiteId,
        child: AwbcLineTaskNodeId,
        captures: Vec<AwbcLineScheduledCapture>,
        result_type: AwbcTypeId,
    },
    ActorLook {
        group: AwbcLineTaskGroupId,
        site: AwbcLineHandleSiteId,
        character: CharacterId,
        actor_type: AwbcTypeId,
        look_type: AwbcTypeId,
        result_type: AwbcTypeId,
    },
    VoiceHandle {
        group: AwbcLineTaskGroupId,
        site: AwbcLineHandleSiteId,
        result_type: AwbcTypeId,
    },
}

impl AwbcLineOperation {
    #[must_use]
    pub const fn group(&self) -> AwbcLineTaskGroupId {
        match self {
            Self::AcquireActor { group, .. }
            | Self::Schedule { group, .. }
            | Self::ActorLook { group, .. }
            | Self::VoiceHandle { group, .. } => *group,
        }
    }

    #[must_use]
    pub const fn site(&self) -> AwbcLineHandleSiteId {
        match self {
            Self::AcquireActor { site, .. }
            | Self::Schedule { site, .. }
            | Self::ActorLook { site, .. }
            | Self::VoiceHandle { site, .. } => *site,
        }
    }

    #[must_use]
    pub const fn result_type(&self) -> AwbcTypeId {
        match self {
            Self::AcquireActor { result_type, .. }
            | Self::Schedule { result_type, .. }
            | Self::ActorLook { result_type, .. }
            | Self::VoiceHandle { result_type, .. } => *result_type,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineCancelHandler {
    pub trigger: RuntimeDialogueMarkId,
    pub function: AwbcFunctionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcLineCleanupPolicy {
    pub child_tasks: AwbcChildCleanup,
    pub presentation: AwbcPresentationCleanup,
    pub audio: AwbcAudioCleanup,
}

awbc_u8_enum! {
    pub enum AwbcChildCleanup {
        CancelAndJoin = 0,
        Detach = 1,
        Finish = 2,
    }
}

awbc_u8_enum! {
    pub enum AwbcPresentationCleanup {
        DropRegistered = 0,
        KeepRegistered = 1,
    }
}

awbc_u8_enum! {
    pub enum AwbcAudioCleanup {
        StopRegistered = 0,
        FadeRegistered = 1,
        KeepRegistered = 2,
    }
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
        trigger: AwbcLineTaskTrigger,
        join: AwbcChildJoinPolicy,
        cancel: AwbcChildCancelPolicy,
        scope: AwbcLineTaskNodeId,
    },
    Action(AwbcFunctionId),
}

awbc_u8_enum! {
    pub enum AwbcParallelPolicy {
        JoinAll = 0,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcLineTaskTrigger {
    Immediate,
    Mark(RuntimeDialogueMarkId),
    Scheduled(AwbcLineHandleSiteId),
    ContentEffect(RuntimeDialogueEffectSiteId),
}

awbc_u8_enum! {
    pub enum AwbcChildJoinPolicy {
        Join = 0,
        Detached = 1,
    }
}

awbc_u8_enum! {
    pub enum AwbcChildCancelPolicy {
        CancelAndJoin = 0,
        Finish = 1,
        Detach = 2,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcStreamPlan {
    pub public_id: AwbcStringId,
    pub item_type: AwbcTypeId,
    pub error_type: AwbcTypeId,
    pub transform: AwbcFunctionId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcPureHelper {
    pub public_id: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub function: AwbcFunctionId,
    pub scalar_eval_supported: bool,
    pub origin: AwbcPureHelperOrigin,
}

/// Exact stable pure-program identity mapped to one verified helper row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcPureProgramBinding {
    pub program: arcweft_id::runtime_program::RuntimePureProgramId,
    pub helper: AwbcPureHelperId,
    pub input_types: Vec<RuntimeSemanticTypeId>,
    pub result_type: RuntimeSemanticTypeId,
}

awbc_u8_enum! {
    pub enum AwbcPureHelperOrigin {
        Annotated = 0,
        Inferred = 1,
        EngineOwned = 2,
    }
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

awbc_u8_enum! {
    pub enum AwbcResourceResidency {
        Startup = 0,
        OnDemand = 1,
        Streaming = 2,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcEntry {
    /// Canonical runtime lookup identity. `public_id` is presentation/debug
    /// text and must never be parsed back into semantic identity.
    pub runtime_id: crate::plan::EntryRuntimeId,
    pub binding: EntryBindingIdentity,
    pub public_id: AwbcStringId,
    pub kind: AwbcEntryKind,
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

/// Exact accepted semantic Flow identity mapped to its Product function.
///
/// This covers every lowered Flow. Entry-role metadata remains separately
/// represented by `AwbcFlowExecutable` and must not be used as a fallback
/// identity index.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcFlowBinding {
    pub flow: FlowRuntimeId,
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
    Function { function: AwbcFunctionId },
    Routes(Vec<AwbcRoute>),
}

impl AwbcEntryTarget {
    /// Returns the single function selected by this entry target, if one exists.
    #[must_use]
    pub const fn function(&self) -> Option<AwbcFunctionId> {
        match self {
            Self::Function { function, .. } => Some(*function),
            Self::Routes(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRoute {
    pub method: crate::plan::RuntimeHttpMethod,
    pub segments: Vec<AwbcRouteSegment>,
    pub target: AwbcFunctionId,
    pub bindings: Vec<AwbcRouteBinding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcRouteSegment {
    Literal(AwbcStringId),
    Capture(crate::plan::RouteCaptureCoordinate),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcRouteBinding {
    pub parameter: crate::entry::FlowParameterCoordinate,
    pub source: AwbcRouteBindingSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AwbcRouteBindingSource {
    PathCapture(crate::plan::RouteCaptureCoordinate),
}
