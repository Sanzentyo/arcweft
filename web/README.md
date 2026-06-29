# Browser bootstrap boundary

This directory contains only:

- Wasm module initialization,
- `.awfb` and project font byte loading,
- a winit-owned canvas host,
- loading and fatal-error surfaces,
- diagnostic observation wiring for tests.

It intentionally contains no speaker element, dialogue element, choice button,
rich DOM renderer, Canvas 2D renderer, WebGL fallback, normal game-layout CSS, or
visible DOM text-input sample UI. Game rendering and interaction live in Rust
through `arcweft-render-wgpu`, `arcweft-render-web`, and Arcweft presentation
hit-testing.

## Player-rendered Web IME sample

The active Web IME sample is canvas/WebGPU-rendered:

```bash
just ime-sample-web
```

Open:

```text
http://127.0.0.1:8786/ime-sample.html
```

Equivalent normal player URL:

```text
http://127.0.0.1:8786/index.html?bundle=./ime-player-rendered.awfb
```

`web/ime-sample.html` is a thin host page. It contains no visible DOM textbox,
text mirror, CSS caret, selection/composition spans, or sample status cards. The
visible `TextField`, `TextArea`, `SecureField`, focus ring, text, mask, caret,
selection, and composition evidence are Arcweft-rendered in the canvas.

`EditContext` remains an invisible browser adapter object owned by
`web/player-editcontext.js` and synchronized from
`PreparedFrame::focused_text_input_target()` through the normal Web player
runtime bridge.

Build the fixture and Wasm manually:

```bash
cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --out web/ime-player-rendered.awfb
cargo build -p arcweft-player-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg --out-name arcweft_player_web target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm
npm --prefix web run test:ime
```

## Audio Worklet

Serve `arcweft-microphone-worklet.js` with JavaScript MIME type from the same
trusted application origin. The browser player should pass its resolved URL to
the Rust microphone adapter only after an explicit Arcweft microphone request.

## Playwright WebGPU smoke

Browser smoke tests run through npm and Playwright. They use the installed Chrome
channel by default. From this directory:

```bash
npm.cmd install
npm.cmd test
```

`test` starts a local static host and runs the Playwright smoke script (requires
WebGPU). The smoke checks canvas-only rendering, semantic input, and the typed
frame observation summary used by the native/Web parity tests. Set
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
