//! Database-qualified session-local HIR identities and stale-ID diagnostics.

use core::num::{NonZeroU32, NonZeroU64};
use thiserror::Error;

/// Process-local identity of one in-memory HIR database.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDatabaseId(NonZeroU64);

/// Stable module identity within one in-memory HIR database.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleId {
    database: HirDatabaseId,
    slot: NonZeroU32,
}

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
struct RawHirId {
    module: HirModuleId,
    slot: NonZeroU32,
    kind: HirIdKind,
}

/// Non-forgeable diagnostic projection of a raw HIR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RawHirIdView {
    module: HirModuleId,
    kind: HirIdKind,
    slot: NonZeroU32,
}

impl RawHirIdView {
    /// Returns the module that owns the projected identity.
    pub const fn module(&self) -> HirModuleId {
        self.module
    }

    /// Returns the arena kind recorded for the projected identity.
    pub const fn kind(&self) -> HirIdKind {
        self.kind
    }
}

impl From<RawHirId> for RawHirIdView {
    fn from(value: RawHirId) -> Self {
        Self {
            module: value.module,
            kind: value.kind,
            slot: value.slot,
        }
    }
}

/// Top-level declaration identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ItemId(RawHirId);

impl ItemId {
    /// Returns the module that owns this item.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Item
    }
}

/// Lexical scope identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopeId(RawHirId);

impl ScopeId {
    /// Returns the module that owns this scope.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Scope
    }
}

/// Local binding identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId(RawHirId);

impl LocalId {
    /// Returns the module that owns this local.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Local
    }
}

/// Expression identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExprId(RawHirId);

impl ExprId {
    /// Returns the module that owns this expression.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Expr
    }
}

/// Statement identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StmtId(RawHirId);

impl StmtId {
    /// Returns the module that owns this statement.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Stmt
    }
}

/// Type-node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeId(RawHirId);

impl TypeId {
    /// Returns the module that owns this type node.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Type
    }
}

/// Pattern-node identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PatternId(RawHirId);

impl PatternId {
    /// Returns the module that owns this pattern.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Pattern
    }
}

/// Closure-capture identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CaptureId(RawHirId);

impl CaptureId {
    /// Returns the module that owns this capture.
    pub const fn module(self) -> HirModuleId {
        self.0.module
    }

    /// Returns this identity's arena kind.
    pub const fn kind(self) -> HirIdKind {
        HirIdKind::Capture
    }
}

/// Typed owner of one source-derived synthetic HIR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticOwner {
    /// Top-level declaration owner.
    Item(ItemId),
    /// Lexical scope owner.
    Scope(ScopeId),
    /// Local binding owner.
    Local(LocalId),
    /// Expression owner.
    Expr(ExprId),
    /// Statement owner.
    Stmt(StmtId),
    /// Type-node owner.
    Type(TypeId),
    /// Pattern-node owner.
    Pattern(PatternId),
    /// Closure-capture owner.
    Capture(CaptureId),
}

impl SyntheticOwner {
    /// Returns the typed arena kind proven by this owner variant.
    pub const fn kind(self) -> HirIdKind {
        match self {
            Self::Item(_) => HirIdKind::Item,
            Self::Scope(_) => HirIdKind::Scope,
            Self::Local(_) => HirIdKind::Local,
            Self::Expr(_) => HirIdKind::Expr,
            Self::Stmt(_) => HirIdKind::Stmt,
            Self::Type(_) => HirIdKind::Type,
            Self::Pattern(_) => HirIdKind::Pattern,
            Self::Capture(_) => HirIdKind::Capture,
        }
    }

    /// Returns the database-qualified module owning this identity.
    pub const fn module(self) -> HirModuleId {
        match self {
            Self::Item(id) => id.module(),
            Self::Scope(id) => id.module(),
            Self::Local(id) => id.module(),
            Self::Expr(id) => id.module(),
            Self::Stmt(id) => id.module(),
            Self::Type(id) => id.module(),
            Self::Pattern(id) => id.module(),
            Self::Capture(id) => id.module(),
        }
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
    #[error("HIR ID {id:?} is born at {born:?}, after snapshot {snapshot:?}")]
    NotYetLive {
        id: RawHirIdView,
        snapshot: HirSnapshotId,
        born: HirRevision,
    },
    #[error("HIR ID {id:?} retired at {retired_at:?} in snapshot {snapshot:?}")]
    Retired {
        id: RawHirIdView,
        snapshot: HirSnapshotId,
        retired_at: HirRevision,
    },
    #[error("HIR ID {id:?} contains {actual:?}, expected {expected:?}")]
    KindMismatch {
        id: RawHirIdView,
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
        CaptureId, ExprId, HirDatabaseId, HirIdKind, HirLimit, HirModuleId, HirRevision,
        HirSnapshotId, IdResolveError, ItemId, LocalId, PatternId, RawHirId, RawHirIdView, ScopeId,
        StmtId, SyntheticOwner, SyntheticRole, TypeId,
    };
    use core::fmt::Debug;
    use core::hash::Hash;
    use core::num::{NonZeroU32, NonZeroU64};

    fn module_id(database: u64, slot: u32) -> HirModuleId {
        HirModuleId {
            database: HirDatabaseId(NonZeroU64::new(database).unwrap()),
            slot: NonZeroU32::new(slot).unwrap(),
        }
    }

    fn expression(module: HirModuleId, slot: u32) -> ExprId {
        ExprId(RawHirId {
            module,
            slot: NonZeroU32::new(slot).unwrap(),
            kind: HirIdKind::Expr,
        })
    }

    fn raw_id(module: HirModuleId, slot: u32, kind: HirIdKind) -> RawHirId {
        RawHirId {
            module,
            slot: NonZeroU32::new(slot).unwrap(),
            kind,
        }
    }

    #[test]
    fn typed_ids_include_database_module_kind_and_global_slot() {
        let module = module_id(1, 2);
        let first = expression(module, 3);
        let second = expression(module, 4);
        let foreign_database = expression(module_id(2, 2), 3);

        assert!(first < second);
        assert!(first < foreign_database);
        assert_ne!(first, foreign_database);
        assert_eq!(first.module(), module);
        assert_eq!(first.kind(), HirIdKind::Expr);

        let snapshot = HirSnapshotId {
            module,
            revision: HirRevision(NonZeroU32::MIN),
        };
        assert_eq!(snapshot.module(), module);
        assert_eq!(snapshot.revision().0.get(), 1);
    }

    #[test]
    fn id_resolve_error_variants_preserve_exact_payload_shapes() {
        let module = module_id(7, 11);
        let id = RawHirIdView::from(RawHirId {
            module,
            slot: NonZeroU32::new(13).unwrap(),
            kind: HirIdKind::Expr,
        });
        let snapshot = HirSnapshotId {
            module,
            revision: HirRevision(NonZeroU32::new(3).unwrap()),
        };

        assert_eq!(id.module(), module);
        assert_eq!(id.kind(), HirIdKind::Expr);
        assert_eq!(id.slot.get(), 13);

        let corrupted_wrapper = ExprId(RawHirId {
            module,
            slot: NonZeroU32::new(14).unwrap(),
            kind: HirIdKind::Stmt,
        });
        assert_eq!(corrupted_wrapper.kind(), HirIdKind::Expr);
        assert_eq!(
            RawHirIdView::from(corrupted_wrapper.0).kind(),
            HirIdKind::Stmt
        );

        match (IdResolveError::WrongModule {
            expected: module,
            actual: module_id(8, 11),
        }) {
            IdResolveError::WrongModule { expected, actual } => {
                assert_eq!(expected, module);
                assert_eq!(actual, module_id(8, 11));
            }
            other => panic!("unexpected resolver error: {other:?}"),
        }

        match (IdResolveError::NotYetLive {
            id,
            snapshot,
            born: HirRevision(NonZeroU32::new(4).unwrap()),
        }) {
            IdResolveError::NotYetLive {
                id: actual_id,
                snapshot: actual_snapshot,
                born,
            } => {
                assert_eq!(actual_id, id);
                assert_eq!(actual_snapshot, snapshot);
                assert_eq!(born.0.get(), 4);
            }
            other => panic!("unexpected resolver error: {other:?}"),
        }

        match (IdResolveError::Retired {
            id,
            snapshot,
            retired_at: HirRevision(NonZeroU32::new(3).unwrap()),
        }) {
            IdResolveError::Retired {
                id: actual_id,
                snapshot: actual_snapshot,
                retired_at,
            } => {
                assert_eq!(actual_id, id);
                assert_eq!(actual_snapshot, snapshot);
                assert_eq!(retired_at.0.get(), 3);
            }
            other => panic!("unexpected resolver error: {other:?}"),
        }

        match (IdResolveError::KindMismatch {
            id,
            expected: HirIdKind::Expr,
            actual: HirIdKind::Stmt,
        }) {
            IdResolveError::KindMismatch {
                id: actual_id,
                expected,
                actual,
            } => {
                assert_eq!(actual_id, id);
                assert_eq!(expected, HirIdKind::Expr);
                assert_eq!(actual, HirIdKind::Stmt);
            }
            other => panic!("unexpected resolver error: {other:?}"),
        }
    }

    #[test]
    fn synthetic_owner_projects_every_typed_id_family() {
        fn assert_structural_traits<
            T: Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd,
        >() {
        }

        assert_structural_traits::<SyntheticOwner>();

        let module = module_id(17, 19);
        let owners = [
            (
                SyntheticOwner::Item(ItemId(raw_id(module, 1, HirIdKind::Item))),
                HirIdKind::Item,
            ),
            (
                SyntheticOwner::Scope(ScopeId(raw_id(module, 2, HirIdKind::Scope))),
                HirIdKind::Scope,
            ),
            (
                SyntheticOwner::Local(LocalId(raw_id(module, 3, HirIdKind::Local))),
                HirIdKind::Local,
            ),
            (
                SyntheticOwner::Expr(ExprId(raw_id(module, 4, HirIdKind::Expr))),
                HirIdKind::Expr,
            ),
            (
                SyntheticOwner::Stmt(StmtId(raw_id(module, 5, HirIdKind::Stmt))),
                HirIdKind::Stmt,
            ),
            (
                SyntheticOwner::Type(TypeId(raw_id(module, 6, HirIdKind::Type))),
                HirIdKind::Type,
            ),
            (
                SyntheticOwner::Pattern(PatternId(raw_id(module, 7, HirIdKind::Pattern))),
                HirIdKind::Pattern,
            ),
            (
                SyntheticOwner::Capture(CaptureId(raw_id(module, 8, HirIdKind::Capture))),
                HirIdKind::Capture,
            ),
        ];

        for (owner, expected_kind) in owners {
            assert_eq!(owner.kind(), expected_kind);
            assert_eq!(owner.module(), module);
        }

        let shared_raw = raw_id(module, 21, HirIdKind::Expr);
        let item = SyntheticOwner::Item(ItemId(shared_raw));
        let expression = SyntheticOwner::Expr(ExprId(shared_raw));
        assert_eq!(item.kind(), HirIdKind::Item);
        assert_eq!(expression.kind(), HirIdKind::Expr);
        assert_ne!(item, expression);
        assert!(item < expression);
    }

    #[test]
    fn owned_identity_vocabularies_have_stable_behavior() {
        assert_eq!(HirIdKind::Capture.as_str(), "capture");
        assert_eq!(HirLimit::LocalsPerScope.maximum(), 4_096);
        assert_eq!(HirLimit::Captures.maximum(), 65_536);
        assert_eq!(HirLimit::TotalSlotsPerModule.maximum(), 786_432);
        assert_eq!(SyntheticRole::ElidedRegion.as_str(), "elided_region");
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
