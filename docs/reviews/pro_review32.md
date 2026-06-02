I treated the uploaded Arcweft context as binding: keep `arcweft-core` Sans I/O; keep browser APIs, WebGPU objects, timers, JS glue, and feature detection in adapter/player crates; treat GPU math as an optimization checked against CPU/VM outputs; and keep browser JIT out of scope.  I also applied the project premise that Arcweft structure and philosophy should be considered first for this work.  The `wgpu` references below are based on current docs.rs for `wgpu` **29.0.3**, whose crate docs describe WebGPU/WebGL-on-Wasm support and the current feature layout. ([Docs.rs][1])

## 1. Browser WebGPU setup should be narrow, explicit, and cfg-isolated

**Recommendation.** Use a browser-only `wgpu` dependency configuration for the browser math adapter, not the native `wgpu` feature set. For `wasm32-unknown-unknown`, prefer:

```toml
[target.'cfg(all(target_arch = "wasm32", target_os = "unknown"))'.dependencies]
wgpu = { version = "29", default-features = false, features = ["std", "webgpu", "wgsl"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
  "Window",
  "Navigator",
  "WorkerGlobalScope",
  "WorkerNavigator",
  "Performance",
  "console",
] }
```

`wgpu`’s current default features include native backends plus `webgpu`, `wgsl`, and `std`, but the browser adapter should spell out only `std`, `webgpu`, and `wgsl` to avoid pulling native-only backends into the Wasm build. The `webgpu` feature pulls the browser WebGPU backend and relevant `web-sys` pieces, while `wgsl` is the portable shader path; `wgpu`’s crate docs state that WebGPU supports WGSL and that the `webgpu` backend is enabled on WebAssembly. ([Docs.rs][2])

Use `wasm32-unknown-unknown` as the browser Wasm target. The Rust target is intentionally minimal and makes few host assumptions; the wasm-bindgen guide is specifically designed around `wasm32-unknown-unknown`, and it does not support the Emscripten Wasm targets. Browser-specific behavior still requires JS glue, because the Rust target alone does not prove that the Wasm is running in a web browser. ([Rust and WebAssembly][3])

Security and support policy should be explicit:

* Require a **secure context** for WebGPU. Local development on `localhost`/`127.0.0.1` is acceptable in Chromium’s documented setup path.
* Do **not** require cross-origin isolation for basic single-threaded WebGPU math. Record `crossOriginIsolated`, but only require COOP/COEP when enabling Wasm threads, `SharedArrayBuffer`, or high-resolution-memory/timer features.
* Support both main-thread and dedicated-worker creation, using `Navigator.gpu` or `WorkerNavigator.gpu`. Do not share WebGPU handles across workers.
* Missing `navigator.gpu`, `requestAdapter()` returning `null`, or device creation failure must produce a structured skip/fallback reason, never a silent CPU fallback. ([MDNウェブドキュメント][4])

For browser/version context: as of the web.dev browser-support article updated **November 25, 2025**, WebGPU is officially supported across major browsers, with baseline entries listed as Chrome 113, Edge 113, Firefox 141, and Safari 26, but platform coverage still varies by OS/GPU/backend. That means Arcweft’s browser benchmark must treat WebGPU as “available when proven at runtime,” not as guaranteed from browser name alone. ([web.dev][5])

For compute-only browser WebGPU, `InstanceDescriptor::new_without_display_handle()` is still the right shape. `wgpu` documents this as the default descriptor without a display handle; display handles matter for presentation paths, especially GLES/windowing, not for compute-only browser kernels. Use WebGPU-only adapter selection and do not compile `webgl` as a math-compute fallback, because WebGL is exposed by `wgpu` as a GLES-style web backend, not a WebGPU compute substitute. ([Docs.rs][6])

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator`: `browser_webgpu` module and feature gating.
* Future browser player crate: WebGPU feature detection, timers, worker/main-thread setup.
* `arcweft-core`: no changes except perhaps pure CPU reference tests.

**Concrete implementation plan.**

1. Move browser `wgpu` dependencies behind `cfg(all(target_arch = "wasm32", target_os = "unknown"))`.
2. Use `default-features = false` with `["std", "webgpu", "wgsl"]` for the browser adapter.
3. Ensure native `wgpu` and Cranelift features remain `not(wasm32)` or native-feature-gated.
4. Construct the `wgpu::Instance` using `InstanceDescriptor::new_without_display_handle()`.
5. Request an adapter with `compatible_surface: None` and `force_fallback_adapter: false`.
6. Request the device with `required_features: Features::empty()` and `required_limits: Limits::default()` for the first portable compute implementation.
7. Add a `BrowserWebGpuAvailability` / `BrowserWebGpuFallbackReason` enum.

A practical fallback enum should include at least:

```rust
pub enum BrowserWebGpuFallbackReason {
    NavigatorGpuMissing,
    AdapterUnavailable,
    DeviceRequestFailed,
    RequiredLimitsUnsupported,
    StorageBufferTooLarge,
    BufferSizeTooLarge,
    WorkgroupCountTooLarge,
    ValidationError,
    OutOfMemory,
    InternalError,
    DeviceLost,
    MapFailed,
    CorrectnessMismatch,
    AutoCpuThreshold,
}
```

**Tests or browser benchmarks to add.**

* `cargo check -p arcweft-runtime-accelerator --target wasm32-unknown-unknown --all-features`
* A browser smoke test that reports structured skip reasons for:

  * insecure context,
  * no `navigator.gpu`,
  * no adapter,
  * device request failure,
  * insufficient limits.
* A worker smoke test using `WorkerNavigator.gpu`.

**Expected performance impact and risk.**

This commit is mostly correctness and portability work. It should reduce accidental native dependency leakage into browser builds and make unsupported browsers diagnosable. Risk is low, except for feature-gate churn around existing native `wgpu` paths.

---

## 2. Browser dispatch/readback must be callback/future-driven, not blocking

**Recommendation.** Keep the current async browser API shape:

```rust
BrowserWebGpuMathContext::new().await
context.matmul_f32(...).await
context.matrix_add_f32(...).await
context.tensor_add_f32(...).await
```

Do not try to port the native synchronous `wgpu` readback path into the browser. `wgpu::Device::poll` is documented as checking mapping callbacks and blocking on native when requested, but it is a **no-op on WebGPU** because browser devices are automatically polled. Browser readback must therefore await `map_async` completion through a callback-to-future bridge, not rely on `Device::poll(Wait)`. ([Docs.rs][7])

Best current browser readback pattern:

1. Create input buffers and output buffers.
2. Encode compute pass.
3. Copy output storage buffer into a readback buffer with `MAP_READ | COPY_DST`.
4. Submit the command buffer.
5. Call `readback.slice(..).map_async(MapMode::Read, callback)`.
6. Convert the callback to a Rust future via a one-shot channel or `wasm-bindgen-futures`.
7. Await the future.
8. Call `get_mapped_range()`, copy into a Rust `Vec<f32>`, drop the mapped view, then `unmap()`.

`wgpu`’s buffer docs make two details important for Arcweft: a buffer is either mapped for CPU access or unmapped for GPU access, never both; and mapped range views must be dropped before unmapping. The docs also note that browser WebGPU mapping can involve copies across Wasm linear memory, browser processes, driver IPC, and shared-memory buffers, so readback cost is real and should be measured separately from GPU kernel time. ([Docs.rs][8])

GPU errors should be surfaced through structured `Result` values. Use `Device::push_error_scope` guards around pipeline creation, buffer creation, bind group creation, command encoding, and submission, then `scope.pop().await` and convert validation, out-of-memory, or internal errors into Arcweft fallback/profile records. `wgpu`’s current API uses `ErrorScopeGuard`, and unhandled errors otherwise become panics by default. Also install `on_uncaptured_error` and `set_device_lost_callback` hooks in the browser context so the player can mark the GPU context unusable and fall back. ([Docs.rs][7])

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator::browser_webgpu`
* Browser profile/counter structures
* Future player async task/Need executor

**Concrete implementation plan.**

Add one small internal helper per readback type:

```rust
async fn map_readback_f32(
    device: &wgpu::Device,
    buffer: &wgpu::Buffer,
    byte_len: u64,
) -> Result<Vec<f32>, BrowserWebGpuError>
```

Implementation requirements:

* reject non-multiple-of-4 byte lengths,
* await `map_async`,
* convert map failure into `BrowserWebGpuError::MapFailed`,
* copy mapped bytes into `Vec<f32>` using safe `bytemuck` casts,
* drop the mapped range before `unmap`,
* increment `bytes_readback`, `map_count`, and `map_wait_ms`.

Add a second helper:

```rust
async fn scoped_gpu_call<T>(
    device: &wgpu::Device,
    label: &'static str,
    f: impl FnOnce() -> Result<T, BrowserWebGpuError>,
) -> Result<T, BrowserWebGpuError>
```

The actual API shape may need to account for `ErrorScopeGuard` lifetimes, but the behavior should be: push validation/out-of-memory scope, run the synchronous GPU setup/encode operation, pop and await, then return structured error if the scope catches anything.

**Tests or browser benchmarks to add.**

* Browser test: validation error is captured and returned, not panicked.
* Browser test: readback works for lengths `0`, `1`, `255`, `256`, `257`, and a multi-megabyte buffer.
* Browser test: device lost callback path is represented as a structured failure where the browser can trigger or simulate it.
* Regression test: no browser code calls a native blocking readback helper.

**Expected performance impact and risk.**

This avoids deadlocks and false assumptions in browsers. The cost is that every GPU result crossing back into the VM has an async boundary and a real copy. That is exactly why Arcweft should distinguish one-shot GPU calls from prepared/resident GPU paths.

---

## 3. Portable limits should be enforced before allocation and dispatch

**Recommendation.** Start with `Limits::default()` for browser compute. `wgpu` documents `Limits::default()` as guaranteed by WebGPU and modern APIs, and recommends requesting only the limits needed because asking for higher limits can reduce portability and potentially performance. The relevant default limits include a 128 MiB `max_storage_buffer_binding_size`, a 256 MiB `max_buffer_size`, 256 max compute invocations per workgroup, and 65,535 workgroups per dimension. ([Docs.rs][9])

Do not use `downlevel_webgl2_defaults` for browser math compute. Arcweft is not using WebGL2 for compute, and WebGPU compute should be treated as a separate capability with structured fallback if unavailable.

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator::browser_webgpu`
* Shared accelerator selection policy
* Browser profile JSON

**Concrete implementation plan.**

Add checked shape and byte-size validation before any GPU buffer creation:

```rust
fn checked_f32_bytes(len: usize) -> Result<u64, BrowserWebGpuFallbackReason> {
    len.checked_mul(core::mem::size_of::<f32>())
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(BrowserWebGpuFallbackReason::BufferSizeTooLarge)
}
```

For each operation:

* `matrix_add_f32`: require equal shapes and `len * 4 <= max_storage_buffer_binding_size`.
* `tensor_add_f32`: same as elementwise length.
* `matmul_f32`: require `a.len() == m*k`, `b.len() == k*n`, output `m*n`; use checked multiplication for all dimensions.
* Reject dispatch dimensions above `max_compute_workgroups_per_dimension`.
* Reject any individual buffer above `max_buffer_size`.
* Fast-path zero-sized outputs on CPU without touching GPU.

**Tests or browser benchmarks to add.**

* Unit tests for overflow-safe shape checks on native host.
* Wasm check tests that large shapes return `StorageBufferTooLarge` or `WorkgroupCountTooLarge`.
* Browser smoke test that records actual device limits in path-free JSON.

**Expected performance impact and risk.**

Limit checks add negligible overhead and prevent browser validation panics, device loss, and confusing CI failures. Risk is low. The main choice is whether to add chunking later; the first commits should fallback instead of chunking.

---

## 4. Split one-shot GPU calls from prepared-buffer GPU calls

**Recommendation.** Keep the current one-shot async methods for correctness and integration, but add prepared/resident APIs before relying on browser WebGPU for performance. `wgpu` docs note that `queue.write_buffer` uses temporary staging, `write_buffer_with` may avoid one allocation, and staging belts/custom staging can help with many small transfers; browser buffer mapping/readback still has unavoidable copy costs. ([Docs.rs][8])

One-shot API:

```rust
matmul_f32(a, b, m, k, n).await -> Vec<f32>
matrix_add_f32(a, b, rows, cols).await -> Vec<f32>
tensor_add_f32(a, b, shape).await -> Vec<f32>
```

Prepared API:

```rust
prepare_matmul_f32(capacity: MatmulCapacity) -> PreparedMatmulF32
PreparedMatmulF32::upload_inputs(...)
PreparedMatmulF32::dispatch(...)
PreparedMatmulF32::readback().await

prepare_elementwise_f32(capacity_len: usize) -> PreparedElementwiseF32
```

Cache these per `BrowserWebGpuMathContext`:

* `BindGroupLayout`
* `PipelineLayout`
* `ComputePipeline`
* shape/parameter buffers
* reusable input/output/readback buffers
* bind groups when buffer identities are unchanged

Readback buffers may be pooled, but only after `map_async` has completed, the mapped range is dropped, and `unmap()` has been called. Never submit a mapped buffer to the GPU.

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator::browser_webgpu`
* Accelerator counters
* Browser benchmark harness
* Player task scheduler later

**Concrete implementation plan.**

Commit sequence:

1. Add pipeline cache per op.
2. Add buffer-capacity helper:

   * exact-size for one-shot,
   * power-of-two or next-capacity reuse for prepared mode.
3. Add prepared elementwise add.
4. Add prepared matmul.
5. Add counters:

   * `pipeline_cache_hits`,
   * `buffer_alloc_count`,
   * `buffer_reuse_count`,
   * `bind_group_rebuild_count`,
   * `bytes_uploaded`,
   * `bytes_readback`,
   * `dispatch_count`.

Avoid hidden JS/Wasm copies in benchmarks by generating deterministic input arrays inside Wasm and passing only config/result JSON through JS. In the player, prefer Rust/Wasm-owned tensor memory as the source and use safe byte casts for `queue.write_buffer`; do not round-trip large tensors through JS `Float32Array`.

**Tests or browser benchmarks to add.**

* Prepared add produces identical output across repeated calls with same capacity.
* Prepared matmul handles shape changes within capacity.
* Reusing a readback buffer after unmap succeeds.
* Attempting to reuse while mapped is impossible by API structure.
* Benchmark modes:

  * CPU Wasm baseline,
  * WebGPU one-shot,
  * WebGPU prepared with upload per iteration,
  * WebGPU prepared/resident with readback only at boundary.

**Expected performance impact and risk.**

This is the first performance-critical architectural change. One-shot WebGPU calls will often lose to CPU for small or memory-bound work; prepared mode is what can make repeated math worthwhile. Risk is moderate because lifetime/state bugs are possible, but the API can keep this safe without `unsafe`.

---

## 5. Initial `Auto` policy should be conservative: matmul first, elementwise only when large or resident

**Recommendation.** Do not enable broad browser WebGPU `Auto` selection until Arcweft has browser benchmark data. WebGPU dispatch and readback overhead are large enough that small kernels can lose badly. A 2026 dispatch-overhead study measured API overhead alone in the tens of microseconds and found small dispatches can be overhead-dominated; browser readback also has extra Wasm/browser/driver copy costs. ([arXiv][10])

Use this initial policy as a safe starting point:

| Operation                           |                                                    One-shot upload + dispatch + readback |                                                                                Prepared/resident mode |
| ----------------------------------- | ---------------------------------------------------------------------------------------: | ----------------------------------------------------------------------------------------------------: |
| `matmul_f32`                        | CPU below roughly `2*m*n*k < 32_000_000` FLOP-equivalent or dimensions below about `128` |              Allow at lower threshold, especially repeated square-ish `128+` or repeated count ≥ 8–16 |
| `matrix_add_f32`                    |                        CPU by default unless `len >= 1_000_000` and benchmark proves win | GPU only if inputs are already resident, output feeds another GPU op, or `len >= 1_000_000–4_000_000` |
| `tensor_add_f32`                    |                                                                       Same as matrix add |                                                                                    Same as matrix add |
| immediate readback after trivial op |                                                                                      CPU |                                                    GPU only if part of a larger prepared/fused region |

These are deliberately estimates, not universal truths. They should live behind a policy table that the browser benchmark can validate and later tune by browser/GPU class.

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator` backend selection
* Browser profile JSON
* Benchmark harness

**Concrete implementation plan.**

Add:

```rust
pub enum BrowserMathBackendPolicy {
    CpuOnly,
    WebGpuOnly,
    AutoConservative,
    AutoBenchCalibrated,
}
```

For the first browser commits:

* default browser `Auto` to CPU for elementwise one-shot operations,
* allow WebGPU matmul only above conservative threshold and only after one correctness check,
* require WebGPU prepared mode for repeated dispatch experiments,
* expose `WebGpuOnly` only for tests/benchmarks, not silent production selection.

Profile every CPU decision with a structured reason:

```json
{
  "backend": "cpu_wasm",
  "reason": "auto_cpu_threshold",
  "op": "tensor_add_f32",
  "len": 65536
}
```

**Tests or browser benchmarks to add.**

* Policy unit tests for thresholds and fallback reasons.
* Browser benchmark validates break-even points for:

  * square matmul: 64, 128, 256, 512,
  * skinny/wide matmul,
  * add lengths from 256 to 4,194,304,
  * repeated prepared dispatch counts: 1, 4, 16, 64.
* Correctness checks run against CPU Wasm for every benchmark case.

**Expected performance impact and risk.**

Large matmul is the most likely early win. One-shot add is the most likely early loss. The main risk is over-selecting GPU and making the browser player slower; conservative `Auto` plus explicit profiling avoids that.

---

## 6. Add a browser benchmark harness before changing production defaults

**Recommendation.** Add a small browser benchmark target that uses the same Wasm CPU path and the same browser WebGPU adapter. Do not compare browser WebGPU against native CLI CPU numbers when deciding browser policy; that mixes host-specific paths and violates the goal of browser-relevant results. wasm-bindgen’s test tooling supports headless browser execution, including browser and worker modes, and `wasm-pack test` can drive Chrome, Firefox, and Safari where available. ([Rust and WebAssembly][11])

Build shape:

```bash
cargo build -p arcweft-browser-bench --release \
  --target wasm32-unknown-unknown \
  --features math-wgpu,browser-bench

wasm-bindgen --target web \
  --out-dir target/arcweft-browser-webgpu \
  --out-name arcweft_browser_webgpu \
  target/wasm32-unknown-unknown/release/arcweft_browser_bench.wasm
```

Export shape:

```rust
#[wasm_bindgen]
pub async fn run_arcweft_browser_math_bench(
    config_json: String,
) -> Result<JsValue, JsValue>;
```

JS feature-detection shape:

```js
if (!globalThis.isSecureContext) {
  return skip("insecure_context");
}

const gpu = globalThis.navigator?.gpu;
if (!gpu) {
  return skip("navigator_gpu_missing");
}

const adapter = await gpu.requestAdapter();
if (!adapter) {
  return skip("adapter_unavailable");
}
```

Use `localhost` for local serving; Chromium documents that local `localhost`/`127.0.0.1` development counts as a secure context for WebGPU setup. ([Chrome for Developers][12])

**Affected Arcweft crates/modules.**

* New `arcweft-browser-bench` crate or `tools/browser-webgpu-bench`
* `arcweft-runtime-accelerator` benchmark feature
* CI scripts

**Concrete implementation plan.**

Benchmark model:

* deterministic PRNG seed generated inside Wasm,
* CPU Wasm baseline,
* WebGPU one-shot mode,
* WebGPU prepared upload-per-iteration mode,
* WebGPU prepared/resident mode,
* 3–5 warmup iterations,
* 10–30 measured samples depending on size,
* report median, MAD, min, p95,
* correctness check CPU vs GPU for each case,
* no absolute paths in output.

Suggested shape set:

```text
add/tensor lengths:
0, 1, 255, 256, 257, 4096, 65536, 1048576, 4194304

matmul:
1x1x1
2x3x4
17x19x23
64x64x64
128x128x128
256x256x256
optional slow: 512x512x512
```

Path-free JSON schema:

```json
{
  "schema_version": "arcweft.browser_webgpu_bench.v1",
  "run": {
    "timestamp_utc": "2026-06-02T00:00:00Z",
    "arcweft_commit": "optional-short-sha-or-null",
    "browser": {
      "family": "chromium|edge|firefox|safari|unknown",
      "version": "optional"
    },
    "secure_context": true,
    "cross_origin_isolated": false,
    "webgpu": {
      "available": true,
      "limits": {
        "max_storage_buffer_binding_size": 134217728,
        "max_buffer_size": 268435456,
        "max_compute_invocations_per_workgroup": 256
      }
    }
  },
  "cases": [
    {
      "case_id": "matmul_f32_m256_k256_n256",
      "op": "matmul_f32",
      "shape": {"m": 256, "k": 256, "n": 256},
      "mode": "cpu_wasm|webgpu_one_shot|webgpu_prepared_upload|webgpu_prepared_resident",
      "warmup_iters": 5,
      "sample_iters": 30,
      "median_ms": 0.0,
      "mad_ms": 0.0,
      "min_ms": 0.0,
      "p95_ms": 0.0,
      "bytes_uploaded": 0,
      "bytes_readback": 0,
      "dispatches": 1,
      "buffer_alloc_count": 0,
      "buffer_reuse_count": 0,
      "correctness": {
        "passed": true,
        "max_abs": 0.0,
        "max_rel": 0.0
      },
      "fallback_reason": null
    }
  ],
  "skips": [
    {"scope": "webgpu", "reason": "adapter_unavailable"}
  ]
}
```

**Tests or browser benchmarks to add.**

* Headless Chrome smoke test where WebGPU is available or cleanly skipped.
* Firefox/Safari CI jobs marked optional/skip-cleanly because WebGPU support depends on browser version, platform, and runner GPU.
* Dedicated-worker benchmark variant.
* Regression test that JSON contains no absolute paths.

**Expected performance impact and risk.**

This does not improve runtime speed directly, but it prevents bad production defaults. The risk is CI variability; treat WebGPU absence as a benchmark skip, while CPU Wasm baseline and JSON schema validation should still pass.

---

## 7. VM/player integration should use an async task/Need boundary, not async core math

**Recommendation.** Keep `math.*` semantics synchronous and deterministic inside the VM/core. Browser WebGPU should be selected only at an adapter/player boundary where async work is already allowed. That preserves `arcweft-core` as Sans I/O and avoids making the VM scheduler depend on browser GPU completion order. 

A good shape is:

```rust
Need::BrowserMathGpu {
    request_id,
    op,
    shape,
    inputs,
    policy,
}
```

The VM reaches a deterministic suspension point, the browser adapter runs GPU work asynchronously, and the player resumes the VM by `request_id`, not by whichever GPU future resolves first. Native CLI behavior remains unchanged: native CPU/glam/ndarray/native-`wgpu` selection and native-only Cranelift stay behind existing target gates.

**Affected Arcweft crates/modules.**

* Browser player task scheduler
* `arcweft-runtime-accelerator` browser adapter
* Profile JSON
* `arcweft-core` only for stable values/results, not browser APIs

**Concrete implementation plan.**

1. Keep current synchronous CPU math path as the semantic reference.
2. Add browser GPU requests only at player task boundaries.
3. Record deterministic request order.
4. On GPU completion, verify result against CPU for representative sampled inputs in debug/bench modes.
5. For replay, allow forcing CPU backend while comparing output hash/tolerance to recorded profile data.
6. Never let GPU completion order reorder VM events.

Profile fields to add:

```json
{
  "math_backend": "cpu_wasm|browser_webgpu",
  "policy": "auto_conservative|webgpu_only|cpu_only",
  "fallback_reason": "auto_cpu_threshold|null",
  "request_id": 42,
  "op": "matmul_f32",
  "shape": {"m": 256, "k": 256, "n": 256},
  "dispatches": 1,
  "bytes_uploaded": 524288,
  "bytes_readback": 262144,
  "map_wait_ms": 0.0,
  "end_to_end_ms": 0.0,
  "correctness": {
    "checked": true,
    "passed": true,
    "max_abs": 0.0,
    "max_rel": 0.0
  }
}
```

**Tests or browser benchmarks to add.**

* VM replay test where GPU result resolves after another task but VM resumes in request order.
* Browser profile snapshot test with no host absolute paths.
* Native CLI test proving browser-only dependencies and Cranelift remain absent/present under the right cfgs.
* Fallback test where WebGPU is unavailable and the profile records the reason.

**Expected performance impact and risk.**

This keeps deterministic replay intact and avoids async contamination of `arcweft-core`. The cost is that GPU acceleration is only useful at boundaries that can tolerate async suspension. That is the right tradeoff for a narrative VM.

---

## 8. Current simple WGSL kernels are appropriate as baseline kernels, not final peak-performance kernels

**Recommendation.** Keep the current simple WGSL kernels as correctness and benchmark baselines:

* row-major `f32` matmul,
* `f32` elementwise add,
* `@workgroup_size(16, 16, 1)` for matmul,
* `@workgroup_size(256)` for add,
* explicit bounds checks for rounded-up dispatches.

A `16x16` matmul workgroup uses 256 invocations, matching the portable default max compute invocations per workgroup. A 256-wide add workgroup also matches that portable ceiling. Rounded dispatch dimensions require bounds checks because WGSL out-of-bounds memory-view access is invalid/indeterminate at runtime. ([MDNウェブドキュメント][13])

Use storage buffers with `array<f32>` for tensor data. Put dimensions in a small parameter struct of `u32`s, padded/aligned predictably. Avoid WGSL matrix types for Arcweft dense runtime matrices; WGSL matrix types are small fixed column-vector matrix types, not general dense tensor storage. ([W3C][14])

Do not implement browser `f64` kernels now. Current WGSL scalar source types include `f32`, `f16`, integer, boolean, texture/sampler-related types, but not portable `f64`. `f16` is optional and extension-gated, so `f32` is the right first target. ([W3C][14])

Floating-point comparison must allow tolerance. WGSL floating-point behavior follows IEEE-754 in broad terms but permits differences such as reassociation or fused operations, so GPU matmul should not be required to be bit-identical to CPU. Use CPU `f32` accumulation in the same logical order as the reference, avoid NaN/Inf/subnormal benchmark inputs initially, and report `max_abs`/`max_rel`. ([W3C][14])

Suggested tolerances:

```text
elementwise add:
  abs_tol = 1e-6
  rel_tol = 1e-6

matmul:
  abs_tol = max(1e-4, 4.0 * f32::EPSILON * k * max_abs_a * max_abs_b)
  rel_tol = 1e-4 initially, relaxed to 1e-3 only if browser data justifies it
```

**Affected Arcweft crates/modules.**

* `arcweft-runtime-accelerator::browser_webgpu` WGSL shader strings/modules
* CPU reference tests
* Browser benchmark correctness checker

**Concrete implementation plan.**

1. Label existing simple shaders as `baseline`.
2. Add shader tests for non-multiple workgroup shapes:

   * add length 255, 256, 257,
   * matmul 17×19×23.
3. Add checked parameter buffer structs with safe `Pod` layout.
4. Add `max_abs`/`max_rel` correctness reporting.
5. Later add a `tiled16` matmul kernel using workgroup memory:

   * 16×16 tile A,
   * 16×16 tile B,
   * about 2 KiB workgroup storage for `f32`,
   * still below portable 16 KiB workgroup storage.

**Tests or browser benchmarks to add.**

* Baseline shader correctness for rectangular matrices.
* Bounds-check correctness for rounded dispatch.
* Benchmark baseline vs tiled matmul.
* Benchmark add with resident buffers to separate dispatch cost from transfer/readback cost.

**Expected performance impact and risk.**

The simple kernels are good for validation and early integration. They will not be peak-performance matmul kernels. The main risk is prematurely judging WebGPU from the simple matmul kernel; keep it as baseline, then add a tiled kernel once the harness is stable.

---

## Recommended commit sequence

1. **Browser dependency and cfg cleanup**
   Add explicit browser `wgpu` features, keep native paths unchanged, add structured availability/fallback enum.

2. **Async readback helper and error scopes**
   Replace any blocking assumptions in browser code with `map_async` futures; add error-scope handling, uncaptured-error callback, and device-lost callback.

3. **Limit/shape validation**
   Add checked dimension and byte-size validation before all GPU allocations and dispatches.

4. **Correctness-first browser smoke tests**
   Exercise add/matmul in a real browser or cleanly skip with structured reasons.

5. **Path-free benchmark harness**
   Add CPU Wasm baseline, WebGPU one-shot, and WebGPU prepared modes with JSON output.

6. **Prepared-buffer API**
   Cache pipelines, buffers, bind groups, and readback staging buffers safely.

7. **Conservative `Auto` policy**
   Enable browser WebGPU only for benchmark-proven cases, starting with larger/repeated matmul.

8. **Tiled matmul kernel**
   Add after benchmark infrastructure proves the baseline and policy.

The most important architectural rule is: **browser WebGPU should never be a silent synchronous substitute for CPU math**. It should be an explicitly async, profiled, correctness-checked adapter optimization with structured fallback and unchanged native CLI behavior.

[1]: https://docs.rs/wgpu/ "https://docs.rs/wgpu/"
[2]: https://docs.rs/crate/wgpu/latest/features "https://docs.rs/crate/wgpu/latest/features"
[3]: https://rustwasm.github.io/docs/wasm-bindgen/reference/rust-targets.html "https://rustwasm.github.io/docs/wasm-bindgen/reference/rust-targets.html"
[4]: https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API "https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API"
[5]: https://web.dev/blog/webgpu-supported-major-browsers "https://web.dev/blog/webgpu-supported-major-browsers"
[6]: https://docs.rs/wgpu/latest/wgpu/struct.InstanceDescriptor.html "https://docs.rs/wgpu/latest/wgpu/struct.InstanceDescriptor.html"
[7]: https://docs.rs/wgpu/latest/wgpu/struct.Device.html "https://docs.rs/wgpu/latest/wgpu/struct.Device.html"
[8]: https://docs.rs/wgpu/latest/wgpu/struct.Buffer.html "https://docs.rs/wgpu/latest/wgpu/struct.Buffer.html"
[9]: https://docs.rs/wgpu/latest/wgpu/struct.Limits.html "https://docs.rs/wgpu/latest/wgpu/struct.Limits.html"
[10]: https://arxiv.org/abs/2604.02344 "https://arxiv.org/abs/2604.02344"
[11]: https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/browsers.html "https://rustwasm.github.io/docs/wasm-bindgen/wasm-bindgen-test/browsers.html"
[12]: https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips "https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips"
[13]: https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits "https://developer.mozilla.org/en-US/docs/Web/API/GPUSupportedLimits"
[14]: https://www.w3.org/TR/WGSL/ "https://www.w3.org/TR/WGSL/"
