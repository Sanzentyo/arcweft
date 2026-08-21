# Constructible Match semantic transcripts

## 1. Current authority

`FinalSemanticAnalysis` is the sole checked semantic lookup owner. It already
binds each module to the exact current `HirSnapshotId`. Therefore:

```rust
pub struct CheckedMatchRef {
    snapshot: HirSnapshotId,
    expression: ExprId,
}
```

is compiler-local lookup evidence. It is never a persistent semantic identity.

Construction checks that the referenced expression belongs to the report's
exact module snapshot and is a checked `HirExprKind::Match`. No
`AcceptedSemanticGeneration` is introduced.

## 2. Common transcript primitives

All semantic digests use a purpose-built sink and a NUL-terminated v1 domain.
Unless stated otherwise:

```text
u8/u32/u64       fixed little-endian
digest           32 bytes
utf8 identity    u32 byte length + accepted canonical UTF-8
option<T>        0 or 1 + T
list<T>          u32 count + source/semantic-order T rows
```

No generic Serde bytes are accepted.

The following values may be used to look up facts but can never be emitted:
raw `ExprId`, `PatternId`, `LocalId`, `ItemId`, `TypeId`, `EffectId`,
`SourceSpan`, source spelling, debug names and hash-map iteration order.

## 3. `AcceptedDeclarationSemanticId`

```text
domain = "arcweft.lang.accepted-declaration-semantic.v1\0"
anchor:
  0 Public:
    declaration family semantic tag
    accepted PublicId semantic bytes
    accepted declaration contract/layout digest
  1 Nested:
    parent AcceptedDeclarationSemanticId
    declaration child-role path
    source-order ordinal
    accepted declaration contract/layout digest
```

A nested declaration is not identified by a source span or local arena number.
The current HIR owner is traversed from an accepted public/root declaration.
The role path and ordinal come from closed declaration structure.

## 4. `CheckedExpressionChildRolePath`

```text
AcceptedDeclarationSemanticId
step_count:u32
for each step:
  role semantic tag
  payload:
    Field: optional accepted field identity
    Indexed: source-order u32 ordinal
```

Closed roles cover receiver, callee, argument, tuple/sequence element, record
field, condition, then/else, block statement/tail, loop body, range bound,
borrow/deref operand, closure body/capture, Match scrutinee/arm guard/arm body,
dialogue target/patch/rich text, postfix target/index and every other current
`HirExprKind` child position.

The constructor walks the current HIR tree from the accepted declaration root
and proves that each reachable `ExprId` maps to exactly one role path. Duplicate
or unreachable mappings are hard errors.

## 5. `StableCheckedValueCoordinate`

```text
0 Expression:
  AcceptedDeclarationSemanticId
  CheckedExpressionChildRolePath

1 PatternBinding:
  AcceptedDeclarationSemanticId
  Match expression child-role path
  arm_ordinal:u32
  StablePatternCoordinate
  binding_ordinal:u32

2 Capture:
  callable AcceptedDeclarationSemanticId
  capture_ordinal:u32
  origin StableCheckedValueCoordinate
```

Local names are deliberately absent. Binding identity follows semantic
structure and source-order pattern coordinates.

## 6. Expression semantic digest

```text
domain = "arcweft.lang.checked-expression-semantic.v1\0"
stable expression coordinate
CheckedExpressionResolution variant transcript
checked RuntimeTypeSemanticDigest
checked effect/control contract digest
ordered child-role rows:
  child role step
  child CheckedExpressionSemanticDigest
```

`Structural` uses the exact checked HIR expression family and child rows.
Other resolution variants add the accepted semantic payload below.

## 7. Exhaustive `CheckedExpressionResolution` table

| Variant | Exact semantic payload |
|---|---|
| `Structural` | expression family tag; runtime type digest; effect digest; ordered child-role-path plus child expression digest |
| `Literal` | CheckedLiteralSemanticV1 |
| `Value` | CheckedValueResolutionSemanticV1 |
| `Select` | CheckedSelectResolutionSemanticV1 |
| `Nominal` | accepted nominal semantic identity; accepted layout digest; ordered type arguments |
| `Variant` | accepted nominal semantic identity; accepted layout digest; accepted case ordinal/identity; optional payload type |
| `StageLook` | accepted Stage schema field identity; raw HirName is lookup-only and never emitted |
| `Effect` | accepted effect semantic identity and checked effect contract digest; raw EffectId is lookup-only |
| `Call` | accepted RuntimeCallableId; CallableContractHash; checked receiver mode; source-order argument role/digest rows |
| `Await` | await mode; checked Need payload type; source child digest; pending-effect semantic digest; no task/runtime identity |
| `Choice` | accepted choice semantic identity; source-order option semantic rows; result type digest |
| `Try` | carrier kind Result|Option; source child digest; propagated type/effect contract |
| `ImplicitCallable` | accepted RuntimeCallableId and CallableContractHash; ordered capture coordinates and type digests |
| `ImplicitParameter` | stable coordinate of owning implicit callable; parameter ordinal and type digest |
| `Pipe` | source and target child-role digests; closed placeholder-routing transcript |
| `PipeLeft` | stable coordinate of owning Pipe plus left-value role; raw ExprId excluded |
| `ViewCall` | ViewProgramId; accepted view callable contract; source-order argument role/digests |
| `ViewCallee` | ViewProgramId; accepted view callable contract |
| `StyleValue` | accepted style property semantic identity; canonical specified-value transcript |
| `StyleCallee` | accepted style callable semantic identity and contract |
| `DialogueLineReference` | typed DialogueLineId semantic bytes |
| `DialogueLineCoordinate` | typed DialogueLineId semantic bytes plus coordinate role |
| `DialogueTextKeyCoordinate` | typed DialogueTextKey semantic bytes plus coordinate role |
| `CharacterDialogueFactory` | accepted character/dialogue contract identity; source-order argument role/digests |
| `CharacterDialogueReconfigure` | accepted character/dialogue contract identity; source-order patch field identity/digests |
| `DialogueApplication` | target child digest; application-patch digest; rich-text digest |
| `PostfixBracket` | closed PostfixBracketResolution semantic tag; target child digest; source-order index/slice child digests |
## 8. Exhaustive `CheckedValueResolution` table

| Variant | Exact semantic payload |
|---|---|
| `Local` | StableCheckedValueCoordinate for the resolved local; checked type digest |
| `LineContext` | accepted line-context semantic owner; no source spelling |
| `CharacterField` | receiver child digest; accepted character semantic identity; accepted field identity |
| `ProjectCallable` | accepted RuntimeCallableId; CallableContractHash |
| `ProjectItem` | AcceptedDeclarationSemanticId; accepted item semantic digest |
| `Entry` | accepted entry semantic identity and contract |
| `Registered` | registered semantic value identity and accepted catalog digest |
| `Constant` | CheckedLiteralSemanticV1; checked runtime type digest |
## 9. Exhaustive `CheckedSelectResolution` table

| Variant | Exact semantic payload |
|---|---|
| `Method` | accepted method RuntimeCallableId; CallableContractHash; receiver mode |
| `DialogueView` | accepted dialogue projection identity; accepted field identity |
| `AgentField` | RuntimeAgentField inherent semantic identity and owner |
| `ProgressField` | closed Progress field semantic identity |
| `Field` | accepted nominal semantic identity/layout; accepted field ordinal/identity |
| `TupleElement` | checked tuple arity; source-order ordinal |
| `RecordElement` | accepted nominal semantic identity/layout; accepted record-field ordinal/identity |
## 10. Exhaustive `CheckedPatternResolution` table

| Variant | Exact semantic payload |
|---|---|
| `Structural` | HirPatternKind transcript plus child pattern digests |
| `Literal` | CheckedLiteralSemanticV1 |
| `Entity` | AcceptedDeclarationSemanticId of the resolved project item |
| `Nominal` | accepted nominal semantic identity and layout digest |
| `Variant` | accepted nominal semantic identity/layout and accepted case ordinal/identity |
## 11. Exhaustive HIR pattern-family table

| Variant | Exact semantic payload |
|---|---|
| `Binding` | stable binding coordinate; checked type digest; mutability=false |
| `MutableBinding` | stable binding coordinate; checked type digest; mutability=true |
| `Literal` | CheckedLiteralSemanticV1 |
| `EntityReference` | AcceptedDeclarationSemanticId |
| `Variant` | accepted owner/layout/case identity; optional payload child role |
| `Discard` | no payload |
| `Tuple` | arity; source-order child digests |
| `Record` | accepted owner/layout; accepted field identities in source order; rest semantic; child digests |
| `BracketSequence` | source-order item digests; rest semantic |
| `WholeBinding` | stable binding coordinate; nested pattern digest |
| `Or` | source-order alternatives; equal binding-shape certificate |
| `TypedBinding` | stable binding coordinate; explicit checked type digest |
| `Error` | not encodable; generic Match construction fails before publication |
## 12. Exact literal payloads

| Literal | Accepted payload | Transcript | Excluded | Invalid/recovery |
|---|---|---|---|---|
| `String` | decoded Unicode scalar sequence | tag || utf8_byte_len:u32-le || exact validated UTF-8 bytes | source quotes, escapes, raw spelling, span | HirStringLiteral::Invalid rejects |
| `Character` | one Unicode scalar | tag || scalar_value:u32-le | source quote/escape spelling | HirCharacterLiteral::Invalid rejects |
| `Integer` | checked sign/width plus canonical non-negative HirBigUint magnitude | tag || signedness_width_tag || limb_count:u32-le || canonical little-endian u32 limbs | authored radix, separators, source suffix spelling; the checked type owns width | invalid literal or type overflow rejects |
| `Float` | checker-admitted HirFloatBits | tag || width_tag || exact IEEE-754 bit pattern (u32-le or u64-le) | decimal source spelling, exponent spelling, redundant zeroes | invalid/unrepresentable literal rejects; exact signed zero bits are retained |
| `UnitNumber` | canonical HirDecimal plus closed HirUnitNumberUnit | tag || unit_tag || coefficient_digit_count:u32-le || digits || scale:u32-le || exponent10:i32-le | source separators and equivalent decimal spelling | HirUnitNumberLiteral::Invalid rejects |
| `Boolean` | bool | tag || 0x00(false) or 0x01(true) | source span | no recovery value |
| `Duration` | canonical whole-nanosecond HirDurationSemanticValue | tag || limb_count:u32-le || canonical little-endian u32 limbs | authored duration unit and source spelling | HirDurationLiteral::Invalid rejects |

Integer semantics use the checked sign/width and canonical magnitude. Authored
radix and separators do not change semantic identity. Float semantics use exact
checker-admitted IEEE-754 bits; decimal source spelling is never hashed.
Duration semantics use whole nanoseconds and exclude the authored unit.

## 13. Pattern digest

```text
domain = "arcweft.lang.checked-pattern-semantic.v1\0"
StablePatternCoordinate
HirPatternKind semantic tag
CheckedPatternResolution semantic transcript
checked pattern type digest
binding rows in stable pattern preorder
child pattern digests in source order
```

`StablePatternCoordinate` is a declaration-rooted sequence of closed steps:

```text
TupleElement(ordinal)
RecordField(accepted field identity, source ordinal)
SequenceElement(ordinal)
VariantPayload(accepted case identity)
WholeBindingInner
OrAlternative(ordinal)
TypedBindingInner
```

Raw pattern IDs and names are absent.

`HirPatternKind::Error` has a table row for exhaustiveness but cannot produce an
accepted digest. It returns `CheckedPatternSemanticError::RecoveryPattern`
before Match publication.

## 14. Callable/project/nominal references

- callable references emit accepted `RuntimeCallableId` plus
  `CallableContractHash`;
- public/nested declarations emit `AcceptedDeclarationSemanticId`;
- accepted project nominal references emit the existing nominal semantic
  identity and exact `TypeLayoutHash`;
- variant/record fields emit accepted owner/layout plus case/field semantic
  identity and source-order child roles;
- current View references emit `ViewProgramId` plus the accepted callable
  contract; revision is not generic expression semantics;
- `EffectId`, `ItemId`, raw nominal indices and display strings are lookup-only.

Every lookup must resolve through current `FinalSemanticAnalysis`, accepted
project catalogs and runtime-plan type projection. Missing accepted authority
is a typed construction failure, not a fallback to source spelling.

## 15. Guard class

```text
0 ConstantTrue
1 ConstantFalse
2 Dynamic
```

Only an exact checked Boolean literal is classified constant. `None` is encoded
by guard presence `0`, not a fourth class. A computed constant, callable,
registered value or source spelling that appears true remains Dynamic unless
the current checked semantic fact itself is the exact Boolean literal.

## 16. Coverage constructors

The same-cut coverage owner publishes a closed constructor algebra:

```rust
pub enum CheckedCoverageConstructor {
    Unit,
    Boolean(bool),
    Tuple { arity: u32 },
    AcceptedVariant {
        owner: AcceptedNominalSemanticIdentity,
        layout: TypeLayoutHash,
        case: AcceptedVariantCaseIdentity,
    },
    AcceptedRecord {
        owner: AcceptedNominalSemanticIdentity,
        layout: TypeLayoutHash,
    },
    SequenceLength {
        kind: CheckedSequenceConstructorKind,
        length: CheckedSequenceLengthConstructor,
    },
    Literal(CheckedLiteralSemanticDigest),
    InfiniteDomain,
}
```

Constructor rows contain accepted identities/layouts and source-order child
roles. They do not contain source spelling or raw declaration IDs.

Coverage uses the retained bounded matrix algorithm. It publishes:

- exact constructor-domain evidence;
- exhaustive/nonexhaustive result and witness;
- unreachable arm rows sorted by arm ordinal;
- reason `CoveredByPriorRows`, `FalseGuard`,
  `RedundantOrAlternative` or `UninhabitedDomain`;
- work counters only as diagnostics, excluded from semantic digest.

## 17. `CheckedMatchSemanticDigest`

```text
domain = "arcweft.lang.checked-match-semantic.v1\0"
scrutinee CheckedExpressionSemanticDigest
scrutinee RuntimeTypeSemanticDigest
arm_count:u32
for arms in source order:
  arm_ordinal:u32
  CheckedPatternSemanticDigest
  binding_count:u32
  stable binding coordinate + binding RuntimeTypeSemanticDigest
  guard_presence:u8
  if present:
    guard CheckedExpressionSemanticDigest
    guard_class:u8
  body CheckedExpressionSemanticDigest
coverage exhaustive:u8
coverage constructor-domain digest
unreachable_count:u32
sorted unreachable arm ordinal + reason
```

The digest includes body semantics and coverage facts. It excludes
`ViewProgramId`, accepted View revision, View output/site coordinates,
ownership evidence, source/HIR coordinates, spans and work counters.

## 18. Differential and tamper corpus

Required pairs:

1. rebuild equivalent HIR with every `ExprId`, `PatternId`, `LocalId`,
   `ItemId`, `TypeId` and span changed: all expression/pattern/Match digests
   equal;
2. preserve source spelling but change accepted callable contract: digest
   differs;
3. preserve field spelling but change accepted nominal layout/field identity:
   digest differs;
4. spell the same integer in decimal/hex with equal checked value/type: literal
   digest equal;
5. spell equal duration in different units with equal whole nanoseconds:
   literal digest equal;
6. change exact float bits including signed zero: digest differs;
7. reorder source arms or source-order child fields: digest differs;
8. reorder a backing hash map without changing semantic/source order: digest
   equal;
9. inject a raw HIR ID/span/debug name into a machine transcript fixture:
   package/implementation structural tests fail.
