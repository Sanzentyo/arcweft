//! Capability, test, Style, and recovery items.

use arcweft_lang_syntax::cst::is_identifier;
use thiserror::Error;

use crate::identity::{ExprId, HirModuleId, ScopeId, StmtId, TypeId};
use crate::leaf::{HirIdRefValue, HirName};

use super::callable::{HirFunctionParameterGroup, HirGenericParameter};
use super::{
    HirItemInvariantError, HirItemPrefix, HirRequiredName, validate_expr, validate_exprs,
    validate_function_parameter_groups, validate_generic_parameters, validate_optional_type,
    validate_scope, validate_statements,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExternCapabilityItem {
    name: HirRequiredName,
    members: Box<[HirCapabilityMember]>,
}

impl HirExternCapabilityItem {
    pub(crate) fn try_new(
        expected: HirModuleId,
        name: HirRequiredName,
        members: Box<[HirCapabilityMember]>,
    ) -> Result<Self, HirItemInvariantError> {
        let declaration = Self { name, members };
        declaration.validate_module(expected)?;
        Ok(declaration)
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn members(&self) -> &[HirCapabilityMember] {
        &self.members
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        for member in &self.members {
            match member {
                HirCapabilityMember::AssociatedType(associated) => {
                    associated.validate_module(expected)?;
                }
                HirCapabilityMember::Function(function) => function.validate_module(expected)?,
                HirCapabilityMember::Error => {}
            }
        }
        Ok(())
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.name.is_recovered() || self.members.iter().any(HirCapabilityMember::has_recovery)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCapabilityMember {
    AssociatedType(HirCapabilityAssociatedType),
    Function(HirCapabilityFunction),
    Error,
}

impl HirCapabilityMember {
    pub(crate) fn has_recovery(&self) -> bool {
        match self {
            Self::AssociatedType(associated) => associated.has_recovery(),
            Self::Function(function) => function.has_recovery(),
            Self::Error => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCapabilityAssociatedType {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    value: Option<TypeId>,
}

impl HirCapabilityAssociatedType {
    pub(crate) fn try_new(
        expected: HirModuleId,
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        value: Option<TypeId>,
    ) -> Result<Self, HirItemInvariantError> {
        let associated = Self {
            prefix,
            name,
            generic_parameters,
            value,
        };
        associated.validate_module(expected)?;
        Ok(associated)
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn value(&self) -> Option<TypeId> {
        self.value
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        self.prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_optional_type(expected, self.value)
    }

    fn has_recovery(&self) -> bool {
        self.name.is_recovered()
            || self
                .generic_parameters
                .iter()
                .any(|parameter| parameter.name().is_recovered())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCapabilityFunction {
    prefix: HirItemPrefix,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameter_groups: Box<[HirFunctionParameterGroup]>,
    return_type: Option<TypeId>,
    callable_scope: ScopeId,
    effects: Box<[ExprId]>,
}

impl HirCapabilityFunction {
    pub(crate) fn try_new(
        prefix: HirItemPrefix,
        name: HirRequiredName,
        generic_parameters: Box<[HirGenericParameter]>,
        parameter_groups: Box<[HirFunctionParameterGroup]>,
        return_type: Option<TypeId>,
        callable_scope: ScopeId,
        effects: Box<[ExprId]>,
    ) -> Result<Self, HirItemInvariantError> {
        let function = Self {
            prefix,
            name,
            generic_parameters,
            parameter_groups,
            return_type,
            callable_scope,
            effects,
        };
        function.validate_module(callable_scope.module())?;
        Ok(function)
    }

    pub const fn prefix(&self) -> &HirItemPrefix {
        &self.prefix
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameter_groups(&self) -> &[HirFunctionParameterGroup] {
        &self.parameter_groups
    }

    pub const fn return_type(&self) -> Option<TypeId> {
        self.return_type
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn effects(&self) -> &[ExprId] {
        &self.effects
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        self.prefix.validate_module(expected)?;
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_function_parameter_groups(expected, &self.parameter_groups)?;
        validate_optional_type(expected, self.return_type)?;
        validate_scope(expected, self.callable_scope)?;
        validate_exprs(expected, &self.effects)
    }

    fn has_recovery(&self) -> bool {
        self.name.is_recovered()
            || self
                .generic_parameters
                .iter()
                .any(|parameter| parameter.name().is_recovered())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTestItem {
    id: HirIdRefValue,
    kind: HirTestKind,
    scope: ScopeId,
    body: Box<[StmtId]>,
}

impl HirTestItem {
    pub(crate) const fn new(
        id: HirIdRefValue,
        kind: HirTestKind,
        scope: ScopeId,
        body: Box<[StmtId]>,
    ) -> Self {
        Self {
            id,
            kind,
            scope,
            body,
        }
    }

    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    pub const fn kind(&self) -> &HirTestKind {
        &self.kind
    }

    pub const fn body(&self) -> &[StmtId] {
        &self.body
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        self.id.recovery_issue().is_some() || self.kind.has_recovery()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_scope(expected, self.scope)?;
        validate_statements(expected, &self.body)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTestKind {
    Scenario,
    Visual,
    Audio,
    Fixture,
    Custom(HirName),
    Recovered(HirTestKindIssue),
}

impl HirTestKind {
    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTestKindIssue {
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBenchItem {
    id: HirIdRefValue,
    scope: ScopeId,
    body: Box<[StmtId]>,
}

impl HirBenchItem {
    pub(crate) const fn new(id: HirIdRefValue, scope: ScopeId, body: Box<[StmtId]>) -> Self {
        Self { id, scope, body }
    }

    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    pub const fn body(&self) -> &[StmtId] {
        &self.body
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub(crate) const fn has_recovery(&self) -> bool {
        self.id.recovery_issue().is_some()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_scope(expected, self.scope)?;
        validate_statements(expected, &self.body)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleItem {
    id: HirIdRefValue,
    tokens: Box<[HirStyleToken]>,
    body: Box<[HirStyleBodyItem]>,
}

impl HirStyleItem {
    pub(crate) fn try_new(
        expected: HirModuleId,
        id: HirIdRefValue,
        tokens: Box<[HirStyleToken]>,
        body: Box<[HirStyleBodyItem]>,
    ) -> Result<Self, HirItemInvariantError> {
        let item = Self { id, tokens, body };
        item.validate_module(expected)?;
        Ok(item)
    }

    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    pub const fn tokens(&self) -> &[HirStyleToken] {
        &self.tokens
    }

    pub const fn body(&self) -> &[HirStyleBodyItem] {
        &self.body
    }

    pub(crate) fn has_recovery(&self) -> bool {
        self.id.is_recovered()
            || self.tokens.iter().any(HirStyleToken::has_recovery)
            || self.body.iter().any(HirStyleBodyItem::has_recovery)
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        for token in &self.tokens {
            token.validate_module(expected)?;
        }
        for item in &self.body {
            item.validate_module(expected)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleToken {
    id: HirIdRefValue,
    value_type: Option<TypeId>,
    value: ExprId,
    recovery: Option<HirStyleTokenIssue>,
}

impl HirStyleToken {
    pub(crate) fn try_new(
        expected: HirModuleId,
        id: HirIdRefValue,
        value_type: Option<TypeId>,
        value: ExprId,
        recovery: Option<HirStyleTokenIssue>,
    ) -> Result<Self, HirItemInvariantError> {
        let token = Self {
            id,
            value_type,
            value,
            recovery,
        };
        token.validate_module(expected)?;
        Ok(token)
    }

    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    pub const fn value_type(&self) -> Option<TypeId> {
        self.value_type
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }

    pub const fn recovery_issue(&self) -> Option<HirStyleTokenIssue> {
        self.recovery
    }

    fn has_recovery(&self) -> bool {
        self.id.is_recovered() || self.recovery.is_some()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_optional_type(expected, self.value_type)?;
        validate_expr(expected, self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleTokenIssue {
    #[error("Style token assignment is missing")]
    MissingAssignment,
    #[error("Style token assignment is malformed")]
    MalformedAssignment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStyleBodyItem {
    Rule(HirStyleRule),
    Environment(HirStyleEnvironment),
    Recovered(HirStyleBodyIssue),
}

impl HirStyleBodyItem {
    pub const fn recovery_issue(&self) -> Option<HirStyleBodyIssue> {
        match self {
            Self::Rule(_) | Self::Environment(_) => None,
            Self::Recovered(issue) => Some(*issue),
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Rule(rule) => rule.has_recovery(),
            Self::Environment(environment) => environment.has_recovery(),
            Self::Recovered(_) => true,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Rule(rule) => rule.validate_module(expected),
            Self::Environment(environment) => environment.validate_module(expected),
            Self::Recovered(_) => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleBodyIssue {
    #[error("Style body member is missing")]
    Missing,
    #[error("Style body member is malformed")]
    Malformed,
    #[error("Style body contains an unexpected member")]
    Unexpected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleRule {
    selector: HirStyleSelector,
    declarations: Box<[HirStyleDeclaration]>,
}

impl HirStyleRule {
    pub(crate) fn try_new(
        expected: HirModuleId,
        selector: HirStyleSelector,
        declarations: Box<[HirStyleDeclaration]>,
    ) -> Result<Self, HirItemInvariantError> {
        let rule = Self {
            selector,
            declarations,
        };
        rule.validate_module(expected)?;
        Ok(rule)
    }

    pub const fn selector(&self) -> &HirStyleSelector {
        &self.selector
    }

    pub const fn declarations(&self) -> &[HirStyleDeclaration] {
        &self.declarations
    }

    fn has_recovery(&self) -> bool {
        self.selector.has_recovery()
            || self
                .declarations
                .iter()
                .any(HirStyleDeclaration::has_recovery)
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        for declaration in &self.declarations {
            declaration.validate_module(expected)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleSelector {
    Resolved(Box<[HirStyleSelectorSequence]>),
    Recovered {
        sequences: Box<[HirStyleSelectorSequence]>,
        issue: HirStyleSelectorIssue,
    },
}

impl HirStyleSelector {
    pub(crate) fn try_new(
        sequences: Box<[HirStyleSelectorSequence]>,
    ) -> Result<Self, HirStyleSelectorIssue> {
        validate_resolved_style_selector(&sequences)?;
        Ok(Self::Resolved(sequences))
    }

    pub(crate) const fn recovered(
        sequences: Box<[HirStyleSelectorSequence]>,
        issue: HirStyleSelectorIssue,
    ) -> Self {
        Self::Recovered { sequences, issue }
    }

    pub const fn sequences(&self) -> &[HirStyleSelectorSequence] {
        match self {
            Self::Resolved(sequences) | Self::Recovered { sequences, .. } => sequences,
        }
    }

    pub const fn recovery_issue(&self) -> Option<HirStyleSelectorIssue> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered { issue, .. } => Some(*issue),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered { .. })
    }
}

fn validate_resolved_style_selector(
    sequences: &[HirStyleSelectorSequence],
) -> Result<(), HirStyleSelectorIssue> {
    let Some((first, rest)) = sequences.split_first() else {
        return Err(HirStyleSelectorIssue::MissingSequence);
    };
    if first.relation_to_previous.is_some()
        || rest
            .iter()
            .any(|sequence| sequence.relation_to_previous.is_none())
    {
        return Err(HirStyleSelectorIssue::InvalidRelation);
    }
    if sequences.iter().any(HirStyleSelectorSequence::is_empty) {
        return Err(HirStyleSelectorIssue::MissingComponent);
    }
    if sequences.iter().any(HirStyleSelectorSequence::has_recovery) {
        return Err(HirStyleSelectorIssue::InvalidComponent);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleSelectorIssue {
    #[error("Style selector has no sequence")]
    MissingSequence,
    #[error("Style selector sequence has no component")]
    MissingComponent,
    #[error("Style selector relation is inconsistent with source order")]
    InvalidRelation,
    #[error("Style selector contains an invalid component")]
    InvalidComponent,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStyleSelectorSequence {
    relation_to_previous: Option<HirStyleCombinator>,
    element: Option<HirStyleName>,
    part: Option<HirStyleName>,
    predicates: Box<[HirStyleName]>,
}

impl HirStyleSelectorSequence {
    pub(crate) const fn new(
        relation_to_previous: Option<HirStyleCombinator>,
        element: Option<HirStyleName>,
        part: Option<HirStyleName>,
        predicates: Box<[HirStyleName]>,
    ) -> Self {
        Self {
            relation_to_previous,
            element,
            part,
            predicates,
        }
    }

    pub const fn relation_to_previous(&self) -> Option<HirStyleCombinator> {
        self.relation_to_previous
    }

    pub const fn element(&self) -> Option<&HirStyleName> {
        self.element.as_ref()
    }

    pub const fn part(&self) -> Option<&HirStyleName> {
        self.part.as_ref()
    }

    pub const fn predicates(&self) -> &[HirStyleName] {
        &self.predicates
    }

    fn is_empty(&self) -> bool {
        self.element.is_none() && self.part.is_none() && self.predicates.is_empty()
    }

    fn has_recovery(&self) -> bool {
        self.element
            .iter()
            .chain(self.part.iter())
            .chain(self.predicates.iter())
            .any(HirStyleName::has_recovery)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleCombinator {
    Descendant,
    Child,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleName {
    Resolved(Box<str>),
    Recovered(HirStyleNameIssue),
}

impl HirStyleName {
    pub(crate) fn try_new(value: Box<str>) -> Result<Self, HirStyleNameIssue> {
        if value.is_empty() {
            return Err(HirStyleNameIssue::Missing);
        }
        if !value.split('-').all(is_identifier) {
            return Err(HirStyleNameIssue::Invalid);
        }
        Ok(Self::Resolved(value))
    }

    pub(crate) const fn recovered(issue: HirStyleNameIssue) -> Self {
        Self::Recovered(issue)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Resolved(value) => Some(value),
            Self::Recovered(_) => None,
        }
    }

    pub const fn recovery_issue(&self) -> Option<HirStyleNameIssue> {
        match self {
            Self::Resolved(_) => None,
            Self::Recovered(issue) => Some(*issue),
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleNameIssue {
    #[error("Style name is missing")]
    Missing,
    #[error("Style name is not a valid identifier or hyphen-separated identifier")]
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleDeclaration {
    property: HirStyleName,
    value: ExprId,
    operation: HirStyleAssignOperation,
}

impl HirStyleDeclaration {
    pub(crate) fn try_new(
        expected: HirModuleId,
        property: HirStyleName,
        value: ExprId,
        operation: HirStyleAssignOperation,
    ) -> Result<Self, HirItemInvariantError> {
        validate_expr(expected, value)?;
        Ok(Self {
            property,
            value,
            operation,
        })
    }

    pub const fn property(&self) -> &HirStyleName {
        &self.property
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }

    pub const fn operation(&self) -> HirStyleAssignOperation {
        self.operation
    }

    fn has_recovery(&self) -> bool {
        self.property.has_recovery() || self.operation.has_recovery()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_expr(expected, self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleAssignOperation {
    Replace,
    Append,
    Recovered(HirStyleAssignOperationIssue),
}

impl HirStyleAssignOperation {
    pub const fn recovery_issue(self) -> Option<HirStyleAssignOperationIssue> {
        match self {
            Self::Replace | Self::Append => None,
            Self::Recovered(issue) => Some(issue),
        }
    }

    pub const fn has_recovery(self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleAssignOperationIssue {
    #[error("Style assignment operation is missing")]
    Missing,
    #[error("Style assignment operation is malformed")]
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStyleEnvironment {
    clauses: Box<[HirStyleEnvironmentClause]>,
    body: Box<[HirStyleBodyItem]>,
}

impl HirStyleEnvironment {
    pub(crate) fn try_new(
        expected: HirModuleId,
        clauses: Box<[HirStyleEnvironmentClause]>,
        body: Box<[HirStyleBodyItem]>,
    ) -> Result<Self, HirItemInvariantError> {
        let environment = Self { clauses, body };
        environment.validate_module(expected)?;
        Ok(environment)
    }

    pub const fn clauses(&self) -> &[HirStyleEnvironmentClause] {
        &self.clauses
    }

    pub const fn body(&self) -> &[HirStyleBodyItem] {
        &self.body
    }

    fn has_recovery(&self) -> bool {
        self.clauses
            .iter()
            .any(HirStyleEnvironmentClause::has_recovery)
            || self.body.iter().any(HirStyleBodyItem::has_recovery)
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        for clause in &self.clauses {
            clause.validate_module(expected)?;
        }
        for item in &self.body {
            item.validate_module(expected)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirStyleEnvironmentClause {
    field: HirStyleEnvironmentField,
    comparison: HirStyleEnvironmentComparison,
    value: ExprId,
}

impl HirStyleEnvironmentClause {
    pub(crate) fn try_new(
        expected: HirModuleId,
        field: HirStyleEnvironmentField,
        comparison: HirStyleEnvironmentComparison,
        value: ExprId,
    ) -> Result<Self, HirItemInvariantError> {
        validate_expr(expected, value)?;
        Ok(Self {
            field,
            comparison,
            value,
        })
    }

    pub const fn field(self) -> HirStyleEnvironmentField {
        self.field
    }

    pub const fn comparison(self) -> HirStyleEnvironmentComparison {
        self.comparison
    }

    pub const fn value(self) -> ExprId {
        self.value
    }

    pub const fn has_recovery(&self) -> bool {
        self.field.has_recovery() || self.comparison.has_recovery()
    }

    fn validate_module(self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_expr(expected, self.value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleEnvironmentField {
    ColorScheme,
    Contrast,
    ReducedMotion,
    TextScale,
    Recovered(HirStyleEnvironmentFieldIssue),
}

impl HirStyleEnvironmentField {
    pub const fn recovery_issue(self) -> Option<HirStyleEnvironmentFieldIssue> {
        match self {
            Self::ColorScheme | Self::Contrast | Self::ReducedMotion | Self::TextScale => None,
            Self::Recovered(issue) => Some(issue),
        }
    }

    pub const fn has_recovery(self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleEnvironmentFieldIssue {
    #[error("Style environment field is missing")]
    Missing,
    #[error("Style environment field is unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleEnvironmentComparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Recovered(HirStyleEnvironmentComparisonIssue),
}

impl HirStyleEnvironmentComparison {
    pub const fn recovery_issue(self) -> Option<HirStyleEnvironmentComparisonIssue> {
        match self {
            Self::Equal
            | Self::NotEqual
            | Self::Less
            | Self::LessOrEqual
            | Self::Greater
            | Self::GreaterOrEqual => None,
            Self::Recovered(issue) => Some(issue),
        }
    }

    pub const fn has_recovery(self) -> bool {
        matches!(self, Self::Recovered(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirStyleEnvironmentComparisonIssue {
    #[error("Style environment comparison is missing")]
    Missing,
    #[error("Style environment comparison is malformed")]
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirErrorItem;

impl HirErrorItem {
    pub(crate) const fn new() -> Self {
        Self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirItemIssue {
    MissingName,
    MissingId,
    MissingKind,
    MissingType,
    MissingBody,
    MalformedHeader,
    InvalidMember,
    Recovery,
    UnclassifiedSyntax,
    TransactionalChildFailure,
}

#[cfg(test)]
mod tests;
