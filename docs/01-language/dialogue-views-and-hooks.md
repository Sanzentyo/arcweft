# Dialogue Views, Character Styles, and Read-State Hooks

Arcweft presents dialogue through ordinary authored Views. Dialogue contributes
typed reactive data and lifecycle operations; it does not define a separate
presentation entity or renderer element.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Character Methods, Dialogue Views, Interpolation, and Preload](dialogue-character-methods-and-views.md)
- [Dialogue Control Tags, Ruby, Inline Formatting, and Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Character Stage / Sprite / Voice Timeline](../03-presentation/character-stage.md)
- [Hooks and Memoization](hooks-and-memoization.md)

## Standard dialogue View

`DialogueView` is a public nominal record from the standard prelude. Ordinary
type checking and LSP completion, hover, definition, and signature help expose
its fields. The standard contract includes the speaker projection, rich
content, occurrence and stage identity, reveal state, and typed primary action.

| Field | Type |
| --- | --- |
| `speaker` | `String` |
| `content` | `DialogueContent` |
| `occurrence` | `DialogueOccurrenceId` |
| `stage` | `DialogueStage` |
| `reveal` | `DialogueReveal` |
| `primary_action` | `DialogueAction` |

The standard library provides the reserved authored View resource
`std.view.dialogue`. Projects cannot redeclare that identity. Its View program
is an ordinary authored resource with the same surface, text, and action nodes
available to project Views. In abbreviated form:

```arcw
pub view StandardDialogueShape(dialogue: DialogueView) {
    Panel(x = 57.6px, y = 460.8px, width = 1164.8px, height = 201.6px) {
        Text(dialogue.speaker)
            .x(85.6px).y(480.8px)
            .width(1108.8px).height(28px)
        RichText(dialogue.content)
            .x(85.6px).y(518.8px)
            .width(1108.8px).height(125.6px)
        Button("", x = 57.6px, y = 460.8px,
                   width = 1164.8px, height = 201.6px)
            .on_click { dialogue.primary_action }
    }
}
```

When a project does not select a View, normal linking selects
`std.view.dialogue`.
There is no runtime-created fallback panel.

Logical page behavior remains dialogue behavior. `[p]` waits and closes the
current logical page; `[l]` waits while retaining it. A View may animate a page
transition, but cannot change those semantics or the line-release rule.

## Project default

A project normally declares reusable style separately from structure:

```arcw
pub style main_dialogue {
    .main_dialogue {
        font-family = text("Noto Sans JP")
        color = rgba(247, 232, 255, 255)
        font-size = 30000milli
    }
}

pub view MainDialogue(dialogue: DialogueView) {
    Panel(x = 57.6px, y = 460.8px, width = 1164.8px, height = 201.6px,
          part = main_dialogue) {
        Text(dialogue.speaker)
            .x(85.6px).y(480.8px)
            .width(1108.8px).height(28px)
        RichText(dialogue.content)
            .x(85.6px).y(518.8px)
            .width(1108.8px).height(125.6px)
            .style(@style.main_dialogue)
        Button("", x = 57.6px, y = 460.8px,
                   width = 1164.8px, height = 201.6px)
            .on_click { dialogue.primary_action }
    }
}

pub dialogue defaults {
    view = @view.MainDialogue
    reveal = typewriter(speed=normal)
}
```

The ID-free declaration is the canonical project-wide defaults profile.
Writing a redundant explicit defaults ID is not the recommended form. Named
profiles use a direct dialogue ID such as
`pub dialogue defaults @dialogue.mobile { ... }`.

## Selecting another View

The `view` line option selects another authored resource without changing the
dialogue content model:

```arcw
pub view SideNote(dialogue: DialogueView) {
    Panel {
        RichText(dialogue.content)
            .style(@style.side_note)
    }
}

narrator.say(view=@view.SideNote)[
    右側の注釈Viewに出る。[p]
]
```

Speaker presets carry the same typed option:

```arcw
let side_alice = alice(view=@view.SideNote, voice=auto)
side_alice: こちらから話すね。[p]
```

The authored resource is a `DialogueViewDefinition`. Each independent runtime
target has a `DialoguePresentationId`, and its active occurrence owns a
persistent `ViewMountId`. Separate targets therefore retain independent View
locals, focus, reveal, and Fx state even when they use the same definition.
Dialogue append, replace, clear, wait, and advance are runtime lifecycle
operations on the captured presentation/mount; they are not methods on a
presentation entity.

Element `x`, `y`, `width`, and `height` arguments or modifiers are authored View
layout constraints. Dynamic dialogue content is laid out inside those retained
bounds; it does not fall back to generic one-line text bounds. The same root
surface union defines rendering, content avoidance, hit/accessibility geometry,
Agent observation, and selected capture.

## Custom dialogue input model

Projects may assign the standard role to an ordinary nominal record:

```arcw
#[dialogue_view]
pub struct PhoneDialogueView {
    speaker: String
    content: DialogueContent
    occurrence: DialogueOccurrenceId
    stage: DialogueStage
    reveal: DialogueReveal
    primary_action: DialogueAction
}
```

The compiler requires exactly these six fields with the listed types. Additional
project data belongs in another ordinary View parameter because the dialogue
runtime does not supply arbitrary projections. A role declaration does not
create another View grammar or bypass nominal type checking. The View signature
uses the custom type normally.

## Character styles and cascade

Character definitions may contribute dialogue typography and read-state style:

```arcw
pub character alice {
    display = "Alice"
    dialogue_style {
        rich_text {
            text {
                color = rgb("#f7e8ff")
            }
        }
    }
}
```

The effective cascade is:

```text
inline rich-text span
  -> line options
  -> speaker preset options
  -> character dialogue_style
  -> authored View style
  -> selected dialogue defaults
  -> standard defaults
```

## Read-state hooks

Read-state and localization hooks receive the dialogue occurrence and selected
View mount as typed context. They may patch dialogue data before the View value
program is evaluated, but may not mutate the retained View tree directly.

Conceptually, the context contains:

```arcw
pub struct DialogueHookCtx {
    line: DialogueLineInfo
    character: CharacterInfo
    view_mount: DialogueViewMount
    locale: Locale
    read_state: ReadState
    content: RichText
    style: TextStyle
    voice: Option<VoiceCue>
    state_hash: StateHash
}
```

The built-in read-state hook can be selected globally:

```arcw
pub dialogue defaults {
    hooks {
        before_text_style += @hook.dialogue.read_state_color
    }
}
```

Hooks are deterministic unless their declared effects explicitly say
otherwise. Product builds may reject nondeterministic debug hooks.

## Agent observation

Agent observation exposes the persistent View mount, dialogue occurrence,
speaker, line and text keys, display stage, logical page, visible range, full
rich content, prepared geometry, reveal cursor, actionable wait, read state,
hooks, and pending voice cue. Geometry comes from the same prepared View text
used for rendering, hit testing, accessibility, and capture.
