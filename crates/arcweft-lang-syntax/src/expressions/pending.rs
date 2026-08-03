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
    SyntaxDialogueApplicationForm, SyntaxLifetimeRegistryPath, SyntaxThreadMode,
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
                | SyntaxKind::IfExpression
                | SyntaxKind::IfLetExpression
                | SyntaxKind::MatchExpression
                | SyntaxKind::ErrorExpression
        )
    }

    pub(crate) const fn accepts_kind(&self, kind: SyntaxKind) -> bool {
        if let ExpressionProjection::DialogueContentApplication(application) = &self.projection {
            return match application.form() {
                SyntaxDialogueApplicationForm::Bracket { .. }
                | SyntaxDialogueApplicationForm::Colon => {
                    matches!(kind, SyntaxKind::DialogueContentApplicationExpression)
                }
            };
        }
        if matches!(self.projection, ExpressionProjection::PostfixBracket(_)) {
            return matches!(kind, SyntaxKind::PostfixBracketExpression);
        }
        matches!(
            (&self.projection, kind),
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
                | (
                    ExpressionProjection::Range { .. },
                    SyntaxKind::RangeExpression
                )
                | (
                    ExpressionProjection::Record(_),
                    SyntaxKind::RecordExpression
                )
                | (
                    ExpressionProjection::RecordLiteral(_),
                    SyntaxKind::RecordLiteralExpression
                )
                | (
                    ExpressionProjection::Binary { .. },
                    SyntaxKind::BinaryExpression
                )
                | (
                    ExpressionProjection::Closure(_),
                    SyntaxKind::ClosureExpression
                )
                | (
                    ExpressionProjection::Block,
                    SyntaxKind::BlockExpression | SyntaxKind::NamedBlockExpression
                )
                | (
                    ExpressionProjection::ComputationBlock(_),
                    SyntaxKind::ComputationBlockExpression
                )
                | (
                    ExpressionProjection::NamedBlock(_),
                    SyntaxKind::NamedBlockExpression
                )
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

        match &self.projection {
            ExpressionProjection::Unit
            | ExpressionProjection::Path
            | ExpressionProjection::Block
            | ExpressionProjection::ComputationBlock(_)
            | ExpressionProjection::Choice => self.components.is_empty(),
            ExpressionProjection::NamedBlock(name) => {
                exact_component_roles(&roles, &self.components, &[ExpressionComponentRole::Name])
                    && component_range(&self.components, ExpressionComponentRole::Name).is_some_and(
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
                    )
            }
            ExpressionProjection::Thread(thread) => {
                let mut expected = Vec::with_capacity(2);
                if thread.mode() == SyntaxThreadMode::Detached {
                    expected.push(ExpressionComponentRole::ThreadMode);
                }
                if thread.name().is_some() {
                    expected.push(ExpressionComponentRole::Name);
                }
                exact_component_roles(&roles, &self.components, &expected)
            }
            ExpressionProjection::Literal(literal) => {
                let shape = literal.shape();
                self.components.iter().all(|component| {
                    matches!(component.role(), ExpressionComponentRole::Literal(_))
                }) && has_role(
                    &roles,
                    ExpressionComponentRole::Literal(ExpressionLiteralPart::Body),
                ) && has_role(
                    &roles,
                    ExpressionComponentRole::Literal(ExpressionLiteralPart::Prefix),
                ) == shape.has_prefix()
                    && has_role(
                        &roles,
                        ExpressionComponentRole::Literal(ExpressionLiteralPart::Suffix),
                    ) == shape.has_suffix()
                    && has_role(
                        &roles,
                        ExpressionComponentRole::Literal(ExpressionLiteralPart::Unit),
                    ) == shape.has_unit()
                    && self.components.len()
                        == 1 + usize::from(shape.has_prefix())
                            + usize::from(shape.has_suffix())
                            + usize::from(shape.has_unit())
            }
            ExpressionProjection::EntityReference(entity) => {
                let shape = entity.shape();
                let expected = 1
                    + usize::from(shape.has_absolute_marker())
                    + (usize::from(shape.has_family()) * 2)
                    + shape.parent_depth()
                    + usize::try_from(shape.segment_count()).unwrap_or(usize::MAX);
                self.components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::EntityReference(_)
                    )
                }) && has_role(
                    &roles,
                    ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Whole),
                ) && component_range(
                    &self.components,
                    ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Whole),
                ) == Some(owner)
                    && has_role(
                        &roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::AbsoluteMarker),
                    ) == shape.has_absolute_marker()
                    && has_role(
                        &roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::Family),
                    ) == shape.has_family()
                    && has_role(
                        &roles,
                        ExpressionComponentRole::EntityReference(SyntaxIdRefPart::FamilySeparator),
                    ) == shape.has_family()
                    && contiguous_roles(
                        shape.parent_depth(),
                        |ordinal| {
                            ExpressionComponentRole::EntityReference(
                                SyntaxIdRefPart::ParentMarker { ordinal },
                            )
                        },
                        &roles,
                    )
                    && contiguous_roles(
                        usize::try_from(shape.segment_count()).unwrap_or(usize::MAX),
                        |ordinal| {
                            ExpressionComponentRole::EntityReference(
                                SyntaxIdRefPart::SuffixSegment { ordinal },
                            )
                        },
                        &roles,
                    )
                    && self.components.len() == expected
            }
            ExpressionProjection::LifetimePath(path) => {
                self.components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::LifetimeScope
                            | ExpressionComponentRole::LifetimeKeySegment { .. }
                            | ExpressionComponentRole::LifetimeOptionalMarker
                    )
                }) && has_role(&roles, ExpressionComponentRole::LifetimeScope)
                    && component_range(&self.components, ExpressionComponentRole::LifetimeScope)
                        .is_some_and(|range| range.start() == owner.start())
                    && contiguous_roles(
                        path.segments().len(),
                        |ordinal| ExpressionComponentRole::LifetimeKeySegment { ordinal },
                        &roles,
                    )
                    && has_role(&roles, ExpressionComponentRole::LifetimeOptionalMarker)
                        == path.is_optional()
                    && self.components.len()
                        == 1 + path.segments().len() + usize::from(path.is_optional())
                    && lifetime_tail_range(&self.components, path) == Some(owner.end())
            }
            ExpressionProjection::ShortVariant(_) => {
                self.components.len() == 2
                    && has_role(&roles, ExpressionComponentRole::ShortVariantMarker)
                    && has_role(&roles, ExpressionComponentRole::ShortVariantName)
                    && component_range(
                        &self.components,
                        ExpressionComponentRole::ShortVariantMarker,
                    )
                    .is_some_and(|range| {
                        range.start() == owner.start() && range.start() < range.end()
                    })
                    && component_range(&self.components, ExpressionComponentRole::ShortVariantName)
                        .is_some_and(|range| range.end() == owner.end())
            }
            ExpressionProjection::Placeholder(_) => {
                self.components.len() == 1
                    && has_role(&roles, ExpressionComponentRole::PlaceholderMarker)
                    && component_range(&self.components, ExpressionComponentRole::PlaceholderMarker)
                        == Some(owner)
            }
            ExpressionProjection::Tuple(slots) => {
                self.components.iter().all(|component| {
                    matches!(component.role(), ExpressionComponentRole::Element { .. })
                }) && contiguous_roles(
                    slots.len(),
                    |ordinal| ExpressionComponentRole::Element { ordinal },
                    &roles,
                ) && self.components.len() == slots.len()
                    && !slots.is_empty()
            }
            ExpressionProjection::BracketSequence(slots) => {
                self.components.iter().all(|component| {
                    matches!(component.role(), ExpressionComponentRole::Element { .. })
                }) && contiguous_roles(
                    slots.len(),
                    |ordinal| ExpressionComponentRole::Element { ordinal },
                    &roles,
                ) && self.components.len() == slots.len()
            }
            ExpressionProjection::NumericBracketSequence(sequence) => {
                self.components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::NumericElement { .. }
                            | ExpressionComponentRole::NumericCommonSuffix
                    )
                }) && contiguous_roles(
                    sequence.source_element_count(),
                    |ordinal| ExpressionComponentRole::NumericElement { ordinal },
                    &roles,
                ) && has_role(&roles, ExpressionComponentRole::NumericCommonSuffix)
                    == sequence.common_suffix().is_some()
                    && self.components.len()
                        == sequence.source_element_count()
                            + usize::from(sequence.common_suffix().is_some())
            }
            ExpressionProjection::ArrayRepeat(_) => {
                self.components.len() == 2
                    && has_role(&roles, ExpressionComponentRole::RepeatValue)
                    && has_role(&roles, ExpressionComponentRole::RepeatLength)
                    && self.components.iter().all(|component| {
                        matches!(
                            component.role(),
                            ExpressionComponentRole::RepeatValue
                                | ExpressionComponentRole::RepeatLength
                        )
                    })
            }
            ExpressionProjection::Call(call) => {
                call::components_validate(call, owner, &roles, &self.components)
            }
            ExpressionProjection::Select(_) => exact_component_roles(
                &roles,
                &self.components,
                &[
                    ExpressionComponentRole::Target,
                    ExpressionComponentRole::SelectedMember,
                ],
            ),
            ExpressionProjection::Index(_) => exact_component_roles(
                &roles,
                &self.components,
                &[
                    ExpressionComponentRole::Target,
                    ExpressionComponentRole::Index,
                ],
            ),
            ExpressionProjection::DialogueContentApplication(application) => {
                dialogue::components_validate(application, &roles, &self.components)
            }
            ExpressionProjection::PostfixBracket(_) => exact_component_roles(
                &roles,
                &self.components,
                &[
                    ExpressionComponentRole::Target,
                    ExpressionComponentRole::OpenBracket,
                    ExpressionComponentRole::CloseBracket,
                    ExpressionComponentRole::Content,
                ],
            ),
            ExpressionProjection::Pipe(_) => exact_component_roles(
                &roles,
                &self.components,
                &[
                    ExpressionComponentRole::LeftOperand,
                    ExpressionComponentRole::Operator,
                    ExpressionComponentRole::RightOperand,
                ],
            ),
            ExpressionProjection::Try { .. }
            | ExpressionProjection::Await { .. }
            | ExpressionProjection::Borrow { .. }
            | ExpressionProjection::Dereference { .. }
            | ExpressionProjection::Unary { .. } => exact_component_roles(
                &roles,
                &self.components,
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
                self.components.iter().all(|component| {
                    matches!(
                        component.role(),
                        ExpressionComponentRole::RangeStart
                            | ExpressionComponentRole::RangeEnd
                            | ExpressionComponentRole::RangeInclusiveMarker
                    )
                }) && has_role(&roles, ExpressionComponentRole::RangeStart) == start.is_some()
                    && has_role(&roles, ExpressionComponentRole::RangeEnd) == end.is_some()
                    && has_role(&roles, ExpressionComponentRole::RangeInclusiveMarker) == *inclusive
                    && self.components.len()
                        == usize::from(start.is_some())
                            + usize::from(end.is_some())
                            + usize::from(*inclusive)
            }
            ExpressionProjection::Record(fields) => {
                record::components_validate(fields, true, &roles, &self.components)
            }
            ExpressionProjection::RecordLiteral(fields) => {
                record::components_validate(fields, false, &roles, &self.components)
            }
            ExpressionProjection::Binary { .. } => exact_component_roles(
                &roles,
                &self.components,
                &[
                    ExpressionComponentRole::LeftOperand,
                    ExpressionComponentRole::Operator,
                    ExpressionComponentRole::RightOperand,
                ],
            ),
            ExpressionProjection::Closure(closure) => {
                let mut expected = vec![
                    ExpressionComponentRole::Body,
                    ExpressionComponentRole::ClosureOpenDelimiter,
                    match closure.syntax().terminator() {
                        SyntaxClosureTerminator::Closed => {
                            ExpressionComponentRole::ClosureCloseDelimiter
                        }
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
                        return false;
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
                exact_component_roles(&roles, &self.components, &expected)
            }
            ExpressionProjection::If { else_branch, .. } => {
                exact_component_roles(
                    &roles,
                    &self.components,
                    &[
                        ExpressionComponentRole::Condition,
                        ExpressionComponentRole::ThenBranch,
                        ExpressionComponentRole::ElseBranch,
                    ],
                ) && (else_branch.is_some() || {
                    component_range(&self.components, ExpressionComponentRole::ThenBranch)
                        .zip(component_range(
                            &self.components,
                            ExpressionComponentRole::ElseBranch,
                        ))
                        .is_some_and(|(then_branch, else_branch)| {
                            else_branch.start() == then_branch.end()
                                && else_branch.start() == else_branch.end()
                        })
                })
            }
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
                exact_component_roles(&roles, &self.components, &expected)
                    && (else_branch.is_some()
                        || component_range(&self.components, ExpressionComponentRole::ThenBranch)
                            .zip(component_range(
                                &self.components,
                                ExpressionComponentRole::ElseBranch,
                            ))
                            .is_some_and(|(then_branch, else_branch)| {
                                else_branch.start() == then_branch.end()
                                    && else_branch.start() == else_branch.end()
                            }))
            }
            ExpressionProjection::Match(projection) => {
                control::match_components_validate(projection, &roles, &self.components)
            }
            ExpressionProjection::Error => exact_component_roles(
                &roles,
                &self.components,
                &[ExpressionComponentRole::Recovery],
            ),
        }
    }
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
