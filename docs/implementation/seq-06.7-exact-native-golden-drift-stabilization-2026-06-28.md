# Seq 06.7 exact native golden drift stabilization implementation

Date: 2026-06-28

## Source request

- `docs/reviews/requests/2026-06-28-seq-06.7-exact-native-golden-drift-stabilization-package.md`
- Builds on seq06.6 visual smoke/golden fixtures and selected capture metadata.

## Goal

Seq06.7 makes the exact native golden tier operationally repeatable without
moving exact pixel comparison into the default fast path. The cut keeps
`test-visual-golden` Tier 2 and turns drift handling into a reviewed workflow:

1. capture a candidate PNG;
2. retain the observe JSON;
3. retain the `imq` JSON;
4. retain an environment fingerprint;
5. classify the result before accepting or rejecting a baseline.

## Decisions

### Environment fingerprint

Exact native golden evidence must include a JSON fingerprint with schema
`arcweft.exact_native_golden.environment.v1`. The fingerprint records:

- OS family, architecture, and version-family probe;
- renderer/backend path (`native_rich_text_observer`) and optional backend env;
- requested font family (`MS Mincho`) and the conservative Windows font-file probe;
- viewport `1280x720` and raw device scale `1.0`;
- PNG capture format (`arcw agent observe --image png`);
- Arcweft git commit, dirty status, and per-fixture `git hash-object` source and
  reference hashes when Git is available;
- `imq` availability, version/help line, and metric set
  `psnr,ssim,mse,mae,maxae`;
- artifact paths and existence booleans for candidate PNG, observe JSON, and
  metrics JSON.

The test helper writes a per-fixture fingerprint under:

```text
target/arcweft-native-golden-drift/test-visual-golden/<fixture-id>/<fixture-id>.environment.json
```

The artifact/refresh Justfile targets write a run-level fingerprint under the
selected output directory:

```text
target/arcweft-native-capture-artifacts/exact-native-golden.environment.json
target/arcweft-native-capture-refresh/exact-native-golden.environment.json
```

### Classification of the seq06.6 `vertical_tutr_golden` drift

The known target validation failure is classified as `baseline_drift`, not as an
automatic checked-in PNG refresh:

```text
fixture: vertical_tutr_golden
dimensions: 1280x720
mse: 0.0030918550895167305
mae: 0.004233718228315644
existing gates: mse <= 0.002, mae <= 0.003
```

The dimensions match and the capture/`imq` pipeline produced metrics, so this is
not a malformed artifact or hard capture failure. Seq06.7 does not prove that the
checked-in reference is stale, because the failing environment fingerprint from
seq06.6 did not include OS version, backend, font probe, commit/source hash, or
`imq` version. The drift remains environment-gated work until a pinned Windows
run produces the complete artifact set and a human review accepts the candidate.

### Baseline refresh rules

The candidate command for review is:

```bash
just native-visual-artifacts
```

The review packet must include:

- `target/arcweft-native-capture-artifacts/<fixture>.candidate.png`;
- `target/arcweft-native-capture-artifacts/<fixture>.observe.json`;
- `target/arcweft-native-capture-artifacts/<fixture>.imq.json`;
- `target/arcweft-native-capture-artifacts/exact-native-golden.environment.json`;
- a short implementation note explaining why renderer behavior, font fallback,
  capture source changes, or stale checked-in PNGs are the chosen cause.

Promotion is a manual copy step after review:

```powershell
Copy-Item target\arcweft-native-capture-artifacts\vertical_tutr_golden.candidate.png `
  tests\fixtures\native_capture\vertical_tutr_golden.png
```

Then run:

```bash
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
just test-visual-golden
```

No command in this cut overwrites checked-in PNGs directly.

### Threshold policy

Seq06.7 keeps the existing hard gates for every checked-in native exact fixture:

```text
dimensions must match
mse <= 0.002
mae <= 0.003
```

`psnr`, `ssim`, and `maxae` remain record-only metrics. This avoids silently
loosening thresholds to make the current `vertical_tutr_golden` drift pass. The
policy JSON now records the same per-fixture thresholds explicitly so future
fixtures can differ without hiding those decisions in test code.

### CI behavior

Exact native golden CI is an explicit Tier 2 job. The milestone/release job must
run on Windows with the pinned fixture font and `imq` available, set:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
```

and execute:

```bash
just test-visual-golden
just native-visual-artifacts
```

CI should upload `target/arcweft-native-capture-artifacts/`.

Status classes:

| Status | Meaning | Required CI result |
| --- | --- | --- |
| `expected_skip` | non-Windows/local exact tier was invoked without a required pinned job | do not use as milestone evidence |
| `environment_not_pinned` | required exact job did not set `ARW_EXACT_NATIVE_GOLDEN_PINNED=1` | fail hard |
| `environment_blocker` | missing `imq`, missing pinned font probe, or unsupported backend | fail hard |
| `baseline_drift` | capture and dimensions are valid but MSE/MAE exceed gates | fail hard and review artifacts |
| `hard_visual_regression` | capture failure, imq failure, dimension mismatch, or malformed PNG | fail hard |

### Artifact retention

`just test-visual-golden` keeps failure artifacts under
`target/arcweft-native-golden-drift/test-visual-golden/<fixture-id>/` instead of
removing the temporary directory at the end of the test. `just native-visual-artifacts`
continues to write release-built candidate artifacts under
`target/arcweft-native-capture-artifacts/` and now also writes a fingerprint.
`just fixture-refresh-native-capture-candidates` writes comparable candidate,
observe, metric, and fingerprint files under
`target/arcweft-native-capture-refresh/` for reviewed promotion.

### Test split

The all-in-one checked-in golden test is split into one ignored test per fixture:

- `agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_tutr`
- `agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_jlreq_preset_loose`
- `agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_jlreq_preset_normal`
- `agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_lr_ruby_text_combine`

`just test-visual-golden` uses a shared substring filter so all fixture tests run
while Cargo reports the failing fixture name directly.

## Implemented overlay

- `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`
  - adds per-fixture policy records;
  - writes observe JSON, `imq` JSON, and environment fingerprint paths;
  - classifies environment blockers and baseline drift;
  - retains target artifacts instead of deleting them;
  - splits exact fixture tests.
- `tools/write-native-golden-fingerprint.rs`
  - new Rust script used by Justfile artifact targets.
- `Justfile`
  - adds `native-visual-preflight`;
  - updates `native-visual-artifacts` and
    `fixture-refresh-native-capture-candidates` to write fingerprint and metrics
    together with candidates.
- `fixtures/visual-smoke-goldens/exact-native-golden-policy.json`
  - replaces seq06.6's compact policy with status classes, fingerprint schema,
    per-fixture thresholds, and artifact roots.
- `docs/implementation/test-execution-policy.md`
  - documents exact native golden pass/fail/skip semantics.
- `docs/implementation/fixture-regeneration.md`
  - documents reviewed promotion instead of blind regeneration.
- `tests/fixtures/native_capture/README.md`
  - documents candidate/fingerprint artifacts and promotion rules.

## PNG baseline update decision

Seq06.7 does not update checked-in native golden PNGs. The existing drift is not
resolved by threshold changes and does not carry enough fingerprint evidence to
prove that a checked-in baseline replacement is valid. The next validation run
must produce a complete candidate packet and then decide whether to promote a
new reference.

## Validation status

The package was prepared from GitHub connector inspection of `main` after
seq06.6. The artifact build environment did not contain a local Arcweft checkout
or Windows native capture stack, so repository validation commands were not run
inside the package-generation sandbox. The exact commands to run in the target
checkout are recorded in `verification/VALIDATION.md`.
