# Historical parent material superseded by this correction

**NON-NORMATIVE.** This file copies the directly adjudicated parent schemas/rows required by the follow-up request. Implementers use the complete corrected normative files in this archive; no manual archive comparison is required.

Parent ZIP: `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`.

## Parent source-query contract (complete)

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


## Parent literal/numeric contract (complete)

# Literal and numeric contract

## 1. Direct typed lowering

The only input is the canonical attached typed literal owner produced by syntax. Lowering consumes decoded characters/strings, typed radix/suffix/unit enums, canonical digit tokens, and typed recovery issues. It must not call `raw_text`, slice the source document, scan Rowan tokens, split a display label, or run another lexer.

## 2. String and character

Escaped and raw string syntax both lower to `HirStringLiteral::Value(decoded_utf8)`. Delimiter spelling, quote count, and escape spelling are source components only. Invalid escape and unterminated forms lower to the String family in poisoned state; they do not fabricate an empty value. Character syntax must decode to exactly one Unicode scalar. Empty, multiple-scalar, invalid-escape, or unterminated forms remain poisoned Character literals.

## 3. Integer

The literal magnitude is non-negative. A leading minus is always `HirUnaryOp::Negate` over the literal. Syntax separators and prefixes are validated before HIR; the HIR magnitude is `HirBigUint` in little-endian base-2^32 limbs. This representation retains `u128::MAX + 1` and arbitrary larger values exactly up to the resource digit limit.

The checker selects a type in this order: explicit suffix; exact contextual integer type; otherwise i32. It compares the arbitrary magnitude to the selected range. Unary minus is checked as a combined operation and admits exactly one extra positive magnitude for the selected signed minimum. An out-of-range value is a typed checker error and no runtime constant is published.

Structural HIR equality includes magnitude, radix, and explicit suffix. `integer_value_eq` compares magnitude plus selected type and ignores radix.

## 4. Canonical decimal

Parse coefficient digits, decimal point, and exponent from typed syntax components. Reject missing/invalid components as typed family recovery. Then:

1. remove coefficient leading zeros;
2. if all digits are zero, canonicalize to digits `[0]`, scale 0, exponent 0;
3. set scale to the count of digits originally following the decimal point;
4. remove trailing coefficient zeros while scale is positive, decrementing scale;
5. remove any remaining trailing coefficient zeros and add their count to exponent10;
6. add the authored exponent to exponent10 using checked arithmetic;
7. enforce coefficient <= 65,536 digits, scale <= 65,536, and `abs(exponent10) <= 1,000,000`.

The value is `coefficient × 10^(exponent10 - scale)`. These rules make `100`, `1e2`, and `1.00e2` canonical to the same decimal value record.

## 5. Float

Width selection is deterministic: explicit `f32`/`f64` suffix; otherwise an exact contextual f32/f64 expectation; otherwise f64. The checker converts the canonical decimal to IEEE-754 round-to-nearest, ties-to-even and records exact `to_bits()` output in `CheckedFloatLiteral`. Finite subnormal values and signed zero are accepted. A literal that rounds to infinity is rejected as `WidthOverflow`. NaN and infinity are not literal spellings; standard constants are resolved as paths. Unary minus is applied to the checked bits, preserving negative zero.

## 6. Unit number

The canonical decimal is paired with exactly one unit enum. Percent is Ratio; px/pt/em/rem/vw/vh are Length; deg/rad/turn are Angle; db/lufs are Audio; bpm/bars are Music. Unit spelling aliases are normalized by typed syntax. No unit-number payload is later re-parsed or guessed from a suffix string.

## 7. Duration

Duration is not a UnitNumber. Its HIR type identity is always `Duration`. The source amount is a non-negative canonical decimal paired with ns/us/ms/s/min/h. Lowering multiplies the exact decimal by the unit's nanosecond factor (1, 1,000, 1,000,000, 1,000,000,000, 60,000,000,000, or 3,600,000,000,000). A valid HIR Duration contains an arbitrary-precision whole-nanosecond magnitude. A fractional nanosecond is poisoned `FractionalNanosecond`; no truncation or rounding occurs. The checker requires the magnitude <= `u64::MAX`, matching `arcweft_core::time::LogicalDuration`; one-over is `RuntimeRangeOverflow`. Equality and ordering compare whole nanoseconds; authored unit is retained only for typed diagnostics/fingerprint and does not change value equality.

## 8. Compact numeric sequence

One `HirNumericSequence` owns one expression identity and ordered ID-less elements. Every element has an arbitrary-precision magnitude and radix. All explicit suffixes must agree; a common suffix is stored once. An absent final element after a separator is `MissingFinalElement { ordinal }` on the same typed variant and poisons the expression. A malformed element is `InvalidElement { ordinal, issue }`; a suffix disagreement is `ConflictingSuffix { ordinal, first, conflicting }`. Valid prefix elements remain ordered, but no missing/invalid element is fabricated and no element receives an ExprId.

Limits: 65,536 elements and 262,144 total digits. Exact commits. Either one-over rolls back the complete expression, its component source rows, diagnostic, and any BigUint allocation. No prefix truncation is permitted.

## 9. Deterministic bytes

Big integers encode limb count as ULEB128 then little-endian u32 limbs. Decimals encode coefficient digit count and bytes, scale u32 LE, exponent i32 LE. Float checked values encode width then exact bits. Duration checked values encode canonical u64 nanoseconds LE. The codec never embeds authored text or `Hash` output.


## Parent path/lifetime/call/thread contract (complete)

# Path, lifetime, call, and Thread contract

## Path resolution

A `HirPath` retains its root and typed segments exactly. Resolution takes `HirPathResolutionContext { snapshot, owner_scope }`.

- `ImplicitCrate`: consult the immutable import-alias table for the first segment. A unique alias substitutes its published typed target while retaining external project identity. If no alias exists, start at the current project's crate root. Ambiguous aliases poison the path; they never fall back.
- `Crate`: start at the current project's crate root and do not consult aliases for the root.
- `SelfModule`: start at the owner module.
- `Super { depth }`: walk exactly `depth` canonical module parents; escaping the crate poisons the path. Depth zero is canonical SelfModule.

Resolution consumes typed `HirPathSegment` values. Project/external symbols preserve hyphen-capable `HirProjectSymbolSegment`; language identifiers use `HirName`. Resolution returns a typed published symbol identity or a typed issue. A source label is never split.

Source query roles are `PathRoot` and `PathSegment { ordinal }`. The semantic path contains no span. Structural equality compares root and segments. Resolved-target equality compares publication identities under the same project generation.

## Type regions versus registry lifetime

`HirTypeRegion` appears only in HIR type nodes. Named regions carry `HirRegionName`; elided regions carry a `SyntheticKey` with the owning TypeId, role ElidedRegion, ordinal zero. Region equality is nominal for named regions and key identity for elided regions.

`HirLifetimeRegistryPath` appears only in runtime registry operations. Scope variants are Frame, Tick, Cue, Line, Scene, Flow, Session, Global, Persistent, and Named. Ordered key segments are validated identifiers. `LifetimePath` expression means Read; its `optional` bit is the authored `?`. Write, MoveOut, Drop, and Expose are statement-only modes. Optional non-read access is invalid. Registry equality compares scope, segments, and optionality; it never compares a type region.

## Ordinary and associated calls

`HirCallExpr` owns one callee and ordered arguments.

- Value callee: one same-module ExprId.
- Associated type callee: one same-module TypeId root, a member HirName, and exact separator category `DotFallback` or `ExplicitDoubleColon`.

For `target.member(...)`, the checker first checks target as a value expression. Any value-space result, including a value-space error, owns the call. Nominal fallback occurs only when typed value lookup returns definitive absence. Explicit `Type::member` is nominal-only. Turbofish and generic delimiters are part of the authored TypeId tree, not a third call-separator category: `Vec::<T>::with_capacity` has `ExplicitDoubleColon`, while its receiver TypeId source components retain `::<T>`. The TypeId tree retains generic parameters, aliases, qualified/project identity, and its own source components. `Vec<T>.with_capacity` projects that tree to the existing nominal product and then directly to `CallCallee::AssociatedType`. Bare `Vec.with_capacity` fails generic arity before candidate admission.

The shared resolver preserves its existing precedence and accounting. Environment methods precede capacity methods; capacity precedes associated trait methods. Untyped/data-last fallback is ineligible. Candidate attempts and retained results are each at most two. No second resolver, Capacity-only enum, argument replay pass, or signature-help candidate inventory may be added.

Call child source roles are Callee, AssociatedReceiver, AssociatedSeparator, AssociatedMember, and `CallArgument { argument, part }`. Receiver generic/turbofish delimiters remain TypeId source components. Ordinary argument limit is 128. RichText call contexts call the same constructor with limit 32. Missing callee/operand remains typed poison when the call family is known; an unclassifiable fragment becomes Error.

## Thread body and runtime projection

Thread lowering creates one child ScopeId and an ordered `HirThreadBody`. Every source `FlowItem` projects directly, in source order, to the exhaustive `HirThreadFlowItem` variant and a typed StmtId or dialogue-application ExprId. `Stmt`, `Choice`, `If`, `IfLet`, `Match`, `Loop`, `While`, `WhileLet`, `For`, `Select`, `SourceLocale`, `Scope`, `Include`, and `AwaitWith` retain their statement/flow identity; `SpeakerLine` and `ContentCall` become the existing typed dialogue-application expression owner; parser recovery becomes `Error(StmtId)`. There is no block expression ID and no tail. An empty authored body is valid and evaluates to Unit; only an absent required body is `MissingBody`. The Thread expression yields `ThreadHandle<Unit>`.

Bindings created by one body item become visible only to later items in the child scope according to the corresponding statement/flow rule. They never leak to the parent or sibling Thread. Nested scopes own their own locals.

Attached admission adds the task to the parent cancellation set. Parent cancellation or scope exit cancels and joins it. Detached admission transfers the task to the scheduler owner; parent scope exit does not join it. Detached capture validation requires owned/static captures and rejects frame/tick/cue/line registry borrows. Both modes return a handle. Explicit handle cancellation and runtime shutdown cancel detached work. Poisoned Thread HIR has no runtime-plan node and cannot execute.

`arcweft-runtime-plan` owns the projection:

```rust
pub struct RuntimeThreadPlan {
    handle: RuntimeValueId,
    mode: RuntimeThreadMode,
    body: Box<[RuntimeFlowStep]>,
    cancellation: RuntimeCancellationOwner,
}
```

The projection consumes typed IDs and checker facts only. It does not reopen syntax or infer mode/name from source.


## Parent relevant Rust schema sections

## B.1 Required trait contract

Every closed fieldless enum above and every other fieldless semantic enum in this document derives `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Opaque scalar IDs reuse the accepted ID trait set. Owned strings, boxed slices, payload records, error records, and enums containing them derive `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`; they are not `Copy`. Context records containing only accepted IDs (`HirPathResolutionContext`, `HirAssociatedTypeReceiver`) derive `Clone, Copy, Debug, Eq, Hash, PartialEq`. `HirExpr` and `HirPattern` derive `Clone, Debug, Eq, PartialEq`; arena ordering is by qualified ID, not by payload ordering. No semantic HIR type derives `Serialize` or `Deserialize`. Error enums additionally derive `thiserror::Error` only where they cross a fallible public query; otherwise diagnostics format them through the owning diagnostic constructor.


## D. Type regions and lifetime registry

```rust
pub enum HirTypeRegion {
    Named(HirRegionName),
    Elided(HirElidedRegion),
}
pub struct HirRegionName(HirName);
pub struct HirElidedRegion { key: SyntheticKey }

pub struct HirLifetimeRegistryPath {
    scope: HirLifetimeRegistryScope,
    segments: Box<[HirName]>,
    optional: bool,
}
pub enum HirLifetimeRegistryScope {
    Frame, Tick, Cue, Line, Scene, Flow, Session, Global, Persistent,
    Named(HirName),
}
pub enum HirLifetimeRegistryAccessMode { Read, Write, MoveOut, Drop, Expose }

pub enum HirTypeRegionIssue { InvalidNamedRegion, InvalidElisionOwner }
pub enum HirLifetimeRegistryIssue {
    InvalidNamedScope,
    InvalidKeySegment { ordinal: u32 },
    OptionalNonReadAccess,
    MissingScope,
}
```

`HirElidedRegion::try_new` requires `key.role() == SyntheticRole::ElidedRegion`, ordinal zero, and a `TypeId` owner. A `LifetimePath` expression always projects to `(path, Read)`; `optional` is the `?` read behavior. Statement forms pair the path with Write, MoveOut, Drop, or Expose. Optional non-read access is invalid.

## E. Literal and numeric schemas

```rust
pub enum HirLiteral {
    String(HirStringLiteral),
    Character(HirCharacterLiteral),
    Integer(HirIntegerLiteral),
    Float(HirFloatLiteral),
    UnitNumber(HirUnitNumberLiteral),
    Boolean(bool),
    Duration(HirDurationLiteral),
}

pub enum HirStringLiteral { Value(Box<str>), Invalid(HirStringIssue) }
pub enum HirCharacterLiteral { Value(char), Invalid(HirCharacterIssue) }

pub struct HirBigUint { limbs_le: Box<[u32]> }
pub enum HirIntegerLiteral {
    Value { magnitude: HirBigUint, radix: HirIntegerRadix, suffix: Option<HirIntegerSuffix> },
    Invalid(HirIntegerIssue),
}
pub enum HirIntegerRadix { Binary, Octal, Decimal, Hexadecimal }
pub enum HirIntegerSuffix {
    I8, I16, I32, I64, I128, ISize,
    U8, U16, U32, U64, U128, USize,
}

pub struct HirDecimal {
    coefficient: HirDecimalDigits,
    scale: u32,
    exponent10: i32,
}
pub struct HirDecimalDigits(Box<[u8]>);

pub enum HirFloatLiteral {
    Value { decimal: HirDecimal, explicit_width: Option<HirFloatWidth> },
    Invalid(HirFloatIssue),
}
pub enum HirFloatWidth { F32, F64 }
pub enum HirFloatBits { F32(u32), F64(u64) }
pub struct CheckedFloatLiteral { bits: HirFloatBits }

pub enum HirUnitNumberLiteral {
    Value { decimal: HirDecimal, unit: HirUnitNumberUnit },
    Invalid(HirUnitNumberIssue),
}
pub enum HirUnitNumberUnit {
    Percent, Px, Pt, Em, Rem, Vw, Vh,
    Deg, Rad, Turn, Db, Lufs, Bpm, Bars,
}

pub enum HirDurationLiteral {
    Value { nanoseconds: HirBigUint, authored_unit: HirDurationUnit },
    Invalid(HirDurationIssue),
}
pub enum HirDurationUnit { Nanos, Micros, Millis, Seconds, Minutes, Hours }

pub struct HirNumericSequence {
    elements: Box<[HirNumericSequenceElement]>,
    common_suffix: Option<HirIntegerSuffix>,
    recovery: HirNumericSequenceRecovery,
}
pub struct HirNumericSequenceElement { magnitude: HirBigUint, radix: HirIntegerRadix }
pub enum HirNumericSequenceRecovery {
    Complete,
    MissingFinalElement { ordinal: u32 },
    InvalidElement { ordinal: u32, issue: HirIntegerIssue },
    ConflictingSuffix { ordinal: u32, first: HirIntegerSuffix, conflicting: HirIntegerSuffix },
}

pub enum HirLiteralIssue {
    String(HirStringIssue), Character(HirCharacterIssue), Integer(HirIntegerIssue),
    Float(HirFloatIssue), UnitNumber(HirUnitNumberIssue), Duration(HirDurationIssue),
}
pub enum HirStringIssue { InvalidEscape, Unterminated, DecodedByteLimitExceeded }
pub enum HirCharacterIssue { InvalidEscape, Unterminated, Empty, MultipleScalars }
pub enum HirIntegerIssue { MissingDigits, InvalidDigit, DigitLimitExceeded { observed: u64, limit: u32 } }
pub enum HirDecimalIssue {
    MissingCoefficient, InvalidDigit,
    CoefficientLimitExceeded { observed: u64, limit: u32 },
    ScaleOutOfRange, ExponentOutOfRange,
}
pub enum HirFloatIssue { Decimal(HirDecimalIssue), NonFinite, WidthOverflow, InvalidSuffix }
pub enum HirUnitNumberIssue { Decimal(HirDecimalIssue), InvalidUnit }
pub enum HirDurationIssue {
    Decimal(HirDecimalIssue), InvalidUnit, FractionalNanosecond,
    RuntimeRangeOverflow, DigitLimitExceeded { observed: u64, limit: u32 },
}
```

`HirBigUint::try_new` enforces little-endian base-2^32 limbs, zero as an empty slice, and a non-zero final limb for non-zero values. `HirDecimalDigits::try_new` enforces digits 0..=9, zero as `[0]`, and no leading or trailing zero for a non-zero coefficient. Canonicalization is defined in `LITERAL_NUMERIC_CONTRACT.md`.


## G. Pattern inventory

```rust
pub enum HirPatternKind {
    Binding { name: HirName, local: LocalId },
    MutableBinding { name: HirName, local: LocalId },
    Literal(HirLiteral),
    EntityReference(HirIdRef),
    Variant { path: HirPath, name: HirName, payload: Option<PatternId> },
    Discard,
    Tuple { elements: Box<[PatternId]> },
    Record { path: Option<HirPath>, fields: Box<[HirPatternField]> },
    BracketSequence { elements: Box<[PatternId]>, rest: Option<LocalId> },
    WholeBinding { name: HirName, local: LocalId, pattern: PatternId },
    TypedBinding { name: HirName, local: LocalId, ty: TypeId },
    Error(HirPatternError),
}
pub struct HirPatternError { issue: HirGenericPatternIssue }
pub enum HirGenericPatternIssue { UnclassifiedSyntax, TransactionalChildFailure }
```


## J. Limits

```rust
pub struct HirLeafLimits {
    max_numeric_digits_per_literal: u32,        // 65_536
    max_decimal_coefficient_digits: u32,        // 65_536
    max_decimal_scale: u32,                     // 65_536
    max_decimal_exponent_abs: u32,              // 1_000_000
    max_numeric_sequence_elements: u32,         // 65_536
    max_numeric_sequence_total_digits: u32,     // 262_144
    max_thread_flow_items: u32,                 // 65_536
}

pub enum HirLimitIssue {
    NumericDigits { observed: u64, limit: u32 },
    DecimalCoefficient { observed: u64, limit: u32 },
    DecimalScale { observed: u64, limit: u32 },
    DecimalExponent { observed_abs: u64, limit: u32 },
    NumericSequenceElements { observed: u64, limit: u32 },
    NumericSequenceDigits { observed: u64, limit: u32 },
    ThreadFlowItems { observed: u64, limit: u32 },
    OrdinaryCallArguments { observed: usize, limit: usize },
    RichTextCallArguments { observed: usize, limit: usize },
    RichTextTags { observed: usize, limit: usize },
    RichTextArguments { observed: usize, limit: usize },
    RichTextTagBodyBytes { observed: usize, limit: usize },
    RichTextKeyBytes { observed: usize, limit: usize },
    RichTextValueBytes { observed: usize, limit: usize },
    ResolverCandidates { observed: usize, limit: usize },
}
```

The existing Proof `HirLimits` remain unchanged and are charged in addition to these leaf-content budgets. A child allocation must satisfy both; the lower effective remaining budget wins.


## Parent affected lowering rows

```tsv
row_id	category	final_variant_or_record	allocation_key	parent_child_roles_and_ordinals	scope_and_local_visibility	known_family_vs_generic_recovery	omitted_tail_missing_tail_missing_operand	clean_query	poisoned_query	stale_query	foreign_query	synthetic_query	rolled_back_query	limits_charged	exact_boundary_assertion	one_over_rollback_assertion	test_ids
E02	expression	Literal	(SyntaxNodeId,HirIdKind::Expr) -> ExprId	literal components	inherit	family-specific poisoned literal; generic Error	missing body => family poison	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR expr + literal digit budget	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-E02; T-Q-02; T-RB-02; literal family
P01	pattern	Binding	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Name	inherit; introduces local after pattern acceptance	typed binding poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/local	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P01; T-PQ-01; T-PRB-01; binding
P02	pattern	MutableBinding	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Name	inherit; introduces mutable local	typed binding poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/local	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P02; T-PQ-02; T-PRB-02; mutable binding
P03	pattern	Literal	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Literal components	inherit	family literal poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/literal budget	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P03; T-PQ-03; T-PRB-03; literal pattern
P04	pattern	EntityReference	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	ID components	inherit	HirIdRef poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P04; T-PQ-04; T-PRB-04; ID pattern all forms
P05	pattern	Variant	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	PathRoot/segments; Name; Pattern payload	inherit then payload scope	typed variant poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/children	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P05; T-PQ-05; T-PRB-05; variant pattern
P06	pattern	Discard	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Whole	inherit	generic Error only if unclassified	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P06; T-PQ-06; T-PRB-06; discard
P07	pattern	Tuple	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Element[n]	inherit	typed tuple poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/children	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P07; T-PQ-07; T-PRB-07; tuple pattern
P08	pattern	Record	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	RecordPath; PatternField[n,part]	inherit	typed record poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/fields	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P08; T-PQ-08; T-PRB-08; record pattern
P09	pattern	BracketSequence	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Element[n]; rest	inherit; rest introduces local	typed sequence poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/children/local	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P09; T-PQ-09; T-PRB-09; sequence pattern
P10	pattern	WholeBinding	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Name; Pattern	inherit; local visible after nested pattern accepts	typed whole poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/local/child	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P10; T-PQ-10; T-PRB-10; whole binding
P11	pattern	TypedBinding	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Name; Type	inherit; local typed	typed binding poison	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern/local/type	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P11; T-PQ-11; T-PRB-11; typed binding
P12	pattern	Error	(SyntaxNodeId,HirIdKind::Pattern) -> PatternId	Whole; Recovery	inherit	generic Error	missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	HIR pattern + diagnostic	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-P12; T-PQ-12; T-PRB-12; pattern error
C08	component	HirPatternField::Explicit	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	Pattern parent; PatternField[n,Whole|Name|Colon|Pattern]	pattern scope	Invalid field typed recovery	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	field/pattern slots	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C08; T-CQ-08; T-CRB-08
C09	component	HirPatternField::Shorthand	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	Pattern parent; PatternField[n,Whole|Name]	pattern scope; introduces local	Invalid field	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	field/local slots	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C09; T-CQ-09; T-CRB-09
C10	component	HirPatternField::Rest	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	Pattern parent; PatternField[n,Whole|Rest|Name]	pattern scope; optional local	multiple rest => Invalid	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	field/local slots	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C10; T-CQ-10; T-CRB-10
C11	component	HirTypeRegion::Named	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	TypeId parent; TypeRegion	type owner	invalid name => region poison	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	type slots	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C11; T-CQ-11; T-CRB-11
C12	component	HirTypeRegion::Elided	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	TypeId owner; Synthetic ElidedRegion ordinal0	type owner	invalid owner => region poison	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	synthetic descendants/type slots	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C12; T-CQ-12; T-CRB-12
C13	component	HirPathSegment	parent typed ID + role + ordinal; no independent arena slot unless listed child ID	Expr/Type/Pattern parent; PathSegment[n]	inherit	invalid segment => path poison	component-specific; optional absence => Missing query, required absence poisons owner	accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable	accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue	project/source lookup rejects stale generation or revision before module query; no range	expr_source_site => Err(IdResolveError::WrongModule); no range	expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained	transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result	segment/digit source budget	all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results	preflight/checked allocation fails atomically; no prefix/truncation/replay/leak	T-C13; T-CQ-13; T-CRB-13
```

## Parent affected test rows

```tsv
test_id	covers_row	positive	negative	malformed_recovery	source_states	limit_exact	limit_one_over	compile_fail	consumer_assertion
T-E02	E02	allocate Literal through (SyntaxNodeId,HirIdKind::Expr) -> ExprId; assert exact payload, ordered children, roles `literal components`, and scope `inherit`	violate one constructor invariant of Literal; assert typed rejection/recovery `family-specific poisoned literal; generic Error` without another family	exercise `missing body => family poison`; assert known-family poison versus generic Error exactly as row specifies	for Literal, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR expr + literal digit budget` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Literal is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Literal plus source query only; no syntax clone, display parse, or source fallback
T-P01	P01	allocate Binding through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Name`, and scope `inherit; introduces local after pattern acceptance`	violate one constructor invariant of Binding; assert typed rejection/recovery `typed binding poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Binding, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/local` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Binding is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Binding plus source query only; no syntax clone, display parse, or source fallback
T-P02	P02	allocate MutableBinding through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Name`, and scope `inherit; introduces mutable local`	violate one constructor invariant of MutableBinding; assert typed rejection/recovery `typed binding poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for MutableBinding, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/local` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where MutableBinding is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed MutableBinding plus source query only; no syntax clone, display parse, or source fallback
T-P03	P03	allocate Literal through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Literal components`, and scope `inherit`	violate one constructor invariant of Literal; assert typed rejection/recovery `family literal poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Literal, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/literal budget` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Literal is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Literal plus source query only; no syntax clone, display parse, or source fallback
T-P04	P04	allocate EntityReference through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `ID components`, and scope `inherit`	violate one constructor invariant of EntityReference; assert typed rejection/recovery `HirIdRef poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for EntityReference, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where EntityReference is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed EntityReference plus source query only; no syntax clone, display parse, or source fallback
T-P05	P05	allocate Variant through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `PathRoot/segments; Name; Pattern payload`, and scope `inherit then payload scope`	violate one constructor invariant of Variant; assert typed rejection/recovery `typed variant poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Variant, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/children` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Variant is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Variant plus source query only; no syntax clone, display parse, or source fallback
T-P06	P06	allocate Discard through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Whole`, and scope `inherit`	violate one constructor invariant of Discard; assert typed rejection/recovery `generic Error only if unclassified` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Discard, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Discard is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Discard plus source query only; no syntax clone, display parse, or source fallback
T-P07	P07	allocate Tuple through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Element[n]`, and scope `inherit`	violate one constructor invariant of Tuple; assert typed rejection/recovery `typed tuple poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Tuple, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/children` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Tuple is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Tuple plus source query only; no syntax clone, display parse, or source fallback
T-P08	P08	allocate Record through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `RecordPath; PatternField[n,part]`, and scope `inherit`	violate one constructor invariant of Record; assert typed rejection/recovery `typed record poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Record, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/fields` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Record is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Record plus source query only; no syntax clone, display parse, or source fallback
T-P09	P09	allocate BracketSequence through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Element[n]; rest`, and scope `inherit; rest introduces local`	violate one constructor invariant of BracketSequence; assert typed rejection/recovery `typed sequence poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for BracketSequence, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/children/local` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where BracketSequence is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed BracketSequence plus source query only; no syntax clone, display parse, or source fallback
T-P10	P10	allocate WholeBinding through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Name; Pattern`, and scope `inherit; local visible after nested pattern accepts`	violate one constructor invariant of WholeBinding; assert typed rejection/recovery `typed whole poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for WholeBinding, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/local/child` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where WholeBinding is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed WholeBinding plus source query only; no syntax clone, display parse, or source fallback
T-P11	P11	allocate TypedBinding through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Name; Type`, and scope `inherit; local typed`	violate one constructor invariant of TypedBinding; assert typed rejection/recovery `typed binding poison` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for TypedBinding, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern/local/type` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where TypedBinding is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed TypedBinding plus source query only; no syntax clone, display parse, or source fallback
T-P12	P12	allocate Error through (SyntaxNodeId,HirIdKind::Pattern) -> PatternId; assert exact payload, ordered children, roles `Whole; Recovery`, and scope `inherit`	violate one constructor invariant of Error; assert typed rejection/recovery `generic Error` without another family	exercise `missing required child => typed field poison or RecoveryOperand; no tail unless nested expression rule`; assert known-family poison versus generic Error exactly as row specifies	for Error, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `HIR pattern + diagnostic` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where Error is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed Error plus source query only; no syntax clone, display parse, or source fallback
T-C08	C08	allocate HirPatternField::Explicit through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `Pattern parent; PatternField[n,Whole|Name|Colon|Pattern]`, and scope `pattern scope`	violate one constructor invariant of HirPatternField::Explicit; assert typed rejection/recovery `Invalid field typed recovery` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirPatternField::Explicit, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `field/pattern slots` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirPatternField::Explicit is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirPatternField::Explicit plus source query only; no syntax clone, display parse, or source fallback
T-C09	C09	allocate HirPatternField::Shorthand through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `Pattern parent; PatternField[n,Whole|Name]`, and scope `pattern scope; introduces local`	violate one constructor invariant of HirPatternField::Shorthand; assert typed rejection/recovery `Invalid field` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirPatternField::Shorthand, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `field/local slots` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirPatternField::Shorthand is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirPatternField::Shorthand plus source query only; no syntax clone, display parse, or source fallback
T-C10	C10	allocate HirPatternField::Rest through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `Pattern parent; PatternField[n,Whole|Rest|Name]`, and scope `pattern scope; optional local`	violate one constructor invariant of HirPatternField::Rest; assert typed rejection/recovery `multiple rest => Invalid` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirPatternField::Rest, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `field/local slots` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirPatternField::Rest is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirPatternField::Rest plus source query only; no syntax clone, display parse, or source fallback
T-C11	C11	allocate HirTypeRegion::Named through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `TypeId parent; TypeRegion`, and scope `type owner`	violate one constructor invariant of HirTypeRegion::Named; assert typed rejection/recovery `invalid name => region poison` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirTypeRegion::Named, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `type slots` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirTypeRegion::Named is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirTypeRegion::Named plus source query only; no syntax clone, display parse, or source fallback
T-C12	C12	allocate HirTypeRegion::Elided through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `TypeId owner; Synthetic ElidedRegion ordinal0`, and scope `type owner`	violate one constructor invariant of HirTypeRegion::Elided; assert typed rejection/recovery `invalid owner => region poison` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirTypeRegion::Elided, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `synthetic descendants/type slots` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirTypeRegion::Elided is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirTypeRegion::Elided plus source query only; no syntax clone, display parse, or source fallback
T-C13	C13	allocate HirPathSegment through parent typed ID + role + ordinal; no independent arena slot unless listed child ID; assert exact payload, ordered children, roles `Expr/Type/Pattern parent; PathSegment[n]`, and scope `inherit`	violate one constructor invariant of HirPathSegment; assert typed rejection/recovery `invalid segment => path poison` without another family	exercise `component-specific; optional absence => Missing query, required absence poisons owner`; assert known-family poison versus generic Error exactly as row specifies	for HirPathSegment, assert clean `accepted expr_source_site => Ok(Some(Span|Insertion)); clean slot state where applicable`, poisoned `accepted expr_source_site => Ok(Some(Span|Insertion)); slot poison retains typed issue`, stale `project/source lookup rejects stale generation or revision before module query; no range`, foreign `expr_source_site => Err(IdResolveError::WrongModule); no range`, synthetic `expr_source_site => Ok(Some(Insertion)); typed SyntheticKey role/ordinal retained`, and rollback `transaction publishes no public ID; committed module has no slot/component/diagnostic/candidate/result`	charge `segment/digit source budget` at exact boundary; assert `all preflight charges pass; one transaction commits IDs, source rows, scopes, diagnostics and results`	exceed the same row charge by one; assert `preflight/checked allocation fails atomically; no prefix/truncation/replay/leak`	where HirPathSegment is public, reject raw field construction, public Serde, old variants, aliases, wrappers, and compatibility readers	the designated sema/verifier/runtime-plan/tooling consumer reads typed HirPathSegment plus source query only; no syntax clone, display parse, or source fallback
T-FLOAT-01	Float	f32/f64 suffix/context/default; exact bits; ties-even; subnormal; signed zero; NaN/inf spelling rejection; overflow	paired negative/invalid case required	assert exact typed recovery and no default	all relevant query states	exact cases named in positive	one-over cases named in positive; atomic rollback	only observable public API constraints	observable typed result through final consumer
T-DUR-01	Duration	all six units; exact nanoseconds; fractional-ns poison; u64::MAX and one-over LogicalDuration range	paired negative/invalid case required	assert exact typed recovery and no default	all relevant query states	exact cases named in positive	one-over cases named in positive; atomic rollback	only observable public API constraints	observable typed result through final consumer
T-SOURCE-01	Source	every source role/ordinal round trip across clean/poison/stale/foreign/synthetic/rollback	paired negative/invalid case required	assert exact typed recovery and no default	all relevant query states	exact cases named in positive	one-over cases named in positive; atomic rollback	only observable public API constraints	observable typed result through final consumer
T-MIGRATION-01	Deletion	public compile switch has no old HIR variants/raw readers/dual APIs; behavior tests replace source scans	paired negative/invalid case required	assert exact typed recovery and no default	all relevant query states	exact cases named in positive	one-over cases named in positive; atomic rollback	only observable public API constraints	observable typed result through final consumer
```
