//! Final lexical-scope, local-binding, and closure-capture records.
//!
//! Source order is preserved by the owned slices. Slot liveness, source
//! attachment, generation allocation, and capture discovery remain lowering
//! transaction responsibilities rather than being reconstructed here.

use std::collections::BTreeSet;

use arcweft_source::SourceSpan;
use thiserror::Error;

use crate::identity::{
    ExprId, HirModuleId, ItemId, LocalGeneration, LocalId, PatternId, ScopeId, StmtId, TypeId,
};
use crate::leaf::HirName;

/// Semantic role of one lexical HIR scope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeKind {
    Module,
    Callable,
    Flow,
    Predicate,
    Proof,
    Block,
    MatchArm,
    Conditional,
    Closure,
    ContractRequires,
    ContractEnsures,
}

/// Typed semantic owner of one lexical scope.
///
/// This relation is intentionally not unique in the owner-to-scope direction.
/// A Match expression or statement, for example, owns one distinct lexical
/// scope per arm. Consumers that need a particular scope must retain or follow
/// its `ScopeId`; they must not build a single-value `owner -> scope` index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirScopeOwner {
    Module(HirModuleId),
    Item(ItemId),
    Expr(ExprId),
    Stmt(StmtId),
}

impl HirScopeOwner {
    const fn module(self) -> HirModuleId {
        match self {
            Self::Module(module) => module,
            Self::Item(item) => item.module(),
            Self::Expr(expression) => expression.module(),
            Self::Stmt(statement) => statement.module(),
        }
    }
}

/// One immutable scope-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirScope {
    kind: HirScopeKind,
    parent: Option<ScopeId>,
    owner: HirScopeOwner,
    children: Box<[ScopeId]>,
    locals: Box<[LocalId]>,
}

impl HirScope {
    pub(crate) fn try_new(
        module: HirModuleId,
        kind: HirScopeKind,
        parent: Option<ScopeId>,
        owner: HirScopeOwner,
        children: Box<[ScopeId]>,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirScopeInvariantError> {
        validate_optional_module(module, parent.map(ScopeId::module))?;
        validate_optional_module(module, Some(owner.module()))?;
        validate_ordered_ids(module, &children, ScopeId::module, HirScopeChildKind::Scope)?;
        validate_ordered_ids(module, &locals, LocalId::module, HirScopeChildKind::Local)?;
        Ok(Self {
            kind,
            parent,
            owner,
            children,
            locals,
        })
    }

    /// Returns this scope's semantic role.
    pub const fn kind(&self) -> HirScopeKind {
        self.kind
    }

    /// Returns the enclosing lexical scope, when one exists.
    pub const fn parent(&self) -> Option<ScopeId> {
        self.parent
    }

    /// Returns the semantic owner.
    pub const fn owner(&self) -> &HirScopeOwner {
        &self.owner
    }

    /// Returns child scopes in source order.
    pub const fn children(&self) -> &[ScopeId] {
        &self.children
    }

    /// Returns locals in binding-publication order.
    pub const fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    /// Rebuilds only source-ordered recursive membership while preserving the
    /// scope's fixed semantic identity, owner, kind, and parent.
    pub(crate) fn try_with_members(
        &self,
        children: Box<[ScopeId]>,
        locals: Box<[LocalId]>,
    ) -> Result<Self, HirScopeInvariantError> {
        Self::try_new(
            self.owner.module(),
            self.kind,
            self.parent,
            self.owner,
            children,
            locals,
        )
    }

    /// Whether this scope's closed semantic kind admits its typed owner.
    ///
    /// Cross-arena liveness, lexical parentage, and item-subtree consistency
    /// are validated once by the immutable module arena bundle.
    pub(crate) const fn has_admitted_owner(&self) -> bool {
        match self.kind {
            HirScopeKind::Module => matches!(self.owner, HirScopeOwner::Module(_)),
            HirScopeKind::Callable
            | HirScopeKind::Flow
            | HirScopeKind::Predicate
            | HirScopeKind::Proof
            | HirScopeKind::ContractRequires
            | HirScopeKind::ContractEnsures => matches!(self.owner, HirScopeOwner::Item(_)),
            HirScopeKind::Block => matches!(
                self.owner,
                HirScopeOwner::Item(_) | HirScopeOwner::Expr(_) | HirScopeOwner::Stmt(_)
            ),
            HirScopeKind::MatchArm | HirScopeKind::Conditional => {
                matches!(self.owner, HirScopeOwner::Expr(_) | HirScopeOwner::Stmt(_))
            }
            HirScopeKind::Closure => matches!(self.owner, HirScopeOwner::Expr(_)),
        }
    }
}

impl crate::arena::HirArenaPayload for HirScope {
    fn is_poisoned(&self) -> bool {
        false
    }
}

/// Semantic source of one local binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLocalKind {
    Parameter,
    LetBinding,
    PatternBinding,
    ClosureParameter,
    LoopBinding,
    MatchBinding,
    PostconditionResult,
}

/// Closed lowering policy for one parser-owned Pattern binding site.
///
/// The policy is deliberately distinct from [`HirLocalKind`]: multiple
/// source contexts can publish the same final Local kind while imposing
/// different admission rules on mutability, refutability, and reserved names.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirPatternBindingPolicy {
    PatternBinding,
    CallableParameter,
    FlowParameter,
    PredicateParameter,
    ProofParameter,
    LetBinding,
    LetElseBinding,
    PredicateLet,
    PredicateLetElse,
    ProofLet,
    ProofLetElse,
    ClosureParameter,
    MatchBinding,
}

impl HirPatternBindingPolicy {
    pub(crate) const fn local_kind(self) -> HirLocalKind {
        match self {
            Self::PatternBinding => HirLocalKind::PatternBinding,
            Self::CallableParameter
            | Self::FlowParameter
            | Self::PredicateParameter
            | Self::ProofParameter => HirLocalKind::Parameter,
            Self::LetBinding
            | Self::LetElseBinding
            | Self::PredicateLet
            | Self::PredicateLetElse
            | Self::ProofLet
            | Self::ProofLetElse => HirLocalKind::LetBinding,
            Self::ClosureParameter => HirLocalKind::ClosureParameter,
            Self::MatchBinding => HirLocalKind::MatchBinding,
        }
    }

    pub(crate) const fn requires_irrefutable(self) -> bool {
        matches!(
            self,
            Self::PredicateParameter
                | Self::ProofParameter
                | Self::LetBinding
                | Self::PredicateLet
                | Self::ProofLet
        )
    }

    pub(crate) const fn forbids_mutable(self) -> bool {
        matches!(
            self,
            Self::PredicateParameter
                | Self::ProofParameter
                | Self::PredicateLet
                | Self::ProofLet
                | Self::PredicateLetElse
                | Self::ProofLetElse
        )
    }

    pub(crate) const fn reserves_result(self) -> bool {
        matches!(
            self,
            Self::FlowParameter
                | Self::PredicateParameter
                | Self::ProofParameter
                | Self::PredicateLet
                | Self::PredicateLetElse
                | Self::ProofLet
                | Self::ProofLetElse
        )
    }
}

/// One immutable local-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocal {
    scope: ScopeId,
    kind: HirLocalKind,
    name: HirName,
    generation: LocalGeneration,
    pattern: Option<PatternId>,
    annotation: Option<TypeId>,
    mutable_binding: bool,
    poisoned: bool,
}

/// Result of one source-position-aware lexical local lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalLookup {
    Found(LocalId),
    NotFound,
    AmbiguousPoisoned(Box<[LocalId]>),
}

impl HirLocal {
    // The constructor mirrors the one final arena record. Grouping these
    // fields would introduce a second provisional local-binding carrier.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_new(
        scope: ScopeId,
        kind: HirLocalKind,
        name: HirName,
        generation: LocalGeneration,
        pattern: Option<PatternId>,
        annotation: Option<TypeId>,
        mutable_binding: bool,
        poisoned: bool,
    ) -> Result<Self, HirScopeInvariantError> {
        let module = scope.module();
        validate_optional_module(module, pattern.map(PatternId::module))?;
        validate_optional_module(module, annotation.map(TypeId::module))?;
        Ok(Self {
            scope,
            kind,
            name,
            generation,
            pattern,
            annotation,
            mutable_binding,
            poisoned,
        })
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub const fn kind(&self) -> HirLocalKind {
        self.kind
    }

    pub const fn name(&self) -> &HirName {
        &self.name
    }

    pub const fn generation(&self) -> LocalGeneration {
        self.generation
    }

    pub const fn pattern(&self) -> Option<PatternId> {
        self.pattern
    }

    pub const fn annotation(&self) -> Option<TypeId> {
        self.annotation
    }

    pub const fn is_mutable_binding(&self) -> bool {
        self.mutable_binding
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

impl crate::arena::HirArenaPayload for HirLocal {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// Access performed through one closure capture.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CaptureAccess {
    Read,
    Reassign,
}

/// One immutable capture-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCapture {
    closure: ExprId,
    local: LocalId,
    access: CaptureAccess,
    first_use: SourceSpan,
}

impl HirCapture {
    pub(crate) fn try_new(
        closure: ExprId,
        local: LocalId,
        access: CaptureAccess,
        first_use: SourceSpan,
    ) -> Result<Self, HirScopeInvariantError> {
        validate_optional_module(closure.module(), Some(local.module()))?;
        Ok(Self {
            closure,
            local,
            access,
            first_use,
        })
    }

    pub const fn closure(&self) -> ExprId {
        self.closure
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn access(&self) -> CaptureAccess {
        self.access
    }

    pub const fn first_use(&self) -> &SourceSpan {
        &self.first_use
    }
}

impl crate::arena::HirArenaPayload for HirCapture {
    fn is_poisoned(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum HirScopeInvariantError {
    #[error("scope record references module {actual:?}, expected {expected:?}")]
    ForeignReference {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("scope record repeats a {kind:?} child")]
    DuplicateChild { kind: HirScopeChildKind },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HirScopeChildKind {
    Scope,
    Local,
}

fn validate_optional_module(
    expected: HirModuleId,
    actual: Option<HirModuleId>,
) -> Result<(), HirScopeInvariantError> {
    if let Some(actual) = actual.filter(|actual| *actual != expected) {
        return Err(HirScopeInvariantError::ForeignReference { expected, actual });
    }
    Ok(())
}

fn validate_ordered_ids<I: Copy + Ord>(
    expected: HirModuleId,
    ids: &[I],
    module: impl Fn(I) -> HirModuleId,
    kind: HirScopeChildKind,
) -> Result<(), HirScopeInvariantError> {
    let mut unique = BTreeSet::new();
    for id in ids {
        validate_optional_module(expected, Some(module(*id)))?;
        if !unique.insert(*id) {
            return Err(HirScopeInvariantError::DuplicateChild { kind });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
