# Callable convergence: correlated sources and first-class function values

The [in-progress scope-owner implementation](2026-09-10-correlated-call-scope-owner.md)
records the path-owned lower boundary, current validation and the remaining
source-protocol, ranking and residual-scope decisions. It does not change this
model's proposed status or establish callable-component completion.

Date: 2026-09-10. Inspected existing `main`, clean, at
`54fae657e04f95688671fd2d8c0395266383858f`; `origin/main` was the same revision.
The preceding [conditional recovery cut](2026-09-10-conditional-recovery-validation.md)
is committed and pushed. This note records the next investigation and proposed
model. It is not an implementation or a decision-complete returned contract.
The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

## Scope and precedence

The governing correction is the [coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Its parent scope contract is useful evidence; its prohibition on specializing
an already published scheme is explicitly under reconsideration. That
prohibition cannot decide the result merely because a runtime route is absent.
The proposed model below admits rank-one specialization at a concrete value
use. It does not admit arbitrary let-generalization, inference of higher-rank
parameters, or binding a monotype unknown to an entire universal scheme.

The result must close the three contextual-constructor and four inferred-row
sema failures, ordinary and generic prefixes used as callbacks, distinct later
uses of a shared prefix, and their native/AWBC consumers. Accepted recursion,
source order, attached defaults, capture, suspension, finite instance discovery,
nominal payload ownership and program-bound restore remain mandatory.
CharacterDialogue value execution is a separate remaining parent obligation;
it is not waived by this model.

## Current-source contradictions

| Current owner | Observed behavior | Consequence |
| --- | --- | --- |
| `AnalyzerExpressionExpectation` | `Parametric` supplies a contextual shape and sorted outer unknowns, but `complete_type()` supplies no nested-call result equation | A child call must independently close even when later parent sources provide its missing result parameters |
| `SourceProbeResult` and `SourceProbeOutcome` | A probe returns an actual `TypeKind`, semantic branch and selected evidence, or rejects | There is no authority for a still-correlated nested application and its existential completion obligations |
| `ConstraintPath` | Owns bindings, effect constraints, pending equations, choice derivation and source trace | This remains the lower constraint authority; a second constructor-specific solver would duplicate it |
| `PreparedCallGraph` | Owns issuer-qualified selected/unselected call nodes, source-site membership and affine deltas | It preserves prepared application evidence, but currently does not retain unresolved nested applications as members of the parent's solve |
| `PreparedFunctionValueOriginQuery` | Returns one `PreparedFunctionValueOriginEvidence` with one producer and capture row | A specialization of a value with multiple possible origins needs complete origin evidence; one selected producer cannot stand for the set |
| `runtime_type_at` | Hashes each input as a standalone semantic type; Function parameters call `runtime_type` again, without their function binder | A valid bound descendant is rejected at empty lexical depth during runtime projection |
| `RuntimeTypeShape` and `RuntimePlanTypeProjection` | Function contains only parameter and result children; array length is concrete; no bound reference or incoming scope is represented | A closed function scheme cannot reach the current type graph faithfully |
| `RuntimeValue` | Function and ProjectContinuation are distinct payloads; the continuation owns immutable prefix values and a typed ABI but no ordinary callable activation | A prefix accepted as a callback cannot be repaired by widening value matching alone |

These are observations of the current sources, not conclusions drawn from
historical test names. The preceding cut's logs retain the existing failures.
The additional probes below expose the ordinary-call form of the same missing
constraint boundary; no production repair is claimed here.

## Proposed ownership and semantics

### One correlated application component

Keep semantic types in `TypeKind`, including Free, Bound and Inference
references. Free references remain rigid lexical evidence. A universal
function binder and a local existential unknown are different obligations.
An incompletely inferred constructor value cannot become a polymorphic value.

An ordinary source call contributes its opened signature, argument/result
relations and alternative decisions to the same active constraint component
as its consuming source. The existing call preparation gate authenticates
each child application against the prepared graph, schema, source and lexical
owner. The lower scope explicitly admits that child opening; copying an
inference reference into another scope is not admission.

The component's alternatives stay correlated. Each frontier path retains the
root and descendant selections together with their shared bindings. A nested
candidate is not accepted because it was visited first or because it works
without the parent's remaining equations. The complete component must have
one accepted semantic result before any constituent call is published.

For `combine(.Left(1i64), .Right("two"))`, the two constructor signatures
contribute `A = i64` and `B = String` to the common `Either<A, B>` relation.
Both calls then close as `Either<i64, String>`. Exchanging the physical source
order changes neither this solution nor the once-only evaluation rule.
`.Empty()` has an existential result parameter, not a special unit-case rule.

Source preparation, lower solving and final materialization remain distinct.
Pending source terms must not enter the existing checked-value carrier. The
winning solution closes every actual, expectation, source constructor,
generic/effect binding and selected branch before materializing semantic facts.
Existing affine fact and prepared-graph deltas cover the entire component.
Failures roll back semantic state while retaining physical work charges.

This replaces independent completion where a child depends on the parent's
active source constraints. It must not introduce whole-argument retry passes,
argument sorting, foreign-variable-as-rigid rules or a second source catalog.

### Function schemes and concrete uses

A published function scheme retains its binder, exact value origins, callable
position and immutable captures/prefix. Applying it or using it at a concrete
function type opens its quantifiers freshly. A successful concrete use retains
the exact specialization evidence; it does not mutate or consume the original
scheme. Sibling uses can choose different types, constants and effect rows.

Specialization is an operation of the existing callable/type preparation and
completion authority, not universal-subtyping implemented by a Boolean type
comparison. It must retain the source value, source binder, closed substitution,
possible origins, preserved captures/prefix, group position and target evidence.
Aliases, selected branches and aggregate/closure transport retain those exact
origin relationships. No consumer may adopt the first possible origin.

A monomorphic function parameter receives an executable monomorphic function
value. Passing a scheme first performs the checked concrete-use specialization.
A monotype inference variable cannot instead absorb the universal binder.
Internally closed schemes may remain in a directly known tuple or captured
value; this does not generalize an arbitrary local binding or create a source
`forall` parameter language.

### Function values and execution

Use one callable-value contract for ordinary functions, closures and retained
prefixes. Backend-specific bodies may keep their legitimate native/AWBC
representations. They must share the semantic distinction between an already
executable target and an unapplied/partially applied scheme with retained
values. Preserve `Unapplied` and exact completed group coordinates.

A nonterminal group still allocates no fabricated concrete FunctionSite.
Checked specialization produces a finite program-owned dispatch from every
admitted source origin to the appropriate closed target and capture/prefix
projection. Runtime execution checks the value against that sealed dispatch;
it does not discover a declaration, solve types, or populate a generic registry.
The target enters the existing same-fiber call/return/suspension authority.

Direct ProjectCall and continuation application retain their source-ordered
operand row and exact logical ABI materialization. Ordinary apply consumes the
same callable evidence. Prefix operands are evaluated once, specialization
executes no source operand, and an attached default runs only when its actual
terminal call omits the attached argument. No wrapper function or synchronous
expression fallback stands in for a missing call operation.

### Effect ownership

An omitted callback effect annotation in a declaration needs a declaration-
owned parameter inferred at applications, not an Unknown row published into
the final catalog. A local annotation's unknown is an existential obligation
of its checking component. A closure's body row must close before publishing
its actual function type; later catalog repair cannot leave stale earlier types.

The row model must retain unions of invoked callback dependencies and concrete
effects. Uninvoked callback rows remain in parameter/value types without being
added to the containing body's execution effects. Explicit contracts remain
upper bounds and visible callable contracts. Recursive body dependencies close
through one monotone row relation; no consumer replaces an unresolved row with
an empty set. Type, constant and effect quantifiers share the same opening,
scope transport, residual binder and final closure rules.

### Scoped runtime types and restore

The runtime type graph must carry its incoming scope on the owning type row,
and function binders, bound type/constant/effect references in the existing type
algebra. A nonempty binder changes the scope of its body children; an empty
binder does not add a depth. Every child edge validates the exact transition.
There is no separate scope side table or placeholder runtime value class.

Value/local/prefix/FunctionSite ABI roots require empty incoming scope.
A function root can be closed while its body contains correctly bound slots.
Its scoped descendants cannot be reused as standalone executable ABI roots.
Template identity and nominal payload ownership must remain exact under this
projection, including nested Option/Result/project nominal arguments.

Native and AWBC consume the same admitted scheme, specialization and closed
target. Version-1 codecs and the existing program-bound restore validators
retain callable origin, position, type and captured values. They reject foreign
programs, absent targets, invalid scope edges and malformed capture/prefix
evidence before execution. No active inference issuer enters stable bytes.

### Discovery and budgets

Every admitted concrete use contributes all of its possible closed targets to
the existing deterministic instance worklist. Same keys must have equal closed
inputs. Self/mutual recursion uses that worklist without recursive rediscovery.
Specialization cannot trigger new runtime discovery.

Retain the existing typed inclusive instance, edge, structural-node, depth and
work limits. Correlated source openings, equations, alternatives and replay
must charge the existing lower/call work session before allocation or publication.
Suspending a source obligation cannot refund work or start a fresh ledger.
No new configurable limit or raised production maximum is proposed here.

## Remaining contract review before implementation

The semantic direction above is not sufficient to publish a new API. Close
these exact ownership questions against the existing code and parent contracts:

1. The consuming move-only protocol that allows the lower source driver to
   admit child openings/constraints while the analyzer retains the sole source
   and prepared-graph mutation authority; exact frontier/choice, candidate-rank
   and replay joins. Current ranks compare exact matches, declared exact matches,
   unchecked/open supplies, omitted parameters and standard/adapter authority.
   Nesting must not add a source-order preference between inference solutions.
2. The final callable-value and specialization fields in native, AWBC and
   restore, including multiple origins and all remaining group positions.
3. The owning scope for implicit effect parameters, the row-union constraint
   representation and completion, and every source-type/ABI consumer of it.
4. Exact scoped runtime graph and nominal payload admission, including stable
   identity projection and every existing ordinary-root consumer.
5. The boundary between a report retaining rejected calls for tooling and the
   complete selected program admitted to executable projection. An empty
   diagnostic list does not prove that every required call has an application.
   The [expression-plan follow-up](2026-09-10-call-execution-admission.md)
   removes the structural execution plan from unavailable calls and makes the
   existing execution projection reject them. Correlated inference and full
   diagnostic publication remain part of this request.

The existing coupled request owns these unresolved decisions. No external
blocker, new compatibility exception or reduction of its acceptance matrix is
being claimed. Implementation must replace the old paths through their actual
producers and consumers; helper-only tests or one passing constructor example
cannot complete this boundary.

## Inspected evidence

The parent scope archive remains byte-identical: 45,039 bytes, SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Its full contract, Rust shapes, acceptance/validator matrices, consumer inventory
and implementation order were read alongside the current source owners above.
The coupled request, parent scope request, ProjectCall request, and maintained
functions/pipeline and types/effects chapters were also read. The larger
call-application amendment and ProjectCall implementation contract still require
the protocol review described above; neither archive nor extracted mirror was
edited.

## Additional executable acceptance probes

Two source families were added to both `generic_calls.rs` and the compiler's
native/AWBC `callable_execution.rs` matrix:

```arcw
fn empty<T>() -> Option<T> { None }
fn fallback<T>(input: Option<T>, value: T) -> T { value }
flow main() -> i64 { return fallback(empty(), 42i64) }
```

```arcw
enum Either<A, B> { Left A, Right B }
fn left<A, B>(value: A) -> Either<A, B> { .Left(value) }
fn right<A, B>(value: B) -> Either<A, B> { .Right(value) }
fn combine<A, B>(left: Either<A, B>, right: Either<A, B>) -> i64 { 42i64 }
flow main() -> i64 {
    let first = combine(left(1i64), right("two"))
    let second = combine(right("two"), left(1i64))
    return first + second
}
```

These use ordinary project-function signatures at the parent boundary. The
sema tests require each call's selected application and expression execution
authority to agree, and both ordinary child results to be `Either<i64, String>`.
The compiler tests require returns of `42` and `84` respectively through each
existing execution harness. They are positive acceptance tests, currently red;
they do not redefine the requested behavior as an expected rejection.

| Performed command | Observed result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib --all-features correlated_ -- --nocapture` | 0 passed, 2 failed, 770 filtered out. Analysis returned a report, but each selected-call assertion found a call with no selected application and an empty callable-diagnostic list |
| `cargo test -p arcweft-compiler --test callable_execution --all-features correlated_ -- --nocapture` | 0 passed, 4 failed, 72 filtered out. The `empty()` pair stopped at `compiler.runtime_reachability.invalid_edge` for a Call with structural runtime projection; the complementary pair stopped at type checking with no admissible final expression type |

The two commands ran sequentially with normal Cargo concurrency, taking
79.68 s combined including compilation. Neither native nor AWBC reached
execution. Logs are under
`.arcweft-local/validation/2026-09-10-correlated-callable-sources/`.
The previous seven sema and 18 compiler failures were not rerun in this probe.
No production Rust changed; this adds executable requirements and records the
current failure phases before the complete correlated-call migration.

`probe-review-gates.log` records passing `cargo fmt --all -- --check`,
`cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features`
and `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking`.
The sequential review took 64.54 s. Existing Clippy warnings remain. The audit
found 95 packages, 2,258 Rust files, 310 review triggers and zero blocking
violations; no generated reports were retained for this test-only change.

The two changed test owners remain below their review thresholds:
`generic_calls.rs` is 373 physical LOC / 12,382 bytes; `callable_execution.rs`
is 616 physical LOC / 15,769 bytes. They use the existing semantic fixture and
execution harnesses. No production owner, public API, dependency, feature,
contract version or I/O boundary changed. No touched structural trigger needs
a new decomposition decision.

Documentation review resolved 25 relative targets, and `git diff --check`
passed. ZIP re-enumeration found 71 archives / 4,802,433 bytes, with no inbox
archive and no path, size or SHA-256 difference from the preceding cut. No new
archive was adjudicated. Workspace tests, whole-workspace check/Clippy,
doctests and Tier 2 were not repeated for these added regression probes; this
does not change the outstanding full-goal validation requirements. The failing
positive tests remain enabled and are not counted as passing execution evidence.

## Application model investigation at the next accepted cut

Inspected on 2026-09-10 at clean `main`,
`22c5a64ef15e1561d5ac4177015a371efafa4605`, matching `origin/main`. The subsequent
test additions below are dirty changes at that base. The
[execution-plan admission cut](2026-09-10-call-execution-admission.md) is complete;
the coupled model remains under review and is not `READY_FOR_IMPLEMENTATION`.

### Consequences of the current owners

`CandidateConstraintDriver` exclusively owns its lower transaction, the mutable
work session and the analyzer callback client. `with_callback` exposes the
client and work session while the lower `ProbeTicket` only lends an expected
hint. `SourceProbeResult` contains an actual type, semantic branch and selected
evidence. `TypeConstraintParameterScope` contains one `OpenedGenericScope`.
These are actual ownership constraints on the pending-source protocol, not
just missing expected-type arguments. A child contribution must be admitted
with its originating source, scope and affine checkpoint evidence before the
parent lower can consume its equations. Passing another mutable solver into a
callback or accepting arbitrary foreign inference IDs would evade that join.

The protocol must also represent a source whose callee or member lookup needs
a type head learned from another source. Its continuation must own typed
preparation state and wait for lower-issued evidence, then resume that state.
It cannot restart a whole authored argument. Callback resumption still charges
actual physical work; no pending contribution is an accepted checked value,
an executable expression or permission to retain an analyzer checkpoint open
across unrelated callbacks. Existing source close, fatal ordering and
materialization transactions remain required.

`select_prepared_candidates` ranks accepted candidates by exact matches,
declared exact matches, unchecked/open supplies, omitted parameters and
standard/adapter authority. That rank belongs to a call choice. The completed
component must preserve those local choices and their correlations. Summing
the ranks of different calls or comparing independent child choices in source
order changes the result. The precise component preference/ambiguity rule and
its consuming lower/driver API still need closure; the existing root-candidate
score is not itself that rule.

`RuntimeProjectCallPlan` owns its caller's callee expression and physical
operand expressions as well as logical materialization, attached/default
stage and Continue/Invoke outcome. `RuntimeProjectCallSiteTable` additionally
owns the caller's result pattern. Neither is a reusable callable value: moving
one of those site IDs into a value would retain another caller's evaluation
and destination. `RuntimeFunctionValue` currently names a concrete structured
site or AWBC function, while `RuntimeProjectContinuation` has lineage,
remaining-function type and prefix values but no ordinary application plan.

The selected runtime ownership requirement is to separate caller evaluation
and result destination from reusable callable application. The application
authority must retain current group, exact retained-value ABI,
current-logical input ABI, optional default stage and Continue/Invoke outcome.
Direct ProjectCall and function-value application must consume that same
authority. A nonterminal application returns a new immutable value at the next
group; a terminal one enters the existing same-fiber function frame. A generic
scheme receives its closed application target through checked specialization.
It cannot reserve an open or synthetic FunctionSite merely to fit the current
value payload. This establishes the required split; exact final Rust fields,
program-owned identities and all backend/restore joins remain decision 2 above.

`EffectRow` currently contains a concrete set plus one `EffectRowTail`, and its
constraint environment stores one-variable covered edges and lower/upper
bounds. `GenericEffectReference::{Free, Bound, Inference}` already exists, but
that namespace is not the row's variable owner. Replacing `Unknown` with an
empty set or adding a second callback variable does not express a union of
independent dependencies. The final row algebra must define both unions and
their inclusion constraints. In particular, a required effect below a union
of multiple unknown rows does not identify which row receives it; allocation
by iteration order is invalid. Declaration-owned implicit quantifiers, local
existentials, inherited bindings and residual binder constraints must be
reconciled before this representation is migrated.

`RuntimeTypeSchema` is a persistence/data schema, including `Named(String)`;
it is not a representation of abstract function-body type terms. The current
runtime type projection's nominal branch requires a concrete nominal/layout
pair, while its Function branch omits both binder and effect row. Scoped
function descendants therefore need exact nominal application and payload
evidence without inventing a concrete persistent layout. This must be closed
against the accepted nominal graph and its real runtime/data owners; neither
discarding the binder nor extending the persistence schema with another
ad hoc type universe is selected. The existing semantic type encoder already
commits incoming binders, so its scoped identity authority should be preserved.

The full 708-line ProjectCall `FINAL_CONTRACT.md` was read at this stage, as
were amendment sections 3.2 and the remaining 3.5 callback protocol. Their
source-order, default, return-frame, same-fiber suspension and program-bound
restore requirements are retained. The accepted Rust nominal gap review was
also compared with the current core schema. This is additional reconciliation,
not a claim that the remaining shapes/matrices or the full amendment are closed.

### Nonterminal callback execution probe

The added source has no generic or inferred effect parameter:

```arcw
fn sum(first: i64)(second: i64)(third: i64) -> i64 { first + second + third }
fn advance(handler: i64 -> (i64 -> i64 effects {}) effects {}, value: i64) -> (i64 -> i64 effects {}) {
    handler(value)
}
flow main() -> i64 {
    let prefix = sum(1i64)
    let left = advance(prefix, 20i64)
    let right = advance(prefix, 30i64)
    return left(21i64) + right(11i64)
}
```

Both uses retain the same initial prefix and produce different second-group
values; each final application must return 42, for a total of 84. The sema
test requires all six applications and their expression plans to agree. The
paired engine tests require the real returns through their existing native
and AWBC harnesses.

| Performed command | Result |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib --all-features final_analysis::tests::callable_values::callback_returns_a_nonterminal_prefix -- --exact` | Passed: 1 test, 773 filtered; all six selected applications retain executable plans |
| `cargo test -p arcweft-compiler --test callable_execution --all-features callback_returns_a_nonterminal_prefix -- --nocapture` | Failed: 0 passed / 2 failed, 76 filtered; native reports an expected runtime function but receives `project-continuation/1`; AWBC traps with `TypeMismatch` at function application for the same value |

Both engine cases reach actual execution, unlike the earlier ordinary
correlated-source probes. The two commands ran sequentially with normal Cargo
concurrency, taking 29.54 s including compilation. Logs are in
`.arcweft-local/validation/2026-09-10-callable-application-model/`.
An initial test insertion was inside another fixture's raw source string;
formatter rejected the Rust before any test ran. The insertion was corrected,
and the reported commands ran against the valid final test file.

No production Rust is changed by this probe. The new positive engine tests
remain enabled. The previous 22 failing callable cases and nine sema failures
were not rerun by these exact commands, and these results do not establish
completion of the coupled request or any later convergence stage.

The test-only review passed `cargo fmt --all -- --check`,
`cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features`
and `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking`
in 29.58 s combined. Existing Clippy warnings remain. The structural audit
reports 95 packages, 2,260 Rust files, 310 review triggers and zero blocking
violations. No generated structural reports were retained. The changed test
owners are `callable_execution.rs` at 634 LOC / 16,248 bytes (base 616 LOC)
and `callable_values.rs` at 241 LOC / 7,495 bytes (base 207 LOC). Both keep the
existing semantic/execution harness and remain below their review triggers;
no production owner, public API, dependency, feature or facade changed.

`source-inputs.json` in this stage's local validation directory records paths,
byte lengths and SHA-256 for 15 inspected production inputs (812,589 bytes).
ZIP re-enumeration again found 71 archives / 4,802,433 bytes, with no path,
size or SHA-256 difference from the preceding admission cut. No frozen
package was edited. Whole-workspace tests/check/Clippy, doctests and Tier 2
were not repeated for these test and investigation changes; their immediately
preceding production results remain in the admission record, including all
known failures and unexecuted later tiers.

Documentation review resolved all 27 relative links in the changed investigation
and request. `git diff --check` passed. This is a test/evidence cut; it records
the reusable-application boundary and leaves the coupled model's open decisions
explicit rather than publishing an incomplete implementation contract.

## Curried effects and ABI implementation follow-up

The [curried application cut](2026-09-10-curried-application-effects-and-abi.md)
starts from clean `main` at `2a466cbcdcea89191b9fe90d9914446322e1ced1`.
It fixes the shared remaining-group function-type projection, constrains known
declaration effects in the existing lower transaction, and replaces the native
and AWBC checks that incorrectly required a new prefix to retain the input's
lineage and function type. The three-group effectful program reaches its real
return through both backends. Its complete validation and ownership review
are recorded in that note.

The full ProjectCall `RUST_SHAPES.md` (934 lines / 37,309 bytes) has now been
read alongside its previously read contract, acceptance matrix and consumer
inventory. Those shapes confirm that the call fact owns the new prefix ABI,
while the input checks its own exact expected lineage. They do not authorize
reusing a caller's result pattern or resume site inside a first-class function
value. The coupled source protocol, implicit effect-row algebra, scheme
specialization and reusable activation questions above remain open.

## Application arity adjudication

The [application-group follow-up](2026-09-10-function-application-arity.md)
starts from clean `main` at `73f8283ceb6dc5bb1b92f557de21e6ccfbe81a08`.
One Apply consumes the current function arrow's arguments; excess values do
not automatically apply the returned function. This agrees with the existing
typed runtime-plan constructor and AWBC behavior and preserves the maintained
call-group distinction. The follow-up removes the contradictory native
expression/pure/flow paths and their post-return argument payload. Prefix
binding and separate later applications retain their existing semantics.
This rule is selected for the final callable contract; the whole coupled
contract remains proposed.

The source probe also pinpoints remaining decision 5. The analyzer's
`publish_recovery_call` publishes rejected/ambiguous facts with an empty
diagnostic vector. `FinalSemanticAnalysis::call_diagnostics` merely borrows
those vectors, and signature-help projection consumes the same empty rows.
For the excess-argument source, the compiler therefore rejects later with
`compiler.runtime_reachability.missing_selected_call_authority` rather than
reporting a source-backed callable diagnostic during semantic admission.
The direct sema test requires the rejected one-parameter candidate and absent
execution plan; no diagnostic-stage claim is inferred from compilation failure.
The remaining repair must publish bounded, source-backed diagnostics from the
one final call outcome and connect compiler/tooling consumers, without making
an unselected call executable or copying diagnostics into a peer call catalog.

The full [call-application authority amendment](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2.1-final-semantic-owner-construction-and-seal-correction/CALL_APPLICATION_AUTHORITY_AMENDMENT.md)
is now read: 3,417 lines / 171,746 bytes, SHA-256
`bc62047619793e971402963e358562b1c325c118bc99a37ddf4da8e11312d88b`.
Its candidate/source/materialization error precedence, affine publication and
acyclic identity obligations remain inputs to the coupled model. The older
rigid-only constant rules and static source-spelling/privacy gates do not
override the later generic-scope contract or current repository validation
policy. Reading the complete amendment is not implementation acceptance.

## Final call diagnostic authority

The [final call diagnostic follow-up](2026-09-10-final-call-diagnostics.md)
starts from clean `main` at `d7b8d5f2c1fabb92ab3ac2544eed8f8a53ae15ab`.
The selected ownership rule is that `CallTargetFacts` derives its mandatory
diagnostic from the final outcome and exact final-HIR source. Prepared
candidate, detached and intermediate seal records do not own independently
supplied diagnostic vectors. The final outcome remains the semantic authority;
its diagnostic is a bounded source projection, not a second call catalog.
The compiler checks these errors before verifier/runtime projection, using
the shared source diagnostic also consumed by CLI and LSP.

Final non-callable evidence follows the same unavailable-result rule as
rejected and ambiguous outcomes. A zero-candidate call still checks and retains
its authored argument sources in its existing fact transaction, without a
candidate probe or an invented schema. Source failures propagate, and a
poisoned type does not become an unknown-call diagnostic. This does not relax
the selected-only execution-plan boundary or the correlated materialization
failure order.

At that cut, the semantic signature API could project rejected/ambiguous
diagnostics, but interactive acquisition still required an executable. The
[semantic project lease follow-up](2026-09-10-semantic-project-lease.md), based
on `f171b4bfcd04b17da51876341a2b511bd06e20f4`, closes that lifetime gap with one
compiler-owned immutable analysis lease. It owns the exact HIR/source/symbol
ancestor, registered world, assertion profile, final report and semantic index.
`CompiledProject` owns this lease plus its executable products; a single phase
enum retains the latest completed HIR, analysis or compiled product.

The LSP accepted snapshot owns that phase lease. Signature acquisition and
semantic-only features use completed analysis after a call diagnostic or
later executable admission failure, with existing source/profile/stamp checks.
Failed analysis construction retains only HIR, and failed compilation does
not flush persistent compile stores. Separate accepted executable fields and
copied world/revision checks are removed because the ancestor relationship is
owned by the lease itself; source/URI/overlay validation remains.

This selects and implements the semantic-phase ownership part of decision 5.
It introduces no analyzer rerun, mutable partial report or peer semantic
catalog. The coupled contract as a whole remains proposed; complete-program
activation, the other open decisions and positive acceptance failures remain
required.
