# Maintainability audit — 2026-07-10

## Scope

This audit tracks removal of ad hoc implementation, redundant wrappers and
low-value tests, clearer responsibility boundaries, and implementation/spec
gaps found while reviewing the current Rust workspace. Stable design chapters
are changed only when a design contract changes; this file records checkout
measurements, implementation decisions, validation, and remaining work.

The audit started from Git revision `a59be17c2455` / Jujutsu change
`qrkrvnuumznk`. The working copy was clean.

## Baseline

The canonical structural audit reported:

```text
files scanned: 2520
Rust files: 1180
Rust physical LOC: 588316
package manifests: 91
violations: 1 error(s), 152 warning(s)
```

The only error-level finding was
`crates/arcweft-cli/src/app/bundle_view.rs`: 91,581 bytes and 2,590 physical
LOC. It mixed stateful component View lowering with deterministic schema
identity serialization. It is production code, contains no embedded test
module, and belongs to `arcweft-cli`. This slice does not change Cargo
dependencies, so workspace dependency fan-in and fan-out are unchanged.

## Slice 1: isolate View schema identity generation

`bundle_view_schema.rs` now owns the deterministic conversion from View
expressions and patterns to digest-backed schema references. The module-level
documentation states the invariant: schema identity generation must not depend
on mutable layout or input-resource lowering state. `bundle_view.rs` retains
orchestration, layout, style, focus, input, and resource emission.

Current checkout measurements after the split:

| Path | Kind | Bytes | Physical LOC | Embedded tests | Major responsibilities |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-cli/src/app.rs` | production facade | 5,352 | 128 | no | CLI application module declarations and command dispatch |
| `crates/arcweft-cli/src/app/bundle_view.rs` | production | 87,720 | 2,468 | no | View lowering orchestration, layout, style, focus, input, and sidecar emission |
| `crates/arcweft-cli/src/app/bundle_view_schema.rs` | production responsibility module | 4,525 | 138 | no | Canonical pattern/expression schema source and deterministic digest references |

The canonical audit now reports zero error-level violations. The remaining
size warning on `bundle_view.rs` is real; later decomposition should separate
scroll/style resolution and text-control authoring as cohesive modules rather
than creating pass-through wrappers.

No new test was added for the file move. Existing bundle behavior tests already
exercise let, await, match, and repeat schema reference emission, so a test that
only asserted the new private module boundary would be implementation-coupled
and low value.

## Remaining follow-up findings

The audit has confirmed these higher-value implementation issues for separate,
reviewable slices:

- `arcweft-lang-sema` manually repeats every `DataFormat` variant even though
  `arcweft-data::DataFormat` owns the enum and its author-facing names. The
  owning enum needs an iterable authoritative inventory, and semantic
  registration must consume it.
- Dialogue speaker and generated text-key normalization is duplicated between
  HIR lowering and ID-context tooling. The implementations have diverged for
  some author spellings and need one shared domain rule with parity tests.
- Pure and engine evaluators duplicate sequence length/index conversion logic.
  Their behavior must be compared before moving shared integer/index behavior
  onto the owning runtime value types.

## Slice 1 validation

```text
cargo fmt --all
cargo check -p arcweft-cli --lib --all-features
cargo test -p arcweft-cli --all-features --lib app::bundle::tests -- --nocapture
  36 passed; 0 failed; 0 ignored
cargo check --workspace
cargo clippy --workspace --all-targets --all-features
  passed with pre-existing warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/slice-01
  0 error(s); 153 warning(s)
```

`just test-workspace` reached `arcweft-core` and exposed two pre-existing stale
tests: `engine_steps_flow_ops_and_applies_goto` and
`game_mode_stops_on_visible_output_but_server_mode_drains` send the authored
source ID `say.opening.001` as an input payload, while `RuntimeLineId` now
normalizes that source family to the canonical runtime label `opening.001`.
Both tests also fail when run alone, so this is not test-order interference and
is unrelated to the CLI-only module split. A separate test-contract slice must
construct the advance payload from the typed runtime line ID and then rerun the
workspace gate.

## Slice 2: restore typed test contracts and remove false source gates

The normal workspace route exposed a chain of stale tests that had survived
earlier API and grammar migrations. This slice now derives dialogue advance
payloads and native status labels from `RuntimeLineId`, tests fixed-arity
spread rejection with a genuinely dynamic sequence, pins presentation handle
IDs to their canonical `handle.main.*` form, and updates the persistent-cache
fixtures from removed `start(...)` syntax to `goto`. The multi-module cache
golden now uses the language-owned dot-separated canonical module path
`crate.support`.

Five tests that inspected implementation source or documentation text were
removed instead of being retargeted to new symbol names or file locations:

- one text-control writeback source-spelling test;
- two inset-shadow implementation/collector source-spelling tests;
- the repository-wide removed-word scan;
- the repository-wide host-path text scan.

The corresponding `just` source-scan entrypoints were removed. Inset-shadow
exact PNG packet validation remains, as do the behavior tests for text-control
commands. This avoids converting the same brittle source scan into a differently
named structural rule.

Current changed-Rust-file measurements at Jujutsu change `vxqyvwvwkoqy`:

| Path | Kind | Bytes | Physical LOC | Embedded tests | Major responsibility touched |
| --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-core/src/tests.rs` | unit-test support | 1,725 | 59 | no | typed dialogue input construction |
| `crates/arcweft-core/src/tests/flow.rs` | unit tests | 88,953 | 2,553 | no | engine flow behavior |
| `crates/arcweft-core/src/tests/step.rs` | unit tests | 6,360 | 198 | no | game/server stepping behavior |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | unit tests | 42,193 | 1,394 | no | declaration and spread semantics |
| `crates/arcweft-player-native/src/lib.rs` | production facade | 13,734 | 367 | yes | native player orchestration and status reporting |
| `crates/arcweft-player-scene/tests/text_control_writeback_source_gates.rs` | integration tests | 1,400 | 30 | no | text-control command behavior evidence |
| `crates/arcweft-player-web/tests/parity.rs` | integration tests | 29,159 | 845 | no | web/native presentation parity |
| `crates/arcweft-render-wgpu/tests/view_box_shadow_exact_png_golden.rs` | integration tests | 6,275 | 201 | no | exact PNG artifact packets |
| `crates/arcweft-runtime-plan/src/flow/tests.rs` | unit tests | 22,292 | 722 | no | flow-to-runtime-plan lowering |
| `crates/arcweft-cli/tests/regression_harness.rs` | integration tests | 5,061 | 173 | no | checkout hygiene and audited unsafe boundaries |

No Cargo dependency, public production contract, or crate boundary changed, so
dependency fan-in and fan-out are unchanged. The largest non-generated
production files remain `arcweft-core/src/awbc/product_step.rs` (95,176 bytes,
2,499 LOC), `arcweft-core/src/value.rs` (83,955 bytes, 2,498 LOC),
`arcweft-core/src/engine/eval/calls.rs` (89,488 bytes, 2,481 LOC), and
`arcweft-cli/src/app/bundle_view.rs` plus `arcweft-runtime-plan/src/flow.rs`
(both 2,468 LOC). They remain warning-level decomposition candidates; this
test-contract slice does not mix in unrelated production splits.

## Slice 2 validation

```text
cargo fmt --all -- --check
cargo test -p arcweft-core --lib
  172 passed; 0 failed
cargo test -p arcweft-runtime-plan --lib flow::tests
  12 passed; 0 failed
cargo test -p arcweft-cli --test regression_harness
  2 passed; 0 failed
cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens
  2 passed; 0 failed
cargo test -p arcweft-render-wgpu --test view_box_shadow_exact_png_golden --all-features
  0 passed; 0 failed; 2 ignored Tier 2 packets
just test-workspace
  passed
cargo clippy --workspace --all-targets --all-features
  passed with pre-existing warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/slice-02
  0 error(s); 153 warning(s)
```

## Slice 3: remove source gates and finish the contracts they obscured

Source gates are not required for Arcweft's correctness. They coupled tests to
spellings, paths, and temporary implementation shapes while missing real
behavioral defects. `AGENTS.md` now prohibits adding or repairing them and
explicitly supersedes older request packages that prescribed source scans.

The checkout removes 43 obsolete entries: 27 Rust integration-test files,
seven Rust audit/tool scripts, eight placeholder or source-gate fixture files,
and one web source-inspection test. Not every deleted test was named as a source
gate; the same review also removed smoke tests that only checked helper output,
authored sidecar literals, fixed object IDs, transparent padding, or observation
geometry that the public report does not expose. The complete slice is 98 files,
1,900 insertions, and 6,166 deletions.

Observable invariants were retained at their owning boundaries:

- patch validation now uses typed tamper, duplicate-operation, wrong-base,
  no-op, manifest-only, and materialized-target behavior tests;
- CLI native-option rejection is exercised by invoking the binary, and visual
  capture tests derive observed IDs and validate serialized capture metadata;
- text wrapping remains covered by `arcweft-text-layout` typed geometry tests,
  rather than a CLI report that does not publish cluster geometry;
- unsafe isolation is enforced by crate lints and two explicitly named unsafe
  modules, rather than repository text scanning;
- macOS adapter availability uses target/feature compilation, while web IME
  smoke reports an explicit environment block only when no WebGPU adapter is
  available;
- Takumi compositing evidence compares the complete deterministic evidence
  object, without a placeholder PNG or authored CSS sidecar.

### Patch identity and hot-swap final form

Patch materialization now recomputes target fingerprints and compatibility from
the actual base and target bytes. `PatchMaterializedTarget` is the verified
boundary: bytes and report are private, duplicate section operation IDs are
rejected, no-op artifacts still materialize and validate, and native owners
reuse that single materialization for resource validation and commit.

The review then exposed a deeper identity hole: AWFB `content_root` covers
section descriptors but not the manifest. A session that remembered only the
content root could accept a patch prepared for a different manifest, and could
misclassify a manifest-only patch as a no-op. At the end of Slice 3,
`BundleSession` stored an optional `ArtifactIdentity`, derived its content-root
accessor from it, compared the complete base identity, and committed the
verified target identity.

The subsequent independent cleanup replaced the unpublished save payload
directly and removed the optional internal identity. `BundleSession` and its
schema-v1 save now store one required artifact identity: either the complete
typed `ArcweftBundle` identity (including manifest, source, executable, and
resources) or the complete AWFB identity. There is no root-only variant, legacy
decoder, or duplicate payload-level schema marker. Native endpoint tests prove
that manifest-only patches update generation, source label, active bytes, and
target identity, and that prepared patches cannot cross the equal-root artifact
boundary. A separate logical-bundle restore test proves the same manifest
boundary for sessions created without AWFB bytes.

### Executor and unsafe boundaries

`ArcweftRuntimeExecutor` is now the application-facing facade. Concrete VM,
bytecode VM, AOT, and product-AWBC executor types are `pub(crate)`, and CLI
wiring no longer depends on their construction. The stable runtime design
chapter now describes the facade instead of the removed public concrete shape.

The Cranelift crate and desktop-native crate deny unsafe code by default. The
JIT native-call ABI module and Windows TSF COM module are the only explicit
unsafe boundaries and opt in locally; both also deny undocumented unsafe blocks
and unsafe operations inside unsafe functions.

No Cargo dependency edge changed. The two Cargo manifest edits configure lints,
and the existing dependency graph continues to point through the runtime facade
and bundle/runtime layers. At that Slice 3 cut, public contract changes were
limited to sealing the concrete executor types and introducing a provisional
complete AWFB identity field. The later independent cleanup supersedes that
unpublished payload shape.

### Just workflow cleanup

The root `Justfile` is now 28,020 bytes / 307 lines. Benchmark and profiling
recipes live in imported `just/bench.just` at 30,948 bytes / 313 lines while
retaining their flat recipe names. Redundant aliases and duplicated profiling
commands were removed. Browser benchmark recipes share one build prerequisite,
fixture refresh dependencies match the artifacts they regenerate, and
`verify-full` includes the full CLI check route.

`test-workspace` is the canonical fast workspace route and now includes the
small CLI behavior/fixture integration binaries that had silently fallen out of
the broad exclusion. `test-workspace-profile` measures that recipe instead of
copying its command list. The native exact-test filters now use fully qualified
module paths; this exposed five filters that had previously selected zero tests
and led to removal or repair of their stale assumptions.

### Current checkout measurements

Measurements are from the checkout stack starting at revision `169f8fb1baf8`
and ending at Jujutsu change `xomrxnyw`, using current files rather than diff
additions. Byte and LOC figures include embedded tests; their LOC is stated
separately where present.

| Path | Owner / kind | Bytes | Physical LOC | Embedded test LOC | Major responsibility touched |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-bundle/src/patch.rs` | `arcweft-bundle` / production | 58,709 | 1,609 | 0 | Patch schema, materialization, fingerprints, and verified target boundary |
| `crates/arcweft-cli/src/app/bundle.rs` | `arcweft-cli` / production | 81,641 | 2,275 | 0 | Bundle command orchestration and verified patch target access |
| `crates/arcweft-cli/src/app/runtime/run.rs` | `arcweft-cli` / production | 48,621 | 1,331 | 253 | Runtime command policy and native/headless option routing |
| `crates/arcweft-cli/src/server_adapter.rs` | `arcweft-cli` / production | 13,861 | 428 | 101 | Runtime-host adapter construction through the executor facade |
| `crates/arcweft-core/src/executor.rs` | `arcweft-core` / production | 15,649 | 478 | 0 | Public executor facade and internal execution tiers |
| `crates/arcweft-desktop-native/src/text_input/windows_tsf/unsafe_com.rs` | `arcweft-desktop-native` / production | 31,175 | 965 | 0 | Audited Windows COM/TSF unsafe boundary |
| `crates/arcweft-lang-jit-cranelift/src/native_call.rs` | `arcweft-lang-jit-cranelift` / production | 45,174 | 1,155 | 0 | Audited JIT/native-call ABI unsafe boundary |
| `crates/arcweft-player-native/src/lib.rs` | `arcweft-player-native` / facade | 13,755 | 367 | 184 | Native player facade and status reporting |
| `crates/arcweft-player-native/src/patch_endpoint.rs` | `arcweft-player-native` / production | 39,018 | 1,016 | 519 | Owned AWFB bytes, prepared patch validation, live apply/restart |
| `crates/arcweft-player-native/src/scene_windowed.rs` | `arcweft-player-native` / production | 65,264 | 1,777 | 54 | Windowed scene/input orchestration after wrapper removal |
| `crates/arcweft-player-native/src/windowed_runtime.rs` | `arcweft-player-native` / production | 37,884 | 976 | 305 | Frame-safe resource and patch commit orchestration |
| `crates/arcweft-player-scene/src/fonts.rs` | `arcweft-player-scene` / production | 9,832 | 292 | 143 | Runtime font inventory without source-spelling guards |
| `crates/arcweft-project-loader/src/release_adapter/trust.rs` | `arcweft-project-loader` / production | 22,268 | 644 | 0 | Release trust projection through the verified patch API |
| `crates/arcweft-runtime-driver/src/session.rs` | `arcweft-runtime-driver` / production | 74,195 | 1,930 | 0 | Session state, complete container identity, and hot-swap commit |
| `crates/arcweft-runtime-driver/src/session_save.rs` | `arcweft-runtime-driver` / production | 16,112 | 448 | 0 | Session-save identity snapshot before the subsequent payload simplification |
| `crates/arcweft-runtime-driver/src/swap.rs` | `arcweft-runtime-driver` / production | 32,314 | 972 | 337 | Program-generation classification and unified swap commit |
| `tools/structure-audit.rs` | workspace tool / production | 31,251 | 969 | 0 | Size, ownership, generated metadata, and dependency audit |
| `crates/arcweft-bundle/tests/patch_schema.rs` | `arcweft-bundle` / integration test | 11,939 | 324 | 0 | Patch tamper, duplicate, identity, and compatibility matrix |
| `crates/arcweft-bundle/tests/view_resource_codecs.rs` | `arcweft-bundle` / integration test | 24,594 | 669 | 0 | Typed View resource codec round trips |
| `crates/arcweft-cli/tests/check_core_cli.rs` | `arcweft-cli` / integration test | 539 | 19 | 0 | Direct CLI check behavior only |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | `arcweft-cli` / integration test | 214,665 | 5,848 | 0 | Native sample/effect observation matrix after stale capture tests were removed |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | `arcweft-cli` / integration test | 238,553 | 6,629 | 0 | Vertical/ruby observation matrix after unobservable wrap test removal |
| `crates/arcweft-cli/tests/check/agent_observe_native/selected_capture_metadata.rs` | `arcweft-cli` / integration test | 2,442 | 55 | 0 | Serialized selected-capture metadata contract |
| `crates/arcweft-cli/tests/check/agent_observe_native/visual_smoke.rs` | `arcweft-cli` / integration test | 13,346 | 355 | 0 | Viewport/layer/object/mask/object-id behavior smoke |
| `crates/arcweft-cli/tests/css_style_parity_sample.rs` | `arcweft-cli` / integration test | 2,009 | 58 | 0 | CSS parity sample behavior |
| `crates/arcweft-cli/tests/runtime_native_options.rs` | `arcweft-cli` / integration test | 2,305 | 82 | 0 | Headless/watch session and trace option rejection |
| `crates/arcweft-desktop-native/tests/macos_text_input_compile.rs` | `arcweft-desktop-native` / integration test | 256 | 7 | 0 | macOS plus feature-gated adapter export compilation |
| `crates/arcweft-runtime-driver/tests/awbc_product_session.rs` | `arcweft-runtime-driver` / integration test | 18,133 | 511 | 0 | Product session save/restore identity rejection |
| `crates/arcweft-runtime-driver/tests/session.rs` | `arcweft-runtime-driver` / integration test | 60,713 | 1,673 | 0 | Hot-swap identity, manifest-only, no-op, and generation behavior |
| `crates/arcweft-takumi-adapter/tests/compositing_capture_fixtures.rs` | `arcweft-takumi-adapter` / integration test | 1,517 | 41 | 0 | Complete deterministic compositing evidence |
| `crates/arcweft-takumi-adapter/tests/css_layout_cascade_coverage.rs` | `arcweft-takumi-adapter` / integration test | 6,225 | 170 | 0 | Typed CSS cascade coverage and unsupported diagnostics |

The largest non-generated production files remain unchanged ownership
hotspots:

| Path | Bytes | Physical LOC | Embedded tests |
| --- | ---: | ---: | --- |
| `crates/arcweft-core/src/awbc/product_step.rs` | 95,176 | 2,499 | no |
| `crates/arcweft-core/src/value.rs` | 83,955 | 2,498 | no |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | no |
| `crates/arcweft-runtime-plan/src/flow.rs` | 90,131 | 2,468 | no |
| `crates/arcweft-cli/src/app/bundle_view.rs` | 87,720 | 2,468 | no |

The changed large production files were reviewed rather than split by line
count alone. `patch.rs` is one cohesive deterministic artifact algorithm;
`session.rs` is the state owner whose identity fix crosses construction,
snapshot, and commit; the large CLI/player files shrank or received only narrow
boundary call-site changes. The two large Agent integration files remain
warning-level domain matrices below the 8,000-LOC error threshold. A future
split should follow observation domains, not add pass-through test modules.

The canonical audit now reports:

```text
files scanned: 2482
Rust files: 1149
Rust physical LOC: 583980
package manifests: 91
violations: 0 error(s), 151 warning(s)
```

Generated Rust classification no longer guesses from comments. The audit reads
explicit `.gitattributes` metadata for the two generated text-layout tables.
The old `TYPE001`/`TYPE002` spelling/name rules were removed; size, embedded-test,
manifest, and structured dependency rules remain.

## Slice 3 validation

```text
cargo fmt --all -- --check
  passed
just test-cli-native
  2 visual smoke tests and 1 fully qualified exact test passed
cargo test -p arcweft-runtime-driver --test session --test awbc_product_session
  32 + 12 passed
cargo test -p arcweft-player-native patch_endpoint::tests --all-features
  9 passed
just test-workspace
  passed, including the explicit CLI behavior and fixture integration targets
cargo clippy --workspace --all-targets --all-features
  passed; pre-existing warnings remain, no warning from this slice after the
  focused patch-schema cleanup
cargo clippy -p arcweft-bundle --test patch_schema --all-features
  passed without warnings
npm.cmd --prefix web run test:ime
  passed; browser run reported the explicit unavailable-WebGPU-adapter block
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/source-gate-cleanup
  0 error(s); 151 warning(s)
just --unstable --fmt --justfile Justfile
just --unstable --fmt --justfile just/bench.just
just --summary
just --dry-run test-workspace
just --dry-run test-cli-native
just --dry-run verify-full
  all parsed and resolved
active CI/Cargo/Just/crate/tool/web search for deleted source-gate names
  no matches
git diff --check
  passed
```

The first `just test-workspace` attempt was terminated by its external ten-minute
command limit and is not counted as validation. Re-running the identical recipe
with the warmed build cache and a sufficient limit completed successfully.

## Remaining TODOs after Slice 3

- Replace the repeated `DataFormat` semantic registry, dialogue ID
  normalization, and evaluator index conversion noted above with owning typed
  APIs in separate reviewable slices.
- Design a sealed, typed product-resource codec inventory before adding another
  resource family. The removed source scans must not be recreated as the
  registry mechanism.
- Run the macOS text-input compile test on an actual macOS target with
  `macos-text-input`; the current Windows checkout can validate cfg and lint
  shape but the platform-validation workflow remains intentionally disabled.
- Decompose existing warning-level production and Agent integration hotspots
  only along cohesive responsibility boundaries. No error-level structural
  violation remains in this slice.

There are no known design deviations in this slice. No compatibility promise
existed for the unpublished session-save payload; the subsequent cleanup
therefore established the corrected identity-bound shape as schema v1 instead
of carrying a migration story for an unused defective format.

## Slice 4: independent ownership and provisional-contract cleanup

The Slice 3 follow-ups are complete. `DataFormat::ALL` now owns the language
inventory; dialogue identity normalization is shared by HIR and ID-context;
runtime collection conversion is owned by `RuntimeValue`/`RuntimeSeq`; and
`ProductSectionCodecKind::ALL` contains only implemented compact codecs.
`docs/implementation/independent-cleanup-inventory-2026-07-10.md` records the
breaking changes and ranks the next independent slices.

Session identity was tightened beyond the Slice 3 AWFB fix. The bundle crate
now exposes `logical_identity::LogicalBundleIdentity`, which hashes the complete
validated typed bundle through its deterministic codec. `BundleSession` always
stores either that complete logical identity or a complete AWFB
`ArtifactIdentity`; there is no optional or root-only state. The schema-v1 save
payload uses the same required enum and omits duplicate schema/quiescence
markers and speculative executor variants.

The broad workspace route also exposed a test-fixture race. Two parallel
release tests could create the same timestamp-derived directory and one test's
explicit cleanup could delete another test's staged publication. The fixture
now uses the shared atomic-sequence `TempDir` and RAII cleanup. This is a test
infrastructure correction, not a production publication compatibility change.

### Slice 4 structural measurements

Measurements are from base revision `fce823c7ab8b` and Jujutsu working change
`lzuoprwr`, using current file sizes from the canonical audit rather than diff
additions. Embedded-test LOC runs from each file's `#[cfg(test)]` module through
EOF; the affected files each have one such trailing module.

| Path | Owner / kind | Bytes | Physical LOC | Embedded test LOC | Major responsibility |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-bundle/src/lib.rs` | `arcweft-bundle` / facade | 71,520 | 2,006 | 620 | Existing bundle facade; only the logical-identity module boundary changed |
| `crates/arcweft-bundle/src/logical_identity.rs` | `arcweft-bundle` / production | 1,221 | 31 | 0 | Complete deterministic typed-bundle identity |
| `crates/arcweft-bundle/src/resource_codec/kind.rs` | `arcweft-bundle` / production | 7,519 | 196 | 0 | Implemented compact-codec inventory and section mapping |
| `crates/arcweft-bundle/src/resource_codec.rs` | `arcweft-bundle` / facade | 3,745 | 73 | 0 | Resource-codec public surface after migration-status removal |
| `crates/arcweft-core/src/engine/eval.rs` | `arcweft-core` / production | 57,090 | 1,516 | 0 | Engine evaluator call sites using owned collection conversion |
| `crates/arcweft-core/src/pure.rs` | `arcweft-core` / production | 74,447 | 2,097 | 0 | Pure evaluator call sites using owned collection conversion |
| `crates/arcweft-core/src/value/sequence_impls.rs` | `arcweft-core` / production | 52,736 | 1,580 | 0 | Runtime collection length/index rules and sequence implementations |
| `crates/arcweft-data/src/codec.rs` | `arcweft-data` / production | 10,408 | 370 | 0 | Authoritative DataFormat metadata and lookup |
| `crates/arcweft-lang-hir/src/dialogue_identity.rs` | `arcweft-lang-hir` / production | 5,900 | 182 | 65 | Typed dialogue families, speaker slug, and generated text-key rule |
| `crates/arcweft-lang-hir/src/id_context.rs` | `arcweft-lang-hir` / production | 31,214 | 960 | 119 | AST-gated source materialization using shared identity rules |
| `crates/arcweft-lang-hir/src/lib.rs` | `arcweft-lang-hir` / facade | 760 | 23 | 0 | Private dialogue-identity module declaration |
| `crates/arcweft-lang-hir/src/lower.rs` | `arcweft-lang-hir` / production | 10,539 | 319 | 134 | HIR orchestration and wrong-family behavior tests |
| `crates/arcweft-lang-hir/src/lower_dialogue.rs` | `arcweft-lang-hir` / production | 3,774 | 91 | 0 | Dialogue lowering through typed speaker identity |
| `crates/arcweft-lang-hir/src/lower_ids.rs` | `arcweft-lang-hir` / production | 12,523 | 362 | 0 | Relative/absolute dialogue ID construction and validation |
| `crates/arcweft-lang-sema/src/env.rs` | `arcweft-lang-sema` / production | 39,921 | 1,203 | 41 | Builtin registration from DataFormat inventory |
| `crates/arcweft-runtime-accelerator/src/external.rs` | `arcweft-runtime-accelerator` / production | 49,385 | 1,255 | 0 | Runtime DataFormat argument parsing through owning lookup |
| `crates/arcweft-runtime-driver/src/session.rs` | `arcweft-runtime-driver` / production | 73,593 | 1,929 | 0 | Required logical/AWFB session identity and restore boundary |
| `crates/arcweft-runtime-driver/src/session_save.rs` | `arcweft-runtime-driver` / production | 14,991 | 403 | 0 | Minimal schema-v1 quiescent session payload |

Changed integration/unit test files remain below their applicable error
thresholds: `resource_codec_common.rs` is 16,422 bytes / 529 LOC,
`arcweft-core/src/tests/value.rs` is 30,981 bytes / 868 LOC,
`data_format.rs` is 1,113 bytes / 34 LOC, sema `typecheck.rs` is 121,562 bytes /
4,005 LOC, `awbc_product_session.rs` is 18,615 bytes / 529 LOC,
`release_trust_json.rs` is 7,017 bytes / 203 LOC, and its shared fixture is
11,559 bytes / 322 LOC.

The only Cargo edge added is the lower-layer normal dependency
`arcweft-lang-sema -> arcweft-data`. Sema has seven normal direct outgoing
dependencies after the change and eight incoming workspace edges when two
development edges are included (six normal plus two development). The edge
does not create a cycle and preserves `syntax -> HIR -> sema` while reusing the
independent Sans I/O data contract.

The first Slice 4 audit reported one size error because adding collection
methods directly to the pre-existing 2,498-LOC `value.rs` raised it to 2,515
LOC. The methods remain inherent `RuntimeValue` APIs but their implementation
now lives in the existing sequence responsibility module; `value.rs` is back to
83,955 bytes / 2,498 LOC. The final canonical audit reports:

```text
files scanned: 2486
Rust files: 1152
Rust physical LOC: 584369
package manifests: 91
violations: 0 error(s), 151 warning(s)
```

The warning-level large files were audited rather than split mechanically.
`sequence_impls.rs` remains a cohesive collection implementation module;
`eval.rs`, `pure.rs`, `env.rs`, and `external.rs` only lost duplicated logic or
received narrow inventory call sites. The 2,006-LOC bundle `lib.rs` remains a
real facade/test hotspot, so the new identity implementation was placed in a
31-LOC responsibility module rather than added to the facade body.

### Slice 4 validation

```text
cargo fmt --all -- --check
  passed
focused DataFormat, HIR/tooling, core evaluator, resource-codec, and session
tests listed in independent-cleanup-inventory-2026-07-10.md
  passed
cargo test -p arcweft-cli --test release_trust_json -- --nocapture
  5 passed in parallel after collision-safe TempDir migration; the complete
  binary then passed five consecutive runs (25 test executions)
just test-workspace
  passed on the final checkout
cargo clippy --workspace --all-targets --all-features
  passed; existing syntax enum-size, sema size, float-comparison, clipboard,
  Option<Option>, and web-input warnings remain; no warning is introduced here
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write target/structure-audit/independent-cleanup-2026-07-10
  0 error(s); 151 warning(s)
git diff --check
  passed
```

The first broad `just test-workspace` attempt reached its external command
limit during compilation and is not counted. A warmed attempt exposed the
release fixture collision described above. After the fixture correction, the
final complete workspace route passed.
