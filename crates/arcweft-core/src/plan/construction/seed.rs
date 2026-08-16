//! Transient, non-wire input algebra for aggregate runtime-plan construction.
//!
//! These values deliberately retain semantic type identities and
//! builder-issued local/site handles. They are consumed by
//! `RuntimePlanBuilder` and never become a parallel executable carrier.

use std::{fmt, sync::Arc};

use crate::effect::{LineEffectRequest, RuntimeAssertionGuardId, RuntimeAssertionProfile};
use crate::entry::{CallableContractHash, RuntimeCallableId, RuntimeCommandTargetId};
use crate::pattern::RuntimeSemanticTypeId;
use crate::runtime_id::{
    RuntimeDialogueContentPlanId, RuntimeDialogueMarkId, RuntimeDialogueValueSlotId,
    RuntimeFunctionSiteId, RuntimeLineTaskGroupId, RuntimeLocalDeclarationId, RuntimePlanTypeId,
};
use crate::step::RuntimeHostCallMode;
use crate::stream::StreamRuntimeId;
use crate::task::{HostCapabilityId, NamedHostArg, NeedId, TaskId};
use crate::value::{
    RuntimeAgentCompareOp, RuntimeAgentField, RuntimeBinaryOp, RuntimeCallArgumentMode,
    RuntimeCallTarget, RuntimeEntityReference, RuntimeEntityReferenceField, RuntimeUnaryOp,
    RuntimeValue,
};

use super::super::{
    FlowRuntimeId, RuntimeBuiltinIteratorEvidence, RuntimeDialogueValueRole, RuntimeLineId,
    RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType,
    RuntimeReceiverMode, RuntimeTraitMethodId, RuntimeTraitMethodIdentity,
};

#[derive(Debug)]
pub(super) struct RuntimePlanConstructionIssuer;

/// One local declaration request in canonical semantic-fact order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLocalDeclarationSeed {
    ty: RuntimeSemanticTypeId,
}

impl RuntimeLocalDeclarationSeed {
    #[must_use]
    pub const fn new(ty: RuntimeSemanticTypeId) -> Self {
        Self { ty }
    }

    #[must_use]
    pub const fn ty(self) -> RuntimeSemanticTypeId {
        self.ty
    }
}

/// One flow root whose parameter locals remain correlated with the semantic
/// admission that issued their construction-only handles.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFlowSeed {
    id: FlowRuntimeId,
    params: Box<[RuntimeLocalSeedId]>,
    ops: Vec<RuntimeFlowOpSeed>,
}

/// Construction-only stream transform declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStreamPlanSeed {
    pub id: StreamRuntimeId,
    pub item_ty: RuntimeSemanticTypeId,
    pub error_ty: RuntimeSemanticTypeId,
    pub ops: Vec<RuntimeStreamOpSeed>,
}

/// Closed typed stream transform algebra before plan-local admission.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeStreamOpSeed {
    Let {
        pattern: RuntimePatternSeed,
        expr: RuntimeExprSeed,
    },
    ForNext {
        pattern: RuntimePatternSeed,
        source: RuntimeExprSeed,
        body: Vec<Self>,
    },
    Yield {
        expr: RuntimeExprSeed,
    },
    If {
        condition: RuntimeExprSeed,
        then_ops: Vec<Self>,
        else_ops: Vec<Self>,
    },
    Match {
        scrutinee: RuntimeExprSeed,
        arms: Vec<RuntimeStreamMatchArmSeed>,
    },
    Close {
        source: RuntimeExprSeed,
    },
    Return,
}

/// One stream `match` arm before plan-local admission.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeStreamMatchArmSeed {
    pub pattern: RuntimePatternSeed,
    pub guard: Option<RuntimeExprSeed>,
    pub ops: Vec<RuntimeStreamOpSeed>,
}

/// One document-local dialogue value whose body is admitted into the owning
/// plan's structured function-site table.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueValueSiteSeed {
    pub slot: RuntimeDialogueValueSlotId,
    pub role: RuntimeDialogueValueRole,
    pub function: RuntimeFunctionSiteSeedId,
}

/// Construction-only dialogue content execution mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueContentPlanSeed {
    pub line: RuntimeLineId,
    pub values: Box<[RuntimeDialogueValueSiteSeed]>,
    pub marks: Box<[String]>,
}

/// Construction-only typed identity of a mark owned by one dialogue content
/// seed. It cannot be manufactured or reused across builders.
#[derive(Clone)]
pub struct RuntimeDialogueMarkSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    content: RuntimeDialogueContentPlanId,
    mark: RuntimeDialogueMarkId,
}

impl RuntimeDialogueMarkSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        content: RuntimeDialogueContentPlanId,
        mark: RuntimeDialogueMarkId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            content,
            mark,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<(RuntimeDialogueContentPlanId, RuntimeDialogueMarkId)> {
        Arc::ptr_eq(&self.issuer, issuer).then_some((self.content, self.mark))
    }
}

impl fmt::Debug for RuntimeDialogueMarkSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeDialogueMarkSeedId")
            .field(&self.mark)
            .finish()
    }
}

impl PartialEq for RuntimeDialogueMarkSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
            && self.content == other.content
            && self.mark == other.mark
    }
}

impl Eq for RuntimeDialogueMarkSeedId {}

/// Recursive source-order graph consumed atomically by the plan builder.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineTaskNodeSeed {
    Sequence(Vec<Self>),
    Start(Vec<Self>),
    Parallel {
        policy: crate::line_task::ParallelPolicy,
        children: Vec<Self>,
    },
    Child {
        id: crate::task::TaskId,
        key: Option<crate::task::TaskKey>,
        name: Option<String>,
        trigger: RuntimeLineTaskTriggerSeed,
        priority: crate::task::TaskPriority,
        join_policy: crate::line_task::ChildJoinPolicy,
        cancel_policy: crate::line_task::ChildCancelPolicy,
        scope: Box<Self>,
    },
    Action(Vec<RuntimeFlowOpSeed>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineTaskTriggerSeed {
    Immediate,
    Mark(RuntimeDialogueMarkSeedId),
    Delay(crate::time::LogicalDuration),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineTaskCancelRuleSeed {
    pub trigger: RuntimeDialogueMarkSeedId,
    pub action: Vec<RuntimeFlowOpSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeLineTaskGroupSeed {
    pub root: RuntimeLineTaskNodeSeed,
    pub cancel_rules: Box<[RuntimeLineTaskCancelRuleSeed]>,
    pub cleanup_completed: Vec<RuntimeFlowOpSeed>,
    pub cleanup_cancelled: Vec<RuntimeFlowOpSeed>,
    pub cleanup_failed: Vec<RuntimeFlowOpSeed>,
    pub cleanup_policy: crate::line_task::LineCleanupPolicy,
}

/// Builder-issued group handle used only to attach the sealed graph to its
/// unique dialogue content plan.
#[derive(Clone)]
pub struct RuntimeLineTaskGroupSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    group: RuntimeLineTaskGroupId,
}

impl RuntimeLineTaskGroupSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        group: RuntimeLineTaskGroupId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            group,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<RuntimeLineTaskGroupId> {
        Arc::ptr_eq(&self.issuer, issuer).then_some(self.group)
    }
}

impl fmt::Debug for RuntimeLineTaskGroupSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeLineTaskGroupSeedId")
            .field(&self.group)
            .finish()
    }
}

impl PartialEq for RuntimeLineTaskGroupSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.group == other.group
    }
}

impl Eq for RuntimeLineTaskGroupSeedId {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableExecutableSeed {
    pub callable: RuntimeCallableId,
    pub contract: CallableContractHash,
    pub code: RuntimeCallableExecutableSeedCode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeCallableExecutableSeedCode {
    PureHelper(RuntimePureHelperSeedId),
    ControllerFlow(FlowRuntimeId),
}

impl RuntimeFlowSeed {
    #[must_use]
    pub fn new(
        id: FlowRuntimeId,
        params: impl IntoIterator<Item = RuntimeLocalSeedId>,
        ops: Vec<RuntimeFlowOpSeed>,
    ) -> Self {
        Self {
            id,
            params: params.into_iter().collect::<Vec<_>>().into_boxed_slice(),
            ops,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        FlowRuntimeId,
        Box<[RuntimeLocalSeedId]>,
        Vec<RuntimeFlowOpSeed>,
    ) {
        (self.id, self.params, self.ops)
    }
}

/// Construction-only flow algebra. Runtime continuation frames are
/// deliberately absent: only source-order canonical operations can enter a
/// finished plan.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeFlowOpSeed {
    Let {
        pattern: RuntimePatternSeed,
        expr: RuntimeExprSeed,
    },
    LetElse {
        pattern: RuntimePatternSeed,
        expr: RuntimeExprSeed,
        else_ops: Vec<Self>,
    },
    AssignNominalField {
        base: RuntimeLocalSeedId,
        owner: RuntimeSemanticTypeId,
        field: RuntimeRecordFieldSeedId,
        value: RuntimeExprSeed,
    },
    Dialogue {
        content: RuntimeDialogueContentPlanSeedId,
    },
    Choice {
        id: Option<String>,
        options: Vec<RuntimeChoiceOptionSeed>,
    },
    Await {
        binding: Option<RuntimePatternSeed>,
        target: RuntimeAwaitTargetSeed,
        pending: Vec<RuntimeLineEffectSeed>,
    },
    AwaitMany {
        binding: Option<RuntimePatternSeed>,
        target: RuntimeAwaitManyTargetSeed,
        pending: Vec<RuntimeLineEffectSeed>,
    },
    HostCall {
        binding: Option<RuntimePatternSeed>,
        target: RuntimeHostCallTargetSeed,
    },
    If {
        condition: RuntimeExprSeed,
        then_ops: Vec<Self>,
        else_ops: Vec<Self>,
    },
    IfLet {
        pattern: RuntimePatternSeed,
        expr: RuntimeExprSeed,
        guard: Option<RuntimeExprSeed>,
        then_ops: Vec<Self>,
        else_ops: Vec<Self>,
    },
    Match {
        scrutinee: RuntimeExprSeed,
        arms: Vec<RuntimeFlowMatchArmSeed>,
    },
    Loop {
        body: Vec<Self>,
    },
    LetLoop {
        pattern: RuntimePatternSeed,
        body: Vec<Self>,
    },
    While {
        condition: RuntimeExprSeed,
        body: Vec<Self>,
    },
    WhileLet {
        pattern: RuntimePatternSeed,
        expr: RuntimeExprSeed,
        guard: Option<RuntimeExprSeed>,
        body: Vec<Self>,
    },
    For {
        pattern: RuntimePatternSeed,
        source: RuntimeExprSeed,
        evidence: RuntimeIteratorEvidenceSeed,
        body: Vec<Self>,
    },
    Thread {
        name: Option<String>,
        body: Vec<Self>,
    },
    Scope(Vec<Self>),
    Break(Option<RuntimeExprSeed>),
    Continue,
    Goto(FlowRuntimeId),
    GotoExpr(RuntimeExprSeed),
    Return(String),
    ReturnExpr(RuntimeExprSeed),
    Effect(RuntimeLineEffectSeed),
    EvaluatedEffect(RuntimeEvaluatedEffectSeed),
    RegisterCleanup {
        key: String,
        effect: RuntimeLineEffectSeed,
    },
    CancelCleanup {
        key: String,
    },
    EnterScope,
    ExitScope,
    Noop,
}

impl RuntimeLineTaskGroupSeed {
    /// Derives captures from every executable action in deterministic
    /// root/cancel/cleanup first-use order.
    pub(super) fn free_locals(&self) -> Box<[RuntimeLocalSeedId]> {
        let mut locals = Vec::new();
        self.root.collect_free_locals(&mut locals);
        for rule in &self.cancel_rules {
            collect_flow_ops_free_locals(&rule.action, &[], &mut locals);
        }
        collect_flow_ops_free_locals(&self.cleanup_completed, &[], &mut locals);
        collect_flow_ops_free_locals(&self.cleanup_cancelled, &[], &mut locals);
        collect_flow_ops_free_locals(&self.cleanup_failed, &[], &mut locals);
        locals.into_boxed_slice()
    }
}

impl RuntimeLineTaskNodeSeed {
    fn collect_free_locals(&self, locals: &mut Vec<RuntimeLocalSeedId>) {
        match self {
            Self::Sequence(children) | Self::Start(children) | Self::Parallel { children, .. } => {
                for child in children {
                    child.collect_free_locals(locals);
                }
            }
            Self::Child { scope, .. } => scope.collect_free_locals(locals),
            Self::Action(actions) => collect_flow_ops_free_locals(actions, &[], locals),
        }
    }
}

fn collect_flow_ops_free_locals(
    ops: &[RuntimeFlowOpSeed],
    initial_bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    let mut bound = initial_bound.to_vec();
    for op in ops {
        collect_flow_op_free_locals(op, &mut bound, locals);
    }
}

fn collect_flow_op_free_locals(
    op: &RuntimeFlowOpSeed,
    bound: &mut Vec<RuntimeLocalSeedId>,
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    if collect_binding_or_host_free_locals(op, bound, locals) {
        return;
    }
    if collect_control_flow_free_locals(op, bound, locals) {
        return;
    }
    collect_terminal_or_effect_free_locals(op, bound, locals);
}

fn collect_binding_or_host_free_locals(
    op: &RuntimeFlowOpSeed,
    bound: &mut Vec<RuntimeLocalSeedId>,
    locals: &mut Vec<RuntimeLocalSeedId>,
) -> bool {
    match op {
        RuntimeFlowOpSeed::Let { pattern, expr } => {
            expr.collect_free_locals(bound, locals);
            pattern.collect_binding_locals(bound);
        }
        RuntimeFlowOpSeed::LetElse {
            pattern,
            expr,
            else_ops,
        } => {
            expr.collect_free_locals(bound, locals);
            collect_flow_ops_free_locals(else_ops, bound, locals);
            pattern.collect_binding_locals(bound);
        }
        RuntimeFlowOpSeed::AssignNominalField { base, value, .. } => {
            push_free_local(base, bound, locals);
            value.collect_free_locals(bound, locals);
        }
        RuntimeFlowOpSeed::Await {
            binding,
            target,
            pending,
        } => {
            collect_host_argument_free_locals(&target.request.args, bound, locals);
            for effect in pending {
                effect.collect_free_locals(bound, locals);
            }
            if let Some(binding) = binding {
                binding.collect_binding_locals(bound);
            }
        }
        RuntimeFlowOpSeed::AwaitMany {
            binding,
            target,
            pending,
        } => {
            target.source.collect_free_locals(bound, locals);
            let mut request_bound = bound.clone();
            push_unique_local(&target.item_binding, &mut request_bound);
            collect_host_argument_free_locals(&target.request.args, &request_bound, locals);
            for effect in pending {
                effect.collect_free_locals(bound, locals);
            }
            if let Some(binding) = binding {
                binding.collect_binding_locals(bound);
            }
        }
        RuntimeFlowOpSeed::HostCall { binding, target } => {
            collect_host_argument_free_locals(&target.args, bound, locals);
            if let Some(binding) = binding {
                binding.collect_binding_locals(bound);
            }
        }
        _ => return false,
    }
    true
}

fn collect_control_flow_free_locals(
    op: &RuntimeFlowOpSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) -> bool {
    match op {
        RuntimeFlowOpSeed::If {
            condition,
            then_ops,
            else_ops,
        } => {
            condition.collect_free_locals(bound, locals);
            collect_flow_ops_free_locals(then_ops, bound, locals);
            collect_flow_ops_free_locals(else_ops, bound, locals);
        }
        RuntimeFlowOpSeed::IfLet {
            pattern,
            expr,
            guard,
            then_ops,
            else_ops,
        } => {
            expr.collect_free_locals(bound, locals);
            let mut then_bound = bound.to_vec();
            pattern.collect_binding_locals(&mut then_bound);
            if let Some(guard) = guard {
                guard.collect_free_locals(&then_bound, locals);
            }
            collect_flow_ops_free_locals(then_ops, &then_bound, locals);
            collect_flow_ops_free_locals(else_ops, bound, locals);
        }
        RuntimeFlowOpSeed::Match { scrutinee, arms } => {
            scrutinee.collect_free_locals(bound, locals);
            for arm in arms {
                let mut arm_bound = bound.to_vec();
                arm.pattern.collect_binding_locals(&mut arm_bound);
                if let Some(guard) = &arm.guard {
                    guard.collect_free_locals(&arm_bound, locals);
                }
                collect_flow_ops_free_locals(&arm.ops, &arm_bound, locals);
            }
        }
        RuntimeFlowOpSeed::Loop { body }
        | RuntimeFlowOpSeed::Thread { body, .. }
        | RuntimeFlowOpSeed::Scope(body) => collect_flow_ops_free_locals(body, bound, locals),
        RuntimeFlowOpSeed::LetLoop { pattern, body } => {
            let mut body_bound = bound.to_vec();
            pattern.collect_binding_locals(&mut body_bound);
            collect_flow_ops_free_locals(body, &body_bound, locals);
        }
        RuntimeFlowOpSeed::While { condition, body } => {
            condition.collect_free_locals(bound, locals);
            collect_flow_ops_free_locals(body, bound, locals);
        }
        RuntimeFlowOpSeed::WhileLet {
            pattern,
            expr,
            guard,
            body,
        } => {
            expr.collect_free_locals(bound, locals);
            let mut body_bound = bound.to_vec();
            pattern.collect_binding_locals(&mut body_bound);
            if let Some(guard) = guard {
                guard.collect_free_locals(&body_bound, locals);
            }
            collect_flow_ops_free_locals(body, &body_bound, locals);
        }
        RuntimeFlowOpSeed::For {
            pattern,
            source,
            body,
            ..
        } => {
            source.collect_free_locals(bound, locals);
            let mut body_bound = bound.to_vec();
            pattern.collect_binding_locals(&mut body_bound);
            collect_flow_ops_free_locals(body, &body_bound, locals);
        }
        _ => return false,
    }
    true
}

fn collect_terminal_or_effect_free_locals(
    op: &RuntimeFlowOpSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    match op {
        RuntimeFlowOpSeed::Dialogue { .. }
        | RuntimeFlowOpSeed::Continue
        | RuntimeFlowOpSeed::Goto(_)
        | RuntimeFlowOpSeed::Return(_)
        | RuntimeFlowOpSeed::CancelCleanup { .. }
        | RuntimeFlowOpSeed::EnterScope
        | RuntimeFlowOpSeed::ExitScope
        | RuntimeFlowOpSeed::Noop => {}
        RuntimeFlowOpSeed::Choice { options, .. } => {
            for option in options {
                for effect in &option.effects {
                    effect.collect_free_locals(bound, locals);
                }
            }
        }
        RuntimeFlowOpSeed::Break(value) => {
            if let Some(value) = value {
                value.collect_free_locals(bound, locals);
            }
        }
        RuntimeFlowOpSeed::GotoExpr(value) | RuntimeFlowOpSeed::ReturnExpr(value) => {
            value.collect_free_locals(bound, locals);
        }
        RuntimeFlowOpSeed::Effect(effect) | RuntimeFlowOpSeed::RegisterCleanup { effect, .. } => {
            effect.collect_free_locals(bound, locals);
        }
        RuntimeFlowOpSeed::EvaluatedEffect(effect) => {
            effect.collect_free_locals(bound, locals);
        }
        RuntimeFlowOpSeed::Let { .. }
        | RuntimeFlowOpSeed::LetElse { .. }
        | RuntimeFlowOpSeed::AssignNominalField { .. }
        | RuntimeFlowOpSeed::Await { .. }
        | RuntimeFlowOpSeed::AwaitMany { .. }
        | RuntimeFlowOpSeed::HostCall { .. }
        | RuntimeFlowOpSeed::If { .. }
        | RuntimeFlowOpSeed::IfLet { .. }
        | RuntimeFlowOpSeed::Match { .. }
        | RuntimeFlowOpSeed::Loop { .. }
        | RuntimeFlowOpSeed::LetLoop { .. }
        | RuntimeFlowOpSeed::While { .. }
        | RuntimeFlowOpSeed::WhileLet { .. }
        | RuntimeFlowOpSeed::For { .. }
        | RuntimeFlowOpSeed::Thread { .. }
        | RuntimeFlowOpSeed::Scope(_) => unreachable!("flow-op collector dispatched variant"),
    }
}

fn collect_host_argument_free_locals(
    arguments: &[RuntimeHostArgumentSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    for argument in arguments {
        match argument {
            RuntimeHostArgumentSeed::Positional(value) | RuntimeHostArgumentSeed::Spread(value) => {
                value.collect_free_locals(bound, locals);
            }
            RuntimeHostArgumentSeed::Named(argument) => {
                argument.value.collect_free_locals(bound, locals);
            }
        }
    }
}

/// A construction-only dialogue content handle issued by one builder.
#[derive(Clone)]
pub struct RuntimeDialogueContentPlanSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    content: RuntimeDialogueContentPlanId,
}

impl RuntimeDialogueContentPlanSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        content: RuntimeDialogueContentPlanId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            content,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<RuntimeDialogueContentPlanId> {
        Arc::ptr_eq(&self.issuer, issuer).then_some(self.content)
    }

    #[must_use]
    pub fn mark(&self, index: usize) -> Option<RuntimeDialogueMarkSeedId> {
        RuntimeDialogueMarkId::from_zero_based(index)
            .map(|mark| RuntimeDialogueMarkSeedId::issued(&self.issuer, self.content, mark))
    }
}

impl fmt::Debug for RuntimeDialogueContentPlanSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeDialogueContentPlanSeedId")
            .field(&self.content)
            .finish()
    }
}

impl PartialEq for RuntimeDialogueContentPlanSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.content == other.content
    }
}

impl Eq for RuntimeDialogueContentPlanSeedId {}

/// Iterator evidence whose witness endpoints remain correlated with this
/// builder's reserved trait-method signatures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorEvidenceSeed {
    Builtin(RuntimeBuiltinIteratorEvidence),
    Witness(RuntimeIteratorWitnessEvidenceSeed),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIteratorWitnessEvidenceSeed {
    pub item: RuntimeSemanticTypeId,
    pub iterator: RuntimeSemanticTypeId,
    pub executable: RuntimeIteratorWitnessExecutableSeed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIteratorWitnessExecutableSeed {
    TraitCalls {
        into_iter: RuntimeTraitMethodSeedId,
        next: RuntimeTraitMethodSeedId,
    },
    IdentityIntoIterator {
        next: RuntimeTraitMethodSeedId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFlowMatchArmSeed {
    pub pattern: RuntimePatternSeed,
    pub guard: Option<RuntimeExprSeed>,
    pub ops: Vec<RuntimeFlowOpSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeChoiceOptionSeed {
    pub id: Option<String>,
    pub label: String,
    pub target: Option<FlowRuntimeId>,
    pub out: Option<crate::line_task::LineOutRequest>,
    pub effects: Vec<RuntimeLineEffectSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitTargetSeed {
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: crate::task::TaskOutcomeContract,
    pub request: RuntimeHostTaskRequestTemplateSeed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeAwaitManyTargetSeed {
    pub need: NeedId,
    pub task: TaskId,
    pub outcome: crate::task::TaskOutcomeContract,
    pub source: RuntimeExprSeed,
    pub item_binding: RuntimeLocalSeedId,
    pub limit: usize,
    pub request: RuntimeHostTaskRequestTemplateSeed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHostTaskRequestTemplateSeed {
    pub capability: HostCapabilityId,
    pub operation: String,
    pub args: Vec<RuntimeHostArgumentSeed>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeHostArgumentSeed {
    Positional(RuntimeExprSeed),
    Named(NamedHostArg<RuntimeExprSeed>),
    Spread(RuntimeExprSeed),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeHostCallTargetSeed {
    pub public_id: String,
    pub capability: String,
    pub operation: String,
    pub args: Vec<RuntimeHostArgumentSeed>,
    pub mode: RuntimeHostCallMode,
    pub deterministic: bool,
}

/// Host-facing effect metadata, with the sole expression-bearing Audio arm
/// split out so a raw `RuntimeAudioCommand` cannot bypass recursive admission.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeLineEffectSeed {
    Static(LineEffectRequest),
    Audio(Box<RuntimeAudioCommandSeed>),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAudioCommandSeed {
    Play {
        voice: RuntimeExprSeed,
        resource: RuntimeExprSeed,
        bus: RuntimeExprSeed,
        gain_db_milli: RuntimeExprSeed,
        pan_milli: RuntimeExprSeed,
        loop_mode: arcweft_interaction_model::audio::AudioLoopMode,
        start_frame: RuntimeExprSeed,
        fade_in_millis: RuntimeExprSeed,
    },
    Stop {
        voice: RuntimeExprSeed,
        fade_out_millis: RuntimeExprSeed,
    },
    StopAll {
        fade_out_millis: RuntimeExprSeed,
    },
    SetVoiceGain {
        voice: RuntimeExprSeed,
        gain_db_milli: RuntimeExprSeed,
        transition_millis: RuntimeExprSeed,
    },
    SetVoicePan {
        voice: RuntimeExprSeed,
        pan_milli: RuntimeExprSeed,
        transition_millis: RuntimeExprSeed,
    },
    SetBusGain {
        bus: RuntimeExprSeed,
        gain_db_milli: RuntimeExprSeed,
        transition_millis: RuntimeExprSeed,
    },
    SetBusMute {
        bus: RuntimeExprSeed,
        muted: RuntimeExprSeed,
    },
    SetEffectEnabled {
        bus: RuntimeExprSeed,
        effect: RuntimeExprSeed,
        enabled: RuntimeExprSeed,
    },
    SetEffectParameter {
        bus: RuntimeExprSeed,
        effect: RuntimeExprSeed,
        parameter: arcweft_interaction_model::audio::AudioEffectParameterKind,
        value: RuntimeExprSeed,
        transition_millis: RuntimeExprSeed,
    },
    ApplySnapshot {
        snapshot: RuntimeExprSeed,
        transition_millis: RuntimeExprSeed,
    },
    RequestMicrophone {
        capture: RuntimeExprSeed,
        constraints: arcweft_interaction_model::audio::MicrophoneConstraints,
    },
    StopMicrophone {
        capture: RuntimeExprSeed,
    },
    SetCaptureMonitor {
        capture: RuntimeExprSeed,
        bus: Option<RuntimeExprSeed>,
        gain_db_milli: RuntimeExprSeed,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEffectFieldSeed {
    pub name: String,
    pub value: RuntimeExprSeed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeEvaluatedEffectSeed {
    Log {
        level: String,
        message: RuntimeExprSeed,
        fields: Vec<RuntimeEffectFieldSeed>,
    },
    SignalWrite {
        target: RuntimeExprSeed,
        value: RuntimeExprSeed,
    },
    MetricWrite {
        target: RuntimeExprSeed,
        value: RuntimeExprSeed,
    },
    EmitEvent {
        event: RuntimeExprSeed,
        fields: Vec<RuntimeEffectFieldSeed>,
    },
    Panic(RuntimeExprSeed),
    Fail(RuntimeExprSeed),
    Bail(RuntimeExprSeed),
    Ensure {
        condition: RuntimeExprSeed,
        message: RuntimeExprSeed,
    },
    Assert {
        guard: RuntimeAssertionGuardId,
        condition: RuntimeExprSeed,
        message: String,
        profile: RuntimeAssertionProfile,
    },
}

impl RuntimeLineEffectSeed {
    fn collect_free_locals(
        &self,
        bound: &[RuntimeLocalSeedId],
        locals: &mut Vec<RuntimeLocalSeedId>,
    ) {
        if let Self::Audio(command) = self {
            command.collect_free_locals(bound, locals);
        }
    }
}

impl RuntimeAudioCommandSeed {
    fn collect_free_locals(
        &self,
        bound: &[RuntimeLocalSeedId],
        locals: &mut Vec<RuntimeLocalSeedId>,
    ) {
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
            } => collect_expr_free_locals(
                &[
                    voice.clone(),
                    resource.clone(),
                    bus.clone(),
                    gain_db_milli.clone(),
                    pan_milli.clone(),
                    start_frame.clone(),
                    fade_in_millis.clone(),
                ],
                bound,
                locals,
            ),
            Self::Stop {
                voice,
                fade_out_millis,
            } => collect_expr_refs_free_locals(&[voice, fade_out_millis], bound, locals),
            Self::StopAll { fade_out_millis } => {
                fade_out_millis.collect_free_locals(bound, locals);
            }
            Self::SetVoiceGain {
                voice,
                gain_db_milli,
                transition_millis,
            }
            | Self::SetVoicePan {
                voice,
                pan_milli: gain_db_milli,
                transition_millis,
            }
            | Self::SetBusGain {
                bus: voice,
                gain_db_milli,
                transition_millis,
            } => collect_expr_refs_free_locals(
                &[voice, gain_db_milli, transition_millis],
                bound,
                locals,
            ),
            Self::SetBusMute { bus, muted } => {
                collect_expr_refs_free_locals(&[bus, muted], bound, locals);
            }
            Self::SetEffectEnabled {
                bus,
                effect,
                enabled,
            } => collect_expr_refs_free_locals(&[bus, effect, enabled], bound, locals),
            Self::SetEffectParameter {
                bus,
                effect,
                value,
                transition_millis,
                ..
            } => collect_expr_refs_free_locals(
                &[bus, effect, value, transition_millis],
                bound,
                locals,
            ),
            Self::ApplySnapshot {
                snapshot,
                transition_millis,
            } => collect_expr_refs_free_locals(&[snapshot, transition_millis], bound, locals),
            Self::RequestMicrophone { capture, .. } | Self::StopMicrophone { capture } => {
                capture.collect_free_locals(bound, locals);
            }
            Self::SetCaptureMonitor {
                capture,
                bus,
                gain_db_milli,
            } => {
                capture.collect_free_locals(bound, locals);
                if let Some(bus) = bus {
                    bus.collect_free_locals(bound, locals);
                }
                gain_db_milli.collect_free_locals(bound, locals);
            }
        }
    }
}

impl RuntimeEvaluatedEffectSeed {
    fn collect_free_locals(
        &self,
        bound: &[RuntimeLocalSeedId],
        locals: &mut Vec<RuntimeLocalSeedId>,
    ) {
        let collect_fields = |fields: &[RuntimeEffectFieldSeed], locals: &mut Vec<_>| {
            for field in fields {
                field.value.collect_free_locals(bound, locals);
            }
        };
        match self {
            Self::Log {
                message, fields, ..
            } => {
                message.collect_free_locals(bound, locals);
                collect_fields(fields, locals);
            }
            Self::SignalWrite { target, value } | Self::MetricWrite { target, value } => {
                collect_expr_refs_free_locals(&[target, value], bound, locals);
            }
            Self::EmitEvent { event, fields } => {
                event.collect_free_locals(bound, locals);
                collect_fields(fields, locals);
            }
            Self::Panic(value) | Self::Fail(value) | Self::Bail(value) => {
                value.collect_free_locals(bound, locals);
            }
            Self::Ensure { condition, message } => {
                collect_expr_refs_free_locals(&[condition, message], bound, locals);
            }
            Self::Assert { condition, .. } => condition.collect_free_locals(bound, locals),
        }
    }
}

fn collect_expr_refs_free_locals(
    expressions: &[&RuntimeExprSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    for expression in expressions {
        expression.collect_free_locals(bound, locals);
    }
}

/// A construction-only local handle issued by one `RuntimePlanBuilder`.
///
/// The private issuer token prevents a handle returned by another builder
/// from being interpreted as the same plan-local ordinal.
#[derive(Clone)]
pub struct RuntimeLocalSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    local: RuntimeLocalDeclarationId,
    ty: RuntimePlanTypeId,
}

impl RuntimeLocalSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        local: RuntimeLocalDeclarationId,
        ty: RuntimePlanTypeId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            local,
            ty,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<(RuntimeLocalDeclarationId, RuntimePlanTypeId)> {
        Arc::ptr_eq(&self.issuer, issuer).then_some((self.local, self.ty))
    }
}

impl fmt::Debug for RuntimeLocalSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeLocalSeedId")
            .field(&self.local)
            .finish()
    }
}

impl PartialEq for RuntimeLocalSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer) && self.local == other.local && self.ty == other.ty
    }
}

impl Eq for RuntimeLocalSeedId {}

/// A construction-only function-site handle issued by one builder.
#[derive(Clone)]
pub struct RuntimeFunctionSiteSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    site: RuntimeFunctionSiteId,
    parameters: Box<[RuntimePlanTypeId]>,
    result: RuntimePlanTypeId,
}

impl RuntimeFunctionSiteSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        site: RuntimeFunctionSiteId,
        parameters: Box<[RuntimePlanTypeId]>,
        result: RuntimePlanTypeId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            site,
            parameters,
            result,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<(
        RuntimeFunctionSiteId,
        &[RuntimePlanTypeId],
        RuntimePlanTypeId,
    )> {
        Arc::ptr_eq(&self.issuer, issuer).then_some((
            self.site,
            self.parameters.as_ref(),
            self.result,
        ))
    }
}

impl fmt::Debug for RuntimeFunctionSiteSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeFunctionSiteSeedId")
            .field(&self.site)
            .finish()
    }
}

impl PartialEq for RuntimeFunctionSiteSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
            && self.site == other.site
            && self.parameters == other.parameters
            && self.result == other.result
    }
}

impl Eq for RuntimeFunctionSiteSeedId {}

/// A construction-only pure-helper handle issued after its recursive body and
/// signature have been admitted.
#[derive(Clone)]
pub struct RuntimePureHelperSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    helper: RuntimePureHelperId,
    parameters: Box<[RuntimePlanTypeId]>,
    result: RuntimePlanTypeId,
}

impl RuntimePureHelperSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        helper: RuntimePureHelperId,
        parameters: Box<[RuntimePlanTypeId]>,
        result: RuntimePlanTypeId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            helper,
            parameters,
            result,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<(RuntimePureHelperId, &[RuntimePlanTypeId], RuntimePlanTypeId)> {
        Arc::ptr_eq(&self.issuer, issuer).then_some((
            self.helper,
            self.parameters.as_ref(),
            self.result,
        ))
    }
}

impl fmt::Debug for RuntimePureHelperSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimePureHelperSeedId")
            .field(&self.helper)
            .finish()
    }
}

impl PartialEq for RuntimePureHelperSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
            && self.helper == other.helper
            && self.parameters == other.parameters
            && self.result == other.result
    }
}

impl Eq for RuntimePureHelperSeedId {}

/// A construction-only trait-method handle. Receiver mode and the complete
/// parameter graph remain correlated with the admitted body.
#[derive(Clone)]
pub struct RuntimeTraitMethodSeedId {
    issuer: Arc<RuntimePlanConstructionIssuer>,
    method: RuntimeTraitMethodId,
    receiver: RuntimeReceiverMode,
    parameters: Box<[RuntimePlanTypeId]>,
    result: RuntimePlanTypeId,
}

impl RuntimeTraitMethodSeedId {
    pub(super) fn issued(
        issuer: &Arc<RuntimePlanConstructionIssuer>,
        method: RuntimeTraitMethodId,
        receiver: RuntimeReceiverMode,
        parameters: Box<[RuntimePlanTypeId]>,
        result: RuntimePlanTypeId,
    ) -> Self {
        Self {
            issuer: Arc::clone(issuer),
            method,
            receiver,
            parameters,
            result,
        }
    }

    pub(super) fn resolve(
        &self,
        issuer: &Arc<RuntimePlanConstructionIssuer>,
    ) -> Option<(
        RuntimeTraitMethodId,
        RuntimeReceiverMode,
        &[RuntimePlanTypeId],
        RuntimePlanTypeId,
    )> {
        Arc::ptr_eq(&self.issuer, issuer).then_some((
            self.method,
            self.receiver,
            self.parameters.as_ref(),
            self.result,
        ))
    }
}

impl fmt::Debug for RuntimeTraitMethodSeedId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RuntimeTraitMethodSeedId")
            .field(&self.method)
            .finish()
    }
}

impl PartialEq for RuntimeTraitMethodSeedId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.issuer, &other.issuer)
            && self.method == other.method
            && self.receiver == other.receiver
            && self.parameters == other.parameters
            && self.result == other.result
    }
}

impl Eq for RuntimeTraitMethodSeedId {}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePureHelperSeed {
    pub name: String,
    pub inputs: Box<[RuntimeLocalSeedId]>,
    pub input_abi: Vec<RuntimePureInputType>,
    pub output_abi: RuntimePureOutputType,
    pub body: RuntimeExprSeed,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

/// Signature-only reservation for one plan-owned pure helper. Bodies are
/// defined only after every callable handle has been issued.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePureHelperDeclarationSeed {
    pub name: String,
    pub inputs: Box<[RuntimeLocalSeedId]>,
    pub input_abi: Vec<RuntimePureInputType>,
    pub result: RuntimeSemanticTypeId,
    pub output_abi: RuntimePureOutputType,
    pub scalar_eval_supported: bool,
    pub origin: RuntimePureHelperOrigin,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTraitMethodSeed {
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub inputs: Box<[RuntimeLocalSeedId]>,
    pub input_abi: Vec<RuntimePureInputType>,
    pub output_abi: RuntimePureOutputType,
    pub body: RuntimeExprSeed,
}

/// Signature-only reservation for one plan-owned trait method.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeTraitMethodDeclarationSeed {
    pub identity: RuntimeTraitMethodIdentity,
    pub receiver: RuntimeReceiverMode,
    pub inputs: Box<[RuntimeLocalSeedId]>,
    pub input_abi: Vec<RuntimePureInputType>,
    pub result: RuntimeSemanticTypeId,
    pub output_abi: RuntimePureOutputType,
}

/// Signature-only reservation for one plan-owned structured function site.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeFunctionSiteDeclarationSeed {
    pub params: Box<[RuntimeLocalSeedId]>,
    pub captures: Box<[RuntimeLocalSeedId]>,
    pub result: RuntimeSemanticTypeId,
}

/// Zero-based field coordinate in one accepted record domain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeRecordFieldSeedId(u32);

impl RuntimeRecordFieldSeedId {
    #[must_use]
    pub const fn from_zero_based(ordinal: u32) -> Self {
        Self(ordinal)
    }

    #[must_use]
    pub const fn zero_based(self) -> u32 {
        self.0
    }
}

/// Checked field coordinate before plan-local owner/type rewriting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeFieldProjectionSeed {
    Nominal {
        owner: RuntimeSemanticTypeId,
        field: RuntimeRecordFieldSeedId,
    },
    Agent(RuntimeAgentField),
    EntityReference(RuntimeEntityReferenceField),
}

/// One recursively typed expression submitted to the aggregate builder.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExprSeed {
    ty: RuntimeSemanticTypeId,
    kind: RuntimeExprSeedKind,
}

impl RuntimeExprSeed {
    #[must_use]
    pub const fn new(ty: RuntimeSemanticTypeId, kind: RuntimeExprSeedKind) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimeSemanticTypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &RuntimeExprSeedKind {
        &self.kind
    }

    pub(super) fn into_parts(self) -> (RuntimeSemanticTypeId, RuntimeExprSeedKind) {
        (self.ty, self.kind)
    }
}

/// Closed canonical expression algebra. Plan-local pure/trait calls accept
/// only correlated construction handles; string-selected methods remain
/// absent until semantic facts own an exact resolved method coordinate.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeExprSeedKind {
    Value(RuntimeValue),
    Agent(RuntimeAgentExprSeed),
    Local(RuntimeLocalSeedId),
    EntityRef(RuntimeEntityReference),
    Let {
        binding: RuntimeLocalSeedId,
        expr: Box<RuntimeExprSeed>,
        body: Box<RuntimeExprSeed>,
    },
    Tuple(Box<[RuntimeExprSeed]>),
    BracketSeq(Box<[RuntimeExprSeed]>),
    RepeatSeq {
        value: Box<RuntimeExprSeed>,
        len: usize,
    },
    Range {
        start: Option<Box<RuntimeExprSeed>>,
        end: Option<Box<RuntimeExprSeed>>,
        inclusive: bool,
    },
    NominalRecord(Box<[RuntimeNominalRecordFieldSeed]>),
    Variant {
        ordinal: u32,
        payload: Option<Box<RuntimeExprSeed>>,
    },
    Field {
        target: Box<RuntimeExprSeed>,
        field: RuntimeFieldProjectionSeed,
    },
    ProjectTuple {
        target: Box<RuntimeExprSeed>,
        ordinal: u32,
    },
    ProjectRecord {
        target: Box<RuntimeExprSeed>,
        field: RuntimeRecordFieldSeedId,
    },
    AssignNominalField {
        base: RuntimeLocalSeedId,
        owner: RuntimeSemanticTypeId,
        field: RuntimeRecordFieldSeedId,
        expr: Box<RuntimeExprSeed>,
        body: Box<RuntimeExprSeed>,
    },
    Call {
        callee: RuntimeCallTarget,
        args: Box<[RuntimeCallArgumentSeed]>,
    },
    Function(RuntimeFunctionSiteSeedId),
    Apply {
        callee: Box<RuntimeExprSeed>,
        args: Box<[RuntimeCallArgumentSeed]>,
    },
    TraitCall {
        callable: RuntimeTraitMethodSeedId,
        receiver: Box<RuntimeExprSeed>,
        args: Box<[RuntimeCallArgumentSeed]>,
    },
    PureCall {
        helper: RuntimePureHelperSeedId,
        args: Box<[RuntimeCallArgumentSeed]>,
    },
    Map {
        source: Box<RuntimeExprSeed>,
        param: RuntimeLocalSeedId,
        body: Box<RuntimeExprSeed>,
    },
    Filter {
        source: Box<RuntimeExprSeed>,
        param: RuntimeLocalSeedId,
        body: Box<RuntimeExprSeed>,
    },
    Sum {
        source: Box<RuntimeExprSeed>,
    },
    Unary {
        op: RuntimeUnaryOp,
        expr: Box<RuntimeExprSeed>,
    },
    Binary {
        lhs: Box<RuntimeExprSeed>,
        op: RuntimeBinaryOp,
        rhs: Box<RuntimeExprSeed>,
    },
    If {
        condition: Box<RuntimeExprSeed>,
        then_expr: Box<RuntimeExprSeed>,
        else_expr: Box<RuntimeExprSeed>,
    },
    IfLet {
        pattern: RuntimePatternSeed,
        expr: Box<RuntimeExprSeed>,
        guard: Option<Box<RuntimeExprSeed>>,
        then_expr: Box<RuntimeExprSeed>,
        else_expr: Box<RuntimeExprSeed>,
    },
    Match {
        scrutinee: Box<RuntimeExprSeed>,
        arms: Box<[RuntimeExprMatchArmSeed]>,
    },
    ReductionUnchanged {
        state: Box<RuntimeExprSeed>,
    },
}

/// Dedicated Agent seed algebra; generic records are not an alternate input.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeAgentExprSeed {
    ChoiceAction {
        choice: RuntimeCommandTargetId,
    },
    CaptureViewport,
    CaptureLayer {
        target: Box<RuntimeExprSeed>,
    },
    CaptureObject {
        target: Box<RuntimeExprSeed>,
    },
    StatePath {
        path: Box<RuntimeExprSeed>,
    },
    ObservationPath {
        path: Box<RuntimeExprSeed>,
    },
    ProbeSignal {
        target: Box<RuntimeExprSeed>,
    },
    ProbeMetric {
        target: Box<RuntimeExprSeed>,
    },
    ProbeState {
        path: Box<RuntimeExprSeed>,
    },
    ProbeObservation {
        path: Box<RuntimeExprSeed>,
    },
    Diagnostics,
    PredicateExists {
        probe: Box<RuntimeExprSeed>,
    },
    PredicateActionEnabled {
        target: Box<RuntimeExprSeed>,
    },
    PredicateDiagnosticsHasError {
        diagnostics: Box<RuntimeExprSeed>,
    },
    PredicateAll {
        predicates: Box<[RuntimeExprSeed]>,
    },
    PredicateAny {
        predicates: Box<[RuntimeExprSeed]>,
    },
    PredicateNot {
        predicate: Box<RuntimeExprSeed>,
    },
    PredicateCompare {
        probe: Box<RuntimeExprSeed>,
        op: RuntimeAgentCompareOp,
        value: Box<RuntimeExprSeed>,
    },
    ViewportPoint {
        x: Box<RuntimeExprSeed>,
        y: Box<RuntimeExprSeed>,
    },
}

/// One call operand with its accepted resolved-order modifier.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCallArgumentSeed {
    value: RuntimeExprSeed,
    mode: RuntimeCallArgumentMode,
}

impl RuntimeCallArgumentSeed {
    #[must_use]
    pub const fn new(value: RuntimeExprSeed, mode: RuntimeCallArgumentMode) -> Self {
        Self { value, mode }
    }

    pub(super) fn into_parts(self) -> (RuntimeExprSeed, RuntimeCallArgumentMode) {
        (self.value, self.mode)
    }
}

/// One nominal-record initializer in authored order.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeNominalRecordFieldSeed {
    field: RuntimeRecordFieldSeedId,
    value: RuntimeExprSeed,
}

impl RuntimeNominalRecordFieldSeed {
    #[must_use]
    pub const fn new(field: RuntimeRecordFieldSeedId, value: RuntimeExprSeed) -> Self {
        Self { field, value }
    }

    pub(super) fn into_parts(self) -> (RuntimeRecordFieldSeedId, RuntimeExprSeed) {
        (self.field, self.value)
    }
}

/// One typed value-producing match arm.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeExprMatchArmSeed {
    pattern: RuntimePatternSeed,
    guard: Option<RuntimeExprSeed>,
    value: RuntimeExprSeed,
}

impl RuntimeExprMatchArmSeed {
    #[must_use]
    pub const fn new(
        pattern: RuntimePatternSeed,
        guard: Option<RuntimeExprSeed>,
        value: RuntimeExprSeed,
    ) -> Self {
        Self {
            pattern,
            guard,
            value,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (RuntimePatternSeed, Option<RuntimeExprSeed>, RuntimeExprSeed) {
        (self.pattern, self.guard, self.value)
    }
}

/// One recursively typed pattern submitted to the aggregate builder.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePatternSeed {
    ty: RuntimeSemanticTypeId,
    kind: RuntimePatternSeedKind,
}

impl RuntimePatternSeed {
    #[must_use]
    pub const fn new(ty: RuntimeSemanticTypeId, kind: RuntimePatternSeedKind) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> RuntimeSemanticTypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &RuntimePatternSeedKind {
        &self.kind
    }

    pub(super) fn into_parts(self) -> (RuntimeSemanticTypeId, RuntimePatternSeedKind) {
        (self.ty, self.kind)
    }
}

/// Closed pattern seed algebra. Binding paths are derived, never supplied.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimePatternSeedKind {
    Bind {
        mutable: bool,
        local: RuntimeLocalSeedId,
    },
    Discard,
    Literal(RuntimeValue),
    Entity(RuntimeEntityReference),
    Tuple(Box<[RuntimePatternSeed]>),
    Record {
        fields: Box<[RuntimeRecordPatternFieldSeed]>,
        rest: RuntimePatternRestSeed,
    },
    Sequence {
        items: Box<[RuntimePatternSeed]>,
        rest: RuntimePatternRestSeed,
    },
    Variant {
        ordinal: u32,
        payload: Option<Box<RuntimePatternSeed>>,
    },
    Whole {
        local: RuntimeLocalSeedId,
        pattern: Box<RuntimePatternSeed>,
    },
    Typed {
        local: RuntimeLocalSeedId,
    },
}

/// Exact, ignored, or binding remainder before coordinate derivation.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimePatternRestSeed {
    Exact,
    Ignore,
    Bind(RuntimeLocalSeedId),
}

/// One accepted record-field pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeRecordPatternFieldSeed {
    field: RuntimeRecordFieldSeedId,
    pattern: RuntimePatternSeed,
}

impl RuntimeRecordPatternFieldSeed {
    #[must_use]
    pub const fn new(field: RuntimeRecordFieldSeedId, pattern: RuntimePatternSeed) -> Self {
        Self { field, pattern }
    }

    pub(super) fn into_parts(self) -> (RuntimeRecordFieldSeedId, RuntimePatternSeed) {
        (self.field, self.pattern)
    }
}

impl RuntimeExprSeed {
    /// Returns free local declarations in deterministic first-use order.
    ///
    /// Construction callers use this to reserve capture lists before the
    /// owning function site is admitted. Nested function sites are lexical
    /// boundaries and are therefore not traversed.
    #[must_use]
    pub fn free_locals(&self) -> Box<[RuntimeLocalSeedId]> {
        let mut locals = Vec::new();
        self.collect_free_locals(&[], &mut locals);
        locals.into_boxed_slice()
    }

    fn collect_free_locals(
        &self,
        bound: &[RuntimeLocalSeedId],
        locals: &mut Vec<RuntimeLocalSeedId>,
    ) {
        match self.kind() {
            RuntimeExprSeedKind::Value(_)
            | RuntimeExprSeedKind::EntityRef(_)
            | RuntimeExprSeedKind::Function(_) => {}
            RuntimeExprSeedKind::Agent(agent) => agent.collect_free_locals(bound, locals),
            RuntimeExprSeedKind::Local(local) => push_free_local(local, bound, locals),
            RuntimeExprSeedKind::Let {
                binding,
                expr,
                body,
            } => {
                collect_let_free_locals(binding, expr, body, bound, locals);
            }
            RuntimeExprSeedKind::Tuple(items) | RuntimeExprSeedKind::BracketSeq(items) => {
                collect_expr_free_locals(items, bound, locals);
            }
            RuntimeExprSeedKind::RepeatSeq { value, .. }
            | RuntimeExprSeedKind::Sum { source: value }
            | RuntimeExprSeedKind::Unary { expr: value, .. }
            | RuntimeExprSeedKind::ReductionUnchanged { state: value } => {
                value.collect_free_locals(bound, locals);
            }
            RuntimeExprSeedKind::Range { start, end, .. } => {
                collect_range_free_locals(start.as_deref(), end.as_deref(), bound, locals);
            }
            RuntimeExprSeedKind::NominalRecord(fields) => {
                collect_nominal_record_free_locals(fields, bound, locals);
            }
            RuntimeExprSeedKind::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    payload.collect_free_locals(bound, locals);
                }
            }
            RuntimeExprSeedKind::Field { target, .. }
            | RuntimeExprSeedKind::ProjectTuple { target, .. }
            | RuntimeExprSeedKind::ProjectRecord { target, .. } => {
                target.collect_free_locals(bound, locals);
            }
            RuntimeExprSeedKind::AssignNominalField {
                base, expr, body, ..
            } => {
                push_free_local(base, bound, locals);
                expr.collect_free_locals(bound, locals);
                body.collect_free_locals(bound, locals);
            }
            RuntimeExprSeedKind::Call { args, .. } | RuntimeExprSeedKind::PureCall { args, .. } => {
                collect_call_argument_free_locals(args, bound, locals);
            }
            RuntimeExprSeedKind::Apply { callee, args } => {
                callee.collect_free_locals(bound, locals);
                collect_call_argument_free_locals(args, bound, locals);
            }
            RuntimeExprSeedKind::TraitCall { receiver, args, .. } => {
                receiver.collect_free_locals(bound, locals);
                collect_call_argument_free_locals(args, bound, locals);
            }
            RuntimeExprSeedKind::Map {
                source,
                param,
                body,
            }
            | RuntimeExprSeedKind::Filter {
                source,
                param,
                body,
            } => {
                collect_bound_body_free_locals(source, param, body, bound, locals);
            }
            RuntimeExprSeedKind::Binary { lhs, rhs, .. } => {
                lhs.collect_free_locals(bound, locals);
                rhs.collect_free_locals(bound, locals);
            }
            RuntimeExprSeedKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.collect_free_locals(bound, locals);
                then_expr.collect_free_locals(bound, locals);
                else_expr.collect_free_locals(bound, locals);
            }
            RuntimeExprSeedKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => collect_if_let_free_locals(
                pattern,
                expr,
                guard.as_deref(),
                then_expr,
                else_expr,
                bound,
                locals,
            ),
            RuntimeExprSeedKind::Match { scrutinee, arms } => {
                collect_match_free_locals(scrutinee, arms, bound, locals);
            }
        }
    }
}

fn collect_let_free_locals(
    binding: &RuntimeLocalSeedId,
    expr: &RuntimeExprSeed,
    body: &RuntimeExprSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    expr.collect_free_locals(bound, locals);
    let mut body_bound = bound.to_vec();
    body_bound.push(binding.clone());
    body.collect_free_locals(&body_bound, locals);
}

fn collect_call_argument_free_locals(
    arguments: &[RuntimeCallArgumentSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    for argument in arguments {
        argument.value.collect_free_locals(bound, locals);
    }
}

fn collect_if_let_free_locals(
    pattern: &RuntimePatternSeed,
    expr: &RuntimeExprSeed,
    guard: Option<&RuntimeExprSeed>,
    then_expr: &RuntimeExprSeed,
    else_expr: &RuntimeExprSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    expr.collect_free_locals(bound, locals);
    collect_scoped_free_locals(pattern, guard, then_expr, bound, locals);
    else_expr.collect_free_locals(bound, locals);
}

fn collect_match_free_locals(
    scrutinee: &RuntimeExprSeed,
    arms: &[RuntimeExprMatchArmSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    scrutinee.collect_free_locals(bound, locals);
    for arm in arms {
        collect_scoped_free_locals(&arm.pattern, arm.guard.as_ref(), &arm.value, bound, locals);
    }
}

fn collect_scoped_free_locals(
    pattern: &RuntimePatternSeed,
    guard: Option<&RuntimeExprSeed>,
    value: &RuntimeExprSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    let mut scoped_bound = bound.to_vec();
    pattern.collect_binding_locals(&mut scoped_bound);
    if let Some(guard) = guard {
        guard.collect_free_locals(&scoped_bound, locals);
    }
    value.collect_free_locals(&scoped_bound, locals);
}

fn collect_range_free_locals(
    start: Option<&RuntimeExprSeed>,
    end: Option<&RuntimeExprSeed>,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    if let Some(start) = start {
        start.collect_free_locals(bound, locals);
    }
    if let Some(end) = end {
        end.collect_free_locals(bound, locals);
    }
}

fn collect_nominal_record_free_locals(
    fields: &[RuntimeNominalRecordFieldSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    for field in fields {
        field.value.collect_free_locals(bound, locals);
    }
}

fn collect_bound_body_free_locals(
    source: &RuntimeExprSeed,
    param: &RuntimeLocalSeedId,
    body: &RuntimeExprSeed,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    source.collect_free_locals(bound, locals);
    let mut body_bound = bound.to_vec();
    body_bound.push(param.clone());
    body.collect_free_locals(&body_bound, locals);
}

impl RuntimeAgentExprSeed {
    fn collect_free_locals(
        &self,
        bound: &[RuntimeLocalSeedId],
        locals: &mut Vec<RuntimeLocalSeedId>,
    ) {
        match self {
            Self::ChoiceAction { .. } | Self::CaptureViewport | Self::Diagnostics => {}
            Self::CaptureLayer { target }
            | Self::CaptureObject { target }
            | Self::StatePath { path: target }
            | Self::ObservationPath { path: target }
            | Self::ProbeSignal { target }
            | Self::ProbeMetric { target }
            | Self::ProbeState { path: target }
            | Self::ProbeObservation { path: target }
            | Self::PredicateExists { probe: target }
            | Self::PredicateActionEnabled { target }
            | Self::PredicateDiagnosticsHasError {
                diagnostics: target,
            }
            | Self::PredicateNot { predicate: target } => {
                target.collect_free_locals(bound, locals);
            }
            Self::PredicateAll { predicates } | Self::PredicateAny { predicates } => {
                collect_expr_free_locals(predicates, bound, locals);
            }
            Self::PredicateCompare { probe, value, .. }
            | Self::ViewportPoint { x: probe, y: value } => {
                probe.collect_free_locals(bound, locals);
                value.collect_free_locals(bound, locals);
            }
        }
    }
}

impl RuntimePatternSeed {
    /// Returns binding locals in canonical pattern preorder.
    ///
    /// The returned handles remain correlated with the builder that admitted
    /// the semantic local table; callers cannot manufacture plan-local IDs.
    #[must_use]
    pub fn binding_locals(&self) -> Box<[RuntimeLocalSeedId]> {
        let mut locals = Vec::new();
        self.collect_binding_locals(&mut locals);
        locals.into_boxed_slice()
    }

    fn collect_binding_locals(&self, locals: &mut Vec<RuntimeLocalSeedId>) {
        match self.kind() {
            RuntimePatternSeedKind::Bind { local, .. }
            | RuntimePatternSeedKind::Typed { local } => push_unique_local(local, locals),
            RuntimePatternSeedKind::Discard
            | RuntimePatternSeedKind::Literal(_)
            | RuntimePatternSeedKind::Entity(_) => {}
            RuntimePatternSeedKind::Tuple(items) => {
                for item in items {
                    item.collect_binding_locals(locals);
                }
            }
            RuntimePatternSeedKind::Record { fields, rest } => {
                for field in fields {
                    field.pattern.collect_binding_locals(locals);
                }
                if let RuntimePatternRestSeed::Bind(local) = rest {
                    push_unique_local(local, locals);
                }
            }
            RuntimePatternSeedKind::Sequence { items, rest } => {
                for item in items {
                    item.collect_binding_locals(locals);
                }
                if let RuntimePatternRestSeed::Bind(local) = rest {
                    push_unique_local(local, locals);
                }
            }
            RuntimePatternSeedKind::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    payload.collect_binding_locals(locals);
                }
            }
            RuntimePatternSeedKind::Whole { local, pattern } => {
                push_unique_local(local, locals);
                pattern.collect_binding_locals(locals);
            }
        }
    }
}

fn collect_expr_free_locals(
    expressions: &[RuntimeExprSeed],
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    for expression in expressions {
        expression.collect_free_locals(bound, locals);
    }
}

fn push_free_local(
    local: &RuntimeLocalSeedId,
    bound: &[RuntimeLocalSeedId],
    locals: &mut Vec<RuntimeLocalSeedId>,
) {
    if !bound.contains(local) {
        push_unique_local(local, locals);
    }
}

fn push_unique_local(local: &RuntimeLocalSeedId, locals: &mut Vec<RuntimeLocalSeedId>) {
    if !locals.contains(local) {
        locals.push(local.clone());
    }
}
