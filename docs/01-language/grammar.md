# 文法サマリ

この章は Arcweft parser 実装時の grammar 草案である。詳細な意味論は各章を参照する。

関連:

- [構文概要](syntax.md)
- [ID と参照](ids-and-references.md)
- [関数、パイプ、カリー化](functions-and-pipeline.md)
- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Character Methods, TextBox Targets, Interpolation, and Preload](dialogue-character-methods-and-textbox.md)
- [Dialogue Calls, Line Plans, Cancellation, and Scoped Content Blocks](dialogue-calls-scopes-cancellation.md)
- [Dialogue Content Calls, `with:` Blocks, Line Return Values, and Scoped Handles](dialogue-line-handles-and-returns.md)
- [Object Hooks / Memoization](hooks-and-memoization.md)
- [契約プログラミング](contracts.md)
- [入力パース](parsing.md)

---

## Core grammar

```ebnf
module      := "mod" path
use_item    := visibility? ("lazy" | "eager")? "use" use_tree
visibility  := "pub" | "pub(crate)" | "pub(super)"

item        := visibility? (
                fn
              | task_fn
              | flow
              | reducer
              | state
              | enum
              | struct
              | component
              | shader
              | parser
              | signal
              | test
              | bench
              | hook_item
              | memo_fn
              | character_item
              | textbox_item
              | layer_item
              )

entity_ref  := "#" ident_path | "#<" ref_body ">"
wiki_link   := "[[" link_body "]]"
attribute   := "@" ident generic_args? "(" args? ")"

type        := ident generic_args?
             | "Ref" "<" type ">"
             | "Result" "<" type "," type ">"
             | "Option" "<" type ">"
             | "Need" "<" type "," type ">"

generic_args:= "<" type_list ">"

expr        := literal
             | entity_ref
             | path
             | call
             | method_call
             | content_call
             | pipe
             | lambda
             | placeholder
             | match
             | block

content_call := method_call content_block line_plan_block?
              | call content_block line_plan_block?

pipe        := expr "|>" expr
placeholder := "_" | "^"

await_with  := "await" expr ("?")? "with" block

contract    := requires | ensures | invariant | modifies | effects | decreases
requires    := "requires" contract_mode? expr
ensures     := "ensures" contract_mode? expr
```

---

## ID ambiguity rules

```text
- `<...>` is for generics.
- Entity references use `#foo.bar` or `#<foo.bar>`.
- `#foo.bar.baz` is read as one entity path.
- Use `#<foo.bar>.method()` or `(#foo.bar).method()` for member access on an EntityRef.
- `#` is not an option-list marker.
- `^` is the pipe-left placeholder.
- `_` is the lambda / partial-application placeholder.
```

---

## Flow-integrated dialogue grammar

Arcweft does not define a separate `script` item. Concise visual-novel syntax is part of `flow_body`. There is no script-lowering phase.

```ebnf
flow        := visibility? "flow" entity_ref ident? param_list? return_type? contract* block
flow_body   := "{" flow_item* "}"
flow_item   := typed_stmt
             | scenario_command
             | speaker_line
             | character_content_call
             | choice_block
             | include_fragment
             | line_plan_attachment

scenario_command := "@" ident scenario_args?

character_content_call := character_expr "." "say" call_args? content_block line_plan_attachment?
                        | speaker_expr call_args? content_block line_plan_attachment?
character_expr := ident | entity_ref | "(" entity_ref ")"

speaker_expr := ident | entity_ref | "(" entity_ref ")"
              ; ident may resolve to Ref<Character> or SpeakerPreset
speaker_line := speaker_expr call_args? ":" dialogue_inline line_plan_attachment?
              | speaker_expr call_args? ":" newline indent dialogue_body dedent line_plan_attachment?
              | speaker_expr call_args? ":[" dialogue_body "]" line_plan_attachment?
              | speaker_expr call_args? content_block line_plan_attachment?

line_plan_attachment := "with" line_plan_block
                      | line_plan_block_compat
line_plan_block_compat := "{" line_plan_item* "}"  ; accepted after content calls, formatted as with

choice_block := "@choice" entity_ref? "{" choice_option* "}"
choice_option := entity_ref? string ("if" expr)? "->" entity_ref

include_fragment := "include" entity_ref
```

Examples:

```awft
alice: おはよう。[p]

alice(id=#say.opening.001, face=smile, voice=auto):
    おはよう。[p]

alice.say(id=#say.opening.001, voice=auto)[
    おはよう。[p]
]
with {
    at(0.42s) { alice.stage.face(worried) }
}

alice(voice=auto):
    おはよう。[p]
with {
    at(0.42s) { alice.stage.face(worried) }
}
```

`alice(args): text` is sugar for `alice.say(args)[text]`. If `alice(args)` appears in expression position, it creates a `SpeakerPreset` instead of displaying text:

```awft
let alice2 = alice(face=smile, voice=auto)
alice2: おはよう。[p]
```

---

## Dialogue call and line plan blocks

```ebnf
call_args   := "(" args? ")"
content_block := "[" dialogue_token* "]"
line_plan_block := "{" line_plan_item* "}"
                 | ":" newline indent line_plan_item* dedent
line_plan_item := line_option
                | return_stmt
                | cancel_rule
                | timed_cue
                | start_group
                | together_group
                | let_stmt
                | assert_stmt
                | memo_line_stmt

start_group := "start" block
together_group := "together" block

timed_cue := "at" "(" timeline_anchor ")" cue_content_block
cue_content_block := "{" cue_stmt* "}"
                   | ":" newline indent cue_stmt* dedent
                   | ":" cue_stmt

cancel_rule := "cancel" "on" cancel_trigger cancel_action

line_option := ident "=" expr
memo_line_stmt := "memo" ident memo_opts?
return_stmt := "return" expr
```

Compatibility: older `at(...)[...]` cue blocks are accepted by the parser but formatted as `at(...) { ... }`.

---

## Dialogue-text mode grammar

`[...]` tags are special only in dialogue text mode.

```ebnf
dialogue_inline := dialogue_token*
dialogue_body   := dialogue_token*

dialogue_token  := text_chunk
                 | dialogue_tag
                 | dialogue_end_tag
                 | dialogue_expr
                 | format_expr
                 | ruby_natural
                 | escaped_char

dialogue_tag    := "[" tag_name tag_attrs? "]"
dialogue_end_tag:= "[/" tag_name "]"
dialogue_expr   := "#[" expr "]"
format_expr     := "fmt" "(" expr ("," format_arg)* ")"
ruby_natural    := "｜" ruby_base "《" ruby_text "》"
escaped_char    := "\\" ("[" | "]" | "#" | "\\" | "｜" | "《" | "》" | ":" | "{" | "}")

tag_name        := ident
```

Reserved tag names include:

```text
p, l, r, br, w, ruby, voice, face, pose, show, hide, move,
anim, hook, call, at, signal, if, else, endif, raw, fmt
```

---

## Hooks and memoization grammar

```ebnf
hook_item   := visibility? "hook" entity_ref hook_target hook_phase hook_check? block
hook_target := "on" ("query" type | entity_ref | hook_selector)
hook_phase  := "phase" ident
hook_check  := "check" ("before" ident
              | "after" ident
              | "every" "frame"
              | "on" "event"
              | "on" "change" expr
              | "every" int "frames"
              | "throttle" duration
              | "debounce" duration
              | "once"
              | "until" expr)

memo_fn     := "memo" "fn" fn_signature memo_opts? block
memo_let    := "memo" "let" ident "=" expr memo_opts?
memo_block  := "memo" ident? memo_opts? block
memo_opts   := ("cache" ident)? ("key" expr_list)? memo_policy_block?
```

---

## Layer grammar

```ebnf
layer_item  := visibility? "layer" entity_ref ":" ident layer_options? block
layer_options := ("z" "=" int)? ("input" "=" ident)? ("hit_test" "=" ident)?
scene_layer := "layer" entity_ref block
```

---

## Example

```awft
alice.say(id=#say.opening.dream_hint, voice=auto)[
    今日は少しだけ、#[fmt("変な夢", color=rgb("#a8b5ff"))]を見たんだ。[p]
]
with {
    at(0.42s) { alice.stage.face(worried, crossfade=120ms) }
}

hook #hook.choice_visible
on #choice.opening.listen
phase AfterLayout
check every frame
{
    signal #signal.choice_visible <- true
}

memo fn route_title(route: Ref<Flow>) -> String
cache session
{
    registry.flow(route).title
}
```


---

## Line result and handle destructuring grammar

Line plans may return values. The returned value is matched by normal pattern grammar.

```ebnf
let_stmt    := "let" pattern "=" expr
pattern     := ident
             | "_"
             | "(" pattern ("," pattern)* ")"
             | struct_pattern

line_expr   := character_content_call
             | speaker_line
```

Examples:

```awft
let (actor, voice) = alice.say(voice=auto)[聞いて。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    let voice = line.voice_handle()
    return (actor, voice)
```

`_` in a pattern explicitly discards the matched value. If the discarded value is a `ScopedHandle`, its drop policy runs immediately after destructuring.

```awft
let (_, cue) = alice[おはよう。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    let cue = at(0.42s): actor.face(smile)
    return (actor, cue)
```

The discarded actor lease is released immediately. The scheduled cue remains owned by `cue`.
