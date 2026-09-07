# Final contract

## 1. Selected authority and scope

Declaration identities identify declarations. They do not identify the unknowns
of a new call. Preserve GenericTypeParameterId and GenericConstParameterId as
declaration-relative IDs. Introduce typed references with three disjoint cases:
Free(declaration), Bound(depth, slot), and Inference(application issuer, slot).
Type, const-length and effect-row references have distinct slot types.

TypeKind remains the sole semantic type algebra. Its generic reference payload,
ArrayLength and EffectRow gain the reference distinction; do not create a
parallel solver type tree. Function types own an explicit GenericBinder.
The callable signature schema owns the binder for its declaration template.
Use the existing structural constraint/compatibility visitor, extended with
binder entry/exit and exhaustive reference handling.

A Free reference is exact, rigid lexical evidence. A Bound reference denotes
a parameter of the specified enclosing binder. An Inference reference exists
only in an active candidate transaction. The same declared T can therefore
occur as Free(T) in an argument and Inference(call, type slot 0) in the
recursive callee pattern. Relating them binds the latter to the former.

GenericBinder has ordered type slots, const-length slots and effect-row slots.
Depth zero names the nearest nonempty binder; entering an empty binder does
not increase depth. Bounds are checked against the exact slot kind and count.
Const slots retain the existing nonnegative array-length domain. This cut does
not add arithmetic const expressions or new const domains. Effect slots bind
canonical rows over the existing effect atom domain.

The term model applies to all accepted callable families. There is no recursive
flag, name-based test, nominal special case, or preference between colliding
roles. No source spelling participates in inference.

## 2. Schema construction and lexical evidence

CallableSignatureSchema seals its accepted candidate inventory once. Candidate
occurrences become Bound references to its template binder; RigidReference
occurrences remain Free. The inventory links each declaration parameter to
one typed binder slot and retains its checked first-use coordinate. That link
is owned by the schema, not reconstructed in consumers. Remove raw unscoped
parameter/result access from callable consumers: projections carry the schema's
binder context. Declaration-body type facts keep Free declaration references.

Construct the enclosing rigid scope from the accepted HIR semantic path and
accepted semantic owner inventories. Include the enclosing callable and its
generic owner context, including nominal/impl/trait parameters visible in a
member, and captured lexical parameters visible inside a closure or attached
default. Use accepted owner links and accepted generic inventories. Do not infer
membership by collecting whatever parameters happen to occur in an argument.

HIR topology remains the lexical owner authority. A root declaration inventory
which already includes its accepted owner references is sufficient; otherwise
the semantic owner projection must join those accepted owner inventories before
issuing the scope. There is one CheckedLexicalGenericScope projection, with an
exact generation and owner path, not an independent lexical registry.

Free references in a function-value schema must be admitted by that lexical
projection or by its sealed captured generic environment. Equal admitted Free
references coalesce. A forged, missing, stale or foreign reference is an
invariant failure. Free references cannot receive Bindable or FutureEligible
eligibility. Candidate slots cannot receive lexical Rigid eligibility.

## 3. Opening the single candidate transaction

PreparedCallGraph's existing preparation gate issues the only affine
OpenedCallConstraintInitialization. It binds the selected schema, current group,
prepared graph issuer, lexical scope, effect scope and inherited substitution.
All schema/candidate/input joins are validated before issuing inference slots,
opening callbacks or charging source work.

A fresh GenericApplicationIssuer is private, non-serializable and unforgeable
outside the owning module. Each schema candidate slot opens to one inference
reference under that issuer. Freshness uses a checked process-local counter;
counter exhaustion is an invariant/abort, never a reused issuer. The counter
does not order candidates, determine replay choices or enter stable bytes.

The same typed binder operation opens types, const lengths and effect rows.
Open only Bound references owned by the candidate template. Preserve nested
function binders, applying de Bruijn shifting during descent. Preserve Free
references exactly. Every source operand, explicit generic argument, expected
result and value/type receiver enters as a semantic type in the lexical scope,
not by opening candidate names in that actual type.

First-use policy is preserved:
- prior-group slots require inherited bindings;
- current-group and implicit extension-group slots are Bindable;
- later-group and result-only slots are FutureEligible before the terminal group;
- every required candidate slot is Bindable at the terminal group.

The exact inherited-key proof also includes every binding already sealed by
the previous prefix, including a future-first-use slot solved early. First-use
alone cannot discard such a row. Reopening validates the complete frozen key
inventory, preserves those rows immutably, and requires every prior-group key.
Only still-unbound residual slots can receive fresh assignments.

The graph derives ValuePreparation or ApplyGroup from the checked callable
value/application owner. The first-use rules above govern ApplyGroup.
ValuePreparation consumes no argument group, preserves Unapplied position
and permits candidate slots to remain residual. A complete contextual function
type can constrain those slots before completion. It uses the same relation,
normalization and sealer, not a raw identity-substitution constructor.

A FutureEligible slot may be inferred early. It is generalized only if it is
still unbound at completion and is needed by the remaining callable scheme or
the normalized inherited substitution. A future first-use label alone does not
make a solved parameter deferred.

The source work driver receives prepared projections from this opened
authority. It cannot construct a raw lower scope, a second opening or an
alternative solution. None, Prepared and Frozen are evidence states, not
different solvers. Frozen input is thawed through the same gate after exact
base/schema/group/lineage and captured-environment validation.

## 4. Relation, normalization and source transactions

Use the existing direction-sensitive SelectedCall relation and Choice algebra.
Generic leaf behavior is:
- equal Free references accept; distinct Free references do not unify;
- Free versus a concrete incompatible type rejects that candidate;
- an eligible Inference reference can bind to a compatible scoped term;
- a foreign Inference issuer, dangling Bound reference, wrong slot kind or
  unauthorized Free reference is an invariant failure;
- Bound references under compared function binders are rigid alpha coordinates;
  they do not become mutable outer inference variables.

Occurs checks follow only inference-variable edges of the active issuer.
A variable bound to itself or to a term containing itself is cyclic. A
declaration parameter's slot bound to Free of that same declaration is valid.
Transitive bindings are normalized before sealing: x -> y, y -> i64 becomes
x -> i64 and y -> i64. Preserve the existing deferred-cycle/source-failure
precedence and Choice branch isolation. Do not turn every relation into a
bidirectional wildcard equation.

The structural visitor handles nominal arguments, receiver types, Result,
Option, Need, arrays, tuples, records, references, functions and every existing
TypeKind constructor. Binder descent, shifting, opening, substitution,
free-use collection and canonical encoding are behavior of this owner.
Do not add consumer-local match-based substitutions.

Const references obey the same ownership, eligibility, transitive normalization
and occurs rules, with concrete lengths represented canonically as u64 at stable
boundaries and checked conversion to host usize. Effect inference keeps the
existing set/subset relation, row normalization and effect work accounting;
only its variable reference and binder opening change. No type variable can
be reinterpreted as a const or effect variable.

Probe and materialization checkpoints remain affine and transactional.
An expected shape with unresolved outer inference references is Parametric.
It may guide non-call grammar but cannot become an equation in a nested call
solver. A projected child with no such references can be Complete. A child
candidate gets its own issuer. No outer inference reference enters a child's
published facts, cache key or selected application.

Opening and reification happen on both probe and replay through the same
authority. Replay comparison uses reified, alpha-canonical evidence rather than
issuer counters. Failed branches roll back facts, candidate applications and
binding rows together. Successful completion publishes no inference references.

## 5. Completion, freezing and callable schemes

Replace completed raw declaration-keyed solver maps with one opaque
CompletedGenericSubstitution. Completion consumes the winning path, normalizes
all type/const/effect rows, checks completeness and performs strict final
acceptance. It then reifies live unbound future inference variables into a
canonical residual binder and removes every active issuer.

The frozen aggregate owns:
- the exact schema and application position;
- declaration-parameter binding rows whose values are normalized scoped terms;
- the residual binder and its schema-slot origins/first remaining use;
- admitted Free lexical/captured references;
- canonical effect substitutions;
- the checked completeness and projection evidence.

The types-owned CompletedGenericSubstitution contains only generic binder,
scope, normalized binding and completeness evidence. It contains no callable
schema digest, group index, HIR call site or first-use group. FrozenCallTypeSolution
adds the callable-owned schema/position and the projection joining residual
slots to the schema's first-use inventory. These are different responsibilities
inside one sealed aggregate, not two binding models.

Callable application position is the closed algebra Unapplied or
AfterGroup(completed_group). Unapplied belongs to a checked bare callable-value
producer and has next group zero; it is not represented by a sentinel group,
empty fake call, or completed group zero. AfterGroup has the existing checked
adjacent next-group rule. Prepared/Frozen prefix handles carry that exact
position, and only a real completed argument group advances it.

Keys in published binding rows are declaration IDs. Values use Free or Bound
references, never active inference IDs. Key T -> Free(T) is valid and is not
reinterpreted as a recursive lookup. Substitution into a template opens its
bound slots and inserts normalized RHS values exactly once. Substitution into
a declaration body replaces Free keys exactly once. Inserted values are not
recursively run through that same substitution.

Residual slots are ordered by kind (type, const, effect), then the schema
declaration slot order. For aliased unresolved roots choose the least original
slot of that kind and retain all origin links to that single residual slot.
Remove unused residual roots after complete reachability through remaining
groups/results and binding RHSs. Empty binders have one canonical empty form.

FrozenCallTypeSolution owns the completed carrier, not the mutable solver's
parameter eligibility map. Its future rows are projections of that carrier;
there is no separately mutable deferred list. A frozen prefix is immutable.
Each later application opens residual slots afresh and restores the inherited
normalized bindings under that opening. Sibling applications cannot mutate
the prefix or share a mutable inference variable.

A nonterminal result is a function scheme: its Function TypeKind binds the
residual slots needed by all remaining groups and results. Remaining group
nesting is preserved; groups are not flattened. The binder on the outermost
remaining function scopes its descendants, including later function groups.
Free caller references remain distinct from these Bound parameters.

For choose<A,B>(a:A)(b:B)->B after choose(1i64), the result is
forall B. (B -> B) with prefix [i64]. Reusing the same value at String and
i64 yields independent closed terminal instances. In a recursive generic
body, a mixed result such as (Free(caller B), Bound(0, future B)) retains both
identities; identical declaration spelling does not collapse them.

Current-group runtime values must be closed relative to the residual binder.
They may contain admitted Free caller parameters that the enclosing runtime
instance will close, or internally closed function schemes. They may not
contain an unbound parameter of the prefix's own residual binder. An
unconstrained empty literal cannot establish such a parametric captured value:
ordinary inference must resolve it or reject it before materialization. This
is the existing value/inference boundary, not general let-polymorphism.

## 6. Canonical bytes and closed instances

All domains and owned versions remain 1. Stable encoders reject active
inference references. A Free reference encodes the existing accepted semantic
declaration owner and ordinal; a Bound reference encodes kind, depth and slot.
A binder encodes its three counts and its body. Source names, HIR allocation
indices, prepared graph issuers and inference counters are excluded.

Schema bytes additionally encode the declaration-slot origin and first-use
inventory. Function-type scheme identity encodes only alpha-canonical binder
structure, existing semantic type structure and canonical effect rows; it does
not encode origin names. Frozen solution bytes encode the base/schema/position,
canonical declaration-keyed bindings, residual binder/origin projection and
captured Free evidence. Preserve existing stable source ownership only in
the call-site/application identities that already own it.

Replace CheckedProjectFunctionInstanceSolution.layers with one normalized
closed substitution. To close a callee:
1. consume the selected frozen solution's normalized RHSs;
2. substitute the enclosing instance's closed Free bindings into those RHSs
   once, preserving nested Bound binders;
3. require complete type/const/effect bindings for the invoked group;
4. seal the resulting closed rows and instantiation transcript.

Do not apply the callee substitution to its own already-normalized RHSs.
Do not retain the caller's layer history. The same T -> Free(T) recursive
solution closes to the caller's current concrete T, while a finite recursive
specialization can close to a different concrete type.

CallableInstantiationDigest encodes the existing base-instantiation semantic
coordinate and the final sorted type/const/effect rows for the selected
declaration. It excludes the caller key, layer count, discovery order,
representative application, captured runtime values and source site.
The instance key remains exactly (RuntimeCallableId,
CallableInstantiationDigest, CallableGroupIndex). Equal keys require equal
closed schema/projection input; a digest collision or unequal input under an
equal key is an invariant failure.

Defaults and nested closures receive this same closed substitution and their
exact accepted executable-owner partition. They cannot use the first observed
call, a global open-body catalog or descendant-scope approximation.

## 7. Runtime type admission and predicative callable values

A closed runtime function type may bind its own generic parameters. Closed
means no Free semantic declaration reference and no active inference reference,
and every Bound reference is owned by an enclosing binder. It does not mean
that every descendant is monomorphic. Executable FunctionSites remain fully
instantiated: their root parameter/body types have no free application slot.

Extend the existing runtime type projection algebra with closed binder
structure, bound type references, bound array lengths and bound effect row
tails. Scope belongs to each admitted type row. Parent-to-child joins verify
the exact scope stack: a function with a nonempty binder pushes it for its
parameters/result; other edges retain scope. Bound-only subnodes cannot be
used as standalone value, local, prefix or function-site ABI roots.
RuntimeNormalizedType and the core type table consume this same scope rule.
Do not fabricate Unit, opaque placeholders or wildcard types.

ProjectContinuation retains lineage, remaining function_type identity and
prefix types/values. Its function_type can name a closed quantified function.
Prefix value checks and expected-ABI equality remain exact. A later call's
input ABI describes the original prefix scheme; its Invoke target describes
the fresh, fully instantiated application. Never rewrite that input ABI to
the terminal monomorphic type.

Preserve the existing predicative call-inference boundary. Generic opening is
an operation on a selected callable application, not an implicit universal
function subtyping rule. A quantified callable value can be retained, aliased,
captured by a closure and used as the callee of a checked application. That
application retains the definition/prefix evidence needed for selected
instance closure. Comparing two schemes compares their alpha-canonical
binders and bodies rigidly. Comparing a scheme to a monomorphic function type
does not erase or instantiate its binder.

A complete contextual result on the *producing application* can solve future
slots before its result is generalized. Likewise, the existing contextual
selection of a bare generic declaration can produce an already instantiated
function value. Once a prefix has been published with a residual binder, a
later ordinary value assignment to a monomorphic arrow does not silently
specialize it. Applications of that prefix instantiate it through the existing
call gate. This distinction is a selected semantic rule, not a lowering-time
unsupported success.

No source forall syntax, arbitrary let-generalization, higher-rank inference,
impredicative inference, implicit polymorphic-value coercion or dynamic
generic-instance registry is added. A monotype inference slot cannot bind to
an entire polymorphic scheme; a source requiring that conversion is a typed
candidate mismatch. This preserves the existing first-use completeness rule.
Ordinary concrete callbacks, closures using caller-rigid types, and producing
applications already closed by context keep their existing behavior.

A generic bare project-function value may be represented as a quantified
Unapplied checked callable value. If emitted at runtime, it uses the existing
ProjectContinuation carrier with an empty prefix and next group zero. Its
checked function-value producer issues lineage; no synthetic Direct call is
created. Generic parameters of ordinary closures are captured Free parameters,
closed by the enclosing instance; this cut does not generalize new closure
parameters. There is no generic closure FunctionSite.

Continuation input at group zero is consequently validated by the general
prefix rule: Unapplied has an empty prefix and a group-zero function schema.
AfterGroup(k) has the exact prefix and next group k+1. Replace the current
unconditional ContinuationGroupMismatch rejection at group zero with this
typed position proof. Direct remains a unit input, valid only at group zero.
All of this uses the one checked callable-value/application authority; no
sentinel group, type-only call opcode or wrapper function is introduced.

## 8. Plan, AWBC and persistence boundary

Keep ProjectCall's Direct/Continuation input, Continue/Invoke outcome, physical
source row, logical ABI materialization, attached-default rules and same-fiber
call/return/suspension protocol. The required additions are scoped type
admission and the precise Unapplied prefix-position validation above.

AWBC stores the binder-aware type graph. Its verifier checks every scope edge,
closed root, binder coordinate, semantic identity and plan membership before
admission. Native and AWBC retain exact continuation ABI equality and the same
checked terminal instance. They never infer type arguments from runtime values.
Nonterminal groups still allocate no concrete FunctionSite.

The continuation snapshot fields remain lineage, function semantic type
identity, prefix types and prefix values. Restore pins the same program
generation and validates the exact admitted scheme and prefix values. Captured
schemes contain no inference issuer or solver map. Extend version-1 type/wire
tags in place and retain no old reader. Snapshot scheduling, frame order,
cleanup semantics and host Need behavior are unchanged.

General callback execution has its own existing parent obligations.
The current native expression evaluator rejects an Executable FunctionSite
and AWBC ApplyFunction accepts RuntimeValue::Function, not ProjectContinuation.
This contract does not route a quantified prefix into those paths or claim
to fix that separate concrete-callback execution defect. Its new quantified
values enter only the checked application route described above. Parent
completion still requires its ordinary callback execution validation.

## 9. Deterministic recursive instance discovery

Replace recursive host-stack discovery and retained substitution layers with
one compiler-owned ProjectInstantiationSession. It owns a deterministic ordered
worklist, canonical closed nodes, dependency edges, counters and cancellation.
Seeds and dependencies use accepted semantic coordinates and canonical instance
keys. Worklist order is ascending canonical key. Within a node, visit accepted
executable-partition coordinates in their stable semantic order, including
nested closure/default partitions exactly once per enclosing closed instance.

A node enters Queued before dependencies are explored. Processing produces
Discovered. References to Queued, Discovering or Discovered nodes record edges
without re-entering the host stack. After successful discovery, materialize all
nodes in canonical key order into a private catalog and publish that catalog
atomically. SCC traversal is not an execution-depth check.

Production inclusive limits are:
- distinct closed instances: 4096;
- distinct dependency edges: 65536;
- cumulative visited/substituted type/const/effect nodes: 1048576;
- maximum structural type depth: 128;
- total discovery work units: 4194304.

Use checked u64 counters. Charge before allocating a node, edge, copied term
or transcript. Count every visited structural occurrence (not only distinct
interned nodes); duplicate edges consume traversal work but not a second
distinct-edge slot. A work unit is one seed/dependency visit, one structural
node visit, one binding-row normalization, or one node-state transition.
All of these share the same session. Per-operation order is cancellation,
checked arithmetic, depth, structural-node budget, total-work budget, then
distinct node/edge insertion budget. Exactly-at-limit succeeds; the next
charge fails with ProjectInstantiationLimitExceeded(kind, limit, attempted,
origin). Do not report a stack overflow or generic internal type error.

Type substitution and canonical encoding use a bounded iterative traversal,
including effects and consts, so a growing type cannot overflow the host stack
before a limit is checked. Materialization consumes only the closed graph
and its previously charged projections; any newly required discovery is an
invariant error. Compilation configuration owns the limit object. Existing
entry points use the production value; explicitly limited compile entry
points accept a checked limit object for embedders and exact boundary tests.
This is not a new CLI option.

Finite self/mutual/polymorphic recursive instance graphs terminate at this
compiler boundary. Runtime termination is independent. A growing specialization
graph rejects at the first selected inclusive limit, with no partial catalog,
and leaves a prior accepted generation unchanged. Do not ban recursion based
on generic syntax or attempt to prove the user's runtime recursion terminates.

## 10. Failure classes and completion

Malformed schema scope, foreign/stale issuer, dangling bound reference,
noncanonical frozen input, replay disagreement, missing inherited key and
same-key/unequal closed instance are typed invariants. Valid authored
incompatibility, uninferred required parameters and cyclic inference are
candidate rejections. Work/limit exhaustion and cancellation are typed aborts.
No failure publishes a partially checked callable or runtime catalog.

RUST_SHAPES.md owns visibility and API direction. The matrices require success,
rejection, invariant, codec, rollback and limit evidence. All existing
Content/Fx, dialogue, named/rest, closure, Agent and suspending ProjectCall
behavior remains required. Implementation completion requires the full
replacement and applicable workspace/runtime gates, not only a green
recursive leaf test.
