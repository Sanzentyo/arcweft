# Final correction

## 1. Normative scope

The keywords **MUST**, **MUST NOT**, **SHALL**, **SHALL NOT**, and **MAY** are normative.

This correction defines only the typed authority between an authored path-member callee whose receiver may denote a type and the already accepted capacity candidate/schema. It does not create a new callable family, builtin ID, source form, call AST, HIR call enum, signature-only resolver, source parser in sema, compatibility carrier, dual reader, or fallback.

## 2. Final owner table

| Concern | Sole owner |
|---|---|
| Ordinary call value, semantic callee expression, arguments, call surface | existing `arcweft_lang_syntax::expr::CallExpr` |
| Parenthesized callee surface classification | `ParenthesizedCalleeSyntax` inside existing `ParenthesizedCallSyntax` |
| Authored receiver type tree | existing `arcweft_lang_syntax::types::AuthoredTypeRef` |
| Every receiver path/generic segment and delimiter range | existing `TypeRefSourceMap<R>`, extended with `TypeRefLexemeSource<R>` |
| Member separator and terminal member | `PathMemberCalleeSyntax` |
| HIR representation | the existing immutable `Expr::Call(CallExpr)` clone and existing HIR source document; no parallel HIR call enum |
| Accepted source binding | existing `SourceBackedTypeRef::try_bind` |
| Generic/Self, builtin, project/import/alias, environment/open type identity | existing nominal `resolve_type_ref(TypeResolutionInput)` and `ResolvedTypeProduct` |
| Value-versus-type namespace decision | checker call-target preparation |
| Validated type receiver | sema-private `ResolvedAssociatedTypeReceiver` |
| Value-selected versus type-associated resolver request | sema-private `CallCallee::{Selected, AssociatedType}` |
| Accepted versus detached callable world | sema-private `CallResolverAuthority` with inherent accessors |
| Static capacity recognition | inherent `CapacityMethodId::resolve_associated` |
| Capacity schema/result | inherent `CapacityMethodId::signature_schema`; accepted `variadic_unchecked` behavior |
| Candidate ordering and work charging | existing `resolve_call_target` entry |
| Argument checking and target publication | existing `check_resolved_call` transaction and checker-owned `CallTargetFacts` |
| Native signature result | existing semantic signature projection from checker facts |

No layer reconstructs a receiver, path, alias, generic parameter, member, or candidate from a display string.

## 3. Exact syntax model

### 3.1 `CallExpr` and ordinary argument ownership do not change

The accepted public shape remains:

```rust
pub struct CallExpr {
    callee: Box<Expr>,
    args: Vec<CallArg>,
    syntax: CallSurfaceSyntax,
}

pub enum CallSurfaceSyntax {
    Parenthesized(ParenthesizedCallSyntax),
    CallbackBlock(CallbackBlockCallSyntax),
}
```

This correction changes neither field nor semantic child. Parenthesized arguments remain owned only by `ArgumentListSyntax`; callback blocks remain unchanged.

### 3.2 Parenthesized callee surface becomes exhaustive

AW-AH-009.3.1's stored `callee: TextRange` is refined as follows:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParenthesizedCallSyntax {
    callee: ParenthesizedCalleeSyntax,
    arguments: ArgumentListSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParenthesizedCalleeSyntax {
    Ordinary { range: TextRange },
    PathMember(PathMemberCalleeSyntax),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AssociatedMemberSeparatorSyntax {
    Dot { range: TextRange },
    Path { range: TextRange },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathMemberCalleeSyntax {
    receiver: AuthoredTypeRef,
    separator: AssociatedMemberSeparatorSyntax,
    member: Name,
    member_range: TextRange,
    whole: TextRange,
}
```

Read-only API:

```rust
impl ParenthesizedCalleeSyntax {
    pub const fn range(&self) -> TextRange;
    pub const fn path_member(&self) -> Option<&PathMemberCalleeSyntax>;
}

impl ParenthesizedCallSyntax {
    pub const fn callee(&self) -> &ParenthesizedCalleeSyntax;
    pub const fn callee_range(&self) -> TextRange;
    pub const fn path_member_callee(&self) -> Option<&PathMemberCalleeSyntax>;
    pub const fn argument_list(&self) -> &ArgumentListSyntax;
    pub const fn range(&self) -> TextRange;
}

impl CallExpr {
    pub const fn path_member_callee_syntax(&self) -> Option<&PathMemberCalleeSyntax>;
}

impl AssociatedMemberSeparatorSyntax {
    pub const fn range(self) -> TextRange;
    pub const fn is_explicit_path(self) -> bool;
}

impl PathMemberCalleeSyntax {
    pub const fn receiver(&self) -> &AuthoredTypeRef;
    pub const fn separator(&self) -> AssociatedMemberSeparatorSyntax;
    pub const fn member(&self) -> &Name;
    pub const fn member_range(&self) -> TextRange;
    pub const fn range(&self) -> TextRange;
}
```

Constructors stay parser-private. `PathMember` means only that the original callee tokens admit a type-shaped receiver followed by a terminal member. It is not a semantic claim that the receiver is a type.

Invariants:

1. `whole` equals the accepted parenthesized callee range.
2. `receiver.root_source().whole()` starts at `whole.start()` and ends at the member separator start.
3. the separator range denotes exactly `.` or `::` as selected by the enum;
4. `member_range` starts at the separator end, covers one valid terminal identifier, and ends at `whole.end()`;
5. the stored `Name` exactly matches the original member token;
6. `Path` separator is constructible only when the receiver has an authored generic argument list or turbofish generic argument list; it does not authorize `String::member`, `Bytes::member`, or bare `Vec::member`;
7. arbitrary expressions such as `factory().member`, indexing, binary expressions, or callback-block applications remain `Ordinary`;
8. existing call, callee, argument-list, recovery, and active-parameter ranges are byte-for-byte unchanged.

### 3.3 `TypeRefSourceMap` owns every type lexeme

The current source-map owner is extended directly rather than adding a call-local parser, helper trait, or display-string decoder:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefLexemeKind {
    PathRoot,
    PathSegment { ordinal: u16 },
    PathSeparator { before: u16 },
    TurbofishSeparator,
    OpenAngle,
    ArgumentSeparator { before: u16 },
    TrailingArgumentSeparator,
    CloseAngle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefLexemeSource<R> {
    owner: TypeRefNodePath,
    kind: TypeRefLexemeKind,
    range: R,
}

pub struct TypeRefSourceMap<R> {
    nodes: Box<[(TypeRefNodePath, TypeRefNodeSource<R>)]>,
    lexemes: Box<[TypeRefLexemeSource<R>]>,
}
```

Required API:

```rust
impl<R> TypeRefLexemeSource<R> {
    pub const fn owner(&self) -> &TypeRefNodePath;
    pub const fn kind(&self) -> &TypeRefLexemeKind;
    pub const fn range(&self) -> &R;
}

impl<R> TypeRefSourceMap<R> {
    pub fn nodes(&self) -> &[(TypeRefNodePath, TypeRefNodeSource<R>)];
    pub fn lexemes(&self) -> &[TypeRefLexemeSource<R>];
    pub fn source_at(&self, path: &TypeRefNodePath) -> Option<&TypeRefNodeSource<R>>;
    pub fn try_map<S, E>(
        &self,
        map: impl FnMut(&R) -> Result<S, E>,
    ) -> Result<TypeRefSourceMap<S>, E>;
}

impl TypeRefSourceMap<TextRange> {
    pub(super) fn try_new(
        value: &TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
    ) -> Result<Self, TypeRefSourceMapError>;
}

impl AuthoredTypeRef {
    pub(super) fn try_new(
        value: TypeRef,
        nodes: Vec<(TypeRefNodePath, TypeRefNodeSource<TextRange>)>,
        lexemes: Vec<TypeRefLexemeSource<TextRange>>,
    ) -> Result<Self, TypeRefSourceMapError>;
}
```

`try_map` maps node and lexeme ranges in one operation. `SourceBackedTypeRef` therefore receives the same structural addresses and exact `SourceSpan` lexemes without a second map.

`TypeRefSourceMapError` gains typed structural variants for missing, extra, duplicate, out-of-owner, out-of-order, ordinal-overflow, and invalid-turbofish lexemes. Parser tests verify token spelling at every recorded range. No lexeme is synthesized for trivia.

Ownership by source element:

- path roots and every path segment: `TypeRefLexemeKind::{PathRoot, PathSegment}`;
- module/path `::`: `PathSeparator`;
- turbofish `::` immediately before `<`: `TurbofishSeparator`;
- `<`, between-argument commas, optional trailing comma, `>`: the generic-node lexeme variants;
- every generic argument's whole range and nested identity: the existing `TypeRefNodePath::GenericArgument` node;
- the terminal call-member `.` or `::`: `PathMemberCalleeSyntax::separator`, outside the receiver type map;
- the terminal member: `PathMemberCalleeSyntax::{member, member_range}`.

### 3.4 Parser construction and rollback

The Pratt/path parser constructs the semantic callee expression, `AuthoredTypeRef`, node map, lexeme map, separator, and member from the same original token transaction. It SHALL NOT:

- obtain a `DottedPath` label and call `parse_type_ref` on that label;
- split `Vec<...>` or `::<...>` text in syntax, HIR, sema, checker, signature help, LSP, or tests;
- run a post-parse source scan;
- hide generic/type structure inside `Token::Ident(String)` as the final authority;
- use display/canonical strings as identity inputs.

The existing comparison lookahead remains authoritative. A failed generic/type-shaped transaction rolls back to the exact Pratt token position. It never consumes `<`/`>` from `a < b`, never publishes a partial lexeme map, and never changes call-argument recovery.

## 4. Accepted source forms

### 4.1 Canonical dot forms

These are accepted and canonical:

```arcw
String.with_capacity(64)
Bytes.with_capacity(4096)
Vec<I32>.with_capacity(8)
Vec<T>.with_capacity(8)
pkg::types::Buffer<I32>.with_capacity(8)
Alias<I32>.with_capacity(8)
```

The dot form is syntactically namespace-ambiguous; sema applies value-first classification.

### 4.2 Current explicit-generic path forms

These remain accepted because Arcweft already has valid source using this family:

```arcw
Vec<I32>::with_capacity(8)
Vec<T>::with_capacity(8)
Vec::<I32>::with_capacity(8)
pkg::types::Vec<I32>::with_capacity(8)
```

The terminal `::` is an explicit type-associated member separator. It is allowed only after a receiver carrying generic arguments or turbofish generic arguments. It does not create general `Type::member` syntax for nongeneric receivers.

### 4.3 Accepted turbofish dot form

```arcw
Vec::<I32>.with_capacity(8)
```

It resolves to the same `TypeRef::Generic` and semantic `TypeKind::Vec(I32)` as `Vec<I32>.with_capacity(8)`, while the lexeme map retains the authored turbofish separator.

### 4.4 Deliberately not introduced

These are not aliases created by this correction:

```arcw
String::with_capacity(64)
Bytes::with_capacity(4096)
Vec::with_capacity(8)
```

They continue through ordinary syntax failure or ordinary non-associated path handling as determined by the existing grammar. No removed-syntax diagnostic is added.

## 5. HIR representation and source binding

Current Arcweft HIR retains syntax `Expr` values. The HIR representation is therefore the cloned `Expr::Call(CallExpr)` whose `ParenthesizedCallSyntax` contains the exhaustive callee surface. No `HirAssociatedCall`, callee side table, text key, or second source map is added.

For an accepted project source, checker preparation binds the receiver through the existing owner:

```rust
let receiver = SourceBackedTypeRef::try_bind(
    syntax.receiver().clone(),
    document,
    document.identity(),
)?;
```

The node map and lexeme map bind to the same accepted `SourceDocumentIdentity`. The separator, member, callee, call, and argument ranges are bound through the same document. Foreign, stale, overflowed, non-UTF-8-boundary, or out-of-document evidence is a typed source-binding error. It never falls back to local text.

Detached checking retains the same `AuthoredTypeRef` and local `TextRange` evidence. It does not fabricate a document identity.

## 6. Value/type receiver classification

### 6.1 Typed classification outcome

Checker preparation uses a closed internal outcome rather than `Option<TypeKind>` or a string heuristic:

```rust
pub(crate) enum PathMemberReceiverClassification<'a> {
    Value {
        expression: TypeExpressionId,
        ty: &'a TypeKind,
    },
    AssociatedType(ResolvedAssociatedTypeReceiver<'a>),
    Failed(AssociatedReceiverFailure),
}
```

### 6.2 Dot-member classification

For `AssociatedMemberSeparatorSyntax::Dot`:

1. resolve the receiver path in the ordinary value namespace exactly once;
2. `Present` selects a value receiver even when a type with the same spelling exists;
3. `Ambiguous`, `Inaccessible`, `Poisoned`, or another typed value error is terminal and does not retry as a type;
4. only typed `Absent` runs nominal type resolution;
5. successful nominal resolution yields `AssociatedType`;
6. nominal failure yields `Failed` and never retries a value or string path.

This closes lexical value, project value, environment value, imported type, qualified type, and alias collisions without spelling heuristics.

### 6.3 Explicit-generic path-member classification

For `AssociatedMemberSeparatorSyntax::Path`, the syntax invariant already proves an explicit generic type receiver. The checker does not perform runtime value lookup. It runs nominal type resolution exactly once. A value named `Vec` cannot convert `Vec<T>::member` into a runtime selected call.

### 6.4 Nominal type resolution order

The existing nominal resolver remains authoritative:

1. scoped `Self` and generic parameters;
2. builtin type constructors;
3. accepted project/import/qualified nominal declarations and aliases;
4. accepted environment/open nominal declarations;
5. typed missing, ambiguous, inaccessible, wrong-kind, wrong-arity, cycle, or poison outcome.

No new type resolver is added.

## 7. Validated associated receiver

Sema owns a borrowed projection of the existing nominal product:

```rust
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResolvedAssociatedTypeReceiver<'a> {
    product: &'a ResolvedTypeProduct,
    root: &'a ResolvedTypeNode,
    ty: &'a TypeKind,
}

impl<'a> ResolvedAssociatedTypeReceiver<'a> {
    pub(crate) fn try_from_product(
        product: &'a ResolvedTypeProduct,
    ) -> Result<Self, AssociatedReceiverFailure>;
    pub(crate) const fn product(&self) -> &'a ResolvedTypeProduct;
    pub(crate) const fn root(&self) -> &'a ResolvedTypeNode;
    pub(crate) const fn ty(&self) -> &'a TypeKind;
}
```

`try_from_product` succeeds only when the root result is a resolved type value and the product is not missing, ambiguous, inaccessible, wrong-kind, cyclic, or poisoned. It preserves the product's alias/module/declaration/generic facts for diagnostics and tooling; the resolver consumes the exact `TypeKind` without parsing presentation text.

Required receiver results:

- `String` -> `TypeKind::String`;
- `Bytes` -> `TypeKind::Bytes`;
- `Vec<I32>` -> `TypeKind::Vec(Box::new(TypeKind::I32))`;
- `Vec<T>` -> `TypeKind::Vec(Box::new(TypeKind::GenericParam(the_exact_id)))`;
- alias to `Vec<I32>` -> the normalized target `TypeKind` with alias resolution facts retained in `ResolvedTypeProduct`;
- qualified project type -> exact project nominal identity;
- bare `Vec` -> builtin arity failure; no receiver value;
- unresolved or ambiguous type -> typed failure; no receiver value.

## 8. Resolver request and instantiation shape

The existing value-selected shape remains unchanged. A type receiver is explicit:

```rust
pub(crate) enum CallCallee<'a> {
    Free { /* accepted fields unchanged */ },
    Selected {
        receiver_expression: TypeExpressionId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
        arguments: &'a [CallArg],
    },
    AssociatedType {
        receiver: ResolvedAssociatedTypeReceiver<'a>,
        member: &'a CallableName,
        arguments: &'a [CallArg],
    },
    Dialogue { /* unchanged */ },
    FunctionValue { /* unchanged */ },
}
```

No sentinel expression ID, empty range, display label, or fabricated value exists.

Resolved candidates record the distinction:

```rust
pub enum CallableInstantiation {
    // existing variants unchanged
    Receiver { receiver: TypeKind },
    TypeReceiver { receiver: TypeKind },
    // existing variants unchanged
}
```

`Receiver` continues to mean an evaluated runtime receiver and remains available to value methods and data-last. `TypeReceiver` means no runtime receiver expression exists. The two variants are not interchangeable.

`UnknownCallKind` gains `AssociatedType` so diagnostics do not misreport a missing associated member as a value method.

## 9. Capacity owner APIs

Behavior is added directly to the existing Arcweft-owned identity:

```rust
impl CapacityMethodId {
    pub(crate) fn resolve_associated(
        receiver: &TypeKind,
        member: &CallableName,
        authored_arity: usize,
    ) -> Result<Option<Self>, CallableIdentityError>;

    pub(crate) fn signature_schema(&self) -> CallableSignatureSchema;
}
```

`resolve_associated` returns `Some` only when:

- `member == "with_capacity"`; and
- `receiver` is exactly `TypeKind::String`, `TypeKind::Bytes`, or `TypeKind::Vec(_)`.

It calls the existing `CapacityMethodId::try_new(receiver.clone(), member.clone(), authored_arity)`. It does not recognize display labels, aliases by name, `Named("Vec<...>")`, a bare constructor, or a value receiver.

The candidate is:

```rust
ResolvedCallable::try_new(
    CallableCandidateId::CapacityMethod(id.clone()),
    SignatureOrigin::Language {
        family: LanguageCallableFamily::CapacityMethod,
    },
    Arc::new(id.signature_schema()),
    CallableInstantiation::TypeReceiver {
        receiver: id.receiver().clone(),
    },
    Vec::new(),
    None,
    limits,
)
```

The result is `id.receiver().clone()`.

The schema owner MUST implement the accepted parent behavior through `variadic_unchecked`:

```rust
variadic_unchecked(
    self.receiver().clone(),
    CallableValidator::Capacity(self.clone()),
    &[],
)
```

The exact authored argument-entry count remains in `CapacityMethodId::arity()`. The schema intentionally accepts zero, one, multiple, named, spread, and recovered entries and checks each value exactly once without an expected type. The baseline `homogeneous(self.arity(), &TypeKind::Named("_"), ...)` shape is not retained.

## 10. Collision precedence

### 10.1 Namespace classification precedence

| Collision | Required outcome |
|---|---|
| dot receiver lexical value and same-name type | value-selected call; no type retry |
| dot receiver project value and imported/qualified type | project value-selected call; no type retry |
| dot receiver environment value and type | environment value-selected call; no type retry |
| dot receiver ambiguous/inaccessible/poisoned value and type | typed value error; no type retry |
| dot receiver value absent and type present | associated type call |
| explicit-generic `::member` and same-name value | associated type call; no value lookup |
| qualified/aliased type | nominal resolver identity; not display spelling |

### 10.2 Associated candidate precedence

For `CallCallee::AssociatedType`, one `resolve_call_target` invocation uses this exact order:

1. typed accepted/detached environment records of `EnvironmentCallableKind::Method` whose receiver and member match;
2. `CapacityMethodId::resolve_associated`;
3. existing associated/inherent/visible trait method resolution;
4. `UnknownCallKind::AssociatedType`.

Consequences:

- typed environment methods beat capacity and traits;
- capacity beats an otherwise viable associated trait;
- unique associated trait resolution is used when no environment or capacity candidate exists;
- associated trait ambiguity is terminal;
- data-last is ineligible because no runtime receiver expression exists;
- `UntypedMethodFallback` is ineligible because it has no typed receiver identity;
- near-miss members do not become capacity; they may resolve as typed environment/trait members or remain unknown.

The existing complete selected-value order remains unchanged for `CallCallee::Selected`.

## 11. Failure and recovery

| Condition | Required behavior |
|---|---|
| malformed generic/turbofish tokens | parser rollback or typed syntax diagnostic; no partial type lexeme map |
| missing terminal member or separator | ordinary malformed/unknown call recovery; no associated seed |
| unknown receiver type | nominal missing diagnostic; no candidate; arguments checked once untyped |
| ambiguous receiver type | nominal ambiguity diagnostic; no candidate; arguments checked once untyped |
| inaccessible/wrong-kind receiver | typed nominal diagnostic; no fallback; arguments once |
| bare `Vec` | builtin generic-arity diagnostic; no `_`; arguments once |
| unresolved generic argument | exact nested nominal diagnostic; no reconstructed name; arguments once |
| invalid associated member | environment/capacity/trait miss, then unknown associated diagnostic; arguments once |
| associated trait ambiguity | terminal ambiguity; arguments once; no data-last |
| `value.with_capacity(...)` | value-selected route only; never static capacity by this correction |
| stale/foreign source identity | typed source identity failure; no local-string fallback |
| cancellation/work exhaustion | atomic rejected outcome; no candidate/fact/cache publication and no argument replay |

Where the parser retains an ordinary recovered `CallExpr`, checker recovery checks each authored or recovered argument expression exactly once without an expected type. A failure before an argument value is constructed does not fabricate an expression or counter entry.

## 12. Registered and non-registered convergence

One typed authority enum replaces the current implicit registered-only assumption:

```rust
pub(crate) enum CallResolverAuthority<'a> {
    Accepted {
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}
```

Existing trait catalog/predicates, lexical scope, source context, cancellation, work, limits, expected type, expression ID, and signature work remain fields on `CallResolverRequest`.

Behavior is implemented as inherent methods on the Arcweft-owned enum:

```rust
impl<'a> CallResolverAuthority<'a> {
    pub(crate) fn typed_method_records(
        &self,
        receiver: &TypeKind,
        member: &CallableName,
    ) -> Result<Option<&'a NonEmptyCallableRecords>, ResolveCallError>;

    pub(crate) fn validate_for(
        &self,
        callee: &CallCallee<'_>,
        source: &CallSourceContext<'_>,
    ) -> Result<(), ResolveCallError>;
}
```

`Accepted` preserves world/revision/source checks. `Detached` is admitted only for a typed `AssociatedType` request in this correction; it cannot be used to invent detached project/free-call resolution. Both variants pass the same `ResolvedAssociatedTypeReceiver`, candidate order, `CapacityMethodId` methods, schema, validator, and checker transaction to the same `resolve_call_target` function.

No trait abstraction, second resolver, synthetic registered world, string seed, or non-registered fast path is introduced.

## 13. Compiling authority-switch and deletion order

The implementation SHALL land as one coherent switch in this exact dependency order:

1. Extend `TypeRefSourceMap` and its parser-owned constructors with lexeme evidence; migrate all existing type-parser construction sites and source binding; pass focused type/source-map tests.
2. Add `ParenthesizedCalleeSyntax`, `PathMemberCalleeSyntax`, and `AssociatedMemberSeparatorSyntax`; change only `ParenthesizedCallSyntax::callee`; update parser construction and existing read-only range accessors; pass call-surface tests.
3. Preserve the typed callee through existing HIR cloning and bind it through the existing document/source-map path; pass HIR/source identity tests.
4. Add checker classification, `ResolvedAssociatedTypeReceiver`, `CallCallee::AssociatedType`, `CallableInstantiation::TypeReceiver`, `UnknownCallKind::AssociatedType`, and `CallResolverAuthority`; pass nominal/collision tests.
5. Add `CapacityMethodId::resolve_associated` and change the existing `CapacityMethodId::signature_schema` to the accepted `variadic_unchecked` owner behavior; connect environment/capacity/trait associated ordering.
6. Connect registered and detached checker modes to the same typed request; publish checker facts; make native signature help consume those facts.
7. In the same production switch, delete every old static-capacity reader listed below. There is no compiling commit with both authorities.
8. Run the full matrix, workspace checks, strict Clippy, Tier 2 fixture, and structural audit before declaring the slice complete.

Old readers that MUST be absent after step 7:

- the `well_known_static_capacity_method_type(&str)` function;
- its `Vec<...>` `strip_prefix`/`strip_suffix` generic parser;
- its bare-`Vec` `TypeKind::Named("_")` construction;
- the early checker success branch that calls `expr_path_label(call.callee())` for static capacity before the shared resolver;
- the helper import and any call site used only by that early branch;
- any registered or non-registered branch that recognizes static capacity from `DottedPath::label`, `Name`, source slice, canonical string, or display text;
- any signature-query/LSP path that independently resolves static capacity;
- any test helper that constructs a receiver by parsing a displayed `Vec<...>` label.

`expr_path_label` may remain for unrelated accepted responsibilities, but no static-capacity path may call it.

## 14. Prohibited implementation outcomes

The implementation SHALL NOT:

- add a 24th callable family or `BuiltinCallableId`;
- add a compatibility alias, deprecated carrier, dual reader, fallback, source gate, or V2 shape;
- retain `_` as the item type of bare `Vec`;
- use a sentinel `TypeExpressionId`, empty range, fake value expression, or display label for a type receiver;
- parse `Vec<T>` in sema, checker, signature help, LSP, or tests;
- restore superseded Dialogue speaker/content carriers;
- change unchecked capacity argument behavior to manufacture rejection cases;
- create a data-last candidate for a type receiver;
- add a helper trait around an Arcweft-owned enum instead of implementing the behavior on the enum;
- add unsafe, unstable features, or a new macro for this slice.
