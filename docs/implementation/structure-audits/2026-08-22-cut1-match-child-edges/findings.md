# Structural findings

## review-trigger SIZE001 — `crates/arcweft-adapter-context/src/codec.rs`

1203 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-adapter-context/src/codec.rs`

large maintained owner contains 254 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-adapter-sema/src/registration/input.rs`

1359 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-adt/src/lib.rs`

1461 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-adt/src/lib.rs`

facade file has 1461 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-adt/src/lib.rs`

large maintained owner contains 121 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-agent-runner/src/tests.rs`

2852 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-bundle/src/container.rs`

2398 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/container.rs`

large maintained owner contains 662 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/lib.rs`

2377 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2377 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-bundle/src/lib.rs`

large maintained owner contains 811 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1784 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/patch.rs`

large maintained owner contains 145 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/product.rs`

1419 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/product.rs`

large maintained owner contains 574 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/release.rs`

2190 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/release.rs`

large maintained owner contains 994 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2085 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

large maintained owner contains 65 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

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

1573 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3231 physical LOC exceeds the 2500 LOC ownership-review trigger

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

1408 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2376 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large maintained owner contains 70 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1622 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/project.rs`

1486 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/project.rs`

large maintained owner contains 55 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/project_commands.rs`

2488 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/runtime/run.rs`

1395 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large maintained owner contains 300 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/output.rs`

1220 physical LOC exceeds the 1200 LOC ownership-review trigger

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

6717 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6109 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4218 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

4542 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

6851 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-compiler/src/lower.rs`

3147 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1943 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large maintained owner contains 525 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-compiler/src/project.rs`

1675 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

2696 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/fiber.rs`

large maintained owner contains 234 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/product_step.rs`

1597 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

2163 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

2410 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

2337 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

1884 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/engine.rs`

1433 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/entry/schema.rs`

1308 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/pattern.rs`

1689 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/pattern.rs`

large maintained owner contains 215 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction.rs`

1973 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/plan/construction.rs`

large maintained owner contains 227 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction/lower.rs`

3864 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/plan/construction/lower.rs`

large maintained owner contains 140 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction/seed.rs`

2088 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/pure.rs`

2631 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/root.rs`

1222 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/root.rs`

large maintained owner contains 94 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/value.rs`

3323 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/value/agent.rs`

2014 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/value/agent.rs`

large maintained owner contains 112 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1771 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/value/sequence_impls.rs`

large maintained owner contains 107 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering.rs`

1445 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering.rs`

2305 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests.rs`

3165 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests/control.rs`

2826 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/pattern_lowering.rs`

1300 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/statement_lowering.rs`

1896 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/item/retained.rs`

1565 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/leaf.rs`

1502 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/module.rs`

1954 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/slot.rs`

1291 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index.rs`

1721 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection.rs`

2429 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection/thread_control.rs`

1312 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest.rs`

1245 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest/candidate_projection.rs`

1336 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection.rs`

1488 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection/flow.rs`

1657 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/pattern_projection.rs`

1344 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/tests.rs`

3019 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

1259 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/table.rs`

1975 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/batch.rs`

1569 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

2225 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-lang-jit-cranelift/src/lib.rs`

facade file has 2225 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `crates/arcweft-lang-jit-cranelift/src/lower.rs`

1469 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/builder.rs`

1517 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/checked_catalog.rs`

1723 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/facts.rs`

1271 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/identity.rs`

1887 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/schema.rs`

1204 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

1529 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

large maintained owner contains 54 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/checked_rich_text/checker.rs`

1704 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/entry/checker.rs`

1417 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/env/base.rs`

1453 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/env/nominal.rs`

1248 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs`

2103 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`

2637 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/model.rs`

1767 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/tests.rs`

6090 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/validation.rs`

2332 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/registration/registrar.rs`

1811 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment.rs`

2381 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/attachment.rs`

large maintained owner contains 1538 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/callable.rs`

1851 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/choice.rs`

1314 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression.rs`

2128 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression/structure.rs`

1225 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/expressions.rs`

1229 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

1265 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

large maintained owner contains 294 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

1396 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/grammar/kinds.rs`

large maintained owner contains 61 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/incremental/database_tests.rs`

3800 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/lint.rs`

1328 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/lint.rs`

large maintained owner contains 589 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

1213 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/parser/cursor.rs`

large maintained owner contains 153 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/declaration.rs`

1924 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/expression.rs`

2492 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/pattern.rs`

1276 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/rich_text_grammar.rs`

2106 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/statement.rs`

2193 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/types/token/grammar.rs`

1621 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lsp/src/diagnostics.rs`

1321 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lsp/src/diagnostics.rs`

large maintained owner contains 903 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

1705 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lsp/src/features/nominal_types.rs`

large maintained owner contains 803 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

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

1576 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large maintained owner contains 789 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

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

1201 physical LOC exceeds the 1200 LOC ownership-review trigger

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

1943 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/compile.rs`

1617 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1622 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

2010 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-accelerator/src/inference.rs`

large maintained owner contains 525 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/lib.rs`

1291 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-runtime-accelerator/src/lib.rs`

facade file has 1291 physical LOC; target is below 250 LOC

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

2660 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/session.rs`

1283 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime.rs`

1292 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs`

1623 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/tests/view_runtime.rs`

2614 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

2477 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

1380 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/final_expr.rs`

1922 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

3944 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

large maintained owner contains 305 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/semantic_facts.rs`

5092 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1903 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1903 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large maintained owner contains 820 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

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
