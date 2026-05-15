# Arcweft: generic `thread`, line lifetime registry, inline marks, cleanup, and drop

## 0. Goal

This document specifies how Arcweft should integrate:

- a generic `thread` construct usable outside dialogue `with:`,
- dialogue `with:` as a line-scoped specialization of generic structured concurrency,
- `'line.*` lifetime-registry access,
- built-in common visual-novel effects such as focus, portrait changes, voice, skip cleanup,
- `init:`, `thread:`, `finally:`, and `on ...:` inside line plans,
- inline dialogue marks,
- deterministic VM-level concurrent execution,
- guaranteed logical cleanup,
- `drop`, `on_drop`, `expose`, typestate-like handle states,
- current implementation and docs changes required in `Sanzentyo/arcweft`.

This is not a backwards-compatibility plan. The implementation is still in progress, so old forms may be explicitly deleted.

---

## 1. Main answer: should `thread` be generic?

Yes.

`thread` should be a generic structured-concurrency construct in the Arcweft VM, not a feature tied only to dialogue `with:`.

However, it should have different **parent lifetimes** depending on where it appears.

```text
thread in flow body
  parent = current flow fiber / lexical flow scope

thread in a named `scope`
  parent = that lexical scope

thread inside dialogue `with:`
  parent = current dialogue line lifetime `'line`

thread inside `on .mark:`
  parent = the current line, spawned at event time

thread in task fn
  parent = that task function call

thread in pure fn
  disallowed unless the fn is explicitly effectful
```

So `with:` should not define a special kind of thread.  
It should only provide a convenient parent scope and event timeline.

---

## 2. Generic `thread` forms

### 2.1 Statement form

A `thread` statement creates a scoped VM child task and registers it under the current parent scope.

```awft
thread preload_next:
    asset.preload(@asset.bg.school.classroom_day)
    alice.preload(look = .smile, voices = auto)
```

Default parent:

```text
nearest runtime scope:
  line > lexical scope > flow > task fn
```

If a statement-form thread has no explicit handle, the parent scope owns it and applies parent cleanup policy when the scope exits.

### 2.2 Named statement form

```awft
thread motion:
    alice.stage.apply(.motion.nod)
    wait 0.35s
    alice.stage.apply(.stage.expr.smile)
```

Thread name is used for:

```text
- diagnostics
- Agent/debug tree
- deterministic ordering
- optional handle path if exposed
```

### 2.3 Expression form

For explicit joining or result capture:

```awft
let t = thread compute_score {
    route_score(state)
}

let score = await t
```

`thread { ... }` expression returns a `ThreadHandle<T>` or `Need<T, ThreadError>`-like value. The exact runtime type can be chosen later, but semantically it is a joinable VM task.

Recommended type direction:

```awft
ThreadHandle<T, E = ThreadError>
```

or:

```awft
Need<T, ThreadError>
```

If it uses `Need`, existing `await ... with` machinery can handle thread joining.

### 2.4 Detached form

Detached threads must be explicit.

```awft
thread detached analytics:
    telemetry.record(route_id)
```

Detached thread requirements:

```text
- cannot capture non-static borrowed values
- cannot capture line-owned handles
- cannot capture MustDrop values unless detached/moved with explicit policy
- must require capability such as effects { thread.detach }
```

Default `thread` is scoped, not detached.

---

## 3. Relationship to existing `spawn`

The current AST has `Stmt::Spawn(Expr)`.

This document recommends:

```text
thread
  structured VM child task with parent lifetime and guaranteed finalization.

spawn
  either deprecated, or reserved for explicit detached/unstructured task requests.
```

Recommended migration:

```awft
spawn expr
```

should become one of:

```awft
thread:
    expr
```

or:

```awft
thread detached:
    expr
```

depending on whether the previous `spawn` was meant to be structured or detached.

Parser/checker should eventually warn:

```text
warning: `spawn` is ambiguous; use `thread:` or `thread detached:`
```

---

## 4. `with:` as a line-scoped specialization

Inside dialogue:

```awft
alice(.smile, voice = auto, focus = .soft):
    聞いて。[mark .release_focus]
with:
    thread motion:
        alice.stage.apply(.motion.nod)

    on .release_focus:
        'line.focus |> drop
```

The `thread motion:` here is not a special "with-thread" construct.  
It is a normal `thread` whose parent scope happens to be `'line`.

Line-scoped defaults:

```text
parent scope     = current dialogue line
cleanup policy   = line cleanup profile
registry access  = `'line.*`
start time       = after `init:` and at line start unless spawned from an event handler
finalization     = before line scope closes
```

---

## 5. `init:` inside `with:`

`init:` is only a line-plan section, not a general flow construct.

It runs synchronously before:

```text
- dialogue content reveal
- line voice playback
- user line threads
- marker traversal
- visible line-start effects
```

Common effects should not require `init:`.

Use line options:

```awft
alice(.smile, focus = .soft):
    聞いて。[p]
```

Use `init:` only for conditional or advanced setup:

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

---

## 6. `thread:` inside `with:`

Line threads are spawned after `init:` completes.

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

If a `thread:` appears inside `on .mark:`, it is spawned at marker time:

```awft
with:
    on .release_focus:
        thread release_fx:
            alice.stage.apply(.motion.look_back)
            wait 0.2s
            'line.focus |> drop
```

The spawned thread is still owned by the line unless declared `detached`.

---

## 7. VM execution model

Arcweft core is Sans I/O. The VM computes state and effect requests; it does not directly perform GPU/audio/filesystem/OS effects.

A line is a structured VM task group:

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

A flow can also have structured task groups:

```text
FlowFiber
├─ MainFlowTask
├─ ScopedThreadTask[]
├─ AwaitContinuation[]
└─ CleanupTask
```

Required runtime properties:

```text
- deterministic logical clock
- stable ordering of simultaneous effects
- no direct side effects in core
- effect requests only
- replayable FrameInput / FrameOutput
- child tasks cannot outlive the parent scope unless explicitly detached
```

Effect order for the same logical tick:

```text
1. logical time
2. parent scope order
3. event source order
4. thread declaration order
5. per-task sequence number
```

---

## 8. Rust-inspired structured concurrency

`thread` follows Rust-like scoped concurrency.

```text
- A scope owns all non-detached threads spawned inside it.
- A child thread cannot outlive its parent scope by accident.
- Live child threads are joined/cancelled/finalized at parent cleanup.
- Captured values must be safe to send/share across VM tasks.
- Unique MustDrop handles cannot be captured by multiple live threads.
- `finally:` discharges moved MustDrop values.
```

### 8.1 Move into a thread

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

### 8.2 Shared handle

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

### 8.3 Capture edge cases

Disallowed by default:

```awft
with:
    init:
        let focus =
            stage.focus(target = alice)
            |> on_drop(release(120ms))

    thread a:
        focus.request(.blur(8px))

    on .release_focus:
        focus |> drop
```

Reason: thread `a` may use `focus` after the marker handler drops it.

Diagnostic:

```text
error: thread `a` may use `focus` after another handler drops it
help: move `focus` into the thread and drop it in `finally:`
help: or convert it to a shared/cancellable handle
```

---

## 9. `finally:`

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
- parent scope cleanup
```

Rules:

```text
- `finally:` must be the last section in a thread.
- `finally:` cannot spawn non-detached scoped threads.
- `finally:` cannot wait for arbitrary input/signal.
- `finally:` cannot await unbounded work.
- `finally:` can drop, detach, log, update exposed state, and emit bounded cleanup requests.
```

---

## 10. Lifetime registry access

### 10.1 Use `'line.*`

Use:

```awft
'line.focus |> drop
```

Do not use:

```awft
line.focus |> drop
```

`'line.focus` means the value is stored in the current line lifetime registry.

### 10.2 Optional keys

If a key is not statically guaranteed:

```awft
'line.focus? : Option<FocusHandle>
```

If statically guaranteed:

```awft
'line.focus : FocusHandle
```

Static guarantee examples:

```awft
alice(.smile, focus = .soft):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

Conditional key:

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

Unsafe:

```awft
on .release_focus:
    'line.focus |> drop
```

if `focus` is conditional.

Diagnostic:

```text
error: lifetime key `'line.focus` is not statically guaranteed
help: use `'line.focus?` and handle Option<FocusHandle>
```

### 10.3 Generic lifetime registries

Inside other scopes, use their lifetime names only when they exist.

Examples:

```awft
'flow.preload_task
'scene.bgm
'thread.cache
```

But do not invent them implicitly. A registry exists only if the runtime scope defines one.

Line registry is special because every dialogue line has one.

---

## 11. Inline marks and `on`

### 11.1 Marker tag

Use `[]` for dialogue control tags:

```awft
[mark .release_focus]
```

Use `#[...]` only for expression interpolation:

```awft
#[player_name]
#[fmt(score)]
```

Delete these forms:

```awft
[hook release_focus]
#[hook release_focus]
#[mark release_focus]
hook release_focus:
```

### 11.2 Handler

```awft
with:
    on .release_focus:
        'line.focus |> drop
```

### 11.3 Marker without handler

Allowed, because marks are useful for:

```text
- transcript anchors
- test anchors
- voice marker matching
- future tool-generated handlers
- `wait mark .name` in threads
```

Optional lint:

```text
warning: unused mark `.release_focus`
```

### 11.4 Handler without marker

Error:

```awft
with:
    on .missing:
        ...
```

Diagnostic:

```text
error: marker handler `.missing` has no matching `[mark .missing]`
help: add `[mark .missing]` to the dialogue text
help: or use a non-marker trigger such as `on line.end:`
```

### 11.5 Duplicate markers

Default: duplicate marker names in one line are errors.

```awft
[mark .beat]
...
[mark .beat]
```

Diagnostic:

```text
error: duplicate mark `.beat` in one dialogue line
```

If repeatable marks are needed, add explicit repeat syntax later.

---

## 12. Line cleanup policy

Cleanup is a line option or project default, not a `with:` statement.

```awft
alice(.smile, cleanup = .fast_skip):
    聞いて。[p]
```

Project default:

```toml
[dialogue.cleanup.default]
pending_marks = "run"
visual = "snap"
audio = "stop_now"
threads = "cancel"
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

Detailed line override:

```awft
alice(
    .smile,
    cleanup = {
        skip = .fast_skip,
        cancel = .snap_and_stop,
        threads = .cancel,
    },
):
    聞いて。[p]
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
1. Enter cleanup mode for the parent scope.
2. Process pending marks according to `pending_marks`.
3. Cancel/join live child threads according to `threads`.
4. Run every cancelled thread's `finally:`.
5. Drop owned handles in lifetime registries.
6. Drop remaining MustDrop locals in reverse creation order.
7. Unregister handlers, subscriptions, exposed state.
8. Close the scope.
```

---

## 13. Focus as a built-in line option

Common focus should not require `init:`.

```awft
alice(.smile, focus = .soft):
    聞いて。[p]
```

`focus = .soft` means:

```text
- Resolve `.soft` as a FocusProfile.
- Create a line-owned FocusHandle before presentation begins.
- Store it at `'line.focus`.
- Apply enter behavior at line start.
- Apply release behavior on drop / cleanup.
```

Focus profile:

```awft
pub focus profile @focus.soft {
    target = speaker
    others = .blur(8px) & .dim(35%)
    enter = 180ms
    release = 120ms
    cleanup_visual = snap
}
```

Early release:

```awft
alice(.smile, focus = .soft, cleanup = .fast_skip):
    聞いて。[mark .release_focus]
with:
    on .release_focus:
        'line.focus |> drop
```

Multiple focus handles:

```awft
alice(.smile, focus = { main = .soft, bg = .background_dim }):
    聞いて。[mark .release_main]
with:
    on .release_main:
        'line.focus.main |> drop
```

---

## 14. Drop and decorators

### 14.1 `on_drop`

```awft
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))
```

`on_drop` attaches policy metadata.

### 14.2 `expose`

```awft
let focus =
    stage.focus(target = alice)
    |> on_drop(release(120ms))
    |> expose(@state.opening.alice_focus)
```

### 14.3 `drop`

`drop` is a compiler intrinsic.

```awft
'line.focus |> drop
drop('line.focus)
```

Preferred source style:

```awft
'line.focus |> drop
```

Override drop policy:

```awft
'line.focus |> drop(release(40ms))
```

### 14.4 Typestate semantics

Drop should be represented as a typestate transition.

```text
FocusHandle<Live> |> drop -> FocusHandle<Dropped>
```

Use-after-drop is an error:

```awft
'line.focus |> drop
'line.focus.release()
```

Diagnostic:

```text
error: use of dropped value `'line.focus`
```

### 14.5 `let _ = value`

Plain values may be discarded.

MustDrop values should reject `let _ = ...`.

```awft
let _ = stage.focus(target = alice)
```

Diagnostic:

```text
error: MustDrop value should be explicitly dropped or scoped
help: use `value |> drop`, `drop(value)`, or attach `|> on_drop(...)`
```

---

## 15. Stage look, portrait look, and focus

Stage and portrait are separate targets.

```awft
alice(.smile):
    おはよう。[p]
```

A common look may affect both stage and portrait.

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

Combined:

```awft
alice(.stage.expr.angry & .portrait.icon.smile):
    怒ってないよ。[p]
```

Use only `&` as the patch merge operator.

```awft
.smile & .casual & .motion.nod
```

`+`, `|`, and `||` are not used for patch composition.

---

## 16. Grammar summary

### 16.1 Generic flow-level thread

```text
ThreadStmt =
  "thread" ThreadModifiers? ThreadName? ":" IndentedBlock

ThreadExpr =
  "thread" ThreadModifiers? ThreadName? "{" Block "}"
```

Modifiers:

```text
detached
```

Examples:

```awft
thread preload:
    ...

let t = thread compute {
    ...
}
```

### 16.2 Line `with:` items

```text
WithItem =
    InitBlock
  | ThreadBlock
  | OnBlock
  | AtBlock
  | Let
  | Out
  | Expr
```

Examples:

```awft
with:
    init:
        ...

    thread:
        ...

    thread motion:
        ...
        finally:
            ...

    on .release_focus:
        ...

    on input.SkipLine:
        ...

    at(0.42s):
        ...

    let x = ...

    out value
```

### 16.3 Deleted forms

Do not implement:

```awft
using ...
state 'line focus drop = ...
scope focus_fx until hook release_focus:
cleanup on skip:
hook release_focus:
[hook release_focus]
#[mark release_focus]
```

---

## 17. Current implementation gaps in `Sanzentyo/arcweft`

### 17.1 AST

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

Needed additions:

```text
look
stage
portrait
focus
cleanup
```

Current `DialogueToken` has:

```text
Text
Raw
Tag
EndTag
Expr
Ruby
Escape
```

Needed addition:

```text
Mark(LineMark)
```

or a semantic pass that recognizes `Tag { name = "mark" }`.

Recommended: add `Mark`.

### 17.2 Parser

Needed changes:

```text
- first positional line option -> look
- parse focus/cleanup/stage/portrait options
- parse `[mark .name]`
- parse generic `thread` in flow/statement/expression contexts
- parse `init:`, `thread:`, `finally:`, `on ...:` inside line plans
- remove local `hook name:`
- remove `cleanup on ...:` as line-plan item
```

### 17.3 Expression parser

Needed expression support:

```awft
'line.focus
'line.focus?
'line.focus.main
```

Potential AST:

```rust
Expr::LifetimePath {
    lifetime: String,
    path: Vec<String>,
    optional: bool,
}
```

Careful conflict:

```awft
out 'label expr
break 'label expr
continue 'label
```

In control-transfer statement position, apostrophe labels remain labels.  
In expression position, apostrophe path is lifetime registry access.

### 17.4 Checker

Needed checks:

```text
- lifetime registry guaranteed-key analysis
- Option typing for unproven registry keys
- MustDrop tracking
- use-after-drop
- double-drop
- thread capture safety
- `finally:` boundedness
- concurrent exclusive-axis write detection
- pending mark cleanup traversal
```

### 17.5 VM runtime

Needed runtime model:

```text
- VM child task groups
- line task group
- deterministic effect merge
- scoped thread finalization
- cleanup policy execution
- pending marker traversal
```

Core must remain Sans I/O: no direct OS/GPU/audio side effects.

---

## 18. Required docs changes

### `docs/01-language/dialogue-character-methods-and-textbox.md`

Update:

```text
- face -> look
- add stage/portrait/focus/cleanup
- use `'line.focus`, not `line.focus`
- remove local `[hook ...]`
- add `[mark .name]` + `on .name:`
```

### `docs/01-language/dialogue-calls-scopes-cancellation.md`

Update:

```text
- add `init:`
- add `thread:`
- add `finally:`
- add `on ...:`
- remove `hook a1:`
- remove `cleanup on skip:`
- move cleanup policy into line options/profiles
```

### `docs/01-language/dialogue-control-tags-and-ruby.md`

Update:

```text
- define `[mark .name]`
- define `[]` as dialogue control tags
- define `#[...]` as expression interpolation only
- delete `[hook name]` local marker usage
```

### `docs/02-runtime/core.md`

Update:

```text
- add generic VM task group model
- add scoped `thread`
- state that `thread` outputs are deterministic effect requests
- retain Sans I/O boundary
```

### `docs/02-runtime/hooks-memoization.md`

Update:

```text
- distinguish top-level runtime hooks from line-local `on` handlers
- do not treat line-local `on` as global HookTable entries unless lowered as scoped hook records
```

### `docs/03-presentation/character-stage.md`

Update:

```text
- separate stage look and portrait look
- add `LookPatch` / `CharacterPatch`
- add focus profile
- add Live2D motion/param support
- use @ refs, not # refs
```

### `docs/03-presentation/audio.md`

Update:

```text
- replace `play voice ...` with `voice(...)` / `alice.voice(...)`
- replace # refs with @ refs
- distinguish dialogue voice option from standalone voice playback
```

---

## 19. Edge cases

### 19.1 Mark in raw text

```awft
[raw][mark .x][/raw]
```

This is text, not a mark.

### 19.2 Escaped mark

```awft
\[mark .x]
```

This is literal text.

### 19.3 Mark inside interpolation

```awft
#[some_content]
```

Static marks inside runtime-generated content are not recognized unless the expression has an explicit `ContentWithMarks` type. Default: no static mark.

### 19.4 Localized text missing required mark

If source has:

```awft
[mark .release_focus]
```

and localized text omits it, fail or warn according to policy.

Recommended default:

```text
error if mark is referenced by `on .release_focus` or `wait mark .release_focus`
```

### 19.5 Handler without mark

Error unless trigger is non-marker.

### 19.6 Duplicate mark

Error by default.

### 19.7 Optional registry drop

Prefer explicit handling:

```awft
if let Some(f) = 'line.focus? {
    f |> drop
}
```

Do not silently define `drop(Option<T>)` unless the function is named explicitly, e.g.:

```awft
'line.focus? |> drop_optional
```

### 19.8 Thread captures line key and cleanup drops it

Unsafe unless shared/cancellable.

### 19.9 Detached thread captures line key

Error.

### 19.10 `finally:` awaits unbounded Need

Error.

### 19.11 `thread` in pure function

Error unless function is effectful.

### 19.12 `thread` in `init:`

Top-level `thread` declarations inside `with:` start after `init:`.  
A nested `thread` expression inside `init:` should either be disallowed or scheduled after `init:` completes. Recommended: disallow nested line threads in `init:` for phase 1.

---

## 20. Recommended source examples

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

### Concurrent behavior in a line

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

### Flow-level generic thread

```awft
flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    thread preload_next:
        asset.preload(@asset.bg.school.classroom_day)
        alice.preload(look = .smile, voices = auto)

    bg(@asset.bg.school.classroom_day)
    alice(.smile): おはよう。[p]
}
```

### Joinable thread expression

```awft
let score_task = thread compute_score {
    route_score(state)
}

let score = await score_task
```

---

## 21. Implementation order

1. Decide `spawn` migration strategy.
2. Add generic `thread` AST and parser support.
3. Add line-plan `init:`, `thread:`, `finally:`, `on ...:`.
4. Add `[mark .name]` to dialogue text model.
5. Add `'line.*` lifetime registry expression syntax.
6. Add `look`, `stage`, `portrait`, `focus`, `cleanup` to `LineOptions`.
7. Add VM task group model and scoped cleanup.
8. Add cleanup profiles.
9. Add typechecker pass for registry key guarantees and `Option`.
10. Add MustDrop/drop checker with typestate semantics.
11. Add thread capture and concurrent-effect conflict checks.
12. Update docs listed above.
