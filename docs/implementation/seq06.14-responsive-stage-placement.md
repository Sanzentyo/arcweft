# Seq06.14 responsive stage placement implementation note

## Ownership

The implementation adds the core contract to `arcweft-layout` because that crate is already the Sans I/O owner of coordinate spaces, fit transforms, safe-area inputs, and capture metadata.

The overlay intentionally avoids solving placement in browser DOM/CSS, canvas 2D fallback, or platform window layout. The player prepares resolved image bounds before native/web/offscreen rendering.

## Crate integration map

| Crate/file | Change |
|---|---|
| `crates/arcweft-layout/src/stage_placement.rs` | New shared typed contract and deterministic resolver. |
| `crates/arcweft-layout/src/lib.rs` | `pub mod stage_placement;` |
| `crates/arcweft-bundle/Cargo.toml` | Add `arcweft-layout.workspace = true`. |
| `crates/arcweft-bundle/src/lib.rs` | Add `placement: Option<StagePlacement>` to `BundleImageObject`. |
| `crates/arcweft-cli/src/app/bundle.rs` | Parse anchored image declarations, reject ambiguous constraints, and derive fallback design bounds. |
| `crates/arcweft-cli/src/app/bundle/stage_placement.rs` | Own CLI bundle parsing for absolute/anchored stage placement and canonical design bounds. |
| `crates/arcweft-runtime-driver/src/display.rs` | Preserve explicit absolute mode for inline images and provide the same placement payload when present. |
| `crates/arcweft-player-scene/src/images.rs` | Resolve placement from bundle object + viewport into `RenderImage.bounds` and `RenderImage.placement`. |
| `crates/arcweft-player-scene/src/frame.rs` | Pass viewport to image lowering. |
| `crates/arcweft-render-wgpu/src/geometry/images.rs` | Carry resolved placement metadata; quad math remains backend-neutral. |
| `crates/arcweft-agent-protocol/src/object.rs` | Report authored/resolved placement in image content. |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | Populate Agent observe placement metadata from prepared `RenderImage`. |
| `crates/arcweft-cli/tests/responsive_stage_placement.rs` | Run real CLI bundle/observe coverage for responsive placement and typed conflict diagnostics. |
| `samples/zundamon-stand-switch/src/main.arcw` | Switch stand images to responsive top-right anchor placement. |

## Resolution order

1. The compiler/bundler parses authored placement.
2. The bundle keeps authored placement and a design-size fallback `bounds` for existing tooling that only reads old fields.
3. `PlayerFramePlanner` asks `BundleImageCatalog` for images with the current `RenderViewport`.
4. `BundleImageCatalog` resolves `StagePlacement` into output logical `HitRect` using `StagePlacementContext`.
5. `RenderImage` carries the resolved `HitRect` plus optional `ResolvedStagePlacement` metadata.
6. Shared wgpu/native renderers draw from `RenderImage.bounds`; no renderer-specific layout occurs.
7. Agent observe serializes the same prepared-frame placement metadata; capture crop uses the prepared bbox.

## Diagnostics implementation

The overlay emits authoring diagnostics in bundle parsing for conflicts that cannot exist in a valid `StagePlacement` value:

- mixed old absolute and new anchor fields;
- missing `size.width` / `size.height`;
- unsupported `scale.x` / `scale.y` independent scaling;
- unsupported anchor names.

The resolver emits geometry diagnostics after resolution:

- object exceeds viewport;
- object exceeds safe area;
- non-finite geometry.

In a follow-up patch, parser/sema can route these through the normal structured diagnostic type with source spans. This package keeps the core diagnostic shape stable and introduces CLI errors at the current bundling boundary.

## Validation status

Applied and validated in the repository checkout on 2026-07-03.

The implementation uses exact integer comparison at the authored contract boundary: tests convert resolved float bboxes back to Arcweft's canonical milli fixed-point units and compare those integer tuples exactly. This avoids `float_cmp` without weakening the placement contract into loose epsilon matching.

Commands run:

```bash
cargo test -p arcweft-layout --test stage_placement
cargo test -p arcweft-player-scene --test responsive_stage_placement
cargo test -p arcweft-cli --test responsive_stage_placement
cargo test -p arcweft-bundle bundle_image_objects_round_trip_as_typed_metadata
cargo check -p arcweft-layout -p arcweft-bundle -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-agent-protocol -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-layout -p arcweft-bundle -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-agent-protocol -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo clippy -p arcweft-cli --test responsive_stage_placement --all-features -- -D warnings
cargo run -p arcweft-cli -- bundle fixtures/responsive-stage-placement/stand-top-right.arcw --output target/seq06.14/stand-top-right.awfb --format awfb
cargo run -p arcweft-cli -- agent observe fixtures/responsive-stage-placement/stand-top-right.arcw --viewport-width 1280 --viewport-height 720 --image png --json > target/seq06.14/observe-1280x720.json
cargo run -p arcweft-cli -- agent observe fixtures/responsive-stage-placement/stand-top-right.arcw --viewport-width 1920 --viewport-height 1080 --image png --json > target/seq06.14/observe-1920x1080.json
cargo run -p arcweft-cli -- agent observe fixtures/responsive-stage-placement/stand-top-right.arcw --viewport-width 2560 --viewport-height 1440 --image png --json > target/seq06.14/observe-2560x1440.json
cargo run -p arcweft-cli -- bundle fixtures/responsive-stage-placement/conflicting-placement.arcw --output target/seq06.14/conflicting.awfb --format awfb
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/seq-06-14-responsive-stage-placement-2026-07-03
```

The final conflicting-placement command is expected to fail. It failed with `error[stage_placement.independent_axis_scale_rejected]`, proving that invalid responsive placement reaches the typed placement diagnostic boundary instead of falling through to renderer behavior.

Observed `resolved_placement.output_bbox` values:

| Viewport | x | y | width | height |
|---|---:|---:|---:|---:|
| 1280x720 | 930 | 20 | 250 | 430 |
| 1920x1080 | 1395 | 30 | 375 | 645 |
| 2560x1440 | 1860 | 40 | 500 | 860 |

## Package application adjustments

The package fixture used `image.show(...)`, but current Arcweft presentation calls use `image(...)`. The fixture was corrected to the current surface syntax.

The package fixture referenced `asset.zundamon.normal` without a bundle virtual asset. Current bundle image asset inventory is sourced from `.arcweft/asset/**`, so the fixture now includes `.arcweft/asset/zundamon/normal.png`. The file uses the repository's existing `web/assets/generated-character.png` bytes because this sequence validates responsive placement, not a specific character art source.

## Structural audit note

The structural audit report is recorded under `docs/implementation/structure-audits/seq-06-14-responsive-stage-placement-2026-07-03/`.

Current audit summary:

- files scanned: 2221
- Rust files: 1076
- Rust physical LOC: 505727
- package manifests: 91
- violations: 3 existing error(s), 126 warning(s)

New/changed seq06.14 modules are below error thresholds. `crates/arcweft-layout/src/stage_placement.rs` is 868 physical LOC, and `crates/arcweft-cli/src/app/bundle/stage_placement.rs` is 175 physical LOC. `crates/arcweft-cli/src/app/bundle.rs` remains an existing large-file warning at 2115 physical LOC, so this cut moved the new placement parsing responsibility out of that file instead of adding more code there.
