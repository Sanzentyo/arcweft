# Normative schemas

## Boundary and vocabulary

These are Rust-shaped implementation requirements, not a public serialization
format. All new identities and digests are opaque Arcweft-owned `[u8; 32]`
newtypes with domain version `1`. None derives `Serialize` or `Deserialize`.
`CheckedMatchRef` remains a private lookup handle; raw HIR IDs in that handle
never enter a semantic digest, diagnostic identity, wire payload, cache key, or
save data.

The following words are normative:

- **existing owner**: a checked identity already selected by the current
  accepted project, callable, entry, registration, dialogue, type, effect, or
  nominal authority;
- **same-cut owner**: a purpose-built opaque semantic identity produced while
  the checked fact is constructed, before transcript construction;
- **coordinate**: an accepted-rooted semantic path. Its canonical prefix is
  `0x00 || accepted_declaration_semantic_id` or
  `0x01 || accepted_item_semantic_id`; the remainder is composed only from
  closed role tags and checked ordinals;
- **source ordinal**: order in an accepted typed row; it is not an arena ID;
- **diagnostic spelling**: a name retained only for lookup or rendering and
  explicitly excluded from transcript authority.

There is one final checked fact graph. No transcript builder may consult HIR
names or recreate an owner that the checker failed to retain.

## Core Match result

```rust
pub(crate) struct CheckedMatch {
    lookup: CheckedMatchRef,             // private, non-semantic
    root: AcceptedSemanticRoot,          // Declaration for callable, Item for item-owned Match
    callable_owner: Option<AcceptedDeclarationId>, // existing join for declaration roots
    path: StableExpressionCoordinate,    // accepted-rooted
    scrutinee: CheckedExpressionSemanticDigest,
    scrutinee_type: SemanticTypeDigest,
    arms: Box<[CheckedMatchArm]>,
    transcript: MatchSemanticTranscript,
    coverage: CheckedMatchCoverage,
}

pub(crate) struct CheckedMatchArm {
    coordinate: StableMatchArmCoordinate,
    pattern: CheckedPatternSemanticDigest,
    guard: CheckedGuardSemantic,
    result: CheckedExpressionSemanticDigest,
}

pub(crate) enum CheckedGuardSemantic {
    Absent,
    ConstantTrue,
    ConstantFalse,
    Dynamic(CheckedExpressionSemanticDigest),
}

pub(crate) struct MatchSemanticTranscript {
    version: ContractVersionOne,
    digest: MatchSemanticDigest,
    byte_len: u64,
}
```

The transcript object may retain its canonical bytes only for tests or
diagnostics. `digest` is the final equality/key authority. Construction is
atomic: any unresolved owner, poison family, coverage error, or limit error
means there is no `CheckedMatch` row.

## Exact owner atoms introduced in the same cut

```rust
pub(crate) struct AcceptedProjectItemSemanticId([u8; 32]);
pub(crate) struct AcceptedVariantCaseSemanticId([u8; 32]);
pub(crate) struct AcceptedRecordFieldSemanticId([u8; 32]);
pub(crate) struct AcceptedEnvironmentFieldSemanticId([u8; 32]);
pub(crate) struct AcceptedCharacterLookSemanticId([u8; 32]);
pub(crate) struct AcceptedViewModifierSemanticId([u8; 32]);
pub(crate) struct CheckedStatementSemanticDigest([u8; 32]);
pub(crate) struct CheckedRichTextSemanticDigest([u8; 32]);
pub(crate) struct CheckedBodySemanticDigest([u8; 32]);
```

Each identity is created by the owning checker from already accepted typed
inputs. The digest domain is version `1`; length prefixes and ordinals use the
canonical grammar in `TRANSCRIPT_GRAMMAR.md`. Source names may be used to find
the row, then are discarded from the semantic payload.

### Project item and entry rows

```rust
pub(crate) struct CheckedProjectItem {
    semantic_id: AcceptedProjectItemSemanticId,
    family: CheckedProjectItemFamily,
    value_type: SemanticTypeDigest,
    owner: CheckedProjectItemOwner, // raw IDs inside are lookup-only
    diagnostic_public_id: PublicId,
}

pub(crate) struct CheckedEntryReference {
    binding: CheckedEntryBindingDigest,
    value_type: SemanticTypeDigest,
    diagnostic_public_id: PublicId,
    lookup_owner: ItemId,
}
```

For a retained/external project item, `semantic_id` is derived from its
accepted public/entity identity, family tag, and value type. For a Flow it is
derived from the accepted `CallableDeclarationDigest`, Flow family tag, and
value type. `ItemId` and public spelling do not participate. Entry resolution
must retain the current entry catalog's `CheckedEntryBindingDigest`; a raw item
owner is insufficient.

### Variant owner and case rows

```rust
pub(crate) enum CheckedVariantOwner {
    Project {
        nominal: CheckedProjectNominal,
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
    Option {
        item: TypeKind,
        cases: [CheckedVariantCase; 2],
    },
    Result {
        ok: TypeKind,
        error: TypeKind,
        cases: [CheckedVariantCase; 2],
    },
}

pub(crate) struct CheckedVariantCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    payload: Option<TypeKind>,
    diagnostic_name: Option<HirName>,
}

pub(crate) struct CheckedVariantResolution {
    owner: CheckedVariantOwner,
    selected: CheckedVariantCase,
}
```

The selected case must equal the unique row at `ordinal` in `owner.cases()`.
Character and builtin case names cease to be checked authority. Case identity
is derived from owner semantic type identity, source ordinal, payload-presence
tag, and payload `SemanticTypeDigest`. Project case identity additionally binds
the existing canonical `TypeLayoutHash`. Option and Result use language-owned
case tags and the semantic digests of their type arguments.

For `TypeKind::Choice`, coverage constructs private source-order
`ChoiceAlternativeId { choice_type_digest, ordinal, alternative_type_digest }`
rows. These are coverage constructors, not persisted variant cases and not a
new public nominal catalog.

### Record-pattern field rows

```rust
pub(crate) struct CheckedRecordPattern {
    nominal: CheckedRecordPatternOwner,
    fields: Box<[CheckedRecordPatternField]>, // authored source order
    has_rest: bool,
}

pub(crate) enum CheckedRecordPatternOwner {
    Project {
        nominal: CheckedProjectNominal,
        semantic_type: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    Environment {
        semantic_type: SemanticTypeDigest,
        fields_in_declaration_order: Box<[CheckedEnvironmentRecordField]>,
    },
}

pub(crate) struct CheckedRecordPatternField {
    source_ordinal: u32,
    declaration_ordinal: u32,
    runtime_field: Option<RuntimeRecordFieldId>,
    semantic_id: CheckedRecordFieldSemanticId,
    field_type: TypeKind,
    field_type_digest: SemanticTypeDigest,
    pattern: StablePatternCoordinate,
}

pub(crate) enum CheckedRecordFieldSemanticId {
    Project(AcceptedRecordFieldSemanticId),
    Environment(AcceptedEnvironmentFieldSemanticId),
}
```

Project field identity binds the existing project nominal semantic type,
canonical layout hash, `RuntimeRecordFieldId`, declaration ordinal, and field
type digest. Environment record identity binds the accepted environment record
semantic type, declaration ordinal, and field type digest. The authored name
is lookup/diagnostic data only. Rest syntax is one Boolean shape atom; it never
fabricates a field row. Duplicate, unknown, or mismatched field rows reject the
pattern before transcript or coverage.

### Typed binding fact

`CheckedPattern` gains a same-cut semantic payload for `TypedBinding`:

```rust
pub(crate) struct CheckedTypedBinding {
    annotation: TypeKind,
    annotation_digest: SemanticTypeDigest,
}
```

This is required for Choice-domain specialization. It is not reconstructed
from a `TypeId` after checking.

## Exact replacements for current unresolved resolution payloads

The following checked payloads replace name/raw-ID-only authority:

```rust
pub(crate) struct CheckedMethodSelection {
    callable: CheckedCallableJoinDigest,
    receiver_type: SemanticTypeDigest,
    receiver_mode: CheckedReceiverMode,
}

pub(crate) struct CheckedFieldSelection {
    owner_type: SemanticTypeDigest,
    field: CheckedFieldSemanticId,
    declaration_ordinal: u32,
    field_type: SemanticTypeDigest,
}

pub(crate) struct CheckedStageLook {
    character_nominal: SemanticTypeDigest,
    character: CharacterId,
    look: AcceptedCharacterLookSemanticId,
}

pub(crate) struct CheckedViewModifier {
    owner: CallableDeclarationDigest,
    semantic_id: AcceptedViewModifierSemanticId,
}
```

- `Method` stores `CheckedMethodSelection`; its current `HirName` is deleted.
- `DialogueView` stores its existing `DialogueProjectionCoordinate` and owner
  type digest; its current name is diagnostic-only.
- `AgentField` and `ProgressField` receive exhaustive owner-defined semantic
  tags on their existing enums.
- `Field` stores a project or environment `CheckedFieldSemanticId`; its current
  name is diagnostic-only. `RecordElement`, which has no producer, is deleted.
- `TupleElement` stores its checked ordinal and result type digest.
- `StageLook` stores `CheckedStageLook`, resolved through the existing
  registered character/look authority. Open-name fallback is deleted.
- `Effect` encodes the existing canonical `EffectId` through an owner-defined
  `EffectSemanticDigest`; the transcript never parses its display spelling.
- `ViewCall` modifier positions store `CheckedViewModifier`. The existing
  closed View callee and View element families receive owner-defined tags.
- `StyleValue` is represented by an exhaustive owner-defined
  `ViewSpecifiedValueSemanticDigest` over all 27 current variants. It is not a
  Serde encoding and is not debug text.
- `DialogueApplication` stores a purpose-built
  `CheckedRichTextSemanticDigest`. It covers semantic text fragments, Ruby,
  control/tag/action/field/default identities, and stable expression-child
  coordinates; it excludes spans and raw HIR IDs. Poisoned rich text rejects.

`PostfixBracket` stores a closed selection tag and the semantic digest of the
selected candidate only. The rejected physical candidate and all raw candidate
`ExprId`s are excluded.

## Declaration/body path bridge

The HIR owner, not sema, adds these closed path forms:

```rust
pub enum HirDeclarationBodyRootRole {
    FunctionBody,
    PredicateBody,
    ProofBody,
    FlowBody,
    ImplFunctionBody,
    ViewValue { ordinal: u32 },
}

pub enum HirSemanticPathStep {
    DeclarationBody(HirDeclarationBodyRootRole),
    ExpressionOwned(HirExpressionOwnedBodyRole),
    // Existing Body, Statement, ThreadBody, Expression, MatchPattern,
    // Pattern, ParameterPattern, and ParameterDefault variants remain.
}

pub enum HirExpressionOwnedBodyRole {
    AwaitBranchPattern { branch: u32 },
    AwaitBranchBody { branch: u32 },
    ChoiceLetStatement { path: HirNestedExpressionPath },
    ChoiceForPattern { path: HirNestedExpressionPath },
    ChoiceMatchArmPattern { path: HirNestedExpressionPath, arm: u32 },
    ChoiceOptionForPattern { path: HirNestedExpressionPath },
    ChoiceOptionSelectBody { path: HirNestedExpressionPath, field: u32 },
    ChoiceOptionLetStatement { path: HirNestedExpressionPath, field: u32 },
    ChoicePlanTimeoutBody { path: HirNestedExpressionPath },
    ChoicePlanCancelBody { path: HirNestedExpressionPath },
    ChoicePlanOnSelectPattern { path: HirNestedExpressionPath },
    ChoicePlanOnSelectBody { path: HirNestedExpressionPath },
    DialogueLinePlanStatement {
        path: HirNestedExpressionPath,
        role: HirLinePlanStatementRole,
    },
    DialogueLinePlanLetPattern { path: HirNestedExpressionPath },
}

pub enum HirLinePlanStatementRole {
    Init { statement: u32 },
    Thread,
    On,
    Statement,
    CancelRule,
    Error,
}

pub enum HirExpressionOwnedChild {
    Pattern(PatternId),
    Statement(StmtId),
    Body(HirBodyChildEdge),
}

pub struct HirExpressionOwnedChildEdge {
    child: HirExpressionOwnedChild,
    role: HirExpressionOwnedBodyRole,
}
```

This is a schema correction to the accepted C1 intent. The former raw
`group_path`/`item` sketches are not an alternative coordinate
format and must not be implemented. `HirNestedExpressionPath` is the one
Choice/line-plan nesting authority. It is nonempty and retains
`StartGroup`/`TogetherGroup` segment kinds. Edge constructors are HIR-private;
callers cannot pair an arbitrary role with an unrelated child or path.

There are 14 `HirExpressionOwnedBodyRole` variants but 19 logical root
families. The last two variants represent seven dialogue families:
`DialogueLinePlanLetPattern` plus the six closed
`HirLinePlanStatementRole` cases. `Init { statement }` selects one statement
inside the source-ordered Init row. The logical-family count is neither a Rust
variant count nor a produced-edge count.

`ChoicePlanCancelBody` may produce the accepted trigger Pattern, when the
trigger owns one, followed by the heterogeneous body children at the same
Choice-plan path. The child kind and the following Pattern/Body path step keep
those coordinates distinct; this does not introduce a fifteenth role or a
twentieth logical family.

Every role/path pair obeys these invariants:

- a Choice role carries a nonempty path whose terminal segment agrees with
  its item, arm, option field, or plan-item owner;
- duplicated `arm` and `field` payloads equal their corresponding typed path
  segments;
- a dialogue path begins with `LinePlanItem` and preserves every nested
  Start/Together group-kind segment;
- an expression-owned Body child appends its existing `HirBodyChildRole`
  after `ExpressionOwned(role)`; and
- the same live HIR child reached through two complete paths is rejected as a
  duplicate rather than deduplicated.

All ordinals use checked conversion under the same `u64` admission budget.
`ViewValue { ordinal }` is joined to
`CallableDeclarationKey::Existing(CallableDeclarationId { owner: View, .. })`.
The baseline types, View source owner, source-index roles, validation matrix,
and downstream project-callable kind already admit that row, but the baseline
linker does not publish it. C1 completes the existing nonbinding callable
publication and registered/checked callable pipeline in place. The retained
View public-ID symbol remains the sole source-name binding; callable
publication must not add a second binding for that name. The retained and
callable rows join through the same `ItemId`, module, and snapshot.

`ViewItem` then ceases to return `MissingBody`; extern capability and trait
requirement still do. No `ViewMatchSiteId`, retained-symbol fallback,
synthetic key, parallel declaration index, copied callable catalog, or module
scan is permitted.

Statement and body semantic digests are private memoized projections over
existing checked facts and typed HIR child roles. A minimal
`CheckedStatementSemanticPayload` may retain only semantic facts absent from
children—for example accepted Include target, resolved control target,
accepted output label, or source-locale identity. It must not become a second
statement AST.

## Coverage result

```rust
pub(crate) struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable: Box<[CheckedUnreachablePattern]>,
    witness: Option<CheckedCoverageWitness>,
    stats: CheckedCoverageStats,
}

pub(crate) struct CheckedUnreachablePattern {
    arm: StableMatchArmCoordinate,
    alternative: Option<StablePatternCoordinate>,
    reason: CheckedUnreachableReason,
}

pub(crate) enum CheckedUnreachableReason {
    CoveredByPriorUsefulArms,
    CoveredByEarlierOrAlternative,
    ConstantFalseGuard,
    UninhabitedDomain,
}

pub(crate) enum CheckedCoverageWitness {
    Unit,
    Bool(bool),
    Literal(CanonicalLiteral),
    Entity(AcceptedProjectItemSemanticId),
    Other { type_digest: SemanticTypeDigest },
    Variant {
        case: AcceptedVariantCaseSemanticId,
        payload: Option<Box<CheckedCoverageWitness>>,
    },
    Tuple(Box<[CheckedCoverageWitness]>),
    Record {
        owner: SemanticTypeDigest,
        fields: Box<[CheckedCoverageWitness]>,
    },
    Array(Box<[CheckedCoverageWitness]>),
    Sequence {
        partition: CheckedSequencePartitionWitness,
        visible_prefix: Box<[CheckedCoverageWitness]>,
    },
    Choice {
        ordinal: u32,
        alternative: SemanticTypeDigest,
        value: Box<CheckedCoverageWitness>,
    },
}
```

Witnesses are structured and deterministic; they are never reconstructed as
Arcweft source text. An exhaustive Match has no witness. A non-exhaustive
Match has exactly one smallest witness selected by the ordering in
`COVERAGE_ALGORITHM.md`.

## Limits and errors

```rust
pub struct CheckedMatchLimits {
    pub max_arms: u64,
    pub max_matrix_rows: u64,
    pub max_or_alternatives: u64,
    pub max_pattern_nodes: u64,
    pub max_expression_nodes: u64,
    pub max_depth: u64,
    pub max_sequence_partitions: u64,
    pub max_specializations: u64,
    pub max_unreachable_rows: u64,
    pub max_witness_nodes: u64,
    pub max_transcript_bytes: u64,
}

pub(crate) enum CheckedMatchBuildError {
    MissingExactOwner { coordinate: StableSemanticCoordinate },
    PoisonedSemanticNode { coordinate: StableSemanticCoordinate },
    DuplicateSemanticPath { coordinate: StableSemanticCoordinate },
    InvalidCheckedRow { coordinate: StableSemanticCoordinate },
    UnsupportedDomain { type_digest: SemanticTypeDigest },
    LimitExceeded { kind: CheckedMatchLimitKind, limit: u64, attempted: u64 },
    ArithmeticOverflow { kind: CheckedMatchLimitKind },
}
```

Every counter is `u64`. Every increment/conversion uses `checked_add` or
`try_from` before allocation, recursion, specialization, byte write, or result
append. Equality with the limit succeeds; the first one-over attempt fails.
Failure is atomic and deterministic. Saturation, truncation, `expect`, and
partial `CheckedMatch` publication are forbidden.

## Explicitly excluded shapes

- external return package types, status tags, wire or persistence schemas;
- raw `ExprId`, `PatternId`, `StmtId`, `ItemId`, `LocalId`, `ScopeId`, snapshot
  ID, span, source offset, diagnostic ordering, or source spelling in a digest;
- persisted generic-Match products or public Match ABI;
- a task-plan seal (owned by the later runtime-plan cut);
- a whole project/callable/nominal/registration catalog digest;
- compatibility readers, `Legacy` variants, `V2` types, or any version marker
  other than `1`.
