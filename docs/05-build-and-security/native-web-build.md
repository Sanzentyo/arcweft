# native / web build

Arcweft runtime target strategy:

- Typed IR / bytecode VM is the semantic source of truth.
- Native product ships an AOT compiled player plus an embedded `.awfb` / bytecode / asset bundle.
- Web product ships an AOT compiled Wasm player plus a bytecode / asset bundle.
- Native Cranelift JIT is optional and limited to pure deterministic functions.
- Wasmtime is a native plugin/activity sandbox backend, not the main runtime.
- Full script AOT through generated Rust or generated Wasm helpers is a later release backend.

## Native

```text
winit main thread
wgpu RenderOwner
ServoViewHost optional
Audio backend
Engine::step
Scheduler
Worker pool
```

Feature:

```toml
native = ["native-st", "wgpu-render", "audio-native"]
native-mt = ["tokio", "rayon"]
native-jit = ["arcweft-lang-jit-cranelift"]
servo-view = ["servo"]
```

The native player reads bundles and assets through platform adapters. Core/runtime data structures only receive decoded bytes, manifests, bytecode, task results, and input events.

## Web

```text
Browser DOM
canvas / wgpu WebGPU/WebGL
Engine::step
WebAudio
DOM View backend
cooperative jobs
optional worker pool
```

Build modes:

```bash
arcw build web --mode single-thread
arcw build web --mode threads
arcw build web --mode both
```

Web thread support requires runtime detection and fallback.

```js
const supportsThreads =
  typeof SharedArrayBuffer !== "undefined" &&
  self.crossOriginIsolated === true &&
  navigator.hardwareConcurrency > 1;
```

## Web artifacts

```text
dist/web/
  arcweft_player_st.wasm
  arcweft_player_mt.wasm
  loader.js
  game.awfb
```

The browser player does not use runtime JIT or Wasmtime. It runs the same
bytecode VM compiled to Wasm, with browser adapters for fetch/cache/storage,
WebAudio, DOM View, and WebGPU/WebGL. Future build-time AOT Wasm helpers may be
emitted for pure functions, but flow/dialogue/choice/Need semantics remain
VM-defined.

`arcweft-runtime-accelerator` uses compile-time target selection for this
boundary. The `native-jit` feature is a native-only dependency edge to
`arcweft-lang-jit-cranelift`; on `wasm32` the accelerator crate compiles with
the same public runtime backend modes, but JIT selection resolves to VM/AOT
execution instead of linking Cranelift. Native wgpu math kernels are selected
only for non-`wasm32` targets because they use blocking readback. Browser WebGPU
math is exposed separately as an async `browser_webgpu` adapter for dense
`f32` matrix/tensor kernels, so browser player code can await GPU work at the
adapter boundary and then feed deterministic dense values back into the VM.
The adapter exposes borrowed `BrowserWebGpuMathRequest` values and typed
`BrowserWebGpuMathResponse` results, letting browser host code dispatch
`math.matmul_f32`, `math.matrix_add_f32`, and `math.tensor_add_f32` without
stringly typed operation switches or pre-dispatch dense-buffer copies.
The browser adapter uses WebGPU-only `wgpu` features (`std`, `webgpu`, `wgsl`)
and does not inherit native DX12/Vulkan/Metal/GLES backend features.
Browser players choose math acceleration through an explicit
`BrowserWebGpuMathAutoPolicy`. The default policy is conservative; embeddings
may instead construct `cpu_only()` for replay/diagnostics,
`explicit_webgpu_resident()` for product profiles that deliberately force
resident WebGPU when limits allow it, or `harness_capacity_matmul(...)` when a
benchmark is probing overprovisioned capacity. Environment-specific tuning must
stay in benchmark/profile evidence and must not silently change the default
policy.
LSP/tooling runner diagnostics use `RuntimeHostCapabilities` presets from
`arcweft-runtime-host`: native embeddings use `standard_native()`, while web
embeddings use `browser_web()` and add only the concrete adapter manifests they
actually implement. These presets describe host-task completion surfaces, not
math accelerator availability; WebGPU math remains an async adapter capability.

## WebGPU / WebGL fallback

- portable shader は WGSL + WebGPU limits。
- WebGL fallback は別 shader / Vello Hybrid / CPU raster。
- GPU object は thread 間共有しない。
- runtime math acceleration on WebGPU requires `math-wgpu` and browser WebGPU
  availability. The async adapter reports creation/readback errors instead of
  silently falling back inside the GPU call; product players decide whether to
  retry with VM/AOT CPU execution.
- Browser math GPU calls return structured availability/fallback reasons such
  as insecure context, missing `navigator.gpu`, adapter/device failure, storage
  buffer limit overflow, workgroup limit overflow, validation error, device
  loss, and map failure.
- Browser WebGPU uses `Limits::default()` and validates shape, byte length, and
  dispatch dimensions before allocation. WebGL is not treated as a compute
  fallback.
- Browser benchmarks are exported from `arcweft-browser-bench` and report
  path-free JSON for CPU Wasm, one-shot WebGPU, prepared upload, and prepared
  resident modes.



## Web device APIs

WebUSB, WebHID, and Web Serial are exposed through `web-sys` in `arcweft-device-web`.

Some `web-sys` device APIs are unstable and require `--cfg=web_sys_unstable_apis`; generated build reports must state this explicitly. Web device access is not assumed to be available in all browsers, so projects must define fallback View and virtual-device test paths.

Required feature hints are generated from device profiles, for example:

```toml
web-sys = { features = [
  "Usb", "UsbDevice", "UsbDeviceRequestOptions",
  "Hid", "HidDevice",
  "Serial", "SerialPort"
] }
```
