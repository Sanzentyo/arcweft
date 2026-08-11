# Complete affected attached and final Rust schemas

These are normative integration schemas, not a patch. Every omitted variant of
an existing enum remains byte-for-byte governed by its accepted predecessor.
The named variants below are added to the original owners directly.

## 1. Central syntax projection in `arcweft-lang-syntax`

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxSelectedMember {
    Name(SyntaxName),
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionProjection {
    // Every accepted existing variant is retained unchanged.
    Select(SyntaxSelectedMember),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExpressionComponentRole {
    // Every accepted existing role is retained unchanged, including Target.
    SelectedMember,
}
```

`ExpressionComponentRole::Target` is reused; it is not duplicated under an
E13 name. `SelectedMember` is a fixed non-ordinal role. The original central
projection owns construction. The original common attached expression record
remains the sole read authority:

```rust
impl AttachedExpressionNode {
    pub fn projection(&self) -> &ExpressionProjection;
    pub fn component(
        &self,
        role: ExpressionComponentRole,
    ) -> Result<AttachedExpressionComponent<'_>, AttachedExpressionAccessError>;
}
```

The method signatures above name the existing central responsibilities; they
do not authorize an E13 map. `AttachedExpressionComponent` retains the common
snapshot-bound `SourceRange`/presence representation. The Select projection
constructor stages exactly:

- `Target`: the authored target expression range;
- `SelectedMember`: the `NameReference` span for `Name`, or the zero-width
  `MissingName` range for `Missing`;
- `Whole`: the common attached expression node range, including the authored
  dot; and
- the already-owned `SyntaxNodeId`, `SyntaxSnapshotId`, and source identity on
  `AttachedExpressionNode`.

There is no `AttachedSelectExpr`, delimiter enum, `OptionalDot`, ErrorNode
member state, source scan, extension trait, or second projection database.
`SyntaxSelectedMember::Name` is constructed only from the parser-validated
`SyntaxName`. `Missing` is constructed only from an exact `MissingName` node.
Any impossible name conversion or missing/duplicate/wrong-role component is an
attachment invariant hard failure and rolls back the enclosing transaction.

## 2. Postfix Try projection reached by `?.`

The accepted central Try projection is retained. The E13-relevant branch is:

```rust
ExpressionProjection::Try {
    form: PostfixQuestion,
    // retained accepted fields, including the operand component
}
```

The lossless lexer emits separate `?` and `.` tokens. The Try projection owns
its `Operand` and `Operator` components and a distinct attached expression
identity. The following ordinary dot emits a second attached expression with
`ExpressionProjection::Select`. No field is added to Select to encode
optionality.

## 3. Final HIR owner in `arcweft-lang-hir::expr`

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSelectedMember {
    Name(HirName),
    Missing,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirSelectExpr {
    target: ExprId,
    member: HirSelectedMember,
}

impl HirSelectExpr {
    pub(crate) const fn new(target: ExprId, member: HirSelectedMember) -> Self {
        Self { target, member }
    }

    pub const fn target(&self) -> ExprId {
        self.target
    }

    pub const fn member(&self) -> &HirSelectedMember {
        &self.member
    }
}
```

Fields are private. Neither type derives or implements Serde. There is no
constructor overload accepting a bare `HirName`, no alias/wrapper, no sentinel,
no source range/revision/syntax handle, no delimiter flag, and no synthetic
member ID. `HirSelectedMember::Invalid` does not exist.

The original name owner remains:

```rust
impl HirName {
    pub(crate) fn try_new(
        value: Box<str>,
    ) -> Result<HirName, HirNameInvariantError>;

    pub fn as_str(&self) -> &str;
}
```

The lowering transaction preflights `HirLimit::NameBytes` before this unchanged
constructor. A `Missing` member does not call it and charges zero. A conversion
failure after parser admission is an invariant failure, not a HIR recovery
variant.

## 4. Singular poison and authored-child propagation

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirPoisonState {
    Clean,
    Poisoned(HirRecoveryIssue),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExpressionRecoveryIssue {
    RecoveredChild { role: HirExprSourceRole },
    // every retained non-E13 variant unchanged
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecoveryIssue {
    MissingOperand { role: HirExprSourceRole },
    InvalidExpression(HirExpressionRecoveryIssue),
    // every retained non-E13 variant unchanged
}
```

The original transaction/final owner implements this exact precedence:

```rust
match (target_state, member) {
    (HirPoisonState::Poisoned(_), _) => HirPoisonState::Poisoned(
        HirRecoveryIssue::InvalidExpression(
            HirExpressionRecoveryIssue::RecoveredChild {
                role: HirExprSourceRole::Target,
            },
        ),
    ),
    (HirPoisonState::Clean, HirSelectedMember::Missing) =>
        HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
            role: HirExprSourceRole::SelectedMember,
        }),
    (HirPoisonState::Clean, HirSelectedMember::Name(_)) =>
        HirPoisonState::Clean,
}
```

This is normative behavior on the original owner, not a new public free helper.
`MissingOperand { role: Target }` remains reserved for an actually missing
synthetic operand. E13 cannot produce one.

## 5. Sole source query and E13 applicability

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceQuery {
    Expr { owner: ExprId, role: HirExprSourceRole },
    Pattern { owner: PatternId, role: HirPatternSourceRole },
    Type { owner: TypeId, role: HirTypeSourceRole },
}

impl HirModule {
    pub fn source_site<'a>(
        &'a self,
        expected_source: &SourceDocumentIdentity,
        query: HirSourceQuery,
    ) -> Result<HirSourceLookup<'a>, HirSourceQueryError>;
}
```

For `HirExprKind::Select`, the source owner admits only:

```text
slot metadata  Whole
components     Target | SelectedMember
```

`Recovery` and all unrelated roles are inapplicable. `Target` is always a
Span. `SelectedMember` is a Span for `Name` and an Insertion for `Missing`.
Owner poison is `HirSourceOwnerStatus`; it does not remove or replace a
component.

## 6. Recovery diagnostics and freeze obligation

```rust
pub struct HirRecoveryDiagnostic {
    owner: SyntheticOwner,
    primary: HirRecoveryPrimary,
    primary_site: HirSourceSite,
}
```

E13 uses one root primary:

```text
HirRecoveryPrimary::ExprRole(HirExprSourceRole::SelectedMember)
```

The key is the qualified `SyntheticOwner`, never `(owner, role)`. Freeze derives
Select-root member obligations directly from `HirSelectedMember`:

```text
Name     exactly zero root member diagnostics
Missing  exactly one root member diagnostic at SelectedMember
```

Target propagation adds no outer diagnostic. A poisoned target keeps its own
terminal diagnostic. With a missing outer member, both distinct owners remain,
ordered descendant before ancestor. Retry replaces/deduplicates by owner and
never appends a second record for the same owner.

## 7. Limits used by this contract

```rust
pub enum HirLimit {
    Expressions,
    Diagnostics,
    TotalSlotsPerModule,
    SourceDocumentBytes,
    NameBytes,
    // every accepted non-E13 variant unchanged
}
```

Inclusive maxima:

```text
HirLimit::Expressions          262_144
HirLimit::Diagnostics            1_024
HirLimit::TotalSlotsPerModule  786_432
HirLimit::SourceDocumentBytes 8_388_608
HirLimit::NameBytes               1_024
```

Syntax maxima used by real `ParsedSource` producers:

```text
SyntaxLimit::Expressions           262_144
SyntaxLimit::IdentityBearingNodes 1_048_576
SyntaxLimit::Diagnostics             1_024
MAX_REGISTRATION_SOURCE_BYTES     8_388_608
```

There is no independent E13 or global hard `SourceComponents` limit in the
accepted enum. Component and slot-metadata totals use checked arithmetic,
unique typed keys, and atomic staging; arithmetic overflow is an invariant hard
failure. This package does not invent a source-component limit merely to add a
one-over row.
