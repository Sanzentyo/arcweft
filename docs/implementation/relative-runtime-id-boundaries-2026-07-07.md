# Relative Runtime ID Boundaries — 2026-07-07

## Status

This cut implements the first owned Rust boundary for the relative-runtime-ID
split in `arcweft-core` and records the migration contract for the remaining
HIR/runtime-plan/AWBC wiring.

Implemented in this cut:

- `arcweft_core::runtime_id::RuntimeIdFamily` describes source/display families
  without storing those families inside canonical runtime lookup strings.
- `arcweft_core::runtime_id::RuntimePublicLabel` makes public/debug strings an
  explicit value domain. Dots inside the label are label text, not namespace
  selectors.
- Inherent APIs on the existing owned runtime ID types define the canonical
  boundary:
  - `FlowRuntimeId::canonical(...)`
  - `FlowRuntimeId::from_source_entity_body(...)`
  - `FlowRuntimeId::public_label()`
  - `EntryRuntimeId::canonical(...)`
  - `EntryRuntimeId::from_source_entity_body(...)`
  - `EntryRuntimeId::public_label()`
  - `RuntimeLineId::public_label()`
- `RuntimeIdError` gives structured diagnostics for malformed source IDs and for
  accidentally passing source-qualified strings as canonical runtime IDs.
- `crates/arcweft-core/tests/runtime_id_boundaries.rs` covers the implemented
  boundary behavior.

The current executable lowering still contains legacy stringly call sites. Those
are intentionally not given compatibility aliases in this cut; they must migrate
to the new inherent APIs instead of adding formatter or lookup shims.

## Boundary design

Arcweft now treats three ID domains as separate values:

1. **Source-relative IDs** are syntax/HIR authoring values. They include
   `IdRef::Relative`, `RelativeId`, and family-relative syntax such as
   `@flow:.next`. Their family information belongs to syntax/HIR resolution.
2. **Canonical runtime IDs** are lookup keys. The family belongs to the owning
   Rust type or declaration table, not to the stored string. A lowered flow
   target is `FlowRuntimeId("main")`, not `FlowRuntimeId("flow.main")`.
3. **Public/debug labels** are strings emitted to AWBC tables, display/source
   maps, logs, and diagnostics. They are represented by `RuntimePublicLabel` and
   may intentionally contain dots, for example `flow.chapter.one.main`.

Namespace family placement:

- `flow` / `fragment`: source declaration or reference family. Runtime lookup is
  owned by `FlowRuntimeId`; the stored canonical string is the resolved suffix.
- `entry`: source declaration family. Runtime lookup is owned by
  `EntryRuntimeId`; AWBC public labels use `entry.<canonical>` deliberately.
- `view`, `asset`, `pure`: source/display declaration families until their
  runtime tables own dedicated typed IDs. They must not be parsed out of public
  labels.
- `line` / `say`: currently content/public line IDs; `RuntimeLineId` preserves
  the stored content key and exposes it as a label without splitting on dots.

## Source resolution contract

The intended lowering path is:

- Parser keeps authored relative syntax typed as `RelativeId` / `IdRef` / family
  relative references.
- HIR resolves relative declarations against the current module, current flow
  slug, and named `scope` stack. Nested scopes resolve by applying
  `parent_depth` before appending the suffix. Walking above the available ID
  scope is a structured diagnostic, not a best-effort fallback.
- Imported modules contribute module path only during source/HIR resolution.
  Runtime plans receive canonical runtime IDs, not module-prefixed source
  labels, unless a runtime table explicitly owns a module-aware key.
- Request-generated samples should author declarations with bare names such as
  `flow main { ... }`; source references may still use source syntax like
  `@flow.main`, but runtime lowering must convert them once to `main`.
- Runtime-plan lowering may not add lookup aliases for old `flow.*` runtime
  strings. A flow target either resolves to the one canonical `FlowRuntimeId` or
  reports a structured diagnostic.

## AWBC emission contract

AWBC must keep lookup keys and public/debug strings separate:

- Function reservation and entry-target lookup use canonical runtime IDs.
- Function `public_id`, entry `public_id`, display-map keys, and diagnostic paths
  use `RuntimePublicLabel` or an equivalent deliberate label field.
- String-table canonicalization may deduplicate identical text bytes, but code
  may not infer a runtime lookup key by splitting a public/debug label.
- A public/debug label containing a dot is valid label text. It is not a
  namespace selector and must not make a hidden `flow.*` runtime alias.

## Diagnostics

The implemented `RuntimeIdError` variants are the boundary diagnostics used by
subsequent lowering migration:

- `Empty { family }`: source stripping or canonical construction produced an
  empty lookup key.
- `CanonicalContainsFamilyPrefix { family, value, prefix }`: a source-qualified
  string was passed to a canonical runtime-ID constructor.
- `WrongSourceFamily { expected, found, value }`: a source reference belongs to a
  different family, such as `view.main` used as a flow target.
- `MissingSourceFamily { expected, value }`: source-boundary conversion expected
  a family-qualified source entity but got a bare value.

These diagnostics are intentionally not compatibility aliases. The fix is to use
source-boundary conversion at the owner boundary or pass a true canonical runtime
ID.

## Migration order for remaining call sites

1. Replace runtime-plan entry/route/choice/flow target construction with
   `FlowRuntimeId::from_source_entity_body(...)` at the HIR-to-runtime boundary.
2. Replace entry ID lowering with `EntryRuntimeId::from_source_entity_body(...)`
   and emit `EntryRuntimeId::public_label()` only at AWBC/debug boundaries.
3. Update AWBC flow lowering so `entry_functions` is keyed by
   `FlowRuntimeId::as_str()` while `AwbcFunction.public_id` interns
   `FlowRuntimeId::public_label()`.
4. Update AWBC entry lowering so target resolution uses canonical flow IDs and
   entry/public strings are emitted deliberately.
5. Update samples/tests so runtime-plan assertions expect `main` for canonical
   flow targets. Keep `@flow.main` only where the source syntax contract is under
   test.

## Tests added

`crates/arcweft-core/tests/runtime_id_boundaries.rs` specifies:

- `flow.main` source body lowers to canonical flow runtime ID `main`.
- `frag.intro` lowers into the same flow runtime domain without a runtime
  `frag.*` alias.
- a public label with multiple dots remains one label string.
- canonical flow runtime IDs reject a `flow.` source prefix.
- wrong source family (`view.main` as a flow target) yields a structured
  diagnostic.
- entry runtime IDs lower through their own boundary.
- line IDs preserve existing content/public IDs without dot splitting.
