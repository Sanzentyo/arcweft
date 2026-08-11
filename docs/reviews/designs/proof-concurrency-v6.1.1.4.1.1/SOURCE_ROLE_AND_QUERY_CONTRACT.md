# Typed source-owner, role, storage-key, and query contract

## 1. Owner and storage key

Owned by `arcweft-lang-hir::source_index`:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirSourceQuery {
    Expr { owner: ExprId, role: HirExprSourceRole },
    Pattern { owner: PatternId, role: HirPatternSourceRole },
    Type { owner: TypeId, role: HirTypeSourceRole },
}
```

The committed component table is:

```rust
pub(crate) struct HirSourceIndex {
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}
```

`Whole` for each owner lives in that owner's arena-slot metadata and is returned through the same query. All other present components use the exact `HirSourceQuery` key above. There is no second private key enum, orphan part enum, owner-kind side flag, raw ID, or vector-position key.

Every role enum and part enum below derives `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Their discriminants are internal, not a public wire format.

## 2. Expression roles

The accepted AW-AH-009.4.2 roles and ordinals remain exact. The complete expression-role enum is:

```rust
pub enum HirExprSourceRole {
    Whole,
    Target,
    OpenBracket,
    CloseBracket,
    Colon,
    Content,
    ContentBody,
    Plan,
    ConfigurationArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
    LiteralBody,
    LiteralPrefix,
    LiteralSuffix,
    LiteralUnit,
    PathRoot,
    PathSegment { ordinal: u32 },
    ShortVariantName,
    TypeRegion,
    RegistryScope,
    RegistryKeySegment { ordinal: u32 },
    OptionalMarker,
    PlaceholderMarker,
    Element { ordinal: u32 },
    NumericElement { ordinal: u32 },
    NumericCommonSuffix,
    RepeatValue,
    RepeatLength,
    Callee,
    AssociatedReceiver,
    AssociatedSeparator,
    AssociatedMember,
    CallArgument {
        argument: HirCallArgumentOrdinal,
        part: HirCallArgumentSourcePart,
    },
    SelectedMember,
    Index,
    LeftOperand,
    RightOperand,
    Operand,
    Operator,
    RangeStart,
    RangeEnd,
    RangeInclusiveMarker,
    RecordPath,
    RecordField { field: u32, part: HirRecordFieldSourcePart },
    ClosureParameter { parameter: u32, part: HirClosureParameterSourcePart },
    ReturnType,
    Body,
    Statement { ordinal: u32 },
    Tail,
    Name,
    Condition,
    ThenBranch,
    ElseBranch,
    Pattern,
    Scrutinee,
    Guard,
    MatchArm { arm: u32, part: HirMatchArmSourcePart },
    ThreadModifier,
    ThreadName,
    ThreadBody,
    ThreadFlowItem { ordinal: u32 },
    DialogueNode { ordinal: u32, part: HirDialogueNodeSourcePart },
    RichTextTag { tag: u32, part: HirRichTextTagSourcePart },
    RichTextArgument {
        tag: u32,
        argument: u16,
        part: HirRichTextArgumentSourcePart,
    },
    Recovery,
}

pub enum HirCallArgumentSourcePart { Whole, Name, Value }
pub enum HirRecordFieldSourcePart { Whole, Name, Colon, Value }
pub enum HirClosureParameterSourcePart { Whole, Pattern, Colon, Type }
pub enum HirMatchArmSourcePart { Whole, Pattern, Guard, Arrow, Value }
pub enum HirDialogueNodeSourcePart {
    Whole, Text, Raw, Escape, RubyBase, RubyText, Interpolation,
    Control, Mark, LineBreak, Error,
}
pub enum HirRichTextTagSourcePart {
    Whole, OpenDelimiter, Name, Payload, CloseDelimiter,
    InferenceInsertion, EndTag,
}
pub enum HirRichTextArgumentSourcePart { Whole, Name, Equals, Value }
```

## 3. Pattern roles

```rust
pub enum HirPatternSourceRole {
    Whole,
    Name,
    MutKeyword,
    Literal(HirLiteralSourcePart),
    EntityReference(HirIdRefSourcePart),
    VariantHead(HirVariantPatternHeadSourcePart),
    VariantName,
    VariantPayload(HirVariantPatternPayloadSourcePart),
    Element { ordinal: u32 },
    RecordPathRoot,
    RecordPathSegment { ordinal: u32 },
    PatternField { field: u32, part: HirPatternFieldSourcePart },
    SequenceRest(HirPatternRestSourcePart),
    WholeBindingName,
    WholeBindingAt,
    NestedPattern,
    TypedBindingColon,
    TypedBindingType,
    Recovery,
}

pub enum HirLiteralSourcePart { Body, Prefix, Suffix, Unit }

pub enum HirIdRefSourcePart {
    Whole,
    AbsoluteMarker,
    Family,
    FamilySeparator,
    ParentMarker { ordinal: u32 },
    SuffixSegment { ordinal: u32 },
}

pub enum HirVariantPatternHeadSourcePart {
    QualifiedRoot,
    QualifiedSegment { ordinal: u32 },
    DotShorthandMarker,
}

pub enum HirVariantPatternPayloadSourcePart {
    Whole,
    OpenDelimiter,
    CloseDelimiter,
}

pub enum HirPatternFieldSourcePart {
    Whole,
    Name,
    Colon,
    Pattern,
    RestMarker,
    RestBinding,
}

pub enum HirPatternRestSourcePart { Whole, Marker, Binding }
```

Applicability is exact:

- `Qualified(HirPath)` admits `QualifiedRoot` when a root token is authored and `QualifiedSegment[n]`; it does not admit `DotShorthandMarker`.
- `Unqualified(DotShorthand)` admits `DotShorthandMarker` and `VariantName`; it admits no qualified path role.
- `Unqualified(BareExpectedType)` admits `VariantName`; `DotShorthandMarker` is an absent optional component.
- `VariantPayload` is `AbsentOptional` when no payload is authored. If present, `Whole`, open, and present-or-insertion close roles are queryable.
- Pattern-field ordinals are zero-based source order and are independent of local allocation order.

## 4. Type roles

```rust
pub enum HirTypeSourceRole {
    Whole,
    NeverMarker,
    ConstInteger,
    PathRoot,
    PathSegment { ordinal: u32 },
    TupleOpen,
    TupleElement { ordinal: u32 },
    TupleSeparator { ordinal: u32 },
    TupleClose,
    FunctionOpen,
    FunctionParameter { ordinal: u32 },
    FunctionSeparator { ordinal: u32 },
    FunctionClose,
    FunctionArrow,
    FunctionReturn,
    FunctionEffectOpen,
    FunctionEffect { ordinal: u32 },
    FunctionEffectClose,
    ChoiceAlternative { ordinal: u32 },
    ChoiceSeparator { ordinal: u32 },
    GenericBase,
    GenericOpen,
    GenericArgument { ordinal: u32 },
    GenericSeparator { ordinal: u32 },
    GenericClose,
    TraitBase,
    TraitOpen,
    TraitArgument { ordinal: u32 },
    TraitSeparator { ordinal: u32 },
    AssociatedBinding {
        ordinal: u32,
        part: HirAssociatedTypeBindingSourcePart,
    },
    TraitClose,
    ProjectionSubject,
    ProjectionSeparator,
    ProjectionName,
    ReferenceAmpersand,
    Region(HirTypeRegionSourcePart),
    ReferenceMutKeyword,
    ReferenceReferent,
    SliceOpen,
    SliceElement,
    SliceClose,
    Recovery,
}

pub enum HirAssociatedTypeBindingSourcePart { Whole, Name, Equals, Value }

pub enum HirTypeRegionSourcePart {
    Whole,
    NamedApostrophe,
    NamedName,
    ElisionInsertion,
}
```

For a named region, `Whole`, `NamedApostrophe`, and `NamedName` are present; `ElisionInsertion` is inapplicable. For an elided region, `ElisionInsertion` is a present `HirSourceSite::Insertion`; all named parts are inapplicable. The insertion offset is copied from `RegionSyntax::Elided::anchor` and is not recomputed from text.

## 5. Public lookup API

```rust
pub enum HirSourcePresence<'a> {
    Present(&'a HirSourceSite),
    AbsentOptional,
}

pub enum HirSourceOwnerStatus {
    Clean,
    Poisoned,
}

pub struct HirSourceLookup<'a> {
    presence: HirSourcePresence<'a>,
    owner_status: HirSourceOwnerStatus,
}

pub enum HirSourceQueryError {
    ExprResolve { owner: ExprId, error: IdResolveError },
    PatternResolve { owner: PatternId, error: IdResolveError },
    TypeResolve { owner: TypeId, error: IdResolveError },
    ExprRoleNotApplicable { owner: ExprId, role: HirExprSourceRole },
    PatternRoleNotApplicable { owner: PatternId, role: HirPatternSourceRole },
    TypeRoleNotApplicable { owner: TypeId, role: HirTypeSourceRole },
    ExprOrdinalOutOfBounds { owner: ExprId, role: HirExprSourceRole, length: u32 },
    PatternOrdinalOutOfBounds { owner: PatternId, role: HirPatternSourceRole, length: u32 },
    TypeOrdinalOutOfBounds { owner: TypeId, role: HirTypeSourceRole, length: u32 },
    WrongSourceDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    StaleSourceRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
    SourceLengthMismatch { expected: u64, actual: u64 },
}

impl HirModule {
    pub fn source_site<'a>(
        &'a self,
        expected_source: &SourceDocumentIdentity,
        query: HirSourceQuery,
    ) -> Result<HirSourceLookup<'a>, HirSourceQueryError>;
}
```

Fields are private. Constructors for query variants are the enum variants themselves; roles carry typed ordinals. Lookup records expose `presence()` and `owner_status()` only. Error records retain typed owner IDs and typed roles. No source query type implements Serde.

The lookup validation order is normative and makes combined failures deterministic:

1. resolve the typed owner in the queried arena and snapshot (foreign module, not-yet-live, retired, and kind mismatch end the query);
2. validate role applicability and its ordinal against the resolved payload;
3. compare `expected_source` to the module's exact source document ID, revision, and retained byte length, in that order;
4. return the committed site/presence and owner poison status.

A rolled-back allocation never reaches step 1 because no public owner ID is returned. The implementation must not reorder these checks per caller.

## 6. Deterministic outcome table

| State | Exact result |
|---|---|
| clean source-backed | `Ok(HirSourceLookup { Present(Span), Clean })` |
| poisoned known family | `Ok(HirSourceLookup { Present(Span|Insertion), Poisoned })` |
| absent optional | `Ok(HirSourceLookup { AbsentOptional, Clean|Poisoned })` |
| synthetic/elided | `Ok(HirSourceLookup { Present(Insertion), Clean|Poisoned })` and the child carries the typed `SyntheticKey` |
| inapplicable role | typed owner-specific `*RoleNotApplicable` error |
| ordinal one-over | typed owner-specific `*OrdinalOutOfBounds` error; no vector fallback |
| foreign owner module | owner-specific `*Resolve { error: IdResolveError::WrongModule }` |
| not-yet-live/retired ID | owner-specific `*Resolve` with the exact liveness error |
| wrong logical source document | `WrongSourceDocument` |
| same document, old revision | `StaleSourceRevision` |
| same ID/revision, different retained length | `SourceLengthMismatch` |
| rolled back | no public owner ID; rollback receipt is `NotPublished`; no committed key exists |

A committed module cannot lack a required component. Transaction validation compares the payload's required-role set with staged entries before publication; a mismatch is an internal `HirCommitInvariantError`, aborts the transaction, and is never converted to `AbsentOptional`.

## 7. Ordinals

All vector ordinals are zero-based and contiguous. Ordinary call arguments use the accepted bounded `HirCallArgumentOrdinal`. RichText argument ordinals are `u16 < 32`. Expression/pattern/type component ordinals use `u32`; lowering rejects a `usize` that does not fit before staging. Candidate-only AW-AH-009.4.2 roles and ordinal zero are unchanged.

## 8. Supersession

`HirModule::expr_source_site` is deleted in the same compiling public switch that publishes `source_site`. No forwarding method, extension trait, deprecation alias, or dual map is permitted. All HIR, sema, verifier, runtime-plan, compiler, LSP, formatter, Agent/debug, cache, and project-publication consumers use `source_site` directly.
