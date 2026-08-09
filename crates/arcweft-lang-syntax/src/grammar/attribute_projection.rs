//! Parser-owned semantic projection shared by structured source and item attributes.

use std::collections::HashSet;

use arcweft_source::SourceRange;

use crate::expressions::{
    ExpressionComponentRole, PendingExpressionComponent, SyntaxCallArgumentListTerminator,
    SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
};

/// Parser-selected recovery attached to the current attribute grammar.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PendingOuterAttributeIssue {
    MissingPath,
    InvalidShape,
}

/// Current attribute form after the dotted path has been consumed once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PendingOuterAttributeForm {
    Marker,
    Parenthesized {
        arguments: Box<[SyntaxCallArgumentProjection]>,
        terminator: SyntaxCallArgumentListTerminator,
    },
}

/// Event-local attribute projection copied into the immutable attached record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingOuterAttributeProjection {
    form: PendingOuterAttributeForm,
    issue: Option<PendingOuterAttributeIssue>,
    components: Box<[PendingExpressionComponent]>,
}

impl PendingOuterAttributeProjection {
    pub(crate) fn marker(issue: Option<PendingOuterAttributeIssue>) -> Self {
        Self {
            form: PendingOuterAttributeForm::Marker,
            issue,
            components: Box::new([]),
        }
    }

    pub(crate) fn parenthesized(
        arguments: Vec<SyntaxCallArgumentProjection>,
        terminator: SyntaxCallArgumentListTerminator,
        components: Vec<PendingExpressionComponent>,
        issue: Option<PendingOuterAttributeIssue>,
    ) -> Self {
        Self {
            form: PendingOuterAttributeForm::Parenthesized {
                arguments: arguments.into_boxed_slice(),
                terminator,
            },
            issue,
            components: components.into_boxed_slice(),
        }
    }

    pub(crate) const fn form(&self) -> &PendingOuterAttributeForm {
        &self.form
    }

    pub(crate) const fn issue(&self) -> Option<PendingOuterAttributeIssue> {
        self.issue
    }

    pub(crate) const fn components(&self) -> &[PendingExpressionComponent] {
        &self.components
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.issue.is_some()
            || match &self.form {
                PendingOuterAttributeForm::Marker => false,
                PendingOuterAttributeForm::Parenthesized {
                    arguments,
                    terminator,
                } => {
                    arguments
                        .iter()
                        .any(SyntaxCallArgumentProjection::has_recovery)
                        || *terminator == SyntaxCallArgumentListTerminator::RecoveredMissing
                }
            }
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

        match &self.form {
            PendingOuterAttributeForm::Marker => self.components.is_empty(),
            PendingOuterAttributeForm::Parenthesized {
                arguments,
                terminator,
            } => validate_argument_components(arguments, *terminator, &roles, &self.components),
        }
    }

    pub(crate) fn rebased(&self, offset: usize) -> Option<Self> {
        let components = self
            .components
            .iter()
            .map(|component| {
                Some(PendingExpressionComponent::new(
                    component.role(),
                    SourceRange::new(
                        component.range().start().checked_add(offset)?,
                        component.range().end().checked_add(offset)?,
                    ),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            form: self.form.clone(),
            issue: self.issue,
            components: components.into_boxed_slice(),
        })
    }
}

fn validate_argument_components(
    arguments: &[SyntaxCallArgumentProjection],
    terminator: SyntaxCallArgumentListTerminator,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let Some(expected) = expected_argument_roles(arguments, terminator, roles) else {
        return false;
    };
    if components.len() != expected.len()
        || expected.iter().any(|role| !roles.contains(role))
        || components
            .iter()
            .any(|component| !expected.contains(&component.role()))
    {
        return false;
    }
    argument_ranges_validate(arguments, terminator, components)
}

fn expected_argument_roles(
    arguments: &[SyntaxCallArgumentProjection],
    terminator: SyntaxCallArgumentListTerminator,
    roles: &HashSet<ExpressionComponentRole>,
) -> Option<Vec<ExpressionComponentRole>> {
    let mut expected = vec![ExpressionComponentRole::CallArgumentListOpen];
    expected.push(match terminator {
        SyntaxCallArgumentListTerminator::Closed => ExpressionComponentRole::CallArgumentListClose,
        SyntaxCallArgumentListTerminator::RecoveredMissing => {
            ExpressionComponentRole::CallArgumentListRecoveryEnd
        }
    });
    if arguments.is_empty() {
        expected.push(ExpressionComponentRole::CallArgumentListEmptyInsertion);
    }

    for (argument, projection) in arguments.iter().enumerate() {
        let argument = u16::try_from(argument).ok()?;
        expected.extend([
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Whole,
            },
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Value,
            },
        ]);
        match projection {
            SyntaxCallArgumentProjection::Positional { .. } => {}
            SyntaxCallArgumentProjection::Named { .. } => expected.extend([
                ExpressionComponentRole::CallArgument {
                    argument,
                    part: SyntaxCallArgumentPart::Name,
                },
                ExpressionComponentRole::CallArgument {
                    argument,
                    part: SyntaxCallArgumentPart::Equals,
                },
            ]),
            SyntaxCallArgumentProjection::Spread { .. } => {
                expected.push(ExpressionComponentRole::CallArgument {
                    argument,
                    part: SyntaxCallArgumentPart::Spread,
                });
            }
        }
        if argument > 0 {
            expected.push(ExpressionComponentRole::CallArgumentSeparator {
                following: argument,
            });
        }
    }
    if roles.contains(&ExpressionComponentRole::CallArgumentTrailingSeparator) {
        expected.push(ExpressionComponentRole::CallArgumentTrailingSeparator);
    }
    Some(expected)
}

fn argument_ranges_validate(
    arguments: &[SyntaxCallArgumentProjection],
    terminator: SyntaxCallArgumentListTerminator,
    components: &[PendingExpressionComponent],
) -> bool {
    let Some(open) = component_range(components, ExpressionComponentRole::CallArgumentListOpen)
    else {
        return false;
    };
    let Some(tail) = component_range(
        components,
        match terminator {
            SyntaxCallArgumentListTerminator::Closed => {
                ExpressionComponentRole::CallArgumentListClose
            }
            SyntaxCallArgumentListTerminator::RecoveredMissing => {
                ExpressionComponentRole::CallArgumentListRecoveryEnd
            }
        },
    ) else {
        return false;
    };
    if open.start() == open.end()
        || open.end() > tail.end()
        || terminator == SyntaxCallArgumentListTerminator::RecoveredMissing
            && tail.start() != tail.end()
    {
        return false;
    }

    let mut previous_end = open.end();
    for argument in 0..arguments.len() {
        let argument = u16::try_from(argument).expect("validated attribute argument ordinal");
        let whole = component_range(
            components,
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Whole,
            },
        )
        .expect("validated attribute argument whole component");
        let value = component_range(
            components,
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Value,
            },
        )
        .expect("validated attribute argument value component");
        if whole.start() < previous_end
            || value.start() < whole.start()
            || value.end() > whole.end()
            || whole.end() > tail.start()
        {
            return false;
        }
        previous_end = whole.end();
    }
    true
}

fn component_range(
    components: &[PendingExpressionComponent],
    role: ExpressionComponentRole,
) -> Option<SourceRange> {
    components
        .iter()
        .find(|component| component.role() == role)
        .map(|component| component.range())
}
