# Mark coordinate and transcript

All multi-byte integers below are little-endian. `len` is `u64_le`; every
variable-length byte/string/sequence field is length-prefixed. Every counter
uses checked `u64` arithmetic before allocation or hashing. Recovery has no
success encoding.

## Stable checked dialogue-mark coordinate

The one structural coordinate is:

```text
StableCheckedDialogueMarkCoordinate :=
    CheckedSemanticPath::canonical_bytes(application)
    || u8(2)       # checked semantic path suffix: dialogue mark
    || u8(0)       # dialogue-content mark family
    || u32_le(ordinal)
```

The suffix assignment is exact: body is `0`, output target is `1`, dialogue
mark is `2`. The mark-family byte is `0`. There is no BLAKE3 layer around this
structural coordinate. The accepted application path, not the local name,
separates equal ordinals in different applications. `HirDialogueMarkName`,
`HirRichTextTagId`, source range, and runtime ID are excluded.

The only issuer is
`SemanticCoordinateIndex::dialogue_mark(HirDialogueMarkId)`. It verifies the
content owner's module/generation and resolves the accepted application path
before appending the suffix. A missing owner, stale ID, wrong content, or
ordinal not present in the exact HIR mark catalog rejects.

## Transcript domains

The domains are fixed at version one:

```text
arcweft.lang.sema.checked-statement.v1\0
arcweft.lang.sema.checked-statement-body.v1\0
arcweft.lang.sema.checked-rich-text-action.v1\0
```

Digest construction is BLAKE3 over the ASCII domain bytes followed by the
canonical payload. Each payload begins with `u8(1)`. The version is evolved in
place and remains `1`; no V2 reader or dual domain is permitted.

## Statement transcript

```text
CheckedStatementTranscript :=
    u8(1)
    || u16_le(HirStmtKind semantic tag)
    || len(StableCheckedStatementCoordinate::canonical_bytes())
    || StableCheckedStatementCoordinate::canonical_bytes()
    || u8(CheckedStatementPayload tag)
    || bytes32(EffectSet::semantic_digest())
    || len(direct_checked_child_count)
    || DirectCheckedChild*
    || len(owned_body_count)
    || OwnedBody*
    || PayloadMeaning

DirectCheckedChild :=
    StatementChildRole
    || u8(child_kind)       # Expression=0 Statement=1 Pattern=2 Type=3 Local=4
    || bytes32(checked_child_semantic_digest)

OwnedBody :=
    StatementBodyRole
    || bytes32(CheckedStatementBodyTranscript)
```

Direct children and bodies are emitted from the final-HIR typed edge APIs in
their canonical source order. The child/body digest is read from the completed
checked fact for that exact typed edge. A missing, extra, duplicate, stale, or
recovered edge rejects. Raw arena IDs are never written.

`HirStmtKind` tags are the existing contiguous `0x0700..0x0722` values in the
35-row order recorded in `TEST_MATRIX.md`. After the version marker, the HIR
tag is the first semantic atom and is always present, including `Structural`,
so structurally different statements cannot collapse through payload tag `0`.
The accepted-rooted statement coordinate follows it. The transcript issuer
consumes `CheckedStatementCoordinateEvidence`; it never writes `StmtId`.

### Direct-child role tags

Tags follow the closed final-HIR enum order and encode variant fields after the
tag:

| tag | role | extra bytes |
| ---: | --- | --- |
| 0 | `AssertionCondition` | `u32_le(ordinal)` |
| 1 | `Pattern` | none |
| 2 | `Annotation` | none |
| 3 | `Initializer` | none |
| 4 | `Input` | none |
| 5 | `Target` | none |
| 6 | `Value` | none |
| 7 | `BodyItem` | `StatementBodyRole`, `u32_le(ordinal)` |
| 8 | `ElseIf` | none |
| 9 | `TriggerExpression` | none |
| 10 | `TriggerPattern` | none |
| 11 | `TriggerSignalTarget` | none |
| 12 | `TriggerSignalValue` | none |
| 13 | `UnsafeReason` | none |
| 14 | `Condition` | none |
| 15 | `Scrutinee` | none |
| 16 | `Guard` | none |
| 17 | `MatchPattern` | `u32_le(arm)` |
| 18 | `MatchGuard` | `u32_le(arm)` |
| 19 | `MatchValue` | `u32_le(arm)` |
| 20 | `ForSource` | none |
| 21 | `ForIterator` | none |
| 22 | `ForNextValue` | none |
| 23 | `SelectOperand` | none |
| 24 | `SelectBinding` | `u32_le(branch)` |
| 25 | `SelectSource` | `u32_le(branch)` |
| 26 | `SelectPattern` | `u32_le(branch)` |

### Statement-body role tags

| tag | role | extra bytes |
| ---: | --- | --- |
| 0 | `LetElse` | none |
| 1 | `Defer` | none |
| 2 | `On` | none |
| 3 | `UnsafeLifetime` | none |
| 4 | `Then` | none |
| 5 | `Else` | none |
| 6 | `MatchArm` | `u32_le(arm)` |
| 7 | `While` | none |
| 8 | `WhileLet` | none |
| 9 | `For` | none |
| 10 | `SelectBranch` | `u32_le(branch)` |
| 11 | `SourceLocale` | none |
| 12 | `Scope` | none |

### Payload tags and meaning bytes

| tag | payload | `PayloadMeaning` |
| ---: | --- | --- |
| 0 | `Structural` | empty |
| 1 | `Assignment` | length-prefixed version-one canonical bytes owned by `CheckedAssignment` |
| 2 | `Assertion` | `u8(CheckedAssertionDisposition tag)` |
| 3 | `Defer` | `u8(DeferOutcome tag)` |
| 4 | `EvaluatedEffect` | length-prefixed canonical bytes of the already selected sealed `CheckedEvaluatedEffect` operation |
| 5 | `Iteration` | length-prefixed canonical bytes owned by `CheckedIteration` |
| 6 | `ControlTransfer` | length-prefixed canonical bytes owned by `CheckedControlTransferTarget` |
| 7 | `Trigger` | `u8(trigger tag)` and, only for Mark, `len(coordinate bytes) || coordinate bytes` |
| 8 | `UnsafeAudit` | `bytes32(AcceptedUnsafeAuditSemanticId) || u8(has_safety_doc)` |
| 9 | `Select` | Select grammar below |
| 10 | `SourceLocale` | `len(LocaleTag::canonical_bytes()) || canonical bytes` |
| 11 | `Scope` | `u8(Anonymous=0, Named=1)` |
| 12 | `Include` | `bytes32(CallableDeclarationDigest)` |
| 13 | `Suspension` | length-prefixed version-one canonical bytes owned by `CheckedSuspensionStatement` |
| 14 | `Yield` | empty |

The referenced existing typed carriers must expose or reuse one owner method
for canonical semantic bytes; the statement writer does not reconstruct their
fields and does not use serde. Their own contract tags remain version `1`.
Child meaning, including unsafe reason, timeout/expression operands, Select
source/patterns, and yielded expression, is represented exactly once through
typed child digests.

### Trigger and Select payload grammar

Trigger tags are:

```text
Input=0 Event=1 Signal=2 Timeout=3 Mark=4
Select=5 Task=6 Scope=7 Expression=8
```

Only Mark has payload bytes. Signal payload/type comes from target/value child
digests. Every other trigger is a unit family tag.

Select grammar is:

```text
SelectOperand  := u8(0)
SelectBranches := u8(1) || len(head_count) || Head*
Head           := u8(Bind=0 | Frame=1 | Event=2)
```

The source-ordered head slice and array index are authoritative. Binding,
source, pattern, and branch body are transcribed through the role-tagged child
and body rows; no head copies a type, local ID, ordinal, or Try bit.

## Body transcript

```text
CheckedStatementBodyTranscript :=
    u8(1)
    || len(StableCheckedBodyCoordinate::canonical_bytes())
    || StableCheckedBodyCoordinate::canonical_bytes()
    || u8(body_projection_kind) # Ordinary=0 Thread=1 Expression=2
    || len(body_child_count)
    || BodyChild*

BodyChild :=
    u8(HirBodyChildRole tag)
    || role_fields
    || bytes32(checked_child_semantic_digest)
```

Body child tags are `Expression=0`, `Statement=1`, `Tail=2`, reserved recovery
tag `3`, and `ThreadItem=4`. `Statement` and `ThreadItem` append their exact
`u32_le(ordinal)` role field; all other successful roles append no role field.
`RecoveryExpression` (tag 3) always rejects and is never hashed as success.
Expression projections contain exactly one expression child row. Empty
ordinary/thread bodies retain their distinct body-kind byte.

The accepted-rooted body coordinate includes its typed owner family/role,
checked path, and body kind under the existing version-one coordinate grammar.
The explicit record body-kind byte remains as the closed transcript shape
selected by the parent contract. The issuer consumes typed body-coordinate
evidence and never writes raw expression/statement owners.

## Rich-text marker action

The existing eight non-marker action families keep tags `0..7` in their
current order. The corrected Marker row is:

```text
CheckedRichTextMarkerTranscript :=
    u8(1)
    || u8(8) # Marker action
    || len(StableCheckedDialogueMarkCoordinate bytes)
    || StableCheckedDialogueMarkCoordinate bytes
```

`diagnostic_name`, `PublicId`, HIR tag ID, source range, compiler runtime ID,
and line-plan handler data are excluded. Marker equality, selection,
transcription, and compiler lookup all consume the same coordinate.

## Perturbation properties

- renaming a mark and all uses while retaining the same content/application
  structure changes diagnostics but not coordinate or semantic transcript;
- moving a mark within content changes its ordinal and coordinate;
- moving identical content to another accepted application changes the
  application path and coordinate;
- swapping Trigger/Select role tags or branch order changes the statement
  digest;
- changing only a source range, raw HIR allocation ID, or display rendering
  does not change a digest;
- changing a child checked digest, effect set, payload tag, unsafe semantic ID,
  SAFETY bit, Include declaration digest, locale, or body kind changes it;
- moving a statement/body to a different accepted semantic path changes its
  statement/body coordinate and digest without consulting a raw arena ID;
- transcript work N succeeds, N+1 rejects without partial digest publication.
