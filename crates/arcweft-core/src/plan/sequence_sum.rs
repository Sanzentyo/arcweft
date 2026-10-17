//! A consuming Sum and its source-ordered local aliases in admitted operations.

use super::FlowOp;
use crate::pattern::{RuntimePattern, RuntimePatternKind};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::value::{RuntimeExpr, RuntimeExprKind, RuntimeLocalReadMode};

#[derive(Clone, Copy)]
pub(crate) enum RuntimeSequenceSumTarget<'a> {
    Bind(&'a RuntimePattern),
    Return,
}

/// Borrowed projection of the actual operations; no independent schedule is kept.
pub(crate) struct RuntimeSequenceSum<'a> {
    pub(crate) skipped_ops: usize,
    pub(crate) aliases: Vec<RuntimeLocalDeclarationId>,
    pub(crate) target: RuntimeSequenceSumTarget<'a>,
    pub(crate) elided: Vec<RuntimeLocalDeclarationId>,
    pub(crate) result_type: RuntimePlanTypeId,
}

impl<'a> RuntimeSequenceSum<'a> {
    pub(crate) fn project(
        accumulator: RuntimeLocalDeclarationId,
        sequence_type: RuntimePlanTypeId,
        operations: impl IntoIterator<Item = &'a FlowOp>,
    ) -> Option<Self> {
        let mut source = accumulator;
        let mut aliases = Vec::new();
        let mut seen = std::collections::BTreeSet::from([accumulator]);
        for (index, operation) in operations.into_iter().enumerate() {
            let (pattern, expression) = match operation {
                FlowOp::Let { pattern, expr } if pattern.ty() == expr.ty() => (Some(pattern), expr),
                FlowOp::ReturnExpr(expr) => (None, expr),
                _ => return None,
            };
            if let RuntimeExprKind::Sum { source: receiver } = expression.kind() {
                if read_local(receiver) != Some(source)
                    || receiver.ty() != sequence_type
                    || pattern.is_some_and(|pattern| bound_local(pattern).is_none())
                {
                    return None;
                }
                return Some(Self {
                    skipped_ops: index + 1,
                    elided: std::iter::once(accumulator)
                        .chain(aliases.iter().copied())
                        .collect(),
                    aliases,
                    target: pattern.map_or(
                        RuntimeSequenceSumTarget::Return,
                        RuntimeSequenceSumTarget::Bind,
                    ),
                    result_type: expression.ty(),
                });
            }
            let pattern = pattern?;
            if read_local(expression) != Some(source)
                || expression.ty() != sequence_type
                || pattern.ty() != sequence_type
            {
                return None;
            }
            let next = bound_local(pattern)?;
            if !seen.insert(next) {
                return None;
            }
            aliases.push(next);
            source = next;
        }
        None
    }
}

fn read_local(expression: &RuntimeExpr) -> Option<RuntimeLocalDeclarationId> {
    let RuntimeExprKind::Local(read) = expression.kind() else {
        return None;
    };
    (matches!(
        read.mode(),
        RuntimeLocalReadMode::Move | RuntimeLocalReadMode::Copy
    ) && read.fields().is_empty())
    .then_some(read.local())
}

fn bound_local(pattern: &RuntimePattern) -> Option<RuntimeLocalDeclarationId> {
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            Some(binding.local())
        }
        _ => None,
    }
}
