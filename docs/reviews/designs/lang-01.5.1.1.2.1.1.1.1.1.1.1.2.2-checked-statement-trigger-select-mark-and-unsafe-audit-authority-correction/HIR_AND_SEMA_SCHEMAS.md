# HIR and sema schemas

All fields shown without `pub` are private. HIR constructors are
`pub(crate)` to their owning crate. Final checked constructors are
`pub(crate)` to final analysis. Public checked types expose borrowed/copying
read-only accessors only because compiler, runtime-plan construction, verifier,
and tools consume them.

## Syntax and HIR mark identity

```rust
pub struct SyntaxDialogueMarkName {
    identifier: SyntaxIdentifier,
    range: SyntaxRange,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkId {
    content: HirDialogueContentId,
    ordinal: HirDialogueMarkOrdinal,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMarkName(HirName);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueMark {
    id: HirDialogueMarkId,
    name: HirDialogueMarkName,
    tag: HirRichTextTagId,
}

pub struct HirDialogueContent {
    // existing fields
    marks: Box<[HirDialogueMark]>,
}

pub enum HirRichTextTagPayload {
    // existing accepted non-marker variants
    Marker(HirDialogueMarkId),
    // existing recovery form remains poison
}
```

`HirDialogueMarkOrdinal::get() -> u32`, `HirDialogueMarkId::content()`,
`ordinal()`, and borrowed accessors on mark/catalog are read-only. Only the
content construction transaction mints IDs. It proves exact content ownership,
zero-based contiguous source order, one marker-tag↔row relation, unique local
names, and absence of recovery.

## HIR Trigger and Select

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTrigger {
    Input(PatternId),
    Event(PatternId),
    Signal { target: ExprId, value: Option<PatternId> },
    Timeout(ExprId),
    Mark(HirDialogueMarkId),
    Select(PatternId),
    Task(PatternId),
    Scope(PatternId),
    Expression(ExprId),
    Recovered(HirTriggerIssue),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirTriggerIssue {
    Missing,
    Malformed,
    UnknownDialogueMark,
    MarkOutsideDialogueApplication,
}

pub enum HirSelectBranchHead {
    Bind {
        binding: HirSelectBindingLocal,
        source: ExprId,
    },
    Frame {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Event {
        pattern: PatternId,
        locals: Box<[LocalId]>,
    },
    Recovered,
}
```

The issue list is source-independent and may grow only by revising this
unreleased closed shape in place; it never stores attempted spelling.
`Recovered`/`HirTriggerIssue` has no checked success tag. Mark has no pattern
edge. There is no `HirTriggerPattern`, `Expr` alias, or `propagates_error`.

## HIR unsafe identity

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnsafeAuditIdentity {
    Accepted(UnsafeAuditId),
    Recovered(HirUnsafeAuditIdentityIssue),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirUnsafeAuditIdentityIssue {
    Missing,
    InvalidReference,
    NonAbsolute,
    WrongFamily,
}

pub struct HirUnsafeAudit {
    identity: HirUnsafeAuditIdentity,
    reason: Option<ExprId>,
    has_safety_doc: bool,
}
```

Both final-HIR statement lowerers call one shared typed absolute-ID helper.
Only an accepted absolute `@unsafe.*` reference constructs `UnsafeAuditId`.

## Registration-owned ingress schema

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StandardStatementIngressTypeId {
    TaskEvent,
    ScopeExit,
    FrameBoundary,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StatementIngressTypeRoleId {
    Task,
    Scope,
    Frame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatementIngressTypePublicationInput {
    role: StatementIngressTypeRoleId,
    ty: StandardStatementIngressTypeId,
}

pub enum TypeKind {
    // existing variants
    StatementIngress(StandardStatementIngressTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredStatementIngressTypes {
    input: TypeKind,
    task: TypeKind,
    scope: TypeKind,
    frame: TypeKind,
}

pub struct RegisteredTypeCheckEnv {
    // existing accepted fields
    statement_ingress: RegisteredStatementIngressTypes,
}
```

`TypeCheckEnv` owns a private
`statement_ingress_inputs: Box<[StatementIngressTypePublicationInput]>` build
input. `TypeCheckEnv::new()` fills the exact three rows. No public ad hoc setter
is added. The in-place registration transaction consumes those rows and calls
the private `RegisteredStatementIngressTypes::try_new`; input is fixed to
`Ref<Input>` and the three mappings are exact. Accessors are:

```rust
impl RegisteredStatementIngressTypes {
    pub const fn input(&self) -> &TypeKind;
    pub const fn task(&self) -> &TypeKind;
    pub const fn scope(&self) -> &TypeKind;
    pub const fn frame(&self) -> &TypeKind;
}

impl RegisteredTypeCheckEnv {
    pub const fn statement_ingress(&self) -> &RegisteredStatementIngressTypes;
}
```

Semantic type tags within the existing version-one type transcript use outer
tag `88` for `StatementIngress`, followed by one byte: `TaskEvent=0`,
`ScopeExit=1`, `FrameBoundary=2`. Current accepted source uses outer tags
through `87`; this contract consumes `88` directly. The implementation must
reject a concurrent collision rather than silently renumber either owner.

Each ingress ID is a distinct closed opaque atomic match domain. Type equality,
compatibility, substitution, and unification accept only the identical ID;
there is no coercion to another ingress atom, `Named`, `Other`, or a runtime
Rust type. Binding, mutable-binding, discard, whole-binding, and an exact typed
binding may consume the atom; literal, entity, nominal, variant, tuple, record,
and sequence destructuring reject because the atom publishes no such schema.

## Private contextual selector

```rust
pub(crate) enum StatementScrutineeRole {
    TriggerInput,
    TriggerEvent,
    TriggerSignal,
    TriggerSelect,
    TriggerTask,
    TriggerScope,
    SelectFrame,
    SelectEvent,
}

pub(crate) struct StatementScrutineeTypeAuthority<'a> {
    standard: &'a RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'a>,
    topology: &'a HirProjectEvaluationTopology,
    entries: &'a PreparedEntrySemanticAuthority<'a>,
}

pub(crate) struct PreparedExecutableIngressFacts {
    declarations: BTreeMap<
        CallableDeclarationKey,
        PreparedDeclarationIngressProof,
    >,
}

pub(crate) struct PreparedExecutableIngressWorklist {
    facts: PreparedExecutableIngressFacts,
    pending: BTreeSet<CallableDeclarationKey>,
}

pub(crate) struct PreparedExecutableIngressSeal {
    facts: PreparedExecutableIngressFacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StatementPreparationLimits {
    max_declarations: u64,
    max_edges: u64,
    max_entry_contributors: u64,
    max_contextual_statements: u64,
    max_work: u64,
}

pub(crate) struct PreparedEntrySemanticAuthority<'a> {
    // existing exact type/item/call/callable/runtime-nominal borrows
    ingress: &'a PreparedExecutableIngressFacts,
}
```

The authority deliberately has no `Clone` implementation, owned `TypeKind`,
map, or published output. `PreparedExecutableIngressFacts` is the worklist's
single private proof-construction map. Short-lived Entry-authority views borrow
it immutably; they are dropped before the worklist mutates. Independent graph
recomputation consumes the worklist into `PreparedExecutableIngressSeal`, whose
private constructor is the only route to final Entry checking. A private
move-only `PreparedEventScrutineeProof` may carry the statement ID, selected
`SemanticTypeDigest`, and exact reachable checked Entry item identities only
until Entry sealing. The seal and every proof are consumed, never placed in
`FinalSemanticAnalysis`.

The production limit constructor is private and derives the first four maxima
from the already accepted bounded callable-declaration, selected-call plus
Include, stateful-Entry, and HIR-statement inventory counts. `max_work` is the
checked sum/product needed to visit each declaration/edge contributor delta at
most once; arithmetic overflow rejects before work begins. Tests use a private
constructor with smaller exact N values. Limits are operational admission, not
semantic identity, and are not encoded.

## Stable checked mark and rich text

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableCheckedDialogueMarkCoordinate {
    application: CheckedSemanticPath,
    ordinal: HirDialogueMarkOrdinal,
}

#[derive(Clone, Debug)]
pub struct CheckedDialogueMark {
    coordinate: StableCheckedDialogueMarkCoordinate,
    diagnostic_name: HirDialogueMarkName,
}

pub enum CheckedRichTextAction {
    // existing closed non-marker variants unchanged
    Marker(CheckedDialogueMark),
}

pub struct CheckedDialogueLinePlan {
    effect_sites: Box<[CheckedDialogueEffectSite]>,
}
```

`StableCheckedDialogueMarkCoordinate` has no public constructor. The sole
issuer is `SemanticCoordinateIndex::dialogue_mark(HirDialogueMarkId)`.
`CheckedDialogueMark` implements `Eq`, `Ord`, and `Hash` manually on
`coordinate` only. `diagnostic_name()` is display-only. The source-ordered
Marker actions are the sole checked mark inventory.

## Checked Trigger, Select, unsafe audit

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedTrigger {
    Input,       // tag 0
    Event,       // tag 1
    Signal,      // tag 2
    Timeout,     // tag 3
    Mark(StableCheckedDialogueMarkCoordinate), // tag 4
    Select,      // tag 5
    Task,        // tag 6
    Scope,       // tag 7
    Expression,  // tag 8
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSelectStatement {
    Operand, // tag 0
    Branches(Box<[CheckedSelectBranchHead]>), // tag 1
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedSelectBranchHead {
    Bind,  // tag 0
    Frame, // tag 1
    Event, // tag 2
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedUnsafeAudit {
    id: UnsafeAuditId,
    has_safety_doc: bool,
}
```

There are no checked recovery variants. Constructors validate completed child
facts and consume their equality proofs. `CheckedUnsafeAudit::id()`,
`has_safety_doc()`, and `semantic_id()` are read-only; `semantic_id()` delegates
to `UnsafeAuditId::semantic_id()` and does not retain copied digest bytes.

## Complete checked statement

```rust
pub struct CheckedStatement {
    effects: EffectSet,
    payload: CheckedStatementPayload,
}

pub enum CheckedStatementPayload {
    Structural,                                      // tag 0
    Assignment(Box<CheckedAssignment>),              // tag 1
    Assertion(CheckedAssertionDisposition),          // tag 2
    Defer(DeferOutcome),                             // tag 3
    EvaluatedEffect(Box<CheckedEvaluatedEffect>),    // tag 4
    Iteration(Box<CheckedIteration>),                // tag 5
    ControlTransfer(CheckedControlTransferTarget),   // tag 6
    Trigger(CheckedTrigger),                         // tag 7
    UnsafeAudit(CheckedUnsafeAudit),                 // tag 8
    Select(CheckedSelectStatement),                  // tag 9
    SourceLocale(LocaleTag),                         // tag 10
    Scope(CheckedScopeIdentity),                     // tag 11
    Include(CheckedIncludeFlowTarget),               // tag 12
    Suspension(Box<CheckedSuspensionStatement>),     // tag 13
    Yield,                                           // tag 14
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedScopeIdentity {
    Anonymous,
    Named,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedIncludeFlowTarget {
    declaration: CallableDeclarationDigest,
}
```

`CheckedStatement::new` is the sole crate-private constructor. It receives the
completed child-effect fold, validates payload-specific effects, validates the
explicit 35-arm producer match, and atomically publishes `effects` and
`payload`. Existing checked assignment/assertion/defer/effect/iteration/control
transfer/suspension types are reused, with read-only visibility promoted where
required. No raw coordinate constructor is exposed.

## Runtime-plan admission boundary

```rust
pub enum RuntimeTriggerAdmission {
    Input,
    Event,
    Signal,
    Timeout,
    Mark(RuntimeDialogueMarkId),
    Select,
    Task,
    Scope,
    Expression,
}
```

The type is public only across compiler→runtime-plan typed input. Its
constructor/projection remains internal. Runtime statement facts are keyed by
same-generation `StmtId` lookup evidence, not persistent identity.

## Deleted schemas

The cut removes, rather than deprecates:

- `HirTriggerPattern` and its `Expr`/pattern-shaped Mark forms;
- Select `propagates_error` everywhere;
- checked `CheckedStatementRole`/`Ordinary` and old constructors;
- checked/public mark ordinal, handler, PublicId mark slices, and mark side
  tables;
- prepared line-plan mark and handler fields/constructor parts;
- runtime `RuntimeDialogueMarkHandler` and
  `RuntimeDialogueApplication.mark_handlers`;
- unsafe `id_ref_label` and checked validators that re-read HIR;
- any success branch using `Any`, `Other`, `TypeKind::Named`, a raw string, or
  fallback lookup for these meanings.
