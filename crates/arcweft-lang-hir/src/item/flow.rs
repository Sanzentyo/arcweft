//! Final semantic payload for one ordinary `flow` declaration.

use crate::expr::HirThreadBody;
use crate::identity::{
    ExprId, HirModuleId, ItemId, LocalId, PatternId, ScopeId, StmtId, SyntheticOwner, TypeId,
};
use crate::leaf::{HirIdRef, HirName};
use crate::source_index::HirSourceQuery;

use super::callable::{
    HirContractScopes, HirGenericParameter, HirParameter, HirParameterKind, HirWherePredicate,
};
use super::{
    HirItemInvariantError, validate_contract_scopes, validate_expr, validate_exprs,
    validate_generic_parameters, validate_optional_type, validate_parameters,
    validate_where_predicates,
};

/// Exact authored declaration-identity state retained by one recognized Flow.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIdentity {
    Name { name: HirName },
    PublicId { public_id: HirIdRef },
    PublicIdAndName { public_id: HirIdRef, name: HirName },
    Missing,
}

impl HirFlowIdentity {
    pub const fn public_id(&self) -> Option<&HirIdRef> {
        match self {
            Self::PublicId { public_id } | Self::PublicIdAndName { public_id, .. } => {
                Some(public_id)
            }
            Self::Name { .. } | Self::Missing => None,
        }
    }

    pub const fn name(&self) -> Option<&HirName> {
        match self {
            Self::Name { name } | Self::PublicIdAndName { name, .. } => Some(name),
            Self::PublicId { .. } | Self::Missing => None,
        }
    }

    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

/// Semantic Flow return shape without fabricating a type node for omitted Unit.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowReturn {
    OmittedUnit,
    Authored(TypeId),
}

impl HirFlowReturn {
    pub const fn authored_type(&self) -> Option<TypeId> {
        match self {
            Self::OmittedUnit => None,
            Self::Authored(ty) => Some(*ty),
        }
    }
}

/// Authored proof/checking mode for one condition-form Flow contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirContractMode {
    Default,
    Prove,
    CheckRuntime,
    DebugCheck,
}

/// One condition-form contract expression and its authored checking mode.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContractCondition {
    mode: HirContractMode,
    expression: ExprId,
}

impl HirContractCondition {
    pub(crate) const fn new(mode: HirContractMode, expression: ExprId) -> Self {
        Self { mode, expression }
    }

    pub const fn mode(&self) -> HirContractMode {
        self.mode
    }

    pub const fn expression(&self) -> ExprId {
        self.expression
    }
}

/// Source-ordered operands for list-form Flow contracts.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContractOperandList {
    operands: Box<[ExprId]>,
}

impl HirContractOperandList {
    pub(crate) fn try_new(
        expected: HirModuleId,
        operands: Box<[ExprId]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_exprs(expected, &operands)?;
        Ok(Self { operands })
    }

    pub const fn operands(&self) -> &[ExprId] {
        &self.operands
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        validate_exprs(expected, &self.operands)
    }
}

/// One Flow contract clause in exact heterogeneous source order.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowContractClause {
    Requires(HirContractCondition),
    Ensures(HirContractCondition),
    Invariant(HirContractCondition),
    Assume { expression: ExprId },
    Reads(HirContractOperandList),
    Effects(HirContractOperandList),
    NoEffect { expression: ExprId },
    Modifies(HirContractOperandList),
    Decreases { expression: ExprId },
}

impl HirFlowContractClause {
    pub const fn is_ensures(&self) -> bool {
        matches!(self, Self::Ensures(_))
    }

    /// Returns the authored effects admitted by this clause.
    ///
    /// `no_effect` is a prohibition and therefore participates in typed
    /// effect-identity validation without becoming part of the Flow's exposed
    /// effect row.
    pub const fn admitted_effect_operands(&self) -> Option<&[ExprId]> {
        match self {
            Self::Effects(operands) => Some(operands.operands()),
            Self::Requires(_)
            | Self::Ensures(_)
            | Self::Invariant(_)
            | Self::Assume { .. }
            | Self::Reads(_)
            | Self::NoEffect { .. }
            | Self::Modifies(_)
            | Self::Decreases { .. } => None,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirItemInvariantError> {
        match self {
            Self::Requires(condition) | Self::Ensures(condition) | Self::Invariant(condition) => {
                validate_expr(expected, condition.expression())
            }
            Self::Assume { expression }
            | Self::NoEffect { expression }
            | Self::Decreases { expression } => validate_expr(expected, *expression),
            Self::Reads(operands) | Self::Effects(operands) | Self::Modifies(operands) => {
                operands.validate_module(expected)
            }
        }
    }
}

/// Typed view of the one synthetic postcondition `result` local.
///
/// Its type and `PostconditionResult` origin remain authoritative in the
/// existing module-local [`crate::scope::HirLocal`] arena.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowResultLocal {
    local: LocalId,
}

impl HirFlowResultLocal {
    pub(crate) const fn new(local: LocalId) -> Self {
        Self { local }
    }

    pub const fn local(self) -> LocalId {
        self.local
    }
}

/// Flow-level issue class in canonical poison-precedence order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIssueClass {
    Prefix,
    Identity,
    Signature,
    Contract,
    MissingBody,
    BodyChild,
    UnclosedBody,
    TrailingRecovery,
}

/// Exact final-HIR owner to which one Flow issue belongs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirFlowIssueOwner {
    Item(ItemId),
    Scope(ScopeId),
    Local(LocalId),
    Stmt(StmtId),
    Expr(ExprId),
    Pattern(PatternId),
    Type(TypeId),
}

impl HirFlowIssueOwner {
    const fn synthetic_owner(self) -> SyntheticOwner {
        match self {
            Self::Item(owner) => SyntheticOwner::Item(owner),
            Self::Scope(owner) => SyntheticOwner::Scope(owner),
            Self::Local(owner) => SyntheticOwner::Local(owner),
            Self::Stmt(owner) => SyntheticOwner::Stmt(owner),
            Self::Expr(owner) => SyntheticOwner::Expr(owner),
            Self::Pattern(owner) => SyntheticOwner::Pattern(owner),
            Self::Type(owner) => SyntheticOwner::Type(owner),
        }
    }

    pub const fn module(self) -> HirModuleId {
        self.synthetic_owner().module()
    }
}

/// One roleful Flow issue anchored by the sole central source query.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowIssue {
    class: HirFlowIssueClass,
    owner: HirFlowIssueOwner,
    source: HirSourceQuery,
}

impl HirFlowIssue {
    pub(crate) const fn new(
        class: HirFlowIssueClass,
        owner: HirFlowIssueOwner,
        source: HirSourceQuery,
    ) -> Self {
        Self {
            class,
            owner,
            source,
        }
    }

    pub const fn class(&self) -> HirFlowIssueClass {
        self.class
    }

    pub const fn owner(&self) -> HirFlowIssueOwner {
        self.owner
    }

    pub(crate) const fn source(&self) -> &HirSourceQuery {
        &self.source
    }

    fn validate_owner(&self, flow: ItemId) -> Result<(), HirItemInvariantError> {
        let expected = flow.module();
        super::validate_module(expected, self.owner.module())?;
        super::validate_module(expected, self.source.owner().module())?;
        if let HirFlowIssueOwner::Item(actual) = self.owner
            && actual != flow
        {
            return Err(HirItemInvariantError::FlowIssueItemOwner {
                expected: flow,
                actual,
            });
        }
        if let SyntheticOwner::Item(actual) = self.source.owner()
            && actual != flow
        {
            return Err(HirItemInvariantError::FlowIssueItemOwner {
                expected: flow,
                actual,
            });
        }
        Ok(())
    }
}

/// Canonically ordered Flow poison: one primary issue and ordered related evidence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirFlowPoison {
    primary: Option<HirFlowIssue>,
    related: Box<[HirFlowIssue]>,
}

impl HirFlowPoison {
    pub(crate) fn clean() -> Self {
        Self {
            primary: None,
            related: Box::new([]),
        }
    }

    /// Canonicalizes the package-defined top-level poison precedence while
    /// preserving the caller's source order within each issue class.
    ///
    /// Lowering supplies the exact within-class order, including special
    /// later-primary/earlier-related pairs such as duplicate `decreases`.
    /// Stable class sorting makes it impossible for an independently assembled
    /// later class to displace an earlier class as the Flow primary issue.
    pub(crate) fn from_ordered_issues(issues: Box<[HirFlowIssue]>) -> Self {
        let mut issues = issues.into_vec();
        issues.sort_by_key(HirFlowIssue::class);
        let mut issues = issues.into_iter();
        let primary = issues.next();
        let related = issues.collect::<Vec<_>>().into_boxed_slice();
        Self { primary, related }
    }

    pub const fn primary(&self) -> Option<&HirFlowIssue> {
        self.primary.as_ref()
    }

    pub const fn related(&self) -> &[HirFlowIssue] {
        &self.related
    }

    pub const fn is_poisoned(&self) -> bool {
        self.primary.is_some()
    }

    fn contains_class(&self, class: HirFlowIssueClass) -> bool {
        self.primary
            .iter()
            .chain(self.related.iter())
            .any(|issue| issue.class() == class)
    }

    fn validate_owner(&self, flow: ItemId) -> Result<(), HirItemInvariantError> {
        if self.primary.is_none() && !self.related.is_empty() {
            return Err(HirItemInvariantError::InvalidFlowPoison);
        }
        if let Some(primary) = &self.primary {
            primary.validate_owner(flow)?;
        }
        for issue in &self.related {
            issue.validate_owner(flow)?;
        }
        Ok(())
    }
}

/// One final ordinary Flow declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFlowItem {
    identity: HirFlowIdentity,
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    result: HirFlowReturn,
    where_predicates: Box<[HirWherePredicate]>,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
    result_local: Option<HirFlowResultLocal>,
    contracts: Box<[HirFlowContractClause]>,
    body: HirThreadBody,
    poison: HirFlowPoison,
}

impl HirFlowItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        owner: ItemId,
        identity: HirFlowIdentity,
        generic_parameters: Box<[HirGenericParameter]>,
        parameters: Box<[HirParameter]>,
        result: HirFlowReturn,
        where_predicates: Box<[HirWherePredicate]>,
        scopes: HirContractScopes,
        result_local: Option<HirFlowResultLocal>,
        contracts: Box<[HirFlowContractClause]>,
        body: HirThreadBody,
        poison: HirFlowPoison,
    ) -> Result<Self, HirItemInvariantError> {
        let item = Self {
            identity,
            generic_parameters,
            parameters,
            result,
            where_predicates,
            callable_scope: scopes.callable(),
            requires_scope: scopes.requires(),
            ensures_scope: scopes.ensures(),
            result_local,
            contracts,
            body,
            poison,
        };
        item.validate_owner(owner)?;
        Ok(item)
    }

    pub const fn identity(&self) -> &HirFlowIdentity {
        &self.identity
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }

    pub const fn result(&self) -> &HirFlowReturn {
        &self.result
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn callable_scope(&self) -> ScopeId {
        self.callable_scope
    }

    pub const fn requires_scope(&self) -> ScopeId {
        self.requires_scope
    }

    pub const fn ensures_scope(&self) -> ScopeId {
        self.ensures_scope
    }

    pub const fn result_local(&self) -> Option<HirFlowResultLocal> {
        self.result_local
    }

    pub const fn contracts(&self) -> &[HirFlowContractClause] {
        &self.contracts
    }

    pub const fn body_scope(&self) -> ScopeId {
        self.body.scope()
    }

    pub const fn body(&self) -> &HirThreadBody {
        &self.body
    }

    pub const fn poison(&self) -> &HirFlowPoison {
        &self.poison
    }

    pub(super) const fn has_recovery(&self) -> bool {
        self.identity.is_missing() || self.poison.is_poisoned()
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        self.validate(expected, None)
    }

    fn validate_owner(&self, owner: ItemId) -> Result<(), HirItemInvariantError> {
        self.validate(owner.module(), Some(owner))
    }

    fn validate(
        &self,
        expected: HirModuleId,
        owner: Option<ItemId>,
    ) -> Result<(), HirItemInvariantError> {
        validate_generic_parameters(expected, &self.generic_parameters)?;
        validate_parameters(expected, &self.parameters)?;
        if self.parameters.iter().any(|parameter| {
            parameter.kind() != HirParameterKind::Fixed || parameter.default().is_some()
        }) {
            return Err(HirItemInvariantError::InvalidFlowParameterShape);
        }
        validate_optional_type(expected, self.result.authored_type())?;
        validate_where_predicates(expected, &self.where_predicates)?;
        validate_contract_scopes(
            expected,
            self.callable_scope,
            self.requires_scope,
            self.ensures_scope,
        )?;
        let body_scope = self.body.scope();
        super::validate_module(expected, body_scope.module())?;
        if [self.callable_scope, self.requires_scope, self.ensures_scope].contains(&body_scope) {
            return Err(HirItemInvariantError::FlowScopeIdentityCollision);
        }
        self.body
            .validate_module(expected)
            .map_err(|actual| HirItemInvariantError::ForeignChild { expected, actual })?;
        for contract in &self.contracts {
            contract.validate_module(expected)?;
        }
        let has_ensures = self.contracts.iter().any(HirFlowContractClause::is_ensures);
        if has_ensures != self.result_local.is_some() {
            return Err(HirItemInvariantError::InvalidFlowResultLocal);
        }
        if let Some(result_local) = self.result_local {
            super::validate_module(expected, result_local.local().module())?;
        }
        if self.identity.is_missing() && !self.poison.contains_class(HirFlowIssueClass::Identity) {
            return Err(HirItemInvariantError::InvalidFlowPoison);
        }
        if let Some(owner) = owner {
            self.poison.validate_owner(owner)?;
        } else if let Some(issue) = self
            .poison
            .primary()
            .into_iter()
            .chain(self.poison.related().iter())
            .find(|issue| {
                issue.owner().module() != expected || issue.source().owner().module() != expected
            })
        {
            super::validate_module(expected, issue.owner().module())?;
            super::validate_module(expected, issue.source().owner().module())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "flow/tests.rs"]
mod tests;
