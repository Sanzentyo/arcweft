use crate::effect::LineEffectRequest;
use crate::pattern::RuntimePattern;
use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use crate::value::{RuntimeBinding, RuntimeExpr};

/// Typed AOT compilation artifact for a runtime plan.
///
/// The artifact is pure data: it records dispatch-shape analysis and owns
/// pre-lowered operation blocks used by the AOT executor's linear fast path.
#[derive(Clone, Debug, PartialEq)]
pub struct AotProgram {
    flows: Vec<AotFlowBlock>,
    stats: AotProgramStats,
}

/// Per-flow dispatch shape emitted by AOT lowering.
#[derive(Clone, Debug, PartialEq)]
pub struct AotFlowBlock {
    pub id: FlowRuntimeId,
    pub ops: usize,
    pub linear_prefix_ops: usize,
    pub dispatch: AotDispatchShape,
    linear_ops: Vec<AotLinearOp>,
}

/// Dispatch shape for a lowered flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AotDispatchShape {
    Linear,
    Mixed,
}

/// Runtime operation class used by the AOT planner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AotOpClass {
    Linear,
    Branch,
    Effect,
    Await,
    Choice,
    Dialogue,
    Jump,
}

/// Linear operation lowered into an `AotProgram`.
///
/// The AOT executor borrows these pre-lowered operations at runtime instead of
/// cloning `FlowOp` values from the semantic runtime plan on every fast-path
/// step.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AotLinearOp {
    Bind(Vec<RuntimeBinding>),
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    Return(String),
    ReturnExpr(RuntimeExpr),
    Effect(LineEffectRequest),
    EnterScope,
    ExitScope,
    ExitScopeBind {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },
    Noop,
}

impl AotLinearOp {
    fn from_flow_op(op: &FlowOp) -> Option<Self> {
        if !aot_linear_supported_op(op) {
            return None;
        }
        match op {
            FlowOp::Bind(bindings) => Some(Self::Bind(bindings.clone())),
            FlowOp::Let { pattern, expr } => Some(Self::Let {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
            FlowOp::Return(value) => Some(Self::Return(value.clone())),
            FlowOp::ReturnExpr(expr) => Some(Self::ReturnExpr(expr.clone())),
            FlowOp::Effect(effect) => Some(Self::Effect(effect.clone())),
            FlowOp::EnterScope => Some(Self::EnterScope),
            FlowOp::ExitScope => Some(Self::ExitScope),
            FlowOp::ExitScopeBind { pattern, expr } => Some(Self::ExitScopeBind {
                pattern: pattern.clone(),
                expr: expr.clone(),
            }),
            FlowOp::Noop => Some(Self::Noop),
            FlowOp::LetElse { .. }
            | FlowOp::Dialogue { .. }
            | FlowOp::Choice { .. }
            | FlowOp::Await { .. }
            | FlowOp::AwaitMany { .. }
            | FlowOp::HostCall { .. }
            | FlowOp::If { .. }
            | FlowOp::IfLet { .. }
            | FlowOp::Match { .. }
            | FlowOp::Loop { .. }
            | FlowOp::LetLoop { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::While { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLet { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::For { .. }
            | FlowOp::ForNext { .. }
            | FlowOp::Thread { .. }
            | FlowOp::Scope(_)
            | FlowOp::LetScope { .. }
            | FlowOp::Break(_)
            | FlowOp::Continue
            | FlowOp::Goto(_)
            | FlowOp::GotoExpr(_) => None,
        }
    }
}

/// Deterministic AOT shape counters for compiler and runtime profiling.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AotProgramStats {
    pub flows: usize,
    pub ops: usize,
    pub linear_ops: usize,
    pub branch_ops: usize,
    pub effect_ops: usize,
    pub await_ops: usize,
    pub choice_ops: usize,
    pub dialogue_ops: usize,
    pub jump_ops: usize,
    pub linear_dispatch_flows: usize,
    pub mixed_dispatch_flows: usize,
}

impl AotProgram {
    pub fn from_runtime_plan(plan: &RuntimePlan) -> Self {
        let flows: Vec<_> = plan
            .flows
            .iter()
            .map(AotFlowBlock::from_runtime_flow)
            .collect();
        let mut stats = flows
            .iter()
            .fold(AotProgramStats::default(), |mut stats, flow| {
                stats.flows += 1;
                match flow.dispatch {
                    AotDispatchShape::Linear => stats.linear_dispatch_flows += 1,
                    AotDispatchShape::Mixed => stats.mixed_dispatch_flows += 1,
                }
                stats
            });
        for flow in &plan.flows {
            stats.record_ops(&flow.ops);
        }
        Self { flows, stats }
    }

    pub fn flows(&self) -> &[AotFlowBlock] {
        &self.flows
    }

    pub fn flow_block(&self, index: usize) -> Option<&AotFlowBlock> {
        self.flows.get(index)
    }

    pub const fn stats(&self) -> &AotProgramStats {
        &self.stats
    }
}

impl AotFlowBlock {
    fn from_runtime_flow(flow: &RuntimeFlow) -> Self {
        let linear_ops = flow
            .ops
            .iter()
            .take_while(|op| aot_linear_supported_op(op))
            .filter_map(AotLinearOp::from_flow_op)
            .collect::<Vec<_>>();
        let linear_prefix_ops = linear_ops.len();
        let dispatch = if linear_prefix_ops == flow.ops.len() {
            AotDispatchShape::Linear
        } else {
            AotDispatchShape::Mixed
        };
        Self {
            id: flow.id.clone(),
            ops: flow.ops.len(),
            linear_prefix_ops,
            dispatch,
            linear_ops,
        }
    }

    pub(crate) fn linear_op(&self, index: usize) -> Option<&AotLinearOp> {
        self.linear_ops.get(index)
    }

    pub fn lowered_linear_ops(&self) -> usize {
        self.linear_ops.len()
    }
}

pub(crate) fn aot_linear_supported_op(op: &FlowOp) -> bool {
    match op {
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => true,
        FlowOp::Effect(effect) => !effect_changes_control(effect),
        FlowOp::LetElse { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Await { .. }
        | FlowOp::AwaitMany { .. }
        | FlowOp::HostCall { .. }
        | FlowOp::If { .. }
        | FlowOp::IfLet { .. }
        | FlowOp::Match { .. }
        | FlowOp::Loop { .. }
        | FlowOp::LetLoop { .. }
        | FlowOp::LoopNext { .. }
        | FlowOp::While { .. }
        | FlowOp::WhileNext { .. }
        | FlowOp::WhileLet { .. }
        | FlowOp::WhileLetNext { .. }
        | FlowOp::For { .. }
        | FlowOp::ForNext { .. }
        | FlowOp::Thread { .. }
        | FlowOp::Scope(_)
        | FlowOp::LetScope { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_) => false,
    }
}

pub(crate) fn effect_changes_control(effect: &LineEffectRequest) -> bool {
    matches!(
        effect,
        LineEffectRequest::Return(_)
            | LineEffectRequest::Goto(_)
            | LineEffectRequest::Panic(_)
            | LineEffectRequest::Fail(_)
            | LineEffectRequest::Bail(_)
            | LineEffectRequest::Break { .. }
            | LineEffectRequest::Continue { .. }
    )
}

impl AotOpClass {
    fn from_flow_op(op: &FlowOp) -> Self {
        match op {
            FlowOp::Bind(_)
            | FlowOp::Let { .. }
            | FlowOp::LetScope { .. }
            | FlowOp::Return(_)
            | FlowOp::ReturnExpr(_)
            | FlowOp::EnterScope
            | FlowOp::ExitScope
            | FlowOp::ExitScopeBind { .. }
            | FlowOp::Noop => Self::Linear,
            FlowOp::LetElse { .. }
            | FlowOp::If { .. }
            | FlowOp::IfLet { .. }
            | FlowOp::Match { .. }
            | FlowOp::Loop { .. }
            | FlowOp::LetLoop { .. }
            | FlowOp::LoopNext { .. }
            | FlowOp::While { .. }
            | FlowOp::WhileNext { .. }
            | FlowOp::WhileLet { .. }
            | FlowOp::WhileLetNext { .. }
            | FlowOp::For { .. }
            | FlowOp::ForNext { .. }
            | FlowOp::Thread { .. }
            | FlowOp::Scope(_) => Self::Branch,
            FlowOp::Effect(_) => Self::Effect,
            FlowOp::Await { .. } | FlowOp::AwaitMany { .. } | FlowOp::HostCall { .. } => {
                Self::Await
            }
            FlowOp::Choice { .. } => Self::Choice,
            FlowOp::Dialogue { .. } => Self::Dialogue,
            FlowOp::Break(_) | FlowOp::Continue | FlowOp::Goto(_) | FlowOp::GotoExpr(_) => {
                Self::Jump
            }
        }
    }
}

impl AotProgramStats {
    fn record_ops(&mut self, ops: &[FlowOp]) {
        for op in ops {
            self.record_op(op);
            match op {
                FlowOp::LetElse { else_ops, .. } => self.record_ops(else_ops),
                FlowOp::If {
                    then_ops, else_ops, ..
                }
                | FlowOp::IfLet {
                    then_ops, else_ops, ..
                } => {
                    self.record_ops(then_ops);
                    self.record_ops(else_ops);
                }
                FlowOp::Match { arms, .. } => {
                    for arm in arms {
                        self.record_ops(&arm.ops);
                    }
                }
                FlowOp::Loop { body }
                | FlowOp::LetLoop { body, .. }
                | FlowOp::While { body, .. }
                | FlowOp::WhileLet { body, .. }
                | FlowOp::For { body, .. }
                | FlowOp::Thread { body, .. } => self.record_ops(body),
                FlowOp::LoopNext { body }
                | FlowOp::WhileNext { body, .. }
                | FlowOp::WhileLetNext { body, .. }
                | FlowOp::ForNext { body, .. } => self.record_ops(body),
                FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => self.record_ops(ops),
                FlowOp::Bind(_)
                | FlowOp::Let { .. }
                | FlowOp::Dialogue { .. }
                | FlowOp::Choice { .. }
                | FlowOp::Await { .. }
                | FlowOp::AwaitMany { .. }
                | FlowOp::HostCall { .. }
                | FlowOp::Break(_)
                | FlowOp::Continue
                | FlowOp::Goto(_)
                | FlowOp::GotoExpr(_)
                | FlowOp::Return(_)
                | FlowOp::ReturnExpr(_)
                | FlowOp::Effect(_)
                | FlowOp::EnterScope
                | FlowOp::ExitScope
                | FlowOp::ExitScopeBind { .. }
                | FlowOp::Noop => {}
            }
        }
    }

    fn record_op(&mut self, op: &FlowOp) {
        self.ops += 1;
        match AotOpClass::from_flow_op(op) {
            AotOpClass::Linear => self.linear_ops += 1,
            AotOpClass::Branch => self.branch_ops += 1,
            AotOpClass::Effect => self.effect_ops += 1,
            AotOpClass::Await => self.await_ops += 1,
            AotOpClass::Choice => self.choice_ops += 1,
            AotOpClass::Dialogue => self.dialogue_ops += 1,
            AotOpClass::Jump => self.jump_ops += 1,
        }
    }
}
