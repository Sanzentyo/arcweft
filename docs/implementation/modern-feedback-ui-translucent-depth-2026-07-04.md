# Modern Feedback UI Translucent Style And Depth

## Scope

This slice extends the seq06.16.4 runtime-control style bridge with authored
control depth and updates `samples/modern-feedback-ui` to exercise the path with
a bundled background image, translucent controls, rounded corners, and
`box-shadow`.

The background asset at
`samples/modern-feedback-ui/src/.arcweft/asset/bg/glass_lights.png` is a
deterministic Arcweft-authored sample image. It is included under the repository
sample terms and does not depend on external image licensing.

## Implemented

- `UiRuntimeControlVisualStyle` now carries `depth_milli`.
- Runtime control style resolution accepts `depth`, `depth-milli`, and
  `z-index`.
- Player-scene style lowering preserves control depth into
  `RenderControlVisualStyle`.
- The shared frame planner sorts TextField/TextArea/Button runtime controls
  together by resolved depth, then kind, then source order.
- `samples/modern-feedback-ui` now renders `image.modern.glass_bg` behind the
  controls and uses style-authored transparent fills, opacity, shadows, rounded
  corners, and z-index.

## Current CSS Support In This Sample

- Supported through the runtime-control style path:
  - `background-color` / `rgba(...)` alpha;
  - `opacity`;
  - `color`, `selection-color`, `caret-color`;
  - `border-color`, `border-width`, `border-radius`;
  - `focus-ring-color`, `focus-ring-width`, `focus-ring-offset`;
  - `box-shadow`;
  - `depth` / `z-index`.
- Not supported for runtime controls yet:
  - `backdrop-filter`;
  - CSS `filter: blur(...)`;
  - per-control backdrop sampling or glass blur behind TextField/TextArea/Button.

## Backdrop Blur Gap

Runtime controls are currently planned as overlay rectangles, shadow plans, and
text blocks after the image/background pass. They do not render as retained UI
compositor nodes and therefore have no typed backdrop-filter effect or backdrop
texture sampling step. The lower compositor already has blur/backdrop concepts,
but player-owned runtime controls do not enter that path.

The follow-up design request is
`docs/reviews/requests/2026-07-04-seq-06.16.4.1-runtime-control-backdrop-filter.md`.

## Validation

- `cargo test -p arcweft-bundle --test runtime_control_style_resolution`
- `cargo test -p arcweft-player-scene --test runtime_control_style_lowering`
- `cargo test -p arcweft-render-wgpu --test geometry_runtime_control_styles`
- `cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-ui/arcw.toml`
- `cargo run -p arcweft-cli -- bundle samples/modern-feedback-ui/src/main.arcw --output target/arcweft/modern-feedback-ui-translucent.awfb`
- `cargo run -p arcweft-cli --features native-capture -- agent observe samples/modern-feedback-ui/src/main.arcw --json --image png --capture color --content-policy-mode local-dev --out target/modern-feedback-ui/transparent-depth-observe.png --mode drain --steps 4 --max-ops 64`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The native observe report produced `diagnostics_count=0` and wrote
`target/modern-feedback-ui/transparent-depth-observe.png`.
The structural audit reported `0 error(s)` and existing warning-level hotspots.

## Remaining TODOs

- Add typed runtime-control backdrop-filter / blur support after the follow-up
  request defines the contract and rendering order.
- Capture native/web visual smoke artifacts for this exact sample if this slice
  becomes a pinned visual baseline milestone.

## Design Deviations

None for the implemented scope. Backdrop blur is intentionally not treated as
implemented because no runtime-control backdrop-filter contract exists yet.
