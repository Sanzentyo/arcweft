use crate::awbc_lower::audio::constant_audio_command;
use crate::awbc_lower::{AwbcLowerOptions, table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcAudioCleanup, AwbcAudioCommand, AwbcAudioCommandId, AwbcAwaitManyPolicy,
    AwbcBackpressurePolicy, AwbcBlock, AwbcBlockId, AwbcCallableExecutable, AwbcChildCancelPolicy,
    AwbcChildCleanup, AwbcChildJoinPolicy, AwbcChoice, AwbcChoiceId, AwbcChoiceOption,
    AwbcConstant, AwbcConstantId, AwbcContentUnit, AwbcContentUnitId, AwbcDisplayMapEntry,
    AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId, AwbcEffectSet, AwbcEffectSetId, AwbcEntry,
    AwbcEntryKind, AwbcEntryTarget, AwbcFlowBinding, AwbcFlowExecutable, AwbcFrameLayout,
    AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind,
    AwbcHostCall, AwbcHostCallId, AwbcHostCallMode, AwbcInstruction, AwbcInstructionId,
    AwbcLineCancelHandler, AwbcLineCleanupPolicy, AwbcLineOption, AwbcLineTaskGroup,
    AwbcLineTaskGroupId, AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger,
    AwbcOverflowPolicy, AwbcParallelPolicy, AwbcPattern, AwbcPatternId, AwbcPresentationCleanup,
    AwbcPrivacyPolicy, AwbcProgram, AwbcRegisterId, AwbcReplayPolicy, AwbcResumePoint,
    AwbcResumePointId, AwbcRoute, AwbcRouteBinding, AwbcRouteBindingSource, AwbcRuntimeType,
    AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcSignedIntKind, AwbcSourceEventKind,
    AwbcSourcePlan, AwbcSourcePlanId, AwbcSourcePolicy, AwbcStreamPlan, AwbcStreamPlanId,
    AwbcStringId, AwbcTableRange, AwbcTaskArgument, AwbcTaskClass, AwbcTaskPlan, AwbcTaskPlanId,
    AwbcTaskPolicy, AwbcTerminator, AwbcTypeId, AwbcUnsignedIntKind,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeEffectExpr, RuntimeWaitTarget};
use arcweft_core::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole};
use arcweft_core::line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineChildTask,
    LineCleanupPolicy, LineTaskGroup, LineTaskNode, LineTaskScope, ParallelPolicy,
    PresentationCleanup,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimeHostCallTarget,
    RuntimePlan,
};
use arcweft_core::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceHandlerPlan, SourceId,
    SourcePolicy,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::stream::StreamRuntimeId;
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeExpr, RuntimeInt, RuntimeRange, RuntimeUInt, RuntimeValue};
use arcweft_text_model::DialogueContentCatalog;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcLowerDiagnostic {
    pub kind: AwbcLowerDiagnosticKind,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcLowerDiagnosticKind {
    Warning,
    Error,
}

impl AwbcLowerDiagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: AwbcLowerDiagnosticKind::Error,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: AwbcLowerDiagnosticKind::Warning,
            path: path.into(),
            message: message.into(),
        }
    }

    pub const fn is_error(&self) -> bool {
        matches!(self.kind, AwbcLowerDiagnosticKind::Error)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AwbcLowerStats {
    pub strings: usize,
    pub constants: usize,
    pub functions: usize,
    pub blocks: usize,
    pub instructions: usize,
    pub patterns: usize,
    pub effects: usize,
    pub audio_commands: usize,
    pub task_plans: usize,
    pub source_plans: usize,
    pub stream_plans: usize,
    pub trait_methods: usize,
    pub callable_executables: usize,
    pub flow_bindings: usize,
    pub flow_executables: usize,
    pub entries: usize,
}

impl AwbcLowerStats {
    pub fn from_program(program: &AwbcProgram) -> Self {
        Self {
            strings: program.strings.len(),
            constants: program.constants.len(),
            functions: program.functions.len(),
            blocks: program.blocks.len(),
            instructions: program.instructions.len(),
            patterns: program.patterns.len(),
            effects: program.effect_plans.len(),
            audio_commands: program.audio_commands.len(),
            task_plans: program.task_plans.len(),
            source_plans: program.source_plans.len(),
            stream_plans: program.stream_plans.len(),
            trait_methods: program.trait_methods.len(),
            callable_executables: program.callable_executables.len(),
            flow_bindings: program.flow_bindings.len(),
            flow_executables: program.flow_executables.len(),
            entries: program.entries.len(),
        }
    }
}

/// Deterministic table inventory. Each `intern_*` method owns one table's
/// stable key; lowerers never scan unrelated tables ad hoc.
#[derive(Clone, Debug)]
pub struct AwbcInventory {
    pub(crate) program: AwbcProgram,
    pub(crate) diagnostics: Vec<AwbcLowerDiagnostic>,
    pub(crate) options: AwbcLowerOptions,
    strings: BTreeMap<String, AwbcStringId>,
    constants: BTreeMap<String, AwbcConstantId>,
    signatures: BTreeMap<String, AwbcSignatureId>,
    frame_layouts: BTreeMap<String, AwbcFrameLayoutId>,
    effects: BTreeMap<String, AwbcEffectPlanId>,
    audio_commands: BTreeMap<String, AwbcAudioCommandId>,
    tasks: BTreeMap<String, AwbcTaskPlanId>,
    host_calls: BTreeMap<String, AwbcHostCallId>,
    sources: BTreeMap<SourceId, AwbcSourcePlanId>,
    streams: BTreeMap<StreamRuntimeId, AwbcStreamPlanId>,
    choices: BTreeMap<String, AwbcChoiceId>,
    flow_functions: BTreeMap<FlowRuntimeId, AwbcFunctionId>,
    pending_closures: Vec<PendingAwbcClosure>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingAwbcClosure {
    pub function: AwbcFunctionId,
    pub params: Vec<(String, AwbcStringId)>,
    pub captures: Vec<(String, AwbcStringId)>,
    pub body: RuntimeExpr,
    pub path: String,
}

#[derive(Clone, Copy, Debug)]
struct NamedTaskSpec<'a> {
    public_id: &'a str,
    need_id: &'a str,
    capability: &'a str,
    operation: &'a str,
    args: &'a [HostTaskArgTemplate],
    class: AwbcTaskClass,
    priority: i32,
    cancel_scope: &'a str,
    policy: AwbcTaskPolicy,
}

impl AwbcInventory {
    pub fn new(source_label: &str, options: AwbcLowerOptions) -> Self {
        let mut this = Self {
            program: AwbcProgram::default(),
            diagnostics: Vec::new(),
            options,
            strings: BTreeMap::new(),
            constants: BTreeMap::new(),
            signatures: BTreeMap::new(),
            frame_layouts: BTreeMap::new(),
            effects: BTreeMap::new(),
            audio_commands: BTreeMap::new(),
            tasks: BTreeMap::new(),
            host_calls: BTreeMap::new(),
            sources: BTreeMap::new(),
            streams: BTreeMap::new(),
            choices: BTreeMap::new(),
            flow_functions: BTreeMap::new(),
            pending_closures: Vec::new(),
        };
        this.intern_string(source_label);
        this
    }

    pub fn finish(mut self) -> AwbcProgram {
        self.program.flow_bindings = self
            .flow_functions
            .into_iter()
            .map(|(flow, function)| AwbcFlowBinding { flow, function })
            .collect();
        self.program
    }

    pub fn take_diagnostics(&mut self) -> Vec<AwbcLowerDiagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    pub fn diagnostic(&mut self, diagnostic: AwbcLowerDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn intern_runtime_primitives(&mut self) {
        self.intern_type(AwbcRuntimeType::Unit);
        self.intern_type(AwbcRuntimeType::Dynamic);
        self.intern_type(AwbcRuntimeType::Bool);
        self.intern_type(AwbcRuntimeType::Int(AwbcSignedIntKind::I64));
        self.intern_type(AwbcRuntimeType::UInt(AwbcUnsignedIntKind::U64));
        self.intern_type(AwbcRuntimeType::F32);
        self.intern_type(AwbcRuntimeType::F64);
        self.intern_type(AwbcRuntimeType::String);
        self.intern_type(AwbcRuntimeType::EntityRef);
    }

    pub fn intern_dialogue_content_catalog(&mut self, catalog: &DialogueContentCatalog) {
        if !self.options.emit_display_map {
            return;
        }
        for spec in catalog.records() {
            let line = spec.line().public_label().into_string();
            let key = self.intern_string(&line);
            let content = self.intern_content_unit(&line, None);
            self.program.display_map.push(AwbcDisplayMapEntry {
                content,
                display_key: key,
            });
        }
    }

    pub fn intern_string(&mut self, value: &str) -> AwbcStringId {
        if let Some(id) = self.strings.get(value).copied() {
            return id;
        }
        let id = AwbcStringId(table_index(self.program.strings.len()));
        self.program.strings.push(value.to_owned());
        self.strings.insert(value.to_owned(), id);
        id
    }

    pub fn string(&self, id: AwbcStringId) -> &str {
        self.program
            .strings
            .get(id.index())
            .map_or("<bad-string>", String::as_str)
    }

    pub fn dynamic_ty(&self) -> AwbcTypeId {
        AwbcTypeId(1)
    }

    pub fn unit_ty(&self) -> AwbcTypeId {
        AwbcTypeId(0)
    }

    pub fn bool_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeType::Bool)
    }

    pub fn i64_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeType::Int(AwbcSignedIntKind::I64))
    }

    pub fn string_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeType::String)
    }

    pub fn intern_type(&mut self, ty: AwbcRuntimeType) -> AwbcTypeId {
        if let Some((index, _)) = self
            .program
            .runtime_types
            .iter()
            .enumerate()
            .find(|(_, candidate)| **candidate == ty)
        {
            return AwbcTypeId(table_index(index));
        }
        let id = AwbcTypeId(table_index(self.program.runtime_types.len()));
        self.program.runtime_types.push(ty);
        id
    }

    pub fn intern_effect_set(&mut self, mut effects: Vec<&str>) -> AwbcEffectSetId {
        effects.sort_unstable();
        effects.dedup();
        let ids = effects
            .into_iter()
            .map(|effect| self.intern_string(effect))
            .collect::<Vec<_>>();
        if let Some((index, _)) = self
            .program
            .effect_sets
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.effects == ids)
        {
            return AwbcEffectSetId(table_index(index));
        }
        let id = AwbcEffectSetId(table_index(self.program.effect_sets.len()));
        self.program
            .effect_sets
            .push(AwbcEffectSet { effects: ids });
        id
    }

    pub fn intern_signature(
        &mut self,
        params: Vec<AwbcTypeId>,
        result: Option<AwbcTypeId>,
        effects: AwbcEffectSetId,
    ) -> AwbcSignatureId {
        let key = format!("{params:?}->{result:?}/{effects:?}");
        if let Some(id) = self.signatures.get(&key).copied() {
            return id;
        }
        let id = AwbcSignatureId(table_index(self.program.signatures.len()));
        self.program.signatures.push(AwbcSignature {
            params,
            result,
            effects,
        });
        self.signatures.insert(key, id);
        id
    }

    pub fn intern_unit_signature(&mut self) -> AwbcSignatureId {
        self.intern_signature(Vec::new(), None, AwbcEffectSetId(0))
    }

    pub fn intern_dynamic_value_signature(&mut self, arity: usize) -> AwbcSignatureId {
        self.intern_signature(
            vec![self.dynamic_ty(); arity],
            Some(self.dynamic_ty()),
            AwbcEffectSetId(0),
        )
    }

    pub fn intern_frame_layout(
        &mut self,
        key: String,
        layout: AwbcFrameLayout,
    ) -> AwbcFrameLayoutId {
        if let Some(id) = self.frame_layouts.get(&key).copied() {
            return id;
        }
        let id = AwbcFrameLayoutId(table_index(self.program.frame_layouts.len()));
        self.program.frame_layouts.push(layout);
        self.frame_layouts.insert(key, id);
        id
    }

    pub fn constant_runtime_value(&mut self, value: &RuntimeValue) -> AwbcConstantId {
        let key = format!("runtime:{value:?}");
        if let Some(id) = self.constants.get(&key).copied() {
            return id;
        }
        let constant = self.runtime_value_constant(value);
        let id = AwbcConstantId(table_index(self.program.constants.len()));
        self.program.constants.push(constant);
        self.constants.insert(key, id);
        id
    }

    pub fn constant_string(&mut self, value: &str) -> AwbcConstantId {
        let key = format!("string:{value}");
        if let Some(id) = self.constants.get(&key).copied() {
            return id;
        }
        let id = AwbcConstantId(table_index(self.program.constants.len()));
        let string = self.intern_string(value);
        self.program.constants.push(AwbcConstant::String(string));
        self.constants.insert(key, id);
        id
    }

    pub fn constant_bytes(&mut self, value: &[u8]) -> AwbcConstantId {
        let key = format!("bytes:{value:02x?}");
        if let Some(id) = self.constants.get(&key).copied() {
            return id;
        }
        let id = AwbcConstantId(table_index(self.program.constants.len()));
        self.program
            .constants
            .push(AwbcConstant::Bytes(value.to_vec()));
        self.constants.insert(key, id);
        id
    }

    pub fn constant_unit(&mut self) -> AwbcConstantId {
        let key = "unit".to_owned();
        if let Some(id) = self.constants.get(&key).copied() {
            return id;
        }
        let id = AwbcConstantId(table_index(self.program.constants.len()));
        self.program.constants.push(AwbcConstant::Unit);
        self.constants.insert(key, id);
        id
    }

    fn runtime_value_constant(&mut self, value: &RuntimeValue) -> AwbcConstant {
        match value {
            RuntimeValue::Unit => AwbcConstant::Unit,
            RuntimeValue::Bool(value) => AwbcConstant::Bool(*value),
            RuntimeValue::Int(value) => signed_constant(*value),
            RuntimeValue::UInt(value) => unsigned_constant(*value),
            RuntimeValue::F32(value) => AwbcConstant::F32Bits(value.to_bits()),
            RuntimeValue::F64(value) => AwbcConstant::F64Bits(value.to_bits()),
            RuntimeValue::String(value) => AwbcConstant::String(self.intern_string(value)),
            RuntimeValue::Char(value) => AwbcConstant::Char(u32::from(*value)),
            RuntimeValue::Duration(value) => AwbcConstant::DurationNanos(value.as_nanos()),
            RuntimeValue::EntityRef(value) => AwbcConstant::EntityRef(self.intern_string(value)),
            RuntimeValue::Tuple(items) => AwbcConstant::Tuple(
                items
                    .iter()
                    .map(|item| self.constant_runtime_value(item))
                    .collect(),
            ),
            RuntimeValue::Seq(seq) => AwbcConstant::Sequence(
                seq.clone()
                    .into_values()
                    .iter()
                    .map(|item| self.constant_runtime_value(item))
                    .collect(),
            ),
            RuntimeValue::Record(fields) => AwbcConstant::Record {
                ty: self.dynamic_ty(),
                field_names: fields
                    .iter()
                    .map(|field| self.intern_string(&field.name))
                    .collect(),
                fields: fields
                    .iter()
                    .map(|field| self.constant_runtime_value(&field.value))
                    .collect(),
            },
            RuntimeValue::NominalRecord(record) => {
                panic!(
                    "nominal runtime record `{}` cannot be encoded as an AWBC constant before \
                     its nominal schema is registered",
                    record.type_id().as_str()
                )
            }
            RuntimeValue::Variant {
                owner,
                ordinal,
                name,
                ..
            } => panic!(
                "runtime variant `{owner:?}` case {ordinal} `{name}` requires a checked RuntimeExpr::Variant type and cannot be encoded through the type-erased constant API"
            ),
            RuntimeValue::Range(range) => self.range_constant(range),
            RuntimeValue::Iterator(_) => {
                panic!("runtime iterator state cannot be encoded as an AWBC constant")
            }
            RuntimeValue::Function(_) => {
                panic!("runtime function state cannot be encoded as an AWBC constant")
            }
            RuntimeValue::MatrixF32(matrix) => AwbcConstant::TensorF32 {
                shape: vec![table_index(matrix.rows()), table_index(matrix.cols())],
                values: matrix
                    .values()
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            },
            RuntimeValue::MatrixF64(matrix) => AwbcConstant::TensorF64 {
                shape: vec![table_index(matrix.rows()), table_index(matrix.cols())],
                values: matrix
                    .values()
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            },
            RuntimeValue::TensorF32(tensor) => AwbcConstant::TensorF32 {
                shape: tensor
                    .shape()
                    .dims()
                    .iter()
                    .map(|dim| table_index(*dim))
                    .collect(),
                values: tensor
                    .values()
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            },
            RuntimeValue::TensorF64(tensor) => AwbcConstant::TensorF64 {
                shape: tensor
                    .shape()
                    .dims()
                    .iter()
                    .map(|dim| table_index(*dim))
                    .collect(),
                values: tensor
                    .values()
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            },
        }
    }

    fn range_constant(&mut self, range: &RuntimeRange) -> AwbcConstant {
        match range {
            RuntimeRange::Int {
                start,
                end,
                inclusive,
            } => AwbcConstant::Range {
                start: start.map(|value| self.constant_runtime_value(&RuntimeValue::Int(value))),
                end: end.map(|value| self.constant_runtime_value(&RuntimeValue::Int(value))),
                inclusive: *inclusive,
            },
            RuntimeRange::UInt {
                start,
                end,
                inclusive,
            } => AwbcConstant::Range {
                start: start.map(|value| self.constant_runtime_value(&RuntimeValue::UInt(value))),
                end: end.map(|value| self.constant_runtime_value(&RuntimeValue::UInt(value))),
                inclusive: *inclusive,
            },
        }
    }

    pub fn intern_pattern(&mut self, pattern: AwbcPattern) -> AwbcPatternId {
        let id = AwbcPatternId(table_index(self.program.patterns.len()));
        self.program.patterns.push(pattern);
        id
    }

    pub fn push_instruction(&mut self, instruction: AwbcInstruction) -> AwbcInstructionId {
        let id = AwbcInstructionId(table_index(self.program.instructions.len()));
        self.program.instructions.push(instruction);
        id
    }

    pub fn push_block(&mut self, block: AwbcBlock) -> AwbcBlockId {
        let id = AwbcBlockId(table_index(self.program.blocks.len()));
        self.program.blocks.push(block);
        id
    }

    pub fn push_resume_point(&mut self, resume: AwbcResumePoint) -> AwbcResumePointId {
        let id = AwbcResumePointId(table_index(self.program.resume_points.len()));
        self.program.resume_points.push(resume);
        id
    }

    pub fn push_function(&mut self, function: AwbcFunction) -> AwbcFunctionId {
        let id = AwbcFunctionId(table_index(self.program.functions.len()));
        self.program.functions.push(function);
        id
    }

    pub fn reserve_function_slot(&mut self) -> AwbcFunctionId {
        let id = AwbcFunctionId(table_index(self.program.functions.len()));
        self.program.functions.push(AwbcFunction {
            public_id: None,
            kind: AwbcFunctionKind::Synthetic,
            signature: AwbcSignatureId::default(),
            frame_layout: AwbcFrameLayoutId::default(),
            blocks: AwbcTableRange::new(0, 0),
            entry_block: AwbcBlockId::default(),
            flags: AwbcFunctionFlags(0),
        });
        id
    }

    pub fn reserve_flow_function_slot(&mut self, flow: &FlowRuntimeId) -> AwbcFunctionId {
        let id = self.reserve_function_slot();
        self.flow_functions.insert(flow.clone(), id);
        id
    }

    pub fn replace_function(
        &mut self,
        id: AwbcFunctionId,
        function: AwbcFunction,
    ) -> AwbcFunctionId {
        if let Some(slot) = self.program.functions.get_mut(id.index()) {
            *slot = function;
        } else {
            debug_assert_eq!(id.index(), self.program.functions.len());
            self.program.functions.push(function);
        }
        id
    }

    pub fn replace_flow_function(
        &mut self,
        flow: &FlowRuntimeId,
        id: AwbcFunctionId,
        function: AwbcFunction,
    ) -> AwbcFunctionId {
        let id = self.replace_function(id, function);
        self.flow_functions.insert(flow.clone(), id);
        id
    }

    pub fn flow_function(&self, flow: &FlowRuntimeId) -> Option<AwbcFunctionId> {
        self.flow_functions.get(flow).copied()
    }

    pub(crate) fn push_pending_closure(&mut self, closure: PendingAwbcClosure) {
        self.pending_closures.push(closure);
    }

    pub(crate) fn pop_pending_closure(&mut self) -> Option<PendingAwbcClosure> {
        self.pending_closures.pop()
    }

    /// Attaches bounded fan-out semantics to an owned task plan.
    pub fn set_await_many_policy(
        &mut self,
        plan: AwbcTaskPlanId,
        item_binding: AwbcRegisterId,
        limit: usize,
    ) {
        let limit = u32::try_from(limit).unwrap_or(u32::MAX).max(1);
        if let Some(task) = self.program.task_plans.get_mut(plan.index()) {
            task.many = Some(AwbcAwaitManyPolicy {
                item_binding,
                limit,
            });
        }
    }

    pub fn intern_content_unit(
        &mut self,
        public_id: &str,
        group: Option<AwbcLineTaskGroupId>,
    ) -> AwbcContentUnitId {
        if let Some((index, _)) = self
            .program
            .content_units
            .iter()
            .enumerate()
            .find(|(_, unit)| self.string(unit.public_id) == public_id)
        {
            return AwbcContentUnitId(table_index(index));
        }
        let id = AwbcContentUnitId(table_index(self.program.content_units.len()));
        let public_id = self.intern_string(public_id);
        self.program.content_units.push(AwbcContentUnit {
            public_id,
            line_task_group: group,
            display: None,
            source: None,
            resources: Vec::new(),
        });
        id
    }

    pub fn lower_line_task_group(&mut self, group: &LineTaskGroup) -> AwbcLineTaskGroupId {
        let root = self.lower_line_task_scope(&group.root);
        let options = group
            .options
            .iter()
            .map(|option| AwbcLineOption {
                name: self.intern_string(&option.name),
                value: self.constant_string(&option.value),
            })
            .collect();
        let cancel_handlers = group
            .cancel_rules
            .iter()
            .map(|rule| AwbcLineCancelHandler {
                trigger: self.intern_string(&rule.trigger),
                function: self.synthetic_empty_function("line.cancel"),
            })
            .collect();
        let id = AwbcLineTaskGroupId(table_index(self.program.line_task_groups.len()));
        self.program.line_task_groups.push(AwbcLineTaskGroup {
            root,
            options,
            bindings: None,
            out: None,
            cancel_handlers,
            cleanup: line_cleanup(&group.cleanup),
        });
        id
    }

    pub fn lower_line_task_scope(&mut self, scope: &LineTaskScope) -> AwbcLineTaskNodeId {
        self.lower_line_task_node(&scope.node)
    }

    pub fn lower_line_task_node(&mut self, node: &LineTaskNode) -> AwbcLineTaskNodeId {
        let lowered = match node {
            LineTaskNode::Seq(nodes) => AwbcLineTaskNode::Sequence(
                nodes
                    .iter()
                    .map(|node| self.lower_line_task_node(node))
                    .collect(),
            ),
            LineTaskNode::Start(nodes) => AwbcLineTaskNode::Start(
                nodes
                    .iter()
                    .map(|node| self.lower_line_task_node(node))
                    .collect(),
            ),
            LineTaskNode::Parallel { policy, children } => AwbcLineTaskNode::Parallel {
                policy: match policy {
                    ParallelPolicy::JoinAll => AwbcParallelPolicy::JoinAll,
                },
                children: children
                    .iter()
                    .map(|node| self.lower_line_task_node(node))
                    .collect(),
            },
            LineTaskNode::Child(task) => self.lower_line_child_task(task),
            LineTaskNode::Effect(effect) => AwbcLineTaskNode::Effect(self.intern_effect(effect)),
        };
        let id = AwbcLineTaskNodeId(table_index(self.program.line_task_nodes.len()));
        self.program.line_task_nodes.push(lowered);
        id
    }

    fn lower_line_child_task(&mut self, task: &LineChildTask) -> AwbcLineTaskNode {
        AwbcLineTaskNode::Child {
            task: self.intern_named_task(NamedTaskSpec {
                public_id: task.id.0.as_str(),
                need_id: task.id.0.as_str(),
                capability: "line_task",
                operation: "run_child",
                args: &[],
                class: AwbcTaskClass::LocalView,
                priority: task.priority.0,
                cancel_scope: "line",
                policy: AwbcTaskPolicy::JoinSameKey,
            }),
            trigger: match &task.trigger {
                arcweft_core::line_task::LineTaskTrigger::Immediate => {
                    AwbcLineTaskTrigger::Immediate
                }
                arcweft_core::line_task::LineTaskTrigger::Mark(mark) => {
                    AwbcLineTaskTrigger::Mark(self.intern_string(mark))
                }
                arcweft_core::line_task::LineTaskTrigger::Delay(duration) => {
                    AwbcLineTaskTrigger::DelayNanos(duration.as_nanos())
                }
            },
            join: match task.join_policy {
                ChildJoinPolicy::Join => AwbcChildJoinPolicy::Join,
                ChildJoinPolicy::Detached => AwbcChildJoinPolicy::Detached,
            },
            cancel: match task.cancel_policy {
                ChildCancelPolicy::CancelAndJoin => AwbcChildCancelPolicy::CancelAndJoin,
                ChildCancelPolicy::Finish => AwbcChildCancelPolicy::Finish,
                ChildCancelPolicy::Detach => AwbcChildCancelPolicy::Detach,
            },
            scope: self.lower_line_task_scope(&task.scope),
        }
    }

    pub fn intern_effect(&mut self, effect: &LineEffectRequest) -> AwbcEffectPlanId {
        let key = format!("effect:{effect:?}");
        if let Some(id) = self.effects.get(&key).copied() {
            return id;
        }
        if let LineEffectRequest::Audio(command) = effect {
            let command = constant_audio_command(self, command, "line_task.audio");
            return self.intern_audio_effect(command, 0);
        }
        let id = AwbcEffectPlanId(table_index(self.program.effect_plans.len()));
        let kind = effect_kind(effect);
        let signature = self.intern_unit_signature();
        let capability = effect_capability(effect).map(|capability| self.intern_string(capability));
        let static_args = effect_static_args(self, effect);
        self.program.effect_plans.push(AwbcEffectPlan {
            kind,
            signature,
            capability,
            audio: None,
            static_args,
            resources: Vec::new(),
        });
        self.effects.insert(key, id);
        id
    }

    pub fn intern_evaluated_effect(&mut self, effect: &RuntimeEffectExpr) -> AwbcEffectPlanId {
        let descriptor = effect.descriptor();
        let arg_count = effect.argument_exprs().len();
        let key = format!("effect:evaluated:{descriptor:?}:{arg_count}");
        if let Some(id) = self.effects.get(&key).copied() {
            return id;
        }
        let id = AwbcEffectPlanId(table_index(self.program.effect_plans.len()));
        let kind = effect_kind(&descriptor);
        let signature =
            self.intern_signature(vec![self.dynamic_ty(); arg_count], None, AwbcEffectSetId(0));
        let capability =
            effect_capability(&descriptor).map(|capability| self.intern_string(capability));
        let static_args = effect_static_args(self, &descriptor);
        self.program.effect_plans.push(AwbcEffectPlan {
            kind,
            signature,
            capability,
            audio: None,
            static_args,
            resources: Vec::new(),
        });
        self.effects.insert(key, id);
        id
    }

    pub fn intern_audio_effect(
        &mut self,
        command: AwbcAudioCommand,
        arg_count: usize,
    ) -> AwbcEffectPlanId {
        let audio = self.intern_audio_command(command);
        let key = format!("effect:audio:{audio:?}:{arg_count}");
        if let Some(id) = self.effects.get(&key).copied() {
            return id;
        }
        let id = AwbcEffectPlanId(table_index(self.program.effect_plans.len()));
        let signature =
            self.intern_signature(vec![self.dynamic_ty(); arg_count], None, AwbcEffectSetId(0));
        let capability = Some(self.intern_string("audio"));
        self.program.effect_plans.push(AwbcEffectPlan {
            kind: AwbcEffectKind::Audio,
            signature,
            capability,
            audio: Some(audio),
            static_args: Vec::new(),
            resources: Vec::new(),
        });
        self.effects.insert(key, id);
        id
    }

    fn intern_audio_command(&mut self, command: AwbcAudioCommand) -> AwbcAudioCommandId {
        let key = format!("audio:{command:?}");
        if let Some(id) = self.audio_commands.get(&key).copied() {
            return id;
        }
        let id = AwbcAudioCommandId(table_index(self.program.audio_commands.len()));
        self.program.audio_commands.push(command);
        self.audio_commands.insert(key, id);
        id
    }

    pub fn intern_host_task(
        &mut self,
        need_id: &str,
        task_id: &str,
        request: &HostTaskRequestTemplate,
    ) -> AwbcTaskPlanId {
        self.intern_named_task(NamedTaskSpec {
            public_id: task_id,
            need_id,
            capability: &request.capability.0,
            operation: &request.operation,
            args: &request.args,
            class: AwbcTaskClass::Io,
            priority: 0,
            cancel_scope: "flow",
            policy: AwbcTaskPolicy::JoinSameKey,
        })
    }

    pub fn intern_host_call(&mut self, target: &RuntimeHostCallTarget) -> AwbcHostCallId {
        let key = format!(
            "host_call:{}:{}:{}:{:?}:{}:{}",
            target.public_id,
            target.capability,
            target.operation,
            target.args,
            target.mode as u8,
            target.deterministic
        );
        if let Some(id) = self.host_calls.get(&key).copied() {
            return id;
        }
        let id = AwbcHostCallId(table_index(self.program.host_calls.len()));
        let signature = self.intern_signature(
            vec![self.dynamic_ty(); target.args.len()],
            Some(self.dynamic_ty()),
            AwbcEffectSetId(0),
        );
        let mode = match target.mode {
            RuntimeHostCallMode::Immediate => AwbcHostCallMode::Immediate,
            RuntimeHostCallMode::Suspend => AwbcHostCallMode::Suspend,
        };
        let public_id = self.intern_string(&target.public_id);
        let capability = self.intern_string(&target.capability);
        let operation = self.intern_string(&target.operation);
        self.program.host_calls.push(AwbcHostCall {
            public_id,
            capability,
            operation,
            signature,
            mode,
            deterministic: target.deterministic,
        });
        self.host_calls.insert(key, id);
        id
    }

    fn intern_named_task(&mut self, spec: NamedTaskSpec<'_>) -> AwbcTaskPlanId {
        let NamedTaskSpec {
            public_id,
            need_id,
            capability,
            operation,
            args,
            class,
            priority,
            cancel_scope,
            policy,
        } = spec;
        let key = format!(
            "task:{public_id}:{need_id}:{capability}:{operation}:{args:?}:{class:?}:{priority}:{cancel_scope}:{policy:?}"
        );
        if let Some(id) = self.tasks.get(&key).copied() {
            return id;
        }
        let id = AwbcTaskPlanId(table_index(self.program.task_plans.len()));
        let public_id = self.intern_string(public_id);
        let need_id = self.intern_string(need_id);
        let capability = self.intern_string(capability);
        let operation = self.intern_string(operation);
        let signature = self.intern_signature(
            vec![self.dynamic_ty(); args.len()],
            None,
            AwbcEffectSetId(0),
        );
        let cancel_scope = self.intern_string(cancel_scope);
        let arguments = args
            .iter()
            .map(|arg| AwbcTaskArgument {
                name: arg.name().map(|name| self.intern_string(name)),
                spread: arg.is_spread(),
            })
            .collect();
        self.program.task_plans.push(AwbcTaskPlan {
            public_id,
            need_id,
            capability,
            operation,
            signature,
            class,
            priority,
            cancel_scope,
            policy,
            arguments,
            many: None,
        });
        self.tasks.insert(key, id);
        id
    }

    pub fn reserve_source_plan_id(&mut self, source: SourceId, id: AwbcSourcePlanId) {
        self.sources.insert(source, id);
    }

    pub fn push_source_plan(&mut self, source: SourceId, plan: AwbcSourcePlan) -> AwbcSourcePlanId {
        let id = AwbcSourcePlanId(table_index(self.program.source_plans.len()));
        self.program.source_plans.push(plan);
        self.sources.entry(source).or_insert(id);
        id
    }

    pub fn source_plan_id(&self, source: &SourceId) -> Option<AwbcSourcePlanId> {
        self.sources.get(source).copied()
    }

    pub fn reserve_stream_plan_id(&mut self, stream: StreamRuntimeId, id: AwbcStreamPlanId) {
        self.streams.insert(stream, id);
    }

    pub fn push_stream_plan(
        &mut self,
        stream: StreamRuntimeId,
        plan: AwbcStreamPlan,
    ) -> AwbcStreamPlanId {
        let id = AwbcStreamPlanId(table_index(self.program.stream_plans.len()));
        self.program.stream_plans.push(plan);
        self.streams.entry(stream).or_insert(id);
        id
    }

    pub fn stream_plan_id(&self, stream: &StreamRuntimeId) -> Option<AwbcStreamPlanId> {
        self.streams.get(stream).copied()
    }

    pub fn intern_choice(
        &mut self,
        key: String,
        public_id: Option<AwbcStringId>,
        options: Vec<AwbcChoiceOption>,
    ) -> AwbcChoiceId {
        if let Some(id) = self.choices.get(&key).copied() {
            return id;
        }
        let option_start = table_index(self.program.choice_options.len());
        self.program.choice_options.extend(options);
        let id = AwbcChoiceId(table_index(self.program.choices.len()));
        self.program.choices.push(AwbcChoice {
            public_id,
            options: AwbcTableRange::new(
                option_start,
                table_range_len(option_start, self.program.choice_options.len()),
            ),
        });
        self.choices.insert(key, id);
        id
    }

    pub fn lower_entries(&mut self, plan: &RuntimePlan) {
        self.lower_callable_executables(plan);
        self.lower_flow_executables(plan);
        for entry in &plan.entries {
            self.lower_entry(entry);
        }
    }

    fn lower_callable_executables(&mut self, plan: &RuntimePlan) {
        for executable in &plan.callable_executables {
            let function = match &executable.code {
                RuntimeCallableExecutableCode::PureHelper(helper) => self
                    .program
                    .pure_helpers
                    .get(helper.0)
                    .map(|helper| helper.function),
                RuntimeCallableExecutableCode::ControllerFlow(flow) => self.flow_function(flow),
            };
            let Some(function) = function else {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    executable.callable.as_str(),
                    "callable executable maps to missing Product AWBC function",
                ));
                continue;
            };
            self.program
                .callable_executables
                .push(AwbcCallableExecutable {
                    role: RuntimeCallableRole {
                        callable: executable.callable.clone(),
                        contract: executable.contract,
                    },
                    function,
                });
        }
    }

    fn lower_flow_executables(&mut self, plan: &RuntimePlan) {
        for executable in &plan.flow_executables {
            let Some(function) = self.flow_function(&executable.flow) else {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    executable.flow.canonical_label(),
                    "flow executable maps to missing Product AWBC function",
                ));
                continue;
            };
            self.program.flow_executables.push(AwbcFlowExecutable {
                metadata: executable.clone(),
                function,
            });
        }
    }

    fn lower_entry(&mut self, entry: &RuntimeEntrySpec) {
        let entry_public_id = entry.id.public_label().into_string();
        let public_id = self.intern_string(&entry_public_id);
        let kind = match &entry.kind {
            RuntimeEntryKind::Game => AwbcEntryKind::Game,
            RuntimeEntryKind::Editor => AwbcEntryKind::Editor,
            RuntimeEntryKind::Cli => AwbcEntryKind::Cli,
            RuntimeEntryKind::Server => AwbcEntryKind::Server,
            RuntimeEntryKind::Activity => AwbcEntryKind::Activity,
            RuntimeEntryKind::Test => AwbcEntryKind::Test,
            RuntimeEntryKind::Bench => AwbcEntryKind::Bench,
            RuntimeEntryKind::Agent => AwbcEntryKind::Agent,
            RuntimeEntryKind::Custom(value) => AwbcEntryKind::Custom(self.intern_string(value)),
        };
        let mut signature = self.intern_unit_signature();
        let target = match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                let flow_public_id = flow.public_label().into_string();
                if let Some(function) = self.flow_function(flow) {
                    signature = self.program.functions[function.index()].signature;
                    AwbcEntryTarget::Function(function)
                } else {
                    self.diagnostic(AwbcLowerDiagnostic::error(
                        entry_public_id.clone(),
                        format!("entry targets missing flow {flow_public_id}"),
                    ));
                    AwbcEntryTarget::Function(AwbcFunctionId(0))
                }
            }
            RuntimeEntryTarget::Routes(routes) => {
                let routes = routes
                    .iter()
                    .map(|route| {
                        let target_public_id = route.target.public_label().into_string();
                        let target = self.flow_function(&route.target).unwrap_or_else(|| {
                            self.diagnostic(AwbcLowerDiagnostic::error(
                                entry_public_id.clone(),
                                format!("route targets missing flow {target_public_id}"),
                            ));
                            AwbcFunctionId(0)
                        });
                        signature = self.program.functions[target.index()].signature;
                        AwbcRoute {
                            method: self.intern_string(&route.method),
                            path: self.intern_string(&route.path),
                            target,
                            bindings: route
                                .bindings
                                .iter()
                                .enumerate()
                                .map(|(index, binding)| {
                                    AwbcRouteBinding {
                                    register: AwbcRegisterId(table_index(index)),
                                    source: match &binding.source {
                                        arcweft_core::plan::RuntimeRouteBindingSource::PathParam(
                                            value,
                                        ) => AwbcRouteBindingSource::PathParameter(
                                            self.intern_string(value),
                                        ),
                                    },
                                }
                                })
                                .collect(),
                        }
                    })
                    .collect();
                AwbcEntryTarget::Routes(routes)
            }
        };
        self.program.entries.push(AwbcEntry {
            runtime_id: entry.id.clone(),
            binding: entry.binding,
            public_id,
            kind,
            signature,
            target,
            roles: entry.roles.clone(),
        });
    }

    pub fn synthetic_empty_function(&mut self, name: &str) -> AwbcFunctionId {
        self.empty_function(name, AwbcFunctionKind::Synthetic, AwbcSafePointKind::Return)
    }

    pub fn source_open_function(&mut self, name: &str) -> AwbcFunctionId {
        self.empty_function(
            name,
            AwbcFunctionKind::SourceOpen,
            AwbcSafePointKind::CallableBoundary,
        )
    }

    fn empty_function(
        &mut self,
        name: &str,
        kind: AwbcFunctionKind,
        safe_point: AwbcSafePointKind,
    ) -> AwbcFunctionId {
        let layout = self.intern_frame_layout(
            format!("{name}:empty"),
            AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            },
        );
        let block = self.push_block(AwbcBlock {
            owner: AwbcFunctionId(table_index(self.program.functions.len())),
            instructions: AwbcTableRange::new(table_index(self.program.instructions.len()), 0),
            terminator: AwbcTerminator::Return { value: None },
            safe_point,
            source_map: None,
        });
        let signature = self.intern_unit_signature();
        let public_id = Some(self.intern_string(name));
        self.push_function(AwbcFunction {
            public_id,
            kind,
            signature,
            frame_layout: layout,
            blocks: AwbcTableRange::new(block.0, 1),
            entry_block: block,
            flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
        })
    }
}

fn signed_constant(value: RuntimeInt) -> AwbcConstant {
    let (kind, bits) = match value {
        RuntimeInt::I8(value) => (AwbcSignedIntKind::I8, i128::from(value).to_le_bytes()),
        RuntimeInt::I16(value) => (AwbcSignedIntKind::I16, i128::from(value).to_le_bytes()),
        RuntimeInt::I32(value) => (AwbcSignedIntKind::I32, i128::from(value).to_le_bytes()),
        RuntimeInt::I64(value) => (AwbcSignedIntKind::I64, i128::from(value).to_le_bytes()),
        RuntimeInt::I128(value) => (AwbcSignedIntKind::I128, value.to_le_bytes()),
        RuntimeInt::ISize(value) => (AwbcSignedIntKind::ISize, i128::from(value).to_le_bytes()),
    };
    AwbcConstant::Int { kind, bits }
}

fn unsigned_constant(value: RuntimeUInt) -> AwbcConstant {
    let (kind, bits) = match value {
        RuntimeUInt::U8(value) => (AwbcUnsignedIntKind::U8, u128::from(value).to_le_bytes()),
        RuntimeUInt::U16(value) => (AwbcUnsignedIntKind::U16, u128::from(value).to_le_bytes()),
        RuntimeUInt::U32(value) => (AwbcUnsignedIntKind::U32, u128::from(value).to_le_bytes()),
        RuntimeUInt::U64(value) => (AwbcUnsignedIntKind::U64, u128::from(value).to_le_bytes()),
        RuntimeUInt::U128(value) => (AwbcUnsignedIntKind::U128, value.to_le_bytes()),
        RuntimeUInt::USize(value) => (AwbcUnsignedIntKind::USize, u128::from(value).to_le_bytes()),
    };
    AwbcConstant::UInt { kind, bits }
}

fn effect_kind(effect: &LineEffectRequest) -> AwbcEffectKind {
    match effect {
        LineEffectRequest::RegisterHandle { .. } => AwbcEffectKind::RegisterHandle,
        LineEffectRequest::DropHandle { .. } => AwbcEffectKind::DropHandle,
        LineEffectRequest::Wait(_) => AwbcEffectKind::Wait,
        LineEffectRequest::Audio(_) => AwbcEffectKind::Audio,
        LineEffectRequest::Call(_) => AwbcEffectKind::Call,
        LineEffectRequest::Log(_) => AwbcEffectKind::Log,
        LineEffectRequest::SignalWrite(_) => AwbcEffectKind::SignalWrite,
        LineEffectRequest::MetricWrite(_) => AwbcEffectKind::MetricWrite,
        LineEffectRequest::EmitEvent(_) => AwbcEffectKind::EmitEvent,
        LineEffectRequest::Out(_) => AwbcEffectKind::Out,
        LineEffectRequest::Return(_) => AwbcEffectKind::Return,
        LineEffectRequest::Goto(_) => AwbcEffectKind::Goto,
        LineEffectRequest::Panic(_) => AwbcEffectKind::Panic,
        LineEffectRequest::Fail(_) => AwbcEffectKind::Fail,
        LineEffectRequest::Bail(_) => AwbcEffectKind::Bail,
        LineEffectRequest::Ensure { .. } => AwbcEffectKind::Ensure,
        LineEffectRequest::Assert(_) => AwbcEffectKind::Assert,
        LineEffectRequest::Close(_) => AwbcEffectKind::Close,
        LineEffectRequest::Select(_) => AwbcEffectKind::Select,
        LineEffectRequest::Break { .. } => AwbcEffectKind::Break,
        LineEffectRequest::Continue { .. } => AwbcEffectKind::Continue,
    }
}

fn effect_capability(effect: &LineEffectRequest) -> Option<&'static str> {
    match effect {
        LineEffectRequest::Audio(_) => Some("audio"),
        LineEffectRequest::Call(_) => Some("host.call"),
        LineEffectRequest::SignalWrite(_) => Some("signal.write"),
        LineEffectRequest::MetricWrite(_) => Some("metric.write"),
        LineEffectRequest::EmitEvent(_) => Some("event.emit"),
        _ => None,
    }
}

fn effect_static_args(
    inventory: &mut AwbcInventory,
    effect: &LineEffectRequest,
) -> Vec<AwbcConstantId> {
    match effect {
        LineEffectRequest::RegisterHandle { key, handle } => {
            vec![
                inventory.constant_string(key),
                inventory.constant_string(handle),
            ]
        }
        LineEffectRequest::DropHandle { key }
        | LineEffectRequest::Return(key)
        | LineEffectRequest::Goto(key)
        | LineEffectRequest::Panic(key)
        | LineEffectRequest::Fail(key)
        | LineEffectRequest::Bail(key)
        | LineEffectRequest::Close(key)
        | LineEffectRequest::Select(key) => vec![inventory.constant_string(key)],
        LineEffectRequest::Wait(
            RuntimeWaitTarget::Mark(value) | RuntimeWaitTarget::Expr(value),
        ) => {
            vec![inventory.constant_string(value)]
        }
        LineEffectRequest::Wait(RuntimeWaitTarget::Duration(duration)) => {
            vec![inventory.constant_runtime_value(&RuntimeValue::Duration(*duration))]
        }
        LineEffectRequest::Call(call) => std::iter::once(inventory.constant_string(&call.callee))
            .chain(call.args.iter().map(|arg| inventory.constant_string(arg)))
            .collect(),
        LineEffectRequest::Log(log) => std::iter::once(inventory.constant_string(&log.level))
            .chain(std::iter::once(inventory.constant_string(&log.message)))
            .chain(log.fields.iter().flat_map(|field| {
                [
                    inventory.constant_string(&field.name),
                    inventory.constant_string(&field.value),
                ]
            }))
            .collect(),
        LineEffectRequest::SignalWrite(write) | LineEffectRequest::MetricWrite(write) => {
            vec![
                inventory.constant_string(&write.target),
                inventory.constant_string(&write.value),
            ]
        }
        LineEffectRequest::EmitEvent(event) => {
            std::iter::once(inventory.constant_string(&event.event))
                .chain(event.fields.iter().flat_map(|field| {
                    [
                        inventory.constant_string(&field.name),
                        inventory.constant_string(&field.value),
                    ]
                }))
                .collect()
        }
        LineEffectRequest::Out(out) => vec![
            optional_string_constant(inventory, out.label.as_deref()),
            inventory.constant_string(&out.value),
        ],
        LineEffectRequest::Ensure { condition, message } => {
            vec![
                inventory.constant_string(condition),
                inventory.constant_string(message),
            ]
        }
        LineEffectRequest::Assert(assertion) => {
            vec![
                inventory.constant_bytes(assertion.guard().as_bytes()),
                inventory.constant_string(assertion.condition()),
                inventory.constant_string(assertion.message()),
                inventory.constant_string(match assertion.profile() {
                    arcweft_core::effect::RuntimeAssertionProfile::Always => "always",
                    arcweft_core::effect::RuntimeAssertionProfile::DebugOnly => "debug_only",
                }),
            ]
        }
        LineEffectRequest::Break { label, value } => vec![
            optional_string_constant(inventory, label.as_deref()),
            optional_string_constant(inventory, value.as_deref()),
        ],
        LineEffectRequest::Continue { label } => {
            vec![optional_string_constant(inventory, label.as_deref())]
        }
        LineEffectRequest::Audio(_) => Vec::new(),
    }
}

fn optional_string_constant(inventory: &mut AwbcInventory, value: Option<&str>) -> AwbcConstantId {
    match value {
        Some(value) => inventory.constant_string(value),
        None => inventory.constant_unit(),
    }
}

fn line_cleanup(cleanup: &LineCleanupPolicy) -> AwbcLineCleanupPolicy {
    AwbcLineCleanupPolicy {
        child_tasks: match cleanup.child_tasks {
            ChildTaskCleanup::CancelAndJoin => AwbcChildCleanup::CancelAndJoin,
            ChildTaskCleanup::Detach => AwbcChildCleanup::Detach,
            ChildTaskCleanup::Finish => AwbcChildCleanup::Finish,
        },
        presentation: match cleanup.presentation {
            PresentationCleanup::DropRegistered => AwbcPresentationCleanup::DropRegistered,
            PresentationCleanup::KeepRegistered => AwbcPresentationCleanup::KeepRegistered,
        },
        audio: match cleanup.audio {
            AudioCleanup::StopRegistered => AwbcAudioCleanup::StopRegistered,
            AudioCleanup::FadeRegistered => AwbcAudioCleanup::FadeRegistered,
            AudioCleanup::KeepRegistered => AwbcAudioCleanup::KeepRegistered,
        },
    }
}

pub(crate) fn source_policy(policy: &SourcePolicy) -> AwbcSourcePolicy {
    AwbcSourcePolicy {
        backpressure: match &policy.backpressure {
            BackpressurePolicy::LatestOnly => AwbcBackpressurePolicy::LatestOnly,
            BackpressurePolicy::BlockingNotAllowed => AwbcBackpressurePolicy::BlockingNotAllowed,
            BackpressurePolicy::BoundedQueue {
                capacity,
                on_overflow,
            } => AwbcBackpressurePolicy::BoundedQueue {
                capacity: table_index(*capacity),
                overflow: match on_overflow {
                    OverflowPolicy::DropOldest => AwbcOverflowPolicy::DropOldest,
                    OverflowPolicy::DropNewest => AwbcOverflowPolicy::DropNewest,
                    OverflowPolicy::Error => AwbcOverflowPolicy::Error,
                    OverflowPolicy::Coalesce => AwbcOverflowPolicy::Coalesce,
                },
            },
        },
        replay: match policy.replay {
            ReplayPolicy::Full => AwbcReplayPolicy::Full,
            ReplayPolicy::HashOnly => AwbcReplayPolicy::HashOnly,
            ReplayPolicy::Summary => AwbcReplayPolicy::Summary,
            ReplayPolicy::EventOnly => AwbcReplayPolicy::EventOnly,
            ReplayPolicy::None => AwbcReplayPolicy::None,
        },
        privacy: match policy.privacy {
            PrivacyPolicy::Transient => AwbcPrivacyPolicy::Transient,
            PrivacyPolicy::Redacted => AwbcPrivacyPolicy::Redacted,
            PrivacyPolicy::Recordable => AwbcPrivacyPolicy::Recordable,
            PrivacyPolicy::Private => AwbcPrivacyPolicy::Private,
        },
        max_queue: table_index(policy.max_queue),
    }
}

pub(crate) fn source_handler_kind(handler: &SourceHandlerPlan) -> AwbcSourceEventKind {
    match handler {
        SourceHandlerPlan::Item { .. } => AwbcSourceEventKind::Item,
        SourceHandlerPlan::Error { .. } => AwbcSourceEventKind::Error,
        SourceHandlerPlan::Progress { .. } => AwbcSourceEventKind::Progress,
        SourceHandlerPlan::Disconnected { .. } => AwbcSourceEventKind::Disconnected,
        SourceHandlerPlan::PermissionRevoked { .. } => AwbcSourceEventKind::PermissionRevoked,
        SourceHandlerPlan::End { .. } => AwbcSourceEventKind::End,
    }
}
