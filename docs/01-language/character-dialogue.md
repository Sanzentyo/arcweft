# CharacterDialogue authoring

`CharacterDialogue` is the sole configured dialogue value. Character dialogue
does not have a `say` method, `Speaker`, `SpeakerRef`, or `SpeakerPreset`
intermediate type.

## Canonical surface

Applying content directly to a Character uses square brackets:

```arcw
alice()[
    おはよう。[p]
]
```

Parentheses create or reconfigure an immutable `CharacterDialogue`; they do
not display content:

```arcw
let phone_alice = alice(
    view = @view.PhoneMessage,
    voice = auto,
)

let worried = phone_alice(
    look = worried,
)
```

Square brackets then apply `DialogueContent` and produce a `DialogueLine`:

```arcw
worried()[
    ……聞こえる？[p]
]
```

The concise colon form is direct content-application sugar:

```arcw
alice:
    おはよう。[p]
```

It lowers through the same path as `alice()[...]`; it does not expand to a
method call. A line plan remains attached to the resulting line:

```arcw
worried()[
    まだ話している途中……[p]
]
with:
    at(0.42s):
        alice.stage.look(surprised)
```

## Type contract

The callable behavior is:

```text
Ref<Character>(CharacterDialoguePatch) -> CharacterDialogue
CharacterDialogue(CharacterDialoguePatch) -> CharacterDialogue
CharacterDialogue[DialogueContent] -> DialogueLine
CharacterDialogue: DialogueContent -> DialogueLine
```

`CharacterDialogue` reconfiguration is not ordinary partial function
application. It returns a new value while preserving the original.

The Character identity is immutable. An omitted scalar keeps its prior value;
an authored scalar replaces it; structured Style and rich-text policy merge by
typed field; and custom named fields use deterministic last-authored-value
replacement for the same typed key.

The configured value may retain:

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
inline failure policy
custom named line arguments
```

Each field is validated through its typed schema. A callee spelling, alias, or
display label is never reconstructed into Character identity.

The accepted standard world supplies six zero-argument configuration roles:
`DialogueStage`, `DialoguePortrait`, `DialogueFocus`, `DialogueCleanup`,
`DialogueHook`, and `RichTextStyle`. They are exact opaque types owned by
`std.character_dialogue`. Style is the ordered choice of `EntityRef<Style>`
and the accepted `RichTextStyle` type; it has no separate authored declaration.
Callable schemas borrow these accepted role types from the same world as their
custom-field registry. No intermediate unresolved role type is published.

The View's current occurrence lifecycle has type `DialogueOccurrenceStage`.
Its `stage` field retains the runtime occurrence state, while the configuration
role `DialogueStage` belongs to the reusable character configuration. Their
semantic identities, opaque owners, and persistence contracts are distinct.

## View projection

Dialogue presentation remains a persistent authored View mount. The target
projection is nested and Character-owned:

```arcw
pub view MainDialogue(dialogue: DialogueView) {
    Panel {
        Text(dialogue.character.display_name)
        RichText(dialogue.content)
    }
}
```

The View receives typed values for:

```text
dialogue.character.id
dialogue.character.display_name
dialogue.content
dialogue.occurrence
dialogue.stage
dialogue.reveal
dialogue.primary_action
```

Character display identity is distinct from an external TTS provider's speaker
key. Provider-specific identity belongs to the audio/TTS adapter contract and
must not replace `CharacterId`.

## Removed surface

The final language and public API contain none of:

```text
Character(...)
SpeakerPreset(...)
SpeakerPreset.call(...)
Speaker
SpeakerRef
SpeakerPreset
DialogueSpeakerPreset
SayOptions
TypeKind::Speaker
TypeKind::SpeakerPreset
method-suffix stripping or reconstruction
```

There is no deprecated alias, dual parser, lowering shim, or dedicated
removed-spelling diagnostic. Unrecognized method-shaped input follows ordinary
current grammar and method resolution without a tombstone-specific branch.

Tooling canonicalization may expand colon sugar to a direct character content
call, but it must never emit a removed method suffix or reconstruct an identity
from source text.

## Migration ownership

The existing production migration is intentionally direct:

1. typed `CharacterDialogue` syntax/HIR ownership;
2. semantic configuration and content application;
3. runtime-plan, AWBC, display-frame, and save replacement;
4. nested View/Agent/accessibility/capture projection;
5. deletion of every old speaker/callee/label path and fixture.

No intermediate successful reader or executable compatibility surface is part
of this design.

## Runtime configuration representation

The runtime value is an exact `std.character_dialogue` opaque value whose semantic
identity comes from its Character. Its producer payload is an 18-element tuple:
Character reference; manifest, defaults, custom-schema and View-contract digests;
voice; look; stage; portrait; focus; cleanup; View reference; source locale; hooks;
Style; RichText; inline-failure policy; and custom entries. Optional fields use
the canonical builtin Option cases. Custom entries are sorted two-element tuples
of field ID and value. There is no root or custom-entry nominal record/layout.

The runtime schema borrows one native or AWBC program and the active generation's
catalogs, defaults digests and role bindings. Each authored role binding contains
its exact opaque source type identity and the source type identity of its payload.
Both resolve through that program's existing tables. Style remains the ordered
choice of a Style entity reference and RichText; its opaque branch uses the
RichText payload binding. A role's opaque header alone does not admit its body.
Custom-field descriptors likewise refer to source semantic identities in the
active program, without repeating layout declarations on values. Generic opaque
admission still leaves producer-owned meaning to the producer.

The four private policy Variant families retain complete nominal headers. Their
layouts are computed from the complete policy schema graph and the active exact
RichText owner, including the ordered Style choice in `Apply`. The graph is
construction evidence and is discarded after deriving those headers. It is not
a second executable nominal catalog. Inline failure is a direct Variant.

Voice IDs use the existing validated `voice.*` string identity. They are not
project declarations in `DeclarationIdentityFamily`; `Id` therefore carries a
String, while `Auto` has no payload. This corrects the older representation
package's `Id(EntityRef)` cell without adding a fictitious declaration family.

The producer checks current manifest/default/custom/View digests, look and View
membership, field View policies, role and custom value trees, configured limits,
and canonical re-encoding before publishing a runtime value. Locally bounded
configuration wrappers are not a substitute for this program-bound admission.
Canonical bytes and digests belong to that runtime schema. Domain equality and
hashing compare the immutable configuration fields directly.

## Runtime presentation target

Character presentation target evidence retains the dialogue contract and the
presentation catalog's semantic and locale-policy digests. It carries no root
layout hash: the exact opaque CharacterDialogue owner has no project-record
layout. The serialized target rejects the removed layout field.
