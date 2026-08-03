//! Parser-owned control-expression projections.

use std::collections::HashSet;

use arcweft_source::SourceRange;

use super::pending::{component_range, exact_component_roles};
use super::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    SyntaxExpressionSlot, SyntaxRequiredTokenState,
};

impl ExpressionProjection {
    /// Whether this expression owns one of the typed value-block families.
    pub const fn is_value_block(&self) -> bool {
        matches!(
            self,
            Self::Block | Self::ComputationBlock(_) | Self::NamedBlock(_)
        )
    }
}

/// Source-owned part of one Match arm.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxMatchArmPart {
    Whole,
    Pattern,
    Guard,
    Arrow,
    Value,
}

/// Parser-owned terminator state for one Match body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxMatchBodyTerminator {
    /// The authored body owns a closing brace.
    Closed,
    /// The required opening body delimiter was absent.
    MissingBody,
    /// The body was opened but its closing brace was recovered.
    RecoveredMissingClose,
}

/// Source-backed semantic shape of one Match arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchArmProjection {
    guard: Option<SyntaxExpressionSlot>,
    arrow: SyntaxRequiredTokenState,
    value: SyntaxExpressionSlot,
}

impl SyntaxMatchArmProjection {
    pub(crate) const fn new(
        guard: Option<SyntaxExpressionSlot>,
        arrow: SyntaxRequiredTokenState,
        value: SyntaxExpressionSlot,
    ) -> Self {
        Self {
            guard,
            arrow,
            value,
        }
    }

    /// Optional authored `when` slot. `Some(Missing)` preserves an authored
    /// guard introducer whose required expression was absent.
    pub const fn guard(&self) -> Option<SyntaxExpressionSlot> {
        self.guard
    }

    /// Authored or recovered required fat arrow.
    pub const fn arrow(&self) -> SyntaxRequiredTokenState {
        self.arrow
    }

    /// Authored or recovered required arm value.
    pub const fn value(&self) -> SyntaxExpressionSlot {
        self.value
    }

    pub fn has_recovery(&self) -> bool {
        self.guard.is_some_and(SyntaxExpressionSlot::is_missing)
            || !matches!(self.arrow, SyntaxRequiredTokenState::Present)
            || self.value.is_missing()
    }
}

/// Central parser projection for one Match expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxMatchProjection {
    scrutinee: SyntaxExpressionSlot,
    arms: Box<[SyntaxMatchArmProjection]>,
    terminator: SyntaxMatchBodyTerminator,
}

impl SyntaxMatchProjection {
    pub(crate) fn new(
        scrutinee: SyntaxExpressionSlot,
        arms: Vec<SyntaxMatchArmProjection>,
        terminator: SyntaxMatchBodyTerminator,
    ) -> Self {
        Self {
            scrutinee,
            arms: arms.into_boxed_slice(),
            terminator,
        }
    }

    pub const fn scrutinee(&self) -> SyntaxExpressionSlot {
        self.scrutinee
    }

    pub fn arms(&self) -> &[SyntaxMatchArmProjection] {
        &self.arms
    }

    pub const fn terminator(&self) -> SyntaxMatchBodyTerminator {
        self.terminator
    }

    pub fn has_recovery(&self) -> bool {
        self.scrutinee.is_missing()
            || !matches!(self.terminator, SyntaxMatchBodyTerminator::Closed)
            || self.arms.iter().any(SyntaxMatchArmProjection::has_recovery)
    }
}

pub(super) fn match_components_validate(
    projection: &SyntaxMatchProjection,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let mut expected = Vec::with_capacity(
        1 + projection
            .arms()
            .iter()
            .map(|arm| 4 + usize::from(arm.guard().is_some()))
            .sum::<usize>(),
    );
    expected.push(ExpressionComponentRole::Scrutinee);
    for (arm, projection) in projection.arms().iter().enumerate() {
        let Ok(arm) = u32::try_from(arm) else {
            return false;
        };
        expected.extend([
            ExpressionComponentRole::MatchArm {
                arm,
                part: SyntaxMatchArmPart::Whole,
            },
            ExpressionComponentRole::MatchArm {
                arm,
                part: SyntaxMatchArmPart::Pattern,
            },
            ExpressionComponentRole::MatchArm {
                arm,
                part: SyntaxMatchArmPart::Arrow,
            },
            ExpressionComponentRole::MatchArm {
                arm,
                part: SyntaxMatchArmPart::Value,
            },
        ]);
        if projection.guard().is_some() {
            expected.push(ExpressionComponentRole::MatchArm {
                arm,
                part: SyntaxMatchArmPart::Guard,
            });
        }
    }
    if !exact_component_roles(roles, components, &expected) {
        return false;
    }

    let Some(scrutinee) = component_range(components, ExpressionComponentRole::Scrutinee) else {
        return false;
    };
    if !slot_range_matches(projection.scrutinee(), scrutinee)
        || matches!(
            projection.terminator(),
            SyntaxMatchBodyTerminator::MissingBody
        ) && !projection.arms().is_empty()
    {
        return false;
    }

    let mut previous_end = scrutinee.end();
    for (arm, arm_projection) in projection.arms().iter().enumerate() {
        let Ok(arm) = u32::try_from(arm) else {
            return false;
        };
        let range =
            |part| component_range(components, ExpressionComponentRole::MatchArm { arm, part });
        let (Some(whole), Some(pattern), Some(arrow), Some(value)) = (
            range(SyntaxMatchArmPart::Whole),
            range(SyntaxMatchArmPart::Pattern),
            range(SyntaxMatchArmPart::Arrow),
            range(SyntaxMatchArmPart::Value),
        ) else {
            return false;
        };
        let guard = range(SyntaxMatchArmPart::Guard);
        if previous_end > whole.start()
            || pattern.start() < whole.start()
            || pattern.end() > whole.end()
            || arrow.start() < pattern.end()
            || arrow.end() > whole.end()
            || value.start() < arrow.end()
            || value.end() > whole.end()
            || !slot_range_matches(arm_projection.value(), value)
            || !required_token_range_matches(arm_projection.arrow(), arrow)
            || guard.is_some() != arm_projection.guard().is_some()
            || guard.is_some_and(|guard| {
                guard.start() < pattern.end()
                    || guard.end() > arrow.start()
                    || !slot_range_matches(
                        arm_projection
                            .guard()
                            .expect("guard range requires one guard slot"),
                        guard,
                    )
            })
        {
            return false;
        }
        previous_end = whole.end();
    }
    true
}

const fn slot_range_matches(slot: SyntaxExpressionSlot, range: SourceRange) -> bool {
    match slot {
        SyntaxExpressionSlot::Authored => range.start() < range.end(),
        SyntaxExpressionSlot::Missing => range.start() == range.end(),
    }
}

const fn required_token_range_matches(state: SyntaxRequiredTokenState, range: SourceRange) -> bool {
    match state {
        SyntaxRequiredTokenState::Present | SyntaxRequiredTokenState::InvalidPresent => {
            range.start() < range.end()
        }
        SyntaxRequiredTokenState::Missing => range.start() == range.end(),
    }
}
