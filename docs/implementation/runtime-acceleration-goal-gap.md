# Runtime Acceleration Goal Gap

Recorded: 2026-06-05 JST

This document records the paused runtime-acceleration goal, the current
implementation state, and the remaining gap. It is an implementation-state
handoff, not a stable language or runtime specification.

## Paused Goal

The active paused goal is:

```text
Arcweft runtime の残 TODO が実装計画上ほぼ残らない水準まで、整数・浮動小数の幅保持 fast path を VM/AOT/JIT/flow batch に拡張し、matrix/tensor accelerator は persistent GPU buffer と wgpu/glam/ndarray backend 選択・計測まで詰め、十分な性能検証・docs 更新・絶対パス混入防止・fmt/clippy/test/bench 検証まで継続する。
```

The goal is intentionally broader than the current implementation. It should
not be treated as complete.

## Current Repository Point

Recent relevant commits:

| Commit | Summary |
| --- | --- |
| `a2de7b95bedf` | Expose explicit browser WebGPU math policy |
| `f89ec6d28dda` | Record browser WebGPU smoke evidence |
| `3a000a41a731` | Clarify Cranelift integer lowering binding |
| `bd45b4136e3a` | Profile mixed-width release trends |

The working copy was clean when this document was written.

## What Is Already Usable

The current runtime acceleration work is usable for explicit backend selection,
policy-controlled math dispatch, and benchmark-driven inspection.

Native CLI/backend policy:

- `--math-backend scalar|glam|ndarray|wgpu` can pin the backend for a run.
- `--math-backend auto --math-wgpu-min-elements N` can express a native Auto
  threshold policy.
- The current docs direct host-specific threshold tuning into checked-in
  profile/bench harnesses rather than hard-coding local measurements.

Browser WebGPU policy:

- `BrowserWebGpuMathAutoPolicy::conservative()` is the default policy.
- `BrowserWebGpuMathAutoPolicy::cpu_only()` explicitly keeps browser math on CPU
  Wasm.
- `BrowserWebGpuMathAutoPolicy::explicit_webgpu_resident()` explicitly selects
  resident WebGPU when shape and storage limits allow it.
- `BrowserWebGpuMathAutoPolicy::harness_capacity_matmul(...)` is reserved for
  benchmark probes of overprovisioned resident matmul capacity.
- The default policy was not retuned from one local browser or GPU.

Browser WebGPU harness:

- Browser WebGPU smoke/check commands exist and can verify whether WebGPU is
  available and whether the benchmark output preserves the expected fields.
- Current recorded smoke evidence showed WebGPU available in the measured
  environment, but small cases were not used to change default product policy.

Dense runtime values and integer fast paths:

- The runtime value model has moved toward sequence storage specialization
  rather than placing every dense type directly under the outer runtime value.
- Existing integer lowering documentation clarifies that Cranelift literal bits
  do not imply an i64-only runtime ABI.
- Mixed-width release profile commands exist for VM/AOT/JIT comparison.

Adapter/runtime separation:

- The work has been moving adapter-heavy capabilities away from core runtime
  layers.
- Current policy is to keep core Sans I/O and expose host capabilities through
  explicit adapter/profile metadata.

## Verified Gates At This Point

Recent checks run successfully during the last implementation slice:

```text
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo test -p arcweft-runtime-accelerator browser_webgpu --lib
just browser-webgpu-bench-check
just scan-absolute-paths
just scan-removed-dsl
```

The absolute-path scan is part of the acceptance surface because benchmark and
profile output must not record machine-local paths.

## Safe To Run Now

It is reasonable to run the current tree for:

- VM execution paths.
- Explicit scalar, glam, ndarray, or wgpu math backend experiments.
- Browser WebGPU policy and smoke-harness checks.
- Path-free performance snapshot generation.
- Continued parser/runtime/tooling development that does not depend on the
  unfinished acceleration items below.

The current tree should not be presented as having completed the full paused
goal. It is a usable intermediate state with explicit policy controls and
regression gates.

## Remaining Gap

### Integer And Floating-Point Width Fast Paths

Remaining work:

- Confirm that each supported integer width keeps its width through the full
  runtime, dense sequence, VM, batch, AOT, and JIT path.
- Avoid fallback paths that materialize dense typed values into generic
  `RuntimeValue` elements before hot loops.
- Extend the same audit to floating-point widths where deterministic semantics
  are acceptable.
- Keep any exact-width dispatch typed. Avoid operation names or type names as
  unstructured strings at hot dispatch boundaries.

Acceptance evidence:

- Width-preserving tests for signed and unsigned integer families.
- Floating-point tests for each supported deterministic path.
- Bench comparisons that show no accidental widening or materialization cost in
  the intended fast paths.

### Cranelift JIT And AOT Codegen Boundary

Remaining work:

- Refactor Cranelift lowering so the main codegen path defines functions
  against `M: Module`.
- Keep JIT finalization and native function pointer extraction in a JIT-specific
  wrapper.
- Add object/AOT emission only after the shared lowering path is stable.
- Keep runtime/core crates free of direct Cranelift details.

Acceptance evidence:

- JIT behavior remains unchanged after the `M: Module` refactor.
- Object emission can be tested without changing VM semantics.
- Unsupported expressions still fall back through structured policy rather than
  partial or stringly typed lowering.

### Matrix And Tensor Acceleration

Remaining work:

- Keep tensor/matrix acceleration behind adapter/backend boundaries.
- Preserve backend choices for scalar, glam, ndarray, and wgpu.
- Add or complete persistent GPU buffer paths beyond the current browser policy
  surface.
- Extend graph/operator coverage beyond matmul where it is justified by the
  runtime model and benchmark evidence.

Acceptance evidence:

- Bench cases compare scalar baseline, glam, ndarray, and wgpu where supported.
- Bench output stays path-free.
- Backend selection remains explicit or policy-controlled.
- Browser and native capabilities remain separated.

### Browser WebGPU Policy

Remaining work:

- Use the new typed policy constructors from embedding code where appropriate.
- Keep per-environment tuning inside benchmark/profile harnesses.
- Avoid changing the default conservative policy until path-free evidence across
  representative environments supports it.

Acceptance evidence:

- Browser smoke results can be regenerated.
- Product policy changes are documented as policy decisions, not hidden local
  optimizations.

### Parallel Scheduling And Toolchain Performance

Remaining work:

- Continue measuring compile, check, clippy, and borrow-check-sensitive code
  paths through stable harness commands.
- Improve scheduling/thread-pool interaction where broad measurements show it is
  useful.
- Avoid one-off tuning for the current machine unless the only deliverable is a
  harness for discovering such tuning.

Acceptance evidence:

- Toolchain-profile commands remain cross-platform.
- Results do not contain absolute paths.
- Performance changes include before/after evidence.

### Documentation State

Remaining work:

- Keep stable design docs focused on intended architecture.
- Keep transient implementation status in `docs/implementation/`.
- Update performance snapshots only with reproducible, path-free evidence.
- Continue documenting deviations from the paused goal before switching focus.

## Recommended Next Move If Changing Direction

It is fine to pause the acceleration goal and move to another direction now.
Before doing so, preserve this boundary:

- Treat the current runtime acceleration work as an intermediate, runnable,
  policy-controlled state.
- Do not claim Julia-class performance or complete AOT/JIT coverage yet.
- Prefer using the existing harnesses before changing default thresholds.
- If the next direction depends on runtime math behavior, pin the backend or
  policy explicitly rather than relying on Auto.

