# Deletion matrix

No row authorizes a compatibility alias or delayed reader.

| Path/area | Old authority | Final action | Cut | Proof |
|---|---|---|---:|---|
| `crates/arcweft-core/src/task.rs` | String-backed NeedId/TaskKey/TaskId | replace with fixed private-field newtypes; delete String constructors/accessors | 5 | type/API absence + wire tamper |
| `crates/arcweft-core/src/task.rs` | TaskSpec.id / TaskSpec.key | delete; TaskSpec carries producer instance and generation | 5 | struct field compile-fail |
| `crates/arcweft-core/src/task.rs` | TaskHandle.id / TaskHandle.key | replace with one TaskCorrelation field | 5 | struct field compile-fail |
| `crates/arcweft-core/src/task.rs` | TaskEvent logical/task-only envelope | replace with complete correlation + cursor | 5 | event tamper tests |
| `crates/arcweft-core/src/task.rs` | uncorrelated RuntimeNeedState | replace with complete correlation | 5 | restore/journal tests |
| `crates/arcweft-runtime-driver/src/swap.rs` | driver-local GenerationId | delete after consumers import arcweft_core::task::GenerationId | 4/5 | single-definition dependency test |
| `crates/arcweft-runtime-driver/src/task.rs` | String task/Need DTO fields and Option<String> | replace with final typed correlation/Option | 5 | API + snapshot tests |
| `crates/arcweft-runtime-scheduler` | caller identity acceptance | scheduler accepts final TaskSpec and uses TaskHost-derived correlation | 5 | join/always transaction tests |
| `crates/arcweft-core/src/awbc/schema.rs` | AwbcTaskPlan.need_id | replace with mandatory AwbcNeedProducerRow | 5 | schema/API absence |
| `crates/arcweft-core/src/awbc/schema.rs` | plan_digest stored in task plan | delete; use semantic_digest(&AwbcProgram) | 4/5 | structural absence + tamper |
| `crates/arcweft-core/src/awbc/verifier` | NeedHandle as String/Dynamic type admission | require payload-typed NeedHandle and policy validation | 5 | negative verifier test |
| `crates/arcweft-core/src/awbc/vm.rs` | NeedHandle RuntimeValue::String construction | construct RuntimeValue::NeedHandle | 5 | value-shape differential |
| `crates/arcweft-core/src/awbc/vm.rs` | await_target nonempty String -> NeedId | delete; read concrete RuntimeNeedHandle | 5 | compile-fail/String negative |
| `crates/arcweft-core/src/awbc/fiber.rs` | old Await/AwaitMany String snapshot fields | replace with correlated handles/in-flight rows | 5 | restore exactness |
| `crates/arcweft-runtime-plan` | task interning keyed by String need_id | key by final plan/producer rows | 5 | deterministic interning test |
| `crates/arcweft-runtime-plan` | indexed NeedId suffix generation | delete; index is canonical child argument | 5 | index-boundary tests |
| `crates/arcweft-runtime-driver` | direct-Await surrogate identity | delete; AwaitTarget owns RuntimeNeedHandle | 5 | API absence |
| `crates/arcweft-runtime-driver` | journal rows lacking producer contract/correlation | replace atomically with final journal | 5 | journal tamper tests |
| `crates/arcweft-runtime-driver` | old save/replay String readers | delete, no compatibility branch | 5 | old snapshot strict rejection |
| `crates/arcweft-runtime-driver` | revision-to-Need identity translation | delete/forbid; replacement rederives generation task correlation only | 3/5 | revision-only replacement |
| `crates/arcweft-native-adapter` | adapter-supplied task identity | accept HostTaskLaunchRequest with derived correlation | 5 | adapter envelope tests |
| `crates/arcweft-web-adapter` | adapter-supplied task identity | accept HostTaskLaunchRequest with derived correlation | 5 | Web parity |
| `crates/arcweft-headless` | task-id-only events | emit complete TaskEvent | 5 | headless parity |
| `crates/arcweft-agent-runner` | task-id-only/Need String observation | consume complete correlation/handle | 5 | Agent parity |
| `crates/arcweft-bundle` | old task/Need String codec | strict v1 fixed-byte final schema | 5 | codec tamper + old bytes rejection |
| `crates/arcweft-bundle` | plan self-digest field | store expected digest only in binding/snapshot envelope | 5 | recompute mismatch test |
| `crates/arcweft-view` | invented ViewProgramSemanticDigest | do not add; retain ViewProgramId | 3 | type/API absence |
| `crates/arcweft-view` | u32 accepted View revision | do not add; retain [u8;32] revision owner | 3 | schema absence |
| `crates/arcweft-lang-sema` | caller-provided CheckedMatchCoverage/exhaustive | delete/forbid; private analyzer only | 1 | constructor signature test |
| `crates/arcweft-lang-sema` | generic Match ownership gate | delete/forbid; separate View admission | 1/3 | ordinary affine Match test |
| `crates/arcweft-lang-sema` | nonliteral checked constant guard folding | delete; only Boolean literal rows | 1 | guard coverage differential |
| `crates/arcweft-lang-sema` | AcceptedNominalInventoryInput without value_class/persistence | delete old constructor | 2 | compile-fail + missing evidence |
| `crates/arcweft-lang-sema` | AcceptedNominalSemantics::Opaque { producer } | replace original variant fields | 2 | exhaustive compile/test |
| `crates/arcweft-lang-sema` | opaque evidence default/name inference | delete/forbid | 2 | negative input tests |
| `crates/arcweft-lang-sema` | ResourceTypeRegistry in current ownership context | delete route until exact typed key exists | 2 | dependency/API absence |
| `crates/arcweft-lang-sema` | whole unrelated catalog digest in View admission | replace with consulted OwnershipEvidenceDigest | 3 | unrelated catalog invariance |
| `crates/arcweft-compiler` | copied Match arms/coverage in View catalog | retain CheckedMatchRef and admission only | 3 | structural absence |
| `crates/arcweft-compiler` | revision included in task-plan digest | delete; use program/site/admission | 3 | revision-only identity property |
| `all task/Need codecs` | hex/String fallback or old numeric reader | delete | 5 | strict negative decode |
| `all task/Need fixtures` | hard-coded String/suffix identities | replace with transcript builders/final owners | 5 | workspace compile |
| `parent View product` | ViewProgramInstruction::Await / ViewAwait / ViewAwaitBranchSpan | delete in atomic final consumer cut where still present | 5 | API/schema/generated absence |
| `parent View evaluator` | four-way Await evaluator / InvalidAwaitState | delete; generic Match over Need states | 5 | runtime differential |
| `parser/formatter/LSP/generated surfaces` | AwaitView spelling or stale old Await product vocabulary | delete/unavailable | 5 | structural/API tests |

## Deletion completion rule

Cut 5 is complete only when all rows assigned to it are absent from production,
tests, fixtures, codecs, generated artifacts, snapshots, documentation examples
that claim current behavior, and adapter envelopes. A dead private fallback is
still a failure.

Search output alone is not the proof. Structural tests must compile against the
intended public API, decode rejected legacy shapes, inspect generated schema
models, and execute final save/replay/adaptor paths.
