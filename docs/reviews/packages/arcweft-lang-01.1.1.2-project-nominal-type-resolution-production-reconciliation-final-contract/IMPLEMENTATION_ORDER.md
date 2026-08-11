# IMPLEMENTATION ORDER

Every cut below is a direct migration. A cut is complete only when its changed
crates compile and the focused tests listed for the cut pass. No cut may add a
compatibility alias, dual reader, extension-trait shim, source gate, or
spelling special case.

Cuts 3 and 4 are ordered implementation phases inside one merge unit. Cut 3
removes entry-owned lookup and may temporarily reject alias-backed entry
schemas through a typed “shared checked resolution not yet available” internal
state; it does not add a temporary alias resolver and is not merged alone.
Cut 4 restores alias-backed entry behavior through the final shared sema
resolver. The first externally reviewable state after Cut 2 contains both
phases and no behavior-regressing intermediate commit.

## Cut 1 — Final typed syntax, IDs, records, and invariants

### Work

1. Add `TypePath`, `AuthoredTypeRef`, `TypeRefSourceMap`,
   `TypeRefNodePath`, source-node types, and typed recovery.
2. Switch the one public `parse_type_ref` to the source-backed result.
3. Atomically migrate every authored type owner.
4. Preserve type-alias generic parameters/name/target/predicate ranges.
5. Replace enum payload strings with typed payloads and exact ranges.
6. Add `arcweft_lang_hir::symbol::nominal` IDs and declaration/source
   records.
7. Add nominal variants directly to `ProjectDeclarationId`,
   `ProjectSymbol`, `ProjectSymbolTargetId`, and `ResolvedProjectSymbol`.
8. Add/adjust inherent enum methods, equality/hash/order/accessors, and
   source-map validators.

### Focused proof

- syntax parser and UTF-8 range tests;
- HIR source-map binding tests;
- ID equality/hash/order/world/revision tests;
- alias generic-parameter retention tests;
- lifetime nominal-parameter rejection fixture;
- exhaustive match compile proof.

### Compile-clean commands

```text
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax --all-targets
cargo check -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-hir --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo clippy -p arcweft-lang-hir --all-targets -- -D warnings
```

## Cut 2 — Atomic project publication and type-target lookup

### Work

1. Collect nominal records from `HirProject::modules()` in canonical order.
2. Insert nominal bindings in `ProjectSymbolTable::link` before import
   resolution.
3. Apply direct duplicate/cross-family/reserved-name checks.
4. Extend limits and checked work charging.
5. Add `resolve_type_target`, typed candidates, inaccessible and wrong-kind
   outcomes, and visible type bindings.
6. Classify all unresolved imports as unknown or unanchored cyclic imports.
7. Preserve current visibility/re-export rules and deterministic report caps.
8. Publish no table on any collection/link diagnostic.

### Focused proof

- local/qualified/relative/import/glob/re-export identity matrix;
- duplicate/cross-family/visibility/import-cycle matrix;
- insertion-order independence and cap tests;
- exact related declaration/import spans.

### Compile-clean commands

```text
cargo check -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-hir symbol --all-targets
cargo clippy -p arcweft-lang-hir --all-targets -- -D warnings
```

## Cut 3 — Entry resolver lookup deletion against shared facts

This cut intentionally follows shared project publication and precedes the
full normal-checker migration. It introduces only the minimum sema adapter
needed to consume typed `ProjectTypeTarget` and declaration records; it does
not create a second resolver.

### Work

1. Make entry project-name selection consume `ProjectSymbolTable`.
2. Make entry schema expansion consume `ProjectNominalDeclaration` records.
3. Delete entry declaration inventories and import/re-export reconstruction.
4. Delete entry alias lookup/cycle stack and enum payload reparsing.
5. Change entry inputs to typed project targets/declaration records and the
   final `ResolvedTypeRefOutcome` interface; until Cut 4 supplies that outcome,
   alias-backed cases use the private non-success internal state described
   above.
6. Delete the `ArcResult` canonical constructor.
7. Keep entry role/schema-shape/canonical contract logic.

### Focused proof

- entry and table select the same declaration ID for all spellings;
- entry schema output is unchanged for valid direct non-generic records;
- alias-backed rows are not accepted by a temporary resolver;
- no entry successful lookup remains.

### Compile-clean commands

```text
cargo check -p arcweft-lang-sema --all-targets
cargo test -p arcweft-lang-sema entry_direct_nominal --all-targets
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

## Cut 4 — One recursive sema resolver, environment/open policy, alias facts,
and poison

### Work

1. Add typed semantic carriers to `TypeKind` and update every inherent
   recursive operation.
2. Add the accepted nominal catalog and direct environment migration.
3. Move `EnvironmentBindingId` destructively and update imports.
4. Publish standard/domain/nominal/enum/Rust/character/adapter/test type facts.
5. Add explicit bounded open rules and collision validation.
6. Implement accepted/detached input constructors and stale-world checks.
7. Implement the single recursive resolver and precedence table.
8. Implement arity, typed substitution, alias chains/cycles, source facts,
   diagnostics, poison records, bounds, work accounting, and caches.
9. Eagerly validate alias targets.
10. Add post-normalization anonymous-choice input.

### Focused proof

- accepted/external/open/generic/`Self`/projection outcomes;
- unknown position matrix;
- alias arity/substitution/chain/cycle matrix;
- exact diagnostic and poison matrix;
- detached/stale/limit/cache tests.

### Compile-clean commands

```text
cargo check -p arcweft-lang-sema --all-targets
cargo test -p arcweft-lang-sema nominal --all-targets
cargo test -p arcweft-lang-sema entry --all-targets
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

## Cut 5 — Normal checker migration and disagreement-path deletion

### Work

1. Resolve function/method/flow/extern signatures before body checking.
2. Resolve closure return annotations with their lexical generic/`Self` scope.
3. Resolve struct/enum/alias/trait/impl fields, payloads, targets, associated
   bindings, bounds, and predicates.
4. Store checked return-target side evidence and poison causes.
5. Gate postfix Try, prefix Try, and propagating Await follow-ons on that
   evidence while preserving operand-success recovery.
6. Migrate all remaining `type_ref_kind*` callers.
7. Delete context-free `TypeKind::from(&TypeRef)`, helper fallbacks, string
   alias maps/erasure, and string-keyed project nominal inventories.
8. Prove no successful project type path bypasses the resolver.

### Focused proof

- TM-072/RD-084, TM-074, TM-080, TM-083;
- prefix Try and propagating Await counterparts;
- all authored position tests;
- no cascade and unrelated-error accumulation tests.

### Compile-clean commands

```text
cargo check -p arcweft-lang-sema --all-targets
cargo test -p arcweft-lang-sema --all-targets
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

## Cut 6 — Structured diagnostics and compiler/index/LSP consumers

### Work

1. Project nominal diagnostics through `arcweft_source::Diagnostic`.
2. Add typed nominal records/reference edges to the project semantic index.
3. Remove string-keyed project nominal projection.
4. Retain the nominal resolution index in the compiler accepted world.
5. Retain it in the LSP accepted project snapshot.
6. Implement diagnostic, hover, definition, completion, references, and rename
   from typed facts.
7. Reject stale snapshots and final `TypeKind::Error`.
8. Switch any persisted internal semantic-index schema directly; no dual
   reader.

### Focused proof

- compiler diagnostics and final-error rejection;
- exact multi-document LSP labels;
- hover/definition/completion/rename behavior;
- stale snapshot invalidation;
- no display-string parsing.

### Compile-clean commands

```text
cargo check -p arcweft-compiler --all-targets
cargo check -p arcweft-lsp --all-targets
cargo test -p arcweft-compiler --all-targets
cargo test -p arcweft-lsp --all-targets
cargo clippy -p arcweft-compiler --all-targets -- -D warnings
cargo clippy -p arcweft-lsp --all-targets -- -D warnings
```

## Cut 7 — Repository-wide acceptance and structural audit

### Required commands

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo metadata --format-version 1 --no-deps
git diff --check
```

Run every repository-prescribed applicable Tier 2 command from the current
root `AGENTS.md`. Run the repository structural audit for public duplicate
types, ownership/dependency direction, and crate visibility. The structural
audit must inspect typed APIs and Cargo metadata; it is not a source-spelling
acceptance gate.

### Required final manual audit

- no authored `TypeRef` can reach successful context-free conversion;
- no normal/entry/LSP project resolver can disagree with
  `ProjectSymbolTable`;
- no `ArcResult` or `Unknown` spelling branch;
- no arbitrary `Named` source fallback;
- no compatibility aliases/fields/readers/traits;
- no source gates or diagnostic/display parsing;
- no dependency on `arcweft-core`, CSS, Takumi, rendering, runtime wire, or
  unrelated runtime crates;
- all `TEST_MATRIX.csv` rows implemented and passing;
- `git diff` contains only the intended language/HIR/sema/compiler/index/LSP
  changes and tests.

## Commit/order rule

Cuts may be separate reviewable commits or one atomic review, but the repository
must never publish a commit in which both an old and new successful resolver
are available. When a consumer moves, its replaced success path is deleted in
the same commit.
