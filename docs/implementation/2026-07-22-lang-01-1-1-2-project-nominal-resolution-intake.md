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

At the carrier checkpoint the package was not complete: the old entry-local
resolver and temporary `TypePath::canonical_string()` bridges still remained.
The current working cut has since introduced the accepted/open environment and
single recursive resolver and has removed entry's independent successful
project/alias resolution. The package remains open until every consumer and
the validation obligations below have converged.

### Shared resolver checkpoint

The current working cut now implements the single accepted resolver boundary
through ordinary checking, entry checking, source-backed callable publication,
trait and impl collection, project nominal record/enum shapes, project index,
and LSP nominal definition/reference/rename projection. The accepted callable
catalog retains the same resolution index used by focused signature queries;
there is no callable-local `Named` fallback.

The resolver and its cache are stack-safe at the production recursive-depth
boundary. A 256-level authored type is accepted, a 257-level type produces the
typed limit diagnostic and poison evidence, and the syntax source map,
resolution walk, poison collection, and structural cache digest all use
bounded iterative traversal for that shape.

Validation for this checkpoint currently includes:

- `cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir
  -p arcweft-lang-sema --all-targets -- -D warnings`;
- all syntax, HIR, and sema library, integration, compile-fail, and doc tests;
- 912 sema library tests, including the exact recursive-depth and cache-key
  boundary tests; and
- `cargo check --workspace --all-targets --all-features`.

The old call-surface fixtures that supplied project-authored types through
`TypeKind::Named` were corrected to use project nominal parameters or an exact
accepted nominal record. Production did not gain a spelling comparison,
`Named`/project compatibility rule, or fabricated `CharacterLook` fallback.

This checkpoint is still not the final package cut. The nine `TM-*` rows that
couple nominal poison to prefix Try, postfix Try, and propagating Await belong
to the selected Lang-01.1.1.1 propagation implementation and remain open. The
full 242-row matrix trace, workspace Clippy/test, applicable Tier 2 route, and
structural audit must be recorded after that propagation boundary is merged.

## Active completion audit

The shared resolver migration is now in progress. This slice is not complete
merely when it compiles. Before its final cut it must also close these observed
requirements from the returned contract:

- callable registration must resolve source-backed signatures through the same
  accepted nominal world without a registration-order cycle;
- trait/impl catalogs must consume source-backed resolver results rather than a
  detached local `TypeRef` converter;
- normal record fields and enum payloads must be selected by typed project
  nominal declaration identity, including generic substitution, rather than by
  a simple-name map;
- the checked type-reference cache must use the complete world, revision,
  module, source root, structural digest, generic/Self fingerprints, catalog
  digest, schema, and limits key specified by the contract;
- project-index and LSP definition/reference/rename must consume exact typed
  reference edges and must not infer an edit target by scanning source text;
- every applicable `TEST_MATRIX.csv` family, focused crate command, Tier 2
  route, and structural-audit requirement must have direct evidence.

`Ref<Entity>` is the one isolated design defect in the returned contract. It is
tracked by [Lang-01.1.1.2.1](../reviews/requests/2026-07-22-lang-01.1.1.2.1-entity-family-applied-type-projection-correction.md);
the rest of this list remains implementation work and is not deferred with it.

The implementation audit also found that adapter/Rust callable publication
still projects `ArcweftRustTypeRef::Named` through `AdapterTypeKind::Named` into
`TypeKind::Named`, while authored `extern` signatures now resolve the same
export to an owner-qualified `AcceptedNominalType`. The returned contract
requires external-owner projection but explicitly does not design callable
publication, so the missing owner/context/registration-order decision is
tracked separately by [Lang-01.1.1.2.2](../reviews/requests/2026-07-22-lang-01.1.1.2.2-adapter-callable-nominal-publication-projection-correction.md).
No `Named` compatibility comparison is admitted while that request is pending.

## Current checkout validation and open gates

The current nominal cut also carries the accepted type evidence into tooling:

- LSP nominal definition, reference, rename, completion, hover, and expression
  inlay consumers select the accepted project and exact source identity;
- dialogue-View role hover retains its richer role metadata instead of being
  shadowed by the generic nominal hover;
- `TypeJudgment` records the exact optional `SourceSpan`, so equal local ranges
  in different accepted documents cannot be mixed; and
- signature-cache final-stamp tests use an explicit long test deadline and
  assert `aw.signature.stale.document_changed`, while the production request
  deadline remains 250 ms.

Current direct evidence:

- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passes;
- `cargo test -p arcweft-lsp --lib --all-features` passes 167 tests with zero
  failures and one explicitly blocked adapter-publication acceptance test;
- the CLI-excluding workspace lib/test phase of `just test-workspace`, including
  all 912 sema library tests and the compile-fail suites, passes;
- `cargo test -p arcweft-cli --lib --bins --quiet` passes all 194 tests after
  stale View-state fixtures were given source-owned nominal declarations;
- current-pass run fixtures pass after `IteratorItem` and `CaptureError` became
  ordinary source declarations;
- `just test-tier2` passes all 46 selected MCP stdio, Agent observe, native
  auxiliary capture, visual smoke, and exact imq golden tests; and
- the canonical structural audit at
  `structure-audits/lang-01-1-1-2-project-nominal-resolution-2026-07-22/`
  scanned 3,571 files, 1,873 Rust files, 869,262 physical Rust LOC, and 94
  manifests with zero errors and 138 warnings.

The structural review also moved test-only responsibilities without changing
production behavior: HIR symbol tests now have a 711-line shared-fixture parent
and six child modules, sema registration tests have a 209-line parent and five
child modules, syntax type-source tests moved out of the 1,049-line production
module, and adapter-manifest tests moved out of the 797-line production module.
All moved test groups and their focused Clippy routes pass.

The normal workspace route is not claimed complete. Its remaining failures are
direct executable evidence for already isolated boundaries:

1. `current_pass/check/014_struct_enum_type_alias.arcw` and
   `015_state_defaults.arcw` use canonical `Ref<Flow>`. They remain rejected
   until [Lang-01.1.1.2.1](../reviews/requests/2026-07-22-lang-01.1.1.2.1-entity-family-applied-type-projection-correction.md)
   selects the typed entity-family projection. An opaque `Ref` fallback is not
   admitted.
2. The ignored LSP adapter signature-help acceptance test uses `TensorF32` and
   remains blocked by [Lang-01.1.1.2.2](../reviews/requests/2026-07-22-lang-01.1.1.2.2-adapter-callable-nominal-publication-projection-correction.md).
   Its ignore annotation names that exact request and must be removed in the
   integration cut.
3. The two `spec_should_pass` filesystem capability fixtures retain
   `type FsError` inside `extern capability`. Proof-concurrency Stage 1 already
   gives that member a private typed syntax node but explicitly defers the
   atomic public AST/HIR switch. The public `ExternCapabilityItem` therefore
   cannot yet publish `FsError` into the accepted nominal world. This is an
   existing proof-switch implementation dependency, not a new design request;
   adding a global `FsError` prelude would hide the missing owner boundary.

The current Tier 2 route passes independently of these authoring-surface gates.
It must be rerun after the three gates land because their downstream accepted
project and observation identities will have changed; the present pass is not
used to claim those future integration cuts complete.

## Non-goals

- no runtime or wire-schema change;
- no released-format compatibility layer, alias, migration reader, or version
  bump;
- no `ArcResult` or `Unknown` spelling exception;
- no source-text gate or spelling-based architecture test;
- no restoration of removed CSS/Takumi, `hook`, `memo`, `parser`, `source`,
  `stream fn`, `task fn`, or `trusted axiom` surfaces.
