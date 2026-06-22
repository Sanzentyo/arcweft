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
