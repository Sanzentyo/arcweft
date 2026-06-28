# Review

Decision class: **defer**
Target fixture: `vertical_tutr_golden`
Inspected revision: `b0b45b44b2dd34573d991839d950b58091c314b4`

## Evidence summary

Current policy requires strict pinned Windows evidence for milestone/release use:

- `ARW_EXACT_NATIVE_GOLDEN_REQUIRED=1`
- `ARW_EXACT_NATIVE_GOLDEN_PINNED=1`
- `ARW_EXACT_NATIVE_GOLDEN_BACKEND=native_rich_text_observer`
- Windows OS
- pinned `MS Mincho` font probe
- available and versioned `imq`
- metric set `psnr,ssim,mse,mae,maxae`
- same-run candidate PNG, observe JSON, `imq` JSON, and environment fingerprint

The package-generation environment did not satisfy those conditions. It was a
Linux sandbox without `imq`, `just`, `cargo`, a local checkout, or a Windows font
probe. Therefore the exact native run was not executed.

Historical seq06.6 evidence remains useful but insufficient:

```json
{
  "schema": "arcweft.exact_native_golden.drift_evidence.v1",
  "fixture": "vertical_tutr_golden",
  "source": "seq06.6 target-checkout validation evidence from request",
  "classification": "baseline_drift_requires_pinned_environment_review",
  "dimensions": {
    "width": 1280,
    "height": 720
  },
  "metrics": {
    "mse": 0.0030918550895167305,
    "mae": 0.004233718228315644
  },
  "bounds": {
    "max_mse": 0.002,
    "max_mae": 0.003
  },
  "seq06_7_decision": "Do not refresh the checked-in PNG in this package. Retain future candidates under target/ with observe JSON, imq JSON, and environment fingerprint before promotion."
}
```

Seq06.7 explicitly treated that drift as environment-gated baseline review work,
not as a PNG refresh approval, because the seq06.6 evidence lacked the complete
fingerprint required to distinguish stale reference pixels from environment,
font, renderer, or capture-path drift.

## Required review decisions

### 1. Does the pinned environment match the seq06.7 policy requirements?

No. This package was generated outside the required pinned Windows environment.
The local probe reports Linux and no `imq`. There is no evidence that
`MS Mincho` was probed through `%WINDIR%\Fonts\msmincho.ttc`, no Windows OS
version, and no same-run `exact-native-golden.environment.json` from
`just native-visual-artifacts`.

Classification: `environment_blocker` for promotion purposes.

### 2. Is `imq` present, versioned, and running the expected metric set?

No. The local probe reports `imq=not found`. The expected metric set is
`psnr,ssim,mse,mae,maxae`, and it is recorded in the policy and fingerprint
schema, but no versioned `imq` execution occurred in this package.

### 3. Are candidate/reference dimensions identical?

Not reviewable for a new candidate. The candidate PNG was not generated.

Historical seq06.6 drift metadata says the observed dimensions were `1280x720`,
which matches the current policy viewport and checked-in PNG integrity rule, but
that historical evidence is not enough for promotion because it is not paired
with a complete pinned fingerprint.

### 4. Are MSE/MAE over threshold because the checked-in PNG is stale, or because the environment/backend/font/render path is not acceptable for promotion?

Not decidable from the available evidence.

The historical values are above gate:

- MSE `0.0030918550895167305` > `0.002`
- MAE `0.004233718228315644` > `0.003`

However, no current pinned Windows run was available to attribute the drift to a
stale checked-in reference. The safe conclusion is deferral, not promotion and
not renderer rejection.

### 5. Did source fixture text, renderer behavior, font fallback, device scale, viewport, or capture code change since the reference PNG?

Not conclusively. The current source fixture still pins `MS Mincho`, uses
`vertical_rl`, and targets the same `vertical_tutr_golden` flow. The policy and
test code still require a `1280x720` viewport and device scale `1.0` for exact
native goldens.

This package does not have the reference PNG generation commit or a pinned run
source/reference hash comparison from the target machine, so it cannot prove
whether renderer behavior, font fallback, device scale, viewport, or capture code
changed since the checked-in reference was generated.

### 6. Should the reviewed candidate be promoted to `tests/fixtures/native_capture/vertical_tutr_golden.png`?

No. There is no reviewed candidate PNG. Promotion would be a blind overwrite and
would violate the seq06.7/seq06.7.1 policy.

### 7. If promoted, are follow-up changes needed to policy metadata, fixture README, or implementation notes?

Not applicable. No promotion is recommended.

If a future pinned run supports promotion, the maintainer should add or update an
implementation note with the same-run environment fingerprint, before/after
metrics, visual review result, source/reference/candidate hashes, and the exact
post-promotion validation commands. Policy thresholds should remain unchanged
unless a separate design explicitly changes them.

### 8. If not promoted, what exact blocker prevents promotion and what run should be attempted next?

Blocker: `pinned_windows_candidate_packet_missing`.

Next run:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
just test-visual-golden
just native-visual-artifacts
```

The next packet must include, from the same run:

- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png`
- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.observe.json`
- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.imq.json`
- `target/arcweft-native-capture-artifacts/exact-native-golden.environment.json`
- command logs for both `just` targets

If the run produces `baseline_drift`, keep the artifacts and decide whether the
candidate is valid renderer output. If the run produces `environment_blocker`, do
not promote.
