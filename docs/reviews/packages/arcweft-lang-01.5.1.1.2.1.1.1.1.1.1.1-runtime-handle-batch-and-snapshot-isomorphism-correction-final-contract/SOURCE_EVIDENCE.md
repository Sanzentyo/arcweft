# Repository evidence and source inventory

**Latest main observed:** `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009`  
**Production source cut:** `3670625a02b9e7e8578b57fc7b148a1758a17dba`

The latest main is one intake/audit documentation commit after the production cut named by the correction. Source facts below were read at the production cut unless the row explicitly says latest main.

| Path | Ref | Evidence used by this package |
|---|---|---|
| `AGENTS.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | root repository constraints and validation expectations |
| `docs/AGENTS.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | documentation evidence and authority rules |
| `docs/reviews/AGENTS.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | review-package requirements |
| `crates/AGENTS.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | crate layering and Sans-I/O boundaries |
| `docs/implementation/2026-08-22-lang-01-5-1-1-2-1-1-1-1-1-1-task-substrate-return-intake.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | parent-return intake; records parent ZIP hash/size/member count and reconciliation blockers |
| `crates/arcweft-core/src/value.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | RuntimeValue, RuntimeFunctionBody Structured/Awbc, RuntimeSeq/DenseSeq live algebra |
| `crates/arcweft-core/src/value/awbc_save.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | existing sole typed AWBC snapshot owner and exact current nested schemas |
| `crates/arcweft-core/src/value/agent.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current RuntimeAgentValue and RuntimeAgentPredicate variants |
| `crates/arcweft-core/src/pattern.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current RuntimeCheckedType and RuntimeVariantIdentity |
| `crates/arcweft-core/src/plan/type_kind.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | RuntimeAgentOperationalType and runtime-plan projection graph |
| `crates/arcweft-core/src/task.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current TaskSpec/HostTaskRequest/event source and migration inventory |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current deterministic scheduler and absence of batch/observer/cancel protocol |
| `crates/arcweft-runtime-scheduler/Cargo.toml` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current scheduler depends only on arcweft-core |
| `crates/arcweft-host-adapter/src/lib.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current immediate submit and cancel->bool paths that must be replaced |
| `crates/arcweft-host-adapter/Cargo.toml` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | host adapter is an upward implementation layer |
| `crates/arcweft-lang-hir/src/expr.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | 38 current HirExprKind families and exact direct_expression_children order |
| `crates/arcweft-lang-hir/src/pattern.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | 13 current HirPatternKind families |
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | current CheckedExpressionResolution/value/select facts |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `3670625a02b9e7e8578b57fc7b148a1758a17dba` | CheckedCallableId/CheckedCallableDigest and accepted callable authority |
| `docs/02-runtime/async-scheduler.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | maintained scheduler contract and accepted epoch/task/sequence event order |
| `docs/02-runtime/need-timeout.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | maintained timeout semantics |
| `docs/02-runtime/executable-runtime-core.md` | `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009` | AWBC v1 and retained opcode allocation including MakeNeedHandle/NeedTimeout |

## Concrete observations

- `arcweft-runtime-scheduler/Cargo.toml` lists only `arcweft-core`; the final   design preserves that Sans-I/O edge.
- `arcweft-host-adapter` currently owns immediate `submit` and boolean cancel   paths; their timing does not provide rollback and must be replaced.
- `TaskEvent` comparison already uses epoch, TaskId, then sequence. This package   preserves it instead of adopting the parent's sequence-before-TaskId text.
- `AwbcRuntimeValueSnapshot` already projects iterator, sequence, opaque,   reduction, Agent and AWBC function data with substantial fidelity. It is   completed in place rather than shadowed.
- `RuntimeFunctionBody` has Structured and Awbc branches. Structured owns an   `Arc<RuntimePlan>`, site, captures and bound arguments; no accepted current   restore authority rebinds that plan, so strict rejection is required.
- `HirExprKind::direct_expression_children` explicitly inventories 38 families   and states that statement/FlowItem owners remain separate roots.
- the current callable identity module provides checked callable ID/digest   authority; names are lookup material, not semantic identity.

## Parent archive boundary

The retained parent archive is identified by SHA-256 `034A2EEAB2D083B5BB4496F4EE63040B2F93B30ABDDA1B18E93138E28B65391B`. The repository intake records 197,348 bytes and 61 members. The execution container did not receive those binary bytes, so this package does not claim an independent local rehash. The searchable frozen mirror and intake findings were used, and this limitation is repeated in `VERIFICATION_SCOPE.md`.
