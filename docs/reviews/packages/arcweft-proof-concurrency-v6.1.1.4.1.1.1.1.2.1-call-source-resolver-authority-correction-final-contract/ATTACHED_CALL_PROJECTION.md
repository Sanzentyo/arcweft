# Central attached Call projection

## Single attached authority

The parser still creates current `CallExpr`/typed type-source values, but the
public final switch attaches them immediately to the central expression
projection. The final owner is one `AttachedExpressionNode` whose payload is
`ExpressionProjection::Call`; component sites are stored only in that node's
central pending component manifest.

```rust
pub enum ExpressionProjection {
    // accepted variants unchanged
    Call(SyntaxCallProjection),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallProjection {
    callee: SyntaxCallCalleeProjection,
    explicit_type_application: Option<SyntaxCallTypeApplicationProjection>,
    arguments: Box<[SyntaxCallArgumentProjection]>,
    terminator: SyntaxCallTerminatorProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallCalleeProjection {
    Ordinary,
    PathMember {
        receiver: AuthoredTypeRef,
        separator: SyntaxAssociatedSeparator,
        member: SyntaxRecoveredName,
    },
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxAssociatedSeparator {
    Present(HirAssociatedCallSyntax),
    Missing { expected: HirAssociatedCallSyntax },
    InvalidPresent { intended: HirAssociatedCallSyntax },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxRecoveredName {
    Valid(SyntaxName),
    Missing,
    InvalidPresent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxCallTypeApplicationProjection {
    spelling: HirCallTypeApplicationSpelling,
    arguments: Box<[SyntaxCallTypeArgumentProjection]>,
    terminator: SyntaxCallTypeTerminatorProjection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallTypeArgumentProjection {
    Present(AuthoredTypeRef),
    Missing,
    InvalidPresent(AuthoredTypeRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxCallArgumentProjection {
    Positional { value: SyntaxComponentState },
    Named {
        name: SyntaxRecoveredName,
        value: SyntaxComponentState,
    },
    Spread { value: SyntaxComponentState },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxComponentState {
    Present,
    Missing,
    InvalidPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxCallTerminatorProjection {
    Closed,
    RecoveredMissing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxCallTypeTerminatorProjection {
    Closed,
    RecoveredMissing,
}
```

These payloads contain no raw source range and no detached syntax ID.

## Central child roles

Expression/type children live in the already central attached node. These roles
address them; they are not identities and cannot be queried independently of
the owning node.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallExpressionChildRole {
    Callee,
    DotValueReceiver,
    ArgumentValue(HirCallArgumentOrdinal),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntaxCallTypeChildRole {
    DotNominalReceiver,
    AssociatedReceiver,
    ExplicitCallTypeArgument(HirCallTypeArgumentOrdinal),
}
```

The `PathMember` projection owns the structured `AuthoredTypeRef` nominal
candidate while the central expression child owns the value candidate. Both are
bound to the same attached source identity. Dot classification may discard the
unused interpretation only after value lookup returns present/terminal or
definitive absence. Explicit `::` never enters value lookup.

## Central component roles

`PendingExpressionProjection` stages exactly one typed component manifest:

- callee;
- associated receiver, separator, member;
- argument-list open, close or recovery end, each comma, trailing comma,
  empty insertion;
- each argument whole/name/equals/value/postfix ellipsis;
- type-application whole, optional turbofish separator, angle delimiters or
  recovery end, type arguments, commas, optional trailing comma, empty insertion.

The pending node validates:

1. database/module/source identity;
2. current source revision and retained source length;
3. every span/insertion UTF-8 boundary;
4. source order and containment;
5. argument/type-argument count and ordinal continuity;
6. form applicability (`Name`/`Equals` only named, `Spread` only spread);
7. exact current token spelling;
8. same-revision value and nominal receiver evidence.

After validation, attachment publishes one `AttachedExpressionNode`. There is no
public attached Call component query. Lowering consumes the node once and stages
the final `HirSourceIndex` entries.

## Parser -> attachment -> final order

1. Pratt/parser builds current-grammar semantic children and current typed
   syntax (`name = value`, `value...`, direct angle/turbofish).
2. Parser emits one `PendingExpressionProjection::Call` with the projection,
   child-role table, and central component manifest.
3. Attachment validates the exact source identity and publishes one node.
4. Final-HIR transaction reserves the Call root, preflights central limits,
   lowers callee/type/argument children, creates only genuinely missing recovery
   children, classifies dot/associated paths, stages `HirSourceQuery` rows, and
   derives root poison.
5. Candidate-neutral argument admission/checking completes.
6. Associated receiver/arity terminal failures take the zero-resolver recovery
   path. Other calls invoke the existing shared resolver once.
7. Facts, diagnostics, work counters, source rows, synthetic rows, and project
   generation commit atomically.
8. The same compiling switch deletes the detached argument/type/cursor readers.

`ArgumentListSyntax` may remain private parser construction input only until step
3. It is not retained as a second tooling/final-HIR reader.
