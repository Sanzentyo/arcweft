# NON-GOALS AND FORBIDDEN MECHANISMS

## Preserved non-goals

This contract does not redesign, implement, or revise:

- Try or Await syntax/source ownership beyond consuming the selected upstream
  typed contracts;
- callable identity, callable publication/resolution, fixed-point effects, or
  AW-AH-009.3 behavior;
- direct suspension, Stream generators, generator classification, or terminal
  semantics;
- runtime lowering, RuntimePlan, AWBC, ABI, bundle, save/load, host wire,
  cancellation, or codecs;
- rendering, presentation objects, CSS, Takumi, or layout;
- character manifest identity/registration except as an accepted nominal
  consumer;
- data-format I/O or `arcweft-core`;
- trait-member existence semantics beyond resolving nominal subjects and
  authored bound types;
- a new lifetime generic-application grammar for nominal declarations;
- format canonicalization or source rewriting.

## Forbidden architecture

The implementation must not introduce:

- another project symbol table;
- another module/import/re-export resolver;
- an adjacent nominal catalog with independent project bindings;
- a checker-only project nominal inventory;
- an entry-only successful project/alias resolver;
- an LSP-only successful project resolver;
- a spelling-keyed alias map;
- a display-string or diagnostic-string identity parser;
- a source-text scan that reconstructs a type path or operator;
- fake project IDs for environment, character, Rust, adapter, or open types;
- a global open-name wildcard;
- arbitrary `TypeKind::Named` acceptance for authored source;
- an `Unknown`, `ArcResult`, fixture, enum, alias, or module special case.

## Forbidden migration techniques

The implementation must not retain or add:

- compatibility shims;
- compatibility type aliases or re-exports;
- dual AST/HIR readers;
- old and new fields in parallel;
- deprecated fields;
- migration ASTs;
- extension traits used to avoid updating the owner enum/impl;
- source gates or spelling deny-list tests;
- permanent removed-syntax diagnostics;
- a legacy semantic-index reader;
- a bridge that parses canonical labels back into type identity.

Each migrated consumer switches to the final typed API and removes its
replaced successful path in the same compile-clean cut.

## Explicitly allowed preservation

The following are allowed because they are existing non-project responsibilities:

- `TypeKind::Named(String)` for internal/host-produced semantic values that do
  not originate from authored `TypeRef`;
- existing value symbols, enum payload inventories, Rust package metadata, and
  nominal structural records after their type-name acceptance is projected
  into the accepted nominal catalog;
- entry-specific schema/role shape policy after project selection is removed;
- `HirProject::linked_module()` for unrelated transitional consumers, provided
  nominal publication and resolution never use it;
- presentation-only `canonical_string()`/labels, provided they are never
  parsed into identity.

## Scope-control acceptance rule

A diff that touches runtime/wire/render/CSS/Takumi/core code to make nominal
resolution work is presumptively incorrect. The implementer must first show a
direct typed dependency required by this contract; no such dependency is
currently specified.
