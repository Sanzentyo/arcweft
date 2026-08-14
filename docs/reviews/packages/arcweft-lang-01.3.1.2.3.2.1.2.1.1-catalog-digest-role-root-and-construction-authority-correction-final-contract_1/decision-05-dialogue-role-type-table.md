# Decision 05 — exact CharacterDialogue runtime role table

## Exact semantic and runtime types

The accepted standard opaque producer is exactly `RuntimeOpaqueTypeProducerId("std.character_dialogue")`. Each authored role is one zero-arity standard accepted nominal with `AcceptedNominalOwnerId::Standard`, `AcceptedNominalOrigin::Domain`, the current accepted world, and no source span. Each nominal's canonical path is an implicit-crate, one-segment accepted path. Its runtime semantic identity is the existing accepted nominal semantic identity, and its final runtime type is an exact-identity opaque checked type.

| Role | Accepted semantic nominal | Generic args | Producer | Final semantic `TypeKind` after substitution | Final `RuntimeCheckedType` | Evidence |
|---|---|---:|---|---|---|---|
| Stage | `DialogueStage` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(stage_nominal)` | `Opaque { owner: RuntimeOpaqueTypeOwner::exact(producer, stage_semantic_id) }` | standard nominal row + accepted world |
| Portrait | `DialoguePortrait` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(portrait_nominal)` | exact opaque owner with portrait semantic ID | standard nominal row + accepted world |
| Focus | `DialogueFocus` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(focus_nominal)` | exact opaque owner with focus semantic ID | standard nominal row + accepted world |
| Cleanup | `DialogueCleanup` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(cleanup_nominal)` | exact opaque owner with cleanup semantic ID | standard nominal row + accepted world |
| Hook | `DialogueHook` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(hook_nominal)` | exact opaque owner with hook semantic ID | standard nominal row + accepted world |
| RichText | `RichTextStyle` | 0 | `std.character_dialogue` | `TypeKind::AcceptedNominal(rich_text_nominal)` | exact opaque owner with RichText semantic ID | standard nominal row + accepted world |
| Style | not authored | — | derived | `TypeKind::Choice(vec![TypeKind::Ref(EntityType::style()), TypeKind::AcceptedNominal(rich_text_nominal)])` | `RuntimeCheckedType::Choice(vec![RuntimeCheckedType::EntityReference, rich_text_checked.clone()])` | derived only after RichText validation |

`Style` alternative order is normative: alternative 0 is `EntityRef<Style>` and alternative 1 is RichText. No third alternative, reorder, flattening, or authored Style declaration is accepted. The Choice validator in Decision 13 requires exactly one match.

The current standard callable rows that use `TypeKind::Named("DialogueStage")`, `DialoguePortrait`, `DialogueFocus`, `DialogueCleanup`, `DialogueHook`, or `RichTextStyle` are directly replaced by the typed role coordinate in Decision 06. No `RuntimeCheckedType::Dynamic` or producer-wide opaque fallback is added.
