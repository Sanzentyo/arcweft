//! Attached Match arm ownership and source-component validation.

use arcweft_source::{SourceRange, SourceSpan};

use super::AttachedExpressionNode;
use super::structure::component_matches_semantic_child;
use crate::attachment::family::{
    ExprNode, ExpressionFamily, FamilyNode, FamilySpec, PatternFamily, RecoveryFamily, RecoveryNode,
};
use crate::attachment::node::{AstNode, MatchArmKind};
use crate::attachment::{
    AttachedPatternNode, SyntaxAccessError, SyntaxNodeHandle, SyntaxNodeId, SyntaxSnapshotId,
};
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    SyntaxExpressionSlot, SyntaxMatchArmPart, SyntaxMatchArmProjection,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};

/// One exact source component owned by an attached Match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMatchArmComponent {
    part: SyntaxMatchArmPart,
    source: SourceSpan,
}

impl AttachedMatchArmComponent {
    pub const fn part(&self) -> SyntaxMatchArmPart {
        self.part
    }

    pub const fn source_span(&self) -> &SourceSpan {
        &self.source
    }
}

/// Authored expression or exact missing-expression recovery owned by a Match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedMatchArmExpression {
    Authored {
        expression: ExprNode,
        source: SourceSpan,
    },
    Missing {
        recovery: RecoveryNode,
    },
}

impl AttachedMatchArmExpression {
    pub fn source_span(&self) -> SourceSpan {
        match self {
            Self::Authored { source, .. } => source.clone(),
            Self::Missing { recovery } => recovery.source_span(),
        }
    }

    pub const fn authored(&self) -> Option<&ExprNode> {
        match self {
            Self::Authored { expression, .. } => Some(expression),
            Self::Missing { .. } => None,
        }
    }

    pub const fn missing(&self) -> Option<&RecoveryNode> {
        match self {
            Self::Missing { recovery } => Some(recovery),
            Self::Authored { .. } => None,
        }
    }

    pub fn authored_semantic(&self) -> Result<Option<AttachedExpressionNode>, SyntaxAccessError> {
        match self {
            Self::Authored { expression, .. } => {
                AttachedExpressionNode::from_syntax(expression.syntax()).map(Some)
            }
            Self::Missing { .. } => Ok(None),
        }
    }
}

/// One source-ordered Match arm bound to its attached Pattern and expression identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedMatchArm {
    syntax: AstNode<MatchArmKind>,
    projection: SyntaxMatchArmProjection,
    pattern: AttachedPatternNode,
    guard: Option<AttachedMatchArmExpression>,
    value: AttachedMatchArmExpression,
    components: Box<[AttachedMatchArmComponent]>,
}

impl AttachedMatchArm {
    pub fn id(&self) -> SyntaxNodeId {
        self.syntax.id()
    }

    pub fn snapshot_id(&self) -> &SyntaxSnapshotId {
        self.syntax.snapshot_id()
    }

    pub const fn syntax(&self) -> &AstNode<MatchArmKind> {
        &self.syntax
    }

    pub fn whole_source_span(&self) -> SourceSpan {
        self.syntax.source_span()
    }

    pub const fn projection(&self) -> &SyntaxMatchArmProjection {
        &self.projection
    }

    pub const fn pattern(&self) -> &AttachedPatternNode {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&AttachedMatchArmExpression> {
        self.guard.as_ref()
    }

    pub const fn value(&self) -> &AttachedMatchArmExpression {
        &self.value
    }

    pub fn component(&self, part: SyntaxMatchArmPart) -> Option<SourceSpan> {
        self.components
            .iter()
            .find(|component| component.part == part)
            .map(|component| component.source.clone())
    }

    pub fn components(&self) -> &[AttachedMatchArmComponent] {
        &self.components
    }
}

pub(super) fn attached_match_arms(
    syntax: &SyntaxNodeHandle,
    projection: &ExpressionProjection,
    components: &[PendingExpressionComponent],
) -> Result<Box<[AttachedMatchArm]>, SyntaxAccessError> {
    let ExpressionProjection::Match(projection) = projection else {
        return Ok(Box::new([]));
    };
    let arm_nodes = syntax.ordered_children(SyntaxRoleClass::MatchArm)?;
    if arm_nodes.len() != projection.arms().len() {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
    }

    arm_nodes
        .into_iter()
        .zip(projection.arms())
        .enumerate()
        .map(|(ordinal, (arm, arm_projection))| {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })?;
            let arm = arm.cast::<MatchArmKind>()?;
            let arm_syntax = arm.syntax();
            if arm_syntax.role() != SyntaxRole::MatchArm(ordinal) {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }
            let component_range = |part| {
                match_arm_component_range(components, ordinal, part)
                    .ok_or(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() })
            };
            let whole_range = component_range(SyntaxMatchArmPart::Whole)?;
            if whole_range != arm.range() {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }

            let patterns = arm_syntax.children_with_role(SyntaxRole::Pattern);
            let [pattern] = patterns.as_slice() else {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            };
            let pattern = FamilyNode::<PatternFamily>::new(pattern.clone())?.semantic()?;
            if pattern.whole_source_span().range() != component_range(SyntaxMatchArmPart::Pattern)?
            {
                return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
            }

            let guard_nodes = arm_syntax.children_with_role(SyntaxRole::Guard);
            let guard = match arm_projection.guard() {
                None if guard_nodes.is_empty() => None,
                Some(slot) => Some(attached_match_arm_expression(
                    &arm_syntax,
                    SyntaxRole::Guard,
                    slot,
                    component_range(SyntaxMatchArmPart::Guard)?,
                )?),
                None => {
                    return Err(SyntaxAccessError::InvalidExpressionProjection { id: syntax.id() });
                }
            };
            let value = attached_match_arm_expression(
                &arm_syntax,
                SyntaxRole::Body,
                arm_projection.value(),
                component_range(SyntaxMatchArmPart::Value)?,
            )?;
            let components = [
                SyntaxMatchArmPart::Whole,
                SyntaxMatchArmPart::Pattern,
                SyntaxMatchArmPart::Guard,
                SyntaxMatchArmPart::Arrow,
                SyntaxMatchArmPart::Value,
            ]
            .into_iter()
            .filter_map(|part| {
                match_arm_component_range(components, ordinal, part).map(|range| {
                    AttachedMatchArmComponent {
                        part,
                        source: syntax.source_span_for_range(range),
                    }
                })
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

            Ok(AttachedMatchArm {
                syntax: arm,
                projection: arm_projection.clone(),
                pattern,
                guard,
                value,
                components,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn attached_match_arm_expression(
    owner: &SyntaxNodeHandle,
    role: SyntaxRole,
    slot: SyntaxExpressionSlot,
    source: SourceRange,
) -> Result<AttachedMatchArmExpression, SyntaxAccessError> {
    let children = owner.children_with_role(role);
    let [child] = children.as_slice() else {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() });
    };
    if !ExpressionFamily::accepts(child.kind())
        || !component_matches_semantic_child(owner, child, source)
    {
        return Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() });
    }
    match slot {
        SyntaxExpressionSlot::Authored if child.kind() != SyntaxKind::MissingExpression => {
            Ok(AttachedMatchArmExpression::Authored {
                expression: FamilyNode::<ExpressionFamily>::new(child.clone())?,
                source: owner.source_span_for_range(source),
            })
        }
        SyntaxExpressionSlot::Missing if child.kind() == SyntaxKind::MissingExpression => {
            Ok(AttachedMatchArmExpression::Missing {
                recovery: FamilyNode::<RecoveryFamily>::new(child.clone())?,
            })
        }
        _ => Err(SyntaxAccessError::InvalidExpressionProjection { id: owner.id() }),
    }
}

fn match_arm_component_range(
    components: &[PendingExpressionComponent],
    arm: u32,
    part: SyntaxMatchArmPart,
) -> Option<SourceRange> {
    components
        .iter()
        .find(|component| component.role() == ExpressionComponentRole::MatchArm { arm, part })
        .map(|component| component.range())
}
