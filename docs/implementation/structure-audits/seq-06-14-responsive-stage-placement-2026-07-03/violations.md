# Structural violations

## error SIZE001 — `crates/arcweft-core/src/value.rs`

2512 physical LOC exceeds the 2500 LOC error threshold

**Fix:** split by cohesive domain and keep a small facade

## error SIZE001 — `crates/arcweft-lang-sema/src/checker/expr.rs`

2560 physical LOC exceeds the 2500 LOC error threshold

**Fix:** split by cohesive domain and keep a small facade

## error SIZE001 — `crates/arcweft-runtime-plan/src/flow.rs`

2593 physical LOC exceeds the 2500 LOC error threshold

**Fix:** split by cohesive domain and keep a small facade

## warning SIZE001 — `crates/arcweft-adt/src/lib.rs`

1486 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code
## warning SIZE002 — `crates/arcweft-adt/src/lib.rs`

facade file has 1486 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-adt/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning ARCH003 — `crates/arcweft-agent-protocol/Cargo.toml`

transport-neutral Agent protocol depends on arcweft-render-text

**Fix:** move MCP/base64 resource mapping to arcweft-agent-mcp and model types to arcweft-text-model

## warning ARCH003 — `crates/arcweft-agent-protocol/Cargo.toml`

transport-neutral Agent protocol depends on base64

**Fix:** move MCP/base64 resource mapping to arcweft-agent-mcp and model types to arcweft-text-model

## warning TYPE001 — `crates/arcweft-agent-protocol/src/artifact.rs:29`

stringly boundary field: pub kind: String,

**Fix:** replace kind/payload strings with a tagged enum and typed payload

## warning TYPE001 — `crates/arcweft-agent-protocol/src/protocol.rs:43`

stringly boundary field: pub kind: String,

**Fix:** replace kind/payload strings with a tagged enum and typed payload

## warning TYPE001 — `crates/arcweft-agent-protocol/src/protocol.rs:133`

stringly boundary field: pub action: String,

**Fix:** replace kind/payload strings with a tagged enum and typed payload

## warning SIZE001 — `crates/arcweft-bundle/src/container.rs`

2383 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/container.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/lib.rs`

1929 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 1929 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-bundle/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1536 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/release.rs`

2225 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2017 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/capture.rs`

2146 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

1520 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_rag.rs`

1378 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/observe.rs`

1325 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1465 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3527 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag.rs`

1353 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag/source_index.rs`

1822 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/script.rs`

1884 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/bundle.rs`

2115 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/bundle.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2376 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1542 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2282 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1272 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/output.rs`

1382 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/toolchain_profile.rs`

2463 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/toolchain_profile.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`

5651 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6758 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6161 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4181 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

5250 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

7944 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1390 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-core/src/awbc/product_step.rs`

2434 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

1840 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

1695 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

1256 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

1234 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/engine/eval.rs`

1531 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/engine/eval/calls.rs`

2481 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/pure.rs`

1935 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TYPE001 — `crates/arcweft-core/src/step.rs:127`

stringly boundary field: pub kind: String,

**Fix:** replace kind/payload strings with a tagged enum and typed payload

## warning SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1553 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-glyphon/src/lib.rs`

1240 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-glyphon/src/lib.rs`

facade file has 1240 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-glyphon/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-jit-cranelift/src/batch.rs`

1563 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

2153 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

facade file has 2153 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lower.rs`

1438 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/agent.rs`

1285 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/module.rs`

1318 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/semantic.rs`

2017 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/traits.rs`

1907 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/items.rs`

1849 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr.rs`

1964 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/expr.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/items.rs`

1309 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lsp/src/features/actions.rs`

1641 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lsp/src/features/actions.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-player-native/src/scene_windowed.rs`

1337 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-player-native/src/scene_windowed.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-presentation/src/text_editor.rs`

1306 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-presentation/src/text_editor.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-presentation/src/text_input.rs`

1542 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-presentation/src/text_input.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

1348 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/persistent_query.rs`

1828 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/release.rs`

1541 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project/src/persistent_object/codec.rs`

1559 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project/src/persistent_object/codec.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-render-native/src/effects.rs`

1272 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-native/src/lib.rs`

1500 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-render-native/src/lib.rs`

facade file has 1500 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `crates/arcweft-render-native/src/renderer.rs`

2247 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-native/src/tests.rs`

4415 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-native/src/window_page.rs`

1531 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/geometry.rs`

1207 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/renderer.rs`

1254 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-render-wgpu/src/renderer.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-runtime-accelerator/examples/math_bench.rs`

2129 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-runtime-accelerator/examples/math_bench.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/call_backend.rs`

1930 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/compile.rs`

1543 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1258 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

2010 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/lib.rs`

1285 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-runtime-accelerator/src/lib.rs`

facade file has 1285 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/math.rs`

2340 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/math/browser_webgpu/context.rs`

1862 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/math/wgpu_backend.rs`

1398 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/src/session.rs`

1290 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning ARCH002 — `crates/arcweft-runtime-plan/Cargo.toml`

runtime plan owns display lowering but depends on renderer-named contract owner

**Fix:** depend on arcweft-text-model; keep resolver/parsers in arcweft-render-text

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

1519 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

1241 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/expr.rs`

1937 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-runtime-plan/src/expr.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning TEST001 — `crates/arcweft-runtime-plan/src/flow.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-text-layout/src/lib.rs`

2458 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-text-layout/src/lib.rs`

facade file has 2458 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE002 — `crates/arcweft-view/src/lib.rs`

facade file has 1027 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `crates/arcweft-view/src/text_field.rs`

1433 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-view/src/text_field.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1586 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1586 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-verify/src/lib.rs`

1923 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1923 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `tools/apply-seq04-6-typecheck-gate.rs`

1380 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `tools/verify-text-raster-parity.rs`

1627 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code
