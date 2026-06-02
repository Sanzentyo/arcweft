# Request: Browser WebGPU Math Best Practices

## Request

Please research current best practices for implementing and benchmarking
browser WebGPU compute acceleration for Arcweft's runtime math path.

We need advice that can be turned into small, verifiable Rust/Wasm commits.
Focus on `wgpu` + `wasm32-unknown-unknown` browser execution, dense `f32`
matrix/tensor kernels, async GPU dispatch, readback, fallback policy, and
benchmarks that avoid host-specific paths.

## Project Context

Arcweft is a Rust narrative engine. Relevant layers:

- `arcweft-core`: Sans I/O runtime values, dense matrix/tensor data, VM.
- `arcweft-runtime-accelerator`: runtime pure helper acceleration, native
  math accelerator selection, browser WebGPU math adapter.
- `arcweft-lang-jit-cranelift`: native-only Cranelift JIT adapter.
- `arcweft-cli`: native CLI, bench/profile/test surfaces.
- future browser player: Wasm VM + browser adapters for fetch/storage/audio/UI
  and WebGPU/WebGL.

Current implementation state:

- Native math acceleration supports scalar, glam, ndarray, and native wgpu.
- Native wgpu uses synchronous/blocking readback and is selected only for
  non-`wasm32` targets.
- `native-jit` is target-specific to non-`wasm32`; browser builds do not link
  Cranelift.
- `arcweft-runtime-accelerator` now has a `wasm32 + math-wgpu` async
  `browser_webgpu` module with:
  - `BrowserWebGpuMathContext::new().await`
  - `matmul_f32(...).await`
  - `matrix_add_f32(...).await`
  - `tensor_add_f32(...).await`
  - basic transfer/readback counters
- The browser WebGPU adapter compiles for:

```bash
cargo check -p arcweft-runtime-accelerator --target wasm32-unknown-unknown --all-features
```

It has not yet been exercised in a real browser or compared against browser CPU
execution.

## Constraints

- Keep `arcweft-core` Sans I/O. Browser APIs, WebGPU objects, JS glue, timers,
  browser feature detection, and storage must stay in adapter/player crates.
- Do not use `unsafe`, unstable Rust, deprecated APIs, compatibility shims, or
  broad parser/compiler compatibility layers.
- Do not record host absolute paths in docs, logs, JSON, snapshots, or bench
  output.
- Preserve deterministic VM semantics. GPU acceleration is an optimization and
  must be correctness-checked against CPU/VM outputs for representative inputs.
- `f64` browser WebGPU kernels are not required unless current browser WebGPU
  support makes portable `f64` compute realistic. Treat `f32` as the first
  target.
- Prefer explicit async adapter boundaries over blocking browser execution.
- Avoid forcing browser-only dependencies into native-only paths.

## Questions To Research

1. WebGPU and `wgpu` browser setup

   Please confirm the current recommended `wgpu` feature set and target setup
   for browser WebGPU compute:

   - `wgpu/webgpu`, `wgpu/wgsl`, `std`, and any required `web-sys` features.
   - `wasm32-unknown-unknown` vs other browser Wasm targets.
   - browser permission/security requirements such as secure context,
     cross-origin isolation, and worker constraints.
   - whether `InstanceDescriptor::new_without_display_handle()` is still the
     right shape for compute-only browser WebGPU.
   - recommended `Limits` for portable compute kernels.

2. Async dispatch and readback

   Please review the best current pattern for:

   - awaiting adapter/device creation,
   - submitting compute work,
   - waiting for mapped readback buffers in browsers,
   - avoiding deadlocks or no-op `Device::poll` assumptions on WebGPU,
   - surfacing async GPU errors to Rust/Wasm callers.

3. Buffer lifetime and reuse

   We need guidance on:

   - when to allocate fresh upload/output/readback buffers,
   - when to keep persistent buffers,
   - how to safely reuse bind groups and staging buffers in browser WebGPU,
   - whether mapped readback buffers should be pooled,
   - how to avoid hidden CPU copies at JS/Wasm/WebGPU boundaries.

4. Performance expectations

   Please estimate and validate the workload sizes where browser WebGPU is
   expected to beat browser CPU/Wasm for:

   - matrix multiplication,
   - matrix add,
   - tensor add,
   - repeated dispatches with persistent input buffers,
   - one-shot calls with upload and readback included.

   Include guidance for when `Auto` should keep work on CPU instead of WebGPU.

5. Benchmark harness

   Please propose a browser benchmark harness that can run locally and in CI
   where available:

   - browser target build command,
   - JS/Wasm glue shape,
   - WebGPU feature detection,
   - warmup/sample/iteration model,
   - correctness checks,
   - path-free JSON output schema,
   - browser CPU baseline,
   - WebGPU one-shot and prepared-buffer modes,
   - how to run in Chrome/Edge/Firefox/Safari or skip cleanly.

6. Player integration

   Please advise how Arcweft should integrate async browser WebGPU math with
   the VM/player:

   - should `math.*` calls remain synchronous in the VM and offload through a
     task/Need boundary,
   - should browser WebGPU math be selected only at adapter task boundaries,
   - how to keep deterministic replay when GPU work resolves later,
   - how to report fallback reasons and counters in browser profile JSON,
   - how to keep native CLI behavior unchanged.

7. Shader/kernel shape

   Please review whether the current simple WGSL kernels are appropriate:

   - row-major `f32` matmul,
   - `f32` elementwise add,
   - workgroup sizes `16x16` for matmul and `256` for add,
   - indexing and bounds checks,
   - alignment and storage-buffer layout,
   - precision and deterministic comparison tolerances.

## Expected Answer Format

Please provide findings in priority order:

1. Finding title.
2. Current best-practice recommendation with sources.
3. Affected Arcweft crates/modules.
4. Concrete implementation plan.
5. Tests or browser benchmarks to add.
6. Expected performance impact and risk.

For browser support claims, please include dates and browser/version context.
For `wgpu` claims, cite the current `wgpu` documentation or source version used
for the recommendation.

## Non-Goals

- Do not recommend runtime JIT in browsers.
- Do not recommend moving browser APIs into `arcweft-core`.
- Do not recommend silently accepting missing WebGPU support. Missing WebGPU
  should be a structured skip/fallback reason.
- Do not recommend absolute paths in benchmark output.
- Do not recommend `unsafe` unless there is no safe alternative and the boundary
  is explicitly justified.
- Do not recommend adding compatibility shims or deprecated APIs.

