# Repository/source evidence

Repository `Sanzentyo/arcweft`.

- current main: `cec30b57fa734efb059d7b846b397ac7d2b0701a`
- production parent: `0fa8a3b845b2dc966f181f450a1ca1f36e49d966`
- compare: one docs-only commit ahead, zero Rust file changes
- acquisition: connected GitHub repository with exact ref per fetch
- local production clone/Cargo: not used for this design-only return

Each row identifies exact SHA/path/line range, owner/API, observed fact, and
consumer dependency. Result-changing claims rely on Rust rows, not filenames.
| ID | Kind | Path | Lines | Owner/API | Observed fact | Consumer |
|---|---|---|---|---|---|---|
| E001 | Doc | docs/01-language/await-need-result.md | 1-35 | unary Need/carrier authority | Need has no error parameter; fallible/optional payloads are nested | sema/runtime types |
| E002 | Doc | docs/01-language/await-need-result.md | 36-67 | await and temporal states | await strips one layer outside View; four temporal states | language/runtime |
| E003 | Doc | docs/01-language/await-need-result.md | 68-116 | cancellation/observers | Ready Err differs from cancellation; error/denied Await branches removed | View correction |
| E004 | Doc | docs/03-presentation/view-reactive.md | 1-45 | View ordinary match surface | ordinary Match is authored View control | syntax/HIR/sema |
| E005 | Doc | docs/03-presentation/view-reactive.md | 46-113 | bundle/mount execution | definition-scoped programs, monotonic mount IDs, strict typed evaluator | bundle/runtime |
| E006 | Doc | docs/03-presentation/view-reactive.md | 114-155 | reactive Need rule | ordinary checked Match; no error/denied; shared budget | this contract |
| E007 | Doc | docs/03-presentation/view-reactive.md | 156-208 | save/reactive examples | restore validates identity; AwaitView absent; nested Ready Result | save/tooling |
| E008 | Package | docs/reviews/requests/2026-08-21-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation.md | 1-130 | primary request | twelve decisions and full inventory/test scope | entire package |
| E009 | Package | docs/reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction.md | 1-202 | correction request | complete redelivery, validation, exact archive | entire package |
| E010 | Package | docs/reviews/packages/arcweft-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation-final-contract/FINAL_CONTRACT.md | 1-125 | accepted parent | one catalog, generic Match, ordinary AWBC/RuntimeValue, transactions | retained parent |
| E011 | Package | docs/reviews/packages/arcweft-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation-final-contract/RUST_SCHEMAS.md | 1-250 | parent schemas | checked catalog, Match, value functions/bindings | schemas/interleave |
| E012 | Package | docs/reviews/packages/arcweft-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation-final-contract/PRODUCER_CONSUMER_MATRIX.md | 1-30 | parent consumers | one validated catalog feeds all backends | consumer matrix |
| E013 | Package | docs/reviews/packages/arcweft-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-final-contract/evidence/design-validation.json | 1-22 | failed validator | pass false and missing/undersized artifacts | redelivery validator |
| E014 | Rust | crates/arcweft-lang-hir/src/expr/control.rs | 1-160 | HirMatchExpr/HirMatchArm | ordinary match owns scrutinee/source-ordered arms | CheckedViewNeedMatch |
| E015 | Rust | crates/arcweft-lang-hir/src/expr/control.rs | 160-360 | control expression identities | match/arm ExprIds are HIR identities, not strings | catalog key |
| E016 | Rust | crates/arcweft-lang-hir/src/pattern.rs | 1-180 | HirPattern/HirPatternKind | ordinary typed pattern graph and PatternId owner | AWBC lowering |
| E017 | Rust | crates/arcweft-lang-hir/src/pattern.rs | 180-360 | pattern children/bindings | nested variants/bindings retain exact identity | nested carriers |
| E018 | Rust | crates/arcweft-lang-sema/src/types.rs | 384-500 | ProgressField/TypeKind | closed Progress and unary Need type | exact source type |
| E019 | Rust | crates/arcweft-lang-sema/src/types.rs | 500-620 | Need/Result/Option types | temporal/domain/absence carriers are distinct | nested contract |
| E020 | Rust | crates/arcweft-lang-sema/src/final_analysis/report.rs | 1-57 | FinalSemanticAnalysis | immutable report bound to exact generation | sole catalog |
| E021 | Rust | crates/arcweft-lang-sema/src/final_analysis/report.rs | 58-126 | try_new transaction | generation validation precedes report publication | precedence |
| E022 | Rust | crates/arcweft-lang-sema/src/final_analysis/report.rs | 127-180 | fact inventory validation | types/bindings/expressions/patterns/items/calls validate | completeness |
| E023 | Rust | crates/arcweft-lang-sema/src/final_analysis/model.rs | 1-130 | checked fact model | current model lacks checked View subscription owner | missing substrate |
| E024 | Rust | crates/arcweft-lang-sema/src/final_analysis/model.rs | 130-260 | patterns/bindings/effects | ordinary semantic facts exist for retained identities | catalog references |
| E025 | Rust | crates/arcweft-runtime-plan/src/semantic_facts.rs | 1-80 | generation-bound facts | qualified HIR IDs; no source/string identity | producer projection |
| E026 | Rust | crates/arcweft-runtime-plan/src/semantic_facts.rs | 80-180 | RuntimeTypeShape | Need/Result/Option/Progress exact runtime shapes | type contract |
| E027 | Rust | crates/arcweft-runtime-plan/src/semantic_facts.rs | 180-300 | checked-type projection | unsupported shape fails, no fallback | type failure |
| E028 | Rust | crates/arcweft-compiler/src/view.rs | 1-180 | View compiler transaction | HIR+final analysis feed scratch product | compiler owner |
| E029 | Rust | crates/arcweft-compiler/src/view.rs | 180-380 | instruction lowering | missing checked path must fail closed | interleave |
| E030 | Rust | crates/arcweft-compiler/src/view.rs | 500-700 | value lowering | current narrow presentation path has no Need binding | parent substrate |
| E031 | Rust | crates/arcweft-compiler/src/view.rs | 700-920 | source/schema hash | source is diagnostic; schema hash deterministic | wire identity |
| E032 | Rust | crates/arcweft-need/src/lib.rs | 1-55 | Need<T> | unary temporal owner without domain error parameter | state projection |
| E033 | Rust | crates/arcweft-need/src/lib.rs | 55-105 | Need methods/states | NotStarted/Pending/Ready/Cancelled owned by Need | selection |
| E034 | Rust | crates/arcweft-core/src/task.rs | 1-50 | NeedId/cursor | typed NeedId, epoch, sequence, cursor exist | journal identity |
| E035 | Rust | crates/arcweft-core/src/task.rs | 51-150 | RuntimeNeedState/outcome | typed RuntimePayload; Result remains payload | ordinary value |
| E036 | Rust | crates/arcweft-core/src/task.rs | 151-260 | task request/policy | typed task plan/policy own start/dedup | start intent |
| E037 | Rust | crates/arcweft-core/src/task.rs | 480-620 | Need state resolution | events normalize to RuntimeNeedState/cursor | journal algorithm |
| E038 | Rust | crates/arcweft-core/src/task.rs | 620-840 | terminal helpers | first terminal and typed publication behavior exist | reuse owner |
| E039 | Rust | crates/arcweft-core/src/value.rs | 1-180 | RuntimeValue/RuntimePayload | ordinary runtime value algebra is value owner | generic Match |
| E040 | Rust | crates/arcweft-core/src/value.rs | 180-340 | variant/aggregate values | nested Result/Option payloads are representable | nested patterns |
| E041 | Rust | crates/arcweft-core/src/value.rs | 430-620 | canonical value behavior | typed equality/ownership support state digest/bindings | duplicates |
| E042 | Rust | crates/arcweft-core/src/value.rs | 620-760 | value depth/containers | recursive validation belongs to ordinary owner | limits |
| E043 | Rust | crates/arcweft-core/src/awbc/schema.rs | 1-90 | AWBC v1 IDs | ABI/version 1 and typed function/pattern/match/task IDs | v1 refs |
| E044 | Rust | crates/arcweft-core/src/awbc/schema.rs | 90-190 | AwbcProgram tables | functions/patterns/match arms/task plans share program | selector |
| E045 | Rust | crates/arcweft-core/src/awbc/schema.rs | 190-330 | canonicalization | cross-table IDs are canonical/verified | strict binding |
| E046 | Rust | crates/arcweft-core/src/awbc/product_step/snapshot/task_publication.rs | 1-180 | publication snapshot | cursors/state are existing snapshot authority | save/replay |
| E047 | Rust | crates/arcweft-core/src/awbc/product_step/snapshot/task_publication.rs | 180-360 | publication validation | cursor/state validation shared by View journal | restore |
| E048 | Rust | crates/arcweft-core/src/awbc/product_step/suspension.rs | 1-180 | NeedHandle/task plan | typed AWBC owners, not RuntimeValue string | producer binder |
| E049 | Rust | crates/arcweft-core/src/awbc/product_step/suspension.rs | 180-360 | resume/cancel | cancellation is control; terminal deterministic | Cancelled |
| E050 | Rust | crates/arcweft-view/Cargo.toml | 1-24 | dependency graph | arcweft-view has no core dependency and stays Sans I/O | dependency direction |
| E051 | Rust | crates/arcweft-view/src/lib.rs | 1-95 | public exports | current crate re-exports old ViewAwait owners | deletion API proof |
| E052 | Rust | crates/arcweft-view/src/program.rs | 1-70 | ViewInstruction | current enum includes obsolete Await | delete/replace |
| E053 | Rust | crates/arcweft-view/src/program.rs | 220-315 | ViewAwait branches | old pending/ready/error/denied DTO | deletion |
| E054 | Rust | crates/arcweft-view/src/program.rs | 520-650 | instruction impl | part helpers explicitly match Await | consumer switch |
| E055 | Rust | crates/arcweft-view/src/program.rs | 650-820 | program validation | deterministic range/part validation | generic Match |
| E056 | Rust | crates/arcweft-view/src/reactive.rs | 1-120 | ReactiveGraph | BTree dependencies and monotonic revision | invalidation |
| E057 | Rust | crates/arcweft-view/src/value_program.rs | 1-115 | value program/mount snapshot | FxRuntimeValue is not arbitrary payload owner | no fallback |
| E058 | Rust | crates/arcweft-view/src/value_program.rs | 115-280 | typed inventory/cache | mount-scoped inputs/revisions | observer state |
| E059 | Rust | crates/arcweft-view/src/value_program.rs | 280-455 | snapshot restore | program/schema/type checks before restore | restore design |
| E060 | Rust | crates/arcweft-view/src/view.rs | 1-105 | ViewMountId/allocator | monotonic mount and checked restore cursor | remount identity |
| E061 | Rust | crates/arcweft-bundle/Cargo.toml | 1-45 | bundle dependencies | bundle depends on core/view for strict DTO joins | placement |
| E062 | Rust | crates/arcweft-bundle/src/resource_codec/view/model.rs | 1-160 | ViewProgram model | product currently owns old Await | wire replacement |
| E063 | Rust | crates/arcweft-bundle/src/resource_codec/view/model.rs | 160-330 | ViewAwaitBranchSpan | old four spans/source program exist | strict deletion |
| E064 | Rust | crates/arcweft-bundle/src/resource_codec/view/codec.rs | 1-220 | ViewProgram codec | strict canonical envelope/transcript owner | new v1 rows |
| E065 | Rust | crates/arcweft-bundle/src/resource_codec/view/codec.rs | 220-520 | instruction validation | variants exhaustively validated/budgeted | Match addition |
| E066 | Rust | crates/arcweft-bundle/src/resource_codec/view/codec.rs | 520-800 | cross-table checks | missing/duplicate refs fail | subscription join |
| E067 | Rust | crates/arcweft-bundle/src/resource_codec/view/codec/part.rs | 1-300 | part codec | old branch/span fields have explicit rows | discriminant deletion |
| E068 | Rust | crates/arcweft-bundle/src/resource_codec/view/semantic.rs | 1-300 | semantic digest | instruction variants contribute deterministic digest | subscription digest |
| E069 | Rust | crates/arcweft-bundle/src/resource_codec/view/merge.rs | 1-320 | atomic merge | candidate validates before publication/source remap | atomicity |
| E070 | Rust | crates/arcweft-bundle/src/resource_codec/view/codec/transcript.rs | 1-280 | strict transcript DTO | closed tagged payload rejects old bytes | no reader |
| E071 | Rust | crates/arcweft-bundle/src/resource_codec/runtime.rs | 1-145 | runtime function/types | NeedHandle and AWBC refs are typed cross-section facts | producer ref |
| E072 | Rust | crates/arcweft-bundle/src/resource_codec/runtime.rs | 145-330 | runtime budgets | codec is bounded/canonical | work accounting |
| E073 | Rust | crates/arcweft-runtime-driver/src/task.rs | 1-110 | RuntimeTaskRegistry | generation/status/cancel owner exists | producer lifecycle |
| E074 | Rust | crates/arcweft-runtime-driver/src/task.rs | 110-235 | task event ordering | events sort epoch/sequence/task; terminal ignores later | publication |
| E075 | Rust | crates/arcweft-runtime-driver/src/task.rs | 235-360 | HostTaskDispatch | generation/epoch/sequence and typed events | start/publication |
| E076 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | 1-120 | ViewProgramCatalog | validated product adapts to one runtime catalog | runtime owner |
| E077 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | 120-330 | catalog build/diff | candidate definitions/fingerprints before use | replacement |
| E078 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | 330-520 | instruction mapping | adapter maps old Await to ViewAwait | deletion switch |
| E079 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog.rs | 520-610 | map_await_branch | old helper must disappear | absence |
| E080 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs | 1-160 | definition fingerprints | instructions/program refs drive replacement | digest inclusion |
| E081 | Rust | crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs | 160-275 | Await references/source | old Await explicit branches remain | deletion |
| E082 | Rust | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | 1-260 | frame evaluator | instructions execute in bounded candidate state | Match integration |
| E083 | Rust | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | 820-1020 | four-way Await evaluator | I32 discriminant picks pending/ready/error/denied | delete |
| E084 | Rust | crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs | 1020-1180 | branch span/error logic | old Await is separate runtime authority | replace |
| E085 | Rust | crates/arcweft-runtime-driver/src/view_runtime/evaluator/support.rs | 1-220 | evaluation errors | InvalidAwaitState exists | delete diagnostic |
| E086 | Rust | crates/arcweft-runtime-driver/src/view_runtime.rs | 1-180 | runtime/frame owner | one owner publishes shared BundleViewFrame | parity |
| E087 | Rust | crates/arcweft-runtime-driver/src/view_runtime.rs | 340-620 | mount/save/eval transaction | mount tables/frame publication centralized | observer/save |
| E088 | Rust | crates/arcweft-runtime-driver/src/view_runtime/replacement/reconcile.rs | 1-320 | replacement reconciliation | candidate diff/reconcile before swap | Need mapping |
| E089 | Rust | crates/arcweft-bundle/tests/view_resource_codecs.rs | 1-120 | codec tests | old branch span imports/strict bytes | replace tests |
| E090 | Rust | crates/arcweft-bundle/tests/view_resource_codecs.rs | 120-330 | tamper/merge tests | unknown/invalid candidates reject atomically | tamper |
