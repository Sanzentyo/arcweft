# seq06.13e.1 Inset Box-Shadow Pinned PNG Golden Promotion — 2026-07-04

## Status

The native PNG baseline is promoted from the current user-approved pinned
Windows environment as a pinned visual-golden run. The Web PNG baseline remains
pending because the repository
still lacks a seq06.13e.1 browser/WebGPU exact readback harness that writes the
required candidate PNG and observe JSON.

## Current upstream state used for this package

Seq06.13e already implemented typed inset planning, compositor ordering, WGSL
kind-flag rendering, Takumi adapter coverage, and an ignored GPU smoke test for a
rounded inset card plus mixed outer+inset card. This package does not redesign
`UiBoxShadow`, `UiBoxShadowPassPlan`, Takumi lowering, or `PASS_BOX_SHADOW`.

## Added review contract

- `fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow-exact-png-policy.json`
  defines native/Web pinned fingerprints, metrics, storage paths, route evidence,
  and failure classifications.
- `docs/fixtures/native/seq06_13e1_inset_box_shadow_exact_golden.json` documents
  the native exact fixture and forbidden fallback paths.
- `docs/fixtures/web/seq06_13e1_inset_box_shadow_exact_golden.json` documents the
  Web exact fixture and the same renderer route.
- `tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs` collects and
  classifies pinned evidence packets without copying baselines.
- `tools/capture-seq06-13e1-inset-shadow-native-frame.rs` renders the native
  compositor fixture into the packet candidate PNG and observe JSON paths.
- `tools/source-gates/seq06_13e1_inset_shadow_exact_golden_policy.rs` validates the
  policy/source documentation contract outside the pinned environment.
- `crates/arcweft-render-wgpu/tests/ui_box_shadow_exact_png_golden.rs` adds
  ignored Tier 2 packet-completeness tests for native/Web exact PNG evidence and
  a non-ignored route-policy test.

## Required pinned native command

```powershell
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND = "wgpu_offscreen_compositor"
cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden --mode native --run
```

The native collector now invokes
`tools/capture-seq06-13e1-inset-shadow-native-frame.rs` during `--run`. It writes:

- `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.candidate.png`
- `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.observe.json`
- `target/seq06.13e.1-inset-box-shadow-golden/native/command-logs/native-exact-png-capture.log`

If the checked-in native reference PNG is still absent, the collector records a
`baseline_missing` metrics JSON and a
`ready_for_first_promotion_review` decision instead of copying the candidate into
the fixture tree. Once the candidate is reviewed and copied to the reference path,
the same command enforces the `max_mse=0.002` and `max_mae=0.003` `imq` gate.

## Required pinned Web command

```powershell
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED = "1"
$env:ARW_SEQ06_13E1_INSET_SHADOW_WEBGPU = "1"
cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden --mode web --run
```

## Review decision

The existing ignored GPU smoke fixture remains an execution regression. Exact PNG
validation is represented by separate native/Web fixture docs and the policy file.
A future reviewer may promote baselines only after candidate PNGs, observation
JSON, `imq` JSON, fingerprints, command logs, and source/candidate/reference
hashes are present in one pinned evidence packet.

## Validation in this package

The package authoring environment did not run pinned GPU/Web evidence. JSON files
were validated, but Rust commands must be run after applying the overlay in a real
checkout.

## Applied checkout validation

Validated in the 2026-07-04 apply checkout:

- `cargo fmt --all -- --check`
- `cargo +nightly -Zscript tools/source-gates/seq06_13e1_inset_shadow_exact_golden_policy.rs --root .`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden --all-features -- --nocapture`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden seq06_13e1_inset_shadow_native_exact_png_packet_is_complete --all-features -- --ignored --exact --nocapture`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden seq06_13e1_inset_shadow_web_exact_png_packet_is_complete --all-features -- --ignored --exact --nocapture`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path --all-features -- --ignored --exact --nocapture`
- `cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden-dry-run --mode both`
- `just seq06-13e1-inset-shadow-policy`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq06-13e1`

The ignored native/Web exact packet tests reported `baseline_missing` in this
non-pinned checkout, which is the intended no-promotion state. The structural
audit reported existing workspace hotspots, not new seq06.13e.1 files: 4 errors
and 127 warnings.

## Local progress after no-promotion package

The native half has now been promoted after the user approved treating the
current terminal as the pinned visual-golden environment. Candidate PNG and
observe JSON generation exist as repo-owned tooling, and the collector produced a
same-run evidence packet before and after the checked-in reference PNG was
added.

Pinned native environment used for promotion:

- OS: `Microsoft Windows [Version 10.0.26200.8737]`
- GPU: `NVIDIA GeForce RTX 3080 Ti`
- Backend: `Vulkan`
- Driver: `NVIDIA 591.86`
- Native route env: `ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND=wgpu_offscreen_compositor`
- Pin env: `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED=1`,
  `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1`
- imq: `imq 0.1.0`
- Arcweft commit: `bd4cc31756063ba2bb863e560d9734f92fbb13be`

Native promotion evidence:

- Candidate PNG:
  `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.candidate.png`
- Checked-in reference PNG:
  `fixtures/visual-smoke-goldens/seq06.13e.1-inset-box-shadow/native/seq06_13e1_inset_box_shadow.png`
- Observe JSON:
  `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.observe.json`
- imq JSON:
  `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.imq.json`
- Environment fingerprint:
  `target/seq06.13e.1-inset-box-shadow-golden/native/seq06_13e1_inset_box_shadow.environment.json`
- Command logs:
  `target/seq06.13e.1-inset-box-shadow-golden/native/command-logs/native-exact-png-capture.log`
  and
  `target/seq06.13e.1-inset-box-shadow-golden/native/command-logs/native-compositor-smoke.log`
- Review decision:
  `target/seq06.13e.1-inset-box-shadow-golden/review/seq06_13e1_native_promotion_decision.json`

Native promotion result:

- Candidate/reference dimensions: `320x180`
- Candidate/reference SHA-256:
  `1106d553d834d9f9f90eddd0ef2655608c9646733e63141a9646b4bc1612b95c`
- imq metrics after promotion: `mse=0.0`, `mae=0.0`, `maxae=0.0`,
  `ssim=1.0`; `psnr=null` because the images are identical and MSE is zero.
- Exact packet test:
  `seq06_13e1_inset_shadow_native_exact_png_packet_is_complete` passed with the
  pinned env set.

The Web exact capture harness remains pending because the existing `web`
Playwright smoke validates the application canvas but does not render this
320x180 seq06.13e.1 compositor fixture through a pinned browser/WebGPU readback
path. DOM/CSS screenshots remain forbidden for this fixture.

Follow-up design/implementation request:
`docs/reviews/requests/2026-07-04-seq-06.13e.1.1-web-exact-png-readback-harness.md`.

Additional local validation after adding the native capture harness:

- `cargo +nightly -Zscript tools/capture-seq06-13e1-inset-shadow-native-frame.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-native-capture-local`
- `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1 ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND=wgpu_offscreen_compositor cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-native-collector-local-2 --mode native --run`
- `just seq06-13e1-inset-shadow-native-capture target\seq06.13e.1-inset-box-shadow-just-native-capture-local`
- `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1 ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND=wgpu_offscreen_compositor just seq06-13e1-inset-shadow-pinned-native-golden target\seq06.13e.1-inset-box-shadow-final-native-local`
- `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED=1 ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1 ARW_SEQ06_13E1_INSET_SHADOW_NATIVE_BACKEND=wgpu_offscreen_compositor cargo +nightly -Zscript tools\collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target\seq06.13e.1-inset-box-shadow-golden --mode native --run`
- `ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_REQUIRED=1 ARW_SEQ06_13E1_INSET_SHADOW_GOLDEN_PINNED=1 cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden seq06_13e1_inset_shadow_native_exact_png_packet_is_complete --all-features -- --ignored --exact --nocapture`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke per_corner_outer_and_elliptical_inset_shadow_cards_execute_gpu_compositor_path --all-features -- --ignored --exact --nocapture`
- `cargo +nightly -Zscript tools\source-gates\seq06_13e1_inset_shadow_exact_golden_policy.rs --root .`
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_exact_png_golden seq06_13e1_inset_shadow_policy_pins_typed_compositor_route --all-features -- --exact`
- `imq image fixtures\visual-smoke-goldens\seq06.13e.1-inset-box-shadow\native\seq06_13e1_inset_box_shadow.png target\seq06.13e.1-inset-box-shadow-golden\native\seq06_13e1_inset_box_shadow.candidate.png --metrics psnr,ssim,mse,mae,maxae --format json`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq06-13e1-native-capture`

The first pinned native packet reached `ready_for_first_promotion_review` with a
`baseline_missing` metrics JSON. After visual review, the candidate was copied to
the checked-in native reference path, the collector was rerun, and the native
packet reached `passed_existing_baseline_gate` with zero drift.
