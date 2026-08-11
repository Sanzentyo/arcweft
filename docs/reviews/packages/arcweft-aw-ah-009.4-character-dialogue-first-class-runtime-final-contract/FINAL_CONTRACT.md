# Normative final contract

## 1. Scope and final outcome

This contract implements the source request without reopening established
character registration, accepted-HIR lifecycle, shared callable resolution,
ordinary call groups, ordinary function/currying, bracket parsing substrate,
dialogue content parsing, line plans, native View presentation, or typed stream
work.

The selected model is:

```text
RUNTIME_VALUE
```

`CharacterDialogue` is a genuine immutable nominal runtime value. It is not a
compiler-only preset and is not an ordinary function closure. The complete
source/runtime chain is:

```text
Ref<Character>
  -- (CharacterDialoguePatch) -->
CharacterDialogue
  -- (CharacterDialoguePatch) -->
CharacterDialogue
  -- [DialogueContent] / : DialogueContent -->
DialogueLine
  -- optional with-plan -->
line execution and plan result
```

The following are never used as identity:

```text
source alias
local variable name
callee display label
source spelling
`.say` suffix
generated line-ID segment
character display name
```

The selected `RUNTIME_VALUE` model accepts:

```arcw
fn phone(character: Ref<Character>) -> CharacterDialogue {
    character(view = @view.PhoneMessage, voice = auto)
}

let selected =
    if state.mobile {
        phone(alice)
    } else {
        bob(view = @view.MainDialogue)
    }

let remembered = Some(selected)
let render_later = || selected

render_later()[
    動的な分岐の後でも表示できる。[p]
]
```

It also permits a `CharacterDialogue` in ordinary supported records,
`Option`, `Result`, sequences, function parameters/returns, and closure
captures, subject to the limits in `TOOLING_DIAGNOSTICS_LIMITS.md`.

The rejected `PROVEN_STATIC_ELIMINATION` model would require rejecting or
special-proving the branch, return, `Option`, closure capture, and indirect
content application above. It would reproduce the current split between simple
compiler presets and string callees. It is therefore rejected.

## 2. Preserved substrate and one required substrate correction

The following substrate MUST be reused:

- `arcweft-character::CharacterId` and its manifest-backed nominal inventories;
- accepted project/HIR snapshots and exact source identity;
- AW-AH-009 registration, source index, alias diagnostics, signature help, and
  shared callable catalog/resolver;
- ordinary call syntax and exact `ArgumentListSyntax`;
- existing generic functions, closures, partial application, curried call
  groups, runtime `Function` values, and AWBC function apply;
- existing dialogue-content parser, RichText source model, line-plan model,
  effects, cancellation, and dialogue state transition machinery;
- persistent authored View mounts and typed View IDs;
- the layer direction
  `syntax -> HIR -> sema -> runtime-plan/verify -> tooling`.

The one generic substrate correction is mandatory:

```rust
// arcweft-core::value

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordValue {
    type_id: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    fields: Vec<RuntimeValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeValue {
    // existing variants remain
    Record(Vec<RuntimeFieldValue>),
    NominalRecord(RuntimeNominalRecordValue),
    Function(RuntimeFunctionValue),
    // ...
}
```

`RuntimeValue::Record` remains the anonymous structural record. A nominal AWBC
record with a `public_id` MUST decode to `NominalRecord`; it MUST NOT collapse to
`Record`. Field order is exactly the defining AWBC record schema order. The
runtime value has no duplicate-field representation.

This correction is required because current AWBC record types carry nominal
identity while the current runtime record carrier does not. No ordinary
function/currying behavior changes.

## 3. Crate ownership

### `arcweft-character`

Owns and continues to validate:

```rust
CharacterId
CharacterLookId
CharacterPartId
CharacterVariantId
CharacterManifest
```

No dialogue crate may duplicate or parse these identities.

### `arcweft-dialogue`

Becomes the Sans-I/O domain owner of:

```rust
CharacterDialogue
CharacterDialogueConfig
CharacterDialoguePatch
CharacterDialogueContentApplication
CharacterDialogueValue
CharacterDialogueContractIdentity
CharacterDialogueVoice
DialogueLocaleId
CharacterDialogueCustomFieldId
CharacterDialogueCustomValue
CharacterDialogueStageValue
CharacterDialoguePortraitValue
CharacterDialogueFocusValue
CharacterDialogueCleanupValue
CharacterDialogueHookValue
CharacterDialogueStyleValue
CharacterDialogueRichTextValue
InlineFailurePolicy
InlineFallback
FallbackStylePolicy
```

The existing inline-failure policy types move directly from
`arcweft-render-text` to `arcweft-dialogue`; every consumer updates in the same
cut. There is no re-export or compatibility module.

`arcweft-dialogue` may depend on `arcweft-character`, `arcweft-core`,
`arcweft-id`, `arcweft-ref`, `arcweft-source`, and `arcweft-view`. None of those
crates depends on `arcweft-dialogue`, so the direction is acyclic. It remains
Sans I/O.

### `arcweft-lang-syntax`

Owns only CST/AST, delimiter/range recovery, dialogue content syntax, and line
plan syntax. It does not resolve a character.

### `arcweft-lang-hir`

Owns structured content-application meaning and exact source identity. It
carries target expressions, not callee strings.

### `arcweft-lang-sema`

Owns `TypeKind::CharacterDialogue`, the typed custom-field registry, dependent
look typing, checked patch facts, checked content-application facts, and shared
resolver publication.

### `arcweft-runtime-plan` / `arcweft-verify`

Own checked executable construction, immutable patching, content application,
line result obligations, and typed source-to-AWBC lowering.

### `arcweft-core::awbc`

Owns the runtime nominal record representation, CharacterDialogue opcodes,
verification, VM execution, fiber snapshots, and codec.

### `arcweft-render-text`

Consumes effective typed dialogue configuration and static content. It no
longer owns speaker identity, preset inheritance, or inline-failure domain
policy.

### `arcweft-runtime-driver`

Owns dialogue state transitions, accepted generation validation, save/restore,
replay, hot reload, and construction of runtime display frames. It does not
parse source or reconstruct `.say`.

### `arcweft-tooling` / `arcweft-lsp`

Consume accepted syntax/HIR/sema facts. They do not infer identity from source
text.

## 4. Exact domain shapes

```rust
// arcweft-dialogue

#[derive(Clone, Debug)]
pub struct CharacterDialogue {
    character: CharacterId,
    layout: TypeLayoutHash,
    contract: CharacterDialogueContractIdentity,
    config: CharacterDialogueConfig,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueContractIdentity {
    character_manifest: RuntimeValueDigest,
    defaults: RuntimeValueDigest,
    custom_schema: RuntimeValueDigest,
    view_contracts: RuntimeValueDigest,
}

#[derive(Clone, Debug)]
pub struct CharacterDialogueConfig {
    voice: Option<CharacterDialogueVoice>,
    look: Option<CharacterLookId>,
    stage: Option<CharacterDialogueStageValue>,
    portrait: Option<CharacterDialoguePortraitValue>,
    focus: Option<CharacterDialogueFocusValue>,
    cleanup: Option<CharacterDialogueCleanupValue>,
    view: ViewId,
    source_locale: Option<DialogueLocaleId>,
    hooks: Vec<CharacterDialogueHookValue>,
    style: CharacterDialogueStyleValue,
    rich_text: CharacterDialogueRichTextValue,
    inline_failure: InlineFailurePolicy,
    custom: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomValue>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CharacterDialoguePatch {
    voice: PatchField<CharacterDialogueVoice>,
    look: PatchField<CharacterLookId>,
    stage: PatchField<CharacterDialogueStageValue>,
    portrait: PatchField<CharacterDialoguePortraitValue>,
    focus: PatchField<CharacterDialogueFocusValue>,
    cleanup: PatchField<CharacterDialogueCleanupValue>,
    view: PatchField<ViewId>,
    source_locale: PatchField<DialogueLocaleId>,
    hooks: PatchField<Vec<CharacterDialogueHookValue>>,
    style: StructuredPatch<CharacterDialogueStyleValue>,
    rich_text: StructuredPatch<CharacterDialogueRichTextValue>,
    inline_failure: PatchField<InlineFailurePolicy>,
    custom: BTreeMap<CharacterDialogueCustomFieldId, PatchField<CharacterDialogueCustomValue>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PatchField<T> {
    #[default]
    Unspecified,
    Set(T),
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructuredPatch<T> {
    clear_all: bool,
    assignments: BTreeMap<RuntimeFieldPath, PatchField<RuntimeValue>>,
    marker: PhantomData<fn() -> T>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFieldPath(Vec<u16>);

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueTypedValue {
    nominal_type: Option<RuntimeNominalTypeId>,
    layout: TypeLayoutHash,
    value: RuntimeValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueStageValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialoguePortraitValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueFocusValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueCleanupValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueHookValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueStyleValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRichTextValue(CharacterDialogueTypedValue);
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueCustomValue(CharacterDialogueTypedValue);

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    nominal_type: Option<RuntimeNominalTypeId>,
    layout: TypeLayoutHash,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldCatalog {
    digest: RuntimeValueDigest,
    fields: BTreeMap<
        CharacterDialogueCustomFieldId,
        CharacterDialogueRuntimeCustomFieldDescriptor,
    >,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterDialogueContentApplication {
    dialogue: CharacterDialogue,
    line: RuntimeLineId,
    text_key: TextKey,
    content: DialogueContent,
    plan: LinePlan,
    source: SourceAnchor,
}

#[derive(Clone, Debug)]
pub struct CharacterDialogueValue {
    record: RuntimeNominalRecordValue,
    dialogue: CharacterDialogue,
}
```

`RuntimeFieldPath` is a schema ordinal path, not a source name path. Sema maps
authored style fields to ordinals before runtime-plan lowering.

`CharacterDialogueTypedValue` is the single lower-layer carrier for closed
runtime-typed option payloads. `nominal_type` is `Some` for nominal values and
`None` for structural/primitive values; `layout` is always required and is the
stable exact type identity. Its validating constructors reject non-finite
numbers, noncanonical records/maps, wrong nominal identities, and values that
do not match the declared layout. The role-specific newtypes prevent a stage,
hook, style, or custom value from being interchanged merely because their
runtime shapes happen to match.

The semantic custom-field registry remains in `arcweft-lang-sema`. The runtime
schema consumes only the lower-layer
`CharacterDialogueRuntimeCustomFieldCatalog`; therefore `arcweft-dialogue`
does not depend on sema or an accepted-environment type.

`CharacterDialogue` implements `PartialEq`, `Eq`, and `Hash` manually from the
canonical bytes specified in section 12. `CharacterDialogueConfig` is compared
only through that enclosing implementation. The implementation must not derive
those traits through raw `RuntimeValue` floating-point equality.

`CharacterDialoguePatch` is not a standalone author-constructible prelude type.
It is an owned checked operand constructed from one parenthesized argument list
and used by the Rust domain API, runtime-plan, verifier, and AWBC. It has no
general source literal, no variable declaration syntax, and no save
representation of its own.

`CharacterDialogueValue` is the only runtime carrier. Its validating decoder is
context-owned:

```rust
pub struct CharacterDialogueRuntimeSchema<'a> {
    character_catalog: &'a CharacterCatalog,
    view_catalog: &'a ViewRegistry,
    custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
    expected_layout: TypeLayoutHash,
}

impl CharacterDialogueRuntimeSchema<'_> {
    pub fn decode(
        &self,
        value: &RuntimeNominalRecordValue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError>;

    pub fn encode(
        &self,
        value: &CharacterDialogue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError>;
}

impl CharacterDialogueValue {
    pub fn dialogue(&self) -> &CharacterDialogue;
    pub fn record(&self) -> &RuntimeNominalRecordValue;
    pub fn into_runtime_value(self) -> RuntimeValue;
}
```

There is no context-free `TryFrom<RuntimeValue>` that would skip manifest, View,
or custom-field validation.

## 5. Nominal runtime layout

The public nominal type ID is:

```text
std.character_dialogue
```

The canonical record fields are exactly:

| Ordinal | Field | Runtime shape |
|---:|---|---|
| 0 | `character_id` | `EntityRef`, validated as `CharacterId` |
| 1 | `character_manifest_digest` | dense `u8[32]` |
| 2 | `defaults_digest` | dense `u8[32]` |
| 3 | `custom_schema_digest` | dense `u8[32]` |
| 4 | `view_contracts_digest` | dense `u8[32]` |
| 5 | `voice` | `Option<DialogueVoice>` |
| 6 | `look` | `Option<String>`, validated as `CharacterLookId` of ordinal 0 |
| 7 | `stage` | `Option<DialogueStage>` |
| 8 | `portrait` | `Option<DialoguePortrait>` |
| 9 | `focus` | `Option<DialogueFocus>` |
| 10 | `cleanup` | `Option<DialogueCleanup>` |
| 11 | `view` | `EntityRef`, validated as `ViewId`; never absent |
| 12 | `source_locale` | `Option<String>`, validated/canonicalized as `DialogueLocaleId` |
| 13 | `hooks` | `Seq<DialogueHook>` in execution order |
| 14 | `style` | nominal `RichTextStyle` value |
| 15 | `rich_text` | nominal `RichTextStyle` value |
| 16 | `inline_failure` | nominal `InlineFailurePolicy` value |
| 17 | `custom` | sorted `Seq<CharacterDialogueCustomEntry>` |

`CharacterDialogueCustomEntry` is a nominal record with the exact fields:

```text
0 field_id: String/PublicId
1 declared_nominal_type: Option<RuntimeNominalTypeId>
2 declared_layout: TypeLayoutHash
3 value: Dynamic
```

Entries are sorted by `field_id`. `declared_nominal_type` is absent for
structural or primitive values and present for nominal values. Duplicate IDs,
wrong declared types/layouts, or noncanonical order are rejected.

The nominal record's `layout` must equal the canonical hash of this exact type
and its transitive field types. The four contract digests are value fields and
participate in equality, hashing, save validation, and hot-reload stale checks.

## 6. Construction and immutable patching

### Character factory

Calling a `Ref<Character>`:

1. evaluates the character reference exactly once;
2. validates the resulting `CharacterId`;
3. looks up the immutable compiled `CharacterDialogueDefaults` record for that
   character in the current accepted generation;
4. evaluates patch expressions in authored argument order;
5. validates every patch field and limit without mutating any live value;
6. applies the complete patch to a fresh config;
7. validates required View/custom/manifest contracts;
8. returns one `CharacterDialogueValue`.

An empty call returns the defaults snapshot and emits no dialogue line.

### Reconfiguration

Calling a `CharacterDialogue` with parentheses:

1. evaluates the base value once;
2. decodes/validates the nominal value under the active generation;
3. evaluates the new patch in authored order;
4. applies it to a cloned config;
5. recomputes canonical bytes and value digest;
6. returns a new value.

The original value is unchanged on success or failure.

### Clear semantics

`None` in a configuration slot maps to `PatchField::Clear`.

- For optional scalar fields, `Clear` produces `None`.
- For `view`, `Clear` produces the required reserved `std.view.dialogue`.
- For hooks, `Clear` produces an empty list.
- For style/rich-text, `Clear` produces the empty structured value; renderer
  engine defaults still apply outside the config.
- For inline failure, `Clear` produces `InlineFailurePolicy::FailLine`.
- For a custom key, `Clear` removes that key.

Clear is a tombstone over all prior selected defaults, character defaults, and
earlier patches. It does not reveal a previously shadowed value. A later `Set`
may set a new value.

### Transactionality and evaluation order

Patch argument expressions are evaluated left to right in source order.
Duplicate coordinates are diagnosed before executable lowering. Runtime
validation occurs before commit. A failed field, budget, type, manifest, View,
or limit check produces no partially patched value and no presentation effect.

Construction and reconfiguration are pure. Content application owns the
existing dialogue presentation/effect boundary.

## 7. Application-only line fields

`id` and `text_key` are not reusable configuration fields. Their values must
be compile-time-resolved entity references; dynamic string or runtime entity
expressions are rejected because line/content catalogs require one static
source-site identity. They are accepted only in the outermost parenthesized call that is the direct target of a bracket
or colon content application:

```arcw
alice(id = @say.opening.001, text_key = @text.opening.001)[
    ...
]
```

The following is rejected:

```arcw
let configured = alice(id = @say.opening.001)
```

The following is accepted because the application-only field is in the outer
call adjacent to content:

```arcw
let configured = alice(view = @view.PhoneMessage)
configured(id = @say.opening.001)[
    ...
]
```

In a chain, only the outermost call may contain `id`/`text_key`:

```arcw
alice(look = smile)(id = @say.opening.001)[ ... ]  // accepted
alice(id = @say.opening.001)(look = smile)[ ... ]  // rejected
```

`id = None` requests generated line identity. `text_key = None` requests
derivation from the final line ID.

## 8. Semantic type

```rust
// arcweft-lang-sema::types

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueCharacterType {
    Exact(CharacterId),
    Any,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueType {
    character: CharacterDialogueCharacterType,
}

pub enum TypeKind {
    // ...
    CharacterDialogue(CharacterDialogueType),
    DialogueLine(Box<TypeKind>),
    // Speaker and SpeakerPreset are deleted.
}
```

Rules:

- a directly resolved character reference produces
  `CharacterDialogue<Exact(character)>`;
- a dynamic `Ref<Character>` produces `CharacterDialogue<Any>`;
- reconfiguration preserves the payload exactly;
- joining the same exact character preserves `Exact`;
- joining different exact characters yields `Any`;
- joining `Any` with exact yields `Any`;
- `Exact` is assignable to `Any`; `Any` is not assignable to `Exact` without an
  explicit checked character match;
- generic parameters may bind either form through ordinary generic rules;
- no implicit conversion exists between `CharacterDialogue` and a function
  type;
- no `Speaker` union/type alias exists.

For `CharacterDialogue<Any>`, a contextual `.smile` shorthand is rejected
because no unique character look family exists. A fully typed look value is
accepted and runtime validation confirms that its owner equals the value's
runtime `CharacterId`.

## 9. DialogueLine execution result

`CharacterDialogue[DialogueContent]` and the colon form create a checked
`DialogueLine<R>` operation. `R` is:

- `Unit` for no line plan or a plan without `out`;
- the common `out` type of all completing plan/cancellation branches;
- an existing `Result<R, LineCancel>` shape when try-line semantics require it.

`DialogueLine<R>` is a non-escaping compiler/runtime-plan operation type. It is
consumed by line execution lowering and is not a `RuntimeValue`, collection
element, closure capture, save value, or host payload. This preserves existing
dialogue suspension, scoped handle, cancellation, and `out` behavior without
inventing a second first-class line descriptor.

The result visible to the author after execution is `R`, not a stored
`DialogueLine`.

## 10. Shared callable families

```rust
pub enum DialogueCallableId {
    CharacterFactory,
    CharacterReconfigure,
    ContentApplication,
    ContentCall,
}

pub enum DialogueCalleeIdentity {
    Character {
        character: CharacterDialogueCharacterType,
    },
    CharacterDialogue {
        character: CharacterDialogueCharacterType,
    },
    Content {
        path: CallablePath,
    },
}
```

There are exactly three CharacterDialogue source operations:

1. parenthesized `CharacterFactory`;
2. parenthesized `CharacterReconfigure`;
3. bracket/colon `ContentApplication`.

They are published through the existing shared callable catalog and resolved
transactionally. Parenthesized operations use the accepted exact
`ArgumentListSyntax`; content application uses the explicit dialogue-content
surface described in `GRAMMAR_HIR_SEMA.md`.

`look` uses a dependent schema coordinate
`CharacterLookOfDialogueTarget`. It is exact for an exact character and
runtime-owner-checked for `Any`.

There is no generic function curry state, `RuntimeFunctionValue`, or
`SignaturePartialCall` for CharacterDialogue. `alice()` is a complete
CharacterDialogue value, not an incomplete function.

## 11. Line identity

The `@say.*` family is retained strictly as the stable dialogue-line entity
namespace. It has no relationship to a method named `say`.

Generated IDs are source-site identities:

```text
flow site:
say.flow.<complete flow-id body>.<named scopes...>.<generated ordinal>

callable site:
say.fn.<package segments>.<canonical module segments>.<owner family>
       .<owner path...>.<callable name>.<named scopes...>.<generated ordinal>
```

Rules:

- the generated ordinal counts generated content applications only within the
  exact prefix, in accepted source order;
- it is zero-padded to a minimum of three decimal digits and is not capped at
  999;
- explicit relative `@.suffix` and family-relative `@say:.suffix` append the
  authored suffix after applying only named-scope parent traversal;
- explicit absolute IDs must be valid `@say.*`;
- a generated or explicit duplicate anywhere in the accepted project is an
  error; the compiler never silently skips to another ordinal;
- generated IDs contain no character segment and work for
  `CharacterDialogue<Any>`;
- consumers never parse a line ID to recover a CharacterId;
- absent `text_key` derives `text.` plus the complete line-ID body after
  `say.`;
- explicit text keys must be `@text.*`;
- a character rename does not rename an explicit line ID and does not alter a
  generated source-site line ID;
- line-ID rename is owned by the line-reference inventory, independently of
  character rename.

A content application outside a typed flow or callable owner must provide an
absolute `@say.*` ID.

## 12. Runtime equality, hashing, capture, and effects

CharacterDialogue canonical bytes include:

```text
nominal type ID
layout hash
CharacterId
four contract digests
every config field in fixed ordinal order
hooks in execution order
structured style leaves in field-ordinal order
custom entries in stable field-ID order
```

Source spans, aliases, local names, display names, accepted-generation ordinal,
and debug labels are excluded.

Config numeric values must be finite. Negative zero is normalized to positive
zero before encoding. Maps/records are canonicalized, and duplicate fields are
rejected. Equality compares canonical bytes. Hashing uses
`RuntimeValueDigest::from_bytes(blake3(canonical_bytes))`. Equal values have
equal hashes.

A CharacterDialogue may be captured by an ordinary runtime function exactly as
any other validated `RuntimeValue`. Existing closure capture and suspension
rules apply. It may cross await/yield only when the containing value/capture is
already permitted by those rules. It introduces no borrow escape.

Factory/reconfiguration are pure and consume only deterministic validation
budget. Content application has the existing dialogue presentation effects and
line-plan effects. No hidden I/O, manifest read, or filesystem lookup occurs at
runtime; all catalogs are bundle-owned immutable data.

## 13. AW-AH-009.4.1 handoff

AW-AH-009.4 fixes the runtime/presentation payload:

```rust
pub struct DialoguePresentationCharacter {
    pub id: CharacterId,
    pub display_name: String,
}
```

`LineDisplayFrame` and Agent observation use this typed payload. Display name is
resolved once from the accepted character catalog and active locale; renderers
must not fall back from a missing label to a callee string.

AW-AH-009.4.1 alone defines the final authored View path
`dialogue.character.*`. Until that sequential implementation lands, the current
internal View projection enum may consume `display_name`, but it is not an
identity source and must not retain `callee` or `speaker_label` in the runtime
wire. AW-AH-009.4.1 must not change `CharacterDialogue`, its nominal layout,
line identity, AWBC, save, replay, or hot-reload decisions.

## 14. Required direct deletion

The implementation MUST delete, not alias, every item in
`DELETION_MATRIX.md`. In particular:

```text
Character.say(...)
SpeakerPreset.say(...)
SpeakerPreset.call(...)
Speaker
SpeakerRef
SpeakerPreset
DialogueSpeakerPreset
SayOptions
DialogueLineBuilder::say()
TypeKind::Speaker
TypeKind::SpeakerPreset
DialogueCalleeIdentity::Speaker
DialogueCalleeIdentity::SpeakerPreset
DialogueCallableId::SpeakerLine
speaker_preset_chain
speaker_preset_from_let
DialogueSpeakerSlug
all `.say` suffix stripping or reconstruction
```

The final parser has no `.say` AST/HIR kind or dedicated removed-syntax
diagnostic. `.say` is parsed as an ordinary selected method and rejected by
normal method resolution because Character has no such method.

## 15. Explicit non-goals and prohibitions

- No production implementation is included in this archive.
- No compatibility shim, alias, deprecated method, dual reader, old enum tag,
  old save decoder, old AWBC decoder, or discarded dialogue-callee/debug trace decoder. The existing generic root replay v1 reader is preserved.
- No source-text source gate or spelling inventory test.
- No formatter-only `.say` migration.
- No CSS authoring path.
- No Takumi path.
- No redesign of ordinary function/curry/closure or typed Stream substrate.
- No redesign of Character registration/source index/signature-help substrate.
- No redesign of authored View projection beyond the exact 009.4.1 handoff.
