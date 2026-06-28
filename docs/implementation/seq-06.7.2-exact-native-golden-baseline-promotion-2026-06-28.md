# Seq 06.7.2 exact native golden baseline promotion

Date: 2026-06-28
Decision: **promote**
Target fixture: `vertical_tutr_golden`

## Summary

The deferred seq06.7.1 blocker was resolved by collecting a same-run pinned
Windows evidence packet for `vertical_tutr_golden`. The run satisfied the
seq06.7 exact native golden requirements:

- Windows pinned run with `ARW_EXACT_NATIVE_GOLDEN_REQUIRED=1`.
- `ARW_EXACT_NATIVE_GOLDEN_PINNED=1`.
- `ARW_EXACT_NATIVE_GOLDEN_BACKEND=native_rich_text_observer`.
- Pinned `MS Mincho` font probe at `C:\WINDOWS\Fonts\msmincho.ttc`.
- `imq 0.1.0` with `psnr`, `ssim`, `mse`, `mae`, and `maxae`.
- Same-run candidate PNG, observe JSON, `imq` JSON, and environment JSON.

The collected candidate matches the current renderer output from both the
debug/test exact-golden path and the release artifact path:

```text
candidate_sha256=2e381ca2dd5ad76767cf4ae5ec03e2a35228b23a5fc7c35b93e99bc144f52fa2
previous_reference_sha256=1d2a7132ac8fdc7e3e9b4d161a36ebf0a105784ee1aeaa47e6da3142ae0760a1
```

The checked-in baseline
`tests/fixtures/native_capture/vertical_tutr_golden.png` was replaced with the
reviewed candidate. No thresholds were loosened.

## Evidence

The evidence packet was collected into:

```text
target/seq06.7.1-pinned-review-evidence/
```

The packet itself is not checked in because it contains generated target output
and a large observe JSON. The review-relevant hashes are recorded here and in
`fixtures/native-golden-drift/vertical_tutr_golden.seq06.7.2-promotion.json`.

```text
2e381ca2dd5ad76767cf4ae5ec03e2a35228b23a5fc7c35b93e99bc144f52fa2  target-artifacts/vertical_tutr_golden.candidate.png
49927c01771e2bdf4bb1b943fc56d6bb01d231c27f34af153f5b76a16693c133  target-artifacts/vertical_tutr_golden.observe.json
a8634b2965536e1780f9ee97b29904debc19a60bc45caf53e987cf9848bace03  target-artifacts/vertical_tutr_golden.imq.json
f2b62e585cf0f34aaec9db9ca03f767c2a4fc9f677c14ffede454910a82671b0  target-artifacts/exact-native-golden.environment.json
```

Pre-promotion metrics against the old checked-in PNG:

```text
dimensions=1280x720
psnr=25.097808689898176 dB
ssim=0.4080704230342105
mse=0.0030918550895167305
mae=0.004233718228315644
maxae=0.9607843137254901
```

The same drift values appeared in the prior seq06.6 evidence, but seq06.7.1
correctly deferred because that earlier evidence did not include the pinned
Windows packet. This run supplies the missing packet, so the drift is now
classified as stale checked-in baseline pixels rather than an environment
blocker.

## Visual review

The candidate was reviewed against
`tests/fixtures/native_capture/vertical_tutr_golden.arcw`, whose text is:

```arcw
吾輩は猫である。ABC 123 2026。ー「縦書き」
```

The promoted candidate preserves the intended `vertical_rl` source coverage with
`MS Mincho`, mixed Latin, text-combine digits, UAX #50 punctuation handling, and
the existing 1280x720 viewport. The old reference was treated as stale baseline
output from before the current vertical text path.

## Commands run

```powershell
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root . --check-env-only
cargo +nightly -Zscript tools/collect-pinned-windows-review-evidence.rs --root . --out-dir target/seq06.7.1-pinned-review-evidence
Copy-Item -LiteralPath target\seq06.7.1-pinned-review-evidence\target-artifacts\vertical_tutr_golden.candidate.png -Destination tests\fixtures\native_capture\vertical_tutr_golden.png -Force
cargo test -p arcweft-cli --features native-capture --test check native_checked_in_visual_golden_fixtures_are_well_formed --quiet
$env:ARW_EXACT_NATIVE_GOLDEN_REQUIRED='1'; $env:ARW_EXACT_NATIVE_GOLDEN_PINNED='1'; $env:ARW_EXACT_NATIVE_GOLDEN_BACKEND='native_rich_text_observer'; just test-visual-golden
```

## Validation result

- `collect-pinned-windows-review-evidence --check-env-only`: passed.
- `collect-pinned-windows-review-evidence`: completed; `test_visual_exit=1`
  before promotion as expected for `baseline_drift`, `native_artifacts_exit=0`.
- `native_checked_in_visual_golden_fixtures_are_well_formed`: passed.
- pinned `just test-visual-golden`: passed all visual smoke, integrity, and four
  exact native golden fixture checks after promotion.

## Remaining TODO

None for `vertical_tutr_golden` baseline promotion. Future exact native golden
promotions should use the same collector and must not overwrite checked-in PNGs
without a complete same-run packet and implementation note.
