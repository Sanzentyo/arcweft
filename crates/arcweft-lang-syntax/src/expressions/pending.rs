//! Parser-staged expression payload and source-component validation.

mod call;
mod dialogue;
mod record;

use std::collections::HashSet;

use arcweft_source::SourceRange;

use super::control;
use super::{
    ExpressionComponentRole, ExpressionLiteralPart, ExpressionProjection,
    SyntaxClosureParameterPart, SyntaxClosureSyntax, SyntaxClosureTerminator,
    SyntaxLifetimeRegistryPath, SyntaxThreadMode,
};
use crate::grammar::kinds::SyntaxKind;
use crate::id_ref::SyntaxIdRefPart;
use crate::name::SyntaxNameIssue;

/// One source-relative component staged on an expression start event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingExpressionComponent {
    role: ExpressionComponentRole,
    range: SourceRange,
}

impl PendingExpressionComponent {
    pub(crate) const fn new(role: ExpressionComponentRole, range: SourceRange) -> Self {
        Self { role, range }
    }

    pub(crate) const fn role(self) -> ExpressionComponentRole {
        self.role
    }

    pub(crate) const fn range(self) -> SourceRange {
        self.range
    }

    fn rebased(self, offset: usize) -> Option<Self> {
        Some(Self::new(
            self.role,
            SourceRange::new(
                self.range.start().checked_add(offset)?,
                self.range.end().checked_add(offset)?,
            ),
        ))
    }
}

/// Event-local semantic projection copied into the immutable attached record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingExpressionProjection {
    projection: ExpressionProjection,
    components: Box<[PendingExpressionComponent]>,
}

impl PendingExpressionProjection {
    pub(crate) fn new(
        projection: ExpressionProjection,
        components: Vec<PendingExpressionComponent>,
    ) -> Self {
        Self {
            projection,
            components: components.into_boxed_slice(),
        }
    }

    pub(crate) const fn projection(&self) -> &ExpressionProjection {
        &self.projection
    }

    pub(crate) fn components(&self) -> &[PendingExpressionComponent] {
        &self.components
    }

    /// Whether the parser retained typed recovery inside this known family.
    pub(crate) fn has_recovery(&self) -> bool {
        self.projection.has_recovery()
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        Some(Self {
            projection: self.projection.clone(),
            components: self
                .components
                .iter()
                .copied()
                .map(|component| component.rebased(offset))
                .collect::<Option<Vec<_>>>()?
                .into_boxed_slice(),
        })
    }

    pub(crate) const fn kind_requires_projection(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::LiteralExpression
                | SyntaxKind::EntityReferenceExpression
                | SyntaxKind::LifetimePathExpression
                | SyntaxKind::PathExpression
                | SyntaxKind::ShortVariantExpression
                | SyntaxKind::PlaceholderExpression
                | SyntaxKind::TupleExpression
                | SyntaxKind::BracketSequenceExpression
                | SyntaxKind::NumericBracketSequenceExpression
                | SyntaxKind::ArrayRepeatExpression
                | SyntaxKind::CallExpression
                | SyntaxKind::PostfixBracketExpression
                | SyntaxKind::DialogueContentApplicationExpression
                | SyntaxKind::PipeExpression
                | SyntaxKind::TryExpression
                | SyntaxKind::AwaitExpression
                | SyntaxKind::ThreadExpression
                | SyntaxKind::ChoiceExpression
                | SyntaxKind::BorrowExpression
                | SyntaxKind::DereferenceExpression
                | SyntaxKind::UnaryExpression
                | SyntaxKind::RangeExpression
                | SyntaxKind::RecordExpression
                | SyntaxKind::RecordLiteralExpression
                | SyntaxKind::BinaryExpression
                | SyntaxKind::ClosureExpression
                | SyntaxKind::BlockExpression
                | SyntaxKind::ComputationBlockExpression
                | SyntaxKind::NamedBlockExpression
                | SyntaxKind::LoopExpression
                | SyntaxKind::IfExpression
                | SyntaxKind::IfLetExpression
                | SyntaxKind::MatchExpression
                | SyntaxKind::ErrorExpression
        )
    }

    pub(crate) const fn accepts_kind(&self, kind: SyntaxKind) -> bool {
        Self::accepts_leaf_kind(&self.projection, kind)
            || Self::accepts_structured_kind(&self.projection, kind)
    }

    const fn accepts_leaf_kind(projection: &ExpressionProjection, kind: SyntaxKind) -> bool {
        matches!(
            (projection, kind),
            (
                ExpressionProjection::Unit | ExpressionProjection::Tuple(_),
                SyntaxKind::TupleExpression
            ) | (
                ExpressionProjection::Literal(_),
                SyntaxKind::LiteralExpression
            ) | (
                ExpressionProjection::EntityReference(_),
                SyntaxKind::EntityReferenceExpression
            ) | (
                ExpressionProjection::LifetimePath(_),
                SyntaxKind::LifetimePathExpression
            ) | (ExpressionProjection::Path, SyntaxKind::PathExpression)
                | (
                    ExpressionProjection::ShortVariant(_),
                    SyntaxKind::ShortVariantExpression
                )
                | (
                    ExpressionProjection::Placeholder(_),
                    SyntaxKind::PlaceholderExpression
                )
                | (
                    ExpressionProjection::BracketSequence(_),
                    SyntaxKind::BracketSequenceExpression
                )
                | (
                    ExpressionProjection::NumericBracketSequence(_),
                    SyntaxKind::NumericBracketSequenceExpression
                )
                | (
                    ExpressionProjection::ArrayRepeat(_),
                    SyntaxKind::ArrayRepeatExpression
                )
                | (ExpressionProjection::Call(_), SyntaxKind::CallExpression)
                | (
                    ExpressionProjection::Select(_),
                    SyntaxKind::SelectExpression
                )
                | (
                    ExpressionProjection::Index(_),
                    SyntaxKind::PostfixBracketExpression
                )
                | (ExpressionProjection::Pipe(_), SyntaxKind::PipeExpression)
                | (ExpressionProjection::Try { .. }, SyntaxKind::TryExpression)
                | (
                    ExpressionProjection::Await { .. },
                    SyntaxKind::AwaitExpression
                )
                | (
                    ExpressionProjection::Borrow { .. },
                    SyntaxKind::BorrowExpression
                )
                | (
                    ExpressionProjection::Dereference { .. },
                    SyntaxKind::DereferenceExpression
                )
                | (
                    ExpressionProjection::Unary { .. },
                    SyntaxKind::UnaryExpression
                )
        )
    }

    const fn accepts_structured_kind(projection: &ExpressionProjection, kind: SyntaxKind) -> bool {
        matches!(
            (projection, kind),
            (
                ExpressionProjection::DialogueContentApplication(_),
                SyntaxKind::DialogueContentApplicationExpression
            ) | (
                ExpressionProjection::PostfixBracket(_) | ExpressionProjection::Index(_),
                SyntaxKind::PostfixBracketExpression
            ) | (
                ExpressionProjection::Range { .. },
                SyntaxKind::RangeExpression
            ) | (
                ExpressionProjection::Record(_),
                SyntaxKind::RecordExpression
            ) | (
                ExpressionProjection::RecordLiteral(_),
                SyntaxKind::RecordLiteralExpression
            ) | (
                ExpressionProjection::Binary { .. },
                SyntaxKind::BinaryExpression
            ) | (
                ExpressionProjection::Closure(_),
                SyntaxKind::ClosureExpression
            ) | (
                ExpressionProjection::Block,
                SyntaxKind::BlockExpression | SyntaxKind::NamedBlockExpression
            ) | (
                ExpressionProjection::ComputationBlock(_),
                SyntaxKind::ComputationBlockExpression
            ) | (
                ExpressionProjection::NamedBlock(_),
                SyntaxKind::NamedBlockExpression
            ) | (ExpressionProjection::Loop, SyntaxKind::LoopExpression)
                | (
                    ExpressionProjection::Thread(_),
                    SyntaxKind::ThreadExpression
                )
                | (ExpressionProjection::Choice, SyntaxKind::ChoiceExpression)
                | (ExpressionProjection::If { .. }, SyntaxKind::IfExpression)
                | (
                    ExpressionProjection::IfLet { .. },
                    SyntaxKind::IfLetExpression
                )
                | (ExpressionProjection::Match(_), SyntaxKind::MatchExpression)
                | (
                    ExpressionProjection::Error,
                    SyntaxKind::ErrorExpression | SyntaxKind::MissingExpression
                )
        )
    }

    pub(crate) fn validates_components(&self, owner: SourceRange) -> bool {
        if self.components.iter().any(|component| {
            component.range().start() < owner.start()
                || component.range().end() > owner.end()
                || component.range().start() > component.range().end()
        }) {
            return false;
        }
        let mut roles = HashSet::with_capacity(self.components.len());
        if self
            .components
            .iter()
            .any(|component| !roles.insert(component.role()))
        {
            return false;
        }
        if let Some(valid) = [
            basic_leaf_components_validate(&self.projection, owner, &roles, &self.components),
            identity_components_validate(&self.projection, owner, &roles, &self.components),
            sequence_components_validate(&self.projection, &roles, &self.components),
            closure_components_validate(&self.projection, &roles, &self.components),
            branch_components_validate(&self.projection, &roles, &self.components),
        ]
        .into_iter()
        .flatten()
        .next()
        {
            return valid;
        }

        remaining_components_validate(&self.projection, owner, &roles, &self.components)
    }
}

fn remaining_components_validate(
    projection: &ExpressionProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    match projection {
        ExpressionProjection::Call(call) => {
            call::components_validate(call, owner, roles, components)
        }
        ExpressionProjection::Select(_) => exact_component_roles(
            roles,
            components,
            &[
                ExpressionComponentRole::Target,
                ExpressionComponentRole::SelectedMember,
            ],
        ),
        ExpressionProjection::Index(_) => exact_component_roles(
            roles,
            components,
            &[
                ExpressionComponentRole::Target,
                ExpressionComponentRole::Index,
            ],
        ),
        ExpressionProjection::DialogueContentApplication(application) => {
            dialogue::components_validate(application, roles, components)
        }
        ExpressionProjection::PostfixBracket(_) => exact_component_roles(
            roles,
            components,
            &[
                ExpressionComponentRole::Target,
                ExpressionComponentRole::OpenBracket,
                ExpressionComponentRole::CloseBracket,
                ExpressionComponentRole::Content,
            ],
        ),
        ExpressionProjection::Pipe(_) | ExpressionProjection::Binary { .. } => {
            exact_component_roles(
                roles,
                components,
                &[
                    ExpressionComponentRole::LeftOperand,
                    ExpressionComponentRole::Operator,
                    ExpressionComponentRole::RightOperand,
                ],
            )
        }
        ExpressionProjection::Await { branches, .. } => {
            let mut expected = vec![
                ExpressionComponentRole::Operand,
                ExpressionComponentRole::Operator,
            ];
            if branches.is_some() {
                expected.push(ExpressionComponentRole::AwaitWith);
            }
            exact_component_roles(roles, components, &expected)
        }
        ExpressionProjection::Try { .. }
        | ExpressionProjection::Borrow { .. }
        | ExpressionProjection::Dereference { .. }
        | ExpressionProjection::Unary { .. } => exact_component_roles(
            roles,
            components,
            &[
                ExpressionComponentRole::Operand,
                ExpressionComponentRole::Operator,
            ],
        ),
        ExpressionProjection::Range {
            start,
            end,
            inclusive,
        } => {
            components.iter().all(|component| {
                matches!(
                    component.role(),
                    ExpressionComponentRole::RangeStart
                        | ExpressionComponentRole::RangeEnd
                        | ExpressionComponentRole::RangeInclusiveMarker
                )
            }) && has_role(roles, ExpressionComponentRole::RangeStart) == start.is_some()
                && has_role(roles, ExpressionComponentRole::RangeEnd) == end.is_some()
                && has_role(roles, ExpressionComponentRole::RangeInclusiveMarker) == *inclusive
                && components.len()
                    == usize::from(start.is_some())
                        + usize::from(end.is_some())
                        + usize::from(*inclusive)
        }
        ExpressionProjection::Record(fields) => {
            record::components_validate(fields, true, roles, components)
        }
        ExpressionProjection::RecordLiteral(fields) => {
            record::components_validate(fields, false, roles, components)
        }
        _ => unreachable!("specialized component validation returns before generic dispatch"),
    }
}

fn basic_leaf_components_validate(
    projection: &ExpressionProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> Option<bool> {
    match projection {
        ExpressionProjection::Unit
        | ExpressionProjection::Path
        | ExpressionProjection::Block
        | ExpressionProjection::ComputationBlock(_)
        | ExpressionProjection::Loop
        | ExpressionProjection::Choice => Some(components.is_empty()),
        ExpressionProjection::NamedBlock(name) => Some(
            exact_component_roles(roles, components, &[ExpressionComponentRole::Name])
                && component_range(components, ExpressionComponentRole::Name).is_some_and(
                    |range| match name {
                        Ok(name) => {
                            range.end().saturating_sub(range.start()) == name.as_str().len()
                        }
                        Err(SyntaxNameIssue::Missing) => false,
                        Err(
                            SyntaxNameIssue::InvalidStart { spelling }
                            | SyntaxNameIssue::InvalidContinuation { spelling },
                        ) => range.end().saturating_sub(range.start()) == spelling.len(),
                    },
                ),
        ),
        ExpressionProjection::Thread(thread) => {
            let mut expected = Vec::with_capacity(2);
            if thread.mode() == SyntaxThreadMode::Detached {
                expected.push(ExpressionComponentRole::ThreadMode);
            }
            if thread.name().is_some() {
                expected.push(ExpressionComponentRole::Name);
            }
            Some(exact_component_roles(roles, components, &expected))
        }
        ExpressionProjection::Literal(literal) => {
            let shape = literal.shape();
            Some(
                components.iter().all(|component| {
                    matches!(component.role(), ExpressionComponentRole::Literal(_))
                }) && has_role(
                    roles,
                    ExpressionComponentRole::Literal(ExpressionLiteralPart::Body),
                ) && has_role(
                    roles,
                    ExpressionComponentRole::Literal(ExpressionLiteralPart::Prefix),
                ) == shape.has_prefix()
                    && has_role(
                        roles,
                        ExpressionComponentRole::Literal(ExpressionLiteralPart::Suffix),
                    ) == shape.has_suffix()
                    && has_role(
                        roles,
                        ExpressionComponentRole::Literal(ExpressionLiteralPart::Unit),
                    ) == shape.has_unit()
                    && components.len()
                        == 1 + usize::from(shape.has_prefix())
                            + usize::from(shape.has_suffix())
                            + usize::from(shape.has_unit()),
            )
        }
        ExpressionProjection::ShortVariant(_) => Some(
            components.len() == 2
                && has_role(roles, ExpressionComponentRole::ShortVariantMarker)
                && has_role(roles, ExpressionComponentRole::ShortVariantName)
                && component_range(components, ExpressionComponentRole::ShortVariantMarker)
                    .is_some_and(|range| {
                        range.start() == owner.start() && range.start() < range.end()
                    })
                && component_range(components, ExpressionComponentRole::ShortVariantName)
                    .is_some_and(|range| range.end() == owner.end()),
        ),
        ExpressionProjection::Placeholder(_) => Some(
            components.len() == 1
                && has_role(roles, ExpressionComponentRole::PlaceholderMarker)
                && component_range(components, ExpressionComponentRole::PlaceholderMarker)
                    == Some(owner),
        ),
        _ => None,
    }
}

fn identity_components_validate(
    projection: &ExpressionProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> Option<bool> {
    match projection {
        ExpressionProjection::EntityReference(entity) => {
            let shape = entity.shape();
            let expected = 1
                + usize::from(shape.has_absolute_marker())
                + (usize::from(shape.has_family()) * 2)
                + shape.parent_depth()
                + usize::try_from(shape.segment_count()).unwrap_or(usize::MAX);
            Some(
                components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::EntityReference(_)
                    )
                }) && has_role(
                    roles,
                    ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Whole),
                ) && component_range(
                    components,
                    ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Whole),
                ) == Some(owner)
                    && has_role(
                        roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::AbsoluteMarker),
                    ) == shape.has_absolute_marker()
                    && has_role(
                        roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Family),
                    ) == shape.has_family()
                    && has_role(
                        roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::FamilySeparator),
                    ) == shape.has_family()
                    && contiguous_roles(
                        shape.parent_depth(),
                        |ordinal| {
                            ExpressionComponentRole::EntityReference(
                                SyntaxIdRefPart::ParentMarker { ordinal },
                            )
                        },
                        roles,
                    )
                    && contiguous_roles(
                        usize::try_from(shape.segment_count()).unwrap_or(usize::MAX),
                        |ordinal| {
                            ExpressionComponentRole::EntityReference(
                                SyntaxIdRefPart::SuffixSegment { ordinal },
                            )
                        },
                        roles,
                    )
                    && components.len() == expected,
            )
        }
        ExpressionProjection::LifetimePath(path) => Some(
            components.iter().all(|component| {
                matches!(
                    component.role(),
                    ExpressionComponentRole::LifetimeScope
                        | ExpressionComponentRole::LifetimeKeySegment { .. }
                        | ExpressionComponentRole::LifetimeOptionalMarker
                )
            }) && has_role(roles, ExpressionComponentRole::LifetimeScope)
                && component_range(components, ExpressionComponentRole::LifetimeScope)
                    .is_some_and(|range| range.start() == owner.start())
                && contiguous_roles(
                    path.segments().len(),
                    |ordinal| ExpressionComponentRole::LifetimeKeySegment { ordinal },
                    roles,
                )
                && has_role(roles, ExpressionComponentRole::LifetimeOptionalMarker)
                    == path.is_optional()
                && components.len() == 1 + path.segments().len() + usize::from(path.is_optional())
                && lifetime_tail_range(components, path) == Some(owner.end()),
        ),
        _ => None,
    }
}

fn sequence_components_validate(
    projection: &ExpressionProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> Option<bool> {
    match projection {
        ExpressionProjection::Tuple(slots) | ExpressionProjection::BracketSequence(slots) => Some(
            components.iter().all(|component| {
                matches!(component.role(), ExpressionComponentRole::Element { .. })
            }) && contiguous_roles(
                slots.len(),
                |ordinal| ExpressionComponentRole::Element { ordinal },
                roles,
            ) && components.len() == slots.len()
                && (!matches!(projection, ExpressionProjection::Tuple(_)) || !slots.is_empty()),
        ),
        ExpressionProjection::NumericBracketSequence(sequence) => Some(
            components.iter().all(|component| {
                matches!(
                    component.role(),
                    ExpressionComponentRole::NumericElement { .. }
                        | ExpressionComponentRole::NumericCommonSuffix
                )
            }) && contiguous_roles(
                sequence.source_element_count(),
                |ordinal| ExpressionComponentRole::NumericElement { ordinal },
                roles,
            ) && has_role(roles, ExpressionComponentRole::NumericCommonSuffix)
                == sequence.common_suffix().is_some()
                && components.len()
                    == sequence.source_element_count()
                        + usize::from(sequence.common_suffix().is_some()),
        ),
        ExpressionProjection::ArrayRepeat(_) => Some(
            components.len() == 2
                && has_role(roles, ExpressionComponentRole::RepeatValue)
                && has_role(roles, ExpressionComponentRole::RepeatLength)
                && components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::RepeatValue
                            | ExpressionComponentRole::RepeatLength
                    )
                }),
        ),
        _ => None,
    }
}

fn closure_components_validate(
    projection: &ExpressionProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> Option<bool> {
    let ExpressionProjection::Closure(closure) = projection else {
        return None;
    };
    let mut expected = vec![
        ExpressionComponentRole::Body,
        ExpressionComponentRole::ClosureOpenDelimiter,
        match closure.syntax().terminator() {
            SyntaxClosureTerminator::Closed => ExpressionComponentRole::ClosureCloseDelimiter,
            SyntaxClosureTerminator::RecoveredMissing => {
                ExpressionComponentRole::ClosureRecoveryEnd
            }
        },
    ];
    if matches!(
        closure.syntax(),
        SyntaxClosureSyntax::CallbackBlock {
            explicit_header: true,
            ..
        }
    ) {
        expected.push(ExpressionComponentRole::ClosureFatArrow);
    }
    if closure.has_result_type() {
        expected.push(ExpressionComponentRole::ReturnType);
    }
    for (parameter, projection) in closure.parameters().iter().enumerate() {
        let Ok(parameter) = u16::try_from(parameter) else {
            return Some(false);
        };
        expected.extend([
            ExpressionComponentRole::ClosureParameter {
                parameter,
                part: SyntaxClosureParameterPart::Whole,
            },
            ExpressionComponentRole::ClosureParameter {
                parameter,
                part: SyntaxClosureParameterPart::Pattern,
            },
        ]);
        if projection.has_type() {
            expected.extend([
                ExpressionComponentRole::ClosureParameter {
                    parameter,
                    part: SyntaxClosureParameterPart::Colon,
                },
                ExpressionComponentRole::ClosureParameter {
                    parameter,
                    part: SyntaxClosureParameterPart::Type,
                },
            ]);
        }
        if parameter > 0 {
            expected.push(ExpressionComponentRole::ClosureParameterSeparator {
                following: parameter,
            });
        }
    }
    Some(exact_component_roles(roles, components, &expected))
}

fn branch_components_validate(
    projection: &ExpressionProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> Option<bool> {
    match projection {
        ExpressionProjection::If { else_branch, .. } => Some(
            exact_component_roles(
                roles,
                components,
                &[
                    ExpressionComponentRole::Condition,
                    ExpressionComponentRole::ThenBranch,
                    ExpressionComponentRole::ElseBranch,
                ],
            ) && (else_branch.is_some() || missing_else_is_at_then_end(components)),
        ),
        ExpressionProjection::IfLet {
            guard, else_branch, ..
        } => {
            let mut expected = vec![
                ExpressionComponentRole::Pattern,
                ExpressionComponentRole::Scrutinee,
                ExpressionComponentRole::ThenBranch,
                ExpressionComponentRole::ElseBranch,
            ];
            if guard.is_some() {
                expected.push(ExpressionComponentRole::Guard);
            }
            Some(
                exact_component_roles(roles, components, &expected)
                    && (else_branch.is_some() || missing_else_is_at_then_end(components)),
            )
        }
        ExpressionProjection::Match(projection) => Some(control::match_components_validate(
            projection, roles, components,
        )),
        ExpressionProjection::Error => Some(exact_component_roles(
            roles,
            components,
            &[ExpressionComponentRole::Recovery],
        )),
        _ => None,
    }
}

fn missing_else_is_at_then_end(components: &[PendingExpressionComponent]) -> bool {
    component_range(components, ExpressionComponentRole::ThenBranch)
        .zip(component_range(
            components,
            ExpressionComponentRole::ElseBranch,
        ))
        .is_some_and(|(then_branch, else_branch)| {
            else_branch.start() == then_branch.end() && else_branch.start() == else_branch.end()
        })
}

pub(super) fn exact_component_roles(
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
    expected: &[ExpressionComponentRole],
) -> bool {
    components.len() == expected.len()
        && expected.iter().all(|role| roles.contains(role))
        && components
            .iter()
            .all(|component| expected.contains(&component.role()))
}

fn has_role(roles: &HashSet<ExpressionComponentRole>, role: ExpressionComponentRole) -> bool {
    roles.contains(&role)
}

pub(super) fn component_range(
    components: &[PendingExpressionComponent],
    role: ExpressionComponentRole,
) -> Option<SourceRange> {
    components
        .iter()
        .find(|component| component.role() == role)
        .map(|component| component.range())
}

fn lifetime_tail_range(
    components: &[PendingExpressionComponent],
    path: &SyntaxLifetimeRegistryPath,
) -> Option<usize> {
    if path.is_optional() {
        return component_range(components, ExpressionComponentRole::LifetimeOptionalMarker)
            .map(SourceRange::end);
    }
    path.segments()
        .len()
        .checked_sub(1)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .and_then(|ordinal| {
            component_range(
                components,
                ExpressionComponentRole::LifetimeKeySegment { ordinal },
            )
        })
        .or_else(|| component_range(components, ExpressionComponentRole::LifetimeScope))
        .map(SourceRange::end)
}

fn contiguous_roles(
    count: usize,
    role: impl Fn(u32) -> ExpressionComponentRole,
    roles: &HashSet<ExpressionComponentRole>,
) -> bool {
    (0..count).all(|ordinal| {
        u32::try_from(ordinal)
            .ok()
            .is_some_and(|ordinal| roles.contains(&role(ordinal)))
    })
}
