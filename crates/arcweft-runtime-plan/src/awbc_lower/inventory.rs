use crate::awbc_lower::{AwbcLowerOptions, table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcAudioCleanup, AwbcBackpressurePolicy, AwbcBlock, AwbcBlockId, AwbcChildCancelPolicy,
    AwbcChildCleanup, AwbcChildJoinPolicy, AwbcChoice, AwbcChoiceId, AwbcChoiceOption,
    AwbcConstant, AwbcConstantId, AwbcContentUnit, AwbcContentUnitId, AwbcDisplayMapEntry,
    AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId, AwbcEffectSet, AwbcEffectSetId, AwbcEntry,
    AwbcEntryKind, AwbcEntryTarget, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction,
    AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcInstruction, AwbcInstructionId,
    AwbcLineCancelHandler, AwbcLineCleanupPolicy, AwbcLineOption, AwbcLineTaskGroup,
    AwbcLineTaskGroupId, AwbcLineTaskNode, AwbcLineTaskNodeId, AwbcLineTaskTrigger,
    AwbcOverflowPolicy, AwbcParallelPolicy, AwbcPattern, AwbcPatternId, AwbcPresentationCleanup,
    AwbcPrivacyPolicy, AwbcProgram, AwbcRegisterId, AwbcReplayPolicy, AwbcResumePoint,
    AwbcResumePointId, AwbcRoute, AwbcRouteBinding, AwbcRouteBindingSource, AwbcRuntimeType,
    AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcSignedIntKind, AwbcSourceEventKind,
    AwbcSourcePolicy, AwbcStringId, AwbcTableRange, AwbcTaskArgument, AwbcTaskClass, AwbcTaskPlan,
    AwbcTaskPlanId, AwbcTaskPolicy, AwbcTerminator, AwbcTypeId, AwbcUnsignedIntKind,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeWaitTarget};
use arcweft_core::line_task::{
    AudioCleanup, ChildCancelPolicy, ChildJoinPolicy, ChildTaskCleanup, LineChildTask,
    LineCleanupPolicy, LineTaskGroup, LineTaskNode, LineTaskScope, ParallelPolicy,
    PresentationCleanup,
};
use arcweft_core::plan::{RuntimeEntryKind, RuntimeEntryTarget, RuntimePlan};
use arcweft_core::source::{
    BackpressurePolicy, OverflowPolicy, PrivacyPolicy, ReplayPolicy, SourceHandlerPlan,
    SourcePolicy,
};
use arcweft_core::stream::StreamOp;
use arcweft_core::task::{HostTaskArgTemplate, HostTaskRequestTemplate};
use arcweft_core::value::{RuntimeInt, RuntimeUInt, RuntimeValue};
use arcweft_render_text::LineDisplayCatalog;
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
    pub task_plans: usize,
    pub source_plans: usize,
    pub stream_plans: usize,
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
            task_plans: program.task_plans.len(),
            source_plans: program.source_plans.len(),
            stream_plans: program.stream_plans.len(),
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
    tasks: BTreeMap<String, AwbcTaskPlanId>,
    choices: BTreeMap<String, AwbcChoiceId>,
    entry_functions: BTreeMap<String, AwbcFunctionId>,
}

#[derive(Clone, Copy, Debug)]
struct NamedTaskSpec<'a> {
    public_id: &'a str,
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
            tasks: BTreeMap::new(),
            choices: BTreeMap::new(),
            entry_functions: BTreeMap::new(),
        };
        this.intern_string(source_label);
        this
    }

    pub fn finish(self) -> AwbcProgram {
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

    pub fn intern_display_catalog(&mut self, display: &LineDisplayCatalog) {
        if !self.options.emit_display_map {
            return;
        }
        for spec in display.lines() {
            let key = self.intern_string(spec.line.0.as_str());
            let content = self.intern_content_unit(spec.line.0.as_str(), None);
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
                fields: fields
                    .iter()
                    .map(|field| self.constant_runtime_value(&field.value))
                    .collect(),
            },
            RuntimeValue::Variant { name, payload, .. } => AwbcConstant::Variant {
                ty: self.dynamic_ty(),
                case: stable_ordinal(name),
                payload: payload
                    .as_deref()
                    .map(|payload| self.constant_runtime_value(payload)),
            },
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

    pub fn push_function(
        &mut self,
        public_id: Option<&str>,
        function: AwbcFunction,
    ) -> AwbcFunctionId {
        let id = AwbcFunctionId(table_index(self.program.functions.len()));
        if let Some(name) = public_id {
            self.entry_functions.insert(name.to_owned(), id);
        }
        self.program.functions.push(function);
        id
    }

    pub fn function_by_name(&self, name: &str) -> Option<AwbcFunctionId> {
        self.entry_functions.get(name).copied()
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
                capability: "line_task",
                operation: "run_child",
                args: &[],
                class: AwbcTaskClass::LocalUi,
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
        let id = AwbcEffectPlanId(table_index(self.program.effect_plans.len()));
        let kind = effect_kind(effect);
        let signature = self.intern_unit_signature();
        let capability = effect_capability(effect).map(|capability| self.intern_string(capability));
        let static_args = effect_static_args(self, effect);
        self.program.effect_plans.push(AwbcEffectPlan {
            kind,
            signature,
            capability,
            static_args,
            resources: Vec::new(),
        });
        self.effects.insert(key, id);
        id
    }

    pub fn intern_host_task(
        &mut self,
        label: &str,
        request: &HostTaskRequestTemplate,
    ) -> AwbcTaskPlanId {
        self.intern_named_task(NamedTaskSpec {
            public_id: label,
            capability: &request.capability.0,
            operation: &request.operation,
            args: &request.args,
            class: AwbcTaskClass::Io,
            priority: 0,
            cancel_scope: "flow",
            policy: AwbcTaskPolicy::JoinSameKey,
        })
    }

    fn intern_named_task(&mut self, spec: NamedTaskSpec<'_>) -> AwbcTaskPlanId {
        let NamedTaskSpec {
            public_id,
            capability,
            operation,
            args,
            class,
            priority,
            cancel_scope,
            policy,
        } = spec;
        let key = format!(
            "task:{public_id}:{capability}:{operation}:{args:?}:{class:?}:{priority}:{cancel_scope}:{policy:?}"
        );
        if let Some(id) = self.tasks.get(&key).copied() {
            return id;
        }
        let id = AwbcTaskPlanId(table_index(self.program.task_plans.len()));
        let public_id = self.intern_string(public_id);
        let capability = self.intern_string(capability);
        let operation = self.intern_string(operation);
        let signature = self.intern_unit_signature();
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
        let signature = self.intern_unit_signature();
        for entry in &plan.entries {
            let public_id = self.intern_string(&entry.id.0);
            let kind = match &entry.kind {
                RuntimeEntryKind::Game => AwbcEntryKind::Game,
                RuntimeEntryKind::Cli => AwbcEntryKind::Cli,
                RuntimeEntryKind::Server => AwbcEntryKind::Server,
                RuntimeEntryKind::Activity => AwbcEntryKind::Activity,
                RuntimeEntryKind::Test => AwbcEntryKind::Test,
                RuntimeEntryKind::Bench => AwbcEntryKind::Bench,
                RuntimeEntryKind::Custom(value) => AwbcEntryKind::Custom(self.intern_string(value)),
            };
            let target = match &entry.target {
                RuntimeEntryTarget::Flow(flow) => self.function_by_name(&flow.0).map_or_else(
                    || {
                        self.diagnostic(AwbcLowerDiagnostic::error(
                            format!("entry.{}", entry.id.0),
                            format!("entry targets missing flow {}", flow.0),
                        ));
                        AwbcEntryTarget::Function(AwbcFunctionId(0))
                    },
                    AwbcEntryTarget::Function,
                ),
                RuntimeEntryTarget::Routes(routes) => AwbcEntryTarget::Routes(
                    routes
                        .iter()
                        .map(|route| AwbcRoute {
                            method: self.intern_string(&route.method),
                            path: self.intern_string(&route.path),
                            target: self.function_by_name(&route.target.0).unwrap_or(AwbcFunctionId(0)),
                            bindings: route
                                .bindings
                                .iter()
                                .enumerate()
                                .map(|(index, binding)| AwbcRouteBinding {
                                    register: AwbcRegisterId(table_index(index)),
                                    source: match &binding.source {
                                        arcweft_core::plan::RuntimeRouteBindingSource::PathParam(value) => {
                                            AwbcRouteBindingSource::PathParameter(self.intern_string(value))
                                        }
                                    },
                                })
                                .collect(),
                        })
                        .collect(),
                ),
            };
            self.program.entries.push(AwbcEntry {
                public_id,
                kind,
                signature,
                target,
            });
        }
        if self.program.entries.is_empty()
            && let Some(entry_flow) = plan.entry_flow.as_ref()
            && let Some(function) = self.function_by_name(&entry_flow.0)
        {
            let public_id = self.intern_string("entry.main");
            self.program.entries.push(AwbcEntry {
                public_id,
                kind: AwbcEntryKind::Game,
                signature,
                target: AwbcEntryTarget::Function(function),
            });
        }
    }

    pub fn synthetic_empty_function(&mut self, name: &str) -> AwbcFunctionId {
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
            safe_point: AwbcSafePointKind::Return,
            source_map: None,
        });
        let signature = self.intern_unit_signature();
        let public_id = Some(self.intern_string(name));
        self.push_function(
            Some(name),
            AwbcFunction {
                public_id,
                kind: AwbcFunctionKind::Synthetic,
                signature,
                frame_layout: layout,
                blocks: AwbcTableRange::new(block.0, 1),
                entry_block: block,
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            },
        )
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

fn stable_ordinal(value: &str) -> u32 {
    value.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(16_777_619).wrapping_add(u32::from(byte))
    })
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
                inventory.constant_string(&assertion.condition),
                inventory.constant_string(&assertion.message),
            ]
        }
        LineEffectRequest::Break { label, value } => vec![
            optional_string_constant(inventory, label.as_deref()),
            optional_string_constant(inventory, value.as_deref()),
        ],
        LineEffectRequest::Continue { label } => {
            vec![optional_string_constant(inventory, label.as_deref())]
        }
        LineEffectRequest::Audio(command) => {
            vec![inventory.constant_string(command.operation_name())]
        }
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

pub(crate) fn stream_op_family(op: &StreamOp) -> &'static str {
    match op {
        StreamOp::Let { .. } => "let",
        StreamOp::ForNext { .. } => "for_next",
        StreamOp::Yield { .. } => "yield",
        StreamOp::If { .. } => "if",
        StreamOp::Match { .. } => "match",
        StreamOp::Close { .. } => "close",
        StreamOp::Return => "return",
        StreamOp::Noop => "noop",
    }
}
