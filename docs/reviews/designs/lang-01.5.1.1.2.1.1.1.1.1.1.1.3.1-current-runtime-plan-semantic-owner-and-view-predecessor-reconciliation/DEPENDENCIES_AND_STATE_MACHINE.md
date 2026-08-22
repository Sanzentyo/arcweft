# Dependencies and state machines

## 1. Crate dependency direction

The final graph is:

```text
arcweft-lang-hir
  -> arcweft-lang-sema
  -> arcweft-compiler

arcweft-core <- arcweft-runtime-plan <- arcweft-compiler
arcweft-core <- arcweft-bundle       <- arcweft-compiler
arcweft-view <- arcweft-bundle       <- arcweft-compiler
arcweft-view                         <- arcweft-compiler
```

Required direct-edge rules:

| From | To | Result |
|---|---|---|
| core | View or bundle | forbidden |
| runtime-plan | compiler, bundle, or View | forbidden |
| bundle | compiler or sema | forbidden |
| compiler | sema, runtime-plan, bundle, View | existing legal orchestration |
| bundle | core and View | existing legal product join |

The `.1.2` product reaches runtime-plan only through the existing compiler-
constructed `RuntimePlanSemanticFactInput`; runtime-plan does not add a sema
dependency. The `.1.4` compiler-local catalog never crosses into bundle.
Compiler projects one joined input containing only types owned by core and the
accepted shared View product owner.

If `.1.4` returns a site/admission type that only compiler or sema can name,
bundle cannot consume it and finalization remains blocked. Adding a bundle
dependency back to either crate is not an option.

## 2. Compile-time state machine

```text
C0 AcceptedProject
   exact HIR generation + accepted .1.2 FinalSemanticAnalysis

C1 CompiledViewBase
   .1.4 retained operations/slots/captures and compiler-local catalog complete
   base ValidatedViewProduct exists; no task binding exists

C2 RuntimePlanDraft
   runtime-plan owns one RuntimePlanBuilder and non-plan outputs
   task pushes issued Arc-owner coordinates
   each View task has one .1.4 stable join key
   no RuntimePlan or task digest exists

C3 JoinedViewBindingCandidate
   compiler joins C2 coordinates/keys to its C1 local catalog
   compiler constructs bundle input using actual shared types
   compiler-local row itself never crosses the call

C4 ValidatedViewAuthority
   bundle validates current program/revision/source set
   exact View-coordinate coverage, order, site and admission pass
   ordinal-sorted bindings implement the sole core View protocol

C5 CorePreflight
   draft is consumed
   count/arithmetic/structural/coordinate/family rules pass
   if View rows exist, authority presence and seal-scope preflight pass

C6 SemanticChildren
   final row visitors and task child digests complete under one work meter

C7 ExecutableDigest
   fixed table transcript E completes with coordinate references only

C8 TaskRowsSealed
   core seals non-View rows
   C4 seals View rows through one-use core requests
   partial rows remain private

C9 FinalChecks
   optional expected bytes compare
   one global duplicate index
   all executable task references resolve to final indexes

C10 PublishedProject
   one RuntimePlanLowerReport and the exact bound CompiledViewProduct are moved
   into CompiledProject in the same compiler success transaction
```

Any failure in C2-C9 drops the draft/binding candidate/partial rows. The
previous compiler session product remains current under its existing
transaction. There is no public partial `RuntimePlan` and no mutation of an
already published `CompiledViewProduct`.

The current compiler order already constructs the View product before runtime
plan lowering. Cut 5 changes only the latter portion:

```text
build base View product
  -> lower RuntimePlan draft
  -> enrich/replace base View product with validated task bindings
  -> seal draft using that product
  -> publish both
```

The enriched product is a new value. It does not mutate the `Arc` already held
by another accepted compilation.

## 3. Ordinary-only path

If C2 contains no View row and no View join:

```text
RuntimePlanLowerDraft::finish_without_view
  -> C5 with no authority lookup
  -> C6..C10
```

Passing an authority is unnecessary. Calling `finish_without_view` with a View
row returns `MissingViewTaskPlanAuthority` before semantic row traversal.
Passing a View authority to an ordinary-only draft is allowed only through the
compiler's common internal path and must not query it; the public ordinary API
remains authority-free.

## 4. View seal-scope and row calls

Before child hashing, core constructs a non-Clone borrowed scope containing
only the candidate's ordered View coordinates and calls a preflight method on
the same protocol:

```rust
pub trait ViewTaskPlanAuthority {
    fn validate_view_task_plan(
        &self,
        request: ViewTaskPlanValidation<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;

    fn validate_task_plan_seal_scope(
        &self,
        scope: ViewTaskPlanSealScope<'_>,
    ) -> Result<(), ViewTaskPlanValidationError>;

    fn seal_view_task_plan(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}
```

The scope exposes an exact-size iterator of borrowed coordinates. It exposes no
row fields or digest inputs. The validated upper product checks, in order:

1. current program/revision/source-set freshness;
2. coordinate count;
3. source-order ordinal equality;
4. same Arc candidate owner for every row;
5. missing/extra/duplicate bindings; and
6. its already validated site/admission ownership stamp.

Per-row sealing then looks up the exact coordinate, checks View family/binding,
and consumes the one-use prefix request. This separation makes stale/coverage
errors deterministic before expensive core semantic traversal while retaining
one protocol and one binding owner.

## 5. Private decode state machine

Bundle decode uses the same `RuntimePlanBuilder`; it does not introduce a
second mutable plan algebra.

```text
D0 Envelope
   strict outer magic/version/section bounds and canonical section order

D1 DecodedSeeds
   purpose-built readers decode current core seed types
   task rows are pushed in encoded ordinal order
   returned coordinates are retained in a private Vec
   expected task keys remain Box<[[u8;32]]>

D2 DecodedViewBindings
   each stored coordinate ordinal indexes D1's exact coordinate Vec
   actual View types are decoded by their owning codecs

D3 ValidatedViewAuthority
   same validation as C4; no public resource yet

D4 CommonCoreSeal
   consume the same builder and invoke C5-C9
   expected bytes compare after recomputation and before duplicate collection

D5 AtomicBundlePublication
   one validated bundle receives sealed RuntimePlan and validated View product
```

The core entry point is purpose-named and treats expected values only as
assertions:

```rust
impl RuntimePlanBuilder {
    pub fn finish_decoded(
        self,
        authority: Option<&dyn ViewTaskPlanAuthority>,
        expected: RuntimeTaskPlanExpectedKeys,
        limits: RuntimeTaskPlanSealLimits,
    ) -> Result<RuntimePlan, RuntimePlanBuildError>;
}

pub struct RuntimeTaskPlanExpectedKeys(Box<[[u8; 32]]>);
```

`RuntimeTaskPlanExpectedKeys::try_from_codec_rows` validates cardinality and
limits but never constructs a typed digest. The normal builder cannot store an
expected key field, and the final RuntimePlan does not retain this value.

## 6. Candidate-to-final edge rewrite

All task-producing core seeds carry `RuntimeTaskPlanBuildCoordinate`. During
final materialization, core verifies the Arc issuer and ordinal, then replaces
the candidate edge with `RuntimeTaskPlanIndex(ordinal)`.

The semantic encoder runs over the verified candidate view and writes the
ordinal. The final public graph runs over indexes. This is one builder-owned
conversion, not two authorities and not a post-publication rewrite.

Migration inventory includes at least:

- `FlowOp::HostCall`;
- `FlowOp::Await`;
- `FlowOp::AwaitMany` base and child roles;
- task-producing operations nested in line actions/cancellation/cleanup;
- line child nodes currently carrying `TaskId`, `TaskKey`, name and priority;
- timeout and MakeNeedHandle producers introduced by the accepted convergence
  cuts; and
- `.1.4` retained View subscription operations.

Search/compile errors must enumerate any additional current producer edge. A
task-producing edge that retains an embedded request or live task identity is
a Cut 5 blocker.

## 7. Deterministic error precedence

### Compiler construction

1. `.1.2` generation/path/complete Match error;
2. `.1.4` retained View operation/slot/capture/catalog error;
3. base View product validation;
4. runtime-plan semantic-fact generation mismatch;
5. runtime-plan seed/lowering error in accepted declaration/source order;
6. compiler View-coordinate/catalog join error in coordinate order;
7. bundle View binding validation; and
8. core seal order below.

### Common core seal

1. checked arithmetic;
2. preflight limits in declared limit-field order;
3. existing builder completeness and RuntimePlan structural validation in
   fixed table/source order;
4. task coordinate owner/order and family/binding compatibility;
5. missing View authority for the first View coordinate;
6. View seal-scope freshness, count, order, owner, duplicate, missing, extra,
   site/admission-stamp validation;
7. non-task executable rows in fixed table/source order;
8. task child resolution in task-row order: producer function, request
   template, control/effect contract;
9. executable digest finalization;
10. per-task row sealing in source order;
11. expected-key cardinality/mismatch in source order, when decoding;
12. global duplicate digest at the second source-order row;
13. final task-index/cross-reference validation; and
14. publication.

Unsupported semantic ownership is a typed step-7 or step-8 error. It never
falls back to source spelling.

### Decode envelope

Before the common order: magic/version/bounds, duplicate/unknown/noncanonical
sections, trailing bytes, row/count/tag decoding, coordinate ordinal, expected
key raw length/cardinality, and View owner codec errors. Once D1-D3 exist, the
common core order is identical.

## 8. Limits

Final numeric defaults are frozen only after the predecessor inventories are
known. Their ordering is fixed now:

1. total RuntimePlan table rows;
2. task-plan rows;
3. maximum children per final row;
4. producer-function parameters/captures/endpoints;
5. request-template roles;
6. control/effect atoms;
7. View rows/bindings;
8. transcript bytes; and
9. semantic work.

Every count uses checked `u64`; exact limit passes and one-over fails before
the charged action. Preflight may inspect constant-size row metadata only.
Recursive/child traversal is charged dynamically so an attacker cannot force
an unmetered preflight walk.

The `.1.4` product retains its own slot/capture/operation limits. Passing one
limit domain never bypasses the other.
