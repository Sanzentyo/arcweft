# Structural violations

## warning SIZE001 — `crates/arcweft-adapter-sema/src/registration/input.rs`

1249 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

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

2335 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2335 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-bundle/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1620 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-bundle/src/product.rs`

1206 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-bundle/src/product.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

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

1203 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1477 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3332 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag.rs`

1353 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/rag/source_index.rs`

1841 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/agent/script.rs`

2057 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/bundle.rs`

1396 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/bundle/tests.rs`

2946 physical LOC exceeds the 2500 LOC review threshold

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

## warning SIZE001 — `crates/arcweft-cli/src/app/project.rs`

1247 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-cli/src/app/project.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2276 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1337 physical LOC exceeds the 1200 LOC review threshold

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

5901 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6712 physical LOC exceeds the 2500 LOC review threshold

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

6864 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1501 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-compiler/src/tests.rs`

3973 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-compiler/src/view/lowering.rs`

1315 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

1998 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

2071 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

1952 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

1841 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

1606 physical LOC exceeds the 1200 LOC review threshold

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

1300 physical LOC exceeds the 1200 LOC review threshold

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

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/facts.rs`

1204 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/identity.rs`

1652 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/resolver.rs`

2450 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/resolver_tests.rs`

3563 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

1326 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/callable/tests.rs`

2538 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker.rs`

2330 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr.rs`

2325 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/agent.rs`

1298 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/expr/registered_call.rs`

1689 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/checker/module.rs`

1962 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/env/base.rs`

1344 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/project_index.rs`

1277 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/registration/registrar.rs`

1245 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/semantic.rs`

2102 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/signature/tests.rs`

3204 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/tests/typecheck.rs`

4591 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-sema/src/traits.rs`

1428 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/items.rs`

1651 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/ast/view.rs`

1799 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/attachment.rs`

2255 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/attachment.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/attachment/callable.rs`

1507 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression.rs`

1661 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr.rs`

1739 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr/call_syntax.rs`

1264 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expr/call_syntax_tests.rs`

2779 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/expressions/dialogue.rs`

1205 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

1293 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

1401 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/incremental/database_tests.rs`

2959 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

1485 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/declaration.rs`

1485 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/expression.rs`

2293 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/items.rs`

2021 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/pattern.rs`

1278 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/rich_text_grammar.rs`

1712 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/statement.rs`

1886 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/style.rs`

1420 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/parser/view.rs`

2037 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/text/rich_text_tag.rs`

1210 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lang-syntax/src/types/token/grammar.rs`

1612 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-lsp/src/diagnostics.rs`

1270 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lsp/src/diagnostics.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

1518 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

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

1347 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/persistent_query.rs`

1828 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-project-loader/src/cache/release.rs`

1579 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-project-loader/src/topology/loader.rs`

1342 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-project/src/persistent_object/codec.rs`

1564 physical LOC exceeds the 1200 LOC review threshold

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

1642 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/tests/session.rs`

3203 physical LOC exceeds the 2500 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-driver/tests/view_runtime.rs`

2659 physical LOC exceeds the 2500 LOC review threshold

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

2385 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-runtime-plan/src/flow.rs`

2083 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1810 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1810 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large production file contains an embedded #[cfg(test)] module

**Fix:** move tests to domain-specific child test modules or integration tests

## warning SIZE001 — `crates/arcweft-verify/src/lib.rs`

1905 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code

## warning SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1905 physical LOC; target is below 250 LOC

**Fix:** move implementations to named modules and keep intentional re-exports

## warning SIZE001 — `tools/verify-text-raster-parity.rs`

1795 physical LOC exceeds the 1200 LOC review threshold

**Fix:** review responsibility boundaries before adding more code
