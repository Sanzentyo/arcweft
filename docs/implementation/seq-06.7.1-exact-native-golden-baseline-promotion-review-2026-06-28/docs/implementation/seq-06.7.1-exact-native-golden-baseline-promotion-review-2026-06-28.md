# Seq 06.7.1 exact native golden baseline promotion review

Date: 2026-06-28
Decision: **defer**
Target fixture: `vertical_tutr_golden`

## Summary

This review does not promote a new `tests/fixtures/native_capture/vertical_tutr_golden.png`
baseline. The required pinned Windows evidence packet is missing: no same-run
candidate PNG, observe JSON, `imq` JSON, or `exact-native-golden.environment.json`
was produced from a Windows machine with pinned `MS Mincho`, `imq`, and
`ARW_EXACT_NATIVE_GOLDEN_BACKEND=native_rich_text_observer`.

The existing seq06.6 metrics still show a drift:

- dimensions: `1280x720`
- MSE: `0.0030918550895167305` against gate `0.002`
- MAE: `0.004233718228315644` against gate `0.003`

Those metrics are not enough to prove that the checked-in PNG is stale. Seq06.7
already classified the drift as environment-gated `baseline_drift` requiring a
pinned review, not as an automatic refresh.

## Decision details

- Pinned environment matched: **no evidence**.
- `imq` present/versioned: **no evidence**.
- Candidate/reference dimensions identical: **not reviewable; candidate absent**.
- MSE/MAE cause: **not attributable from current evidence**.
- Source/font/scale/viewport/capture changes since reference: **not conclusively provable**.
- Promote `vertical_tutr_golden.png`: **no**.
- Follow-up metadata changes: **none for this deferral**.
- Next blocker: `pinned_windows_candidate_packet_missing`.

## Required next run

```powershell
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_PINNED = "1"
$env:ARW_EXACT_NATIVE_GOLDEN_BACKEND = "native_rich_text_observer"
just test-visual-golden
just native-visual-artifacts
```

Retain:

```text
target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png
target/arcweft-native-capture-artifacts/vertical_tutr_golden.observe.json
target/arcweft-native-capture-artifacts/vertical_tutr_golden.imq.json
target/arcweft-native-capture-artifacts/exact-native-golden.environment.json
```

If the successor review recommends promotion, put the reviewed PNG under
`promotion-overlay/tests/fixtures/native_capture/vertical_tutr_golden.png` and
run post-promotion validation without loosening thresholds.
