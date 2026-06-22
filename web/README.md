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

## Playwright WebGPU smoke

Browser smoke tests run through Deno and Playwright. They use the installed
Chrome channel by default so Deno is not asked to run Playwright's npm browser
downloader. From this directory:

```bash
deno task test
```

`test` synchronizes Playwright through Deno, starts a local static host, and runs
the `tests/` smoke script (requires WebGPU). Set `ARW_PLAYWRIGHT_CHANNEL` to use
another installed Playwright browser channel.

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
