# Match role, tag, and callable-join contract

## Inventory authority

The production source at the reconciled cut has **38** `HirExprKind` variants. The table below is an exhaustive match of `HirExprKind::direct_expression_children`; statement IDs and FlowItem owners intentionally remain roots in their own typed inventories.

Tags are version-1 semantic transcript tags (`u16`). They are not AWBC opcodes and do not alter the retained AWBC numeric allocation.

## `HirExprKind` tags and ordered direct-child roles

| Tag | Family | Ordered roles | Payload order beyond children |
|---:|---|---|---|
| `0x0100` | `Unit` | — | — |
| `0x0101` | `Literal` | — | `literal semantic transcript` |
| `0x0102` | `EntityReference` | — | `accepted entity identity` |
| `0x0103` | `LifetimePath` | — | `accepted lifetime semantic identity` |
| `0x0104` | `Path` | — | `accepted resolved path identity` |
| `0x0105` | `ShortVariant` | — | `accepted variant owner` → `case ordinal` |
| `0x0106` | `Placeholder` | — | `placeholder role` |
| `0x0107` | `Tuple` | `Element[*]` | `ordered child-role transcript` |
| `0x0108` | `BracketSequence` | `Element[*]` | `ordered child-role transcript` |
| `0x0109` | `NumericBracketSequence` | — | `numeric element type` → `canonical range payload` |
| `0x010A` | `ArrayRepeat` | `RepeatedValue` → `RepeatLength` | `ordered child-role transcript` |
| `0x010B` | `Call` | `Callee?` → `Argument[*]` | `callee form` → `ordered call argument modes` → `ordered child-role transcript` |
| `0x010C` | `Select` | `Target` | `accepted selected-member class` → `ordered child-role transcript` |
| `0x010D` | `Index` | `Target` → `Index` | `ordered child-role transcript` |
| `0x010E` | `Pipe` | `PipeLeft` → `PipeRight` | `ordered child-role transcript` |
| `0x010F` | `Try` | `Operand` | `ordered child-role transcript` |
| `0x0110` | `Await` | `Operand` | `ordered child-role transcript` |
| `0x0111` | `Thread` | — | `typed thread-body owner digest` |
| `0x0112` | `Choice` | `ChoiceBodyDepthFirst` → `ChoicePlanSourceOrder` | `choice body owner digest` → `ordered flattened child-role transcript` → `optional plan owner digest` |
| `0x0113` | `Range` | `RangeStart?` → `RangeEnd?` | `ordered child-role transcript` |
| `0x0114` | `Record` | `RecordField[explicit-source-order]` | `accepted field identities in source order` → `ordered explicit field child digests` → `shorthand local identities` |
| `0x0115` | `RecordLiteral` | `RecordField[explicit-source-order]` | `accepted field identities in source order` → `ordered explicit field child digests` → `shorthand local identities` |
| `0x0116` | `Binary` | `BinaryLeft` → `BinaryRight` | `ordered child-role transcript` |
| `0x0117` | `Borrow` | `Operand` | `ordered child-role transcript` |
| `0x0118` | `Dereference` | `Operand` | `ordered child-role transcript` |
| `0x0119` | `Closure` | `ClosureBody` | `parameter semantic types` → `result semantic type` → `capture contract` → `body digest` |
| `0x011A` | `Unary` | `Operand` | `ordered child-role transcript` |
| `0x011B` | `Block` | `BlockTail` | `typed statement-owner digest list` → `tail digest` |
| `0x011C` | `ComputationBlock` | `BlockTail` | `typed statement-owner digest list` → `tail digest` |
| `0x011D` | `NamedBlock` | `BlockTail` | `accepted block identity` → `typed statement-owner digest list` → `tail digest` |
| `0x011E` | `Loop` | `LoopTail` | `typed loop statement-owner digest list` → `tail digest` |
| `0x011F` | `If` | `Condition` → `ThenBranch` → `ElseBranch` | `ordered child-role transcript` |
| `0x0120` | `IfLet` | `Scrutinee` → `IfLetGuard?` → `ThenBranch` → `ElseBranch` | `pattern digest` → `scrutinee digest` → `guard presence+digest` → `then digest` → `else digest` |
| `0x0121` | `Match` | `Scrutinee` → `(Guard?,ArmValue)[arm-source-order]` | `scrutinee digest` → `arms in source order: pattern, guard presence+digest, value digest` |
| `0x0122` | `DialogueContentApplication` | `DialogueTarget` → `DialogueCoordinate[*]` → `DialogueInterpolation[*]` → `DialogueTagPayload[*]` → `LinePlanDepthFirst` | `target` → `coordinates` → `interpolations` → `tag payloads` → `line-plan owner digest` |
| `0x0123` | `PostfixBracket` | `Target` → `(PostfixIndexCandidate,PostfixDialogueCandidate)?` | `target` → `candidate shape` → `index candidate?` → `dialogue candidate?` |
| `0x0124` | `Error` | — | `rejected before transcript emission` |
| `0x0125` | `ForSynthetic` | `ForInput` | `synthetic provenance` → `input digest` |

Optional roles are emitted only when present; their role tag prevents compacted ordinals from changing identity. Dynamic `[*]` roles encode the source ordinal before the child digest.

### Choice walk

The exact current helper order is retained:

- walk each body item in order while nested bodies are pushed onto the current   LIFO pending-body stack;
- `If`: branch conditions in branch order, then branch bodies/else body are queued;
- `For`: source, then body;
- `Match`: scrutinee, optional guards in arm order, then arm bodies are queued;
- `Option`: ID, then option fields in field order;
- `OptionFor`: source, then option fields;
- `CompactArm`: label, optional condition, optional Out value;
- option `View`: key then value for each entry in entry order; and
- plan items follow source order: Assignment value, Timeout duration, and only   expression-bearing Signal/Timeout/Expr cancel triggers.

Every nested role carries a typed `CheckedNestedPathV1`; a raw flattened ordinal is not a substitute.

### Dialogue/line-plan walk

Order is target; coordinate values; interpolation expressions; expression-bearing tag payloads; then the line-plan depth-first walk. In a line-plan slice, Option, Let, Out, TimelineAssert and Expression contribute one value; TimedCue contributes anchor then body; Start/Together groups use the current LIFO pending-group stack.

## `CheckedExpressionChildRole` tags

| Tag | Role | Payload fields |
|---:|---|---|
| `0x1000` | `Element` | `ordinal` |
| `0x1001` | `RepeatedValue` | — |
| `0x1002` | `RepeatLength` | — |
| `0x1003` | `Callee` | — |
| `0x1004` | `Argument` | `ordinal` |
| `0x1005` | `Target` | — |
| `0x1006` | `Index` | — |
| `0x1007` | `PipeLeft` | — |
| `0x1008` | `PipeRight` | — |
| `0x1009` | `Operand` | — |
| `0x100A` | `RangeStart` | — |
| `0x100B` | `RangeEnd` | — |
| `0x100C` | `RecordField` | `source_ordinal`, `accepted_field` |
| `0x100D` | `BinaryLeft` | — |
| `0x100E` | `BinaryRight` | — |
| `0x100F` | `ClosureBody` | — |
| `0x1010` | `BlockTail` | — |
| `0x1011` | `LoopTail` | — |
| `0x1012` | `Condition` | — |
| `0x1013` | `ThenBranch` | — |
| `0x1014` | `ElseBranch` | — |
| `0x1015` | `Scrutinee` | — |
| `0x1016` | `Guard` | `arm` |
| `0x1017` | `ArmValue` | `arm` |
| `0x1018` | `IfLetGuard` | — |
| `0x1019` | `DialogueTarget` | — |
| `0x101A` | `DialogueCoordinate` | `ordinal` |
| `0x101B` | `DialogueInterpolation` | `ordinal` |
| `0x101C` | `DialogueTagPayload` | `ordinal` |
| `0x101D` | `LinePlanOptionValue` | `path` |
| `0x101E` | `LinePlanLetValue` | `path` |
| `0x101F` | `LinePlanOut` | `path` |
| `0x1020` | `LinePlanTimelineAssert` | `path` |
| `0x1021` | `LinePlanExpression` | `path` |
| `0x1022` | `LinePlanTimedCueAnchor` | `path` |
| `0x1023` | `LinePlanTimedCueBody` | `path` |
| `0x1024` | `PostfixIndexCandidate` | — |
| `0x1025` | `PostfixDialogueCandidate` | — |
| `0x1026` | `ForInput` | — |
| `0x1027` | `ChoiceIfCondition` | `path`, `branch` |
| `0x1028` | `ChoiceForSource` | `path` |
| `0x1029` | `ChoiceMatchScrutinee` | `path` |
| `0x102A` | `ChoiceMatchGuard` | `path`, `arm` |
| `0x102B` | `ChoiceOptionId` | `path` |
| `0x102C` | `ChoiceOptionForSource` | `path` |
| `0x102D` | `ChoiceCompactLabel` | `path` |
| `0x102E` | `ChoiceCompactCondition` | `path` |
| `0x102F` | `ChoiceCompactOut` | `path` |
| `0x1030` | `ChoiceOptionLabel` | `path`, `field` |
| `0x1031` | `ChoiceOptionFieldId` | `path`, `field` |
| `0x1032` | `ChoiceOptionValue` | `path`, `field` |
| `0x1033` | `ChoiceOptionVisible` | `path`, `field` |
| `0x1034` | `ChoiceOptionEnabled` | `path`, `field` |
| `0x1035` | `ChoiceOptionOrder` | `path`, `field` |
| `0x1036` | `ChoiceOptionHotkey` | `path`, `field` |
| `0x1037` | `ChoiceOptionViewKey` | `path`, `field`, `entry` |
| `0x1038` | `ChoiceOptionViewValue` | `path`, `field`, `entry` |
| `0x1039` | `ChoicePlanAssignment` | `item` |
| `0x103A` | `ChoicePlanTimeout` | `item` |
| `0x103B` | `ChoicePlanCancelSignal` | `item` |
| `0x103C` | `ChoicePlanCancelTimeout` | `item` |
| `0x103D` | `ChoicePlanCancelExpr` | `item` |

## Constructor tags and payload order

### `CheckedExpressionResolution`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0200` | `Structural` | `HirExprKind semantic tag` → `accepted result type digest` → `ordered role+child digest sequence` |
| `0x0201` | `Literal` | `literal constructor transcript` → `accepted result type digest` |
| `0x0202` | `Value` | `CheckedValueResolution tag+payload` → `accepted result type digest` |
| `0x0203` | `Select` | `CheckedSelectResolution tag+payload` → `accepted result type digest` |
| `0x0204` | `Nominal` | `accepted nominal identity` → `semantic type digest` → `layout digest` |
| `0x0205` | `Variant` | `accepted variant owner` → `source-order case ordinal` → `case contract digest` |
| `0x0206` | `StageLook` | `accepted stage-look catalog row digest` |
| `0x0207` | `Effect` | `accepted effect identity` → `effect contract digest` |
| `0x0208` | `Call` | `separate call-target fact digest` → `CheckedCallableId` → `CheckedCallableDigest` → `argument mode/type transcript` |
| `0x0209` | `Await` | `source Need type digest` → `payload type digest` → `await contract digest` |
| `0x020A` | `Choice` | `accepted choice fact digest` → `result type digest` |
| `0x020B` | `Try` | `carrier kind` → `carrier type digest` → `boundary fact digest` |
| `0x020C` | `ImplicitCallable` | `CheckedCallableId` → `CheckedCallableDigest` |
| `0x020D` | `ImplicitParameter` | `accepted parameter identity` → `type digest` |
| `0x020E` | `Pipe` | `left type digest` → `right callable join` → `placeholder contract` |
| `0x020F` | `PipeLeft` | `left role contract` → `type digest` |
| `0x0210` | `ViewCall` | `accepted view callable identity` → `view contract digest` |
| `0x0211` | `ViewCallee` | `accepted view identity` → `callee contract digest` |
| `0x0212` | `StyleValue` | `accepted style value identity` → `type digest` |
| `0x0213` | `StyleCallee` | `accepted style callable identity` → `callable digest` |
| `0x0214` | `DialogueLineReference` | `accepted line identity` → `line contract digest` |
| `0x0215` | `DialogueLineCoordinate` | `accepted coordinate identity` → `coordinate contract digest` |
| `0x0216` | `DialogueTextKeyCoordinate` | `accepted text-key coordinate identity` → `contract digest` |
| `0x0217` | `CharacterDialogueFactory` | `accepted character/dialogue catalog join` |
| `0x0218` | `CharacterDialogueReconfigure` | `accepted character/dialogue catalog join` → `patch contract digest` |
| `0x0219` | `DialogueApplication` | `accepted dialogue application fact digest` |
| `0x021A` | `PostfixBracket` | `accepted branch: index or dialogue` → `selected branch contract digest` |

### `CheckedValueResolution`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0300` | `Local` | `accepted local semantic identity` → `type digest` |
| `0x0301` | `LineContext` | `accepted line-context capability identity` |
| `0x0302` | `CharacterField` | `accepted character nominal` → `field ordinal` → `field contract digest` |
| `0x0303` | `ProjectCallable` | `CheckedCallableId` → `CheckedCallableDigest` |
| `0x0304` | `ProjectItem` | `accepted project item identity` → `item contract digest` |
| `0x0305` | `Entry` | `accepted entry identity` → `entry contract digest` |
| `0x0306` | `Registered` | `registry identity` → `catalog digest` |
| `0x0307` | `Constant` | `RuntimeValueDigest` → `type digest` |

### `CheckedSelectResolution`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0400` | `Method` | `receiver type digest` → `CheckedCallableId` → `CheckedCallableDigest` → `receiver mode` |
| `0x0401` | `DialogueView` | `accepted dialogue view identity` → `view contract digest` |
| `0x0402` | `AgentField` | `RuntimeAgentOperationalType` → `accepted field ordinal` → `field contract digest` |
| `0x0403` | `ProgressField` | `closed progress field tag` |
| `0x0404` | `Field` | `accepted nominal/layout digest` → `field ordinal` → `field type digest` |
| `0x0405` | `TupleElement` | `tuple arity` → `element ordinal` → `element type digest` |
| `0x0406` | `RecordElement` | `accepted record layout digest` → `field ordinal` → `field type digest` |

### `HirPatternKind`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0500` | `Binding` | `accepted local identity` → `mutable=false` |
| `0x0501` | `MutableBinding` | `accepted local identity` → `mutable=true` |
| `0x0502` | `Literal` | `literal transcript` |
| `0x0503` | `EntityReference` | `accepted entity identity` |
| `0x0504` | `Variant` | `accepted owner` → `case ordinal` → `payload presence` → `payload child digest?` |
| `0x0505` | `Discard` | — |
| `0x0506` | `Tuple` | `arity` → `ordered child pattern digests` |
| `0x0507` | `Record` | `accepted field identities in source order` → `ordered child digests` → `rest mode` |
| `0x0508` | `BracketSequence` | `ordered child digests` → `rest mode` |
| `0x0509` | `WholeBinding` | `accepted local identity` → `child pattern digest` |
| `0x050A` | `Or` | `ordered alternative digests` → `binding-set equivalence digest` |
| `0x050B` | `TypedBinding` | `accepted local identity` → `checked type projection digest` |
| `0x050C` | `Error` | `rejected before transcript emission` |

### `CheckedPatternResolution`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0600` | `Structural` | `HirPatternKind tag` → `ordered child pattern digests` |
| `0x0601` | `Literal` | `literal transcript` |
| `0x0602` | `Entity` | `accepted entity identity` |
| `0x0603` | `Nominal` | `accepted nominal identity` → `semantic type digest` → `layout digest` |
| `0x0604` | `Variant` | `accepted variant owner` → `case ordinal` → `case contract digest` |

### `Literal`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0700` | `String` | `UTF-8 byte length` → `UTF-8 bytes` |
| `0x0701` | `Character` | `Unicode scalar u32` |
| `0x0702` | `Integer` | `signedness` → `width` → `little-endian value bits` |
| `0x0703` | `Float` | `width` → `IEEE bits` |
| `0x0704` | `UnitNumber` | `unit catalog identity` → `numeric literal transcript` |
| `0x0705` | `Boolean` | `0|1` |
| `0x0706` | `Duration` | `logical duration canonical units` |

### `Guard`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0800` | `ConstantTrue` | — |
| `0x0801` | `ConstantFalse` | — |
| `0x0802` | `Dynamic` | `guard expression digest` → `accepted bool type digest` |

### `Coverage`

| Tag | Constructor | Payload order |
|---:|---|---|
| `0x0900` | `Unit` | — |
| `0x0901` | `Boolean` | `covered false bit` → `covered true bit` |
| `0x0902` | `Tuple` | `arity` → `ordered child coverage digests` |
| `0x0903` | `AcceptedVariant` | `owner` → `covered case ordinals in ascending order` → `payload coverage` |
| `0x0904` | `AcceptedRecord` | `layout digest` → `ordered field coverage` |
| `0x0905` | `SequenceLength` | `exact|min length` → `ordered item coverage` |
| `0x0906` | `Literal` | `literal semantic digest` |
| `0x0907` | `InfiniteDomain` | `checked type digest` → `proof class` |

## Callable joins

### Unit `Call`

1. load the separate current call-target fact for the expression;
2. obtain its `CheckedCallableId` and expected `CheckedCallableDigest`;
3. resolve the exact row in `CheckedCallableCatalogV1`;
4. validate signature, receiver mode, effects, generic instantiation and digest;
5. only then emit the Call constructor payload.

### Selected Method

1. read the receiver's accepted checked type;
2. use `(receiver semantic type, HirName lookup key)` in the checked receiver catalog;
3. obtain one exact checked callable ID/digest row;
4. reject zero or multiple matches;
5. emit ID/digest/signature/receiver mode. The HirName bytes are not emitted.

A same-cut `RuntimeCallableProjectionV1` may be constructed only from that exact catalog join. Missing joins return `MissingCheckedCallableJoin`; source spelling, HirName text, declaration arena IDs and ExprId are not fallback identity.

## Work limits and first error

| Limit | Value |
|---|---:|
| expression nodes | 65,536 |
| pattern nodes | 65,536 |
| role depth | 256 |
| direct children per owner | 65,536 |
| catalog lookups | 262,144 |
| transcript bytes | 16,777,216 |

Traversal is preorder by owner and then the exact role order above. The first typed error terminates construction and no digest is returned.

## Differential requirements

- changing source spelling, HirName spelling storage, ExprId, PatternId, arena   allocation order or HirSnapshotId while preserving accepted catalogs and   semantic structure leaves the digest unchanged;
- changing `CheckedCallableDigest` while preserving display spelling changes the   digest;
- swapping child roles, optional-role presence, case ordinals, field identities   or guard placement changes the digest; and
- a missing callable/receiver join rejects rather than emitting a partial transcript.
