# Semantic transcript grammar

## Canonical byte grammar

All semantic digests in this cut use BLAKE3 over a purpose-built domain and a
version-`1` canonical byte stream. Version is part of the fixed domain string;
there is no generic version field, fallback reader, or alternate domain.

Primitive productions are:

```text
tag8       := one byte
tag16      := unsigned 16-bit little-endian
u32        := unsigned 32-bit little-endian
u64        := unsigned 64-bit little-endian
bool       := 0x00 | 0x01
digest32   := exactly 32 bytes
bytes      := u64(byte_count) || byte[byte_count]
seq<T>     := u64(element_count) || T...
option<T>  := 0x00 | 0x01 || T
```

Every count is admitted as a checked `u64` before writing. An in-memory length
uses `u64::try_from`; a typed `u32` ordinal is widened losslessly. Tags are
closed owner-defined discriminants, never enum debug text or Serde output.
`TranscriptHasher::update` returns `Result`, performs `checked_add`, and checks
the byte limit before hashing. The current saturating counter and every
`expect`/sentinel conversion are deleted.

Canonical literals reuse the current checked numeric semantics: UTF-8 string
bytes, Unicode scalar `u32`, unsigned magnitude limbs plus checked signed/type
meaning, IEEE `to_bits` for checked f32/f64, canonical coefficient/scale/
exponent plus unit tag for unit numbers, Boolean byte, and canonical duration
nanoseconds. Invalid/recovered literals reject. Source lexemes never enter.

## Domains

```text
expression := BLAKE3("arcweft.lang.checked-expression-semantic.v1\0" || ...)
pattern    := BLAKE3("arcweft.lang.checked-pattern-semantic.v1\0" || ...)
statement  := BLAKE3("arcweft.lang.checked-statement-semantic.v1\0" || ...)
body       := BLAKE3("arcweft.lang.checked-body-semantic.v1\0" || ...)
coverage   := BLAKE3("arcweft.lang.checked-match-coverage.v1\0" || ...)
match      := BLAKE3("arcweft.lang.checked-match-semantic.v1\0" || ...)
project item := BLAKE3("arcweft.lang.accepted-project-item.v1\0" || ...)
variant case := BLAKE3("arcweft.lang.accepted-variant-case.v1\0" || ...)
record field := BLAKE3("arcweft.lang.accepted-record-field.v1\0" || ...)
environment field := BLAKE3("arcweft.lang.accepted-environment-field.v1\0" || ...)
character look := BLAKE3("arcweft.lang.accepted-character-look.v1\0" || ...)
view modifier := BLAKE3("arcweft.lang.accepted-view-modifier.v1\0" || ...)
rich text := BLAKE3("arcweft.lang.checked-rich-text-semantic.v1\0" || ...)
```

The owner of each existing identity supplies its canonical bytes. A transcript
does not hash a digest's display form.

## Stable coordinate grammar

Every expression, statement, pattern, local, and Match arm is reached from one
accepted semantic root. The root prefix is closed and carries the root kind:

```text
semantic_root := 0x00 || accepted_declaration_semantic_id
               | 0x01 || accepted_item_semantic_id
```

The canonical coordinate is:

```text
coordinate := semantic_root || u64_le(step_count) || path_step...
path_step  := closed_step_tag || typed_role_payload
```

Existing accepted HIR body, statement-child, thread-body, expression-child,
pattern-child, parameter-pattern, parameter-default, Match-pattern, and nested
Choice/dialogue path tags retain their current assignments. Same-cut tags are
appended for `ViewValue { ordinal }` and the non-expression roots enumerated in
`SCHEMAS.md`; no existing tag is renumbered. Payload is only a checked ordinal,
accepted field ID, or nested closed-role path. Raw arena IDs, snapshot IDs,
scope/local IDs, spans, offsets, names, and file paths are forbidden.

`ViewValue { ordinal }` is rooted in the existing View `CallableDeclarationKey`
and uses the declaration root. Item-owned attributes, resources, entries,
styles, tests, benches, and inline members use the item root and their checked
source-order entry/member role. A coordinate never omits its root tag or
collapses an item root into a declaration root.

## Expression transcript

```text
expression_record :=
    coordinate
 || expression_shape_tag16
 || resolution_tag16
 || semantic_type_digest
 || effect_row_semantic_digest
 || shape_atoms
 || resolution_atoms
 || seq<child_role_bytes || child_expression_digest>
 || option<match_payload_digest>
 || seq<body_role_bytes || body_digest>
```

`match_payload_digest` is present exactly for `HirExprKind::Match`. Body
digests are present for Await, Choice, and dialogue expression-owned bodies.
Children are written once in accepted semantic child order. The writer rejects
duplicates, missing edges, path disagreement, callable disagreement, poison,
or cycles.

The 38 expression shape tags are fixed in source enum order:
`Unit=0x0100` through `ForSynthetic=0x0125`. In full they are Unit, Literal,
EntityReference, LifetimePath, Path, ShortVariant, Placeholder, Tuple,
BracketSequence, NumericBracketSequence, ArrayRepeat, Call, Select, Index,
Pipe, Try, Await, Thread, Choice, Range, Record, RecordLiteral, Binary, Borrow,
Dereference, Closure, Unary, Block, ComputationBlock, NamedBlock, Loop, If,
IfLet, Match, DialogueContentApplication, PostfixBracket, Error, ForSynthetic.
`Error` (`0x0124`) is a reserved rejecting tag, never a successful transcript.

Shape atoms are exhaustive:

| HIR shape | Non-child atoms |
|---|---|
| Unit | none |
| Literal | canonical checked literal |
| EntityReference, LifetimePath, Path, ShortVariant | none beyond exact resolution atoms |
| Placeholder | closed placeholder-kind tag |
| Tuple, BracketSequence | checked child count |
| NumericBracketSequence | checked element count and every owner-defined numeric-sequence separator/range-mode tag; invalid element rejects |
| ArrayRepeat | no extra atom beyond repeated-value/repeat-length roles |
| Call | checked call-form tag, argument slot/name-coordinate tags, explicit type-argument semantic digests, and exact callable join digest |
| Select | checked select-form tag; exact selection lives in resolution atoms |
| Index | closed index-form tag |
| Pipe | closed pipe-form atoms; exact owner/use coordinates live in resolution atoms |
| Try | closed propagation-form tag; boundary/callable live in resolution atoms |
| Await | branch count and source-order closed branch-kind tags; branch pattern/body digests are body atoms |
| Thread | closed thread mode plus body digest |
| Choice | source-order Choice item/field/plan tags, accepted compact-action tag/target ID, and nested body digests |
| Range | inclusive Boolean and endpoint-presence roles |
| Record | accepted nominal semantic type/layout and checked field IDs in authored semantic order |
| RecordLiteral | checked field semantic IDs and rest/update-presence tag |
| Binary | owner-defined binary-operator tag |
| Borrow | owner-defined borrow-kind/mutability tag |
| Dereference | owner-defined dereference tag |
| Closure | parameter mode/type digests, result type, capture coordinates/types, and closure-kind tag |
| Unary | owner-defined unary-operator tag |
| Block, ComputationBlock | body-kind tag and body digest |
| NamedBlock | accepted control-label semantic coordinate and body digest; not label spelling |
| Loop | loop-kind/control target coordinate and body digest |
| If | branch count/else-presence; conditions/results are child roles |
| IfLet | checked pattern digest and binding coordinates in addition to expression children |
| Match | `match_payload_digest` below |
| DialogueContentApplication | checked content shape plus rich-text/body atoms below |
| PostfixBracket | selected interpretation tag only; rejected candidate omitted |
| ForSynthetic | checked iterator-family tag, binding pattern digest, and body digest |

This shape layer closes a current hole: `Structural` is not permitted to make
distinct operators or expression families collide.

### All 27 resolution families

The existing tags `0x0200..0x021A` remain assigned in source enum order. Every
tag has exactly these owner atoms:

| Resolution | Exact atoms |
|---|---|
| Structural | none; exhaustive shape atoms remain mandatory |
| Literal | canonical literal bytes |
| Value | nested value tag and atoms below |
| Select | nested select tag and atoms below |
| Nominal | checked nominal semantic type, project layout hash when project-owned, type-argument digests |
| Variant | checked owner semantic type/layout, selected `AcceptedVariantCaseSemanticId`, ordinal, payload type digest/presence |
| StageLook | checked character nominal digest, Character identity bytes, `AcceptedCharacterLookSemanticId` |
| Effect | owner-defined `EffectSemanticDigest` |
| Call | exact current `CheckedCallableJoin` semantic digest |
| Await | checked outcome/continuation type digests, branch kind, pattern digest, body digest in source order |
| Choice | exact accepted project-item IDs for compact goto arms plus checked Choice structural/body atoms |
| Try | carrier/result type digests, accepted-rooted propagation-boundary coordinate, optional exact accepted callable declaration digest |
| ImplicitCallable | owner coordinate, parameter type/coordinate rows, capture origin/type rows, checked body resolution/digest |
| ImplicitParameter | owning implicit-callable coordinate and checked parameter ordinal/type |
| Pipe | pipe owner coordinate, left/right evaluation contract, source-order placeholder-use coordinates/types |
| PipeLeft | owning pipe coordinate and checked placeholder ordinal/type |
| ViewCall | existing View declaration/callable digest, closed element tags, exact view-modifier IDs in argument order |
| ViewCallee | closed callee tag and View owner digest |
| StyleValue | owner-defined `ViewSpecifiedValueSemanticDigest` covering all 27 current variants |
| StyleCallee | closed constructor tag and owner/type digest |
| DialogueLineReference | existing accepted `DialogueLineId` canonical identity bytes |
| DialogueLineCoordinate | existing accepted `DialogueLineId` canonical identity bytes |
| DialogueTextKeyCoordinate | existing accepted `DialogueTextKey` canonical identity bytes |
| CharacterDialogueFactory | checked Character/nominal target digest, callable/factory tag, checked patch field rows |
| CharacterDialogueReconfigure | checked target digest and source-order checked patch operation/field/type/value-digest rows |
| DialogueApplication | checked target, optional checked patch digest, `CheckedRichTextSemanticDigest`, and line-plan body digests |
| PostfixBracket | closed selected-candidate tag and selected candidate digest |

Raw expression/statement/pattern/item IDs inside the current Await/Try/
implicit/Pipe/dialogue/postfix structs are lookup evidence only. The builder
must resolve them to the listed coordinate/digest, prove exact equality with
the checked child edge, and then exclude them.

### Nested value families

Existing value tags `0x0300..0x0307` encode:

- Local: accepted-rooted binding coordinate plus type digest;
- LineContext: tag only;
- CharacterField: receiver value digest, checked Character identity, and
  owner-defined Character-field tag;
- ProjectCallable: exact checked callable ID and interface/join digest;
- ProjectItem: `AcceptedProjectItemSemanticId`, family, and value type digest;
- Entry: `CheckedEntryBindingDigest` and value type digest;
- Registered: existing `RegisteredSemanticValueId` canonical bytes;
- Constant: canonical literal.

Existing select tags `0x0400..0x0406` encode:

- Method: `CheckedCallableJoinDigest`, receiver type, receiver mode;
- DialogueView: owner type plus existing `DialogueProjectionCoordinate`;
- AgentField and ProgressField: owner type plus exhaustive owner-defined field
  tag;
- Field: owner type, project/environment checked field semantic ID,
  declaration ordinal, and result type;
- TupleElement: checked ordinal and result type;
- RecordElement (`0x0406`): always reject and delete its unproduced enum
  variant; the tag remains reserved and is not reassigned.

No nested family emits a name.

## Pattern transcript

```text
pattern_record :=
    stable_pattern_coordinate
 || pattern_shape_tag16
 || pattern_resolution_tag16
 || semantic_type_digest
 || shape_atoms
 || resolution_atoms
 || seq<pattern_child_role || child_pattern_digest>
 || seq<binding_role || stable_binding_coordinate || binding_type_digest>
```

Pattern shape tags are `0x0500..0x050C` in the exact 13-family source order.
`Error=0x050C` rejects. Existing resolution tags `0x0600..0x0604` retain their
assignments.

- Binding/MutableBinding encode mutability and stable binding coordinate, not
  a local ID or name.
- Literal encodes the canonical literal.
- Entity encodes `AcceptedProjectItemSemanticId` and value type.
- Variant encodes the exact case row; payload is a typed child role.
- Tuple encodes arity and child digests.
- Record encodes the checked nominal owner, rest Boolean, and source-order
  `CheckedRecordPatternField` rows (source ordinal, declaration ordinal,
  semantic field ID, field type digest, child digest). It does not resolve a
  field spelling.
- BracketSequence encodes element count and rest mode
  (`Absent|Unbound|Bound`); a bound rest uses a stable binding coordinate.
- WholeBinding encodes binding coordinate/type and inner pattern.
- Or encodes alternative count and source-order child digests.
- TypedBinding encodes binding coordinate plus retained annotation type digest.

## Statement and body transcripts

Statement tags are `0x0700..0x0722` in the exact 35-family source order listed
in `SOURCE_EVIDENCE.md`; `Error=0x0722` rejects. A statement record contains
its accepted-rooted coordinate, tag, minimal checked semantic payload, and
all typed HIR child roles paired with expression/pattern/statement/body/type/
binding semantic digests. Closed assertion/defer/trigger/control/thread/
select/include/source-locale tags are encoded by their owning types. Accepted
labels/targets use semantic coordinates or IDs, never spelling.

```text
body_record := body_coordinate || body_kind_tag ||
               seq<body_child_role || child_semantic_digest>
```

Body children remain in HIR semantic order. Await, Choice, and dialogue
line-plan non-expression roots use the appended roles from `SCHEMAS.md`.
StartGroup/TogetherGroup path segments preserve recursive group order. This is
a memoized projection, not a duplicated statement model.

## Rich text

`CheckedRichTextSemanticDigest` covers, in accepted content order:

- canonical text fragments, line-break/Ruby/control tags and semantic payload;
- resolved builtin/custom tag/action identities and closed operation tags;
- checked field and default identities/types;
- stable expression-child roles and their expression digests;
- checked dialogue coordinate identities and line-plan statement/body digests.

It excludes source spans, source ranges, raw node/tag/argument/Expr IDs,
diagnostic spelling, and parser recovery. Any unresolved/recovered tag or
semantic child rejects instead of hashing a placeholder.

## Match payload, coverage, and final Match digest

Match payload construction is bottom-up and cycle checked:

```text
match_payload :=
    scrutinee_expression_digest
 || scrutinee_type_digest
 || seq<arm_coordinate
        || pattern_digest
        || guard_tag
        || option<guard_expression_digest>
        || result_expression_digest
        || seq<binding_coordinate || binding_type_digest>>
 || coverage_digest

coverage_record :=
    scrutinee_type_digest
 || domain_constructor_digest
 || bool(exhaustive)
 || seq<arm_coordinate || option<alternative_coordinate> || unreachable_reason_tag>
 || option<structured_witness_bytes>

match_record :=
    accepted_declaration_digest
 || stable_match_expression_coordinate
 || checked_match_expression_digest
 || match_payload_digest
```

The expression digest for a Match contains its `match_payload_digest`; it does
not merely hash scrutinee/guard/value child edges. The payload uses child
digests, so there is no self-reference. Nested Match payloads are completed
before their parents through the same memoized builder; a cycle rejects.

Coverage constructor bytes use the exact semantic tags/IDs, field type
digests, and symbolic sequence partitions defined in
`COVERAGE_ALGORITHM.md`. Structured witnesses use constructor tags and nested
witnesses, never source rendering. Coverage work statistics and configured
limits are excluded because changing a budget above the required work must not
change semantic identity.

The analyzer may compute a non-exhaustive `CheckedMatchCoverage` to produce its
structured error, but only exhaustive coverage publishes a `CheckedMatch` into
`FinalSemanticAnalysis`. A non-exhaustive nested Match therefore rejects its
enclosing transcript. Unreachable rows remain part of the exhaustive
transcript because they are source-order semantic diagnostics.

## Completeness and rejection

The writer has exhaustive matches over all 38 expression shapes, 27 expression
resolutions, 8 value resolutions, 7 select resolutions (one deletion-only
rejecting family), 13 pattern shapes, 5 pattern resolutions, and 35 statement
shapes. No wildcard success arm is permitted. Adding a family produces a
compile error until an owner tag and exact atoms are selected.

The only successful semantic inputs are checked typed atoms. The following
always reject: missing owner row, stale path, duplicate path, recovery/poison,
invalid literal, unresolved record field/case/entity, unsupported type domain,
limit overflow, and non-exhaustiveness. There is no `UnsupportedIdentity`
success downgrade, source-string reconstruction, legacy branch, whole-catalog
seal, or alternate version.
