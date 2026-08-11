# Source-role, ordinal, and query contract

## Preserved accepted roles

These AW-AH-009.4.2 variants are preserved exactly and keep their existing ordinal meaning:

```rust
Whole, Target, OpenBracket, CloseBracket, Colon, Content, ContentBody, Plan,
ConfigurationArgument { argument: HirCallArgumentOrdinal, part: HirCallArgumentSourcePart }
```

`HirCallArgumentSourcePart` remains `Whole | Name | Value`. `SyntheticRole::PostfixIndexCandidateExpression` and `SyntheticRole::DialogueContentCandidateExpression` are preserved: ordinal zero is the interpretation root; nested candidate-only children use deterministic zero-based preorder per `HirIdKind`; no candidate-only key becomes the selected committed expression key.

## Exact extensions

```rust
pub enum HirExprSourceRole {
    Whole, Target, OpenBracket, CloseBracket, Colon, Content, ContentBody, Plan,
    ConfigurationArgument { argument: HirCallArgumentOrdinal, part: HirCallArgumentSourcePart },
    LiteralBody, LiteralPrefix, LiteralSuffix, LiteralUnit,
    PathRoot, PathSegment { ordinal: u32 }, ShortVariantName,
    TypeRegion, RegistryScope, RegistryKeySegment { ordinal: u32 }, OptionalMarker,
    PlaceholderMarker,
    Element { ordinal: u32 }, NumericElement { ordinal: u32 }, NumericCommonSuffix,
    RepeatValue, RepeatLength,
    Callee, AssociatedReceiver, AssociatedSeparator, AssociatedMember,
    CallArgument { argument: HirCallArgumentOrdinal, part: HirCallArgumentSourcePart },
    SelectedMember, Index,
    LeftOperand, RightOperand, Operand, Operator,
    RangeStart, RangeEnd, RangeInclusiveMarker,
    RecordPath, RecordField { field: u32, part: HirRecordFieldSourcePart },
    ClosureParameter { parameter: u32, part: HirClosureParameterSourcePart },
    ReturnType, Body, Statement { ordinal: u32 }, Tail, Name,
    Condition, ThenBranch, ElseBranch, Pattern, Scrutinee, Guard,
    MatchArm { arm: u32, part: HirMatchArmSourcePart },
    ThreadModifier, ThreadName, ThreadBody, ThreadFlowItem { ordinal: u32 },
    DialogueNode { ordinal: u32, part: HirDialogueNodeSourcePart },
    RichTextTag { tag: u32, part: HirRichTextTagSourcePart },
    RichTextArgument { tag: u32, argument: u16, part: HirRichTextArgumentSourcePart },
    Recovery,
}

pub enum HirRecordFieldSourcePart { Whole, Name, Colon, Value }
pub enum HirClosureParameterSourcePart { Whole, Pattern, Colon, Type }
pub enum HirMatchArmSourcePart { Whole, Pattern, Guard, Arrow, Value }
pub enum HirPatternFieldSourcePart { Whole, Name, Colon, Pattern, Rest }
pub enum HirDialogueNodeSourcePart {
    Whole, Text, Raw, Escape, RubyBase, RubyText, Interpolation,
    Control, Mark, LineBreak, Error,
}
pub enum HirRichTextTagSourcePart {
    Whole, OpenDelimiter, Name, Payload, CloseDelimiter, InferenceInsertion, EndTag,
}
pub enum HirRichTextArgumentSourcePart { Whole, Name, Equals, Value }
```

All vector-backed ordinals are zero-based and contiguous. Call argument ordinals are `u16` and below their context limit. Dialogue node/tag ordinals are `u32`; RichText argument ordinals are `u16` and below 32. A role/ordinal pair is unique under its parent.

## Query API and exact outcomes

The accepted module query remains unchanged:

```rust
impl HirModule {
    pub fn expr_source_site(
        &self,
        id: ExprId,
        role: &HirExprSourceRole,
    ) -> Result<Option<&HirSourceSite>, IdResolveError>;
}
```

`Whole` is supplied by expression-slot metadata rather than duplicated in the component map. The query result is interpreted together with the immutable expression slot and the accepted project/source snapshot:

| state | exact observable result |
|---|---|
| clean source-backed | `Ok(Some(HirSourceSite::Span(_)))` and `HirPoisonState::Clean` |
| poisoned known family | `Ok(Some(Span(_)))` or `Ok(Some(Insertion(_)))` and `HirPoisonState::Poisoned(_)` |
| synthetic/elided/implicit | `Ok(Some(HirSourceSite::Insertion(_)))`; the typed synthetic role/ordinal is retained by the allocated child key |
| absent optional component | `Ok(None)` |
| stale project/source snapshot | project-level lookup rejects before module query with the accepted stale-generation/source-revision error; no range is projected |
| foreign module ID | `Err(IdResolveError::WrongModule { .. })`; no range is projected |
| rolled-back allocation | the transaction returns no public `ExprId`; its rollback receipt records `NotPublished`, and the committed module has neither slot nor component row |

A generic `Error` expression may expose only `Whole` and `Recovery`. There is no parallel query enum, wrapper reader, or vector-position fallback.

`HirInsertionPoint::try_new` preserves the accepted `SourceDocumentIdentity`, verifies the accepted source revision, checks `offset <= document.len()`, and requires a UTF-8 boundary. It has no Serde implementation.

## Tail and recovery ordinals

- `ImplicitUnitTail`: owner is the block-like expression, ordinal zero.
- `MissingRequiredTail`: owner is the requiring expression, ordinal zero, poisoned.
- `RecoveryOperand`: owner is the requiring expression, ordinal equals the missing operand's declared child ordinal (zero for unary/single; zero/one for binary sides).
- `ElidedRegion`: owner is TypeId, ordinal zero.
- Candidate roles: owner is the postfix parent. Ordinal zero is the candidate root; nested candidate-only Expr/Stmt/Pattern IDs use deterministic zero-based preorder per `HirIdKind` within that interpretation. The shared target is excluded.

These roles are not aliases and are not re-numbered by this correction.
