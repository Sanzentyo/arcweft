# Sealed call application authority amendment

Status: `READY_FOR_IMPLEMENTATION`

Implementation verdict: `IMPLEMENTABLE`

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
same schema and lower constraint solution. Before catalogs/effects can seal it,
that exact solution is owned by one prepared graph node, with a partial node
using the move-only continuation carrier; afterward it is shared only through
the frozen solution handle.

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

pub struct CallableParameterAlternativeIndex(u32);

pub struct CallableParameterValueAlternative {
    index: CallableParameterAlternativeIndex,
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

`CallableParameterValueRule` construction assigns contiguous alternative
indexes in source-independent schema order and rejects a supplied row whose
index is not its exact position. `Any` occurs exactly once and last. Earlier
evidence rows are pairwise exclusive. The checker selects an alternative
through final typed semantic evidence, never through first successful type
inference, source spelling, or parameter optionality. A checked slot retains a
schema-relative `CallableParameterAlternativeIndex`, the evidence that
selected it, and the composed final expected type. It does not retain a
parallel `Supply`/`Clear` flag; consumers obtain the action from the selected
schema alternative.
`Clear` is legal only when the same parameter owns a `DialoguePatch` consumer.
The fixed and custom dialogue coordinates move from `final_analysis` to the
shared `character_dialogue` owner so schema construction and final patch rows
use the same typed identity.
`UncheckedSupply` has no expected projection, contributes no type constraint,
and always has action `Supply`; it cannot own a Clear alternative or a
non-Value consumer.

The argument mapper chooses how one source value supplies a logical slot, but
the types layer owns the projection algebra and its only constructor. This is
the same typed projection stored by lower equations, materialization requests,
and the final execution slot; callable code does not retain a parallel
projection enum or repeat the container match:

```rust
pub(crate) enum PreparedConstraintSourceProjection {
    Scalar,
    InferSpreadContainer { policy: ConstraintSourceContainerPolicy },
}

pub(crate) enum ConstraintSourceContainerPolicy { Positional, Named }

pub(crate) enum CheckedConstraintSourceProjection {
    Scalar,
    SpreadContainer(CheckedConstraintContainerConstructor),
}

pub(crate) enum CheckedConstraintContainerConstructor {
    Vec,
    Seq,
    Slice,
    Array { len: ArrayLength },
    MapValue { kind: MapKind, key: Box<TypeKind> },
}
```

`CheckedConstraintSourceProjection::derive` is the sole exhaustive
`(prepared projection, checked actual type)` match. `Scalar` accepts only the
identity projection. `Positional` admits exactly Vec, Seq, Slice, and Array and
retains the exact array length; `Named` admits exactly Map and retains its exact
kind and key. Its `compose_expected` owner method wraps a selected value
expected with that checked constructor. There is no callable-side
`from_prepared`, header reconstruction, or fallback to the item type.

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

### 3.2 Callable schema inventory and the single preparation gate

The types layer owns one exhaustive, metered-free schema-construction visitor
for generic occurrences. `TypeGenericUseCollector` walks every `TypeKind`
constructor, function parameter/result child, nominal argument, projection,
map key/value, and `ArrayLength`. It returns distinct sorted type-parameter and
const-parameter identities, coalesces repeated occurrences of the same
identity, and rejects a malformed identity. Callable schema code does not own
another recursive type walker.

`CallableSignatureSchema::try_new` invokes that collector for every checked
parameter in group order and then for the result. It seals the following
derived inventory beside the groups; callers cannot supply first-use rows:

```rust
pub(crate) struct CallableGenericParameterInventory {
    types: Arc<[CallableGenericTypeUse]>,
    rigid_consts: Arc<[CallableRigidConstUse]>,
}

pub(crate) struct CallableGenericTypeUse {
    parameter: GenericTypeParameterId,
    role: CallableSchemaGenericRole,
    first_use: CallableGenericFirstUse,
}

pub(crate) struct CallableRigidConstUse {
    parameter: GenericConstParameterId,
    first_use: CallableGenericFirstUse,
}

pub(crate) enum CallableSchemaGenericRole { Candidate, RigidReference }

pub(crate) enum CallableGenericFirstUse {
    Group(CallableGroupIndex),
    Result,
}
```

The schema constructor receives the declaration- or intrinsic-owned candidate
parameter inventory from the same typed schema issuer that creates the generic
IDs. Every declared candidate parameter must occur, every occurrence must be
classified exactly once, and all remaining foreign/enclosing occurrences are
`RigidReference`. A duplicate ID, a candidate declaration absent from the
schema, a result inconsistent with the recomputed first use, or an inferable
const parameter rejects schema construction. Const occurrences are retained
only as exact rigid references until the lower algebra has a real const-binding
authority. The inventory is derivable from and cross-checked against the
schema, while the schema digest continues to commit the owning parameter and
result types; no caller-supplied side table is trusted.

There is one analyzer-integration validation and preparation entry point in
`final_analysis/analyzer/calls/constraints.rs`. It orchestrates callable-owned
schema validation and types-owned plan construction; neither lower layer
imports `PreparedCallGraph`:

```rust
pub(crate) fn validate_and_prepare_call_constraints(
    graph: &PreparedCallGraph<AnalyzerPreparedCallPrefix>,
    site: CheckedCallSite,
    candidate: PreparedResolvedCallable,
    mapping: PreparedCallArgumentMapping,
    enclosing: &EnclosingGenericParameterScope,
    context_expected: Option<&TypeKind>,
) -> Result<PreparedCallConstraintSet, CallConstraintPreparationError>;

struct PreparedCallConstraintSet {
    issuer: CheckedCallSite,
    candidate: PreparedResolvedCallable,
    schema: CallableSignatureSchemaDigest,
    current_group: CallableGroupIndex,
    initialization: PreparedConstraintInitialization,
    base_constraint: Option<PreparedTypeConstraint>,
    receiver: Option<PreparedReceiverConstraint>,
    arguments: Box<[PreparedArgumentConstraint]>,
    expected_result: Option<TypeKind>,
}

enum PreparedCallConstraintSeed {
    None,
    Prepared(PreparedCallContinuationSeed),
    Frozen(Arc<FrozenCallTypeSolution>),
}

struct PreparedConstraintInitialization {
    parameter_scope: TypeConstraintParameterScope,
    continuation_seed: PreparedCallConstraintSeed,
}

enum CallConstraintPreparationError {
    Rejected(CallConstraintRejection),
    Invariant(CallConstraintInvariant),
}

enum CallConstraintRejection {
    DuplicateArgument,
    ParameterAlreadyBound,
    UnknownNamedArgument,
    MissingArgument,
    TooManyPositionalArguments,
    UnsupportedSpread,
}

enum CallConstraintInvariant {
    MalformedMapperSeal,
    MalformedSchemaInventory,
    MissingContinuationSeed,
    UnexpectedContinuationSeed,
    ForeignPreparedIssuer,
    MissingOrStalePreparedNode,
    InvalidPreparedNodeState,
    InvalidPreparedDependencyOrder,
    PreparedBaseMismatch,
    PreparedSchemaMismatch,
    PreparedGroupMismatch,
    PreparedDeferredMismatch,
    PreparedFunctionTypeMismatch,
    FrozenBaseMismatch,
    FrozenSchemaMismatch,
    FrozenCompletedGroupMismatch,
    FrozenPrefixCoreMismatch,
    FrozenDeferredMismatch,
    FrozenFunctionTypeMismatch,
    FrozenSolutionDigestMismatch,
    TerminalFutureEligibleParameter,
    Lower(TypeConstraintInvariant),
}
```

`PreparedCallConstraintSeed` and `PreparedConstraintInitialization` are owned
by `callable/constraints.rs`, not by final analysis. The integration gate holds
the resulting token inside `PreparedCallConstraintSet` but cannot construct or
open its fields; after completing all higher validation it invokes the one
callable-owned issuer method to pair the already sealed lower scope and exact
callable seed. Consequently `CandidateConstraintWorkSession::start` imports no
final-analysis type, and the token introduces no reverse layer dependency.

The entry point derives the group and optional seed from the candidate state;
the analyzer may not pass raw group/solution pieces. For a base candidate it
requires group zero and `None`. A pre-seal continuation candidate must contain
an issuer-bound `PreparedCallContinuationRef`; the entry point resolves it only
through the supplied graph and validates strict earlier-node dependency,
base/schema identity, completed/next group adjacency, deferred rows, and
projected function type, then mints one move-only
`PreparedCallContinuationSeed` for this run. A post-seal or restored
continuation must contain the exact `Frozen` handle and additionally validates
prefix core and frozen-solution digest. Every foreign, stale, wrong-state,
wrong-order, wrong-scope, or impossible prepared/frozen condition above is a
`CallConstraintInvariant`; it is not a semantic candidate rejection. Neither
route exposes a raw solution pair. The entry point then joins
the schema inventory, exact base instantiation,
receiver, enclosing rigid inventory, mapping, and terminal/continuation state
to construct the only lower parameter scope. Candidate parameters first owned
by the current or an already consumed group are bindable or immutable inherited
bindings; a later-group or result-only parameter is future-eligible only while
a continuation still exists; every foreign/enclosing parameter and every const
is rigid. Terminal preparation emits no future-eligible row.

The same operation validates and consumes the mapper result and constructs all
types-owned prepared source constraints described below. The mapper's closed
authored arity/name incompatibility algebra is `CallConstraintRejection`; type
incompatibility later enters `TypeConstraintRejection`. Malformed mapper/schema
evidence is an invariant because only sealed in-process producers can supply
it. Its returned value is the only way to start the callable constraint driver.
The gate pairs the derived scope and exact none/prepared/frozen seed into one
move-only, field-private `PreparedConstraintInitialization`. It has no `Clone`,
scope getter, seed getter, or public constructor. Starting a prepared-seed run
consumes this token and shares the carrier's exact
`Arc<TypeConstraintSolution>` only into lower initialization; a frozen-seed run
borrows the same field through `FrozenCallTypeSolution`. The graph borrow ends
before `AnalyzerCallConstraintClient` takes `&mut Analyzer`. No analyzer scan,
empty-seed fallback, `call_group()` recomputation, raw solution getter,
independent scope/inherited pair constructor, or driver start accepting raw
scope/solution pieces exists.

### 3.3 Lower solution and parameter scope

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
therefore return a typed `ParameterScope` invariant for an out-of-scope binding
or impossible binding attempt. A well-formed relation against a rigid enclosing
generic remains ordinary mismatch when the exact identities differ. The lower
scope does not know or store a callable group index. The higher callable sealer
alone proves which
future group first owns an eligible parameter and constructs the corresponding
`DeferredContinuationParameter`. A terminal call permits no such higher-owned
deferred row.

An inherited solution is admitted only through the validated prepared or
frozen seed of the exact previous continuation. The single preparation gate
above validates its issuer/base/schema/group identity and higher-owned deferred
rows and derives the new lower parameter scope. The callable driver borrows the
opaque carrier's `Arc<TypeConstraintSolution>` only while initializing lower;
the analyzer never extracts or stores it independently. The lower layer then
performs the sole binding-content walk and verifies that every inherited
binding is ordered, normalized, in scope, and closed for its sealed prefix under
`SelectedCall`. A canonical reference to an in-scope parameter eligible for the
current or a future group, such as `T -> U`, is closed for that prefix; an
unresolved placeholder or a missing required prior-group binding is not.
Callable preparation validates only carrier coordinates and metadata; it never
rescans, normalizes, copies, or repairs binding rows.

Inherited bindings are immutable canonical seeds: a later group may close or
extend them but cannot replace their meaning. Canonical inherited `{T -> U}`
followed by a current-group `U -> i32` constraint is legal and closes to
`{T -> i32, U -> i32}`. A completed inherited carrier containing
`{T -> U, U -> i32}` is noncanonical and returns
`InheritedSolutionInvariantKind::NonCanonical` under
`TypeConstraintInvariant::InheritedSolution` during
initialization. Every invalid inherited carrier is an invariant/seal/restore
failure, never a candidate mismatch and never authority to retry with an empty
scope or another overload.

### 3.4 One reserved work session

The callable layer creates one `CandidateConstraintWorkSession<'a>` with an
exclusive `&'a mut ResolverWork` borrow per candidate run:

```rust
pub(crate) struct CandidateConstraintWorkSession<'a> {
    work: &'a mut ResolverWork,
    reservation: PendingCandidateConstraintReport,
}

pub(crate) enum TypeConstraintInitializationFailure {
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintInvariant),
}

impl<D: ConstraintDomain> TypeConstraintTransaction<D> {
    fn initialize<A>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        inherited: Option<Arc<TypeConstraintSolution>>,
    ) -> Result<(), TypeConstraintInitializationFailure>;
}

impl<'a> CandidateConstraintWorkSession<'a> {
    fn start<D, C>(
        self,
        initialization: PreparedConstraintInitialization,
        client: C,
    ) -> Result<
        CandidateConstraintDriver<'a, D, C>,
        TypeConstraintInitializationFailure,
    >;
}
```

The pending reservation retains the exact `previous` full accounting report
and the only checked `proposed` full report. While the session or its completed
run exists, Rust's exclusive borrow prevents every other observation or update
of that `ResolverWork`. The lower solver never receives it, and source
callbacks receive only the narrow session methods required to charge their
work.

Initialization is fallible and cannot park a malformed inherited carrier in a
transaction's ordinary `first_failure`. Its closed result is exactly
`TypeConstraintInitializationFailure::{Abort, Invariant}`; `Rejected` and
`FatalSource` are unconstructible before current constraints or source
callbacks. `start` consumes only the sealed initialization token plus the
client, privately opens the token once, and propagates that closed result before
returning a driver. No overload-selection caller can supply or pair a raw
`TypeConstraintParameterScope` and `Option<Arc<TypeConstraintSolution>>`.
The raw inherited option shown on the lower `initialize` implementation is
private to `start` after that one token opening; it is not reexported, callable
from analyzer code, or another construction boundary.
Because no driver exists on this path, no probe or materialization callback can
execute. The consumed session still drops its pending reservation and commits
the exact attempted initialization accounting once. No special caller-side
accounting or retry path exists.

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

### 3.5 Callback-only source authority

The callable-owned candidate driver is generic over a
`TypeConstraintClient`. The client is the only bridge to expression checking
and semantic facts; the lower types transaction receives only types-owned
constraints, projected callback results, and its traversal context. Its
checkpoint `open`/`close` methods are private hooks called only inside the
driver's ticketed begin/close operations; they are not an unticketed callback
API or source-coordinate authority:

```rust
pub(crate) trait ConstraintDomain {
    type Source: Copy + Ord;
    type AlternativeIndex: Copy + Eq + Ord;
    type EvidenceRule: Eq;
    type CheckedEvidence: Eq;
    type ProbeSemanticBranch: Eq;
    type SealedBranchValue: Eq;
    type Projection: Eq + Ord;
    type SourceErrorCause;
    type ClientInvariant;

    fn evidence_accepts(
        rule: &Self::EvidenceRule,
        checked: &Self::CheckedEvidence,
    ) -> bool;
    fn client_invariant_source(invariant: &Self::ClientInvariant) -> Self::Source;
    fn empty_sealed_branch() -> Self::SealedBranchValue;
}

pub(crate) enum PreparedSourceConstraint<D: ConstraintDomain> {
    Unchecked {
        source: D::Source,
    },
    Checked {
        source: D::Source,
        source_projection: PreparedConstraintSourceProjection,
        alternatives: Box<[PreparedSourceAlternative<D>]>,
    },
}

pub(crate) struct PreparedSourceAlternative<D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    evidence: D::EvidenceRule,
    value_expected: TypeKind,
}

pub(crate) enum ProjectedExpectedHint<'h> {
    Complete(&'h TypeKind),
    Parametric {
        expected: &'h TypeKind,
        unbound: &'h [GenericTypeParameterId],
    },
}

pub(crate) struct SourceAlternativeHint<'h, D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    evidence: &'h D::EvidenceRule,
    value_expected: ProjectedExpectedHint<'h>,
    source_projection: PreparedConstraintSourceProjection,
}

pub(crate) enum ExpectedHint<'h, D: ConstraintDomain> {
    Unchecked,
    Alternatives(&'h [SourceAlternativeHint<'h, D>]),
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

pub(crate) enum SourceCallbackFailure<D: ConstraintDomain> {
    Fatal(SourceError<D::Source, D::SourceErrorCause>),
    Abort(TypeConstraintAbort),
    Invariant(D::ClientInvariant),
}

pub(crate) enum TypeConstraintError {
    Rejected(TypeConstraintRejection),
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintInvariant),
}

pub(crate) enum TypeConstraintFailure<D: ConstraintDomain> {
    Rejected(TypeConstraintCandidateFailure<D>),
    FatalSource(SourceError<D::Source, D::SourceErrorCause>),
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintFailureInvariant<D>),
}

pub(crate) enum TypeConstraintFailureInvariant<D: ConstraintDomain> {
    Constraint(TypeConstraintInvariant),
    Client(D::ClientInvariant),
}

pub(crate) enum TypeConstraintCandidateFailure<D: ConstraintDomain> {
    Constraint(TypeConstraintRejection),
    Source(SourceError<D::Source, Box<[D::SourceErrorCause]>>),
}

pub(crate) enum TypeConstraintRejection {
    Mismatch,
    AmbiguousSolution { actual: usize },
    CyclicInstantiation { parameter: GenericTypeParameterId },
    UnresolvedType,
    IncompleteInstantiation { parameter: GenericTypeParameterId },
}

pub(crate) enum TypeConstraintAbort {
    Cancelled,
    ArithmeticOverflow,
    WorkLimit { requested: u64, consumed: u64, limit: u64 },
    NodeLimit { actual: u64, limit: u64 },
    BranchLimit { actual: u64, limit: u64 },
    BindingLimit { actual: u64, limit: u64 },
    SourceProbeLimit { actual: u64, limit: u64 },
    MaterializationLimit { actual: u64, limit: u64 },
}

pub(crate) enum TypeConstraintInvariant {
    InheritedSolution(InheritedSolutionInvariant),
    ParameterScope(TypeConstraintParameterScopeInvariant),
    PreparedSource(PreparedSourceConstraintInvariant),
    SourceProtocol(TypeConstraintSourceProtocolInvariant),
    Projection(TypeConstraintProjectionInvariant),
}

pub(crate) struct InheritedSolutionInvariant {
    kind: InheritedSolutionInvariantKind,
    parameter: Option<GenericTypeParameterId>,
}

pub(crate) enum InheritedSolutionInvariantKind {
    OutOfScope,
    RigidBinding,
    DuplicateOrUnordered,
    SelfBinding,
    Forbidden,
    OccursOrCycle,
    Unclosed,
    NonCanonical,
}

pub(crate) enum SourceProbeSelection<A, E> {
    Unchecked,
    Checked { alternative: A, evidence: E },
}

pub(crate) struct SourceProbeResult<D: ConstraintDomain> {
    actual: TypeKind,
    canonical_branch: D::ProbeSemanticBranch,
    selection: SourceProbeSelection<D::AlternativeIndex, D::CheckedEvidence>,
}

pub(crate) enum SourceProbeOutcome<D: ConstraintDomain> {
    Accepted(SourceProbeResult<D>),
    Rejected(D::SourceErrorCause),
}

pub(crate) enum MaterializationOutcome<S, V, C> {
    Sealed(V),
    Rejected { source: S, cause: C },
}

pub(crate) struct SourceCallbackTicket<D: ConstraintDomain> {
    identity: SourceCallbackTicketIdentity,
    authority: SourceCallbackAuthority<D>,
}

#[derive(Clone)]
struct SourceCallbackTicketIdentity {
    issuer: Arc<SourceCallbackTicketIssuer>,
    ordinal: u64,
}

struct SourceCallbackTicketIssuer;

pub(crate) enum SourceCallbackAuthority<D: ConstraintDomain> {
    Probe { source: D::Source },
    Materialize { sources: Box<[D::Source]> },
}

pub(crate) struct BoundSourceCheckpoint<C> {
    identity: SourceCallbackTicketIdentity,
    checkpoint: C,
}

pub(crate) enum MaterializedSourceRequest<'h, D: ConstraintDomain> {
    Unchecked {
        source: D::Source,
        canonical_branch: &'h D::ProbeSemanticBranch,
    },
    Checked {
        source: D::Source,
        alternative: D::AlternativeIndex,
        evidence: &'h D::CheckedEvidence,
        source_projection: &'h CheckedConstraintSourceProjection,
        expected: &'h TypeKind,
        canonical_branch: &'h D::ProbeSemanticBranch,
    },
}

pub(crate) trait TypeConstraintClient<D: ConstraintDomain> {
    type ProbeCheckpoint;
    type MaterializationCheckpoint;
    type PreparedSealedBranchValue;

    fn probe_source<'h>(
        &mut self,
        source: D::Source,
        hint: ExpectedHint<'h, D>,
        checkpoint: &mut Self::ProbeCheckpoint,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>;

    fn open_probe_checkpoint(
        &mut self,
    ) -> Result<Self::ProbeCheckpoint, TypeConstraintSourceProtocolInvariant>;

    fn close_probe_checkpoint(
        &mut self,
        checkpoint: Self::ProbeCheckpoint,
    ) -> Result<(), TypeConstraintSourceProtocolInvariant>;

    fn open_materialization_checkpoint(
        &mut self,
    ) -> Result<Self::MaterializationCheckpoint, TypeConstraintSourceProtocolInvariant>;

    fn materialize_sources<'h, I>(
        &mut self,
        sources: I,
        checkpoint: &mut Self::MaterializationCheckpoint,
        work: &mut CandidateConstraintWorkSession<'_>,
    ) -> Result<
        MaterializationOutcome<D::Source, Self::PreparedSealedBranchValue, D::SourceErrorCause>,
        SourceCallbackFailure<D>,
    >
    where
        I: IntoIterator<Item = MaterializedSourceRequest<'h, D>>;

    fn close_materialization_checkpoint(
        &mut self,
        checkpoint: Self::MaterializationCheckpoint,
        sealed: Option<Self::PreparedSealedBranchValue>,
    ) -> Result<Option<D::SealedBranchValue>, TypeConstraintSourceProtocolInvariant>;
}

impl<'a, D, C> CandidateConstraintDriver<'a, D, C>
where
    D: ConstraintDomain,
    C: TypeConstraintClient<D>,
{
    fn begin_probe_callback(
        &mut self,
        source: D::Source,
    ) -> Result<
        (
            SourceCallbackTicket<D>,
            BoundSourceCheckpoint<C::ProbeCheckpoint>,
        ),
        TypeConstraintFailure<D>,
    >;

    fn close_probe_callback(
        &mut self,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::ProbeCheckpoint>,
        attempt: Result<SourceProbeOutcome<D>, SourceCallbackFailure<D>>,
    ) -> Result<SourceProbeOutcome<D>, TypeConstraintFailure<D>>;

    fn begin_materialization_callback(
        &mut self,
        sources: Box<[D::Source]>,
    ) -> Result<
        (
            SourceCallbackTicket<D>,
            BoundSourceCheckpoint<C::MaterializationCheckpoint>,
        ),
        TypeConstraintFailure<D>,
    >;

    fn close_materialization_callback(
        &mut self,
        ticket: SourceCallbackTicket<D>,
        checkpoint: BoundSourceCheckpoint<C::MaterializationCheckpoint>,
        attempt: Result<
            MaterializationOutcome<
                D::Source,
                C::PreparedSealedBranchValue,
                D::SourceErrorCause,
            >,
            SourceCallbackFailure<D>,
        >,
    ) -> Result<
        MaterializationOutcome<D::Source, D::SealedBranchValue, D::SourceErrorCause>,
        TypeConstraintFailure<D>,
    >;
}
```

#### Failure disposition

This four-way algebra is the only lower exit contract. `Rejected` means that a
well-formed source-program candidate was semantically incompatible: ordinary
mismatch, ambiguity, a cycle introduced by the current constraints, unresolved
or incomplete inference, or one or more typed source rejections. `FatalSource`
means an authoritative probe/materialization callback returned
`Err(SourceCallbackFailure::Fatal(ticket-matching SourceError))` rather than an
ordinary rejected outcome. `SourceCallbackFailure::Abort` preserves callback
work cancellation, checked overflow, and every configured limit as `Abort`.
`SourceCallbackFailure::Invariant` moves a domain-owned invariant raised while
checking the full source expression, including a nested call's prepared,
frozen, lower, or projection invariant, into the final `Invariant` disposition.
A wrong source, phase, ticket, or checkpoint is
`Invariant(SourceProtocol(..))`, not `FatalSource`.
`Abort` owns
cancellation, checked arithmetic overflow, and every configured work, node,
branch, binding, source-probe, or materialization limit. `Invariant` owns both
sealed/lower protocol violations and the domain-owned client invariant payload;
the two are distinguished by `TypeConstraintFailureInvariant::Constraint` and
`TypeConstraintFailureInvariant::Client` and are never flattened together.
A lower `TypeConstraintError::Invariant` converts only to the `Constraint`
branch; only `SourceCallbackFailure::Invariant(D::ClientInvariant)` can create
the `Client` branch.

The nested invariant owners are closed typed algebras. `ParameterScope` owns
foreign type/const IDs, rigid or unsupported binding attempts, and duplicate
scope rows; `PreparedSource` owns empty, unordered, duplicate-coordinate,
fallback, and spread-plan violations; `SourceProtocol` owns wrong source/phase,
unknown alternative, invalid evidence, and ticket violations; and `Projection`
owns duplicate or mismatched projection keys. The current inherited kinds move
unchanged into `InheritedSolutionInvariantKind`; they are not retained as a
variant of `TypeConstraintRejection`.

There is no lower `SourceFailureCause::{Rejected, Fatal}` wrapper. An ordinary
source rejection is stored directly as the boxed cause list in
`TypeConstraintCandidateFailure::Source`. The callback boundary instead has
the closed `SourceCallbackFailure::{Fatal, Abort, Invariant}` channel. It
contains no `Rejected`, so semantic rejection remains an outcome; its
`Invariant(D::ClientInvariant)` is the sole typed escape for an invariant raised
inside the full expression boundary, while ticket/checkpoint protocol
validation remains driver-owned. The driver maps a validated `Fatal` directly
to `TypeConstraintFailure::FatalSource`, `Abort` directly to
`TypeConstraintFailure::Abort`, and client invariant directly to
`TypeConstraintFailure::Invariant(TypeConstraintFailureInvariant::Client(..))`.
None is inferred from a cause, side state, or panic, and none is flattened into
another disposition.

For every callback the driver checks/charges work, mints one non-`Clone`
`SourceCallbackTicket`, asks the client to open its raw checkpoint, and
immediately wraps that checkpoint in `BoundSourceCheckpoint` with the ticket's
private issuer/ordinal. A probe ticket authorizes `Probe` for exactly one source
coordinate. A materialization ticket authorizes `Materialize` for the exact
ordered source-coordinate list projected from that closed trace; the same list
drives the request iterator. Ticket identities and bound checkpoints are
generation-local affine capabilities with no encoder, getter, or semantic
identity role. Ticket minting is the sole source-probe/materialization count
charge; callback expression work uses the same session but cannot charge or
mint a second source callback. Cancellation, overflow, or a limit while minting
returns `Abort` before a checkpoint opens; failure of the client's private open
hook is `Invariant(SourceProtocol(..))` before the callback executes.

`SourceCallbackFailure`, the ticket issuer/authority, bound checkpoint, and
client hooks live with the callable driver in `callable/constraints.rs`. The
types transaction neither imports them nor stores a parallel callback plan; it
continues to own prepared source rows and receives only the driver's validated
typed submission/failure. `ConstraintDomain::ClientInvariant` is an associated
type with no analyzer-independent behavior or `Clone`/`Eq`/`Ord` requirement,
so the generic types layer can move it without importing or interpreting an
analyzer type.

The driver-owned close operation consumes the ticket, bound checkpoint, and
raw callback attempt together. Before delegating rollback/extraction to the
client closer it validates the ticket issuer/ordinal, phase, full ordered
materialization authority, every returned rejection/error/client-invariant
coordinate through its domain owner, and the checkpoint binding. Probe `Fatal`
or client invariant must name the ticket's sole source; materialization `Fatal`,
`Rejected`, or client invariant must name one coordinate in the ticket's exact
ordered set. A mismatch is `Invariant(SourceProtocol(..))` and the
callback output is never trusted by itself. When the checkpoint binding is
valid but the attempt is malformed, the driver first uses the rollback-only
client close and then returns the invariant; when the binding itself is invalid,
the enclosing candidate rollback owns cleanup.

The client hooks cannot reclassify an attempt. Probe close receives only the
inner checkpoint and returns `()` after rollback. Materialization close
receives `Some(prepared)` only for a validated `Sealed(prepared)` outcome and
returns the extracted sealed value; every Rejected/Fatal/Abort/client-Invariant
path passes `None` and performs rollback. The driver retains the original validated
disposition and returns it only after successful close. A foreign, stale,
already-consumed, non-LIFO, or outcome/close-shape mismatch returns the closed
`TypeConstraintSourceProtocolInvariant`, which the driver wraps in
`TypeConstraintFailureInvariant::Constraint` under
`TypeConstraintInvariant::SourceProtocol`.
Close-authority invariant has precedence over the raw attempt, including a
client invariant. Otherwise a ticket-matching `Fatal` alone becomes
`FatalSource`, callback `Abort` remains `Abort`, and the exact moved client
invariant becomes `TypeConstraintFailureInvariant::Client` through closer and
driver propagation by construction.

`PreparedSourceConstraint` is a types-owned input, not an analyzer hint object.
The callable preparation gate moves into it the exact schema-keyed alternatives
and value-expected templates. It validates strict alternative-index order,
unique keys, one terminal fallback, and the mapper's one source projection.
Prepared sources are in strict authored argument/physical-slot order and their
typed source coordinates are unique; a duplicate returns `PreparedSource`
invariant before the first callback and a duplicate sealed submission returns
`SourceProtocol` invariant when it violates the affine ticket.
The lower owner moves accepted checked evidence and probe-semantic branches
into private `Arc` cells before a frontier can fork. Domain values therefore do
not need `Clone`, `Copy`, or `Ord`; path copies share the exact issuer-sealed
value and trace order remains the deterministic source/branch-derivation order.
The lower transaction alone substitutes a frontier binding into those
templates and constructs the borrowed hints. `Complete` and `Parametric` have
their prior meanings, but now exist per keyed alternative. The callback may
select only one supplied key and return checked evidence for that key; it never
returns, composes, or rewrites an expected type. The lower domain hook validates
that issuer-sealed complete evidence against every rule and requires the
returned key to be the first accepting nonfallback row, or the terminal `Any`
row only when no earlier row accepts. Unknown keys, an `Any` shortcut around an
earlier match, inconsistent evidence, and more than one nonfallback match are
source protocol violations.

For `Scalar`, the selected value expected is also the source expected. For
`InferSpreadContainer`, the current contract admits only the single
`Any + Identity + Supply` alternative. The client probes its actual container
without inventing a container expected; after receiving the actual type the
lower owner derives the exact checked source projection and composes the
expected container, including array length or map kind/key. A malformed
container is a lower mismatch. No callback can return a projected expected or
choose a container constructor.

An accepted checked probe is converted inside the lower transaction into one
source equation and one correlated trace row containing the selected
alternative, checked evidence, checked source projection, expected template,
actual type, and canonical semantic branch. At final closure the lower owner
normalizes the template through the unique candidate solution, composes the
source projection, rechecks the whole equation, and moves a
`ClosedConstraintProbe` with its `final_expected` into the closed
materialization request row. `MaterializedSourceRequest::Checked` is only a
borrowed view of that row. The final keyed projection returned to callable
sealing retains the
same alternative, evidence, projection, actual, and final expected; none is
re-derived by the analyzer or callable layer. Unchecked sources remain
`Unchecked` through every phase and cannot acquire an alternative or expected.

Every source probe executes in a fresh client-owned semantic checkpoint;
evidence alternatives attempted inside a probe use nested checkpoints and are
all closed before return. The driver mints the one-source probe ticket, calls
the client's private `open_probe_checkpoint` hook, binds the returned checkpoint
to that ticket, and later consumes ticket, bound checkpoint, and attempt in
`close_probe_callback`. After driver validation the client
`close_probe_checkpoint` hook receives only the inner checkpoint and rolls it
back; the driver retains and returns accepted, rejected, valid-fatal, abort, or
client invariant. Valid fatal becomes `FatalSource`, callback work abort remains
`Abort`, a valid client invariant remains `Invariant(Client(..))`, and a
ticket/checkpoint violation becomes `Invariant(Constraint(SourceProtocol(..)))`.

Every materialization similarly starts from one baseline. The driver derives
the exact ordered source-coordinate list from the closed request rows, mints
the materialization ticket, opens and binds one affine checkpoint, and consumes
ticket, checkpoint, and raw attempt in `close_materialization_callback`. Only
then does `close_materialization_checkpoint` receive `Some(prepared)` for a
validated sealed outcome or `None` for a rollback-only outcome.
On `Sealed(prepared)`, the analyzer performs
`SemanticFactState::extract_and_rollback`, combines the move-only semantic
projection with the prepared source rows, and returns the final sealed value.
On ordinary rejection, valid `Fatal`, callback `Abort`, or client `Invariant`
it performs rollback and forwards the typed `Rejected`, `FatalSource`, `Abort`,
or information-preserving client-invariant disposition; on a ticket/checkpoint
violation it returns the constraint `SourceProtocol` invariant with precedence
and the enclosing candidate rollback owns cleanup. There is no unconditional
driver rollback after a successful extract, no commit path, and no reusable
ticket or checkpoint.

`SealedBranchValue` deliberately requires `Eq`, not `Ord`: lower coalescing
needs exact equality but never semantic ordering. The analyzer implementation
owns `AnalyzerCallConstraintDomain`, `AnalyzerCallConstraintSource`,
`AnalyzerProbeSemanticBranch`, `AnalyzerCallSealedBranch`,
`AnalyzerCallProjectionKey`, and `AnalyzerCallSourceFailureCause` in
`final_analysis/analyzer/calls/constraints.rs`. Common callable coordinates are
imported from their existing owners; the types layer never imports an analyzer
type. `AnalyzerCallSealedBranch` is move-only and compares an ordered list of
client-owned materialization outcomes plus exact `CandidateSemanticProjection`
equality.
That projection implements manual `PartialEq/Eq` by issuer identity and every
typed map key and optional value. Prepared-graph deltas use exact
dependency-preserving isomorphism: nodes are compared in call-site/topological
order, references to nodes inside the delta are rewritten to that local
ordinal for comparison, references to an existing baseline node compare their
same-issuer coordinate, and every payload field is compared by its owner.
Allocation-only node IDs do not affect equality. This is a borrowed comparison
view over the move payload, not a second stored graph or digest identity. The
projection has no `Clone` or `Ord`. A cached digest or ordering key may
accelerate candidate lookup, but it is only a comparison candidate and must be
followed by this full equality; it never replaces or copies the move payload.

The analyzer domain mapping is closed and local:

```rust
pub(crate) enum AnalyzerCallConstraintSource {
    Receiver { source: ExprId },
    Argument {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
        source: CheckedCallArgumentSlotSource,
    },
}

pub(super) struct AnalyzerProbeSemanticBranch {
    decisions: Arc<[AnalyzerProbeSemanticDecision]>,
}

pub(super) enum AnalyzerCallProjectionKey {
    BaseInstantiation,
    Receiver,
    Argument {
        argument: HirCallArgumentOrdinal,
        slot: CallableArgumentSlotIndex,
    },
    Result,
    Future(GenericTypeParameterId),
}

pub(super) struct AnalyzerCallSealedBranch {
    outcomes: Box<[AnalyzerMaterializedSourceOutcome]>,
    projection: CandidateSemanticProjection,
}

pub(crate) enum AnalyzerCallClientInvariant {
    NestedCall {
        source: AnalyzerCallConstraintSource,
        invariant: Box<CallAnalysisInvariant>,
    },
}

impl AnalyzerCallClientInvariant {
    fn nested_call(
        source: AnalyzerCallConstraintSource,
        invariant: CallAnalysisInvariant,
    ) -> Self;
}

impl ConstraintDomain for AnalyzerCallConstraintDomain {
    type Source = AnalyzerCallConstraintSource;
    type AlternativeIndex = CallableParameterAlternativeIndex;
    type EvidenceRule = CallableSemanticValueEvidenceRule;
    type CheckedEvidence = CheckedSemanticValueEvidence;
    type ProbeSemanticBranch = AnalyzerProbeSemanticBranch;
    type SealedBranchValue = AnalyzerCallSealedBranch;
    type Projection = AnalyzerCallProjectionKey;
    type SourceErrorCause = AnalyzerCallSourceFailureCause;
    type ClientInvariant = AnalyzerCallClientInvariant;
    // evidence_accepts, client_invariant_source, and empty_sealed_branch are
    // exhaustive owner methods.
}
```

`AnalyzerProbeSemanticDecision` is the expression checker's canonical ordered
typed decision transcript for one generation; it is not a semantic-fact copy,
source spelling, or digest-only identity. The source enum is the complete set
of expression callbacks: base instantiation and expected result are ordinary
lower equations, while receiver and physical argument slots are source
constraints. `AnalyzerCallSourceFailureCause` retains the authoritative typed
expression or semantic-rejection cause. Malformed evidence, checkpoint, and
projection protocol conditions belong to the typed lower/call invariant
algebras instead of this source cause. `AnalyzerMaterializedSourceOutcome` is
constructed
one-for-one in request order and retains only the client-owned semantic outcome;
it does not copy the lower alternative, evidence, source projection, actual, or
final expected. Those remain on the closed lower trace and are paired with the
sealed value in the materialized record and final keyed projection. Empty-source
candidates use the domain's one empty sealed branch with an empty exact
semantic projection.

Receiver and argument checking are full expression-analysis boundaries. If a
nested call returns `CallAnalysisFailure::Invariant(invariant)`, the expression
owner consumes it with the private
`AnalyzerCallClientInvariant::nested_call(source, invariant)` capability and
returns `SourceCallbackFailure::Invariant` from the outer source callback. The
payload retains the exact nested `CallAnalysisInvariant`, including prepared,
frozen, lower inherited, or projection detail, plus the outer source coordinate
used by `client_invariant_source`. It is not converted to
`AnalyzerCallSourceFailureCause`, `FatalSource`, `Abort`, side-state, a hard
error, or a panic. The type is move-only and can be minted only at this boundary.

Both callbacks receive only the narrow mutable candidate work session needed
to charge nested expression/source work. They cannot mint callback tickets,
increment the source-callback count, or access the underlying `ResolverWork` or
pending accounting report. Its charge/check methods return
the exact `TypeConstraintAbort`; callback code must forward cancellation,
overflow, and every limit as `SourceCallbackFailure::Abort`. A `SourceError`
inside `SourceCallbackFailure::Fatal` retains the claimed source,
`Probe`/`Materialize` phase, and typed cause; the driver validates that claim
against the ticket before it can become `FatalSource`. Neither channel is
flattened into a boolean mismatch or string diagnostic.

### 3.6 Correlated frontier and final materialization

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
5. close every retained source equation, preserving its selected alternative,
   evidence, checked source projection, actual, and normalized final expected;
6. before opening any materialization checkpoint or invoking any
   materialization callback, project every original inherited row through the
   current canonical binding and prove that the result equals that binding's
   final row for the same parameter; return
   `Invariant(TypeConstraintInvariant::InheritedSolution(..NonCanonical..))`
   if current work replaced rather than monotonically closed an inherited
   meaning;
7. for every correlated trace inside each canonical binding, reset the client
   to the same pre-probe semantic baseline, derive and charge one affine
   materialization ticket for that trace's exact closed source list in authored
   argument/slot order, bind the opened checkpoint to it, and invoke
   `materialize_sources` with those same coordinates and lower-derived final
   expectations;
8. consume the ticket, bound checkpoint, and attempt in the driver-owned close,
   validate their authority, then close the client checkpoint; prune a trace
   that semantically no longer satisfies final materialization,
   retain a typed source error as an error, seal every successful private
   branch value, and coalesce equal sealed values within that binding by their
   exact equality;
9. discard a binding with no surviving value, reject semantic-branch ambiguity
   when one binding has more than one distinct final sealed value, then require
   unicity of the surviving `(canonical binding, sealed branch value)` pair
   across bindings;
10. project every checked slot, receiver, result, and future-eligible parameter,
   together with each closed source alternative/evidence/projection/type row,
   from that unique pair for the higher callable sealer; and
11. `finish`, followed by the exactly-once `run.complete()` accounting commit.

Step 6 is not a second inherited-carrier validator. Initialization already
validated the seed's own rows exactly once before any source callback; step 6
checks the different, path-dependent invariant that current constraints only
extend that seed. It accepts inherited `{T -> U}` plus current `U -> i32`
because projecting the original `T -> U` row yields the final `T -> i32` row.

Two Choice paths that reach the same binding are not represented by one
optional branch token. That binding retains every correlated trace through
materialization. Traces that produce the same sealed final value coalesce;
only different final values for that binding create semantic-branch ambiguity.
Different canonical bindings remain distinct even if their source facts look
the same. Zero pairs is `Rejected(Mismatch)`, more than one surviving pair is
`Rejected(AmbiguousSolution)`, a cycle introduced by current constraints is
`Rejected(CyclicInstantiation)`, and a bindable terminal parameter that remains
unclosed is `Rejected(IncompleteInstantiation)`. Callable diagnostics may
project those typed lower rejections, but do not change their disposition.

Failure precedence is callable carrier preparation invariant, lower inherited
initialization invariant/abort, base instantiation, receiver, first authored
argument/slot, expected result, normalization or cycle, final Choice rejection,
post-extension inherited immutability invariant, first source-order
materialization failure, pair ambiguity, incomplete terminal/deferred closure,
projection validation, then score. Mandatory cancellation and metering checks
still abort at the operation boundary where they are observed. A later source
failure cannot replace an earlier source failure. Within a ticketed close,
invalid ticket/checkpoint authority takes `SourceProtocol` invariant precedence;
otherwise a source result retains its exact `Rejected`, `FatalSource`, `Abort`,
or client-invariant disposition instead of becoming a generic mismatch.

`materialize_sources` is not a speculative shortcut. Every correlated trace
of each canonical binding is re-evaluated from the identical client baseline
in an isolated transaction, in exact source order, and may produce a move-only
private prepared branch. Nonselected materializations are rolled back. This
preserves first-source failure precedence and prevents facts from one binding
or Choice path leaking into another. Final public slot facts contain only types
and evidence projected after this phase; prefix probe hints are diagnostic
evidence only.

### 3.7 B3 ownership and replay sequence

The B3 integration sequence is one candidate-wide operation:

1. consume the mapper result through
   `validate_and_prepare_call_constraints`, which validates the schema generic
   inventory and continuation and creates every types-owned source plan, the
   exact parameter scope paired with exactly one none/prepared/frozen
   continuation seed inside one move-only
   `PreparedConstraintInitialization`, without publishing facts;
2. exclusively borrow the query-local `ResolverWork` into one candidate work
   session and call fallible `CandidateConstraintWorkSession::start` with only
   that initialization token and the analyzer client, returning its closed
   `Abort`/`Invariant` before a callback if initialization fails;
3. through the returned driver, add base, receiver, source-ordered argument,
   and expected-result
   constraints to one transaction;
4. complete the correlated solve and affine ordered materialization once;
5. move the unique `Arc<TypeConstraintSolution>`, exact coalesced sealed branch
   value, closed source rows, final projections, and score into
   `PreparedCandidateTransaction`; and
6. consume that transaction into one prepared graph node; a partial result is
   wrapped by the move-only `PreparedCallContinuation` and immediately exposes
   only its issuer-bound reference, while a terminal result remains a selected
   value node. Keep the graph private until the C sealer consumes it.

`analyze_call` first destructures `ResolvedCallQuery`, so `ResolverWork` is a
local value rather than a field behind the analyzer borrow. Mapping and the
single preparation gate, including prepared-graph reference resolution and
one-run seed plus `PreparedConstraintInitialization` minting, finish before a
session starts and release the graph borrow. Moving the token into `start` does
not reborrow the graph or expose its paired fields. The driver then owns
the exclusive session while `AnalyzerCallConstraintClient` separately borrows
`&mut Analyzer`; callbacks cannot reach the local work except through the
narrow callback capability. Nested call analysis executes synchronously inside
that expression callback and returns any `CallAnalysisInvariant` through the
move-only domain client-invariant payload before the callback borrow ends; it
does not publish a side-channel failure or reborrow the underlying work.
`driver.finish()` consumes and drops the client,
ending the analyzer borrow while the returned run still owns the work
reservation. `run.complete()` then commits and releases the local work. No
`RefCell`, raw pointer, split analyzer facade, copied resolver report, or
re-entrant resolver borrow is permitted.

The callable driver preserves the lower disposition without reinterpretation:
`Rejected` and `FatalSource` retain their exact source rows and causes, `Abort`
retains the operational reason whether raised by solver work, ticket minting,
or callback work. `TypeConstraintFailureInvariant::Constraint` maps through
`CallConstraintInvariant::Lower`, while
`TypeConstraintFailureInvariant::Client` moves unchanged into
`CallAnalysisInvariant::Client`; the generic driver never inspects the latter.
Preparation has neither a source callback nor a metered work session and thus
cannot produce `FatalSource` or `Abort`. No layer turns either invariant branch
into a rejection, diagnostic mismatch, fatal source, abort, panic-only branch,
or retry with a reconstructed carrier.

`TypeParameterSubstitutions` may remain for nominal-only paths, but call
selection, join, continuation, and execution never use it or re-run `observe`.
There is no per-equation run, caller-side binding merge, expected-dependent
Option/Result schema, result override, or join-side inference.

For a singleton candidate, the selected path moves its complete prepared
transaction and move-only sealed semantic projection into private call
analysis; no checkpoint, solution, source branch, or argument vector is cloned
into publication. Prepared graph insertion splits and applies that projection
exactly once inside its atomic private-publication transaction, then stores only
the projection-free selected record. For multiple candidates,
every full `Probe` run includes base,
receiver, every source, result, closure, materialization, final projection, and
score, then leaves the analyzer at the common rolled-back baseline. Selection
retains only the immutable prepared descriptor and diagnostic score. It starts
a fresh checkpoint and new reserved work session and performs that same entire
operation in `SelectedReplay`. The replay must produce the same application
core inputs and score before its one semantic projection is applied during
prepared graph insertion. Replaying
only arguments, reusing a probe projection, or mixing a probe solution with a
replayed source vector is forbidden. The replay transaction is moved to the
private call analysis and later consumed by the C sealer.

Analyzer selection treats the four dispositions as follows. `Rejected` records
typed per-candidate rejection evidence and may continue to another overload.
`FatalSource`, `Abort`, and `Invariant` roll back the active candidate and abort
the whole call query immediately; a prior or later successful overload cannot
mask them. This rule is identical for singleton, multi-candidate `Probe`, and
`SelectedReplay`, and therefore does not depend on producer order. An invariant
from either `CallAnalysisInvariant::Constraint` or
`CallAnalysisInvariant::Client`, including a nested-call client payload, never
increments source-program hard-error counts or candidate score and never enters
`CandidateSelection::Rejected`, `CheckedRejectedCallEvidence`, or any other
public unselected evidence. A successful sibling overload cannot mask it in
either producer order. Replay failure also discards the replay transaction and
publishes no prepared graph node, side-state failure, or rejected evidence.

For ordinary source text, arity/name/type incompatibility, a current authored
cycle, ambiguity, and incomplete inference remain `Rejected`. The private
issuers, affine seeds, and consume-only sealers make foreign, stale,
wrong-scope, noncanonical, and otherwise impossible carriers unconstructible in
well-formed in-process execution. Encountering one is a typed invariant (an
internal defect, stale capability, tamper, or invalid restore), not a user
diagnostic and not permission for overload fallback.

## 4. Language intrinsic generic ownership

`GenericParameterOwnerId::AgentIntrinsic` is replaced by the general lower-layer
owner:

```rust
pub enum GenericParameterOwnerId {
    Callable(CallableDeclarationKey),
    Nominal(ProjectNominalDeclarationId),
    AcceptedNominal(AcceptedNominalId),
    AcceptedSource(SourceSpan),
    Detached(DetachedGenericOwnerId),
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
its existing `GenericParameterOwnerId::AcceptedNominal`, obtained from the accepted
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

The callable layer owns one generic issuer-bound prepared call graph in
`callable/continuation.rs`; it depends only on a narrow
`PreparedCallPrefixPayload` contract and callable/sema-root coordinates.
Analysis instantiates it with `AnalyzerPreparedCallPrefix` until checked
callables and final effects are ready. That instance replaces the
`pending_calls` side map and is part of `SemanticFactState`, so candidate
checkpoint journaling also rolls back graph node insertion:

```rust
trait PreparedCallPrefixPayload {
    type Unselected;

    fn selected(&self) -> &PreparedResolvedCallable;
    fn schema(&self) -> CallableSignatureSchemaDigest;
    fn completed_group(&self) -> CallableGroupIndex;
    fn solution(&self) -> &Arc<TypeConstraintSolution>;
}

struct PreparedCallGraph<P: PreparedCallPrefixPayload> {
    issuer: Arc<PreparedCallGraphIssuer>,
    next_node: u64,
    nodes: BTreeMap<PreparedCallNodeId, PreparedCallNode<P>>,
}

struct PreparedCallNode<P: PreparedCallPrefixPayload> {
    site: CheckedCallSite,
    dependencies: Box<[PreparedCallContinuationRef]>,
    payload: PreparedCallNodePayload<P>,
}

enum PreparedCallNodePayload<P: PreparedCallPrefixPayload> {
    SelectedValue {
        prefix: P,
        result: TypeKind,
    },
    SelectedContinuation(PreparedCallContinuation<P>),
    Unselected(P::Unselected),
}

struct AnalyzerPreparedCallPrefix {
    candidates: Box<[PreparedResolvedCallable]>,
    selected: AnalyzerPreparedCandidateRecord,
    diagnostics: Arc<[CallableDiagnostic]>,
    accounting: CallResolverAccountingReport,
}

struct PreparedCallContinuation<P: PreparedCallPrefixPayload> {
    coordinate: PreparedCallContinuationCoordinate,
    prefix: P,
    next_group: CallableGroupIndex,
    deferred: Box<[DeferredContinuationParameter]>,
    function_type: TypeKind,
}

#[derive(Clone)]
struct PreparedCallContinuationCoordinate {
    issuer: Arc<PreparedCallGraphIssuer>,
    node: PreparedCallNodeId,
}

#[derive(Clone)]
struct PreparedCallContinuationRef(PreparedCallContinuationCoordinate);

struct PreparedCallContinuationSeed {
    coordinate: PreparedCallContinuationCoordinate,
    solution: Arc<TypeConstraintSolution>,
}

enum PreparedResolvedCallableState {
    Base,
    PreparedContinuation(PreparedCallContinuationRef),
    CheckedContinuation(Arc<CheckedCallContinuation>),
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

pub(crate) enum CallAnalysisInvariant {
    Constraint(CallConstraintInvariant),
    Client(AnalyzerCallClientInvariant),
}

pub(crate) enum CallAnalysisFailure {
    FatalSource(
        SourceError<AnalyzerCallConstraintSource, AnalyzerCallSourceFailureCause>,
    ),
    Abort(TypeConstraintAbort),
    Invariant(CallAnalysisInvariant),
}

pub(crate) type CallAnalysisResult =
    Result<CallAnalysisOutcome, CallAnalysisFailure>;

pub struct CheckedCandidateInventory {
    candidates: Arc<[Arc<ResolvedCallable>]>,
    selected: CheckedCandidateIndex,
    digest: CheckedCallCandidateInventoryDigest,
}

pub struct FrozenCallTypeSolution {
    base: ResolvedCallableDigest,
    schema: CallableSignatureSchemaDigest,
    completed_group: CallableGroupIndex,
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

`CallTargetFacts` is constructed only from `Ok(CallAnalysisOutcome)`. A lower
constraint invariant maps through
`CallAnalysisInvariant::Constraint(CallConstraintInvariant::Lower(..))`; a
preparation/frozen invariant maps through the same `Constraint` branch. A
validated `TypeConstraintFailureInvariant::Client` maps through
`CallAnalysisInvariant::Client` with the exact moved domain payload. No branch
can synthesize a public `CallAnalysisOutcome`, and no client invariant is
collapsed into a callable/lower code.

`PreparedCallContinuation` is the single pre-seal continuation carrier. It does
not implement `Clone`. The analyzer-domain publication operation consumes the
prefix `PreparedCandidateTransaction` into `AnalyzerPreparedCallPrefix`; graph
insertion consumes that prefix into the carrier, assigns a fresh issuer-bound
node, and returns an opaque reference. The reference may be shared
as generation-local dependency evidence, but exposes no base/group/solution or
transaction getter. Only `PreparedCallGraph` can resolve it to a borrow. This
supports an immediately enclosing call and multiple uses of the same partial
function value without copying the carrier or its lower solution.
`PreparedCallContinuationSeed` is not a second carrier: it is an affine,
non-`Clone` run capability minted only after resolving one reference, contains
only the issuer coordinate and an `Arc` to the carrier's exact opaque solution,
is immediately enclosed with its derived scope in
`PreparedConstraintInitialization`, and is consumed only when `start` opens
that token for lower initialization. It cannot create a continuation, survive a
candidate run, or enter final identity.

All carrier constructors, graph insertion/resolution, affine-seed and
initialization-token minting, and frozen-handle construction remain private
owner methods. There is no raw
binding getter, empty fallback, deserializable in-process carrier, consumer
repair, or independent scope-plus-solution constructor. Consequently the
preparation gate is the single callable validation boundary, while lower
initialization is the single binding-content validation boundary.

The carrier's owner methods derive completed group, base/schema identity, and
the exact lower solution through the sealed prefix-payload methods; only next group, sorted
deferred rows, and the already checked projected function type are stored.
Construction proves adjacency, schema first-use ownership, and the function
projection before the node becomes observable. A ref from another issuer, a
missing/rolled-back node, a node that is not a continuation, or a node at the
same or later insertion ordinal returns the corresponding
`CallConstraintInvariant`; it is not candidate rejection. Dependency rows are
strict node order, sorted, and unique, so the prepared graph is acyclic by
construction.
Node IDs are monotonic and never reused after rollback. Only a
`CandidateSemanticProjection` minted by the same graph may restore extracted
nodes with their original IDs, and it restores them in dependency order after
checking that every baseline or earlier-delta dependency exists. A failed
check aborts restoration as an invariant and publishes none of the delta.

`AnalyzerPreparedCandidateRecord` is the post-probe, pre-seal remainder of one
selected transaction. The analyzer-domain owner consumes
`PreparedCandidateTransaction<AnalyzerCallConstraintDomain>` exactly once and
splits its `AnalyzerCallSealedBranch` into the move-only
`CandidateSemanticProjection` and a record containing the exact lower solution,
closed source rows, client-owned materialization outcomes, keyed projections,
score, and prepared execution inputs. The record contains no semantic
projection and has no public constructor. It implements
`PreparedCallPrefixPayload::solution` by borrowing that sole solution handle.
Its associated unselected payload is the analyzer-owned typed ambiguous/
rejected/non-callable/missing evidence, so the callable graph imports no
final-analysis type.

Prepared publication is one atomic `SemanticFactState` operation: first apply
the selected semantic projection, including any nested prepared-graph node
deltas, then insert the current value or continuation node with the record. A
failure rolls the whole operation back. This private publication is required
during body analysis so later expressions can observe nested expression facts
and continuation references; it is not a provisional `CallTargetFacts` success.
When an enclosing candidate checkpoint later extracts and rolls back these
facts, `CandidateSemanticProjection` moves the affected prepared graph nodes as
part of its exact payload. Applying the selected outer projection restores the
nested nodes before inserting the outer node. No graph node retains another
`CandidateSemanticProjection`, so this ownership is acyclic rather than a
recursive projection copy.

`FrozenCallTypeSolution` is the callable-sealed handle to the sole lower
solution. It owns an `Arc<TypeConstraintSolution>` whose pointee is opaque and
non-`Clone`; it does not copy its normalized bindings into a callable-owned
collection. Deferred continuation parameters are deliberately higher-owned:
the callable sealer derives and sorts `deferred` from the schema-sealed generic
inventory after proving each row's exact first remaining group. The frozen
handle also owns the exact resolved base digest, schema digest, and completed
group that produced the lower solution. Its version-1 digest commits those
coordinates before the lower solution's sorted completed binding iterator and
the sorted deferred rows. Sharing a frozen solution between an application
core and its continuation clones only the sealed `Arc` handle. Probe
publication cannot clone or reconstruct the underlying solution: the first
frozen handle is created only when the graph sealer consumes the prefix
transaction from its selected value node or move-only continuation carrier.
The next call can obtain a lower inherited seed only through
`validate_and_prepare_call_constraints`, using either the issuer-bound prepared
reference before that consumption or the frozen handle afterward. A
digest-equal solution with a wrong base/schema/group, a hand-built deferred
list, a mismatched version-1 digest, or an empty replacement seed returns
`CallConstraintInvariant` before a callback executes. The gate never repairs,
rebuilds, or drops rows and never tries another overload after that failure.

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

The phase order remains explicit: body expression analysis builds only the
prepared graph; `finish_checked_callables` seals callable definitions;
effect closure consumes the graph's typed selected-candidate/effect dependencies
without requiring a final application digest; C1 seals semantic coordinates;
then `finalize_call_facts` consumes the entire graph. There is no attempt to
create a `CheckedCallContinuation` during early expression analysis and no need
to move catalog/effect sealing before bodies.

Graph consumption is deterministic node order and performs one reconciliation:

1. require every dependency reference to name an already consumed earlier node
   from the same issuer;
2. convert each base prepared candidate through the checked catalogs and each
   `PreparedContinuation` candidate through the earlier node's one
   `Arc<CheckedCallContinuation>`; an already checked continuation is validated
   directly;
3. consume the node's projection-free prepared candidate record, seal the
   canonical candidate inventory, and create the base/schema/group-bound
   `FrozenCallTypeSolution` from its exact lower solution;
4. for a continuation carrier, recompute and compare next group, deferred rows,
   and function type, seal the prefix application core, then create exactly one
   `CheckedCallContinuation`, retain only its final `Arc` for later dependent
   nodes, and replace every exact prepared function-value seed owned by that
   node with the checked handle;
5. seal the complete application outcome, validate its already published
   private prepared facts, and write the final fact in the final-publication
   checkpoint; and
6. consume the carrier/node. No prepared reference, node ID, issuer, or graph
   table survives finalization.

The sealer's earlier-node result vector is an affine local of this consuming
operation, not a published side map. A shared prepared reference therefore
reuses the one final `Arc<CheckedCallContinuation>` without resealing the prefix.
Failure rolls back the publication transaction and consumes no final facts; it
never falls back to reconstructing `base + group + solution`.

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
The C sealer alone opens the final-fact publication checkpoint and writes the
one complete application fact. It receives a projection-free prepared record:
the selected `CandidateSemanticProjection` was already consumed exactly once by
private prepared-graph publication during body analysis. Any final seal or
write failure rolls the final-fact checkpoint back. Probe paths never apply a
projection; singleton or selected replay may apply one only while atomically
publishing its private prepared node, never as `CallTargetFacts`.

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
dispatch or digest. During body analysis, `ResolvedFunctionValueSeed` carries
only `PreparedCallContinuationRef` (or an already checked continuation supplied
by a prior accepted authority). `PreparedResolvedCallable` creates its private
continuation state only through `try_from_prepared_continuation(graph, reference)`.
During graph consumption that state is reconciled to `ResolvedCallable` through
`try_from_continuation(Arc<CheckedCallContinuation>)`. Raw
`continuation_base + next_group`, independent next-group recomputation,
solution extraction, and function-type schema reconstruction are deleted. The
continuation digest binds the exact catalog/intrinsic base, schema, group,
prefix application core, and inherited solution.

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
    source_projection: CheckedConstraintSourceProjection,
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
  domain || resolved-base-callable-digest32 || schema-digest32 ||
  u32(completed-group) || u32(bound-count) ||
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
`PreparedCallGraphIssuer`, `PreparedCallNodeId`,
`PreparedCallContinuationCoordinate`, its reference, and its affine seed are
also generation-local pre-seal capabilities. They have no encoder and are
forbidden in every checked fact, snapshot, cache key, or digest. Graph
reconciliation replaces them with the exact checked continuation digest before
canonical identity is computed.

The frozen base/schema/completed-group prefix is validated against the selected
resolved base and schema before any bound row is exposed. Bound rows sort by
generic parameter identity. Deferred rows sort by parameter identity then first
group. Counts precede their rows exactly as shown. The generic parameter
identity is encoded as the semantic digest of its exact
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
alternative, solution, continuation, execution row, or application. A future
decode/restore path first decodes untrusted private DTO rows, never an owned
solution, prepared reference, frozen handle, or checked application via direct
`Deserialize`. Restore enters the same callable sealer used for in-process
freezing. That sealer first resolves base/schema/group coordinates from their
authorities and derives the exact parameter scope, invokes the same types-owned
canonical inherited-row validator used by initialization to construct the
opaque solution, then validates deferred rows, recomputes every digest, and
compares the stored version-1 bytes before constructing a handle or candidate.
The callable layer never walks binding contents, and the types layer never
reconstructs callable coordinates.

Restore rejects foreign, stale, wrong-scope, unordered, incomplete, cyclic, or
noncanonical rows without sorting, normalizing, dropping, completing, or
otherwise repairing them. In particular, restored `{T -> U, U -> i32}` is
`InheritedSolutionInvariantKind::NonCanonical`; restore does not rewrite it to
`{T -> i32, U -> i32}`. Restore failure is returned as typed seal/invariant
failure before analyzer selection and therefore cannot produce rejected-call
evidence or overload fallback. Every DTO, digest domain, and restore contract
keeps version `1`; there is no legacy reader or version-dispatch path.

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
   traversal observer, and sorted completed binding iterator. Replace the flat
   lower error enum with the exact `Rejected`/`Abort`/`Invariant` error algebra
   and `Rejected`/`FatalSource`/`Abort`/`Invariant` failure algebra, moving all
   inherited kinds under `TypeConstraintInvariant` and splitting failure
   invariants into generic `Constraint(TypeConstraintInvariant)` and
   `Client(D::ClientInvariant)` payloads; retain no lower callable-group or
   deferred ownership;
3. add the exhaustive types-owned `TypeGenericUseCollector`, seal the callable
   generic inventory/role/first-use rows in every schema constructor, and
   reject inferable const generics rather than omitting them;
4. replace resolver accounting handoff with the exclusively borrowed
   `CandidateConstraintWorkSession<'_>`, pending previous/proposed full-report
   reservation, exactly-once infallible complete/drop commit, and
   cancellation/limit accounting. Make lower initialization return only the
   closed `TypeConstraintInitializationFailure::{Abort, Invariant}` and prove
   that early initialization failure invokes no callback while drop commits
   accounting once;
5. install the types-owned prepared source constraint, keyed alternative hints,
   checked source projection constructor, selected alternative/evidence source
   equation, closed trace, and final materialization request. At this point
   delete the callable-owned prepared/checked source projection enums and their
   alternate container match;
6. extend the callable driver with the domain-owned `ClientInvariant` associated
   type/source capability and affine
   `SourceCallbackFailure::{Fatal, Abort, Invariant}`, driver-owned ticket
   issuer, one-source probe authority, exact-ordered materialization authority,
   bound checkpoints, and validated affine closers. Replace the unconditional
   materialization rollback in the same compiling switch, then delete the old
   unticketed begin/callback/rollback protocol and the lower
   `SourceFailureCause::{Rejected, Fatal}` wrapper. Map ordinary rejected
   outcomes, validated fatal callback error, callback abort, client/nested
   invariant, and ticket/checkpoint protocol violation directly to `Rejected`,
   `FatalSource`, `Abort`, `Invariant(Client(..))`, and precedence-taking
   `Invariant(Constraint(SourceProtocol(..)))`;
7. implement branch-local Choice pruning followed by all-constraint
   normalization, occurs checking, final Choice pruning, exact source-order
   materialization of every correlated trace from one baseline per canonical
   binding, the distinct post-extension inherited immutability check before
   materialization callbacks, exact sealed-value equality/coalescing, pair
   unicity, final keyed projection, and run completion;
8. establish `PreparedCallGraph`, its issuer/node/ref algebra, the move-only
   `PreparedCallContinuation` that consumes a prefix transaction, its affine
   one-run seed, the base/schema/group-bound `FrozenCallTypeSolution`, and the
   one `validate_and_prepare_call_constraints` entry. Migrate mapping, source
   alternatives/evidence, parameter scope, prepared/frozen inherited seed,
   base, receiver, arguments, and expected result together. Establish the
   move-only `PreparedConstraintInitialization` that seals the derived scope
   with its exact seed, and atomically replace callable `start` with the
   token-plus-client-only fallible signature; delete the raw
   scope-plus-`Option<Arc<TypeConstraintSolution>>` signature. Establish the
   closed `CallConstraintInvariant` algebra for every foreign/stale/wrong-state
   prepared reference and wrong prepared/frozen identity, order, scope, group,
   deferred, function, or digest condition; delete every analyzer scope scan
   and empty/raw inherited-solution constructor immediately;
9. implement `final_analysis/analyzer/calls/constraints.rs`, exact move-only
   `CandidateSemanticProjection` equality, the analyzer client, query-local
   graph/work/analyzer borrow split, prepared-graph checkpoint journaling,
   `AnalyzerCallClientInvariant::NestedCall`, the recursive boxed
   `CallAnalysisInvariant::{Constraint, Client}` algebra, ticket-bound
   checkpoint open/close hooks, affine extract-and-rollback sealing, and the
   four-way selection disposition:
   only `Rejected` continues overload selection, while `FatalSource`, `Abort`,
   and `Invariant` roll back and stop without rejected evidence;
10. migrate Option, Result, Agent, Collection, Reduction, Fx, dialogue
    fixed/custom, and synthetic Dialogue schemas through the same preparation,
    callback, and typed evidence route, with no inference placeholder and
    fail-closed Traverse/Parallel deletion;
11. atomically replace `CandidateProbeBatch`, `CandidateProbeRequest`,
    `CandidateProbe`, `probe_resolved_call`, `probe_call_candidate`,
    `evaluate_call_arguments`, `evaluate_mapped_call_argument`, and
    `evaluate_mapped_call_slot` with one move-only prepared transaction,
    singleton move, and multi-candidate full `SelectedReplay`. Once the new
    selected route inserts its value/continuation graph node, delete
    `PendingCallAnalysis`, the `pending_calls` map, `commit_call_arguments`, and
    `replay_rejected_call_arguments`; no adapter may call either route. Replace
    prepared `try_curried` and raw `Curried { base, group }` with
    `try_from_prepared_continuation(graph, reference)`, and replace every raw
    function-value continuation pair with `PreparedCallContinuationRef`, in
    this same switch;
12. establish sema-root semantic-coordinate ownership, lower intrinsic generic
    owners, canonical digests, higher-owned sorted deferred rows, cumulative
    continuation, and prepared-callee sealing;
13. after `finish_checked_callables`, effect closure, and C1 coordinates,
    consume the prepared graph in dependency order, reconcile every prepared
    continuation exactly once into `FrozenCallTypeSolution` plus
    `CheckedCallContinuation`, and publish the one final application. Migrate
    execution projection and compiler/runtime-plan/tooling consumers, then
    reduce join to validation, Method enrichment, and move-only edge handoff.
    In this switch delete provisional selected `CallTargetFacts`,
    `publish_selected_call`, `publish_selected_call_in_transaction`, final
    selected rebuild, join-side substitutions, result overrides, and compiler
    reconstruction. Establish the version-1 private DTO restore gate through
    the same types canonical-row validator and callable sealer, with digest
    recomputation and fail-without-repair semantics; and
14. prove there is no remaining call consumer of `TypeParameterSubstitutions`,
    old solver relations, flat constraint failures, `SourceFailureCause`,
    unticketed callback hooks, raw scope/solution driver initialization,
    nested-invariant side state/panic conversion, source reconstruction, or
    recovery compatibility, delete their now-unused
    call-only APIs and reexports, and run the full C2 gates. Nominal-only
    substitution APIs remain only when an enumerated nominal consumer still
    requires them.

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
  array-length, map-kind, map-key, alternative, evidence, source projection,
  final expected, and solution tampering; the callback must be unable to return
  an expected type or container constructor;
- types-owned generic-use collection over every `TypeKind` and `ArrayLength`
  constructor, schema inventory role/first-use derivation for group zero,
  later groups, and result-only use, and rejection of omitted/duplicate/wrong
  first-use rows and inferable const generics;
- the single preparation gate returning `CallConstraintInvariant` for a
  malformed mapper/schema seal, missing/unexpected continuation seed, wrong
  prepared-graph issuer/node/state/order, wrong base/schema/completed/next
  group, wrong frozen prefix core or digest, incorrect projected function type,
  and terminal future-eligible inventory before the first source callback,
  plus compile evidence that no raw seed can be supplied;
- `PreparedConstraintInitialization` pairing the exact derived scope with each
  None/Prepared/Frozen seed, affine prepared-seed consumption, absence of
  `Clone`/getters/public constructors, and compile-fail evidence that
  `CandidateConstraintWorkSession::start` accepts neither a raw scope nor an
  `Option<Arc<TypeConstraintSolution>>` and cannot mix a scope with another
  candidate's seed;
- an inner partial call seeding its immediately enclosing call before
  `finish_checked_callables`/effect closure, two- and three-group chains, a
  partial result retained without immediate use, and multiple outer uses of one
  prepared reference sharing one carrier and later one checked continuation;
- prepared-graph checkpoint rollback producing
  `CallConstraintInvariant::MissingOrStalePreparedNode`, typed invariant results
  for cross-issuer/forward/self/cycle cases, strict unique dependency order,
  affine one-run seed consumption, and compile-time absence of a raw solution
  accessor or public carrier constructor;
- receiver-only inference, expected-result inference, terminal incomplete
  rejection, and continuation-owned deferred parameters;
- a spurious-Choice matrix where equal intermediate bindings carry different
  probe semantic branches, proving that a failed local Choice branch cannot
  prune another row or survive final SelectedCall pruning;
- callback-lifetime Unchecked and keyed Alternatives hints, per-alternative
  Complete/Parametric values, exact sorted Parametric unbound identities,
  ordinary semantic evidence nonacceptance as source `Rejected`,
  duplicate/unknown alternatives or malformed returned evidence as
  `Invariant(SourceProtocol(..))`, and an expected-dependent probe whose final
  public facts and final expected come only from lower closure and ordered
  materialization, never from its speculative checkpoint;
- driver-minted affine probe tickets authorizing exactly one `Probe` source,
  driver-minted materialization tickets authorizing the exact ordered
  `Materialize` source list, checkpoint binding to ticket issuer/ordinal, and
  compile evidence that tickets/bound checkpoints are not `Clone`, encodable,
  constructible by the callback, or retained in semantic facts;
- callback `SourceCallbackFailure::Fatal` with the exact authorized source and
  phase becoming `FatalSource`, callback work cancellation/overflow and every
  work/node/branch/binding/source/materialization limit remaining `Abort`, and
  open-hook failure or wrong phase/source/set/order/ticket/checkpoint becoming
  `Invariant` even when the callback claims a plausible `SourceError`;
- receiver and argument expression callbacks in both probe and materialization
  forwarding nested-call prepared, frozen, lower-inherited, and projection
  `CallAnalysisInvariant` payloads across one- and two-level nested calls
  through the private `AnalyzerCallClientInvariant::nested_call` capability and
  `SourceCallbackFailure::Invariant`, with the exact nested source chain and
  outer source-coordinate validation and no `FatalSource`/`Abort` coercion,
  side state, hard error, or panic;
- valid ticket plus successful rollback preserving the exact moved nested
  invariant through `TypeConstraintFailureInvariant::Client` and
  `CallAnalysisInvariant::Client`, versus wrong ticket/source/phase/binding or
  stale/non-LIFO close taking typed `Constraint(SourceProtocol(..))` precedence
  over the same nested-invariant attempt;
- duplicate prepared and sealed source coordinates returning typed
  `PreparedSource`/`SourceProtocol` invariants before any binding/value
  publication, including duplicate-source tamper of an otherwise valid sealed
  branch;
- dynamic typed-rest probing with no container expected, lower-only derivation
  of Vec/Seq/Slice/Array/Map projections, exact header retention, whole final
  equation recheck, and the same selected alternative/evidence/projection/final
  expected present in the closed equation, trace, materialization request,
  keyed solution projection, and execution slot;
- every correlated trace for one canonical binding re-materialized from the
  same baseline in both source orders, equal final sealed values coalescing,
  different final sealed values remaining ambiguous, semantic failures
  pruning only their trace, and typed materialization errors retaining their
  source phase/cause;
- affine materialization closure on sealed, rejected, callback-Fatal,
  callback-Abort, callback-Invariant, foreign, stale, and non-LIFO checkpoints:
  sealed performs exactly one
  `extract_and_rollback`, ordinary rejection, valid callback `Fatal`, and
  callback `Abort`/`Invariant` perform exactly one rollback and become
  `Rejected`/`FatalSource`/`Abort`/`Invariant(Client(..))`, wrong or foreign
  ticket binding and
  stale/non-LIFO closure become `Invariant(SourceProtocol(..))`, and no outcome
  can trigger a second close or leave a live token;
- exact `CandidateSemanticProjection` equality over issuer and every typed map
  entry, semantically equal move payload coalescing, any result-changing
  payload difference remaining distinct, digest-collision fallback to full
  equality, and compile evidence that the projection and sealed branch are
  neither `Clone` nor `Ord`;
- two semantically identical probe graph deltas allocated with different raw
  node IDs comparing equal through exact dependency isomorphism, while a
  changed site, dependency edge, continuation payload, solution, or nested fact
  compares unequal; same-issuer baseline refs remain exact and foreign refs
  return a typed graph/protocol invariant rather than unequal candidate
  evidence;
- different bindings remaining distinct even when their visible source types
  match;
- group-zero bindings consumed by later groups and exact curried base tamper;
- every `InheritedSolutionInvariantKind`: wrong scope/rigid binding, unknown or
  duplicate/unordered parameter, self binding, forbidden form, occurs/cycle,
  unclosed row, and noncanonical row, plus callable invariants for wrong
  base/schema/group and incorrect deferred first-group ownership;
- completed inherited `{T -> U, U -> i32}` returning `NonCanonical` without
  repair, canonical inherited `{T -> U}` plus current `U -> i32` succeeding
  with final `{T -> i32, U -> i32}`, and a canonical inherited binding that
  conflicts with a current constraint remaining ordinary `Rejected(Mismatch)`;
- every base-instantiation variant and payload-order/scalar/tag tamper;
- singleton/replay application-core and final-application digest equality, with
  full base/receiver/argument/result/materialization/score replay and no probe
  solution or semantic projection reuse;
- multi-candidate mismatch-plus-success continuing and selecting the success,
  versus `FatalSource`/`Abort`/constraint-invariant/nested-client-invariant plus
  success aborting the whole query in both producer orders with full rollback,
  zero `CandidateSelection::Rejected` or `CheckedRejectedCallEvidence` for the
  non-rejection failure, and the same behavior in singleton and
  `SelectedReplay`;
- compile/runtime evidence for the query-local `ResolverWork` plus independent
  `&mut Analyzer` borrow split, including callback access only through the
  narrow work capability, graph-borrow release before the initialization token
  moves into `start`, synchronous nested-invariant return without a re-entrant
  work borrow or side slot, and analyzer borrow release before `run.complete()`;
- candidate producer-order invariance, canonical selected-index rewriting,
  equivalent duplicate coalescing, and digest-equal authority mismatch
  returning a typed authority invariant;
- acyclic continuation sealing: prefix-core digest first, continuation digest
  second, and final application digest last, with typed tamper failure at each
  boundary;
- prepared-graph reconciliation after callable/effect/C1 sealing, including
  exact prepared-versus-recomputed base/schema/group/deferred/function checks,
  one carrier consumption despite shared references, dependency-first semantic
  projection publication, prepared/frozen seed solution parity, and proof that
  no prepared issuer/node/ref enters a final fact or digest;
- version-1 restore DTO round trip through the common lower row validator and
  callable sealer, with foreign/stale/wrong-scope, row-order, noncanonical,
  coordinate, deferred, and digest tamper failing before handle/candidate
  construction, no input normalization/sorting/row dropping, no direct
  `Deserialize` for opaque carriers, and no legacy/version-dispatch reader;
- equivalent HIR allocated in different raw ID orders producing identical
  lexical/function-value/resolved-callable/core/application/continuation
  digests, while a stable path, ordinal, schema, type, effect, or capture change
  changes the owning digest;
- standard Option/Result result closure without an override helper;
- the synthetic Dialogue site passing through the same sealer;
- selected execution typed tamper failure and the absence of execution/result/
  continuation APIs on unselected outcomes;
- direct versus value-callee sealing, receiver/callee source tamper, and
  compiler parity from the sealed execution row without an outer callee fact or
  raw HIR/schema/name reread;
- exact error-disposition tests proving candidate mismatch/ambiguity/current
  cycle/incomplete/source rejection are `Rejected`, only validated callback
  `Fatal` is `FatalSource`, solver/ticket/callback cancellation, checked
  overflow, and every configured limit are `Abort`, carrier/scope/source-plan/
  protocol/projection impossibilities are `Invariant(Constraint(..))`, and
  domain client/nested failures are `Invariant(Client(..))` without later
  reclassification;
- recursive solver cancellation at every node/path/binding boundary, checked
  counter overflow, callback work cancellation at probe and materialization,
  exact report accounting, and unchanged error precedence;
- CandidateConstraintWorkSession accounting on success and every failure,
  previous/proposed full-report reservation, attempted double completion,
  exclusive-borrow enforcement, and pending session/run drop infallibly
  committing exactly once while exposing no outcome, including fallible
  initialization's closed `Abort`/`Invariant` result producing zero
  probe/materialization callbacks and one accounting commit, and ticket-mint,
  open-hook, callback-Abort, and close-invariant paths retaining the exact
  checked/accepted callback charge with no double count;
- Recovery/SelectedCall/Invariant matrices for Error, `_`, Never, Choice,
  StageActor and other erased families, rigid enclosing generics,
  functions/effects, and arrays; strict structural `first_mismatch` versus
  directional Invariant acceptance; and exact constant/generic array lengths
  with recovered/inferred lengths accepted only by Recovery;
- compile-direction evidence that callable/final-analysis share the sole
  sema-root coordinate types, the generic types/callable layers mention only
  `D::ClientInvariant`, and no types/callable-to-final-analysis dependency or
  `Clone`/`Eq`/`Ord` bound on the client invariant exists;
  and
- one inference-free callable join, Method enrichment, and move-only edge
  publication.

For the recorded 351-pass/16-failure pre-B3 baseline used to validate this
amendment, the fifteen call-bearing failures are one closure group, not fifteen
exception paths. Ordinary, receiver/extension, optional/default, typed spread,
generic intrinsic, overload/Choice, dialogue, expected-result, and curried
fixtures close through steps 5-13 above and must pass without fixture-specific
branches. `checked_match_project_enum_commits_constructor_layout_evidence` is
the sole non-call failure; its `InvalidNominalOwner` belongs to the independent
Match/nominal seal and is neither masked nor accepted by B3. The full library
gate is complete only when both groups pass, but that Match repair is not an
authorization to add a call-side fallback.
