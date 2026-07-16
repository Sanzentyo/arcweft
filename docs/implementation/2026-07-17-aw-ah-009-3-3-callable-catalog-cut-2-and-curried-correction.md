# AW-AH-009.3.3 callable catalog Cut 2 and AW-AH-009.3.3.1 correction

## Basis and status

This cut consumes two implementation-ready design packages:

- `arcweft-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation-final-contract.zip`, SHA-256 `9D1F989F5E0E698AEFF1098DD7ECEE7E01A66616A00A0571EE333A3B1B7DDC78`;
- `arcweft-aw-ah-009.3.3.1-curried-callable-group-validation-contract-correction-final-contract.zip`, SHA-256 `3D81158EB37F503EF7B0F242A79015BA1AB00E3954A8DAE4384F45EAAB55B672`.

Both archives were read in full before implementation. Their manifests matched
their members, both status files declare the packages ready, and both have zero
open questions. The implementation was developed from Git revision
`95a3631483b9`, then successively rebased onto the current `main` Git revision
`7996499e`. The intermediate rebase required only a mechanical merge of
independent test imports in the registration suite; the final rebase was
conflict-free, and the combined behavior was validated afterward.

This is a coherent publication/registration cut, not completion of the entire
AW-AH-009.3.3 shared-resolver migration. The immutable catalog is now accepted
atomically with the semantic world, but the checker still owns the one production
successful resolver. Native semantic signature help and the deletion of the old
checker maps remain later ordered cuts.

## Implemented publication path

### Exact syntax and HIR source evidence

Function parsing now retains exact source ranges for the name, complete signature,
result, parameter group, parameter, parameter name, parameter type, and default.
`HirCallableSignatureSource` publishes the typed `FnSignature`, declaration and
module identity, documentation, declared effects, and revision-bound `SourceSpan`
evidence. `HirProject::callable_signature_sources` and
`HirProject::module_callable_signature_sources` expose deterministic module-order
rows, including an explicit empty row for a module without callables.

HIR remains independent of sema. It publishes source facts only and does not
construct sema candidate or catalog types.

### Typed adapter publication

`arcweft-adapter-context` now owns validated adapter-local callable names, paths,
indices, grouped signatures, parameter passing/presence, tooling subjects, and
documentation. `AdapterManifest::try_callable_publication` is the single adapter
normalization boundary into sema-owned `EnvironmentCallablePublication`.

The six standard manifests, the selected custom adapter, desktop manifests, project
loader, codecs, LSP profile loading, and CLI project loading use typed path segments.
Rust export names are validated as one callable segment; `rust_path` is retained only
as provenance and is never split. Complete Rust package identity, purity, effects,
parameter groups, documentation, and declaration order are retained.

`AdapterManifest::apply_to_env` no longer mutates callable function or method maps.
It continues to publish non-callable symbols, capabilities, Rust nominal types, and
target effect availability owned by the existing environment boundary.

### Atomic registered catalog

`RegisteredCallableCatalogBuilder` constructs project and environment catalogs with
checked work accounting and deterministic structural ordering. Registration adds, in
order, HIR project callables and bindings, the core environment publication, the six
standard manifest publications, and the selected adapter publication. It validates
limits, IDs, overload continuity, same-rank collisions, standard/adapter equivalence,
and source identity before publication.

`RegisteredTypeCheckEnv` now owns an immutable `Arc<RegisteredCallableCatalog>`.
`CharacterRegistrar` publishes that environment only after the complete catalog
succeeds; any catalog failure becomes a typed registration diagnostic and preserves
the previous accepted world. Public reads use:

- `RegisteredTypeCheckEnv::callable_catalog`;
- `RegisteredCallableCatalog::{project, environment, project_binding, project_record, free, method, environment_record}`;
- `CallableRecord` and its retained `Arc<CallableSignatureSchema>`.

No mutable builder is publicly constructible and no serialized catalog format was
added.

## AW-AH-009.3.3.1 curried correction

`CallableIdentityError::MissingGroup` was deleted. The context-free
`CurriedCallableId::try_new` boundary now:

1. rejects an already wrapped or data-last base before checking the group;
2. rejects group zero with `InvalidCurriedGroup { base, group }`;
3. accepts every structurally valid nonzero group without consulting a schema.

`ResolvedCallable::try_new` is the schema-aware success boundary. Only a matching
`CallableCandidateId::Curried` and `CallableInstantiation::Curried` pair can succeed,
and the retained schema must contain the selected group. A base candidate paired with
a curried instantiation, a mismatched base/group, a non-curried instantiation, a
missing group, a one-over group, or a corrupt prebuilt candidate fails closed. Missing
schema groups produce `ResolveCallError::InvalidCallGroup` with the unwrapped base
candidate.

Direct tests cover project, standard, and adapter candidates; exact multi-group
success; wrapper-before-group error precedence; group zero; one-over groups; base and
instantiation mismatches; corrupt-world defense; and the stable diagnostic code.
The correction does not add a schema lookup to the identity constructor, a global
catalog lookup, a compatibility alias, or a second resolver.

## Boundaries for the later top-level declaration reduction

The catalog does not use `FunctionKind` as callable identity. Every current
`HirFunction`, including `FunctionKind::Task`, `FunctionKind::Dialogue`, and
`FunctionKind::Stream`, is published through the same
`HirCallableSignatureSource`; `CallableDeclarationId::for_function` is keyed by
package, module, function owner, and name. Therefore removing the dedicated task or
stream surface later must reuse this catalog and must not create another callable
catalog, ID family, or signature publication route.

The later change must audit these exact seams:

- syntax/HIR `FunctionKind` parsing and lowering may change, while the ordinary
  `FnSignature` and exact source-span publication should remain the catalog input;
- if task/stream execution policy changes callable effects or result typing, that
  policy belongs in schema construction or a selected validator, not in a parallel
  lookup table;
- accepted worlds must continue to expose one `RegisteredCallableCatalog` through
  `RegisteredTypeCheckEnv`;
- the eventual shared checker resolver must consume that catalog and publish one
  `ResolvedCallable` through `ResolvedCallable::try_new`;
- LSP signature help must project the accepted semantic result and delete its current
  adapter-word fallback only when that native query exists.

## Explicit remaining work

The following package requirements are not claimed complete by this cut:

1. `CallResolverRequest` and a production `resolve_call_target` implementation do
   not yet exist. Consequently the correction package's resolver-integration test
   rows cannot be connected yet; constructor and resolved-boundary rows are complete.
2. Existing checker builtin, FX, Agent, presentation, dialogue, trait, selected-call,
   data-last, and ordinary function-map branches have not been migrated or deleted.
3. Checker call/argument facts and native semantic signature help are not connected
   to the catalog. LSP still has a typed adapter-metadata word fallback.
4. The legacy core `TypeCheckEnv` function inventory is string-keyed. Its one-time
   core publication currently converts that legacy dotted storage into typed path
   segments. This transitional conversion must disappear with the old successful
   function-map route; it is not a permissible final identity owner.
5. `ProjectSymbolTable` still exposes opaque external leaves such as
   `character.akane` as strings. Compact aliases can be published, but complete
   qualified non-callable shadow publication requires the independently throwable
   design request
   `docs/reviews/requests/2026-07-17-aw-ah-009.3.3.2-typed-external-project-binding-path-publication.md`.

These gaps keep the overall AW-AH-009.3.3 goal open. They are recorded rather than
hidden behind a compatibility reader, guessed string split in the external binding
builder, or a second successful resolver.

## Verification after the final rebase

```text
cargo fmt --all -- --check
  PASS

cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir
  -p arcweft-lang-sema -p arcweft-adapter-context
  -p arcweft-adapter-desktop -p arcweft-project-loader
  -p arcweft-lsp -p arcweft-verify-lsp
  --all-targets --features arcweft-adapter-context/sema
  --no-deps -- -D warnings
  PASS

The same changed-crate clippy command without `--no-deps`
  BLOCKED — the final base introduces six pre-existing `-D warnings` failures in
  arcweft-bundle View validation code before all changed crates can be linted.

cargo test -p arcweft-lang-syntax -p arcweft-lang-hir
  -p arcweft-lang-sema -p arcweft-adapter-context
  -p arcweft-adapter-desktop -p arcweft-project-loader
  -p arcweft-lsp -p arcweft-verify-lsp
  --all-targets --features arcweft-adapter-context/sema
  PASS
  Notable complete suites: sema 667, syntax 202, LSP library 127.

cargo test -p arcweft-lang-sema curried_id_
  PASS — 5
cargo test -p arcweft-lang-sema resolved_curried_
  PASS — 9
cargo test -p arcweft-lang-sema registration::tests
  PASS — 57

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS — 0 errors, 128 warnings

jj diff --git --color never |
  git apply --check --reverse --whitespace=error-all -
  PASS

cargo check --workspace --all-targets
  BLOCKED — the existing checkout does not contain
  web/assets/noto-sans-jp-vf.ttf, required by arcweft-glyphon and
  arcweft-render-wgpu tests.

cargo check --workspace
  BLOCKED — the same missing font is included by arcweft-player-scene.

cargo clippy --workspace --all-targets --all-features -- -D warnings
  BLOCKED — the missing font is included by arcweft-glyphon and
  arcweft-render-wgpu tests. The dependency-aware changed-crate command above
  separately exposes the final base's six pre-existing arcweft-bundle lints.
```

The first combined test attempt exceeded its five-minute command timeout while
compiling and left no failing test result. After compilation completed, the exact
changed-crate suite was rerun and passed. Three LSP tests initially exposed an old
dotted Rust export fixture; the fixture was moved directly to the required
single-segment model and all LSP tests then passed. No old-spelling acceptance path
was added.

The Jujutsu workspace has no ordinary `.git` worktree, so the patch was checked in
reverse against the current files to obtain the same whitespace-error evidence as
`git diff --check`. The workspace-wide commands were attempted and their unrelated
asset blocker is recorded rather than hidden. All changed crates pass their complete
tests and `-D warnings` clippy on the final base.

## Structural audit

The canonical reports are under
`docs/implementation/structure-audits/aw-ah-009-3-3-callable-catalog-cut-2-2026-07-17/`.
They contain exact metrics for 3,113 scanned files, 1,560 Rust files, 716,353 Rust
physical lines, all dependency edges, and all warning-level hotspots.

No Cargo manifest changed. Current fan-out/fan-in counts are: syntax 7/14, HIR 5/11,
sema 12/11, adapter-context 9/7, adapter-desktop 8/2, project-loader 17/2, LSP 28/0,
and verify-lsp 10/1.

Changed-file warning hotspots were reviewed with current sizes, not diff additions:

| Path | Bytes | Physical LOC | Classification and responsibility |
|---|---:|---:|---|
| `crates/arcweft-lang-sema/src/registration/tests.rs` | 92,855 | 2,539 | test; atomic registration, collision, limit, and inventory behavior |
| `crates/arcweft-lsp/src/session/tests.rs` | 85,047 | 2,524 | test; session/profile lifecycle and request behavior |
| `crates/arcweft-verify-lsp/src/lib.rs` | 71,767 | 1,899 | production facade plus 788 embedded test lines; existing verifier/LSP projection surface |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | 41,691 | 1,758 | production; top-level typed AST declarations |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | 58,024 | 1,631 | production; top-level declaration parsing and recovery |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | 37,119 | 1,294 | production; one closed callable identity hierarchy |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | 43,495 | 1,224 | production; existing checker helper inventory, touched only for owned type conversion |
| `crates/arcweft-adapter-context/src/manifest.rs` | 46,994 | 1,376 | production manifest boundary plus 417 embedded test lines |

The new responsibility modules are within the preferred range:
`callable/builder.rs` is 34,507 bytes/892 lines, `types/order.rs` is 14,856 bytes/394
lines, adapter `callable.rs` is 16,900 bytes/543 lines, adapter `publication.rs` is
19,058 bytes/496 lines, and HIR `callable_source.rs` is 5,507 bytes/217 lines.
The audit found no error-level structural violation. Existing large facade/parser/test
files remain warning-level debt; this cut did not add a new crate edge or mix runtime,
transport, persistence, rendering, or platform I/O into catalog ownership.

## Design deviations

`TypeKind::stable_ordering` is implemented in the responsibility module
`types/order.rs` rather than by debug/display formatting in the catalog builder. This
keeps deterministic structural ordering owned by `TypeKind` and avoids a scattered
field projection.

The adapter manifest's Rust ingestion API is fallible (`try_with_rust_manifest`)
because the final typed model must reject invalid exported callable names and index
overflow before publication. It does not retain the previous infallible stringly
constructor as a compatibility path.
