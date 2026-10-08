//! Physical slot admission from the actual owned executable grammar.

use super::RuntimeLocalDeclarationTableBuilder;
use crate::pattern::RuntimePattern;
use crate::plan::{
    FlowOp, RuntimeFunctionInputSource, RuntimeFunctionInputTransfer, RuntimeFunctionSiteBody,
    RuntimeFunctionSiteTable, RuntimeProjectCallSiteTable, RuntimePureHelper, RuntimePureHelperId,
    RuntimeTraitMethod, RuntimeTraitMethodId,
};
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimeLineTaskGroupId, RuntimeLocalDeclarationId};
use crate::value::{RuntimeExprKind, RuntimeExpressionNode as Node};

#[cfg(test)]
mod tests;

/// Coordinate in an actual executable table, never a completed body digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalOwner {
    Function(RuntimeFunctionSiteId),
    PureHelper(RuntimePureHelperId),
    TraitMethod(RuntimeTraitMethodId),
    Line {
        group: RuntimeLineTaskGroupId,
        body: RuntimeLineLocalBody,
    },
    Stream {
        ordinal: u32,
    },
}

/// The separately emitted bodies within one Line group.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLineLocalBody {
    Activation,
    Action { node: u32 },
    Cancellation { rule: u32 },
    Cleanup { exit: crate::line_task::ScopeExit },
}

/// Physical lifetime; authored retained-state policy remains source provenance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalStorage {
    InvocationFrame,
    LineFrame,
    StreamFrame,
}

/// Static first initialization, independent of subsequent move/reinitialization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeLocalInitialization {
    Input {
        source: RuntimeFunctionInputSource,
        transfer: RuntimeFunctionInputTransfer,
    },
    InputPattern {
        source: RuntimeFunctionInputSource,
        transfer: RuntimeFunctionInputTransfer,
    },
    CallableParameter {
        position: u32,
    },
    Pattern,
    ExpressionLet,
    ProjectCallResult,
    ScheduledCapture,
    MatchCandidate,
}

/// Complete immutable execution placement of one final declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeLocalPlacement {
    owner: RuntimeLocalOwner,
    initialization: RuntimeLocalInitialization,
    mutable: bool,
}

impl RuntimeLocalPlacement {
    #[must_use]
    pub const fn owner(self) -> RuntimeLocalOwner {
        self.owner
    }
    #[must_use]
    pub const fn initialization(self) -> RuntimeLocalInitialization {
        self.initialization
    }
    /// Resolves physical ingress, independently of the binding's authored
    /// provenance. Pattern destinations retain the carrier's capture source.
    pub(crate) fn function_capture_position(self, function: RuntimeFunctionSiteId) -> Option<u32> {
        if self.owner != RuntimeLocalOwner::Function(function) {
            return None;
        }
        match self.initialization {
            RuntimeLocalInitialization::Input { source, .. }
            | RuntimeLocalInitialization::InputPattern { source, .. } => match source {
                RuntimeFunctionInputSource::Capture { position }
                | RuntimeFunctionInputSource::CapturedParameter { position, .. } => Some(position),
                RuntimeFunctionInputSource::Parameter { .. } => None,
            },
            RuntimeLocalInitialization::CallableParameter { .. }
            | RuntimeLocalInitialization::Pattern
            | RuntimeLocalInitialization::ExpressionLet
            | RuntimeLocalInitialization::ProjectCallResult
            | RuntimeLocalInitialization::ScheduledCapture
            | RuntimeLocalInitialization::MatchCandidate => None,
        }
    }
    #[must_use]
    pub const fn is_mutable(self) -> bool {
        self.mutable
    }
    #[must_use]
    pub const fn storage(self) -> RuntimeLocalStorage {
        match self.owner {
            RuntimeLocalOwner::Function(_)
            | RuntimeLocalOwner::PureHelper(_)
            | RuntimeLocalOwner::TraitMethod(_) => RuntimeLocalStorage::InvocationFrame,
            RuntimeLocalOwner::Line { .. } => RuntimeLocalStorage::LineFrame,
            RuntimeLocalOwner::Stream { .. } => RuntimeLocalStorage::StreamFrame,
        }
    }
    #[cfg(test)]
    pub(super) fn test_fixture() -> Self {
        Self {
            owner: RuntimeLocalOwner::Function(RuntimeFunctionSiteId::from_accepted_ordinal(
                std::num::NonZeroU32::MIN,
            )),
            initialization: RuntimeLocalInitialization::Pattern,
            mutable: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RuntimeLocalPlacementError {
    #[error("runtime local {local} has no executable declaration owner")]
    Unowned { local: RuntimeLocalDeclarationId },
    #[error("executable declaration references unknown local {local}")]
    Unknown { local: RuntimeLocalDeclarationId },
    #[error("runtime local {local} has conflicting placements: {first:?}, {second:?}")]
    Conflict {
        local: RuntimeLocalDeclarationId,
        first: RuntimeLocalPlacement,
        second: RuntimeLocalPlacement,
    },
    #[error("local declaration references missing project call site")]
    UnknownProjectCall,
    #[error("scheduled callback references no executable child body")]
    InvalidScheduledBody,
    #[error("local owner coordinate exceeds its typed ordinal domain")]
    CoordinateOverflow,
}

impl RuntimeLocalDeclarationTableBuilder {
    fn claim(
        &mut self,
        local: RuntimeLocalDeclarationId,
        placement: RuntimeLocalPlacement,
    ) -> Result<(), RuntimeLocalPlacementError> {
        let row = self
            .declarations
            .get_mut(local.get().get() as usize - 1)
            .ok_or(RuntimeLocalPlacementError::Unknown { local })?;
        if let Some(first) = row.placement {
            if first != placement {
                return Err(RuntimeLocalPlacementError::Conflict {
                    local,
                    first,
                    second: placement,
                });
            }
        } else {
            row.placement = Some(placement);
        }
        Ok(())
    }

    fn pattern(
        &mut self,
        pattern: &RuntimePattern,
        owner: RuntimeLocalOwner,
        initialization: RuntimeLocalInitialization,
        carrier: Option<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimeLocalPlacementError> {
        for declaration in pattern.binding_declarations() {
            if Some(declaration.local()) == carrier {
                continue;
            }
            self.claim(
                declaration.local(),
                RuntimeLocalPlacement {
                    owner,
                    initialization,
                    mutable: declaration.is_mutable(),
                },
            )?;
        }
        Ok(())
    }

    fn node(
        &mut self,
        root: Node<'_>,
        owner: RuntimeLocalOwner,
        initialization: RuntimeLocalInitialization,
    ) -> Result<(), RuntimeLocalPlacementError> {
        // Pattern inventory handles Or, Whole and rest declarations exactly once.
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            match node {
                Node::Pattern(pattern) => {
                    self.pattern(pattern, owner, initialization, None)?;
                }
                Node::Expression(expression) => {
                    if let RuntimeExprKind::Let { binding, .. } = expression.kind() {
                        let mutable = self
                            .declarations
                            .get(binding.get().get() as usize - 1)
                            .and_then(|row| row.source.binding())
                            .is_some_and(super::RuntimeLocalBindingDeclaration::is_mutable);
                        self.claim(
                            *binding,
                            RuntimeLocalPlacement {
                                owner,
                                initialization: RuntimeLocalInitialization::ExpressionLet,
                                mutable,
                            },
                        )?;
                    }
                    pending.extend(node.owned_children().map(|(_, child)| child));
                }
            }
        }
        Ok(())
    }

    fn flow(
        &mut self,
        ops: &[FlowOp],
        owner: RuntimeLocalOwner,
        calls: &RuntimeProjectCallSiteTable,
        group: Option<(RuntimeLineTaskGroupId, &crate::line_task::LineTaskGroup)>,
    ) -> Result<(), RuntimeLocalPlacementError> {
        crate::plan::try_visit_ops_events(ops, &mut |event| {
            let crate::plan::RuntimeFlowTreeEvent::EnterOperation { op, .. } = event else {
                return Ok(());
            };
            if let FlowOp::ProjectCall { site } = op {
                let call = calls
                    .get(*site)
                    .ok_or(RuntimeLocalPlacementError::UnknownProjectCall)?;
                self.node(
                    Node::Expression(call.plan().callee()),
                    owner,
                    RuntimeLocalInitialization::Pattern,
                )?;
                for operand in call.plan().operands() {
                    self.node(
                        Node::Expression(operand.value()),
                        owner,
                        RuntimeLocalInitialization::Pattern,
                    )?;
                }
                self.pattern(
                    call.result(),
                    owner,
                    RuntimeLocalInitialization::ProjectCallResult,
                    None,
                )?;
            }
            if let FlowOp::LineOperation {
                operation:
                    crate::plan::RuntimeLineOperation::Schedule {
                        child, captures, ..
                    },
                ..
            } = op
                && let Some((group, line)) = group
            {
                let Some(crate::line_task::LineTaskNode::Child { scope, .. }) = line.node(*child)
                else {
                    return Err(RuntimeLocalPlacementError::InvalidScheduledBody);
                };
                for capture in captures {
                    // An existing enclosing declaration is an explicit import,
                    // not a second declaration. Closed callbacks have fresh slots.
                    if self
                        .declarations
                        .get(capture.local().get().get() as usize - 1)
                        .is_some_and(|row| row.placement.is_some())
                    {
                        continue;
                    }
                    self.claim(
                        capture.local(),
                        RuntimeLocalPlacement {
                            owner: RuntimeLocalOwner::Line {
                                group,
                                body: RuntimeLineLocalBody::Action {
                                    node: u32::try_from(scope.index()).map_err(|_| {
                                        RuntimeLocalPlacementError::CoordinateOverflow
                                    })?,
                                },
                            },
                            initialization: RuntimeLocalInitialization::ScheduledCapture,
                            mutable: false,
                        },
                    )?;
                }
            }
            if let FlowOp::Match { arms, .. } = op {
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.claim(
                            guard.candidate,
                            RuntimeLocalPlacement {
                                owner,
                                initialization: RuntimeLocalInitialization::MatchCandidate,
                                mutable: false,
                            },
                        )?;
                    }
                }
            }
            op.try_visit_value_roots(&mut |_, node| {
                self.node(node, owner, RuntimeLocalInitialization::Pattern)
            })
        })
    }

    pub(crate) fn admit_executable_placements(
        &mut self,
        functions: &RuntimeFunctionSiteTable,
        calls: &RuntimeProjectCallSiteTable,
        helpers: &[RuntimePureHelper],
        methods: &[RuntimeTraitMethod],
        lines: &[crate::line_task::LineTaskGroup],
        streams: &[crate::stream::StreamPlan],
    ) -> Result<(), RuntimeLocalPlacementError> {
        self.admit_functions(functions, calls)?;
        for helper in helpers {
            let owner = RuntimeLocalOwner::PureHelper(helper.id);
            self.callable_inputs(&helper.inputs, owner)?;
            self.node(
                Node::Expression(&helper.expr),
                owner,
                RuntimeLocalInitialization::Pattern,
            )?;
        }
        for method in methods {
            let owner = RuntimeLocalOwner::TraitMethod(method.id);
            self.callable_inputs(&method.inputs, owner)?;
            self.node(
                Node::Expression(&method.body),
                owner,
                RuntimeLocalInitialization::Pattern,
            )?;
        }
        for (index, line) in lines.iter().enumerate() {
            let group = RuntimeLineTaskGroupId::from_zero_based(index)
                .ok_or(RuntimeLocalPlacementError::CoordinateOverflow)?;
            self.flow(
                line.activation_ops(),
                RuntimeLocalOwner::Line {
                    group,
                    body: RuntimeLineLocalBody::Activation,
                },
                calls,
                Some((group, line)),
            )?;
            for (node, body) in line.nodes().iter().enumerate() {
                if let crate::line_task::LineTaskNode::Action(ops) = body {
                    self.flow(
                        ops,
                        RuntimeLocalOwner::Line {
                            group,
                            body: RuntimeLineLocalBody::Action {
                                node: u32::try_from(node)
                                    .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?,
                            },
                        },
                        calls,
                        Some((group, line)),
                    )?;
                }
            }
            for (rule, cancellation) in line.cancel_rules().iter().enumerate() {
                self.flow(
                    cancellation.action(),
                    RuntimeLocalOwner::Line {
                        group,
                        body: RuntimeLineLocalBody::Cancellation {
                            rule: u32::try_from(rule)
                                .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?,
                        },
                    },
                    calls,
                    Some((group, line)),
                )?;
            }
            for exit in [
                crate::line_task::ScopeExit::Completed,
                crate::line_task::ScopeExit::Cancelled,
                crate::line_task::ScopeExit::Failed,
            ] {
                self.flow(
                    line.cleanup().actions(exit),
                    RuntimeLocalOwner::Line {
                        group,
                        body: RuntimeLineLocalBody::Cleanup { exit },
                    },
                    calls,
                    Some((group, line)),
                )?;
            }
        }
        for (index, stream) in streams.iter().enumerate() {
            let owner = RuntimeLocalOwner::Stream {
                ordinal: u32::try_from(index)
                    .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?,
            };
            self.stream(stream.ops(), owner)?;
        }
        Ok(())
    }

    fn admit_functions(
        &mut self,
        functions: &RuntimeFunctionSiteTable,
        calls: &RuntimeProjectCallSiteTable,
    ) -> Result<(), RuntimeLocalPlacementError> {
        for (index, function) in functions.iter().enumerate() {
            let id = RuntimeFunctionSiteId::from_accepted_ordinal(
                std::num::NonZeroU32::new(
                    u32::try_from(index + 1)
                        .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?,
                )
                .ok_or(RuntimeLocalPlacementError::CoordinateOverflow)?,
            );
            let owner = RuntimeLocalOwner::Function(id);
            for input in function.inputs() {
                let mutable = input.pattern().binding_declarations().any(|declaration| {
                    declaration.local() == input.input_local() && declaration.is_mutable()
                });
                self.claim(
                    input.input_local(),
                    RuntimeLocalPlacement {
                        owner,
                        initialization: RuntimeLocalInitialization::Input {
                            source: input.source(),
                            transfer: input.transfer(),
                        },
                        mutable,
                    },
                )?;
                self.pattern(
                    input.pattern(),
                    owner,
                    RuntimeLocalInitialization::InputPattern {
                        source: input.source(),
                        transfer: input.transfer(),
                    },
                    Some(input.input_local()),
                )?;
            }
            match function.body() {
                RuntimeFunctionSiteBody::Expression(body) => self.node(
                    Node::Expression(body),
                    owner,
                    RuntimeLocalInitialization::Pattern,
                )?,
                RuntimeFunctionSiteBody::Executable(body) => {
                    self.flow(body.ops(), owner, calls, None)?;
                }
            }
        }
        Ok(())
    }

    fn callable_inputs(
        &mut self,
        inputs: &[crate::plan::RuntimeCallableParameter],
        owner: RuntimeLocalOwner,
    ) -> Result<(), RuntimeLocalPlacementError> {
        for (position, input) in inputs.iter().enumerate() {
            let mutable = self
                .declarations
                .get(input.local().get().get() as usize - 1)
                .and_then(|row| row.source.binding())
                .is_some_and(super::RuntimeLocalBindingDeclaration::is_mutable);
            self.claim(
                input.local(),
                RuntimeLocalPlacement {
                    owner,
                    initialization: RuntimeLocalInitialization::CallableParameter {
                        position: u32::try_from(position)
                            .map_err(|_| RuntimeLocalPlacementError::CoordinateOverflow)?,
                    },
                    mutable,
                },
            )?;
        }
        Ok(())
    }

    fn stream(
        &mut self,
        ops: &[crate::stream::StreamOp],
        owner: RuntimeLocalOwner,
    ) -> Result<(), RuntimeLocalPlacementError> {
        use crate::stream::StreamOp as Op;
        let mut pending = vec![ops];
        while let Some(ops) = pending.pop() {
            for op in ops {
                match op {
                    Op::Let { pattern, expr } => {
                        self.pattern(pattern, owner, RuntimeLocalInitialization::Pattern, None)?;
                        self.node(
                            Node::Expression(expr),
                            owner,
                            RuntimeLocalInitialization::Pattern,
                        )?;
                    }
                    Op::ForNext {
                        pattern,
                        source,
                        body,
                    } => {
                        self.pattern(pattern, owner, RuntimeLocalInitialization::Pattern, None)?;
                        self.node(
                            Node::Expression(source),
                            owner,
                            RuntimeLocalInitialization::Pattern,
                        )?;
                        pending.push(body);
                    }
                    Op::Yield { expr } => self.node(
                        Node::Expression(expr),
                        owner,
                        RuntimeLocalInitialization::Pattern,
                    )?,
                    Op::If {
                        condition,
                        then_ops,
                        else_ops,
                    } => {
                        self.node(
                            Node::Expression(condition),
                            owner,
                            RuntimeLocalInitialization::Pattern,
                        )?;
                        pending.push(else_ops);
                        pending.push(then_ops);
                    }
                    Op::Match { scrutinee, arms } => {
                        self.node(
                            Node::Expression(scrutinee),
                            owner,
                            RuntimeLocalInitialization::Pattern,
                        )?;
                        for arm in arms {
                            self.pattern(
                                &arm.pattern,
                                owner,
                                RuntimeLocalInitialization::Pattern,
                                None,
                            )?;
                            if let Some(guard) = &arm.guard {
                                self.node(
                                    Node::Expression(guard),
                                    owner,
                                    RuntimeLocalInitialization::Pattern,
                                )?;
                            }
                            pending.push(&arm.ops);
                        }
                    }
                    Op::Close { source } => self.node(
                        Node::Expression(source),
                        owner,
                        RuntimeLocalInitialization::Pattern,
                    )?,
                    Op::Return => {}
                }
            }
        }
        Ok(())
    }
}

impl RuntimeLocalInitialization {
    pub(crate) fn encode_semantic_initialization(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            RuntimeLocalInitialization::Input { source, transfer } => {
                encoder.tag(0);
                source.encode_semantic_source(encoder);
                transfer.encode_semantic_transfer(encoder);
            }
            RuntimeLocalInitialization::InputPattern { source, transfer } => {
                encoder.tag(1);
                source.encode_semantic_source(encoder);
                transfer.encode_semantic_transfer(encoder);
            }
            RuntimeLocalInitialization::CallableParameter { position } => {
                encoder.tag(2);
                encoder.ordinal(position);
            }
            RuntimeLocalInitialization::Pattern => encoder.tag(3),
            RuntimeLocalInitialization::ExpressionLet => encoder.tag(4),
            RuntimeLocalInitialization::ProjectCallResult => encoder.tag(5),
            RuntimeLocalInitialization::ScheduledCapture => encoder.tag(6),
            RuntimeLocalInitialization::MatchCandidate => encoder.tag(7),
        }
    }
}

impl RuntimeLineLocalBody {
    pub(crate) fn encode_semantic_local_body(
        self,
        encoder: &mut crate::task::semantic::TaskSemanticEncoder<'_>,
    ) {
        match self {
            RuntimeLineLocalBody::Activation => encoder.tag(0),
            RuntimeLineLocalBody::Action { node } => {
                encoder.tag(1);
                encoder.ordinal(node);
            }
            RuntimeLineLocalBody::Cancellation { rule } => {
                encoder.tag(2);
                encoder.ordinal(rule);
            }
            RuntimeLineLocalBody::Cleanup { exit } => {
                encoder.tag(3);
                encoder.tag(match exit {
                    crate::line_task::ScopeExit::Completed => 0,
                    crate::line_task::ScopeExit::Cancelled => 1,
                    crate::line_task::ScopeExit::Failed => 2,
                });
            }
        }
    }
}
