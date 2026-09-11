# Callable component design decisions

Date: 2026-09-11. Inspected `main` and `origin/main` at
`55633925b72706132ae3cf4c4287d8645be31a68`, with an empty index and 28 Rust
working-copy paths. This is a design development record for the
[coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
It is not a returned package, implementation acceptance, or a readiness award.
The [full convergence goal](2026-09-08-convergence-goal-plan.md) remains active.

The decisions below replace the corresponding tentative choices in the
[2026-09-10 model](2026-09-10-callable-convergence-model.md). The accepted
borrowed constraint driver, physical operand order, attached-default ownership,
and same-fiber invocation remain foundations. No second solver, callable
catalog, runtime generic registry, or snapshot owner is introduced.

## 1. Correlated applications

`PreparedCallGraph` owns applications and their prepared source identities.
`ConstraintPath` owns the admitted application scopes, bindings, row relations,
pending equations and choices. Analyzer fact transactions own prepared
expression/statement facts. These are three responsibilities, not three copies
of an expression tree. HIR remains the expression topology authority.

A source contribution contains alternatives, each with its own actual term,
source evidence and path. The present ticket with multiple paths but one shared
actual/evidence result is replaced. A child application contributes its opened
signature and equations to the parent's component; it is not independently
completed before its result is related to the parent.

The callable preparation gate authenticates a child against its source site,
parent source ticket, schema and graph generation. It issues the affine child
initialization consumed by the existing lower context. The child gets a fresh
opening in that same context and path inventory. Admission cannot be simulated
by copying an inference reference or borrowing the accountant into a fresh
context. Existing node, branch, source and work counters remain accumulated.

Prepared source computations expose typed result ports. A pending result is
not a `CheckedExpression`, `TypeKind::Error`, empty effect set, or successful
source receipt. Computations waiting for an enum head, member owner or other
type premise are activated by their dependencies. They do not repeat the whole
argument or change physical argument order. Source expressions and selected
callee classifications are prepared under the existing affine fact delta.

Component closure performs these operations in order:

1. Saturate currently available type, constant, effect and source obligations.
2. Reject inconsistent alternatives while preserving abort precedence and all
   consumed work. A nested rejection rejects that containing alternative.
3. Close all demands for values evaluated by this application, recursively
   including child actuals, captures, prefix ABI and selected source evidence.
4. Compare completed candidate choices; retain the unique semantic result.
5. Reify remaining future quantifiers, eliminate child existential openings,
   and close every source projection with the same winning solution.
6. Materialize the component's facts and applications into one transaction.
   Commit only after the complete replay/receipt and final-seal checks succeed.

Candidate ordering preserves the existing local rank. Compare outer candidate
ranks first. Equal ranks for distinct outer declarations remain an overload
ambiguity; a cheaper nested overload cannot choose an outer declaration.
Within the same outer choice, independent child choices use product ordering:
one derivation dominates another only when it is no worse at every common
child decision and better at at least one. An improvement in one child cannot
compensate for a worse choice in another. Compare descendants recursively after
their common ancestor choice. Equivalent completed semantic evidence coalesces;
different incomparable completed evidence is ambiguous. HIR source order and
live opening counters never break a tie.

Result-only variables of ordinary child calls are existential. They must close
before any constructed value is published. They are not generalized into a
polymorphic `Option`, enum or record. Future quantification is available only
for an actual remaining callable scheme. A variable needed by a currently
evaluated value is a current demand even if its declaration first-use label
names a later group. Demand propagation follows aliases and all child scopes.

## 2. Schemes and value uses

A known, closed rank-one function scheme is a value type. Aliases, tuples,
nominal containers and captures may retain it intact. A generic identity may
transport that already known scheme as a type argument; this is explicit
support for substitution of an existing closed scheme, not inference of a new
universal binder. It corrects the proposed blanket prohibition on a monotype
slot receiving any scheme. The type relation still cannot invent a quantified
type to solve a hole or infer a higher-rank callback parameter.

Applying a scheme or checking it against a concrete function type opens its
outer binder freshly through the callable application authority. The source
scheme remains unchanged. Two uses can instantiate types, lengths and effects
independently. A context already requiring a known scheme checks alpha-equivalent
binders and constraints rigidly; it does not open the target binder as unknowns.
Arbitrary let-generalization and source `forall` syntax remain absent.

The same structural fold performs opening, shifting, substitution, free-use
collection and closure for all type constructors, array lengths and effect
formulas. Empty binders do not increase lexical depth. Inserting a caller-owned
replacement is one substitution, not recursively applying the callee's keys
inside that replacement. Free references remain rigid and generation-owned.

Residual origins are kind-separated and ordered by the existing schema slot
inventory. Equivalent unresolved roots share one residual slot and retain all
origin links. Remove unreachable slots. Retain effect predicates with their
binder; dropping a predicate can change which later applications are accepted.
The result contains no active opening, inference issuer or substitution history.

Local assignment follows the maintained local mutability rule: immutable
bindings cannot be assigned; mutable bindings have a fixed established type.
A mutable place may receive independently specialized values of that type.
Origin flow joins all assignments to the place. Bare locals and projected
fields belong to one typed place authority; a callback-specific assignment
variant or bypass is not an acceptable implementation.

## 3. Effect sets, predicates and completion

Each omitted row in a declaration's input function types introduces an
independent declaration-owned effect parameter at its typed schema position.
Omitted result rows, local annotations and closure body rows are inferred
results of their checking component. They do not automatically introduce
universal parameters. An explicit row remains the exposed callable contract
and bounds the body's actual effects. A declaration may infer constraints on
its implicit input effect parameters from its body.

Effect rows are finite sets. Their symbolic algebra must express unions,
intersection and difference of independently scoped row references. A single
tail, or a union of variables without retained constraints, is insufficient.
For example, the least solution of `p subset q union e` is `e = p minus q`.
Merging `p` and `q` would lose the type of either callback when it escapes.

Use one canonical membership algebra in `effect_row`:

- A reduced ordered Boolean decision DAG describes membership for one effect
  atom, with `GenericEffectReference` as its decision variables.
- A row stores a default membership root plus sorted concrete-effect overrides.
  An override equal to the default is omitted. Literal effects select their
  own override; different effect labels are not independent Boolean variables.
- The default row root evaluates to false when all row references are empty.
  Together with finitely many overrides, this guarantees a finite result for
  finite input rows. Unbounded complement is not a valid standalone row.
- A predicate uses the same membership algebra but means a relation that must
  hold for every effect label. It may have a true default. A subset predicate
  is `not actual or permitted`; equality is the conjunction in both directions.
  A default predicate false at the all-empty valuation is globally impossible:
  finite inputs are all empty at infinitely many unmentioned labels. It must
  not be satisfied by inventing an infinite existential effect set. False
  explicit label classes also reject; retain their labels for exact diagnostics.
- DAG nodes have ordered references, unequal children and unique triples.
  Stable encoding rebuilds deterministic child-first indices after scope
  normalization; private allocation IDs never enter equality or stable bytes.

`GenericBinder` remains the arity/scope descriptor. Function schemes and callable
schemas additionally own their effect predicate. Predicates participate in the
same binder fold, residual reachability, identity and runtime type projection
as their parameter/result types. This replaces the issuer overlay as the
semantic row authority; the old scalar row/substitution route is deleted during
the migration, not retained as a second implementation.

For current existential rows `y`, rigid/future rows `x`, givens `G(x)` and the
component relation `R(x,y)`, completion is exact:

1. Existentially eliminate `y` from `R` and require
   `G implies exists y. R`. At a declaration boundary, the derived admissibility
   predicate is retained with its implicit input parameters instead of silently
   narrowing an explicit rigid type parameter.
2. For each `y_i`, form `m_i(x) = not exists y. (R and not y_i)` on the
   admissible domain. Set it to false outside that domain.
3. Substitute all `m_i` simultaneously and prove `R(x,m(x))` under `G`.
   Success establishes the unique least solution in pointwise set inclusion.
   If the intersection of solutions is not itself a solution, current inference
   is ambiguous. Never allocate an effect to whichever variable was visited first.
4. Variables exclusively needed by a future callable remain quantified with
   the projected predicate. Current existential obligations cannot be postponed
   merely because the relation has multiple solutions.

The default label class and each explicit label class are solved separately
through the same DAG operations. This avoids enumerating the power set of
concrete effect labels. Relation saturation, quantification, substitution and
canonicalization all charge the existing structural/work budgets before growth.

Declaration body equations and recursive callable dependencies belong to this
same effect owner. Their least fixed point joins effects of executed operations;
it does not treat a legitimate recursive dependency as a cyclic exact
substitution. Merely creating or returning a function does not execute its row.
Uninvoked callbacks remain latent. A returned callback retains its own row,
even when another callback's row also contributes to the enclosing invocation.
No provisional empty row is published while a closure/body dependency is pending.

Overload selection cannot invent effect-dependent runtime dispatch. Compare
candidate alternatives only after their symbolic constraints are retained.
Alternatives with distinct inferred admissibility domains cannot resolve a tie
by choosing a narrower domain. A unique static candidate may retain its input
predicate; otherwise ordinary ambiguity is reported.

## 4. Callable values and executable instances

Replace the disjoint Function/ProjectContinuation runtime payloads with one
private callable value: an owning program reference, a program-local callable
state ID, and an immutable row of retained runtime values. Backend program
references use the existing native plan or verified AWBC program. The program
owns state definitions; a state does not own its program, avoiding an Arc cycle.

A state definition owns its semantic function/scheme type, origin, application
position, and exact lexical-capture/bound-parameter layout. Position is
Unapplied, WithinGroup with exact bound formal coordinates, or AfterGroup.
Counts alone do not identify a named/receiver/rest prefix. A nonterminal state
has no fabricated executable FunctionSite.

The operations are:

- MakeCallable: capture values once according to the checked capture plan and
  create an admitted initial state.
- SpecializeCallable: consume checked specialization evidence, select the
  source state's sealed dispatch row, and preserve/retype retained values as
  proven by its layout projection. This performs no source evaluation.
- ApplyGroup: evaluate the callee and physical operands in the established
  order, validate the complete dispatch/argument row, then either retain a
  partial state or enter the existing same-fiber FunctionSite invocation.

Specialization evidence is a checked application product linking the source
scheme and complete origin relation, substitution, position, retained-value
projection and resulting states. Runtime consumers cannot discover a target
from a value or declaration name. Direct ProjectCall keeps its source operand
and logical ABI evidence but lowers to the same state transition/invocation
authority. A source call site, with operand expressions and result pattern,
is not the reusable plan stored inside a callable value.

One dispatch can map different origins to retain or invoke transitions when
their exposed function types agree. This is necessary for callbacks returning
another prefix and values joined from different producers. Validate the input
state, generation, types, argument count and ownership before mutating a frame
or environment. Preserve current capture move/copy/borrow admission; immutable
storage does not make an affine capture freely duplicable.

An omitted attached default executes only at its terminal invocation. Returns,
fallthrough rejection, suspension, cancellation, nonlocal control and cleanup
use existing call frames. Empty effects do not imply expression-only execution.

Origin flow is a finite, sound relation over declaration/closure producers,
state positions, bindings, aggregate fields and branch alternatives. Preserve
alias and product correlations; do not choose the first origin or identify a
value solely by its callable type. Recursive flow uses graph references, not
unbounded expansion of capture histories. Every admitted possible origin must
have a valid specialization/transition.

The existing deterministic instance worklist owns closed targets and origin
propagation. Discovery reaches a fixed point before publication. Instance keys
retain the existing flat, closed semantic substitution and ABI; origin demand
propagation is not an ever-growing recursive capture-history key. Same-key
inputs must agree. Root/edge membership and all discovered dispatch targets
are sealed before materialization, which cannot add a target. Preserve the
current inclusive instance/edge/node/depth/work limits and cancellation owner.

## 5. Scoped types, bytes and restore

Runtime type rows own their incoming lexical scope. Function shapes carry
binder arities, effect predicate, invocation row and parameter/result edges.
Bound type, length and effect references use checked depth/kind/slot coordinates.
Every child edge proves its scope transition. Executable local/capture/prefix
and FunctionSite ABI roots require empty incoming scope, but can contain a
closed nested function scheme. No scoped descendant is reused as a root.

Nominal logical transformation keeps the owner type and payload field types
together. Stable case/field identities are issued after owner closure. Runtime
projection uses the existing nominal catalog and scoped fold; neither digest
reconstruction nor a second nominal registry is allowed.

Version-1 semantic encoding includes binder arities, canonical effect DAGs and
predicates, scoped type edges, callable state layouts and transition evidence.
Counts, indices and scope coordinates use canonical bounded integer encoding.
Active issuers are rejected. Program identities include these tables without
embedding the program's own digest in a table that determines that digest.

A dormant callable snapshot contains its state ID and retained-value snapshots
inside the existing generation-bound program snapshot. It is inert data until
the existing restore transaction resolves the exact program and validates the
state, closed root type, every capture/layout/ownership relation and all nested
values. Context-free decoding cannot construct a live callable. Native and
AWBC use the same semantic state evidence and their respective verified code
references. Delete the old Function and ProjectContinuation snapshot variants,
constructors and readers when this replacement is connected.

## Work and validation state

This record selects the domain model for continued implementation. It does
not claim that consumer migration, a complete returned archive, or independent
readiness review has occurred. In particular, the current Rust tree still uses
the scalar effect row and disjoint callable values. The existing 28 Rust paths
are preserved; no new Rust validation result is attributed to this note.

The next implementation stages follow the coupled request: the type/effect
algebra and correlated source owner; final applications and specialization;
instance/state discovery; native/AWBC execution and program-bound persistence;
old-path deletion; and the complete required validation matrix. These stages
are one obligation, not separately completed feature subsets.

Current-source correction: `CallAnalysisOutcome::seal_diagnostics` already
issues `NoViableSignature` for rejected calls, and the compiler checks these
diagnostics before verification/lowering. `FinalAnalysisExecutionProjection::plan`
also rejects an unavailable call. The latest restored-WIP log confirms those
diagnostics for both ordinary correlated-call failures. Older observations of
empty diagnostics are historical; they do not justify another admission layer.
