# Lang-01.5.1.1.2.1.1.1.1.1.1.1.3 — task-plan semantic child encoder and seal correction

Status: `OPEN_DESIGN_REQUEST`

## Parent, split reason, and precedence

This request is a design-gated child of the accepted runtime Need/View/task
identity corrections. It does not reopen numeric allocation, producer-instance
identity, Match/View admission meaning, scheduler composition, or two-phase
restore.

The accepted `TaskPlanSemanticDigest` transcript is authoritative:

```text
domain = "arcweft.task.plan-semantic.v1\0"
plan_owner_tag:u8
executable_semantic_digest:digest32
producer_function_semantic_digest:digest32
family_tag:u8
task_class_tag:u8
request_template_digest:digest32
control_effect_contract_digest:digest32
semantic_binding
```

It explicitly excludes producer contract, producer site, payload type, actual
arguments, generation, policy, launch ordinal, priority/cancellation scope,
debug labels, expected stored digest, and accepted View revision.

Cut 4 correctly retains only the standalone `TaskPlanSemanticDigest` type and a
completed-digest reference in `NeedProducerSpec`. Its earlier provisional
`RuntimeTaskPlan`/table was deleted because it hashed the excluded fields and
lacked the required child authorities. The final row/table must not be
published before Cut 3 establishes the real View program/site/admission owners.

## Fixed dependency decision

`arcweft-core` must not depend on `arcweft-view` and must not copy
`ViewProgramId`, `ViewMatchSiteId`, or `CheckedViewMatchAdmissionDigest` into a
raw projection. The final cross-layer join is:

```text
core RuntimePlan semantic base
  + compiler-local Cut 3 View admission
  -> bundle ValidatedViewProgramResource / ValidatedViewTaskPlanBinding
  -> sealed TaskPlanSemanticDigest
```

`ValidatedViewProgramResource` is the production upper owner that can already
join core and View products. A caller-provided byte sink, extension trait,
public `[u8; 32]` constructor, or core-owned duplicate View identity is
forbidden.

## Decisions required

Return one coherent implementation-ready design that closes all of these.

1. Define the exact typed owners, transcripts, limits, and constructor
   visibility for `RuntimeExecutableSemanticDigest`,
   `ProducerFunctionSemanticDigest`, `TaskRequestTemplateDigest`, and
   `ControlEffectContractDigest`.
2. Define the structured `RuntimePlan` semantic transcript. It must exclude the
   task-plan map key and every task-plan self digest so
   `TaskPlanSemanticDigest -> executable digest -> task-plan table` cannot form
   a cycle. State exactly which executable rows and source-order roles it does
   include.
3. Define the final core `RuntimeTaskPlan` static fields and closed semantic
   binding marker/payload for Ordinary, View, AwaitManyBase, AwaitManyChild,
   Timeout, and Line without storing a self digest.
4. Define a field-private, nonserialized, non-`Clone` borrowed digest-base
   request minted only by the owning `RuntimePlan` semantic encoder. Raw
   construction outside the owner must be impossible.
5. Extend the existing core `ViewTaskPlanAuthority` as the sole cross-layer
   protocol. Its request carries an opaque core-minted base and plan-build
   coordinate; it must not expose a general byte sink or accept caller digest
   fields.
6. Define `ValidatedViewTaskPlanBinding` inside the validated bundle/View
   product. It owns the real `ViewProgramId`, stable site, and exact checked View
   admission and computes the accepted View binding transcript while excluding
   accepted revision.
7. Define builder finish, compiler projection, bundle validation, private
   decoded-image expected-key verification, and runtime publication order. All
   rows must be recomputed and sealed before a public `RuntimePlan` or lookup
   table exists.
8. Define duplicate semantics across Ordinary/View/timeout/line families,
   missing binding behavior, expected-key mismatch, stale View authority, work
   limits, and deterministic first-error precedence.
9. Provide the deletion-driven compile-clean sequence that publishes the final
   row/table only inside the Cut 5 atomic switch and deletes every provisional
   caller-digest, self-digest, or parallel table path.

Every Arcweft-owned version marker remains exactly `1`.

## Required Rust-shaped boundary

Names may follow legitimate existing owners, but the return must provide
equivalent roles for:

```rust
// arcweft-core
pub struct RuntimeTaskPlan {
    producer_function: RuntimeFunctionSiteId,
    family: NeedProducerFamily,
    class: TaskClass,
    request_template: RuntimeHostTaskRequestTemplate,
    control_effect: RuntimeControlEffectContractId,
    binding: RuntimeTaskSemanticBinding,
}

pub enum RuntimeTaskSemanticBinding {
    Ordinary,
    View, // marker only; no copied upper identity
    AwaitManyBase,
    AwaitManyChild,
    Timeout { contract: NeedTimeoutContractDigest },
    Line { plan: LinePlanSemanticDigest },
}

pub struct RuntimeTaskPlanDigestBase<'a> { /* private fields */ }
pub struct ViewTaskPlanDigestRequest<'a> { /* private fields */ }

pub trait ViewTaskPlanAuthority {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}
```

The return must prove how `RuntimePlanBuilder::finish` and private decode
validation obtain the same authority without making ordinary non-View plans
depend on a View registry.

## Consumers to inventory

- core RuntimePlan types/builders, task identities, AWBC plan owners, and line
  task plan owners;
- sema/compiler producer function, endpoint, child-role, control/effect, and
  Cut 3 View admission products;
- runtime-plan lowering and executable semantic encoder;
- bundle validated View resources and runtime resource codecs;
- private decoded RuntimePlan/task-plan images and expected digest keys;
- Need producer instance construction, snapshot verification, generated
  schemas/fixtures, maintained docs, and structural gates.

## Required tests

- mutation of each of the seven included transcript roles changes the digest;
- mutation of every explicitly excluded producer/site/payload/value/runtime
  field leaves the plan digest unchanged while changing its legitimate owner;
- View accepted revision changes do not change plan digest;
- executable digest excludes task-plan keys/self digest and terminates without
  recursion;
- ordinary/View/AwaitMany/timeout/line bindings, duplicate cross-family digest,
  missing/mismatched View binding, stale authority, expected-key tamper, and
  exact-limit/one-over cases;
- builder and codec publish nothing before all rows are sealed;
- raw digest-base, raw View projection, self-digest, and caller-digest compile
  failures;
- Cargo metadata proof that `arcweft-core` has no `arcweft-view` dependency; and
- focused core/compiler/runtime-plan/bundle/codec tests, deterministic artifact
  comparison, Clippy record, and structural gate.

## Non-goals

- no production patch in the returned archive;
- no scheduler/restore redesign, numeric allocation change, or producer-instance
  transcript change;
- no core copy of View identity, caller byte sink, public raw digest/base
  constructor, extension helper, self digest, expected digest field, or parallel
  task table;
- no source spelling, HIR ID, debug text, whole-catalog digest, or generic
  Serde transcript; and
- no V2 type, version bump, legacy reader, fallback, compatibility alias, or
  optional old field.

## Required returned archive

Return exactly:

`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip`

The archive must contain the complete final contract, Rust-shaped schemas,
owner/consumer and dependency matrices, exact child and executable transcripts,
cycle proof, builder/decode seal state machines, compile-clean deletion order,
exhaustive test matrix, repository-aware validator and negative self-tests,
manifest, source inventory, `FINAL_STATUS`, and `OPEN_QUESTIONS`. It may claim
`READY_FOR_IMPLEMENTATION` only when every decision above is closed and
`OPEN_QUESTIONS` is exactly `none`.
