# Runtime-plan, AWBC, bundle, save, replay, hot-reload, and observation contract

## 1. Runtime-plan target shapes

```rust
pub enum RuntimeExpr {
    // existing variants
    MakeCharacterDialogue {
        character: Box<RuntimeExpr>,
        patch: RuntimeCharacterDialoguePatch,
    },
    PatchCharacterDialogue {
        base: Box<RuntimeExpr>,
        patch: RuntimeCharacterDialoguePatch,
    },
}

pub struct RuntimeCharacterDialoguePatch {
    fields: Vec<RuntimeCharacterDialoguePatchField>,
    source: Option<RuntimeSourceRange>,
}

pub struct RuntimeCharacterDialoguePatchField {
    coordinate: RuntimeCharacterDialogueFieldCoordinate,
    operation: RuntimePatchOperation,
    value: Option<RuntimeExpr>,
    source: Option<RuntimeSourceRange>,
}

pub enum RuntimePatchOperation {
    Set,
    Clear,
}

pub struct RuntimeDialogueContentApplication {
    dialogue: RuntimeExpr,
    line: RuntimeLineId,
    text_key: TextKey,
    content: RuntimeContentUnitId,
    line_task_group: RuntimeLineTaskGroupId,
    source: RuntimeSourceRange,
}
```

Patch fields are emitted in authored evaluation order. The verifier requires
unique coordinates. Final `CharacterDialogueConfig` encoding is canonical field
order.

A direct `Ref<Character>[content]` lowers to
`MakeCharacterDialogue { empty patch }` followed by content application. There
is no static callee string in runtime-plan.

## 2. Runtime-plan verifier obligations

The verifier MUST prove:

- factory target has type `Ref<Character>`;
- reconfigure target has type `CharacterDialogue`;
- patch coordinates match the selected CharacterDialogue schema;
- `Set` has one value and `Clear` has none;
- no patch coordinate repeats;
- no character coordinate exists;
- the application-only standard coordinates `id` and `text_key` are absent from reusable patches;
- look type is exact or owner-checked under `Any`;
- custom field ID/type/layout matches the accepted registry;
- content application target produces CharacterDialogue;
- line and text-key families are valid and project-unique;
- View is present after patch/default resolution;
- source maps for patch fields and content application are valid;
- patch work and encoded-size upper bounds can be proven or guarded.

Unsupported or missing proof produces structured `runtime.plan.lower` errors;
it never falls back to `RuntimeExpr::Call`, a string callee, or static preset
reconstruction.

## 3. AWBC version switch

The implementation atomically changes:

```text
AWBC_ABI_VERSION   = 2
AWBC_CODEC_VERSION = 8
```

Reasons:

- `RuntimeValue` gains a nominal-record discriminant;
- dialogue construction and immutable patch opcodes are added;
- the Dialogue terminator consumes a runtime value register;
- fiber/save validation gains CharacterDialogue obligations.

Only ABI 2 / codec 8 is accepted after the cut. There is no ABI-1/codec-7 reader
in the final code. This is a genuine wire change, not a compatibility version
for the discarded preset representation.

## 4. AWBC program tables

Add:

```rust
pub struct AwbcCharacterDialogueDefaultsId(pub u32);

pub struct AwbcCharacterDialogueDefaults {
    pub character: AwbcStringId,
    pub base_value: AwbcConstantId,
    pub character_manifest_digest: AwbcDigest,
    pub defaults_digest: AwbcDigest,
    pub custom_schema_digest: AwbcDigest,
    pub view_contracts_digest: AwbcDigest,
}

pub struct AwbcCharacterDialogueCustomField {
    pub id: AwbcStringId,
    pub value_type: AwbcTypeId,
    pub nominal_type: Option<AwbcStringId>,
    pub layout: AwbcDigest,
    pub clearable: bool,
    pub accepted_views: AwbcTableRange,
}
```

Program tables:

```rust
pub character_dialogue_defaults: Vec<AwbcCharacterDialogueDefaults>;
pub character_dialogue_custom_fields: Vec<AwbcCharacterDialogueCustomField>;
```

Ordering:

- defaults sorted by validated `CharacterId`;
- custom fields sorted by stable field ID;
- duplicate IDs rejected;
- `base_value` must be nominal `std.character_dialogue`, with field 0 matching
  `character`;
- every digest is exactly 32 bytes.

Runtime lookup is binary search or an immutable index built after validation.
It never uses source aliases.

## 5. AWBC instructions

Add exact opcodes:

```rust
pub enum AwbcOpcode {
    // existing opcodes
    MakeCharacterDialogue = 0x27,
    PatchCharacterDialogue = 0x28,
}

pub enum AwbcInstruction {
    // existing instructions
    MakeCharacterDialogue {
        dst: AwbcRegisterId,
        character: AwbcRegisterId,
        fields: AwbcTableRange,
    },
    PatchCharacterDialogue {
        dst: AwbcRegisterId,
        base: AwbcRegisterId,
        fields: AwbcTableRange,
    },
}

pub struct AwbcCharacterDialoguePatchField {
    pub coordinate: AwbcCharacterDialogueFieldCoordinate,
    pub operation: AwbcPatchOperation,
    pub value: Option<AwbcRegisterId>,
    pub source_map: Option<AwbcSourceMapId>,
}
```

The patch-field table preserves authored evaluation order. The codec validates:

- table range bounds;
- unique coordinates;
- Set/value and Clear/no-value pairing;
- source-map bounds;
- custom field index bounds;
- no character coordinate;
- maximum field count and work.

VM execution:

1. read/validate inputs without mutating `dst`;
2. evaluate/collect field values already present in registers;
3. decode base/default nominal record;
4. apply patch to a candidate;
5. validate the final candidate;
6. store only the accepted nominal value in `dst`.

## 6. Dialogue terminator and suspension

Change the terminator to:

```rust
pub enum AwbcTerminator {
    Dialogue {
        dialogue: AwbcRegisterId,
        content: AwbcContentUnitId,
        line_task_group: AwbcLineTaskGroupId,
        resume: AwbcResumePointId,
    },
    // ...
}
```

The register type must be the nominal `std.character_dialogue` type.

Change suspension state to:

```rust
pub enum FiberSuspensionReason {
    Dialogue {
        dialogue: RuntimeValue,
        content: AwbcContentUnitId,
        line_task_group: AwbcLineTaskGroupId,
    },
    // ...
}
```

Validation decodes `dialogue` through the active
`CharacterDialogueRuntimeSchema`, validates content/line-task IDs, and confirms
that the current program generation publishes matching contract digests.

The runtime-driver receives a decoded `CharacterDialogueValue`; it never
receives a callee label.

## 7. Runtime value validation

`validate_nested_runtime_value` adds `NominalRecord` traversal and retains the
existing maximum nesting depth of 64.

For a nominal record:

- type ID must exist in the AWBC type table;
- layout hash must match the exact transitive type layout;
- field count must be exact;
- every field must match the declared AWBC type;
- nested values must validate;
- CharacterDialogue-specific validation runs when type ID is
  `std.character_dialogue`;
- unknown nominal IDs are rejected unless their exact type is present in the
  current program.

A `std.character_dialogue` value must satisfy every domain invariant in
`FINAL_CONTRACT.md` and the limits file.

## 8. Static content catalog and runtime display frame

The current `LineDisplaySpec` mixes static content and dynamic
speaker/configuration. Split it.

Static:

```rust
pub struct DialogueContentSpec {
    pub line: RuntimeLineId,
    pub text_key: TextKey,
    pub content: RichTextDocument,
    pub inline_styles: Vec<RichTextStyleContribution>,
    pub source: ProductSourceRef,
}
```

Runtime:

```rust
pub struct LineDisplayFrame {
    pub line: RuntimeLineId,
    pub character: DialoguePresentationCharacter,
    pub text_key: TextKey,
    pub effective: CharacterDialoguePresentationConfig,
    pub text: String,
    pub base_styles: Vec<RichTextStyle>,
    pub style_contributions: Vec<RichTextStyleContribution>,
    pub nodes: Vec<RichTextNode>,
    pub display_map: RichTextDisplayMap,
    pub host_events: Vec<DialogueHostEvent>,
    pub inline_failures: Vec<InlineTextFailure>,
    pub unresolved: Vec<String>,
}

pub struct DialoguePresentationCharacter {
    pub id: CharacterId,
    pub display_name: String,
}

pub struct CharacterDialoguePresentationConfig {
    pub view: ViewId,
    pub voice: Option<CharacterDialogueVoice>,
    pub look: Option<CharacterLookId>,
    pub stage: Option<CharacterDialogueStageValue>,
    pub portrait: Option<CharacterDialoguePortraitValue>,
    pub focus: Option<CharacterDialogueFocusValue>,
    pub cleanup: Option<CharacterDialogueCleanupValue>,
    pub source_locale: Option<DialogueLocaleId>,
    pub hooks: Vec<CharacterDialogueHookValue>,
    pub inline_failure: InlineFailurePolicy,
    pub custom: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
    pub config_digest: RuntimeValueDigest,
}
```

`callee` and `speaker_label` are deleted. `RichTextCascadeLayer::SpeakerPreset`
is renamed directly to `CharacterDialogueConfig`.

Display-name resolution is:

```text
CharacterId + effective locale + accepted Character metadata
  -> required character_display_name
```

If metadata cannot produce a label, the character catalog supplies its
validated canonical fallback during compilation/publication. Renderers do not
derive a label from a callee string.

## 9. Bundle boundaries

### Character manifest

Remains the existing typed Character resource. The executable product records
its digest in every compiled CharacterDialogue default entry.

### AWBC

Carries the executable CharacterDialogue default/custom tables, nominal type
layout, construction/patch instructions, and runtime register at Dialogue
suspension.

### Display catalog

The existing compact `DisplayCatalogSection` changes atomically to an explicit
sole schema:

```rust
pub const DISPLAY_CATALOG_SCHEMA_VERSION: u16 = 2;

pub struct DisplayCatalogSection {
    pub schema_version: u16,
    pub dialogue: Vec<DialogueContentSpec>,
    pub image_objects: Vec<BundleImageObject>,
}
```

It carries static `DialogueContentSpec` only and does not duplicate dynamic
CharacterDialogue config. Canonical order is `(line, text_key)`; duplicate line
IDs or noncanonical order are rejected. The previous unversioned transcript is
not read after the cut. The enclosing `ProductResourceEnvelope` and
`ProductSectionCodecKind::DisplayCatalog` are preserved; only the typed
transcript contract changes.

### View resources

Remain the typed native View program/resources. CharacterDialogue stores a
validated `ViewId`; content application validates the View dialogue contract.

### Product cross-validation

Bundle validation must verify:

- every CharacterDialogue defaults entry references a present Character
  manifest;
- each default look belongs to that character;
- each default View exists and passes dialogue View contract validation;
- every custom field used by defaults has one descriptor and is accepted by
  the effective View;
- every Dialogue terminator content ID maps to one display spec with the same
  line ID/source revision;
- AWBC and display source maps refer to the same accepted source set;
- all table and encoded-size limits.

Failure rejects the whole candidate product. The previous accepted bundle
remains active.

## 10. Patch/hot-reload payload

There is no independent legacy CharacterDialogue patch wire. The existing
atomic product replacement carries a candidate AWBC program, display catalog,
Character manifests, View products, and source maps. They are validated as one
generation before publication.

No old Speaker/preset payload is decoded.

## 11. Save snapshot

The implementation atomically changes:

```text
BUNDLE_SESSION_SAVE_SCHEMA_VERSION = 2
```

Only schema 2 is accepted. There is no schema-1 reader after the cut.

A CharacterDialogue value is saved only through existing reachable
`RuntimeValue` locations:

- fiber registers;
- function captures;
- source/stream queues when their ordinary type rules permit it;
- returned values retained by the existing session model.

There is no second CharacterDialogue save table.

Save validation checks:

- bundle/artifact identity;
- AWBC ABI/codec;
- nominal type ID/layout;
- contract digests;
- CharacterId and manifest digest;
- View/custom schema references;
- all config fields/limits;
- runtime value nesting;
- canonical ordering.

A malformed value rejects the entire restore transaction. No live session state
is mutated.

## 12. Replay and debug trace

Current `RootReplayTraceV1` is a generic root-transition/external-outcome wire;
it has no dialogue callee, SpeakerPreset, or dialogue-display subrecord. Its
schema remains exactly:

```text
ROOT_REPLAY_SCHEMA_VERSION = 1
ROOT_REPLAY_ENGINE_IDENTITY = "arcweft.root-replay.v1"
```

No new dialogue-specific replay record is invented. When a
`CharacterDialogue` is reachable through an existing `RuntimePayload`, it is
encoded only through the new validated `RuntimeValue::NominalRecord` variant
and participates in the existing payload digest. Old traces that contain only
still-valid runtime values remain schema-1 traces; malformed or unknown runtime
value discriminants fail normal payload validation. There is no discarded
Speaker/preset reader because that model never had a typed root-replay
representation.

Debug traces that project active presentation state use the same typed fields
as `AgentObservedDialogue` below. They may include
`character_display_name` as debug presentation data, but identity is the tuple
`(line, character_id, character_dialogue_digest, view_id,
content_revision)`. No trace stores or accepts a source alias, `callee`,
`speaker`, or `speaker_label` as semantic identity.

## 13. Agent observation wire

Replace dialogue identity fields with one typed record:

```rust
pub struct AgentObservedDialogue {
    pub line: RuntimeLineId,
    pub character_id: CharacterId,
    pub character_display_name: String,
    pub view_id: ViewId,
    pub character_dialogue_digest: RuntimeValueDigest,
    pub content_revision: RuntimeValueDigest,
    pub stage: DialogueStageIndex,
    pub reveal_complete: bool,
    pub advance_available: bool,
}
```

No `callee`, `speaker`, `speaker_label`, or preset field remains in the
observation wire. Existing observation redaction rules apply to text content;
CharacterId and public display name remain public presentation metadata unless
a separate classification policy says otherwise.

## 14. Hot-reload contract

### Contract identity

A CharacterDialogue value is generation-portable only when the candidate
generation has exact equality for:

```text
CharacterId
character manifest digest
CharacterDialogue nominal layout hash
defaults digest
custom-field schema digest
all referenced View contract digests
```

The ephemeral generation number itself is not in language equality/hash.

### Rebinding

At an atomic replacement safe point:

- exact contract match: the value may be rebound to the new generation without
  byte changes;
- character manifest mismatch/missing: `AW-CD-R007`;
- defaults/config layout mismatch: `AW-CD-R008`;
- effective View missing/incompatible: `AW-CD-R009`;
- custom field schema mismatch: `AW-CD-R011`.

There is no field-by-field migration.

### Active line/content

A pending dialogue line also pins:

```text
RuntimeLineId
content revision digest
line-task-group digest
```

If those differ, the active line continues on the old retained generation.
It is never reinterpreted under new content. A forced cross-generation resume
is rejected with `AW-CD-R010`.

New content applications use the newly accepted generation.

### Failed replacement

Any CharacterDialogue validation failure rejects the candidate generation
before publication and leaves the previous runtime/catalog/View state intact.

## 15. Equality and stale detection

Two CharacterDialogue values are language/runtime-equal only when their
canonical nominal bytes are equal. Compatible hot reload does not rewrite the
value. A value with the same effective visual config but a different defaults
or manifest digest is not equal, because later validation and resource meaning
may differ.

## 16. No-representation inventory

| Boundary | Decision |
|---|---|
| CST/AST source | structured node/ranges, not serialized as runtime value |
| accepted HIR cache | in-memory typed nodes/facts only; no new external codec |
| sema query cache | in-memory generation-bound facts only |
| CharacterDialoguePatch save | no representation; patch is instruction operand |
| DialogueLine save as value | no representation; it is consumed executable operation |
| source labels/aliases | no runtime representation |
| old Speaker/preset model | no representation and no reader |
| CSS/Takumi | no representation |

## 17. Tamper behavior

The following are hard rejection before mutation:

- unknown/wrong nominal type ID;
- wrong layout hash;
- wrong field count/order/type;
- digest not exactly 32 bytes;
- invalid CharacterId/ViewId/custom ID/locale/look;
- duplicate/noncanonical custom entries;
- wrong custom declared type/layout;
- missing required View;
- malformed `Option`/enum discriminant;
- invalid Set/Clear operand shape;
- out-of-bounds table/register/source-map;
- truncated/oversized codec;
- unsupported ABI/codec/save/trace version;
- stale contract/content identity;
- any configured limit exceeded.

There is no best-effort field dropping.

## 18. Complete boundary inventory

| Boundary | Representation after this cut | Version/discriminant | Canonical order and limits | Tamper/stale behavior | Source provenance |
|---|---|---|---|---|---|
| syntax parse result | typed CST/AST application surfaces | no external codec | existing parser limits | malformed current grammar yields ordinary recovery | exact token/range ownership |
| accepted HIR query cache | `HirDialogueContentApplication` and source map | in-memory accepted generation only | existing accepted-HIR budgets | stale source revision/query generation rejected | exact `SourceDocument` spans |
| sema query cache | checked factory/reconfigure/application facts and custom registry | in-memory accepted world only | shared query budget | stale/missing accepted world yields no fact | primary/related `SourceSpan`s |
| runtime plan | typed Make/Patch/application nodes | in-memory; no external reader | patch/config limits | verifier fails closed; no string-call fallback | `RuntimeSourceRange` sidecar |
| generic runtime value | `RuntimeValue::NominalRecord` | new closed enum discriminant | nesting 64; canonical nominal field order | wrong type/layout/fields rejected | none in value identity |
| AWBC executable | CharacterDialogue types/defaults/custom tables, opcodes, Dialogue register | ABI 2 / codec 8 only | sorted tables; bounded ranges and config limits | old/wrong/truncated/duplicate/noncanonical rejected | AWBC source-map IDs |
| Character resource | existing manifest/catalog | existing current resource codec | existing Character budgets/order | missing/digest mismatch rejects product/value | existing manifest source map |
| display catalog | schema-2 `DisplayCatalogSection.dialogue` static specs | `schema_version = 2` only | `(line,text_key)` order; existing 16 MiB transcript budget plus line limits | unversioned/old/duplicate/noncanonical rejected | `ProductSourceRef` per spec |
| View resources | existing typed native View product | existing current View codecs | existing View budgets | missing/incompatible View rejects candidate/patch | existing View source ranges |
| atomic product patch | whole candidate AWBC/display/Character/View/source set | existing atomic replacement envelope | existing replacement budgets | any cross-section failure leaves old generation active | candidate source set |
| bundle session save | reachable generic `RuntimeValue` locations; no second table | `arcweft.bundle_session` schema 2 only | value/config/capture/container limits | schema1, malformed, stale, noncanonical reject whole restore | save values omit source spans |
| root replay | existing generic transition trace and `RuntimePayload` | root replay schema 1 retained | existing trace limits plus nominal payload limits | malformed nominal payload/digest divergence rejected | existing transition identity only |
| debug presentation trace | typed Character dialogue identity tuple | existing outer debug envelope; replaced dialogue payload | ID/string/config digest limits | old callee/speaker payload rejected by typed decoder | optional source-map reference outside identity |
| Agent observation | `AgentObservedDialogue` typed fields | existing response envelope; one final dialogue object shape | ID/display string and observation budgets | missing required/new wrong fields or old callee fields rejected | observation may link existing source anchor |
| hot-reload retained value | unchanged canonical CharacterDialogue bytes plus contract identity | no migration version | exact digest/layout equality | no field migration; stale value/candidate rejected or old generation retained for active line | source maps belong to generations, not value |
| standalone `CharacterDialoguePatch` | no wire | none | not applicable | any attempted decoder is a contract violation | checked patch spans only |
| standalone `DialogueLine` value | no wire | none | not applicable | escape is compile/lowering error | application/plan spans |
| source alias/callee/display spelling | no semantic wire | none | not applicable | never accepted as identity | tooling presentation only |
| Speaker/preset discarded model | no representation | no reader | not applicable | old API/bytes/payloads rejected | none |
| CSS/Takumi route | no representation | none | not applicable | dependency/architecture gate fails if introduced | none |

This table is exhaustive for current-main boundaries found during the inspection.
A later implementation that discovers an additional persisted or transport
boundary must stop that coherent cut, record the concrete boundary in the
implementation note, and apply the same already-frozen rule: one typed
CharacterDialogue representation or explicit no-representation. It may not
reopen the runtime model, line family, merge rules, or add a compatibility
reader.
