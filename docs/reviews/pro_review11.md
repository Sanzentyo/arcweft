# Arcweft: line lifetime registry, inline marks, VM threads, cleanup, and drop

## 0. Goal

This document specifies how Arcweft should integrate:

- dialogue line lifetimes,
- `'line.*` lifetime registry access,
- built-in common visual-novel line effects such as focus,
- `init:`, `thread:`, `finally:`, and line event handlers,
- inline text marks,
- deterministic VM-level concurrent execution,
- guaranteed logical cleanup,
- `drop`, `on_drop`, and typestate-like handle states,
- current implementation and docs changes required in `Sanzentyo/arcweft`.

This is not a backwards-compatibility plan. The implementation is still in progress, so this document explicitly says which old spellings should be deleted.

---

## 1. Main decisions

### 1.1 Use `'line.*`, not `line.*`, for lifetime registry values

Use:

```awft
'line.focus |> drop
```

Do not use:

```awft
line.focus |> drop
```

Reason:

- `line.focus` looks like a normal object field.
- The value is actually coming from the current line lifetime registry.
- Arcweft already uses apostrophe syntax for lifetime-like labels in several contexts, so `'line.focus` communicates “this value is scoped to the line lifetime”.

Examples:

```awft
alice(.smile, focus = .soft):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

Named focus handles:

```awft
alice(.smile, focus = { main = .soft, bg = .background_dim }):
    聞いて。[mark .release_main]
with:
    on .release_main:
        'line.focus.main |> drop
```

### 1.2 Registry keys are `Option<T>` unless statically guaranteed

Lifetime registry access has two forms:

```awft
'line.focus
'line.focus?
```

Rules:

```text
If the key is statically guaranteed:
  'line.focus : FocusHandle

If the key is not statically guaranteed:
  'line.focus : Option<FocusHandle>
  or require explicit optional access: 'line.focus?
```

Recommended source rule:

```text
- Non-optional access is allowed only when the checker can prove the key exists.
- If not provable, the source must write `?` or use a safe accessor.
```

Examples:

```awft
alice(.smile, focus = .soft):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

`focus = .soft` is unconditional and profile resolution guarantees a focus handle.  
Therefore `'line.focus` is non-optional.

Conditional focus:

```awft
alice(.smile):
    聞いて。[mark .release_focus]
with:
    init:
        if state.flags.focus_alice {
            'line.focus <- stage.focus(target = alice) |> on_drop(release(120ms))
        }

    on .release_focus:
        if let Some(f) = 'line.focus? {
            f |> drop
        }
```

Here `'line.focus?` is `Option<FocusHandle>`.

A compile error should be emitted for unsafe non-optional access:

```awft
on .release_focus:
    'line.focus |> drop
```

Diagnostic:

```text
error: lifetime key `'line.focus` is not statically guaranteed
help: use `'line.focus?` and handle Option<FocusHandle>
```

### 1.3 Delete local `hook` syntax

Do not use local `hook` syntax inside dialogue text or `with:`.

Delete these forms:

```awft
[hook release_focus]
hook release_focus:
    ...
```

Use:

```awft
[mark .release_focus]
on .release_focus:
    ...
```

Top-level `hook` declarations remain a different feature for object/runtime hooks. This document removes only dialogue-local hook marker/handler syntax.

### 1.4 Decide `[]` vs `#[...]`

Use `[]` for dialogue control tags.

Use `#[...]` only for expression interpolation / pure content insertion.

```text
[p]                   page wait
[r]                   hard line break
[mark .release_focus] zero-width line marker
[call flash(...)]     dialogue-safe call, if retained by tag system

#[player_name]        expression interpolation
#[fmt(score)]         formatting expression
```

Delete:

```awft
#[mark .release_focus]
#[hook release_focus]
```

Reason:

- `#[...]` already means expression/content insertion in dialogue text.
- Markers are timeline/control tags, so they belong to `[]`.

### 1.5 `[mark]` is useful and should stay

It is possible to avoid custom marks by using built-in anchors such as page numbers or word indices:

```awft
on text.page(1):
    ...
on text.reveal.word(3):
    ...
```

But this is fragile for localization and editing. A named mark is more stable.

Therefore keep:

```awft
[mark .release_focus]
```

Marker names should be relative IDs within the line marker namespace. The leading dot is recommended.

```awft
[mark .release_focus]
on .release_focus:
    ...
```

Allowing bare `[mark release_focus]` would be easy, but it conflicts with the relative-ID style and is less explicit. Prefer dot form.

---

## 2. Default common visual-novel options

Common visual-novel effects should be built in as standard line options, not forced into `with init:`.

### 2.1 Recommended built-in line options

```text
look
  Character look patch. First positional argument maps to this.

stage
  Stage-only look patch.

portrait
  Text-window portrait/icon-only look patch.

voice
  Dialogue voice source or auto policy.

focus
  Built-in focus effect.

cleanup
  Line cleanup policy.

window
  Dialogue window / textbox.

reveal
  Text reveal policy.

skip
  Skip behavior policy.

auto_mouth
  Lipsync / mouth-control policy.

camera
  Optional camera framing or focus target.

ducking
  Audio ducking policy while this line is active.

id
text_key
source_locale
hooks
style
args
```

### 2.2 `focus = .soft`

`focus = .soft` means:

```text
Use the built-in focus profile `.soft`.
Create a line-owned FocusHandle.
Store it under `'line.focus`.
Apply its enter effect when the line starts.
Drop/release it during line cleanup unless dropped earlier.
```

Example profile:

```awft
pub focus profile @focus.soft {
    target = speaker
    others = .blur(8px) & .dim(35%)
    enter = 180ms
    release = 120ms
    cleanup_visual = snap
}
```

The short line:

```awft
alice(.smile, focus = .soft):
    聞いて。[p]
```

is conceptually:

```awft
alice(.smile):
    聞いて。[p]
with:
    init:
        'line.focus <-
            stage.focus(
                target = alice,
                others = .blur(8px) & .dim(35%),
            )
            |> on_drop(release(120ms))
```

But authors should not have to write this for common cases.

### 2.3 Built-in profiles should be extensible

Projects can define profiles:

```awft
pub focus profile @focus.confession {
    target = speaker
    others = .blur(12px) & .dim(45%)
    enter = 240ms
    release = 180ms
}
```

Then use:

```awft
alice(.embarrassed, focus = @focus.confession):
    えっと……。[p]
```

Expected-type shorthand:

```awft
alice(.embarrassed, focus = .confession):
    えっと……。[p]
```

---

## 3. Stage look vs portrait look

Standing sprite / Live2D stage display and text-window portrait icon are distinct.

A look patch can affect:

```text
stage
  Standing sprite, Live2D model, 3D model, stage pose, motion, etc.

portrait
  Text-window face icon, nameplate icon, bust-up portrait.

both
  Common named look, such as `.smile`, may affect both stage and portrait.
```

Examples:

```awft
alice(.smile):
    おはよう。[p]
```

May apply:

```text
.stage.expr.smile
.portrait.icon.smile
```

Stage only:

```awft
alice(stage = .expr.worried):
    ……大丈夫。[p]
```

Portrait only:

```awft
alice(portrait = .icon.wink):
    ひみつだよ。[p]
```

Explicit combined form:

```awft
alice(.stage.expr.angry & .portrait.icon.smile):
    怒ってないよ。[p]
```

Use only `&` as the look-patch merge operator.

```awft
.smile & .casual & .motion.nod
```

`+`, `|`, and `||` are not used for patch composition.

---

## 4. Inline marks and `with on`

### 4.1 Marker tag

```awft
[mark .release_focus]
```

This is a zero-width timeline mark. It is not displayed.

### 4.2 Handler

```awft
with:
    on .release_focus:
        'line.focus |> drop
```

### 4.3 Marker-only usage

A marker can exist without a handler if it is used by tooling, tests, transcript sync, or a thread wait.

```awft
alice:
    聞いて。[mark .important_beat]
```

No error.

An unused mark may be linted if the project enables:

```toml
[lint.dialogue]
unused_marks = "warn"
```

### 4.4 Handler without marker

This is an error unless the trigger is not a marker.

```awft
with:
    on .missing_marker:
        ...
```

Diagnostic:

```text
error: marker handler `.missing_marker` has no matching `[mark .missing_marker]`
help: add `[mark .missing_marker]` to the dialogue text
help: or use an event trigger such as `on line.end:`
```

### 4.5 Duplicate marks

Duplicate marker names in one line are error by default.

```awft
聞いて。[mark .a]
もう一度。[mark .a]
```

Diagnostic:

```text
error: duplicate line mark `.a`
help: use `.a1` and `.a2`, or define an indexed mark policy explicitly
```

If repeatable marks are desired, require explicit repeatability:

```awft
[mark .beat repeat]
```

Then the handler must accept an index:

```awft
on .beat(i):
    debug_log("beat {i}")
```

This can be deferred; default should be duplicate-error.

### 4.6 Localization edge case

Marks are semantic timeline anchors. Localized variants must preserve required marks.

If source locale has:

```awft
聞いて。[mark .release_focus]
```

and target locale omits `.release_focus`, localization check should fail or warn according to policy:

```text
error: localized text for @text... is missing required mark `.release_focus`
```

Required marks are those referenced by:

```text
- `on .mark`
- `wait mark .mark`
- test assertions
- generated voice marker bindings
```

---

## 5. `init:`, `thread:`, `finally:`

### 5.1 `init:`

`init:` runs synchronously before:

```text
- dialogue content reveal
- line voice playback
- user threads
- marker traversal
- visible line-start effects
```

```awft
alice(.smile):
    聞いて。[mark .release_focus]
with:
    init:
        if state.flags.focus_alice {
            'line.focus <-
                stage.focus(target = alice)
                |> on_drop(release(120ms))
        }

    on .release_focus:
        if let Some(f) = 'line.focus? {
            f |> drop
        }
```

`init:` is useful for conditional setup. It should not be required for common effects such as `focus = .soft`.

### 5.2 `thread:`

`thread:` creates a real VM child task scoped to the line.

```awft
alice(.smile, voice = auto):
    おはよう。[p]
with:
    thread motion:
        alice.stage.apply(.motion.nod)
        wait 0.35s
        alice.stage.apply(.stage.expr.smile)

    thread portrait:
        wait mark .release_focus
        alice.portrait(.portrait.icon.wink)
```

Multiple `thread:` blocks may run concurrently.

### 5.3 `finally:`

`finally:` is thread-local guaranteed finalization.

```awft
thread motion:
    let lease =
        alice.stage.lease()
        |> on_drop(release)

    alice.stage.apply(.motion.nod)

    finally:
        lease |> drop
```

`finally:` runs on:

```text
- normal thread completion
- thread cancellation
- skip
- cancel
- goto
- return
- error propagation
- line cleanup
```

`finally:` should be the last section in a thread.

---

## 6. VM execution model

Arcweft core is Sans I/O: it does not directly perform filesystem, GPU, audio, device, or OS effects. It computes state and effect requests.

A line is a structured VM task group.

```text
LineTask
├─ InitTask
├─ ContentTask
├─ VoiceTask
├─ UserThreadTask[]
├─ EventHandlerTask[]
├─ TimedCueTask[]
└─ CleanupTask
```

The VM may execute independent tasks in parallel, but effect outputs are collected into deterministic frame-boundary outputs.

Required properties:

```text
- deterministic logical clock
- stable ordering of simultaneous effects
- no direct side effects in core
- effect requests only
- replayable FrameInput / FrameOutput
- child tasks cannot outlive the line unless explicitly detached
```

This aligns with the existing runtime direction that Flow/dialogue/choice/Need/effect emission are processed by the bytecode VM, while the core emits `FrameOutput` and `EffectRequest` data rather than executing side effects directly.

---

## 7. Structured concurrency rules

`thread:` follows Rust-like structured concurrency.

```text
- A line owns all threads spawned in its `with:` block.
- Child threads cannot outlive the line by accident.
- Live child threads are cancelled/joined during line cleanup according to cleanup policy.
- Captured values must be thread-safe or moved.
- Unique MustDrop handles cannot be captured by multiple live threads.
- `finally:` must discharge moved MustDrop values.
```

### 7.1 Move into thread

```awft
with:
    init:
        let lease =
            alice.stage.lease()
            |> on_drop(release)

    thread motion:
        let lease = move lease
        alice.stage.apply(.motion.nod)

        finally:
            lease |> drop
```

After `move lease`, the outer `lease` is unavailable.

### 7.2 Shared handle

```awft
with:
    init:
        let focus =
            stage.focus(target = alice)
            |> on_drop(release(120ms))
            |> share

    thread a:
        focus.request(.dim(30%))

    thread b:
        focus.request(.blur(8px))
```

A unique handle cannot be implicitly shared.

### 7.3 Conflict detection

Concurrent writes to the same exclusive axis should warn or error.

```awft
with:
    thread a:
        alice.stage.apply(.stage.expr.smile)

    thread b:
        alice.stage.apply(.stage.expr.worried)
```

Diagnostic:

```text
warning: concurrent writes to exclusive axis `alice.stage.expr`
help: sequence them with `wait`, use one thread, or give an explicit ordering policy
```

---

## 8. Cleanup policy

Cleanup is line option or project default, not a `with:` statement.

```awft
alice(.smile, cleanup = .fast_skip):
    聞いて。[p]
```

Profile:

```awft
pub cleanup profile @cleanup.fast_skip {
    pending_marks = run
    visual = ignore
    audio = stop_now
    threads = cancel
}
```

Project default:

```toml
[dialogue.cleanup.default]
pending_marks = "run"
visual = "snap"
audio = "stop_now"
threads = "cancel"
```

Fields:

```text
pending_marks
  run        Run pending marker handlers in cleanup mode.
  drop_only  Run only drop/state-cleanup portions.
  skip       Do not run pending marker handlers; finalizers still run.

visual
  normal
  snap
  suppress_transient
  ignore

audio
  normal
  stop_now
  suppress_new
  ignore

threads
  join
  cancel
  detach_error
```

Cleanup order:

```text
1. Enter line cleanup mode.
2. Process pending mark handlers according to `pending_marks`.
3. Cancel/join live threads according to `threads`.
4. Run every cancelled thread's `finally:`.
5. Drop line-owned handles from lifetime registries, e.g. `'line.focus`.
6. Drop any remaining MustDrop locals in reverse creation order.
7. Unregister line-local handlers, subscriptions, exposed state.
8. Close the line lifetime.
```

---

## 9. Drop, `on_drop`, and typestate

### 9.1 `on_drop`

```awft
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))
```

`on_drop` attaches drop policy metadata.

### 9.2 `drop`

```awft
'line.focus |> drop
drop('line.focus)
```

Preferred source style:

```awft
'line.focus |> drop
```

Override policy:

```awft
'line.focus |> drop(release(40ms))
```

### 9.3 Type-state semantics

`drop` should be modelled as typestate/capability transition, not dynamic trait mutation.

```text
FocusHandle<Live> |> drop -> FocusHandle<Dropped>
```

The dropped value cannot be used again.

```awft
'line.focus |> drop
'line.focus.release()
```

Diagnostic:

```text
error: use of dropped value `'line.focus`
```

The uploaded typestate/capability note argues for modelling operations as type-level state transitions rather than dynamically adding traits. This document follows that direction.

### 9.4 Idempotent logical cleanup

Runtime cleanup must be logically idempotent.

If marker cleanup and finalizer both attempt to drop the same value:

```awft
on .release_focus:
    'line.focus |> drop
```

then line cleanup sees `'line.focus` as already dropped and does not execute a second physical cleanup.

However, explicit double drop in the same reachable code path is an error:

```awft
on .release_focus:
    'line.focus |> drop
    'line.focus |> drop
```

Diagnostic:

```text
error: value `'line.focus` is already dropped
```

---

## 10. `[]` and `#[...]` final rule

### Dialogue control tags: `[]`

Allowed in dialogue text:

```awft
[p]
[r]
[l]
[mark .release_focus]
[raw]...[/raw]
[ruby rt="..."]...[/ruby]
[call flash(color = "#fff")]
```

### Expression interpolation: `#[...]`

Allowed:

```awft
#[player_name]
#[fmt(score)]
#[route_title(state.route)]
```

### Deleted forms

Delete these from docs and examples:

```awft
[hook a1]
#[hook a1]
#[mark a1]
hook a1:
```

If local event handling is needed:

```awft
[mark .a1]
with:
    on .a1:
        ...
```

If global/object hook is needed, use top-level `hook`.

---

## 11. Current Arcweft implementation gaps

The following are concrete differences from the current repository.

### 11.1 AST line options

Current `LineOptions` has:

```text
id
text_key
voice
window
source_locale
hooks
style
args
```

It does not yet have:

```text
look
stage
portrait
focus
cleanup
```

Add these to `LineOptions` and `LineOptionsInit`.

### 11.2 Dialogue tokens

Current `DialogueToken` has `Text`, `Raw`, `Tag`, `EndTag`, `Expr`, `Ruby`, and `Escape`.

It does not yet have a structured mark token.

Add:

```rust
DialogueToken::Mark(LineMark)
```

or keep it as `Tag` but require a semantic lowering pass that recognizes `Tag { name: "mark" }`.

Recommended: add `Mark`, because marker matching is semantic and should not be hidden in generic tags.

### 11.3 Parser line options

Current `parse_line_options` expects named options and reports unnamed line options as errors.

Needed changes:

```text
- first positional option -> look
- `face` compatibility alias may be deleted or rewritten to `look`
- parse `focus = ...`
- parse `cleanup = ...`
- parse `stage = ...`
- parse `portrait = ...`
```

### 11.4 Parser line plan items

Current line-plan parsing handles existing plan items such as `out`, `let`, `cancel`, `at`, `start`, `together`, `memo`, assertions, and expression fallback.

Needed changes:

```text
- add `init:`
- add `thread name:`
- add `finally:` inside thread
- add `on trigger:`
- remove local `hook name:`
- remove `cleanup on ...:` from line plan grammar
```

### 11.5 Expression parser

`'line.focus` needs expression support.

Possible AST:

```rust
Expr::LifetimePath {
    lifetime: String,
    path: Vec<String>,
    optional: bool,
}
```

Examples:

```awft
'line.focus
'line.focus?
'line.focus.main
```

Careful with existing label syntax such as `out 'label expr`. In expression position, apostrophe path is lifetime registry access. In control-transfer syntax, apostrophe before an identifier remains a label.

### 11.6 Type checker

Need new semantic checks:

```text
- lifetime registry guaranteed-key analysis
- Option typing for unproven keys
- MustDrop tracking
- use-after-drop
- double-drop
- thread capture safety
- `finally:` boundedness
- concurrent exclusive-axis write detection
- pending mark cleanup traversal
```

### 11.7 Runtime / VM

Current design docs say Arcweft core is Sans I/O and emits `EffectRequest` / desired state, and that Flow/dialogue/choice/Need/effect emission are VM semantics.

The new `thread:` model must be implemented as VM child fibers/tasks inside this Sans I/O core. It must not introduce direct OS threads or direct side effects in `arcweft-core`.

---

## 12. Required docs changes

Update these docs.

### `docs/01-language/dialogue-character-methods-and-textbox.md`

- Replace `face` canonical option with `look`.
- Add `stage`, `portrait`, `focus`, `cleanup`.
- Make `alice(.smile):` mean `alice(look = .smile):`.
- Use `'line.focus`, not `line.focus`.
- Delete local `[hook ...]` examples.
- Use `[mark .name]` + `on .name:`.

### `docs/01-language/dialogue-calls-scopes-cancellation.md`

- Remove `stop voice fade=...`.
- Use `voice_handle |> drop(stop_now)` or `voice_handle.stop(...)` depending on final API.
- Add `init:`, `thread:`, `finally:`, `on`.
- Remove `hook a1:`.
- Remove `cleanup on skip:` from `with:` examples.
- Move cleanup policy into line options or profile declarations.

### `docs/01-language/dialogue-control-tags-and-ruby.md`

- Clearly separate `[]` tags and `#[...]` interpolation.
- Add `[mark .name]`.
- Delete `[hook name]` if present.
- Keep `[call ...]` only if dialogue-safe side-effect tags remain part of the language.
- Define escaping for literal `[mark ...]`.

### `docs/02-runtime/core.md`

- Add line task group model.
- Add VM child task model for `thread:`.
- State that thread outputs are normalized into deterministic `FrameOutput`.
- State that core remains Sans I/O.

### `docs/02-runtime/hooks-memoization.md`

- Clarify distinction between top-level runtime hooks and line-local `on` handlers.
- Do not treat line-local `on` as global HookTable entries unless lowering wants a scoped `HookRecord` variant.
- Ensure line-local handlers cannot directly mutate state outside allowed outputs.

### `docs/03-presentation/character-stage.md`

- Distinguish stage look and portrait look.
- Add `CharacterPatch` / `LookPatch` with `&`.
- Add focus profiles.
- Add Live2D stage axes / motions / params.
- Replace `#` entity refs with `@` refs.

### `docs/03-presentation/audio.md`

- Replace `play voice ...` with `voice(...)` / `alice.voice(...)`.
- Replace `#voice`, `#bus`, `#bgm` with `@voice`, `@bus`, `@bgm`.
- Clarify dialogue `voice = ...` vs standalone `voice(...)`.

---

## 13. Edge cases

### 13.1 Mark in raw text

```awft
[raw][mark .x][/raw]
```

This is text, not a marker.

### 13.2 Escaped mark

```awft
\[mark .x]
```

This is literal text.

### 13.3 Marker inside interpolated expression

```awft
#[some_content_with_mark]
```

Marks returned from runtime content are not static line marks unless explicitly typed as `ContentWithMarks` and accepted by localization/extraction tooling. Default: not allowed as static markers.

### 13.4 Conditional marker text

If a mark exists only in conditional text, handlers depending on it must treat it as potentially pending or absent.

Preferred: keep marks in statically authored text.

### 13.5 Multiple locales

Required marks must exist in all localized variants unless the handler is declared locale-optional.

```awft
on .release_focus optional:
    ...
```

This can be deferred; default should require marks across locales.

### 13.6 Optional lifetime keys

```awft
'line.focus?
```

is `Option<FocusHandle>`.

Unsafe:

```awft
'line.focus |> drop
```

if focus is conditional.

### 13.7 Dropping optional values

Allowed:

```awft
'line.focus? |> drop
```

This should mean:

```awft
if let Some(v) = 'line.focus? {
    v |> drop
}
```

But this implicit Option behavior may hide mistakes. Safer rule:

```text
drop(Option<T>) is allowed only when T: MustDrop and the call is explicitly written as `drop_optional`.
```

Recommended:

```awft
'line.focus? |> drop_optional
```

### 13.8 Thread and Option

If a thread captures an optional handle:

```awft
thread fx:
    if let Some(f) = 'line.focus? {
        f.request(...)
    }
```

The checker must ensure the handle cannot be dropped concurrently by another thread unless it is shared or the access is mediated.

### 13.9 Mark handler drops value while thread uses it

```awft
thread fx:
    wait 1s
    'line.focus.request(...)

on .release_focus:
    'line.focus |> drop
```

This is unsafe unless `'line.focus` is a shared handle with cancellation-aware methods.

Diagnostic:

```text
error: thread `fx` may use `'line.focus` after handler `.release_focus` drops it
help: move the handle into the thread and drop it in `finally:`
help: or use a shared/cancellable handle
```

### 13.10 Cleanup visual/audio suppression

Even if `visual = ignore` or `audio = ignore`, ownership cleanup still happens. The effect request emission is suppressed, not the logical drop.

### 13.11 `finally:` starts new thread

Disallow by default.

```awft
finally:
    thread cleanup_fx:
        ...
```

Diagnostic:

```text
error: `finally:` cannot spawn line-scoped threads
```

Allow only explicit detached tasks with capability.

### 13.12 `finally:` awaits long Need

Disallow unless bounded.

```awft
finally:
    let r = await long_task()
```

Diagnostic:

```text
error: `finally:` cannot await unbounded work
help: use a bounded cleanup request or detach an explicit task
```

---

## 14. Recommended final style

### Simple

```awft
alice(.smile):
    おはよう。[p]
```

### Built-in focus

```awft
alice(.smile, focus = .soft):
    聞いて。[p]
```

### Focus release at mark

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
    こっちをみて[r]
with:
    on .release_focus:
        'line.focus |> drop
```

### Conditional focus

```awft
alice(.smile, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
with:
    init:
        if state.flags.focus_alice {
            'line.focus <-
                stage.focus(target = alice, others = .blur(8px) & .dim(35%))
                |> on_drop(release(120ms))
        }

    on .release_focus:
        if let Some(f) = 'line.focus? {
            f |> drop
        }
```

### Concurrent behavior

```awft
alice(.smile, voice = auto, focus = .soft, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
    こっちをみて[r]
with:
    thread motion:
        alice.stage.apply(.motion.nod)
        wait 0.35s
        alice.stage.apply(.stage.expr.worried)

        finally:
            alice.stage.apply(.motion.idle)

    thread portrait:
        wait mark .release_focus
        alice.portrait(.portrait.icon.wink)

    on .release_focus:
        'line.focus |> drop
```

### Thread-owned handle

```awft
alice(.smile, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
with:
    init:
        let lease =
            alice.stage.lease()
            |> on_drop(release)

    thread motion:
        let lease = move lease
        alice.stage.apply(.motion.nod)
        wait mark .release_focus
        alice.stage.apply(.stage.expr.smile)

        finally:
            lease |> drop
```

---

## 15. Implementation order

1. Update docs to delete `[hook]` and local `hook`.
2. Add `[mark .name]` to dialogue token model.
3. Add `'line.*` lifetime registry expression syntax.
4. Add `look`, `stage`, `portrait`, `focus`, `cleanup` to `LineOptions`.
5. Add `init:`, `thread:`, `finally:`, and `on ...:` to line plan AST/parser.
6. Add VM line task group model.
7. Add cleanup profiles and line cleanup policy.
8. Add typechecker pass for lifetime registry key guarantees and Option access.
9. Add MustDrop/drop checker with typestate semantics.
10. Add thread capture and concurrent-effect conflict checks.
11. Update presentation/audio docs and resource IDs.
