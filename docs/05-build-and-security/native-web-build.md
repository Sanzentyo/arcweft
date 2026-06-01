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
ServoUiHost optional
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
servo-ui = ["servo"]
```

The native player reads bundles and assets through platform adapters. Core/runtime data structures only receive decoded bytes, manifests, bytecode, task results, and input events.

## Web

```text
Browser DOM
canvas / wgpu WebGPU/WebGL
Engine::step
WebAudio
DOM UI backend
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
WebAudio, DOM UI, and WebGPU/WebGL. Future build-time AOT Wasm helpers may be
emitted for pure functions, but flow/dialogue/choice/Need semantics remain
VM-defined.

`arcweft-runtime-accelerator` uses compile-time target selection for this
boundary. The `native-jit` feature is a native-only dependency edge to
`arcweft-lang-jit-cranelift`; on `wasm32` the accelerator crate compiles with
the same public runtime backend modes, but JIT selection resolves to VM/AOT
execution instead of linking Cranelift. Native wgpu math kernels are likewise
selected only for non-`wasm32` targets. Browser WebGPU compute will need its own
async adapter and benchmark harness before it can be treated as an enabled
runtime math accelerator.

## WebGPU / WebGL fallback

- portable shader は WGSL + WebGPU limits。
- WebGL fallback は別 shader / Vello Hybrid / CPU raster。
- GPU object は thread 間共有しない。



## Web device APIs

WebUSB, WebHID, and Web Serial are exposed through `web-sys` in `arcweft-device-web`.

Some `web-sys` device APIs are unstable and require `--cfg=web_sys_unstable_apis`; generated build reports must state this explicitly. Web device access is not assumed to be available in all browsers, so projects must define fallback UI and virtual-device test paths.

Required feature hints are generated from device profiles, for example:

```toml
web-sys = { features = [
  "Usb", "UsbDevice", "UsbDeviceRequestOptions",
  "Hid", "HidDevice",
  "Serial", "SerialPort"
] }
```
