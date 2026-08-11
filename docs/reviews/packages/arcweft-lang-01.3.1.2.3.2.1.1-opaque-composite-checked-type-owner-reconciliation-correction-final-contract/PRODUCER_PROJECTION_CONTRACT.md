# Producer and semantic projection contract

## 1. Canonical producer IDs

Producer IDs are explicit descriptor data, not derived from display names.
The following IDs are fixed:

| Semantic producer | Producer ID | Representation |
|---|---|---|
| `VirtualPath` | `std.virtual_path` | exact opaque |
| `ArcError` | `std.arc_error` | exact opaque |
| `ReducerError` | `std.reducer_error` | exact opaque |
| `AgentError` | `std.agent_error` | exact opaque |
| `AssetError` | `std.asset_error` | exact opaque |
| `ContentLoadError` | `std.content_load_error` | exact opaque |
| `DialogueText` | `std.dialogue_text` | exact opaque |
| `ImageHandle` | `std.image_handle` | exact opaque |
| `PresentationLifetime` | `std.presentation_lifetime` | exact opaque |
| `VoiceError` | `std.voice_error` | exact opaque |
| `VoiceHandle` | `std.voice_handle` | exact opaque |
| `Reduction<T>` | `std.reduction` | exact opaque generic leaf |
| `CharacterDialogue<Exact/Any>` | `std.character_dialogue` | exact / producer-wide opaque |

These strings are validated once into `RuntimeOpaqueTypeProducerId` by the
original producer/catalog owner. Core and AWBC never branch on these spellings.

## 2. Accepted nominal catalog

`AcceptedNominalSemantics::Opaque` becomes a struct variant with mandatory
producer ID. `AcceptedNominalRecord::try_new_opaque` is the only opaque record
constructor. Existing standard domain atoms that currently instantiate
`TypeKind::Named` are republished as opaque accepted nominal records. This is a
single accepted-world catalog change; there is no post-build producer overlay.

`AcceptedNominalType` retains the producer selected with its declaration and
arguments. `runtime_opaque_owner(identity)` always returns exact admission.
Accepted Rust metadata can select an already registered opaque accepted record,
but metadata itself never supplies schema/layout or synthesizes a producer ID.
A Rust export that wants a new opaque type must declare a validated producer ID
in its accepted descriptor before catalog publication.

## 3. `TypeKind` mapping

| `TypeKind` | Runtime result |
|---|---|
| `ProjectNominal` | parent exact nominal schema/layout projection |
| `AcceptedNominal` | exact opaque owner from retained producer + semantic ID |
| `CharacterDialogue(Exact)` | exact opaque owner |
| `CharacterDialogue(Any)` | producer-wide opaque owner |
| `Named(String)` | typed `MissingOpaqueProducerEvidence`; no runtime shape |
| primitive/composite | existing structural mapping |
| unsupported function/reference/host-only shape | typed unsupported error |

`TypeKind::Named` is not deleted from sema because current internal/host
semantic facts use it, but it is deleted from successful runtime projection.
No name lookup is performed at that boundary.

## 4. `RuntimeTypeShape` mapping

`RuntimeTypeShape::Named` is deleted. `RuntimeTypeShape::Opaque` gains required
`producer` and `admission` fields. `RuntimeNormalizedType.identity` remains the
single exact semantic identity. `checked_type()` combines those fields directly
without hashing or catalog lookup.

`RuntimeTypeSchema` remains unchanged. Opaque types do not publish a dummy
`Named`, empty record, bytes schema, or copied producer payload schema.

## 5. CharacterDialogue

The existing `CharacterDialogueRuntimeSchema` remains a payload validator that
receives expected record layout and generation catalogs. It does not become a
global top-level checked-type layout owner. Its inherent
`opaque_type_producer()` supplies the fixed producer ID.

- Exact character type produces exact admission.
- Any character type produces producer-wide admission.
- Encoding validates the existing domain value, preserves its current nominal
  record payload, and wraps it with the exact owner.
- Decoding requires the producer ID, validates the record through the current
  schema, derives the exact character semantic type, recomputes semantic ID,
  and rejects mismatch.

## 6. Error and Reduction producers

`ArcError`, `ReducerError`, and `AgentError` do not publish closed schemas or
layout hashes in this cut. Their standard accepted catalog rows are the
canonical type producers. Existing typed runtime/host conversion points receive
or retain the exact opaque owner from compiler/runtime-plan facts, encode their
existing domain payload, validate it in the original error owner, then call
`try_wrap`. No VM/core spelling switch and no error-specific checked type is
added.

`Reduction<GameState>` follows the same generic accepted-nominal producer. Its
exact semantic identity contains `GameState`; projection does not fabricate a
layout for either `Reduction` or `GameState`.

## 7. Entry roles

Entry-role projection for
`Result<Reduction<GameState>, ReducerError>` and
`Result<Unit, AgentError>` constructs both branches before lowering any
constructor. The resulting `RuntimeCheckedType::Result` is stored in the entry
signature/facts and reused by constructors, patterns, calls, returns, native
validation, and AWBC type interning. A selected `Reduction<GameState>` success
never causes the unselected error branch to become `Never` or `Dynamic`.
