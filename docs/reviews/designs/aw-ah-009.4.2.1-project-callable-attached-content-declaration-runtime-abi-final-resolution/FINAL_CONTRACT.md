# Final contract

## 1. Normative outcome

1. A callable signature, not an attribute, result type, parameter name, call
   site, or registration side table, is the sole declaration owner of project
   attached content.
2. The canonical source form is one trailing bracket parameter after every
   ordinary `()` parameter group and before `->`, `where`, contracts, effects,
   or the body.
3. The bracket owns one simple binding, one closed role, one presence, and an
   optional ordinary default expression. It is not an ordinary parameter
   group and cannot be supplied through parentheses.
4. Project schemas publish `Declared(role) + RuntimeContent` only from that
   accepted HIR declaration. Presentation/text-proxy schemas continue to
   publish their catalog-owned `Structural` contracts.
5. A supplied body becomes one distinct checked attached-content operand. The
   raw `HirDialogueContentId` is paired with the outer accepted stable
   application coordinate; raw IDs never enter stable digests.
6. Runtime attached content occupies one deterministic final ABI position for
   the terminal callable group. Caller and callee derive it from the same
   checked interface; no numeric position is authored or copied.
7. One `RuntimeContentFragmentFact` owns the static template, value bindings,
   reveal-time effect programs, and their capture bindings for every root or
   nested content fragment.
8. Checked body admission is stored on `CheckedRichTextReport`. Nested
   `PreserveBodyRole` inherits that report's role; literal admission is a
   distinct member and is never guessed as Rich.
9. All schema, semantic, runtime-plan, and AWBC domains remain version `1`.
10. The public switch deletes every superseded split template/value/effect
    inventory and every generic-body fallback in the same uncommitted cut.

## 2. Source grammar

The exact forms are:

```arcw
fn required(args)[body: InlineContent] -> DialogueContent { body }
fn optional(args)[body?: RichContent] -> DialogueContent { body }
fn defaulted(args)[body: DialogueContent = default_content()] -> DialogueContent { body }
```

The role spellings are exactly `InlineContent`, `RichContent`, and
`DialogueContent`. They are contextual tokens in this production. A type alias,
qualified path, string, identifier with the same spelling outside this
position, or source name cannot define a role.

The grammar is:

```text
AttachedContentParameter =
    '[' Binding OptionalMarker? ':' ContentRole Default? ']'

Binding        = one valid simple binding name
OptionalMarker = '?'
ContentRole    = 'InlineContent' | 'RichContent' | 'DialogueContent'
Default        = '=' Expression
```

`?` and `=` are mutually exclusive. The default is an ordinary expression
expected to produce the accepted standard `DialogueContent` nominal. It is not
an inline content block and does not fabricate a declaration-owned
`HirDialogueContentId`.

There is at most one attached parameter. It follows all curried `()` groups,
so its group is the last declared callable group. Missing delimiters, binding,
colon, role, or default expression retain typed recovery and make the
declaration non-executable. Alternate attribute and annotated-value-parameter
forms are not parsed as aliases.

## 3. Syntax, attachment, and HIR shapes

The exact names may be adjusted only to match established module naming; the
ownership and fields are normative:

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

Final HIR owns no syntax ranges:

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

The binding owns exactly one `HirLocal` in the callable scope. Required and
defaulted bindings have semantic type `DialogueContent`; optional bindings
have semantic type `Option<DialogueContent>`. The role is separate from both
types.

`HirFunctionSignature`, `HirFunctionItem`, inherent/trait method signatures,
and extern-capability function signatures gain
`attached_content: Option<HirCallableAttachedContentParameter>`. Source roles
cover whole/open/binding/question/colon/role/equals/default/close. The HIR
callable digest and symbol publication commit the typed declaration. A
`CallableSymbol` exposes the typed contract or a stable declaration reference;
it does not copy role strings.

## 4. Declaration-owner admission

The source production is admitted for ordinary item functions, inherent
methods, trait requirements, trait implementations, and extern-capability
functions. It is rejected for Flow, Predicate, Proof, View, compile-time
`#[fx]` functions, and every non-callable item.

Ordinary item functions and inherent methods admit required, optional, and
defaulted presence. Trait requirements, trait implementations, and extern
capabilities admit required and optional presence only. They reject defaulted
presence because no single concrete Arcweft body owns a default-expression
prologue across dynamic dispatch or a host boundary.

A trait implementation must declare a binding and exactly match the
requirement's role and presence. Binding spelling is implementation-local and
does not enter interface equality. An inherent impl has no requirement join
and follows the ordinary-function rules.

Every admitted project declaration must have a terminal result exactly equal
to the accepted standard `DialogueContent` nominal after generic resolution.
An arbitrary type named `DialogueContent`, `ContentEmission`, `Unit`, or a
convertible type is rejected. Structural catalog rows are not source
declarations and retain their existing owner-specific result validation.

## 5. Callable schema and checked declaration

The callable schema field evolves in place:

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

For project declarations the builder issues:

```text
group     = last declared group
presence  = Required | Optional | Defaulted
policy    = Declared(mapped HIR role)
execution = RuntimeContent
```

No other project producer may construct that row. A runtime row with a
non-Declared policy, a structural row on a project callable, a group other
than the terminal group, or an attached row on a non-content result is an
invalid schema.

The checked callable interface additionally owns:

```rust
pub struct CheckedCallableAttachedContentParameter {
    group: CallableGroupIndex,
    binding: StableCheckedBindingCoordinate,
    admission: CheckedContentRole,
    presence: CallableParameterPresence,
    abi_position: u32,
    abi_type: TypeKind,
    default: Option<CheckedAttachedContentDefault>,
}

pub struct CheckedAttachedContentDefault {
    source: StableCheckedValueCoordinate,
    result: SemanticTypeDigest,
    effects: EffectRow,
    expression: CheckedAttachedContentDefaultExpressionDigest,
}
```

Required ABI type is `DialogueContent`. Optional and defaulted ABI type is
`Option<DialogueContent>`. For defaulted presence the callee prologue maps
`Some(value)` to the binding and evaluates the declaration-owned checked
default expression only for `None`. Its effects are included in the callable
effect contract. The raw default `ExprId` remains generation-local.

The default-expression digest is deliberately not the ordinary expression
semantic digest. Ordinary expression transcripts commit a referenced project
callable's final interface digest, while this default is itself part of its
owner's interface. Reusing that transcript would create a self or mutually
recursive digest equation.

Accepted declaration semantic roots commit the checked declaration identity,
not its interface digest. Coordinates therefore exist before the interface
seal and remain stable when an interface changes; ordinary semantic leaves
commit the final interface separately. This is the single coordinate
authority used by both default and ordinary transcripts, not a draft-root
side table.

Default sealing therefore uses this single order:

1. analyze every callable body and attached default and close the callable
   effect graph as one SCC-capable graph;
2. freeze a typed resolution draft containing final checked callable IDs,
   schemas, execution families, and exposed effect rows, but no interface
   digest;
3. seal default call applications against that draft;
4. issue `CheckedAttachedContentDefaultExpressionDigest` from the final
   selected expression graph, stable coordinates, types, literals, and call
   application digests; a project-callable leaf commits its checked
   declaration identity, schema digest, execution family, and exposed effect
   row and never another callable interface digest or body;
5. join checked attached rows into every callable and issue each
   `CallableInterfaceDigest` exactly once in one catalog transaction; and
6. build ordinary expression semantic transcripts against the published final
   catalog.

The typed resolution draft is a move-only builder state, not a catalog,
digest, cache key, or published fallback. There is no preliminary interface
digest. Checked application digests already commit the selected declaration,
schema, generic/type/effect solution, result, and exposed call effects, so
self recursion, mutual recursion, generic instantiation, and cross-module
calls terminate at finite declaration leaves without losing a result-changing
interface fact. Body changes behind an equal interface do not perturb a
caller's interface.

Trait/implementation equality compares the structural attached contract:
group, admission role, presence, ABI position/type, and required absence of a
default. Declaration-local binding coordinates and binding spellings do not
enter that equality. Each declaration's own interface digest still commits
its stable binding coordinate.

## 6. Candidate preparation and C1/C2 seal

Prepared call inputs retain a third, separate channel:

```rust
pub enum PreparedCallAttachedContentOperand {
    Omitted,
    Present { source: HirDialogueContentId },
}
```

`None` means the selected schema has no attached contract. `Some(Omitted)`
means an optional/defaulted declared contract was omitted. Required omission
and a present body with no declared contract reject that candidate. A present
body never becomes an ordinary `HirCallArgument` or semantic operand.

This mapper is entered only after selected-call site-family dispatch.
`DialogueLine` validates that the selected site family and HIR application
owner are exactly the line application, then returns `Accepted(None)` from the
attached-content mapper. Its line-owned semantic content operand remains in
the dialogue application channel. It is neither malformed attached metadata
nor an attached-body omission. This routing rule is exhaustive over the typed
site-family algebra; it is not a callable-name or result-type exception.

C1 seals:

```rust
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

pub struct CheckedCallAttachedContentSource {
    raw: HirDialogueContentId,
    application: StableCheckedValueCoordinate,
}
```

The exact enum may factor the execution tag into a field, but it must be
impossible to assign an ABI position to Structural or omit it from
RuntimeContent. C1 validates selected schema/group/presence/execution, exact
HIR body presence, raw owner equal to the application expression, stable outer
coordinate, and a complete contiguous ABI-position permutation atomically.

C2 joins this checked operand to the affine prepared checked-content report.
The report, not the insertion token, owns
`CheckedAttachedContentAdmission::{Role(...), Literal}`. Preserve inherits the
already checked surrounding role; Inline/Rich/Declared fix their role; Literal
admits only opaque raw content. The insertion derives its role from its report.

## 7. Call-site rules

The typed `ContentCall` site family is general. Its validator does not name a
callable or special-case ContentResult:

- absent body plus no attached schema is allowed; the outer content
  application must still prove the call result is exact `DialogueContent`;
- present body plus no attached schema rejects the candidate;
- required schema plus absent body rejects the candidate;
- optional/defaulted schema plus absent body seals `RuntimeOmitted`;
- any present declared body seals the exact source and is checked under the
  declared role;
- structural catalog rows obey their catalog-owned presence/policy; and
- a `DialogueLine` site validates its exact family/HIR owner and leaves this
  channel as `None`; its separate semantic content operand is sealed by the
  dialogue application authority.

For curried callables, earlier groups ignore the terminal attached contract.
Only completion of the declared terminal group maps or omits it. A partial
application cannot rank itself as having omitted attached content.

## 8. Runtime ABI and evaluation order

The attached runtime slot is appended after the receiver and all ordinary
runtime operands of the terminal group. `CheckedCallRuntimeOperand` gains a
distinct `AttachedContent` member so the one source-ordered projection remains
complete while every member retains its independent ABI destination. A sorted
ABI view is derived only after evaluation; no caller sorts the stored row or
manually appends a value.

Source evaluation order is:

```text
callee/receiver
ordinary authored operands and expanded slots in source order
present attached-content value and effect-capture bindings
ordinary call execution
```

The compiler and runtime fact retain that physical source row with a typed
`abi_position` on each member. Construction validates the ABI positions as one
exact permutation without sorting or dropping the row. General call, host, and
project-call consumers evaluate the row once in source order, then install the
materialized values by ABI destination. Project-call logical materialization
indices address source-row indices, never ABI positions. Structural specialized
targets (Agent expressions, Agent comparisons/diagnostics, Variant, and
Reduction) first ANF-materialize the same source row into typed synthetic
locals, then build their target payload from those locals in checked ABI/role
order. They may not reorder expression trees or lower one source twice.

Required passes `DialogueContent`. Optional/defaulted present passes
`Some(DialogueContent)`. Optional/defaulted omission passes `None` in the same
ABI position. The runtime resolved row retains the exact normalized ABI type
for every presence, including omission; Required seals exact
`DialogueContent`, and Optional/Defaulted seal exact
`Option<DialogueContent>`. `None` is constructed from that row without callee
schema lookup or nominal reconstruction. For Optional, that Option value is
also the callee binding. For Defaulted, the checked Option ABI remains the
call-interface contract, but ProjectCall materialization converts a present
source to the exact `DialogueContent` binding or invokes the declaration-owned
default FunctionSite for omission. The terminal target FunctionSite receives
only the final binding type; it never branches on a hidden raw Option or
reconstructs the callee schema.

The checked callable interface independently derives the same final ABI
position and type. Interface digest mismatch, missing slot, duplicate slot,
non-contiguous order, or caller/callee type disagreement fails before runtime
publication.

The compiler projects that final interface once into
`RuntimeProjectCallable.attached_content_abi`. `RuntimeProjectCallable` also
retains the exact generation-local `HirCallableSourceOwner` copied from its
`CallableSymbol`; this selects one final-HIR member without scanning by
binding, default, or spelling. The descriptor owns checked group and ABI
position, presence, declared binding/local type, final ABI type, and an
optional checked default row `(generation-local source, stable coordinate,
acyclic default digest)`.
Required has `DialogueContent` binding/ABI types; Optional has
`Option<DialogueContent>` for both; Defaulted has a `DialogueContent` binding,
an `Option<DialogueContent>` ABI slot, and exactly one default row. Entry,
direct dispatch, and callable values all carry this same
`RuntimeProjectCallable`; `RuntimeEntryCallableInput` owns it directly rather
than duplicating declaration/owner or another descriptor.

Defaulted function-site reservation creates a declaration-owned default site
with the exact checked result, effect row, suspension role, executable-control
role, and capture-source row. Capture sources address already materialized
`ContinuationPrefix(position)` or `CurrentLogical(position)` values. The
ProjectCall default stage invokes that site only for omission, may suspend,
validates its result as the attached binding type, and then enters the already
selected terminal target without reevaluating any callee or operand.

Project callable execution is owned by `RuntimeFunctionSite`, not by
`RuntimePureHelper`. Each checked call owns a typed input `Direct |
Continuation { exact_callee, expected_abi }` and outcome `Continue {
result_abi, next_group } | Invoke(instance)`. `Direct` carries no invented
lineage: a continuation digest is issued only by the checked nonterminal result
that creates it. `Continue` constructs a
`ProjectContinuation` containing the stable declaration/prefix lineage and
the once-evaluated flattened prefix values; it neither reserves nor returns a
function site. A later checked application validates that lineage before it
appends the current operands exactly once. Only `Invoke` names a closed
`(RuntimeCallableId, CallableInstantiationDigest, CallableGroupIndex)`
instance. Runtime-plan reserves every reached terminal instance site before
defining any body, so self/mutual recursion and cross-module calls resolve
through one typed site graph.

The live continuation ABI stores `RuntimeSemanticTypeId` for its remaining
function type and every prefix value. `RuntimePlanTypeId` and `AwbcTypeId` are
local coordinates and may never enter that carrier. Native execution resolves
semantic identities through the plan type table; AWBC resolves its local type
rows and copies `AwbcRuntimeType::semantic_identity()` into the same carrier.
Session-save stores semantic identities, not table ordinals. Ordinal +/-1
conversion between plan and AWBC type domains is prohibited.

The terminal instance owns the attached binding and authored body. Its ABI
explicitly maps every earlier-group parameter to a stable continuation-prefix
position and every terminal-group parameter to a logical current-group
position; the physical source-to-logical materialization remains on the call
fact, including deterministic rest packing. The
callee never rediscovers prefix locals from scopes or operand values. A final
DirectFrame may use `Expression` only when the shared selected-body fold seals
all three conditions: `ExpressionCompatible`, `NonSuspending`, and a closed
empty effect row. Any selected ordinary project call seals `FlowRequired`, so
even a pure, non-suspending caller uses `Executable`. Nonempty effects or
`MaySuspend` also select `Executable`. Ordinary functions, closures, and
attached defaults consume this same executable-control fold; item roles do
not publish a parallel emission selector. This decision consumes final checked
execution/control/suspension/effect facts, never raw HIR effect-clause
presence. The instantiation digest comes from the final callable
join and is independent of call-site/application identity. Compiler
projection publishes the exact terminal function type and
substitution-backed body/local/type projection. That projection follows the
exact HIR executable-owner row and excludes nested closure bodies, locals, and
types, which belong to their own executable rows. Runtime-plan does not choose
a representative call, scan scope descendants, or lower open generics.

Every runtime call retains its checked completed group. Its attached operand
retains the ABI position rather than consuming it during sorting. The call's
group plus operand position join the callee descriptor's group plus position;
an earlier partial group carries no attached row, while the terminal group
must carry exactly one. A value-dispatch continuation joins the same values to
its remaining checked function type and call-fact outcome; it does not
reconstruct a project descriptor or select a function site from the runtime
callee value.

The ordinary flow algebra gains one typed `ProjectCall { plan, result }`
control operation in seed and final forms. Its plan owns the direct or exact
continuation input, source-ordered physical operands, logical materialization,
optional attached/default stage, and Continue/Invoke outcome. Structured
native execution pushes a function-call frame that owns the caller cursor,
pending operations, control stack, environment, await observer, cleanups, and
result pattern; return validates the site result and restores/binds atomically.
Suspension preserves that stack inside the same fiber. AWBC lowers the same
state machine to the canonical `ProjectCall` terminator and resumes into one
result register; it never falls back to synchronous `ApplyFunction`. A pure
expression context is ANF-lowered to a mini CFG containing that same transfer.
No default-, content-, or effect-specific interpreter may invoke a project
callable.

An AWBC default/target return frame retains only the exact ProjectCall
terminator coordinate `(caller function, block)`, its stage tag, and the
already evaluated prefix/logical values still needed by the default stage. It
does not clone `AwbcProjectCall` or copy outcome/result-pattern/type/resume
fields. Return and snapshot admission rejoin that coordinate to the verified,
artifact-pinned program, require the expected default/target function and
resume cursor, then consume the program-owned attached/outcome/result rows.
The AWBC fiber snapshot uses an explicit return-continuation DTO so retained
values pass through `AwbcRuntimeValueSnapshot`; a raw `RuntimeValue` row is not
serialized inside `FiberReturnPoint`.

`return`/`ReturnExpr` complete the nearest FunctionCall boundary; break and
continue may not cross it. Goto/GotoExpr remain nonlocal Flow transfers: after
the dynamic target (if any) is evaluated once, they unwind all call/default
stages and scopes, run cleanups once, discard pending result bindings, and enter
the target Flow. Runtime-plan always appends an explicit typed ReturnExpr for
the authored function tail; falling off a live FunctionCall frame is invalid.
ProjectCall is suspension-capable and never AOT-linear: the AOT prefix stops
before it, the flow is Mixed, and statistics classify it with Await/control
transfer.

This requirement replaces the prior
`EffectfulDirectFrameUnsupported`/`SuspendingDirectFrameUnsupported` result
for a reached DirectFrame project callable. It does not silently widen
StreamFactory emission: the existing stream contract remains its sole owner.
The predecessor and this attached-content ABI switch form one implementation
cut because admitting an effectful default while retaining pure-helper-only
callee execution would drop accepted semantics.

## 9. Runtime content fragment authority

`arcweft-runtime-plan` owns one report-local fact:

```rust
pub struct RuntimeContentFragmentFact {
    id: RuntimeContentFragmentId,
    source: ExprId,
    owner: StableCheckedValueCoordinate,
    template: DialogueContentFragmentTemplate,
    values: Box<[RuntimeDialogueValueExpression]>,
    effects: Box<[RuntimeDialogueEffectProgramFact]>,
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

The exact runtime ID is issued from the accepted content semantic path plus a
fragment ordinal/path, never an HIR ordinal alone. `source` is an explicit
generation-local join from final expression to report-local fact; the catalog
requires a one-to-one `source -> id` relation. It is excluded from fragment
identity, semantic digest, cache identity, and persistence. No consumer
reverses a stable coordinate or treats `ExprId` as semantic identity. Every
template value slot and effect site has exactly one ordered binding row. Extra, missing,
duplicate, foreign-owner, or differently ordered rows reject the whole fact
batch. Each effect capture list is exactly the operation program's typed free
locals in deterministic first-use order. It is not reconstructed later by the
compiler. Each program also carries the final checked site's closed
`EffectSet`; deriving it from the operation is invalid.

The core runtime content manifest/value evolves in place to carry both value
bindings and effect callbacks. The template manifest owns each effect site's
core trigger and capture-type ABI. The content value owns an exact site-to-zero-
argument-`RuntimeFunctionValue` binding; that callback packages the static
function site and captured runtime values through the existing closure
authority. Text-model materialization rebases effect sites and callbacks
together.

Final sema seals `CheckedDialogueEffectSite.effects: EffectSet` directly from
the selected checked call application's closed effect row and commits that set
in the rich-text semantic transcript. The canonical `EffectId` value and its
v1 digest move unchanged to foundational `arcweft-id`; sema re-exports it and
retains the HIR projection. Core's sorted, duplicate-free
`RuntimeFunctionEffectSet(Box<[arcweft_id::EffectId]>)` is the sole runtime
typed owner. Runtime-plan projects it exactly once from the program fact;
AWBC interning consumes that typed set and is only an encoding destination.
Operation variants, debug labels, integers, and ad hoc strings cannot produce
or amend the row.

The existing function-site authority evolves in place because its current
body is expression-only and cannot execute `RuntimeEvaluatedEffectFact` or
lower to `EmitEffect`:

```rust
pub enum RuntimeFunctionSiteBody {
    Expression(RuntimeExpr),
    Executable(RuntimeFunctionExecutableBody),
}

pub struct RuntimeFunctionExecutableBody {
    effects: RuntimeFunctionEffectSet,
    ops: Box<[FlowOp]>,
}
```

The construction boundary owns matching
`RuntimeFunctionSiteBodySeed::{Expression, Executable}` and
`RuntimeFunctionExecutableBodySeed`. This is an in-place version-1 evolution
of `RuntimeFunctionSite`/`RuntimeFunctionSiteSeedId`, not a content-specific
function table. An executable body uses the existing closed flow-op algebra,
has the declared result type, owns the exact sorted typed
`RuntimeFunctionEffectSet`, validates all paths and locals, and may emit or
suspend only within that set. A content reveal callback has zero ordinary
parameters, exact ordered captures, an executable body, and Unit result. Its
sole runtime-plan producer lowers the already sealed
`RuntimeEvaluatedEffectFact` once into the existing
`RuntimeFlowOpSeed::EvaluatedEffect` path and projects the checked effect row
into that set in the same transaction. AWBC lowering must intern this set into
the callback function signature; a synthetic callback may not default to
effect-set zero. Executable lowering also passes the same typed set to
`intern_evaluated_effect`; every emitted effect plan signature must carry the
same `AwbcEffectSetId` admitted by its enclosing callback. Updating only the
function signature while leaving the effect plan at id zero is invalid.

Structured synchronous expression application accepts only `Expression`.
Reveal handling resolves the exact callback by effect-site identity and starts
an executable function frame through the ordinary structured flow scheduler.
AWBC pending closure lowering accepts the same body enum; `Expression` uses
the existing expression lowerer and `Executable` uses the existing flow
lowerer, producing an ordinary AWBC function that may contain `EmitEffect`.
No callback operation is interpreted from the content value.

`RuntimeExprSeedKind::DialogueContent`, final `RuntimeExprKind`, plan
construction, `MakeDialogueContent`, AWBC codec, verifier, VM, and AWBC session
save consume this same schema. Construction evaluates value bindings and
snapshots callback captures once. Reveal-time operations invoke the stored
callback according to the manifest trigger; they are not executed or silently
dropped during content construction. Core owns the lower trigger enum, so it
does not depend on runtime-plan.

The AWBC reveal entrypoint is a core-owned internal-function activation from a
zero-argument `RuntimeFunctionValue`: it validates an AWBC-backed callback,
creates a `FiberState` for that exact `AwbcFunctionId`, transactionally binds
the stored ordered capture values to the function frame, and then runs it only
through `awbc::vm::step_with_host_context`. `EmitEffect` therefore produces
the ordinary `VmObservation::Effect`. Structured callbacks enter the
structured scheduler instead; neither backend calls a pure evaluator. A
foreign program, nonzero remaining arity, wrong capture ABI, non-executable
structured site, missing effect site, or duplicate reveal activation rejects
before a fiber/frame or observation is published.

The existing AWBC `ApplyFunction` instruction and reveal activation share one
factored runtime-function activation validator and frame binder. Reveal does
not implement a second application path. Saved/restored AWBC callbacks remain
ordinary function-value snapshots and re-enter through that same validator.

Reveal activation has one replay identity:
`RuntimeDialogueEffectCallbackActivationId { dialogue:
DialogueActivationId, site: RuntimeDialogueEffectSiteId }`. The site is the
rebased materialized site; `DialogueActivationId` already includes artifact,
owner fiber, content plan, and occurrence. Function-site IDs, AWBC function
IDs, and fiber generations are execution destinations, never allocation keys.
The activation transaction validates and consumes the content effect event,
resolves the stored callback, reserves this identity, and enqueues the
ordinary runtime function atomically.

This callback path replaces the old line-task direct-effect path in the same
public switch. Runtime-plan deletes `lower_dialogue_effects` and produces
neither delayed line operations nor `ContentEffect` child actions from
fragment effects. Core/AWBC delete their ContentEffect line-task trigger
variants and the reducer effect-ready set. The content reveal Effect event
remains only as callback ingress; marks alone continue into the line-task
reducer. Keeping a direct `RuntimeFlowOp::EvaluatedEffect` line action beside
the callback is invalid because it would execute one authored site twice.

`RuntimeResolvedCall` owns
`attached_content: Option<RuntimeResolvedAttachedContent>` separately from
ordinary argument origins. FinalExpr lowers it to the required ABI value and
the ordinary call instruction consumes the complete ABI-ordered iterator.

## 10. Digests and atomicity

All domains remain v1 and commit:

- callable schema: terminal group, presence, policy/role, execution;
- callable interface: stable binding coordinate, ABI position/type, acyclic
  checked default-expression digest/effects or explicit absence;
- checked call core: no-contract/omitted/present family, stable application
  coordinate for present, and runtime ABI position; raw HIR content ID omitted;
- checked RichText: final admission tag before token/effect-plan content,
  including each site's closed effect set and capture schema;
- runtime fragment: stable owner/id, template digest, ordered value programs,
  ordered effect programs/sets, capture schemas, and explicit absence; the
  generation-local `source` lookup key is excluded;
- runtime plan/AWBC: the evolved content manifest and call ABI inventory.

Candidate probing, selected replay, C1 call sealing, C2 content sealing,
runtime-fact publication, plan construction, and AWBC admission are each
preflight-then-commit transactions. Failure publishes no partial call, content
report, fragment, type slot, instruction, or digest.

## 11. Diagnostics and tooling

Role, presence, default type/effect, owner, and result failures are authored
semantic diagnostics with exact attached-parameter or body source roles.
They do not surface as `WrongPayloadFamily`, mapper corruption, or source-name
errors. Signature help displays the attached parameter after ordinary groups;
hover displays required/optional/defaulted plus the closed role. Formatter
prints the canonical bracket form and never rewrites an attribute/value
parameter into it.

## 12. Deletion boundary

After the public switch delete:

- any schema-less success for a present generic attached body;
- any `effective_role().unwrap_or(Rich)` or equivalent fallback;
- raw scans of attached applications to infer runtime calls;
- split root-only content template/value/effect inventories;
- ContentResult child-template construction that discards bindings/effects;
- source spelling, reserved parameter name, result-type, or attribute scans;
- temporary rejection tests whose only purpose was the absent declaration
  authority; and
- any second runtime operand or fragment projection.
