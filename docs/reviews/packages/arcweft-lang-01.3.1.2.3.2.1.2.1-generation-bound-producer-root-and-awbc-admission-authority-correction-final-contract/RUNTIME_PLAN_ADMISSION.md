# RuntimePlan serialized shape and whole-plan admission

## 1. Raw serialized fields

At the pinned source, `RuntimePlan` owns the current fields:

```text
flows
line_nodes
line_task_graphs
choices
streams
root_reducers
root_resources
root_views
root_generation
content_catalog
callable_executables
traits
trait_impls
entries
```

This correction adds exactly:

```text
generation_contract
```

The contract contains the nominal catalog, project roots, producer payload
contracts, claimed authorization sets, role/custom facts, catalog correlations,
and claimed generation identity.

There is no second catalog field, producer-row table, role side table, custom
digest side scalar, or optional generation token.

## 2. Raw construction

`RuntimePlan::try_with_generation_contract` is a raw builder used by
runtime-plan/compiler projection. It checks only local canonical construction
invariants and replaces the private field.

It does not:

- perform whole-plan reachability;
- issue a catalog;
- issue a producer shape;
- make the plan executable;
- mark deserialized values admitted.

A raw plan remains Clone/Serde quarantine data.

## 3. Project-root projection

Runtime-plan computes project roots from the accepted semantic facts actually
used by the final plan. The projection includes all typed boundaries that can
construct, persist, restore, compare, pattern-match, or expose runtime values.

The root ID is a typed semantic identity emitted with the fact. Root discovery
does not parse plan display IDs, flow names, entry names, string-table rows, or
source labels.

The plan admission implementation recomputes the expected root set from the
plan's typed tables where current typed evidence is present and compares it to
the generation declaration. For facts that exist only at the runtime-plan
bridge, the bridge stores their typed root coordinate in the plan table rather
than relying on an uncheckable side assertion.

## 4. Exact `try_admit` algorithm

`RuntimePlan::try_admit(self)` is consuming and performs:

### Phase P1 — existing plan verification

Run the current `RuntimePlan::verify` logic in its existing deterministic order:

- entry inventory;
- flow/line/choice/stream/root tables;
- callable/trait links;
- content catalog;
- current structural identity and bounds.

A failure returns the existing `RuntimePlanError`. No generation-contract
validation has published state.

### Phase P2 — declaration shape

Validate:

1. project-root count/order/duplicates;
2. producer count/order/duplicates;
3. payload-kind-specific local structure;
4. six required CharacterDialogue base roles and derived Style equality;
5. no unresolved semantic coordinate or Named type;
6. custom field/View order, duplicates, and limits;
7. exact `std.character_dialogue` producer identity.

The first failure follows canonical array order.

### Phase P3 — custom digest

For each CharacterDialogue producer declaration in producer order:

1. encode canonical descriptor body;
2. recompute BLAKE3 runtime custom digest;
3. compare with claimed digest.

Digest failure precedes nominal-catalog lookup.

### Phase P4 — nominal catalog consistency

Validate catalog count/order/duplicates and every descriptor:

1. key equals descriptor nominal/semantic/layout scalars;
2. defining field IDs are `1..=count`;
3. names/fields are canonical;
4. checked types satisfy depth/work-local limits;
5. equal keys never have conflicting descriptors.

Build only a temporary quarantine map. Do not expose it.

### Phase P5 — independent project traversal

Traverse every project root in root-ID order against the temporary catalog.
Record the project closure. Claimed producer keys are not inputs.

### Phase P6 — independent producer traversal

For every producer in producer-ID order:

1. enumerate roots from payload;
2. traverse in root-coordinate order;
3. produce the derived key set;
4. compare it with claimed authorization keys by sorted merge walk.

First missing or extra key is deterministic by producer ID then catalog key.

### Phase P7 — global exact reachability

Union project and producer closures. Compare with the catalog key set:

1. first reachable key missing from catalog;
2. first catalog key unreachable from all independent roots.

A claimed row alone is never reachable.

### Phase P8 — plan/declaration correlation

Recompute the plan-side typed root inventory and producer coordinates from
current typed plan tables. Require exact equality with the declaration's root
coordinates and checked-type canonical bytes.

This prevents a valid but unrelated generation contract from being attached to
a structurally valid plan.

### Phase P9 — generation identity

Canonical-encode the parsed declaration body, recompute
`RuntimeGenerationIdentity`, and compare with the claim.

If the plan is being joined to an existing admitted aggregate, also compare
canonical body bytes.

### Phase P10 — atomic publication

Only after all phases succeed:

1. freeze the temporary catalog;
2. freeze project and producer derived sets;
3. freeze CharacterDialogue typed payload;
4. construct `AdmittedRuntimeGenerationInner`;
5. wrap it in `AdmittedRuntimeGeneration`;
6. return `AdmittedRuntimePlan`.

No user callback or runtime code runs during admission.

## 5. Required error order

The public observable order is:

1. Serde/codec/header/fixed version checks outside `try_admit`;
2. current `RuntimePlan::verify`;
3. root/role/custom declaration;
4. custom digest;
5. catalog scalar/structure;
6. project traversal;
7. producer traversal;
8. producer claimed-set equality;
9. catalog missing/unreachable;
10. plan/declaration root correlation;
11. generation identity/body correlation;
12. wrapper publication.

An error from a later phase is unreachable when an earlier phase fails.

## 6. `RuntimePlanError`

The original enum in `plan::entry_inventory` remains the owner. Add one
source-preserving variant:

```rust
#[error("runtime generation contract is invalid: {source}")]
GenerationContract {
    #[source]
    source: RuntimeGenerationContractError,
},
```

When plan/declaration correlation needs a distinct source, add it to the same
original enum, not a parallel error wrapper.

## 7. Runtime use

Runtime entry points accept `AdmittedRuntimePlan`, `&AdmittedRuntimePlan`, or a
higher generation image that owns it. A function may accept raw RuntimePlan
only if it consumes and fully admits it before returning any runtime object.

`verify()` remains a diagnostic structural check. Its return type does not
implement or convert into admitted authority.

## 8. BytecodeProgram boundary

`BytecodeProgram::from_runtime_plan` and `into_runtime_plan` remain quarantine
conversion boundaries only. They do not produce an Engine, executor, VM, or
admitted wrapper.

Operational conversion is explicit:

```text
BytecodeProgram
  -> raw RuntimePlan or raw AwbcProgram
  -> complete try_admit
  -> admitted plan/product
  -> runtime
```

No conversion method carries a previously issued catalog independently of the
generation contract.

## 9. Atomic failure

Every phase builds local temporary values. On any failure:

- `self` is consumed/dropped;
- no operational catalog exists outside the call;
- no producer shape exists;
- no generation image changes;
- no restore/ownership traversal runs;
- no partial plan is returned.

## 10. Implementation assertions

Structure tests must prove:

- raw RuntimePlan type has no execution trait implementation;
- `RuntimePlan::verify` result cannot be converted to admitted plan;
- `AdmittedRuntimePlan` has no Serde/Deref/into-inner;
- generation contract is required, not Option/Default fallback;
- no producer-only row table remains;
- every runtime constructor accepting a plan requires admitted authority.
