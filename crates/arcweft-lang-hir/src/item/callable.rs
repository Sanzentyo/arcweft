//! Callable, flow, predicate, and proof item payloads.

use arcweft_id::PublicId;

use crate::identity::{ExprId, HirModuleId, LocalId, PatternId, ScopeId, StmtId, TypeId};
use crate::proof_return::HirProofReturnSemanticClass;

use super::{
    HirContractOperandList, HirItemInvariantError, HirRequiredName, validate_contract_scopes,
    validate_function_body, validate_function_signature, validate_locals, validate_optional_expr,
    validate_parameters, validate_predicate_body, validate_proof_body, validate_signature,
    validate_type, validate_types,
};

/// One final generic parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirGenericParameter {
    Lifetime {
        name: HirRequiredName,
    },
    Type {
        name: HirRequiredName,
        bounds: Box<[TypeId]>,
    },
}

impl HirGenericParameter {
    pub(crate) const fn lifetime(name: HirRequiredName) -> Self {
        Self::Lifetime { name }
    }

    pub(crate) const fn ty(name: HirRequiredName, bounds: Box<[TypeId]>) -> Self {
        Self::Type { name, bounds }
    }

    pub const fn name(&self) -> &HirRequiredName {
        match self {
            Self::Lifetime { name } | Self::Type { name, .. } => name,
        }
    }

    pub fn bounds(&self) -> &[TypeId] {
        match self {
            Self::Lifetime { .. } => &[],
            Self::Type { bounds, .. } => bounds,
        }
    }
}

/// Arity behavior retained for one source-ordered callable parameter.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirParameterKind {
    Fixed,
    RestPositional,
}

/// One source-ordered callable parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirParameter {
    pattern: PatternId,
    ty: TypeId,
    kind: HirParameterKind,
    default: Option<ExprId>,
    locals: Box<[LocalId]>,
}

impl HirParameter {
    pub(crate) fn try_new(
        pattern: PatternId,
        ty: TypeId,
        kind: HirParameterKind,
        default: Option<ExprId>,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirItemInvariantError> {
        let expected = pattern.module();
        validate_type(expected, ty)?;
        validate_optional_expr(expected, default)?;
        validate_locals(expected, &locals)?;
        Ok(Self {
            pattern,
            ty,
            kind,
            default,
            locals,
        })
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    pub const fn kind(&self) -> HirParameterKind {
        self.kind
    }

    pub const fn default(&self) -> Option<ExprId> {
        self.default
    }

    pub const fn locals(&self) -> &[LocalId] {
        &self.locals
    }
}

/// One source-ordered ordinary-function parameter group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionParameterGroup {
    parameters: Box<[HirParameter]>,
}

impl HirFunctionParameterGroup {
    pub(crate) fn try_new(
        expected: HirModuleId,
        parameters: Box<[HirParameter]>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_parameters(expected, &parameters)?;
        Ok(Self { parameters })
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }
}

/// One source-ordered where predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirWherePredicate {
    subject: TypeId,
    bounds: Box<[TypeId]>,
}

impl HirWherePredicate {
    pub(crate) fn try_new(
        subject: TypeId,
        bounds: Box<[TypeId]>,
    ) -> Result<Self, HirItemInvariantError> {
        if bounds.is_empty() {
            return Err(HirItemInvariantError::EmptyWhereBounds);
        }
        validate_types(subject.module(), &bounds)?;
        Ok(Self { subject, bounds })
    }

    pub const fn subject(&self) -> TypeId {
        self.subject
    }

    pub const fn bounds(&self) -> &[TypeId] {
        &self.bounds
    }
}

/// Source-ordered function body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFunctionBody {
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    },
    Error(ExprId),
}

/// Typed callable signature accepted by source-item constructors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableSignature {
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    return_type: TypeId,
}

impl HirCallableSignature {
    pub(crate) fn try_new(
        generic_parameters: Box<[HirGenericParameter]>,
        parameters: Box<[HirParameter]>,
        where_predicates: Box<[HirWherePredicate]>,
        requires: Box<[ExprId]>,
        ensures: Box<[ExprId]>,
        return_type: TypeId,
    ) -> Result<Self, HirItemInvariantError> {
        validate_signature(
            return_type.module(),
            &generic_parameters,
            &parameters,
            &where_predicates,
            &requires,
            &ensures,
            return_type,
        )?;
        Ok(Self {
            generic_parameters,
            parameters,
            where_predicates,
            requires,
            ensures,
            return_type,
        })
    }
}

/// Typed ordinary-function signature, retaining an omitted return without a fabricated type ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionSignature {
    generic_parameters: Box<[HirGenericParameter]>,
    parameter_groups: Box<[HirFunctionParameterGroup]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    effects: Box<[HirContractOperandList]>,
    return_type: Option<TypeId>,
}

impl HirFunctionSignature {
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor validates every field of the closed ordinary-function signature schema"
    )]
    pub(crate) fn try_new(
        expected: HirModuleId,
        generic_parameters: Box<[HirGenericParameter]>,
        parameter_groups: Box<[HirFunctionParameterGroup]>,
        where_predicates: Box<[HirWherePredicate]>,
        requires: Box<[ExprId]>,
        ensures: Box<[ExprId]>,
        effects: Box<[HirContractOperandList]>,
        return_type: Option<TypeId>,
    ) -> Result<Self, HirItemInvariantError> {
        validate_function_signature(
            expected,
            &generic_parameters,
            &parameter_groups,
            &where_predicates,
            &requires,
            &ensures,
            &effects,
            return_type,
        )?;
        Ok(Self {
            generic_parameters,
            parameter_groups,
            where_predicates,
            requires,
            ensures,
            effects,
            return_type,
        })
    }
}

/// Callable, `requires`, and `ensures` scopes retained as distinct identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirContractScopes {
    callable: ScopeId,
    requires: ScopeId,
    ensures: ScopeId,
}

impl HirContractScopes {
    pub(crate) fn try_new(
        callable: ScopeId,
        requires: ScopeId,
        ensures: ScopeId,
    ) -> Result<Self, HirItemInvariantError> {
        validate_contract_scopes(callable.module(), callable, requires, ensures)?;
        Ok(Self {
            callable,
            requires,
            ensures,
        })
    }

    pub const fn callable(self) -> ScopeId {
        self.callable
    }

    pub const fn requires(self) -> ScopeId {
        self.requires
    }

    pub const fn ensures(self) -> ScopeId {
        self.ensures
    }

    pub(super) fn validate_module(
        self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_contract_scopes(expected, self.callable, self.requires, self.ensures)
    }
}

/// One final ordinary function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionItem {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameter_groups: Box<[HirFunctionParameterGroup]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    effects: Box<[HirContractOperandList]>,
    return_type: Option<TypeId>,
    body: HirFunctionBody,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
}

impl HirFunctionItem {
    pub(crate) fn try_new(
        name: HirRequiredName,
        signature: HirFunctionSignature,
        body: HirFunctionBody,
        scopes: HirContractScopes,
    ) -> Result<Self, HirItemInvariantError> {
        let expected = scopes.callable.module();
        validate_function_signature(
            expected,
            &signature.generic_parameters,
            &signature.parameter_groups,
            &signature.where_predicates,
            &signature.requires,
            &signature.ensures,
            &signature.effects,
            signature.return_type,
        )?;
        validate_function_body(expected, &body)?;
        scopes.validate_module(expected)?;
        Ok(Self {
            name,
            generic_parameters: signature.generic_parameters,
            parameter_groups: signature.parameter_groups,
            where_predicates: signature.where_predicates,
            requires: signature.requires,
            ensures: signature.ensures,
            effects: signature.effects,
            return_type: signature.return_type,
            body,
            callable_scope: scopes.callable,
            requires_scope: scopes.requires,
            ensures_scope: scopes.ensures,
        })
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

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn requires(&self) -> &[ExprId] {
        &self.requires
    }

    pub const fn ensures(&self) -> &[ExprId] {
        &self.ensures
    }

    /// Authored effect upper-bound clauses in exact source order.
    ///
    /// An empty slice means the row was omitted (infer-only). One empty
    /// operand list means an explicit closed empty `effects {}` row.
    pub const fn effect_clauses(&self) -> &[HirContractOperandList] {
        &self.effects
    }

    pub const fn return_type(&self) -> Option<TypeId> {
        self.return_type
    }

    pub const fn body(&self) -> &HirFunctionBody {
        &self.body
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

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_function_signature(
            expected,
            &self.generic_parameters,
            &self.parameter_groups,
            &self.where_predicates,
            &self.requires,
            &self.ensures,
            &self.effects,
            self.return_type,
        )?;
        validate_contract_scopes(
            expected,
            self.callable_scope,
            self.requires_scope,
            self.ensures_scope,
        )?;
        validate_function_body(expected, &self.body)
    }
}

/// Final predicate record with its three distinct contract scopes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPredicate {
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    return_type: TypeId,
    body: HirPredicateBody,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
}

impl HirPredicate {
    pub(crate) fn try_new(
        name: HirRequiredName,
        signature: HirCallableSignature,
        body: HirPredicateBody,
        scopes: HirContractScopes,
    ) -> Result<Self, HirItemInvariantError> {
        let expected = signature.return_type.module();
        validate_predicate_body(expected, &body)?;
        validate_contract_scopes(expected, scopes.callable, scopes.requires, scopes.ensures)?;
        Ok(Self {
            name,
            generic_parameters: signature.generic_parameters,
            parameters: signature.parameters,
            where_predicates: signature.where_predicates,
            requires: signature.requires,
            ensures: signature.ensures,
            return_type: signature.return_type,
            body,
            callable_scope: scopes.callable,
            requires_scope: scopes.requires,
            ensures_scope: scopes.ensures,
        })
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn requires(&self) -> &[ExprId] {
        &self.requires
    }

    pub const fn ensures(&self) -> &[ExprId] {
        &self.ensures
    }

    pub const fn return_type(&self) -> TypeId {
        self.return_type
    }

    pub const fn body(&self) -> &HirPredicateBody {
        &self.body
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

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_signature(
            expected,
            &self.generic_parameters,
            &self.parameters,
            &self.where_predicates,
            &self.requires,
            &self.ensures,
            self.return_type,
        )?;
        validate_contract_scopes(
            expected,
            self.callable_scope,
            self.requires_scope,
            self.ensures_scope,
        )?;
        validate_predicate_body(expected, &self.body)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirPredicateBody {
    Expression {
        scope: ScopeId,
        expression: ExprId,
    },
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    },
    Error {
        scope: ScopeId,
        expression: ExprId,
    },
}

impl HirPredicateBody {
    pub const fn scope(&self) -> ScopeId {
        match self {
            Self::Expression { scope, .. }
            | Self::Block { scope, .. }
            | Self::Error { scope, .. } => *scope,
        }
    }
}

/// Non-blank decoded justification carried by semantic trusted Proof metadata.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustReason(Box<str>);

impl TrustReason {
    /// Admits one decoded reason when Unicode trimming leaves content.
    pub fn try_new(decoded: impl Into<Box<str>>) -> Result<Self, TrustReasonError> {
        let decoded = decoded.into();
        if decoded.trim().is_empty() {
            return Err(TrustReasonError::Empty);
        }
        Ok(Self(decoded))
    }

    /// Returns the exact decoded reason bytes without normalization.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Invalid semantic trusted-Proof reason.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TrustReasonError {
    /// The decoded reason was empty or Unicode whitespace only.
    Empty,
}

/// Semantic Proof trust. Exact attribute and reason coordinates live only in
/// `HirSourceIndex`; `Recovery` is an explicit poisoned metadata state and is
/// never equivalent to ordinary verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofTrust {
    /// Ordinary Proof whose body must be verified.
    Verified,
    /// Proof admitted directly with a non-blank justification.
    Trusted {
        /// Exact decoded non-blank justification.
        reason: TrustReason,
    },
    /// Malformed trust metadata retained as explicit poisoned recovery.
    Recovery,
}

impl ProofTrust {
    /// Whether this Proof is admitted directly instead of through verification.
    pub const fn is_directly_trusted(&self) -> bool {
        matches!(self, Self::Trusted { .. })
    }

    /// Returns the direct-trust reason when one is retained.
    pub const fn reason(&self) -> Option<&TrustReason> {
        match self {
            Self::Trusted { reason } => Some(reason),
            Self::Verified | Self::Recovery => None,
        }
    }

    /// Whether malformed trust metadata poisoned this Proof.
    pub const fn is_recovery(&self) -> bool {
        matches!(self, Self::Recovery)
    }
}

/// Final proof record with its three distinct contract scopes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProof {
    public_id: Option<PublicId>,
    name: HirRequiredName,
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirParameter]>,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
    return_type: TypeId,
    return_semantic_class: HirProofReturnSemanticClass,
    trust: ProofTrust,
    body: HirProofBody,
    callable_scope: ScopeId,
    requires_scope: ScopeId,
    ensures_scope: ScopeId,
}

impl HirProof {
    pub(crate) fn try_new(
        name: HirRequiredName,
        public_id: Option<PublicId>,
        signature: HirCallableSignature,
        return_semantic_class: HirProofReturnSemanticClass,
        trust: ProofTrust,
        body: HirProofBody,
        scopes: HirContractScopes,
    ) -> Result<Self, HirItemInvariantError> {
        let expected = signature.return_type.module();
        validate_proof_body(expected, &body)?;
        validate_contract_scopes(expected, scopes.callable, scopes.requires, scopes.ensures)?;
        Ok(Self {
            public_id,
            name,
            generic_parameters: signature.generic_parameters,
            parameters: signature.parameters,
            where_predicates: signature.where_predicates,
            requires: signature.requires,
            ensures: signature.ensures,
            return_type: signature.return_type,
            return_semantic_class,
            trust,
            body,
            callable_scope: scopes.callable,
            requires_scope: scopes.requires,
            ensures_scope: scopes.ensures,
        })
    }

    pub const fn name(&self) -> &HirRequiredName {
        &self.name
    }

    pub const fn public_id(&self) -> Option<&PublicId> {
        self.public_id.as_ref()
    }

    pub const fn generic_parameters(&self) -> &[HirGenericParameter] {
        &self.generic_parameters
    }

    pub const fn parameters(&self) -> &[HirParameter] {
        &self.parameters
    }

    pub const fn where_predicates(&self) -> &[HirWherePredicate] {
        &self.where_predicates
    }

    pub const fn requires(&self) -> &[ExprId] {
        &self.requires
    }

    pub const fn ensures(&self) -> &[ExprId] {
        &self.ensures
    }

    pub const fn return_type(&self) -> TypeId {
        self.return_type
    }

    /// Sema-owned return classification retained by the final HIR snapshot.
    pub const fn return_semantic_class(&self) -> HirProofReturnSemanticClass {
        self.return_semantic_class
    }

    /// Returns the semantic trust classification without source coordinates.
    pub const fn trust(&self) -> &ProofTrust {
        &self.trust
    }

    pub const fn body(&self) -> &HirProofBody {
        &self.body
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

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirItemInvariantError> {
        validate_signature(
            expected,
            &self.generic_parameters,
            &self.parameters,
            &self.where_predicates,
            &self.requires,
            &self.ensures,
            self.return_type,
        )?;
        validate_contract_scopes(
            expected,
            self.callable_scope,
            self.requires_scope,
            self.ensures_scope,
        )?;
        validate_proof_body(expected, &self.body)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirProofBody {
    Expression {
        scope: ScopeId,
        expression: ExprId,
    },
    Block {
        scope: ScopeId,
        statements: Box<[StmtId]>,
        tail: ExprId,
    },
    Error {
        scope: ScopeId,
        expression: ExprId,
    },
}

impl HirProofBody {
    pub const fn scope(&self) -> ScopeId {
        match self {
            Self::Expression { scope, .. }
            | Self::Block { scope, .. }
            | Self::Error { scope, .. } => *scope,
        }
    }
}
