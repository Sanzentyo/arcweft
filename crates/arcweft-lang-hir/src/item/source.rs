//! Temporary final-HIR owner for attached `source` declarations.
//!
//! Lang-01.3 deletes this item family when Source is replaced by ordinary
//! Stream-producing functions. Until that authority switch, this record keeps
//! the attached grammar typed without repairing the detached Source reader.

#[cfg(test)]
mod tests;

use crate::identity::{ExprId, HirModuleId, PatternId, ScopeId, StmtId, TypeId};
use crate::leaf::{HirIdRefIssue, HirIdRefValue, HirName};

use super::{
    HirItemInvariantError, HirRequiredName, validate_optional_expr, validate_optional_pattern,
    validate_scope, validate_statements, validate_type,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceItem {
    id: Option<HirSourceId>,
    name: Option<HirRequiredName>,
    source_type: TypeId,
    headers: HirSourceHeaders,
    handlers: Box<[HirSourceHandler]>,
    body: HirSourceBody,
}

impl HirSourceItem {
    pub(crate) fn try_new(
        expected: HirModuleId,
        id: Option<HirSourceId>,
        name: Option<HirRequiredName>,
        source_type: TypeId,
        headers: HirSourceHeaders,
        handlers: Box<[HirSourceHandler]>,
        body: HirSourceBody,
    ) -> Result<Self, HirItemInvariantError> {
        let item = Self {
            id,
            name,
            source_type,
            headers,
            handlers,
            body,
        };
        item.validate_module(expected)?;
        Ok(item)
    }

    pub const fn id(&self) -> Option<&HirSourceId> {
        self.id.as_ref()
    }

    pub const fn name(&self) -> Option<&HirRequiredName> {
        self.name.as_ref()
    }

    pub const fn source_type(&self) -> TypeId {
        self.source_type
    }

    pub const fn from(&self) -> &HirSourceRequiredSlot<HirSourceExpressionValue> {
        self.headers.from()
    }

    pub const fn backpressure(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceBackpressureValue>> {
        self.headers.backpressure()
    }

    pub const fn replay(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceReplayValue>> {
        self.headers.replay()
    }

    pub const fn privacy(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourcePrivacyValue>> {
        self.headers.privacy()
    }

    pub const fn headers(&self) -> &HirSourceHeaders {
        &self.headers
    }

    pub const fn handlers(&self) -> &[HirSourceHandler] {
        &self.handlers
    }

    pub const fn body(&self) -> HirSourceBody {
        self.body
    }

    pub fn has_structural_recovery(&self) -> bool {
        self.id.as_ref().is_some_and(HirSourceId::has_recovery)
            || self
                .name
                .as_ref()
                .is_some_and(HirRequiredName::is_recovered)
            || self.headers.has_recovery()
            || self.handlers.iter().any(HirSourceHandler::has_recovery)
            || self.body.has_recovery()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        if self.id.is_none() && self.name.is_none() {
            return Err(HirItemInvariantError::MissingSourceIdentity);
        }
        if self.id.as_ref().is_some_and(HirSourceId::requires_name) && self.name.is_none() {
            return Err(HirItemInvariantError::InvalidSourceRecovery);
        }
        validate_type(expected, self.source_type)?;
        self.headers.validate_module(expected)?;
        if matches!(self.body, HirSourceBody::Missing) && !self.handlers.is_empty() {
            return Err(HirItemInvariantError::InvalidSourceRecovery);
        }
        for handler in &self.handlers {
            handler.validate_module(expected)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceHeaders {
    from: HirSourceRequiredSlot<HirSourceExpressionValue>,
    backpressure: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceBackpressureValue>>,
    replay: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceReplayValue>>,
    privacy: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourcePrivacyValue>>,
}

impl HirSourceHeaders {
    pub(crate) const fn new(
        from: HirSourceRequiredSlot<HirSourceExpressionValue>,
        backpressure: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceBackpressureValue>>,
        replay: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceReplayValue>>,
        privacy: HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourcePrivacyValue>>,
    ) -> Self {
        Self {
            from,
            backpressure,
            replay,
            privacy,
        }
    }

    pub const fn from(&self) -> &HirSourceRequiredSlot<HirSourceExpressionValue> {
        &self.from
    }

    pub const fn backpressure(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceBackpressureValue>> {
        &self.backpressure
    }

    pub const fn replay(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourceReplayValue>> {
        &self.replay
    }

    pub const fn privacy(
        &self,
    ) -> &HirSourceRequiredSlot<HirSourcePolicyBinding<HirSourcePrivacyValue>> {
        &self.privacy
    }

    fn has_recovery(&self) -> bool {
        self.from
            .has_recovery(HirSourceExpressionValue::has_recovery)
            || self.backpressure.has_recovery(|binding| {
                binding.has_recovery(HirSourceBackpressureValue::has_recovery)
            })
            || self
                .replay
                .has_recovery(|binding| binding.has_recovery(HirSourceReplayValue::has_recovery))
            || self
                .privacy
                .has_recovery(|binding| binding.has_recovery(HirSourcePrivacyValue::has_recovery))
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        self.from
            .validate_module(expected, |expected, value| value.validate_module(expected))?;
        self.backpressure
            .validate_module(expected, |expected, binding| {
                binding.value.validate_module(expected)
            })?;
        self.replay
            .validate_module(expected, |_, binding| binding.value.validate())?;
        self.privacy
            .validate_module(expected, |_, binding| binding.value.validate())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSourceId {
    value: HirIdRefValue,
    canonical_source_family: bool,
    requires_name: bool,
}

impl HirSourceId {
    pub(crate) const fn new(
        value: HirIdRefValue,
        canonical_source_family: bool,
        requires_name: bool,
    ) -> Self {
        Self {
            value,
            canonical_source_family,
            requires_name,
        }
    }

    pub const fn value(&self) -> &HirIdRefValue {
        &self.value
    }

    pub const fn is_canonical_source_family(&self) -> bool {
        self.canonical_source_family
    }

    pub const fn requires_name(&self) -> bool {
        self.requires_name
    }

    pub fn has_recovery(&self) -> bool {
        !self.canonical_source_family
            || match self.value.recovery_issue() {
                None => false,
                Some(HirIdRefIssue::Missing) if self.requires_name => false,
                Some(_) => true,
            }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceRequiredSlot<T> {
    Authored { value: T, duplicate: bool },
    Missing,
}

impl<T> HirSourceRequiredSlot<T> {
    pub(crate) const fn authored(value: T, duplicate: bool) -> Self {
        Self::Authored { value, duplicate }
    }

    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Authored { value, .. } => Some(value),
            Self::Missing => None,
        }
    }

    pub const fn is_duplicate(&self) -> bool {
        matches!(
            self,
            Self::Authored {
                duplicate: true,
                ..
            }
        )
    }

    fn has_recovery(&self, value_has_recovery: impl FnOnce(&T) -> bool) -> bool {
        match self {
            Self::Authored { value, duplicate } => *duplicate || value_has_recovery(value),
            Self::Missing => true,
        }
    }

    fn validate_module(
        &self,
        expected: HirModuleId,
        validate: impl FnOnce(HirModuleId, &T) -> Result<(), HirItemInvariantError>,
    ) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Authored { value, .. } => validate(expected, value),
            Self::Missing => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourcePunctuationState {
    Present,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourcePolicyBinding<T> {
    assignment: HirSourcePunctuationState,
    value: T,
}

impl<T> HirSourcePolicyBinding<T> {
    pub(crate) const fn new(assignment: HirSourcePunctuationState, value: T) -> Self {
        Self { assignment, value }
    }

    pub const fn assignment(&self) -> HirSourcePunctuationState {
        self.assignment
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    fn has_recovery(&self, value_has_recovery: impl FnOnce(&T) -> bool) -> bool {
        matches!(self.assignment, HirSourcePunctuationState::Missing)
            || value_has_recovery(&self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceExpressionValue {
    Expression(ExprId),
    Missing,
    Invalid,
}

impl HirSourceExpressionValue {
    pub const fn expression(self) -> Option<ExprId> {
        match self {
            Self::Expression(expression) => Some(expression),
            Self::Missing | Self::Invalid => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        !matches!(self, Self::Expression(_))
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_optional_expr(expected, self.expression())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceBackpressureValue {
    Resolved(HirSourceBackpressurePolicy),
    Recovered {
        authored: Option<HirName>,
        issue: HirSourcePolicyIssue,
    },
}

impl HirSourceBackpressureValue {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Resolved(policy) => policy.has_recovery(),
            Self::Recovered { .. } => true,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Resolved(HirSourceBackpressurePolicy::Bounded {
                capacity, overflow, ..
            }) => {
                capacity.value.validate_module(expected)?;
                overflow.value.validate()
            }
            Self::Resolved(
                HirSourceBackpressurePolicy::Latest
                | HirSourceBackpressurePolicy::BlockingNotAllowed,
            ) => Ok(()),
            Self::Recovered { authored, issue } => {
                validate_policy_recovery(authored.as_ref(), *issue)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceBackpressurePolicy {
    Latest,
    Bounded {
        capacity: HirSourceBoundedArgument<HirSourceExpressionValue>,
        overflow: HirSourceBoundedArgument<HirSourceOverflowValue>,
        unexpected_arguments: bool,
        recovered_call: bool,
    },
    BlockingNotAllowed,
}

impl HirSourceBackpressurePolicy {
    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Latest | Self::BlockingNotAllowed => false,
            Self::Bounded {
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call,
            } => {
                capacity.has_recovery(HirSourceExpressionValue::has_recovery)
                    || overflow.has_recovery(HirSourceOverflowValue::has_recovery)
                    || *unexpected_arguments
                    || *recovered_call
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSourceBoundedArgument<T> {
    value: T,
    duplicate: bool,
}

impl<T> HirSourceBoundedArgument<T> {
    pub(crate) const fn new(value: T, duplicate: bool) -> Self {
        Self { value, duplicate }
    }

    pub const fn value(&self) -> &T {
        &self.value
    }

    pub const fn is_duplicate(&self) -> bool {
        self.duplicate
    }

    fn has_recovery(&self, value_has_recovery: impl FnOnce(&T) -> bool) -> bool {
        self.duplicate || value_has_recovery(&self.value)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceOverflowValue {
    Resolved(HirSourceOverflowPolicy),
    Recovered {
        authored: Option<HirName>,
        issue: HirSourcePolicyIssue,
    },
}

impl HirSourceOverflowValue {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Resolved(_) => Ok(()),
            Self::Recovered { authored, issue } => {
                validate_policy_recovery(authored.as_ref(), *issue)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceOverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceReplayValue {
    Resolved(HirSourceReplayPolicy),
    Recovered {
        authored: Option<HirName>,
        issue: HirSourcePolicyIssue,
    },
}

impl HirSourceReplayValue {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Resolved(_) => Ok(()),
            Self::Recovered { authored, issue } => {
                validate_policy_recovery(authored.as_ref(), *issue)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceReplayPolicy {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourcePrivacyValue {
    Resolved(HirSourcePrivacyPolicy),
    Recovered {
        authored: Option<HirName>,
        issue: HirSourcePolicyIssue,
    },
}

impl HirSourcePrivacyValue {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Resolved(_) => Ok(()),
            Self::Recovered { authored, issue } => {
                validate_policy_recovery(authored.as_ref(), *issue)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourcePrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourcePolicyIssue {
    Missing,
    Invalid,
    Unsupported,
}

fn validate_policy_recovery(
    authored: Option<&HirName>,
    issue: HirSourcePolicyIssue,
) -> Result<(), HirItemInvariantError> {
    let valid = match issue {
        HirSourcePolicyIssue::Unsupported => authored.is_some(),
        HirSourcePolicyIssue::Missing | HirSourcePolicyIssue::Invalid => authored.is_none(),
    };
    if valid {
        Ok(())
    } else {
        Err(HirItemInvariantError::InvalidSourceRecovery)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceBody {
    Missing,
    Braced { closed: bool },
}

impl HirSourceBody {
    pub const fn is_closed(self) -> bool {
        matches!(self, Self::Braced { closed: true })
    }

    pub const fn has_recovery(self) -> bool {
        !self.is_closed()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSourceHandler {
    event: HirSourceEventPattern,
    arrow: HirSourcePunctuationState,
    scope: ScopeId,
    body: HirSourceHandlerBody,
}

impl HirSourceHandler {
    pub(crate) const fn new(
        event: HirSourceEventPattern,
        arrow: HirSourcePunctuationState,
        scope: ScopeId,
        body: HirSourceHandlerBody,
    ) -> Self {
        Self {
            event,
            arrow,
            scope,
            body,
        }
    }

    pub const fn event(&self) -> &HirSourceEventPattern {
        &self.event
    }

    pub const fn arrow(&self) -> HirSourcePunctuationState {
        self.arrow
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn body(&self) -> &HirSourceHandlerBody {
        &self.body
    }

    pub fn has_recovery(&self) -> bool {
        self.event.has_recovery()
            || matches!(self.arrow, HirSourcePunctuationState::Missing)
            || self.body.has_recovery()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_optional_pattern(expected, self.event.pattern())?;
        self.event.validate()?;
        validate_scope(expected, self.scope)?;
        self.body.validate_module(expected)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceEventPattern {
    Item(HirSourcePatternValue),
    Error(HirSourcePatternValue),
    Progress(HirSourcePatternValue),
    Disconnected(HirSourceChildState),
    PermissionRevoked(HirSourceChildState),
    End(HirSourceChildState),
    Recovered {
        authored: Option<HirName>,
        condition: HirSourceChildState,
        issue: HirSourceEventIssue,
    },
}

impl HirSourceEventPattern {
    pub const fn pattern(&self) -> Option<PatternId> {
        match self {
            Self::Item(pattern) | Self::Error(pattern) | Self::Progress(pattern) => {
                pattern.pattern()
            }
            Self::Disconnected(_)
            | Self::PermissionRevoked(_)
            | Self::End(_)
            | Self::Recovered { .. } => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        match self {
            Self::Item(pattern) | Self::Error(pattern) | Self::Progress(pattern) => {
                pattern.has_recovery()
            }
            Self::Disconnected(condition)
            | Self::PermissionRevoked(condition)
            | Self::End(condition) => condition.has_recovery(),
            Self::Recovered { .. } => true,
        }
    }

    fn validate(&self) -> Result<(), HirItemInvariantError> {
        let Self::Recovered {
            authored, issue, ..
        } = self
        else {
            return Ok(());
        };
        let valid = match issue {
            HirSourceEventIssue::Unsupported => authored.is_some(),
            HirSourceEventIssue::Missing | HirSourceEventIssue::Invalid => authored.is_none(),
        };
        if valid {
            Ok(())
        } else {
            Err(HirItemInvariantError::InvalidSourceRecovery)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourcePatternValue {
    Pattern(PatternId),
    Missing,
    Invalid,
}

impl HirSourcePatternValue {
    pub const fn pattern(self) -> Option<PatternId> {
        match self {
            Self::Pattern(pattern) => Some(pattern),
            Self::Missing | Self::Invalid => None,
        }
    }

    pub const fn has_recovery(self) -> bool {
        !matches!(self, Self::Pattern(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceChildState {
    Authored,
    Missing,
    Invalid,
}

impl HirSourceChildState {
    pub const fn has_recovery(self) -> bool {
        !matches!(self, Self::Authored)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceEventIssue {
    Missing,
    Invalid,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSourceHandlerBody {
    Missing,
    Statement(StmtId),
    Block {
        statements: Box<[StmtId]>,
        closed: bool,
    },
}

impl HirSourceHandlerBody {
    pub fn statements(&self) -> &[StmtId] {
        match self {
            Self::Missing => &[],
            Self::Statement(statement) => std::slice::from_ref(statement),
            Self::Block { statements, .. } => statements,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing | Self::Block { closed: false, .. })
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Missing => Ok(()),
            Self::Statement(statement) => validate_statements(expected, &[*statement]),
            Self::Block { statements, .. } => validate_statements(expected, statements),
        }
    }
}
