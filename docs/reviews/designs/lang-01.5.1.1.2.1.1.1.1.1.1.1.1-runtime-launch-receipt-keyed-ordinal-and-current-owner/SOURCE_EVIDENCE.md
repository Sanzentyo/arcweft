# Current repository evidence

All observations below are from Git commit
`61779d1432b902efc2d19041a7326f3c1319828a`. Blob IDs are recorded in
`machine/final_contract.json` and verified by `tools/validate_design.rs`.

| Current owner | Evidence used by the final design |
|---|---|
| `crates/arcweft-core/src/task.rs` | current TaskSpec/request/scheduler boundary is the deletion target; event normalization owner remains core |
| `crates/arcweft-runtime-scheduler/src/lib.rs` and Cargo manifest | current scheduler is Sans I/O and depends only on core; final generic protocol preserves that direction |
| `crates/arcweft-host-adapter/src/lib.rs` | current immediate `submit` and boolean cancel timing cannot implement reservation semantics and is deleted, not wrapped |
| `crates/arcweft-core/src/value.rs` | current RuntimeValue owner; `DenseSeq::Units(usize)` and `DenseSeq::Bool(DenseSeqStorage<bool>)`; dormant AWBC function body |
| `crates/arcweft-core/src/value/awbc_save.rs` | sole current `AwbcRuntimeValueSnapshot`; function row is exactly function/remaining params/captures and has no program authority; Dense snapshot already reuses `DenseSeq` |
| `crates/arcweft-core/src/awbc/fiber.rs` | enclosing fiber snapshot is correlated with its generation-pinned program and `validate_for_program` validates executable ownership |
| `crates/arcweft-core/src/pattern.rs` | `RuntimeCheckedType::variant_case`: Some=0/None=1 and Ok=0/Err=1 |
| `crates/arcweft-core/src/value/agent.rs` | concrete Agent carriers include Diagnostics and ViewportPoint |
| `crates/arcweft-lang-sema/src/types.rs` | 85 outer TypeKind variants and eight nested AgentBuiltinType cases; ArrayLength and other nested families require payload-level classification |
| `crates/arcweft-lang-sema/src/callable/checked_catalog.rs` | existing immutable `CheckedCallableCatalog` with `callable` and method lookup; no replacement catalog is needed |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `CheckedCallableId::semantic_digest` is the current callable digest owner |
| `crates/arcweft-lang-sema/src/callable/resolver/outcome.rs` | selected `ResolvedCallable::checked()` and typed intrinsic candidate facts distinguish catalog-backed and intrinsic calls |
| `crates/arcweft-lang-hir/src/expr.rs` | exactly 38 HirExprKind variants and the current direct-expression ordering to preserve during edge migration |
| `crates/arcweft-lang-hir/Cargo.toml` | HIR depends on id/syntax/source and not on core or sema; checked roles cannot be HIR-owned |
| `crates/arcweft-core/src/plan.rs` and `plan/type_kind.rs` | current RuntimePlan, type/layout/opaque projection, and operational-type owners reused by task/snapshot authority |
| `crates/arcweft-core/src/awbc/schema.rs` | current AwbcProgram and task-plan table reused directly; numeric allocation is not changed |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | actual accepted View product owner remains above core and implements the core validation protocol rather than being copied downward |

Maintained `docs/02-runtime/async-scheduler.md`, `need-timeout.md`, and
`executable-runtime-core.md` supply event, timeout, AWBC, safe-point, and
version-1 constraints. The retained parent package supplies accepted AwaitMany,
observer, cancellation, restartable-host, and Match tag details except where
the correction request explicitly supersedes it.

The frozen predecessor requests are also blob-pinned by the validator. The
runtime Need-instance correction makes `GenerationId(0)` and Join ordinal `0`
valid. The task-persistence correction reserves all-zero only for fixed
producer/Need/task identities and explicitly permits every semantic digest
value; `Option` owns absence. The final schema preserves that boundary instead
of extending `NonZero` to generation or digest domains.
