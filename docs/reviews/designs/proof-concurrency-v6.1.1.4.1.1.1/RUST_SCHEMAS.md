# Complete affected Rust schemas

All definitions in this file are owned by `arcweft_lang_hir::identity` unless stated otherwise. Fields remain private. No type here implements `Serialize`, `Deserialize`, a text parser, or a public raw constructor. Existing qualified typed-ID and source-query contracts remain unchanged.

## 1. Qualified owner substrate retained unchanged

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
```

Each typed ID remains `Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd`, exposes `module()` and its fixed `kind()`, and exposes no database/module/HIR numeric slot accessor outside `identity.rs`.

```rust
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

`SyntheticOwner::kind()` and `module()` retain the exact already-landed inherent match over all eight variants. No `Syntax` or raw-ID variant is added.

## 2. Closed role vocabulary retained unchanged

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

No role is added, removed, renamed, aliased, or converted to a string.

## 3. Ordinal constants and policy

```rust
pub(crate) const MAX_SOURCE_ORDERED_SYNTHETIC_ORDINAL: u32 = 1_023;

impl SyntheticRole {
    pub(crate) const fn accepts_owner_kind(self, owner_kind: HirIdKind) -> bool {
        use HirIdKind::{Expr, Item, Pattern, Scope, Stmt, Type};
        use SyntheticRole::*;

        match self {
            ImplicitUnitTail
            | MissingRequiredTail
            | ClosureEnvironment
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

`Local` and `Capture` are deliberately absent from all accepted owner matches. The table in `ROLE_OWNER_ORDINAL_MATRIX.tsv` is the normative semantic generator for each accepted combination.

## 4. Synthetic key and exact error precedence

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

The owner-kind error wins whenever both predicates are false. `try_new` does not access a HIR database, arena, snapshot, source document, or transaction.

## 5. Stable fingerprint transcript API

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

    // Private identity.rs-only projection. It does not become an accessor.
    fn raw_for_fingerprint(self) -> RawHirId;
}

impl SyntheticRole {
    pub(crate) const fn fingerprint_tag(self) -> u8;
}
```

`fingerprint_input()` writes the exact fixed layout in `FINGERPRINT_TRANSCRIPT.md`. It obtains the process-local database ID, module slot, and HIR slot only through private fields inside `identity.rs`. No `RawHirId`, numeric slot, or constructor becomes public. The owner tag comes from the typed `SyntheticOwner` variant, not from a runtime arena probe or Rust enum discriminant.

This layer deliberately defines no digest type and adds no hashing dependency. `SyntheticKeyFingerprintInput` is canonical transcript data for an accepted higher fingerprint owner. `std::hash::Hash` remains an in-process collection trait and is not the transcript encoder.

## 6. Transaction boundary retained and completed

The mutable HIR transaction stages synthetic allocation by the pair:

```text
(SyntheticKey, child HirIdKind)
```

The child kind is part of slot allocation and any full-slot fingerprint, but it is not a field of `SyntheticKey` and therefore is not present in the 51-byte key-only transcript. Re-requesting the same pair reuses the live/reserved ID and counts once. A different child kind is a distinct synthetic child slot under the same owner/key fields, preserving the accepted per-kind candidate preorder.

Before staging a fresh pair, the transaction:

1. resolves the typed owner in its exact variant;
2. permits a staged owner only after that owner reservation exists in the same transaction;
3. computes the target-revision descendant count with checked arithmetic;
4. rejects count 1,025 with `HirLowerError::Limit(HirLimitError { limit: HirLimit::SyntheticDescendantsPerOwner, observed: 1_025, maximum: 1_024 })`; and
5. stages the slot and source insertion/span only after all checks succeed.

`SyntheticKeyError` remains construction-only. Liveness uses the accepted `IdResolveError`; count failure uses the existing HIR limit error. No author-facing diagnostic is fabricated for an internal role/owner bug.

## 7. Elided region retained unchanged

```rust
pub struct HirElidedRegion {
    key: SyntheticKey,
}

pub enum HirElidedRegionError {
    OwnerMismatch {
        expected: TypeId,
        actual: SyntheticOwner,
    },
}

impl HirElidedRegion {
    pub(crate) fn try_new(
        owner: TypeId,
        key: SyntheticKey,
    ) -> Result<Self, HirElidedRegionError>;

    pub const fn owner_type(self) -> TypeId;
    pub const fn key(self) -> SyntheticKey;
}
```

The key must be `SyntheticOwner::Type(owner)`, `SyntheticRole::ElidedRegion`, ordinal 0. The role/ordinal properties are already proven by `SyntheticKey::try_new`; `HirElidedRegion::try_new` proves exact TypeId equality.

## 8. Trait, visibility, and serialization policy

- `SyntheticOwner`, `SyntheticRole`, `SyntheticKey`, `SyntheticKeyError`, and `SyntheticKeyFingerprintInput` use structural `Clone`, `Copy`, `Debug`, `Eq`, `Hash`, `Ord`, `PartialEq`, and `PartialOrd` as shown.
- `SyntheticKey` and `SyntheticKeyFingerprintInput` fields and constructors remain private; only the key's read-only accessors and transcript encoder are public.
- No type in this contract implements Serde, a text codec, `Display` as identity, `From<u32>`, `TryFrom<u32>`, or a public raw conversion.
- Structural `Ord` is not required to equal lexicographic transcript-byte order; the transcript is a fingerprint preimage, not a sorting codec.
