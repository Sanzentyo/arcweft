# Explicit extension receiver implementation plan — 2026-08-19

## Status and inspected state

- Inspected Git revision: `bf30960973bc59f915a822850fd9862eecc606fa`.
- Working tree at inspection: clean on `main`, matching `origin/main`.
- Implementation performed in this cut: documentation only.
- Documentation validation performed: changed-document relative links passed;
  `git diff --check` passed.
- Rust validation: not run because this cut changes no Rust source, manifest,
  generated artifact, or executable fixture.

This record selects the final source and typed-boundary design for ordinary
function extension receivers. The maintained surface specification is
[Functions, pipelines, and currying](../01-language/functions-and-pipeline.md),
with grammar in [Language grammar](../01-language/grammar.md) and the standard
`map` family in [Traits, Seq, and ranges](../01-language/traits-seq-ranges.md).
This record is implementation sequencing evidence, not a second language
authority.

## Selected contract

An ordinary `fn` opts into dot-call syntax only by declaring exactly one typed
`self` parameter:

```arcw
fn normalize(self: String, locale: Locale) -> String
fn map<A, B>(mapping: A -> B)(self: Vec<A>) -> Vec<B>
```

The receiver coordinate is either the first parameter of the first call group
or the sole parameter of the final call group. The same declaration and
callable identity own free, pipe, and dot surfaces:

```arcw
normalize(text, locale)
text.normalize(locale)

map(project)(values)
values |> map(project)
values.map(project)
```

No `extension` declaration keyword, wrapper method, duplicate body, detached
receiver side table, or type-matched free-call fallback is admitted. Extension
status does not imply purity; the declaration's ordinary checked effect row is
authoritative.

## Current source mismatch

The current resolver still implements the replaced model:

- `crates/arcweft-lang-sema/src/callable/resolver/resolution.rs` calls
  `resolve_data_last_method` after real method lookup;
- `DataLastCallableId` records a receiver coordinate chosen by the resolver
  rather than by the declaration;
- project and environment free-callable catalogs are searched by method name;
- `CollectionMethodId::Map` and `CollectionMethodId::Filter` provide hard-coded
  method identities separate from the documented curried free functions.

The current source already has reusable typed substrate: callable parameter
group coordinates, exact parameter types and passing modes, free and method
lookup keys, checked argument-slot mappings, and plan-qualified callable
identities. The implementation must extend those owners rather than add a
parallel resolver.

## Ordered implementation cuts

### 1. Parse and retain the declared receiver

Extend the ordinary-function parameter grammar and attachment projection for
`self: Type`. Publish one HIR-owned extension receiver record containing the
parameter group, parameter coordinate, local binding, declared type, and
ownership mode. Validate all of the following before callable publication:

- ordinary `fn` only; no `flow`, predicate, proof, Fx, or host declaration;
- exactly one receiver;
- first parameter of the first group or sole parameter of the final group;
- required and positional-only;
- no default and no rest parameter;
- owned `T`, shared `&T`, or mutable `&mut T` mode derived from the declared
  type.

Do not reinterpret a parameter named `self` after HIR construction and do not
infer a receiver from another parameter's type.

Primary owners:

- `crates/arcweft-lang-syntax` function parameter grammar and attachment nodes;
- `crates/arcweft-lang-hir` callable parameter/source projection;
- source index roles for the receiver token, type, and local binding.

### 2. Add the receiver to the callable schema and catalog

Add a closed `CallableExtensionReceiver` value to
`CallableSignatureSchema`. It carries exact group and parameter coordinates
plus ownership mode. It is independent of `CallableMethodRole`, because the
function remains an ordinary free declaration rather than becoming an inherent
or trait method.

Include the field in schema equality, canonical digest input, checked catalog
validation, and registered standard-callable projection. Keep every affected
version marker at `1`.

The catalog retains one `CallableRecord` and one declaration identity. Its
ordinary path remains in the free index, while an extension index references
that same identity by typed receiver key and method name. Do not clone the
record into free and method catalogs.

### 3. Resolve dot calls from explicit candidates only

Replace the sequential fallback algorithm with one typed candidate collection:

1. inherent methods;
2. visible trait methods;
3. visible explicit-extension records from the receiver index.

Check argument groups, receiver ownership, generic instantiation, effects, and
the exact checked receiver type for every candidate. If more than one distinct
identity remains applicable, emit one structured ambiguity diagnostic listing
all candidates. Do not let family order silently change the selected meaning.
Qualified free-call syntax disambiguates an extension declaration.

Map the receiver expression into the declared checked argument slot and retain
the authored non-receiver group boundaries. Receiver-first declarations remove
the first slot; a sole final receiver group is consumed by the dot receiver.
Evaluate the receiver once before authored call arguments, then use the normal
checked call projection. RuntimePlan, structured execution, and AWBC require no
new call opcode or callable family.

### 4. Delete the implicit data-last method family

Once explicit extension lookup is green, delete in the same reviewable cut:

- `resolve_data_last_method` and its free-catalog scans;
- `DataLastCallableId` and `CallableCandidateId::DataLast`;
- fallback-only ambiguity, shadowing, spread, diagnostic, and digest paths;
- tests and documentation that assert a name-and-type-matched free function can
  be called with dot syntax.

No compatibility alias or legacy reader is warranted because this is an
unreleased internal language surface. The no-`^` pipe remains ordinary
function-value Apply and the `^` pipe remains explicit once-only substitution;
neither consults the extension index.

### 5. Migrate the standard functional collection surface

Publish `map`, `filter`, and `fold` as standard ordinary functions whose final
call group is a sole explicit receiver. Migrate each current hard-coded
collection method consumer to those same callable records, then remove
`CollectionMethodId::Map` and `CollectionMethodId::Filter`. Retain unrelated
closed collection operations only where their own trait or intrinsic authority
still requires them.

The first `map` family is closed and concrete:

```text
Vec<A>       -> Vec<B>
Seq<A>       -> Seq<B>
Array<A,N>   -> Array<B,N> (the standard catalog preserves checked N)
Slice<A>     -> Vec<B>
Option<A>    -> Option<B>
Result<A,E>  -> Result<B,E>
Need<A>      -> Need<B>
Parser<A,E>  -> Parser<B,E>
Stream<A,E>  -> Stream<B,E>
```

Do not expose the currently rejected GAT-like `Mappable::Mapped<B>` as if it
were executable. A later trait-backed implementation must replace these rows
atomically and preserve the same `map(f)(value)`, `value |> map(f)`, and
`value.map(f)` surfaces. The unary `Need` overload belongs to the unary-Need
transaction in item 2; item 1A must not create a binary-Need compatibility row
to make that overload land early.

### 6. Tooling and diagnostics

Formatter output preserves `self: Type`. LSP completion reads the extension
index only, labels candidates as extensions, and points hover, definition, and
rename to the one ordinary function declaration. Diagnostics cover invalid
receiver placement, duplicate receivers, unsupported declaration families,
ownership mismatch, invisible extension candidates, and cross-family
ambiguity.

## Validation matrix

The implementation is complete only when all of the following pass:

1. parser/HIR/source tests for both receiver positions, ownership modes, and
   every rejected declaration shape;
2. sema tests proving identical callable identity and argument mapping for
   free and dot calls;
3. ambiguity tests across inherent, trait, and extension candidates, including
   import visibility changes that turn a call into an error rather than silently
   selecting a different declaration;
4. pipe tests proving `x |> f(a)` is `f(a)(x)`, never `f(a, x)`, and evaluates
   `x` once without extension lookup;
5. `map` parity for free, pipe, and dot forms across every supported receiver,
   including `Slice -> Vec` and generic result/error preservation;
6. RuntimePlan and structured/AWBC behavior parity with no new call opcode;
7. formatter, LSP completion/hover/definition/rename, fixture, codec/digest, and
   deterministic generated-artifact checks;
8. repository lint gates plus `rg` evidence that the deleted implicit fallback
   identities and resolver paths are absent.

## Dependencies and non-goals

This cut depends on the ordinary callable catalog, checked argument mapping,
and generic function-value Apply. It does not depend on unary Need, Await
observers, RichText, line plans, Const, or timeout, so it may land as item 1A in
the post-Try convergence order before the unary-Need transaction.

This plan does not add UFCS syntax, fully qualified trait calls, dynamic trait
objects, default method bodies, GAT execution, arbitrary receiver holes inside
call groups, or method syntax for declarations that did not explicitly opt in.

## Structural review boundary

The structural owner is the callable schema/catalog: it alone records the
declared receiver coordinate and indexes the one callable identity for free and
dot lookup. Syntax and HIR retain source identity, sema selects and maps checked
arguments, and RuntimePlan/AWBC consume the resulting ordinary call; none owns
a second extension model. The API boundary is the closed schema field plus the
catalog's typed extension query. The test boundary is the free/pipe/dot identity
and behavior matrix above. The implementation is deliberately decomposed into
parse/HIR, schema/catalog, resolver/deletion, standard-library migration, and
tooling cuts so each owner can be reviewed without making the obsolete fallback
a compatibility dependency.
