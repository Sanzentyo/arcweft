# Structure Refactor Audit - 2026-06-21

Source package: `D:/sanze/Downloads/arcweft-structure-refactor-2026-06-21.zip`

Current checkout measured at Jujutsu change
`knylnsvnuklltylxqzxyzqsumvlyqzrk`
(`f81da81a8d37ff39045465140855b46ba9dffa8e`).

## Implemented in this cut

- Added the structural audit and decomposition gate to `AGENTS.md`.
- Added the checked-in structural audit tool as a Rust script:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

- Recorded current checkout reports under
  `docs/implementation/structure-audit-2026-06-21/`.
- Added PR1 guard tests from the structure-refactor package:
  - `RuntimePureCallStats::saturating_delta` counter completeness.
  - MCP tool descriptor uniqueness and required-schema-property invariants.
- Added `arcweft-interaction-model` as the typed interaction contract crate.
- Exposed new typed contracts through explicit public modules rather than
  compatibility-style root `pub use` facades.
- Replaced `arcweft-core::step` stringly `InputEvent` and `AudioEvent`
  boundary structs with `RoutedInputEvent` and `AudioEvent` from
  `arcweft-interaction-model`.
- Updated native Agent semantic action dispatch to produce typed routed input
  events.
- Split these production roots into responsibility modules without broad root
  re-export facades:
  - `arcweft-agent-mcp`
  - `arcweft-agent-protocol`
  - `arcweft-agent-runner`
  - `arcweft-browser-bench`
  - `arcweft-cli/src/app/runtime.rs`
  - `arcweft-debug-sqlite/src/store.rs`
  - `arcweft-lang-syntax/src/cst.rs`
  - `arcweft-runtime-plan/src/render_text.rs`
  - `arcweft-tooling/src/lib.rs`
- Split `arcweft-compiler/src/lib.rs` into `agent`, `agent_project`,
  `effect_manifest`, `error`, `hir`, `lower`, `parse`, `source`, and `types`
  modules, then removed the crate-root re-export surface. Downstream callers now
  import from explicit module paths such as `arcweft_compiler::lower` and
  `arcweft_compiler::source`.
- Split `arcweft-lang-sema/src/project_index.rs` into child modules for
  Agent prelude construction, entity projection, flow-control summaries,
  relation indexing, and tests. The root module now remains a typed index API
  and HIR projection coordinator.
- Split `arcweft-core/src/engine/eval.rs` by moving call, pure-call, map, and
  sum expression evaluation into `engine/eval/calls.rs`; the root eval module
  now keeps control-flow evaluation, data expression evaluation, call-shape
  checks, and shared runtime-call helpers.
- Split `arcweft-core/src/pure.rs` into private child modules for AOT pure
  helper compilation/execution and VM runtime pure-call backend dispatch. The
  public plan and backend types remain defined at their structural home in
  `pure.rs`, while implementation blocks live in `pure/aot.rs` and
  `pure/runtime_backend.rs`.
- Removed the redundant runtime binary-op error wrapper from
  `arcweft-core/src/value.rs`; runtime unary/binary operator labels now live on
  the enum impls instead of free label functions.
- Split `arcweft-core/src/value.rs` into private child modules for sequence /
  dense-sequence implementation blocks and runtime environment binding
  management. Public value, sequence, and environment types remain defined in
  `value.rs` without compatibility re-export shims.
- Split `arcweft-lang-sema/src/checker/expr.rs` by moving Agent intrinsic
  expression checking into `checker/expr/agent.rs`. The parent module keeps
  expression dispatch and general expression checking; Agent-specific entry
  points are visible only inside the checker expression module boundary.
- Split `arcweft-lang-jit-cranelift/src/lib.rs` into private child modules for
  compiled wrapper impls, batch helper construction, lowering helpers, and
  embedded tests. Public JIT request/result/backend types remain defined in the
  crate root, while implementation responsibility now lives under
  `compiled.rs`, `batch.rs`, `lower.rs`, and `tests.rs`.
- Split `arcweft-text-layout/src/lib.rs` by moving ruby placement/collision
  logic into `ruby.rs` and moving unit tests into `tests.rs` plus
  `tests/ruby.rs`, `tests/vertical_sequences.rs`, and
  `tests/vertical_class_mix.rs`. Public layout result/configuration types
  remain defined in the crate root.
- Split `arcweft-runtime-accelerator/src/lib.rs` into explicit modules for the
  public accelerator API, runtime call backend implementations, compile/batch
  helper mechanics, external/data adapter calls, and unit tests. Public
  accelerator configuration/statistics/cache summary types remain defined in
  the crate root.
- Split `arcweft-runtime-accelerator/src/math.rs` by moving native WGPU,
  Browser WebGPU policy, Browser WebGPU adapter implementation, and tests into
  `math/` child modules. Browser adapter public types remain defined in
  `math/browser_webgpu.rs`, with implementation blocks in
  `math/browser_webgpu/`.
- Split `arcweft-render-native/src/lib.rs` into explicit modules for reusable
  offscreen capture sessions, effect execution, effect/shader/motion helpers,
  visual layout measurement, window page/style state, renderer/readback helpers,
  and unit tests. Public native renderer contract types remain defined in the
  crate root; private implementation boundaries use named child modules rather
  than compatibility re-export shims.
- Split the legacy `arcweft-cli/tests/check.rs` integration-test file into
  topic-owned include files under `tests/check/`: Agent script/debug coverage,
  toolchain/JIT coverage, CLI runtime/bench coverage, and native observe/MCP
  capture chunks. The remaining `check.rs` file now owns shared integration-test
  helpers and inclusion order rather than the full legacy test body.
- Split the remaining CLI Agent production hotspots:
  - `arcweft-cli/src/app/agent.rs` now keeps option definitions and command
    dispatch while RAG orchestration, source/project graph indexing, and Agent
    Script execution live in named child modules.
  - `arcweft-cli/src/app/agent/native.rs` now keeps native command dispatch and
    shared imports while REPL, MCP protocol/debug/RAG/resources, observation,
    capture, runtime observation, image mapping, and tests live in named child
    modules.
- Updated the structural audit tool to classify `*/tests.rs` as unit-test
  source and generated lookup tables as `generated`, with generated Rust source
  excluded from ordinary production size violations.
- Moved embedded tests out of `arcweft-lsp/src/session.rs`.
- Added low-risk CLI integration-test support modules plus `check_core_cli.rs`.
- Used Cursor Agent CLI with `--model composer-2.5-fast --sandbox disabled` for
  seven mechanical follow-up slices:
  - `tooling-lib-split-2`
  - `lsp-session-test-split-2`
  - `compiler-lib-split-2`
  - `debug-sqlite-store-split-2`
  - `cli-runtime-split-2`
  - `syntax-cst-split-2`
  - `runtime-plan-render-text-split-2`

## Current Audit Result

Command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audit-2026-06-21
```

Summary:

- files scanned: 1138
- Rust files: 611
- Rust physical LOC: 318758
- package manifests: 70
- violations: 0 errors, 82 warnings

This cut reduced error-level size findings from 21 to 0. The removed
error-level findings were:

- `crates/arcweft-agent-protocol/src/lib.rs`
- `crates/arcweft-agent-runner/src/lib.rs`
- `crates/arcweft-browser-bench/src/lib.rs`
- `crates/arcweft-cli/src/app/runtime.rs`
- `crates/arcweft-compiler/src/lib.rs`
- `crates/arcweft-debug-sqlite/src/store.rs`
- `crates/arcweft-core/src/engine/eval.rs`
- `crates/arcweft-core/src/pure.rs`
- `crates/arcweft-core/src/value.rs`
- `crates/arcweft-lang-jit-cranelift/src/lib.rs`
- `crates/arcweft-lang-sema/src/checker/expr.rs`
- `crates/arcweft-lang-sema/src/project_index.rs`
- `crates/arcweft-runtime-accelerator/src/lib.rs`
- `crates/arcweft-runtime-accelerator/src/math.rs`
- `crates/arcweft-render-native/src/lib.rs`
- `crates/arcweft-runtime-plan/src/render_text.rs`
- `crates/arcweft-text-layout/src/lib.rs`
- `crates/arcweft-cli/tests/check.rs`
- `crates/arcweft-cli/src/app/agent.rs`
- `crates/arcweft-cli/src/app/agent/native.rs`
- `crates/arcweft-text-layout/src/vertical_orientation.rs` is now classified
  as generated Unicode lookup-table source rather than an ordinary production
  hotspot.

## Largest Measured Rust Files

| Path | Bytes | LOC | Kind | Embedded tests |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12399 | generated lookup table | false |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 253604 | 7651 | integration test warning | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 231411 | 6282 | integration test warning | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 228636 | 6161 | integration test warning | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 215053 | 5633 | integration test warning | false |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 201078 | 5250 | integration test warning | false |
| `crates/arcweft-render-native/src/tests.rs` | 153646 | 4395 | unit test | false |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 147541 | 4181 | integration test warning | false |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 116363 | 3342 | unit test warning | false |
| `crates/arcweft-runtime-plan/src/flow.rs` | 89343 | 2472 | production warning | true |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75712 | 2463 | production warning | true |
| `crates/arcweft-text-layout/src/lib.rs` | 81491 | 2458 | production warning | false |
| `crates/arcweft-core/src/value.rs` | 81600 | 2434 | production warning | false |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 93708 | 2427 | production warning | false |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 86959 | 2417 | production warning | false |
| `crates/arcweft-cli/src/app/debug.rs` | 77792 | 2376 | production warning | true |
| `crates/arcweft-runtime-accelerator/src/math.rs` | 87460 | 2314 | production warning | false |
| `crates/arcweft-render-native/src/renderer.rs` | 76620 | 2247 | production warning | false |
| `crates/arcweft-cli/src/app/agent/native/repl.rs` | 74148 | 2247 | production warning | false |
| `crates/arcweft-cli/src/app/agent/native/image_mapping.rs` | 76275 | 2197 | production warning | false |
| `crates/arcweft-core/src/tests/flow.rs` | 77146 | 2170 | unit test | false |
| `crates/arcweft-runtime-accelerator/examples/math_bench.rs` | 76869 | 2155 | example warning | true |
| `crates/arcweft-lang-jit-cranelift/src/lib.rs` | 75625 | 2140 | production warning | false |
| `crates/arcweft-agent-runner/src/tests.rs` | 75992 | 2103 | unit test | false |
| `crates/arcweft-lsp/src/session/tests.rs` | 71453 | 2088 | unit test | false |
| `crates/arcweft-runtime-accelerator/src/inference.rs` | 66806 | 2010 | production warning | true |
| `crates/arcweft-runtime-accelerator/src/call_backend.rs` | 76102 | 1930 | production warning | false |
| `crates/arcweft-lang-sema/src/semantic.rs` | 74925 | 1993 | production warning | false |
| `crates/arcweft-runtime-plan/src/expr.rs` | 65110 | 1873 | production warning | true |
| `crates/arcweft-cli/src/app/agent/script.rs` | 65746 | 1865 | production warning | false |
| `crates/arcweft-runtime-accelerator/src/math/browser_webgpu/context.rs` | 72237 | 1862 | production warning | false |
| `crates/arcweft-lang-jit-cranelift/src/tests.rs` | 67689 | 1859 | unit test | false |
| `crates/arcweft-core/src/pure.rs` | 66409 | 1837 | production warning | false |
| `crates/arcweft-compiler/src/tests.rs` | 52586 | 1833 | unit test | false |
| `crates/arcweft-lang-syntax/src/expr.rs` | 56215 | 1795 | production warning | false |
| `crates/arcweft-text-layout/src/tests/vertical_class_mix.rs` | 74126 | 1749 | unit test | false |
| `crates/arcweft-lang-jit-cranelift/src/batch.rs` | 55620 | 1554 | production warning | false |
| `crates/arcweft-core/src/value/sequence_impls.rs` | 51830 | 1553 | production warning | false |
| `crates/arcweft-runtime-accelerator/src/compile.rs` | 51536 | 1529 | production warning | false |
| `crates/arcweft-lang-jit-cranelift/src/lower.rs` | 57691 | 1431 | production warning | false |
| `crates/arcweft-runtime-accelerator/src/math/wgpu_backend.rs` | 48359 | 1398 | production warning | false |
| `crates/arcweft-lang-sema/src/checker/expr/agent.rs` | 48917 | 1285 | production warning | false |
| `crates/arcweft-runtime-accelerator/src/lib.rs` | 41481 | 1274 | production warning | false |
| `crates/arcweft-render-native/src/window_page.rs` | 52906 | 1531 | production warning | false |
| `crates/arcweft-render-native/src/lib.rs` | 52073 | 1514 | production warning | false |
| `crates/arcweft-render-native/src/effects.rs` | 44963 | 1272 | production warning | false |
| `crates/arcweft-core/src/pure/runtime_backend.rs` | 37163 | 1057 | production | false |
| `crates/arcweft-runtime-accelerator/src/external.rs` | 42343 | 1054 | production | false |
| `crates/arcweft-core/src/pure/aot.rs` | 32688 | 940 | production | false |
| `crates/arcweft-cli/tests/check.rs` | 27104 | 786 | integration test coordinator | false |
| `crates/arcweft-runtime-accelerator/src/accelerator_api.rs` | 30639 | 766 | production | false |
| `crates/arcweft-lang-jit-cranelift/src/compiled.rs` | 25992 | 741 | production | false |
| `crates/arcweft-core/src/value/env.rs` | 12551 | 412 | production | false |

## Remaining Error-Level Hotspots

The structural audit reports 0 error-level findings in this checkout. Remaining
items are warnings and dependency/type-shape review notes recorded in
`docs/implementation/structure-audit-2026-06-21/violations.md`.

## Design Deviations

- The structural audit tool from the package was converted from a standalone
  Cargo package into a Rust script with Cargo frontmatter, per repository tool
  policy.
- Generated lookup-table Rust source is now reported with `classification =
  generated` in `file_metrics.csv` and is excluded from ordinary handwritten
  production size violations. This applies to the Unicode vertical-orientation
  and JLREQ punctuation-data tables, which carry generated-source headers in
  the files themselves.
- `arcweft-interaction-model` was adapted from the overlay to this workspace's
  package metadata style (`version = "0.1.0"`, `publish = false`).
- The CLI Agent split has no local `wildcard_imports` allowance. Some internal
  `use super::*` imports remain in the divided Agent subtree and should be
  narrowed in a follow-up cut with module-local imports rather than parent
  facade imports.
