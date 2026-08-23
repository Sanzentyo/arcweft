# Exact schemas

All shown fields and visibility are normative. Implementations may split a
module for cohesion but may not change ownership or add a second model.

## 1. Final-analysis draft and seal

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

pub(crate) struct FinalSemanticAnalysisDraft {
    // collected existing maps, callables, type reports and work
    expressions: BTreeMap<ExprId, PreparedExpressionFact>,
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
    nominal: &'draft mut RuntimeNominalProjectionContext<'draft>,
}
```

After Entry checking, `RuntimeNominalProjectionContext::finish` consumes the
context and returns its catalog. `FinalSemanticAnalysisDraft::seal(self,
CheckedEntryCatalog, RuntimeNominalProjectionCatalog, control)` is crate-private
and consuming. It checks that `Entry` variants equal the exact Entry-reference
expression inventory before creating the final expression map.

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
}
```

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

```rust
pub(crate) struct RuntimeNominalProjectionContext<'a> {
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    limits: NominalResolutionLimits,
    work: u64,
    visiting: BTreeSet<SemanticTypeDigest>,
    accepted: BTreeMap<SemanticTypeDigest, RuntimeProjectNominalProjection>,
    control: FinalSemanticAnalysisControl<'a>,
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
    ) -> Option<&RuntimeProjectNominalProjection>;
}
```

`RuntimeProjectNominalProjection` remains the existing schema. Its record
fields use `RuntimeRecordFieldId::try_from_zero_based_ordinal`; its variants use
checked source `u32` ordinals. The existing public
`FinalSemanticAnalysis::project_checked_runtime_nominal` becomes a sealed
catalog lookup returning a borrowed projection. `project_runtime_type_schema`
remains the sole pure `TypeShape -> RuntimeTypeSchema` projection.

## 3. Ordered environment records

```rust
// arcweft-lang-sema::env
pub struct AcceptedEnvironmentRecord {
    name: String,
    semantic_type: SemanticTypeDigest,
    fields: Box<[AcceptedEnvironmentRecordField]>,
    field_lookup: HashMap<String, u32>, // derived, nonsemantic
}

pub struct AcceptedEnvironmentRecordField {
    diagnostic_name: String,
    ordinal: u32,
    ty: TypeKind,
    type_digest: SemanticTypeDigest,
    semantic_id: AcceptedEnvironmentFieldSemanticId,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedEnvironmentFieldSemanticId([u8; 32]);

impl AcceptedEnvironmentRecord {
    pub fn fields(&self) -> &[AcceptedEnvironmentRecordField];
    pub fn field(&self, name: &str) -> Option<&AcceptedEnvironmentRecordField>;
    pub const fn semantic_type(&self) -> SemanticTypeDigest;
}
```

`TypeCheckEnv::nominal_records()` returns
`&HashMap<String, AcceptedEnvironmentRecord>`. The old nested map is deleted in
the same compile-clean cut.

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
absent. The sema-private stable pattern coordinate uses the shared semantic
field identity rather than `RuntimeRecordFieldId`, because environment rows do
not fabricate project nominal identity. Project rows continue to retain their
accepted runtime field separately for compiler/runtime lowering. Existing C1
`HirPatternChildRole` values remain unchanged; the checker joins their source
field ordinal to the C2 row before constructing this sema-private coordinate.

## 7. Call joins, Effect, StageLook, View, and Style

```rust
// callable/join.rs
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableJoinDigest([u8; 32]);
impl CheckedCallableJoin {
    pub fn semantic_digest(&self) -> CheckedCallableJoinDigest;
}

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
