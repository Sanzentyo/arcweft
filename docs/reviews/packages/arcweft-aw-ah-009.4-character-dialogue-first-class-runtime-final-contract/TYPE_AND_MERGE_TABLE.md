# Normative source, type, configuration, and merge table

## 1. Source operations

| Source operation | Checked operation | Result | Effect |
|---|---|---|---|
| `character()` | Character factory with empty patch | `CharacterDialogue<Exact/Any>` | pure |
| `character(patch...)` | Character factory | `CharacterDialogue<Exact/Any>` | pure |
| `dialogue(patch...)` | immutable reconfiguration | same `CharacterDialogue` type | pure |
| `dialogue[content]` | content application | `DialogueLine<R>` | dialogue execution |
| `dialogue: content` | colon surface for same application | `DialogueLine<R>` | dialogue execution |
| `... with { plan }` / `with:` | attach one line plan | user result `R` | line-plan effects |

A direct bracket target of type `Ref<Character>` is semantically an empty
factory followed by content application. It does not create a separate path.

The first positional parenthesized argument remains shorthand for `look`.
Every other standard field is named-only. A second positional argument is an
error.

## 2. Patch syntax and tri-state

For every clearable field:

```text
field omitted  -> Unspecified
field = value  -> Set(value)
field = None   -> Clear
```

`Some(value)` is not required and is not the canonical spelling. If it appears
in an expression position where the field expects `Option<T>`, ordinary
expected-type rules apply, but tooling canonicalizes direct field values rather
than adding `Some`.

A field may occur at most once in one argument list. Later-wins semantics apply
between successive CharacterDialogue patches, not between duplicate entries in
one patch.

The reserved names are:

```text
id
text_key
voice
look
stage
portrait
focus
cleanup
view
source_locale
hooks
style
rich_text
inline_error
inline_error_policy
inline_fallback
character
character_id
content
```

The three inline-failure spellings map to the same `inline_failure` coordinate.
Using more than one in the same patch is a conflicting duplicate.

`character`, `character_id`, and `content` are never custom fields.

## 3. Exact field table

| Field | Source expected type | Domain owner/type | Placement | Default/absent | Explicit clear | Patch merge | Validation and provenance | Wire and limit |
|---|---|---|---|---|---|---|---|---|
| `id` | `Ref<DialogueLine>` in `@say.*` | HIR `RuntimeLineId` | content application only | generated source-site ID | request generated ID | no reusable merge | syntax range; HIR family validation; project collision index | static content spec; max 256 UTF-8 bytes |
| `text_key` | `TextKey` / `Ref<Text>` in `@text.*` | `arcweft-id::TextKey` | content application only | derive from final line ID | request derivation | no reusable merge | exact argument range; family validation | static content spec; max 256 bytes |
| `voice` | `DialogueVoice` | `arcweft-dialogue::CharacterDialogueVoice` | reusable | `None` | `None` | later `Set` replaces, `Unspecified` preserves | sema checks `auto` or typed voice ID; field source span | nominal field 5; ID max 256 bytes |
| `look` | dependent `CharacterLookOfDialogueTarget` | `arcweft-character::CharacterLookId` | reusable | `None` | `None` | replace/preserve | exact owner checked in sema or at runtime for `Any`; manifest declaration span retained | nominal field 6; local ID max 128 bytes |
| `stage` | nominal `DialogueStage` | `CharacterDialogueStageValue` | reusable | `None` | `None` | replace/preserve | shared resolver expected type; AWBC layout validation; source span | nominal field 7; encoded value max 64 KiB |
| `portrait` | nominal `DialoguePortrait` | `CharacterDialoguePortraitValue` | reusable | `None` | `None` | replace/preserve | shared resolver expected type; source span | nominal field 8; max 64 KiB |
| `focus` | nominal `DialogueFocus` | `CharacterDialogueFocusValue` | reusable | `None` | `None` | replace/preserve | typed focus value and required effect/lifetime guarantees; source span | nominal field 9; max 64 KiB |
| `cleanup` | nominal `DialogueCleanup` | `CharacterDialogueCleanupValue` | reusable | `None` | `None` | replace/preserve | typed cleanup policy; line-plan compatibility; source span | nominal field 10; max 64 KiB |
| `view` | `Ref<View>` | `arcweft-view::ViewId` | reusable, runtime-required | resolved selected/character/standard default | `std.view.dialogue` | replace/preserve | accepted View registry and dialogue contract; source span | nominal field 11; ID max 256 bytes |
| `source_locale` | canonical BCP-47 string | `arcweft-dialogue::DialogueLocaleId` | reusable | active runtime locale (`None` override) | `None` | replace/preserve | compile-time literal validation when known, runtime validation otherwise; source span | nominal field 12; max 64 bytes |
| `hooks` | `Seq<DialogueHook>` or one value normalized to a sequence | `Vec<CharacterDialogueHookValue>` | reusable | selected/character defaults or empty | empty list | a specified list replaces the complete prior list; no implicit append | each hook typed and effect-checked; authored order/provenance retained | nominal field 13; max 64 hooks, 256 KiB aggregate |
| `style` | `StyleRef \| RichTextStyle` | `CharacterDialogueStyleValue` | reusable | empty/selected style value | empty value | field-by-field structured merge; same leaf later wins | names resolve to typed style owner; every leaf retains source range and schema ordinal | nominal field 14; depth 8, 256 leaves, 256 KiB |
| `rich_text` | `RichTextStyle` | `CharacterDialogueRichTextValue` | reusable | empty/selected value | empty value | field-by-field structured merge; same leaf later wins | exact field ordinal path and source range; invalid leaf/type is diagnostic | nominal field 15; shared style limits |
| inline failure | `InlineFailurePolicy` | types moved to `arcweft-dialogue` | reusable | `FailLine` after all defaults | `FailLine` | replace/preserve | constructor/variant checked by sema; exact source range | nominal field 16; fallback text max 16 KiB |
| custom named argument | type from immutable custom-field registry | `CharacterDialogueCustomFieldId` + `CharacterDialogueCustomValue` | reusable | absent key | remove key | distinct keys preserved; same key later replaces | source name resolves to stable field ID/type/declaration; no string-typed fallback | sorted entries in ordinal 17 (the eighteenth record field); max 32 keys, `character_dialogue_field.*` ID max 128 bytes, each value max 64 KiB |

## 4. CharacterDialogueVoice

The existing `VoicePolicy` is directly renamed and narrowed to the character
dialogue role:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDialogueVoice {
    Auto,
    Id(CharacterDialogueVoiceId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CharacterDialogueVoiceId(PublicId);
```

`CharacterDialogueVoiceId` requires the `voice.` entity family. TTS/provider
speaker identity remains a separate audio binding and is not renamed to
CharacterDialogue.

## 5. Locale identity

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLocaleId(String);
```

Validation:

- ASCII BCP-47 syntax only at this boundary;
- language lower-case;
- script title-case;
- region upper-case;
- no empty or duplicate subtags;
- no control characters;
- 1..=64 UTF-8 bytes after canonicalization.

A dynamic invalid locale traps/rejects the patch transaction. It is not
silently retained as an arbitrary string.

## 6. Structured style merge

`style` and `rich_text` are typed structured values with schema-ordinal field
paths. Merge is:

```text
base leaf absent + patch Unspecified -> absent
base leaf value  + patch Unspecified -> base value
base leaf any    + patch Set(v)       -> v
base leaf any    + patch Clear        -> absent
clear_all=true                       -> start from empty, then apply assignments
```

Assignments in one patch have unique `RuntimeFieldPath`s. Parent and child
assignments that overlap are conflicting and rejected; there is no order-based
guess. For example, setting `rich_text.text` and
`rich_text.text.color` in the same patch is a conflict unless the schema
explicitly models the parent as a mergeable record assignment and the compiler
expands it into disjoint leaf assignments before validation.

Authored expression evaluation remains source ordered. The final structured
value is serialized by field ordinal.

## 7. Custom field registry

```rust
pub struct CharacterDialogueCustomFieldId(PublicId);

// arcweft-lang-sema
pub struct CharacterDialogueCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    bindings: Vec<CharacterDialogueCustomFieldBinding>,
    value_type: TypeKind,
    runtime_nominal_type: Option<RuntimeNominalTypeId>,
    runtime_layout: TypeLayoutHash,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
    declaration: SourceSpan,
}

pub struct CharacterDialogueCustomFieldRegistry {
    generation: AcceptedEnvironmentGeneration,
    by_id: BTreeMap<CharacterDialogueCustomFieldId, CharacterDialogueCustomFieldDescriptor>,
    bindings: BTreeMap<ProjectBindingPath, CharacterDialogueCustomFieldId>,
}
```

This descriptor and registry are owned by `arcweft-lang-sema`; the lower
`arcweft-dialogue` crate owns only `CharacterDialogueCustomFieldId`, typed
runtime values, and the runtime descriptor catalog defined in
`FINAL_CONTRACT.md`. The semantic registry is built transactionally with the
registered semantic world and uses the existing project binding and collision
rules. A local source name is
not stored in runtime config. The stable field ID, declared runtime type/layout,
and value are stored. Unknown or ambiguous names are diagnostics.

When the selected View changes, every retained custom field must be accepted by
the new View's dialogue contract. Failure rejects the patch; no key is silently
discarded.

## 8. Defaults and final value

A Character factory begins from the compiled effective defaults for the
runtime CharacterId:

```text
engine dialogue defaults
  -> selected project dialogue defaults
  -> character dialogue defaults
  -> factory patch
```

Successive CharacterDialogue patches then apply in source/runtime order.

The resulting `CharacterDialogueConfig` is an effective snapshot. It does not
retain a stack of source layers. Clear is a tombstone, not “reveal the previous
layer.” Contract digests retain the exact defaults and schema provenance needed
for hot-reload validation.

## 9. Application-only field context

The accepted outer AST relationship, not textual scanning, determines whether
`id`/`text_key` are legal. The shared resolver receives:

```rust
pub enum CharacterDialoguePatchContext {
    ReusableValue,
    ImmediateContentApplication,
}
```

`ImmediateContentApplication` is supplied only for the outermost call expression
that is the direct target of `DialogueContentApplication`. All nested calls use
`ReusableValue`.

## 10. Character identity immutability

There is no `character` member in `CharacterDialoguePatch`. Any authored
`character` or `character_id` argument resolves to the reserved-coordinate
diagnostic. A custom registry is forbidden from publishing either binding.

Reconfiguration preserves:

```text
CharacterDialogue.character
CharacterDialogue.contract.character_manifest
```

A caller that wants another character constructs another value from another
`Ref<Character>`.

## 11. Source provenance

Compile-time provenance is separate from runtime value identity:

```rust
pub struct CheckedCharacterDialoguePatchField {
    field: CharacterDialogueFieldCoordinate,
    operation: CheckedPatchOperation,
    value: Option<TypeExpressionId>,
    source: SourceSpan,
    inherited_from: Option<SourceSpan>,
}

pub struct CheckedCharacterDialoguePatch {
    context: CharacterDialoguePatchContext,
    fields: Vec<CheckedCharacterDialoguePatchField>,
    source: SourceSpan,
}
```

The final winner and any shadowed default contribution are available to hover,
cascade tooling, diagnostics, and source maps. They are not serialized into
`CharacterDialogueValue` or included in equality/hash.

## 12. Serialization summary

- `CharacterDialogue`: yes, as one `RuntimeValue::NominalRecord`.
- `CharacterDialogueConfig`: yes, as fixed fields 5..17 of that nominal record.
- `CharacterDialoguePatch`: no standalone saved value; encoded as typed AWBC
  instruction operands.
- `CharacterDialogueContentApplication`: static content/line-plan/source tables
  plus a runtime CharacterDialogue register at the Dialogue terminator.
- `id`/`text_key`: static application metadata, never reusable config.
- source provenance: AWBC/source-map sidecar only.
