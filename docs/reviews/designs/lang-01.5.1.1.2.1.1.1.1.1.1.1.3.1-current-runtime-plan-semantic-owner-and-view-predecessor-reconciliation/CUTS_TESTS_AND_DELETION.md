# Cuts, tests, and deletion order

## 1. Dispatch and intake gates

### Gate P1 — `.1.2`

Dispatch/continue `.1.2` first. Intake must establish:

- complete checked expression/pattern/body transcript success domain;
- stable View declaration/body path;
- accepted record-field and variant-case identities;
- accepted function/body identities needed by runtime lowering; and
- typed fail-closed behavior for every still unsupported family.

Then land and validate `.1.2`. A returned design that is not implemented does
not make the task-plan encoder constructible.

### Gate P2 — `.1.4`

Only after P1 is accepted and landed, dispatch `.1.4`. Intake must establish:

- executable retained Match instruction and value-slot/capture completeness;
- stable site and checked View admission actual owners;
- compiler-local catalog key/row;
- the shared type boundary compiler and bundle can both name;
- exact `CompiledViewProduct` and `ValidatedViewProgramResource` consumption;
- replacement/invalidation/cancellation behavior; and
- complete limits and generation/current-revision validation.

Then land and validate `.1.4`. No compiler-only row may be serialized or passed
directly to bundle.

### Gate P3 — `.1.3.1` finalization

Reopen this design at the then-current full Git SHA and:

1. replace every gated semantic reference in `SCHEMAS.md` with the exact
   accepted type;
2. freeze every producer/request/control/View transcript tag and byte atom;
3. close every question in `OPEN_QUESTIONS.md`;
4. reconcile the current RuntimePlan owner matrix again;
5. add exact default limits after the complete inventories are known;
6. prove the compiler-to-bundle bridge has no reverse edge; and
7. change status to ready only if `OPEN_QUESTIONS.md` is exactly `none`.

## 2. Safe pre-Cut-5 work

Before P3, only these independent actions are eligible for separate reviewable
cuts:

- `.1.2` and `.1.4` themselves in their required order;
- deletion of an independently proven obsolete source-string identity route;
- tests/getters on an existing sole owner when required by those predecessors;
- maintained request/intake/design documentation; and
- structured dependency checks.

Do not add a public task row/table, draft, coordinate, expected-key type, View
binding, digest child, codec section, or golden hash early. Even a private
parallel task row is forbidden if it duplicates the builder's final owner.

## 3. Cut 5 compile-clean implementation sequence

After P3 becomes ready, implement one atomic public switch in this order.

1. **Enrich sole semantic owners.** Add accepted `.1.2` identities to the
   existing core record/case/function/helper/method rows through their existing
   compiler -> semantic-facts -> runtime-plan seed -> core builder path.
2. **Delete lookup-only seed routes.** Remove name-only field/case and
   params/captures/body-only function seed constructors after all callers
   compile against enriched seeds.
3. **Add the closed request/control seed algebra.** Evolve the existing host
   request seed into `RuntimeTaskRequestTemplateSeed`; resolve Host operations
   through the Cut 4 catalog; add only constructible closed producer families.
4. **Add task seed admission.** Add `push_runtime_task_plan_seed` and Arc-issued
   coordinates. Migrate runtime-plan lowering to push every task row before
   creating its referring operation seed.
5. **Migrate candidate task edges.** Replace embedded request/live ID/static
   task fields in HostCall/Await/AwaitMany/line/timeout/View/MakeNeedHandle
   candidate operations with owner-bound coordinates.
6. **Introduce the draft.** Replace direct runtime-plan `builder.finish()` with
   `RuntimePlanLowerDraft`; migrate compiler orchestration without exposing a
   RuntimePlan.
7. **Add exact row visitors and child encoders.** Implement only the P3-frozen
   typed transcript. Every enum match is exhaustive; unsupported owners reject.
8. **Add core common sealer.** Preflight, structural validation, View scope,
   child digests, executable digest, non-View/View row seal, expected compare,
   duplicate and final reference resolution share one path.
9. **Extend the View protocol.** Preserve live validation, add seal-scope and
   one-use semantic request methods, and implement both on the accepted
   validated `.1.4` product.
10. **Add the compiler/bundle join.** Compiler maps local catalog rows into
    bundle inputs using only shared actual types; bundle returns a new validated
    product containing ordinal-sorted bindings.
11. **Publish one final core table.** Convert candidate coordinates to final
    indexes, construct `RuntimeTaskPlanTable`, then construct `RuntimePlan` once.
12. **Migrate codec decode/encode.** Purpose-built decoder pushes the same
    seeds, retains expected raw bytes privately, validates View bindings, and
    invokes the common sealer. Encoder accepts only a sealed RuntimePlan.
13. **Migrate Need/task/snapshot consumers.** `NeedProducerSpec` receives only
    a table-issued `TaskPlanSemanticDigest`; restore resolves bytes against the
    sealed table and reuses the existing live View validation method.
14. **Delete every old path below.** Compile errors enumerate consumers; no
    compatibility field or fallback is added.
15. **Regenerate version-one artifacts and validate.** Only then expose the
    final row/table and publish the Cut 5 commit.

## 4. Mandatory deletions

| Delete | Final replacement |
|---|---|
| public `TaskPlanSemanticDigest::from_bytes` and Serde | owner sealer; codec lookup against sealed table |
| raw `NeedTimeoutContractDigest::from_bytes` | inherent digest of typed `RuntimeNeedTimeoutContract` |
| any caller-created `LinePlanSemanticDigest` | inherent digest of the final `LineTaskGroup` |
| any task-plan self/expected digest field | table association/private expected assertion |
| caller-provided task digest | table-issued typed digest |
| package-proposed `RuntimePlanConstructionToken(NonZeroU32)` | existing Arc issuer identity |
| `RuntimeTaskPlan` field-literal construction | builder-owned seed admission |
| current direct runtime-plan `finish()` inside lowerer | unpublished `RuntimePlanLowerDraft` then common seal |
| embedded Host capability/operation/request in `RuntimeHostCallTarget` | referenced final task row with typed catalog operation |
| static/live Need/Task IDs, names and priority in line task nodes | task-plan index plus accepted live correlation/scheduling owners |
| name-only record/case semantic authority | enriched sole core row with `.1.2` accepted identity |
| params/captures/body-only producer-function authority | enriched sole `RuntimeFunctionSite` |
| raw usize/String trait-method identity used semantically | accepted callable/method identity |
| hypothetical `RuntimeControlEffectContractId` side table | inline closed task-row contract |
| bundle parameter of compiler catalog-row type | compiler projection into bundle input of shared types |
| public/caller BLAKE3 hasher or byte sink | core-owned one-use prefix request finalizer |
| replacement of live `validate_view_task_plan` | retain method; add semantic methods to same protocol |
| completed task digest/reference inside executable transcript | owner-checked coordinate ordinal |
| public coordinate/raw numeric constructor or codec | Arc-issued in-memory coordinate; wire stores checked ordinal only |
| generic Serde RuntimePlan/task semantic codec | purpose-built version-one codec |
| old reader, alias, optional legacy field, fallback | none |

## 5. Required tests

### Predecessor consumption

- `.1.2` generation mismatch, unsupported transcript family, field/case
  identity mismatch and View-body path mismatch reject before draft creation.
- `.1.4` slot/capture/operation incompleteness, wrong current generation,
  wrong program/revision, site/admission mismatch and local-catalog mismatch
  reject before bundle binding construction.
- Compile/API tests prove bundle cannot name compiler-local row types.

### Core owner construction

- Every task seed child from a foreign Arc issuer rejects and pushes no row.
- Exact task-row limit passes; one-over rejects before push.
- Final `RuntimeTaskPlan` cannot be constructed by a field literal or public
  constructor.
- Coordinate has no Serde, raw token getter, `Copy`, or raw constructor.
- Two builders with the same ordinal never compare as the same coordinate.

### Current table transcript ownership

- One differential per final field of all fifteen tables.
- Local rows prove no nonexistent mutability/init/owner atom.
- Record/case identity mutation changes digest; runtime label-only mutation is
  either impossible without identity mutation or leaves identity ownership to
  the accepted `.1.2` type as specified at P3.
- Function identity, role, parameter mode/order/type, capture origin/mode/type,
  result, endpoint role/order and body each change the producer digest.
- Raw HIR IDs, allocation order perturbations that do not change owner roles,
  spans, formatting and debug labels do not change digests.
- Current helper/method String/usize identities cannot enter a success
  transcript.

### Request/control

- Host catalog operation and each positional/named/spread role are sensitive;
  evaluated values are excluded and change `RuntimeValueDigest` instead.
- Host operation capability/request-contract changes alter the request child;
  route/restart/cancellation-only changes do not. A catalog admission from one
  catalog cannot be paired with another operation identity.
- Every final request-family and control/effect variant has an exact byte
  golden and one mutation test per payload.
- Unsupported/default/unknown variants reject; no empty fallback transcript.
- Host route/restart/cancellation policy remains on the catalog/producer owner
  and is not silently copied into plan semantics.

### Coordinates, cycle and publication

- Candidate HostCall/Await/AwaitMany/line/timeout/View edges encode the checked
  ordinal, then final RuntimePlan contains only task indexes.
- Mutation of a completed task digest, expected key or lookup-map layout cannot
  affect the executable digest.
- A forbidden graph cycle rejects before any task digest is observable.
- Builder and decode over the same semantic input produce identical rows,
  child digests, executable digest and task keys.
- No public RuntimePlan, report, table iterator, or bound View product is
  observable after any prepublication failure.

### View authority

- Missing authority rejects before semantic traversal when View rows exist.
- Stale, missing, extra, duplicate, reordered and foreign-candidate bindings
  follow the fixed precedence.
- A test authority cannot access or replace the core prefix, obtain its child
  digests, clone/reuse a request, omit/reorder View payload fields, or finalize
  outside an active seal.
- A revision-only change is observed by freshness validation but does not
  change the plan digest when program/site/admission semantics are unchanged.
- Ordinary-only finish does not call any View protocol method.
- Existing live `validate_view_task_plan` tests continue to pass through
  task-validation and restore authorities.

### Codec and duplicates

- Strict version-one decode rejects unknown/duplicate/out-of-order sections,
  tags, noncanonical counts, trailing bytes and overflow.
- Expected key mismatch precedes duplicate; expected bytes are never converted
  to a typed digest.
- Duplicate digest rejects at the second source-order row across all families.
- Encode after decode emits recomputed sealed keys, not untrusted input bytes.

### Limits and structure

- Exact-limit/one-over tests for every P3-frozen count, semantic-work and byte
  meter.
- Preflight does not recursively walk unmetered child graphs.
- Cargo metadata proves no core -> View/bundle, runtime-plan -> compiler/
  bundle/View, or bundle -> compiler/sema edge.
- Trybuild/rustdoc API tests protect private constructors and non-Serde proof
  types.
- Formatter, selected Clippy/full tests, deterministic artifact comparison,
  canonical structure audit and applicable Tier 2 runtime/View tests are
  recorded with exact results.

## 6. Completion rule

The existence of these tests in a matrix is not validation. Cut 5 completes
only when the tests are implemented, the selected repository gates actually
pass, all mandatory deletions are physically complete, generated artifacts are
deterministic, and no open predecessor question remains.
