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

## Non-Goals For This Cut

- No new concrete seq-02 section families were added.
- No content/presentation, shader/UI, entity, runtime-types, entrypoints, or
  adapter-requirements codecs were implemented.
- No common resource wire codec was introduced.
- No patch v2 format, AWFR release archive, external payload-carrier redesign,
  or signing-policy redesign was implemented.
- No generated overlay from `apply_seq_02.py` was applied as a production patch.

The follow-up design packages remain responsible for those larger contract
surfaces. This cut intentionally avoids treating the broad seq-02 request as
complete.

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
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
  - files scanned: 1522
  - Rust files: 838
  - Rust physical LOC: 412856
  - package manifests: 89
  - violations: 0 errors, 105 warnings
- `git diff --check`

`just test-workspace` was run and reached the existing
`arcweft-player-web --test parity` fixture gap:
`web/demo.awfb` is a JSON fixture with `schema_version` 3 while the current
bundle schema expects 4, and that fixture has not yet been regenerated as a
product-AWBC-capable web demo. Earlier workspace tests, including
`arcweft-bundle` and `arcweft-player-native`, passed before that failure.

## Structural Audit Notes

Repository state measured at Jujutsu change `lokxmrmv`.

| Path | Bytes | LOC | Kind | Embedded test LOC | Responsibilities |
| --- | ---: | ---: | --- | ---: | --- |
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
