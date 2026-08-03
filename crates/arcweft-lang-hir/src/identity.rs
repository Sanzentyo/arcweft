//! Database-qualified session-local HIR identities and stale-ID diagnostics.

use core::num::{NonZeroU32, NonZeroU64};
use core::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

static NEXT_HIR_DATABASE_ID: AtomicU64 = AtomicU64::new(1);

/// Process-local identity of one in-memory HIR database.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDatabaseId(NonZeroU64);

impl HirDatabaseId {
    pub(crate) fn allocate() -> Result<Self, HirDatabaseCreateError> {
        allocate_database_id(&NEXT_HIR_DATABASE_ID).ok_or(HirDatabaseCreateError::IdentityExhausted)
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_test(value: NonZeroU64) -> Self {
        Self(value)
    }
}

fn allocate_database_id(counter: &AtomicU64) -> Option<HirDatabaseId> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            NonZeroU64::new(current)?;
            Some(current.checked_add(1).unwrap_or(0))
        })
        .ok()
        .and_then(NonZeroU64::new)
        .map(HirDatabaseId)
}

/// Failure to allocate a fresh process-local HIR database identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirDatabaseCreateError {
    #[error("HIR database identity allocation is exhausted")]
    IdentityExhausted,
}

/// Stable module identity within one in-memory HIR database.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleId {
    database: HirDatabaseId,
    slot: NonZeroU32,
}

impl HirModuleId {
    pub(crate) const fn new(database: HirDatabaseId, slot: NonZeroU32) -> Self {
        Self { database, slot }
    }

    pub(crate) const fn database(self) -> HirDatabaseId {
        self.database
    }

    pub(crate) const fn slot(self) -> NonZeroU32 {
        self.slot
    }
}

/// Monotonic immutable snapshot revision for one HIR module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRevision(NonZeroU32);

impl HirRevision {
    pub(crate) const INITIAL: Self = Self(NonZeroU32::MIN);

    pub(crate) fn checked_next(self) -> Option<Self> {
        self.0
            .get()
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .map(Self)
    }

    #[cfg(test)]
    pub(crate) const fn from_raw_for_test(value: NonZeroU32) -> Self {
        Self(value)
    }
}

/// Module and revision identifying one immutable HIR snapshot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSnapshotId {
    module: HirModuleId,
    revision: HirRevision,
}

impl HirSnapshotId {
    pub(crate) const fn new(module: HirModuleId, revision: HirRevision) -> Self {
        Self { module, revision }
    }

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
    kind: HirIdKind,
}

impl RawHirId {
    pub(crate) const fn new(module: HirModuleId, slot: NonZeroU32, kind: HirIdKind) -> Self {
        Self { module, slot, kind }
    }

    pub(crate) const fn module(self) -> HirModuleId {
        self.module
    }

    pub(crate) const fn slot(self) -> NonZeroU32 {
        self.slot
    }

    pub(crate) const fn kind(self) -> HirIdKind {
        self.kind
    }

    pub(crate) const fn view(self) -> RawHirIdView {
        RawHirIdView {
            module: self.module,
            kind: self.kind,
            slot: self.slot,
        }
    }
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
        value.view()
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

mod sealed {
    pub(crate) trait Sealed {}
}

pub(crate) trait HirTypedId: sealed::Sealed + Copy {
    const KIND: HirIdKind;

    fn from_raw(raw: RawHirId) -> Self;
    fn raw(self) -> RawHirId;
}

impl sealed::Sealed for ItemId {}

impl HirTypedId for ItemId {
    const KIND: HirIdKind = HirIdKind::Item;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for ScopeId {}

impl HirTypedId for ScopeId {
    const KIND: HirIdKind = HirIdKind::Scope;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for LocalId {}

impl HirTypedId for LocalId {
    const KIND: HirIdKind = HirIdKind::Local;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for ExprId {}

impl HirTypedId for ExprId {
    const KIND: HirIdKind = HirIdKind::Expr;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for StmtId {}

impl HirTypedId for StmtId {
    const KIND: HirIdKind = HirIdKind::Stmt;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for TypeId {}

impl HirTypedId for TypeId {
    const KIND: HirIdKind = HirIdKind::Type;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for PatternId {}

impl HirTypedId for PatternId {
    const KIND: HirIdKind = HirIdKind::Pattern;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
    }
}

impl sealed::Sealed for CaptureId {}

impl HirTypedId for CaptureId {
    const KIND: HirIdKind = HirIdKind::Capture;

    fn from_raw(raw: RawHirId) -> Self {
        debug_assert_eq!(raw.kind(), Self::KIND);
        Self(raw)
    }

    fn raw(self) -> RawHirId {
        self.0
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

    pub(crate) const fn fingerprint_tag(self) -> u8 {
        match self {
            Self::Item(_) => 0x01,
            Self::Scope(_) => 0x02,
            Self::Local(_) => 0x03,
            Self::Expr(_) => 0x04,
            Self::Stmt(_) => 0x05,
            Self::Type(_) => 0x06,
            Self::Pattern(_) => 0x07,
            Self::Capture(_) => 0x08,
        }
    }

    fn raw_for_fingerprint(self) -> RawHirId {
        match self {
            Self::Item(id) => id.0,
            Self::Scope(id) => id.0,
            Self::Local(id) => id.0,
            Self::Expr(id) => id.0,
            Self::Stmt(id) => id.0,
            Self::Type(id) => id.0,
            Self::Pattern(id) => id.0,
            Self::Capture(id) => id.0,
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

    pub(crate) const fn allocation_limit(self) -> HirLimit {
        match self {
            Self::Item => HirLimit::Items,
            Self::Scope => HirLimit::Scopes,
            Self::Local => HirLimit::LocalsPerModule,
            Self::Expr => HirLimit::Expressions,
            Self::Stmt => HirLimit::Statements,
            Self::Type => HirLimit::Types,
            Self::Pattern => HirLimit::Patterns,
            Self::Capture => HirLimit::Captures,
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

impl LocalGeneration {
    /// First successfully published binding generation in one scope/name key.
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    /// Constructs a non-zero binding generation.
    pub const fn try_new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the numeric generation used for deterministic ordering.
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Advances one same-scope, same-name shadow generation without wrapping.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.get().checked_add(1) {
            Some(value) => Self::try_new(value),
            None => None,
        }
    }
}

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

pub(crate) const MAX_SOURCE_ORDERED_SYNTHETIC_ORDINAL: u32 = 1_023;

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

    pub(crate) const fn accepts_owner_kind(self, owner_kind: HirIdKind) -> bool {
        use HirIdKind::{Expr, Item, Pattern, Scope, Stmt, Type};

        match self {
            Self::ImplicitUnitTail | Self::MissingRequiredTail => {
                matches!(owner_kind, Expr | Scope)
            }
            Self::ClosureCapture
            | Self::PostfixIndexCandidateExpression
            | Self::DialogueContentCandidateExpression => matches!(owner_kind, Expr),
            Self::PredicateBoolReturn
            | Self::ProofUnitReturn
            | Self::ContractRequiresScope
            | Self::ContractEnsuresScope => matches!(owner_kind, Item),
            Self::ElidedRegion => matches!(owner_kind, Type),
            Self::RecoveryOperand
            | Self::DesugaredTemporary
            | Self::IfLetScrutinee
            | Self::MatchScrutinee => matches!(owner_kind, Expr | Stmt),
            Self::PostconditionResult => matches!(owner_kind, Scope),
            Self::DestructuredBinding | Self::PatternRest => {
                matches!(owner_kind, Pattern)
            }
            Self::ForIterator | Self::ForNextValue | Self::WhileLetScrutinee => {
                matches!(owner_kind, Stmt)
            }
        }
    }

    pub(crate) const fn accepts_ordinal(self, ordinal: u32) -> bool {
        match self {
            Self::RecoveryOperand
            | Self::DesugaredTemporary
            | Self::DestructuredBinding
            | Self::ClosureCapture
            | Self::PostfixIndexCandidateExpression
            | Self::DialogueContentCandidateExpression => {
                ordinal <= MAX_SOURCE_ORDERED_SYNTHETIC_ORDINAL
            }
            Self::ImplicitUnitTail
            | Self::PredicateBoolReturn
            | Self::ProofUnitReturn
            | Self::ElidedRegion
            | Self::PostconditionResult
            | Self::MissingRequiredTail
            | Self::ContractRequiresScope
            | Self::ContractEnsuresScope
            | Self::ForIterator
            | Self::ForNextValue
            | Self::IfLetScrutinee
            | Self::WhileLetScrutinee
            | Self::MatchScrutinee
            | Self::PatternRest => ordinal == 0,
        }
    }

    pub(crate) const fn accepts_owner(self, owner_kind: HirIdKind, ordinal: u32) -> bool {
        self.accepts_owner_kind(owner_kind) && self.accepts_ordinal(ordinal)
    }

    pub(crate) const fn fingerprint_tag(self) -> u8 {
        match self {
            Self::ImplicitUnitTail => 0x01,
            Self::PredicateBoolReturn => 0x02,
            Self::ProofUnitReturn => 0x03,
            Self::ElidedRegion => 0x04,
            Self::RecoveryOperand => 0x05,
            Self::PostconditionResult => 0x06,
            Self::DesugaredTemporary => 0x07,
            Self::MissingRequiredTail => 0x08,
            Self::DestructuredBinding => 0x09,
            Self::ClosureCapture => 0x0b,
            Self::ContractRequiresScope => 0x0c,
            Self::ContractEnsuresScope => 0x0d,
            Self::ForIterator => 0x0e,
            Self::ForNextValue => 0x0f,
            Self::IfLetScrutinee => 0x10,
            Self::WhileLetScrutinee => 0x11,
            Self::MatchScrutinee => 0x12,
            Self::PatternRest => 0x13,
            Self::PostfixIndexCandidateExpression => 0x14,
            Self::DialogueContentCandidateExpression => 0x15,
        }
    }
}

/// Structurally validated identity of one source-derived synthetic HIR node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKey {
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
}

/// Structural rejection produced while constructing a [`SyntheticKey`].
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticKeyError {
    #[error("synthetic role {role:?} does not accept owner kind {actual:?}")]
    WrongOwnerKind {
        role: SyntheticRole,
        actual: HirIdKind,
    },
    #[error("synthetic role {role:?} does not accept ordinal {ordinal}")]
    InvalidOrdinal { role: SyntheticRole, ordinal: u32 },
}

/// Exact byte length of the v1 database-qualified synthetic-key transcript.
pub const SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN: usize = 51;

const SYNTHETIC_KEY_FINGERPRINT_DOMAIN: &[u8; 29] = b"arcweft-hir-synthetic-key-v1\0";

/// Opaque, database-qualified bytes suitable as input to a higher-level hasher.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKeyFingerprintInput([u8; SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN]);

impl SyntheticKeyFingerprintInput {
    /// Returns the canonical v1 transcript bytes.
    pub const fn as_bytes(&self) -> &[u8; SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN] {
        &self.0
    }
}

impl SyntheticKey {
    pub(crate) fn try_new(
        owner: SyntheticOwner,
        role: SyntheticRole,
        ordinal: u32,
    ) -> Result<Self, SyntheticKeyError> {
        let actual = owner.kind();
        if role.accepts_owner(actual, ordinal) {
            return Ok(Self {
                owner,
                role,
                ordinal,
            });
        }
        if !role.accepts_owner_kind(actual) {
            return Err(SyntheticKeyError::WrongOwnerKind { role, actual });
        }
        Err(SyntheticKeyError::InvalidOrdinal { role, ordinal })
    }

    /// Returns the typed owner used to allocate this synthetic identity.
    pub const fn owner(self) -> SyntheticOwner {
        self.owner
    }

    /// Returns the semantic role within the owner.
    pub const fn role(self) -> SyntheticRole {
        self.role
    }

    /// Returns the role-defined ordinal within the owner.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Encodes the canonical session-qualified v1 fingerprint input.
    #[must_use]
    pub fn fingerprint_input(self) -> SyntheticKeyFingerprintInput {
        let raw = self.owner.raw_for_fingerprint();
        let mut bytes = [0; SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN];
        bytes[..29].copy_from_slice(SYNTHETIC_KEY_FINGERPRINT_DOMAIN);
        bytes[29] = self.owner.fingerprint_tag();
        bytes[30..38].copy_from_slice(&raw.module.database.0.get().to_le_bytes());
        bytes[38..42].copy_from_slice(&raw.module.slot.get().to_le_bytes());
        bytes[42..46].copy_from_slice(&raw.slot.get().to_le_bytes());
        bytes[46] = self.role.fingerprint_tag();
        bytes[47..51].copy_from_slice(&self.ordinal.to_le_bytes());
        SyntheticKeyFingerprintInput(bytes)
    }
}

/// Inclusive HIR allocation limit.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirLimit {
    ModulesPerDatabase,
    Items,
    DeclarationMembers,
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
    SourceDocumentBytes,
    DecodedStringBytes,
    NameBytes,
    PathSegments,
    PathSemanticBytes,
    RegistrySegments,
    RegistrySemanticBytes,
    NumericDigitsPerLiteral,
    DecimalCoefficientDigits,
    DecimalScale,
    DecimalExponentAbs,
    NumericSequenceElements,
    NumericSequenceTotalDigits,
    ThreadFlowItems,
    CallArguments,
    AssertionConditions,
    RichTextCallArguments,
    CallTypeArguments,
    StyleNestingDepth,
}

impl HirLimit {
    /// Inclusive hard maximum for the allocation family.
    pub const fn maximum(self) -> usize {
        match self {
            Self::Items | Self::Scopes => 16_384,
            Self::ModulesPerDatabase
            | Self::Statements
            | Self::LocalsPerModule
            | Self::Captures
            | Self::PathSemanticBytes
            | Self::RegistrySemanticBytes
            | Self::NumericDigitsPerLiteral
            | Self::DecimalCoefficientDigits
            | Self::DecimalScale
            | Self::NumericSequenceElements
            | Self::ThreadFlowItems => 65_536,
            Self::Expressions | Self::NumericSequenceTotalDigits => 262_144,
            Self::Types | Self::Patterns => 131_072,
            Self::LocalsPerScope => 4_096,
            Self::DeclarationMembers
            | Self::Diagnostics
            | Self::SyntheticDescendantsPerOwner
            | Self::NameBytes => 1_024,
            Self::TotalSlotsPerModule => 786_432,
            Self::SourceDocumentBytes | Self::DecodedStringBytes => 8_388_608,
            Self::PathSegments | Self::RegistrySegments => 256,
            Self::DecimalExponentAbs => 1_000_000,
            Self::CallArguments | Self::CallTypeArguments => 128,
            Self::AssertionConditions => 64,
            Self::RichTextCallArguments => 32,
            Self::StyleNestingDepth => 64,
        }
    }
}

#[cfg(test)]
#[path = "identity/tests.rs"]
mod tests;
