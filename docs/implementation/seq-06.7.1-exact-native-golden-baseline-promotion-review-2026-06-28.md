# Seq 06.7.1 exact native golden baseline promotion review

Date: 2026-06-28
Decision: **defer**
Target fixture: `vertical_tutr_golden`

Superseded by:
`docs/implementation/seq-06.7.2-exact-native-golden-baseline-promotion-2026-06-28.md`.
Seq06.7.1 remains as the historical no-packet deferral record.

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

The complete review packet is retained under
`docs/implementation/seq-06.7.1-exact-native-golden-baseline-promotion-review-2026-06-28/`.
The repository copy of the next-run collector is
`tools/collect-pinned-windows-review-evidence.rs`. The package-retained
PowerShell script remains inside the review packet as received evidence, but the
repo-supported collector is the Rust script.

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

Or use the repository collector:

```powershell
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root . --out-dir seq06.7.1-pinned-review-evidence
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

## Repository application

This checkout applies seq06.7.1 as a deferral packet only. It does not update
`tests/fixtures/native_capture/vertical_tutr_golden.png`, does not loosen exact
native golden thresholds, and does not move exact native validation into the
default workspace test path.

## Validation in this checkout

```text
rustfmt --check tools\collect-pinned-windows-review-evidence.rs
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --help
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root . --check-env-only
SHA256SUMS.txt verified for the retained seq06.7.1 packet
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit reported `0 error(s), 113 warning(s)`. The actual pinned
capture run was intentionally not executed during this application; that run is
the deferred successor evidence packet.
