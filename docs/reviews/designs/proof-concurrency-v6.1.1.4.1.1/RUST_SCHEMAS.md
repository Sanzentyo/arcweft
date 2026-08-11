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
    StaleSource,
    ForeignSource,
}
```

`HirExpr::try_new` and `HirPattern::try_new` require all referenced IDs to be live in the same module and the supplied scope to own the child visibility. A clean slot may not contain an invalid/recovery payload. A poisoned slot may retain valid children allocated earlier in the same transaction, but is never executable.

Hard source/byte/digit/count limits do not produce `HirRecoveryIssue`: they return `HirLowerError::Limit(HirLimitError)` before publication and roll back the complete transaction.

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

Every closed fieldless semantic enum derives `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Opaque scalar IDs reuse the accepted ID trait set. Owned strings, boxed slices, payload records, error records, and enums containing them derive `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`; they are not `Copy`. Context records containing only accepted IDs derive `Clone, Copy, Debug, Eq, Hash, PartialEq`. `HirExpr` and `HirPattern` derive `Clone, Debug, Eq, PartialEq`; arena ordering is by qualified ID, not by payload ordering. No semantic HIR type derives `Serialize` or `Deserialize`.

Duration has no custom structural equality exception. `HirDurationValue` derives the ordinary structural traits and includes `authored_unit`; unit-insensitive semantics are expressed only through the separate `HirDurationSemanticValue` returned by `semantic_value()`.

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

## D. Type regions, typed synthetic owners, and lifetime registry

```rust
pub enum HirTypeRegion {
    Named(HirRegionName),
    Elided(HirElidedRegion),
}

pub struct HirRegionName(HirName);
pub struct HirElidedRegion { key: SyntheticKey }

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticOwner {
    Item(ItemId),
    Scope(ScopeId),
    Local(LocalId),
    Expr(ExprId),
    Stmt(StmtId),
    Type(TypeId),
    Pattern(PatternId),
    Capture(CaptureId),
}

impl SyntheticOwner {
    pub const fn kind(self) -> HirIdKind;
    pub const fn module(self) -> HirModuleId;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntheticKey {
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
}

impl SyntheticRole {
    pub(crate) const fn accepts_owner(
        self,
        owner_kind: HirIdKind,
        ordinal: u32,
    ) -> bool;
}

impl SyntheticKey {
    pub(crate) fn try_new(
        owner: SyntheticOwner,
        role: SyntheticRole,
        ordinal: u32,
    ) -> Result<Self, SyntheticKeyError>;

    pub const fn owner(self) -> SyntheticOwner;
    pub const fn role(self) -> SyntheticRole;
    pub const fn ordinal(self) -> u32;
}

pub enum SyntheticKeyError {
    WrongOwnerKind {
        role: SyntheticRole,
        actual: HirIdKind,
    },
    InvalidOrdinal {
        role: SyntheticRole,
        ordinal: u32,
    },
}

pub enum HirElidedRegionError {
    OwnerMismatch {
        expected: TypeId,
        actual: SyntheticOwner,
    },
}

impl HirElidedRegion {
    pub(crate) fn try_new(
        owner: TypeId,
        key: SyntheticKey,
    ) -> Result<Self, HirElidedRegionError>;

    pub const fn owner_type(self) -> TypeId;
    pub const fn key(self) -> SyntheticKey;
}

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

`SyntheticOwner::kind()` and `SyntheticOwner::module()` are inherent methods on the original identity enum. `SyntheticKey::try_new(owner, role, ordinal)` calls the original inherent `SyntheticRole::accepts_owner(owner.kind(), ordinal)` policy; no extension trait or arena-kind probe is used. The key constructor does not pretend to validate snapshot liveness: before a key is staged, the owning HIR transaction resolves the exact typed owner variant against its module/snapshot. A foreign module produces `IdResolveError::WrongModule`; a committed stale owner produces the existing not-yet-live/retired error; a staged owner is accepted only when it is already present in the same transaction.

The complete role addition relevant here is exact: `SyntheticRole::ElidedRegion` accepts only `HirIdKind::Type` and ordinal zero. All pre-existing role/owner policies retain their accepted behavior. `HirElidedRegion::try_new(owner, key)` additionally verifies `key.owner() == SyntheticOwner::Type(owner)` and returns `HirElidedRegionError::OwnerMismatch` otherwise.

`SyntheticOwner`, `SyntheticKey`, and their fieldless error/role components derive structural `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Stable key fingerprint input is the version tag `arcweft-synthetic-key-v1`, owner-kind discriminant, owner module/slot, role discriminant, and little-endian ordinal. The private current `RawHirId` owner field is deleted in the same compiling change; no raw-owner accessor or conversion remains.

`HirTypeRegion` appears only in HIR type nodes. Named regions compare by `HirRegionName`; elided regions compare by the typed `SyntheticKey`. `HirLifetimeRegistryPath` appears only in runtime registry operations and compares scope, ordered segments, and optionality. A `LifetimePath` expression always has Read access; optional non-read access is invalid.

## E. Literal, semantic-value, and checker-result schemas

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
    Value {
        magnitude: HirBigUint,
        radix: HirIntegerRadix,
        suffix: Option<HirIntegerSuffix>,
    },
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
    Value(HirDurationValue),
    Invalid(HirDurationIssue),
}

pub struct HirDurationValue {
    semantic: HirDurationSemanticValue,
    authored_unit: HirDurationUnit,
}

pub struct HirDurationSemanticValue { nanoseconds: HirBigUint }
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
    ConflictingSuffix {
        ordinal: u32,
        first: HirIntegerSuffix,
        conflicting: HirIntegerSuffix,
    },
}

pub enum HirLiteralIssue {
    String(HirStringIssue),
    Character(HirCharacterIssue),
    Integer(HirIntegerIssue),
    Float(HirFloatIssue),
    UnitNumber(HirUnitNumberIssue),
    Duration(HirDurationIssue),
}

pub enum HirStringIssue { InvalidEscape, Unterminated }
pub enum HirCharacterIssue { InvalidEscape, Unterminated, Empty, MultipleScalars }
pub enum HirIntegerIssue { MissingDigits, InvalidDigit }
pub enum HirDecimalIssue { MissingCoefficient, InvalidDigit }
pub enum HirFloatIssue { Decimal(HirDecimalIssue), NonFinite, InvalidSuffix }
pub enum HirUnitNumberIssue { Decimal(HirDecimalIssue), InvalidUnit }
pub enum HirDurationIssue {
    Decimal(HirDecimalIssue),
    InvalidUnit,
    FractionalNanosecond,
}
```

`HirBigUint::try_new` enforces little-endian base-2^32 limbs, zero as an empty slice, and a non-zero final limb for non-zero values. `HirDecimalDigits::try_new` enforces digits 0..=9, zero as `[0]`, and no leading or trailing zero for a non-zero coefficient.

`HirDurationValue` and `HirDurationSemanticValue` both derive structural `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. `HirDurationValue::semantic_value()` returns `&HirDurationSemanticValue`; `authored_unit()` returns the normalized unit. Consequently `1s != 1000ms` structurally while their semantic values are equal.

Checker-owned records in `arcweft-lang-sema::literal` are exact:

```rust
pub enum FloatLiteralCheckResult {
    Accepted(CheckedFloatLiteral),
    Rejected(FloatLiteralCheckError),
}

pub enum FloatLiteralCheckError {
    WidthOverflow {
        expression: ExprId,
        width: HirFloatWidth,
        observed: HirDecimal,
    },
}

pub enum DurationLiteralCheckResult {
    Accepted(CheckedDurationLiteral),
    Rejected(DurationLiteralCheckError),
}

pub struct CheckedDurationLiteral { nanoseconds: u64 }

pub enum DurationLiteralCheckError {
    RuntimeRangeOverflow {
        expression: ExprId,
        observed: HirDurationSemanticValue,
        maximum: u64,
    },
}
```

Rejected checks publish no `CheckedFloatLiteral`, `CheckedDurationLiteral`, runtime constant, verifier constant, or runtime-plan node. `HirFloatIssue::WidthOverflow`, `HirDurationIssue::RuntimeRangeOverflow`, `HirStringIssue::DecodedByteLimitExceeded`, and all digit/count limit issue variants are deleted rather than retained as unconstructible compatibility variants.

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

## G. Pattern inventory and pathless variant heads

```rust
pub enum HirPatternKind {
    Binding { name: HirName, local: LocalId },
    MutableBinding { name: HirName, local: LocalId },
    Literal(HirLiteral),
    EntityReference(HirIdRef),
    Variant(HirVariantPattern),
    Discard,
    Tuple { elements: Box<[PatternId]> },
    Record { path: Option<HirPath>, fields: Box<[HirPatternField]> },
    BracketSequence { elements: Box<[PatternId]>, rest: Option<LocalId> },
    WholeBinding { name: HirName, local: LocalId, pattern: PatternId },
    TypedBinding { name: HirName, local: LocalId, ty: TypeId },
    Error(HirPatternError),
}

pub struct HirVariantPattern {
    head: HirVariantPatternHead,
    name: HirName,
    payload: Option<PatternId>,
}

pub enum HirVariantPatternHead {
    Qualified(HirPath),
    Unqualified(HirUnqualifiedVariantForm),
}

pub enum HirUnqualifiedVariantForm {
    DotShorthand,
    BareExpectedType,
}

pub enum HirVariantPatternIssue {
    MissingName,
    InvalidQualifiedPath,
    MissingPayload,
    ForeignPayload,
    InvalidPayloadKind,
}

pub struct HirPatternError { issue: HirGenericPatternIssue }
pub enum HirGenericPatternIssue { UnclassifiedSyntax, TransactionalChildFailure }
```

`HirVariantPattern::try_new` accepts no empty path. `Qualified` requires a valid non-empty `HirPath`; `Unqualified` carries no path field. `.Foo` maps to `DotShorthand`; `Some`, `None`, `Ok`, and `Err` map to `BareExpectedType`. The checker resolves both unqualified forms through the expected enum type and shared variant catalog; HIR does not select Option/Result early.

If present, `payload` must be a live same-module PatternId whose kind is Tuple or Record. It inherits the parent pattern scope and does not create another scope. A semantically unknown variant remains clean HIR and is rejected by sema. Known-family malformed syntax remains Variant plus typed poison; only unclassifiable syntax becomes Error.

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
    LimitExceeded(HirRichTextLimitIssue),
}
pub enum HirDialogueIssue {
    ForeignChild, DuplicateNodeId, NonContiguousNodeOrdinal,
    InvalidTagReference, InvalidArgumentReference, InvalidPlan,
}
pub enum HirRichTextIssue {
    UnknownTag, UnknownFx, UnknownRegisteredTag, InvalidNesting,
    InvalidPayload, ForeignNestedExpression, Argument(HirRichTextArgumentIssue),
    LimitExceeded(HirRichTextLimitIssue),
}
```

`HirDialogueContentId::new(owner)` is one-to-one with the application expression. Node and tag slices are both contiguous zero-based `u32`; argument ordinals are contiguous zero-based `u16` and `< 32`. Every start-tag node resolves its ID in `content.tags`; every tag and argument ID has this content as its transitive owner, and no ID is valid in another content or module. A tag's argument slice is the sole HIR argument owner; argument IDs are derived from that tag and must match vector order. `FxCall` and `DialogueCall` child expressions must resolve to `HirExprKind::Call`; `Condition` may be any expression but checker result must be Bool. `CheckedRichTextValueId`, owned by `arcweft-lang-sema::rich_text`, is keyed by `HirRichTextArgumentId`; no invalid value produces a default.

The HIR projection enums above are required because `arcweft-lang-hir` cannot depend upward on `arcweft-presentation`. They are exhaustive one-to-one projections of the current presentation-owned inventories, not a second membership registry: direct style has 8 variants, style 5, layout 7, transform 4, object 1, and builtin Fx 10. Grammar aliases are canonicalized by attached typed syntax before HIR and therefore never appear as semantic enum variants.

## J. Limits and hard-failure owner

The existing `arcweft-lang-hir::identity::HirLimit` enum remains the sole HIR limit owner. This correction adds the following variants to the original enum and implements their behavior in the original inherent `maximum()` method:

```rust
pub enum HirLimit {
    // accepted allocation variants remain unchanged
    ModulesPerDatabase,
    Items,
    Statements,
    Expressions,
    Types,
    Patterns,
    Scopes,
    LocalsPerScope,
    LocalsPerModule,
    Captures,
    Diagnostics,
    SyntheticDescendantsPerOwner,
    TotalSlotsPerModule,

    SourceDocumentBytes,
    DecodedStringBytes,
    NameBytes,
    PathSegments,
    PathSemanticBytes,
    RegistrySegments,
    RegistrySemanticBytes,
    NumericDigitsPerLiteral,
    DecimalCoefficientDigits,
    DecimalScale,
    DecimalExponentAbs,
    NumericSequenceElements,
    NumericSequenceTotalDigits,
    ThreadFlowItems,
}

pub struct HirLimitError {
    limit: HirLimit,
    observed: usize,
    maximum: usize,
}

pub enum HirLowerError {
    // accepted non-limit variants are unchanged
    Limit(HirLimitError),
}
```

New inclusive maxima:

```text
SourceDocumentBytes             8_388_608
DecodedStringBytes              8_388_608
NameBytes                       1_024
PathSegments                    256
PathSemanticBytes               65_536
RegistrySegments                256
RegistrySemanticBytes           65_536
NumericDigitsPerLiteral         65_536
DecimalCoefficientDigits        65_536
DecimalScale                     65_536
DecimalExponentAbs               1_000_000
NumericSequenceElements         65_536
NumericSequenceTotalDigits      262_144
ThreadFlowItems                 65_536
```

`observed` and `maximum` are `usize`; every source `u64` or parser count is converted with checked arithmetic before comparison. `SourceDocumentBytes` intentionally equals `arcweft_source::MAX_REGISTRATION_SOURCE_BYTES`; the final HIR lowerer applies the same maximum to registered and local documents before starting a transaction. `DecodedStringBytes` is charged after typed escape decoding and before allocating the HIR `Box<str>`.

`PathSemanticBytes` is the checked sum of normalized segment UTF-8 bytes, excluding root/separator spelling. `RegistrySemanticBytes` is the checked sum of a named scope (zero for builtin scopes) plus key segment UTF-8 bytes. Every segment also satisfies `NameBytes`; counts satisfy their separate segment limit.

Exact boundary values commit. One-over or arithmetic overflow returns `HirLowerError::Limit` and publishes no owner ID, source key, scope, diagnostic, candidate, checked value, or partial payload. Callable, resolver, and RichText limits remain in their accepted owners and are charged independently.
