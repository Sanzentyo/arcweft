//! Session-local HIR identities and stale-ID diagnostics.

use core::num::NonZeroU32;
use thiserror::Error;

/// Stable module identity within one in-memory HIR database.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleId(NonZeroU32);

/// Monotonic immutable snapshot revision for one HIR module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRevision(NonZeroU32);

/// Module and revision identifying one immutable HIR snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSnapshotId {
    module: HirModuleId,
    revision: HirRevision,
}

impl HirSnapshotId {
    /// Returns the module owning this snapshot.
    pub const fn module(self) -> HirModuleId {
        self.module
    }

    /// Returns the immutable module revision.
    pub const fn revision(self) -> HirRevision {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RawHirId {
    module: HirModuleId,
    slot: NonZeroU32,
}

/// Top-level declaration identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId(RawHirId);

impl ItemId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Lexical scope identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopeId(RawHirId);

impl ScopeId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Local binding identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId(RawHirId);

impl LocalId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Expression identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExprId(RawHirId);

impl ExprId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Statement identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StmtId(RawHirId);

impl StmtId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Type-node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId(RawHirId);

impl TypeId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Pattern-node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternId(RawHirId);

impl PatternId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Closure-capture identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CaptureId(RawHirId);

impl CaptureId {
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }
}

/// Kind recorded for one globally unique module slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdKind {
    Item,
    Scope,
    Local,
    Expr,
    Stmt,
    Type,
    Pattern,
    Capture,
}

impl HirIdKind {
    /// Stable diagnostic label for the slot kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Scope => "scope",
            Self::Local => "local",
            Self::Expr => "expression",
            Self::Stmt => "statement",
            Self::Type => "type",
            Self::Pattern => "pattern",
            Self::Capture => "capture",
        }
    }
}

/// Failure to resolve a typed ID in an immutable HIR snapshot.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum IdResolveError {
    #[error("HIR ID belongs to module {actual:?}, expected {expected:?}")]
    WrongModule {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    #[error("HIR ID is born at {born:?}, after snapshot {snapshot:?}")]
    NotYetLive {
        born: HirRevision,
        snapshot: HirRevision,
    },
    #[error("HIR ID was last live at {last_live:?}, before snapshot {snapshot:?}")]
    Retired {
        last_live: HirRevision,
        snapshot: HirRevision,
    },
    #[error("HIR slot contains {actual:?}, expected {expected:?}")]
    KindMismatch {
        expected: HirIdKind,
        actual: HirIdKind,
    },
}

/// Monotonic generation for one normalized local spelling in a module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalGeneration(NonZeroU32);

/// Stable role used to key a source-derived synthetic HIR node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticRole {
    ImplicitUnitTail,
    PredicateBoolReturn,
    ProofUnitReturn,
    ElidedRegion,
    RecoveryOperand,
    PostconditionResult,
    DesugaredTemporary,
    MissingRequiredTail,
    DestructuredBinding,
    ClosureEnvironment,
    ClosureCapture,
    ContractRequiresScope,
    ContractEnsuresScope,
    ForIterator,
    ForNextValue,
    IfLetScrutinee,
    WhileLetScrutinee,
    MatchScrutinee,
    PatternRest,
    PostfixIndexCandidateExpression,
    DialogueContentCandidateExpression,
}

impl SyntheticRole {
    /// Stable diagnostic and cache label owned by the role enum.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImplicitUnitTail => "implicit_unit_tail",
            Self::PredicateBoolReturn => "predicate_bool_return",
            Self::ProofUnitReturn => "proof_unit_return",
            Self::ElidedRegion => "elided_region",
            Self::RecoveryOperand => "recovery_operand",
            Self::PostconditionResult => "postcondition_result",
            Self::DesugaredTemporary => "desugared_temporary",
            Self::MissingRequiredTail => "missing_required_tail",
            Self::DestructuredBinding => "destructured_binding",
            Self::ClosureEnvironment => "closure_environment",
            Self::ClosureCapture => "closure_capture",
            Self::ContractRequiresScope => "contract_requires_scope",
            Self::ContractEnsuresScope => "contract_ensures_scope",
            Self::ForIterator => "for_iterator",
            Self::ForNextValue => "for_next_value",
            Self::IfLetScrutinee => "if_let_scrutinee",
            Self::WhileLetScrutinee => "while_let_scrutinee",
            Self::MatchScrutinee => "match_scrutinee",
            Self::PatternRest => "pattern_rest",
            Self::PostfixIndexCandidateExpression => "postfix_index_candidate_expression",
            Self::DialogueContentCandidateExpression => "dialogue_content_candidate_expression",
        }
    }
}

/// Deterministic allocation key for a synthetic child of a source-backed node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKey {
    owner: RawHirId,
    role: SyntheticRole,
    ordinal: u32,
}

impl SyntheticKey {
    pub const fn role(self) -> SyntheticRole {
        self.role
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

/// Checked allocator family reported by fatal HIR lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirIdentityKind {
    Module,
    Revision,
    Slot,
    LocalGeneration,
}

/// Inclusive HIR allocation limit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLimit {
    ModulesPerDatabase,
    Items,
    Statements,
    Expressions,
    Types,
    Patterns,
    Scopes,
    LocalsPerScope,
    LocalsPerModule,
    Captures,
    Diagnostics,
    SyntheticDescendantsPerOwner,
    TotalSlotsPerModule,
}

impl HirLimit {
    /// Inclusive hard maximum for the allocation family.
    pub const fn maximum(self) -> usize {
        match self {
            Self::Items | Self::Scopes => 16_384,
            Self::ModulesPerDatabase
            | Self::Statements
            | Self::LocalsPerModule
            | Self::Captures => 65_536,
            Self::Expressions => 262_144,
            Self::Types | Self::Patterns => 131_072,
            Self::LocalsPerScope => 4_096,
            Self::Diagnostics | Self::SyntheticDescendantsPerOwner => 1_024,
            Self::TotalSlotsPerModule => 786_432,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExprId, HirIdKind, HirLimit, HirModuleId, HirRevision, HirSnapshotId, RawHirId,
        SyntheticKey, SyntheticRole,
    };
    use core::num::NonZeroU32;

    #[test]
    fn typed_ids_order_by_module_then_global_slot() {
        let module = HirModuleId(NonZeroU32::new(2).unwrap());
        let first = ExprId(RawHirId {
            module,
            slot: NonZeroU32::new(3).unwrap(),
        });
        let second = ExprId(RawHirId {
            module,
            slot: NonZeroU32::new(4).unwrap(),
        });
        assert!(first < second);
        assert_eq!(first.module(), module);

        let snapshot = HirSnapshotId {
            module,
            revision: HirRevision(NonZeroU32::MIN),
        };
        assert_eq!(snapshot.module(), module);
        assert_eq!(snapshot.revision().0.get(), 1);
    }

    #[test]
    fn owned_identity_vocabularies_have_stable_behavior() {
        assert_eq!(HirIdKind::Capture.as_str(), "capture");
        assert_eq!(HirLimit::LocalsPerScope.maximum(), 4_096);
        assert_eq!(HirLimit::Captures.maximum(), 65_536);
        assert_eq!(HirLimit::TotalSlotsPerModule.maximum(), 786_432);
        let owner = RawHirId {
            module: HirModuleId(NonZeroU32::MIN),
            slot: NonZeroU32::MIN,
        };
        let key = SyntheticKey {
            owner,
            role: SyntheticRole::ElidedRegion,
            ordinal: 2,
        };
        assert_eq!(key.role().as_str(), "elided_region");
        assert_eq!(key.ordinal(), 2);
        assert_eq!(SyntheticRole::ClosureCapture.as_str(), "closure_capture");
        assert_eq!(
            SyntheticRole::ContractEnsuresScope.as_str(),
            "contract_ensures_scope"
        );
        assert_eq!(
            SyntheticRole::PostfixIndexCandidateExpression.as_str(),
            "postfix_index_candidate_expression"
        );
        assert_eq!(
            SyntheticRole::DialogueContentCandidateExpression.as_str(),
            "dialogue_content_candidate_expression"
        );
    }
}
