//! Memoized typed callable graph transcripts. Allocation coordinates are only
//! private memo keys; accepted code/type references and ordered child digests
//! supply the bytes. One borrowed graph and one meter own the whole traversal.

pub(super) mod preflight;

use super::{RuntimeBodySemanticContext, RuntimeBodySemanticError};
use crate::runtime_id::RuntimeCallableStateId;
use crate::task::semantic::TaskSemanticEncoder;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum CallableNode {
    Definition(RuntimeCallableStateId),
    Origin(RuntimeCallableStateId),
}
impl CallableNode {
    const fn state(self) -> RuntimeCallableStateId {
        match self {
            Self::Definition(state) | Self::Origin(state) => state,
        }
    }
    const fn domain(self) -> &'static [u8] {
        match self {
            Self::Definition(_) => b"arcweft.runtime-plan.callable-state.v1\0",
            Self::Origin(_) => b"arcweft.runtime-plan.callable-origin.v1\0",
        }
    }
}
#[derive(Clone, Copy)]
enum Memo {
    Visiting,
    Done(blake3::Hash),
}

/// Edges borrow the sole admitted row and yield in semantic source order.
struct CallableEdges<'a> {
    node: CallableNode,
    row: &'a crate::plan::RuntimeCallableState,
    origin: bool,
    transition: bool,
    partials: std::slice::Iter<'a, crate::plan::RuntimeCallablePartialTransition>,
}
impl<'a> CallableEdges<'a> {
    fn new(node: CallableNode, row: &'a crate::plan::RuntimeCallableState) -> Self {
        Self {
            node,
            row,
            origin: false,
            transition: false,
            partials: row.partials.iter(),
        }
    }
}
impl Iterator for CallableEdges<'_> {
    type Item = CallableNode;
    fn next(&mut self) -> Option<Self::Item> {
        if matches!(self.node, CallableNode::Definition(_)) && !self.origin {
            self.origin = true;
            return Some(CallableNode::Origin(self.row.origin));
        }
        if !self.transition {
            self.transition = true;
            if let crate::plan::RuntimeCallableTransition::Retain { state, .. } =
                &self.row.transition
            {
                return Some(match self.node {
                    CallableNode::Definition(_) => CallableNode::Definition(*state),
                    CallableNode::Origin(_) => CallableNode::Origin(*state),
                });
            }
        }
        match self.node {
            CallableNode::Definition(_) => self
                .partials
                .next()
                .map(|partial| CallableNode::Definition(partial.state)),
            CallableNode::Origin(_) => None,
        }
    }
}

impl RuntimeBodySemanticContext<'_> {
    pub(crate) fn write_callable_state(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        initial: RuntimeCallableStateId,
    ) -> Result<(), RuntimeBodySemanticError> {
        enum Work<'a> {
            Enter(CallableNode),
            Children(CallableEdges<'a>),
            Finish(CallableNode),
        }
        let root = CallableNode::Definition(initial);
        let mut stack = vec![Work::Enter(root)];
        let mut memo = BTreeMap::new();
        while let Some(work) = stack.pop() {
            encoder.status()?;
            encoder.enter_element();
            encoder.status()?;
            match work {
                Work::Enter(node) => {
                    match memo.get(&node) {
                        Some(Memo::Done(_)) => continue,
                        Some(Memo::Visiting) => {
                            encoder.reject_owner();
                            return Err(RuntimeBodySemanticError::CallableCycle {
                                state: node.state().index(),
                            });
                        }
                        None => {}
                    }
                    let row = self
                        .plan
                        .callable_states()
                        .get(node.state())
                        .ok_or_else(|| {
                            encoder.reject_owner();
                            RuntimeBodySemanticError::MissingRow {
                                table: "callable states",
                                ordinal: node.state().index(),
                            }
                        })?;
                    memo.insert(node, Memo::Visiting);
                    stack.push(Work::Finish(node));
                    stack.push(Work::Children(CallableEdges::new(node, row)));
                }
                Work::Children(mut children) => {
                    if let Some(child) = children.next() {
                        stack.push(Work::Children(children));
                        stack.push(Work::Enter(child));
                    }
                }
                Work::Finish(node) => {
                    let row = self
                        .plan
                        .callable_states()
                        .get(node.state())
                        .expect("Enter resolved the immutable row");
                    let digest = encoder.child_digest(node.domain(), |child| match node {
                        CallableNode::Definition(_) => {
                            self.write_callable_definition(child, row, &memo)
                        }
                        CallableNode::Origin(_) => self.write_callable_origin(child, row, &memo),
                    })?;
                    memo.insert(node, Memo::Done(digest));
                }
            }
        }
        Self::write_callable_child(encoder, &memo, root)
    }

    fn write_callable_child(
        encoder: &mut TaskSemanticEncoder<'_>,
        memo: &BTreeMap<CallableNode, Memo>,
        node: CallableNode,
    ) -> Result<(), RuntimeBodySemanticError> {
        let Some(Memo::Done(digest)) = memo.get(&node) else {
            encoder.reject_owner();
            return Err(RuntimeBodySemanticError::CallableCycle {
                state: node.state().index(),
            });
        };
        encoder.digest(digest.as_bytes());
        encoder.status().map_err(Into::into)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive admitted callable metadata algebra, with memoized child edges"
    )]
    fn write_callable_definition(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        row: &crate::plan::RuntimeCallableState,
        memo: &BTreeMap<CallableNode, Memo>,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::plan::{
            RuntimeCallableAttachedContract as Attached, RuntimeCallableDefault as Default,
            RuntimeCallableParameterKind as Kind, RuntimeCallablePosition as Position,
            RuntimeCallableRetainedRole as Retained, RuntimeCallableTransition as Transition,
        };
        Self::write_callable_child(encoder, memo, CallableNode::Origin(row.origin))?;
        self.write_type(encoder, row.function_type)?;
        self.write_type(encoder, row.result)?;
        match &row.position {
            Position::Unapplied => encoder.tag(0),
            Position::WithinGroup { group, bound } => {
                encoder.tag(1);
                encoder.ordinal(*group);
                encoder.count(bound.len());
                for coordinate in bound {
                    encoder.enter_element();
                    encoder.ordinal(coordinate.group);
                    encoder.ordinal(coordinate.parameter);
                }
            }
            Position::AfterGroup { completed } => {
                encoder.tag(2);
                encoder.ordinal(*completed);
            }
        }
        encoder.count(row.retained.len());
        for retained in &row.retained {
            encoder.enter_element();
            match retained.role {
                Retained::Capture { position } => {
                    encoder.tag(0);
                    encoder.ordinal(position);
                }
                Retained::Parameter(coordinate) => {
                    encoder.tag(1);
                    encoder.ordinal(coordinate.group);
                    encoder.ordinal(coordinate.parameter);
                }
            }
            self.write_type(encoder, retained.ty)?;
        }
        encoder.count(row.parameters.len());
        for parameter in &row.parameters {
            encoder.enter_element();
            encoder.ordinal(parameter.coordinate.group);
            encoder.ordinal(parameter.coordinate.parameter);
            encoder.tag(match parameter.kind {
                Kind::Fixed => 0,
                Kind::Rest => 1,
            });
            self.write_type(encoder, parameter.abi_ty)?;
            self.write_type(encoder, parameter.binding_ty)?;
        }
        match &row.attached {
            Attached::None => encoder.tag(0),
            Attached::Required { ty } => {
                encoder.tag(1);
                self.write_type(encoder, *ty)?;
            }
            Attached::Optional { value, binding } => {
                encoder.tag(2);
                self.write_type(encoder, *value)?;
                self.write_type(encoder, *binding)?;
            }
            Attached::Defaulted { ty, default } => {
                encoder.tag(3);
                self.write_type(encoder, *ty)?;
                match default {
                    Default::RequiresSpecialization => encoder.tag(0),
                    Default::Body { function, captures } => {
                        encoder.tag(1);
                        self.write_function_reference(encoder, *function)?;
                        Self::write_callable_inputs(encoder, captures);
                    }
                }
            }
        }
        match &row.transition {
            Transition::RequiresSpecialization => encoder.tag(0),
            Transition::Retain { state, values } => {
                encoder.tag(1);
                Self::write_callable_inputs(encoder, values);
                Self::write_callable_child(encoder, memo, CallableNode::Definition(*state))?;
            }
            Transition::Invoke {
                function,
                captures,
                arguments,
            } => {
                encoder.tag(2);
                self.write_function_reference(encoder, *function)?;
                Self::write_callable_inputs(encoder, captures);
                Self::write_callable_inputs(encoder, arguments);
            }
        }
        encoder.count(row.partials.len());
        for partial in &row.partials {
            encoder.enter_element();
            encoder.count(partial.parameters.len());
            for coordinate in &partial.parameters {
                encoder.enter_element();
                encoder.ordinal(coordinate.group);
                encoder.ordinal(coordinate.parameter);
            }
            Self::write_callable_inputs(encoder, &partial.values);
            Self::write_callable_child(encoder, memo, CallableNode::Definition(partial.state))?;
        }

        encoder.status().map_err(Into::into)
    }

    /// Origin is an accepted root reference, with code definitions as leaves.
    /// A Retain chain shares its computed suffix and still rejects cycles.
    fn write_callable_origin(
        &self,
        encoder: &mut TaskSemanticEncoder<'_>,
        row: &crate::plan::RuntimeCallableState,
        memo: &BTreeMap<CallableNode, Memo>,
    ) -> Result<(), RuntimeBodySemanticError> {
        use crate::plan::RuntimeCallableTransition as Transition;
        self.write_type(encoder, row.function_type)?;
        match &row.transition {
            Transition::RequiresSpecialization => encoder.tag(0),
            Transition::Retain { state, .. } => {
                encoder.tag(1);
                Self::write_callable_child(encoder, memo, CallableNode::Origin(*state))?;
            }
            Transition::Invoke { function, .. } => {
                encoder.tag(2);
                self.write_function_reference(encoder, *function)?;
            }
        }
        encoder.status().map_err(Into::into)
    }

    fn write_callable_inputs(
        encoder: &mut TaskSemanticEncoder<'_>,
        inputs: &[crate::plan::RuntimeCallableInputSource],
    ) {
        encoder.count(inputs.len());
        for input in inputs {
            encoder.enter_element();
            match input {
                crate::plan::RuntimeCallableInputSource::Retained { position } => {
                    encoder.tag(0);
                    encoder.ordinal(*position);
                }
                crate::plan::RuntimeCallableInputSource::Argument { position } => {
                    encoder.tag(1);
                    encoder.ordinal(*position);
                }
                crate::plan::RuntimeCallableInputSource::Attached => encoder.tag(2),
            }
        }
    }
}
