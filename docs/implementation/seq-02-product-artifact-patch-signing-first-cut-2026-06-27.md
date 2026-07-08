# Seq-02 Product Artifact / Patch / Signing First Cut

Package: `D:/sanze/Downloads/arcweft-seq-02-product-artifact-patch-signing-2026-06-25.zip`

This note records the implementation-ready first cut extracted from the seq-02
package. The package status marks the full request as not implementation-ready:
the overlay modules and focused codec tests for the complete artifact / patch /
signing sequence are not materialized, and the package explicitly calls out
unknown optional AWFB section preservation and artifact identity as the first
safe implementation cut.

## Implemented

- AWFB section descriptors now retain unknown optional section-kind codes instead
  of dropping those sections during parse.
- Known section-kind APIs remain available through `known_kind()` and
  `kind_code()` so code that needs typed sections can ignore opaque optional
  sections without panicking.
- Embedded and external unknown optional sections preserve their section id,
  raw kind code, schema version, residency, placement, compression, sizes, and
  content digest.
- Unknown required section kinds still fail with
  `ContainerError::UnknownRequiredSectionKind`.
- AWFB content roots now include the raw section-kind code, so unknown optional
  sections participate in deterministic content identity.
- `ArtifactIdentity` records the container version, bundle kind, content root,
  and manifest digest. `BundleView::artifact_identity()` exposes the current
  AWFB identity, and manifest-only changes produce a different artifact identity
  while leaving the content root unchanged.
- Patch materialization paths preserve raw section-kind codes when carrying
  section descriptors through patch payloads.
- Native player AWFB test fixtures now lower product AWBC from the same
  `RuntimePlan` used for their legacy structured bytecode fixture, so workspace
  smoke tests no longer fail before reaching the remaining web demo fixture gap.
- The web demo fixture `web/demo.awfb` has been regenerated as a binary AWFB
  with product AWBC from the current `web/demo.arcw` source.
- Web parity frame preparation now advances the opening dialogue line through
  `BundleSession::queue_dialogue_advance()` before comparing the two-choice,
  four-image frame, keeping concrete runtime input event construction inside
  `arcweft-runtime-driver`.
- The release-cache product bundle fixture now carries a minimal product AWBC
  executable so cached product fetch/decode tests exercise the current product
  bundle contract.
- Runtime-driver session and hot-swap fixtures now lower product AWBC from the
  same `RuntimePlan` used by their structured bytecode fixture. Tests that
  intentionally verify structured bytecode rejection explicitly select the
  structured executor path.
- Product AWBC hot-swap identity is tracked per function rather than as one
  whole-program code slot, so code-body changes can be classified separately
  from function interface/layout changes.
- Runtime-host bundle runner fixtures now carry product AWBC for product runner
  execution and AWFB encoding paths, while bytecode verification tests keep a
  structured-only fixture and executor selection.
- Product AWBC lowering now marks dynamic-goto functions with
  `HAS_DYNAMIC_TARGET`, emits `FlowEvent::Goto` for AWBC static/dynamic goto
  terminators, and lowers stream `for next` item bindings without leaving
  uninitialized product registers.

## Non-Goals For This Cut

- No new concrete seq-02 section families were added.
- No content/presentation, shader/View, entity, runtime-types, entrypoints, or
  adapter-requirements codecs were implemented.
- No common resource wire codec was introduced.
- No patch v2 format, AWFR release archive, external payload-carrier redesign,
  or signing-policy redesign was implemented.
- No generated overlay from `apply_seq_02.py` was applied as a production patch.

The follow-up design packages remain responsible for those larger contract
surfaces. This cut intentionally avoids treating the broad seq-02 request as
complete.

## Follow-Up Design Requests

The remaining seq-02 product artifact surfaces are split into
sequence-preserving requests so they are not accidentally deferred to seq-03:

- `docs/reviews/requests/2026-06-27-seq-02.1-common-resource-wire-codec-and-resource-codec-plan.md`
- `docs/reviews/requests/2026-06-27-seq-02.2-runtime-types-entrypoints-adapter-requirements-codecs.md`
- `docs/reviews/requests/2026-06-27-seq-02.3-content-presentation-entity-resource-codecs.md`
- `docs/reviews/requests/2026-06-27-seq-02.4-shader-ui-audio-debug-contract-resource-codecs.md`
- `docs/reviews/requests/2026-06-27-seq-02.5-patch-v2-compatibility-and-materialization.md`
- `docs/reviews/requests/2026-06-27-seq-02.6-awfr-release-archive-and-external-payload-carrier.md`
- `docs/reviews/requests/2026-06-27-seq-02.7-signing-policy-redesign.md`
- `docs/reviews/requests/2026-06-27-seq-02.8-overlay-production-application.md`

## Verification

- `uv run .tmp\apply_seq_02.py --self-test`
- `cargo check -p arcweft-bundle --all-targets --all-features`
- `cargo test -p arcweft-bundle awfb_v1_retains_unknown_optional --all-features -- --nocapture`
- `cargo test -p arcweft-bundle artifact_identity_changes_for_manifest_only_delta --all-features -- --nocapture`
- `cargo test -p arcweft-bundle --all-features`
- `cargo test -p arcweft-player-native --all-features`
- `cargo fmt --all -- --check`
- `cargo check -p arcweft-core -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-project-loader -p arcweft-cli -p arcweft-player-native --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo run -p arcweft-cli --quiet -- bundle web/demo.arcw --output target/codex-web-demo/demo.awfb`
- `cargo run -p arcweft-cli --quiet -- inspect target/codex-web-demo/demo.awfb --json`
- `cargo test -p arcweft-player-web --test parity --all-features`
- `cargo test -p arcweft-project-loader fetch_release_product_bundle_decodes_cached_awfb_product --all-features -- --nocapture`
- `cargo check -p arcweft-player-web -p arcweft-project-loader --all-targets --all-features`
- `cargo clippy -p arcweft-player-web -p arcweft-project-loader --all-targets --all-features -- -D warnings`
- `cargo check -p arcweft-player-web -p arcweft-runtime-driver --all-targets --all-features`
- `cargo clippy -p arcweft-player-web -p arcweft-runtime-driver --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-runtime-driver --all-features`
- `cargo test -p arcweft-runtime-host --all-features`
- `cargo test -p arcweft-runtime-host --test bundle_runner --all-features -- --nocapture`
- `cargo test -p arcweft-runtime-plan awbc_product_parity_dynamic_goto --all-features -- --nocapture`
- `cargo test -p arcweft-runtime-plan awbc_product_parity_stream_for_next_binds_source_item --all-features -- --nocapture`
- `cargo test -p arcweft-runtime-plan runtime_plan_lowers_stream_and_source_plans_separately_from_flow_ops --all-features -- --nocapture`
- `cargo test -p arcweft-cli --test regression_harness source_tree_does_not_reintroduce_removed_whitespace_command_dsl_or_shims --all-features -- --nocapture`
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run current_run_fixtures_pass --all-features -- --nocapture`
- `cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-core -p arcweft-runtime-plan -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features -- -D warnings`
- `just test-workspace`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  - files scanned: 1522
  - Rust files: 838
  - Rust physical LOC: 413244
  - package manifests: 89
  - violations: 0 errors, 105 warnings
- `git diff --check`

`just test-workspace` now passes through the former web parity gap, the
runtime-driver session/hot-swap fixture gap, the runtime-host bundle runner
fixture gap, and the current run fixture product AWBC verification path.

## Structural Audit Notes

Repository state measured at Jujutsu change `lokxmrmv`.

Follow-up web fixture regeneration was measured at Jujutsu change `ysnymssu`.

Runtime session, host, and AWBC lowering closure was measured at Jujutsu change
`usuyrkqm`.

| Path | Bytes | LOC | Kind | Embedded test LOC | Responsibilities |
| --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-core/src/awbc/vm.rs` | 44788 | 1189 | production | 0 | compact AWBC VM instruction and terminator execution, host observations, goto observation emission |
| `crates/arcweft-core/src/awbc/product_step.rs` | 91610 | 2322 | production plus minimal test module | 2 | product AWBC runtime stepping, event projection, source/stream state handling, native host request projection |
| `crates/arcweft-core/src/awbc/parity.rs` | 8255 | 240 | production | 0 | AWBC/structured parity event normalization and VM observation mapping |
| `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | 48354 | 1219 | production | 0 | runtime flow to AWBC lowering, flow frame construction, dynamic-goto flag propagation |
| `crates/arcweft-runtime-plan/src/awbc_lower/source.rs` | 18630 | 460 | production | 0 | source/stream plan lowering, stream source parameter inference, source handler AWBC functions |
| `crates/arcweft-runtime-plan/tests/awbc_product_parity.rs` | 59106 | 1683 | integration test | 0 | structured/product AWBC parity fixtures for control, source, stream, audio, await, and expression behavior |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | 46520 | 1359 | integration test | 0 | parser/HIR/runtime-plan lowering coverage, including source handler binding preservation |
| `crates/arcweft-runtime-driver/src/swap.rs` | 29447 | 814 | production plus unit tests | 310 | bundle generation construction, product AWBC per-function code identity, hot-swap compatibility classification |
| `crates/arcweft-runtime-driver/tests/session.rs` | 23866 | 615 | integration test | 0 | bundle session stepping, product AWBC session fixtures, hot-swap and patch-readiness behavior |
| `crates/arcweft-runtime-host/src/bundle_runner.rs` | 39525 | 1077 | production plus unit tests | 266 | bundle runner execution, product AWBC fixture construction, image/source validation |
| `crates/arcweft-runtime-host/tests/bundle_runner.rs` | 10073 | 270 | integration test | 0 | native-adapter bundle runner product fixture and structured bytecode rejection tests |
| `crates/arcweft-player-web/src/parity.rs` | 9770 | 260 | production | 0 | native-side WebGPU parity frame preparation, runtime stepping through `BundleSession`, interaction visual state selection |
| `crates/arcweft-runtime-driver/src/session.rs` | 27834 | 708 | production | 0 | portable bundle session stepping, queued semantic runtime input, product AWBC runtime construction, hot-swap and patch readiness |
| `crates/arcweft-player-web/tests/parity.rs` | 4079 | 125 | integration test | 0 | browser/native frame parity fixture loading and observation contract comparison |
| `crates/arcweft-project-loader/src/cache/release.rs` | 58082 | 1459 | production plus unit tests | 758 | release manifest cache fetch, local/http/https mirror handling, product fetch tests, minimal AWBC release fixture |
| `crates/arcweft-bundle/src/container.rs` | 76306 | 2335 | production plus unit tests | 662 | AWFB header/index codec, section descriptors, read budgets, content/signing roots, unknown optional preservation tests |
| `crates/arcweft-bundle/src/container/identity.rs` | 1526 | 47 | production | 0 | current AWFB artifact identity digest transcript |
| `crates/arcweft-bundle/src/container/opaque.rs` | 1404 | 56 | production | 0 | raw section-kind code and decoded known/unknown section-kind boundary |
| `crates/arcweft-bundle/src/lib.rs` | 66114 | 1867 | facade plus unit tests | 621 | bundle data model, format codecs, JSON fixture tests, product AWBC test helpers |
| `crates/arcweft-bundle/src/patch.rs` | 49313 | 1298 | production plus unit tests | 505 | patch plan diff/apply, patch AWFB encoding, raw section-kind preservation |
| `crates/arcweft-bundle/src/product.rs` | 24091 | 647 | production plus unit tests | 262 | product AWFB section encode/decode and AWBC executable payload handling |
| `crates/arcweft-player-native/src/lib.rs` | 12640 | 348 | production plus unit tests | 182 | headless native bundle player report and product AWBC test fixtures |
| `crates/arcweft-player-native/src/main.rs` | 12434 | 371 | production plus unit tests | 169 | native player CLI entry and AWFB test fixture |
| `crates/arcweft-player-native/src/patch_endpoint.rs` | 28630 | 758 | production plus unit tests | 366 | native patch endpoint, patch transport tests, product AWBC patch fixtures |

Largest workspace Rust files at this cut are unchanged seq-independent
hotspots:

| Path | Bytes | LOC | Note |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12399 | generated-like vertical orientation table |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255424 | 7945 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 225209 | 6282 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 6161 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209852 | 5651 | integration fixture suite |

`container.rs`, `lib.rs`, and `patch.rs` remain warning-level size hotspots but
below error thresholds. This cut added the new `container::identity` and
`container::opaque` modules instead of expanding the root module with new
subsystem types.
