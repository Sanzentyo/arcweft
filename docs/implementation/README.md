# Implementation Status

This directory records the current implementation state of Arcweft Engine.

Design specifications remain in the numbered `docs/` chapters. Files here describe what exists in the Rust workspace today, what has been verified, and what is intentionally deferred.

## Current Milestone

Phase 0 / Phase 1 minimal Rust workspace:

- Cargo workspace skeleton.
- Foundational ID, source anchor, Need, and dialogue surface model crates.
- Syntax and CLI crates with Phase 1 parser/HIR/check surfaces and the
  `arcw check <file.arcw>` developer entry point.
- Language responsibilities are now split across `arcweft-lang-syntax`
  (lossless CST, surface AST, parser, syntax lint), `arcweft-lang-hir`
  (HIR types and lowering), `arcweft-lang-sema` (name/symbol/type readiness and
  minimal type checking), and `arcweft-runtime-plan` (HIR to Sans I/O runtime
  plan lowering).
- Entry declarations now parse, lower through HIR, materialize into
  `RuntimePlan.entries`, and can be selected by `arcw run --entry`; `--flow`
  remains available for direct flow selection. When no entry is provided,
  runtime lowering keeps the first flow as the deterministic fallback for
  current headless fixtures.
- `extern capability` declarations parse and lower as structured HIR
  declarations. Capability functions are registered for type checking, their
  declared `effects { ... }` are enforced against the active flow/function
  effect scope, and filesystem capability calls reject direct OS absolute path
  string literals in favor of `VirtualPath` constructors.
- `arcweft-core` no longer depends on dialogue or presentation; the facade
  crate `arcweft` exposes crate-family namespaces instead of a flat prelude.
- Awaited capability calls now carry typed `HostTaskRequest` data through
  `AwaitTarget` into emitted `TaskSpec`s. The core remains Sans I/O; adapters
  consume the request data and later return deterministic `TaskEvent`s.
- `Vec<T>.traverse(capability.fn).parallel(limit = N)` is implemented for
  awaited capability fanout. Runtime-plan lowering emits `FlowOp::AwaitMany`,
  the VM keeps bounded in-flight task state, duplicate same-request tasks use
  joinable scheduler keys, and native CLI runs can execute real file reads while
  reporting `max_in_flight` without recording host absolute paths.
- `arcw serve --listen` owns a minimal native HTTP adapter in the CLI layer. It
  consumes lowered server route plans and executes matched flows through
  `RuntimeStepMode::Server`; `arcweft-core` remains free of network I/O. The
  listening path is now gated by the active adapter manifest's `http.respond`
  host call, matching native task dispatch rather than relying on an implicit
  server shim.
- CLI native task dispatch now uses a manifest-derived `HostCallPolicy` plus a
  separate native adapter registry. Core `HostTaskRequest` values expose stable
  ids such as `fs.read_text` and `system.available_parallelism`; the bridge
  rejects requests missing from the active policy and also rejects policy-allowed
  ids that have no registered native implementation. Standard native file,
  system-info, selected profile, and internal scheduler-marker manifests define
  permission explicitly instead of relying on Rust pattern matches.
- `arcweft-host-adapter` owns the Sans I/O host-adapter policy/registry types.
  `arcweft-runtime-host` owns native task bridging, system information, bundle
  materialization, and typed bundle execution reports. Embedding runners, LSP
  tooling, and player adapters can depend on this crate without compiling or
  linking the CLI binary/argument parser. `arcweft-verify-lsp` now exposes a
  runtime-host capability set on `ArcweftLspContext`, plus a profile-context
  builder and combined profile completions, hover, and diagnostics for cases
  where a profile's adapter manifest declares a host call that the selected
  runner does not implement. `arcweft-runtime-host` owns native and browser-web
  capability presets plus a typed conformance report so transports can
  distinguish native virtual-file support from browser-only host capabilities
  and share the same manifest-vs-runner host-call check. `arcweft-lsp` now owns
  the first actual stdio language-server transport on top of `lsp-server`,
  while `arcweft-verify-lsp` remains Sans I/O. The transport negotiates LSP
  position encoding, keeps a FULL-sync open-document cache, publishes syntax /
  HIR / verifier diagnostics, and routes completion, hover, signature help,
  inlay hints, and code actions through the helper crates. Source-level sugar
  expansion and ID materialization actions now return LSP `WorkspaceEdit`
  values through the same byte-span mapper used for diagnostics, and
  `workspace/executeCommand` can translate the older command argument shape into
  the same edit without writing files server-side. The transport negotiates
  workspace-edit shape from client capabilities, returning versioned
  `documentChanges` when supported and `changes` otherwise. It resolves
  `arcw.toml` near opened documents, caches profile metadata per document URI,
  refreshes profile metadata on open, save, watched-file, and configuration
  notifications, loads project-local adapter manifests and Rust ABI JSON into
  the selected adapter, and reports profile metadata diagnostics with profile
  ids and profile-relative resource labels without recording host absolute
  paths in checked-in artifacts. Source diagnostics now run the same
  profile-aware semantic path as CLI checks, using the selected adapter
  environment for type analysis and verifier diagnostics after parse, HIR,
  reference resolution, and readiness pass. Completion, hover, and signature
  help use the refreshed document-scoped profile context. `arcweft-cli` remains both a
  library and a binary for argv-compatible execution through
  `run_with_native_adapters`.
- `arcweft-bundle` owns the first Sans I/O `.awfb` data model and deterministic
  JSON codec. `arcw bundle` and `arcw build bundle` package source text,
  executable structured bytecode, runtime summary counters, required host-call
  ids, adapter manifest bodies, adapter manifest ids, and relative virtual
  files without recording host absolute paths. `arcw run-bundle` decodes that
  bytecode directly, materializes virtual files into a temporary CLI workspace,
  and can execute the standard native file, system-info, and internal scheduler
  adapters through the same native task bridge used by `arcw run`. Bundle JSON
  reports compile/package phases, and run-bundle JSON reports read/decode/
  materialize/bytecode/run phases so source compilation cost and bytecode
  execution cost can be measured separately. Integration coverage now builds a
  bundle with a project-local custom adapter manifest and executes it directly
  through `arcweft_runtime_host::run_bundle_file_with_native_adapters`, proving
  the bundle supplies policy data while the embedding runner supplies concrete
  host code and receives a typed `BundleRunnerReport`. Runtime-host crate
  coverage also constructs a custom-adapter bytecode bundle directly and runs it
  through `arcweft_runtime_host::run_bundle_with_native_adapters`, so this
  boundary is tested without involving the CLI crate.
- `arcw check`, `arcw verify`, `arcw unsafe`, and plan/report generation now
  resolve profile adapters through the standard `arcweft-adapter-context`
  manifest registry plus profile-local `adapter_manifests`, then pass the
  resulting `TypeCheckEnv` through both type checking and semantic
  verification. Generic direct-path mode still uses the Sans I/O manifest.
- Rust adapter metadata is now an explicit profile-selected input. The
  `arcweft-rust-abi` data crate defines deterministic JSON metadata for Rust
  exported functions and ADTs, `arcweft-rust-abi-macros` provides opt-in
  function/type metadata generation, and `arcweft-rust-abi-build` owns the
  build-script file I/O helper so the data crate stays Sans I/O.
  `arcweft-adapter-context` merges that metadata into typed adapter manifests
  alongside standard symbols, methods, typed effect capabilities, host calls,
  project-local JSON/TOML adapter manifests, and package export tables, and
  `arcweft-verify-lsp` exposes Sans I/O completion, hover, and signature-help
  helpers from the same manifest. `extern rust mod`
  declarations parse as structured type/function/activity members and are
  checked against the profile-selected package metadata. The CLI reads
  `rust_metadata` entries from launch profiles; direct-path checks remain
  strict and do not infer Rust APIs.
- `arcweft-core::aot` provides a pure `AotProgram` artifact with typed flow
  dispatch-shape analysis, deterministic operation-class counters, and
  pre-lowered linear operation blocks. Full generated flow state machines remain
  future work, but `AotExecutor` now executes fully linear flows and mixed-flow
  linear prefixes through this artifact without cloning `FlowOp` values on each
  fast-path step. Mixed control-flow boundaries still fall back to the
  VM-compatible state machine. `arcw run`, `arcw cli`, `arcw test`, `arcw profile`, and runtime `arcw bench`
  sections can select the AOT boundary with `--executor aot` and report that
  tier in JSON without introducing different semantics. Pure helper AOT is
  implemented separately: `AotPureFunctionBackend` compiles the deterministic
  `i64` subset to a typed plan and rejects unsupported helpers instead of
  delegating to the VM.
- `arcweft-core::pure` exposes the pure-helper backend contract used by future
  AOT/JIT adapters. `VmPureFunctionBackend` is the semantic reference,
  candidate backends report deterministic evaluation stats, and conformance
  checks compare candidate output against VM output without recording host
  absolute paths.
- `arcweft-lang-jit-cranelift` now owns the first native Cranelift adapter. It
  JIT-compiles deterministic `i64` pure helper expressions, including integer
  add/sub/mul/div arithmetic, unary negation, comparisons, value-producing `if`,
  lexical `let` bindings, and selected local bindings passed as runtime `i64`
  inputs. The native call boundary supports 0 to 4 runtime integer inputs.
  Generated code uses Cranelift's `speed` optimization level and executes
  through an isolated native-call boundary. The regression harness keeps Rust
  `unsafe` sites confined to that boundary and requires nearby `SAFETY`
  comments, while `arcw jit check --json` exercises it against the VM reference
  backend and the typed AOT plan with deterministic seed-controlled varying
  inputs, sample timing, and speedup reporting.
  `arcw jit check path.arcw --helper NAME --json` now runs
  the normal checked-source pipeline, extracts a `#[pure] fn` helper from HIR,
  lowers its expression body or simple local-`let` statement body with a final
  value or tail `return` to a pure-helper request, and reports the helper source
  without persisting the host path. Source-helper JIT reports include the same
  source compiler phase timings plus typecheck and borrow-check counters used by
  `arcw check --json`, so native speedup can be evaluated against front-end
  compilation cost. Builtin JIT checks now expose `--case score`,
  `--case branch-mix`, `--case let-chain`, and `--case four-input-mix`, and JSON
  includes workload metadata plus a JIT-compiled batch loop over the same
  deterministic input series. The batch loop carries generated input values as
  loop parameters and advances them with bounded wraparound, avoiding per-input
  modulo work inside the hot loop. Julia baseline reports include scalar
  JIT/Julia and JIT-batch/Julia speed ratios.
  The parameterized integer, floating-point, small-integer, and wide-integer
  batch paths now separate module codegen from JIT execution. `define_*`
  functions define entry, row-batch, and when supported row-batch-sum functions
  into a generic Cranelift `Module`. The deterministic `arcw jit check`
  benchmark loop follows the same boundary through `define_i64_benchmark_batch`
  before the JIT wrapper finalizes the module. The corresponding `compile_*`
  functions remain native JIT wrappers that finalize the `JITModule` and
  install function pointers. Cranelift lowering, JIT compilation, and object
  emission now return `CraneliftCodegenError` rather than a JIT-only error type.
  `emit_object_{i64,i32,u32,u64,i8,i16,u8,u16,f32,f64}_with_inputs` and
  `emit_object_{i128,u128}_batch_with_inputs` now use the same generic `Module`
  lowering paths to emit relocatable object bytes. Scalar integer objects
  include scalar, row-batch, and row-batch-sum symbols; floating-point objects
  include scalar and row-batch symbols; wide integer objects are batch-only to
  keep by-value `i128`/`u128` out of the native FFI boundary. Runtime native
  JIT scalar calls for `i128` and `u128` use those pointer-based artifacts as
  one-row batches, so the executable path also avoids by-value wide integer
  ABI calls. Cranelift can also emit a multi-helper object bundle with
  exact-kind, entry, batch, optional batch-sum, parameter-name, and
  lowering-stats metadata per helper. Runtime AOT
  policy records a single native object bundle for helpers whose cache entries
  selected AOT or AutoAOT when `native-jit` is available and the runtime config
  explicitly enables object artifacts. Object attempts and successes count
  helpers, while emitted bytes count the bundled object. This is off by default
  so ordinary Auto/AOT startup does not pay build-time AOT emission cost.
  `arcweft-runtime-accelerator` now classifies exact-width native helper
  support through one private typed kind, so JIT promotion, native JIT compile
  dispatch, and object-bundle input-kind selection share the same width-preserving
  source of truth instead of parallel per-width predicates.
  The object bytes are not executed by the runtime cache yet; typed AOT and
  native JIT remain the executable paths, while object artifacts are measured
  build-time AOT evidence for a future loader/linker boundary.
- `arcw bench` runs measurable `measure { start(@flow.id) }` sections through
  the selected headless runtime executor, includes deterministic runtime
  counters in JSON, completes native file task requests through the CLI adapter,
  and evaluates `assert { expect.*(...) }` sections against a separate
  correctness run before reporting a measured bench as successful. Bench reports
  also expose native I/O task completion, read/write operation, and byte-count
  counters, plus compile phase timings and type, borrow, runtime-type, bytecode,
  and AOT dispatch-shape counters so runtime performance can be compared with
  parser/checker/lowering cost. Bench assertions can check real file output with
  `expect.file(path.save("output.txt"), equals="...")` while keeping the host
  filesystem path out of JSON.
  Runtime bench deterministic summaries include child-fiber activity ticks and
  peak child-fiber fanout, so source-level `thread` scheduling can be compared
  across VM/scheduler changes without recording host paths.
  The measured bench loop folds per-step counters directly into compact sample
  totals instead of allocating per-step bench trace entries, keeping benchmark
  harness allocation out of the runtime hot path. Sample vectors are
  preallocated from the requested iteration count, so measured sections do not
  grow counter buffers as they run.
  Bytecode and AOT executor artifacts for a measured flow are prepared once
  before warmup and measured iterations; per-iteration elapsed time starts after
  fresh executor state has been instantiated from that template.
  The native task bridge is created lazily only when a measured runtime section
  actually needs to complete emitted host tasks, so pure/runtime-only benches do
  not pay adapter setup cost.
  They also include median pure argument/result byte-copy counters, so scalar
  pure-call boundary costs are visible in the same bench report as elapsed time
  and VM op counts.
  Bench regression coverage now includes a mixed flow with source-level
  `thread` child fibers and thread-local native file reads, so scheduler
  counters show both cooperative child-fiber markers and adapter-owned I/O
  tasks in one path-free report. Drain/server stepping can continue across
  already-emitted host requests while runnable child fibers or the main fiber
  can still produce more work, allowing sibling thread reads to reach the
  native scheduler in the same host batch.
  `measure { pure(helper_name) }` sections additionally run the selected
  checked `#[pure] fn` helper through the VM reference, typed AOT plan, native
  Cranelift JIT, and JIT batch loop, reporting conformance, deterministic
  accumulators, timing samples, compile time, and speedup ratios in the same
  bench JSON.
- Runtime pure acceleration lazily constructs the native worker pool only when
  an AOT/VM batch has more rows than the configured per-worker parallel
  threshold multiplied by the resolved worker count. JSON
  `pure_config.worker_pool_active` makes this boundary visible: scalar calls,
  JIT-only helpers, and sub-threshold AOT/VM batches avoid worker-pool setup
  overhead, while parallel batches still create and report the pool.
- Runtime pure batch parallelization now uses backend-aware weighted work units
  instead of row count alone. The accelerator computes a deterministic helper
  expression weight at construction time, then evaluates `rows * weight`
  against a lower VM threshold and a higher AOT threshold; JIT flat batches stay
  single native calls and report a backend skip instead of constructing the
  worker pool. Runtime and bench JSON expose policy checks, weighted work,
  parallel batches, backend skips, small-batch skips, and pool build time so
  threshold decisions can be tuned from path-free measurements.
- Auto pure scalar execution now uses the same work-unit policy family as flat
  batches. Cold scalar helpers still start on typed AOT, but repeated scalar
  calls accumulate per-helper work units and promote supported native kinds to
  JIT once the hot scalar threshold is crossed. Warmed natural `for` loops over
  exact-width integers and floats can therefore reach native JIT without an
  explicit `--pure-backend jit`, while small scalar flows avoid JIT startup
  cost.
- AOT pure scratch calls reset caller-owned slot buffers in place when the
  compiled slot count is unchanged, so repeated scalar and batch helper calls do
  not rebuild the slot vector before writing dynamic inputs.
- VM pure scratch calls now use a scalar i64/bool evaluator for supported
  deterministic helper expressions. The VM remains the reference backend, but
  repeated dynamic pure calls avoid constructing intermediate `RuntimeValue`
  payloads until the final result.
- Runtime pattern matching now derives binding capacity from the structured
  pattern enum and uses it for temporary scopes, map-row scopes, and `for`
  iteration scopes, reducing allocation churn in natural iterator-heavy flows.
  `RuntimeEnv` also reuses popped temporary scopes within the same runtime
  environment; cloned child-fiber environments do not clone the spare-scope
  cache, and runtime equality ignores that cache.
- Source-level thread child fibers build their scoped pending-op queue directly
  instead of first inserting scope markers into a temporary `Vec`, keeping child
  startup allocation and movement local to the final `VecDeque`.
- The VM pure reference backend also builds evaluator root bindings from a
  borrowed binding slice, avoiding an extra request-binding vector clone during
  JIT/AOT conformance checks and standalone helper evaluation.
- Value-slice pure VM fallback now reuses `VmPureFunctionScratch` and borrowed
  argument slices instead of reconstructing a `PureFunctionRequest` and binding
  vector for each fallback call. `arg_vec_allocations` remains zero for that
  backend boundary while `arg_bytes_borrowed` records the borrowed slice size.
- Runtime pure accelerator cache construction now builds each helper's compile
  request once and shares it across JIT/AOT attempts, reducing Auto-mode setup
  work for helpers that fall through from JIT to AOT or VM.
- Runtime dense math now has `MatrixF32`, `TensorF32`, `MatrixF64`, and
  `TensorF64` value variants in `arcweft-core`, with scalar row-major
  correctness kernels kept Sans I/O and generic storage underneath the
  width-specific runtime variants.
  Native math acceleration is isolated in `arcweft-runtime-accelerator` behind
  selectable `scalar`, `glam`, `ndarray`, `wgpu`, and `auto` backends. The wgpu
  backend is feature-gated, keeps DX12 enabled for Windows alongside Vulkan,
  Metal, and GLES, and the workspace Rust floor is raised to 1.96 so the latest
  wgpu stack can be used directly. CLI runtime commands and launch profiles now
  route `math_backend` and `math_wgpu_min_elements` into
  `RuntimePureAcceleratorConfig`, so built-in math calls use the same adapter
  config path as pure helper VM/AOT/JIT selection. CLI runtime bindings can
  supply `matrix/f32/<rows>x<cols>:<csv>`, `tensor/f32/<dims>:<csv>`,
  `matrix/f64/<rows>x<cols>:<csv>`, and `tensor/f64/<dims>:<csv>` values,
  making source-level `math.*` flows measurable through the normal bench JSON.
  The portable wgpu kernels remain `f32`; `f64` math uses scalar, glam 4x4, or
  ndarray CPU backends without narrowing. `Auto` keeps `f64` off wgpu, uses the
  scalar row-major matmul kernel for small general matrices up to 64^3 work
  items, and switches larger `f64` matmul calls to ndarray after the measured
  scalar/ndarray crossover. The standalone `math_bench` example now also
  accepts `matmul-f64`, `matrix-add-f64`, and `tensor-add-f64`, so f64 backend
  selection can be measured through the same typed path-free JSON report as f32
  math; explicit wgpu f64 requests report the portable-kernel unsupported reason
  instead of narrowing to f32. Native prepared wgpu math now
  caches by power-of-two capacity buckets in the runtime/Auto path, keeps exact
  repeated inputs resident, updates changed inputs with `queue.write_buffer`,
  and exposes capacity-prepared matrix/tensor APIs for dispatching smaller
  compatible shapes without recreating GPU storage or bind groups. The
  `math_bench` example exposes the explicit prepared-capacity path with
  `--reuse-capacity`, and the Justfile has matrix, tensor, and matmul recipes
  for collecting path-free JSON measurements. Browser WebGPU benchmarking now
  exposes matching prepared-capacity resident and prepared-capacity pipelined
  modes with typed `shape` and `capacity` fields, so native and browser GPU
  measurements can both show whether work used exact or overprovisioned
  resident buffers. Browser bench reports now also include typed per-shape
  recommendations with measured selected mode, selected capacity, speedup, and
  reason, with selected/CPU MAD and P95 fields for outlier inspection, plus the
  runtime policy mode, policy capacity, policy reason, and policy/measurement
  match flag; this moves browser-side `Auto` threshold evidence into the
  Rust-produced JSON schema instead of leaving it as a JS-only smoke summary.
  Native standalone `math_bench` reports now also include
  `speedup_vs_scalar` for each measured backend when a scalar baseline is
  present, so scalar/glam/ndarray/wgpu/Auto comparisons are visible in the
  path-free JSON artifact instead of requiring manual table calculation.
  Browser WebGPU contexts now also expose policy-driven async Auto calls for
  `matmul_f32`, `matrix_add_f32`, and `tensor_add_f32`, so browser embeddings
  can use the calibrated policy at the adapter boundary without copying the
  threshold logic into player code. The browser WebGPU
  context also separates resident compute submission from explicit readback:
  `submit_resident_*_without_readback` keeps prepared output on the GPU, and
  `read_resident_*` performs the host-visible copy/map only at the requested
  boundary. Browser WebGPU also has a typed resident `f32` graph fragment API.
  The current fragments cover `matmul -> add` and `matmul -> bias_add`. The
  `matmul -> add` fragment owns the matmul buffers, add buffers, and chained
  add bind group. The `matmul -> bias_add` fragment stores only the last-axis
  bias vector and broadcasts it in the second GPU kernel. Repeated graph-edge
  submissions do not rebuild intermediate bindings and never copy the matmul
  result out of GPU storage. The forward inference session now recognizes
  private `matmul -> bias_add` pairs at the adapter boundary and calls a typed
  `InferenceAdapter::matmul_bias_add` hook instead of forcing all adapters
  through a materialized intermediate tensor. The fusion is not applied when the
  matmul output is also an observable graph output or is consumed by another
  node. `AcceleratedInferenceAdapter` routes that hook into
  `RuntimeMathAccelerator::matmul_bias_add_f32`; the scalar backend fuses
  matmul and bias application in one loop, and native wgpu uses the same fused
  matmul plus bias-add compute passes for the one-shot value-returning path
  without reading the intermediate matmul output back to the host. Runtime math
  stats record `fused_matmul_bias_add_calls`. The standalone `math_bench` example accepts
  `--op matmul-bias-add`, so scalar/Glam/ndarray/wgpu/Auto selection and fused
  call counts are measurable through the same path-free JSON schema as the
  existing matmul and elementwise benchmarks. Native wgpu also has prepared
  resident storage for `matmul -> bias_add`: one prepared object owns the
  matmul input/output buffers, a compact last-axis bias buffer, and the final
  output buffer, then dispatches matmul and bias passes in one command encoder
  with a single final readback. `math_bench --op matmul-bias-add --reuse` now
  measures exact prepared reuse, while `--reuse-update-inputs` and
  `--reuse-capacity` measure repeated uploads into existing resident buffers.
  Native prepared `matmul`, `matmul -> bias_add`, `matrix_add`, and
  `tensor_add` also expose explicit submit/readback split APIs:
  `submit_prepared_*_without_readback` submits GPU work while leaving the
  prepared output resident, and `read_prepared_*_output` copies/maps the result
  only at the requested boundary. `math_bench --submit-only` uses that split to
  measure the native resident compute-submit lower bound, then performs one
  final readback for correctness instead of downloading every sample. The
  Justfile includes `bench-math-matrix-add-submit-only` and
  `bench-math-tensor-add-submit-only` for path-free elementwise lower-bound
  measurements.
  The optional inference adapter manifest now exposes
  `infer.matmul_bias_add_f32` as a normal adapter method/host call, and
  runtime-plan lowering converts selected adapter namespaces such as `infer.*`
  and `conv2d.*` into named external-call targets only when the profile-selected
  adapter has made those namespaces type-checkable. `RuntimePureAccelerator`
  routes that external call through the same prepared wgpu cache used by runtime
  math calls when the backend policy selects wgpu.
  Repeated flow/external calls with unchanged inputs reuse the resident
  matmul-bias buffers without host upload; changed compatible inputs update the
  existing buffers instead of rebuilding them. The default
  `AcceleratedInferenceAdapter` used by Rust-side `InferenceSession` now owns
  the same typed prepared matmul-bias cache, so private `matmul -> bias_add`
  graph fusion also reuses resident wgpu buffers across repeated session runs.
  Runtime executor JSON now includes `fused_matmul_bias_add_calls`, so CLI bench
  output can distinguish a fused adapter/math boundary from separate matmul and
  bias-add execution. Profile-selected Arcweft flow benches now cover both the
  scalar and explicit `ndarray` versions of `infer.matmul_bias_add_f32`,
  confirming that adapter-contributed methods lower to the same math backend
  selection boundary and report accelerated-call counters without embedding
  workspace or temporary absolute paths.
  `math_bench --op inference-matmul-bias-add` measures this adapter-boundary
  path directly: without `--reuse` it builds cold sessions for each sample,
  while `--reuse` keeps the same `InferenceSession` and prepared GPU cache.
  `InferenceSession::run_borrowed` is the implementation path for graph
  execution: graph constants and supplied input tensors stay borrowed in the
  per-run value table until an op produces an owned output, so adapter execution
  no longer requires an extra input/constant tensor clone inside the session.
  The owned `run` API delegates to the borrowed path after collecting its input
  tensors, while callers that keep tensors resident can call `run_borrowed`
  directly.
- Runtime pure helper plans record whether the scalar evaluator is supported at
  lowering/construction time, avoiding a recursive expression-shape scan on
  every VM scratch call.
- Flow cursors carry only the resolved flow index and op index. Normal VM and
  AOT stepping read the current flow from the runtime plan vector directly and
  reserve the flow-id map lookup for entry/goto resolution.
- VM and prechecked AOT stepping borrow the current cursor while fetching the
  next op, avoiding an extra per-op cursor clone in the hot path.
- Prechecked AOT linear stepping now advances the current flow cursor in place,
  avoiding a per-op cursor allocation for straight-line VM-compatible
  operations.
- Normal VM flow stepping uses the same in-place cursor advance for
  non-suspending operations; choice and await states still materialize a resume
  cursor only when they suspend.
- Suspended await/choice state stores `Option<FlowCursor>` for resume points, so
  pending-op suspensions do not rely on a default cursor sentinel.
- Flow cursors are `Copy` index pairs, so suspend/resume bookkeeping does not
  clone heap-owned flow identifiers.
- Prechecked AOT linear stepping borrows pre-lowered operations from
  `AotProgram`, avoiding per-step `FlowOp` clones for straight-line dispatch.
- Mixed-flow AOT stepping now runs the pre-lowered linear prefix and then
  continues through the VM-compatible branch/loop/await dispatcher in the same
  runtime step. This extends AOT coverage to common setup-then-branch shapes
  without adding a speculative generated state machine or a host boundary at the
  mixed prefix edge.
- AOT linear dispatch checks use that cursor index to read the corresponding
  AOT flow block directly, keeping the hot eligibility check aligned with the
  runtime-plan flow vector.
- AOT programs store dispatch-shape metadata plus the lowered linear prefix;
  executor construction keeps the semantic runtime plan in the VM engine while
  the AOT artifact owns the fast-path dispatch payload.
- Runtime bench executor templates keep bytecode/AOT artifacts beside the
  selected runtime plan so per-iteration setup does not need to decode bytecode
  back into a plan before creating a fresh executor.
- AOT dispatch-shape planning and the prechecked linear executor share the same
  supported-op predicate, so control-changing effects and branch/jump ops fall
  back to the VM path instead of entering a fast path that would reject them.
- `RuntimePlan` no longer constructs engine cursors. Engine initialization owns
  cursor construction from the flow-position map, keeping plan data independent
  of executor state while avoiding a second entry-flow scan.
- Type-check stats now record judgment rule counters as judgments are emitted.
  CLI profile/check JSON reuses those counters instead of rescanning every
  judgment when building report summaries.
- The native task scheduler dispatch path takes the full pending queue directly
  when the budget covers every ready task, avoiding front-drain movement in the
  common unbounded host-dispatch case.
- Scheduler completion now allocates joined-completion storage only when a
  completed task actually has join waiters.
- CLI bench sample aggregation computes medians with nth-element selection
  instead of fully sorting every timing/counter sample vector.
- Borrow release uses unordered removal from the active-borrow list because
  active borrow order is not semantically meaningful, reducing element movement
  during drop-heavy borrow checking.
- `arcw toolchain-profile` accepts warmup runs and reports warmup samples
  separately from measured samples, so build-cache priming does not pollute
  median timing while failures remain visible in JSON.
- `arcweft-cli` keeps user-facing JSON report schemas in `output.rs`,
  including check, profile, verify-types, bench, runtime step, and compiler
  counter summaries. `main.rs` remains the command orchestration layer instead
  of also owning these report data models.
- `arcw toolchain-profile` measures workspace toolchain commands through the
  CLI layer without recording host absolute paths in JSON. It currently supports
  `--command fmt`, `--command check`, `--command check-full`,
  `--command clippy`, `--command test-build`, and `--command test`, with
  `--repeat N` median/min/max timing summaries, dry-run planning for regression
  tests, and real elapsed-time reports for local performance tracking. It also
  supports `--command bench-003` and `--command bench-009` as path-free local
  trend commands for the scalar pure JIT and flat-batch pure JIT fixtures. Those
  bench commands parse the nested `arcw bench --json` stdout into compact
  `arcweft_bench` summaries with runtime median/counter data while keeping the
  original benchmark stdout out of the profile report. The profiler preallocates
  repeat samples and counts stdout/stderr lines from bytes directly, avoiding
  UTF-8 string allocation in its own reporting path.
- No renderer, Servo, audio, camera, USB, or MCP implementation.

## Files

- `phase-0-1-workspace.md`: current crate layout, public types, verification status, and deferred work.
- `refactor-checklist.md`: direction-package checklist for the runtime boundary,
  entry/capability grammar, RuntimeStep, executor, and fixture-driven gates.

## Verification Snapshot

Last verified for the active workspace after enabling the spec fixture gates:

- `cargo fmt --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features`
- `arcw toolchain-profile --command fmt --json`
- `arcw toolchain-profile --command check --json`
- `arcw toolchain-profile --command clippy --json`
- `arcw jit check --json`
- `arcw check` over `tests/fixtures/arcw/spec_should_pass/check`
- `arcw run --mode drain --steps 16` over `tests/fixtures/arcw/spec_should_pass/run`
- `arcw check` over `tests/fixtures/arcw/spec_should_fail`, expecting every
  fixture to fail with diagnostics

## Design Reviews Reflected

The implementation notes track accepted syntax decisions from `docs/reviews/` when
they affect parser, HIR, formatter, LSP, or CLI work.

`pro_review21.md` is reflected for the current module-boundary scope, with
explicit evidence tracked in `phase-0-1-workspace.md` under
"pro_review21 Prompt-to-Artifact Checklist".

Current high-confidence state:

- Done: core split + tests, sema public split, syntax AST split, HIR split,
  syntax parser family split, runtime-plan split, dependency cleanup
  (`runtime-plan -> hir`, duplicate `arcweft-test` dependency removal, and
  `arcweft-dialogue -> arcweft-presentation` cleanup), and adapter-frame view
  lifetime APIs.

- `pro_review4.md`: adopted value-producing `{ ... }` blocks, `scope name { ... }`
  blocks for relative ID namespaces, unnamed `scope { ... }` as name-omitted
  sugar, relative IDs only in ID-bearing contexts,
  `self::` / `super::` / `crate::` module-path roots, reserved `parent::`
  normalization, and explicit sugar expansion for `with:`, speaker colon lines,
  speaker-preset calls, and `await?`.
- `pro_review5.md`: adopted structured function signatures with generic params,
  curried parameter groups and `where` clauses; structured hook headers
  (`when`, `priority`, `once`, `effects`); structured dialogue line options; and
  a parsed `dialogue defaults` top-level declaration.
- `pro_review28.md`: adopted the first general variadic signature slice as
  `param: ...T`. Syntax stores rest parameters as parameter kind, semantic
  checking binds them as `Vec<T>`, and function-call checking consumes remaining
  positional arguments as rest items. Call and method-call syntax now carries
  `CallArg::{Positional, Named, Spread}` instead of embedding named/spread
  argument markers as expression variants. Positional call-site spread parses as
  `expr...`, typechecks only when it splices a sequence into a rest parameter,
  and is preserved into runtime/host call templates so the VM expands tuple and
  bracket-sequence values at the call boundary.
- `pro_review29.md`: adopted anonymous sum types as `A | B`, where alternatives
  are types rather than named variant rows. Syntax and semantic checking now
  reject duplicate alternatives and alias collapse, expected-type checking
  injects values into a unique branch, `if`/`match` joins can produce anonymous
  sums, typed match patterns eliminate branches, and runtime typed patterns
  check value shape before binding. VM and host request execution keep
  anonymous sums erased to concrete `RuntimeValue` payloads, including spread
  custom-host arguments, so dynamic host request lowering stays independent of
  anonymous sum typing. Public function signatures and public type aliases that
  expose anonymous sums now emit non-fatal type-analysis warnings steering
  stable ABI/save-data surfaces toward nominal enums.
- `pro_review7.md`: adopted rowan-compatible lossless CST as the public parsing
  foundation for `arcweft-lang-syntax`, with `ParsedSource` returning syntax,
  typed syntax views, diagnostics, source text metadata, and line index even for
  malformed files. The typed syntax view is still produced by the existing
  parser builder and should be migrated onto CST/event parsing next.
- `pro_review8.md`: accepted VM / Typed IR as the semantic source of truth.
  Native Cranelift JIT is a pure-function optimization tier in
  `arcweft-lang-jit-cranelift`; Wasmtime is only a native plugin/activity
  sandbox; web uses an AOT compiled Wasm player plus bytecode bundle. Data
  formats, manifests, bundles, schemas, bytecode, and save snapshots must remain
  Sans I/O.
- `pro_review9.md`: adopted `@...` entity references, Rust-like `#[...]`
  attributes, ordinary effectful calls instead of `@` scenario commands,
  color-as-string typing, explicit primitive numeric widths, typed unit-number
  literals such as `100pt`, `2.0f32`, `10i32`, and angle units including `rad`.
  Relative IDs are unified on `@.suffix`, parent-dot forms such as
  `@..suffix` / `@...suffix`, and explicit `@super...` forms; bare `.suffix`
  is not part of the core grammar.
- `pro_review26.md`: `TypeKind` now keeps explicit primitive widths for
  `i8` through `i128`, `u8` through `u128`, `isize`, `usize`, `f32`, and `f64`.
  Numeric literals preserve raw spelling and suffixes in syntax. Unsuffixed
  integer and float literals are rejected unless the checker has an expected
  numeric type from an annotation, return context, branch context, unary/binary
  operand context, range endpoint, collection index, or array context. There is
  no `Int` / `Float` fallback type in the active checker path.
- `pro_review11.md`: adopted canonical dialogue `look` line options, extended
  `stage` / `portrait` / `focus` / `cleanup` line options, `[mark .name]`
  zero-width dialogue markers, line-plan `on mark(.name):` handlers, generic
  line-scoped `thread` blocks, scoped `defer { ... }`, outcome-guarded
  `defer on completed|cancelled|failed`, flat `=== ... ===` fence sugar,
  `wait(mark(...))` / `wait(duration)` waits, and `'line.*`
  lifetime registry paths with optional `?` reads. Local dialogue `[hook ...]`
  and `#[hook ...]` syntax is removed; top-level engine hooks remain.

## Current Direction

- Parser work now starts from a lossless rowan CST: `SyntaxKind`,
  `ArcweftLanguage`, `SyntaxNode`, source text retention, line index, source
  hash, and always-returning `ParsedSource`.
- The typed parser now receives `CstLineEvents` projected from CST `Line`
  nodes through `From<&SyntaxNode>` instead of splitting raw source
  independently. Each projected line carries a `CstLineKind` classification for
  blank/comment/doc/code handling, and top-level dispatch now starts from
  `CstTopLevelLineKind` / `CstTopLevelItemKind` event classifications owned by
  the CST layer instead of an open-ended parser string chain. This keeps
  declaration detection distinct from AST construction while the grammar moves
  toward rowan events. Flow-body dispatch now likewise starts from CST-owned
  `CstFlowItemKind`, `CstStructuredFlowBlockKind`, and `CstLetFlowItemKind`
  classifications so the typed parser receives a syntax-family event before it
  calls the existing AST builders. Shared balanced
  scans for delimiters, top-level punctuation, top-level keywords, top-level
  whitespace, leading identifiers, lifetimes, entity refs, relative IDs, and
  matching punctuation live in the CST layer so expression, type, pattern, and
  top-level parsing do not grow separate ad hoc splitters. Current line-event
  parsing uses those CST helpers for multiline delimiter recovery, `let`/type
  binding splits, associated-type generic heads, pattern guard splits,
  multi-token separators such as `=>` / `->` / `<-` / `::`, `borrow ... as ...`,
  await grouping, await `with` heads, extern module headers, event fields,
  scenario command args, labels, entity refs, and shared pattern/type delimiter
  parsing.
- Balanced brace-block collection for ordinary blocks and function-body blocks
  now lives on `CstLineEvents` and returns a `CstBlockEvent`. The typed parser
  still consumes the result, but brace recovery and body-open detection are no
  longer duplicated in parser methods. Block open/close offsets come from the
  per-line punctuation summary built during CST line projection, so collecting a
  complete brace block does not re-lex the assembled block text.
  Normal `parse_source` line events borrow from the original source buffer
  instead of allocating a second owned `String` for every line, so parser bench
  JSON reports `line_owned_bytes = 0`. The standalone `cst_lines(root)` helper
  still owns line text for tooling contexts that only have a CST root.
  Balanced block events also borrow source-backed head/body fragments when the
  original line endings can be preserved exactly, so the checked-in parser bench
  now reports `block_owned_bytes = 0` for the normal LF source path. AST fields
  that intentionally store source text still own at the typed AST boundary.
  Non-line body fragments use `CstPunctuationScan` when a parser needs multiple
  punctuation queries over the same slice, avoiding repeated fragment lexing in
  shared helpers such as `split_brace_item`.
  Dialogue trailing `with { ... }` and bare scope blocks use the same path:
  same-line blocks reuse the fragment scan for both brace depth and brace
  splitting, while multiline continuations add each following line's stored
  punctuation summary before the final split. The same-line path stays on
  borrowed slices; only multiline continuations assemble an owned block string
  and charge it to `block_owned_bytes`.
  Logical block item splitting now yields borrowed slices for ordinary
  single-line body items and allocates only when an item spans multiple lines or
  consumes a method-chain continuation. The same splitter scans the whole body
  fragment once and reuses per-line punctuation deltas instead of lexing each
  raw body line separately.
- Flow-like block collection also lives on `CstLineEvents`. It keeps contract
  and `effects { ... }` prelude lines in the header while collecting the
  following brace body as the block event, so flow/callable/entity/source
  builders no longer own header-prelude scanning.
- Parser-facing grammar delimiter decisions have been moved out of the typed
  parser's local string scans and into CST helpers. The remaining raw
  character scans live in the CST lexer / CST text utilities themselves, where
  they tokenize source text or implement named text utilities such as line
  splitting, documentation-prefix extraction, wiki-link extraction, and
  string-literal extraction. Future grammar behavior should continue to enter
  through CST helpers or grammar-level rowan events rather than parser module
  scans.
  Cheap parser instrumentation now counts dot-continuation normalizations that
  actually allocate and dialogue/index disambiguation attempts that actually
  parse bracket content as an expression; both counters are collected from
  existing parser decisions rather than from additional scans.
- Parser recovery for flow items, choice-body items, choice-plan items, and
  line-plan items now uses a typed `RawSyntax` recovery node with grammar
  family and source span metadata. Statement parsing also enters through a
  CST-owned `CstStmtKind` classifier, and remaining unsupported statements use
  `RawSyntaxFamily::Stmt` instead of opaque strings. These nodes are
  diagnostics carriers only: HIR lowering rejects raw flow recovery nodes, and
  semantic/verifier/runtime-plan passes report raw recovery as typed
  obligations instead of treating it as executable syntax.
- CST reference helpers now keep absolute `EntityRef`, ID-context `IdRef`, and
  family-relative `EntityRefSyntax` separate. `@.suffix`, `@..suffix`,
  `@...suffix`, `@super...`, and ID-context family forms such as
  `@say:.suffix` / `@choice:.suffix` are accepted in ID-bearing contexts;
  general relative references use family-qualified forms such as `@flow:.next`
  and `@textbox:.side`. HIR lowering normalizes these structured nodes against
  the current flow, speaker, choice, and named-scope stack.
- Old `@` command and attribute spellings are no longer treated as migration
  syntax. Attributes are `#[...]`; staging operations use canonical ordinary
  calls such as `bg(@asset.bg.room, fade = 300ms)` and
  `show(@character.alice, .normal)`.
- `arcweft-dialogue` contains the current Sans I/O model for scoped
  dialogue lines, speaker presets, content, and line plans. Presentation
  staging helpers live in `arcweft-presentation`; `arcweft-dialogue` no longer
  depends on the presentation crate. Compatibility type aliases such as
  `DialogueOptions` and `VoiceRef` have been removed; Rust callers use the
  canonical `SayOptions` and `VoicePolicy` names directly.
- `arcweft-presentation` contains the Sans I/O model for scoped presentation
  handles. `bg(...)` and `show(...)` return typed
  `PresentationHandle<T>` values registered against a `PresentationTarget`,
  `PresentationSlot`, and `PresentationScope`; slots behave like typed
  static-option cells and expose read-only `SlotRef<T>` plus clear operations.
  `PresentationRegistry<T>` enforces scope lifetime at the data-model level by
  clearing registered slots when `exit_scope` is called.
- `arcweft-lang-syntax` now recognizes presentation set/read/clear calls as
  type-checkable ordinary call syntax: `bg(...)`, `show(...)`,
  `bg.ref(...)`, `show.ref(...)`, `bg.clear(...)`, and `hide(...)`. The checker validates
  `@target.*`, family-correct `@slot.background.*` /
  `@slot.character.*` usage, and reports simultaneous default slot handles
  that should be given explicit slots.
- Runtime observation APIs are ordinary call syntax. `log.info(...)`,
  `log.debug(...)`, `log.warn(...)`, `signal.set(target, value)`,
  `metric.set(target, value)`, and `event.emit(Event, fields)` parse as normal
  method calls; line-plan runtime lowering recognizes those well-known calls
  and emits typed Sans I/O `LineEffectRequest::Log`, `SignalWrite`,
  `MetricWrite`, and `EmitEvent` records.
- Dialogue syntax now parses `look`, `stage`, `portrait`, `focus`, and
  `cleanup` as first-class line options. The first positional line option maps
  to `look`; `face` is rejected as a line option while stage methods such as
  `alice.stage.look(...)` remain ordinary calls.
- Dialogue text now tokenizes `[mark .name]` into a structured marker token.
  The checker rejects duplicate marks, rejects local `[hook ...]`, and verifies
  marker-triggered line-plan `on mark(.name):` handlers against marks in the same
  line.
- Line plans now preserve `init`, generic `thread name` blocks, scoped
  `defer { ... }`, outcome-guarded `defer on completed|cancelled|failed`, `on`
  handlers, `wait` statements, and `'line.* <- expr` lifetime registry writes
  as structured AST/HIR-checkable syntax. Line cleanup now uses `defer` rather
  than a separate cleanup construct; `with:`, `with { ... }`, and flat
  `=== with ===` fences are sugar over the same line-plan model. `spawn` is
  rejected in favor of `thread`. Line-plan flat fence blocks report parser
  diagnostics for unknown fence kinds, close mismatches, and missing close
  fences instead of relying on later raw-node rejection.
- Syntax-level ID policy linting exists as `lint_id_policy`. It currently
  reports deep dot-run relative IDs such as `@...suffix` and flow IDs whose
  tail does not match the module tail. Further hierarchy checks should build on
  this pass rather than parser diagnostics.
- `pro_review12.md` P0-P2 work is partially implemented: syntax/checking now
  uses structured `LifetimeScopeKind`/`LifetimeKey`, recognizes upper-lifetime
  write capabilities such as `state.write(flow)`, and accepts source effects
  selectors such as `effects { state.write('flow) }` as capability facts.
  It rejects `'line.*` outside line scope and across thread boundaries, parses
  expression-form `thread`, keeps function parameter defaults, supports `&`
  patch merge parsing/checking, and recognizes surface aliases plus
  voice/se/bgm/bus/mix/ducking/motion/rig entity families.
- `pro_review13.md`: adopted Phase 1.5 as the next execution direction. The
  CLI now provides `arcw check <file.arcw> [--json]` and runs parse, HIR lowering,
  reference validation, ID policy lints, typecheck readiness, minimal typecheck,
  and line-plan runtime lowering. `arcweft-runtime-plan::line_task` exposes
  `lower_line_task_groups`, which converts checked dialogue line plans into
  `arcweft-core::line_task::LineTaskGroup` values without renderer/audio/device backends.
  Scoped `defer` lowers as cleanup on the current runtime scope rather than as
  thread-only syntax.
- Phase 1.5 line-plan lowering now preserves line options, line-local `let`
  bindings, `out`, `cancel on`, memo directives, assertions, structured logs,
  signal writes, metric writes, `event.emit(...)` calls, scenario commands, and
  ordinary calls as typed runtime IR categories rather than dropping them or
  collapsing them into a stringly signal placeholder.
- `pro_review16.md`: line-plan runtime data now uses a structured
  `LineTaskScope` / `LineTaskNode` graph instead of flat `init` and `children`
  vectors. `thread`, `on`, and `at` lower to child tasks with stable task IDs,
  task keys, triggers, priority, join policy, and cancel policy. `start` and
  `together` preserve their graph boundaries, and `together` runs an initial
  deterministic access-conflict check for signal/lifetime/control/output writes
  while allowing append-only logs and events. Handler and child-task typecheck
  scopes are isolated so locals, line guarantees, and dropped-lifetime state do
  not leak across task or line boundaries.
- `arcweft-core` now exposes initial Sans I/O task/source event envelopes:
  `TaskSpec`, `HostTaskRequest`, `TaskEvent`, `TaskHost`,
  `normalize_task_events`, `SourcePolicy`, `BackpressurePolicy`,
  `ReplayPolicy`, and `SourceEvent`. `TaskSpec` carries a typed host request
  plus a diagnostics-only `debug_label`; file, HTTP, process, asset, shader,
  audio, TTS, Wasm, and custom capability requests are pure data contracts for
  host adapters. No Tokio/Rayon/filesystem, device, audio, or GPU runtime is
  linked into core.
- Phase 2.0 structured headless runtime work is implemented in `arcweft-core`:
  `RuntimePlan`, `RuntimeFlow`, `FlowOp`, `RuntimeValue`, `RuntimeExpr`,
  `RuntimePattern`, `RuntimeEnv`, `Engine`, `FlowFiber`, and
  `run_line_task_group` can step lowered flow/dialogue task graphs over
  `RuntimeStepInput` into `RuntimeStepOutput` without performing I/O. The spine emits
  child/await `TaskSpec`s and deterministic line effects, evaluates
  let/let-else, if/if-let, match, loop/while/while-let/for, scope, goto, and
  return runtime nodes, runs scope cleanup stacks, and leaves actual
  native/cooperative/web execution to adapters.
- `arcweft-runtime-plan::flow` now exposes `lower_runtime_plan`, which converts
  checked HIR flows to core `RuntimePlan` data for the Phase 2.0 execution
  slice. Runtime lowering supports dialogue, `choice`, `await with`, typed
  `let`, `let else`, structured `if`, `if let`, `match`, `loop`, `while`,
  `while let`, `for`, `scope`, dynamic `goto`, dynamic `return`, flow-level
  ordinary effects, `out`, and line `cancel on` rules. Unsupported executable
  flow items fail lowering explicitly instead of being converted to `Noop`.
- `arcw plan <file.arcw> [--json]` now exposes lowered line task graph metadata
  for CLI, LSP, and Agent inspection. Runtime parallel conflicts are also
  surfaced as verifier obligations so direct verifier users can see the same
  class of graph conflict as `arcw check`.
- `arcw run <file.arcw> [--steps N] [--mode one-op|drain|game|server] [--max-ops N] [--value name=value] [--json]` now
  performs a deterministic dry run through the Phase 2.0 headless runtime slice and
  reports per-step flow events, effects, host requests, diagnostics, stop reason,
  and final fiber status. `--value` injects pure runtime bindings such as
  `ready=true`, `count=3`, or `route=@flow.next`; the CLI owns filesystem I/O
  and runtime execution remains Sans I/O.
- Runtime stepping now uses the shared `RuntimeExecutor` trait. `VmExecutor`
  wraps the semantic `Engine` implementation used by CLI and tests, and
  `Engine::step` enforces `RuntimeStepMode::{OneOp, Drain, Game, Server}` plus
  `RuntimeStepBudget::max_ops` inside the VM loop. `Game` mode returns on
  presentation-visible output while pure observations can drain to a harder
  boundary.
- `arcweft-core::bytecode` provides a pure `BytecodeProgram` bundle and
  deterministic bytecode stats. `arcweft-core::aot` provides a pure `AotProgram`
  bundle with flow dispatch-shape and operation-class stats without retaining
  the full runtime plan. `BytecodeVmExecutor` executes bytecode through the
  semantic VM, and `AotExecutor` owns the AOT artifact before using a core-local
  linear fast path for supported straight-line flow ops. Unsupported or stateful
  cases fall back to the same VM-compatible state machine so VM, bytecode, AOT,
  and future JIT tiers have a shared conformance boundary while generated
  dispatch is expanded.
- CLI runtime stepping now routes `arcw run`, `arcw cli`, `arcw test`, and
  `arcw profile` through the selected runtime executor. Run/CLI JSON reports the
  typed `executor = "bytecode_vm"` or `executor = "aot"` tier as an explicit
  conformance and performance observation, and `arcw profile --json` includes
  bytecode and AOT lowering time plus deterministic bytecode
  flow/instruction/source/stream counters and AOT linear/mixed dispatch
  counters.
- `arcweft-runtime-accelerator` now owns the pure-helper execution policy used
  by ordinary flow execution. The CLI and launch profiles can select
  `auto`/`vm`/`aot`/`jit`, `auto` or fixed worker counts, and a per-worker batch
  threshold.
  Scalar pure helper calls use the fixed `i64` argument pack in both the default
  VM backend and adapter accelerators; batch AOT calls can use an
  accelerator-owned Rayon pool when the batch length exceeds the threshold
  multiplied by the resolved worker count.
  Batch AOT evaluation reuses scratch slot storage instead of cloning the
  compiled local-slot vector for each item.
  Runtime JSON reports scalar/batch call counts, copied
  argument/result bytes, thread-pool jobs, Vec argument allocations, fallback
  counts, compile attempts, cache hits/misses, and compile elapsed time without
  writing host absolute paths.
- `arcweft-runtime-host::native_task` owns the native task bridge for the first real I/O
  slice. It completes `fs.read_text`, `fs.read_bytes`, `fs.write_text`, and
  `fs.write_bytes` task requests as VM `TaskEvent` input on the next step,
  resolving virtual paths under source-local `.arcweft/<space>/...` roots while
  keeping `arcweft-core` Sans I/O. The bridge is used by `arcw run`,
  `arcw cli`, `arcw test`, `arcw bench`, and `arcw profile` runtime stepping so
  headless correctness and timing runs can include real file reads/writes.
  Runtime JSON reports include native I/O counters for completed and failed
  tasks, read/write operations, and bytes read/written without recording host
  paths.
- `arcweft-runtime-scheduler` is the first Sans I/O scheduler crate. It depends
  only on `arcweft-core`, accepts `TaskSpec` values, deduplicates in-flight
  `JoinSameKey` work, dispatches by priority and stable submission order,
  records cancellation requests as data, normalizes completed events, and
  exposes scheduler counters. The CLI native task bridge now routes file tasks,
  line-plan child task markers, and source-level flow `thread` markers through
  this scheduler before performing adapter-owned completion work. Joinable flow
  `thread` blocks lower to a deterministic scheduler marker plus a scoped VM
  child fiber; their bodies now share the ordinary flow-item AST/HIR path, so
  `try await` and other await-rich flow items lower without statement-only
  parser branches. Detached flow threads remain rejected until the detach
  contract is checked explicitly. Child-fiber activity checks use the queue
  length directly because completed/failed children are removed when stepped,
  avoiding repeated scans during return and stop-reason decisions. Task policy
  is represented as a copied enum in the scheduler hot path, so join and
  always-start submission no longer clone policy values.
- The scheduler tracks whether pending tasks are already in deterministic
  priority/submission order and skips dispatch sorting for already ordered
  batches. Scheduler stats expose `dispatch_sorts` and `dispatch_sort_items` so
  thread-heavy benches can distinguish actual scheduling sort work from task
  completion work.
- Task event normalization now checks whether completion events are already in
  replay-stable order before sorting, and uses reference comparison when a sort
  is necessary. This keeps deterministic replay ordering while avoiding
  per-event task-id cloning on the common ordered native completion path.
- Scheduler stats expose `completion_sorts`, `completion_sort_items`,
  `completion_normalization_passes`, `completion_normalization_checks`,
  `completion_events_in`, `completion_events_joined`, `completion_events_out`,
  `completion_sort_skipped_items`, `completion_sort_performed_items`, and
  `joined_completion_events_emitted`. This separates sorting work from
  already-normalized completion checks and joined-event fanout in CLI
  `native_io.scheduler` bench and profile output.
- Scheduler stats also expose `submitted_by_class`, `dispatched_by_class`, and
  `completed_by_class` counters for every `TaskClass`, keeping task-class
  breakdown in the Sans I/O scheduler while host timing and worker-pool work
  stay in the CLI/native bridge layer.
- The CLI/native bridge reports phase elapsed counters separately from the
  Sans I/O scheduler: `scheduler_submit_elapsed_ns`,
  `scheduler_dispatch_elapsed_ns`, `host_complete_elapsed_ns`,
  `event_build_elapsed_ns`, and `scheduler_complete_elapsed_ns`. These are
  host-side measurements in `native_io`, not deterministic scheduler state.
- The CLI native task bridge now completes read-only dispatched task batches
  and host system-info reads on a worker pool and reports path-free
  `parallel_batches`, `parallel_tasks`, `parallel_io_tasks`,
  `parallel_system_info_tasks`, `parallel_marker_tasks`, and
  `parallel_workers` counters in `native_io`; write tasks stay ordered. The
  split counters keep actual adapter I/O and host system reads separate from
  scheduler marker completions in thread-heavy flows.
- The native bridge keeps marker-only batches on the serial path, avoiding
  Rayon worker-pool startup for cheap runtime bookkeeping while preserving
  parallel completion for read-only file I/O and host system-info batches.
- `tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw`
  provides a checked-in path-free bench fixture for direct CLI measurement of
  source-level `thread` fanout, child-fiber activity, and scheduler sort
  counters. The checked fixture regression also runs with a wide VM op budget
  so the native bridge must report a three-task scheduler batch with parallel
  marker completion and `max_in_flight = 3`.
- `tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw`
  adds a checked-in path-free native scheduling bench that runs three
  `system.*` host requests inside source-level threads and records system info
  task counts, worker fanout, and scheduler in-flight counters.
- `tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw`
  covers a natural unannotated deterministic helper being inferred as pure,
  batched, and JIT-compiled without argument vector allocation.
- `tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw`
  covers a branchy i64 pure helper through both `map(...).sum()` and a
  `for`/`if` loop, giving the runtime bench harness a mixed iter workload for
  JIT, VM-op, line-effect, and local-sequence sum counters.
- Runtime if-let/match guards, source handlers, stream pattern bodies, and
  await-many request templates now evaluate temporary bindings in environment
  scopes instead of cloning the full VM environment, reducing branch and
  scheduling overhead without changing binding visibility. Guards, map fallback
  evaluation, and await-many request templates use borrowed temporary binding
  insertion, avoiding extra `RuntimeBinding` vector/value clones before the
  scoped environment owns the actual temporary values. Runtime call
  argument evaluation preallocates the visible argument count before handling
  spread expansion, avoiding repeated Vec growth for ordinary calls.
- Runtime `for` state shares evaluated item sequences with `Arc<[RuntimeValue]>`
  across `ForNext` steps, so natural loops no longer clone the full source
  vector on every iteration. Each iteration now borrows the current item during
  pattern matching, avoiding an unconditional per-item `RuntimeValue` clone
  before binding.
- Runtime `ForNext` continuations also share their lowered loop body as
  `Arc<[FlowOp]>`, so each iteration keeps a cheap continuation handle instead
  of cloning the whole body into the next continuation.
- `ForNext` now opens the iteration scope and binds the loop item directly when
  the continuation runs, then queues only the body, `ExitScope`, and next
  continuation. A branching for-loop pure-call bench dropped from 31 to 23 VM
  ops per run while keeping JIT calls and arg-vector allocations unchanged.
- `LoopNext`, `WhileNext`, and `WhileLetNext` continuations now also share their
  lowered loop bodies as `Arc<[FlowOp]>`, avoiding body clones on repeated
  loop iterations and `continue` paths.
- Flow scoped-operation scheduling pushes `EnterScope`, body ops, `ExitScope`,
  and loop continuations directly into the VM pending queue. Loop, while,
  while-let, and for iterations avoid building temporary scoped `Vec<FlowOp>`
  buffers before execution.
- Runtime environment scopes now use compact ordered binding vectors instead of
  per-scope maps. Typical flow/function scopes are small, so local lookup and
  `let` binding avoid tree-map fixed costs while preserving deterministic
  visibility.
- Stream stepping temporarily takes the immutable stream-plan list while running
  stream ops, then restores it after the step, avoiding a full stream-plan clone
  on every runtime step.
- Suspended await/choice/await-many resume now moves the current fiber status
  out for dispatch instead of cloning the whole suspended state, and selected
  choice/await-many entries are moved where possible.
- The VM builds a deterministic flow-ID index when `Engine` is created, so VM
  and AOT-linear stepping fetch the current flow without scanning the runtime
  plan's flow list for every operation.
- Runtime pure-call evaluation keeps pure helper metadata borrowed from the
  runtime plan instead of cloning the helper on each scalar JIT/AOT/VM call.
- Runtime pure helper metadata records typed input kinds instead of treating
  every input name as an implicit integer. The VM/JIT/AOT i64 fast path is
  selected only for helpers whose inputs are all i64 and whose expression shape
  returns i64; value-shaped helpers stay on the VM value backend without a
  misleading integer probe.
- Engine construction precomputes that i64 call-shape bit per pure helper, so
  scalar calls and map/bracket batch detection do not rescan helper input kinds
  while stepping hot runtime paths.
- Runtime `sum()` over a local tuple/bracket sequence now borrows the stored
  sequence and folds i64 items directly, avoiding a clone of already
  materialized map results before summing.
- The pure VM fallback uses the same shared borrowed i64 sequence fold, so
  helper-local `sum()` expressions do not clone local tuple/bracket sequence
  values before reducing them.
- Runtime-plan lowering folds `let tmp = values.map(...); let total = tmp.sum()`
  into `let total = values.map(...).sum()` when `tmp` is not used later, so
  naturally written map-then-sum code can use the existing fused batch path.
  The optimizer no longer builds a local-use suffix table for each flow slice.
  It first recognizes concrete map/sum candidate windows, then scans later ops
  only for the specific local names that must be dead. It also fuses the common
  `let values = [...]; let tmp =
  values.map(...); let total = tmp.sum()` window in one rewrite while keeping
  the sequence binding when the map body itself reads that local.
- Runtime-plan flow, source, and stream lowering now receive the pure-helper
  map before they lower expressions, so ordinary calls to known pure helpers
  become `RuntimeExpr::PureCall` at expression construction time. The later
  plan-finalization pass optimizes flow map/sum windows without walking flow,
  source, or stream ops just to rewrite pure calls. Profile and bench JSON now
  expose runtime-plan lowering counters for pure helpers, zero plan-wide pure
  rewrite visits, pure candidate discovery attempts, inferred discovery
  failures, lowered/cloned pure expression node counts, optimized flow slices,
  targeted local-use scans, map/sum fusions, sequence-source inlines, and
  remaining `PureCall` expressions. Pure candidate extraction caches a
  `PureHelperShape`, so runtime-plan helper construction reuses the scalar
  support flag instead of rescanning the lowered helper body.
- The fused runtime map batch path now borrows local or literal tuple/bracket
  sequence sources while packing flat `i64` inputs, avoiding a `RuntimeValue`
  sequence clone before crossing into VM/AOT/JIT pure helper backends.
- Fixed-size `Array<T, N>` receivers now participate in the same read-only
  iterable `map`/`sum` typing path as `Vec<T>`/slices. Literal array-repeat
  runtime lowering stores repeated literal values as one sequence value instead
  of expanding thousands of runtime expression nodes, keeping large batch
  runtime type validation proportional to program shape rather than repeat
  length.
- Fused `sum(map(...))` runtime paths now call a flat pure batch sum API, so
  VM/AOT/JIT backends can accumulate row results without materializing or
  copying an intermediate output vector. JIT emits a dedicated Cranelift
  rows-batch-sum function for the same automatic flow-level acceleration path.
- When a fused `sum(map(...))` sees repeated identical `i64` input rows, the
  runtime calls the pure helper once and multiplies the result by the logical
  row count. The bench counters still report the logical batch size while
  `arg_bytes_borrowed` drops to the single repeated row.
- `tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw`
  covers the complementary non-repeated map/sum path so JIT/AOT/VM batch
  input packing, result-copy elimination, and runtime type validation work can
  be compared without triggering the repeated-row shortcut.
- Literal bracket sequences now lower to a single `RuntimeValue::Seq` when every
  element is already a runtime value. Integer-only numeric bracket sequences use
  suffix-aware dense sequence storage for fixed-width integer types, so
  non-repeated literal input benches keep runtime type validation proportional
  to the flow shape instead of the literal element count.
- Type judgments keep expected-type evidence as a compact relation when the
  expected type matches the inferred judgment type. `expected_type()` preserves
  the reporting view, but the checker no longer clones another `TypeKind` for
  the common identical expected/actual case.
- Dense sequence storage is generic at the backing-store layer and exposes
  borrowed views for deterministic scalar integer widths accepted by the
  runtime (`i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `u8`, `u16`, `u32`,
  `u64`, `u128`, `usize`) plus unit, bool, byte, char, logical duration,
  `String`, native `f32`/`f64`, and entity reference sequences. Unit dense
  storage is length-only, so `Vec<Unit>` and repeated unit arrays do not
  allocate element storage. `isize`/`usize` dense storage uses stable
  `i64`/`u64` backing values at the runtime boundary rather than host
  pointer-width buffers. `u8` dense storage is also available through the byte
  view so byte-oriented host paths can borrow it without materializing
  `RuntimeValue` elements. Textual/entity dense storage is intentionally a
  homogeneous backing store, not a numeric ABI path; string interning or
  columnar record storage remains a separate optimization. `DenseSeqKind`
  records this scalar coverage explicitly so adding a new dense class requires
  extending the typed kind, borrowed view, materialization fallback, and tests
  together.
- Scalar integer values keep the same width evidence after dynamic
  materialization. Runtime integers are represented as
  `RuntimeValue::Int(RuntimeInt::...)` or
  `RuntimeValue::UInt(RuntimeUInt::...)`, so `i32` and `u8` values that leave a
  dense sequence through spread, pattern fallback, record/tuple column
  materialization, or dynamic arguments do not silently widen to `i64`/`u64`.
  The i64 VM/AOT/JIT fast paths now explicitly require `RuntimeInt::I64`, while
  width-specific pure paths keep their exact ABI type.
- Dense storage exposes exact borrowed views for each scalar storage class.
  Narrower or unsigned integer storage is not widened with `.map(i64::from)` on
  the hot path, because that would erase the bandwidth/cache advantage of
  `i32`, `u16`, `u8`, and similar dense storage. Runtime pure helper metadata
  preserves source integer input and output widths (`i8` through `i128`, `u8`
  through `u128`, `isize`, `usize`, and `bool` outputs). The exact i64
  accelerator still owns JIT/AOT execution, while the VM flat-batch sum path
  keeps exact integer helper rows typed as `&[i8]`, `&[i16]`, `&[i32]`,
  `&[u8]`, `&[u16]`, `&[u32]`, `&[u64]`, `&[i128]`, or `&[u128]` instead of
  widening them with `.map(i64::from)`. The checked-in dense i32 pure map bench
  verifies that the pure boundary borrows 1024 bytes for 128 two-arg rows
  rather than widening to 2048 bytes, and the multi-width flow tests assert the
  same byte accounting for narrower and unsigned integer widths. JIT/AOT
  specialization remains exact-i64-only until typed accelerator kernels are
  added for the other widths.
- `Vec<T>.len()` / `Seq<T>.len()` / fixed array length calls typecheck as
  `usize` and read `RuntimeSeq::len()` directly in the VM and pure VM. The
  checked-in dense scalar length bench exercises unit, bool, char, duration,
  and `u8` dense storage without crossing a dynamic materialization boundary.
- Literal array repeats now lower to a structured runtime repeat expression
  instead of materializing a large sequence in the runtime plan. Fused
  `map(...).sum()` paths over repeated sources call the repeated-row pure batch
  boundary directly, so `[value; N]` keeps logical batch counters without
  cloning or scanning `N` runtime values.
- Runtime evaluation now keeps repeated scalar values dense when the repeated
  value is a deterministic scalar (`unit`, `bool`, signed/unsigned integer,
  `char`, logical duration, string, raw float literal, or entity reference).
  Dynamic bracket sequences also fold homogeneous evaluated scalar values into
  dense storage instead of leaving them as `Vec<RuntimeValue>`.
- Fast-path scalar pure calls read local integer arguments by borrow when
  packing `RuntimeI64Args`, avoiding a `RuntimeValue` clone before crossing into
  VM/AOT/JIT pure backends.
  Scalar `i64` pure-call stats also record stack-pack, argument byte-copy, and
  result byte-copy counters, matching batch pure-call reports and making the
  ordinary flow call boundary visible in `arcw run --json`,
  `arcw profile --json`, and `arcw bench --json`.
- Runtime statement `match` now moves the selected arm body out of the owned
  `FlowOp::Match` being executed instead of cloning that body again. CLI bench
  coverage includes a runtime match that jumps into a JIT-backed pure helper
  flow and records VM op count, pure call count, and zero arg-vector allocation.
- Simple `for item in [i64, ...]` iteration binds the item through an integer
  setter, so scalar pure-call loops no longer clone a `RuntimeValue` just to
  make the current integer item visible in the loop scope.
- The CLI/player pure accelerator stores compiled helper entries in dense
  helper-ID slots instead of a map, reducing scalar pure-call dispatch overhead
  while preserving deterministic cache statistics.
- Scalar Cranelift helpers store an arity-typed native caller when compilation
  finishes, so repeated flow calls no longer reinterpret the finalized code
  pointer on every JIT invocation.
- Runtime JIT scalar calls now pass the fixed `RuntimeI64Args` pack directly
  into the compiled helper instead of first re-expressing the arguments as a
  dynamic slice.
- Scalar AOT helper calls reuse accelerator-owned slot scratch storage instead
  of cloning the plan's initial slot vector on every flow invocation.
- `RuntimePureCallBackend` now exposes row-major `i64` batch calls in the
  Sans I/O core trait. The default VM backend records deterministic batch
  counters, while the runtime accelerator overrides the same boundary with
  AOT/JIT and worker-pool execution.
- Runtime bracket sequence expressions containing only the same statically
  integer-shaped pure helper call now evaluate through the batch trait boundary,
  giving ordinary collection-style source a path to AOT/JIT batch execution.
- The same bracket-sequence path now packs evaluated integer inputs into one
  row-major slice and calls the flat batch backend boundary. Natural source
  batches therefore avoid per-row `RuntimeI64Args` stack-pack accounting at the
  accelerator boundary, expose borrowed-input/result-copy bytes in runtime bench
  JSON, and can use the configured AOT worker pool when the per-worker batch
  threshold is met. The VM engine keeps a reusable row-major input scratch buffer so repeated
  collection batches do not allocate a fresh input vector on every evaluation.
  Engine construction also caches each pure helper's conservative integer-result
  shape, so repeated collection-batch eligibility checks no longer rescan helper
  expression trees.
- Runtime bench deterministic summaries now include median pure batch-call
  counts, flat-batch row/input counters, flatten materialization/copy counters,
  and JIT/AOT/VM/fallback pure-call counts, making backend selection and batch
  execution visible without inspecting per-step traces.
- Runtime executable expressions now have a typed `map` node lowered from
  one-parameter closure method calls such as `values.map(|item| score(item,
  2i64))`. The VM evaluates ordinary maps sequentially, but maps whose body is a
  statically integer-shaped pure helper call use the same flat batch boundary as
  bracket-sequence batches, so natural iterator-style source can use JIT/AOT
  pure batching without explicit batching syntax. The VM reuses scratch buffers
  for both flat `i64` batch inputs and batch `i64` outputs before constructing
  the returned runtime sequence.
- Runtime executable expressions also lower `.sum()` over strict runtime
  sequences to a typed sum node. When the source is a pure-call `map`, the VM
  fuses map plus sum into one flat batch accelerator call and sums the `i64`
  result scratch directly instead of materializing an intermediate runtime
  sequence. The same direct-sum path covers bracket sequences made of same
  helper `i64` pure calls.
- Semantic type checking for `Vec.map` now uses the closure body type rather
  than assuming the output item type matches the input item type, and `Vec.sum`
  is accepted only for integer item vectors. This keeps iterator-style runtime
  acceleration aligned with actual source-level types.
- Cranelift input helpers now include a row-major batch entry point that accepts
  input and output slices through the native adapter boundary. Runtime pure
  batch execution can call JIT once per batch instead of crossing the
  Rust/native boundary once per row, and the accelerator reuses its flat
  integer input scratch buffer across batches. CLI pure-helper bench now feeds
  the runtime accelerator with flat row-major inputs directly, avoiding an
  intermediate `RuntimeI64Args` row vector for measured batches. Flat batch
  stats therefore keep `arg_stack_packs` and `arg_bytes_copied` at zero while
  reporting the shared input slice through `arg_bytes_borrowed` and the output
  write volume through `result_bytes_copied`. Row-batch JIT fallback remains
  available, but it now reports `flatten_materializations` and
  `flatten_bytes_copied` when it has to build the adapter-owned flat input
  buffer.
  Scalar `i64` pure calls return through the typed call result instead of an
  output buffer, so they no longer report result-byte copies.
- Sequential AOT pure batches now reuse the accelerator-owned `i64` scratch
  slots instead of allocating a local scratch vector per batch. Parallel AOT
  batches keep thread-local scratch slots for worker isolation.
- CLI runtime pure-helper batch measurements now reuse row-major input and
  output scratch buffers across samples, keeping large JIT/AOT comparison runs
  from adding per-sample benchmark-harness allocations.
- VM pure-helper `i64` fallback calls now reuse scratch runtime environments
  and update matching root input bindings in place instead of allocating and
  reinserting every argument through repeated lookup. This reduces fallback
  overhead while keeping JIT/AOT as the automatic fast path.
- Awaited `system.core_count()`, `system.thread_count()`, and
  `system.available_parallelism()` calls now lower to typed system-info task
  requests. The CLI adapter resolves physical cores, logical CPUs, and
  process-available parallelism separately, reports `system_info_ops`, and
  keeps the JSON output path-free.
- `arcw run --json`, `arcw profile --json`, and measured `arcw bench --json`
  sections now include a path-free `host_system` summary with physical core,
  logical thread, and process-available parallelism counts so performance
  samples can be interpreted without embedding host paths.
- `arcw toolchain-profile --json` reports the same `host_system` summary for
  cargo fmt/check/clippy/test timing samples, keeping compiler and borrow/type
  checking measurements comparable across machines without host path leakage.
- Path-free local trend samples are recorded in
  `docs/implementation/performance-snapshot.md`; they contain host core/thread
  counts and timing counters, but no workspace or source absolute paths.
- `arcw jit check --json` now includes the same path-free `host_system`
  summary, so pure JIT/AOT/VM and optional Julia comparisons carry core/thread
  context without recording host filesystem paths.
- The `arcw jit check` VM baseline measurement loop now uses the reusable
  `VmPureFunctionScratch` i64 path and stack input arrays instead of allocating
  a fresh pure-function request and binding vector per iteration. The
  conformance check still uses the full VM backend, while timings better
  isolate VM expression evaluation from benchmark harness allocation.
- The scalar JIT measurement loop in `arcw jit check` now calls compiled
  helpers through the same fixed `RuntimeI64Args` boundary used by runtime flow
  pure calls, avoiding per-iteration slice dispatch in the CLI harness.
- Runtime step JSON summarization now moves `TaskSpec` requests out of each
  step result after deriving display labels, rather than cloning the full task
  request list before native completion. Thread and native I/O benches therefore
  measure one fewer host-side copy at the VM/native scheduling boundary.
- The VM runtime step now moves owned root input bindings into the fiber
  environment instead of cloning them again inside `Engine::step`, matching the
  AOT fast path ownership model and reducing per-step adapter binding copy work.
- The AOT executor now checks the linear-dispatch precondition by borrowing the
  step input before dispatch, so AOT success and fallback paths no longer clone
  the full `RuntimeStepInput` just to probe the fast path.
- CLI runtime stepping now passes route/argument root bindings to VM and AOT
  executors as a borrowed slice, avoiding `values.to_vec()` allocation before
  each measured step while preserving the owned step-input path for adapters
  that need to transfer events.
- `arcw bench` flow measurement now uses a compact runtime trace that stores
  raw step counters, task request counts, executor stats, and native I/O stats
  without constructing display labels or observation summaries inside the
  measured loop. `arcw run` and `arcw profile` still build the full JSON step
  summaries.
- Flow bench sections now build the selected-entry runtime plan once before
  sampling and clone that ready-to-run plan for each isolated run, avoiding
  repeated entry-flow string construction inside the measurement loop.
- Bench flow sections now compile the pure accelerator once per section and
  reset runtime counters per sample, so steady-state runtime measurements do
  not include repeated JIT/AOT helper compilation.
- `arcw profile --json` and `arcw verify-types --run --json` now report
  executor and pure accelerator construction in an `executor_prepare` phase
  before `run`, so runtime execution timing is not blurred with JIT/AOT helper
  preparation.
- Flow `for` loops now bind simple identifier and discard patterns directly
  instead of allocating an intermediate pattern-binding vector for each
  iteration; structured patterns still use the full matcher.
- Scalar pure helper calls from normal flow execution now pass evaluated `i64`
  arguments to VM/JIT/AOT backends as borrowed slices, eliminating the
  fixed-pack value copy from the common non-batched call path.
- Borrowed root binding updates now reuse existing environment slots without
  recloning binding names, so repeated runtime steps only clone the value when
  an adapter-provided root binding is already present.
- Borrowed root binding updates also fast-path the common same-order binding
  set, updating root values in one pass instead of performing one linear name
  lookup per binding on every measured runtime step.
- Type checking no longer clones a function's full effect-capability list before
  validating a call. The checker now borrows the declared effect slice and
  materializes only missing capability names needed for diagnostics.
- Type checking now uses scoped local binding snapshots for flow/statement/value
  match arms plus flow-level await, if-let, while-let, and for bindings instead
  of cloning the full locals table for every branch or loop binding scope.
- Type-checker local bindings now flow through a single `bind_local` path with
  mutation-log scopes, so nested statement, choice, and block-expression bodies
  can restore locals without cloning the whole locals table.
- Source event normalization now sorts by borrowed source id and sequence
  comparisons instead of constructing cloned sort keys, reducing per-step work
  for source-heavy and stream-heavy runtime benches.
- Source event normalization also skips sorting when adapter events are already
  in replay-stable source/sequence order, matching the task-event fast path.
- Native task completion now consumes moved `TaskSpec` request vectors and
  submits supported tasks directly into the scheduler, removing the remaining
  task clone between runtime JSON summarization and host scheduling.
- The CLI regression harness now rejects generated `.arcweft` directories under
  checked-in fixtures and scans non-review source/docs/tests for removed
  whitespace-command DSL or compatibility-shim text. Run fixtures execute from
  temporary copies so native file I/O benchmarks do not leave repository-local
  runtime artifacts.
- Phase 2.0 headless observation state is implemented for the current runtime
  slice. `arcweft-core` records
  cumulative log, signal, metric, and event observations from emitted
  `LineEffectRequest` values without performing host I/O, and
  `arcw run --json` exposes those observations for CLI, test, LSP, replay, and
  Agent tooling.
- Source and stream runtime execution now has a first Sans I/O slice.
  `RuntimeStepInput.source_events` are normalized, dispatched through lowered
  `SourcePlan` handlers, and `yield` pushes structured `RuntimePayload` items
  through the declared
  backpressure policy. `StreamPlan` can drain source/stream queues through
  `ForNext` and emit deterministic stream events within a per-step budget.
  Flow `for` loops also lower to bounded `ForNext` continuations instead of
  unrolling the whole iteration space into a single step queue.
  `arcw plan --json` reports generation plans, and `arcw run --json` reports
  source/stream events and queue state. CLI output renders payload labels for
  display while the Sans I/O boundary keeps `RuntimeValue` shape for replay and
  downstream runtime consumers. Device acquisition, permissions, and native
  callbacks remain adapter responsibilities.
- The Phase 2.0 runtime now has an explicit `FlowFiber` control stack for lexical
  scopes and loop continuations. `break` and `continue` discard queued body ops,
  pop body-local scopes, and transfer to the nearest loop/while/while-let entry.
  Branch, match, and while-let pattern bindings are scoped to the selected body;
  guard evaluation uses temporary bindings and restores the previous runtime
  environment. `RuntimeStepInput::bindings` bind into the root runtime scope so
  ambient per-step values are not lost when a nested scope exits.
- Bytecode VM artifacts preserve pure-helper metadata alongside flow ops,
  entries, line-task groups, and source/stream plans. `arcw bench` and
  `arcw profile` therefore exercise the same automatic pure JIT/AOT call path
  as `arcw run`, including natural `for` loop bodies that call pure helpers.
  Auto pure helpers now separate cold and warm tiers: supported i64 helpers
  start from typed AOT with JIT deferred, and large flat batches promote the
  helper to JIT at runtime. Executor JSON reports the Auto decisions and
  promotions so the JIT compile cost stays visible instead of being hidden
  inside the default path.
- Gap audit result: broad runtime docs still exceed the Phase 2.0 headless
  target. Full story VM value execution, complete expression evaluation, source
  adapter execution, hook/memo runtime tables, save/replay traces, activities,
  layered input routing, and full stream operators are beyond the
  current Sans I/O runtime slice. The implemented slice executes
  `let name = scope { ... }` value bindings and `let name = loop { break expr }`
  result binding in the headless runtime.
- `pro_review14.md` / `pro_review15.md`: adopted proof-aware
  lifetime/thread/drop direction and Agent-friendly tooling diagnostics.
  Formal `proof @proof.*` items, `trusted axiom @axiom.*` declarations,
  explicit proof references such as `proof = @proof.id`, and audited
  `unsafe lifetime @unsafe.*` regions with required `reason` and `SAFETY`
  documentation are the accepted design. The syntax crate preserves proof and
  trusted-axiom items as HIR metadata and parses `unsafe lifetime` audit blocks
  as structured statements. `arcweft-lang-hir` is now the public HIR facade.
  `arcweft-lang-sema` now owns the first `SemanticReport` pass for CFG-aware
  lifetime/drop/thread/write analysis. The pass carries path-sensitive
  `FlowFacts`, applies `defer` cleanup by completed/cancelled/failed outcome,
  runs bounded fixed-point loop analysis for `break`/`continue`, checks proof
  references against the promoted lifetime target, validates that unsafe audit
  blocks contain the unchecked promotion they justify, and prefers
  semantic-owned obligations over the older verifier scan.
  `arcweft-verify` merges that report with shared JSON diagnostics for lifetime
  promotion, unsafe audits, upper-lifetime writes, effect capabilities, thread
  capture, thread join typing, MustDrop discharge, trusted assumptions, raw
  syntax, and simple runtime write conflicts. Solver dependencies are isolated in
  `arcweft-verify-z3` and `arcweft-verify-oxiz`; CLI/LSP consume verifier
  reports rather than reimplementing validation.
- `Char` / `TextCluster` are now part of the accepted primitive model. `Char`
  is a Unicode scalar value and is not a visual character; `TextCluster` is the
  display/reveal/ruby/effect unit. The syntax crate parses `"x"c` char
  literals and typechecks `Char` separately from `String`.
- Capacity traits are accepted for owning collections: `WithCapacity` and
  `Reservable` expose `with_capacity`, `reserve`, `shrink`, and `shrink_to`.
  Capacity is non-observable and may be a no-op on constrained/Wasm targets.
  The syntax checker recognizes these methods for `Vec<T>`, `String`, and
  `Bytes`.
- Top-level `test @test.* KIND { ... }` and `bench @bench.* { ... }` are now
  parsed as structured declarations and lowered into HIR metadata. The
  `arcweft-test` crate extracts a Sans I/O manifest. `arcw test` now executes
  `scenario` declarations through the headless runtime when they contain
  `start(@flow.id)`, evaluates initial signal/log/no-assertion expectations, and
  reports pass/fail/skipped JSON. `arcw bench` now validates headless bench
  plans, requires `measure`, accepts `setup`/`measure`/`assert`/`report`
  sections, measures `measure` bodies that name `start(@flow...)`, and reports
  measured/validated/skipped/failed JSON. Measured bench counters include
  median task requests and task events consumed, allowing native file I/O
  sections to be timed and checked without embedding local absolute paths.
  Visual, audio, fixture, and allocation execution remain player/headless
  adapter responsibilities.
- `RuntimeStepResult` now carries deterministic `RuntimeStepStats` for executed
  VM ops, pending queue depth, incoming task/source events, emitted source/stream
  events, line effects, and diagnostics. `arcw profile --json` reports compiler
  phase timings plus those VM counters without recording absolute local source
  paths.
- `arcweft-lang-sema` now exposes `TypeCheckReport` / `TypeCheckStats` plus
  typed `TypeJudgment` evidence for successful expression, let-binding, and
  return checks.
  Expression judgment subjects store static expression-kind labels, and
  expected-type checks are represented by the judgment rule plus the expression
  subject instead of an additional context-only judgment, avoiding per-judgment
  kind allocation and duplicated expected evidence on `expect_expr_type` paths.
  `arcw check --json`, `arcw profile --json`, `arcw bench --json`, and
  `arcw verify-types --json` surface deterministic typecheck counters and
  integrated borrow-check counters, including expression/statement counts,
  borrow binding groups, type judgment counts, rule-family judgment counts,
  bounded judgment samples, borrow state snapshots/restores/merges, boundary
  checks, escape checks, active-borrow removals, delta entries, full-clone
  counts, merge-key counts, and maximum active borrows.
  Loop, source-handler, dialogue-line runtime, and child-task scopes restore
  only inserted or shadowed bindings, so typecheck performance counters are not
  distorted by full local environment clones at common scoped-binding
  boundaries.
  Borrow-state release and branch merge avoid avoidable snapshot/state clones:
  dropping a borrowed local moves the tracked state out of the map before
  updating it, and branch restore/merge paths no longer clone whole base
  snapshots just to describe control-flow paths.
  Active borrow tracking uses a deterministic counted lifetime map instead of a
  linear `Vec<String>` removal path, so duplicate lifetime labels are collapsed
  for diagnostics while release/drop updates avoid per-remove scans.
  Branch borrow-state checking now uses checkpointed journal entries and
  touched-key deltas. If/match/loop restore paths replay only changes made after
  the checkpoint, and branch merges inspect the union of changed borrow locals
  instead of cloning and merging a full `HashMap` snapshot for every path.
  Dialogue-line and child-task runtime scopes also keep borrow state as a
  checkpoint rather than cloning the tracked borrow map; their snapshots still
  preserve presentation and lifetime-scope state explicitly.
- `arcweft-verify` exposes `validate_runtime_plan_types(plan, report)` for the
  post-lowering runtime plan consumed by the VM. `arcw profile --json` now runs
  this pass between runtime-plan lowering and bytecode lowering and reports
  deterministic counters for runtime ops, expressions, conditions, guards,
  targets, returns, and type-judgment evidence.
- `arcw verify-types` is the direct CLI gate for executable type-soundness
  inspection. It keeps the successful `TypeCheckReport`, lowers to
  `RuntimePlan`, runs `validate_runtime_plan_types`, and reports typecheck,
  borrow-check, runtime-plan type validation, and semantic verifier counters in
  one JSON document without recording absolute source paths. Its JSON also
  includes compiler phase timings for read, parse, lint, HIR lowering,
  reference resolution, readiness, typecheck, line-task lowering,
  runtime-plan lowering, runtime type validation, verification, and optional
  bounded runtime execution. With `--run`, it also performs a bounded headless
  runtime progress self-check through the selected executor and records
  per-step runtime evidence plus AOT fast-path counters.
- `VerificationReport` now records solver outcomes as typed `solver_checks`.
  CLI solver I/O remains outside the Sans I/O verifier core, but `arcw verify
  --backend oxiz|z3 --json` writes each outcome back to the report. Required
  missing obligations in `test`/`release` mode fail unless the solver returns
  `unsat`.
- Declaration ID positions whose family is known now accept current-scope and
  family-relative IDs. `flow @.opening`, `flow @flow:.opening`, and bare
  `flow opening` normalize to `flow.opening`; declarations such as
  `character @.alice`, `hook @.visible`, `source @source:.events`, and
  `dialogue defaults @dialogue:.opening` follow the same rule. Empty declaration
  markers are accepted when a declaration name follows them: `flow @. opening`,
  `flow @flow:. opening`, `character @. alice Alice`, `signal @signal:. ready`,
  and `source @source:. metrics()` normalize through that following name.
- Remaining P2 semantic work is now refinement rather than missing surface
  coverage: fixed-point loop analysis is bounded and syntactic, proof discharge
  is target-aware and checks structured proof-body `ensures`/`check` targets,
  unjustified `assume` clauses, and unknown trusted axiom references; unsafe
  audits validate shape but not memory
  semantics, and thread result inference is based on current syntactic result
  labels. Effect capabilities are now represented as typed semantic facts:
  `effects { signal.write, metric.write }` on flows/functions and hook header
  `effects` entries grant the corresponding known write calls such as
  `signal.set` and `metric.set`. Semantic conflict checks now use typed
  resource access facts for lifetime/signal/metric writes. Ownership/region
  checking rejects borrowed values escaping through block finals, returns,
  line-plan `out`, or upper-lifetime registry writes. Direct explicit
  `drop(local)`, `drop_optional(local)`, `on_drop(local)`, and local `.drop()`
  statements end the tracked local borrow before suspension boundaries such as
  `await`; branch merges keep one-sided drops as maybe-dropped so they remain
  rejected before suspension or reuse. Full solver-backed proof term checking
  and type-directed effect inference remain beyond the current Phase 2.0
  semantic/verifier slice.
- Verifier JSON uses a stable adjacent-tagged representation for proof
  expressions, including string-carrying variants such as
  `{ "kind": "var", "value": "signal.write" }`.
- Phase 2.1 tooling has a first Sans I/O crate, `arcweft-tooling`, for source
  edit reports, sugar expansion, ID materialization edits, source code actions,
  and inferred-ID hints. The CLI now wires `arcw fmt` and
  `arcw ids materialize` as dry-run-by-default adapter commands with `--write`
  and `--json`; `arcweft-verify-lsp` exposes the same source actions and hints
  without owning an LSP transport. The current ID materialization table covers
  top-level declarations, explicit and omitted dialogue line `id=` /
  `text_key=` options, flat `=== line ... ===` dialogue heads, and
  choice/choice-option IDs. Canonical `with { ... }`, `with:`, and flat
  `=== with ===` line-plan attachments share the same materialization context.
- The old `arcweft-tooling` dialogue-ID line scanner has been removed from the
  tooling crate. ID materialization now flows through
  `arcweft-lang-hir::collect_id_context`, which emits typed source operations
  for declarations, choices, choice options, explicit dialogue `id` /
  `text_key` options, and omitted dialogue options. Speaker-preset discovery
  now walks the parsed typed tree instead of source lines. Tooling, CLI, and
  LSP convert typed operations into edits, hints, and actions instead of
  keeping scanner-specific logic.
- `pro_review19.md` is reflected with Rust-like collection names. The facade
  crate exposes minimal Sans I/O standard data crates through explicit
  namespaces rather than a flat compatibility prelude:
  `arcweft-adt` (`Unit`, `Never`, `Vec<T>`, `Array<T,N>`,
  `OrderedMap`/`SortedMap`/`OrderedSet`/`SortedSet`, `SmallList`, state paths, patch/diff/version/log/queue/cache
  types, source/stream descriptors, arena/slot/generational ID structures,
  deterministic tree/graph structures, ring buffers, signal buses, compiler
  node IDs, and rich-text/localization data),
  `arcweft-ref` (`Id<T>`, `Ref<T>`, `Handle<T>`, `WeakHandle<T>`,
  `Borrow`, `Slice`, `Lease`), and `arcweft-memory` (`Bytes`, `Blob`,
  `BlobRef`, `SharedSliceDesc`, `SharedSlice<T>`, `MemoryLease`,
  `PodSlice<T>`), while `arcweft-source` provides `SourceRange`, `SourceSpan`,
  and shared diagnostic bags. The language docs use `Vec<T>` for growable ordered sequences,
  `Array<T,N>` for fixed-length sequences, and `[value; N]` for fixed-length repeat
  literals. Adapter-backed implementations remain outside the Sans I/O prelude
  slice; the exported structures are data contracts only.
- Runtime value lowering is stricter for executable flow plans. Unsupported
  value-position expressions such as ordinary calls now produce runtime-plan
  lowering errors instead of being coerced into string labels; adapter-facing
  payload labels still use the existing lossy labeler where the runtime treats
  them as observational data rather than executable values.
- `pro_review21.md`: module boundaries are being treated as first-class
  architecture boundaries rather than temporary file organization. `arcweft-core`
  is split into public responsibility modules (`time`, `frame`, `value`,
  `pattern`, `effect`, `task`, `source`, `stream`, `plan`, `line_task`,
  `observation`, and `engine`) without root-level compatibility aliases.
  Downstream crates import core types through those module paths. The runtime
  engine implementation is also split by execution responsibility under
  `engine/`: `eval`, `flow`, `line`, `source`, `stream`, and `suspend`, while
  `engine.rs` owns only the engine state types, construction, frame stepping,
  and shared diagnostics/observation plumbing. The
  `arcweft-lang-sema` split now has public `check`, `checker`, `types`, `env`,
  `diagnostics`, `borrow`, and `lifetime` modules, and the checker body has
  started language-family child modules for `choice`, `effects`, `expr`,
  `flow`, `line_plan`, `presentation`, `source`, `suspension`, and `stmt`,
  plus `lifetime_access` for lifetime registry reads/writes/drops, `module`
  for module/top-level entry checks, and `borrow_state` for borrow binding and
  branch-merge helpers; `helpers` now owns shared
  type/pattern/merge/divergence helper functions used by those checker modules.
  `checker.rs` is reduced to checker state, public entrypoints, and a small
  set of shared local helpers.
  Semantic traversal and flow-fact helper families are now isolated under
  `semantic/facts.rs` and `semantic/traversal.rs`. Additional checker-family
  splits remain tracked work.
  `arcweft-runtime-plan` is split into `errors`,
  `expr`, `flow`, `labels`, `line_task`, `pattern`, `source`, and `stream`
  modules for lowering diagnostics, runtime expression/effect lowering, flow
  and whole-runtime-plan lowering, shared textual label helpers, lowered
  line-task metadata and line-plan graph lowering, runtime pattern lowering,
  source declaration lowering, and stream-function lowering. The crate root is
  now only a public module namespace.
- `arcweft-lang-hir` now exposes responsibility modules instead of flat
  compatibility exports: public consumers import HIR data through `model`,
  lowering through `lower`, ID-context tooling through `id_context`, and syntax
  ownership through the namespaced `syntax` module. The lowering implementation
  is split into public responsibility namespaces `lower_flow`,
  `lower_dialogue`, `lower_choice`, `lower_ids`, and `lower_context`.
- `arcweft-lang-syntax` has started the AST family split requested by
  `pro_review21.md`: top-level tree/item/recovery wrappers live in
  `ast/items.rs`, shared range/module/use/doc primitives live in
  `ast/common.rs`, entity/reference ID syntax lives in `ast/ids.rs`, structured
  binding syntax lives in `ast/pattern.rs`, flow/control-transfer syntax lives
  in `ast/flow.rs`, dialogue surface syntax lives in `ast/dialogue.rs`,
  line-plan syntax lives in `ast/line_plan.rs`, choice syntax lives in
  `ast/choice.rs`, proof/test/bench declarations live in `ast/proof.rs`, and
  declarative source-stream syntax lives in `ast/source.rs`. `ast.rs` is now a
  public module namespace rather than the owner of AST family definitions or a
  flat compatibility re-export layer.
- `arcweft-lang-syntax` parser splitting has started with `parser/recovery.rs`
  owning `ParseError` and `RecoverySuggestion`, `parser/source.rs` owning
  source-item header/handler/body parsing, `parser/proof.rs` owning
  proof/test item clause parsing, and `parser/line_plan.rs` owning line-plan
  body, trigger, defer, and thread parsing. `parser/choice.rs` owns choice
  top-level blocks, `let choice` bindings, choice item, arm, option block, and
  choice-plan parsing. `parser/items.rs` owns enum/struct/state field parsing
  and trait/impl member parsing. `parser/hooks.rs` owns hook item parsing and
  hook-header diagnostics. These parser
  family modules are public responsibility namespaces; recovery types are
  addressed as `parser::recovery::ParseError` /
  `parser::recovery::RecoverySuggestion` rather than through a flat
  compatibility re-export. `parser/helpers.rs` owns shared parser helpers for
  module/use path handling and attribute parsing, `parser/source.rs` owns source
  item header/body parsing, source-locale blocks, source handlers, and source
  statement helpers, `parser/top_level.rs` owns module/use/item-family dispatch,
  `parser/flow.rs` owns flow item, flow-body, scope/thread/defer, and
  bare-scope dispatch, `parser/control_flow.rs` owns structured
  flow/control blocks (`if`/`if let`/`match`/`loop`/`while`/`for`/`select`),
  value-producing `let` control-flow expressions, and shared control-flow block
  helpers, `parser/statements.rs` owns statement parsing, `let` statement
  forms, control-transfer statements, unsafe lifetime statement blocks, and
  statement-label parsing, and
  `parser/await_.rs` owns `await ... with` parsing (await `let` bindings,
  multiline await heads, and await-branch parsing), while
  `parser/dialogue.rs` owns dialogue defaults, dialogue-content calls,
  speaker-line sugar, trailing line-plan attachment, and flat dialogue/with
  fence handling. `parser/items.rs` now owns parser methods for function-like,
  enum, struct, state, trait, impl, and type-alias top-level items in addition
  to entity declarations, extern modules, memo functions, parser items, and
  item-member helpers.
  `parser/proof.rs` owns proof, trusted-axiom, test, and bench top-level parser
  methods plus proof/test clause parsing. `parser/headers.rs` owns declaration
  headers, visibility, entity/ID reference parsing, contract clauses, function
  signatures, and related header-level helpers addressed from sibling modules
  as `super::headers::*`. The parser driver still needs further slimming of
  lifecycle/error-plumbing and a small set of cross-cutting helpers, but family
  parsing and statement parsing are no longer owned by `parser.rs`.
- The application-facing `arcweft` facade no longer provides
  `arcweft::prelude::*`. It exposes namespaced crate families such as
  `arcweft::core`, `arcweft::dialogue`, `arcweft::presentation`,
  `arcweft::adt`, `arcweft::need`, and `arcweft::source` so module boundaries
  remain visible to consumers.
- `arcweft-lang-syntax` crate-root exports are module namespaces only. Downstream
  crates now import syntax-owned types through `ast::*`, `expr`, `types`,
  `parser`, `cst`, `lint`, `source`, or `text` instead of flat crate-root
  compatibility re-exports.
- `arcweft-runtime-plan` no longer depends directly on
  `arcweft-lang-syntax`; runtime lowering imports syntax-owned surface types
  through `arcweft-lang-hir::syntax::{ast, expr, types}` so the dependency
  direction remains `runtime-plan -> hir` without a flat HIR syntax prelude.
- `arcweft-core` tests are split by runtime family under `core/src/tests/`:
  frame, task, source, stream, observation, flow, and line-task coverage now
  live in separate files, while the root `tests.rs` only wires modules and
  shared helpers.
- Continue migrating typed AST/HIR/checking APIs into semantic views or lowering
  outputs over the CST instead of extending the current line parser.
- Keep `.awfb`, schemas, manifests, bytecode, and save/debug snapshots as pure
  data models and codecs over bytes/strings. Filesystem, network, path watching,
  embedding, signing, upload, and platform storage live in CLI/build/player
  adapters.
- Use `thiserror` for Rust error types across the workspace while preserving
  structured fields such as `kind`, `range`, `anchor`, and `message`.
- Keep `arcweft-core` free of Cranelift, Wasmtime, filesystem, network, GPU,
  audio, device, and OS dependencies.
- Keep AST, HIR, runtime-plan, schemas, and manifests as owned data models.
  Rust lifetime parameters should stay at adapter/view boundaries unless a local
  crate-internal API clearly benefits; Arcweft lifetime and ownership rules are
  semantic facts checked by `arcweft-lang-sema`, not Rust borrows threaded
  through every intermediate representation.

The stable specification locations for the `pro_review4.md` decisions are:

- `docs/00-overview/decisions.md`: canonicalization and high-level language decisions.
- `docs/00-overview/naming.md`: relative ID naming rules.
- `docs/01-language/block-scopes.md`: value-producing blocks and named/unnamed `scope` blocks.
- `docs/01-language/ids-and-references.md`: `@.suffix`, parent-dot, and `@super...` relative IDs plus module-path roots.
- `docs/01-language/grammar.md`: grammar summary for `scope`, relative IDs, module paths, and await grouping.
- `docs/01-language/scenario-surface-syntax.md`: dialogue, choice, and scenario-facing sugar examples.
- `docs/01-language/modules.md`: `self::`, `super::`, `crate::`, and `parent::` normalization.
- `docs/04-tooling/cli.md`: explicit sugar expansion and ID materialization commands.
- `docs/04-tooling/lsp.md`: sugar expansion and ID materialization code actions.
- `docs/02-runtime/core.md`: VM, effect requests, and data-format Sans I/O boundary.
- `docs/02-runtime/cranelift-jit.md`: native-only pure-function JIT boundary.
- `docs/02-runtime/plugins.md`: WIT/Wasm plugin sandbox boundary.
- `docs/05-build-and-security/native-web-build.md`: native/web runtime target model.
- `docs/05-build-and-security/packaging.md`: Sans I/O bundle format boundary.
- `docs/schemas/README.md`: schemas as data formats rather than I/O APIs.


