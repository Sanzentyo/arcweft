# Tooling, diagnostics, and production limits

## 1. Formatter

`arcw fmt` remains syntax-only.

It MUST:

- format parenthesized configuration using ordinary call formatting;
- format bracket dialogue content without inserting `.say`;
- format colon dialogue content without semantic classification;
- format `with:` or `with {}` according to the existing formatter setting;
- preserve exact current-grammar tokens and comments.

It MUST NOT:

- expand or introduce `.say`;
- decide whether an ambiguous postfix bracket is dialogue by inspecting a name;
- load a project or sema inventory;
- act as a compatibility migrator.

## 2. Project-aware canonicalization

`arcw canonicalize` and the LSP code action use accepted source-revision-bound
`CheckedCharacterDialogueApplication` facts.

Canonical transformations:

```arcw
alice:
    text
```

to:

```arcw
alice[
    text
]
```

and:

```arcw
alice(look = worried):
    text
```

to:

```arcw
alice(look = worried)[
    text
]
```

It preserves comments, argument order, content bytes after existing dialogue
text canonicalization, and the attached line plan. It does not expand a
parenthesized reusable value or rewrite ordinary indexing.

If accepted semantic input is missing/stale/inconsistent, it returns a
structured no-edit or partial report. It never falls back to a name heuristic.

## 3. Completion

### Character/CharacterDialogue call

Inside `(` completion publishes:

- standard config fields valid in the current context;
- `id` and `text_key` only for the immediate outer content application;
- exact look variants for `CharacterDialogue<Exact(C)>`;
- typed custom fields visible in the accepted semantic world;
- `None` only for clearable fields;
- no `say` method.

### Bracket/colon content

Completion publishes dialogue tags, interpolation, RichText selectors, and
line-plan attachment. It does not list config fields inside content.

### Custom fields

Completion uses descriptor binding, documentation, declared type, clearability, accepted View contracts, and owning declaration. It does not discover custom
fields from the selected View source text.

## 4. Hover

Hover on a CharacterDialogue value shows:

```text
CharacterDialogue<character.alice>
view: view.PhoneMessage
configured: voice, look, ...
contract: <short digest>
```

For `Any`, it shows `CharacterDialogue<any character>`.

Hover may show source provenance and effective/shadowed config contributors
from checked facts. It must not show a callee/preset classification.

Hover on `character_display_name` is deferred to AW-AH-009.4.1's authored View
projection, but the runtime field is already fixed here.

## 5. Signature help

- `(` on Character: Character factory configuration.
- `(` on CharacterDialogue: immutable reconfiguration.
- `[` or colon: content application and `DialogueLine<R>`.
- `with`: line-plan result/effect/cancellation contract.
- exact active parameter is selected by accepted argument coordinates.
- custom fields display their declared type and definition source.
- no signature contains `.say`, `Speaker`, or `SpeakerPreset`.

## 6. Go-to-definition

- target Character/CharacterDialogue exact owner -> Character declaration or
  manifest source index;
- look -> exact Character look declaration;
- View -> View definition;
- style -> style declaration;
- custom field -> custom-field descriptor declaration;
- line ID -> content-application source site;
- text key -> localization/source declaration;
- dynamic `Any` target without an exact source owner returns a typed
  non-applicable result, not a first-match definition.

## 7. Rename

### Character

Character rename consumes the existing bounded typed character reference
inventory. It updates source references and manifests. Generated line IDs are
source-site based and do not change. Explicit `@say.*` IDs do not change.

### Line ID

Line rename consumes the line identity/reference inventory and changes only the
line ID and references. It never renames a Character.

### Custom field

Custom-field rename follows the stable descriptor/reference inventory. Runtime
field identity changes only through the atomic accepted registry/product
replacement; no alias is retained.

### Removed names

There is no rename route from `.say`, Speaker, or preset spellings because
those are not accepted declarations.

## 8. Semantic tokens

Required classifications:

| Surface | Token |
|---|---|
| Character target | entity/reference, character modifier |
| CharacterDialogue local | variable with nominal-dialogue modifier |
| standard config name | property with dialogue-config modifier |
| custom config name | property with custom-dialogue-field modifier |
| exact look variant | enum-member with character-owner modifier |
| `[`/`]` content delimiters | dialogue-content delimiter |
| colon content delimiter | dialogue-content delimiter |
| `with` | keyword, line-plan modifier |
| `@say.*` | entity-reference, dialogue-line modifier |
| `@text.*` | entity-reference, text-key modifier |

No token is classified as SpeakerPreset.

## 9. Code actions

Permitted actions:

- expand colon content syntax to bracket syntax;
- insert/qualify an explicit Character reference;
- qualify an ambiguous exact look;
- insert an explicit `@say.*` ID;
- replace a wrong custom field with an accepted descriptor binding;
- move `id`/`text_key` to the outer immediate application call;
- add `None` only where clear is permitted.

Prohibited actions:

- add `.say`;
- rewrite to Speaker/Preset APIs;
- add a compatibility import/alias;
- add a source spelling recognizer;
- silently delete an unknown custom field.

## 10. Stable compiler/sema diagnostics

| Code | Stable kind | Required fields and behavior |
|---|---|---|
| `AW-CD-001` | `NotCharacterFactory` | callee type, target range, expected `Ref<Character>` |
| `AW-CD-002` | `NotCharacterDialogueReconfigure` | callee type, target range, expected CharacterDialogue |
| `AW-CD-003` | `InvalidContentApplicationTarget` | target type/range; bracket/colon surface |
| `AW-CD-004` | `ImmutableCharacterIdentity` | reserved argument range; character target source |
| `AW-CD-005` | `DuplicateConfigField` | first and duplicate field ranges/coordinate |
| `AW-CD-006` | `ConflictingConfigField` | overlapping style paths or inline-failure aliases |
| `AW-CD-007` | `ApplicationOnlyField` | field name/range and required immediate-content context |
| `AW-CD-008` | `InvalidCharacterLook` | actual type/value, expected character owner, manifest source |
| `AW-CD-009` | `InvalidVoice` | actual type/value and voice declaration expectation |
| `AW-CD-010` | `InvalidView` | View ID/type/range and registry reason |
| `AW-CD-011` | `InvalidStructuredStyle` | schema field path, expected/actual, source range |
| `AW-CD-012` | `InvalidLocale` | value/range and BCP-47 reason |
| `AW-CD-013` | `InvalidLineIdFamily` | actual ID/range, expected `say` |
| `AW-CD-014` | `UnknownCustomField` | authored binding/range and current scope |
| `AW-CD-015` | `CustomFieldTypeMismatch` | stable field ID, declared/actual type, declaration and use ranges |
| `AW-CD-016` | `FieldNotClearable` | field ID/range and declaration |
| `AW-CD-017` | `DialogueLineEscape` | attempted storage/capture/return range |
| `AW-CD-018` | `CharacterDialogueTypeMismatch` | exact/any character payload mismatch |
| `AW-CD-019` | `AmbiguousPostfixBracket` | index and dialogue candidates plus ranges |
| `AW-CD-020` | `LineIdCollision` | ID and both source spans |
| `AW-CD-021` | `ConfigLimitExceeded` | limit kind, actual, allowed, aggregate call range |
| `AW-CD-022` | `CustomFieldViewIncompatible` | field ID, selected View, both declaration sources |
| `AW-CD-023` | `MissingDialogueDefaults` | runtime-dynamic Character path and accepted catalog generation |
| `AW-CD-024` | `InvalidPatchOperation` | coordinate, Set/Clear shape, source range |

All compile diagnostics are source-ranged. Related declarations are secondary
labels. Diagnostic ordering is:

```text
(primary start, primary end, code, stable identity fields)
```

`.say` has no dedicated diagnostic. It receives the ordinary current
method-resolution diagnostic.

## 11. Stable runtime/wire diagnostics

| Code | Stable kind | Rejection point |
|---|---|---|
| `AW-CD-R001` | `WrongNominalType` | runtime decode before field access |
| `AW-CD-R002` | `NominalLayoutMismatch` | AWBC/fiber/save validation |
| `AW-CD-R003` | `NoncanonicalNominalFields` | wrong count/order/duplicate custom entries |
| `AW-CD-R004` | `InvalidCharacterId` | defaults lookup/decode |
| `AW-CD-R005` | `InvalidConfigValue` | field-specific runtime validation |
| `AW-CD-R006` | `RuntimeLimitExceeded` | constructor/patch/codec/save |
| `AW-CD-R007` | `StaleCharacterManifest` | hot reload/restore |
| `AW-CD-R008` | `StaleDialogueDefaults` | hot reload/restore |
| `AW-CD-R009` | `StaleOrMissingView` | content application/rebind |
| `AW-CD-R010` | `StaleDialogueContent` | active-line resume/replay |
| `AW-CD-R011` | `StaleCustomFieldSchema` | patch/rebind/restore |
| `AW-CD-R012` | `MalformedCodec` | wrong discriminant/truncated/invalid table |
| `AW-CD-R013` | `UnsupportedWireVersion` | old/new unknown AWBC/save/trace version |
| `AW-CD-R014` | `DuplicatePatchCoordinate` | verifier/VM |
| `AW-CD-R015` | `InvalidPatchOperand` | Set without value or Clear with value |
| `AW-CD-R016` | `PatchBudgetExceeded` | VM before commit |
| `AW-CD-R017` | `GenerationContractMismatch` | execution under wrong accepted generation |
| `AW-CD-R018` | `InvalidLineIdentity` | content/spec/replay validation |
| `AW-CD-R019` | `InvalidCustomFieldValue` | custom field ID/type/layout/value validation |
| `AW-CD-R020` | `NoncanonicalConfigEncoding` | equality/hash/save decode |

Runtime errors include a typed owner path and AWBC source-map reference when
available. They do not expose source-label strings as identity.

## 12. Production limits

```rust
pub struct CharacterDialogueLimits {
    pub max_patch_fields: u16,                 // 64
    pub max_patch_work: u32,                   // 1024
    pub max_custom_fields: u16,                // 32
    pub max_custom_field_id_bytes: u16,        // 128
    pub max_hooks: u16,                        // 64
    pub max_config_string_bytes: u32,          // 16_384
    pub max_locale_bytes: u16,                 // 64
    pub max_structured_depth: u8,               // 8
    pub max_structured_leaves: u16,             // 256
    pub max_fx_applications: u16,               // 128
    pub max_field_value_bytes: u32,             // 65_536
    pub max_config_encoded_bytes: u32,          // 524_288
    pub max_values_per_sequence: u32,           // 4_096
    pub max_captured_values_per_function: u16,  // 256
    pub max_defaults_entries: u32,              // 4_096
    pub max_line_id_bytes: u16,                 // 256
}
```

The production constant has exactly the values in comments.

Additional inherited limit:

```text
runtime value nesting depth = 64
```

CharacterDialogue does not increase the existing general AWBC register, frame,
scope, source-map, table-index, or total bundle limits.

### Work accounting

Patch work units:

```text
1 base/default lookup
1 per standard Set/Clear field
1 per hook
1 per custom field
1 per structured leaf
1 per nested nominal value visited
```

Construction/patch fails before commit when work would exceed 1024.

### Collection/capture

A sequence or other collection may contain at most 4,096 CharacterDialogue
values, independent of the ordinary container's potentially lower limit. A
single runtime function may directly capture at most 256 CharacterDialogue
values, independent of the ordinary capture count's potentially lower limit.
Nested values also consume the 64-level nesting limit and encoded-size limit.

### Strings and IDs

- CharacterId/ViewId/PublicId retain their own validation and additionally fit
  the listed line/config limits where applicable.
- display name is not part of CharacterDialogue canonical bytes; its
  presentation string limit remains the existing text resource limit.
- inline fallback text is at most 16,384 bytes.
- non-finite numeric config values are rejected; negative zero is normalized.

## 13. Limit tests

For every numeric limit, tests MUST include:

```text
zero/empty when legal
exact limit
one over limit
oversized encoded form with small logical count
nested form reaching exact depth
nested form one over depth
```

No test reads repository source text to prove a limit or deletion.
