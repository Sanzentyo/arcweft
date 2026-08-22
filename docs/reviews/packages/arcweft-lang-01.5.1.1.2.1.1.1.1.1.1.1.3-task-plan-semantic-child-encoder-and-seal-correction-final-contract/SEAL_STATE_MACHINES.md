# Builder, bundle, decode, and publication state machines

## 1. Core builder state machine

```text
B0 MutableBuilder
  push static rows; each task push returns owner-bound build coordinate
  no digest exists

B1 ConsumedCandidate
  finish consumes builder into private UnsealedRuntimePlanImage
  no public RuntimePlan exists

B2 StructurallyVerified
  all dense references, family/binding matches, coordinate ownership/order,
  line/timeout payloads, and existing RuntimePlan invariants pass

B3 ChildrenResolved
  each task row in source order has F_i, Q_i, C_i
  no final task digest exists

B4 ExecutableSealed
  all fifteen executable tables produce E
  no final task digest/table exists

B5 RowsSealing
  one opaque non-Clone base is minted per source-order row
  core seals non-View rows
  validated upper authority seals View rows
  partial results stay in a private Vec

B6 ExpectedVerified
  decode path only: every expected key equals recomputed bytes
  builder path has no expected keys and passes directly

B7 Unique
  global digest-to-first-coordinate index accepts all rows

B8 FinalCrossReferencesVerified
  Need producer templates, line/AwaitMany references, AWBC joins that consume
  structured keys, and final table indexes agree

B9 Published
  RuntimeTaskPlanTable and public RuntimePlan are moved into existence together
```

Any error in `B1..B8` drops the private candidate, child digests, partial sealed
rows, and optional expected keys. It returns no plan, table, digest iterator,
lookup handle, or callback.

### Entry APIs

```rust
RuntimePlanBuilder::finish()
  -> common finish_inner(None, default_limits)

RuntimePlanBuilder::finish_with_view_task_plan_authority(authority, limits)
  -> common finish_inner(Some(authority), limits)
```

The common path is not duplicated. `finish()` succeeds only when the candidate
has no View marker. It does not allocate, discover, or query a View registry.

## 2. Compiler/lowering and bundle join state machine

```text
C0 FinalSemanticAnalysis
  generic Match and checked producer/effect facts are complete

C1 CompilerLocalCut3
  CompilerLocalViewMatchCatalogRow owns:
    current compiler lookup evidence
    actual ViewProgramId
    accepted revision
    stable ViewMatchSiteId
    exact CheckedViewMatchAdmissionDigest
  this row is not serialized

C2 RuntimePlanLowering
  static RuntimeTaskPlan rows are pushed in source order
  lowering records returned RuntimeTaskPlanBuildCoordinate values
  View rows contain only RuntimeTaskSemanticBinding::View

C3 PrivateBundleCandidate
  joins each View coordinate to its exact Cut 3 row and current View resource
  no task digest supplied by compiler

C4 ValidatedViewBindings
  validates program, current revision, site ownership, admission equality,
  canonical coordinate order, exact coverage, no extra/duplicate row, limits

C5 ValidatedViewProgramResource
  owns actual ValidatedViewTaskPlanBinding rows and implements the sole core
  ViewTaskPlanAuthority protocol

C6 CoreSeal
  C5 is supplied to RuntimePlanBuilder common finish

C7 CompleteValidatedBundle
  sealed RuntimePlan and validated View resource are assembled atomically
```

A compiler-local row cannot be serialized because it has no resource codec.
The persistent resource stores the actual View types in the legitimate upper
layer; it does not create raw core projection newtypes.

## 3. Private decode state machine

```text
D0 EnvelopeBytes
  strict version-one bundle envelope

D1 PrivateSectionImages
  purpose-built decoders reject unknown/duplicate fields, trailing bytes,
  noncanonical lengths/tags/order, and arithmetic overflow

D2 DecodedRuntimePlanImage
  static rows and private ExpectedTaskPlanKey bytes are retained
  source-order coordinate resolver mints owner-bound tokens only in range

D3 DecodedViewImage
  stored coordinate ordinal is resolved through D2, then actual ViewProgramId,
  revision, site, and admission are decoded by their legitimate upper owners

D4 ValidatedViewProgramResource
  complete View/source product and task bindings validated; still unpublished

D5 CommonCoreSeal
  D2 calls the same RuntimePlanSemanticEncoder/seal_task_plans as the builder,
  passing D4 only if View rows exist

D6 ExpectedKeyComparison
  recomputed key bytes compared source order; no expected byte is hashed or
  wrapped into TaskPlanSemanticDigest

D7 UniqueAndCrossReferenced
  global duplicate and complete bundle joins pass

D8 AtomicPublication
  one ValidatedRuntimeBundle containing RuntimePlan + optional View resource
```

At no state before `D8` can runtime lookup, Need producer construction,
snapshot authority, or resource access observe the candidate.

## 4. View authority call state machine

For each View request:

```text
V0 OpaqueRequest
  request owns one non-Clone core base and coordinate

V1 Freshness
  validated resource's current program/revision/source-set stamp matches the
  outer accepted bundle authority

V2 Lookup
  exact owner-bound coordinate resolves one binding

V3 BaseAdmission
  coordinate owner/ordinal equal
  binding marker is View
  family is ViewMatchSubscription
  binding program equals validated owner program
  site/admission remain the exact validated Cut 3 join

V4 Transcript
  local BLAKE3 hasher receives accepted core prefix from typed getters,
  then binding tag 1, actual program string, actual site, actual admission
  accepted revision is not written

V5 OneUseFinish
  consuming request wraps the finalized owner transcript as
  TaskPlanSemanticDigest
```

A failure in `V1..V4` does not call the finalizer and does not return a digest.
The request is consumed by the failed call and cannot be retried with different
fields.

## 5. Runtime consumer state

After publication:

- `RuntimePlan::task_plans()` exposes one immutable table;
- `NeedProducerSpec` receives an already completed table key/reference;
- Need producer instance construction never recomputes plan semantics;
- snapshot decode resolves stored bytes against the sealed table;
- accepted View revision remains on the validated resource/replacement owner;
- AWBC and line plan owners retain their existing owner-tag encoders; and
- no runtime scheduler path owns a parallel structured task-plan table.

## 6. Publication atomicity proof

The public constructors for `RuntimePlan` and `ValidatedRuntimeBundle` accept
only fully sealed private images. No `Default`, public field literal, public
partial builder result, or deserializer can create either final object.

The final move has no fallible action afterward. All allocations required by
the task-plan rows and index are performed before the final object is returned.
A production implementation may use `try_reserve` during private staging; an
allocation failure remains prepublication.
