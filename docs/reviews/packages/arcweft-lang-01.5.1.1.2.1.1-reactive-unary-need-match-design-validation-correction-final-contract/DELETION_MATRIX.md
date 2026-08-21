# Strict old-Await deletion matrix

All rows land in the same v1 consumer cut. Alias/reader does not satisfy deletion.
| ID | Owner | Path | Old surface | Action | Proof |
|---|---|---|---|---|---|
| DEL01 | arcweft-view | crates/arcweft-view/src/program.rs | ViewInstruction::Await | delete variant; generic Match only | compile-fail + AST/rg/API/schema/generated scan |
| DEL02 | arcweft-view | crates/arcweft-view/src/program.rs | ViewAwait | delete type | compile-fail + AST/rg/API/schema/generated scan |
| DEL03 | arcweft-view | crates/arcweft-view/src/program.rs | ViewAwaitBranch | delete type | compile-fail + AST/rg/API/schema/generated scan |
| DEL04 | arcweft-view | crates/arcweft-view/src/lib.rs | ViewAwait re-export | delete export | compile-fail + AST/rg/API/schema/generated scan |
| DEL05 | arcweft-view | crates/arcweft-view/src/lib.rs | ViewAwaitBranch re-export | delete export | compile-fail + AST/rg/API/schema/generated scan |
| DEL06 | arcweft-view | crates/arcweft-view/src/program.rs | part_kind Await arm | delete branch | compile-fail + AST/rg/API/schema/generated scan |
| DEL07 | arcweft-view | crates/arcweft-view/src/program.rs | part_id Await arm | delete branch | compile-fail + AST/rg/API/schema/generated scan |
| DEL08 | arcweft-view | crates/arcweft-view/src/program.rs | set_part_id Await arm | delete branch | compile-fail + AST/rg/API/schema/generated scan |
| DEL09 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | ViewProgramInstruction::Await | delete variant/tag | compile-fail + AST/rg/API/schema/generated scan |
| DEL10 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | ViewAwaitBranchSpan | delete DTO | compile-fail + AST/rg/API/schema/generated scan |
| DEL11 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await source_program | delete field | compile-fail + AST/rg/API/schema/generated scan |
| DEL12 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await pending_branch | delete field | compile-fail + AST/rg/API/schema/generated scan |
| DEL13 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await ready_branch | delete field | compile-fail + AST/rg/API/schema/generated scan |
| DEL14 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await error_branch | delete field | compile-fail + AST/rg/API/schema/generated scan |
| DEL15 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await denied_branch | delete field | compile-fail + AST/rg/API/schema/generated scan |
| DEL16 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/codec.rs | Await validation arm | delete; Match/subscription validation | compile-fail + AST/rg/API/schema/generated scan |
| DEL17 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/codec/part.rs | Await encode discriminant | delete codec row | compile-fail + AST/rg/API/schema/generated scan |
| DEL18 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/codec/part.rs | Await decode discriminant | delete codec row | compile-fail + AST/rg/API/schema/generated scan |
| DEL19 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/codec/transcript.rs | await tagged DTO | delete; unknown tag rejects | compile-fail + AST/rg/API/schema/generated scan |
| DEL20 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/semantic.rs | Await digest branch | delete; Match/subscription digest | compile-fail + AST/rg/API/schema/generated scan |
| DEL21 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/merge.rs | Await source remap | delete; generic remap | compile-fail + AST/rg/API/schema/generated scan |
| DEL22 | arcweft-bundle | crates/arcweft-bundle/src/resource_codec/view/model.rs | Await accessor/helper branches | delete branches | compile-fail + AST/rg/API/schema/generated scan |
| DEL23 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | ViewAwait imports | delete imports | compile-fail + AST/rg/API/schema/generated scan |
| DEL24 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | map ViewProgramInstruction::Await | delete adapter | compile-fail + AST/rg/API/schema/generated scan |
| DEL25 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | map_await_branch | delete helper | compile-fail + AST/rg/API/schema/generated scan |
| DEL26 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs | Await referenced-program arm | delete; subscription refs | compile-fail + AST/rg/API/schema/generated scan |
| DEL27 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs | Await source-stripping arm | delete | compile-fail + AST/rg/API/schema/generated scan |
| DEL28 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | ViewInstruction::Await dispatch | delete arm | compile-fail + AST/rg/API/schema/generated scan |
| DEL29 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | four-way I32 evaluator | delete implementation | compile-fail + AST/rg/API/schema/generated scan |
| DEL30 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | pending/ready/error/denied span selection | delete | compile-fail + AST/rg/API/schema/generated scan |
| DEL31 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/evaluator/support.rs | InvalidAwaitState | delete diagnostic | compile-fail + AST/rg/API/schema/generated scan |
| DEL32 | runtime-driver | crates/arcweft-runtime-driver/src/view_runtime/replacement/reconcile.rs | Await reconcile arm | delete/switch | compile-fail + AST/rg/API/schema/generated scan |
| DEL33 | tests | crates/arcweft-bundle/tests/view_resource_codecs.rs | ViewAwaitBranchSpan fixtures/imports | replace | compile-fail + AST/rg/API/schema/generated scan |
| DEL34 | tests | crates/arcweft-runtime-driver/tests/view_runtime.rs | Await evaluator cases | replace with Need Match | compile-fail + AST/rg/API/schema/generated scan |
| DEL35 | tests | workspace tests | InvalidAwaitState assertions | delete | compile-fail + AST/rg/API/schema/generated scan |
| DEL36 | syntax/tooling | workspace source | AwaitView spelling | zero definitions/references | compile-fail + AST/rg/API/schema/generated scan |
| DEL37 | generated | generated APIs/schemas | Await tag/discriminant | regenerate zero old | compile-fail + AST/rg/API/schema/generated scan |
| DEL38 | docs | accepted parent stale rows | CheckedViewExecution::Await/DirectAwait | supersede | compile-fail + AST/rg/API/schema/generated scan |
| DEL39 | docs | maintained chapters | direct View Await examples | zero stale | compile-fail + AST/rg/API/schema/generated scan |
| DEL40 | compatibility | workspace | alias/re-export/dual reader/source gate | must not exist | compile-fail + AST/rg/API/schema/generated scan |
