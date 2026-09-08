# Structural findings

## review-trigger SIZE001 — `crates/arcweft-adapter-context/src/codec.rs`

1289 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-adapter-context/src/codec.rs`

large maintained owner contains 327 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-adapter-sema/src/registration/input.rs`

1374 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-adt/src/lib.rs`

1370 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-adt/src/lib.rs`

facade file has 1370 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-adt/src/lib.rs`

large maintained owner contains 111 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-agent-runner/src/tests.rs`

3541 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-bundle/src/container.rs`

2398 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/container.rs`

large maintained owner contains 662 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/lib.rs`

2379 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-bundle/src/lib.rs`

facade file has 2379 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-bundle/src/lib.rs`

large maintained owner contains 796 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/patch.rs`

1827 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/patch.rs`

large maintained owner contains 188 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/product.rs`

1432 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/product.rs`

large maintained owner contains 587 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/release.rs`

2190 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/release.rs`

large maintained owner contains 994 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

2633 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-bundle/src/resource_codec/runtime.rs`

large maintained owner contains 199 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/codec.rs`

1695 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

1823 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-bundle/src/resource_codec/view/model.rs`

large maintained owner contains 55 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

1630 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/agent/native/mcp_protocol.rs`

large maintained owner contains 65 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/mcp_rag.rs`

1378 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/observe.rs`

1401 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/repl.rs`

1572 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/native/tests.rs`

3300 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/rag.rs`

1352 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/rag/source_index.rs`

1500 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/agent/script.rs`

2265 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/agent/script.rs`

large maintained owner contains 77 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/debug.rs`

2371 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/debug.rs`

large maintained owner contains 70 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/app/jit.rs`

1628 physical LOC exceeds the 1200 LOC ownership-review trigger

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

1450 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/app/runtime/run.rs`

large maintained owner contains 312 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/src/output.rs`

1209 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/src/toolchain_profile.rs`

2463 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-cli/src/toolchain_profile.rs`

large maintained owner contains 296 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`

5791 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs`

6717 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs`

6108 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs`

4211 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/agent_script_debug.rs`

4536 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-cli/tests/check/cli_runtime_bench.rs`

6868 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-compiler/src/lower.rs`

7821 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-compiler/src/persistent.rs`

1943 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-compiler/src/persistent.rs`

large maintained owner contains 525 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-compiler/src/project.rs`

1707 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-compiler/src/view.rs`

1421 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/codec/code.rs`

1655 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/codec/code.rs`

large maintained owner contains 58 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/codec/metadata.rs`

1426 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/codec/metadata.rs`

large maintained owner contains 94 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/codec/runtime.rs`

1226 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/fiber.rs`

3549 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/fiber.rs`

large maintained owner contains 191 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/product_step.rs`

2842 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/product_step/line.rs`

1425 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/product_step/snapshot.rs`

2280 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/awbc/product_step/snapshot.rs`

large maintained owner contains 121 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/schema.rs`

3040 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/tests.rs`

4882 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/code.rs`

3738 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/verify/structure.rs`

2842 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/awbc/vm.rs`

2855 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/engine.rs`

2056 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/dialogue.rs`

1676 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/engine/dialogue.rs`

large maintained owner contains 102 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/eval.rs`

1349 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/engine/eval.rs`

large maintained owner contains 124 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/flow.rs`

1566 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/engine/suspend.rs`

1239 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/entry/schema.rs`

1580 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/entry/schema.rs`

large maintained owner contains 49 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/line_task.rs`

1873 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/line_task/handle.rs`

3416 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/line_task/handle.rs`

large maintained owner contains 171 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/pattern.rs`

2905 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/pattern.rs`

large maintained owner contains 630 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan.rs`

1394 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction.rs`

2758 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/plan/construction.rs`

large maintained owner contains 256 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction/lower.rs`

5361 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/plan/construction/lower.rs`

large maintained owner contains 208 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/construction/seed.rs`

2524 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/plan/entry_inventory.rs`

1492 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-core/src/pure.rs`

3067 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-core/src/pure.rs`

large maintained owner contains 132 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/root.rs`

1215 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/root.rs`

large maintained owner contains 86 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/runtime_id.rs`

1287 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/runtime_id.rs`

large maintained owner contains 96 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/task.rs`

1568 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/task.rs`

large maintained owner contains 100 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/value.rs`

3800 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-core/src/value/agent.rs`

2300 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/value/agent.rs`

large maintained owner contains 205 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/value/opaque.rs`

2442 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/value/opaque.rs`

large maintained owner contains 645 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-core/src/value/sequence_impls.rs`

1772 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-core/src/value/sequence_impls.rs`

large maintained owner contains 107 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE002 — `crates/arcweft-host-adapter/src/lib.rs`

facade file has 1123 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering.rs`

1472 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering.rs`

2403 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests.rs`

3141 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests/control.rs`

2826 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/item_lowering/callable.rs`

1305 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/pattern_lowering.rs`

1300 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_lowering/statement_lowering.rs`

2105 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs`

1398 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_project/semantic_paths.rs`

6396 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/final_project/tests.rs`

4837 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/item.rs`

1212 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/item/retained.rs`

1544 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/leaf.rs`

1502 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/module.rs`

2058 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/slot.rs`

1291 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index.rs`

1731 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection.rs`

2761 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/block_projection/thread_control.rs`

1311 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest.rs`

1395 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest/candidate_projection.rs`

1394 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/expression_manifest/projection.rs`

1264 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection.rs`

1468 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/item_projection/flow.rs`

1658 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/pattern_projection.rs`

1344 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/source_index/tests.rs`

3019 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/stmt.rs`

1926 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

1269 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-hir/src/symbol/identity.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-hir/src/symbol/table.rs`

2027 physical LOC exceeds the 1200 LOC ownership-review trigger

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

1704 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/checked_application.rs`

4503 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/checked_catalog.rs`

2639 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/constraints.rs`

2185 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/constraints.rs`

large maintained owner contains 1317 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/continuation.rs`

2385 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/continuation.rs`

large maintained owner contains 27 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/facts.rs`

1265 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/identity.rs`

1893 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/join.rs`

1776 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/limits.rs`

1405 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/limits.rs`

large maintained owner contains 149 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/projection.rs`

1212 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/resolver.rs`

1579 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/resolver/outcome.rs`

1514 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/schema.rs`

4548 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/schema.rs`

large maintained owner contains 1186 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

2598 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/callable/schema/families.rs`

large maintained owner contains 540 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/checked_text_proxy.rs`

1847 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/checked_text_proxy/prepared.rs`

1678 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/effect_row.rs`

1289 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/effect_row.rs`

large maintained owner contains 339 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/entry/checker.rs`

2024 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/env/base.rs`

2274 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/env/nominal.rs`

1720 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs`

2015 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs`

4333 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs`

large maintained owner contains 185 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs`

4913 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs`

large maintained owner contains 476 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/dialogue_line_plan.rs`

1669 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/dialogue_line_plan.rs`

large maintained owner contains 66 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/evaluated_effects.rs`

1890 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`

3904 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`

large maintained owner contains 88 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/fx_definition.rs`

2234 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/items.rs`

1536 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/state.rs`

3064 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/analyzer/state.rs`

large maintained owner contains 503 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/fx_application.rs`

2581 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/match_edges.rs`

1461 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/model.rs`

2775 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs`

2934 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/prepared.rs`

1511 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/report.rs`

2234 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs`

4098 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs`

large maintained owner contains 98 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/tests.rs`

9019 physical LOC exceeds the 8000 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/final_analysis/validation.rs`

2749 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/ownership.rs`

2266 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/ownership.rs`

large maintained owner contains 299 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/registration/model.rs`

2104 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/registration/model.rs`

large maintained owner contains 254 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/registration/registrar.rs`

1929 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/semantic_coordinate.rs`

2095 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/semantic_coordinate.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs`

1206 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs`

large maintained owner contains 341 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types.rs`

1715 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/types.rs`

large maintained owner contains 108 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/compatibility.rs`

1749 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/types/compatibility.rs`

large maintained owner contains 711 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/constraints/context.rs`

1650 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/constraints/tests.rs`

4062 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/constraints/transaction.rs`

1779 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/digest.rs`

1289 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-sema/src/types/digest.rs`

large maintained owner contains 123 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-sema/src/types/mismatch.rs`

1271 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment.rs`

1960 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-lang-syntax/src/attachment.rs`

large maintained owner contains 1115 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/callable.rs`

2039 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/choice.rs`

1314 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression.rs`

2134 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/expression/structure.rs`

1250 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/attachment/flow/tests.rs`

2590 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/expressions.rs`

1230 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/grammar/build.rs`

1265 physical LOC exceeds the 1200 LOC ownership-review trigger

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

3794 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/lint.rs`

1323 physical LOC exceeds the 1200 LOC ownership-review trigger

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

2127 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/expression.rs`

2519 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/pattern.rs`

1276 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/rich_text_grammar.rs`

2096 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-lang-syntax/src/parser/statement.rs`

2190 physical LOC exceeds the 1200 LOC ownership-review trigger

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

1270 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-presentation/src/fx/builtin.rs`

1220 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/fx/builtin.rs`

large maintained owner contains 381 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/fx/builtin/schema.rs`

1399 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/fx/builtin/schema.rs`

large maintained owner contains 99 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/fx/graph.rs`

2125 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-presentation/src/fx/graph/canonical.rs`

1249 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-presentation/src/fx/program.rs`

1730 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/fx/program.rs`

large maintained owner contains 205 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/rich_text/content_catalog.rs`

1414 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/rich_text/content_catalog.rs`

large maintained owner contains 39 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/text_editor.rs`

1965 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-presentation/src/text_editor.rs`

large maintained owner contains 73 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-presentation/src/text_input.rs`

1699 physical LOC exceeds the 1200 LOC ownership-review trigger

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

1588 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-project-loader/src/cache/release.rs`

large maintained owner contains 801 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

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

1346 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/geometry.rs`

2162 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/renderer.rs`

1657 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-render-wgpu/src/view_compositor.rs`

1563 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/call_backend.rs`

1947 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/compile.rs`

1630 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-accelerator/src/external.rs`

1623 physical LOC exceeds the 1200 LOC ownership-review trigger

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

2697 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/display.rs`

1295 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/session.rs`

1581 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-driver/src/session.rs`

large maintained owner contains 227 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime.rs`

1570 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs`

2008 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-driver/tests/view_runtime.rs`

2854 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-host/src/bundle_runner.rs`

1219 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-host/src/bundle_runner.rs`

large maintained owner contains 483 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-host/src/native_task.rs`

1331 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-runtime-host/src/native_task.rs`

large maintained owner contains 265 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/expr.rs`

2081 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs`

3247 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs`

2170 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/final_expr.rs`

2539 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

6875 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger TEST001 — `crates/arcweft-runtime-plan/src/final_flow.rs`

large maintained owner contains 361 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/semantic_facts.rs`

10526 physical LOC exceeds the 2500 LOC upper ownership-review trigger; LOC alone is not a structural error

**Disposition:** record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/semantic_facts/project_function.rs`

2364 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-runtime-plan/src/semantic_facts/tests.rs`

2855 physical LOC exceeds the 2500 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE001 — `crates/arcweft-text-model/src/content.rs`

1431 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-text-model/src/content.rs`

large maintained owner contains 207 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-text-model/src/playback.rs`

1711 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-text-model/src/playback.rs`

large maintained owner contains 399 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-verify-lsp/src/lib.rs`

1670 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-verify-lsp/src/lib.rs`

facade file has 1670 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger TEST001 — `crates/arcweft-verify-lsp/src/lib.rs`

large maintained owner contains 733 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `crates/arcweft-verify/src/lib.rs`

1229 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger SIZE002 — `crates/arcweft-verify/src/lib.rs`

facade file has 1229 physical LOC; target is below 250 LOC

**Disposition:** review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports

## review-trigger SIZE001 — `crates/arcweft-view/src/program.rs`

1222 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `crates/arcweft-view/src/program.rs`

large maintained owner contains 253 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `tools/structure-audit.rs`

1628 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code

## review-trigger TEST001 — `tools/structure-audit.rs`

large maintained owner contains 278 physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding

**Disposition:** review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling

## review-trigger SIZE001 — `tools/verify-text-raster-parity.rs`

1795 physical LOC exceeds the 1200 LOC ownership-review trigger

**Disposition:** name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code
