# Ordinary call composition and typed ID coordinates

## 1. Sole call-surface owner

AW-AH-009.3.1 remains authoritative. The final implementation preserves:

```rust
Expr::Call(CallExpr)
CallSurfaceSyntax::Parenthesized { arguments: ArgumentListSyntax }
CallSurfaceSyntax::CallbackBlock { ... }
```

including private fields, parser-only checked construction, exact target and
argument ranges, positional/named/spread/nested/trailing-comma arguments,
callback blocks, 128-argument limit, 32 nested-call limit, and exact missing-`)`
terminator recovery.

No dialogue-specific call parser or `ContentCallSurface` is implemented.

## 2. Nested target ownership

For:

```arcw
alice(look = smile, id = @.greeting)[Welcome]
```

ownership is:

```text
DialogueContentApplicationExpr
  target = Expr::Call(CallExpr)
    callee = Expr::Path(alice)
    args = existing ordered CallArg values
    syntax = exact existing parenthesized CallSurfaceSyntax
  content = existing DialogueContent
  bracket surface = generic postfix root
```

The postfix root begins at the complete call target start. A missing `)` is
recovered by the call substrate at the `[` insertion point; the entire
recovered `CallExpr` still remains the target. The postfix parser does not
search backward for `(` or reconstruct the call from source.

A record literal used as an ordinary argument remains an argument expression.
Its braces, field names, and `id`/`text_key` fields are not dialogue
coordinates.

## 3. Direct expression carrier correction

Change the original syntax enum directly:

```rust
// final
Expr::EntityRef(IdRef)
```

The parser emits:

```text
@say.opening.greeting -> IdRef::Absolute(EntityRef)
@.greeting            -> IdRef::Relative(RelativeId)
@say:.greeting        -> IdRef::FamilyRelative(FamilyRelativeEntityRef)
```

The repository-owned `IdRef`, `RelativeId`, `RelativeIdSpelling`, and
`FamilyRelativeEntityRef` retain their fields, invariants, constructors, and
accessors. The expression-only `EntityRefSyntax` enum and its range-rebasing
helper are deleted after all callers move. There is no new `Expr::IdRef`
variant, wrapper, alias, or extension trait.

All authored range arithmetic in `IdRef` construction is changed where needed
to checked arithmetic. Saturating range rebasing is not permitted in the final
parser-owned path.

## 4. Typed HIR ID carrier

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirIdRef {
    Absolute(HirEntityReference),
    Relative(HirRelativeId),
    FamilyRelative(HirFamilyRelativeId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIdSuffix(Box<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirIdFamily(Box<str>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRelativeId {
    suffix: HirIdSuffix,
    parent_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFamilyRelativeId {
    family: HirIdFamily,
    relative: HirRelativeId,
}
```

All fields are private; lowering-only constructors validate the already parsed
segments/depth and expose read-only accessors. `HirEntityReference` is the
existing absolute entity-reference payload. No type implements Serde.

Lowering copies the parser-validated typed segments from `IdRef`; it does not
slice, scan, compare, or reparse source. The whole/value spans remain in HIR
source metadata, not these payloads.

## 5. Immediate coordinate collection

After lowering the dialogue application's target, coordinate collection
examines only the typed target root:

1. if it is not `Expr::Call` / `HirExprKind::Call`, the coordinate list is
   empty;
2. if it is a call, iterate its existing argument array in authored order;
3. only `CallArg::Named` whose parsed normalized identifier is exactly `id` or
   `text_key` contributes;
4. store kind, checked argument ordinal, and the already lowered value `ExprId`;
5. do not recurse into the callee, nested calls, callback blocks, record
   literals, or argument value expressions;
6. retain every duplicate.

Identifier equality here is the normal typed named-argument identifier
comparison. It is not a postfix classification heuristic and never searches
source text.

## 6. Compile-time ID versus runtime expression

The stored coordinate value is always `ExprId`. The original `HirExprKind`
implementation exposes an inherent typed query:

```rust
pub enum HirCoordinateValueRef<'a> {
    IdRef(&'a HirIdRef),
    Runtime(ExprId),
    Error(ExprId),
}

impl HirModule {
    pub fn coordinate_value(
        &self,
        coordinate: &HirDialogueCoordinate,
    ) -> Result<HirCoordinateValueRef<'_>, IdResolveError>;
}
```

The query follows only the existing transparent grouping edge, when such a
node survives typed lowering. It returns `IdRef` only for
`HirExprKind::EntityReference`; an ordinary call, path, string, record,
interpolation, binary expression, or other expression is `Runtime`; a poisoned
error node is `Error`.

No source text is read and no `IdRef` is fabricated from a runtime expression.
Sema owns diagnostics and eventual acceptance. Final line-ID/text-key
materialization and collision policy remain AW-AH-009.4.3 work.

## 7. Duplicate and malformed retention

Duplicate `id` and `text_key` coordinates remain separate ordered records and
retain exact whole/name/value source roles. Sema may diagnose duplicates later,
but syntax/HIR never drops, merges, or chooses one.

A malformed named argument retains the existing recovered `CallArg` and value
error expression. If its name is still typed as `id` or `text_key`, a
coordinate points to that error `ExprId`; otherwise it is not a coordinate. The
application is poisoned. No raw string map or fallback scan is constructed.

## 8. Call regression boundary

The implementation must keep all existing public/compile-fail behavior for:

- named, positional, spread, nested, trailing-comma, and recovered arguments;
- callback-block syntax and callback parameter ranges;
- signature-help source roles;
- exact missing-`)` insertion;
- deletion of source-less public call constructors.

Dialogue application composes with this substrate; it does not fork it.
