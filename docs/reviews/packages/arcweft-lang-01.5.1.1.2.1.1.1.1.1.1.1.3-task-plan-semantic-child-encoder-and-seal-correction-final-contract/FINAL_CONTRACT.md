# Final contract

## 1. Scope, precedence, and retained substrate

This contract closes only the child encoders and the seal/publication boundary
for structured `RuntimeTaskPlan` rows. It does not alter numeric allocation,
producer-instance identity, generic Match meaning, retained View admission,
scheduler transactions, two-phase restore, task execution truth tables, or
Need/task correlation.

The accepted task-plan transcript remains exactly:

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

The version is exactly `1`. There is one reader/writer shape and no alternate
version, compatibility path, fallback, translation table, or optional old
field.

## 2. Decision 1 — exact child owners

The four completed child digests are opaque Arcweft-owned newtypes in
`arcweft_core::plan::task_semantic`:

| Type | Sole semantic owner | Construction visibility | Public surface |
|---|---|---|---|
| `RuntimeExecutableSemanticDigest` | `RuntimePlanSemanticEncoder` over a complete private plan candidate | private `from_hasher_output` only | `as_bytes` |
| `ProducerFunctionSemanticDigest` | the same encoder after resolving one actual `RuntimeFunctionSite` | private | `as_bytes` |
| `TaskRequestTemplateDigest` | inherent `RuntimeHostTaskRequestTemplate::semantic_digest` called with the private semantic context | `pub(crate)` method, private context type | `as_bytes` |
| `ControlEffectContractDigest` | inherent `RuntimeControlEffectContract::semantic_digest` after ID resolution | `pub(crate)` method, private context type | `as_bytes` |

They implement only value traits needed for typed comparison and maps. They do
not implement `Serialize`, `Deserialize`, `Default`, a public `ZERO`,
`From<[u8; 32]>`, `TryFrom<[u8; 32]>`, or a public raw constructor. A purpose-
built codec may retain raw expected bytes in a private decoded image; it must
resolve those bytes against recomputed sealed keys rather than construct one of
these types.

All four use BLAKE3, exact NUL-terminated domains, fixed little-endian integer
fields, bounded UTF-8 strings, and explicit closed tags. Generic Serde is never
a transcript. Exact bytes and limits are normative in `TRANSCRIPTS.md`.

## 3. Decision 2 — structured executable transcript

`RuntimeExecutableSemanticDigest` commits the complete execution-relevant
structured `RuntimePlan` candidate in a fixed table order. The table order is:

```text
0  type declarations
1  local declarations
2  nominal record domains
3  variant domains
4  function sites
5  dialogue content plans
6  entries
7  callable executables
8  flow executables
9  flows
10 pure helpers
11 trait methods
12 line task groups
13 stream plans
14 runtime task-plan base rows
```

Rows are encoded in the owning table's canonical dense/source order. Every row
encodes its zero-based source-order role before its typed row semantic digest.
Task launch references inside function/flow/line rows encode a construction-
only `RuntimeTaskPlanBuildCoordinate`; they never encode a final digest key.

Table 14 is special: its rows are encoded inline, not through a completed task-
plan digest. Each source-order base row contains:

```text
coordinate:u32-le
producer_function_semantic_digest:digest32
family_tag:u8
task_class_tag:u8
request_template_digest:digest32
control_effect_contract_digest:digest32
binding_shape
```

`binding_shape` is the same closed tag/payload used by the core static row,
except that View contributes only tag `1` and no upper-layer payload. Timeout
and Line contribute their core-owned contract/plan payloads. The executable
transcript contains no task-plan map key, no completed
`TaskPlanSemanticDigest`, no expected decoded key, and no self-digest field.

`EXECUTABLE_TRANSCRIPT.md` lists every included table row and source-order role.

## 4. Decision 3 — final static row and binding

The final core row is exactly the static information required to reconstruct
the accepted transcript:

```rust
pub struct RuntimeTaskPlan {
    producer_function: RuntimeFunctionSiteId,
    family: NeedProducerFamily,
    class: TaskClass,
    request_template: RuntimeHostTaskRequestTemplate,
    control_effect: RuntimeControlEffectContractId,
    binding: RuntimeTaskSemanticBinding,
}
```

The closed binding is:

```rust
pub enum RuntimeTaskSemanticBinding {
    Ordinary,
    View,
    AwaitManyBase,
    AwaitManyChild,
    Timeout { contract: NeedTimeoutContractDigest },
    Line { plan: LinePlanSemanticDigest },
}
```

No variant contains a copied `ViewProgramId`, `ViewMatchSiteId`,
`CheckedViewMatchAdmissionDigest`, accepted revision, self digest, expected key,
producer contract, producer site, payload type, evaluated arguments,
generation, policy, ordinal, priority, cancellation scope, or debug label.

Family/binding admission is exhaustive and inherent on
`NeedProducerFamily::validate_runtime_task_binding`:

| Family | Allowed binding in structured `RuntimePlan` |
|---|---|
| `StructuredTaskPlan` | `Ordinary` |
| `ViewMatchSubscription` | `View` |
| `AwaitManyBase` | `AwaitManyBase` |
| `AwaitManyChild` | `AwaitManyChild` |
| `Timeout` | `Timeout` |
| `LineTask` | `Line` |
| `HostAdapterTask` | `Ordinary` |
| `MakeNeedHandle` | `Ordinary` |
| `AwbcTaskPlan` | rejected here; the existing AWBC owner remains authoritative |

No free helper or extension trait duplicates this match.

## 5. Decision 4 — unforgeable borrowed digest base

`RuntimeTaskPlanDigestBase<'a>` and `ViewTaskPlanDigestRequest<'a>` have private
fields and no public or crate-wide constructor. They intentionally implement
neither `Clone`, `Copy`, `Serialize`, nor `Deserialize`.

A private `RuntimePlanSemanticSealIssuer` is created inside one call to the
owning semantic encoder. The base borrows the issuer, the candidate row, and
the completed child digests. Because the issuer type and constructor are
private to `plan::task_semantic::seal`, code outside that owner cannot mint a
base even from otherwise valid typed values. The lifetime cannot outlive the
seal pass.

The request exposes only read-only typed getters needed by the upper authority:
plan build coordinate, owner tag, completed child digests, family, class, and
binding marker. It exposes no hasher, byte sink, `Write`, closure, mutable
buffer, raw `[u8; 32]` argument, or caller-supplied completed digest.

## 6. Decision 5 — sole cross-layer protocol

The existing core protocol is evolved in place:

```rust
pub trait ViewTaskPlanAuthority {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, ViewTaskPlanValidationError>;
}
```

This is the only cross-layer sealing protocol. Core has no dependency on
`arcweft-view` or `arcweft-bundle`; the upper implementation depends on core.
There is no extension trait, raw View projection, side catalog, callback byte
sink, or second validation protocol.

The protocol may be omitted only when the candidate contains zero `View`
rows. `RuntimePlanBuilder::finish()` therefore remains sufficient for an
ordinary-only plan. Encountering the first View row without an authority
returns `MissingViewTaskPlanAuthority` before a public plan exists.

## 7. Decision 6 — upper validated View binding

`ValidatedViewTaskPlanBinding` lives beside the production
`ValidatedViewProgramResource` in the bundle/View product. It owns the real:

- `ViewProgramId`;
- `AcceptedViewProgramRevision` as current-resource validation evidence;
- stable `ViewMatchSiteId`;
- exact `CheckedViewMatchAdmissionDigest`; and
- core `RuntimeTaskPlanBuildCoordinate` joined from lowering.

The validated program stores a canonical map from core coordinate to binding.
Construction joins the compiler-local Cut 3 row, current View program and
revision, stable site, and exact admission. It rejects missing, extra,
duplicate, stale, cross-program, or mismatched rows before implementing the
core authority.

When called, the authority first proves that its program/revision/source-set
stamp is still current, then looks up the exact coordinate, verifies that the
opaque base is a View marker with family `ViewMatchSubscription`, and hashes:

```text
accepted core base prefix
binding_tag = 1
ViewProgramId:string
ViewMatchSiteId:digest32
CheckedViewMatchAdmissionDigest:digest32
```

`AcceptedViewProgramRevision` is checked but not written. A revision-only
replacement therefore leaves a plan digest unchanged when program/site/
admission semantics are unchanged.

## 8. Decision 7 — one common seal path and publication order

### Builder

1. `RuntimePlanBuilder` consumes all mutable construction seeds into a private
   `UnsealedRuntimePlanImage`.
2. Existing structural/type/function/flow verification runs without exposing a
   `RuntimePlan`.
3. The private semantic encoder preflights limits and computes all non-task
   executable row digests.
4. It resolves every task row in source order and computes the four child
   digests.
5. It computes one `RuntimeExecutableSemanticDigest` from all fifteen tables.
6. For every task row in source order, it mints an opaque borrowed base.
7. Core seals Ordinary/AwaitMany/Timeout/Line rows; View rows call the supplied
   authority.
8. The candidate checks any expected keys (decode path only), then checks global
   digest uniqueness.
9. Only after every row succeeds does it build one immutable
   `RuntimeTaskPlanTable` and the public `RuntimePlan` in the same final move.

`finish()` invokes this common path with no View authority and default limits.
`finish_with_view_task_plan_authority` invokes it with the validated upper
owner. They do not maintain separate encoders.

### Compiler and bundle

1. Sema publishes the compiler-local Cut 3 View admission product.
2. Runtime-plan lowering pushes static task rows in source order and receives
   construction-only coordinates.
3. Bundle validation joins those coordinates to actual validated View program,
   site, admission, and current revision, producing
   `ValidatedViewTaskPlanBinding` rows.
4. The validated program is passed as the sole View authority to core finish.
5. The resulting sealed table supplies the completed plan digest to
   `NeedProducerSpec` construction and to the purpose-built bundle codec.

### Private decode

1. The codec strictly decodes into private images; stored keys remain private
   `ExpectedTaskPlanKey([u8; 32])` values.
2. It validates canonical lengths, versions, tags, source-order coordinates,
   and references without constructing a public plan.
3. The outer bundle loader validates the View resource and its task bindings.
4. It calls the same core seal function used by the builder.
5. Recomputed digest bytes are compared against each expected key in source
   order. Expected bytes are never an input to hashing.
6. Global uniqueness and all cross-references are checked.
7. The outer validated bundle atomically publishes its `RuntimePlan` and View
   resources only after both are complete.

Thus builder and codec use the same authority. Ordinary plans never need a View
registry because the optional authority is touched only for a validated View
marker.

## 9. Decision 8 — duplicates, limits, and first errors

The final table keeps source-order rows plus a digest-to-index lookup. A digest
must identify at most one static row in one plan. The uniqueness check is global
across every binding and family. Identical static producer sites may reference
one shared row; two separately declared rows that seal to the same digest are a
canonical duplicate and the second source-order row is rejected. This applies
equally to Ordinary, View, AwaitMany, Timeout, and Line.

Because a real BLAKE3 collision cannot be constructed as a normal test fixture,
the uniqueness collector has a private test-only constructor for typed digest
fixtures under `cfg(test)`. It is absent from production API and is used to
prove cross-family collision handling.

The exact first-error order is:

1. outer envelope/version/canonical decode;
2. checked arithmetic, then preflight count/byte limits in declared limit-field
   order;
3. core structural references and family/binding compatibility in fixed table
   and source order;
4. validated View product/binding canonicality and stale-resource stamp;
5. non-task executable row semantic encoding in fixed table/source order;
6. task child resolution per source-order row: producer function, request
   template, then control/effect contract;
7. missing View authority, stale authority, missing binding, then mismatched
   binding for the first View row;
8. semantic work/byte limit at the first charged atom;
9. decoded expected-key mismatch for the first source-order row;
10. duplicate final digest at the second source-order row;
11. final sealed-table cross-reference verification;
12. publication.

A stale authority outranks missing binding because the current-resource stamp
is validated before row lookup. An expected-key mismatch outranks a duplicate
because comparison is performed while collecting recomputed source-order rows;
the duplicate index is built afterward. Builder inputs have no expected keys,
so duplicate is their first applicable post-seal error.

Exact limits and atom accounting are in `ERROR_PRECEDENCE_AND_LIMITS.md`.

## 10. Decision 9 — deletion-driven Cut 5 switch

The final row/table is not independently published before Cut 5. Cuts 1–3
remain the accepted generic Match, ownership, and compiler-local View products.
Cut 4 contains only standalone opaque digest/identity infrastructure and
private sink plumbing.

Inside one Cut 5 atomic switch, implementation introduces the final static row,
private candidate/encoder, upper validated binding, common sealer, expected-key
verification, and final table; migrates every consumer; then deletes:

- every caller-provided task-plan digest parameter;
- every task-plan self/expected digest field;
- the provisional row that contained producer contract/site/payload/operation;
- every raw core View identity projection;
- every general sink/callback/extension sealing path;
- every parallel task-plan table or catalog;
- every generic Serde task transcript;
- every old codec reader, fallback, alias, and optional legacy field; and
- stale generated schema/fixture/document rows.

The exact compile-clean order and consumer cutover are normative in
`COMPILE_CLEAN_SEQUENCE.md`.

## 11. Public identity and zero policy

`TaskPlanSemanticDigest` is a semantic digest, so all 32-byte BLAKE3 outputs,
including all-zero, are semantically valid. This does not authorize a public
raw constructor. A typed value is obtained only from the owner encoder or by
resolving privately decoded expected bytes against an already sealed table.
Absence remains `Option`, never a zero sentinel.

## 12. Final status

Every requested result-changing decision is closed. `OPEN_QUESTIONS` is exactly
`none`. The design is `READY_FOR_IMPLEMENTATION` subject to the implementation
validation commands listed in `TEST_MATRIX.md`; those production commands were
not run for this design-only return.
