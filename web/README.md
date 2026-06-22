# Browser bootstrap boundary

This directory contains only:

- Wasm module initialization,
- `.awfb` and project font byte loading,
- a winit-owned canvas host,
- loading and fatal-error surfaces,
- diagnostic observation wiring for tests.

It intentionally contains no speaker element, dialogue element, choice button,
rich DOM renderer, Canvas 2D renderer, WebGL fallback, or normal game-layout CSS.
Game rendering and interaction live in Rust through `arcweft-render-wgpu`,
`arcweft-render-web`, and Arcweft presentation hit-testing.

## Audio Worklet

Serve `arcweft-microphone-worklet.js` with JavaScript MIME type from the same
trusted application origin. The browser player should pass its resolved URL to
the Rust microphone adapter only after an explicit Arcweft microphone request.
The baseline worklet transfers PCM blocks over `MessagePort`; output playback
uses the Rust audio host boundary.

## Playwright WebGPU smoke

Browser smoke tests run through npm and Playwright. They use the installed
Chrome channel by default. From this directory:

```bash
npm.cmd install
npm.cmd test
```

`test` starts a local static host and runs the Playwright smoke script (requires
WebGPU). The smoke checks canvas-only rendering, semantic input, and the
typed frame observation summary used by the native/Web parity tests. Set
`ARW_PLAYWRIGHT_CHANNEL` to use another installed Playwright browser channel.

## Manual launch

From the repository root:

```bash
cargo +nightly -Zscript tools/generate-webgpu-demo-assets.rs
cargo run -p arcweft-cli -- bundle web/demo.arcw --output web/demo.awfb
cargo build -p arcweft-player-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg --out-name arcweft_player_web target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm
```

Then from this directory:

```bash
deno task serve
```

Open `http://127.0.0.1:4173/index.html` in Chrome with WebGPU enabled.

## Native/Web Pixel Parity

The full local parity route rebuilds the bundle, compiles the Wasm player,
regenerates wasm-bindgen glue, captures the shared renderer through native
offscreen WebGPU, captures the browser canvas through Playwright, and enforces
the approved PNG metric thresholds for focus, hover, pressed, compact viewport,
and HiDPI scale-factor checkpoints:

```bash
just webgpu-parity
```

The parity checkpoint names are:

- `focus-first-choice`
- `hover-second-choice`
- `press-first-choice`
- `compact-focus-first-choice`
- `hidpi-focus-first-choice`

The equivalent manual capture command shape from the repository root is:

```bash
cargo +nightly -Zscript tools/capture-webgpu-native-frame.rs --checkpoint focus-first-choice --output target/webgpu-parity/native-focus-first-choice.png --visual-time-millis 160 --target-format rgba8unorm
```

Then from this directory:

```bash
$env:ARW_WEB_PARITY_DIR = (Resolve-Path ..\target\webgpu-parity).Path
$env:ARW_WEB_PARITY_CHECKPOINTS = "focus-first-choice,hover-second-choice,press-first-choice,compact-focus-first-choice,hidpi-focus-first-choice"
npm.cmd test
```

Compare the two captures:

```bash
cargo +nightly -Zscript tools/verify-webgpu-parity.rs --native target/webgpu-parity/native-focus-first-choice.png --web target/webgpu-parity/web-focus-first-choice.png --report target/webgpu-parity/parity-focus-first-choice.json
imq compare target/webgpu-parity/native-focus-first-choice.png target/webgpu-parity/web-focus-first-choice.png --format json --output target/webgpu-parity/imq-focus-first-choice.json
```
