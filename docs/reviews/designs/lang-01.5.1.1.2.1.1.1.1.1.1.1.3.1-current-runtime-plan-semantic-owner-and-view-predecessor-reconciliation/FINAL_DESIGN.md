# Final correction direction

## 1. Status and precedence

This document is final for the core-independent correction direction and
blocked for the predecessor-dependent transcript/product boundary. It is not a
production implementation contract yet.

Current source at
`f43ca943d84f9a6a6da17605947a3d30c518a5a8`, maintained documentation, the
landed Cut 3/Cut 4 implementation notes, and later accepted `.1.2`/`.1.4`
returns outrank the returned `.1.3` archive.

The returned archive is right about two points that remain retained:

1. the completed task-plan digest must not occur inside its own executable
   input graph; and
2. no public `RuntimePlan` or task table may exist before all rows have been
   recomputed, View-sealed when applicable, expected-key checked, and globally
   deduplicated.

Its claims about existing row semantic visitors, Cut 3 View products, and
cross-crate reachability are rejected.

## 2. Mandatory predecessor graph

The dependency graph is strict:

```text
.1.2 accepted design
  -> .1.2 landed complete transcript/path implementation
  -> .1.4 design dispatch
  -> .1.4 accepted design
  -> .1.4 landed retained View operation/product implementation
  -> .1.3.1 exact type/transcript finalization
  -> task-plan integration implementation
  -> Cut 5 atomic runtime/bundle/persistence switch
```

The `.1.3.1` folder may be updated after those intakes. Until then:

- no public task row/table;
- no `ViewMatchSiteId` or admission stand-in;
- no bundle binding section;
- no provisional semantic digest bytes or fixture;
- no `RuntimePlanLowerDraft` public API; and
- no replacement of the existing live View validation method.

Private experiments are also disallowed when they create a second row model or
a transcript that consumers can begin depending on. Ordinary Rust refactors
that merely expose an already-owned typed getter remain independently
reviewable, but receive no task-plan completion credit.

## 3. Current owner inventory and corrections

The executable semantic encoder may hash only data physically owned by the
final immutable `RuntimePlan` graph or a completed child type derived from that
graph. It may not reach back into HIR/sema, source text, compiler caches, or a
parallel catalog.

| Current owner | Current final fields | Correction decision |
|---|---|---|
| `RuntimePlanTypeDeclaration` | `RuntimeSemanticTypeId`, typed projection | Retain as the type-row authority. Encode its accepted semantic identity and closed projection graph; do not copy sema layouts beside it. |
| `RuntimeLocalDeclaration` | type only | Encode only its row ordinal and type reference. The returned package's storage/mutability/initialization/owner atoms are deleted because current execution has no such final semantics. If later execution genuinely needs one, it must first become a field of this owner in a separately justified design. |
| `RuntimeNominalRecordDomain` | owner type; fields `{ name, type }` | `.1.2` must supply accepted field identity. Cut 5 replaces each name-only field row with one field containing runtime lookup label, accepted semantic identity, ordinal identity, and type. The runtime label remains for lookup; hashing uses the accepted identity, not spelling. No side catalog remains. |
| `RuntimeVariantDomain` | owner type, nominal identity; cases `{ name, payload }` | Same rule as records. `.1.2` accepted case identity becomes a field of the sole final case row. Hashing never treats the name as identity. |
| `RuntimeFunctionSite` | params, captures, body | Insufficient for `ProducerFunctionSemanticDigest`. Same-cut final row must add accepted function semantic identity, explicit role, parameter passing modes/result type, capture modes/stable capture coordinates, and producer endpoints supplied through checked lowering. No digest can be implemented before those seed fields are constructible. |
| `RuntimeDialogueContentPlan` | line ID, value sites, marks/labels, optional line group | Encode typed line identity, source-order value roles/function references, mark ordinal IDs, and line-group reference. Raw mark label is runtime lookup data and is not independently hashed; its accepted identity/path must come from the owner used by dialogue lowering. |
| `RuntimeEntrySpec` | typed entry ID/kind, `EntryBindingIdentity`, target, roles | `EntryBindingIdentity` is the accepted semantic owner. Encode typed kind, binding, target and role algebra. Do not hash display labels or generic Serde. |
| callable/flow executable rows | accepted typed IDs/contracts/targets already in core | Encode exhaustive closed fields after verifying each identity accessor is source-independent. Any remaining String/usize lookup identity blocks that row rather than falling back to spelling. |
| `RuntimeFlow` / `FlowOp` | typed flow ID/params and closed op tree; task operations currently embed live/request data | Encode the closed op tree and source-order children. Cut 5 replaces task-producing payloads with sealed task-plan indexes; the candidate transcript writes their owner-checked coordinate ordinals. |
| `RuntimePureHelper` | numeric local ID, name, ABI, body, origin | Numeric helper ID and name are not accepted semantic identity. The final row needs a checked function semantic identity from lowering; otherwise this table remains typed fail-closed for executable digest construction. |
| `RuntimeTraitMethod` | raw usize/String identity fields, receiver, ABI, body | Raw impl/trait/witness indexes and names cannot be hashed. The final row replaces them with the accepted callable/method identity already required by semantic lowering; no adapter may digest the current debug-shaped identity. |
| `LineTaskGroup` | captures, topology, current child `TaskId`/`TaskKey`/name/priority, actions, cleanup | Live task IDs/keys/name/priority are excluded. The final static graph references structured task-plan indexes and retains topology, triggers, join/cancel and cleanup policies. Line actions reuse the same typed FlowOp visitor. |
| `StreamPlan` | typed ID/types and closed op tree | Encode typed stream identity, type references, and source-order closed ops. Match rows consume `.1.2` complete pattern/expression semantics; unsupported identities fail closed. |
| structured task rows | absent | Added only in Cut 5 through the core builder seed and common seal defined in `SCHEMAS.md`. |

This table deliberately removes invented atoms rather than adding fields solely
to reproduce the returned package. Any new field above is required because an
already accepted semantic product must survive into the sole runtime owner.

## 4. Producer-function authority

The final `ProducerFunctionSemanticDigest` is derived from the resolved
`RuntimeFunctionSite`, never from a caller digest. Its final transcript may
contain only fields present on that final row:

```text
accepted function semantic identity
function role
parameters in declaration order: ordinal, local/type, passing mode
captures in accepted capture order: ordinal, stable capture identity,
  local/type, capture mode
result type
typed body semantic digest
producer endpoints in checked source order: ordinal, kind, stable child role
```

Construction path:

1. `.1.2` publishes the accepted declaration/body path used by checked
   functions and bodies.
2. Existing compiler semantic projection adds that accepted information to
   `RuntimePlanSemanticFactInput`; runtime-plan does not import sema.
3. `RuntimeFunctionSiteDeclarationSeed` is evolved in place to carry the
   accepted identity, role, parameter/result modes, and endpoints.
4. The builder resolves every local/type/child path under its existing issuer
   and materializes one final `RuntimeFunctionSite`.
5. The encoder traverses that final row only.

The existing params/captures/body-only seed path is deleted in the same atomic
cut. There is no compatibility constructor.

Exactly which `.1.2` identity/path types occupy these fields remains an open
predecessor result. Therefore the byte tags are not final in this revision.

## 5. Request-template authority

The current `HostTaskRequestTemplate { capability, operation, args }` and
`RuntimeHostCallTarget` duplicate String authority. The final model has one
task request template on `RuntimeTaskPlan` and one FlowOp table reference.

For Host tasks, compiler/runtime-plan resolve the accepted operation against
the Cut 4 `HostOperationCatalog` before the task row is accepted. The final
template stores:

- one `HostOperationIdentity` for runtime catalog lookup;
- one route-independent `HostOperationRequestSemanticDigest` issued by that
  exact catalog row;
- positional/named/spread argument templates in checked call order;
- accepted named-argument identity supplied by the catalog contract, never the
  raw name as semantic identity; and
- each typed expression child through the final RuntimeExpr graph.

The catalog remains the operation/request-shape authority. Its
`HostOperationPlanAdmission` atomically supplies runtime lookup identity plus
the route-independent request semantic digest, so callers cannot mismatch
them. The child digest commits operation family, capability, and
`HostTaskRequestContract`; it excludes route, restart, and cancellation. Those
remain on the live catalog/producer validation owner rather than leaking into
plan identity through a custom operation's whole-catalog digest.

Await, AwaitMany, timeout, line, View, and MakeNeedHandle use closed runtime
template variants rather than pretending every request is Host-shaped. Their
exact View child roles are gated on `.1.4`. Actual evaluated values remain in
`RuntimeValueDigest`/producer-instance identity and are excluded here.

## 6. Control/effect authority

There is no current `RuntimeControlEffectContractId` or table. The selected
model does not invent one. `RuntimeTaskPlan` owns one closed inline
`RuntimeControlEffectContract`; its inherent encoder produces
`ControlEffectContractDigest`.

The contract carries only checked static execution control already required by
current producers:

- Host call mode and deterministic/effect class;
- Await suspension and pending-observer role;
- AwaitMany base/child cardinality and ordering;
- timeout race and terminal behavior;
- line trigger/join/cancel/cleanup behavior;
- MakeNeedHandle nonlaunch construction behavior; and
- retained View subscription/invalidation/cancellation behavior supplied by
  `.1.4`.

`TaskOutcomeContract` remains its existing typed owner and is referenced rather
than redefined. Priority, live cancellation scope, route, retry/backoff,
generation, launch ordinal, and debug label remain excluded.

At `.1.3.1` finalization, each closed contract variant must cite the exact
current or landed predecessor field that constructs it. A variant without such
an input is removed, not accepted with a default.

The timeout and line binding digests likewise require typed owners. Cut 5
introduces `RuntimeNeedTimeoutContract` from the accepted timeout semantics and
derives `NeedTimeoutContractDigest` inherently. It derives
`LinePlanSemanticDigest` from the final `LineTaskGroup` after live child IDs,
keys, names, and priority have been replaced by static task-plan references.
Neither digest is caller input. These children depend only on typed structure
and candidate coordinate ordinals, so they remain below `E` and do not create a
task-plan self cycle.

## 7. Record and variant authority

`.1.2` owns accepted record-field/case identity. The only legal consumption is
an atomic enlargement of the current core domain rows during Cut 5:

```text
sema accepted row
  -> compiler RuntimePlanSemanticFactInput
  -> runtime-plan domain seed
  -> core domain-table admission
  -> final RuntimeNominalRecordDomainField / RuntimeVariantCase
```

The old `(String, type)` seed and final row are deleted once every consumer
uses the enriched row. Pattern/expression/codec/AWBC consumers resolve the same
ordinal and accepted identity; no sema catalog is retained beside the plan.

## 8. View bridge and trust

`.1.4` must first publish an executable retained Match operation and its
compiler-local catalog. Only then may the compiler join:

```text
RuntimeViewTaskPlanJoin(core coordinate + .1.4 stable key)
  + compiler-local .1.4 catalog row
  + current ValidatedViewProgramResource
  -> bundle ViewTaskPlanBindingInput using shared actual types
  -> ValidatedViewTaskPlanBinding
```

Bundle never accepts `CompilerLocalViewMatchCatalogRow`; compiler performs the
join because it already depends on both sides. Bundle validates current
program/revision/source set, exact View-coordinate coverage, stable site and
admission, and stores one ordinal-sorted binding slice.

The upper product is an explicit semantic trust root. Core cannot independently
validate View facts without an illegal dependency. Core nevertheless prevents
authority misuse of its own domain:

- core constructs and privately retains the preseeded prefix hasher;
- the request is one-use and non-Clone;
- no base-child digest getters are exposed;
- one finalization operation appends the complete closed View payload in fixed
  order and consumes the request; and
- no authority can construct `TaskPlanSemanticDigest` otherwise.

The exact borrowed `.1.4` types in this finalizer are still open. Raw strings,
byte arrays, a hasher, or a generic writer are not acceptable substitutes.

The existing live `validate_view_task_plan` protocol remains required by
`TaskValidationAuthority` and snapshot/restore. Semantic sealing is added to
that same trait; it does not replace live validation.

## 9. Cycle and publication result

The corrected dependency graph remains acyclic:

```text
final non-task RuntimePlan rows
  -> row semantic digests and task child digests

row digests + source-order task base rows + coordinate references
  -> RuntimeExecutableSemanticDigest E

E + one task row's child digests + closed binding
  -> TaskPlanSemanticDigest P[i]

all P[i]
  -> expected comparison
  -> one global duplicate index
  -> final task references resolved to RuntimeTaskPlanIndex
  -> one public RuntimePlan
```

No `P[i]`, expected key, table key, or final index is an input to `E`.
Candidate coordinates are checked against one Arc issuer and encoded only by
ordinal. The issuer is absent from bytes and from the public plan.

## 10. Readiness rule

This design becomes implementation-ready only when:

1. `.1.2` is accepted and landed;
2. `.1.4` is then dispatched, accepted, and landed;
3. every open item in `OPEN_QUESTIONS.md` is replaced by the exact accepted
   type/API/transcript;
4. the executable owner matrix contains no String/raw-ID identity fallback and
   no unsupported success row;
5. the exact transcript tags and failure variants are frozen; and
6. `OPEN_QUESTIONS.md` is exactly `none` and `FINAL_STATUS.md` is changed to
   `READY_FOR_IMPLEMENTATION` in that later review cut.
