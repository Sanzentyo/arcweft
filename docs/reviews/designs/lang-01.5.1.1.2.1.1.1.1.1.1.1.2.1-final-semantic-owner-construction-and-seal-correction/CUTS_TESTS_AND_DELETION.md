# Compile-clean cuts, tests, and deletion

All subcuts belong to one C2 reviewable commit. Each subcut must compile and
pass its focused tests before the next; no intermediate commit/push is made.
An internal compile-clean checkpoint is not a reviewable cut, accepted
completion, or publishable authority. Only the completed C2.6 state may be
committed and pushed.

## C2.1 — lower-owner semantic behavior

Implement:

- `EffectSemanticDigest` on `EffectId`;
- exhaustive `RuntimeAgentField::semantic_tag` in core;
- exhaustive `ProgressField::semantic_tag` and `CharacterField::semantic_tag`;
- exhaustive `ViewElementKind::semantic_tag`; and
- exhaustive `ViewSpecifiedValue::semantic_digest` and nested payload tags in
  `arcweft-view`/`arcweft-presentation` owners.

Tests:

- owner inventories compile without wildcard arms;
- pairwise tag uniqueness from dynamic collection length;
- same value gives same digest;
- every field/payload mutation changes the digest; and
- no literal `26` gate or Serde/debug golden is used.

## C2.2a — projection context and expander foundation

Extract `NominalSchemaExpander` onto the single
`RuntimeNominalProjectionContext`. Give the context fresh per-root
`ProjectionBudget`s, one non-resetting project aggregate budget, the accepted
cache, and retained canonical `TypeShape`. Existing
`FinalSemanticAnalysis::project_*` wrappers temporarily delegate to that
context so this development state remains compile-clean.

This checkpoint does not yet construct the exhaustive request inventory or a
sealed final catalog because the exact C2.3 fact families do not yet exist. The
delegating wrappers are compile-only scaffolding: they may not be committed,
pushed, described as accepted completion, or retained as a second authority.
They are consumed and deleted by C2.4 before C2 publication.

Delete in this subcut:

- the `FinalSemanticAnalysis` dependency inside the expander; and
- every second project shape/layout projection below the temporary delegating
  wrappers.

Tests cover per-root reset across two maximum-work roots; project aggregate
exact-limit/one-over across roots and cache hits; arithmetic overflow;
cancellation before charge/descent; depth, node, generic-argument and work
limits; cache hit; retained canonical `TypeShape`; project generation,
owner/arity/identity mismatch; legal recursive named schema; illegal generic
cycle; and layout sensitivity through the existing projection callers.

## C2.2b — accepted environment Record authority

Move ordered environment fields into
`AcceptedNominalSemantics::Record` and delete every separate TypeCheckEnv
record map. The standard Record constructor atomically creates the accepted
type and ordered field rows; catalog/world digest and borrowed lookup remain
under the existing accepted nominal authority.

Delete in this subcut:

- `HashMap<String, HashMap<String, TypeKind>>` and every replacement name/index
  map; and
- reader-side iteration outside the accepted Record row.

Tests cover environment duplicate and unknown fields, declaration order,
catalog/world digest sensitivity, ordinal overflow, cloned-Record identity
mismatch, raw constructor privacy, exact catalog lookup, instantiation, and
typed runtime-plan rejection without diagnostic-name lookup.

## C2.3 — exact project/case/field/look rows

Add private ID constructors, project item rows, one owner case table, selected
ordinal access, project/environment record pattern rows, typed-binding facts,
field/method selection rows, and manifest-joined StageLook. Every nominal row
producer uses the single C2.2a projection context; no row owns another
projector or projection cache.

The checked-call join builder produces `CheckedCallableJoinDigest` once. A
private call-join staging map is moved into final edge facts after method/View
callee enrichment; it is not retained beside them.

Delete in this subcut:

- string case vectors and selected case name authority;
- duplicated selected case objects;
- name-only Method/Field rows;
- StageLook open-name fallback;
- record-pattern transcript name lookup; and
- runtime-plan/project-field semantic use of diagnostic field names; and
- `CheckedProjectItemFamily`/`CheckedReceiverMode` sketches or any equivalent
  duplicate types.

Tests cover every owner family, generic/project layout changes, payload
presence, selected ordinal mismatch, Character/Builtin/Option/Result case
order, project/environment field selection and record patterns, typed Choice
binding annotations, stale world/revision, opaque-constructor privacy, and
Character look selection-order invariance with selection-payload sensitivity.

## C2.4 — private Entry seal and complete nominal publication

Introduce the draft, narrow Entry authority, prepared Entry references, and
sealed Entry catalog ownership. Now that the exact C2.3 families exist, add the
exhaustive typed projection request visitor over every prepared and final fact
family, finish the complete projection catalog, and publish both catalogs only
through `FinalSemanticAnalysis`. Change `analyze_final_project` error handling
and compiler phase mapping.

Delete in this subcut:

- the C2.2a temporary `FinalSemanticAnalysis::project_*` delegating wrappers
  and every post-seal expansion/recomputation path;
- the public late Entry checker and compiler-owned Entry catalog storage; and
- any `CompiledProject` duplicate of either final-analysis catalog.

Tests cover:

- success with zero, one, and multiple Entry references;
- nested Entry references as call arguments, record values, and control-flow
  children returning their prepared type/effects to the parent checker;
- candidate probe, selected replay, commit, and rollback with Entry references,
  proving one enum-row journal and no leaked probe fact;
- binding digest and value-type sensitivity;
- unrelated Entry invariance;
- missing/duplicate/wrong-owner catalog joins;
- every draft/Entry/seal cancellation boundary;
- deterministic retry and first-error order;
- draft drop and every error publishing neither analysis nor catalog;
- exact seal precedence: checked ID/public ID, source item, value type, then
  binding digest copy;
- every actual C2.3 prepared/published projection visitor family, including a
  nested/generic nominal reachable only through each family;
- first digest-ordered missing cached projection at final seal, borrowed
  post-seal lookup, and zero post-seal expansion;
- compile-fail/private API evidence for draft construction and unsealed
  publication; and
- the selected Entry-before-verification precedence differential.

## C2.5 — fail-closed View and dead Select deletion

Delete View Modifier semantic success and compiler handling. Delete
`TupleElement` and `RecordElement` plus validation/transcript/compiler arms.
Add structured reservations for `0x0405`/`0x0406`.

Tests cover unresolved-dot View rejection at final sema, absence of compiler
success, all remaining Select producers, reserved-tag uniqueness, and source
fixtures proving no behavior depended on either dead variant.

## C2.6 — boundary and workspace validation

Add C2 tests proving:

- Postfix compiler lowering/reachability still follows its selected lookup ID;
- raw Postfix IDs are explicitly nonsemantic inputs for C3;
- RichText accepted token/action rows and child roles are complete while no C2
  RichText digest exists;
- proposed domains are byte-distinct from existing repository domains and
  unique in the C2 structured registry; and
- C1 semantic paths remain byte/behavior identical.

Run and record:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-sema
cargo test -p arcweft-compiler
cargo test -p arcweft-core
cargo test -p arcweft-view
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
just structure-audit
just structure-audit-gate
just test-workspace
```

Do not pass Cargo an explicit job count. Tier 2 is not required unless the
implementation changes runtime/render behavior beyond the typed leaf encoder.

## C3 handoff, not C2 work

C3 consumes C1 paths and these C2 rows to build recursive transcript digests.
It alone adds RichText stable token/open ordinals and Postfix selected-child
digests. C2 receives no completion credit for C3 and must not add a partial
digest or raw-ID hash.
