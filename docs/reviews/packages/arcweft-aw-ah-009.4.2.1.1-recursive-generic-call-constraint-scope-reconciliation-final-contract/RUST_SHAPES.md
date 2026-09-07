# Rust shapes and ownership

The declarations below specify the new boundary fields and exhaustive new
variants. Unchanged payload fields remain in their current owning types.
This is a design document; these signatures have not been compiled.

## Semantic references: types/nominal.rs and types/generics.rs

~~~rust
struct GenericApplicationIssuer(NonZeroU64);
struct TypeVariableSlot(u16);
struct ConstVariableSlot(u16);
struct EffectVariableSlot(u32);

pub struct BoundTypeParameter { depth: u32, slot: u16 }
pub struct BoundConstParameter { depth: u32, slot: u16 }
pub struct BoundEffectParameter { depth: u32, slot: u32 }

pub struct InferenceTypeParameter { issuer: GenericApplicationIssuer, slot: TypeVariableSlot }
pub struct InferenceConstParameter { issuer: GenericApplicationIssuer, slot: ConstVariableSlot }
pub struct InferenceEffectParameter { issuer: GenericApplicationIssuer, slot: EffectVariableSlot }

pub enum GenericTypeReference {
    Free(GenericTypeParameterId),
    Bound(BoundTypeParameter),
    Inference(InferenceTypeParameter),
}
pub enum GenericConstReference {
    Free(GenericConstParameterId),
    Bound(BoundConstParameter),
    Inference(InferenceConstParameter),
}
pub enum GenericEffectReference {
    Free(EffectVar),
    Bound(BoundEffectParameter),
    Inference(InferenceEffectParameter),
}
pub struct GenericBinder { types: u16, const_lengths: u16, effects: u32 }
pub struct GenericScope { binders: Box<[GenericBinder]> }
pub struct ScopedType<'a> { scope: &'a GenericScope, ty: &'a TypeKind }
~~~

GenericApplicationIssuer and all variable-slot fields are private to the lower
types owner. Slot identity is the candidate binder slot, with distinct
type/const/effect wrappers. Issuer equality includes its checked process-local
nonce. No issuer or inference wrapper implements Serialize,
Deserialize, Default or a public constructor. Generic reference enums may be
inspected publicly, but their Bound/Inference payloads can only be issued by
scope-checked owner APIs. Equality/hash include the reference case.

GenericBinder and GenericScope have read-only accessors. Their constructors
validate counts, kind and depth and are crate-private. An empty binder is
canonical. A public user-facing type constructor can only build a checked
closed function through the existing type owner; it cannot publish a dangling
Bound reference. Existing declaration-ID constructors stay declaration-only.

TypeKind::GenericParam changes its payload to GenericTypeReference.
ArrayLength::Generic changes its payload to GenericConstReference.
Function TypeKind gains a GenericBinder beside parameters/result/effects.
EffectRow changes its variable-reference payload to GenericEffectReference;
do not maintain a second bound-effect row type. Existing concrete atoms,
errors and unresolved recovery nodes retain their ownership and policy.

The types-owned structural fold in types/constraints/shape.rs is extended
and promoted for shared binder traversal. Its operations include collect_free,
open_binder, shift_bound, reify, apply_template, apply_free and encode_checked.
All return typed errors for malformed scope; inference/encoding operations
also accept the existing accounting/cancellation observer. Array lengths and
effect rows participate in that fold. Existing consumer substitutions and
source-label reconstructions are removed.

## Schema and lexical scope

~~~rust
// callable/schema.rs; constructed only by the accepted schema sealer.
pub(crate) struct CallableGenericParameterInventory {
    binder: GenericBinder,
    types: Box<[CallableGenericTypeUse]>,
    consts: Box<[CallableGenericConstUse]>,
    effects: Box<[CallableGenericEffectUse]>,
}

// final_analysis lexical projection; no new registry.
pub(crate) struct CheckedLexicalGenericScope<'topology> {
    generation: &'topology Arc<AcceptedHirProjectGeneration>,
    owner: HirSemanticPathLocation<'topology>,
    free: CheckedFreeGenericInventory,
}

// callable/continuation.rs
pub(crate) enum CallableApplicationPosition {
    Unapplied,
    AfterGroup(CallableGroupIndex),
}
pub(crate) struct OpenedCallConstraintInitialization {
    issuer: Arc<PreparedCallGraphIssuer>,
    opening: OpenedGenericScope,
    inherited: PreparedCallConstraintSeed,
}
~~~

AcceptedHirProjectGeneration and HirSemanticPathLocation are the existing
HIR project-generation and rooted-path carriers. The projection borrows them
from HirProjectEvaluationTopology. It joins accepted symbol/world inventories
before issuing its Free inventory; no new generation or path allocator exists.
The preparation gate consumes the checked Free inventory into its own scope;
the topology borrow ends before analyzer callbacks acquire mutable access.

The inventory retains the current checked declaration owner, generic kind,
role and first-use fields, adding the exact binder-slot projection.
RigidReference entries have no candidate slot. Candidate entries each own one.
Callers do not submit the derived first-use or slot list.

The existing validate_and_prepare_call_constraints entry point invokes
PreparedCallGraph::validate_and_issue_constraint_initialization. Its token
constructor is module-private. The callable driver consumes it exactly once
through into_lower_parts. No public/raw opener, independent scope/seed pair,
test-only production bypass or terminal boolean is introduced.

## Completion and frozen solution

~~~rust
// types/constraints/solution.rs; no callable module dependency.
pub(crate) struct CompletedGenericSubstitution {
    free: CheckedFreeGenericInventory,
    residual: GenericBinder,
    types: Box<[CheckedTypeArgumentBinding]>,
    consts: Box<[CheckedConstArgumentBinding]>,
    effects: Box<[CheckedEffectArgumentBinding]>,
    residual_origins: CheckedResidualOrigins,
    completeness: CompletedGenericScopeProof,
}

// callable/checked_application.rs
pub struct FrozenCallTypeSolution {
    base: ResolvedCallableDigest,
    schema: CallableSignatureSchemaDigest,
    position: CallableApplicationPosition,
    substitution: Arc<CompletedGenericSubstitution>,
    residual: CheckedResidualCallableProjection,
    digest: FrozenCallTypeSolutionDigest,
}

// callable/join.rs
pub struct CheckedProjectFunctionInstanceSolution {
    substitution: ClosedGenericSubstitution,
    instantiation: CallableInstantiationDigest,
}
~~~

CompletedGenericSubstitution has no Clone or public constructor. Sharing uses
Arc. Its only constructor consumes a completed lower path after normalization,
exact scope, required-key checks and strict acceptance. No externally supplied
"already normalized" flag exists.

CheckedResidualOrigins links residual slots to lower template slot identities.
CheckedResidualCallableProjection joins that evidence to the one schema's
remaining group/result coordinates; it contains no copied type binding values.
CheckedTypeArgumentBinding keys remain GenericTypeParameterId and their values
are stored TypeKind terms under the carrier's residual binder; borrowed
accessors return ScopedType rather than an unscoped reference.
The const and effect rows use their respective declaration IDs. Type and const
rows cannot be interchanged. All collections have one strict canonical order.

FrozenCallTypeSolution exposes checked apply_template, apply_free,
remaining_function_scheme and instantiate_effect_row projections. Replace the
ambiguous instantiate_type method whose input provenance was implicit.
No caller rebuilds a BTreeMap from its binding iterator.

ClosedGenericSubstitution has no residual *outer* binder and no Free or Inference
reference in its RHSs; nested function binders are legal closed terms. Its only
constructor closes one completed callee substitution under an optional already
closed enclosing substitution. This operation also computes the final
instantiation transcript. There is no layers field or repeated-layer fold.

## Callable values and application position

The existing callable graph/value preparation pipeline owns the exhaustive
operation distinction ValuePreparation and ApplyGroup. It derives that
distinction from the checked HIR/value owner, not from a caller-supplied
terminal flag. Both routes use the same schema binder and completion owner.

ValuePreparation preserves Unapplied position. Its candidate slots may remain
residual; a complete expected function type can constrain them through the
same relation before sealing. It consumes no physical argument group.
ApplyGroup derives Bindable/FutureEligible and required inherited rows from
the actual group, as specified in FINAL_CONTRACT.md. Only ApplyGroup advances
position. No separate raw substitution constructor is needed for a bare value.

Checked callable-value evidence retains the exact definition/closure origin,
captured generic environment and Unapplied/AfterGroup position. A generic
unapplied value projects the schema's canonical function binder; a partial
result projects the frozen residual binder. Aliases and closure captures share
this evidence and its immutable prefix, never a mutable inference environment.

A quantified runtime project value uses ProjectContinuation with its checked
function identity and prefix. Empty-prefix Unapplied values may enter the
existing Continuation input at group zero after typed position validation.
Remove the unconditional group-zero continuation rejection. Do not fabricate
a Direct call or a completed group zero to produce a function value.

Existing monomorphic function values and contextual declaration selection keep
their accepted function-site/capture path. A published quantified prefix is
not implicitly converted to that carrier. No InstantiateCallable expression,
new coercion witness, wrapper FunctionSite, generic closure generalization or
dynamic generic callable registry is introduced by this contract.

## Runtime type graph

~~~rust
// core/plan/type_kind.rs; used by runtime-plan via existing dependency direction.
pub struct RuntimeGenericBinder { types: u16, const_lengths: u16, effects: u32 }
pub struct RuntimeTypeScope { binders: Box<[RuntimeGenericBinder]> }
pub struct RuntimeBoundTypeParameter { depth: u32, slot: u16 }
pub enum RuntimeArrayLength {
    Value(u64),
    Bound { depth: u32, slot: u16 },
}
pub struct RuntimeFunctionEffectRow {
    atoms: Box<[EffectId]>,
    tail: Option<RuntimeBoundEffectParameter>,
}
~~~

RuntimePlanTypeProjection adds BoundType(RuntimeBoundTypeParameter).
Its Function variant gains binder and effects beside parameters/result.
Its Array variant uses RuntimeArrayLength. The existing type declaration row
gains RuntimeTypeScope; scope is not maintained in a side table.
RuntimeNormalizedType retains this scope with the existing normalized shape.

Every bound subnode's semantic identity commits its scope stack, reference
kind/depth/slot and full structural projection. Parent-child admission checks
the expected scope transition before accepting a referenced row. Standalone
runtime values, locals, captured prefixes and callable ABIs require a closed
root (empty incoming scope), while a function root may push its own binder.
Bound subnodes are not assigned an executable value class or unchecked value
predicate. This prevents an unrelated type ID from laundering a binder.

## Instance graph and limits: compiler/lower/project_instances.rs

~~~rust
pub struct ProjectInstantiationLimits {
    instances: u64,
    edges: u64,
    structural_nodes: u64,
    type_depth: u64,
    work: u64,
}
pub enum ProjectInstantiationLimitKind {
    Instances, Edges, StructuralNodes, TypeDepth, Work,
}
pub struct ProjectInstantiationLimitExceeded {
    kind: ProjectInstantiationLimitKind,
    limit: u64,
    attempted: u64,
    origin: RuntimeProjectFunctionInstanceProjectionOrigin,
}
pub(crate) enum ProjectInstanceState {
    Queued(ClosedProjectInstanceSeed),
    Discovering(ClosedProjectInstanceSeed),
    Discovered(ClosedProjectInstanceNode),
    Complete(RuntimeProjectFunctionInstance),
}
pub(crate) struct ProjectInstantiationSession {
    limits: ProjectInstantiationLimits,
    accounting: ProjectInstantiationAccounting,
    pending: BTreeSet<RuntimeProjectFunctionInstanceKey>,
    nodes: BTreeMap<RuntimeProjectFunctionInstanceKey, ProjectInstanceState>,
    edges: BTreeSet<ProjectInstanceEdge>,
}
~~~

All fields are private. The checked configuration constructor accepts inclusive
bounds from zero through the production maxima, allowing embedders to tighten
limits; it cannot silently raise production limits. Zero permits an empty
inventory and rejects the first charged operation of that kind. The session
is compiler-local, non-Clone, non-serializable and bound to one accepted
generation and its cancellation source.

Existing compile entry points use production limits. A checked compile-options
field carries an explicitly supplied lower limit. The compiler's semantic
projection error gains the typed limit/cancellation cases and formats their
actual fields into a diagnostic. No free-form error string is the authority.

Move instance discovery/materialization out of the oversized lower.rs into
this owner while preserving its dependency-inversion role. Use the exact
runtime semantic partition APIs. No function-pointer hooks, filesystem access
or host scheduling enter the Sans-I/O layers.

## Required deletions and visibility proof

Delete the declaration-ID-as-inference-variable path, duplicate rigid/bindable
collision branches, unscoped schema projection getters, completed raw maps,
parallel deferred binding lists, effect-only freshening, layers, recursive
instance-discovery calls and open-generic runtime fallbacks.

Compile-time privacy tests must fail to construct or deserialize issuers,
initialization tokens, completed substitutions, frozen prefixes and type
application scope proofs from raw fields. Lower unit tests use private validators;
production call-driver tests use the real preparation gate and accepted
schema/source evidence. Source spelling scans are not privacy or acceptance
evidence.
