# Final Rust shapes and owner map

These are the final ownership shapes. Established ID and wrapper names may be
imported from their current modules, but implementations must not weaken a
field, replace it with source spelling, or add a parallel projection.

## Syntax and HIR declaration

`arcweft-lang-syntax` adds one dedicated node after the last ordinary
parameter group:

```rust
pub enum AttachedContentRoleSyntax {
    InlineContent,
    RichContent,
    DialogueContent,
}

pub enum AttachedContentPresenceSyntax {
    Required,
    Optional { question: AstNode<QuestionKind> },
    Defaulted {
        equals: AstNode<EqualsKind>,
        value: AttachedExpressionNode,
    },
}

pub struct AttachedCallableContentParameter {
    syntax: AstNode<AttachedContentParameterKind>,
    open: AstNode<OpenBracketKind>,
    binding: AttachedRequiredName,
    presence: AttachedContentPresenceSyntax,
    colon: AttachedRequiredPunctuation,
    role: AttachedContentRoleSyntax,
    close: AstNode<CloseBracketKind>,
}
```

`arcweft-lang-hir::item::callable` owns the declaration semantics:

```rust
pub enum HirAttachedContentRole {
    Inline,
    Rich,
    Dialogue,
}

pub enum HirAttachedContentPresence {
    Required,
    Optional,
    Defaulted { value: ExprId },
}

pub struct HirCallableAttachedContentParameter {
    binding: LocalId,
    role: HirAttachedContentRole,
    presence: HirAttachedContentPresence,
}
```

Every callable signature/item shape admitted by the owner matrix gains:

```rust
attached_content: Option<HirCallableAttachedContentParameter>
```

The group coordinate is not copied into HIR. The grammar makes the attached
parameter trailing, so the accepted callable's last `parameter_groups` index
is its only group. HIR construction rejects zero/foreign binding locals,
foreign default expressions, a default outside the callable scope, and an
attached parameter on a disallowed item family. The source map owns distinct
whole/open/binding/question/colon/role/equals/default/close roles.

## Callable schema and checked declaration

`arcweft-lang-sema::callable::schema` evolves the existing type in place:

```rust
pub struct CallableAttachedContentParameter {
    group: CallableGroupIndex,
    presence: CallableParameterPresence,
    policy: CallableAttachedContentPolicy,
    execution: CallableAttachedContentExecution,
}

pub enum CallableAttachedContentExecution {
    Structural,
    RuntimeContent,
}
```

The existing `RegisteredCallableCatalogBuilder::project_record` is the only
project producer. It maps accepted HIR to `Declared(role) + RuntimeContent`,
uses the terminal group, and publishes no row when HIR has no attached
parameter. Presentation and text-proxy factories remain Structural producers.

The checked callable catalog/interface owns the binding and ABI contract:

```rust
pub struct CheckedCallableAttachedContentParameter {
    group: CallableGroupIndex,
    binding: LocalId,
    binding_coordinate: StableCheckedBindingCoordinate,
    admission: CheckedContentRole,
    presence: CallableParameterPresence,
    abi_position: u32,
    binding_type: TypeKind,
    abi_type: TypeKind,
    default: Option<CheckedAttachedContentDefault>,
}

pub struct CheckedAttachedContentDefault {
    source: ExprId,
    coordinate: StableCheckedValueCoordinate,
    result: SemanticTypeDigest,
    effects: EffectRow,
    suspension: CheckedSuspensionRole,
    control: CheckedExecutableControlRole,
    expression: CheckedAttachedContentDefaultExpressionDigest,
    captures: Box<[CheckedAttachedContentDefaultCapture]>,
}

pub struct CheckedAttachedContentDefaultCapture {
    parameter: CallableParameterCoordinate,
    pattern: PatternId, // generation-local execution join
    pattern_digest: CheckedPatternSemanticDigest,
    bindings: Box<[LocalId]>, // exact accepted parameter binding row
    binding_evidence: Box<[CheckedAttachedContentDefaultCaptureLocal]>,
    used_locals: Box<[CheckedAttachedContentDefaultCaptureLocal]>,
    binding_type: TypeKind,
}
```

`binding` and default `source` are generation-local execution joins;
`binding_coordinate` and default `coordinate` are their stable semantic
identities. Raw IDs are excluded from every digest. `abi_position` is derived after receiver and ordinary runtime slots of the
terminal group. Required has exact `DialogueContent`; Optional and Defaulted
have exact `Option<DialogueContent>`. Only Defaulted has `default`. The
compiler consumes these paired rows directly and does not reopen HIR to pair
an independently selected raw owner with a stable coordinate.

Each default capture is one logical ordinary parameter value. Validation joins
the exact parameter coordinate and instantiated logical binding type
(`RestPositional(T)` binds `Vec<T>`), the complete pattern binding row and
independently checked child-local types, plus the nonempty free-local subset.
Every free parameter local is covered exactly once; a free nonparameter local,
duplicate origin, reordered/missing binding, RestNamed row, or pattern digest
mismatch rejects the complete interface batch. The interface digest commits
only stable pattern/binding/type evidence, never raw PatternId/LocalId values.

`CheckedAttachedContentDefaultExpressionDigest` is a distinct public semantic
digest domain:

```rust
pub struct CheckedAttachedContentDefaultExpressionDigest([u8; 32]);

pub(crate) struct FinalCallableResolutionDraft {
    // Final checked IDs, records/schemas, execution, and closed exposed rows.
    // Deliberately no CallableInterfaceDigest field or accessor.
}
```

The checked callable builder changes state from effect inference to this
move-only draft. Final call applications for attached defaults resolve against
the draft. The default transcript commits stable expression structure and, for
each project-callable leaf, `CheckedCallableId::semantic_digest`, callable
schema digest, checked execution tag, and exposed effect row. Checked call
application digests commit the instantiated generic/type/effect solution and
result. The transcript never reads `CallableInterfaceDigest` and never walks a
referenced callable body.

After all defaults are sealed, the draft and the complete default-row batch
are consumed by one final catalog seal. `CheckedCallableFacts` then owns:

```rust
control: CheckedExecutableControlRole,
attached_content: Option<CheckedCallableAttachedContentParameter>,
interface_digest: CallableInterfaceDigest,
```

`CheckedClosureExecution` owns the same `control` row beside its closed effect
and suspension rows. The shared selected-expression fold seals
`ExpressionCompatible` only when the executable contains no ordinary project
call; otherwise it seals `FlowRequired`. This role is independent from effects
and suspension and is committed by callable/default interface transcripts.
There is no item-role emission helper or second selector.

No checked catalog or interface digest exists before that seal. Ordinary
expression semantic transcripts run afterward and continue to consume final
interface digests. Trait/impl validation compares a structural projection of
the attached row that excludes the declaration-local binding coordinate; a
default is invalid on both sides of that join.

## Prepared and checked selected-call channels

`arcweft-lang-sema::callable::resolver::PreparedCallInputs` retains three
orthogonal fields: ordinary mapping, semantic operands, and attached content.

```rust
pub enum PreparedCallAttachedContentOperand {
    Omitted,
    Present { source: HirDialogueContentId },
}

pub struct PreparedCallInputs {
    mapping: PreparedCallArgumentMapping,
    semantic_operands: Box<[PreparedCallSemanticOperand]>,
    attached_content: Option<PreparedCallAttachedContentOperand>,
}
```

Site-family dispatch happens before attached schema/body pairing. An exact
`DialogueLine` verifies the selected family and HIR application owner and
returns `Accepted(None)` here; its separate semantic content operand remains
in `semantic_operands` under the dialogue application seal.

C1 in `callable::checked_application` seals an execution-complete algebra:

```rust
pub struct CheckedCallAttachedContentSource {
    raw: HirDialogueContentId,
    application: StableCheckedValueCoordinate,
}

pub enum CheckedCallAttachedContentOperand {
    StructuralPresent {
        source: CheckedCallAttachedContentSource,
    },
    RuntimeOmitted {
        abi_position: u32,
    },
    RuntimePresent {
        source: CheckedCallAttachedContentSource,
        abi_position: u32,
    },
}

pub struct CheckedCallExecutionProjection {
    receiver: CheckedCallReceiverProjection,
    arguments: Box<[CheckedCallExecutionArgument]>,
    semantic_operands: Box<[CheckedCallSemanticOperand]>,
    attached_content: Option<CheckedCallAttachedContentOperand>,
}
```

`CheckedCallRuntimeOperand` adds:

```rust
AttachedContent {
    source: Option<&CheckedCallAttachedContentSource>,
    presence: CallableParameterPresence,
    ty: &TypeKind,
    abi_position: u32,
}
```

For an omitted Optional/Defaulted row `source` is `None`; for RuntimePresent it
is `Some`. Structural never appears in this runtime projection. The stored row
is receiver-first and then authored argument/slot source order, with attached
content last. Each member retains its ABI destination. Construction validates
a copied position set as an exact permutation but never sorts the execution
row; ABI order is an allocation-free derived iterator only.

## Runtime call carrier

`arcweft-runtime-plan::semantic_facts` keeps attached content distinct while
validating it in the same positioned ABI transaction:

```rust
pub enum RuntimeResolvedAttachedContent {
    Required {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    OptionalPresent {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    OptionalOmitted {
        ty: RuntimeNormalizedType,
    },
    DefaultedPresent {
        source: ExprId,
        ty: RuntimeNormalizedType,
    },
    DefaultedOmitted {
        ty: RuntimeNormalizedType,
    },
}

pub struct RuntimeResolvedAttachedContentOperand {
    abi_position: u32,
    content: RuntimeResolvedAttachedContent,
}

pub struct RuntimeResolvedCall {
    completed_group: CallableGroupIndex,
    dispatch: RuntimeResolvedCallDispatch,
    operands: Box<[RuntimeResolvedCallOperand]>,
    attached_content: Option<RuntimeResolvedAttachedContentOperand>,
    result: RuntimeCallResultShape,
}
```

The constructor receives the checked completed group and attached ABI
position, validates the one source row plus a contiguous ABI-position
permutation without reordering, and retains both. The group is call-owned and
is not duplicated in the attached
operand. Every presence row retains the exact final normalized ABI
type. Required must be the accepted `DialogueContent` identity; Optional and
Defaulted must be the accepted `Option<DialogueContent>` identity. Present
rows must have one source whose checked expression type is the option item;
omitted rows must have no source. General consumers evaluate the source row
once, then install values by the derived ABI view. Specialized structural call
families use one shared typed-local ANF materializer before assembling
ABI/semantic-role payloads. The attached consumer constructs
`DialogueContent`, `Some(content)`, or a typed `None` from this row. It never reopens the callee
schema, reconstructs a nominal by spelling, scans HIR attached applications,
or appends a value after a completed operand list.

## Runtime callable ABI descriptor

The callee side derives its ABI independently from the final checked
interface. `RuntimeProjectCallable` owns the one reusable descriptor:

```rust
pub struct RuntimeProjectCallable {
    declaration: CallableDeclarationKey,
    owner: ItemId,
    source_owner: HirCallableSourceOwner,
    runtime: RuntimeCallableId,
    attached_content_abi: Option<RuntimeCallableAttachedContentAbi>,
}

pub struct RuntimeCallableAttachedContentAbi {
    group: CallableGroupIndex,
    abi_position: u32,
    presence: CallableParameterPresence,
    binding: LocalId,
    binding_ty: RuntimeNormalizedType,
    abi_ty: RuntimeNormalizedType,
    default: Option<RuntimeCallableAttachedContentDefault>,
}

pub struct RuntimeCallableAttachedContentDefault {
    source: ExprId,
    coordinate: StableCheckedValueCoordinate,
    digest: CheckedAttachedContentDefaultExpressionDigest,
}
```

Required seals `binding_ty == abi_ty == DialogueContent` and no default.
Optional seals both types as exact `Option<DialogueContent>` and no default.
Defaulted seals `binding_ty == DialogueContent`, `abi_ty ==
Option<DialogueContent>`, and exactly one source/coordinate/digest row.
Descriptor construction validates the binding/default against the same
checked callable interface and semantic-coordinate index. `source_owner` is
copied from the selected `CallableSymbol` and resolves the exact final-HIR
member row; scanning an item by binding, default, or spelling is invalid.
`group` and `abi_position` are copied from the final checked interface and are
not recomputed from a flattened HIR parameter list. The generation-local IDs
are execution joins and the coordinate/digest are semantic identity.

Function-site reservation consumes only this descriptor. Required and Optional
materialize their final binding values directly. Defaulted retains the public
`Option<DialogueContent>` call ABI but ProjectCall materialization supplies the
target site a `DialogueContent`: present projects the checked content source;
omitted invokes the declaration-owned default site with exact logical capture
sources. Neither target reservation nor entry lowering reads a call-site
operand, adopts the first call, or reopens HIR/schema.

`RuntimeEntryCallableInput` owns a `RuntimeProjectCallable` directly instead
of duplicating declaration/owner or carrying another attached descriptor.
Thus entry, direct dispatch, and callable-value lowering consume the same
descriptor instance shape and cannot drift.

Each closed terminal instance owns the instantiated default execution fact:

```rust
pub struct RuntimeProjectAttachedDefaultFunctionFact {
    source: ExprId,
    coordinate: StableCheckedValueCoordinate,
    digest: CheckedAttachedContentDefaultExpressionDigest,
    result: RuntimeNormalizedType,
    suspension: CheckedSuspensionRole,
    control: CheckedExecutableControlRole,
    execution: RuntimeProjectFunctionExecution,
    effects: Box<[EffectId]>,
    captures: Box<[RuntimeProjectAttachedDefaultCapture]>,
}
```

Its constructor accepts `ExpressionFunctionSite` only for
`ExpressionCompatible + NonSuspending + empty effects`; every other admitted
combination requires `ExecutableFunctionSite`. The ordinary instance fact
retains and validates the same suspension/control evidence, so compiler
projection cannot publish an expression body that contains `ProjectCall`.

## Ordinary project callable execution

Project functions use the existing structured `RuntimeFunctionSite`
authority. They do not use `RuntimePureHelper` as a peer declaration/dispatch
model. Nonterminal applications have no function site. The call fact owns the
only transition authority:

```rust
pub enum RuntimeProjectFunctionCallInput {
    Direct,
    Continuation {
        callee: ExprId,
        abi: RuntimeProjectContinuationAbi,
    },
}

pub enum RuntimeProjectFunctionCallOutcome {
    Continue {
        abi: RuntimeProjectContinuationAbi,
        next_group: CallableGroupIndex,
    },
    Invoke {
        instance: RuntimeProjectFunctionInstanceKey,
    },
}

pub struct RuntimeProjectContinuationAbi {
    lineage: RuntimeProjectContinuationLineageId,
    function_type: RuntimeNormalizedType,
    prefix_types: Box<[RuntimeNormalizedType]>,
}

pub struct RuntimeProjectFunctionInstanceKey {
    callable: RuntimeCallableId,
    instantiation: CallableInstantiationDigest,
    group: CallableGroupIndex,
}
```

The semantic fact retains normalized shapes for compiler validation. The core
runtime value projects only their stable identities; it never stores a
plan-local or bytecode-local ordinal:

```rust
pub struct RuntimeProjectContinuationAbi {
    lineage: RuntimeProjectContinuationLineageId,
    function_type: RuntimeSemanticTypeId,
    prefix_types: Box<[RuntimeSemanticTypeId]>,
}
```

Native execution resolves each identity through
`RuntimePlanTypeTable::id_for_semantic` before plan-local shape/value checks.
AWBC call payloads may retain `AwbcTypeId` because they are program-local, but
VM construction reads `AwbcRuntimeType::semantic_identity()` into the runtime
value; it never converts an ordinal to `RuntimePlanTypeId`. AWBC session-save
stores these semantic identities directly and restores only after exact
program type lookup/function-shape/value validation.

`completed_group` is copied from
`CheckedCallApplicationCore::current_group()` for every direct or value call.
The attached row retains its checked ABI position after construction; its
group is the enclosing call's `completed_group` and is not duplicated. A
direct project call joins both values to the declaration descriptor. A value
continuation joins them to its checked lineage and remaining function type;
it does not reconstruct the declaration descriptor or look up a site. Earlier partial groups cannot
carry attached content, and completing the descriptor's terminal group cannot
omit its runtime attached row.

`Direct` is a unit variant: it consumes no runtime continuation and there is no
checked continuation digest to copy. Only `Continue` issues `abi`; the matching
later `Continuation` input carries the exact callee ExprId plus that expected
ABI. Core seed/final forms preserve the same split and `next_group`; they do not
invent a direct lineage or infer an ABI from a runtime value.

`Continue` constructs `ProjectContinuation { lineage_id, prefix_values }`.
The prefix values are source-ordered, evaluated once, and appended once at
each checked continuation application. A call-fact `Continue` never allocates
a site. A call-fact `Invoke` is the only consumer that names a function
instance. All reached closed terminal instances are reserved before any body
is defined.
The instantiation digest is the final callable-join digest, not a call
application digest: applications are site-specific, while equal generic
solutions must share one monomorphization. One compiler-produced
`RuntimeProjectFunctionInstanceFact` retains that digest, the terminal
instantiated function type/result ABI, every prefix/current parameter
coordinate, and the substitution-backed body/local/type projection:

```rust
pub enum RuntimeProjectFunctionParameterSource {
    ContinuationPrefix { position: u32 },
    CurrentGroup { position: u32 },
}

pub struct RuntimeProjectFunctionParameterAbi {
    group: CallableGroupIndex,
    parameter: u32,
    source: RuntimeProjectFunctionParameterSource,
    pattern: PatternId,
    source_type: TypeId,
    kind: HirParameterKind,
    bindings: Box<[LocalId]>,
    abi_ty: RuntimeNormalizedType,
    binding_ty: RuntimeNormalizedType,
}

pub struct RuntimeProjectFunctionInstanceFact {
    key: RuntimeProjectFunctionInstanceKey,
    callable: RuntimeProjectCallable,
    suspension: CheckedSuspensionRole,
    control: CheckedExecutableControlRole,
    execution: RuntimeProjectFunctionExecution,
    function_type: RuntimeNormalizedType,
    parameters: Box<[RuntimeProjectFunctionParameterAbi]>,
    effects: Box<[EffectId]>,
    body: RuntimeProjectFunctionBody,
    type_projection: Box<[RuntimeProjectFunctionTypeProjection]>,
}
```

The terminal instance binds prior groups solely from continuation-prefix
positions and the current group solely from logical current positions. The
call-fact materialization maps source-row indices to those logical values and
packs RestPositional into one `Vec<T>` binding. Its type
projection follows `HirRuntimeExecutableSemanticOwners` for the declaration
item. The enclosing function row retains a nested closure's value expression
but excludes that closure's body/local/type rows; the closure executable owns
those rows completely. Runtime-plan never adopts the first observed call,
scans descendant scopes, or lowers an open generic HIR body as if it were
concrete. The final checked selector is exhaustive:

```text
DirectFrame + ExpressionCompatible + NonSuspending + closed empty row -> Expression
DirectFrame + FlowRequired or MaySuspend or nonempty closed row       -> Executable
StreamFactory                                                          -> stream contract
```

Source effect-clause spelling never selects emission. Default effects are
already members of the callable's final exposed row. Only a pure,
non-suspending, expression-compatible default may remain an Expression site;
a project-call-containing, effectful, or suspending default executes as an
Executable site before the authored terminal target.

Every project invocation uses one suspension-capable ordinary flow transfer:

```rust
pub enum RuntimeFlowOpSeed {
    ProjectCall {
        plan: RuntimeProjectCallPlanSeed,
        result: RuntimePatternSeed,
    },
    // existing members
}

pub enum FlowOp {
    ProjectCall {
        plan: RuntimeProjectCallPlan,
        result: RuntimePattern,
    },
    // existing members
}
```

The call plan owns source operands, logical materialization, exact continuation
input, optional default stage, and Continue/Invoke outcome. The callee's
checked function type and `RuntimeFunctionSite` validate logical inputs and
result type. Expression contexts ANF-lower to a mini CFG and resume local; no
synchronous expression fallback may execute a ProjectCall.

The native engine owns a stack of `RuntimeStructuredCallFrame`. Each frame
saves the caller cursor, pending operations, control stack, environment,
pending-await observer, root cleanups, and optional result pattern. Activation
evaluates callee and arguments exactly once, validates a same-plan structured
function with full arity, installs a fresh environment from ordered captures
and arguments, and starts the executable operations. `ReturnExpr` validates
the declared site result, runs callee cleanup, restores the caller frame, and
binds the result atomically. Suspension keeps the same fiber and call-frame
stack; no child fiber or content-specific function runner owns an ordinary
call.

AWBC lowers the same operation to one `ProjectCall` terminator with one resume
block/result register. Pattern lowering occurs only after the VM call/default
frames return. The ordinary function signature and every emitted effect plan
use the same checked `RuntimeFunctionEffectSet`. Native and AWBC preserve the
same pending default/target stage in snapshots; neither reevaluates operands
after suspension.

```rust
pub struct AwbcProjectCallSite {
    caller_function: AwbcFunctionId,
    block: AwbcBlockId,
}

pub enum FiberReturnContinuation {
    Ordinary,
    ProjectCallDefault {
        site: AwbcProjectCallSite,
        prefix_values: Vec<RuntimeValue>,
        logical_values: Vec<RuntimeValue>,
    },
    ProjectCallTarget {
        site: AwbcProjectCallSite,
    },
}
```

The site rejoins the block's verified ProjectCall terminator on every return
and after restore. No continuation clones the terminator or copies its
outcome/result type/pattern/resume payload. The snapshot owns a parallel DTO
only for serialization shape: site/stage plus prefix/logical
`AwbcRuntimeValueSnapshot` rows. The outer artifact fingerprint pins the exact
program; a patch or coordinate/function/resume mismatch rejects restore.

## Runtime content fragment semantic fact

Final sema seals callback captures and the exact closed row together:

```rust
pub struct CheckedDialogueEffectSite {
    id: CheckedDialogueEffectSiteOrdinal,
    trigger: CheckedDialogueEffectTrigger,
    effects: EffectSet,
    effect: Box<CheckedEvaluatedEffect>,
    captures: Box<[CheckedDialogueEffectCapture]>,
}
```

The sealer copies `effects` from the selected
`CheckedCallApplicationCore::effects` only after proving its tail is Closed.
The semantic transcript commits the ordered semantic digests in that set.
Neither compiler nor runtime-plan reopens the call graph or derives the row
from `CheckedEvaluatedEffectOperation`.

The dependency-safe producer owner is
`arcweft-runtime-plan::semantic_facts`, alongside the final typed expression,
call, and evaluated-effect facts. It may depend on HIR/sema/text-model/core
identities; lower semantic crates and `arcweft-text-model` must not depend on
runtime-plan. `arcweft-compiler` only projects this fact and does not own a
side table.

```rust
pub struct RuntimeContentFragmentFact {
    id: RuntimeContentFragmentId,
    source: ExprId,
    owner: StableCheckedValueCoordinate,
    template: DialogueContentFragmentTemplate,
    values: Box<[RuntimeDialogueValueExpression]>,
    effects: Box<[RuntimeDialogueEffectProgramFact]>,
}

pub struct RuntimeDialogueValueExpression {
    slot: RuntimeDialogueValueSlotId,
    role: RuntimeDialogueValueRole,
    expression: ExprId,
    ty: RuntimeNormalizedType,
}

pub struct RuntimeDialogueEffectProgramFact {
    site: RuntimeDialogueEffectSiteId,
    trigger: RuntimeDialogueEffectTrigger,
    effects: EffectSet,
    operation: RuntimeEvaluatedEffectFact,
    captures: Box<[RuntimeDialogueEffectCaptureFact]>,
}

pub struct RuntimeDialogueEffectCaptureFact {
    local: LocalId,
    ty: RuntimeNormalizedType,
}
```

`RuntimeContentFragmentId` is derived from the accepted stable content path
plus fragment path/ordinal. `source` is the one generation-local lookup key
used to join a final expression to this report-local fact. The fact catalog
requires a one-to-one `source -> id` relation. `source` is excluded from the
fragment ID, semantic digest, cache identity, and persistence; stable
`owner`/fragment path remains the semantic authority. This explicit transient
join is not a reverse mapping reconstructed from stable coordinates and is
not a peer fragment identity. The text-model template's dense runtime ID is an
interning destination and is likewise excluded from semantic identity. Value
rows are canonical by template slot. Effect rows are canonical by template
effect site. Capture rows are the operation program's exact free locals in
deterministic first-use order. The seal proves local ownership/type, no
duplicates, and exact equality with the effect program's free-local inventory.
`effects` is copied from the final checked site's closed `EffectSet`; it is
never inferred from `operation`. A zero-capture or zero-effect row has an
explicit empty slice/set.

The fact is produced for every accepted root or nested checked-content report.
Publication preflights the template plus all value/effect/capture rows and
commits the whole batch atomically. There is no root-only template inventory.

## Core runtime content carrier

`arcweft-core::plan::dialogue_content` evolves the v1 template manifest with
the static effect ABI:

```rust
pub enum RuntimeDialogueContentEffectTrigger {
    Content,
    Delay { duration: LogicalDuration },
}

pub struct RuntimeDialogueContentEffectSlot {
    site: RuntimeDialogueEffectSiteId,
    trigger: RuntimeDialogueContentEffectTrigger,
    capture_types: Box<[RuntimePlanTypeId]>,
}

pub struct RuntimeDialogueContentTemplateManifest {
    id: RuntimeDialogueContentTemplateId,
    digest: RuntimeDialogueContentTemplateDigest,
    slots: Box<[RuntimeDialogueContentSlot]>,
    effects: Box<[RuntimeDialogueContentEffectSlot]>,
}
```

`arcweft-core::value::opaque` evolves the one content envelope:

```rust
pub struct RuntimeDialogueContentEffectBinding {
    site: RuntimeDialogueEffectSiteId,
    callback: RuntimeFunctionValue,
}

pub struct RuntimeDialogueContentValue {
    artifact: RuntimeArtifactFingerprint,
    template: RuntimeDialogueContentTemplateId,
    template_digest: RuntimeDialogueContentTemplateDigest,
    bindings: Box<[RuntimeDialogueContentBinding]>,
    effects: Box<[RuntimeDialogueContentEffectBinding]>,
}
```

The callback is a zero-argument runtime function whose captured values match
the corresponding manifest capture schema. Structured plans use a
`RuntimeFunctionSiteId` plus capture values; AWBC uses an `AwbcFunctionId` plus
capture registers. This reuses the existing closure authority and ownership
rules. It does not invent an effect-expression bytecode inside the value. The
runtime-plan trigger maps into the lower core-owned trigger; core never depends
on runtime-plan.

Text-model materialization rebases value slots and effect sites together and
returns the rebased effect bindings with the document. The dialogue/reveal
runtime selects the binding by exact site and invokes its callback according
to the manifest trigger. Missing, duplicate, foreign, differently ordered, or
capture-type-mismatched bindings reject construction/materialization before
any effect can run.

## Construction, AWBC, and VM shapes

`RuntimeDialogueContentTemplateManifestSeed` gains effect slots. The existing
function-site authority evolves in place:

```rust
pub enum RuntimeFunctionSiteBodySeed {
    Expression(RuntimeExprSeed),
    Executable(RuntimeFunctionExecutableBodySeed),
}

pub struct RuntimeFunctionExecutableBodySeed {
    effects: RuntimeFunctionEffectSet,
    ops: Box<[RuntimeFlowOpSeed]>,
}

pub enum RuntimeFunctionSiteBody {
    Expression(RuntimeExpr),
    Executable(RuntimeFunctionExecutableBody),
}

pub struct RuntimeFunctionExecutableBody {
    effects: RuntimeFunctionEffectSet,
    ops: Box<[FlowOp]>,
}
```

The existing canonical `EffectId` parser and v1 semantic digest move unchanged
from `arcweft-lang-sema` to foundational `arcweft-id`; sema re-exports the
type and retains only the HIR-to-ID projection. `CheckedDialogueEffectSite`
owns the one final closed `EffectSet`, and
`RuntimeDialogueEffectProgramFact.effects` copies that typed set in the same
report projection as the operation and captures.

`RuntimeFunctionEffectSet` is the sole runtime typed owner: a core
plan/function-site-owned, sorted, duplicate-free
`Box<[arcweft_id::EffectId]>` projected exactly once from the checked set. It
is not source spelling and cannot be reconstructed from an operation variant,
debug label, integer, or name table. `AwbcEffectSet` is only an encoded program
table destination: `intern_effect_set` consumes
`&RuntimeFunctionEffectSet` and interns its typed canonical IDs. No caller
supplies `Vec<&str>`, and no synthetic callback may select table entry zero
unless the typed set is actually empty.
`RuntimeFunctionSiteSeedId` remains the sole correlated site handle and adds
no parallel effect-function identity. Reservation fixes parameters, captures,
result type, effect set, and body family. Definition validates the entire body
before committing it. Existing expression closures use `Expression`; content
effect sites use zero ordinary parameters, exact capture locals, Unit result,
and `Executable`. The runtime-plan producer maps its one
`RuntimeEvaluatedEffectFact` to `RuntimeFlowOpSeed::EvaluatedEffect` and its
checked effect row to `RuntimeFunctionEffectSet` in the same transaction.

`RuntimeExprSeedKind::DialogueContent` and final `RuntimeExprKind` carry one
complete fragment construction input:

```rust
DialogueContent {
    template: RuntimeDialogueContentTemplateId,
    values: Box<[RuntimeDialogueContentBindingExpr]>,
    effects: Box<[RuntimeDialogueContentEffectBindingExpr]>,
}

pub struct RuntimeDialogueContentEffectBindingExpr {
    site: RuntimeDialogueEffectSiteId,
    function: RuntimeFunctionSiteSeedId,
    captures: Box<[RuntimeExprSeed]>,
}
```

Plan admission requires the referenced site to be an executable,
zero-ordinary-argument, Unit-result function and joins its ordered capture
types to the template effect slot and capture expressions. A pure expression
site cannot satisfy this relation.

AWBC v1 evolves `MakeDialogueContent` and its codec schema:

```rust
pub struct AwbcDialogueContentEffectBinding {
    pub site: RuntimeDialogueEffectSiteId,
    pub function: AwbcFunctionId,
    pub captures: Vec<AwbcRegisterId>,
}

MakeDialogueContent {
    template: RuntimeDialogueContentTemplateId,
    values: Vec<AwbcDialogueValueBinding>,
    effects: Vec<AwbcDialogueContentEffectBinding>,
    destination: AwbcRegisterId,
}
```

The verifier joins template slot/effect schemas, function capture ABI, source
register types, and destination type before execution. The VM snapshots each
capture register once in canonical order, constructs the callbacks and the
content envelope atomically, and publishes no partial value on failure.
`value::awbc_save` snapshots callbacks through the existing AWBC function
snapshot authority; generic serde remains non-authoritative for executable
function values.

`PendingAwbcClosure` evolves to own a matching body enum. Executable sites are
lowered by the ordinary AWBC flow lowerer, so evaluated effects become
`AwbcInstruction::EmitEffect` in the referenced `AwbcFunctionId`; expression
sites retain the current expression lowering. The executable body's
`RuntimeFunctionEffectSet` is interned as the function signature's exact
`AwbcEffectSetId`. Using the empty set merely because the function is
synthetic is invalid.

The same set is passed through the executable lowering context to
`intern_evaluated_effect(effect, &RuntimeFunctionEffectSet)`. The resulting
`AwbcEffectPlan.signature.effects` must equal the enclosing callback
function's set; it cannot retain the old id-zero signature while only the
function signature changes. The verifier joins `EmitEffect`'s plan signature
to the enclosing function signature, so neither site may infer a capability
from the effect variant.

Reveal uses two backend-specific entrypoints over the same
`RuntimeFunctionValue`:

```rust
// Structured Engine: enqueue one executable function frame in the ordinary
// flow scheduler; expression-only sites are rejected here.
Engine::activate_dialogue_effect_callback(callback)

// AWBC: validate zero remaining arity/AWBC body, create an internal function
// FiberState, and atomically bind callback captures before the first step.
FiberState::for_runtime_function_callback(
    program,
    entry,
    callback,
    instance,
    generation,
    budget_quantum,
)
```

The concrete methods live in `arcweft-core` beside the existing structured
function application and `FiberState::for_function` authorities. Both the
existing `AwbcInstruction::ApplyFunction` path and reveal activation use one
factored runtime-function activation validator/frame binder; reveal does not
reimplement application. The AWBC fiber is executed only with
`awbc::vm::step_with_host_context`; its emitted effect is the ordinary
`VmObservation::Effect`. Dialogue materialization retains the exact callback
next to its rebased effect site, and the native reveal scheduler selects it
inside the accepted content-event transaction. It never evaluates callback
operations in text-model, runtime-driver, or a content-specific interpreter.
AWBC callback save/restore continues through the existing function-value
snapshot path and is accepted by the same activation validator after restore.

Callback activation is keyed by one core-owned typed coordinate:

```rust
pub struct RuntimeDialogueEffectCallbackActivationId {
    dialogue: DialogueActivationId,
    site: RuntimeDialogueEffectSiteId,
}
```

The site is the rebased site in the materialized content value, so nested
fragments cannot collide. `DialogueActivationId` already commits artifact,
owner fiber, content plan, and occurrence. This pair is the replay/duplicate
key for both structured and AWBC scheduling; callback function IDs and AWBC
fiber generations are destinations, not allocation identity.

The public switch removes direct rich-text effect execution from line tasks.
`final_flow/line_plan.rs` no longer calls or defines
`lower_dialogue_effects`; it does not create delayed schedule operations or
`ContentEffect` children for fragment effects. Consequently
`TriggerDraft::ContentEffect`, `RuntimeLineTaskTriggerSeed::ContentEffect`,
`LineTaskTrigger::ContentEffect`, `AwbcLineTaskTrigger::ContentEffect`, and the
line-task reducer's `content_effects` ready set are deleted. The reveal event
kind remains: the activation transaction validates and consumes it once,
resolves the callback by rebased site, reserves the callback activation ID,
and enqueues that callback. Marks alone continue into the line-task reducer.
There is no interval in which the old action and the callback both execute.

## Digest and deletion consequences

Every new field above is added to its owning v1 transcript. Raw `ExprId`,
`LocalId`, and dense template IDs are excluded where stable coordinates,
semantic type digests, template digests, and ordered path identities exist.
Explicit `None`, omitted, and empty-capture states receive distinct tags/counts.

The switch deletes `dialogue_content_templates` as a detached semantic-fact
side inventory, root-only value/effect scans, compiler `_child_values`, dropped
`child_effects`, manual attached runtime-call append logic, and every second
template/value/effect projection. The final fact, checked ABI iterator, core
content envelope, and AWBC instruction are the only successive owners.
