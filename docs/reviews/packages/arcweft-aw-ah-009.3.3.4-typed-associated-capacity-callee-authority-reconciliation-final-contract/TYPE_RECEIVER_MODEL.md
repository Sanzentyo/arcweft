# Type receiver model

## 1. Layer model

| Layer | Final carrier | Identity/source owned |
|---|---|---|
| Syntax call surface | `ParenthesizedCalleeSyntax::PathMember(PathMemberCalleeSyntax)` | receiver type tree, member separator, member name, exact local byte ranges |
| Syntax type structure | `AuthoredTypeRef` + `TypeRefSourceMap<TextRange>` | every type node, path root/segment/separator, turbofish separator, angle/comma delimiters |
| HIR | existing cloned `Expr::Call(CallExpr)` + existing source document | same immutable structure; no parallel call enum |
| Accepted source binding | `SourceBackedTypeRef` + mapped separator/member/callee spans | exact accepted document/revision |
| Nominal sema | `ResolvedTypeProduct` | generic/Self, builtin, declaration, alias, module, environment/open identity and typed failures |
| Associated receiver | `ResolvedAssociatedTypeReceiver` | validated reference to the exact nominal product and normalized `TypeKind` |
| Callable request | `CallCallee::AssociatedType` | explicit absence of runtime receiver expression |
| Candidate instantiation | `CallableInstantiation::TypeReceiver` | exact resolved receiver type |

## 2. Receiver form matrix

| Authored receiver/callee | Syntax representation | Semantic result |
|---|---|---|
| `String.with_capacity` | path segment `String`, dot separator, terminal member | builtin `TypeKind::String` when value namespace is absent |
| `Bytes.with_capacity` | path segment `Bytes`, dot separator | builtin `TypeKind::Bytes` |
| `Vec.with_capacity` | path segment `Vec`, dot separator | builtin generic-arity failure; no candidate |
| `Vec<I32>.with_capacity` | generic node, normal angle lexemes, dot separator | `TypeKind::Vec(I32)` |
| `Vec<T>.with_capacity` | generic node, `T` child node, dot separator | `TypeKind::Vec(GenericParam(exact T id))` |
| `Vec<I32>::with_capacity` | generic node, terminal path separator | same `Vec(I32)`; explicit type-associated classification |
| `Vec<T>::with_capacity` | generic node, terminal path separator | same exact generic parameter identity |
| `Vec::<I32>.with_capacity` | generic node with turbofish lexeme, dot separator | same `Vec(I32)` after value-absence/type classification |
| `Vec::<I32>::with_capacity` | generic node with turbofish lexeme, path separator | same `Vec(I32)`; explicit type-associated classification |
| `pkg::types::Vec<I32>.with_capacity` | all path segments/separators plus generic node | exact qualified declaration/builtin resolution path |
| `Alias<I32>.with_capacity` | alias path/generic nodes | normalized alias target; alias facts retained |
| `value.with_capacity` | path-member syntax candidate with dot | value-selected when `value` exists; never static capacity |
| `factory().with_capacity` | ordinary callee syntax | existing value-selected expression path |

## 3. Exact representative byte ranges

All ranges are half-open UTF-8 byte ranges.

### 3.1 Canonical generic dot form

For `Vec<I32>.with_capacity(8)`:

| Element | Range |
|---|---|
| receiver whole `Vec<I32>` | `0..8` |
| receiver segment `Vec` | `0..3` |
| `<` | `3..4` |
| generic argument `I32` | `4..7` |
| `>` | `7..8` |
| member separator `.` | `8..9` |
| member `with_capacity` | `9..22` |
| callee whole | `0..22` |
| argument open `(` | `22..23` |
| argument value `8` | `23..24` |
| close `)` | `24..25` |
| call whole | `0..25` |

### 3.2 Existing explicit-generic path form

For `Vec<I32>::with_capacity(8)`:

| Element | Range |
|---|---|
| receiver whole | `0..8` |
| terminal associated separator `::` | `8..10` |
| member | `10..23` |
| callee whole | `0..23` |
| `(` | `23..24` |
| argument | `24..25` |
| `)` | `25..26` |
| call whole | `0..26` |

### 3.3 Turbofish dot form

For `Vec::<I32>.with_capacity(8)`:

| Element | Range |
|---|---|
| segment `Vec` | `0..3` |
| turbofish `::` | `3..5` |
| `<` | `5..6` |
| argument `I32` | `6..9` |
| `>` | `9..10` |
| receiver whole | `0..10` |
| member separator `.` | `10..11` |
| member | `11..24` |
| callee whole | `0..24` |
| call whole | `0..27` |

### 3.4 Turbofish explicit path form

For `Vec::<I32>::with_capacity(8)`:

| Element | Range |
|---|---|
| receiver whole | `0..10` |
| terminal associated separator `::` | `10..12` |
| member | `12..25` |
| callee whole | `0..25` |
| call whole | `0..28` |

## 4. Type lexeme inventory

For every `AuthoredTypeRef`, `TypeRefSourceMap` has two complementary inventories:

1. structural nodes keyed by `TypeRefNodePath`;
2. source lexemes keyed by the owning node and typed lexeme kind.

For `pkg::types::Vec<Option<T>>`, the root generic node owns:

- path segments `pkg`, `types`, `Vec`;
- two path separators;
- root `<` and `>`;
- the root generic-argument node `Option<T>`.

The nested `Option<T>` generic node owns its segment, `<`, `>`, and child `T`. The child `T` node owns its segment. No delimiter is inferred from gaps between whole ranges.

For `Vec::<T>`, the root generic node additionally owns `TurbofishSeparator` before `<`. The semantic tree is otherwise identical to `Vec<T>`.

## 5. Generic identity preservation

Inside:

```arcw
fn make<T>() -> Vec<T> {
    Vec<T>.with_capacity(8)
}
```

nominal resolution uses the callable's existing generic scope. The receiver result is:

```text
TypeKind::Vec(Box::new(TypeKind::GenericParam(the_same_GenericTypeParameterId)))
```

The exact ID is the one published for the function parameter. It is not recreated from the name `T`. The same assertion applies to `Vec<T>::with_capacity`, `Vec::<T>.with_capacity`, and `Vec::<T>::with_capacity`.

Shadowed generic names use their scope-qualified IDs. A project nominal named `T` cannot replace an in-scope generic parameter because the existing nominal precedence remains generic/Self first.

## 6. Alias and qualified identity

An alias is resolved by the existing nominal resolver. The associated receiver retains:

- the normalized target `TypeKind` used for capacity eligibility/result;
- the alias/declaration/path facts in `ResolvedTypeProduct` used for diagnostics and tooling;
- the exact authored alias type and lexeme ranges in syntax/HIR source evidence.

No alias display string is parsed after resolution.

A qualified type path retains every root/segment/separator range. Project symbol identity is module/revision qualified. Same-spelling declarations in another module or source revision cannot be substituted.

## 7. Bare `Vec`

`Vec.with_capacity(8)` produces a valid dot path-member syntax candidate, but the normal builtin type constructor requires one generic argument. The nominal product is a typed arity failure. The checker:

1. emits the existing nominal arity diagnostic at the receiver;
2. creates no `ResolvedAssociatedTypeReceiver`;
3. creates no `CapacityMethodId`;
4. checks the authored argument expression once in unknown-call recovery;
5. never creates `Vec<_>` or `TypeKind::Named("_")`.

`Vec::with_capacity(8)` is not introduced by this correction and is not a bare-Vec compatibility alias.

## 8. Value shadowing model

For dot syntax, value lookup precedes type lookup because the surface is ambiguous:

```arcw
let String = custom_builder
String.with_capacity(64)
```

The receiver is the lexical value `String`. The call uses the ordinary selected-value resolver. It does not become the builtin type `String` even if selected method resolution later reports an unknown method.

The same rule applies to project and environment values. A typed value ambiguity or access error is terminal. Only a typed `Absent` outcome permits nominal type lookup.

Explicit-generic path syntax is not ambiguous:

```arcw
Vec<T>::with_capacity(8)
```

It resolves as a type-associated call even if a value named `Vec` exists, because the syntax carries an explicit generic type receiver and terminal path separator.

## 9. Type-namespace precedence

Once the checker is in type position, existing nominal precedence applies without reference to value names:

1. scoped `Self` and generic parameters;
2. builtins (`String`, `Bytes`, `Vec`, and other constructors);
3. project declarations/imports/qualified paths and aliases;
4. environment/open nominal declarations;
5. typed failure.

A wrong-arity builtin is a terminal type error; it does not fall through to a same-spelling open type. An ambiguous imported type remains ambiguous. Alias cycles remain typed cycles.

## 10. Malformed syntax and recovery

- malformed angle/turbofish syntax publishes no partial `AuthoredTypeRef`;
- comparison expressions retain `<` and `>` operators after lookahead rollback;
- a recovered parenthesized call may retain an `Ordinary` callee and exact argument list;
- no missing receiver/member token is represented by an empty range;
- no member name is guessed from the text after a malformed separator;
- arguments retained by the ordinary call recovery path are checked once.

## 11. Source identity

Accepted source evidence is mapped only through the exact `SourceDocumentIdentity` admitted by the owning project/HIR world. Reparse, document replacement, or foreign source evidence fails typed binding. Detached local ranges are never mixed with accepted spans.

## 12. Model invariants

1. Every associated receiver has one authored type tree and one structural source map.
2. Every required path/generic lexeme has one typed source entry.
3. Every lexeme belongs to exactly one structural node, except the terminal member separator/member, which belong to the callee surface.
4. HIR does not duplicate the call or source map.
5. Sema classification produces either a real value expression or a resolved type product; never a sentinel.
6. `TypeReceiver` candidates never participate in data-last receiver injection.
7. Generic parameter identity is scope identity, not text.
8. Bare generic constructors never receive placeholder arguments.
9. All accepted spelling variants converge to the same capacity identity when their resolved receiver and authored argument count are equal.
