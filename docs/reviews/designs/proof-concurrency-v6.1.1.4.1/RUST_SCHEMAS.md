# Exact Rust-facing schemas

Unless a section states another owner, every type below is owned by `arcweft-lang-hir::expr` or its named responsibility module. Fields are private. `new` is `pub(crate)` when all inputs are already validated; `try_new` is `pub(crate)` when invariants are checked. Every field has a same-named public read-only accessor returning a copy for Copy fields, `&T` for owned records, `&str` for strings, or `&[T]` for boxed slices. No type below has a public raw constructor or public Serde implementation.

## A. Arena slot and recovery

```rust
pub struct HirExpr {
    kind: HirExprKind,
    scope: ScopeId,
    state: HirPoisonState,
}

pub struct HirPattern {
    kind: HirPatternKind,
    scope: ScopeId,
    state: HirPoisonState,
}

pub enum HirPoisonState {
    Clean,
    Poisoned(HirRecoveryIssue),
}

pub enum HirRecoveryIssue {
    MissingOperand { role: HirExprSourceRole },
    MissingRequiredTail,
    MalformedLiteral(HirLiteralIssue),
    InvalidName(HirNameInvariantError),
    InvalidPath(HirPathIssue),
    InvalidId(HirIdRefInvariantError),
    InvalidTypeRegion(HirTypeRegionIssue),
    InvalidLifetimeRegistry(HirLifetimeRegistryIssue),
    InvalidCall(HirCallIssue),
    InvalidThread(HirThreadIssue),
    InvalidDialogue(HirDialogueIssue),
    InvalidRichText(HirRichTextIssue),
    LimitExceeded(HirLimitIssue),
    StaleSource,
    ForeignSource,
}
```

`HirExpr::try_new` and `HirPattern::try_new` require all referenced IDs to be live in the same module and the supplied scope to own the child visibility. A clean slot may not contain any invalid/recovery payload. A poisoned slot may retain valid children allocated earlier in the same transaction, but is never executable.

## B. Final expression inventory

```rust
pub enum HirExprKind {
    Unit,
    Literal(HirLiteral),
    EntityReference(HirIdRef),
    LifetimePath(HirLifetimeRegistryPath),
    Path(HirPath),
    ShortVariant(HirName),
    Placeholder(HirPlaceholderKind),
    Tuple(HirTupleExpr),
    BracketSequence(HirBracketSequenceExpr),
    NumericBracketSequence(HirNumericSequence),
    ArrayRepeat(HirArrayRepeatExpr),
    Call(HirCallExpr),
    Select(HirSelectExpr),
    Index(HirIndexExpr),
    Pipe(HirPipeExpr),
    Try(HirTryExpr),
    Await(HirAwaitExpr),
    Thread(HirThreadExpr),
    Range(HirRangeExpr),
    Record(HirRecordExpr),
    RecordLiteral(HirRecordLiteralExpr),
    Binary(HirBinaryExpr),
    Borrow(HirBorrowExpr),
    Dereference(HirDereferenceExpr),
    Closure(HirClosureExpr),
    Unary(HirUnaryExpr),
    Block(HirBlockExpr),
    ComputationBlock(HirComputationBlockExpr),
    NamedBlock(HirNamedBlockExpr),
    If(HirIfExpr),
    IfLet(HirIfLetExpr),
    Match(HirMatchExpr),
    DialogueContentApplication(HirDialogueContentApplication),
    PostfixBracket(HirPostfixBracket),
    Error(HirExprError),
}

pub enum HirPlaceholderKind { PartialApplication, PipeLeft }

pub struct HirTupleExpr { elements: Box<[ExprId]> }
pub struct HirBracketSequenceExpr { elements: Box<[ExprId]> }
pub struct HirArrayRepeatExpr { value: ExprId, length: ExprId }
pub struct HirSelectExpr { target: ExprId, member: HirName }
pub struct HirIndexExpr { target: ExprId, index: ExprId }
pub struct HirPipeExpr { left: ExprId, right: ExprId }
pub struct HirTryExpr { operand: ExprId, form: HirTryForm }
pub enum HirTryForm { PrefixTry, PostfixQuestion }
pub struct HirAwaitExpr { operand: ExprId, propagation: HirAwaitPropagation }
pub enum HirAwaitPropagation { PreserveResult, PropagateError }
pub struct HirRangeExpr { start: Option<ExprId>, end: Option<ExprId>, inclusive: bool }
pub struct HirRecordExpr { path: HirPath, fields: Box<[HirRecordField]> }
pub struct HirRecordLiteralExpr { fields: Box<[HirRecordField]> }
pub struct HirBinaryExpr { left: ExprId, operator: HirBinaryOp, right: ExprId }
pub struct HirBorrowExpr { kind: HirBorrowKind, operand: ExprId }
pub struct HirDereferenceExpr { operand: ExprId }
pub struct HirUnaryExpr { operator: HirUnaryOp, operand: ExprId }
pub struct HirClosureExpr {
    scope: ScopeId,
    parameters: Box<[HirClosureParameter]>,
    result_type: Option<TypeId>,
    body: ExprId,
    captures: Box<[CaptureId]>,
}
pub struct HirBlockExpr { scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId }
pub struct HirComputationBlockExpr {
    kind: HirComputationBlockKind,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}
pub struct HirNamedBlockExpr {
    name: HirName,
    scope: ScopeId,
    statements: Box<[StmtId]>,
    tail: ExprId,
}
pub struct HirIfExpr { condition: ExprId, then_branch: ExprId, else_branch: ExprId }
pub struct HirIfLetExpr {
    scope: ScopeId,
    pattern: PatternId,
    scrutinee: ExprId,
    guard: Option<ExprId>,
    then_branch: ExprId,
    else_branch: ExprId,
}
pub struct HirMatchExpr { scrutinee: ExprId, arms: Box<[HirMatchArm]> }
pub struct HirExprError { issue: HirGenericExprIssue }
pub enum HirGenericExprIssue { UnclassifiedSyntax, TransactionalChildFailure }
```

The operator vocabularies are exact HIR-owned projections; no syntax enum crosses the HIR crate boundary:

```rust
pub enum HirBinaryOp {
    Implies, Or, And, In, Equal, NotEqual,
    GreaterOrEqual, LessOrEqual, Greater, Less,
    Merge, Add, Subtract, Multiply, Divide, Remainder,
}
pub enum HirUnaryOp { Not, Negate }
pub enum HirBorrowKind { Shared, Mutable }
pub enum HirComputationBlockKind { Result, Task, Seq, Stream }
```

`HirBinaryOp::Implies` is the semantic projection of `=>`. Token spelling and ranges remain source components.

## B.1 Required trait contract

Every closed fieldless enum above and every other fieldless semantic enum in this document derives `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Opaque scalar IDs reuse the accepted ID trait set. Owned strings, boxed slices, payload records, error records, and enums containing them derive `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`; they are not `Copy`. Context records containing only accepted IDs (`HirPathResolutionContext`, `HirAssociatedTypeReceiver`) derive `Clone, Copy, Debug, Eq, Hash, PartialEq`. `HirExpr` and `HirPattern` derive `Clone, Debug, Eq, PartialEq`; arena ordering is by qualified ID, not by payload ordering. No semantic HIR type derives `Serialize` or `Deserialize`. Error enums additionally derive `thiserror::Error` only where they cross a fallible public query; otherwise diagnostics format them through the owning diagnostic constructor.

## C. Names, paths, and ID references

```rust
pub struct HirName(Box<str>);
pub struct HirProjectSymbolSegment(Box<str>);

pub struct HirPath {
    root: HirPathRoot,
    segments: Box<[HirPathSegment]>,
}

pub enum HirPathRoot {
    ImplicitCrate,
    Crate,
    SelfModule,
    Super { depth: usize },
}

pub enum HirPathSegment {
    Identifier(HirName),
    ProjectSymbol(HirProjectSymbolSegment),
}

pub struct HirPathResolutionContext {
    snapshot: HirSnapshotId,
    owner_scope: ScopeId,
}

pub enum HirPathIssue {
    Empty,
    InvalidSegment { ordinal: u32 },
    SuperEscapesCrate { depth: usize, available: usize },
    UnknownAlias { segment: HirProjectSymbolSegment },
    AmbiguousAlias { segment: HirProjectSymbolSegment },
    UnknownExternalProject { segment: HirProjectSymbolSegment },
    UnpublishedTarget,
    StaleSnapshot,
    ForeignScope,
}

pub enum HirIdRef {
    Absolute(HirEntityReference),
    Relative(HirRelativeId),
    FamilyRelative(HirFamilyRelativeId),
}
pub struct HirEntityReference(Box<str>);
pub struct HirIdSuffix(Box<str>);
pub struct HirIdFamily(Box<str>);
pub struct HirRelativeId { suffix: HirIdSuffix, parent_depth: usize }
pub struct HirFamilyRelativeId { family: HirIdFamily, relative: HirRelativeId }

pub enum HirIdRefInvariantError {
    EmptyAbsolute,
    EmptySuffix,
    AuthoredRelativeMarker,
    InvalidSuffix,
    InvalidFamily,
}
```

`HirName::try_new` accepts exactly one parser-validated identifier. It performs no Unicode normalization or case folding. `HirProjectSymbolSegment::try_new` accepts non-empty Unicode letters/numbers plus `_` and `-`, rejects controls and path separators, and preserves code points. `HirPath::try_new` rejects an empty segment list; `Super { depth: 0 }` is canonicalized to `SelfModule`. `HirRelativeId::new` preserves every `usize` depth, including zero; no narrower type or arbitrary compatibility bound is introduced.

Expression and pattern entity references both use the complete `HirIdRef` family. This is a deliberate final decision: the attached token grammar is shared, and restricting patterns to absolute IDs would create a second unresolved ID-reference grammar.

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

## F. Call and child records

```rust
pub struct HirCallExpr { callee: HirCallCallee, arguments: Box<[HirCallArgument]> }
pub enum HirCallCallee {
    Value { expression: ExprId },
    AssociatedType(HirAssociatedTypeCallee),
}
pub struct HirAssociatedTypeCallee {
    receiver: HirAssociatedTypeReceiver,
    member: HirName,
    syntax: HirAssociatedCallSyntax,
}
pub struct HirAssociatedTypeReceiver { root: TypeId }
pub enum HirAssociatedCallSyntax { DotFallback, ExplicitDoubleColon }

pub enum HirCallArgument {
    Positional { value: ExprId },
    Named { name: HirName, value: ExprId },
    Spread { value: ExprId },
}

pub enum HirCallIssue {
    TooManyArguments { observed: usize, limit: usize },
    DuplicateNamedArgument { name: HirName },
    PositionalAfterNamed,
    SpreadNotLast,
    ForeignChild,
    MissingCallee,
    InvalidAssociatedReceiver,
    BareGenericArity,
}

pub enum HirRecordField {
    Explicit { name: HirName, value: ExprId },
    Shorthand { name: HirName, local: LocalId },
    Invalid { issue: HirRecordFieldIssue },
}
pub enum HirRecordFieldIssue { MissingName, MissingValue, DuplicateName, ForeignChild }

pub struct HirClosureParameter { pattern: PatternId, ty: Option<TypeId>, local_scope: ScopeId }
pub struct HirMatchArm {
    scope: ScopeId,
    pattern: PatternId,
    guard: Option<ExprId>,
    value: ExprId,
    locals: Box<[LocalId]>,
}
pub enum HirPatternField {
    Explicit { name: HirName, pattern: PatternId },
    Shorthand { name: HirName, local: LocalId },
    Rest { binding: Option<LocalId> },
    Invalid { issue: HirPatternFieldIssue },
}
pub enum HirPatternFieldIssue { MissingName, MissingPattern, DuplicateName, MultipleRest, ForeignChild }
```

Ordinary calls accept at most 128 arguments. RichText inline Fx/dialogue calls use the same `HirCallExpr` but their lowering context supplies the stricter limit 32. The argument record itself does not encode context or a second call model.

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

## H. Thread

```rust
pub struct HirThreadExpr {
    name: Option<HirName>,
    mode: HirThreadMode,
    scope: ScopeId,
    body: HirThreadBody,
}
pub enum HirThreadMode { Attached, Detached }
pub struct HirThreadBody { items: Box<[HirThreadFlowItem]> }
pub enum HirThreadFlowItem {
    Statement(StmtId),
    DialogueApplication(ExprId),
    Choice(StmtId),
    If(StmtId),
    IfLet(StmtId),
    Match(StmtId),
    Loop(StmtId),
    While(StmtId),
    WhileLet(StmtId),
    For(StmtId),
    Select(StmtId),
    SourceLocale(StmtId),
    Scope(StmtId),
    Include(StmtId),
    AwaitWith(StmtId),
    Error(StmtId),
}
pub enum HirThreadIssue {
    ForeignScope, ForeignBodyChild, InvalidName,
    DetachedBorrowedCapture { capture: CaptureId },
    DetachedEphemeralRegistryAccess,
    MissingBody,
}
```

There is no block `ExprId` and no optional tail. An authored empty body is valid and yields Unit; `MissingBody` applies only when the required block/body owner is absent. `HirThreadBody` preserves source order exactly. Every item ID is owned by the thread child scope or by a nested descendant scope; the scope arena is the sole local table, so the thread payload does not duplicate locals. Attached threads join/cancel with the parent cancellation set. Detached threads are scheduler-owned and require owned/static captures; they do not join at parent scope exit. Poisoned threads have no runtime-plan node.

## I. Accepted Dialogue outer records and final content

The existing AW-AH-009.4.2 outer records remain the owner. This contract adds the previously undefined content dependency without changing the accepted ID, coordinate, line-plan, candidate, source-site, or insertion shapes.

```rust
pub struct HirDialogueContentApplication {
    target: ExprId,
    content: HirDialogueContent,
    plan: Option<HirLinePlan>,
    coordinates: Box<[HirDialogueCoordinate]>,
}

pub struct HirPostfixBracket {
    target: ExprId,
    candidates: HirPostfixBracketCandidates,
}

pub struct HirDialogueCoordinate {
    kind: HirDialogueCoordinateKind,
    argument: HirCallArgumentOrdinal,
    value: ExprId,
}
pub enum HirDialogueCoordinateKind { Id, TextKey }
pub struct HirCallArgumentOrdinal(u16);

pub struct HirLinePlan { root_scope: ScopeId, label: Option<HirName>, items: Box<[HirLinePlanItem]> }
pub enum HirLinePlanItem {
    Init(Box<[StmtId]>), Thread(StmtId), On(StmtId),
    Option { name: HirName, value: ExprId },
    Let { pattern: PatternId, value: ExprId },
    Statement(StmtId), Out(ExprId), CancelRule(StmtId),
    TimedCue { anchor: ExprId, body: ExprId },
    StartGroup(Box<[HirLinePlanItem]>), TogetherGroup(Box<[HirLinePlanItem]>),
    TimelineAssert { policy: TimelineAssertPolicy, condition: ExprId },
    Expression(ExprId), Error(StmtId),
}

pub enum HirPostfixBracketCandidates {
    Ambiguous { index: ExprId, dialogue: ExprId },
    Invalid { index: HirPostfixCandidateFailure, dialogue: HirPostfixCandidateFailure },
}
pub struct HirPostfixCandidateFailure { kind: HirPostfixCandidateFailureKind }
pub enum HirPostfixCandidateFailureKind {
    EmptyPayload, UnexpectedToken, MissingOperand, TrailingToken, InvalidDialogueAtom,
}
```

`HirDialogueContentApplication::try_new` requires target, coordinate values, plan children, nested content expressions, and content IDs to share the owner module. Coordinates are strictly increasing by `argument`, may repeat `kind`, and retain the authored argument ordinal. `HirPostfixBracket::try_new` requires both ambiguous candidate roots to use their accepted candidate-only role at ordinal zero, requires every nested candidate-only child to use deterministic zero-based preorder within its interpretation and `HirIdKind`, and verifies that both candidate targets equal `target`.

```rust
pub struct HirDialogueContent {
    id: HirDialogueContentId,
    nodes: Box<[HirDialogueNode]>,
    tags: Box<[HirRichTextTag]>,
}
pub struct HirDialogueContentId { owner: ExprId }
pub struct HirDialogueNodeId { content: HirDialogueContentId, ordinal: u32 }
pub struct HirRichTextTagId { content: HirDialogueContentId, ordinal: u32 }
pub struct HirRichTextArgumentId { tag: HirRichTextTagId, ordinal: u16 }

pub struct HirDialogueNode { id: HirDialogueNodeId, kind: HirDialogueNodeKind }
pub enum HirDialogueNodeKind {
    Text(HirTextFragment),
    Raw(HirTextFragment),
    Escape(char),
    Ruby(HirRuby),
    AuthoredStartTag(HirRichTextTagId),
    InferredStartTag(HirRichTextTagId),
    AuthoredEndTag(HirRichTextEndTag),
    InferredEndTag(HirRichTextEndTag),
    Interpolation(ExprId),
    Control(HirDialogueControl),
    Mark(HirName),
    LineBreak(HirLineBreakKind),
    Error(HirDialogueContentError),
}
pub struct HirTextFragment(Box<str>);
pub struct HirRuby { base: Box<str>, ruby: Box<str> }
pub enum HirLineBreakKind { Line, Paragraph, Page }

pub struct HirRichTextEndTag {
    identity: Option<HirRichTextTagIdentity>,
    inferred: bool,
    issue: Option<HirRichTextIssue>,
}

pub struct HirRichTextTag {
    id: HirRichTextTagId,
    identity: HirRichTextTagIdentity,
    arguments: Box<[HirRichTextArgument]>,
    payload: HirRichTextTagPayload,
}

pub enum HirRichTextTagIdentity {
    Builtin(HirBuiltinRichTextTag),
    Marker(HirName),
    Registered(HirRegisteredRichTextTagId),
    Unresolved(HirUnresolvedRichTextTag),
}

pub enum HirBuiltinRichTextTag {
    Page,
    LineWait,
    HardBreak,
    TimedWait,
    Clear,
    Reset,
    Speed,
    DirectStyle(HirRichTextDirectStyle),
    Style(HirRichTextStyleSelector),
    Layout(HirRichTextLayoutSelector),
    Transform(HirRichTextTransformSelector),
    Object(HirRichTextObjectSelector),
    Fx(HirBuiltinRichTextFx),
    HostEvent(HirRichTextHostEvent),
    Conditional(HirRichTextConditionalTag),
}
pub enum HirRichTextDirectStyle {
    Emphasis, Strong, Italic, Oblique, Color, Font, Size, Ruby,
}
pub enum HirRichTextStyleSelector { Italic, Oblique, Opacity, Layer, ZIndex }
pub enum HirRichTextLayoutSelector {
    HorizontalTb, VerticalRl, VerticalLr, Direction,
    RubyOver, RubyUnder, RubyInterCharacter,
}
pub enum HirRichTextTransformSelector { Offset, Rotate, Scale, Skew }
pub enum HirRichTextObjectSelector { Object }
pub enum HirBuiltinRichTextFx {
    Wave, Shake, Jitter, Arc, Spin, Pulse, Motion, Typewriter, Sparkle, Shader,
}
pub enum HirRichTextHostEvent {
    Voice, Face, Pose, Show, Hide, Move, Scale, Rotate,
    Animation, StageShake, TimedCue, Call, Signal,
}
pub enum HirRichTextConditionalTag { If, Else, EndIf }
pub enum HirRegisteredRichTextTagId {
    Project(ItemId),
    External(HirExternalSymbolId),
}
pub struct HirExternalSymbolId { project: HirProjectSymbolSegment, path: HirPath }
pub struct HirUnresolvedRichTextTag { name: HirProjectSymbolSegment, issue: HirRichTextIssue }

pub enum HirRichTextTagPayload {
    Arguments,
    FxCall(ExprId),
    DialogueCall(ExprId),
    Condition(ExprId),
    None,
}

pub enum HirRichTextArgument {
    Positional { id: HirRichTextArgumentId, value: HirRichTextValue },
    Named { id: HirRichTextArgumentId, name: HirName, value: HirRichTextValue },
    Invalid { id: HirRichTextArgumentId, issue: HirRichTextArgumentIssue },
}
pub struct HirRichTextValue(Box<str>);
pub enum HirRichTextArgumentIssue {
    EmptyKey, InvalidKey, InvalidEscape, UnterminatedQuote,
    KeyTooLong, ValueTooLong, MissingValue, DecoderFailure,
}

pub enum HirDialogueControl {
    Wait, Reset, Clear, Erase, ClearMessage, Speed, Voice, Face, Pose,
    Show, Hide, Move, Scale, Rotate, Animation, StageShake, At, Call, Signal,
    ConditionalIf, ConditionalElse, ConditionalEnd,
}
pub enum HirDialogueContentError {
    UnclassifiedToken, InvalidEscape, InvalidRuby, UnmatchedEndTag, UnclosedTag,
    LimitExceeded(HirLimitIssue),
}
pub enum HirDialogueIssue {
    ForeignChild, DuplicateNodeId, NonContiguousNodeOrdinal,
    InvalidTagReference, InvalidArgumentReference, InvalidPlan,
}
pub enum HirRichTextIssue {
    UnknownTag, UnknownFx, UnknownRegisteredTag, InvalidNesting,
    InvalidPayload, ForeignNestedExpression, Argument(HirRichTextArgumentIssue),
    LimitExceeded(HirLimitIssue),
}
```

`HirDialogueContentId::new(owner)` is one-to-one with the application expression. Node and tag slices are both contiguous zero-based `u32`; argument ordinals are contiguous zero-based `u16` and `< 32`. Every start-tag node resolves its ID in `content.tags`; every tag and argument ID has this content as its transitive owner, and no ID is valid in another content or module. A tag's argument slice is the sole HIR argument owner; argument IDs are derived from that tag and must match vector order. `FxCall` and `DialogueCall` child expressions must resolve to `HirExprKind::Call`; `Condition` may be any expression but checker result must be Bool. `CheckedRichTextValueId`, owned by `arcweft-lang-sema::rich_text`, is keyed by `HirRichTextArgumentId`; no invalid value produces a default.

The HIR projection enums above are required because `arcweft-lang-hir` cannot depend upward on `arcweft-presentation`. They are exhaustive one-to-one projections of the current presentation-owned inventories, not a second membership registry: direct style has 8 variants, style 5, layout 7, transform 4, object 1, and builtin Fx 10. Grammar aliases are canonicalized by attached typed syntax before HIR and therefore never appear as semantic enum variants.

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
