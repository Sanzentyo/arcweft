# Complete Rust-facing schemas

The code below is the authoritative replacement shape. Referenced existing
types are the accepted Arcweft owners:

- qualified final-HIR identities: `ExprId`, `TypeId`;
- names and poison: `HirName`, `HirPoisonState`;
- ordinary argument coordinate: `HirCallArgumentOrdinal`;
- central limits and source query: `HirLimit`, `HirSourceQuery`;
- semantic facts: `CallTargetFacts`, `CallableName`, `TypeExpressionId`,
  `TypeKind`, and `CallPoison`.

Every new type introduced by this correction is defined below. Fields are
private. Construction is crate-owned. Public APIs are read-only. No type below
derives or implements public Serde.

```rust
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirCallTypeArgumentOrdinal(u16);

impl HirCallTypeArgumentOrdinal {
    pub const MAX: u16 = 127;

    pub(crate) const fn try_new(value: u16) -> Result<Self, HirCallBuildError> {
        if value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(HirCallBuildError::LimitExceeded {
                limit: HirLimit::CallTypeArguments,
                observed: u64::from(value) + 1,
            })
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HirCallExpr {
    callee: HirCallCallee,
    explicit_type_application: HirCallTypeApplication,
    arguments: Box<[HirCallArgument]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallCallee {
    Value {
        value: ExprId,
    },
    Missing {
        recovery: ExprId,
    },
    UnresolvedDot {
        value_receiver: ExprId,
        nominal_receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    },
    Associated {
        receiver: HirAssociatedReceiver,
        separator: HirAssociatedSeparator,
        member: HirRecoveredName,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirAssociatedReceiver {
    Resolved {
        receiver: TypeId,
    },
    InvalidPresent {
        poisoned: TypeId,
    },
    Missing,
    BareGenericArity {
        poisoned: TypeId,
        declared: u16,
        supplied: u16,
    },
    NominalError {
        error: HirAssociatedReceiverError,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirAssociatedReceiverError {
    UnknownNominal,
    AmbiguousNominal,
    ForeignProject,
    Inaccessible,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirAssociatedCallSyntax {
    DotFallback,
    ExplicitDoubleColon,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirAssociatedSeparator {
    Present(HirAssociatedCallSyntax),
    Missing {
        expected: HirAssociatedCallSyntax,
    },
    InvalidPresent {
        intended: HirAssociatedCallSyntax,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirRecoveredName {
    Valid(HirName),
    Missing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallTypeApplication {
    Absent,
    Present {
        spelling: HirCallTypeApplicationSpelling,
        arguments: Box<[HirCallTypeArgument]>,
        terminator: HirCallTypeApplicationTerminator,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirCallTypeApplicationSpelling {
    DirectAngle,
    Turbofish,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirCallTypeApplicationTerminator {
    Closed,
    RecoveredMissing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallTypeArgument {
    Resolved {
        ty: TypeId,
    },
    InvalidPresent {
        poisoned: TypeId,
    },
    Missing,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallArgument {
    Positional {
        value: HirCallValue,
    },
    Named {
        name: HirRecoveredName,
        equals: HirRequiredTokenState,
        value: HirCallValue,
    },
    Spread {
        value: HirCallValue,
        ellipsis: HirRequiredTokenState,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirRequiredTokenState {
    Present,
    Missing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallValue {
    Present {
        value: ExprId,
    },
    Missing {
        recovery: ExprId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCallBuildError {
    LimitExceeded {
        limit: HirLimit,
        observed: u64,
    },
    InvalidArgumentOrdinal {
        observed: u16,
    },
    InvalidTypeArgumentOrdinal {
        observed: u16,
    },
    InvalidRecoveryOwner,
    SourceManifestInvalid,
    ChildIdentityMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirCallChildPoison {
    Clean,
    Poisoned,
}

pub struct HirCallChildStates<'a> {
    callee: HirCallChildPoison,
    argument_values: &'a [HirCallChildPoison],
    type_arguments: &'a [HirCallChildPoison],
}
```

## Constructor and accessor surface

Construction that needs allocation, source staging, or diagnostics remains an
inherent operation of the accepted final-HIR transaction owner. The immutable
payload constructor itself is:

```rust
impl HirCallExpr {
    pub(crate) fn try_new(
        callee: HirCallCallee,
        explicit_type_application: HirCallTypeApplication,
        arguments: Vec<HirCallArgument>,
        child_states: HirCallChildStates<'_>,
        rich_text_context: bool,
    ) -> Result<(Self, HirPoisonState), HirCallBuildError>;

    pub const fn callee(&self) -> &HirCallCallee;
    pub const fn explicit_type_application(&self) -> &HirCallTypeApplication;
    pub fn arguments(&self) -> &[HirCallArgument];

    pub fn issues(
        &self,
        child_states: HirCallChildStates<'_>,
    ) -> Box<[HirCallIssue]>;

    pub fn primary_issue(
        &self,
        child_states: HirCallChildStates<'_>,
    ) -> Option<HirCallIssue>;
}
```

Constructor invariants:

1. `arguments.len()` is preflighted against `HirLimit::CallArguments` (128) or
   `HirLimit::RichTextCallArguments` (32) before root/child/source/work
   publication.
2. `HirCallArgumentOrdinal` is the only ordinary argument coordinate.
3. A missing callee owns a real `ExprId` child produced by
   `SyntheticKey(root, RecoveryOperand, 0)`.
4. A missing argument value at argument ordinal `n` owns a real `ExprId` child
   produced by `SyntheticKey(root, RecoveryOperand, 1 + n)`.
5. Missing/invalid names never allocate a HIR child and never fabricate
   `HirName`.
6. Present-invalid expression syntax is `HirCallValue::Present` with the real
   poisoned `ExprId`. Present-invalid type syntax is
   `HirCallTypeArgument::InvalidPresent` with the real qualified poisoned
   `TypeId`. Only `Missing` lacks an authored type ID.
7. `UnresolvedDot` retains both the value receiver `ExprId` and same-revision
   nominal receiver evidence. `Associated` is emitted only after value-first
   classification is final.
8. Receiver generic arguments remain inside `HirAssociatedReceiver`'s `TypeId`;
   member/free/path call type arguments remain in `HirCallTypeApplication`.
9. Positional-after-named and spread-not-last retain the original argument in
   authored order. They are issues, not constructor rejection.
10. Duplicate names retain both source ordinals. No argument is dropped or
    rewritten as positional.
11. Missing or invalid `=`/postfix ellipsis is retained by
    `HirRequiredTokenState`; punctuation source remains in `HirSourceIndex`.

## Issue schema and singular root poison

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HirCallIssue {
    MissingCallee,
    InvalidCalleeExpression,
    UnresolvedDotMember,

    MissingAssociatedReceiver,
    InvalidAssociatedReceiver,
    AssociatedReceiverNominalError(HirAssociatedReceiverError),
    BareGenericArity {
        declared: u16,
        supplied: u16,
    },
    MissingAssociatedSeparator {
        expected: HirAssociatedCallSyntax,
    },
    InvalidAssociatedSeparator {
        intended: HirAssociatedCallSyntax,
    },
    MissingAssociatedMember,
    InvalidAssociatedMember,

    MissingTypeApplicationClose,
    InvalidTypeApplicationClose,
    MissingTypeArgument {
        argument: HirCallTypeArgumentOrdinal,
    },
    InvalidTypeArgument {
        argument: HirCallTypeArgumentOrdinal,
    },

    MissingArgumentListClose,
    MissingArgumentName {
        argument: HirCallArgumentOrdinal,
    },
    InvalidArgumentName {
        argument: HirCallArgumentOrdinal,
    },
    MissingNamedEquals {
        argument: HirCallArgumentOrdinal,
    },
    InvalidNamedEquals {
        argument: HirCallArgumentOrdinal,
    },
    MissingArgumentValue {
        argument: HirCallArgumentOrdinal,
    },
    InvalidArgumentValue {
        argument: HirCallArgumentOrdinal,
    },
    MissingSpreadEllipsis {
        argument: HirCallArgumentOrdinal,
    },
    InvalidSpreadEllipsis {
        argument: HirCallArgumentOrdinal,
    },
    DuplicateNamedArgument {
        first: HirCallArgumentOrdinal,
        duplicate: HirCallArgumentOrdinal,
    },
    PositionalAfterNamed {
        argument: HirCallArgumentOrdinal,
    },
    SpreadNotLast {
        argument: HirCallArgumentOrdinal,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HirCallIssueKey {
    phase: u8,
    ordinal: u16,
    component: u8,
    tie: u16,
}
```

The exact key is:

- phase 0: `MissingCallee(0)`, `InvalidCalleeExpression(1)`,
  `UnresolvedDotMember(2)`;
- phase 1: receiver/arity/nominal, separator, member in that order;
- phase 2: type terminator, then type arguments by ordinal;
- phase 3: call terminator, then each argument by ordinal with components
  `Name(0)`, `Equals(1)`, `Value(2)`, `Spread(3)`;
- phase 4: `DuplicateNamedArgument`, `PositionalAfterNamed`,
  `SpreadNotLast`, keyed by offending ordinal and then related ordinal;
- phase 5: remaining child poison by semantic role and ordinal.

`HirCallExpr` stores structural states, not a second issue slice. `issues()`
derives the complete sequence and sorts by this key.

Root poison remains singular:

```rust
pub enum HirRecoveryIssue {
    // accepted variants unchanged
    InvalidCall(HirCallIssue),
}

pub enum HirPoisonState {
    Clean,
    Poisoned { primary: HirRecoveryIssue },
}
```

Invariant:

- no derived issue and all referenced children clean => root `Clean`;
- otherwise root primary is the first canonical issue;
- diagnostics and checker facts consume the complete derived issue sequence;
- payload equality/hash uses structural fields;
- root equality/hash uses the same canonical primary;
- retry identity cannot change with diagnostic scheduling.

## Final-HIR source role extension

No Call source surface record exists. These roles extend the original
`HirExprSourceRole` used by `HirSourceQuery`.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallArgumentSourcePart {
    Whole,
    Name,
    Equals,
    Value,
    Spread,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeArgumentSourcePart {
    Whole,
    Type,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirCallTypeApplicationSourceRole {
    Whole,
    TurbofishSeparator,
    OpenAngle,
    CloseAngle,
    RecoveryEnd,
    EmptyInsertion,
    Argument {
        argument: HirCallTypeArgumentOrdinal,
        part: HirCallTypeArgumentSourcePart,
    },
    Separator {
        following: HirCallTypeArgumentOrdinal,
    },
    TrailingSeparator,
}

pub enum HirExprSourceRole {
    // accepted non-Call roles unchanged
    CallCallee,
    CallAssociatedReceiver,
    CallAssociatedSeparator,
    CallAssociatedMember,
    CallArgumentListOpen,
    CallArgumentListClose,
    CallArgumentListRecoveryEnd,
    CallArgumentListEmptyInsertion,
    CallArgumentSeparator {
        following: HirCallArgumentOrdinal,
    },
    CallArgumentTrailingSeparator,
    CallArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
    CallTypeApplication(HirCallTypeApplicationSourceRole),
}
```

There is no root `CallWhole` role. Root whole comes from arena-slot metadata.

## Checker fact correction in the existing owner

The existing `CheckedCallArgumentFact` is changed in place. No second fact type
or resolver entry is introduced.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallArgumentName {
    Valid(CallableName),
    Missing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedCallArgumentForm {
    Positional,
    Named {
        name: CheckedCallArgumentName,
        equals: HirRequiredTokenState,
        name_source: HirSourceQuery,
        equals_source: HirSourceQuery,
    },
    Spread {
        ellipsis: HirRequiredTokenState,
        ellipsis_source: HirSourceQuery,
    },
}

pub struct CheckedCallArgumentFact {
    index: CallableArgumentIndex,
    whole_source: HirSourceQuery,
    value_source: HirSourceQuery,
    form: CheckedCallArgumentForm,
    slots: Arc<[CheckedCallArgumentSlotFact]>,
    poison: CallPoison,
}

pub struct CheckedCallArgumentSlotFact {
    slot: CallableArgumentSlotIndex,
    expression: TypeExpressionId,
    source: HirSourceQuery,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<TypeKind>,
    expected: Option<TypeKind>,
    poison: CallPoison,
}
```

`CallTargetFacts` retains every existing outcome and field. Its constructor
validates each source query through the final `HirModule` and exact
`SourceDocumentIdentity`; signature projection resolves those same query keys.
Raw component spans are not a parallel coordinate authority.

## Visibility and traits

- Owned immutable payloads derive structural traits shown above.
- Ordinals derive `Copy`, `Ord`, and `PartialOrd`.
- Semantic facts retain existing `Clone`, `Debug`, `Eq`, `PartialEq`.
- No public `Default`, raw field construction, public constructor, public Serde,
  extension trait, alias, compatibility wrapper, or source-string conversion.
- Allocation/interner/source/diagnostic behavior stays on the accepted
  transaction/context owner, not a free conversion helper.
