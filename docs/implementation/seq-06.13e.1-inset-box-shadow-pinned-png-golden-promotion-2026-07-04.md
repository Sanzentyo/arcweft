# seq06.13e.1 Inset Box-Shadow Pinned PNG Golden Promotion — 2026-07-04

## Status

No PNG baseline is promoted by this cut. The exact native/Web promotion remains
environment-gated and requires a pinned visual-golden run with complete same-run
evidence.

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
- `cargo test -p arcweft-render-wgpu --test ui_box_shadow_gpu_smoke rounded_inset_and_mixed_shadow_cards_execute_gpu_compositor_path --all-features -- --ignored --exact --nocapture`
- `cargo +nightly -Zscript tools/collect-seq06-13e1-inset-shadow-pinned-golden-evidence.rs --root . --out-dir target/seq06.13e.1-inset-box-shadow-golden-dry-run --mode both`
- `just seq06-13e1-inset-shadow-policy`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq06-13e1`

The ignored native/Web exact packet tests reported `baseline_missing` in this
non-pinned checkout, which is the intended no-promotion state. The structural
audit reported existing workspace hotspots, not new seq06.13e.1 files: 4 errors
and 127 warnings.
