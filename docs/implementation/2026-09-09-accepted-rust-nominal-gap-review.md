# Accepted Rust nominal implementation gap review — 2026-09-09

Inspected source: `8dfe60d18640bb2c5785b112bad6dcd874d216d4` on the existing
`main`, already pushed to `origin/main`. Rust sources were unchanged during
this inspection; the checkpoint validation note was dirty. This is the early
nominal investigation required by the
[convergence plan](2026-09-08-convergence-goal-plan.md), not completion of C1-C6.

## Authority and conclusion

The authority reviewed is the
[repository-local accepted design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/README.md),
including its final design, schemas, dependency matrix, wire/restore contract,
cuts and test matrix, decision register, source evidence, and final status.
The two returned archives were rejected; their proposed carrier and invented
runtime crate are not implementation authority.

The accepted C1-C6 work remains necessary. Current project-nominal execution,
ordered environment enum payloads, and checked structural record predicates
are useful existing parts, but do not provide the accepted Rust ADT catalog
join or program-bound structural restore. Reuse those parts while completing
the existing core, sema, plan, AWBC, and restore owners. The old design's
source-spelling gates are superseded by the current Rust workspace policy;
source searches below are inspection aids, not executable acceptance tests.

## Current boundaries compared with C1-C6

| Cut | Inspected current behavior | Remaining accepted obligation |
| --- | --- | --- |
| C1 — core schema and identity | `RuntimeTypeSchema` still models tree-shaped persistence schemas and `Named(String)`. `RuntimeCheckedType::Record` already validates contiguous field IDs and duplicate names. Nominal record layouts retain required string names; variant identity retains nominal and semantic IDs. | Complete the bounded typed nominal-reference graph, reachable recursive layout identity, all four record shapes, and layout-bearing variant admission. Reconcile existing checked-record behavior instead of adding another record predicate. |
| C2 — exact accepted Rust join | Adapter Rust metadata publication still creates `AcceptedOpaqueRuntimeCarrier` with `ExternalOpaqueProducer::Rust`. Accepted metadata retains package/item-path/source but drops its publication item. Environment enum record payloads already preserve declaration order and reject duplicate names. | Publish the distinct Rust ADT semantic role, retain exact publication identity, prove the atomic bijection with metadata, and build a generation-bound reachable schema graph. Preserve the ordered payload owner and complete its required empty-name validation. |
| C3 — compiler/plan admission | `RuntimeNominalProjectionContext` and `RuntimeResolvedNominal` still require project declaration ownership; the generic plan row remains project-specific. Existing nominal record and variant domains support project execution. | Generalize provenance through the one existing nominal projection and admit the accepted graph as one verified type/domain batch. Complete recursive value admission and private checked variant construction before exposing accepted Rust structural execution. |
| C4 — AWBC | `AwbcRecordField` has a required name and type, without an explicit field ID or record shape. Nominal variant identity has only its public ID. Record and variant constants retain duplicate field/case names. | Evolve the existing version-1 rows and their codecs/consumers in place; use exact accepted type rows as the name/layout authority, and replace the duplicated constant fields. Existing project nominal AWBC tests do not prove this matrix. |
| C5 — restore | `AwbcRuntimeValueSnapshot::into_runtime_value` recursively reconstructs values without an `AwbcProgram`. `nominal_into_live` calls public `RuntimeNominalRecordValue::new` using snapshot-provided type/layout/fields. | Resolve the current program's exact descriptors before constructing private candidate values, including every recursive capture/iterator/reduction/Agent/task site. Delete the context-free conversions and unchecked constructor as part of that complete migration. |
| C6 — ownership | `classify_accepted_nominal` admits an exact catalog row through its opaque carrier; all non-opaque semantics take `MissingRuntimeSnapshotOwner`. It has no accepted Rust structural graph proof. | After C1-C5, admit exact records/variants with the current ownership certificate and validate their live values before canonical digest exposure. Preserve legitimate opaque behavior and remove only the replaced structural rejection path. |

Concrete source owners inspected:

- [core schema](../../crates/arcweft-core/src/entry/schema.rs),
  [checked predicates and variant identity](../../crates/arcweft-core/src/pattern.rs),
  [nominal records](../../crates/arcweft-core/src/value/nominal_record.rs), and
  [AWBC snapshot conversions](../../crates/arcweft-core/src/value/awbc_save.rs);
- [nominal catalog](../../crates/arcweft-lang-sema/src/env/nominal.rs),
  [ordered enum payloads](../../crates/arcweft-lang-sema/src/env/enums.rs),
  [Rust metadata](../../crates/arcweft-lang-sema/src/env/rust_metadata.rs),
  [adapter publication](../../crates/arcweft-adapter-sema/src/registration/input.rs),
  [nominal projection](../../crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs), and
  [ownership classification](../../crates/arcweft-lang-sema/src/ownership.rs);
- [plan type projection](../../crates/arcweft-core/src/plan/type_kind.rs),
  [runtime semantic facts](../../crates/arcweft-runtime-plan/src/semantic_facts.rs), and
  [AWBC types/constants](../../crates/arcweft-core/src/awbc/schema.rs).

## Validation, order, and non-goals

Performed: accepted-contract and current-source inspection. This review adds
no Rust, manifest, codec, test, or generated production artifact. No new nominal
acceptance test was run, and no C1-C6 completion credit is claimed. Concurrent
cold validation of the unchanged source is recorded separately in the
[checkpoint note](2026-09-09-convergence-checkpoint.md).

Keep callable/source/effect convergence as the active implementation priority.
Nominal implementation must finish before scheduler integration, using C1-C6
as the accepted dependency order. This review closes the early gap inspection,
not the implementation. It does not authorize a second nominal catalog,
weakened restore admission, compatibility reader, contract version change, or
early structural ownership success. No design amendment was selected here.
