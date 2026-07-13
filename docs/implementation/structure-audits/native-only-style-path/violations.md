# Structural violations

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

## warning SIZE001 — `crates/arcweft-bundle/src/container.rs`

2393 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/container.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/lib.rs`

2174 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2174 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-bundle/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1609 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/release.rs`

2225 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2021 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/codec.rs`

2360 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

2396 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

1589 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_rag.rs`

1378 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/observe.rs`

1371 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1474 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3181 physical LOC exceeds the 2500 LOC review threshold

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

1975 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2376 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1543 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2296 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1331 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/output.rs`

1386 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/toolchain_profile.rs`

2463 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/toolchain_profile.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`

5850 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6620 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6109 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4177 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

5249 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

7935 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1419 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-compiler/src/tests.rs`

5350 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

1925 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/product_step.rs`

2430 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

1969 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

1945 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

1547 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

1562 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/engine/eval.rs`

1516 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/engine/eval/calls.rs`

2481 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/pure.rs`

2097 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/tests/flow.rs`

2553 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/value.rs`

2500 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1580 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

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

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker.rs`

2460 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr.rs`

2469 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/agent.rs`

1296 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/module.rs`

2116 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/diagnostics.rs`

1211 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/env.rs`

1365 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-sema/src/env.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-sema/src/semantic.rs`

2046 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/tests/typecheck.rs`

4120 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/traits.rs`

1923 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/items.rs`

1876 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/view.rs`

1794 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr.rs`

2243 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/expr.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr/source_ranges.rs`

1488 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/expr/source_ranges.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/control_flow.rs`

1224 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/items.rs`

1513 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/view.rs`

1661 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lsp/src/features/actions.rs`

1491 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lsp/src/features/actions.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-player-native/src/scene_windowed.rs`

1735 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-player-native/src/scene_windowed.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-presentation/src/text_editor.rs`

1965 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-presentation/src/text_editor.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-presentation/src/text_input.rs`

1695 physical LOC exceeds the 1200 LOC review threshold

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

## warning SIZE001 — `crates/arcweft-render-wgpu/src/geometry.rs`

2112 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/renderer.rs`

1516 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/view_compositor.rs`

1560 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

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

1549 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1255 physical LOC exceeds the 1200 LOC review threshold

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

## warning SIZE001 — `crates/arcweft-runtime-driver/src/display.rs`

1450 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-runtime-driver/src/display.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-runtime-driver/src/session.rs`

2431 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs`

1469 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/tests/session.rs`

2504 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning ARCH002 — `crates/arcweft-runtime-plan/Cargo.toml`

runtime plan owns display lowering but depends on renderer-named contract owner

**Fix:** depend on arcweft-text-model; keep resolver/parsers in arcweft-render-text

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

2182 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

1337 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/expr.rs`

2363 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/flow.rs`

1903 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1590 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1590 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-verify/src/lib.rs`

1927 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1927 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `tools/verify-text-raster-parity.rs`

1795 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code
