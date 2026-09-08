# Generic scope migration — 2026-09-08

Status: IN_PROGRESS. The working copy does **not compile** during this API
migration. This is continuation evidence for the existing
[convergence goal](2026-09-08-convergence-goal-plan.md), not a completed cut,
an acceptance waiver, or an external blocker.

Latest continuation evidence:
[source completion and binder scope](2026-09-08-source-completion-scope.md).

The existing checkout remains on `main` at
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. The inherited working-copy changes
remain, the index is empty, and this implementation has not been committed or
pushed. The passing tests in the earlier
[callable execution record](2026-09-08-callable-execution-continuation.md)
predate these changes and are historical evidence only.

## Changes in the working copy

- `types/generics.rs` introduces kind-separated Free, Bound, and Inference
  references. Declaration identities remain declaration identities. Only the
  types owner can issue an application namespace or construct a checked bound
  slot. Type and constant references have been connected to `TypeKind` and
  `ArrayLength`. Effect references are defined but their producers and
  consumers still require migration.
- Function types retain a three-namespace binder. Comparison, mismatch paths,
  constraint shape reconstruction, named-type mapping, and the existing
  substitution operations preserve that binder. An empty binder introduces no
  lexical depth; each nonempty binder introduces one depth.
- Callable initialization separates the enclosing Free inventory from the
  callee template inventory. The same declaration identity can lawfully occur
  in both inventories. Opening a pattern produces the callee's inference
  references; actual operand types do not undergo that opening.
- Active constraint maps, normalization, and occurs checks use reference
  identities. `constraints/references.rs` supplies one scope-aware reference
  mapper for template opening and the completion/reopening transitions over
  the existing constraint shape. It enters the candidate's cancellation and
  node accounting at every visited type and array-length node.
- Completed type/constant solutions retain a declaration contract and a
  residual binder, rather than their active application issuer. Normalization
  precedes reification. Unsolved future references become bound residual slots;
  a later continuation opens those slots in its own fresh application scope.
  The caller's Free reference is preserved, including when its declaration ID
  is also a callee template ID.
- Completed binding iterators and keyed projection views carry their lexical
  scope. Keyed projections are reified after the solution seal, before being
  returned by the lower transaction. The frozen solution's type/constant
  binding interfaces and type visitor now forward these scoped views. Deferred
  rows are restricted to parameters that actually remain residual.
- The generic-use visitor now has two typed inventory domains over the same
  traversal. Stable declaration inventories reject active inference atoms;
  application-local expression hints retain exact reference identities. This
  does not turn an inference atom into a declaration ID or a source label.
- `TypeKind::semantic_identity_digest` now returns a scope error instead of
  permitting active type/constant inference references to enter stable bytes.
  The explicit scoped entry point encodes incoming binder arities; function
  schemes encode their own binder. A declaration ID itself still has an
  infallible Free-reference digest through the same canonical encoder, since
  its identity contains no lexical or application-local type children.
- Array-length encoding has an explicit scoped path. A naked bound or active
  length cannot be encoded as a root checked length. Variant payload field
  construction propagates invalid type scopes. Case identity uses its ordered
  sealed field identities, which already include the field types, instead of
  hashing the same field type again.
- Entry contract encoding rejects generic references rather than turning a
  diagnostic source label into a nominal type. Persisted nominal schema
  substitution explicitly matches Free declaration references; bound or
  active references cannot enter that declaration-keyed substitution path.

These are connected migration changes, not evidence that the complete generic
call contract or runtime callable ABI has been implemented. Contract versions
remain `1`; no old reader or compatibility version was introduced. The frozen
review package was not edited.

## Added behavioral tests

`types/constraints/solution/residual/tests.rs` exercises:

1. A recursive binding whose callee inference variable and caller Free variable
   share one declaration ID. The completed result must not be an occurs cycle
   and must compare equal across independently issued application scopes.
2. A function with its own binder, a future type, a future constant length, and
   a caller Free reference. The inner binder must remain at depth zero and the
   residual references must be reified at the correct outer depth. A root
   digest must reject the open term while a scoped digest can encode it.
3. Reopening one prefix twice, then completing it with different type and
   length arguments. The openings must have different inference identities,
   and completing one must not change or capture the other or its Free caller
   reference.

These tests are **not run**: the crate does not yet compile. Their presence is
not acceptance evidence.

## Validation actually performed

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**.
  The latest invocation reports 273 library errors and 344 library-test
  errors. These are compiler diagnostic counts, not failed test counts.
  `target/generic-reference-migration.log` contains the latest diagnostic log.
  The command was repeated after representation and API changes to expose
  consumers requiring migration.
- `cargo fmt -p arcweft-lang-sema`: completed.
- `cargo fmt --all -- --check`: **PASSED**.
- `git diff --check`: **PASSED**. The staged diff remains empty.
- Runtime execution, native/AWBC integration tests, workspace check, Clippy,
  test-workspace, doctests, codec/golden validation, the structure gate, and
  Tier 2: **NOT RUN for this migration**. Earlier results are not current
  passes. There is no external condition preventing continued implementation.

## Required continuation

1. Migrate stable digest consumers and their owning seals to propagate scope
   errors or consume already validated scoped evidence. Do not encode an
   active issuer, sort failed digests as data, add a default digest, or insert
   unchecked `expect` calls to restore the old infallible API. Declaration-only
   identity encoding and application-local structural memoization must retain
   their distinct, typed roles.
2. Finish scoped projection and binding consumers. Retire the old unscoped
   `apply` / `instantiate_type` contract in favor of explicit template
   application and caller-Free closure, with correct shifts under binders and
   one flat closed instance substitution. The old multi-pass substitution and
   project instance solution layers remain and are not a final model.
3. Complete the type structural fold for every producer and consumer, including
   the sealed nominal/payload boundary. The current constraint shape still
   treats fixed vectors and variant payloads as leaves. Do not manufacture
   active, prehashed payload field evidence to work around that ownership.
4. Complete the effect Free/Bound/Inference migration through the same opening
   and residual authority. The old effect issuer overlay, effect scope storage,
   and effect-row substitution remain. In particular, the existing `apply`
   path's unknown-effect-row panic is not fixed by this record. An unknown row
   must not be silently treated as pure.
5. Close the full source/materialization transcript. `ClosedConstraintProbe`
   still aliases its active representation; domain evidence and sealed branch
   values need the same completion boundary as solution rows and projections.
   Reifying keyed projections alone does not prove complete publication.
6. Resolve and implement function-scheme specialization through all ordinary
   callable, continuation, closure, native, and AWBC consumers. In particular,
   the function-value schema's new binder input still needs its implementation.
   Do not infer a language ban on a polymorphic prefix as a monomorphic callback
   from that missing execution path.
7. Migrate the older lower tests from declaration IDs to explicit template,
   actual-Free, and opened-reference fixtures where appropriate; then run the
   added scope tests, all lower tests, sema generic regressions, and the existing
   real native/AWBC callable matrix. Proceed through the remaining full goal
   after the callable boundary is implemented and verified.

The instance graph work, lazy branch execution changes, and the remaining
Match/View/RuntimePlan/nominal/scheduler/restore acceptance criteria remain in
scope. This record neither narrows the goal nor declares a reviewable commit.

## Continuation: sealed identity consumers and scoped visitation

The next continuation retained the same `main` and accepted SHA, preserved the
inherited changes, and left the index empty. It made the following connected
changes; none is a completed callable cut.

- `CallableSignatureSchema` now publishes only after its construction contents
  have produced a valid canonical digest. The immutable schema retains that
  digest. Reserved-name, extension-receiver, and evaluated-effect updates
  consume the schema and seal the changed contents again. Evaluated-effect
  attachment returns an error for an incompatible effect authority. Fixed
  built-in/standard table constructors retain explicit assertions about their
  already-sealed fixed schemas; arbitrary type digest failures are propagated.
- `EnvironmentCallableId::try_new` validates and encodes its receiver-bearing
  identity before construction. Ordering and hashing consume its retained
  immutable canonical bytes. This removes repeated fallible type encoding
  from those operations. No failed digest is used as an ordering key.
- Callable catalog and environment publication encoding now propagate scope
  errors. Canonical ordering establishes all fallible keys before sorting or
  publishing rows. Registered-catalog construction is fallible as well.
- Checked-call encoding propagates root type-scope failures. Callable value,
  continuation, outcome, diagnostic, and final fact visitors carry scoped type
  views through the owning traversal. A root view is contextual input, not a
  certificate of validity; stable encoding remains fallible.
- Generic-Match semantic transcripts, final owner-bound resolution and final
  validation now propagate type-scope failures. The existing Match work/byte
  budgets and transaction remain; this change does not complete C3/C5.
- Project-nominal occurrence traversal retains the lexical scope while entering
  function binders. Stable generic-reference collection distinguishes external
  Free/Bound dependencies from function-local quantifiers and rejects active
  inference atoms. The runtime nominal request collector uses this scoped
  evidence before admitting a fully closed nominal. It does not promote an
  open nominal into a runtime request by dropping the incoming scope.

Four tests were added for reserved-name schema identity, rejection of an active
receiver by environment identity construction, outer versus function-local
generic dependencies, and rejection of active hint variables by stable
inventory/encoding. They are **not run** because the crate remains uncompilable.

Latest verification after these changes:

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  175 library diagnostics and 235 library-test diagnostics. The log remains
  `target/generic-reference-migration.log`; these counts are not test results.
- `cargo fmt -p arcweft-lang-sema`, `cargo fmt --all -- --check`, and
  `git diff --check`: **PASSED** at the inspected points. The index is empty.
- Native/AWBC execution, workspace check, Clippy, test-workspace, doctests,
  codec/golden, structure, and Tier 2: **NOT RUN for this continuation**.

The remaining errors expose unconverted model/seal and digest consumers, older
test fixtures, and the function-value schema's binder input. The implementation
still needs the unscoped application/layer replacement, effect namespace
migration, source/branch completion, and runtime value integration listed above.

The result-changing function-scheme boundary is now tracked by
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
It groups use-site instantiation, template/operand opening, residual solutions,
multiple possible value origins, callable value/group execution, effects,
discovery, and program-bound restore. There is no named external blocker and
no returned contract yet. The parent package's published-prefix restriction
is under adjudication, not accepted as a way to avoid its missing runtime route.
Do not assign a fabricated declaration ID to anonymous scheme slots or add a
blanket nonempty-binder rejection just to restore compilation.

## Continuation: variant owner sealing and nominal/Match consumers

Supersedes the preceding **latest verification** status, not its historical
results. This continuation retains `main` at
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, preserves the inherited working
copy, and leaves the index empty. It is still an uncommitted implementation
migration, not an accepted cut.

- `CheckedVariantOwner` is an opaque structure with one immutable case table,
  one retained semantic type digest, and a read-only typed origin descriptor.
  Its owning module seals type identity, case ordinals, payload rows, and
  field/case identity before publishing the owner. Project construction checks
  the supplied nominal identity against its typed projection. Environment and
  runtime-builtin descriptors retain the actual source type; they do not
  reconstruct it from a name. Generation membership and declaration/schema
  selection remain the responsibility of final semantic analysis.
- `try_option` and `try_result` return `CheckedVariantOwnerError`. Unbound
  lexical type/constant references and active type/constant inference atoms
  cannot be hashed into these checked owners. A function payload's own binder
  remains valid. Effect references still require the namespace migration
  listed above; this change does not certify the old effect representation.
- The public owner enum and its externally replaceable case-table fields are
  gone. `has_valid_case_rows`, its repeated resolver/coverage/final-validation
  checks, and the old free construction helpers are deleted. Compiler variant
  projection reads the immutable owner and its type projection. Independent
  checks against runtime catalogs, source payloads, and final generations are
  retained where they prove a join to another authority.
- `VariantPayloadSealError` is now a public structured error used by the owner
  seal. Payload-type admission still uses its separate owning constructor;
  this is not a claim that the complete payload structural fold is finished.
- Match domain construction, pattern deconstruction, recursive witness
  selection, and coverage transcripts propagate scope failures. Their stable
  keys are successful digests, not `Result` values. Existing budgets and
  coverage algorithms are unchanged. Nominal record/pattern sealing now
  propagates digest failure before comparing source and declared field types.
  The redundant free nominal-type reconstruction helper was removed in favor
  of the nominal owner's `ty()` projection.
- Accepted environment-record construction/validation, project nominal
  semantic definitions, checked callable interface digests, attached-default
  report construction, and the try-carrier transcript now propagate invalid
  scopes. Existing `Option`-returning model admission methods reject types
  without a stable root digest. The sole new infallible assertion is for the
  fixed `Character` entity leaf, whose digest has no type children or generic
  references; it does not unwrap a user-supplied type.
- Existing fixture digest comparisons unwrap valid fixture results explicitly.
  Function-shape fixtures now compare binder fields. Three focused tests were
  added: escaped bound/active type rejection, locally bound function payload
  acceptance, and refusal to transplant payload rows across owners or case
  ordinals. These tests are **not run** while the crate fails to compile.

Latest validation actually performed:

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  **72 library diagnostics and 96 library-test diagnostics**, exit 101. The
  current log is `target/generic-variant-owner-migration.log`. Intermediate
  checks failed at 157/214, 115/158, 95/134, and 72/111 respectively. These are
  compiler diagnostics, not executed-test counts.
- `cargo fmt -p arcweft-lang-sema -p arcweft-compiler`,
  `cargo fmt --all -- --check`, and `git diff --check`: **PASSED**.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-generic-variant-owner
  --fail-on-blocking`: **PASSED**, exit 0; 95 workspace packages, 2,210 Rust
  files, 311 review triggers, and **0 blocking violations**. Generated
  [measurements](structure-audits/2026-09-08-generic-variant-owner/file_metrics.csv),
  [dependency edges](structure-audits/2026-09-08-generic-variant-owner/dependency_edges.csv),
  and [findings](structure-audits/2026-09-08-generic-variant-owner/findings.md)
  are retained as inspection evidence, not production source or semantic tests.
- Focused execution tests, compiler/runtime checks beyond the failing sema
  dependency, workspace all-target/all-feature check, Clippy, test-workspace,
  doctests, codec/golden, and Tier 2: **NOT RUN for this continuation**. The
  unresolved sema migration prevents execution acceptance; no earlier green
  result is substituted for it. No explicit Cargo job count was used.

### Ownership review of touched structure triggers

The following are complete working-copy measurements. Base LOC is measured
against the full accepted SHA above and includes inherited work in the overall
goal; the growth is not attributed solely to this continuation. The generated
CSV records classification and embedded test LOC for every file. Paths in the
table are relative to `crates/`; all rows are production except `tests.rs`.

| Path | Base LOC | Current LOC | Bytes |
| --- | ---: | ---: | ---: |
| arcweft-compiler/src/lower.rs | 3,981 | 7,774 | 328,864 |
| arcweft-lang-sema/src/callable/checked_catalog.rs | 1,898 | 2,639 | 96,797 |
| arcweft-lang-sema/src/callable/resolver.rs | 1,412 | 1,639 | 60,857 |
| arcweft-lang-sema/src/env/nominal.rs | 1,666 | 1,714 | 57,136 |
| arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs | 3,357 | 3,990 | 175,398 |
| arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs | 1,235 | 1,282 | 51,588 |
| arcweft-lang-sema/src/final_analysis/model.rs | 2,481 | 2,774 | 89,997 |
| arcweft-lang-sema/src/final_analysis/model/variant_owner.rs | 0 | 479 | 16,412 |
| arcweft-lang-sema/src/final_analysis/nominal_schema.rs | 2,357 | 2,947 | 121,037 |
| arcweft-lang-sema/src/final_analysis/report.rs | 1,165 | 2,230 | 89,832 |
| arcweft-lang-sema/src/final_analysis/semantic_transcript.rs | 1,193 | 4,098 | 164,098 |
| arcweft-lang-sema/src/final_analysis/tests.rs | 8,292 | 8,983 | 310,090 |
| arcweft-lang-sema/src/final_analysis/validation.rs | 2,242 | 2,749 | 113,205 |
| arcweft-lang-sema/src/types.rs | 1,330 | 1,709 | 56,055 |

Disposition and reviewed boundaries:

- **Variant semantic owner:** decomposition completed at the state and API
  boundary. Case creation, complete-owner sealing, immutable projections, and
  selected-ordinal evidence moved together into `model/variant_owner.rs`.
  Construction remains private or crate-owned except the public built-in
  `Option`/`Result` factories. The public kind view and error have real compiler
  consumers; they are not exports introduced merely to permit a file split.
  The 25 embedded test LOC exercise private payload admission. Other owner
  behavior tests remain in `model/variant_tests.rs`.
- **Final model and type vocabulary:** retain the generation-bound fact model
  and exhaustive type vocabulary in their current responsibility modules.
  The changed sections add no parallel identity store or independent state.
  Variant state is now separately owned; existing record/stage/payload/type
  modules retain their responsibilities. This is a cohesion justification for
  the touched model/type seams, not final acceptance of the still-incomplete
  generic/effect contracts elsewhere in the goal.
- **Nominal schema and environment catalog:** these own exact declaration
  instantiation, staged projection/sealing, and schema admission. Their
  private phase types retain construction state; final catalogs retain only
  accepted rows. This continuation changes scope-error propagation inside
  those joins and does not add a second projector, transport, or persistence
  executor. Keeping these phase transitions with their owning construction
  contexts is the current cohesion disposition.
- **Callable catalog and resolver:** the checked catalog owns final callable
  interfaces; the resolver consumes selected checked facts. Interface identity
  construction now propagates invalid types. Resolver revalidation of the
  opaque variant table is removed. Neither change introduces a second catalog.
  The obsolete multi-layer instance substitution in `join.rs` is explicitly
  still incomplete and has not been repaired by discarding scoped views.
- **Expression/pattern analyzers, report, validation, and transcript:** these
  remain separate generation analysis/publication responsibilities. Their
  variant handling delegates to the owning seal. Report joins attach exact
  accepted facts, validation checks generation/consumer relations, and the
  transcript writes typed facts through its existing byte budget. The changes
  introduce no mutable state clusters in these large dispatchers. Existing
  expression/transcript embedded tests remain 88/98 LOC respectively and were
  not extended with unrelated tests.
- **Compiler projection:** the large `lower.rs` remains the orchestration
  entry for projection from one accepted semantic generation. Variant-family
  projection stays in `lower/variants.rs` (411 LOC); this change adds typed
  error propagation and consumes the semantic owner instead of rebuilding its
  type/digest. It introduces no I/O, additional catalog, or runtime executor.
- **Tests:** the large final-analysis suite retains its shared generation
  fixtures. This continuation migrates its digest/binder assertions; the new
  owner tests were placed with the owner responsibility. No new unrelated
  subsystem tests were added to that suite.

The Cargo graph reports sema production fan-in/fan-out **8/14** and compiler
**3/23** (development **3/0** and **1/5**). No dependency edge or Cargo feature
was added by this continuation. The measured graph has no blocking direction
violation; this does not prove the remaining generic source/branch, instance,
function-value execution, or restore contracts complete.

Remaining work is unchanged: complete stable digest consumers still failing
in registration, entry, text proxies, ownership, and prepared facts; replace
the old unscoped solution/layer application; finish the full structural fold,
effect namespace, and source/branch completion; resolve and implement the
function-scheme/value-execution request; then run the required connected tests
and all remaining goal stages. No design package was edited, no compatibility
path or version bump was added, and no external blocker is asserted.

## Continuation: structural fold and payload-owner evidence

Supersedes the preceding latest verification status. The previous goal turn
made implementation and validation progress; this continuation also preserves
the full goal. Git remains `main` at
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`, with an empty index and inherited
changes preserved. After these edits, status contains 8 deleted, 635 modified,
and 57 untracked entries. Untracked directories count as status entries, not
individual files. No commit or push was made.

Performed changes and findings:

- Fixed vectors now participate in the common unary constraint shape with
  their exact dimension discriminator and component child. Opening,
  normalization, occurs checks, reference reification, and rebuilding consume
  that shared shape. They no longer classify the component as an opaque leaf.
  This completes the fixed-vector part of continuation item 3 above; variant
  payloads still need the boundary resolution described below.
- Two lower-owner regression tests were added in
  `types/constraints/references/tests.rs`: opening and completing a component
  alias for dimensions 2/3/4, and rejecting a component cycle before a
  completed solution can publish. They have not executed.
- Typed-binding, Stage-look, prepared nominal/record-pattern, and checked
  record-pattern admission now handle fallible type identities. Existing
  `Option` admission methods reject unencodable roots. The checked record
  field constructor is fallible and its nominal seal consumer propagates the
  error. The fixed Entry entity leaf retains an explicit invariant assertion;
  it has no generic/type children and is not a user-type digest unwrap.
- The planned replacement of `TypeConstraintSolution::apply` exposed a
  prerequisite that cannot be solved by renaming the method. The current
  `VariantPayloadType` retains its owner's digest but not its owner type term.
  In `types/substitution.rs`, `try_map_variant_payload_type` transforms payload
  fields and reissues their identities under that unchanged owner digest. A
  payload from `Option<T>` thus cannot become the exact payload of
  `Option<i64>` when `T` is closed. The checks can agree with the stale owner
  locally while failing the required instantiated-owner join. Its infallible
  wrappers also assume transformed fields cannot fail stable sealing.
- A new real-engine fixture,
  `generic_option_payload_keeps_the_instantiated_owner`, calls a generic
  `Option<T>` extractor and requires `42` from both native and AWBC. It is
  part of `arcweft-compiler/tests/callable_execution.rs`; both generated tests
  are **not run** while sema compilation fails.
- [The function-scheme correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
  now explicitly requires owner type/projection evidence, complete payload
  traversal, the distinction between active terms and sealed semantic rows,
  and the generic payload execution cases. The request remains `ACTIVE`.
  No returned design or final payload representation is claimed here.

The old unscoped `apply`, multi-pass substitutions, and instance layers remain
unchanged in this continuation. Passing only `ScopedTypeView::value()` to
those layers would lose the residual scope and is not an acceptable repair.
The required resolution must transform owner arguments and payload types from
one typed schema/projection authority, then issue stable identities at the
appropriate completion boundary. It must not recover an owner from a digest,
retain the old digest as an alias, or publish inference-bearing checked rows.
Effect namespace and function-scheme opening decisions remain coupled to that
full application contract. The incomplete boundary is internal design work,
not an external blocker or a new exemption from the goal.

Source evidence for that finding (working-copy SHA-256; these are inputs, not
acceptance tests):

| Path under `crates/` | Bytes | SHA-256 |
| --- | ---: | --- |
| arcweft-lang-sema/src/types/variant_payload.rs | 23,879 | `0c3cb3a0a51689ed5525bb546b86f06c073c3db7891c00502e36118f02c64140` |
| arcweft-lang-sema/src/types/substitution.rs | 47,517 | `68704dc2018299553996374cbb119f411818af6592f50a3574a44e11083fa2ae` |
| arcweft-lang-sema/src/types/constraints/shape.rs | 18,290 | `6e58924edf9bb92ccd80070030773142c2413cc35c45a7c18db733326b33f540` |
| arcweft-lang-sema/src/types/constraints/references/tests.rs | 4,070 | `8082896a47d14341abf02ac61604d3b395af7bfcafd4ad7e7484c01ce7ac569f` |
| arcweft-compiler/tests/callable_execution.rs | 7,101 | `599240635edd4dcd647bf96290c946c6377ef343f2b1e5eed16e3fd8f7015896` |

Validation actually performed:

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  latest **61 library diagnostics and 85 library-test diagnostics**, exit 101.
  The earlier check in this continuation failed at 72/96. The latest log is
  `target/generic-structural-fold-migration.log`. These are compilation
  diagnostics; no focused test has run.
- `cargo fmt -p arcweft-lang-sema -p arcweft-compiler`,
  `cargo fmt --all -- --check`, and `git diff --check`: **PASSED**.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-generic-structural-fold
  --fail-on-blocking`: **PASSED**, exit 0; 95 workspace packages, 2,211 Rust
  files, 311 review triggers, and 0 blocking violations. The generated
  [measurements](structure-audits/2026-09-08-generic-structural-fold/file_metrics.csv)
  and [findings](structure-audits/2026-09-08-generic-structural-fold/findings.md)
  are retained. No Cargo dependency or feature changed.
- Native/AWBC execution, workspace all-target/all-feature checks, Clippy,
  test-workspace, doctests, codec/golden, and Tier 2: **NOT RUN** for this
  continuation. These remain required before accepting the connected cut.

Structural disposition: the constraint shape (522 LOC, 18,290 bytes) owns
constructor metadata and child reconstruction; the reference mapper (138 LOC,
4,572 bytes) owns scope transitions through that shape. The separate 112-LOC
test module exercises completion and rejection, not source spelling. No new
solver or second vector traversal was introduced. The record model (550 LOC,
17,260 bytes) retains field identity admission, and its only changed
construction consumer remains the nominal seal. The prepared-fact owner
(1,647 LOC, 53,283 bytes) retains generation-local admission shells; this edit
adds no independent state or projection authority, so its cohesive preparation
responsibility is retained. The prior nominal-seal cohesion review still
applies (2,947 LOC, now 121,042 bytes). The engine fixture is a 269-LOC test
module (7,101 bytes) using actual Engine/VM execution. The unresolved payload
representation is explicitly pending design resolution; size screening is not
evidence that its semantics are correct.

## Continuation: typed payload projection and stable identity consumers

Supersedes the preceding continuation's pending payload-representation status,
not its historical validation. The existing `main` checkout still has accepted
HEAD/origin/main `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. Current status is
8 deleted, 640 modified, and 59 untracked entries; the index is empty. The
inherited changes remain in place. No implementation commit or push has been
made. This is an in-flight part of the connected convergence goal.

### Working implementation

- Logical `VariantPayloadType` now retains the actual owner `TypeKind`, case
  ordinal, and raw tuple/record field types in
  `types/variant_payload/projection.rs`. A shared child walk transforms the
  owner and fields together. Raw terms contain no prehashed field/case rows
  and can participate in opening, normalization, occurs checks, and residual
  reification. Record names remain diagnostic data in logical type equality.
- `CheckedVariantPayload` is the separate stable-row seal. It validates the
  transformed owner and fields and issues field/case identities using the
  existing version-1 encoding. Scoped payload encoding validates all logical
  children in their actual binder; it does not discard that scope to reuse a
  previously cached case ID. The old field-only mapping helpers and their
  infallible field-sealing assumption have been removed.
- Prepared record fields now retain declaration coordinates. Final record
  sealing joins each coordinate and type to the actual owner definition and
  only then consumes the accepted field ID. `AcceptedEnvironmentRecord`
  shares the actual accepted schema, replacing the digest/count-only identity
  carrier. It is the owned view of the same catalog record, not another
  catalog or copied field inventory. Project and payload record joins use
  their respective exact definitions at the same final boundary.
- Pattern binding, payload coverage/deconstruction, nominal visitation,
  substitutions, type ordering, and compiler runtime-field projection consume
  the logical or sealed representation appropriate to their phase. Generic
  use and poison checks include owner-only arguments. Physical ownership and
  runtime layout still traverse fields, not recursively materialize the
  entire variant owner.
- Stable identity errors now propagate through ownership admission, producer
  argument evidence, custom CharacterDialogue registries, Rust metadata,
  registration/environment digests, Entry, captures, Match edges/literals,
  text proxies, project indexing, and resolver-owned callable identities.
  Source diagnostics preserve the relevant source span; typed error owners
  retain the scope failure. Option/bool admission APIs reject invalid type
  identities. No error is replaced with a digest or used as a successful key.
- `SemanticTypeDigest` owns its context-free conversion to the core runtime
  identity. `AcceptedNominalId` owns its declaration-only digest; concrete
  type arguments continue through `TypeKind`. The obsolete free nominal
  digest function/re-export and ownership conversion helper were removed.
  No dependency, feature, serialized version, or released-compatibility path
  was introduced.

New behavioral tests, all **NOT RUN**, cover:

- normalization/resealing of a Result payload whose generic parameter occurs
  only in its owner, including changes to owner/case/field identity;
- rejection of a cycle occurring only in the payload owner;
- a residual owner containing both type and constant slots, whose scoped
  identity is valid while its naked root cannot be sealed;
- an owned environment record retaining and distinguishing exact field
  definitions even when nominal ID, root type digest, and field count agree;
- refusal to mint a persistent Need identity from a payload with an escaped
  lexical binder.

The earlier native/AWBC `generic_option_payload_keeps_the_instantiated_owner`
fixture remains unexecuted. None of the new tests establishes engine or
restore acceptance until the crate compiles and they actually run.

### Validation actually performed

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  latest **15 library diagnostics and 38 library-test diagnostics**, exit 101.
  The log is `target/generic-identity-consumers-migration.log`. Earlier checks
  in this continuation failed at 64/88, 56/79, 32/55, and 16/39. These counts
  describe compile diagnostics, not executed or passing tests.
- `cargo fmt -p arcweft-lang-sema -p arcweft-compiler`,
  `cargo fmt --all -- --check`, and `git diff --check`: **PASSED**.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-payload-projection
  --fail-on-blocking`: **PASSED**, exit 0; 95 packages, 2,212 Rust files,
  311 review triggers, and 0 blocking violations. Generated
  [measurements](structure-audits/2026-09-08-payload-projection/file_metrics.csv),
  [dependency edges](structure-audits/2026-09-08-payload-projection/dependency_edges.csv),
  and [findings](structure-audits/2026-09-08-payload-projection/findings.md)
  are retained. Structural screening does not prove the new semantics.
- Local Markdown file targets in this record and the updated correction
  request: **PASSED**, 18 targets checked, 0 missing. This checks file links,
  not semantic acceptance or Markdown anchor validity.
- Native/AWBC execution, workspace all-target/all-feature checks, Clippy,
  test-workspace, doctests, codec/golden, and Tier 2: **NOT RUN** while the
  semantic crate is uncompilable. They remain required for the full cut.

### Structure and ownership disposition

The new projection owner is 343 LOC / 11,713 bytes. It owns logical payload
children, transformation, and the transition to the stable row owner. Its
sealed counterpart is 802 LOC / 26,419 bytes, including 267 embedded test LOC.
The shared constraint shape is 530 LOC / 18,830 bytes; its 307-LOC reference
test module exercises that same transformation/completion boundary. The
decomposition follows phase and invariant ownership. No public API was added
merely to split a file, and no second traversal/solver was introduced.

Touched larger owners are listed below. Base LOC refers to the full accepted
HEAD file, so growth includes inherited goal work, not only this continuation.
Paths are relative to `crates/`; exact bytes, classification, embedded test
LOC, and package ownership are in the generated measurements.

| Owner | Base LOC | Current LOC | Cohesion disposition for this continuation |
| --- | ---: | ---: | --- |
| arcweft-compiler/src/lower.rs | 3,981 | 7,777 | Existing compiler projection context; payload field projection remains with its nominal/runtime join. Full instance authority replacement is still required |
| arcweft-lang-sema/src/callable/resolver.rs | 1,412 | 1,640 | Callable origin/capture error conversion; no new resolver state or lookup route |
| arcweft-lang-sema/src/checked_text_proxy.rs | 0 | 1,847 | Checked proxy definition/application values and canonical encoders retain their own error boundaries |
| arcweft-lang-sema/src/checked_text_proxy/prepared.rs | 0 | 1,678 | Source preparation, scalar admission, and diagnostics; field IDs are issued only after valid field identities |
| arcweft-lang-sema/src/entry/checker.rs | 1,999 | 2,024 | Entry roles and target parameters preserve source-backed admission; no runtime discovery added |
| arcweft-lang-sema/src/env/nominal.rs | 1,666 | 1,720 | Accepted catalog remains the single environment record owner; shared schema access replaces the identity-only carrier |
| arcweft-lang-sema/src/final_analysis/analyzer/patterns.rs | 1,235 | 1,247 | Pattern source binding consumes raw field types and declaration coordinates |
| arcweft-lang-sema/src/final_analysis/analyzer/dialogue_line_plan.rs | 482 | 1,664 | Attached content discriminator admission; only error propagation changes in this continuation |
| arcweft-lang-sema/src/final_analysis/analyzer/evaluated_effects.rs | 850 | 1,889 | Evaluated-effect operand validation retains its existing authority and error ordering |
| arcweft-lang-sema/src/final_analysis/match_edges.rs | 1,259 | 1,461 | HIR-to-checked child edge enrichment; scope error remains structured in the existing edge error |
| arcweft-lang-sema/src/final_analysis/nominal_schema.rs | 2,357 | 2,908 | Final nominal seal owns coordinate-to-exact-field joins transactionally; no prepared identity side table remains |
| arcweft-lang-sema/src/final_analysis/prepared.rs | 957 | 1,576 | Prepared fact shells own declaration coordinates and logical owner terms; stable field rows move to the final seal |
| arcweft-lang-sema/src/final_analysis/validation.rs | 2,242 | 2,749 | Validation consumes the sealed payload's logical projection, without a second identity reconstruction |
| arcweft-lang-sema/src/ownership.rs | 2,188 | 2,266 | One bounded ownership traversal and evidence transaction; 299 embedded test LOC exercise the same boundary |
| arcweft-lang-sema/src/registration/model.rs | 1,396 | 2,104 | Registered type identities remain with their registration owner; 254 test LOC inspect the same role/identity contracts |
| arcweft-lang-sema/src/registration/registrar.rs | 1,843 | 1,929 | One accepted-world publication transaction propagates identity failures before publication |
| arcweft-lang-sema/src/types/digest.rs | 1,028 | 1,289 | One canonical typed encoder, with shared payload case composition; 123 test LOC remain identity tests |
| arcweft-lang-sema/src/types.rs | 1,330 | 1,710 | Existing type algebra/facade only exports the selected logical/sealed owners; no additional state table |

These are explicit cohesion dispositions for the touched responsibilities,
not approval of the remaining unscoped execution model. Larger inherited
content/callable algorithms still require their complete consumer migration
and the final structural review before the connected push cut.

### Required continuation

The remaining library errors are in callable joins, function-value schema
construction, and call source/projection completion. In particular,
`project_checked_evidence` still tries to mint a stable variant-owner ID from
an active projected type, and completed call projections still feed raw
`TypeKind` fields. Replacing these errors with `.ok()?`, copying only a
`ScopedTypeView::value()`, or rejecting every nonempty function binder would
hide the missing completion boundary.

Finish the scoped application/source algebra and scheme opening together
with effects, flat instance substitution, and callable-value execution.
Remove `TypeConstraintSolution::apply`, the old unscoped frozen application,
and instance layers when their complete replacement is in place. Owner-only
payload dependencies must be closed by that general authority, including
phantom arguments; do not recursively lower the entire owner as a payload's
physical carrier, which would cycle through Option/Result construction.

The [active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
now describes the current logical/sealed payload model and the remaining
source/runtime obligations. No returned design, complete native/AWBC
integration, restore proof, or overall goal completion is claimed. There is
no external blocker. Frozen packages and maintained language rules were not
changed by this continuation.
