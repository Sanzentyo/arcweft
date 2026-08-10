# Structural findings

## review-trigger SIZE001 — `crates/arcweft-adapter-sema/src/registration/input.rs`

1249 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-adt/src/lib.rs`

1486 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-adt/src/lib.rs`

facade file has 1486 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-adt/src/lib.rs`

large maintained owner contains 121 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/container.rs`

2398 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/container.rs`

large maintained owner contains 662 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/lib.rs`

2422 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2422 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-bundle/src/lib.rs`

large maintained owner contains 822 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1785 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/patch.rs`

large maintained owner contains 145 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/product.rs`

1476 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/product.rs`

large maintained owner contains 600 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/release.rs`

2190 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/release.rs`

large maintained owner contains 994 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2040 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/codec.rs`

1501 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

1812 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

large maintained owner contains 42 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

1625 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

large maintained owner contains 65 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_rag.rs`

1378 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/observe.rs`

1387 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs`

1206 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1572 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3324 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/rag.rs`

1352 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/rag/source_index.rs`

1500 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/script.rs`

2266 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/agent/script.rs`

large maintained owner contains 77 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/bundle.rs`

1407 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/bundle/tests.rs`

2738 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2376 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large maintained owner contains 70 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1504 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/project.rs`

1462 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/project.rs`

large maintained owner contains 55 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2493 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1395 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large maintained owner contains 300 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/output.rs`

1298 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/toolchain_profile.rs`

2463 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/toolchain_profile.rs`

large maintained owner contains 296 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`

5901 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6712 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6109 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4218 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

5257 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

6865 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-compiler/src/lower.rs`

2282 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1946 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large maintained owner contains 525 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-compiler/src/project.rs`

1649 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

2072 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/fiber.rs`

large maintained owner contains 211 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

2146 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

1972 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

2012 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

1751 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/eval.rs`

1529 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/eval/calls.rs`

2481 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/pure.rs`

2108 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/tests/flow.rs`

2826 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/value.rs`

2490 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1580 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering.rs`

1445 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering.rs`

2204 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests.rs`

3162 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests/control.rs`

2826 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/pattern_lowering.rs`

1300 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/statement_lowering.rs`

1822 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/item/retained.rs`

1565 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/leaf.rs`

1485 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/module.rs`

1958 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/slot.rs`

1291 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index.rs`

1725 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection.rs`

2264 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection/thread_control.rs`

1589 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest.rs`

1240 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest/candidate_projection.rs`

1356 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection.rs`

1495 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection/flow.rs`

1661 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/pattern_projection.rs`

1344 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/tests.rs`

3020 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

1259 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/table.rs`

1976 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/batch.rs`

1563 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

2153 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

facade file has 2153 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lower.rs`

1438 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/builder.rs`

1481 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/checked_catalog.rs`

1727 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/facts.rs`

1265 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/identity.rs`

1931 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

1367 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/checked_rich_text/checker.rs`

1704 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/entry/checker.rs`

1412 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/env/base.rs`

1265 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs`

2015 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`

1854 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/tests.rs`

4960 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/validation.rs`

1915 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/registration/registrar.rs`

1800 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment.rs`

2394 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/attachment.rs`

large maintained owner contains 1538 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/callable.rs`

1842 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/choice.rs`

1315 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression.rs`

1911 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression/structure.rs`

1224 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/flow/tests.rs`

2565 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/source.rs`

1227 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/expressions.rs`

1222 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

1290 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

large maintained owner contains 294 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

1399 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

large maintained owner contains 61 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/incremental/database_tests.rs`

3903 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/lint.rs`

1373 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/lint.rs`

large maintained owner contains 589 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

1245 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

large maintained owner contains 153 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/declaration.rs`

1905 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/expression.rs`

2390 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/pattern.rs`

1276 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/rich_text_grammar.rs`

2106 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/statement.rs`

1973 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/types/token/grammar.rs`

1621 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lsp/src/diagnostics.rs`

1335 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lsp/src/diagnostics.rs`

large maintained owner contains 917 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

1693 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

large maintained owner contains 791 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lsp/src/profiles/accepted_project.rs`

1261 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-presentation/src/text_editor.rs`

1965 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/text_editor.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/text_input.rs`

1695 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/text_input.rs`

large maintained owner contains 32 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

1345 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project-loader/src/cache/inspect.rs`

large maintained owner contains 453 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-project-loader/src/cache/persistent_query.rs`

1828 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-project-loader/src/cache/release.rs`

1579 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large maintained owner contains 792 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-project-loader/src/project.rs`

1378 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project-loader/src/project.rs`

large maintained owner contains 412 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-project-loader/src/topology/loader.rs`

1521 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-project/src/persistent_object/codec.rs`

1553 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project/src/persistent_object/codec.rs`

large maintained owner contains 441 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-render-text/src/resolved_document.rs`

1203 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/geometry.rs`

2163 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/renderer.rs`

1657 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/view_compositor.rs`

1563 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/call_backend.rs`

1930 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/compile.rs`

1550 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1605 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

2010 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

large maintained owner contains 525 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/lib.rs`

1285 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-runtime-accelerator/src/lib.rs`

facade file has 1285 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/math.rs`

2340 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/math/browser_webgpu/context.rs`

1862 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/math/wgpu_backend.rs`

1398 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/tests.rs`

2504 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/session.rs`

1287 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime.rs`

1291 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs`

1623 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/tests/session.rs`

2708 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/tests/view_runtime.rs`

2613 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-host/src/bundle_runner.rs`

1287 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-host/src/bundle_runner.rs`

large maintained owner contains 401 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/expr.rs`

1207 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

2209 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

1413 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

1984 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

large maintained owner contains 207 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/semantic_facts.rs`

2040 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1896 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1896 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large maintained owner contains 807 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1109 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `tools/structure-audit.rs`

1628 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `tools/structure-audit.rs`

large maintained owner contains 278 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `tools/verify-text-raster-parity.rs`

1795 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code
