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
