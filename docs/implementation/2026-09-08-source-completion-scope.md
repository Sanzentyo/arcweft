# Source completion and binder scope — 2026-09-08

Status: IN_PROGRESS. Supersedes the latest current-state summary in
[generic scope migration](2026-09-08-generic-scope-migration.md), while preserving
its historical changes and validation. This remains part of the full
[convergence goal](2026-09-08-convergence-goal-plan.md).

Latest current-state evidence is in
[Observed source evidence](#continuation-observed-source-evidence) below.

The existing `main` checkout remains at accepted HEAD/origin/main
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. The inherited dirty tree is retained
and the index is empty. There has been no implementation commit or push.

## Changes established in the working copy

The common constraint shape now owns its binder projection. Reference mapping,
normalization, equality, and occurs checks enter the same function binder when
visiting children. Header validation rejects out-of-scope type and constant
references before a reflexive comparison can admit them. Array recovery or
inferred lengths also cannot establish checked equality. Selected-call
relation uses that header validation, including the identical-parameter path.

Payload relation now traverses the logical owner's type followed by its fields,
through the same typed children used for opening and normalization. A generic
parameter used only in a Result payload's owner can therefore participate in
inference. Physical field layout remains a separate projection.

`ClosedConstraintProbe` is no longer an alias of the probing representation.
The source owner under `types/constraints/transaction/source.rs` provides an
explicit active-to-closed transition. It projects the actual and expectation
under the selected type/constant bindings, derives the container header from
the projected actual, and projects semantic evidence before materialization.
The closed selector owns its expectation only in its checked variant, so an
unchecked source cannot independently retain a checked expectation. Original
template expectations and the provisional container copy are not retained.
Skipping or repeating the transition is a protocol error.

Source type and constant references must close against the candidate
application. Declaration-Free references retain their declaration ownership;
function-local quantifiers remain inside the function operand. Residual callee
quantification belongs to the continuation result. This is not a rule banning
scheme-valued operands. Their use-site opening and execution still require the
complete callable model described below.

The analyzer now reads an argument's inferred type from the same closed source
row that supplies its semantic selection. The automatic source-to-keyed-type
projection path, its trait hook, and the argument/receiver variants in the
explicit projection key algebra were removed. Explicit result, future, and
base-instantiation projections remain. Source value-coordinate uniqueness is
checked before the solved candidate is returned; it no longer depends on
duplicate keys in a copied type table. The obsolete test of that copied-key
inventory was deleted.

The domain's post-projection variant identity hook now receives an actual whose
application type/constant variables have been closed. This does not fix the
earlier source callback, which still attempts stable variant identity checks
while contextual types can be provisional. Those callback errors remain
visible in the compiler diagnostics.

## Tests added, not executed

- Five lower normalization tests cover nested type/constant binders, escaped
  references in equality and selected relation, occurs checks, scope restoration
  after a node-budget failure, and unresolved array lengths.
- One payload relation test requires inference of an owner-only type parameter
  through the lower relation and completed solution.
- Four source tests cover refusal of an unowned future reference, retention
  of a function operand's own type/constant binder, agreement of normalized
  actual/expectation/container evidence, and exactly-once completion.
- The native/AWBC matrix adds
  `later_argument_closes_an_earlier_contextual_variant`. It calls
  `choose(None, 42i64)` so the later argument must determine the earlier
  contextual constructor's type, and both engines must return `42`.

All of these tests are **NOT RUN**. No engine, snapshot, or semantic acceptance
is inferred from adding a fixture or from a partial compiler diagnostic list.

## Validation actually performed

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  latest **13 library diagnostics and 36 library-test diagnostics**, exit 101.
  Log: `target/generic-source-completion-migration.log`. Intermediate checks in
  this continuation failed at 15/39, 15/38, and 14/37. No test binary ran.
- `cargo fmt -p arcweft-lang-sema -p arcweft-compiler`,
  `cargo fmt --all -- --check`, and
  `git diff --check`: **PASSED**.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-source-completion-scope
  --fail-on-blocking`: **PASSED**, exit 0; 95 workspace packages, 2,215 Rust
  files, 311 review triggers, and 0 blocking violations. Generated
  [measurements](structure-audits/2026-09-08-source-completion-scope/file_metrics.csv),
  [dependency edges](structure-audits/2026-09-08-source-completion-scope/dependency_edges.csv),
  and [findings](structure-audits/2026-09-08-source-completion-scope/findings.md)
  are retained.
- Local Markdown file targets in this record, the previous migration record,
  and the correction request: **PASSED**, 25 checked, 0 missing. Anchors and
  semantic acceptance are outside this file-link check.
- Compiler/native/AWBC execution, workspace all-target/all-feature checks,
  Clippy, test-workspace, doctests, codec/golden, and Tier 2: **NOT RUN** while
  the semantic crate remains uncompilable. They remain required for the full
  connected cut.

## Ownership and structure review

Paths below are relative to `crates/arcweft-lang-sema/src/`. Base LOC is the
complete accepted HEAD file; growth includes inherited goal work. Exact
package ownership, bytes, classification, and embedded test LOC are retained
in the generated measurements.

| Owner | Base LOC | Current LOC | Current bytes | Disposition |
| --- | ---: | ---: | ---: | --- |
| types/constraints/context.rs | 1,282 | 1,530 | 55,895 | The existing scope/accounting context owns reference eligibility and header validation; no second reference registry |
| types/constraints/transaction.rs | 1,957 | 1,815 | 67,654 | Transaction orchestration retains frontier/materialization/solution publication and delegates the source state transition to its owner |
| types/constraints/transaction/source.rs | 0 | 339 | 11,912 | New cohesive owner of active/closed source state, projection, immutable access, and phase-specific equality; no public constructor for closed rows |
| final_analysis/analyzer/calls/constraints.rs | 4,241 | 4,933 | 201,839 | Existing domain/client owner retains source coordinates and mapper admission; copied source type projections were deleted rather than preserved as a fallback |

The new source owner contains no embedded tests; its 193-LOC test module
exercises the transition. The shared normalization owner is 896 LOC with a
separate 181-LOC test module. Shape/binding relation/reference mapping retain
their existing responsibilities. No dependency, Cargo feature, runtime I/O,
public facade contract, or additional resolver was introduced. The large
analyzer owner remains cohesive for this change: the edited state is the
same candidate/source transaction, and moving its helpers without moving that
authority would only widen internal APIs. Its broader inherited scope remains
subject to the full cut's structural review.

## Required continuation and limits of this evidence

The semantic crate still does not compile. Remaining production errors are in
callable joins, function-value binder admission, initial source variant
evidence, and the result projection that feeds the old unscoped callable
application. The source callback and all materialized branch facts must be
reconciled with the scoped completion authority. The contextual `None` case
must be verified without guessing how a child call retains its enclosing
candidate's unknown type or changing physical evaluation order.

`TypeConstraintSolution::apply`, `FrozenCallTypeSolution::instantiate_type`,
and `CheckedProjectFunctionInstanceSolution`'s layers remain. They must be
replaced with the complete scope-aware template application and flat instance
authority, not repaired by discarding a `ScopedTypeView`'s scope. Function
schemes must open through their legitimate quantifiers, without a fabricated
declaration ID or a blanket rejection of nonempty binders.

Effects still use the old issuer overlay. This continuation does not prove
that all effect variables or branch facts are free of active issuers. Nor does
it unify Runtime Function/ProjectContinuation values, close finite instance
discovery, or validate native/AWBC suspension and program-bound restore.

The [active correction request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
is updated to the current source states and contextual example. The remaining
design and implementation work is internal to the full goal, not an external
blocker. Frozen packages, contract versions, and maintained language rules
were not changed. This record is not a smaller completion contract or a
claim that the goal is achieved.

## Continuation: observed source evidence

Supersedes the preceding current-state counts and the earlier statement that
the source callback itself still hashes its provisional owner. The accepted
HEAD/origin/main is unchanged at
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`. Status at this continuation is
8 deleted, 641 modified, and 64 untracked entries; the index is empty. No
implementation commit or push has occurred.

`SemanticValueEvidence<Owner>` now owns one discriminator algebra with an
explicit owner representation. Its observation role retains `TypeKind`; its
checked role retains `SemanticTypeDigest`. The two role aliases are used by
different phases of the same contract, not alternative readers or legacy
formats. `ConstraintDomain` distinguishes `ObservedEvidence` from
`CheckedEvidence`, and the source completion transition is the conversion
between them. Materialization requests and final callable transcripts still
consume only checked evidence.

The analyzer's initial observation compares the actual against the variant
owner's typed projection and retains the resulting owner type. It no longer
requires a stable type digest while candidate variables are provisional.
The observed guard uses the typed owner projection; the checked guard retains
the stable identity comparison. Common discriminator matching and owner
projection are on the owning schema algebra. Completion projects the observed
discriminator with the normalized actual before issuing its checked identity.
The source carrier is named `AnalyzerCallObservedSource` to reflect its phase.

Pending equations no longer copy the source's selection, evidence, or
container header. Those unused fields and the resulting unnecessary domain
type parameter were removed; the source ordinal connects the equation to its
one source trace. Stable argument-slot joins and text-proxy discriminator
admission now propagate scope errors at their proper boundaries. One stale
test constructor was migrated to the declaration-Free generic constructor.

Two additional tests were added and are **NOT RUN**:

- an inference-bearing observation can match its typed guard without hashing;
  a direct stable projection rejects the inference reference, while a closed
  owner produces checked evidence accepted only by the matching guard;
- a lower source domain with genuinely different observed and checked Rust
  types closes actual/expectation and converts its evidence without requiring
  `Clone` on the observed domain value.

Validation actually performed:

- `cargo check -p arcweft-lang-sema --tests --message-format=short`: **FAILED**,
  latest **7 library diagnostics and 28 library-test diagnostics**, exit 101.
  Log: `target/generic-observed-evidence-migration.log`. Intermediate checks
  failed at 10/33 and 8/30. No tests ran.
- `cargo fmt -p arcweft-lang-sema`, `cargo fmt --all -- --check`, and
  `git diff --check`: **PASSED**.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/2026-09-08-observed-source-evidence
  --fail-on-blocking`: **PASSED**, exit 0; 95 packages, 2,215 Rust files,
  311 review triggers, 0 blocking violations. Retained generated
  [measurements](structure-audits/2026-09-08-observed-source-evidence/file_metrics.csv),
  [dependency graph](structure-audits/2026-09-08-observed-source-evidence/dependency_edges.csv),
  and [findings](structure-audits/2026-09-08-observed-source-evidence/findings.md).
- Compiler/native/AWBC execution, workspace all-target/all-feature checks,
  Clippy, test-workspace, doctests, codec/golden, and Tier 2: **NOT RUN**.
  They remain required for the full connected cut.

Touched structure triggers (paths relative to
`crates/arcweft-lang-sema/src/`; base is the full accepted HEAD file):

| Owner | Base LOC | Current LOC | Bytes | Embedded test LOC | Cohesion disposition |
| --- | ---: | ---: | ---: | ---: | --- |
| callable/schema.rs | 2,693 | 4,324 | 157,978 | 1,114 | The schema owns the discriminator algebra and guard semantics; phase roles share that algebra, without a separate matcher registry |
| callable/join.rs | 859 | 1,734 | 66,015 | 0 | Stable argument joins own scope failures; the unscoped instance methods remain explicitly pending replacement |
| callable/constraints.rs | 2,174 | 2,182 | 87,054 | 1,314 | Driver tests adopt the phase-specific associated type; no new driver state or callback protocol was added |
| callable/schema/families.rs | 2,334 | 2,598 | 99,699 | 540 | Existing family-schema tests retain declaration-Free parameter meaning |
| types/constraints/transaction.rs | 1,957 | 1,779 | 65,935 | 0 | Pending equations retain only equation data and source linkage; source evidence remains in the source owner |
| final_analysis/analyzer/calls/constraints.rs | 4,241 | 4,934 | 201,839 | 476 | Observation, guard, materialization comparison, and mapper admission remain in the existing candidate client; no new resolver or side table |

The separate source tests are now 291 LOC / 9,700 bytes. The callable facade
is 232 LOC and exports the common evidence algebra and appropriate phase
roles. No dependency or feature changed. The above files retain their named
responsibilities; physical extraction alone would not improve the ownership
of this phase conversion. The full cut still requires its wider structural
and runtime review.

Remaining library errors are in old instance/template substitution, scoped
result projection, and function-value binder handling. Initial variant-owner
hashing errors are resolved by the phase distinction, but that does not prove
all nested contextual inference or generic guard selection. In particular,
guard owner projection must be checked against the same candidate scope and
bindings as its value expectation; unopened declaration references must not
be silently confused with opened inference variables. Existing constructor
and function-value producers also require the full generic integration.

The old unscoped `apply`/`instantiate_type`, instance layers, effect issuer
overlay, and disjoint callable execution routes remain. No blanket scheme
rejection, scope-dropping conversion, runtime fallback, or completion waiver
was introduced. The active correction request remains open for those coupled
obligations; there is no external blocker and the full goal remains active.

The subsequent [flat instance and template projection record](2026-09-08-flat-instance-and-template-projection.md)
supersedes this paragraph's substitution/instance status. It records deletion
of the old application/layer routes, the new scoped projection roles, and the
remaining compilation failure; none of the earlier test results are promoted
to acceptance of that working copy.
