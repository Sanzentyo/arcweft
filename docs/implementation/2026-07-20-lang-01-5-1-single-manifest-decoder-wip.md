# Lang-01.5.1 single-manifest decoder reconciliation — work in progress

## Input and completion boundary

This cut implements
`lang-01.5.1-single-manifest-decoder-implementation-ready-final-contract-9a63ac55.zip`
(SHA-256
`1A3432EB09994AC4E75209CAE2392ED62DEA2F89B26077B244A57440CD01E647`).
The package acceptance matrix remains the completion boundary. This note does
not mark the cut complete.

Implemented so far:

- one strict Taplo-backed schema-1 decoder and source map in `arcweft-launch`;
- typed package/build/profile resolution without filesystem or metadata handles;
- contained project layout and one retained `SourceDocument` for the manifest;
- deterministic source-module and `@character.*` package admission;
- exact selected external-module metadata admission:
  - contained project path;
  - raw metadata hash;
  - strict neutral metadata decode;
  - codec-owned byte, JSON nesting-depth, and lexical-node limits before the
    spanned JSON tree is allocated, with a second semantic-node budget during
    recursive source-map lowering;
  - expected package, version, module, target family, and ABI hash;
  - exact Activity export and abstract `ActivityId` binding;
- mounted generated type/function facts derived into the selected package's
  semantic adapter view, with projector-owned type-reference byte,
  nesting-depth, and work limits;
- removal of the empty `adapter_sources` and `rust_metadata_sources` topology
  carriers;
- deletion of the old `arcweft-launch` `TomlScanner`/source-map reader, the old
  `arcweft-project` manifest reader, the project-local adapter-manifest reader,
  and the Rust-only metadata reader;
- compiler, CLI, and LSP production consumers now retain the accepted manifest,
  layout, topology, adapter projection, and source revision rather than
  reparsing manifest text;
- direct test instrumentation shows that profile resolution and source-map
  consumers reuse the one retained manifest decode rather than invoking the
  decoder again;
- TOML date/datetime nodes are rejected as typed values rather than coerced
  into authored strings;
- every tracked `arcw.toml` uses schema 1 with an explicit package ID and
  version; the Web demo's authored image assets now reside under the canonical
  contained `assets/` root rather than an obsolete manifest-selected root;
- manifest `profiles.<id>.dialogue.defaults` has been removed from the final
  schema and is rejected by the ordinary strict unknown-field diagnostic;
- manifest dialogue profiles now decode nominal `ViewId` and
  `ViewStyleSheetId` selections and resolve them with the engine-owned
  `std.view.dialogue` default into a `DialoguePresentationProfile`;
- the retained typed `profiles.<id>.dialogue.inline-failure` policy is carried
  into runtime-plan lowering instead of being decoded and then discarded;
- LSP accepted-environment publication uses an exact compare-and-swap against
  the environment observed before candidate construction, with typed
  `CurrentChanged`, shutdown, and overflow failures and no partial mutation.

Still required before completion:

- migrate the remaining maintained inline manifests and LSP slow-path fixtures
  to final schema 1;
- finish consumer-equivalence and atomic candidate-publication evidence;
- inject manifest-owned content-unit facts into `ProjectIndex`, then remove the
  obsolete source `content` declaration after the shared syntax/HIR migration
  cut is available;
- remove concrete authored external-module syntax through the same shared
  syntax/HIR migration cut;
- run the package test matrix, workspace check/Clippy, Tier 2, and structural
  audit at the final reviewable gate.

Dialogue presentation-profile admission remains incomplete in production. The
corrected
[Lang-01.5.1.1](../reviews/requests/2026-07-20-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction.md)
redelivery is now implementation-ready against the accepted Lang-01.5.1 crate
graph. It keeps the sole decoder, `ProfileSpec`, and generic source-map owner in
`arcweft-launch`, preserves `inline-failure`, and assigns catalog-aware
admission to the compiler transaction after the one validated View product
exists.

The non-conflicting owner substrate has been implemented in the existing
owners: `ViewId::standard_dialogue()`, dialogue-owned
`DialoguePresentationProfile`, launch-owned nominal profile fields, exact
source-map entries, and field-wise profile resolution. This does not claim
that a selected View or Style has been admitted into a validated runtime
catalog or cascade. The former result-changing conflicts recorded in
[Lang-01.5.1.1.1](../reviews/requests/2026-07-20-lang-01.5.1.1.1-dialogue-profile-owner-and-admission-reconciliation.md)
are resolved by the corrected redelivery. The current public source-map
projection exposes
exact `dialogue.view`, `dialogue.style`, and `dialogue.inline-failure` spans,
but the selected presentation is not yet published into a catalog-validated
runtime plan. There is no `dialogue.defaults` field or orphan `@dialogue.*`
owner. The obsolete `rich-text-profiled` sample and its CLI,
native-observe, and Tier 2 MCP profile-selection assertions were removed
because they only exercised that discarded owner. Package rows F-07 and H-03
must be reintroduced against the corrected owner/admission contract; no old
`DialogueDefaultsItem` projection, presentation-owner guess, or silent
fallback is accepted.

## Contract deviation required by repository policy

The package matrix's D-01 through D-10 rows prescribe permanent
`manifest.removed.*` diagnostics for unreleased spellings. That conflicts with
the repository-wide removed-syntax and source-gate policy in `AGENTS.md`.
The final implementation therefore keeps no legacy spelling table, historical
DTO, dual reader, or spelling-specific diagnostic test. Removed fields are
rejected by the ordinary strict unknown-root/table/field or typed-shape
diagnostics. D-12, F, and H evidence must instead show that final fields reach
their typed consumers and that the old schema cannot be accepted or published.

The package's section 5 and tests A28 through A30 name
`arcweft-render-text::InlineFailurePolicy` as the owner. After the package was
written, the CharacterDialogue migration moved the production policy to
`arcweft-dialogue`. This cut therefore decodes the current owner type directly.
`InlineFailurePolicy`, `InlineFallback`, and `FallbackStylePolicy` derive the
strict tagged Serde contract themselves with `deny_unknown_fields`; no parallel
decode DTO, bridge enum, or field-by-field conversion is retained.

The package's original dialogue-default selection rows F-07 and H-03 are superseded by
the user-directed removal of manifest `dialogue.defaults` and the orphan
`@dialogue.*` presentation owner. Lang-01.5.1.1 supplies their nominal
View/Style/profile owner substrate. The corrected Lang-01.5.1.1 contract now
owns catalog admission and complete runtime publication; Lang-01.5.1.1.1 is
resolved and must not be dispatched again.

## Generated metadata and semantic projection boundary

`LoadedExternalModuleMetadata` retains the complete accepted neutral metadata.
Consequently the authoritative `ManifestVisibility`, `FunctionPurity`, artifact
descriptor, requirements, Activity hashes, source document, and import policy
remain lossless and revision-bound.

The selected `AdapterManifest` is only a derived semantic view for the same
selected package:

- private generated exports are not projected;
- package and public exports are both visible while compiling that selected
  package;
- a pure function with effects, or an effectful function without effects, is
  rejected before projection;
- generated nominal type references are mounted under the selected import;
- a duplicate mounted type/function identity aborts the topology transaction.

Cross-package re-export semantics are not introduced in this cut. They must
consume the exact retained visibility facts later rather than infer visibility
from the same-package `AdapterManifest` projection.

## Remaining source-surface removal inventory

The manifest migration deliberately does not preserve source
`dialogue defaults`. Its syntax/HIR/runtime removal must land atomically with
the shared top-level migration. The following maintained inputs still depend on
that removed owner and must be reconciled in the same cut:

- `web/src/main.arcw`;
- `web/tests/fixtures/style-environment-player.arcw`;
- `samples/zundamon-stand-switch/src/main.arcw`;
- `samples/rich-text-effects-animation.arcw`;
- `samples/rich-text-showcase.arcw`;
- `samples/native-style-parity/main.arcw`;
- `samples/native-style-layout-coverage/main.arcw`;
- `samples/rich-text-full-grammar.arcw`;
- `samples/reactive-view-style/src/main.arcw`;
- `samples/unified-text-visual-parity/main.arcw`;
- `samples/vertical-writing-style/main.arcw`;
- `tests/fixtures/native_capture/unified_text_effects_migration_baseline.arcw`;
- `tests/fixtures/arcw/spec_should_pass/check/053_dialogue_rich_text_full_grammar.arcw`.

No replacement presentation owner is inferred in this cut. A fixture whose
  purpose can be expressed solely through an already accepted authored View or
  Character contract may be rewritten to that existing contract. A fixture that
  only tests the orphan defaults owner must be removed with its assertions and
  replaced by the corrected Lang-01.5.1.1 admitted View/Style behavior, never
  by restoring the old owner.

## Package matrix coverage snapshot

This is an implementation/evidence inventory, not a passing-test claim. The
new and migrated tests below still require the post-integration Cargo gate.

- A-01 through A-17 and A-19 through A-30 have final decoder/lowering paths and
  direct tests. The added grouped cases cover listen, workers, pure thresholds,
  resolved viewport defaults and raw constraints, every current
  `InlineFailurePolicy` variant, strict unknown members/kinds, and exact
  round-trips. A-18's orphan `@dialogue.*` reference is superseded. Its
  replacement nominal View/Style decode and source-map evidence exists, while
  catalog admission remains implementation work under corrected Lang-01.5.1.1.
- B-01 through B-20 have direct index/source-map evidence. The added cases
  cover all typed root-map duplicate IDs, nested table/field collisions,
  profile array and Activity-binding duplicates, non-empty applied style
  arrays, and multibyte UTF-8 byte ranges.
- C-01 through C-15 have pure selection/reference tests, including explicit
  failure without default fallback, default/previous/lexical precedence
  independent of declaration order, the `sans-io` default, omitted unselected
  modules, and deterministic valid joins. C-16 is an API-shape review item and
  remains part of the final structural audit.
- E-01 through E-18, E-20, E-21, and E-23 have production paths and direct or
  owning-crate tests. Added coverage includes every expected metadata field,
  Rust/WASM/process family mismatches, character identity, and nested package
  paths. E-22 currently proves missing/invalid character manifests but not
  missing layer payloads. E-24 proves that an unselected optional unit causes
  no file claim, but an explicit absence fact is not yet published. E-29 proves
  exact membership for the currently admitted manifest, module, generated
  metadata, character-manifest, and consumed text-overlay resources; it remains
  partial because binary character layers and binary overlays do not yet have
  an accepted topology carrier. E-19 and E-25 through E-28 remain
  implementation gaps tied to runtime binding and manifest-content injection
  into project sema. The binary package/optional-presence/root-family decisions
  needed to complete E-22, E-24 through E-29 are isolated in
  [Lang-01.5.1.2](../reviews/requests/2026-07-20-lang-01.5.1.2-typed-content-root-admission-contract-correction.md);
  it does not reopen the completed decoder or metadata substrate. E-19's
  fail-closed host binding key/catalog is independently specified by
  [Lang-01.5.1.3](../reviews/requests/2026-07-20-lang-01.5.1.3-generated-artifact-runtime-binding-contract.md),
  which explicitly excludes provider loading or artifact execution.
- F-01 through F-06 and F-08 through F-10 use the retained typed topology in
  production. The sole decoder has internal one-pass instrumentation. Static
  manifest inspection confirms that `arcweft-launch` has one direct Taplo
  dependency and that `arcweft-project` and `arcweft-project-loader` have no
  direct `toml` dependency; the final structured Cargo-graph review and
  cross-crate consumer evidence in F-11 remain to be completed. F-12 is the
  final workspace compile gate. F-07's replacement profile is decoded and
  resolved; catalog admission and runtime publication remain implementation
  work under the corrected Lang-01.5.1.1 contract.
- G-01 through G-03 and G-07 through G-11 have typed candidate/CAS/cache
  ownership and focused state tests. Failure injection for every metadata,
  character, and source-overlay construction stage (G-04 through G-06), plus
  the explicit no-LKG-report assertion in G-12, still needs final LSP evidence
  after the shared call-surface cut lands.
- H-01, H-02, and H-04 through H-08 are migrated. H-03's obsolete
  `@dialogue.*` sample is removed; its View/Style replacement belongs to the
  corrected Lang-01.5.1.1 admission cut. H-09 still requires the
  remaining LSP inline fixtures; H-10 through H-14 are the final
  format/test/Clippy/workspace, structure-audit, diff, and archive gates.
- D-01 through D-14's permanent removed-spelling diagnostics are superseded by
  the repository-wide prohibition described above. Their actual invariant is
  covered by ordinary strict rejection plus final-schema consumer fixtures;
  no legacy recognizer or dual reader will be restored.

## Post-gate atomic integration checklist

This checklist separates work that can land with the current decoder from work
that requires a returned implementation-ready correction. It is deliberately
file- and evidence-oriented so that releasing the shared Cargo gate does not
turn an unresolved design topic into an inferred production contract.

### Entry conditions

- The shared call-surface/sema cut is committed and its Cargo gate is released.
- The isolated authored `extern rust mod` removal is restored on top of that
  cut before fixing the resulting callers.
- No Lang-01.5.1 change rewrites or preserves the accepted call-surface facts
  merely to make an obsolete fixture compile.
- `jj status` and `jj diff` identify each concurrent slice before any selective
  commit; unrelated Character, suspension, resource, and callable work remains
  owned by its original slice.

### Current decoder cut: proceed immediately after the gate

1. Run `cargo test -p arcweft-launch`. Fix decoder, strict-value, duplicate,
   source-map, profile-selection, and round-trip failures at their owning typed
   boundary. In particular, retain non-empty applied fallback-style evidence;
   do not weaken B-17 to an empty array fixture.
2. Run `cargo test -p arcweft-project-loader --lib topology::tests`, followed by
   `cargo check -p arcweft-launch -p arcweft-project
   -p arcweft-project-loader --all-targets`.
3. Migrate remaining final-schema LSP fixture owners:
   - `crates/arcweft-lsp/src/session/tests.rs`;
   - `crates/arcweft-lsp/src/session/character_definition_tests.rs`;
   - `crates/arcweft-lsp/src/features/entry_roles/tests.rs`;
   - `crates/arcweft-lsp/src/profiles/tests.rs`; and
   - `crates/arcweft-lsp/tests/character_completions.rs`,
     `character_manifest_profile.rs`, and
     `character_nominal_identity.rs`.
   The current static inventory finds old `[package].name` fixture text in six
   of these files, `adapter_manifests`/`rust_metadata` in
   `session/tests.rs`, and `character_manifests` in the four Character fixture
   owners. These counts are a migration inventory only, not a source gate.
4. Replace old adapter-manifest/Rust-metadata fixtures with accepted
   `external-modules` plus strict generated metadata fixtures. Preserve the
   completion, hover, signature, watch-refresh, and document-scoping behavior
   through the accepted topology; do not restore the deleted readers.
5. Rewrite Character fixtures to selected `content-units` roots and canonical
   `.awchar` paths. Do not claim missing-layer coverage until Lang-01.5.1.2
   supplies the binary carrier.
6. Delete LSP assertions whose only subject is the removed
   `dialogue.defaults` manifest field or orphan `@dialogue.*` owner. Record the
  replacement coverage under corrected Lang-01.5.1.1 rather than translating
  those assertions to an invented View or Style owner.
7. Complete G-03 through G-12 failure-injection evidence against final
   metadata, Character, and source-overlay candidate construction. Each failure
   must leave the prior accepted environment, generation, world, catalog, and
   cache namespace unchanged while reporting the candidate as failed, not
   last-known-good.
8. Run `cargo check -p arcweft-lsp --all-targets` and focused LSP tests before
   the broader review gate.

### Authored external-module syntax: ready for direct removal

After restoring the isolated removal, finish the compile-error-driven cleanup
in one coherent cut:

- delete the remaining `ExternModuleItem` AST/CST/grammar surface and parser
  item classification/budget entries;
- delete sema errors and checker/resolver paths that exist only for authored
  `extern rust mod`;
- migrate or delete the parser, declaration, and callable-resolver tests that
  still construct that removed declaration; and
- retain generated external-module registration exclusively through the
  accepted manifest/metadata topology.

The final parser evidence is ordinary current-grammar rejection and absence of
an executable typed node. No spelling-specific diagnostic, deprecated node,
alias, or compatibility import is permitted.

### Dialogue defaults removal: typed owner and admission stage selected

The corrected Lang-01.5.1.1 redelivery keeps the accepted sole-decoder owner
and places catalog-aware admission in the compiler after typed View lowering.
The former Lang-01.5.1.1.1 blocker is resolved. The atomic deletion inventory
is:

- `DialogueDefaultsItem` and its assignment/path types in
  `arcweft-lang-syntax`, including CST classification, grammar budget, parser,
  lint, and parser tests;
- `HirTopLevelDecl::DialogueDefaults`, lowering, cache facts, symbol/project
  index handling, and sema checks;
- `RuntimePlanLowerOptions::dialogue_defaults`, selection and raw/style
  projection in `arcweft-runtime-plan`;
- bundle View-mount selection in `arcweft-cli`;
- dialogue-default canonicalization/edit production code in
  `arcweft-tooling`;
- LSP code actions, hover, definition, references, and source-map assertions
  tied to the orphan owner; and
- every maintained `.arcw`, CLI/native/MCP fixture, and sample listed in
  “Remaining source-surface removal inventory”.

The accepted inline-failure policy remains dialogue-owned and must not be
deleted with the presentation-default owner. The returned contract must select
the real typed View/Style/resource owner before any retained behavior is
rewired.

### Source `content` removal: blocked on typed admission

#### Lang-01.5.1.2 safe binary substrate (2026-07-22)

The parts of the returned Lang-01.5.1.2 contract that do not depend on its
incorrectly revived Source family are now implemented:

- `arcweft-project` owns exact `ProjectBinaryResource` bytes and the canonical
  `ProjectTopologyRevision` v1 transcript types, including typed present,
  semantic, and optional-absence records, stable ordering, duplicate rejection,
  and a fixed digest vector;
- profile topology input has disjoint text and binary overlay/dependency maps,
  rejects same-path kind conflicts and unconsumed binary overlays, never turns
  binary payloads into `SourceDocument`, and publishes exact typed watch
  entries for every retained present resource;
- selected Character roots acquire only the manifest-named layer paths, retain
  the sole source-backed manifest decode, build one complete
  `CharacterPackage`, and share the retained `Arc<[u8]>` layer allocations;
- Character package validation decodes each complete PNG stream and requires
  its dimensions to match the typed manifest rectangle; and
- `SourceBackedManifest` projects content-unit, root-occurrence, and selected
  profile-policy spans from the existing source map without reparsing TOML.

This is deliberately not final content-root admission. Required/optional
candidate facts, accepted absences, the accepted resource-registry semantic
record, candidate topology-revision publication, `ProjectIndex`/`ProgramHash`
injection, source `content` deletion, and bundle/LSP generation migration remain
open. In particular, no `SourceContentRootFamily::Source`, `EntityKind::Source`,
compatibility reader, or provisional closed family was introduced while
[Lang-01.5.1.2.1](../reviews/requests/2026-07-22-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation.md)
is unresolved.

Focused validation for this substrate passed:

- `cargo fmt --all -- --check`;
- `cargo test -p arcweft-project --lib` (31 passed);
- `cargo test -p arcweft-character --all-targets` (51 passed across four test
  binaries);
- `cargo test -p arcweft-launch --lib` (41 passed);
- `cargo check -p arcweft-project-loader --all-targets`;
- `cargo test -p arcweft-project-loader --lib topology::tests` (34 passed); and
- `cargo test -p arcweft-bundle --test character_package` (3 passed).

Do not delete or replace `EntityDeclKind::Content` until
[Lang-01.5.1.2](../reviews/requests/2026-07-20-lang-01.5.1.2-typed-content-root-admission-contract-correction.md)
returns a complete binary resource/revision and `ProjectIndex` injection
contract. Its eventual atomic inventory includes:

- syntax header/body parsing, `ContentDeclBody`, lint and recovery tests;
- HIR/entity-kind lowering and sema entity/project-index ownership;
- manifest content-unit facts, presence/absence and family diagnostics,
  complete `.awchar` payload admission, and binary overlay revision;
- compiler/bundle/watch/LSP consumers of the injected typed facts; and
- ordinary parser rejection of source `content` with no historical node.

The returned package's binary topology and Character-package parts are
concrete, but its closed family incorrectly retains the independently removed
Source entity. Final family and admission work therefore also waits on
[Lang-01.5.1.2.1 content-root family / Source-elimination reconciliation](../reviews/requests/2026-07-22-lang-01.5.1.2.1-content-root-family-source-elimination-reconciliation.md).

Until then E-22, E-24, and E-29 remain partial and E-25 through E-28 remain
open. A text-only `SourceDocument`, `Arc<str>` overlay, directory scan, or
ad-hoc digest is not an acceptable stand-in.

### Generated runtime binding: blocked on an exact key

E-19 waits for
[Lang-01.5.1.3](../reviews/requests/2026-07-20-lang-01.5.1.3-generated-artifact-runtime-binding-contract.md).
Do not bind a selected export by callable spelling, Activity spelling, mount,
basename, or adapter profile. The later implementation must consume the exact
accepted metadata/artifact/export/revision key and fail closed before host work
is started.

### Final review gate

After every implementation-ready slice above has landed at a coherent cut:

1. run focused formatting/tests and `cargo clippy` for changed crates;
2. run `just test-workspace`;
3. run `just test-tier2`, because this cut spans public manifest, runtime,
   Agent/MCP, Character, and LSP boundaries;
4. reconcile stale resource URIs, semantic identities, and authored View
   geometry without adding production compatibility paths;
5. run
   `cargo +nightly -Zscript tools/structure-audit.rs --root .`;
6. run `git diff --check` and inspect the exact final `jj diff`; and
7. update this note with command exits and the final row-by-row matrix before
   describing Lang-01.5.1 as complete.

## Verification status

Current focused evidence:

- the public `arcweft-launch` maintained-manifest integration test decodes all
  13 tracked `arcw.toml` fixtures through `SourceBackedManifest::decode` and
  verifies canonical schema 1. This exposed invalid root-valued
  `source-dir = "."` entries in the Zundamon stand-switch and Web samples;
  both sources now live at canonical `src/main.arcw` paths, their manifests
  select those exact files, and exact `arcw check` profile runs pass;
- the checked-in `web/demo.awfb` was deterministically regenerated through
  `just fixture-refresh-web-demo-awfb`, inspected through `arcw inspect`, and
  exercised by the seven-case Web parity suite. The bundle now uses canonical
  schema 3 and contains 26 virtual files;
- `cargo check -p arcweft-launch -p arcweft-project
  -p arcweft-project-loader --all-targets` passed;
- `cargo test -p arcweft-launch` passed all 32 decoder, source-map, strict
  shape, duplicate, profile-selection, and round-trip tests;
- `cargo test -p arcweft-project-loader --lib topology::tests` passed all 39
  topology tests, the complete project-loader unit set passed all 128 tests,
  and `cargo test -p arcweft-project-loader --test dependency_direction`
  passed all four structured dependency-direction tests;
- `cargo clippy -p arcweft-launch --all-targets --all-features --
  -D warnings` passed, as did the project-loader and compiler-style changed
  targets with `--no-deps -D warnings`. The latter form was used only to
  isolate this cut while concurrent retained-grammar work was changing the
  shared syntax dependency;
- `cargo check -p arcweft-cli --lib` and `cargo check -p arcweft-lsp --lib`
  passed after their production consumer migrations;
- the generated metadata JSON/value and type-reference resource limits have
  passing negative unit/integration tests;
- `cargo test -p arcweft-lang-sema --lib` passed all 804 tests after two Tier 2
  boundary defects were fixed at their shared semantic owners:
  - payload-free `Ref<Signal>`/`Ref<Metric>` family constraints now accept a
    same-family payload-specialized reference without erasing the actual
    payload type; and
  - the `image` callable schema leaves positional `source` optional because the
    canonical named `asset = ...` form is an alternative. The presentation
    validator remains the single owner of the rule requiring either a declared
    Image source or an Asset source.
- the exact ignored Tier 2 cases
  `agent_mcp_stdio_runs_agent_script` and
  `agent_mcp_stdio_reads_animated_image_layer_resource` passed after those
  root-cause fixes;
- `cargo test -p arcweft-cli --features native-capture --test check
  agent_mcp_stdio -- --ignored --nocapture` passed all 22 MCP stdio cases with
  no poisoned-mutex cascade and no remaining stale resource, identity, or
  geometry expectation in that set; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  passed after the final CLI/LSP consumer migration;
- direct `arcw run --runner native samples/modern-feedback-view/src/main.arcw`
  now discovers the containing manifest for its canonical project-owned
  asset/content/local-state roots and reuses the matching automatic game
  profile. It compiles, encodes, and writes the bundle without the former
  missing-image-asset or missing-entry failure. The independent duplicate
  native View-handler `submit` projection was subsequently removed at its
  owner; the exact command now reaches and keeps the native player window
  alive without a diagnostic;
- missing authored Image assets now produce
  `bundle.image.missing_asset_reference` through the shared structured
  diagnostic emitter with the parser-owned declaration-ID span and a primary
  snippet label. The focused Image diagnostic tests passed all three cases,
  the direct containing-manifest root test passed, and the real CLI output was
  inspected; and
- the structural audit completed over 3,383 files / 1,747 Rust files /
  807,340 physical Rust LOC with zero errors and 129 ownership warnings.

The 22-case MCP stdio group was rerun after the native View-handler correction
and passed again. The settled integrated checkout subsequently passed
`just test-workspace`; all remaining Tier 2 Agent-observe, auxiliary-capture,
native-capture, and visual-golden groups; the final all-feature workspace check
and Clippy run; `cargo fmt --all -- --check`; and `git diff --check`.

The product-catalog boundary was also corrected while regenerating the Web
fixture. Raw virtual-file bytes are bounded independently at 16 MiB, while the
canonical JSON transcript has its own 64 MiB limit. This prevents raw payload
growth from escaping accounting without incorrectly rejecting deterministic
JSON expansion. Focused bundle tests and the final workspace/Tier 2 gates cover
the corrected boundary.
