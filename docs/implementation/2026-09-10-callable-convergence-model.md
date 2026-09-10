# Callable convergence: correlated sources and first-class function values

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
