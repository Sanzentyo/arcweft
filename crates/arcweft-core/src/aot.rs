use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};

/// Typed AOT compilation artifact for a runtime plan.
///
/// The artifact is pure data: it preserves the validated runtime plan and
/// records dispatch-shape analysis that hosts can profile before generated
/// dispatch replaces the VM-compatible executor backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AotProgram {
    plan: RuntimePlan,
    flows: Vec<AotFlowBlock>,
    stats: AotProgramStats,
}

/// Per-flow dispatch shape emitted by AOT lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AotFlowBlock {
    pub id: FlowRuntimeId,
    pub ops: usize,
    pub linear_prefix_ops: usize,
    pub dispatch: AotDispatchShape,
}

/// Dispatch shape for a lowered flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AotDispatchShape {
    Linear,
    Mixed,
}

/// Runtime operation class used by the AOT planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AotOpClass {
    Linear,
    Branch,
    Effect,
    Await,
    Choice,
    Dialogue,
    Jump,
}

/// Deterministic AOT shape counters for compiler and runtime profiling.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
    pub fn from_runtime_plan(plan: RuntimePlan) -> Self {
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
        Self { plan, flows, stats }
    }

    pub const fn plan(&self) -> &RuntimePlan {
        &self.plan
    }

    pub fn flows(&self) -> &[AotFlowBlock] {
        &self.flows
    }

    pub const fn stats(&self) -> &AotProgramStats {
        &self.stats
    }

    pub fn into_runtime_plan(self) -> RuntimePlan {
        self.plan
    }
}

impl From<RuntimePlan> for AotProgram {
    fn from(plan: RuntimePlan) -> Self {
        Self::from_runtime_plan(plan)
    }
}

impl AotFlowBlock {
    fn from_runtime_flow(flow: &RuntimeFlow) -> Self {
        let linear_prefix_ops = flow
            .ops
            .iter()
            .take_while(|op| Self::op_is_linear_dispatch(op))
            .count();
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
        }
    }

    fn op_is_linear_dispatch(op: &FlowOp) -> bool {
        match op {
            FlowOp::Bind(_)
            | FlowOp::Let { .. }
            | FlowOp::Dialogue { .. }
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
            | FlowOp::Noop => true,
            FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => {
                ops.iter().all(Self::op_is_linear_dispatch)
            }
            FlowOp::LetElse { .. }
            | FlowOp::Choice { .. }
            | FlowOp::Await { .. }
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
            | FlowOp::Thread { .. } => false,
        }
    }
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
            | FlowOp::Thread { .. }
            | FlowOp::Scope(_) => Self::Branch,
            FlowOp::Effect(_) => Self::Effect,
            FlowOp::Await { .. } => Self::Await,
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
                | FlowOp::LoopNext { body }
                | FlowOp::While { body, .. }
                | FlowOp::WhileNext { body, .. }
                | FlowOp::WhileLet { body, .. }
                | FlowOp::WhileLetNext { body, .. }
                | FlowOp::For { body, .. }
                | FlowOp::Thread { body, .. } => self.record_ops(body),
                FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => self.record_ops(ops),
                FlowOp::Bind(_)
                | FlowOp::Let { .. }
                | FlowOp::Dialogue { .. }
                | FlowOp::Choice { .. }
                | FlowOp::Await { .. }
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
