# Fixture Regeneration

This note is the repository-local checklist for refreshing generated test
fixtures without rediscovering the commands from individual tests.

## Main Command

```bash
just fixture-refresh
```

This command refreshes checked-in deterministic generated artifacts and then
runs focused validation. It intentionally stays portable: native renderer PNG
goldens are checked in, but their pixels depend on the Windows native text path
and pinned fonts, so the full command writes review candidates under `target/`
instead of overwriting checked-in PNGs.

Current refresh targets:

| Target | Source | Command |
| --- | --- | --- |
| `web/demo.awfb` | `web/demo.arcw` | `just fixture-refresh-web-demo-awfb` |
| `web/assets/generated-background.png` | `tools/generate-webgpu-demo-assets.rs` | `just fixture-refresh-webgpu-demo-assets` |
| `web/assets/generated-character.png` | `tools/generate-webgpu-demo-assets.rs` | `just fixture-refresh-webgpu-demo-assets` |
| `web/assets/generated-pulse.gif` | `tools/generate-webgpu-demo-assets.rs` | `just fixture-refresh-webgpu-demo-assets` |
| `web/assets/generated-pulse.webp` | `tools/generate-webgpu-demo-assets.rs` | `just fixture-refresh-webgpu-demo-assets` |
| `web/.arcweft/asset/generated/*` | `tools/generate-webgpu-demo-assets.rs` | `just fixture-refresh-webgpu-demo-assets` |
| `crates/arcweft-lang-syntax/src/jlreq_punctuation_data.rs` | `tools/generate_jlreq_punctuation_data.rs` | `just generate-jlreq-punctuation` |

Focused validation run by `just fixture-refresh-check`:

```bash
cargo run -p arcweft-cli --quiet -- inspect web/demo.awfb --json
cargo test -p arcweft-player-web --test parity --all-features --quiet
cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet
```

## Full Refresh With Native Candidates

```bash
just fixture-refresh-all
```

This runs the portable refresh plus native capture PNG candidate generation and
the native fixture-integrity check. Use it on a Windows machine with the pinned
fixture fonts available when native renderer output is intentionally changing.
It also runs the deterministic non-exact `visual_smoke` suite, which checks
selected object/layer metadata and image-content smoke without exact pixels.
Promote candidate PNGs to checked-in goldens only after reviewing the pixel
result and rerunning the integrity check.

Seq06.7 makes native exact refresh a reviewed promotion step rather than blind
regeneration. Candidate generation must retain the full review packet:

```text
target/arcweft-native-capture-refresh/
  exact-native-golden.environment.json
  <fixture>.png
  <fixture>.observe.json
  <fixture>.imq.json
```

Before replacing a checked-in PNG, compare the candidate against the checked-in
reference, inspect the PNG visually, and record the environment fingerprint and
before/after metrics in an implementation note. Do not copy a candidate over a
checked-in PNG merely because `fixture-refresh-all` produced it. Missing `imq`,
missing pinned `MS Mincho`, unsupported backend, or a non-pinned milestone run is
an environment blocker, not a PNG-refresh approval.

The `vertical_tutr_golden` seq06.7.1 promotion review initially deferred because
no pinned Windows candidate packet was available. Seq06.7.2 collected that
packet on Windows and promoted the reviewed candidate to
`tests/fixtures/native_capture/vertical_tutr_golden.png` without changing
thresholds. Use
`cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root .`
for future exact native golden promotion reviews to collect the candidate PNG,
observe JSON, `imq` JSON, environment fingerprint, and command logs in one
directory. The collector refuses to run outside Windows.

Additional native candidate targets:

| Target | Source | Command |
| --- | --- | --- |
| `target/arcweft-native-capture-refresh/vertical_tutr_golden.png` | `tests/fixtures/native_capture/vertical_tutr_golden.arcw` | `just fixture-refresh-native-capture-candidates` |
| `target/arcweft-native-capture-refresh/vertical_jlreq_preset_loose_golden.png` | `tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw` | `just fixture-refresh-native-capture-candidates` |
| `target/arcweft-native-capture-refresh/vertical_jlreq_preset_normal_golden.png` | `tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw` | `just fixture-refresh-native-capture-candidates` |
| `target/arcweft-native-capture-refresh/vertical_lr_ruby_text_combine_golden.png` | `tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw` | `just fixture-refresh-native-capture-candidates` |
| `target/arcweft-native-capture-artifacts/vertical_goal_clear_smoke.candidate.png` | `tests/fixtures/native_capture/vertical_goal_clear_smoke.arcw` | `just native-visual-artifacts` |

Additional validation run by `just fixture-refresh-native-capture-check`:

```bash
cargo test -p arcweft-cli --features native-capture --test check visual_smoke -- --nocapture
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
```

## List Command

```bash
just fixture-refresh-list
```

Use this before adding a new generated fixture. If a checked-in artifact can be
deterministically regenerated, add it to this note and to `fixture-refresh`.
If a checked-in artifact is renderer- or platform-sensitive, add it to the full
refresh candidate path instead.

## Candidate-Only Artifacts

These commands produce review artifacts under `target/` and do not overwrite
checked-in fixtures:

```bash
just native-visual-artifacts
just webgpu-parity
```

Promote candidate images or reports to checked-in fixtures only after comparing
the output and documenting why the expected result changed. For exact native
goldens, promotion additionally requires candidate PNG, observe JSON, `imq` JSON,
and `exact-native-golden.environment.json` from the same run.

## Authored Fixtures

The `.arcw` files under `tests/fixtures/arcw/` are authored test inputs. They
are validated by fixture tests but are not regenerated by `fixture-refresh`.

Runtime-driver and runtime-host product AWBC fixtures are constructed inside
Rust tests from typed `RuntimePlan` values. They should stay source-generated in
tests rather than becoming checked-in binary blobs unless a future design note
explicitly asks for binary product fixtures.
