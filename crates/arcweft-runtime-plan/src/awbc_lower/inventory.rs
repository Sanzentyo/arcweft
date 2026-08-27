use crate::awbc_lower::audio::constant_audio_command;
use crate::awbc_lower::pattern::intern_runtime_type;
use crate::awbc_lower::{AwbcLowerOptions, table_index, table_range_len};
use arcweft_core::awbc::schema::{
    AwbcAgentTypeShape, AwbcAudioCleanup, AwbcAudioCommand, AwbcAudioCommandId,
    AwbcAwaitManyPolicy, AwbcBlock, AwbcBlockId, AwbcCallableExecutable, AwbcChildCleanup,
    AwbcChoice, AwbcChoiceId, AwbcChoiceOption, AwbcConstant, AwbcConstantId, AwbcContentUnit,
    AwbcContentUnitId, AwbcDisplayMapEntry, AwbcEffectKind, AwbcEffectPlan, AwbcEffectPlanId,
    AwbcEffectSet, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget, AwbcFlowBinding,
    AwbcFlowExecutable, AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags,
    AwbcFunctionId, AwbcFunctionKind, AwbcHostArgument, AwbcHostCall, AwbcHostCallId,
    AwbcHostCallMode, AwbcInstruction, AwbcInstructionId, AwbcLineCleanupPolicy,
    AwbcLineTaskGroupId, AwbcPattern, AwbcPatternId, AwbcPresentationCleanup, AwbcProgram,
    AwbcPureHelperId, AwbcPureProgramBinding, AwbcRegisterId, AwbcResumePoint, AwbcResumePointId,
    AwbcRoute, AwbcRouteBinding, AwbcRouteBindingSource, AwbcRouteSegment, AwbcRuntimeType,
    AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcSignedIntKind,
    AwbcStreamPlan, AwbcStreamPlanId, AwbcStringId, AwbcStructuralRuntimeTypeKind,
    AwbcSyntheticRuntimeTypeKind, AwbcTableRange, AwbcTaskClass, AwbcTaskPlan, AwbcTaskPlanId,
    AwbcTaskPolicy, AwbcTerminator, AwbcTraitMethodId, AwbcTypeId, AwbcUnsignedIntKind,
    AwbcVariantIdentity,
};
use arcweft_core::effect::{LineEffectRequest, RuntimeEffectExpr, RuntimeWaitTarget};
use arcweft_core::entry::{RuntimeCallableExecutableCode, RuntimeCallableRole, RuntimeEntryRoles};
use arcweft_core::line_task::{
    AudioCleanup, ChildTaskCleanup, LineCleanupPolicy, PresentationCleanup,
};
use arcweft_core::pattern::{RuntimeCheckedType, RuntimeSemanticTypeId, RuntimeVariantIdentity};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimeHostCallTarget,
    RuntimePlan, RuntimeTraitMethodId,
};
use arcweft_core::runtime_id::{
    RuntimeFunctionSiteId, RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::stream::StreamRuntimeId;
use arcweft_core::task::{
    HostTaskRequestTemplate, RuntimeHostArgumentTemplate, TaskOutcomeContract,
};
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
    pub line_task_groups: usize,
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
            line_task_groups: program.line_task_groups.len(),
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
    semantic_types: BTreeMap<RuntimeSemanticTypeId, AwbcTypeId>,
    plan_types: BTreeMap<RuntimePlanTypeId, AwbcTypeId>,
    reserved_types: BTreeMap<AwbcTypeId, RuntimeSemanticTypeId>,
    pending_types: BTreeMap<AwbcTypeId, AwbcRuntimeType>,
    unit_type: AwbcTypeId,
    dynamic_type: AwbcTypeId,
    constants: BTreeMap<String, AwbcConstantId>,
    signatures: BTreeMap<String, AwbcSignatureId>,
    frame_layouts: BTreeMap<String, AwbcFrameLayoutId>,
    effects: BTreeMap<String, AwbcEffectPlanId>,
    audio_commands: BTreeMap<String, AwbcAudioCommandId>,
    tasks: BTreeMap<String, AwbcTaskPlanId>,
    host_calls: BTreeMap<String, AwbcHostCallId>,
    streams: BTreeMap<StreamRuntimeId, AwbcStreamPlanId>,
    choices: BTreeMap<String, AwbcChoiceId>,
    flow_functions: BTreeMap<FlowRuntimeId, AwbcFunctionId>,
    trait_methods: BTreeMap<RuntimeTraitMethodId, AwbcTraitMethodId>,
    function_sites: BTreeMap<RuntimeFunctionSiteId, AwbcFunctionId>,
    pending_closures: Vec<PendingAwbcClosure>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingAwbcClosure {
    pub function: AwbcFunctionId,
    pub params: Box<[RuntimeLocalDeclarationId]>,
    pub captures: Box<[RuntimeLocalDeclarationId]>,
    pub body: RuntimeExpr,
    pub path: String,
}

#[derive(Clone, Copy, Debug)]
struct NamedTaskSpec<'a> {
    public_id: &'a str,
    need_id: &'a str,
    capability: &'a str,
    operation: &'a str,
    args: &'a [RuntimeHostArgumentTemplate],
    class: AwbcTaskClass,
    priority: i32,
    cancel_scope: &'a str,
    policy: AwbcTaskPolicy,
    outcome: &'a TaskOutcomeContract,
}

impl AwbcInventory {
    pub fn new(source_label: &str, options: AwbcLowerOptions) -> Self {
        let program = AwbcProgram::default();
        let semantic_types = program
            .runtime_types
            .iter()
            .enumerate()
            .map(|(index, ty)| (ty.semantic_identity(), AwbcTypeId(table_index(index))))
            .collect::<BTreeMap<_, _>>();
        let unit_type = *semantic_types
            .get(&RuntimeCheckedType::Unit.semantic_identity_digest())
            .expect("default AWBC inventory owns its canonical Unit row");
        let dynamic_type = *semantic_types
            .get(&AwbcSyntheticRuntimeTypeKind::Dynamic.semantic_identity())
            .expect("default AWBC inventory owns its canonical Dynamic row");
        let mut this = Self {
            program,
            diagnostics: Vec::new(),
            options,
            strings: BTreeMap::new(),
            semantic_types,
            plan_types: BTreeMap::new(),
            reserved_types: BTreeMap::new(),
            pending_types: BTreeMap::new(),
            unit_type,
            dynamic_type,
            constants: BTreeMap::new(),
            signatures: BTreeMap::new(),
            frame_layouts: BTreeMap::new(),
            effects: BTreeMap::new(),
            audio_commands: BTreeMap::new(),
            tasks: BTreeMap::new(),
            host_calls: BTreeMap::new(),
            streams: BTreeMap::new(),
            choices: BTreeMap::new(),
            flow_functions: BTreeMap::new(),
            trait_methods: BTreeMap::new(),
            function_sites: BTreeMap::new(),
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

    pub fn lower_pure_program_bindings(&mut self, plan: &RuntimePlan) {
        for binding in plan.pure_programs() {
            let Ok(helper) = u32::try_from(binding.helper().0) else {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    binding.program().to_string(),
                    "pure-program helper index exceeds Product AWBC limits",
                ));
                continue;
            };
            let helper = AwbcPureHelperId(helper);
            if self.program.pure_helpers.get(helper.index()).is_none() {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    binding.program().to_string(),
                    "pure-program binding references a missing Product AWBC helper",
                ));
                continue;
            }
            self.program.pure_programs.push(AwbcPureProgramBinding {
                program: binding.program(),
                helper,
                input_types: binding.input_types().to_vec(),
                result_type: binding.result_type(),
            });
        }
    }

    pub fn take_diagnostics(&mut self) -> Vec<AwbcLowerDiagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    pub fn diagnostic(&mut self, diagnostic: AwbcLowerDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn intern_runtime_primitives(&mut self) {
        let _ = self.unit_ty();
        let _ = self.dynamic_ty();
        self.intern_type(AwbcRuntimeTypeShape::Bool);
        self.intern_type(AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64));
        self.intern_type(AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U64));
        self.intern_type(AwbcRuntimeTypeShape::F32);
        self.intern_type(AwbcRuntimeTypeShape::F64);
        self.intern_type(AwbcRuntimeTypeShape::String);
        self.intern_type(AwbcRuntimeTypeShape::EntityRef);
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
            .map(String::as_str)
            .expect("AWBC string IDs are issued by this inventory")
    }

    pub fn dynamic_ty(&self) -> AwbcTypeId {
        self.dynamic_type
    }

    pub fn unit_ty(&self) -> AwbcTypeId {
        self.unit_type
    }

    pub fn bool_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeTypeShape::Bool)
    }

    pub fn i64_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeTypeShape::Int(AwbcSignedIntKind::I64))
    }

    pub fn string_ty(&mut self) -> AwbcTypeId {
        self.intern_type(AwbcRuntimeTypeShape::String)
    }

    pub fn intern_type(&mut self, shape: AwbcRuntimeTypeShape) -> AwbcTypeId {
        let semantic_identity = self
            .canonical_non_plan_type_identity(&shape)
            .expect("plan-backed AWBC shapes must use intern_semantic_type");
        self.intern_semantic_type(semantic_identity, shape)
            .expect("canonical non-plan AWBC types cannot conflict")
    }

    pub(crate) fn intern_semantic_type(
        &mut self,
        semantic_identity: RuntimeSemanticTypeId,
        shape: AwbcRuntimeTypeShape,
    ) -> Result<AwbcTypeId, AwbcLowerDiagnostic> {
        if let Some(id) = self.semantic_types.get(&semantic_identity).copied() {
            if self
                .program
                .runtime_types
                .get(id.index())
                .is_some_and(|row| row.shape() == &shape)
            {
                return Ok(id);
            }
            return Err(AwbcLowerDiagnostic::error(
                format!("type.{semantic_identity:?}"),
                "one semantic type identity projects to conflicting AWBC shapes",
            ));
        }
        let id = AwbcTypeId(table_index(self.program.runtime_types.len()));
        self.program
            .runtime_types
            .push(AwbcRuntimeType::new(semantic_identity, shape));
        self.semantic_types.insert(semantic_identity, id);
        Ok(id)
    }

    pub(crate) fn plan_type(&self, plan_type: RuntimePlanTypeId) -> Option<AwbcTypeId> {
        self.plan_types.get(&plan_type).copied()
    }

    pub(crate) fn reserve_plan_type(
        &mut self,
        plan_type: RuntimePlanTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Result<AwbcTypeId, AwbcLowerDiagnostic> {
        if self.plan_types.contains_key(&plan_type) {
            return Err(AwbcLowerDiagnostic::error(
                format!("type.{plan_type}"),
                "RuntimePlan type was reserved more than once",
            ));
        }
        let awbc_type = if let Some(existing) = self.semantic_types.get(&semantic_identity) {
            *existing
        } else {
            let index = self
                .program
                .runtime_types
                .len()
                .checked_add(self.reserved_types.len())
                .and_then(|index| u32::try_from(index).ok())
                .ok_or_else(|| {
                    AwbcLowerDiagnostic::error(
                        format!("type.{plan_type}"),
                        "AWBC runtime type identity space is exhausted",
                    )
                })?;
            let id = AwbcTypeId(index);
            self.semantic_types.insert(semantic_identity, id);
            self.reserved_types.insert(id, semantic_identity);
            id
        };
        self.plan_types.insert(plan_type, awbc_type);
        Ok(awbc_type)
    }

    pub(crate) fn define_plan_type(
        &mut self,
        plan_type: RuntimePlanTypeId,
        shape: AwbcRuntimeTypeShape,
    ) -> Result<(), AwbcLowerDiagnostic> {
        let awbc_type = self.plan_type(plan_type).ok_or_else(|| {
            AwbcLowerDiagnostic::error(
                format!("type.{plan_type}"),
                "RuntimePlan type has no reserved AWBC identity",
            )
        })?;
        if let Some(existing) = self.program.runtime_types.get(awbc_type.index()) {
            return (existing.shape() == &shape).then_some(()).ok_or_else(|| {
                AwbcLowerDiagnostic::error(
                    format!("type.{plan_type}"),
                    "plan semantic identity conflicts with a canonical AWBC type shape",
                )
            });
        }
        let semantic_identity = self
            .reserved_types
            .get(&awbc_type)
            .copied()
            .ok_or_else(|| {
                AwbcLowerDiagnostic::error(
                    format!("type.{plan_type}"),
                    "reserved AWBC runtime type owner is absent",
                )
            })?;
        if self
            .pending_types
            .insert(awbc_type, AwbcRuntimeType::new(semantic_identity, shape))
            .is_some()
        {
            return Err(AwbcLowerDiagnostic::error(
                format!("type.{plan_type}"),
                "reserved AWBC runtime type was defined more than once",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_plan_types(&mut self) -> Result<(), AwbcLowerDiagnostic> {
        if self.pending_types.len() != self.reserved_types.len() {
            return Err(AwbcLowerDiagnostic::error(
                "runtime_types",
                "AWBC type preflight left one or more reserved rows undefined",
            ));
        }
        let expected_start = self.program.runtime_types.len();
        for (offset, (id, row)) in std::mem::take(&mut self.pending_types)
            .into_iter()
            .enumerate()
        {
            if id.index() != expected_start + offset {
                return Err(AwbcLowerDiagnostic::error(
                    "runtime_types",
                    "AWBC type preflight produced a non-contiguous row reservation",
                ));
            }
            self.program.runtime_types.push(row);
        }
        self.reserved_types.clear();
        Ok(())
    }

    fn canonical_non_plan_type_identity(
        &self,
        shape: &AwbcRuntimeTypeShape,
    ) -> Option<RuntimeSemanticTypeId> {
        let checked = match shape {
            AwbcRuntimeTypeShape::Unit => RuntimeCheckedType::Unit,
            AwbcRuntimeTypeShape::Bool => RuntimeCheckedType::Bool,
            AwbcRuntimeTypeShape::Int(kind) => RuntimeCheckedType::Signed(match kind {
                AwbcSignedIntKind::I8 => arcweft_core::value::RuntimeSignedIntWidth::I8,
                AwbcSignedIntKind::I16 => arcweft_core::value::RuntimeSignedIntWidth::I16,
                AwbcSignedIntKind::I32 => arcweft_core::value::RuntimeSignedIntWidth::I32,
                AwbcSignedIntKind::I64 => arcweft_core::value::RuntimeSignedIntWidth::I64,
                AwbcSignedIntKind::I128 => arcweft_core::value::RuntimeSignedIntWidth::I128,
                AwbcSignedIntKind::ISize => arcweft_core::value::RuntimeSignedIntWidth::ISize,
            }),
            AwbcRuntimeTypeShape::UInt(kind) => RuntimeCheckedType::Unsigned(match kind {
                AwbcUnsignedIntKind::U8 => arcweft_core::value::RuntimeUnsignedIntWidth::U8,
                AwbcUnsignedIntKind::U16 => arcweft_core::value::RuntimeUnsignedIntWidth::U16,
                AwbcUnsignedIntKind::U32 => arcweft_core::value::RuntimeUnsignedIntWidth::U32,
                AwbcUnsignedIntKind::U64 => arcweft_core::value::RuntimeUnsignedIntWidth::U64,
                AwbcUnsignedIntKind::U128 => arcweft_core::value::RuntimeUnsignedIntWidth::U128,
                AwbcUnsignedIntKind::USize => arcweft_core::value::RuntimeUnsignedIntWidth::USize,
            }),
            AwbcRuntimeTypeShape::F32 => RuntimeCheckedType::F32,
            AwbcRuntimeTypeShape::F64 => RuntimeCheckedType::F64,
            AwbcRuntimeTypeShape::String => RuntimeCheckedType::String,
            AwbcRuntimeTypeShape::Char => RuntimeCheckedType::Char,
            AwbcRuntimeTypeShape::Duration => RuntimeCheckedType::Duration,
            AwbcRuntimeTypeShape::Progress => RuntimeCheckedType::Progress,
            AwbcRuntimeTypeShape::EntityRef => RuntimeCheckedType::EntityReference,
            AwbcRuntimeTypeShape::AgentValue => RuntimeCheckedType::AgentValue,
            AwbcRuntimeTypeShape::Bytes => RuntimeCheckedType::Bytes,
            AwbcRuntimeTypeShape::Never => RuntimeCheckedType::Never,
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(agent)) => {
                RuntimeCheckedType::Agent(*agent)
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(_)) => return None,
            AwbcRuntimeTypeShape::Dynamic => {
                return Some(AwbcSyntheticRuntimeTypeKind::Dynamic.semantic_identity());
            }
            AwbcRuntimeTypeShape::MatrixF32 => {
                return Some(AwbcSyntheticRuntimeTypeKind::MatrixF32.semantic_identity());
            }
            AwbcRuntimeTypeShape::MatrixF64 => {
                return Some(AwbcSyntheticRuntimeTypeKind::MatrixF64.semantic_identity());
            }
            AwbcRuntimeTypeShape::TensorF32 => {
                return Some(AwbcSyntheticRuntimeTypeKind::TensorF32.semantic_identity());
            }
            AwbcRuntimeTypeShape::TensorF64 => {
                return Some(AwbcSyntheticRuntimeTypeKind::TensorF64.semantic_identity());
            }
            AwbcRuntimeTypeShape::Record { public_id, fields } => {
                let public_id = public_id.map(|id| self.string(id).to_owned());
                let fields = fields
                    .iter()
                    .map(|field| {
                        let semantic_identity = self
                            .program
                            .runtime_types
                            .get(field.ty.index())?
                            .semantic_identity();
                        Some((self.string(field.name).to_owned(), semantic_identity))
                    })
                    .collect::<Option<Vec<_>>>()?;
                return Some(
                    AwbcStructuralRuntimeTypeKind::Record { public_id, fields }.semantic_identity(),
                );
            }
            AwbcRuntimeTypeShape::Tuple(_)
            | AwbcRuntimeTypeShape::Sequence(_)
            | AwbcRuntimeTypeShape::Variant { .. }
            | AwbcRuntimeTypeShape::Choice(_)
            | AwbcRuntimeTypeShape::Nominal { .. }
            | AwbcRuntimeTypeShape::NominalRecord { .. }
            | AwbcRuntimeTypeShape::Opaque { .. }
            | AwbcRuntimeTypeShape::Range(_)
            | AwbcRuntimeTypeShape::Iterator(_)
            | AwbcRuntimeTypeShape::Array { .. }
            | AwbcRuntimeTypeShape::Map { .. }
            | AwbcRuntimeTypeShape::Need(_)
            | AwbcRuntimeTypeShape::Task(_)
            | AwbcRuntimeTypeShape::Stream { .. }
            | AwbcRuntimeTypeShape::Shared(_)
            | AwbcRuntimeTypeShape::Reference(_)
            | AwbcRuntimeTypeShape::Function { .. } => return None,
        };
        Some(checked.semantic_identity_digest())
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

    /// Interns one accepted runtime value under its exact plan-owned type row.
    /// Container and owner-bearing constants recursively retain that row's
    /// child coordinates instead of inferring a parallel type from the value.
    pub(crate) fn constant_runtime_value_typed(
        &mut self,
        value: &RuntimeValue,
        ty: AwbcTypeId,
    ) -> AwbcConstantId {
        let key = format!("runtime-typed:{}:{value:?}", ty.0);
        if let Some(id) = self.constants.get(&key).copied() {
            return id;
        }
        let constant = self.runtime_value_constant_typed(value, ty);
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

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive conversion owns the complete runtime-value constant vocabulary"
    )]
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
            RuntimeValue::EntityRef(value) => AwbcConstant::EntityRef(value.clone()),
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
                    .map(|field| self.intern_string(field.name()))
                    .collect(),
                fields: fields
                    .iter()
                    .map(|field| self.constant_runtime_value(field.value()))
                    .collect(),
            },
            RuntimeValue::NominalRecord(record) => {
                panic!(
                    "nominal runtime record `{}` cannot be encoded as an AWBC constant before \
                     its nominal schema is registered",
                    record.type_id().as_str()
                )
            }
            RuntimeValue::Opaque(opaque) => {
                assert_eq!(
                    opaque.persistence(),
                    arcweft_core::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                    "snapshot-only opaque handles cannot enter the AWBC constant pool"
                );
                assert_eq!(
                    opaque.value_class(),
                    arcweft_core::value::RuntimeOpaqueValueClass::Plain,
                    "affine opaque handles cannot enter the AWBC constant pool"
                );
                panic!(
                    "runtime opaque value `{}` requires an exact accepted type row",
                    opaque.producer().as_str()
                )
            }
            RuntimeValue::Reduction(_) => {
                panic!("runtime reduction state cannot be encoded as an AWBC constant")
            }
            RuntimeValue::Progress(_) => {
                panic!("runtime Progress publications cannot be encoded as AWBC constants")
            }
            RuntimeValue::Agent(value) => panic!(
                "typed Agent runtime value `{}` must be produced by AWBC MakeAgent and cannot be encoded as a constant",
                value.label()
            ),
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

    #[allow(
        clippy::too_many_lines,
        reason = "the typed constant projection exhaustively preserves container child coordinates"
    )]
    fn runtime_value_constant_typed(
        &mut self,
        value: &RuntimeValue,
        ty: AwbcTypeId,
    ) -> AwbcConstant {
        let row = self
            .program
            .runtime_types
            .get(ty.index())
            .cloned()
            .expect("typed constants use a committed AWBC runtime type row");
        match (value, row.shape()) {
            (RuntimeValue::Tuple(values), AwbcRuntimeTypeShape::Tuple(items)) => {
                assert_eq!(
                    values.len(),
                    items.len(),
                    "accepted tuple constant must match its exact AWBC type arity"
                );
                AwbcConstant::Tuple(
                    values
                        .iter()
                        .zip(items)
                        .map(|(value, item)| self.constant_runtime_value_typed(value, *item))
                        .collect(),
                )
            }
            (RuntimeValue::Seq(values), AwbcRuntimeTypeShape::Sequence(item)) => {
                AwbcConstant::Sequence(
                    values
                        .clone()
                        .into_values()
                        .iter()
                        .map(|value| self.constant_runtime_value_typed(value, *item))
                        .collect(),
                )
            }
            (RuntimeValue::Seq(values), AwbcRuntimeTypeShape::Array { item, length }) => {
                assert_eq!(
                    u64::try_from(values.len()).ok(),
                    Some(*length),
                    "accepted array constant must match its exact AWBC length"
                );
                AwbcConstant::Sequence(
                    values
                        .clone()
                        .into_values()
                        .iter()
                        .map(|value| self.constant_runtime_value_typed(value, *item))
                        .collect(),
                )
            }
            (RuntimeValue::Record(values), AwbcRuntimeTypeShape::Record { fields, .. }) => {
                assert_eq!(
                    values.len(),
                    fields.len(),
                    "accepted record constant must match its exact AWBC field count"
                );
                for (value, field) in values.iter().zip(fields) {
                    assert_eq!(
                        value.name(),
                        self.string(field.name),
                        "accepted record constant must match its exact AWBC field order"
                    );
                }
                AwbcConstant::Record {
                    ty,
                    field_names: fields.iter().map(|field| field.name).collect(),
                    fields: values
                        .iter()
                        .zip(fields)
                        .map(|(value, field)| {
                            self.constant_runtime_value_typed(value.value(), field.ty)
                        })
                        .collect(),
                }
            }
            (
                RuntimeValue::NominalRecord(value),
                AwbcRuntimeTypeShape::NominalRecord {
                    public_id,
                    layout,
                    fields,
                    ..
                },
            ) => {
                assert_eq!(
                    value.type_id().as_str(),
                    self.string(*public_id),
                    "accepted nominal record constant must retain its exact nominal owner"
                );
                assert_eq!(
                    value.layout().as_bytes(),
                    layout,
                    "accepted nominal record constant must retain its exact layout"
                );
                assert_eq!(
                    value.fields().len(),
                    fields.len(),
                    "accepted nominal record constant must match its exact field count"
                );
                AwbcConstant::Record {
                    ty,
                    field_names: fields.iter().map(|field| field.name).collect(),
                    fields: value
                        .fields()
                        .iter()
                        .zip(fields)
                        .map(|(value, field)| self.constant_runtime_value_typed(value, field.ty))
                        .collect(),
                }
            }
            (
                RuntimeValue::Variant {
                    owner,
                    ordinal,
                    name,
                    payload,
                },
                AwbcRuntimeTypeShape::Variant {
                    owner: expected_owner,
                    cases,
                    ..
                },
            ) => {
                let owner_matches = match (owner, expected_owner) {
                    (
                        RuntimeVariantIdentity::Nominal {
                            nominal,
                            semantic_identity,
                        },
                        AwbcVariantIdentity::Nominal { public_id },
                    ) => {
                        nominal.as_str() == self.string(*public_id)
                            && *semantic_identity == row.semantic_identity()
                    }
                    (
                        RuntimeVariantIdentity::Builtin(runtime),
                        AwbcVariantIdentity::Builtin(awbc),
                    ) => runtime == awbc,
                    _ => false,
                };
                assert!(
                    owner_matches,
                    "accepted variant constant must retain its exact AWBC owner"
                );
                let case = usize::try_from(*ordinal)
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .expect("accepted variant constant ordinal exists in its exact AWBC type");
                assert_eq!(
                    name,
                    self.string(case.name),
                    "accepted variant constant must retain its exact case name"
                );
                let payload = match (payload.as_deref(), case.payload) {
                    (Some(value), Some(payload_ty)) => {
                        Some(self.constant_runtime_value_typed(value, payload_ty))
                    }
                    (None, None) => None,
                    _ => panic!(
                        "accepted variant constant payload must match its exact AWBC case schema"
                    ),
                };
                AwbcConstant::Variant {
                    ty,
                    case: *ordinal,
                    case_name: case.name,
                    payload,
                }
            }
            (RuntimeValue::Opaque(value), AwbcRuntimeTypeShape::Opaque { arguments, .. }) => {
                let owner = row
                    .try_opaque_owner(&self.program.strings)
                    .expect("accepted AWBC opaque owner has a valid producer identity")
                    .expect("typed opaque constant references an opaque row");
                assert!(
                    owner.accepts_opaque_value(value),
                    "accepted opaque constant must retain its exact AWBC owner"
                );
                assert_eq!(
                    value.persistence(),
                    arcweft_core::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                    "snapshot-only opaque handles cannot enter the AWBC constant pool"
                );
                assert_eq!(
                    value.value_class(),
                    arcweft_core::value::RuntimeOpaqueValueClass::Plain,
                    "affine opaque handles cannot enter the AWBC constant pool"
                );
                let payload = match arguments.as_slice() {
                    [payload_ty] => self.constant_runtime_value_typed(value.payload(), *payload_ty),
                    [] => self.constant_runtime_value(value.payload()),
                    _ => panic!(
                        "opaque constant payload schema must have zero or one exact type argument"
                    ),
                };
                AwbcConstant::Opaque { ty, payload }
            }
            (value, AwbcRuntimeTypeShape::Choice(alternatives)) => {
                let mut accepted = alternatives.iter().copied().filter(|alternative| {
                    self.program
                        .checked_type(*alternative)
                        .is_ok_and(|checked| checked.accepts_value(value))
                });
                let selected = accepted
                    .next()
                    .expect("accepted choice constant matches one exact alternative");
                assert!(
                    accepted.next().is_none(),
                    "accepted choice constant must have one unambiguous exact alternative"
                );
                self.runtime_value_constant_typed(value, selected)
            }
            (RuntimeValue::Unit, AwbcRuntimeTypeShape::Unit)
            | (RuntimeValue::Bool(_), AwbcRuntimeTypeShape::Bool)
            | (RuntimeValue::F32(_), AwbcRuntimeTypeShape::F32)
            | (RuntimeValue::F64(_), AwbcRuntimeTypeShape::F64)
            | (RuntimeValue::String(_), AwbcRuntimeTypeShape::String)
            | (RuntimeValue::Char(_), AwbcRuntimeTypeShape::Char)
            | (RuntimeValue::Duration(_), AwbcRuntimeTypeShape::Duration)
            | (RuntimeValue::EntityRef(_), AwbcRuntimeTypeShape::EntityRef)
            | (RuntimeValue::MatrixF32(_), AwbcRuntimeTypeShape::MatrixF32)
            | (RuntimeValue::MatrixF64(_), AwbcRuntimeTypeShape::MatrixF64)
            | (RuntimeValue::TensorF32(_), AwbcRuntimeTypeShape::TensorF32)
            | (RuntimeValue::TensorF64(_), AwbcRuntimeTypeShape::TensorF64)
            | (RuntimeValue::Range(_), AwbcRuntimeTypeShape::Range(_)) => {
                self.runtime_value_constant(value)
            }
            (RuntimeValue::Int(value), AwbcRuntimeTypeShape::Int(kind))
                if signed_kind(*value) == *kind =>
            {
                signed_constant(*value)
            }
            (RuntimeValue::UInt(value), AwbcRuntimeTypeShape::UInt(kind))
                if unsigned_kind(*value) == *kind =>
            {
                unsigned_constant(*value)
            }
            (_, AwbcRuntimeTypeShape::Dynamic) => self.runtime_value_constant(value),
            _ => panic!(
                "accepted runtime constant does not match its exact AWBC type row {}",
                ty.0
            ),
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
            flags: AwbcFunctionFlags::empty(),
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

    pub(crate) fn record_trait_method(
        &mut self,
        method: RuntimeTraitMethodId,
        lowered: AwbcTraitMethodId,
    ) {
        self.trait_methods.insert(method, lowered);
    }

    pub(crate) fn trait_method(&self, method: RuntimeTraitMethodId) -> Option<AwbcTraitMethodId> {
        self.trait_methods.get(&method).copied()
    }

    pub fn function_site_function(&self, site: RuntimeFunctionSiteId) -> Option<AwbcFunctionId> {
        self.function_sites.get(&site).copied()
    }

    pub fn reserve_function_site_slot(&mut self, site: RuntimeFunctionSiteId) -> AwbcFunctionId {
        if let Some(function) = self.function_site_function(site) {
            return function;
        }
        let function = self.reserve_function_slot();
        self.function_sites.insert(site, function);
        function
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
    ) -> Result<(), AwbcLowerDiagnostic> {
        let limit = u32::try_from(limit).map_err(|_| {
            AwbcLowerDiagnostic::error(
                format!("task_plan.{}", plan.0),
                format!("AwaitMany limit {limit} exceeds the u32 AWBC domain"),
            )
        })?;
        if limit == 0 {
            return Err(AwbcLowerDiagnostic::error(
                format!("task_plan.{}", plan.0),
                "AwaitMany limit must be positive",
            ));
        }
        let task = self
            .program
            .task_plans
            .get_mut(plan.index())
            .ok_or_else(|| {
                AwbcLowerDiagnostic::error(
                    format!("task_plan.{}", plan.0),
                    "AwaitMany policy references an absent AWBC task plan",
                )
            })?;
        task.many = Some(AwbcAwaitManyPolicy {
            item_binding,
            limit,
        });
        Ok(())
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
            marks: Vec::new(),
            effect_site_count: 0,
            line_task_group: group,
            display: None,
            source: None,
            resources: Vec::new(),
        });
        id
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

    pub fn intern_evaluated_effect(
        &mut self,
        effect: &RuntimeEffectExpr,
    ) -> Option<AwbcEffectPlanId> {
        let descriptor = effect.host_descriptor()?;
        let arg_count = effect.argument_exprs().len();
        let key = format!("effect:evaluated:{descriptor:?}:{arg_count}");
        if let Some(id) = self.effects.get(&key).copied() {
            return Some(id);
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
        Some(id)
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
        self.intern_host_task_with_outcome(
            need_id,
            task_id,
            request,
            &TaskOutcomeContract::default(),
        )
    }

    pub fn intern_host_task_with_outcome(
        &mut self,
        need_id: &str,
        task_id: &str,
        request: &HostTaskRequestTemplate,
        outcome: &TaskOutcomeContract,
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
            outcome,
        })
    }

    pub fn intern_host_call(
        &mut self,
        target: &RuntimeHostCallTarget,
        plan: &RuntimePlan,
    ) -> Option<(AwbcHostCallId, AwbcTypeId)> {
        let checked_result = match plan.checked_type(target.result) {
            Ok(Some(result)) => result,
            Ok(None) => {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    "host_call.result",
                    format!(
                        "host call `{}` result is outside the checked AWBC type image",
                        target.public_id
                    ),
                ));
                return None;
            }
            Err(error) => {
                self.diagnostic(AwbcLowerDiagnostic::error(
                    "host_call.result",
                    error.to_string(),
                ));
                return None;
            }
        };
        let result_type = intern_runtime_type(self, &checked_result);
        let key = format!(
            "host_call:{}:{}:{}:{:?}:{:?}:{}:{}:{}",
            target.public_id,
            target.capability,
            target.operation,
            target.contract,
            target.args,
            result_type.0,
            target.mode as u8,
            target.deterministic
        );
        if let Some(id) = self.host_calls.get(&key).copied() {
            return Some((id, result_type));
        }
        let id = AwbcHostCallId(table_index(self.program.host_calls.len()));
        let signature = self.intern_signature(
            vec![self.dynamic_ty(); target.args.len()],
            Some(result_type),
            AwbcEffectSetId(0),
        );
        let mode = match target.mode {
            RuntimeHostCallMode::Immediate => AwbcHostCallMode::Immediate,
            RuntimeHostCallMode::Suspend => AwbcHostCallMode::Suspend,
        };
        let public_id = self.intern_string(&target.public_id);
        let capability = self.intern_string(&target.capability);
        let operation = self.intern_string(&target.operation);
        let arguments = self.intern_host_arguments(&target.args);
        self.program.host_calls.push(AwbcHostCall {
            public_id,
            capability,
            operation,
            contract: target.contract,
            signature,
            mode,
            deterministic: target.deterministic,
            arguments,
        });
        self.host_calls.insert(key, id);
        Some((id, result_type))
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
            outcome,
        } = spec;
        let key = format!(
            "task:{public_id}:{need_id}:{capability}:{operation}:{args:?}:{class:?}:{priority}:{cancel_scope}:{policy:?}:{outcome:?}"
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
            .map(|arg| self.intern_host_argument(arg))
            .collect();
        let payload_type = intern_runtime_type(self, outcome.payload());
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
            payload_type,
            arguments,
            many: None,
        });
        self.tasks.insert(key, id);
        id
    }

    fn intern_host_arguments(
        &mut self,
        arguments: &[RuntimeHostArgumentTemplate],
    ) -> Vec<AwbcHostArgument> {
        arguments
            .iter()
            .map(|argument| self.intern_host_argument(argument))
            .collect()
    }

    fn intern_host_argument(&mut self, argument: &RuntimeHostArgumentTemplate) -> AwbcHostArgument {
        AwbcHostArgument {
            name: argument.name().map(|name| self.intern_string(name)),
            spread: argument.is_spread(),
        }
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
        let entries = plan.entries().iter().collect::<Vec<_>>();
        self.lower_selected_entries(plan, &entries);
    }

    pub fn lower_selected_entries(&mut self, plan: &RuntimePlan, entries: &[&RuntimeEntrySpec]) {
        self.lower_callable_executables(plan, entries);
        self.lower_flow_executables(plan, entries);
        for entry in entries {
            self.lower_entry(entry);
        }
    }

    fn lower_callable_executables(&mut self, plan: &RuntimePlan, entries: &[&RuntimeEntrySpec]) {
        for executable in plan.callable_executables().iter().filter(|executable| {
            entries.iter().any(|entry| match &entry.roles {
                RuntimeEntryRoles::Stateful(roles) => {
                    (roles.initializer.callable == executable.callable
                        && roles.initializer.contract == executable.contract)
                        || (roles.reducer.callable == executable.callable
                            && roles.reducer.contract == executable.contract)
                }
                RuntimeEntryRoles::Agent(roles) => {
                    roles.controller.callable == executable.callable
                        && roles.controller.contract == executable.contract
                }
                RuntimeEntryRoles::None => false,
            })
        }) {
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

    fn lower_flow_executables(&mut self, plan: &RuntimePlan, entries: &[&RuntimeEntrySpec]) {
        for executable in plan.flow_executables().iter().filter(|executable| {
            entries
                .iter()
                .any(|entry| entry.references_flow(&executable.flow))
        }) {
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
        let target = match &entry.target {
            RuntimeEntryTarget::Flow(flow) | RuntimeEntryTarget::Controller(flow) => {
                let flow_public_id = flow.public_label().into_string();
                if let Some(function) = self.flow_function(flow) {
                    AwbcEntryTarget::Function { function }
                } else {
                    self.diagnostic(AwbcLowerDiagnostic::error(
                        entry_public_id.clone(),
                        format!("entry targets missing flow {flow_public_id}"),
                    ));
                    AwbcEntryTarget::Function {
                        function: AwbcFunctionId(0),
                    }
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
                        AwbcRoute {
                            method: route.method,
                            segments: route
                                .path
                                .segments()
                                .iter()
                                .map(|segment| match segment {
                                    arcweft_core::plan::RuntimeRoutePathSegment::Literal(
                                        literal,
                                    ) => AwbcRouteSegment::Literal(self.intern_string(literal)),
                                    arcweft_core::plan::RuntimeRoutePathSegment::Capture(
                                        coordinate,
                                    ) => AwbcRouteSegment::Capture(*coordinate),
                                })
                                .collect(),
                            target,
                            bindings: route
                                .bindings
                                .iter()
                                .map(|binding| {
                                    AwbcRouteBinding {
                                    parameter: binding.parameter,
                                    source: match &binding.source {
                                        arcweft_core::plan::RuntimeRouteBindingSource::PathCapture(
                                            capture,
                                        ) => AwbcRouteBindingSource::PathCapture(*capture),
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
            target,
            roles: entry.roles.clone(),
        });
    }

    pub fn synthetic_empty_function(&mut self, name: &str) -> AwbcFunctionId {
        self.empty_function(name, AwbcFunctionKind::Synthetic, AwbcSafePointKind::Return)
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
            flags: AwbcFunctionFlags::empty()
                .with(arcweft_core::awbc::schema::AwbcFunctionFlag::Deterministic),
        })
    }
}

fn signed_constant(value: RuntimeInt) -> AwbcConstant {
    let bits = match value {
        RuntimeInt::I8(value) => i128::from(value).to_le_bytes(),
        RuntimeInt::I16(value) => i128::from(value).to_le_bytes(),
        RuntimeInt::I32(value) => i128::from(value).to_le_bytes(),
        RuntimeInt::I64(value) => i128::from(value).to_le_bytes(),
        RuntimeInt::I128(value) => value.to_le_bytes(),
        RuntimeInt::ISize(value) => i128::from(value).to_le_bytes(),
    };
    AwbcConstant::Int {
        kind: signed_kind(value),
        bits,
    }
}

fn unsigned_constant(value: RuntimeUInt) -> AwbcConstant {
    let bits = match value {
        RuntimeUInt::U8(value) => u128::from(value).to_le_bytes(),
        RuntimeUInt::U16(value) => u128::from(value).to_le_bytes(),
        RuntimeUInt::U32(value) => u128::from(value).to_le_bytes(),
        RuntimeUInt::U64(value) => u128::from(value).to_le_bytes(),
        RuntimeUInt::U128(value) => value.to_le_bytes(),
        RuntimeUInt::USize(value) => u128::from(value).to_le_bytes(),
    };
    AwbcConstant::UInt {
        kind: unsigned_kind(value),
        bits,
    }
}

const fn signed_kind(value: RuntimeInt) -> AwbcSignedIntKind {
    match value {
        RuntimeInt::I8(_) => AwbcSignedIntKind::I8,
        RuntimeInt::I16(_) => AwbcSignedIntKind::I16,
        RuntimeInt::I32(_) => AwbcSignedIntKind::I32,
        RuntimeInt::I64(_) => AwbcSignedIntKind::I64,
        RuntimeInt::I128(_) => AwbcSignedIntKind::I128,
        RuntimeInt::ISize(_) => AwbcSignedIntKind::ISize,
    }
}

const fn unsigned_kind(value: RuntimeUInt) -> AwbcUnsignedIntKind {
    match value {
        RuntimeUInt::U8(_) => AwbcUnsignedIntKind::U8,
        RuntimeUInt::U16(_) => AwbcUnsignedIntKind::U16,
        RuntimeUInt::U32(_) => AwbcUnsignedIntKind::U32,
        RuntimeUInt::U64(_) => AwbcUnsignedIntKind::U64,
        RuntimeUInt::U128(_) => AwbcUnsignedIntKind::U128,
        RuntimeUInt::USize(_) => AwbcUnsignedIntKind::USize,
    }
}

fn effect_kind(effect: &LineEffectRequest) -> AwbcEffectKind {
    match effect {
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
        LineEffectRequest::Return(key)
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

pub(crate) fn line_cleanup(cleanup: &LineCleanupPolicy) -> AwbcLineCleanupPolicy {
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
