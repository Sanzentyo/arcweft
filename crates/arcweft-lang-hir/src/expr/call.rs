//! Ordinary and associated Call payloads owned by the final expression arena.

use std::collections::BTreeMap;

use thiserror::Error;

use super::{
    HirExprInvariantError, HirPoisonState, HirRecoveryIssue, validate_expr, validate_module,
};
use crate::identity::{ExprId, HirLimit, HirModuleId, TypeId};
use crate::leaf::HirName;

/// Zero-based position within the final ordinary-Call argument limit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCallArgumentOrdinal(u16);

impl HirCallArgumentOrdinal {
    /// Converts a zero-based source argument position into the bounded final-HIR ordinal.
    pub fn try_from_usize(value: usize) -> Result<Self, HirCallArgumentOrdinalError> {
        let limit = HirLimit::CallArguments.maximum();
        if value >= limit {
            return Err(HirCallArgumentOrdinalError {
                ordinal: value,
                limit,
            });
        }
        u16::try_from(value)
            .map(Self)
            .map_err(|_| HirCallArgumentOrdinalError {
                ordinal: value,
                limit,
            })
    }

    pub(crate) fn try_new(value: usize) -> Result<Self, HirCallArgumentOrdinalError> {
        Self::try_from_usize(value)
    }

    /// Returns the zero-based authored argument position.
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("Call argument ordinal {ordinal} exceeds the exclusive limit {limit}")]
pub struct HirCallArgumentOrdinalError {
    ordinal: usize,
    limit: usize,
}

/// Zero-based position within the final explicit Call type-argument limit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCallTypeArgumentOrdinal(u16);

impl HirCallTypeArgumentOrdinal {
    /// Converts a zero-based source type-argument position into the bounded final-HIR ordinal.
    pub fn try_from_usize(value: usize) -> Result<Self, HirCallTypeArgumentOrdinalError> {
        let limit = HirLimit::CallTypeArguments.maximum();
        if value >= limit {
            return Err(HirCallTypeArgumentOrdinalError {
                ordinal: value,
                limit,
            });
        }
        u16::try_from(value)
            .map(Self)
            .map_err(|_| HirCallTypeArgumentOrdinalError {
                ordinal: value,
                limit,
            })
    }

    pub(crate) fn try_new(value: usize) -> Result<Self, HirCallTypeArgumentOrdinalError> {
        Self::try_from_usize(value)
    }

    /// Returns the zero-based authored type-argument position.
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
#[error("Call type-argument ordinal {ordinal} exceeds the exclusive limit {limit}")]
pub struct HirCallTypeArgumentOrdinalError {
    ordinal: usize,
    limit: usize,
}

/// One ordinary, unresolved-dot, or explicit-associated Call.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCallExpr {
    callee: HirCallCallee,
    explicit_type_application: HirCallTypeApplication,
    arguments: Box<[HirCallArgument]>,
    terminator: HirCallArgumentListTerminator,
}

impl HirCallExpr {
    pub(crate) fn try_new(
        callee: HirCallCallee,
        explicit_type_application: HirCallTypeApplication,
        arguments: Box<[HirCallArgument]>,
        terminator: HirCallArgumentListTerminator,
        child_states: HirCallChildStates<'_>,
        rich_text_context: bool,
    ) -> Result<(Self, HirPoisonState), HirCallBuildError> {
        let limit = if rich_text_context {
            HirLimit::RichTextCallArguments
        } else {
            HirLimit::CallArguments
        };
        if arguments.len() > limit.maximum() {
            return Err(HirCallBuildError::LimitExceeded {
                limit,
                observed: arguments.len(),
            });
        }
        if explicit_type_application.argument_count() > HirLimit::CallTypeArguments.maximum() {
            return Err(HirCallBuildError::LimitExceeded {
                limit: HirLimit::CallTypeArguments,
                observed: explicit_type_application.argument_count(),
            });
        }
        if child_states.argument_values.len() != arguments.len()
            || child_states.type_arguments.len()
                != explicit_type_application.present_argument_count()
        {
            return Err(HirCallBuildError::ChildStateShapeMismatch);
        }

        for (argument, state) in arguments.iter().zip(child_states.argument_values) {
            if matches!(argument.value_state(), HirCallValue::Missing { .. })
                && !matches!(state, HirCallChildPoison::Poisoned)
            {
                return Err(HirCallBuildError::ChildStateShapeMismatch);
            }
        }
        explicit_type_application.validate_child_states(child_states.type_arguments)?;

        let call = Self {
            callee,
            explicit_type_application,
            arguments,
            terminator,
        };
        let state = call
            .primary_issue(child_states)
            .map_or(HirPoisonState::Clean, |issue| {
                HirPoisonState::Poisoned(HirRecoveryIssue::InvalidCall(issue))
            });
        Ok((call, state))
    }

    pub const fn callee(&self) -> &HirCallCallee {
        &self.callee
    }

    pub const fn explicit_type_application(&self) -> &HirCallTypeApplication {
        &self.explicit_type_application
    }

    pub fn arguments(&self) -> &[HirCallArgument] {
        &self.arguments
    }

    pub const fn terminator(&self) -> HirCallArgumentListTerminator {
        self.terminator
    }

    pub(crate) fn issues(&self, child_states: HirCallChildStates<'_>) -> Box<[HirCallIssue]> {
        let mut issues = Vec::new();
        if matches!(
            (&self.callee, child_states.callee),
            (HirCallCallee::Value { .. }, HirCallChildPoison::Poisoned)
        ) {
            issues.push(HirCallIssue::InvalidCalleeExpression);
        }

        if let Some((receiver, separator, member)) = self.callee.associated_parts() {
            match receiver {
                HirAssociatedReceiver::Resolved { .. } => {}
                HirAssociatedReceiver::InvalidPresent { .. } => {
                    issues.push(HirCallIssue::InvalidAssociatedReceiver);
                }
                HirAssociatedReceiver::BareGenericArity {
                    declared, supplied, ..
                } => issues.push(HirCallIssue::BareGenericArity {
                    declared: *declared,
                    supplied: *supplied,
                }),
                HirAssociatedReceiver::NominalError { error, .. } => {
                    issues.push(HirCallIssue::AssociatedReceiverNominalError(*error));
                }
            }
            match separator {
                HirAssociatedSeparator::Present(_) => {}
            }
            match member {
                HirRecoveredName::Valid(_) => {}
                HirRecoveredName::Missing => issues.push(HirCallIssue::MissingAssociatedMember),
                HirRecoveredName::InvalidPresent => {
                    issues.push(HirCallIssue::InvalidAssociatedMember);
                }
            }
        }

        self.explicit_type_application
            .append_issues(child_states.type_arguments, &mut issues);
        if self.terminator == HirCallArgumentListTerminator::RecoveredMissing {
            issues.push(HirCallIssue::MissingArgumentListClose);
        }
        append_argument_issues(&self.arguments, child_states.argument_values, &mut issues);
        issues.sort_by_key(HirCallIssue::key);
        issues.into_boxed_slice()
    }

    /// Applies the ordinary Call argument-order, punctuation, and child-state
    /// rules without constructing a callee expression owner.
    pub(crate) fn argument_issues(
        arguments: &[HirCallArgument],
        child_states: &[HirCallChildPoison],
    ) -> Result<Box<[HirCallIssue]>, HirCallBuildError> {
        if arguments.len() != child_states.len() {
            return Err(HirCallBuildError::ChildStateShapeMismatch);
        }
        let mut issues = Vec::new();
        append_argument_issues(arguments, child_states, &mut issues);
        issues.sort_by_key(HirCallIssue::key);
        Ok(issues.into_boxed_slice())
    }

    pub(crate) fn primary_issue(
        &self,
        child_states: HirCallChildStates<'_>,
    ) -> Option<HirCallIssue> {
        self.issues(child_states).first().cloned()
    }

    pub(super) fn has_duplicate_named_arguments(&self) -> bool {
        self.arguments
            .iter()
            .enumerate()
            .any(|(ordinal, argument)| {
                argument.resolved_name().is_some_and(|name| {
                    self.arguments[..ordinal]
                        .iter()
                        .any(|earlier| earlier.resolved_name() == Some(name))
                })
            })
    }

    pub(super) fn contains_recovery_payload(&self) -> bool {
        let associated_recovery =
            self.callee
                .associated_parts()
                .is_some_and(|(receiver, separator, member)| {
                    !matches!(receiver, HirAssociatedReceiver::Resolved { .. })
                        || !matches!(separator, HirAssociatedSeparator::Present(_))
                        || !matches!(member, HirRecoveredName::Valid(_))
                });
        let type_application_recovery = match &self.explicit_type_application {
            HirCallTypeApplication::Absent => false,
            HirCallTypeApplication::Present {
                arguments,
                terminator,
                ..
            } => {
                *terminator != HirCallTypeApplicationTerminator::Closed
                    || arguments
                        .iter()
                        .any(|argument| !matches!(argument, HirCallTypeArgument::Resolved { .. }))
            }
        };
        let argument_recovery = self
            .arguments
            .iter()
            .enumerate()
            .any(|(position, argument)| {
                let structural_recovery = match argument {
                    HirCallArgument::Positional { value } => {
                        matches!(value, HirCallValue::Missing { .. })
                    }
                    HirCallArgument::Named {
                        name,
                        equals,
                        value,
                    } => {
                        !matches!(name, HirRecoveredName::Valid(_))
                            || *equals != HirRequiredTokenState::Present
                            || matches!(value, HirCallValue::Missing { .. })
                    }
                    HirCallArgument::Spread { value, ellipsis } => {
                        *ellipsis != HirRequiredTokenState::Present
                            || matches!(value, HirCallValue::Missing { .. })
                            || position + 1 != self.arguments.len()
                    }
                };
                structural_recovery
                    || matches!(argument, HirCallArgument::Positional { .. })
                        && self.arguments[..position]
                            .iter()
                            .any(|earlier| matches!(earlier, HirCallArgument::Named { .. }))
            });
        associated_recovery
            || type_application_recovery
            || self.terminator == HirCallArgumentListTerminator::RecoveredMissing
            || argument_recovery
            || self.has_duplicate_named_arguments()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        self.callee.validate_module(expected)?;
        self.explicit_type_application.validate_module(expected)?;
        for argument in &self.arguments {
            validate_expr(expected, argument.value())?;
        }
        Ok(())
    }
}

fn append_argument_issues(
    arguments: &[HirCallArgument],
    child_states: &[HirCallChildPoison],
    issues: &mut Vec<HirCallIssue>,
) {
    let mut first_named = None;
    let mut names = BTreeMap::<&HirName, HirCallArgumentOrdinal>::new();
    for (position, (argument, child_state)) in arguments.iter().zip(child_states).enumerate() {
        let ordinal = HirCallArgumentOrdinal::try_new(position)
            .expect("constructor preflight keeps every argument ordinal valid");
        match argument {
            HirCallArgument::Positional { .. } => {
                if first_named.is_some() {
                    issues.push(HirCallIssue::PositionalAfterNamed { argument: ordinal });
                }
            }
            HirCallArgument::Named { name, equals, .. } => {
                first_named.get_or_insert(ordinal);
                match name {
                    HirRecoveredName::Valid(name) => {
                        if let Some(first) = names.insert(name, ordinal) {
                            issues.push(HirCallIssue::DuplicateNamedArgument {
                                first,
                                duplicate: ordinal,
                            });
                        }
                    }
                    HirRecoveredName::Missing => {
                        issues.push(HirCallIssue::MissingArgumentName { argument: ordinal });
                    }
                    HirRecoveredName::InvalidPresent => {
                        issues.push(HirCallIssue::InvalidArgumentName { argument: ordinal });
                    }
                }
                match equals {
                    HirRequiredTokenState::Present => {}
                    HirRequiredTokenState::Missing => {
                        issues.push(HirCallIssue::MissingNamedEquals { argument: ordinal });
                    }
                    HirRequiredTokenState::InvalidPresent => {
                        issues.push(HirCallIssue::InvalidNamedEquals { argument: ordinal });
                    }
                }
            }
            HirCallArgument::Spread { ellipsis, .. } => {
                match ellipsis {
                    HirRequiredTokenState::Present => {}
                    HirRequiredTokenState::Missing => {
                        issues.push(HirCallIssue::MissingSpreadEllipsis { argument: ordinal });
                    }
                    HirRequiredTokenState::InvalidPresent => {
                        issues.push(HirCallIssue::InvalidSpreadEllipsis { argument: ordinal });
                    }
                }
                if position + 1 != arguments.len() {
                    issues.push(HirCallIssue::SpreadNotLast { argument: ordinal });
                }
            }
        }
        match argument.value_state() {
            HirCallValue::Missing { .. } => {
                issues.push(HirCallIssue::MissingArgumentValue { argument: ordinal });
            }
            HirCallValue::Present { .. } if *child_state == HirCallChildPoison::Poisoned => {
                issues.push(HirCallIssue::InvalidArgumentValue { argument: ordinal });
            }
            HirCallValue::Present { .. } => {}
        }
    }
}

/// Value-space or typed associated-callee authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallCallee {
    Value {
        value: ExprId,
    },
    UnresolvedDot {
        value_receiver: ExprId,
        nominal_receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    },
    Associated {
        receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    },
}

impl HirCallCallee {
    pub(crate) const fn value(expression: ExprId) -> Self {
        Self::Value { value: expression }
    }

    pub(crate) const fn unresolved_dot(
        value_receiver: ExprId,
        nominal_receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    ) -> Self {
        Self::UnresolvedDot {
            value_receiver,
            nominal_receiver,
            separator,
            member,
        }
    }

    pub(crate) const fn associated(
        receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    ) -> Self {
        Self::Associated {
            receiver,
            separator,
            member,
        }
    }

    pub const fn value_expression(&self) -> Option<ExprId> {
        match self {
            Self::Value { value } => Some(*value),
            Self::UnresolvedDot { value_receiver, .. } => Some(*value_receiver),
            Self::Associated { .. } => None,
        }
    }

    pub const fn associated_parts(
        &self,
    ) -> Option<(
        &HirAssociatedReceiver,
        &HirAssociatedSeparator,
        &HirRecoveredName,
    )> {
        match self {
            Self::UnresolvedDot {
                nominal_receiver,
                separator,
                member,
                ..
            } => Some((nominal_receiver, separator, member)),
            Self::Associated {
                receiver,
                separator,
                member,
            } => Some((receiver, separator, member)),
            Self::Value { .. } => None,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Value { value } => validate_expr(expected, *value),
            Self::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                ..
            } => {
                validate_expr(expected, *value_receiver)?;
                nominal_receiver.validate_module(expected)
            }
            Self::Associated { receiver, .. } => receiver.validate_module(expected),
        }
    }
}

/// Typed nominal receiver retained for dot fallback or explicit association.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssociatedReceiver {
    Resolved {
        receiver: TypeId,
    },
    InvalidPresent {
        poisoned: TypeId,
    },
    BareGenericArity {
        poisoned: TypeId,
        declared: u16,
        supplied: u16,
    },
    NominalError {
        error: HirAssociatedReceiverError,
    },
}

impl HirAssociatedReceiver {
    pub(crate) const fn resolved(receiver: TypeId) -> Self {
        Self::Resolved { receiver }
    }

    pub(crate) const fn invalid_present(poisoned: TypeId) -> Self {
        Self::InvalidPresent { poisoned }
    }

    pub const fn type_id(&self) -> Option<TypeId> {
        match self {
            Self::Resolved { receiver } => Some(*receiver),
            Self::InvalidPresent { poisoned } | Self::BareGenericArity { poisoned, .. } => {
                Some(*poisoned)
            }
            Self::NominalError { .. } => None,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self.type_id() {
            Some(ty) => validate_module(expected, ty.module()),
            None => Ok(()),
        }
    }
}

/// Project-aware nominal classification failure retained without a string path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssociatedReceiverError {
    UnknownNominal,
    AmbiguousNominal,
    ForeignProject,
    Inaccessible,
}

/// Authored associated-call separator family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssociatedCallSyntax {
    DotFallback,
    ExplicitDoubleColon,
}

/// Required associated-call separator, including exact recovery intent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirAssociatedSeparator {
    Present(HirAssociatedCallSyntax),
}

/// Required or recovered identifier without a fabricated name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecoveredName {
    Valid(HirName),
    Missing,
    InvalidPresent,
}

impl HirRecoveredName {
    pub const fn resolved(&self) -> Option<&HirName> {
        match self {
            Self::Valid(name) => Some(name),
            Self::Missing | Self::InvalidPresent => None,
        }
    }
}

/// Optional explicit type application attached to the terminal callable name.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeApplication {
    Absent,
    Present {
        spelling: HirCallTypeApplicationSpelling,
        arguments: Box<[HirCallTypeArgument]>,
        terminator: HirCallTypeApplicationTerminator,
    },
}

impl HirCallTypeApplication {
    pub(crate) const fn absent() -> Self {
        Self::Absent
    }

    pub(crate) fn present(
        spelling: HirCallTypeApplicationSpelling,
        arguments: Box<[HirCallTypeArgument]>,
        terminator: HirCallTypeApplicationTerminator,
    ) -> Self {
        Self::Present {
            spelling,
            arguments,
            terminator,
        }
    }

    pub fn arguments(&self) -> &[HirCallTypeArgument] {
        match self {
            Self::Absent => &[],
            Self::Present { arguments, .. } => arguments,
        }
    }

    pub const fn spelling(&self) -> Option<HirCallTypeApplicationSpelling> {
        match self {
            Self::Absent => None,
            Self::Present { spelling, .. } => Some(*spelling),
        }
    }

    fn argument_count(&self) -> usize {
        self.arguments().len()
    }

    fn present_argument_count(&self) -> usize {
        self.arguments()
            .iter()
            .filter(|argument| !matches!(argument, HirCallTypeArgument::Missing))
            .count()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        for argument in self.arguments() {
            if let Some(ty) = argument.type_id() {
                validate_module(expected, ty.module())?;
            }
        }
        Ok(())
    }

    fn validate_child_states(
        &self,
        child_states: &[HirCallChildPoison],
    ) -> Result<(), HirCallBuildError> {
        let mut states = child_states.iter();
        for argument in self.arguments() {
            let expected = match argument {
                HirCallTypeArgument::Resolved { .. } => Some(HirCallChildPoison::Clean),
                HirCallTypeArgument::InvalidPresent { .. } => Some(HirCallChildPoison::Poisoned),
                HirCallTypeArgument::Missing => None,
            };
            if let Some(expected) = expected
                && states.next().copied() != Some(expected)
            {
                return Err(HirCallBuildError::ChildStateShapeMismatch);
            }
        }
        if states.next().is_some() {
            return Err(HirCallBuildError::ChildStateShapeMismatch);
        }
        Ok(())
    }

    fn append_issues(&self, child_states: &[HirCallChildPoison], issues: &mut Vec<HirCallIssue>) {
        let Self::Present {
            arguments,
            terminator,
            ..
        } = self
        else {
            return;
        };
        match terminator {
            HirCallTypeApplicationTerminator::Closed => {}
            HirCallTypeApplicationTerminator::RecoveredMissing => {
                issues.push(HirCallIssue::MissingTypeApplicationClose);
            }
            HirCallTypeApplicationTerminator::InvalidPresent => {
                issues.push(HirCallIssue::InvalidTypeApplicationClose);
            }
        }
        let mut states = child_states.iter();
        for (position, argument) in arguments.iter().enumerate() {
            let ordinal = HirCallTypeArgumentOrdinal::try_new(position)
                .expect("constructor preflight keeps every type-argument ordinal valid");
            let poisoned = match argument {
                HirCallTypeArgument::Resolved { .. } => {
                    states.next() == Some(&HirCallChildPoison::Poisoned)
                }
                HirCallTypeArgument::InvalidPresent { .. } => {
                    let _ = states.next();
                    true
                }
                HirCallTypeArgument::Missing => {
                    issues.push(HirCallIssue::MissingTypeArgument { argument: ordinal });
                    false
                }
            };
            if poisoned {
                issues.push(HirCallIssue::InvalidTypeArgument { argument: ordinal });
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeApplicationSpelling {
    DirectAngle,
    Turbofish,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeApplicationTerminator {
    Closed,
    RecoveredMissing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeArgument {
    Resolved { ty: TypeId },
    InvalidPresent { poisoned: TypeId },
    Missing,
}

impl HirCallTypeArgument {
    pub const fn type_id(&self) -> Option<TypeId> {
        match self {
            Self::Resolved { ty } => Some(*ty),
            Self::InvalidPresent { poisoned } => Some(*poisoned),
            Self::Missing => None,
        }
    }
}

/// Structural state of the required parenthesized argument-list terminator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallArgumentListTerminator {
    Closed,
    RecoveredMissing,
}

/// One source-ordered Call argument, including recoverable punctuation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallArgument {
    Positional {
        value: HirCallValue,
    },
    Named {
        name: HirRecoveredName,
        equals: HirRequiredTokenState,
        value: HirCallValue,
    },
    Spread {
        value: HirCallValue,
        ellipsis: HirRequiredTokenState,
    },
}

impl HirCallArgument {
    #[cfg(test)]
    pub(crate) const fn positional(value: ExprId) -> Self {
        Self::Positional {
            value: HirCallValue::Present { value },
        }
    }

    #[cfg(test)]
    pub(crate) const fn named(name: HirName, value: ExprId) -> Self {
        Self::Named {
            name: HirRecoveredName::Valid(name),
            equals: HirRequiredTokenState::Present,
            value: HirCallValue::Present { value },
        }
    }

    #[cfg(test)]
    pub(crate) const fn spread(value: ExprId) -> Self {
        Self::Spread {
            value: HirCallValue::Present { value },
            ellipsis: HirRequiredTokenState::Present,
        }
    }

    #[cfg(test)]
    pub(crate) const fn missing_positional(recovery: ExprId) -> Self {
        Self::Positional {
            value: HirCallValue::Missing { recovery },
        }
    }

    pub const fn value(&self) -> ExprId {
        self.value_state().expression()
    }

    pub const fn value_state(&self) -> &HirCallValue {
        match self {
            Self::Positional { value } | Self::Named { value, .. } | Self::Spread { value, .. } => {
                value
            }
        }
    }

    pub const fn resolved_name(&self) -> Option<&HirName> {
        match self {
            Self::Named { name, .. } => name.resolved(),
            Self::Positional { .. } | Self::Spread { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallValue {
    Present { value: ExprId },
    Missing { recovery: ExprId },
}

impl HirCallValue {
    pub const fn expression(&self) -> ExprId {
        match self {
            Self::Present { value } => *value,
            Self::Missing { recovery } => *recovery,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRequiredTokenState {
    Present,
    Missing,
    InvalidPresent,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallChildPoison {
    Clean,
    Poisoned,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct HirCallChildStates<'a> {
    callee: HirCallChildPoison,
    argument_values: &'a [HirCallChildPoison],
    type_arguments: &'a [HirCallChildPoison],
}

impl<'a> HirCallChildStates<'a> {
    pub(crate) const fn new(
        callee: HirCallChildPoison,
        argument_values: &'a [HirCallChildPoison],
        type_arguments: &'a [HirCallChildPoison],
    ) -> Self {
        Self {
            callee,
            argument_values,
            type_arguments,
        }
    }
}

#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirCallBuildError {
    #[error("{limit:?} accepts at most {maximum} entries, observed {observed}", maximum = .limit.maximum())]
    LimitExceeded { limit: HirLimit, observed: usize },
    #[error("Call child-state slices do not match the structural payload")]
    ChildStateShapeMismatch,
}

/// Canonically ordered Call recovery retained by root poison and diagnostics.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallIssue {
    InvalidCalleeExpression,
    UnresolvedDotMember,
    InvalidAssociatedReceiver,
    AssociatedReceiverNominalError(HirAssociatedReceiverError),
    BareGenericArity {
        declared: u16,
        supplied: u16,
    },
    MissingAssociatedMember,
    InvalidAssociatedMember,
    MissingTypeApplicationClose,
    InvalidTypeApplicationClose,
    MissingTypeArgument {
        argument: HirCallTypeArgumentOrdinal,
    },
    InvalidTypeArgument {
        argument: HirCallTypeArgumentOrdinal,
    },
    MissingArgumentListClose,
    MissingArgumentName {
        argument: HirCallArgumentOrdinal,
    },
    InvalidArgumentName {
        argument: HirCallArgumentOrdinal,
    },
    MissingNamedEquals {
        argument: HirCallArgumentOrdinal,
    },
    InvalidNamedEquals {
        argument: HirCallArgumentOrdinal,
    },
    MissingArgumentValue {
        argument: HirCallArgumentOrdinal,
    },
    InvalidArgumentValue {
        argument: HirCallArgumentOrdinal,
    },
    MissingSpreadEllipsis {
        argument: HirCallArgumentOrdinal,
    },
    InvalidSpreadEllipsis {
        argument: HirCallArgumentOrdinal,
    },
    DuplicateNamedArgument {
        first: HirCallArgumentOrdinal,
        duplicate: HirCallArgumentOrdinal,
    },
    PositionalAfterNamed {
        argument: HirCallArgumentOrdinal,
    },
    SpreadNotLast {
        argument: HirCallArgumentOrdinal,
    },
}

impl HirCallIssue {
    const fn key(&self) -> HirCallIssueKey {
        match self {
            Self::InvalidCalleeExpression => HirCallIssueKey::new(0, 0, 0, 1),
            Self::UnresolvedDotMember => HirCallIssueKey::new(0, 0, 0, 2),
            Self::InvalidAssociatedReceiver => HirCallIssueKey::new(1, 0, 0, 0),
            Self::AssociatedReceiverNominalError(error) => {
                HirCallIssueKey::new(1, 0, 1, *error as u16)
            }
            Self::BareGenericArity { declared, supplied } => {
                HirCallIssueKey::new(1, 0, 3, declared.saturating_add(*supplied))
            }
            Self::MissingAssociatedMember => HirCallIssueKey::new(1, 0, 5, 0),
            Self::InvalidAssociatedMember => HirCallIssueKey::new(1, 0, 5, 1),
            Self::MissingTypeApplicationClose => HirCallIssueKey::new(2, 0, 0, 0),
            Self::InvalidTypeApplicationClose => HirCallIssueKey::new(2, 0, 0, 1),
            Self::MissingTypeArgument { argument } => HirCallIssueKey::new(2, argument.get(), 1, 0),
            Self::InvalidTypeArgument { argument } => HirCallIssueKey::new(2, argument.get(), 1, 1),
            Self::MissingArgumentListClose => HirCallIssueKey::new(3, 0, 0, 0),
            Self::MissingArgumentName { argument } => HirCallIssueKey::new(3, argument.get(), 1, 0),
            Self::InvalidArgumentName { argument } => HirCallIssueKey::new(3, argument.get(), 1, 1),
            Self::MissingNamedEquals { argument } => HirCallIssueKey::new(3, argument.get(), 2, 0),
            Self::InvalidNamedEquals { argument } => HirCallIssueKey::new(3, argument.get(), 2, 1),
            Self::MissingArgumentValue { argument } => {
                HirCallIssueKey::new(3, argument.get(), 3, 0)
            }
            Self::InvalidArgumentValue { argument } => {
                HirCallIssueKey::new(3, argument.get(), 3, 1)
            }
            Self::MissingSpreadEllipsis { argument } => {
                HirCallIssueKey::new(3, argument.get(), 4, 0)
            }
            Self::InvalidSpreadEllipsis { argument } => {
                HirCallIssueKey::new(3, argument.get(), 4, 1)
            }
            Self::DuplicateNamedArgument { first, duplicate } => {
                HirCallIssueKey::new(4, duplicate.get(), 0, first.get())
            }
            Self::PositionalAfterNamed { argument } => {
                HirCallIssueKey::new(4, argument.get(), 1, 0)
            }
            Self::SpreadNotLast { argument } => HirCallIssueKey::new(4, argument.get(), 2, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HirCallIssueKey {
    phase: u8,
    ordinal: u16,
    component: u8,
    tie: u16,
}

impl HirCallIssueKey {
    const fn new(phase: u8, ordinal: u16, component: u8, tie: u16) -> Self {
        Self {
            phase,
            ordinal,
            component,
            tie,
        }
    }
}
