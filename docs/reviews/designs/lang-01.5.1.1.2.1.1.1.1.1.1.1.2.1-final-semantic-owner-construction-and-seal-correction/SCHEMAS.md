# Exact schemas

All shown fields and visibility are normative. Implementations may split a
module for cohesion but may not change ownership or add a second model.

## 1. Final-analysis draft and seal

The types in this section are placed in C2.4. They are not forward-declared or
approximated during C2.2; their prepared/final inventories are defined only
after the exact C2.3 owner-row types exist.

```rust
// arcweft-lang-sema::final_analysis
pub enum FinalSemanticProjectError {
    Semantic(FinalSemanticAnalysisError),
    Entry(Box<[CheckedEntryDiagnostic]>),
}

pub fn analyze_final_project(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    catalogs: FinalSemanticCatalogs<'_>,
    control: FinalSemanticAnalysisControl<'_>,
) -> Result<FinalSemanticAnalysis, FinalSemanticProjectError>;

pub struct FinalSemanticAnalysis {
    // existing generation and checked fact fields
    checked_entries: CheckedEntryCatalog,
    runtime_nominals: RuntimeNominalProjectionCatalog,
}

impl FinalSemanticAnalysis {
    pub const fn checked_entries(&self) -> &CheckedEntryCatalog;
}

pub struct CheckedExpressionRecordField {
    source_ordinal: u32,
    semantic_id: AcceptedRecordFieldSemanticId,
}

pub struct CheckedExpressionEdgeFact {
    edges: Box<[(ExprId, CheckedExpressionChildRole)]>,
    record_fields: Box<[CheckedExpressionRecordField]>,
    callable: Option<CheckedCallableJoin>,
}

pub(crate) struct FinalSemanticAnalysisDraft {
    // collected existing maps, callables, type reports and work
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
    patterns: BTreeMap<PatternId, PreparedPatternFact>,
    callable_joins:
        BTreeMap<ExprId, Result<CheckedCallableJoin, CheckedCallableJoinError>>,
}

pub(crate) struct PreparedEntryReference {
    diagnostic_public_id: PublicId,
    lookup_owner: ItemId,
}

pub(crate) struct PreparedEntryExpression {
    reference: PreparedEntryReference,
    ty: TypeKind,
    value_type: SemanticTypeDigest,
    type_selection: CheckedTypeSelection,
    effects: EffectSet,
}

pub(crate) enum PreparedExpressionFact {
    Complete(CheckedExpression),
    Entry(PreparedEntryExpression),
    ProjectVariant(PreparedProjectVariantExpression),
    ProjectField(PreparedProjectFieldExpression),
    ProjectRecord(PreparedProjectRecordExpression),
}

impl PreparedExpressionFact {
    pub(crate) fn ty(&self) -> &TypeKind;
    pub(crate) fn type_selection(&self) -> CheckedTypeSelection;
    pub(crate) fn effects(&self) -> &EffectSet;
}

pub(crate) struct SemanticFactState {
    // existing staging fields
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
    // existing candidate transaction journal/checkpoints
}

pub(crate) struct PreparedEntrySemanticAuthority<'draft, 'project> {
    project: HirExecutableProjectView<'project>,
    symbols: &'draft ProjectSymbolTable,
    checked_callables: &'draft CheckedCallableCatalog,
    types: &'draft BTreeMap<TypeId, TypeKind>,
    items: &'draft BTreeMap<ItemId, CheckedItem>,
    calls: &'draft BTreeMap<ExprId, CallTargetFacts>,
    nominal: &'draft RuntimeNominalProjectionContext<'draft>,
}

pub(crate) struct PreparedExpressionShell {
    ty: TypeKind,
    type_selection: CheckedTypeSelection,
    effects: EffectSet,
}

pub(crate) struct PreparedVariantCaseSeed {
    ordinal: u32,
    payload: Option<TypeKind>,
    diagnostic_name: Option<String>,
}

pub(crate) struct PreparedProjectVariantOwnerSeed {
    nominal: CheckedProjectNominal,
    cases: Box<[PreparedVariantCaseSeed]>,
}

pub(crate) struct PreparedProjectVariantExpression {
    shell: PreparedExpressionShell,
    owner: PreparedProjectVariantOwnerSeed,
    selected_ordinal: u32,
}

pub(crate) struct PreparedProjectFieldExpression {
    shell: PreparedExpressionShell,
    nominal: CheckedProjectNominal,
    declaration_ordinal: u32,
    field_type: TypeKind,
    diagnostic_name: Option<HirName>,
}

pub(crate) struct PreparedProjectRecordExpressionField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    field_type: TypeKind,
    target: ExprId,
}

pub(crate) struct PreparedProjectRecordExpression {
    shell: PreparedExpressionShell,
    nominal: CheckedProjectNominal,
    fields: Box<[PreparedProjectRecordExpressionField]>,
}

pub(crate) enum PreparedPatternFact {
    Complete(CheckedPattern),
    ProjectVariant(PreparedProjectVariantPattern),
    ProjectRecord(PreparedProjectRecordPattern),
}

pub(crate) struct PreparedProjectVariantPattern {
    ty: TypeKind,
    owner: PreparedProjectVariantOwnerSeed,
    selected_ordinal: u32,
}

pub(crate) struct PreparedProjectRecordPatternField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    field_type: TypeKind,
    target: PatternId,
}

pub(crate) struct PreparedProjectRecordPattern {
    ty: TypeKind,
    nominal: CheckedProjectNominal,
    fields: Box<[PreparedProjectRecordPatternField]>,
    has_rest: bool,
}
```

Analyzer performs no projection. It first creates the complete draft. The
exhaustive visitor borrows that draft once to produce an owned ordered request
inventory, then `FinalSemanticAnalysisDraft::into_parts` consumes the draft so
`types`, prepared expressions/patterns, joins, and all other maps can be
borrowed or mutated as disjoint locals. One context borrows the moved `types`
local and projects the owned inventory in `SemanticTypeDigest` order. Every
project seed is then consumed through a cached lookup into its exact final row;
Entry checking also uses cached lookup only.
`RuntimeNominalProjectionContext::finish` consumes the context and ends the
borrow before `FinalSemanticAnalysisDraft::from_sealed_parts` reconstructs the
draft. No `Arc` type-map authority and no prepared/final parallel row is
retained.

`FinalSemanticAnalysisDraft::seal(self, CheckedEntryCatalog,
RuntimeNominalProjectionCatalog, control)` is crate-private and consuming. It
checks that `Entry` variants equal the exact Entry-reference expression
inventory before creating the final expression map.

`SemanticFactState::set_expression`, candidate capture, commit, and rollback
operate on `PreparedExpressionFact`. Their journal stores the prior enum row,
not a second Entry-specific undo table. Recursive analysis and candidate probes
call only the enum's common accessors. The seal consumes each enum value:

```rust
match prepared {
    PreparedExpressionFact::Complete(expression) => expression,
    PreparedExpressionFact::Entry(entry) => {
        seal_entry_expression(entry, checked_entries)?
    }
    PreparedExpressionFact::ProjectVariant(_)
    | PreparedExpressionFact::ProjectField(_)
    | PreparedExpressionFact::ProjectRecord(_) => {
        return Err(FinalSemanticAnalysisError::UnsealedPreparedC2Owner)
    }
}
```

The C2 seed types carry no layout, semantic ID, runtime field ID, or final row.
Their raw `ExprId`/`PatternId` targets are lookup coordinates consumed while
building stable checked edge/pattern rows and are never transcript bytes.
Project declaration-order cases/fields are validated before seed construction;
the seal repeats ordinal and cached projection identity checks before minting
private semantic IDs.

`seal_entry_expression` performs checked ID/public ID, source item, value type,
then binding-digest-copy validation in that exact order.

```rust
pub struct CheckedEntryReference {
    binding: CheckedEntryBindingDigest,
    value_type: SemanticTypeDigest,
    diagnostic_public_id: PublicId,
    lookup_owner: ItemId,
}

impl CheckedEntryReference {
    pub const fn binding(&self) -> &CheckedEntryBindingDigest;
    pub const fn value_type(&self) -> SemanticTypeDigest;
    pub const fn diagnostic_public_id(&self) -> &PublicId;
    pub const fn lookup_owner(&self) -> ItemId;
    pub fn ty(&self) -> TypeKind;
}
```

Its constructor is `pub(crate)` and takes `&CheckedEntryBinding`; callers
cannot supply the digest or type independently. Construction proves
`value_type == ty().semantic_identity_digest()`.

## 2. Runtime nominal projection

`RuntimeNominalProjectionContext`, `ProjectionBudget`, and retained
`RuntimeProjectNominalProjection::shape` are placed in C2.2a. The existing
final-analysis projection wrappers may delegate to the context only as an
internal compile-clean bridge. That bridge is not a published API, accepted
completion, or second projection authority and is deleted in C2.4.

`FinalSemanticPartsView`, `RuntimeNominalProjectionRequestInventory`, complete
catalog sealing, and the final borrowed lookup are placed in C2.4, after C2.3
has supplied the exact owner-row families the visitor must enumerate. No
placeholder row family or partial visitor is legal in C2.2a.

```rust
pub(crate) struct RuntimeNominalProjectionContext<'a> {
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    root_limits: NominalResolutionLimits,
    aggregate_limits: NominalAggregationLimits,
    aggregate_work: u64,
    visiting: BTreeSet<SemanticTypeDigest>,
    accepted: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
    control: FinalSemanticAnalysisControl<'a>,
}

pub(crate) struct ProjectionBudget {
    limits: NominalResolutionLimits,
    nodes: u64,
    depth: u16,
    generic_arguments: u64,
    work: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionRequest {
    semantic_type: SemanticTypeDigest,
    nominal: CheckedProjectNominal,
}

pub(crate) struct FinalSemanticPartsView<'a> {
    types: &'a BTreeMap<TypeId, TypeKind>,
    locals: &'a BTreeMap<LocalId, CheckedBinding>,
    captures: &'a BTreeMap<CaptureId, CheckedBinding>,
    expressions: &'a BTreeMap<ExprId, CheckedExpression>,
    patterns: &'a BTreeMap<PatternId, CheckedPattern>,
    statements: &'a BTreeMap<StmtId, CheckedStatement>,
    items: &'a BTreeMap<ItemId, CheckedItem>,
    calls: &'a BTreeMap<ExprId, CallTargetFacts>,
    checked_callables: &'a CheckedCallableCatalog,
    checked_entries: &'a CheckedEntryCatalog,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionRequestInventory {
    by_semantic_type: BTreeMap<SemanticTypeDigest, CheckedProjectNominal>,
}

impl RuntimeNominalProjectionRequestInventory {
    pub(crate) fn from_prepared(
        draft: &FinalSemanticAnalysisDraft,
    ) -> Result<Self, NominalSchemaProjectionError>;
    pub(crate) fn from_final_parts(
        parts: FinalSemanticPartsView<'_>,
    ) -> Result<Self, NominalSchemaProjectionError>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeNominalProjectionCatalog {
    by_semantic_type:
        BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
}

impl RuntimeNominalProjectionCatalog {
    pub(crate) fn get(
        &self,
        nominal: &CheckedProjectNominal,
    ) -> Result<&RuntimeProjectNominalProjection, NominalSchemaProjectionError>;
}

pub struct RuntimeProjectNominalProjection {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    shape: TypeShape,
    layout: TypeLayoutHash,
    schema: RuntimeTypeSchema,
    kind: RuntimeProjectNominalKind,
}

impl RuntimeProjectNominalProjection {
    pub const fn shape(&self) -> &TypeShape;
    // existing borrowed identity/layout/schema/kind accessors remain
}

pub enum NominalProjectionLimitKind {
    Root(NominalResolutionLimitKind),
    Project(NominalAggregationLimitKind),
}

pub enum NominalSchemaProjectionError {
    // existing generation/owner/arity/shape/schema errors remain
    Cancelled,
    LimitExceeded {
        kind: NominalProjectionLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow,
    IdentityMismatch {
        requested: SemanticTypeDigest,
        projected: SemanticTypeDigest,
    },
    MissingCachedProjection { semantic_type: SemanticTypeDigest },
}
```

`from_prepared` includes every `PreparedExpressionFact` and
`PreparedPatternFact` variant above. Projection-dependent seeds are request
sources, not demand-order projection calls. `project_inventory` iterates only
the inventory's ordered `by_semantic_type` map. Once it returns, C2 row sealing
and Entry checking may call cached lookup but may not expand or charge another
root.

Each request starts a fresh `ProjectionBudget`. Context aggregate work is
charged before request lookup; a cache miss then charges its root budget before
allocation/descent. Cancellation precedes the next charge; checked-add
overflow precedes limit comparison; limit failure precedes lookup/allocation.
`RuntimeProjectNominalProjection` retains the canonical `TypeShape` returned by
the sole expander. Its record fields use
`RuntimeRecordFieldId::try_from_zero_based_ordinal`; its variants use checked
source `u32` ordinals. The existing public
`FinalSemanticAnalysis::project_checked_runtime_nominal` becomes a sealed
catalog lookup returning a borrowed projection. It reports identity mismatch
before missing cache. `project_runtime_type_schema` remains the sole pure
`TypeShape -> RuntimeTypeSchema` projection.

The exhaustive request visitor has explicit arms for every prepared/published
fact family and every `TypeKind` variant that can contain a project nominal.
The final seal repeats the request set over final facts, compares it with the
catalog key set, and reports the first digest-ordered
`MissingCachedProjection`; extra cache rows are harmless accepted reachable
dependencies, while missing rows reject. No post-seal API owns a context or
expander.

## 3. Ordered environment records

This section is placed in C2.2b. It can be implemented and tested against the
existing accepted nominal catalog before C2.3 adds consumers; it does not own
or complete the runtime nominal request visitor.

```rust
// arcweft-lang-sema::env::nominal
pub enum AcceptedNominalSemantics {
    Exact(TypeKind),
    Opaque(AcceptedOpaqueRuntimeCarrier),
    Character(CharacterNominalType),
    Record(AcceptedEnvironmentRecordSemantics),
}

pub struct AcceptedEnvironmentRecordSemantics {
    ty: TypeKind,
    semantic_type: SemanticTypeDigest,
    fields: Box<[AcceptedEnvironmentRecordField]>,
}

pub struct AcceptedEnvironmentRecordField {
    diagnostic_name: String,
    ordinal: u32,
    ty: TypeKind,
    type_digest: SemanticTypeDigest,
    semantic_id: AcceptedEnvironmentFieldSemanticId,
}

impl AcceptedEnvironmentRecordField {
    pub fn diagnostic_name(&self) -> &str;
    pub const fn ordinal(&self) -> u32;
    pub const fn ty(&self) -> &TypeKind;
    pub const fn type_digest(&self) -> SemanticTypeDigest;
    pub(crate) const fn semantic_id(&self) -> AcceptedEnvironmentFieldSemanticId;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedEnvironmentFieldSemanticId([u8; 32]);

impl AcceptedEnvironmentRecordSemantics {
    pub const fn ty(&self) -> &TypeKind;
    pub fn fields(&self) -> &[AcceptedEnvironmentRecordField];
    pub fn field(&self, name: &str) -> Option<&AcceptedEnvironmentRecordField>;
    pub const fn semantic_type(&self) -> SemanticTypeDigest;
}

impl AcceptedNominalRecord {
    pub(crate) fn try_new_record(
        id: AcceptedNominalId,
        ty: TypeKind,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
        origin: AcceptedNominalOrigin,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError>;
}
```

`AcceptedEnvironmentRecordSemantics` and its fields derive `Clone`, `Debug`,
`Eq`, `Hash`, and `PartialEq`, so the existing catalog digest includes field
order and type. All fields are private. `try_new_record` is the only mint; the
existing public `try_new` validates any cloned `Record` semantics against the
supplied accepted ID, exact type identity, every ordinal, and every field ID,
and returns `IdentityMismatch` on disagreement. It cannot mint or reorder raw
rows. `try_instantiate([])` returns `Record.ty`; nonempty arguments reject.

`AcceptedNominalCatalogError` gains
`RecordIdentityMismatch { id, expected, actual }`,
`DuplicateRecordField { id, field }`, and `RecordFieldOrdinalOverflow { id }`.
For a Record passed through public `try_new`, validation order is accepted ID/
path and arity, exact `ty` semantic identity, field count/ordinal, duplicate
diagnostic name, field type digest, then private field semantic ID. The first
disagreement rejects before catalog digest construction.

`TypeCheckEnv::nominal_records` and its name map are deleted. Lookup resolves a
`TypePath`, calls `AcceptedNominalCatalog::exact`, matches
`AcceptedNominalSemantics::Record`, then performs a borrowed bounded linear
field lookup. The catalog/world digest is the generation stamp; no second
environment-record stamp exists.

## 4. Opaque identity atoms

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedProjectItemSemanticId([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedVariantCaseSemanticId([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedRecordFieldSemanticId([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AcceptedCharacterLookSemanticId([u8; 32]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CheckedFieldSemanticId {
    Project(AcceptedRecordFieldSemanticId),
    Environment(AcceptedEnvironmentFieldSemanticId),
}
```

Only `as_bytes(&self) -> &[u8; 32]` is exposed within the crate. There is no
`from_bytes` outside owner tests.

### Canonical atom grammar

These atoms use the parent's fixed grammar: `tag8`, little-endian `u32/u64`,
`bool`, `digest32`, and `bytes := u64(byte_count) || bytes`. Counts convert
with `u64::try_from`; overflow is an error before hashing.

```text
project_item :=
  "arcweft.lang.accepted-project-item.v1\0"
  owner_tag8                         // entity=0, Flow=1
  family_tag8
  value_type_digest32
  (bytes(PublicId UTF-8) | CallableDeclarationDigest32)

variant_case :=
  "arcweft.lang.accepted-variant-case.v1\0"
  owner_tag8                         // project=0, character=1, environment=2,
                                     // Option=3, Result=4
  owner_type_digest32
  option<TypeLayoutHash32>           // present only for project
  ordinal_u32
  option<payload_type_digest32>

project_field :=
  "arcweft.lang.accepted-record-field.v1\0"
  owner_runtime_semantic_type32
  TypeLayoutHash32
  RuntimeRecordFieldId_u32
  declaration_ordinal_u32
  field_type_digest32

environment_field :=
  "arcweft.lang.accepted-environment-field.v1\0"
  owner_type_digest32
  declaration_ordinal_u32
  field_type_digest32

character_look :=
  "arcweft.lang.accepted-character-look.v1\0"
  bytes(CharacterId UTF-8)
  bytes(CharacterLookId UTF-8)
  seq<bytes(CharacterPartId UTF-8) || bytes(CharacterVariantId UTF-8)>
```

Character selections are encoded in canonical ascending `CharacterPartId`
order, matching the existing manifest-fingerprint treatment; manifest source
reordering is therefore invariant. The manifest validator already rejects
duplicate/missing parts; sema rechecks that the selected row belongs to the
exact registered Character generation before hashing.

`family_tag8` is an exhaustive sema-local encoding of the current
`DeclarationIdentityFamily`; it is not a cast. Flow hashes the accepted
`CallableDeclarationDigest`, never a call-site join. Other entity families hash
the canonical `PublicId`; raw `ItemId` and `ExternalDeclarationId` are excluded.

## 5. Project item and variant rows

```rust
pub struct CheckedProjectItem {
    semantic_id: AcceptedProjectItemSemanticId,
    family: DeclarationIdentityFamily,
    value_type: SemanticTypeDigest,
    owner: CheckedProjectItemOwner, // lookup only
    diagnostic_public_id: PublicId,
    character: Option<CharacterId>,
    value: Option<TypeKind>,
}

pub enum CheckedVariantOwner {
    Project {
        nominal: CheckedProjectNominal,
        semantic_type: SemanticTypeDigest,
        layout: TypeLayoutHash,
        cases: Box<[CheckedVariantCase]>,
    },
    CharacterNominal {
        nominal: CharacterNominalType,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    BuiltinClosed {
        nominal: EnvironmentBindingId,
        semantic_type: SemanticTypeDigest,
        cases: Box<[CheckedVariantCase]>,
    },
    Option { item: TypeKind, cases: [CheckedVariantCase; 2] },
    Result { ok: TypeKind, error: TypeKind, cases: [CheckedVariantCase; 2] },
}

pub struct CheckedVariantCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    payload: Option<TypeKind>,
    diagnostic_name: Option<String>,
}

pub struct CheckedVariantResolution {
    owner: CheckedVariantOwner,
    selected_ordinal: u32,
}

impl CheckedVariantResolution {
    pub(crate) fn try_new(
        owner: CheckedVariantOwner,
        selected_ordinal: u32,
    ) -> Option<Self>;
    pub fn selected(&self) -> &CheckedVariantCase;
}
```

`try_new` succeeds only when `cases()[ordinal].ordinal == ordinal`. No public
case constructor exists. Option is `[Some(payload), None]`; Result is
`[Ok(payload), Err(payload)]`. Names are diagnostic-only.

## 6. Record patterns, typed bindings, and selections

```rust
pub struct CheckedRecordPattern {
    owner: CheckedRecordPatternOwner,
    fields: Box<[CheckedRecordPatternField]>, // authored source order
    has_rest: bool,
}

pub enum CheckedRecordPatternOwner {
    Project {
        nominal: CheckedProjectNominal,
        semantic_type: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    Environment {
        semantic_type: SemanticTypeDigest,
    },
}

pub struct CheckedRecordPatternField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    runtime_field: Option<RuntimeRecordFieldId>,
    semantic_id: CheckedFieldSemanticId,
    field_type: TypeKind,
    field_type_digest: SemanticTypeDigest,
    target: StablePatternCoordinate,
}

pub enum StablePatternCoordinateStep {
    TupleElement(u32),
    RecordField {
        field: CheckedFieldSemanticId,
        source_ordinal: u32,
    },
    SequenceElement(u32),
    VariantPayload,
    WholeBindingInner,
    OrAlternative(u32),
    TypedBindingInner,
}

pub struct CheckedTypedBinding {
    annotation: TypeKind,
    annotation_digest: SemanticTypeDigest,
}

pub enum CheckedPatternResolution {
    Structural,
    Literal(HirLiteral),
    Entity(CheckedProjectItem),
    Record(CheckedRecordPattern),             // retains old tag 0x0603
    Variant(CheckedVariantResolution),
    TypedBinding(CheckedTypedBinding),        // appended tag 0x0605
}

pub struct CheckedMethodSelection {
    callable: CheckedCallableJoinDigest,
    receiver_type: SemanticTypeDigest,
    receiver_mode: CallableReceiverMode,
}

pub struct CheckedFieldSelection {
    owner_type: SemanticTypeDigest,
    field: CheckedFieldSemanticId,
    declaration_ordinal: u32,
    field_type: SemanticTypeDigest,
    diagnostic_name: Option<HirName>,
}

impl CheckedFieldSelection {
    pub const fn owner_type(&self) -> SemanticTypeDigest;
    pub const fn field(&self) -> CheckedFieldSemanticId;
    pub const fn declaration_ordinal(&self) -> u32;
    pub const fn field_type(&self) -> SemanticTypeDigest;
    pub fn project_runtime_field(&self) -> Option<RuntimeRecordFieldId>;
}

pub struct RuntimeProjectFieldProjection<'analysis> {
    owner: &'analysis RuntimeProjectNominalProjection,
    field: RuntimeRecordFieldId,
    field_type: SemanticTypeDigest,
}

impl RuntimeProjectFieldProjection<'_> {
    pub const fn owner(&self) -> &RuntimeProjectNominalProjection;
    pub const fn field(&self) -> RuntimeRecordFieldId;
    pub const fn field_type(&self) -> SemanticTypeDigest;
}

impl FinalSemanticAnalysis {
    pub fn project_runtime_field(
        &self,
        selection: &CheckedFieldSelection,
    ) -> Result<Option<RuntimeProjectFieldProjection<'_>>, NominalSchemaProjectionError>;
}

pub enum CheckedSelectResolution {
    Method(CheckedMethodSelection),
    DialogueView {
        projection: DialogueProjectionCoordinate,
        owner_type: SemanticTypeDigest,
        diagnostic_name: HirName,
    },
    AgentField {
        field: RuntimeAgentField,
        owner_type: SemanticTypeDigest,
        field_type: SemanticTypeDigest,
    },
    ProgressField {
        field: ProgressField,
        owner_type: SemanticTypeDigest,
        field_type: SemanticTypeDigest,
    },
    Field(CheckedFieldSelection),
}
```

The source field name is used once to select a project/environment row, then
retained only in `diagnostic_name`. `TupleElement` and `RecordElement` are
absent. The sema-root shared stable pattern coordinate uses the shared semantic
field identity rather than `RuntimeRecordFieldId`, because environment rows do
not fabricate project nominal identity. Project rows continue to retain their
accepted runtime field separately for compiler/runtime lowering. Existing C1
`HirPatternChildRole` values remain unchanged; the checker joins their source
field ordinal to the C2 row before constructing this sema-private coordinate.
`FinalSemanticAnalysis::project_runtime_field` is a sealed-catalog lookup. It
returns `None` only for an exact environment field row and validates project
owner, field ordinal/runtime ID, and field type against the cached projection
before returning a borrowed owner plus typed coordinate. It never projects or
uses `diagnostic_name`.

Existing `CheckedExpressionChildRole::RecordField { source_ordinal,
accepted_field }` and all of its C1 transcript bytes remain unchanged. The
atomic `CheckedExpressionEdgeFact` constructor requires exactly one
`CheckedExpressionRecordField` for each such role and no extras. It validates
equal source ordinals, declaration ordinal equal to
`accepted_field.zero_based()`, child field-type digest, and the semantic ID
derived from owner semantic type, layout, the role-owned runtime field ID,
declaration ordinal, and field type. The role remains the only runtime
coordinate owner; the adjacent row remains the only semantic field-ID owner.
C3 hashes the unchanged role bytes followed by the atom from the same edge
fact. No side table or duplicated runtime coordinate is retained.

## 7. Call joins, Effect, StageLook, View, and Style

The call shapes and phase boundary in this section are superseded and expanded
by [CALL_APPLICATION_AUTHORITY_AMENDMENT.md](CALL_APPLICATION_AUTHORITY_AMENDMENT.md).
In particular, `finalize_call_facts` means consuming the sole private prepared
transaction through the unpublished core/continuation boundary into one sealed
`CheckedCallApplication`; it does not rebuild a previously public selected
fact. `prepare_checked_callable_joins` accepts only
sealed applications and performs no type inference or result reconstruction.

```rust
// callable/join.rs
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableJoinDigest([u8; 32]);
impl CheckedCallableJoin {
    pub fn semantic_digest(&self) -> CheckedCallableJoinDigest;
}

pub(crate) fn prepare_checked_callable_joins(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    expressions: &mut BTreeMap<ExprId, PreparedExpressionFact>,
    calls: &BTreeMap<ExprId, CallTargetFacts>,
    checked_callables: &CheckedCallableCatalog,
) -> BTreeMap<ExprId, Result<CheckedCallableJoin, CheckedCallableJoinError>>;

// effects.rs
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectSemanticDigest([u8; 32]);
impl EffectId {
    pub fn semantic_digest(&self) -> EffectSemanticDigest;
}

pub struct CheckedStageLook {
    character_nominal: SemanticTypeDigest,
    character: CharacterId,
    look: AcceptedCharacterLookSemanticId,
    diagnostic_name: HirName,
}

pub enum CheckedViewCallKind { Element(ViewElementKind), Text, RichText }
pub struct CheckedViewCall {
    owner: CallableDeclarationDigest,
    kind: CheckedViewCallKind,
}
pub enum CheckedViewCalleeKind { Element(ViewElementKind), Text, RichText }
pub struct CheckedViewCallee {
    owner: CallableDeclarationDigest,
    kind: CheckedViewCalleeKind,
}
```

The exact order is:

```text
private prepared calls
-> checked callable/effect catalogs
-> stable candidate + application-core seal
-> continuation/result + final application seal and sole publication
-> inference-free join construction
-> Method enrichment
-> join-map move into edge facts
```

`prepare_checked_callable_joins` runs exactly once after
`finish_checked_callables` and the single final call-application publication.
Its sole private composer
owns method-key construction plus `validate_selected_call`. Every successful
join enriches the corresponding explicit Method callee with join digest,
receiver type digest, and cloned `CallableReceiverMode`. The returned map moves
through `FinalSemanticAnalysisInput`/draft and is consumed by
`collect_checked_edges`; edge collection cannot call the composer or callable
catalog again. Recovery errors move into the corresponding atomic edge error.
No callable-join map is stored in `FinalSemanticAnalysis`.

`EffectSemanticDigest` is:

```text
BLAKE3("arcweft.lang.effect-semantic.v1\0" || bytes(EffectId canonical UTF-8))
```

The string is the already parsed canonical `EffectId`, not source/display text.

`RuntimeAgentField::semantic_tag() -> u16` is an exhaustive method in
`arcweft-core`; `ProgressField::semantic_tag() -> u8` is owned in `types.rs`;
`CharacterField::semantic_tag() -> u8` is owned beside `CharacterField`; and
`ViewElementKind::semantic_tag() -> u8` is owned by `arcweft-view`. Tags follow
the current declaration order starting at zero and are explicit match arms,
not discriminant casts. Tests enumerate owner-provided `ALL` rows; adding an
enum variant without a tag is a compile error.

```rust
// arcweft-view::style
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewSpecifiedValueSemanticDigest([u8; 32]);
impl ViewSpecifiedValue {
    pub fn semantic_digest(&self) -> ViewSpecifiedValueSemanticDigest;
}
```

Its domain is `"arcweft.view.specified-value-semantic.v1\0"`. Current outer
tags are explicit and fixed:

| tag | variant | tag | variant |
|---:|---|---:|---|
| 0 | Token | 13 | Position |
| 1 | BoxAxes | 14 | Overflow |
| 2 | Bool | 15 | FlexDirection |
| 3 | Integer | 16 | FlexWrap |
| 4 | Ratio | 17 | Alignment |
| 5 | Scalar | 18 | BorderRadii |
| 6 | Length | 19 | ShadowList |
| 7 | Angle | 20 | FilterList |
| 8 | Color | 21 | Clip |
| 9 | FontFamilyList | 22 | Mask |
| 10 | FontWeight | 23 | BlendMode |
| 11 | FontStyle | 24 | Transition |
| 12 | Display | 25 | Resource |

Payloads use declaration field order. Fixed integers use their exact
little-endian width; Boolean is one byte; strings/PublicIds/token IDs use
length-prefixed canonical UTF-8; lists use checked `u64` count and element
order. Every nested closed enum has explicit zero-based owner match tags.
`PresentationColor` encodes RGBA8 channel order, `SystemColor` its owner tag;
border radii encode top-left, top-right, bottom-right, bottom-left; shadow
encodes x, y, blur, spread, color, inset; transition encodes property tag,
duration, delay. No Serde/debug representation participates.

Existing `canonical_tag` methods are reused for `ViewBoxAxisMode`,
`ViewDisplay`, `ViewPosition`, and `ViewOverflow`. The implementation adds
owner-local exhaustive `semantic_tag` methods for every remaining nested
closed enum used above: `ViewStyleValueKind`, `ViewPropertyKind`,
`ViewSystemFontFamily`, `ViewFlexDirection`, `ViewFlexWrap`, `ViewFontStyle`,
`ViewAlignment`, `SystemColor`, `ViewFilter`, `ViewClip`, `ViewMask`, and
`ViewBlendMode`. Each current variant is assigned its explicit zero-based
source-declaration ordinal; no wildcard or discriminant cast is legal. Named
font families, mask/resource IDs, and tokens encode their validated canonical
UTF-8 identities. List order is semantic for font families, shadows, filters,
and transitions.

## 8. Reserved tags and domain inventory

```rust
pub(crate) const REMOVED_SELECT_TUPLE_ELEMENT_TAG: u16 = 0x0405;
pub(crate) const REMOVED_SELECT_RECORD_ELEMENT_TAG: u16 = 0x0406;
```

The C2 structured registry contains the five accepted-ID domains above plus
Effect and View-specified-value domains. The rejected View-modifier domain is
absent. A repository-wide exact-byte audit at the inspected SHA found no equal
existing domain. The executable registry tests only these C2 domains/tags; it
does not become a global domain authority.
