# Sealed call application authority amendment

Status: `READY_FOR_IMPLEMENTATION`

This amendment replaces the call-side assumptions in the original C2 design.
Implementation evidence showed that the checker discarded the selected
generic solution, published provisional `CallTargetFacts` beside a duplicated
`PendingCallAnalysis`, rebuilt the final fact after effect closure, and then
asked the callable join and compiler to infer parts of the application again.
That is not one final typed authority.

The selected final model is a single sealed call application. Diff size and
implementation cost are not design inputs. Optional arguments, dialogue clear,
typed rest, receiver inference, intrinsic constructors, curried continuation,
join validation, and compiler execution projection are all projections of the
same schema and frozen constraint solution.

## 1. Orthogonal schema and source algebras

`CallableParameterPresence::Optional` means only that a parameter may be
omitted. It never changes `T` into `Option<T>`.

`CallableParameterType` is replaced by an admission that cannot pair a missing
declared type with a typed projection rule:

```rust
pub struct CallableParameter {
    index: CallableParameterIndex,
    name: Option<CallableName>,
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    consumer: CallableParameterConsumer,
    // documentation and source
}

pub enum CallableParameterAdmission {
    Checked {
        declared: TypeKind,
        rule: CallableParameterValueRule,
    },
    UncheckedSupply,
}

pub struct CallableParameterValueRule {
    alternatives: Arc<[CallableParameterValueAlternative]>,
}

pub struct CallableParameterValueAlternative {
    evidence: CallableSemanticValueEvidenceRule,
    expected: ParameterExpectedTypeProjection,
    action: CallableArgumentSemanticAction,
}

pub enum CallableSemanticValueEvidenceRule {
    VariantCase {
        owner: ParameterExpectedTypeProjection,
        ordinal: u32,
        payload: VariantPayloadRequirement,
    },
    Any,
}

pub enum VariantPayloadRequirement { Unit, Present }
pub enum ParameterExpectedTypeProjection {
    Identity,
    ApplyUnary(CallableUnaryTypeConstructor),
}
pub enum CallableUnaryTypeConstructor { Option }
pub enum CallableArgumentSemanticAction { Supply, Clear }

pub enum CallableParameterConsumer {
    Value,
    DialoguePatch(CharacterDialogueFieldCoordinate),
    DialogueApplicationMetadata(DialogueApplicationMetadataCoordinate),
}

pub enum DialogueApplicationMetadataCoordinate { Id, TextKey }

pub struct OpenArgumentId {
    schema: CallableSignatureSchemaDigest,
    binding: CallableName,
}

pub enum UnknownNamedArgumentPolicy {
    Reject,
    OpenSupply,
}
```

`Any` occurs exactly once and last. Earlier evidence rows are pairwise
exclusive. The checker selects an alternative through final typed semantic
evidence, never through first successful type inference, source spelling, or
parameter optionality. A checked slot retains a schema-relative
`CallableParameterAlternativeIndex`, the evidence that selected it, and the
composed final expected type. It does not retain a parallel `Supply`/`Clear`
flag; consumers obtain the action from the selected schema alternative.
`Clear` is legal only when the same parameter owns a `DialoguePatch` consumer.
The fixed and custom dialogue coordinates move from `final_analysis` to the
shared `character_dialogue` owner so schema construction and final patch rows
use the same typed identity.
`UncheckedSupply` has no expected projection, contributes no type constraint,
and always has action `Supply`; it cannot own a Clear alternative or a
non-Value consumer.

The argument mapper separately owns how one source value supplies a logical
slot:

```rust
enum PreparedArgumentSourceProjection {
    Scalar,
    InferSpreadContainer { policy: CallableRestContainerPolicy },
}

pub enum CheckedArgumentSourceProjection {
    Scalar,
    SpreadContainer(CheckedContainerConstructor),
}

pub enum CheckedContainerConstructor {
    Vec,
    Seq,
    Slice,
    Array { len: ArrayLength },
    MapValue { kind: MapKind, key: Box<TypeKind> },
}
```

For declared type `D`, selected value alternative `A`, and checked source
projection `S`, the only final expected constraint is `S(A(D))`. Fixed literal
spreads become scalar element slots. A nonliteral typed-rest container is one
container slot. Because the current runtime cannot observe a semantic action
per element, a nonliteral rest container admits only a single
`Any + Identity + Supply` value rule; every clear-capable rest rule rejects.

## 2. Dialogue patch authority

Character Factory/Reconfigure fixed patch fields and clearable custom fields
use two schema alternatives:

1. exact `Option::None` unit case evidence, `ApplyUnary(Option)`, `Clear`;
2. terminal `Any`, `Identity`, `Supply`.

`id`, `text_key`, ContentCall parameters, and non-clearable custom fields are
Supply-only. Custom field rows obtain the rule from the accepted descriptor's
`clearable` policy. The typed field coordinate is stored by the parameter's
consumer policy; it is not reconstructed from the name after checking.

The normal variant checker resolves the projected Option owner and exact case
ordinal. Delete `is_none_patch_value`, every raw `"None"` comparison, the
analyzer's second clear test, and the join's optional-parameter fallback.
The checker exposes a private `PreparedVariantCaseEvidence`; the call sealer
turns it into owner semantic-type digest, case ordinal, and payload presence.
An alias or local symbol spelled `None` cannot satisfy this evidence.

Factory/Reconfigure enumerate every visible custom descriptor as a typed
parameter and use `UnknownNamedArgumentPolicy::Reject`. ContentCall does the
same. The only deliberately open policy is `OpenSupply`; the current
`OpenChecked`/`OpenUnchecked` split is deleted. The schema cannot own an
`OpenArgumentId`, because that identity includes the call-site name. Instead,
after `OpenSupply` admission the mapper privately constructs `OpenArgumentId`
from the selected schema digest and canonical authored argument name. The
sealed slot owns `destination=Open(id)`, `alternative=None`, `expected=None`,
`evidence=Any`, required final `inferred`, and schema-derived `Supply`. It contributes no type
constraint. Raw open-name success and any Clear-capable open argument are
forbidden.
The manifest-specific `look` parameter is published only for an exact Character
whose accepted manifest supplies its Look nominal. A Character-Any target does
not receive an unchecked look/clear success path.

## 3. Candidate-wide type solution

### 3.1 One compatibility engine

The types layer owns one recursive compatibility engine. Its behavior is
selected by a closed typed policy rather than by separate structural,
diagnostic, checker, and call-solver match trees:

```rust
pub(crate) enum CompatibilityPolicy {
    Recovery,
    SelectedCall,
    Invariant,
}

pub(crate) enum CompatibilityOutcome {
    Exact,
    Compatible(CompatibilityEvidence),
    Mismatch(TypeMismatch),
    Unresolved(UnresolvedCompatibilityCause),
}
```

`Recovery` is the normal checker policy. It preserves deterministic cascade
suppression for an already diagnosed `TypeKind::Error`, checker-local
`Named("_")`, and recovered or inferred array lengths. It also owns the normal
directional language relations: `Never`, unique Choice injection, erased
families, Ref payload specialization, Agent values, Bytes, ActionName,
nominals, functions, and effect rows.

`SelectedCall` owns the same directional language relation only after generic
normalization. It rejects every error, poison, checker-local `_`, inferred
array length, unresolved projection, unclosed generic, and unknown effect tail.
Non-recovered array lengths are exact: constants must be equal and generic
lengths must have the same declaration-owned identity. A generic type parameter
is never treated as a wildcard by this policy; binding is exclusively the
constraint transaction's responsibility. Only a parameter classified as
bindable or future-eligible by the candidate scope can be unclosed. A rigid
enclosing generic is compared by its exact identity and is not rejected merely
for remaining generic.

`Invariant` is the fail-closed final-fact validation policy. It rejects
recovery and unresolved values but preserves the normal directional language
relation, including StageActor family widening, unique Choice injection, Ref
payload specialization, and effect-row admission. A rigid enclosing generic is
a checked atom under this policy: the same declaration-owned generic identity
is compatible with itself and is not an unclosed call parameter. No final
validator calls the recovery wrapper.

`TypeKind::accepts` is only the convenience entry for `Recovery`. It does not
own recursion. `TypeKind::first_mismatch` is a strict structural diagnostic
projection that compares constructor, payload, owner, arity, and child
identity without directional widening. Its mismatch is independent of every
acceptance verdict: two types may have a structural mismatch while Recovery or
Invariant still accepts the directional relation. The standalone array-length
acceptance helper is deleted; array-length comparison is a node of the one
engine. `mismatch.rs` retains the typed mismatch path/reason values but no
recursive type algebra. The duplicate nominal, Choice, Agent-value,
function/effect, and terminal acceptance match trees in the checker and
constraint solver are deleted.

The engine accepts a lower traversal observer. A normal check uses the no-op
observer; a candidate run supplies its `TypeConstraintContext`, so every
compatibility descent is charged and cancellation-checked by the same run.
Calling the unmetered recovery convenience entry from a selected-call seal is
forbidden.

### 3.2 Lower solution and parameter scope

One private candidate transaction owns all constraints. The types layer owns
the complete parameter scope and the only solution representation:

```rust
pub(crate) struct TypeConstraintParameterScope {
    // exact declaration-owned parameter inventory and eligibility
}

pub(crate) enum TypeConstraintParameterEligibility {
    Rigid,
    Bindable,
    FutureEligible,
}

pub(crate) struct TypeConstraintSolution {
    // normalized binding rows only
}

struct PreparedCallConstraintSet {
    issuer: CheckedCallSite,
    inherited: Option<Arc<FrozenCallTypeSolution>>,
    parameter_scope: TypeConstraintParameterScope,
    receiver: Option<PreparedReceiverConstraint>,
    arguments: Box<[PreparedArgumentConstraint]>,
    expected_result: Option<TypeKind>,
}
```

`TypeConstraintSolution` is opaque and does not implement `Clone`. A completed
solution is shared only as `Arc<TypeConstraintSolution>`; callable code cannot
construct a binding, copy a binding map, merge solutions, or observe a
transaction frontier. The lower owner supplies consume-only projection and
one sorted binding iterator on a completed solution. It owns no callable group,
first-remaining-group, continuation, or deferred-parameter row. No
pre-normalization, pre-merge, or provisional binding getter exists.

`TypeConstraintParameterScope` enumerates every exact
`GenericTypeParameterId` visible to the candidate and classifies it as rigid,
bindable now, or eligible to remain for a future prefix. The lower scope can
therefore reject an out-of-scope binding, an attempt to bind a rigid enclosing
generic, or an unbound parameter that is not future-eligible. It does not know
or store a callable group index. The higher callable sealer alone proves which
future group first owns an eligible parameter and constructs the corresponding
`DeferredContinuationParameter`. A terminal call permits no such higher-owned
deferred row.

An inherited solution is admitted only from the exact previous continuation.
Before the lower run, the higher layer validates its base/schema identity,
current group, and higher-owned deferred rows against that continuation and
derives the new lower parameter scope. The lower layer then verifies that every
inherited binding is normalized, in scope, and complete under `SelectedCall`.
Inherited bindings are immutable seeds: a later group may consume or extend
them but cannot replace them. An invalid inherited contract is the first
candidate failure and never falls back to a fresh empty scope.

### 3.3 One reserved work session

The callable layer creates one `CandidateConstraintWorkSession<'a>` with an
exclusive `&'a mut ResolverWork` borrow per candidate run:

```rust
pub(crate) struct CandidateConstraintWorkSession<'a> {
    work: &'a mut ResolverWork,
    reservation: PendingCandidateConstraintReport,
}
```

The pending reservation retains the exact `previous` full accounting report
and the only checked `proposed` full report. While the session or its completed
run exists, Rust's exclusive borrow prevents every other observation or update
of that `ResolverWork`. The lower solver never receives it, and source
callbacks receive only the narrow session methods required to charge their
work.

The session projects the exact remaining limits and cancellation token into
one lower `TypeConstraintContext`. Its context is the sole authority that may
enter a node, fork or prune a branch, record a source callback, or add a
binding. Every such operation performs, in order, an `Acquire` cancellation
load, checked work charge, arithmetic-overflow check, configured-limit check,
and only then allocation or descent. The context records each accepted delta
directly in the session's `proposed` full report through its narrow observer;
there is no detached lower report waiting for a caller merge.

`finish` consumes the transaction and moves the session reservation into one
`TypeConstraintRun`. The run owns its outcome but has no outcome getter while
the reservation is pending. `complete(self)` infallibly commits the checked
`previous -> proposed` full report, disarms the drop guard, releases the
exclusive borrow, and only then returns the outcome. Dropping either an
unfinished session or an uncompleted run performs the same infallible commit
exactly once and releases the borrow; it never exposes the outcome. Success,
mismatch, source failure, ambiguity, cancellation, overflow, configured-limit
failure, and early return therefore account through the same path. There is no
caller-side report merge, raw-work return, pending getter, or second completion
operation.

The limits are version-1 lower values and include explicit bounded nodes,
branches, source probes/materializations, and solution bindings. They are
projections of the one resolver reservation, not a second budget. Every
checked update needed by the eventual full report occurs before work or
allocation, so the drop-time commit itself cannot fail.

### 3.4 Callback-only source authority

The callable-owned candidate driver is generic over a
`TypeConstraintClient`. The client is the only bridge to expression checking
and semantic facts; the lower types transaction receives only types-owned
constraints, projected callback results, and its traversal context:

```rust
pub(crate) enum ExpectedHint<'h> {
    Unchecked,
    Complete(&'h TypeKind),
    Parametric {
        expected: &'h TypeKind,
        unbound: &'h [GenericTypeParameterId],
    },
}

pub(crate) enum SourcePhase {
    Probe,
    Materialize,
}

pub(crate) struct SourceError<S, C> {
    source: S,
    phase: SourcePhase,
    cause: C,
}

pub(crate) struct SourceProbeResult<B> {
    actual: TypeKind,
    canonical_branch: B,
}

pub(crate) enum MaterializedSourceRequest<'h, S, B> {
    Unchecked {
        source: S,
        canonical_branch: &'h B,
    },
    Checked {
        source: S,
        expected: &'h TypeKind,
        canonical_branch: &'h B,
    },
}

pub(crate) trait TypeConstraintClient {
    type Source: Copy + Ord;
    type ProbeSemanticBranch: Eq + Ord;
    type SealedBranchValue: Eq + Ord;
    type SourceErrorCause;

    fn probe_source<'h>(
        &mut self,
        source: Self::Source,
        hint: ExpectedHint<'h>,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        SourceProbeResult<Self::ProbeSemanticBranch>,
        SourceError<Self::Source, Self::SourceErrorCause>,
    >;

    fn materialize_sources<'h>(
        &mut self,
        sources: &[MaterializedSourceRequest<'h, Self::Source, Self::ProbeSemanticBranch>],
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        Self::SealedBranchValue,
        SourceError<Self::Source, Self::SourceErrorCause>,
    >;
}
```

The candidate driver creates the hint from the solver-owned projected expected
type. `Complete(expected)` means that projection contains no bindable unbound
parameter. `Parametric { expected, unbound }` carries the exact projected type
and sorted declaration-owned parameters whose later binding can change the
source result. `Unchecked` has no expected constraint and cannot later acquire
one. The callback borrows these values only for its invocation and cannot
retain or rewrite them. There is no reverse source-to-solver hint query.

The callback result is a separate value containing the checked actual type and
canonical semantic branch, or a typed `SourceError`; it is never encoded into
the hint. Both callbacks receive only the narrow mutable candidate work session
needed to charge expression/source work. They cannot access the underlying
`ResolverWork` or any pending accounting report.

Every source probe executes in a fresh client-owned semantic checkpoint;
parametric alternatives each receive a distinct checkpoint. The callback
returns only its actual type and canonical
`ProbeSemanticBranch`; its facts are rolled back before another branch is
visited. It cannot publish an expression, argument fact, alternative, or
application and cannot inspect the solver frontier. A failure is a typed
`SourceError` carrying exact source identity, `Probe`/`Materialize` phase, and
the client's typed cause. Source errors are not flattened into a boolean
mismatch or a string diagnostic.

### 3.5 Correlated frontier and final materialization

The deterministic constraint order is:

1. validate and seed the inherited continuation solution;
2. constrain the complete base instantiation;
3. constrain the receiver;
4. constrain arguments by authored argument ordinal and then physical slot
   ordinal;
5. constrain the expected result in the reverse direction required by
   `context` accepting the projected result; and
6. close and materialize the whole candidate before computing its score.

Each frontier row retains both its binding state and the ordered canonical
probe-semantic branch trace. Rows with equal bindings but different semantic
branches are not deduplicated. A Choice is explored inside the current row;
failure prunes only that branch. It never discards another row, resets the
frontier for the next equation, or merges source evidence by binding alone.
Choice-to-Choice covers every actual alternative under the same correlated
row. Actual `Never` contributes no binding. Unchecked/open supplies contribute
neither a constraint nor a semantic Choice branch.

After every constraint has been added, the transaction performs these phases
in exactly this order:

1. normalize each complete binding environment to a fixed point;
2. perform occurs checking and reject cyclic instantiation;
3. prune every remaining Choice branch with the metered `SelectedCall`
   compatibility engine, including exact array-length and closed-effect checks;
4. canonicalize binding environments while retaining the complete ordered set
   of correlated probe traces for each binding;
5. for every correlated trace inside each canonical binding, reset the client
   to the same pre-probe semantic baseline and invoke `materialize_sources`,
   checking that trace's exact source list in authored argument/slot order with
   the final projected expectations;
6. prune a trace that semantically no longer satisfies final materialization,
   retain a typed source error as an error, seal every successful private
   branch value, and coalesce equal sealed values within that binding by their
   canonical typed identity;
7. discard a binding with no surviving value, reject semantic-branch ambiguity
   when one binding has more than one distinct final sealed value, then require
   unicity of the surviving `(canonical binding, sealed branch value)` pair
   across bindings;
8. project every checked slot, receiver, result, and future-eligible parameter
   from that unique pair for the higher callable sealer; and
9. `finish`, followed by the exactly-once `run.complete()` accounting commit.

Two Choice paths that reach the same binding are not represented by one
optional branch token. That binding retains every correlated trace through
materialization. Traces that produce the same sealed final value coalesce;
only different final values for that binding create semantic-branch ambiguity.
Different canonical bindings remain distinct even if their source facts look
the same. Zero pairs is mismatch, more than one surviving pair is
`AmbiguousInstantiation`, a cycle is `CyclicInstantiation`, and a bindable
terminal parameter that remains unclosed is `IncompleteInstantiation`.

Failure precedence is invalid inherited contract, base instantiation,
receiver, first authored argument/slot, expected result, normalization or
cycle, final Choice rejection, first source-order materialization error,
pair ambiguity, incomplete terminal/deferred closure, projection validation,
then score. A later source error cannot replace an earlier source error, and a
source error retains its phase/cause instead of becoming a generic candidate
mismatch.

`materialize_sources` is not a speculative shortcut. Every correlated trace
of each canonical binding is re-evaluated from the identical client baseline
in an isolated transaction, in exact source order, and may produce a move-only
private prepared branch. Nonselected materializations are rolled back. This
preserves first-source failure precedence and prevents facts from one binding
or Choice path leaking into another. Final public slot facts contain only types
and evidence projected after this phase; prefix probe hints are diagnostic
evidence only.

### 3.6 B3 ownership and replay sequence

The B3 integration sequence is one candidate-wide operation:

1. prepare mapper destinations, schema alternatives, typed evidence, and
   source projections without publishing facts;
2. create the exact parameter scope and inherited seed;
3. exclusively borrow `ResolverWork` into one candidate work session;
4. add base, receiver, source-ordered argument, and expected-result
   constraints to one transaction;
5. complete the correlated solve and ordered materialization once;
6. move the unique `Arc<TypeConstraintSolution>`, coalesced sealed branch
   value, final projections, and score into `PreparedCandidateTransaction`; and
7. keep that transaction private until the C sealer can consume it.

`TypeParameterSubstitutions` may remain for nominal-only paths, but call
selection, join, continuation, and execution never use it or re-run `observe`.
There is no per-equation run, caller-side binding merge, expected-dependent
Option/Result schema, result override, or join-side inference.

For a singleton candidate, the selected path moves its complete prepared
transaction and retained semantic checkpoint; no solution, source branch, or
argument vector is cloned into publication. For multiple candidates, every
probe is isolated and rolled back. Selection starts a fresh checkpoint and
performs the entire candidate transaction again in `SelectedReplay`, including
one new reserved work session, all constraints, source materialization, final
projection, and scoring. Replaying only the arguments is forbidden. The replay
transaction is then moved to the private call analysis and later consumed by
the C sealer.

## 4. Language intrinsic generic ownership

`GenericTypeOwnerId::AgentIntrinsic` is replaced by the general lower-layer
owner:

```rust
pub enum GenericTypeOwnerId {
    Callable(CallableDeclarationKey),
    Nominal(ProjectNominalDeclarationId),
    AcceptedNominal(AcceptedNominalId),
    AcceptedSource(SourceSpan),
    Detached(DetachedTypeOwnerId),
    LanguageIntrinsic(LanguageIntrinsicGenericOwner),
}

pub enum LanguageIntrinsicGenericOwner {
    OptionConstructor,
    ResultConstructor,
    CollectionMap,
    FxExists,
    AgentSignal,
    AgentMetric,
}
```

The lower owner supplies exhaustive version-1 tags. Callable families map their
typed identity to this owner exhaustively; the types layer never depends on a
callable enum. The actual inventory may add another variant only when a real
schema generic owner is migrated in the same cut; it may not be a bag of
examples or derive identity from a raw integer/schema digest.

At minimum the final schemas express:

```text
Some<T>(T) -> Option<T>
Ok<T, E>(T) -> Result<T, E>
Err<T, E>(E) -> Result<T, E>
Container<A>.map<B>(fn(A) -> B) -> Container<B>
exists<T>(Probe<T>) -> Predicate
```

`Reduction::unchanged` uses the exact accepted `Reduction<S>` nominal owner and
its existing `GenericTypeOwnerId::AcceptedNominal`, obtained from the accepted
world. It is not a second language-intrinsic generic owner. Collection method
candidates are emitted only for a supported concrete container constructor;
invalid-receiver `_` fallbacks reject.

The current repository has no `Traversal` semantic type, accepted nominal, or
runtime carrier. `DomainMethodId::{Traverse, Parallel}` therefore publish no
candidate in C2, their pseudo schemas and success fixtures are deleted or
changed to typed rejection, and no `DomainTraverse` generic owner exists. A
future accepted carrier design may reintroduce them as one independent result.

Expected result is a typed solver constraint; it does not rebuild Option or
Result schemas. Delete contextual Option/Result schema specialization,
`CallableInstantiation::{Option, Result}.expected`,
`inferred_constructor_result`, the Agent-specific
`apply_argument_expected` exception, inference-bearing `Named("_")`, and
invalid-receiver `_` fallback.

A terminal selected call may not expose an unbound language-intrinsic
parameter. Missing Result sides are not defaulted to `Never`. A partial call may
defer an unbound parameter only when it remains explicitly quantified by the
continuation schema and is owned by `DeferredContinuationParameter`.

## 5. One application and one continuation

Analysis owns only a private prepared row until checked callables and final
effects are ready:

```rust
struct PreparedCallAnalysis {
    candidates: Box<[PreparedResolvedCallable]>,
    // candidate transaction and diagnostics
}

pub enum CheckedCallSite {
    HirCall(ExprId),
    DialogueApplication(ExprId),
}

pub struct CallTargetFacts {
    enclosing_callable: Option<CallableDeclarationKey>,
    outcome: CallAnalysisOutcome,
    diagnostics: Arc<[CallableDiagnostic]>,
    accounting: CallResolverAccountingReport,
}

pub enum CallAnalysisOutcome {
    Selected(CheckedCallApplication),
    Ambiguous(CheckedAmbiguousCallEvidence),
    Rejected(CheckedRejectedCallEvidence),
    NonCallable(CheckedNonCallableEvidence),
    Missing(CheckedMissingCallEvidence),
}

pub struct CheckedCandidateInventory {
    candidates: Arc<[Arc<ResolvedCallable>]>,
    selected: CheckedCandidateIndex,
    digest: CheckedCallCandidateInventoryDigest,
}

pub struct FrozenCallTypeSolution {
    solution: Arc<TypeConstraintSolution>,
    deferred: Box<[DeferredContinuationParameter]>,
    digest: FrozenCallTypeSolutionDigest,
}

pub struct CheckedCallApplicationCore {
    site: CheckedCallSite,
    current_group: CallableGroupIndex,
    candidates: CheckedCandidateInventory,
    solution: Arc<FrozenCallTypeSolution>,
    callee: CheckedCallCalleeExecution,
    execution: CheckedCallExecutionProjection,
    effects: EffectRow,
    digest: CheckedCallApplicationCoreDigest,
}

pub struct CheckedCallApplication {
    core: Arc<CheckedCallApplicationCore>,
    result: CheckedCallResult,
    digest: CheckedCallApplicationDigest,
}
```

`FrozenCallTypeSolution` is the callable-sealed handle to the sole lower
solution. It owns an `Arc<TypeConstraintSolution>` whose pointee is opaque and
non-`Clone`; it does not copy its normalized bindings into a callable-owned
collection. Deferred continuation parameters are deliberately higher-owned:
the callable sealer derives and sorts `deferred` after proving each row's exact
first remaining group from the schema. It computes the version-1 digest from
the lower solution's sorted completed binding iterator followed by those
sorted deferred rows. Sharing a frozen solution between an application core
and its continuation clones only the sealed `Arc` handle. Probe publication
cannot clone or reconstruct the underlying solution: the first frozen handle
is created by consuming the completed candidate transaction.

`PreparedResolvedCallable` is an issuer-only resolver object. It may contain
raw lookup IDs, origin, schema pointers, probe state, and base instantiation,
but has no stable digest and is never published. After checked callable/effect
catalogs and the C1 semantic-coordinate index exist, the application sealer
consumes every prepared candidate into `ResolvedCallable`; probe and replay
objects cannot cross that boundary. Therefore a raw local/function candidate
does not pretend to own a stable identity before the inputs for that identity
exist.

There is no phase cycle through final publication. After callable/effect
catalog completion, a private `SemanticCoordinateIndex` is sealed from the HIR
declaration-path index plus projection-independent checked structural child
roles in the draft. Method join enrichment occurs later and cannot change those
C1 path bytes. The call sealer borrows this index, consumes prepared candidates,
and then the index is consumed by the remaining transcript/coordinate owners;
it is not stored as a second final-analysis side table.

The candidate inventory consumes resolver rows, sorts them by the full
`ResolvedCallableDigest`, requires strict digest order, and rewrites the
selected index to that canonical order. Two prepared rows with the same digest
must validate as the same checked authority and are coalesced; a digest-equal
but authority-unequal row is corruption. It then validates a nonempty bounded
list and one in-range selected index; `selected()` borrows the indexed `Arc` and
no second selected object exists. Probe order remains diagnostic evidence only.
Ambiguous/rejected/non-callable/missing variants own only their
exact tooling evidence and expose no execution, result, or continuation API.
Each unselected evidence type owns its optional
`CallCalleeClassificationFact`; recovery that never established a
classification stores `None` there. There is no common callee classification
beside the selected execution row.
Each unselected evidence row owns and validates its own `CheckedCallSite`; the
selected site exists only in `CheckedCallApplicationCore`, so no wrapper/site
copy is retained. The core is sealed before the result. This is an identity
phase boundary, not a partially usable application: only the complete
`CheckedCallApplication` is published through
`CallAnalysisOutcome::Selected`.
Delete provisional
selected `CallTargetFacts`, `PendingCallAnalysis`, final selected rebuild, and
duplicated selected candidate ownership. Delete the common
`CallTargetFacts.callee` and every compiler fallback that rediscovers
callee/receiver sources.

The result owns the only continuation coordinate:

```rust
pub enum CheckedCallResult {
    Value(TypeKind),
    Continuation(Arc<CheckedCallContinuation>),
}

pub struct CheckedCallContinuation {
    base: Arc<ResolvedCallableBase>,
    next_group: CallableGroupIndex,
    inherited_solution: Arc<FrozenCallTypeSolution>,
    prefix_application_core: CheckedCallApplicationCoreDigest,
    function_type: TypeKind,
    digest: CheckedCallContinuationDigest,
}

pub struct ResolvedCallableBase {
    authority: Arc<ResolvedCallableAuthority>,
    instantiation: ResolvedCallableBaseInstantiation,
    digest: ResolvedCallableDigest,
}

pub enum ResolvedCallableBaseInstantiation {
    None,
    ExpectedEnum { expected: TypeKind },
    Result { kind: ResultConstructorKind },
    Option,
    Character { owner: ResolvedCharacterOwner },
    Receiver { receiver: TypeKind },
    TypeReceiver { receiver: TypeReceiverInstantiation },
    Extension {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

pub struct ResolvedCallable {
    base: Arc<ResolvedCallableBase>,
    state: ResolvedCallableState,
}

pub enum ResolvedCallableState {
    Base,
    Continuation(Arc<CheckedCallContinuation>),
}

pub struct DeferredContinuationParameter {
    parameter: GenericTypeParameterId,
    first_remaining_group: CallableGroupIndex,
}
```

`ResolvedCallable` is split into one shared private `Arc<ResolvedCallableBase>`
plus either base or the exact continuation result object. It never copies the
continuation's group, solution, prefix-core digest, or function type into a
second state row. The base owns the exact non-Curried instantiation; the
authority owns one stable identity plus checked record/schema, diagnostic
origin, equivalent sources, family, and authority rank.
The continuation shares that exact authority; it never reconstructs it from a
dispatch or digest. `ResolvedFunctionValueSeed` carries this opaque
continuation. `ResolvedCallable` creates a curried candidate only through
`try_from_continuation`; raw
`continuation_base + next_group`, independent next-group recomputation, and
function-type schema reconstruction are deleted. The continuation digest binds
the exact catalog/intrinsic base, schema, group, prefix application core, and
inherited solution.

Every deferred parameter belongs to the base schema, first appears at its
recorded remaining group or later, and occurs only in remaining groups/result.
It may not occur in a consumed receiver/current slot or a terminal value. The
sealer derives `function_type` exactly once from the base schema, frozen
solution, deferred quantifiers, and next group; construction validates the
stored type against that projection. A terminal `Value` permits no deferred
parameter.

Synthetic Dialogue content application uses an explicit
`CheckedCallSite::DialogueApplication` origin but enters the same resolver,
constraint builder, and sealer. It may not hand-construct a selected target.
`CheckedCallSite::{HirCall, DialogueApplication}` is generation-local issuer
evidence. The sealer validates its ExprId/HIR family, but raw ExprId bytes do
not enter stable application identity.

## 6. Execution projection and join

The sealed application owns the complete sema-to-compiler projection:

```rust
pub struct CheckedCallExecutionProjection {
    receiver: CheckedCallReceiverProjection,
    arguments: Box<[CheckedCallExecutionArgument]>,
}

pub enum CheckedCallCalleeExecution {
    Direct,
    Value { source: ExprId },
}

pub enum CheckedCallReceiverProjection {
    None,
    SemanticOnly { mode: CallableReceiverMode, ty: TypeKind },
    Operand {
        mode: CallableReceiverMode,
        ty: TypeKind,
        source: ExprId,
        abi_position: u32,
    },
}

pub struct CheckedCallExecutionArgument {
    argument: HirCallArgumentOrdinal,
    passing: CheckedCallArgumentPassing,
    slots: Box<[CheckedCallExecutionSlot]>,
}

pub enum CheckedCallArgumentPassing {
    Positional,
    Named,
    Spread,
}

pub struct CheckedCallExecutionSlot {
    slot: CallableArgumentSlotIndex,
    source: CheckedCallArgumentSlotSource,
    abi_position: u32,
    destination: CheckedCallOperandDestination,
    source_projection: CheckedArgumentSourceProjection,
    alternative: Option<CallableParameterAlternativeIndex>,
    evidence: CheckedSemanticValueEvidence,
    inferred: TypeKind,
    expected: Option<TypeKind>,
}

pub enum CheckedSemanticValueEvidence {
    VariantCase {
        owner: SemanticTypeDigest,
        ordinal: u32,
        payload: VariantPayloadRequirement,
    },
    Any,
}

pub enum CheckedCallOperandDestination {
    Parameter(CallableParameterCoordinate),
    Open(OpenArgumentId),
}

```

Each authored argument is one atomic source-order row; its slots atomically own
destination, source/value projections, selected schema alternative, checked
evidence, and final types. Receiver and arguments own unique
contiguous ABI positions per slot, so a fixed-literal spread may supply several
parameters. `ordered_runtime_operands()` validates and returns
that order without schema or HIR lookup. A type receiver uses `SemanticOnly`;
value and extension receivers use `Operand`.

`CheckedCallCalleeExecution` and the receiver `source` are sealed
generation-local execution coordinates. Catalog/language direct calls require
`Direct`; a callee evaluated as a lexical/function/continuation value requires
`Value`. The sealer validates the source belongs to the checked call site and
callee classification, and that receiver source/mode agrees with the selected
candidate. The raw source IDs are excluded from stable digests just like an
argument expression source, but remain in the in-memory sealed application so
compiler lowering never rereads HIR or an outer callee fact.

The selected candidate is the only dispatch identity. Owner methods on
`CheckedCallApplicationCore` project its catalog/language/lexical/function/
continuation dispatch form directly from `candidates.selected()`; the execution
row does not copy a second dispatch identity.

`CheckedSemanticValueEvidence` is `Any` or a shared lower semantic variant-case
row containing owner type digest, exact ordinal, and payload presence. The core
owner derives `CallableArgumentSemanticAction` by joining the selected
candidate schema with destination/admission/alternative. OpenSupply and
UncheckedSupply derive Supply; a checked parameter derives its selected
alternative's action. No slot stores or hashes a copied action, and callers
cannot mint one independently.
For a checked Parameter destination, `alternative` and `expected` are both
Some and the evidence matches that row. For UncheckedSupply Parameter and
OpenSupply Open destinations, both are None and evidence is Any. Every other
combination rejects at the core seal.
Closed named arguments retain only the `Named` tag; their destination
coordinate is authority. The sema owner method obtains any host ABI label from
the already validated selected schema. Only `OpenArgumentId` retains its typed
binding name.

The execution projection deliberately excludes the result. The application
core commits to all inputs, dispatch, effects, and the frozen solution; the
continuation may therefore bind that acyclic core digest. The final application
then commits to the core plus exactly one value or continuation result.

Compiler/runtime-plan lowering is a typed conversion from this row plus the
selected candidate owner method. Delete its schema/name/HIR reconstruction of
dispatch, receiver insertion, callee value discovery, named operand
binding, argument action, and partial-call shape. Need producer admission,
verify, LSP, project index, Entry, reachability, and signature help consume the
same sealed outcome or an explicit diagnostic view.

Decision D19 remains: `prepare_checked_callable_joins` runs once after the
application and catalog seals, validates catalog generation/pointer/schema and
dispatch without type inference, enriches Method, and moves the join into edge
facts. No join side map remains in final analysis. Delete every join-side
substitution, receiver observation, expected-form search, rest reconstruction,
result recomputation, and curried-base reconstruction.

## 7. Canonical identity and tamper checks

`ResolvedCallableDigest`, `FrozenCallTypeSolutionDigest`,
`CheckedCallCandidateInventoryDigest`, `CheckedCallContinuationDigest`,
`CheckedCallApplicationCoreDigest`, and `CheckedCallApplicationDigest` are
opaque version-1 newtypes without public `from_bytes`. Owner methods encode
exhaustive semantic tags without discriminant casts.

The exact domains are:

```text
arcweft.lang.resolved-callable.v1\0
arcweft.lang.call-type-solution.v1\0
arcweft.lang.call-candidate-inventory.v1\0
arcweft.lang.call-continuation.v1\0
arcweft.lang.checked-call-application-core.v1\0
arcweft.lang.checked-call-application.v1\0
```

Every grammar uses `tag8`, checked little-endian `u32` counts/indexes, checked
little-endian `u64` scalar lengths, `bytes = u64 length || bytes`, and raw
32-byte child digests. Overflow rejects; no saturation or discriminant cast is
allowed. Tags are fixed as follows:

```text
evidence VariantCase=0 Any=1; payload Unit=0 Present=1
admission Checked=0 UncheckedSupply=1
expected Identity=0 ApplyUnary=1; unary Option=0
schema action Supply=0 Clear=1 (never stored in an execution slot)
source Scalar=0 SpreadContainer=1
container Vec=0 Seq=1 Slice=2 Array=3 MapValue=4
solution Bound=0 Deferred=1
result Value=0 Continuation=1
callee execution Direct=0 Value=1
outcome Selected=0 Ambiguous=1 Rejected=2 NonCallable=3 Missing=4
receiver None=0 SemanticOnly=1 Operand=2
passing Positional=0 Named=1 Spread=2
destination Parameter=0 Open=1
source coordinate Expression=0 CompactNumericElement=1
optional None=0 Some=1
stable callable identity Catalog=0 Language=1 Lexical=2 FunctionValue=3
callable state Base=0 Continuation=1
capture mode Read=0 Reassign=1
base instantiation None=0 ExpectedEnum=1 Result=2 Option=3 Character=4
  Receiver=5 TypeReceiver=6 Extension=7
result constructor Ok=0 Err=1
character owner source EntityReference=0
receiver mode None=0 Value=1 Type=2 Extension=3
site HirCall=0 DialogueApplication=1 (issuer validation only)
```

The exact parent grammars are:

```text
resolved-callable =
  domain || Base || stable-callable-identity || schema-digest32 ||
    base-instantiation
  | domain || Continuation || continuation-digest32

stable-callable-identity =
  Catalog || checked-callable-digest32
  | Language || checked-language-callable-identity
  | Lexical || bytes(c1-stable-binding-coordinate) || effect-row
  | FunctionValue || bytes(c1-semantic-path) || u32(function-ordinal) ||
    function-type-digest32 || effect-row || u32(capture-count) || capture-row*

capture-row = bytes(c1-stable-value-coordinate) || capture-mode-tag8 ||
  capture-type-digest32

checked-language-callable-identity = language-family-tag8 || family-payload

family-payload(Fx) = fx-operation-tag8
family-payload(EnumConstructor) = owner-type-digest32 || u32(case-ordinal)
family-payload(Result) = result-constructor-tag8
family-payload(Option) = option-constructor-tag8
family-payload(Builtin) = canonical-builtin-operation
family-payload(Agent) = agent-operation-tag8
family-payload(Presentation) = presentation-operation-tag8
family-payload(Dialogue) = dialogue-operation-tag8 || optional(content-coordinate)
family-payload(Collection) = collection-operation-tag8
family-payload(PresentationHandle) = presentation-handle-operation-tag8
family-payload(Integer) = integer-operation-tag8
family-payload(Domain) = canonical-domain-operation
family-payload(Capacity) =
  capacity-operation-tag8 || receiver-type-digest32 || u32(arity)
family-payload(Stage) = stage-operation-tag8
family-payload(LineContext) = line-context-operation-tag8
family-payload(LineSchedule) = line-schedule-operation-tag8
family-payload(Drop) = drop-operation-tag8
family-payload(Promotion) = promotion-operation-tag8

content-coordinate = canonical-module-path || canonical-callable-path
canonical-module-path = u32(segment-count) || bytes(segment-utf8)*
canonical-callable-path = u32(segment-count) || bytes(segment-utf8)*

canonical-domain-operation =
  FxSampleOrdinalPhase
  | ObservedObjectRequireRole
  | MapGet || key-type-digest32 || value-type-digest32
  | ProbeCompare || value-type-digest32 || comparison-operator-tag8
  | DiagnosticsHasError
  | RagContextPackSummary
  | Context
  | WithContext

base-instantiation =
  None
  | ExpectedEnum || expected-type-digest32
  | Result || result-constructor-tag8
  | Option
  | Character || bytes(canonical-CharacterId-UTF8) || character-owner-source-tag8
  | Receiver || receiver-type-digest32
  | TypeReceiver || normalized-receiver-type-digest32
  | Extension || receiver-type-digest32 || u32(group) || u32(parameter)

call-type-solution =
  domain || u32(bound-count) ||
  (Bound || generic-parameter-type-digest32 || bound-type-digest32)* ||
  u32(deferred-count) ||
  (Deferred || generic-parameter-type-digest32 || u32(first-group))*

candidate-inventory =
  domain || u32(candidate-count) || resolved-callable-digest32* ||
  u32(selected-index)

continuation =
  domain || resolved-base-callable-digest32 || u32(next-group) ||
  solution-digest32 || prefix-application-core-digest32 ||
  function-type-digest32

checked-call-application-core =
  domain || candidate-inventory-digest32 || u32(current-group) ||
  solution-digest32 || callee-execution-tag8 || receiver ||
  u32(argument-count) ||
  execution-argument* || effect-row

checked-call-application =
  domain || checked-call-application-core-digest32 || result
```

`ResolvedCallableAuthority` separates stable semantic identity from issuer and
diagnostic evidence:

```rust
pub struct ResolvedCallableAuthority {
    stable: ResolvedCallableStableIdentity,
    checked: ResolvedCallableCheckedDefinition,
    issuer: ResolvedCallableIssuerEvidence,
    // family, rank, equivalent sources, and diagnostic origin
}

pub enum ResolvedCallableStableIdentity {
    Catalog(CheckedCallableDigest),
    Language(CheckedLanguageCallableIdentity),
    Lexical(CheckedLexicalCallableIdentity),
    FunctionValue(CheckedFunctionValueIdentity),
}

pub struct CheckedLexicalCallableIdentity {
    binding: StableCheckedBindingCoordinate,
    effects: EffectRow,
}

pub struct CheckedFunctionValueIdentity {
    expression: CheckedSemanticPath,
    ordinal: FunctionValueOrdinal,
    function_type: SemanticTypeDigest,
    effects: EffectRow,
    captures: Box<[CheckedCaptureSignatureRow]>,
}

pub struct CheckedCaptureSignatureRow {
    binding: StableCheckedValueCoordinate,
    mode: CheckedCaptureMode,
    ty: SemanticTypeDigest,
}

pub enum CheckedLanguageCallableIdentity {
    Fx(FxCallableSignatureId),
    EnumConstructor { owner: SemanticTypeDigest, case: u32 },
    Result(ResultConstructorKind),
    Option(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(CheckedDialogueCallableIdentity),
    Collection(CollectionMethodId),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(CheckedDomainMethodIdentity),
    Capacity(CheckedCapacityMethodIdentity),
    Stage(StageMethodId),
    LineContext(LineContextMethodId),
    LineSchedule(LineScheduleCallableId),
    Drop(DropCallableId),
    Promotion(PromotionCallableId),
}

pub struct CheckedDialogueCallableIdentity {
    operation: DialogueCallableId,
    content: Option<CheckedContentCallableCoordinate>,
}

pub struct CheckedContentCallableCoordinate {
    module: CanonicalModulePath,
    path: CallablePath,
}

pub enum CheckedDomainMethodIdentity {
    FxSampleOrdinalPhase,
    ObservedObjectRequireRole,
    MapGet { key: SemanticTypeDigest, value: SemanticTypeDigest },
    ProbeCompare {
        value: SemanticTypeDigest,
        operation: ProbeComparisonOperator,
    },
    DiagnosticsHasError,
    RagContextPackSummary,
    Context,
    WithContext,
}

pub struct CheckedCapacityMethodIdentity {
    operation: CheckedCapacityOperation,
    receiver: SemanticTypeDigest,
    arity: u16,
}

pub enum CheckedCapacityOperation {
    WithCapacity,
    Trim,
    ToString,
    Pop,
    PopFront,
    Collect,
    Push,
    Reserve,
    ShrinkTo,
    Shrink,
}
```

The coordinate algebra is moved, not copied, from
`final_analysis/model.rs` and `final_analysis/match_edges/model.rs` into the
sema-root lower owner `crate::semantic_coordinate`. That module owns
`AcceptedDeclarationSemanticId`, `CheckedNestedPathV1`,
`CheckedNestedPathSegmentV1`, `CheckedExpressionChildRole`,
`CheckedSemanticPathStep`, `CheckedSemanticPath`,
`StableCheckedBindingCoordinate`, `StablePatternCoordinate`, and
`StableCheckedValueCoordinate`, plus their sole canonical encoders. It depends
only on HIR structural-role types and lower runtime field identities; it does
not depend on `callable` or `final_analysis`. Callable authority and final
analysis both use those exact types. The old definitions and encoders are
deleted rather than reexported as a second algebra.

Catalog-backed Project, Detached, Environment, Standard, Adapter, and trait
methods use their accepted `CheckedCallableDigest`.
`CheckedLanguageCallableIdentity` is an exhaustive typed encoding of every
non-catalog language family and its semantic candidate payload: Fx, enum
constructor, Result, Option, Builtin, Agent, Presentation, Dialogue,
Collection, PresentationHandle, Integer, Domain, Capacity, Stage, LineContext,
LineSchedule, Drop, and Promotion. An enum constructor encodes the exact
expected enum semantic-type digest plus case ordinal; it never encodes the raw
project item ID. Each other family encodes its fixed family/variant tag and all
typed semantic owner payloads needed to distinguish candidates. Adding a
`CallableCandidateId` variant without adding its stable projection is a compile
failure; a candidate for which a stable projection cannot be formed rejects
before publication.

Language family tags are Fx=0, EnumConstructor=1, Result=2, Option=3,
Builtin=4, Agent=5, Presentation=6, Dialogue=7, Collection=8,
PresentationHandle=9, Integer=10, Domain=11, Capacity=12, Stage=13,
LineContext=14, LineSchedule=15, Drop=16, and Promotion=17. The family payload
is emitted only by an exhaustive owner method on the corresponding typed ID.
Closed enum variants use fixed `tag8`; semantic `TypeKind`, nominal owner,
receiver, map key/value, and enum owner/case payloads use their canonical
semantic digest; bounded ordinals/arity use checked little-endian scalars; and
accepted names occur only where the typed domain is deliberately open.
Capacity operations are first normalized to a closed operation enum, so the
source method string is not a semantic payload. Unsupported
`DomainMethodId::{Traverse, Parallel}` never reaches this encoder.

Every `*-operation-tag8` in the grammar is fixed to the zero-based position in
the following exhaustive list. Implementations use an explicit match and never
cast an enum discriminant:

- Fx: Style, Text, Color, Transform, Mask, Filter, Shader, Transition,
  Conditional, Stack.
- Result: Ok, Err. Option: Some.
- Builtin: InlineFailureFallback, Panic, Fail, Bail, Ensure, Rgb, Sin, Cos,
  Vector, Math, StdFloat, Capability, Reduction. Vector payload: Two, Three,
  Four. Math payload: MatMulF32, MatrixAddF32, MatMulF64, MatrixAddF64,
  TensorAddF32, TensorAddF64. Capability payload: EventEmit. Reduction payload:
  Unchanged.
- StdFloat encodes width F32/F64, then operation Abs, Floor, Ceil, Round,
  Trunc, Fract, Sqrt, Sin, Cos, Tan, Exp, Exp2, Ln, Log2, Log10, Powf, Atan2,
  MulAdd, IsNan, IsInfinite, IsFinite, IsSignPositive, IsSignNegative, ToBits,
  FromBits, ToF32, ToF64. Invalid width/conversion pairs reject before encoding.
- Agent: Observe, Expect, Deny, Checkpoint, Note, Attach, ChoiceAction,
  Viewport, Layer, Object, Capture, ReadResource, EntityMeta, ProjectNeighbors,
  Signal, Metric, StatePath, ObservationPath, State, Observation, Diagnostics,
  Exists, ActionEnabled, All, Any, Not, Wait, AdvanceText, ViewportPoint,
  PointerClick, Invoke, RagQuery.
- Presentation: View, Menu, Overlay, Background, Image, PlayerViewport, Show,
  RefBackground, RefShow, ClearBackground, Hide. Dialogue: CharacterFactory,
  CharacterReconfigure, ContentApplication, ContentCall.
- Collection: Len, Map, Filter, Sum, Contains. PresentationHandle: Show, Hide,
  Unmount, Release, Destroy, OverlayPop. Integer: Clamp, Min, Max.
- Domain: FxSampleOrdinalPhase, ObservedObjectRequireRole, MapGet, ProbeCompare,
  DiagnosticsHasError, RagContextPackSummary, Context, WithContext. Comparison
  operator: Eq, NotEq, Greater, GreaterOrEqual, Less, LessOrEqual; source
  spelling aliases are normalized before identity construction.
- Capacity: WithCapacity, Trim, ToString, Pop, PopFront, Collect, Push, Reserve,
  ShrinkTo, Shrink. Stage: Acquire, Look. LineContext: VoiceHandle.
  LineSchedule: At. Drop: Drop. Promotion: Promote, PromoteUnchecked, Assume.

For Dialogue, `content` is `Some` exactly for ContentCall and `None` for the
other operations. Its module/path is an accepted canonical semantic coordinate,
not the spelling of the call expression. The outer resolved-callable grammar
always appends the selected `schema-digest32`; therefore the same operation tag
with a different checked parameter/result/effect schema has a different base
identity without duplicating schema fields inside each family payload.

`CheckedSemanticPath` and `StableCheckedBindingCoordinate` are produced only
from C1's accepted declaration semantic-path index. Their bytes, and those of
`StableCheckedValueCoordinate`, are exactly the moved C1 canonical child
grammar wrapped by the displayed `bytes` length; this amendment does not define
a second path encoding or path digest domain. A lexical identity binds the accepted enclosing
declaration/binding path and its checked schema/effect digests; raw
`SemanticScopeId` and `LexicalBindingIndex` remain issuer lookup evidence only.
A function-value identity binds its expression semantic path, ordinal within
that expression, checked schema/function type/effects, and the canonical
capture signature. That signature sorts rows by the canonical bytes of their
accepted stable binding coordinate and commits to `CaptureAccess::Read` or
`CaptureAccess::Reassign` plus semantic type; duplicate coordinates reject and
raw `CaptureId` is
excluded. `FunctionValueSignatureId.expression` remains issuer lookup evidence
only.

Diagnostic origin, authority rank, equivalent sources, and generation-local
lookup keys are validated against the checked definition but do not enter
`ResolvedCallableDigest`. They may affect diagnostics or deterministic
selection precedence; they cannot change the semantic identity of the same
accepted callable. Catalog and language stable identity encoders bind their
schema/type semantics through the checked definition and the base digest's
schema field, never source spelling.

The complete non-Curried base instantiation is encoded by its owning typed
encoder. ExpectedEnum includes the expected type digest; Result includes the
constructor-kind tag; Character includes its exact semantic owner; Receiver
and TypeReceiver include their exact typed receiver; Extension includes
receiver, group, and parameter. A continuation candidate hashes its exact
`CheckedCallContinuationDigest`, whose prefix is the previous application core,
so inventory identity always includes call-site instantiation rather than
authority alone. The candidate inventory hashes the full
`ResolvedCallableDigest`, not an authority or base-only digest.

Raw `ExprId`, `ItemId`, `LocalId`, `PatternId`, `StmtId`, `TypeId`, `ScopeId`,
`SemanticScopeId`, and equivalent generation-local integers are forbidden in
`ResolvedCallableDigest`, `CheckedCallApplicationCoreDigest`,
`CheckedCallApplicationDigest`, and `CheckedCallContinuationDigest`. Their
presence in issuer structures is not a serialization exception. A tamper gate
reissues equivalent HIR with different allocation order and requires identical
stable digests.

Bound rows sort by generic parameter identity. Deferred rows sort by parameter
identity then first group. Counts precede their rows exactly as shown. The
generic parameter identity is encoded as the semantic digest of its exact
`TypeKind::GenericParam` owner/ordinal, never a display label.

Execution rows are:

```text
receiver =
  None
  | SemanticOnly || receiver-mode || receiver-type-digest32
  | Operand || receiver-mode || receiver-type-digest32 || u32(abi-position)

execution-argument =
  u32(argument-ordinal) || passing-tag8 || u32(slot-count) || execution-slot*

execution-slot =
  u32(slot-ordinal) || source-coordinate || u32(abi-position) || destination ||
  source-projection || optional(u32(alternative-index)) || evidence ||
  inferred-type-digest32 || optional(expected-type-digest32)

source-coordinate = Expression
  | CompactNumericElement || u32(element-ordinal)
destination = Parameter || u32(group) || u32(parameter)
  | Open || open-argument-id
open-argument-id = schema-digest32 || bytes(canonical CallableName UTF-8)
source-projection = Scalar
  | SpreadContainer || container-constructor
container-constructor = Vec | Seq | Slice
  | Array || ArrayLength canonical bytes
  | MapValue || map-kind-tag8 || key-type-digest32
evidence = Any
  | VariantCase || owner-type-digest32 || u32(case-ordinal) || payload-tag8
result = Value || value-type-digest32
  | Continuation || continuation-digest32
effect-row = tail-tag8 || optional(u32(effect-variable)) ||
  u32(effect-count) || sorted EffectSemanticDigest32*
```

Raw ExprId values in callee/receiver/argument source coordinates and
`CheckedCallSite` are excluded after issuer/owner validation. A Value callee
still hashes the `Value` execution tag so Direct and Value execution cannot
collide. Closed named passing hashes only its tag;
`OpenArgumentId` alone retains a canonical typed name. Receiver plus every slot
ABI position must be unique and contiguous. Optional encodings always include
the `None`/`Some` tag before payload.
Effect tail tags are Unknown=0, Closed=1, and Variable=2. Only Variable carries
the optional payload. Effects sort by their existing canonical typed identity,
not display spelling.

Callable schema digest grammar adds, for every parameter in group/index order,
the admission tag; Checked admission then encodes declared type digest,
consumer, alternative count, and each evidence/expected/action row.
UncheckedSupply has no declared/projection payload and validates Value consumer
plus Supply action. Parameter consumer and ordered value alternatives use the
tags below.

The schema argument-policy grammar replaces its unknown-named field with
Reject=0 or OpenSupply=1. No third tag, legacy reader, OpenChecked, or
OpenUnchecked variant remains. An execution `OpenArgumentId` is encoded only
in the destination row shown above and is cross-checked against the selected
schema digest and OpenSupply policy.

`LanguageIntrinsicGenericOwner` tags are OptionConstructor=0,
ResultConstructor=1, CollectionMap=2, FxExists=3, AgentSignal=4, and
AgentMetric=5. `CallableParameterConsumer` tags are Value=0,
DialoguePatch=1, and DialogueApplicationMetadata=2.
`CharacterDialogueFieldCoordinate` tags are Voice=0, Look=1, Stage=2,
Portrait=3, Focus=4, Cleanup=5, View=6, SourceLocale=7, Hooks=8, Style=9,
RichText=10, InlineFailure=11, and Custom=12 followed by the canonical custom
field ID. Dialogue application metadata tags are Id=0 and TextKey=1.

The core and final application digests thereby commit to exact
dispatch/candidate identity, schema, current group, inherited
application/solution, ordered bindings and deferred parameters, receiver,
argument ABI/source order, source projection, alternative index, typed
evidence, schema-derived action, inferred/final expected types,
result/continuation, final
effects, and function-value identity.

There is no digest cycle: the continuation binds only the already sealed
application core, while the final application binds that core and the completed
continuation digest. Neither the continuation nor its callable state refers to
the final application digest that contains it.

Private issuers and consume-only sealers prevent callers from minting an
alternative, solution, continuation, execution row, or application. Any future
decode/restore path validates through the same sealer rather than deriving
`Deserialize` directly.

## 8. Internal implementation sequence

These are compile-clean checkpoints inside the single C2 reviewable result,
not independently accepted authorities:

1. replace the recursive compatibility copies with the sole types-owned
   Recovery/SelectedCall/Invariant directional engine, make
   `first_mismatch` an independent strict structural diagnostic, delete the
   standalone array-length acceptance helper, and establish exact array,
   rigid-generic, and unresolved policy matrices;
2. establish the opaque non-`Clone` `TypeConstraintSolution`, exact
   rigid/bindable/future-eligible `TypeConstraintParameterScope`, lower
   traversal observer, and sorted completed binding iterator, with no lower
   callable-group or deferred ownership;
3. replace resolver accounting handoff with the exclusively borrowed
   `CandidateConstraintWorkSession<'_>`, pending previous/proposed full-report
   reservation, exactly-once infallible complete/drop commit, and
   cancellation/limit accounting;
4. add the callable-owned generic `TypeConstraintClient`, callback-lifetime
   `ExpectedHint::{Unchecked, Complete, Parametric}`, typed source result and
   phase/cause errors, narrow work-session access, isolated semantic probes,
   and correlated binding/probe-trace frontier;
5. implement branch-local Choice pruning followed by all-constraint
   normalization, occurs checking, final Choice pruning, exact source-order
   materialization of every correlated trace from one baseline per canonical
   binding, sealed-value coalescing/pruning, pair unicity, final projection,
   and run completion;
6. migrate mapping source projections, parameter alternatives, typed evidence,
   and the complete B3 base/receiver/argument/expected-result transaction;
7. migrate Option, Result, Agent, Collection, Reduction, and Fx schemas with no
   inference placeholders, plus fail-closed Traverse/Parallel deletion;
8. migrate dialogue fixed/custom rules and typed variant evidence, and route
   synthetic Dialogue through the same source callbacks and transaction;
9. replace candidate probes with one move-only prepared transaction, singleton
   move, multi-candidate full replay, and one private post-catalog/effect call
   analysis;
10. establish sema-root semantic-coordinate ownership, lower intrinsic generic
    owners, canonical digests, higher-owned sorted deferred rows on
    `FrozenCallTypeSolution`, cumulative continuation, and prepared-callee
    sealing;
11. publish the one final application, migrate execution projection and
    compiler/runtime-plan/tooling consumers, then reduce join to validation,
    Method enrichment, and move-only edge handoff; and
12. delete every old substitution, provisional call fact, solver relation,
    source reconstruction, and compatibility authority before running the full
    C2 gates.

No checkpoint may commit a public pending variant, fallback reader, dual call
fact, incomplete witness/form enum, or compiler reconstruction path.

## 9. Required differentials

Implementation must cover:

- ordinary optional generic `Option<i64>` versus clearable `None`, including
  equal final expected types but different alternative/solution/application
  identity and both authored source orders (`None` then `T`, and `T` then
  `None`), with identical failure precedence and no order-selected binding;
- illegal/aliased/local `None`, overlapping evidence, multiple fallback rows,
  custom clearable/non-clearable fields, and clear-capable rest rejection;
- Reject/OpenSupply unknown-name admission, schema/name-sensitive
  `OpenArgumentId`, impossible open/unchecked/checked slot combinations, and
  action derivation with no copied slot field;
- fixed spread versus typed rest, every container constructor, and constructor,
  array-length, map-key, alternative, evidence, and solution tampering;
- receiver-only inference, expected-result inference, terminal incomplete
  rejection, and continuation-owned deferred parameters;
- a spurious-Choice matrix where equal intermediate bindings carry different
  probe semantic branches, proving that a failed local Choice branch cannot
  prune another row or survive final SelectedCall pruning;
- callback-lifetime Complete, Parametric, and Unchecked expected hints,
  including exact sorted Parametric unbound identities and an
  expected-dependent probe whose final public facts come only from ordered
  materialization and never from its speculative checkpoint;
- every correlated trace for one canonical binding re-materialized from the
  same baseline in both source orders, equal final sealed values coalescing,
  different final sealed values remaining ambiguous, semantic failures
  pruning only their trace, and typed materialization errors retaining their
  source phase/cause;
- different bindings remaining distinct even when their visible source types
  match;
- group-zero bindings consumed by later groups and exact curried base tamper;
- inherited-solution rejection for wrong base/schema/scope, unknown or
  duplicate parameter, non-normalized binding, changed inherited binding, and
  incorrect deferred first-group ownership, plus legal immutable extension in
  a later group;
- every base-instantiation variant and payload-order/scalar/tag tamper;
- singleton/replay application-core and final-application digest equality;
- candidate producer-order invariance, canonical selected-index rewriting,
  equivalent duplicate coalescing, and digest-equal authority mismatch
  rejection;
- acyclic continuation sealing: prefix-core digest first, continuation digest
  second, and final application digest last, with tamper rejection at each
  boundary;
- equivalent HIR allocated in different raw ID orders producing identical
  lexical/function-value/resolved-callable/core/application/continuation
  digests, while a stable path, ordinal, schema, type, effect, or capture change
  changes the owning digest;
- standard Option/Result result closure without an override helper;
- the synthetic Dialogue site passing through the same sealer;
- selected execution tamper rejection and the absence of execution/result/
  continuation APIs on unselected outcomes;
- direct versus value-callee sealing, receiver/callee source tamper, and
  compiler parity from the sealed execution row without an outer callee fact or
  raw HIR/schema/name reread;
- recursive solver cancellation at every node/path/binding boundary, checked
  counter overflow, exact report accounting, and unchanged error precedence;
- CandidateConstraintWorkSession accounting on success and every failure,
  previous/proposed full-report reservation, attempted double completion,
  exclusive-borrow enforcement, and pending session/run drop infallibly
  committing exactly once while exposing no outcome;
- Recovery/SelectedCall/Invariant matrices for Error, `_`, Never, Choice,
  StageActor and other erased families, rigid enclosing generics,
  functions/effects, and arrays; strict structural `first_mismatch` versus
  directional Invariant acceptance; and exact constant/generic array lengths
  with recovered/inferred lengths accepted only by Recovery;
- compile-direction evidence that callable/final-analysis share the sole
  sema-root coordinate types and no callable-to-final-analysis dependency;
  and
- one inference-free callable join, Method enrichment, and move-only edge
  publication.
