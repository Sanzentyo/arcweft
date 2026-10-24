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
    array_construction: Option<RuntimePlanTypeId>,
}

impl<'a> RuntimeSequenceSum<'a> {
    pub(crate) fn project(
        accumulator: RuntimeLocalDeclarationId,
        mut sequence_type: RuntimePlanTypeId,
        operations: impl IntoIterator<Item = &'a FlowOp>,
    ) -> Option<Self> {
        let mut source = accumulator;
        let mut aliases = Vec::new();
        let mut array_construction = None;
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
                    array_construction,
                });
            }
            let pattern = pattern?;
            let input = if let RuntimeExprKind::ArrayFromVec { value } = expression.kind() {
                if array_construction.replace(expression.ty()).is_some() {
                    return None;
                }
                value.as_ref()
            } else {
                expression
            };
            if read_local(input) != Some(source)
                || input.ty() != sequence_type
                || pattern.ty() != expression.ty()
            {
                return None;
            }
            let next = bound_local(pattern)?;
            if !seen.insert(next) {
                return None;
            }
            aliases.push(next);
            source = next;
            sequence_type = expression.ty();
        }
        None
    }

    /// A fused reduction may elide exact Array construction only when its
    /// complete numeric output has the same item identity and closed length.
    /// Open extents retain their ordinary checked construction operation.
    pub(crate) fn admits_array_construction(
        &self,
        plan: &super::RuntimePlan,
        item_type: RuntimePlanTypeId,
        items: usize,
    ) -> bool {
        self.array_construction.is_none_or(|ty| {
            matches!(plan.type_table().get(ty).map(|row| row.projection()),
                Some(super::RuntimePlanTypeProjection::Array { item, length })
                    if *item == item_type && length.constant() == u64::try_from(items).ok())
        })
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
