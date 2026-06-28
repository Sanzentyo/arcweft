# Decision

Decision: **DEFER**
Promotion overlay: **not included**
Threshold changes: **none**
Checked-in PNG changes: **none**

## Rationale

The seq06.7.1 request permits promotion, rejection, or deferral, but it requires
a decision backed by a pinned Windows packet. This package does not contain that
packet. It contains current source/policy inspection and historical seq06.6 drift
metadata, but no same-run candidate PNG, observe JSON, `imq` metrics JSON, or
pinned Windows environment fingerprint.

Promotion would be unsafe because the package cannot distinguish:

- stale checked-in reference PNG;
- renderer/backend drift;
- font fallback or missing pinned `MS Mincho`;
- device-scale or viewport drift;
- capture-code drift;
- `imq` metric/version drift.

Rejection would also be overclaimed: no current candidate was reviewed, so the
renderer/font/environment path has not been proven invalid. The correct decision
is to defer until the strict pinned Windows run exists.

## Concrete blocker

`pinned_windows_candidate_packet_missing`

Required missing artifacts:

```text
target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png
target/arcweft-native-capture-artifacts/vertical_tutr_golden.observe.json
target/arcweft-native-capture-artifacts/vertical_tutr_golden.imq.json
target/arcweft-native-capture-artifacts/exact-native-golden.environment.json
```

## Next attempt

Run this on Windows with the pinned fixture font and `imq` available:

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
just test-visual-golden
just native-visual-artifacts
```

Then build a successor packet that includes the real artifacts. If promotion is
recommended in that successor packet, place only the reviewed replacement PNG
under:

```text
promotion-overlay/tests/fixtures/native_capture/vertical_tutr_golden.png
```

and run:

```bash
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
just test-visual-golden
git diff --check
```
