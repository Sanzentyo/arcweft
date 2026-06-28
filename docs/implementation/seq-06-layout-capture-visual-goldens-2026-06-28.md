# Seq-06 Layout, Capture, and Visual Goldens implementation note (2026-06-28)

## Source request

`docs/reviews/requests/2026-06-24-seq-06-layout-capture-visual-goldens.md`

## Current evidence inspected

- `AGENTS.md` current main repository policy.
- `docs/implementation/integrated-execution-2026-06-24.md`.
- `docs/implementation/layout-scaling-units-and-capture-2026-06-24.md`.
- `docs/reviews/requests/2026-06-24-layout-fit-mode-coordinate-contract.md`.
- `docs/reviews/requests/2026-06-24-layout-units-text-fitting-and-shared-capture.md`.
- `crates/arcweft-layout/src/lib.rs`.
- `crates/arcweft-cli/src/app/agent/native/runtime_observation.rs`.
- Native Agent observe/capture supporting files:
  - `crates/arcweft-agent-protocol/src/geometry.rs`;
  - `crates/arcweft-agent-protocol/src/image.rs`;
  - `crates/arcweft-agent-protocol/src/object.rs`;
  - `crates/arcweft-cli/src/app/agent/native/image_mapping.rs`;
  - `crates/arcweft-cli/src/app/agent/native/observe_resources.rs`.

## Implemented in this overlay

- Extended `arcweft-layout` with typed public coordinate-space names:
  `design`, `content`, `output`, `physical`, `logical`, `object_local`, and
  `layer_local`.
- Added `FitTransformMetadata` to centralize policy, content rect, visible output
  rect, visible design rect, scale, raw-mode flag, bars, crop, serialized
  geometry basis, and hit-test input basis.
- Added `ContentRect::bars`, `ContentRect::crop`,
  `ContentRect::visible_output_rect`, `ContentRect::visible_design_rect`,
  `ContentRect::unmap_rect`, `ContentRect::fit_transform_metadata`, and
  `ContentRect::hit_test_mapping`.
- Added `LayoutUnitResolutionPhase` plus dependency methods on `LayoutUnit`:
  `earliest_resolution_phase`, `requires_font_metrics`, `requires_safe_area`,
  and `requires_content_rect`.
- Added `TextFitOutcome`, compact `TextFitReportFlags`, `TextFitReport`, and
  `TextFitResult::report()` so text fitting can report truncation, scaling,
  pagination, expansion, failure, and diagnostics without renderer I/O.
- Added selected capture metadata types:
  `CaptureScope`, `CaptureComposition`, `CaptureRendererKind`,
  `CaptureCropBounds`, `CaptureMaskMetadata`, and `CaptureMetadata` with
  `selected_object` / `selected_layer` constructors.
- Added focused integration tests under
  `crates/arcweft-layout/tests/presentation_contract.rs`.
- Extended native Agent observe so `layout.viewport_scale` serializes the new
  fit-transform metadata fields.
- Added the design answer under
  `docs/reviews/designs/2026-06-28-seq-06-layout-capture-visual-goldens.md`.

## Intended application order

1. Copy `overlay/crates/arcweft-layout/src/lib.rs` over the current file.
2. Copy `overlay/crates/arcweft-layout/tests/presentation_contract.rs` into the
   crate tests directory.
3. Copy the design and implementation markdown files under `overlay/docs/`.
4. Apply `patches/0001-agent-observe-fit-transform-metadata.patch` from the
   repository root, or manually port the hunk if nearby code has drifted.
5. Run formatting and validation commands listed below.

## Validation commands to run after application

```bash
cargo fmt --all -- --check
cargo test -p arcweft-layout
cargo test -p arcweft-cli --features native-capture --lib agent_observe_
cargo check -p arcweft-layout -p arcweft-cli --features native-capture --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/seq-06-layout-capture-visual-goldens-2026-06-28
```

## Validation performed

The package environment originally performed only package-local inventory
checks. After applying the overlay to this checkout, the following repository
validation passed:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-layout
cargo test -p arcweft-cli --features native-capture --lib agent_observe_
cargo check -p arcweft-layout -p arcweft-cli --features native-capture --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

Structural audit scanned `1697` files, `913` Rust files, and `438423` Rust
physical LOC with `0` errors and `107` warnings. The applied
`crates/arcweft-layout/src/lib.rs` remains below the AGENTS.md `lib.rs` warning
threshold of 1,000 LOC.

## Remaining TODOs

- Wire selected object/layer capture resource metadata into
  `arcweft-agent-protocol` and native/WebGPU adapters once the current resource
  constructors are updated together.
- Add `--scale-policy raw|contain|cover|stretch` only as an explicit follow-up;
  v1 default remains raw.
- Add visual smoke/golden fixtures after shared WebGPU capture assets and pinned
  font/backend policy are available in the checkout.

## Design deviations

- The overlay intentionally does not move renderer/GPU/filesystem/capture I/O
  into `arcweft-layout`.
- It does not change Agent observe's default coordinate behavior from raw to fit
  mode.
- The packaged Agent observe patch was malformed, so the same hunk was manually
  ported to the current `runtime_observation.rs`.
- `TextFitReport` uses a compact `TextFitReportFlags` value instead of five
  public bool fields so the API passes the active clippy policy without local
  lint allowances.
- It provides typed shared capture metadata in `arcweft-layout`; full
  `arcweft-agent-protocol` resource-schema migration is left as a follow-up
  because current protocol constructors exist in several crates and should be
  migrated in one focused cut.
