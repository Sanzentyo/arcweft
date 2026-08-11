# Complete affected Rust schemas

All identity definitions are owned by `arcweft_lang_hir::identity`; lowering
context records stay crate-private in their existing responsibility modules. Fields
remain private. No type below implements Serde, a text parser, or a public raw
constructor.

## 1. Retained typed identity and owner substrate

```rust
pub struct HirDatabaseId(NonZeroU64);

pub struct HirModuleId {
    database: HirDatabaseId,
    slot: NonZeroU32,
}

struct RawHirId {
    module: HirModuleId,
    slot: NonZeroU32,
    kind: HirIdKind,
}

pub struct ItemId(RawHirId);
pub struct ScopeId(RawHirId);
pub struct LocalId(RawHirId);
pub struct ExprId(RawHirId);
pub struct StmtId(RawHirId);
pub struct TypeId(RawHirId);
pub struct PatternId(RawHirId);
pub struct CaptureId(RawHirId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticOwner {
    Item(ItemId),
    Scope(ScopeId),
    Local(LocalId),
    Expr(ExprId),
    Stmt(StmtId),
    Type(TypeId),
    Pattern(PatternId),
    Capture(CaptureId),
}

impl SyntheticOwner {
    pub const fn kind(self) -> HirIdKind;
    pub const fn module(self) -> HirModuleId;
}
```

The existing inherent matches over all eight variants are retained. No `Syntax` or
raw-owner variant is added. Each typed ID exposes only `module()` and its fixed
`kind()` outside `identity.rs`.

## 2. Closed role vocabulary

```rust
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
```

## 3. Complete structural admission

```rust
pub(crate) const MAX_SOURCE_ORDERED_SYNTHETIC_ORDINAL: u32 = 1_023;

impl SyntheticRole {
    pub(crate) const fn accepts_owner_kind(self, owner_kind: HirIdKind) -> bool {
        use HirIdKind::{Expr, Item, Pattern, Scope, Stmt, Type};
        use SyntheticRole::*;

        match self {
            ImplicitUnitTail | MissingRequiredTail => {
                matches!(owner_kind, Expr | Scope)
            }
            ClosureEnvironment
            | ClosureCapture
            | PostfixIndexCandidateExpression
            | DialogueContentCandidateExpression => matches!(owner_kind, Expr),

            PredicateBoolReturn
            | ProofUnitReturn
            | ContractRequiresScope
            | ContractEnsuresScope => matches!(owner_kind, Item),

            ElidedRegion => matches!(owner_kind, Type),

            RecoveryOperand
            | DesugaredTemporary
            | IfLetScrutinee
            | MatchScrutinee => matches!(owner_kind, Expr | Stmt),

            PostconditionResult => matches!(owner_kind, Scope),
            DestructuredBinding | PatternRest => matches!(owner_kind, Pattern),
            ForIterator | ForNextValue | WhileLetScrutinee => {
                matches!(owner_kind, Stmt)
            }
        }
    }

    pub(crate) const fn accepts_ordinal(self, ordinal: u32) -> bool {
        use SyntheticRole::*;
        match self {
            RecoveryOperand
            | DesugaredTemporary
            | DestructuredBinding
            | ClosureCapture
            | PostfixIndexCandidateExpression
            | DialogueContentCandidateExpression => {
                ordinal <= MAX_SOURCE_ORDERED_SYNTHETIC_ORDINAL
            }
            ImplicitUnitTail
            | PredicateBoolReturn
            | ProofUnitReturn
            | ElidedRegion
            | PostconditionResult
            | MissingRequiredTail
            | ClosureEnvironment
            | ContractRequiresScope
            | ContractEnsuresScope
            | ForIterator
            | ForNextValue
            | IfLetScrutinee
            | WhileLetScrutinee
            | MatchScrutinee
            | PatternRest => ordinal == 0,
        }
    }

    pub(crate) const fn accepts_owner(
        self,
        owner_kind: HirIdKind,
        ordinal: u32,
    ) -> bool {
        self.accepts_owner_kind(owner_kind) && self.accepts_ordinal(ordinal)
    }
}
```

`Local` and `Capture` are accepted `SyntheticOwner` variants but no current role
admits those owner kinds. The complete semantic truth table is
`ROLE_OWNER_ORDINAL_MATRIX.tsv`.

## 4. Key and error precedence

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKey {
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticKeyError {
    #[error("synthetic role {role:?} does not accept owner kind {actual:?}")]
    WrongOwnerKind {
        role: SyntheticRole,
        actual: HirIdKind,
    },
    #[error("synthetic role {role:?} does not accept ordinal {ordinal}")]
    InvalidOrdinal {
        role: SyntheticRole,
        ordinal: u32,
    },
}

impl SyntheticKey {
    pub(crate) fn try_new(
        owner: SyntheticOwner,
        role: SyntheticRole,
        ordinal: u32,
    ) -> Result<Self, SyntheticKeyError> {
        let actual = owner.kind();
        if role.accepts_owner(actual, ordinal) {
            return Ok(Self { owner, role, ordinal });
        }
        if !role.accepts_owner_kind(actual) {
            return Err(SyntheticKeyError::WrongOwnerKind { role, actual });
        }
        Err(SyntheticKeyError::InvalidOrdinal { role, ordinal })
    }

    pub const fn owner(self) -> SyntheticOwner { self.owner }
    pub const fn role(self) -> SyntheticRole { self.role }
    pub const fn ordinal(self) -> u32 { self.ordinal }
    #[must_use]
    pub fn fingerprint_input(self) -> SyntheticKeyFingerprintInput;
}
```

Owner kind is checked before ordinal. `try_new` performs no database/snapshot/
transaction lookup.

## 5. Exact liveness schema retained from current main

```rust
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
```

No `last_live` field, alias, or conversion exists.

## 6. Retained affected HIR payload shapes

The correction does not add fields or IDs. These exact affected shapes remain:

```rust
pub struct HirBlockExpr {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

pub struct HirComputationBlockExpr {
    kind: HirComputationBlockKind,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

pub struct HirNamedBlockExpr {
    name: HirName,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

pub struct HirClosureExpr {
    scope: ScopeId,
    parameters: Box<[HirClosureParameter]>,
    result_type: Option<TypeId>,
    body: ExprId,
    captures: Box<[CaptureId]>,
}

pub struct HirIfExpr {
    condition: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
}

pub struct HirIfLetExpr {
    scope: ScopeId,
    pattern: PatternId,
    scrutinee: ExprId,
    guard: Option<ExprId>,
    then_branch: ExprId,
    else_branch: ExprId,
}

pub struct HirMatchExpr {
    scrutinee: ExprId,
    arms: Box<[HirMatchArm]>,
}

pub struct HirMatchArm {
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    value: ExprId,
    locals: Box<[LocalId]>,
}
```

The affected block variants of the retained predicate/proof body owners are exactly:

```rust
HirPredicateBody::Block {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}

HirProofBody::Block {
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}
```

No independent body or match-arm expression ID is introduced. Their existing
`ScopeId` is the synthetic-tail owner.

## 7. Fingerprint transcript API retained unchanged

```rust
pub const SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN: usize = 51;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKeyFingerprintInput(
    [u8; SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN],
);

impl SyntheticKeyFingerprintInput {
    pub const fn as_bytes(
        &self,
    ) -> &[u8; SYNTHETIC_KEY_FINGERPRINT_INPUT_LEN] {
        &self.0
    }
}

impl SyntheticOwner {
    pub(crate) const fn fingerprint_tag(self) -> u8;
    fn raw_for_fingerprint(self) -> RawHirId;
}

impl SyntheticRole {
    pub(crate) const fn fingerprint_tag(self) -> u8;
}
```

The exact layout, tags, fixed vectors, and digest boundary are in
`FINGERPRINT_TRANSCRIPT.md` and are byte-for-byte retained.

## 8. Transaction allocation and source ownership

The allocation ledger key is `(SyntheticKey, child HirIdKind)`. Tail producers
always request `child HirIdKind::Expr`. A same pair returns the same live/reserved
ID and counts once; a different child kind remains distinct.

For a scope-owned tail, the allocated child still owns its source insertion as an
Expr source-index row. The `ScopeId` appears only inside `SyntheticKey`; no new source
query overload or scope-range reader is added.

Before staging a fresh pair the transaction resolves the typed owner, admits an
owner reserved earlier in the same transaction, preflights checked descendant
accounting, then stages child slot and source insertion. Exactly 1,024 descendants
per exact owner commit; the 1,025th fresh pair returns the existing typed HIR limit
error and rolls back the enclosing transaction.

## 9. Traits and visibility

`SyntheticOwner`, `SyntheticRole`, `SyntheticKey`, `SyntheticKeyError`, and
`SyntheticKeyFingerprintInput` use structural `Clone`, `Copy`, `Debug`, `Eq`,
`Hash`, `Ord`, `PartialEq`, and `PartialOrd`. `SyntheticKey` construction remains
`pub(crate)`; public consumers receive read-only keys. No type gains Serde, numeric
conversion, raw-slot access, a decoder, `Display` identity, or a compatibility API.
