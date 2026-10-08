//! Count-only reachability over the same admitted callable edge grammar.
//! Missing references/cycles remain structural errors after ordered preflight.
use super::{CallableEdges, CallableNode};
use crate::plan::body_semantic::RuntimePlanInventory;
use crate::runtime_id::{RuntimeCallableSpecializationId, RuntimeCallableStateId};
use crate::value::{RuntimeExpr, RuntimeExprKind};
use std::collections::BTreeSet;

pub(in crate::plan::body_semantic) struct RuntimeCallableChildPreflight<'a> {
    plan: &'a RuntimePlanInventory,
    visited: BTreeSet<CallableNode>,
    specializations: BTreeSet<RuntimeCallableSpecializationId>,
}
impl<'a> RuntimeCallableChildPreflight<'a> {
    pub(in crate::plan::body_semantic) fn new(plan: &'a RuntimePlanInventory) -> Self {
        Self {
            plan,
            visited: BTreeSet::new(),
            specializations: BTreeSet::new(),
        }
    }
    pub(in crate::plan::body_semantic) fn state<E>(
        &mut self,
        initial: RuntimeCallableStateId,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        enum Work<'a> {
            Enter(CallableNode),
            Edges(CallableEdges<'a>),
        }
        let mut work = vec![Work::Enter(CallableNode::Definition(initial))];
        while let Some(next) = work.pop() {
            match next {
                Work::Enter(node) => {
                    if !self.visited.insert(node) {
                        continue;
                    }
                    let Some(row) = self.plan.callable_states().get(node.state()) else {
                        continue;
                    };
                    if matches!(node, CallableNode::Definition(_)) {
                        row.try_visit_semantic_child_counts(visitor)?;
                    }
                    work.push(Work::Edges(CallableEdges::new(node, row)));
                }
                Work::Edges(mut edges) => {
                    if let Some(child) = edges.next() {
                        work.push(Work::Edges(edges));
                        work.push(Work::Enter(child));
                    }
                }
            }
        }
        Ok(())
    }
    fn specialization<E>(
        &mut self,
        id: RuntimeCallableSpecializationId,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        if !self.specializations.insert(id) {
            return Ok(());
        }
        let Some(row) = self.plan.callable_specializations().get(id.index()) else {
            return Ok(());
        };
        visitor(row.arguments.types.len())?;
        visitor(row.arguments.const_lengths.len())?;
        visitor(row.arguments.effects.len())?;
        for effect in &row.arguments.effects {
            effect.try_visit_semantic_child_counts(visitor)?;
        }
        visitor(row.states.len())?;
        for state in &row.states {
            self.state(state.source, visitor)?;
            self.state(state.target, visitor)?;
        }
        Ok(())
    }
    pub(in crate::plan::body_semantic) fn expression<E>(
        &mut self,
        expression: &RuntimeExpr,
        visitor: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(expression.guard_copy_locals().len())?;
        match expression.kind() {
            RuntimeExprKind::MakeCallable { state, .. } => self.state(*state, visitor)?,
            RuntimeExprKind::SpecializeCallable { specialization, .. } => {
                self.specialization(*specialization, visitor)?;
            }
            RuntimeExprKind::DialogueContent { effects, .. } => {
                visitor(effects.len())?;
                for effect in effects {
                    self.state(effect.state, visitor)?;
                    visitor(effect.captures.len())?;
                }
            }
            // Owned expression operands are counted by the shared node visitor.
            // This pass follows callable references; other metadata is separate.
            RuntimeExprKind::Value(_)
            | RuntimeExprKind::Agent(_)
            | RuntimeExprKind::Local(_)
            | RuntimeExprKind::SequencePopFront { .. }
            | RuntimeExprKind::SequencePush { .. }
            | RuntimeExprKind::SequencePopBack { .. }
            | RuntimeExprKind::EntityRef(_)
            | RuntimeExprKind::Let { .. }
            | RuntimeExprKind::Scope { .. }
            | RuntimeExprKind::Tuple(_)
            | RuntimeExprKind::FormatContent { .. }
            | RuntimeExprKind::CharacterDialogue { .. }
            | RuntimeExprKind::BracketSeq(_)
            | RuntimeExprKind::RepeatSeq { .. }
            | RuntimeExprKind::Range { .. }
            | RuntimeExprKind::NominalRecord(_)
            | RuntimeExprKind::Variant { .. }
            | RuntimeExprKind::Field { .. }
            | RuntimeExprKind::ProjectTuple { .. }
            | RuntimeExprKind::ProjectRecord { .. }
            | RuntimeExprKind::Assign { .. }
            | RuntimeExprKind::Call { .. }
            | RuntimeExprKind::ApplyGroup { .. }
            | RuntimeExprKind::TraitCall { .. }
            | RuntimeExprKind::PureCall { .. }
            | RuntimeExprKind::StandardMap { .. }
            | RuntimeExprKind::Sum { .. }
            | RuntimeExprKind::Unary { .. }
            | RuntimeExprKind::Binary { .. }
            | RuntimeExprKind::If { .. }
            | RuntimeExprKind::IfLet { .. }
            | RuntimeExprKind::Match { .. }
            | RuntimeExprKind::ReductionUnchanged { .. } => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
