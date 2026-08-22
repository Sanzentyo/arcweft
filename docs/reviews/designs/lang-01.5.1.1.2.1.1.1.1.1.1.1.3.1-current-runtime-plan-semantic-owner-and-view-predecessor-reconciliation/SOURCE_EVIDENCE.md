# Source evidence

## Repository basis

- branch: `main`
- full Git SHA:
  `f43ca943d84f9a6a6da17605947a3d30c518a5a8`
- initial dirty state: untracked `.1.3` intake note, returned package/ZIP, and
  `.1.3.1` request only
- production edits by this design: none

## Current production facts used

| Path | Observation |
|---|---|
| `crates/arcweft-core/src/plan.rs` | `RuntimePlan` has fourteen current tables and no task-plan table. `FlowOp::{Await, AwaitMany, HostCall}` embed current task/request payloads. |
| `crates/arcweft-core/src/plan/construction.rs` | `RuntimePlanBuilder` owns one `Arc<RuntimePlanConstructionIssuer>`, builds the complete immutable plan, calls `verify`, and currently returns it directly from `finish`. |
| `crates/arcweft-core/src/plan/construction/seed.rs` | Current seed handles validate candidate ownership with `Arc::ptr_eq`; this is the legitimate coordinate issuer model. |
| `crates/arcweft-core/src/plan/type_table.rs` | A type declaration owns `RuntimeSemanticTypeId` plus one typed projection. |
| `crates/arcweft-core/src/plan/local_declarations.rs` | A local row owns only its `RuntimePlanTypeId`; returned-package mutability/init/owner roles do not exist. |
| `crates/arcweft-core/src/plan/nominal_record_domains.rs` | A final record field owns `String name` and type only; no accepted field semantic identity exists in core. |
| `crates/arcweft-core/src/plan/variant_domains.rs` | A final variant case owns `String name` and optional payload only; no accepted case semantic identity exists in core. |
| `crates/arcweft-core/src/plan/function_sites.rs` | A function site owns params, captures, and body only. Function identity/role/modes/result/endpoints are absent. |
| `crates/arcweft-core/src/task.rs` | Cut 4 identity substrate is landed; `TaskPlanSemanticDigest` still has a public raw constructor/Serde. Existing `ViewTaskPlanAuthority` has live `validate_view_task_plan`. `HostTaskRequestTemplate` is capability/operation String plus arguments. |
| `crates/arcweft-core/src/line_task.rs` | Static line nodes currently contain live `TaskId`, optional `TaskKey`, name and priority; these cannot enter static task-plan semantics. |
| `crates/arcweft-runtime-plan/src/final_flow.rs` | `lower_runtime_plan_with_stats` creates a builder and calls `finish` internally, so no authority can be joined after coordinates without a two-stage draft. |
| `crates/arcweft-runtime-plan/src/semantic_facts.rs` | Compiler-projected semantic facts are the existing dependency-safe bridge; runtime-plan does not need to import sema. Several current host/effect facts still contain String/raw HIR identities. |
| `crates/arcweft-compiler/src/project.rs` | Compiler builds the validated View product before lowering the RuntimePlan and already depends on runtime-plan, bundle, and View; it is the legal two-product orchestrator. |
| `crates/arcweft-compiler/src/view.rs` | Current View lowering has no retained Match operation/catalog and publishes one `CompiledViewProduct` around `ValidatedViewProduct`. |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | `ValidatedViewProgramResource` owns actual program/revision/source-set data. Bundle depends on core and View, but not compiler/sema. |
| crate manifests and `docs/00-overview/crate-map.md` | Core -> View/bundle and bundle -> compiler/sema edges are forbidden by the maintained layer direction. |

## Predecessor evidence

- `docs/implementation/2026-08-22-runtime-convergence-cut-3-match-producer-admission-safe-subset.md`
  explicitly states that Cut 3 publishes no View site, admission, catalog, or
  product connection.
- request `.1.2` owns complete Match transcripts and missing View body paths.
- request `.1.4` requires `.1.2` and owns retained View operations, slots,
  captures, site, admission, catalog, and executable product connection.
- `docs/implementation/2026-08-22-runtime-convergence-cut-4-core-identity-catalog.md`
  records that Cut 4 landed only the standalone core substrate.

## Returned package check

Archive:
`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip`

- SHA-256:
  `9A201483978DBBF060145E31638364FFFEAB64836589139F69124103CEC1BEDE`
- byte length: `86257`
- archive validator: passed
- negative self-tests: 14 passed
- repository-aware validator at current main: failed because the package pins
  `515bb071437c3af053f1560c3119906dc8002efc`, not current main

Package integrity does not cure its result-changing source/API assumptions.
