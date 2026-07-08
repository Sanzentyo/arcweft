# Native runner and shared renderer migration

The WebGPU player is not allowed to fork runtime semantics or invent a browser-only renderer. Native migration therefore has two parity tracks: **runtime parity** and **visual/geometry parity**.

## Stage A — portable runtime session parity (Commit 2)

1. Introduce `arcweft-runtime-driver::BundleSession` without changing the existing native CLI/player path.
2. Build one `.awfb` fixture and execute it through:
   - current `arcweft-runtime-host::run_bundle_with_native_adapters`, and
   - the new `BundleSession` plus a native host-task broker.
3. Compare, step by step:
   - stop reason and final `FlowFiberStatus`,
   - ordered `FlowEvent` values,
   - diagnostics,
   - requested task IDs/calls/cancel scopes/source-close requests,
   - resolved `LineDisplayFrame`,
   - choice IDs and labels.
4. Preserve request order as `(logical_epoch, sequence, task_id)`. Do not keep `LogicalEpoch(0)` as an undocumented assumption.
5. Move `arcweft-player-native::run_bundle_headless` to the portable session only after parity is green.
6. Remove duplicated entry/flow selection and display-frame resolution from the native runner after all call sites move.

## Stage B — extract the shared GPU renderer (Commit 3)

The current `arcweft-render-native` mixes shared visual work with winit, surface ownership, platform backend features, wall-clock metrics, mpsc readback, and `Device::poll(Wait)`. Split it directly:

```text
arcweft-render-wgpu
  geometry / visual plan
  text layout + glyphon
  image/View pipelines
  pipeline/bind-group/resource caches
  render(Device, Queue, TextureView, viewport, frame)

arcweft-render-native
  winit window/event loop
  native surface configuration
  native backend feature selection
  blocking capture/readback (native-only)
```

No compatibility wrapper should keep a second renderer implementation alive. Move the implementation and update all call sites.

## Stage C — native visual regression gate

Before connecting the Web host:

1. Run existing native rich-text screenshots/goldens with the exact same fixed font bytes.
2. Add deterministic fixtures for:
   - dialogue panel and text,
   - choice layout,
   - hover, pressed and visible keyboard focus,
   - resize and scale-factor geometry,
   - image/View display-list paths already supported by native.
3. Compare pre-extraction and post-extraction captures. Any change must be either fixed or explicitly approved with a documented tolerance.
4. Assert that renderer-produced semantic bounds and `HitTree` bounds match the pixels/layout used for each choice.
5. Keep native capture in `arcweft-render-native`; shared renderer must not gain blocking readback.

## Stage D — native/Web semantic and visual parity

After `arcweft-render-web` exists:

1. Generate the `.awfb` once in CI.
2. Register identical font bytes in both hosts.
3. Replay identical logical tick/dt and semantic input sequences.
4. Compare runtime event/status hashes exactly.
5. Capture native and browser frames at agreed checkpoints. Browser readback is a test-only asynchronous adapter, never a blocking shared-renderer API.
6. Compare normalized screenshots with a small, documented GPU-raster tolerance while requiring exact geometry/hit bounds.
7. Resize both hosts to the same logical viewport/DPI pair and re-run hit-test parity.

## Completion gate

Native migration is complete only when:

- native player uses `BundleSession`,
- native surface host uses `arcweft-render-wgpu`,
- no shared renderer imports winit/web-sys/filesystem,
- native regression is green,
- browser WebGPU smoke is green,
- semantic parity is exact,
- visual parity differences are reviewed and bounded.

## Current WebGPU browser cut — 2026-06-22

Implemented in the current checkout:

- `arcweft-player-web` boots the browser player through wasm-bindgen, winit, and
  `arcweft-render-web`; JavaScript only fetches bytes and surfaces fatal/status
  events.
- `web/demo.arcw` declares generated background, character stand, GIF, and WebP
  assets, and `web/demo.awfb` is regenerated from that source.
- `tools/generate-webgpu-demo-assets.rs` is a Rust script with embedded Cargo
  metadata that generates and validates the PNG/GIF/WebP fixtures.
- Web smoke execution uses `npm test` and Playwright against an installed Chrome
  channel.
- Bundle-declared image objects are lowered into the portable presentation
  snapshot, decoded from packaged PNG/GIF/WebP assets in the browser host, and
  rendered through the shared WebGPU scene.
- The browser emits a typed frame observation summary from `PreparedFrame`, and
  the native/headless test path builds the same summary from `web/demo.awfb` to
  pin shared geometry, text, choice, and image-object parity before pixel
  readback is connected.
- `tools/capture-webgpu-native-frame.rs` renders the same `web/demo.awfb`
  `PreparedFrame` through the shared renderer into a native offscreen PNG with
  an explicit render-target format.
  `web/tests/webgpu-canvas-smoke.mjs` can write the browser canvas PNG when
  `ARW_WEB_PARITY_DIR` is set. `tools/verify-webgpu-parity.rs` enforces the
  approved PNG metric thresholds, and `imq compare` records full-reference
  image metrics for the two outputs.
- `just webgpu-parity` rebuilds the bundle, compiles the Wasm player,
  regenerates wasm-bindgen glue, captures native/browser PNG artifacts for
  `focus-first-choice`, `hover-second-choice`, `press-first-choice`, and
  `compact-focus-first-choice`, plus the `hidpi-focus-first-choice` scale-factor
  checkpoint, runs the Playwright WebGPU smoke with npm, enforces the
  Rust-script parity thresholds, and writes `imq` reports.
- The shared renderer now draws the background rectangle, authored image quads,
  UI rectangles/focus ring, and text in that order. This keeps image objects
  behind dialogue/choice interaction visuals while preserving the shared
  renderer as the only game renderer.
- The Windows/headless Chrome WebGPU smoke passed on this machine with the
  installed Chrome channel and D3D11 ANGLE:

```bash
npm.cmd --prefix web test
```

Follow-up boundaries:

- Native/Web screenshot parity now has capture producers, enforced Rust-script
  thresholds, and `imq` report paths for focus, hover, pressed, and compact
  viewport checkpoints plus a HiDPI scale-factor checkpoint. The initial
  thresholds are the approved Windows/Chrome WebGPU tolerance for this cut and
  should be tightened as browser readback/color-management behavior is made more
  deterministic.
- The current parity summary proves shared planner output and the PNG comparison
  now bounds final GPU pixels for the 1280x720 focus/hover/pressed checkpoints
  plus a 960x540 compact viewport checkpoint and a 640x360 physical /
  320x180 logical / 2.0 scale-factor checkpoint. More browser/OS matrix entries
  can still be added later, but Windows local scale-factor coverage is now in
  the gate.
- On this Windows machine, the native offscreen PNGs and Playwright canvas PNGs
  passed `tools/verify-webgpu-parity.rs` and were compared with
  `imq compare --format json`; the recorded Rust-script metrics were:
  focus-first-choice PSNR 24.0221 dB / SSIM 0.7928 / MSE 0.003961,
  hover-second-choice PSNR 24.1604 dB / SSIM 0.8153 / MSE 0.003837,
  press-first-choice PSNR 23.9857 dB / SSIM 0.7790 / MSE 0.003994, and
  compact-focus-first-choice PSNR 21.4657 dB / SSIM 0.7345 / MSE 0.007136,
  and hidpi-focus-first-choice PSNR 20.0349 dB / SSIM 0.8295 / MSE 0.009920.
- Richer image lifecycle semantics such as explicit hide/clear and authored
  layer ordering remain future presentation-model work, not a blocker for the
  current WebGPU-first browser player cut.
