# Environment product source contract correction

- Date: 2026-07-18
- Package:
  `arcweft-seq-06.11d.4.2.3-environment-product-source-contract-correction-final-contract.zip`
- Package SHA-256:
  `df2490c69d67151f12c908fc259e9ca7e9fb8e7c4485d747fe545282e69ec874`
- Package basis: Git `9a63ac5512cd75947ba70195681e43ab968f9f12`
- Implementation change: Jujutsu change `uutzwzrwlnry`
- Rebased parent: Git `77e38b3337f9cd15e2ce1b0f0bbe0f884478e5bf`
- Status: package implementation, focused validation, workspace-wide gates,
  generated-fixture refresh, and canonical structural audit complete

## Implemented contract

The package's environment source contract is implemented end to end:

- syntax and HIR environments now retain exact predicate, body, and scope
  ranges, including parser recovery from the delimiters actually owned by each
  environment;
- semantic checking produces a typed outer-to-inner wrapper inventory,
  canonical clauses, and explicit clause ownership;
- compiler lowering projects each wrapper's predicate/body/scope source plus
  its owned clauses instead of collapsing the environment to one aggregate
  range;
- the View model and codecs use the replacement wire shape with a typed wrapper
  index and exact per-wrapper source references;
- complete-product validation proves the same-source local interval graph,
  predicate-before-body ordering, clause ownership, nested scope containment,
  and rule placement in the innermost body;
- Style source accounting covers wrapper, clause, rule, declaration, token,
  rule-declaration, and patch-declaration references, with the package's exact
  wrapper and aggregate relation limits;
- `ValidatedViewProduct` performs the atomic program/style/source validation
  boundary, canonical candidate checks, and repeated validation before making
  either validated resource available;
- `ViewProgramStyleResources::merge_with_budget` performs immutable local
  preflight, checked dense-ID offsets, combined inventory limits, and an
  unpublished canonical encode/decode transaction before returning a merged
  candidate;
- canonical View transcript encoding counts JSON bytes through a bounded
  counting writer before allocating the transcript payload;
- runtime construction accepts only a validated product, and replacement
  distinguishes a real `StyleProgramChanged` result from source-only or
  provenance-only changes;
- standard dialogue Style now contributes its generated source document to the
  bundle source union without changing the authored primary-source display;
  and
- bundle construction is fallible through `ArcweftBundle::try_new`, so source
  count overflow and reserved-ID collision are typed errors rather than
  reachable panics.

`ValidatedViewStyleResource::has_same_runtime_semantics` intentionally compares
the executable Style contract while ignoring source IDs, ranges, and wrapper
provenance. All other Style semantics remain part of the comparison.

## Verification cases authored

The changed tests cover:

- exact predicate/body/scope range propagation through syntax, HIR, semantic
  checking, tooling, compiler lowering, and product construction;
- recovered delimiters and nested outer/inner wrapper ownership;
- canonical JSON, CBOR, and MessagePack round trips;
- independent outer and inner predicate/body/scope wire tampering across all
  three codecs;
- cross-source wrapper, clause, rule, and declaration rejection with exact
  typed source roles;
- local interval-graph containment and ordering failures;
- wrapper, relation, source-count, and reserved-source-ID limits;
- noncanonical in-memory Style candidate rejection;
- atomic complete-product validation and publication;
- standard dialogue generated-source inclusion while retaining authored
  primary-source identity; and
- runtime acceptance of source-only replacements without generation/frame
  invalidation, plus rejection of executable Style changes.
- merge source-ID, patch-ID, instruction-span, and public-table overflow without
  large allocations;
- exact and one-over wrapper, clause, source-range, and transcript merge
  budgets;
- malformed predicate/scope, nested scope/body, and guarded-rule/body
  preflight on either input;
- canonical merged-candidate rejection, public owner collision rollback, and
  complete-product rejection when the merged Style references a right-hand
  document absent from the caller-owned SourceMap.

## Integration and overlap decisions

The standard dialogue source is engine generated, so the bundle constructor
unions its source document with the authored source map through the same
fallible source-map builder. This preserves the authored document as the
primary diagnostic/display source while ensuring every generated Style source
reference resolves.

The change was first rebased from `9a63ac55` onto `69dc5152`, which includes
Seq-06.11d.4.2.2 and .2.2.1/.2.2.2. The two textual conflicts were resolved by:

- retaining the Lang-01.2 selected-entry Agent compilation pipeline and changing
  only its bundle construction to `ArcweftBundle::try_new`; and
- retaining the split runtime-driver swap test module, with its bundle
  constructor migration applied in `swap/tests.rs`.

The auto-merged Style parser retains the .2.2 typed `ParseErrorKind` and
owner-local `RecoverySuggestion` model together with this package's exact
predicate/body/scope delimiter-derived ranges. No removed string diagnostic
owner was restored.

The resulting change was then rebased without conflict onto `77e38b33`, which
contains the Proof capability-policy correction. This package does not modify
the Proof implementation files, so the generic diagnostics and delete-and-
derive policy from that parent remain authoritative and unchanged.

No compatibility reader, deprecated field, historical AST/CST kind, source
gate, or stringly conversion shim was added.

## Executable validation status

Focused commands were run with `CARGO_BUILD_JOBS=2`.
After the final rebase onto `77e38b33`, bundle check, bundle Clippy, the
runtime-driver exact target, and formatting were rerun. The remaining focused
test results below were obtained before that conflict-free rebase; none of
their implementation files overlapped the Proof capability-policy parent
change.

Passed:

```bash
cargo test -p arcweft-bundle --lib --no-run
cargo test -p arcweft-bundle --lib resource_codec::view::merge::tests
# 3 passed
cargo test -p arcweft-bundle --all-features --test view_style_environment_codec
# 19 passed
cargo test -p arcweft-bundle --all-features --test view_resource_codecs view_resource_merge
# 2 passed
cargo test -p arcweft-bundle --test view_style_program
# 6 passed
cargo check -p arcweft-bundle --all-targets --all-features
cargo clippy -p arcweft-bundle --all-targets --all-features -- -D warnings
cargo test -p arcweft-lang-syntax --test style_environment
# 10 passed
cargo test -p arcweft-runtime-driver --test root_command_dispatch
# 28 passed
cargo fmt --all -- --check
```

The first all-features `view_style_program` link attempt ended with transient
Windows `link.exe` status `0xc000026b`; the same exact test target subsequently
linked and all six tests passed. Initial focused compilation also exposed and
fixed an ambiguous `Result` error type in complete-product promotion. Focused
Clippy exposed package-owned warnings; these were fixed without changing the
wire contract, including a narrowly documented `struct_field_names` exemption
for the required `predicate_source`/`body_source`/`scope_source` field names.

The runtime-driver exact test had timed out twice during its earlier cold
dependency build. After rebasing onto `77e38b33`, the warmed target completed:
all 28 `root_command_dispatch` tests passed.

The final integration checkout also passed:

```bash
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-player-web --test parity
# 7 passed
just test-workspace
# exit 0; all tests passed
cargo +nightly -Zscript tools/structure-audit.rs --root .
# exit 0; 0 errors, 132 warnings
cargo fmt --all -- --check
```

The first `just test-workspace` run exposed two `arcweft-player-web` parity
failures because the checked-in `web/demo.awfb` predated the standard dialogue
Style source-map union. The fixture was regenerated through its canonical
checked-in recipe:

```bash
just fixture-refresh-web-demo-awfb
```

That command rebuilds the fixture from `web/arcw.toml` profile `main`; it is
also recorded in `docs/implementation/fixture-regeneration.md`. The full parity
target then passed all seven tests, and the subsequent `just test-workspace`
run passed in 754.076 seconds. No read-time repair, compatibility decoder, or
fixture-specific runtime branch was added.

## Static structural audit

The final canonical audit was run against the integration checkout and
completed in 16.744 seconds:

- 3,252 files;
- 1,663 Rust files;
- 762,483 physical Rust lines;
- 92 Cargo manifests;
- 0 errors; and
- 132 warnings.

No Cargo manifest changed. The only changed file newly crossing a configured
size warning was the integration test
`crates/arcweft-runtime-driver/tests/view_runtime.rs`, which grew from 2,452 to
2,537 physical lines. It is above the 2,500-line integration-test warning
threshold but far below the 8,000-line error threshold. The added cases exercise
one coherent View-product replacement matrix; no production ownership boundary
or dependency edge was added.

The detailed inventory below was first taken before rebase, from Jujutsu change
`uutzwzrwlnry` against Git `9a63ac5512`, and remains the package-scope
responsibility snapshot.

Responsibilities remain aligned with the existing layer boundaries:

- syntax, HIR, semantic checking, compiler projection, and tooling own their
  respective environment representations;
- `arcweft-view` owns the executable Style model and semantic comparison;
- `arcweft-bundle` owns source maps, canonical codecs, complete-product
  validation, and standard generated resources;
- runtime crates consume only validated products and adapt replacement state;
  and
- player/runner/CLI changes are construction-edge and fixture adaptations.

The following post-rebase measurements were taken from Jujutsu change
`uutzwzrwlnry` after the MERGE-06 through MERGE-14 implementation and focused
validation:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-bundle/src/resource_codec/view/merge.rs` | 36,677 | 986 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/codec/transcript.rs` | 5,894 | 185 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/codec/style.rs` | 20,739 | 525 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | 22,549 | 576 | Production |
| `crates/arcweft-view/src/style/environment.rs` | 17,518 | 504 | Production |
| `crates/arcweft-bundle/tests/view_style_environment_codec.rs` | 47,524 | 1,216 | Integration test |

`merge.rs` grew by more than 300 physical lines in this coherent change, which
triggered the decomposition review. At 986 physical lines it remains below the
1,200-line production warning threshold and owns one cohesive responsibility:
immutable merge inventory, checked offset/budget preflight, candidate
construction, and atomic canonical validation. Its behavioral matrix is kept in
the integration test rather than embedded in the production module. The
environment integration test remains below the 2,500-line integration-test
warning threshold. No Cargo dependency, public crate boundary, or source gate
was added.

Exact metrics for every changed Rust file follow. `Production` includes
ordinary responsibility modules, `Facade` denotes a small public module root,
`Unit test` denotes test code under `src`, `Integration test` denotes a test
target, and `Build tool` denotes the standalone fixture generator.

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-agent-runner/src/tests.rs` | 76,370 | 2,119 | Unit test |
| `crates/arcweft-bundle/src/lib.rs` | 80,114 | 2,211 | Production/facade |
| `crates/arcweft-bundle/src/product/source_projection.rs` | 679 | 22 | Production |
| `crates/arcweft-bundle/src/product.rs` | 42,865 | 1,178 | Production |
| `crates/arcweft-bundle/src/resource_codec/source_map/model.rs` | 12,326 | 359 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/codec/style.rs` | 20,296 | 537 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/codec/style_environment.rs` | 10,328 | 306 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/codec.rs` | 54,059 | 1,496 | Production |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | 22,289 | 614 | Production |
| `crates/arcweft-bundle/src/resource_codec/view.rs` | 1,290 | 33 | Facade |
| `crates/arcweft-bundle/src/resource_codec.rs` | 4,430 | 86 | Facade |
| `crates/arcweft-bundle/src/standard_view.rs` | 13,496 | 358 | Production |
| `crates/arcweft-bundle/tests/product_awbc_only.rs` | 6,155 | 173 | Integration test |
| `crates/arcweft-bundle/tests/product_catalog_resource_codecs.rs` | 13,293 | 362 | Integration test |
| `crates/arcweft-bundle/tests/standard_dialogue_view.rs` | 9,162 | 262 | Integration test |
| `crates/arcweft-bundle/tests/style_cross_section_refs.rs` | 12,487 | 349 | Integration test |
| `crates/arcweft-bundle/tests/view_product_validation.rs` | 14,247 | 407 | Integration test |
| `crates/arcweft-bundle/tests/view_resource_codecs.rs` | 59,078 | 1,602 | Integration test |
| `crates/arcweft-bundle/tests/view_style_environment_codec.rs` | 40,291 | 1,093 | Integration test |
| `crates/arcweft-cli/src/app/bundle/tests/view_part_recovery.rs` | 2,909 | 91 | Unit test |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 82,583 | 2,566 | Unit test |
| `crates/arcweft-cli/src/app/bundle.rs` | 72,089 | 2,027 | Production |
| `crates/arcweft-compiler/src/agent.rs` | 11,092 | 270 | Production |
| `crates/arcweft-compiler/src/style.rs` | 30,680 | 834 | Production |
| `crates/arcweft-compiler/tests/style.rs` | 14,241 | 394 | Integration test |
| `crates/arcweft-lang-hir/src/style.rs` | 22,078 | 777 | Production |
| `crates/arcweft-lang-hir/tests/style_environment.rs` | 4,447 | 128 | Integration test |
| `crates/arcweft-lang-sema/src/style/catalog.rs` | 9,494 | 377 | Production |
| `crates/arcweft-lang-sema/src/style/check.rs` | 34,364 | 930 | Production |
| `crates/arcweft-lang-sema/src/style.rs` | 561 | 16 | Facade |
| `crates/arcweft-lang-sema/tests/style_environment.rs` | 10,528 | 305 | Integration test |
| `crates/arcweft-lang-syntax/src/ast/style.rs` | 15,619 | 670 | Production |
| `crates/arcweft-lang-syntax/src/parser/style.rs` | 50,445 | 1,393 | Production |
| `crates/arcweft-lang-syntax/tests/style_environment.rs` | 10,185 | 308 | Integration test |
| `crates/arcweft-player-native/src/dev_capture.rs` | 19,534 | 530 | Production |
| `crates/arcweft-player-native/src/lib.rs` | 14,644 | 386 | Production/facade |
| `crates/arcweft-player-native/src/main.rs` | 12,921 | 382 | Production/facade |
| `crates/arcweft-player-native/src/patch_endpoint.rs` | 39,602 | 1,028 | Production |
| `crates/arcweft-player-native/src/windowed_runtime.rs` | 37,599 | 992 | Production |
| `crates/arcweft-player-native/tests/support/windowed_live_patch_fixtures.rs` | 49,462 | 1,401 | Integration-test support |
| `crates/arcweft-player-native/tests/windowed_ingress_runtime.rs` | 8,822 | 239 | Integration test |
| `crates/arcweft-player-scene/src/frame/view_style/tests.rs` | 45,974 | 1,298 | Unit test |
| `crates/arcweft-player-scene/tests/dialogue_view.rs` | 8,141 | 217 | Integration test |
| `crates/arcweft-player-web/tests/parity.rs` | 36,835 | 1,070 | Integration test |
| `crates/arcweft-project-loader/src/cache/release.rs` | 58,491 | 1,550 | Production |
| `crates/arcweft-runtime-driver/src/session/construction.rs` | 17,426 | 461 | Production |
| `crates/arcweft-runtime-driver/src/swap.rs` | 32,715 | 980 | Production |
| `crates/arcweft-runtime-driver/src/view_runtime/axis_seed_tests.rs` | 31,521 | 870 | Unit test |
| `crates/arcweft-runtime-driver/src/view_runtime/replacement.rs` | 14,585 | 400 | Production |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 45,868 | 1,201 | Production |
| `crates/arcweft-runtime-driver/tests/awbc_product_session.rs` | 28,509 | 802 | Integration test |
| `crates/arcweft-runtime-driver/tests/session.rs` | 106,866 | 2,943 | Integration test |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 91,362 | 2,540 | Integration test |
| `crates/arcweft-runtime-host/src/bundle_runner.rs` | 40,659 | 1,178 | Production |
| `crates/arcweft-runtime-host/tests/bundle_runner.rs` | 10,639 | 313 | Integration test |
| `crates/arcweft-tooling/src/style_environment.rs` | 34,382 | 950 | Production |
| `crates/arcweft-view/src/lib.rs` | 8,024 | 170 | Facade |
| `crates/arcweft-view/src/style/environment.rs` | 17,345 | 545 | Production |
| `crates/arcweft-view/src/style.rs` | 6,589 | 193 | Facade |
| `crates/arcweft-view/tests/computed_style.rs` | 30,456 | 989 | Integration test |
| `crates/arcweft-view/tests/logical_axis_provider.rs` | 62,015 | 2,117 | Integration test |
| `crates/arcweft-view/tests/style_environment.rs` | 19,179 | 611 | Integration test |
| `tools/build-web-ime-player-rendered-fixture.rs` | 11,852 | 311 | Build tool |

Exact embedded `#[cfg(test)]` ownership is:

| Path | Test-guarded physical LOC |
| --- | ---: |
| `crates/arcweft-bundle/src/lib.rs` | 733 |
| `crates/arcweft-bundle/src/product.rs` | 436 |
| `crates/arcweft-bundle/src/resource_codec/source_map/model.rs` | 38 |
| `crates/arcweft-bundle/src/resource_codec/view/codec/style_environment.rs` | 25 |
| `crates/arcweft-cli/src/app/bundle.rs` | 96 |
| `crates/arcweft-lang-hir/src/style.rs` | 28 |
| `crates/arcweft-player-native/src/dev_capture.rs` | 174 |
| `crates/arcweft-player-native/src/lib.rs` | 190 |
| `crates/arcweft-player-native/src/main.rs` | 179 |
| `crates/arcweft-player-native/src/patch_endpoint.rs` | 531 |
| `crates/arcweft-player-native/src/windowed_runtime.rs` | 318 |
| `crates/arcweft-project-loader/src/cache/release.rs` | 767 |
| `crates/arcweft-runtime-driver/src/swap.rs` | 345 |
| `crates/arcweft-runtime-driver/src/view_runtime/replacement.rs` | 24 |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 2 (external module declaration) |
| `crates/arcweft-runtime-host/src/bundle_runner.rs` | 283 |
| `crates/arcweft-view/src/lib.rs` | 2 (external module declaration) |

The largest current workspace Rust files, excluding `target`, VCS internals,
vendored source, and historical documentation, are:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | Generated lookup data |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 256,505 | 7,970 | Integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,805 | 6,620 | Integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | Integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,731 | 5,850 | Integration test |
| `crates/arcweft-compiler/src/tests.rs` | 180,052 | 5,363 | Unit test |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,821 | 5,249 | Integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,206 | 4,177 | Integration test |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 127,110 | 4,175 | Unit test |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 110,752 | 3,181 | Unit test |
| `crates/arcweft-runtime-driver/tests/session.rs` | 106,866 | 2,943 | Integration test |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 82,583 | 2,566 | Unit test |
| `crates/arcweft-core/src/tests/flow.rs` | 88,953 | 2,553 | Unit test |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 91,362 | 2,540 | Integration test |
| `crates/arcweft-lsp/src/session/tests.rs` | 85,047 | 2,524 | Unit test |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | 76,729 | 2,521 | Integration test |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | Production |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 95,235 | 2,492 | Production |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | Production |
| `crates/arcweft-runtime-accelerator/src/tests.rs` | 98,746 | 2,465 | Unit test |

Changed production hotspots above the repository warning threshold are bundle
`lib.rs` (2,211 LOC), bundle View codec (1,496), CLI bundle orchestration
(2,027), Style parser (1,393), project-loader release cache (1,550), and
runtime View orchestration (1,201). They already own the adapted boundary; this
slice keeps substantive new interval validation in the 545-line
`arcweft-view::style::environment` module and the 614-line bundle validated
product module. Splitting the legacy orchestration files here would mix a
separate architectural refactor into the contract correction. The final
canonical audit confirmed this assessment with no structural errors.

## Completion boundary

Changed files against Git `77e38b3337f9` are 71 total: 69 Rust files, this
implementation note, and the deterministically regenerated `web/demo.awfb`.
The implementation introduces no Cargo dependency or stable design-document
change.

There is no remaining implementation or verification TODO within
Seq-06.11d.4.2.3. MERGE-06 through MERGE-14 are complete, there is no known
design deviation, and no follow-up design request is required. Review,
integration commit, and push are owned by the parent cut point.
