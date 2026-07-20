# Structural violations

## warning SIZE001 — `crates/arcweft-adapter-context/src/manifest.rs`

1304 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-adapter-context/src/manifest.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

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

2264 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2264 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-bundle/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1620 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/release.rs`

2190 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2021 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/codec.rs`

1501 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

1811 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

1625 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_rag.rs`

1378 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/observe.rs`

1383 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs`

1201 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1484 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3336 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag.rs`

1353 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag/source_index.rs`

1834 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/script.rs`

2051 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/bundle.rs`

2042 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/bundle/tests.rs`

2652 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2376 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1511 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2282 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1350 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/output.rs`

1383 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/toolchain_profile.rs`

2463 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/toolchain_profile.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`

5977 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6717 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6109 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4218 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

5257 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

7062 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1388 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-compiler/src/tests.rs`

3838 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

1937 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

2071 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

1945 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

1841 physical LOC exceeds the 1200 LOC review threshold

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

2465 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1580 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-hir/src/symbol/table.rs`

1247 physical LOC exceeds the 1200 LOC review threshold

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

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/identity.rs`

1588 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/resolver.rs`

1871 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

1261 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker.rs`

1930 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr.rs`

2337 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/agent.rs`

1296 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/registered_call.rs`

2120 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/module.rs`

2483 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/env/base.rs`

1232 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/project_index.rs`

1206 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/semantic.rs`

2121 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/tests/typecheck.rs`

4285 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/traits.rs`

2054 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/items.rs`

1764 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/view.rs`

1808 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr.rs`

1633 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/items.rs`

1828 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/style.rs`

1410 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/view.rs`

1824 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lsp/src/features/actions.rs`

1472 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lsp/src/features/actions.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lsp/src/session/tests.rs`

2732 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

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

1347 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/persistent_query.rs`

1828 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/release.rs`

1556 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project/src/persistent_object/codec.rs`

1550 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project/src/persistent_object/codec.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-render-wgpu/src/geometry.rs`

2154 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/renderer.rs`

1657 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-render-wgpu/src/view_compositor.rs`

1563 physical LOC exceeds the 1200 LOC review threshold

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

1550 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1259 physical LOC exceeds the 1200 LOC review threshold

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

1240 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime.rs`

1343 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs`

1623 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/tests/session.rs`

3073 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/tests/view_runtime.rs`

2631 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning ARCH002 — `crates/arcweft-runtime-plan/Cargo.toml`

runtime plan owns display lowering but depends on renderer-named contract owner

**Fix:** depend on arcweft-text-model; keep resolver/parsers in arcweft-render-text

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

2194 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

1398 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/expr.rs`

2382 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/flow.rs`

2067 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1909 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1909 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-verify/src/lib.rs`

1959 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1959 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `tools/verify-text-raster-parity.rs`

1795 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code
