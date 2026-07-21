# Lang-01.1.1.2 project nominal type resolution intake

## Baseline and source package

- implementation baseline: `126f7ece0f69`
- package: `docs/reviews/arcweft-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation-final-contract.zip`
- package SHA-256: `FF695EADEF1A4C833D86F53CA5E9010C7DF3D3643418109980B0E9F1D6CFE1AB`
- package status: implementation-ready, with no open design questions

The package is the acceptance source for this slice. The earlier goal text
listed Lang-01.1.1.2 as unreturned; receipt of this package supersedes that
stale exclusion.

## Required merge units

1. Replace string-backed authored type heads with `TypePath`, add
   `AuthoredTypeRef` plus an exact one-to-one structural source map, and migrate
   every syntax owner without a dual reader.
2. Publish struct, enum, and type-alias declarations atomically through the
   existing `ProjectSymbolTable`, including typed source records, collision and
   limit checks, import/re-export resolution, and unresolved-import
   classification.
3. Replace the entry-local nominal resolver and all context-free successful
   `TypeRef` conversions with the single recursive
   `arcweft_lang_sema::nominal::resolve_type_ref` authority. Cuts 2 and 3 are
   one atomic compatibility boundary: no intermediate string resolver is a
   valid result.
4. Migrate ordinary checking, trait/impl checking, alias substitution,
   `Try`/`Await` propagation, compiler diagnostics, project index, and LSP to
   typed nominal IDs, source spans, and poison evidence.
5. Run focused tests during each cut, then workspace check/clippy/tests,
   explicit Tier 2, and the structural audit before the final push.

## Direct-final syntax decisions

- `parse_type_ref` now returns one `AuthoredTypeRef`; the unspanned value parser
  is private.
- Every structural `TypeRef` node has exactly one source-map entry. Generic
  arguments are capped at 256 and complete type trees at 4,096 nodes.
- Malformed owned annotations retain `TypeRef::Recovery` plus their exact
  range; they are not dropped before semantic poison propagation.
- The unpublished enum record-payload scaffold is not retained. An enum
  variant has zero or one typed payload. Brace-shaped old payload text follows
  ordinary current-grammar recovery and creates no historical syntax kind or
  dedicated removed-spelling diagnostic.
- Structs, enums, and aliases retain typed generic parameters, typed where
  predicates, exact name/generic/member ranges, and typed payload/target
  sources. No string payload is reparsed in HIR or sema.

## Current implementation state

### Carrier and project-symbol checkpoint

The first two merge units are implemented:

- syntax owns `TypePath`, `AuthoredTypeRef`, exact structural source maps,
  recovery nodes, and bounded direct-final nominal declaration grammar;
- HIR publishes typed struct, enum, and type-alias source records through the
  existing `ProjectSymbolTable` transaction;
- local, qualified, alias, glob, and re-export lookup share the same nominal
  IDs and provenance;
- duplicate, reserved, inaccessible, ambiguous, unknown, cyclic, and bounded
  lookup failures are typed project-symbol diagnostics; and
- authored type carriers have been migrated through the existing sema,
  runtime-plan, compiler persistent facts, View lowering, verifier, and LSP
  compile surface without a dual reader or parser-side string reparse.

Focused syntax tests cover nested/repeated/UTF-8/multiline type maps, inclusive
limits, malformed-map rejection, absolute function annotation ranges, nominal
declaration ranges, typed where predicates, and ordinary enum payload recovery.
HIR tests cover all supported import forms, declaration/source provenance,
collision classes, lookup failures, and link budgets.

Validation at this checkpoint:

- focused syntax, HIR, sema, runtime-plan, compiler, verifier, project-loader,
  bundle, and LSP tests passed;
- `cargo check --workspace` passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passed; and
- `just test-workspace` passed after current-grammar fixture and typed
  project-symbol diagnostic expectations were updated.

The package is not complete. The old entry-local nominal resolver and the two
temporary `TypePath::canonical_string()` bridges at that boundary remain. The
next checkpoint must introduce the accepted/open nominal environment and the
single recursive `arcweft_lang_sema::nominal::resolve_type_ref` authority,
delete context-free successful `TypeRef` conversion, and migrate checker,
`Try`/`Await`, compiler/index/LSP evidence. Final Tier 2 and structural audit
also remain.

## Non-goals

- no runtime or wire-schema change;
- no released-format compatibility layer, alias, migration reader, or version
  bump;
- no `ArcResult` or `Unknown` spelling exception;
- no source-text gate or spelling-based architecture test;
- no restoration of removed CSS/Takumi, `hook`, `memo`, `parser`, `source`,
  `stream fn`, `task fn`, or `trusted axiom` surfaces.
