# Arcweft requirements gap audit after pro_review11

## Scope

This audit compares the current `Sanzentyo/arcweft` repository state against the requirements proposed in the recent design discussion, excluding requirements that were explicitly withdrawn.

This document focuses on:

- current implementation status,
- missing non-withdrawn requirements,
- docs inconsistencies,
- concrete fixes,
- lifetime registry semantics,
- safe and unsafe-like lifetime promotion / upward access / global mutation.

## Sources checked

Repository files checked directly:

- `docs/reviews/pro_review11.md`
- `docs/implementation/README.md`
- `crates/arcweft-lang-syntax/src/ast.rs`
- `crates/arcweft-lang-syntax/src/parser.rs`
- `crates/arcweft-lang-syntax/src/expr.rs`
- `crates/arcweft-lang-syntax/src/types.rs`
- `crates/arcweft-lang-syntax/src/check.rs`
- `crates/arcweft-lang-syntax/src/cst.rs`
- `crates/arcweft-lang-syntax/src/tests/line_plan.rs`
- `docs/01-language/ids-and-references.md`
- `docs/02-runtime/core.md`
- `docs/02-runtime/hooks-memoization.md`
- `docs/03-presentation/audio.md`
- `docs/03-presentation/character-stage.md`

The current code already includes several changes that were only proposals earlier, especially around `LineOptions`, `LinePlanItem`, `DialogueToken::Mark`, `ThreadBlock`, `Stmt::LifetimeSet`, and flat fences.

---

## 1. Explicitly withdrawn or replaced requirements

These should **not** be reintroduced.

| Withdrawn / replaced form | Replacement |
|---|---|
| `using ...` | Do not add. Keep `use` as import syntax only. |
| `state 'line focus drop = ...` | Use line option, lifetime registry assignment, `on_drop`, and `drop`. |
| `scope focus_fx until hook release_focus:` | Use line lifetime + marks + `on .mark`. |
| `until ...` marker scopes | Use `[mark .name]` + `on .name`, or explicit `wait mark .name` in threads. |
| local `hook a1:` | Use `on .a1:`. |
| `[hook a1]` / `#[hook a1]` | Use `[mark .a1]`. |
| `line.focus` | Use `'line.focus`. |
| `face` as canonical option | Use `look`; `face` should be rejected or migrated. |
| `cleanup on skip:` inside `with:` | Use `cleanup = .profile` line option or project defaults. |
| thread-local `finally` | Use `defer` inside `thread`; keep `finally` line-level only. |
| naked `:` unnamed scope | Use `scope:`. |
| `=== block ===` alias | Use `=== scope ===`. |
| implicit close of flat fence blocks | Require explicit `=== /kind ===`. |

---

## 2. Requirements already reflected in current repository

### 2.1 pro_review11 is present and implementation README tracks it

`docs/reviews/pro_review11.md` contains the current block/thread/line registry design: brace canonical form, indent sugar, flat fence sugar, `thread`, `finally`, `defer`, `'line.*`, `[mark .name]`, and removal of local `[hook ...]`.

`docs/implementation/README.md` also records `pro_review11.md` as adopted, including `look`, `stage`, `portrait`, `focus`, `cleanup`, `[mark .name]`, line-plan `on .name:`, generic `thread`, `defer`, line-level `finally`, flat fences, `wait mark`, and `'line.*`.

Status: **mostly reflected in review docs and implementation status docs**.

### 2.2 AST already has many required nodes

Current `ast.rs` already has:

- `Stmt::Thread(ThreadBlock)`
- `Stmt::DeferBlock(Vec<Stmt>)`
- `Stmt::Defer(Expr)`
- `Stmt::LifetimeSet { target, expr }`
- `Stmt::Wait(WaitTarget)`
- `ThreadBlock`
- `ThreadModifier::Detached`
- `WaitTarget::{Duration, Mark, Expr}`
- `DialogueToken::Mark(LineMark)`
- `LineOptions` fields: `look`, `stage`, `portrait`, `focus`, `cleanup`
- `BlockStyle::Flat`
- `LinePlanItem::{Init, Thread, On, Finally, Stmt, ...}`

Status: **substantial syntax-level support exists**.

### 2.3 Parser already handles line options and many line-plan constructs

Current `parser.rs` already:

- treats the first positional dialogue line option as `look`;
- rejects `face` as non-canonical;
- parses `look`, `stage`, `portrait`, `focus`, `cleanup`;
- parses `init:`, `finally:`, `defer:`, `thread:`, `on ...:`, and `scope:`;
- parses brace forms of `init`, `finally`, `defer`, `thread`, `on`, and `scope`;
- parses `thread` and `defer` as typed statements;
- parses `wait mark .name`;
- parses lifetime registry assignment syntax such as `'line.focus <- expr`.

Status: **syntax parser has already moved beyond the original proposal**.

### 2.4 Current tests cover important new syntax

`line_plan.rs` already tests:

- `[mark .release_focus]`;
- `on .release_focus:`;
- `'line.focus |> drop`;
- `init:`;
- `thread motion:`;
- `defer { ... }`;
- `finally:`;
- flat line blocks;
- flat `thread`;
- flat `scope`;
- rejection of removed `spawn`;
- rejection of malformed flat fences;
- duplicate mark rejection;
- local `[hook ...]` rejection.

Status: **good test coverage at syntax/checker-contract level**.

### 2.5 Current checker has initial mark/lifetime validation

Current `check.rs` already:

- records a line focus guarantee when `focus` option is present;
- checks duplicate line marks;
- rejects `[hook ...]`;
- rejects `on .missing` if no `[mark .missing]` exists;
- checks `wait mark` against current line marks;
- models `'line.*` with optional reads;
- tracks dropped lifetime keys very roughly;
- rejects double drop of a lifetime key.

Status: **basic validation exists, but it is still shallow and string-key based**.

---

## 3. Missing non-withdrawn requirements

### 3.1 Full lifetime hierarchy is not yet specified or implemented

Current implementation has `Expr::LifetimePath` and lifetime key strings, but it does not yet have a full lifetime hierarchy.

Required lifetimes:

```text
'frame
'tick
'cue
'line
'scene
'flow
'session
'global
'persistent
```

Minimum rules:

```text
shorter <= longer:
  'cue <= 'line <= 'scene <= 'flow <= 'session <= 'global

'persistent is storage-backed and should not be treated as normal runtime memory.
```

Current gap:

- checker has `lifetime_guarantees: HashSet<String>` style behavior, not a proper lifetime-region environment;
- no distinction between `'line.focus`, `'flow.some_state`, `'global.settings`;
- no ownership kind per key;
- no mutation policy per lifetime;
- no capability check for mutating upper lifetimes.

Fix:

Add:

```rust
pub enum LifetimeScopeKind {
    Frame,
    Tick,
    Cue,
    Line,
    Scene,
    Flow,
    Session,
    Global,
    Persistent,
    Named(String),
}

pub struct LifetimeKey {
    scope: LifetimeScopeKind,
    path: Vec<String>,
}

pub enum LifetimeAccessMode {
    Read,
    Write,
    MoveOut,
    Drop,
    Expose,
}
```

Then use `LifetimeKey` instead of raw `"line.focus"` strings.

---

### 3.2 Upward lifetime access is not designed enough

User requirement:

> game script should be stricter than usual Rust by default, but allow controlled unsafe-like promotion and upper-lifetime access when useful, such as mutating global state or writing `'flow` state from a line.

Current docs do not sufficiently specify:

```awft
'flow.flags.seen_alice_intro <- true
'global.settings.skip_seen <- true
```

Recommended rule:

```text
Reading upper lifetime from lower scope:
  allowed if the key exists or optional access is used.

Writing upper lifetime from lower scope:
  allowed only if:
    - current function/flow has required effect/capability,
    - written value is valid for the target lifetime,
    - write operation is deterministic and replayable,
    - conflict policy is explicit when multiple threads may write.
```

Example:

```awft
alice:
    見たことにする。[p]
with:
    on .seen:
        'flow.flags.seen_alice_intro <- true
```

This should lower to a deterministic VM event/update, not direct mutation.

Add effect requirement:

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow) }
{
    ...
}
```

or capability on flow/module:

```awft
requires capability @cap.state.write_flow
```

---

### 3.3 Safe promotion and unsafe-like promotion are missing

Needed operations:

```awft
value |> promote('flow)
value |> promote('scene)
value |> promote('global)
value |> promote_unchecked('flow, reason = "script migration")
```

Safe `promote` should require:

```text
- value owns its data;
- value contains no references/handles tied to shorter lifetime;
- value implements Promote<'target>;
- value is serializable/replay-safe if target is session/global/persistent;
- dropping original value does not invalidate promoted value.
```

Unsafe-like `promote_unchecked` should be possible but gated.

Recommended syntax:

```awft
unsafe lifetime {
    'flow.cache.last_focus <-
        line_summary
        |> promote_unchecked('flow, reason = "cached debug-only summary")
}
```

Rules for `unsafe lifetime`:

```text
- allowed only in modules/flows with explicit capability;
- must require a reason string;
- emits trace/debug diagnostic;
- still cannot violate deterministic core boundaries;
- still cannot perform host I/O directly.
```

Do not let `unsafe` bypass the Sans I/O model. It only bypasses static lifetime proof with runtime/audit obligations.

---

### 3.4 Global mutation needs a script-safe design

Requirement:

> globalな状態のものを変更する、lineの中から、'flow のものを変更するなど？

Recommended API:

```awft
'global.settings.skip_seen <- true
'flow.flags.seen_alice_intro <- true
'session.unlocks.alice_route <- true
```

But these should not be raw memory writes. In Arcweft core, they should lower to deterministic state update events:

```rust
EffectRequest::Signal(...)
GameEvent::StateWrite { key, value, scope, source }
```

Rules:

```text
'global
  write requires global-state capability;
  value must be Persistable or explicitly transient-global.

'session
  write requires session-state capability;
  value must be serializable in save snapshot or marked volatile.

'flow
  line may write if flow has state.write('flow) effect.

'scene
  line may write if scene context exists.

'line
  line may freely write own registry during line.
```

Conflict policy for concurrent thread writes:

```awft
'flow.counter <- update(|x| x + 1)
'flow.flags.seen <- true
'global.settings <- merge_patch({ skip_seen = true })
```

Avoid allowing unsynchronized raw writes from multiple threads to the same key.

---

### 3.5 Lower lifetime access from upper scope must be forbidden

Invalid:

```awft
thread preload_next {
    'line.focus |> drop
}
```

outside a line.

Invalid:

```awft
flow @flow.opening opening {
    'line.focus |> drop
}
```

unless inside a currently active line context.

Diagnostic:

```text
error: lifetime `'line` is not available in this scope
```

Current checker appears to treat lifetime paths by string key, so this availability check is missing or incomplete.

---

### 3.6 `drop`, `on_drop`, `expose`, `share`, `detach`, `promote` need real intrinsic semantics

Current checker has shallow recognition of lifetime pipes to `drop`, `drop_optional`, and `on_drop`, but there is no full intrinsic model.

Required intrinsics:

```text
drop
drop_optional
on_drop
expose
share
detach
promote
promote_unchecked
clone_owned
```

Required semantics:

```text
drop(T<Live>) -> T<Dropped>
on_drop(T<Live>, policy) -> T<Live with DropPolicy>
expose(T, id) -> T with debug/agent exposure
share(T<Unique>) -> Shared<T>
detach(T scoped) -> T detached or DetachedHandle
promote(T, 'target) -> T<'target> if safe
promote_unchecked(T, 'target, reason) -> T<'target> under unsafe lifetime capability
```

This also aligns with the uploaded typestate/capability note: operations should transform state/capability rows rather than dynamically attach traits.

---

### 3.7 MustDrop and typestate checking is not complete

Current `TypeKind` is still minimal: it has primitive types, `Ref`, `Need`, `Result`, `Named`, `Tuple`, `Unit`, `Never`, but no `MustDrop`, `Live/Dropped`, thread handle, focus handle, or typestate rows.

Required additions:

```rust
pub enum TypeKind {
    ...
    Handle {
        name: String,
        lifetime: LifetimeScopeKind,
        state: HandleState,
        must_drop: bool,
    },
    ThreadHandle(Box<TypeKind>),
    Shared(Box<TypeKind>),
    Option(Box<TypeKind>),
    Function { ... },
    CharacterPatch(String),
    FocusPatch,
}

pub enum HandleState {
    Live,
    Dropped,
    Detached,
    MovedOut,
}
```

Required checker passes:

```text
- use-after-drop
- double-drop
- all-paths MustDrop discharge
- `defer` stack validation
- line `finally` uniqueness and boundedness
- thread cancellation safety
- capture safety
```

Current `dropped_lifetime_keys` only handles dropped registry keys by string and is not enough.

---

### 3.8 Thread capture safety is not complete

Current checker recursively checks thread body and rejects active borrows at a "thread suspension boundary", but it does not model:

```text
- move into thread;
- shared capture;
- unique handle capture;
- detached thread restrictions;
- thread parent lifetime;
- thread result/join type;
- `finally`/`defer` interaction with moved handles;
- concurrent access conflict.
```

Required design:

```awft
thread motion {
    let lease = move lease

    defer {
        lease |> drop
    }

    alice.stage.apply(.motion.nod)
}
```

Detached thread:

```awft
thread detached analytics {
    telemetry.record(route_id)
}
```

Detached restrictions:

```text
- cannot capture 'line keys;
- cannot capture non-static borrow;
- cannot capture MustDrop scoped handle unless detached first;
- requires `effects { thread.detach }`.
```

---

### 3.9 Generic `thread` expression / joinable handle is not implemented

Requirement remains:

```awft
let t = thread compute_score {
    route_score(state)
}

let score = await t
```

Current AST has `Stmt::Thread(ThreadBlock)`, but no `Expr::Thread` or return type for joinable threads.

Fix:

Add:

```rust
Expr::Thread {
    block: ThreadBlock,
}
```

or define a statement-only model and explicitly reject expression-form thread until implemented.

Docs must not claim expression-form thread is available unless implemented.

---

### 3.10 `&` look-patch merge operator is missing

Requirement remains:

```awft
alice(.smile & .casual & .motion.nod):
    ...
```

Current `BinaryOp` does not include a single-ampersand merge operator. It has logical `And` but no `Merge`.

Fix:

```rust
pub enum BinaryOp {
    ...
    Merge, // single `&`
}
```

Parser binding:

```text
&& -> logical And
&  -> patch/capability merge
```

Checker:

```text
LookPatch<C> & LookPatch<C> -> LookPatch<C>
FocusPatch & FocusPatch -> FocusPatch
conflicting exclusive axis -> error
```

This is a major missing feature.

---

### 3.11 Function default parameters and real partial/currying are missing

Requirements not withdrawn:

```text
- implicit currying is allowed;
- unused implicit partial warns;
- default parameters are not implicitly curried;
- `_` opens default/optional parameters explicitly;
- ordinary function overloading is not planned;
- operator overloading can be trait-like.
```

Current `FnParam` has only `doc`, `pattern`, and `ty`; no `default`. `parse_fn_param` parses only `pattern: Type`.

Fix:

```rust
pub struct FnParam {
    doc: Option<DocBlock>,
    pattern: Pattern,
    ty: TypeRef,
    default: Option<Expr>,
}
```

Need checker/function env:

```rust
FunctionType {
    param_groups,
    return_type,
}
```

Need diagnostics:

```text
UnusedImplicitPartial
UnusedExplicitPartial
DefaultArgOpenedByPlaceholder
```

This is still not implemented.

---

### 3.12 `alice2 = alice(voice=auto)` and `alice2(.smile):` need semantic support

Syntax likely parses because speaker line heads are string-based and line options support look. But type checking does not yet model:

```text
Speaker<C>
SpeakerPreset<C>
SpeakerPreset<C>.call(look: CharacterPatch<C>, options: SayOptions<C>)
```

Fix:

```rust
TypeKind::Speaker(CharacterId)
TypeKind::SpeakerPreset(CharacterId)
TypeKind::CharacterPatch(CharacterId)
```

Then:

```text
alice(voice = auto) -> SpeakerPreset<Alice>
alice2(.smile) -> SpeakerPreset<Alice>
alice2(.smile)[...] -> DialogueLine
```

Also ensure `alice2(.smile):` can lower without assuming `alice2.say`.

---

### 3.13 `surface character ... as alice` remains missing

Requirement was proposed and not explicitly withdrawn.

Current `EntityDeclKind` only includes `Character`, `Component`, `Activity`, `Signal`, `Layer`, and `EntityDeclItem` has no `surface`/alias field.

Fix:

```awft
pub surface character @character.alice Alice as alice {
    ...
}
```

Add:

```rust
pub struct SurfaceDecl {
    alias: Option<String>,
}

pub struct EntityDeclItem {
    ...
    surface: Option<SurfaceDecl>,
}
```

Parser/CST must recognize `surface character`.

If you decide to abandon `surface`, record that explicitly in docs and replace it with a different alias declaration. Otherwise it is a missing requirement.

---

### 3.14 Resource/entity kinds are incomplete

Requirements:

```text
- voice
- se
- bgm
- audio bus
- mixer snapshot
- ducking
- textbox
- motion
- rig
```

Current `EntityDeclKind` lacks these.

Current `EntityKind` also lacks `Voice`, `Se`, `Bgm`, `AudioBus`, `MixerSnapshot`, `Ducking`, `Motion`, `Rig`.

Docs use `pub audio bus`, `pub bgm`, `pub voice profile`, etc., but parser entity families do not support them fully.

Fix:

```rust
pub enum EntityDeclKind {
    Character,
    Component,
    Activity,
    Signal,
    Layer,
    Textbox,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
}
```

and matching `EntityKind` entries.

---

### 3.15 Voice relative ID policy is still inconsistent

Requirement:

```awft
mod hoge

flow @flow.fuga fuga {
    alice.voice(@voice:.sigh)
}
```

should resolve to:

```text
@voice.alice.hoge.fuga.sigh
```

or a defined project policy equivalent.

Current `ids-and-references.md` still says `voice=auto` becomes:

```text
@voice.{locale}.{speaker}.{flow}.{scope_path}.{line_suffix_or_slot}
```

and gives `@voice.ja-JP.alice.opening.greeting`.

This conflicts with the later requirement that logical voice ID should be locale-free and speaker/module/flow scoped, with locale as a resource variant.

Fix docs:

```text
logical voice id:
  @voice.{speaker}.{module_path}.{flow}.{scope_path}.{suffix}

locale-specific resource:
  assets/voice/{locale}/{speaker}/{module_path}/{flow}/{suffix}.ogg
```

Example:

```text
@voice.alice.hoge.fuga.sigh
assets/voice/ja-JP/alice/hoge/fuga/sigh.ogg
assets/voice/en-US/alice/hoge/fuga/sigh.ogg
```

If the old locale-first rule is kept, explicitly reject the new requirement. Otherwise update docs.

---

### 3.16 Resource directory mapping and rename tooling are not documented enough

Requirement:

```text
- resources derive IDs from directory names;
- LSP and CLI support rename/move/update refs/keep-id/add-alias;
- resource manifests separate EntityId/PublicId/source_path/semantic_hash.
```

Current docs still mention rename generally in `ids-and-references.md`, but do not fully specify resource mapping for voice/bg/se/bgm/live2d/presentation assets.

Add doc section:

```text
assets/bg/... -> @asset.bg...
assets/voice/{locale}/{speaker}/{module}/{flow}/x.ogg -> @voice...
assets/se/... -> @se...
assets/bgm/... -> @bgm...
assets/character/... -> @asset.char...
assets/live2d/... -> @asset.live2d...
```

CLI/LSP required commands:

```bash
arcw resource scan
arcw resource check
arcw resource fix
arcw rename @voice.alice.hoge.fuga.sigh @voice.alice.hoge.fuga.soft_sigh
arcw resource move ... --update-refs
arcw resource move ... --keep-id
```

---

### 3.17 Standalone voice playback still conflicts in docs

Requirement:

```awft
alice.voice(@voice:.sigh)
voice(@voice.system.notice)
```

Docs still contain:

```awft
play voice speech.audio
play voice #cue.voice.alice.001 spatial { ... }
command audio.ensure_bgm(...)
```

Fix:

```awft
voice(speech.audio, speaker = alice)
alice.voice(@voice:.sigh)
bgm(@bgm.alice_theme, section = @music.intro, fade_in = 1s)
```

Also replace remaining `#mix`, `#duck`, `#listener`, `#audio_source`, `#cue` examples with `@...` forms or remove them.

---

### 3.18 Character-stage docs still use old `#` refs and face-centric model

`character-stage.md` still has old examples like:

```awft
pub character #character.alice Alice { ... }
sprite_sheet #sprite...
part face { ... }
expression smile { face = smile; ... }
```

Requirement:

```text
- use @ refs;
- distinguish stage look and portrait look;
- use `look`, not `face`, as canonical line option;
- add LookPatch / CharacterPatch with `&`;
- include Live2D rig/motion/params;
- include focus profile.
```

Fix that doc wholesale.

---

### 3.19 Dialogue docs likely still need synchronization with pro_review11

Even though `docs/reviews/pro_review11.md` is current, the stable language docs still need to be updated so review notes are not the only source of truth.

Required stable docs changes:

```text
docs/01-language/dialogue-character-methods-and-textbox.md
docs/01-language/dialogue-calls-scopes-cancellation.md
docs/01-language/dialogue-control-tags-and-ruby.md
docs/01-language/syntax.md
docs/01-language/grammar.md
docs/02-runtime/core.md
docs/03-presentation/audio.md
docs/03-presentation/character-stage.md
docs/04-tooling/lsp.md
docs/04-tooling/cli.md
```

---

## 4. Lifetime design: recommended final model

### 4.1 Safe default

Use strict static checks by default:

```text
- lower lifetime value cannot escape to higher lifetime;
- upper lifetime mutation requires effects/capabilities;
- registry key access is Option unless guaranteed;
- MustDrop values must be discharged;
- no direct host I/O in core;
- all mutation lowers to deterministic VM state/effect events.
```

### 4.2 Lifetime hierarchy

```text
'cue <= 'line <= 'scene <= 'flow <= 'session <= 'global
```

`'persistent` is storage-backed and should be treated separately.

### 4.3 Read access

From a line:

```awft
let x = 'flow.flags?
let y = 'global.settings?
```

Read is allowed if the scope exists. Non-optional access is allowed only if statically guaranteed.

### 4.4 Write access

```awft
'flow.flags.seen_alice_intro <- true
'global.settings.skip_seen <- true
```

Allowed only with effect/capability:

```awft
flow @flow.opening opening(state: GameState)
effects { state.write('flow), state.write('global) }
{
    ...
}
```

Writes lower to deterministic state events, not direct mutation.

### 4.5 Safe promotion

```awft
'flow.summary <-
    line_summary
    |> promote('flow)
```

Allowed only if:

```text
T: Promote<'flow>
T contains no shorter-lifetime references
T is serializable if needed
T has no live scoped handles unless detached/promoted safely
```

### 4.6 Unsafe-like promotion

```awft
unsafe lifetime {
    'flow.debug.last_line <-
        value
        |> promote_unchecked('flow, reason = "debug state across line")
}
```

Rules:

```text
- requires explicit capability;
- requires reason string;
- records diagnostic/trace;
- cannot bypass Sans I/O;
- cannot bypass determinism;
- should be banned in release unless project allows.
```

### 4.7 Upward mutation from child threads

If multiple threads may update the same higher-lifetime key, require an atomic/merge operation:

```awft
'flow.counter <- update(|x| x + 1)
'flow.flags <- merge_patch({ seen_alice_intro = true })
```

Raw concurrent assignment to the same upper key should warn or error.

### 4.8 Downward access

Outside a line:

```awft
'line.focus
```

is an error.

Inside a line, `'line.*` is available. Inside a thread owned by a line, `'line.*` may be available only under capture rules.

---

## 5. Documentation inconsistency fixes

### 5.1 `#` vs `@`

Fix all old `#entity` examples in stable docs.

Replace:

```awft
#character.alice
#voice.alice.001
#bus.voice
#bgm.alice_theme
```

with:

```awft
@character.alice
@voice.alice.001
@bus.voice
@bgm.alice_theme
```

`#` remains for `#[...]` expression interpolation / attributes where applicable, not entity references.

### 5.2 `play voice` / `stop voice`

Replace:

```awft
play voice speech.audio
play voice #cue.voice.alice.001 spatial { ... }
stop voice fade=40ms
```

with:

```awft
voice(speech.audio, speaker = alice)
alice.voice(@voice:.sigh)
'line.voice |> drop(stop_now)
```

or handle methods if the final API keeps them:

```awft
voice_handle.stop(fade = 40ms)
```

### 5.3 `face` option

Replace canonical `face = ...` docs with `look = ...`.

Allow parser to reject `face` with migration diagnostic as current parser does.

### 5.4 `alice.say()[...]` vs `alice[...]`

Make stable docs say:

```text
alice[...] is compact canonical content application.
alice.say()[...] is explicit detailed method form.
alice: is indentation/dialogue sugar.
```

Do not describe `alice[...]` only as a shorthand for `.say()` without also defining content application.

### 5.5 Local hook syntax

Delete local `[hook ...]` examples. Keep top-level hook docs.

Use:

```awft
[mark .name]
with {
    on .name { ... }
}
```

### 5.6 Block styles

Stable docs should state:

```text
brace is canonical;
indent and flat are source styles;
BlockStyle is formatting metadata;
lint/formatter can require/convert styles.
```

### 5.7 `{}` vs `scope {}`

Document context-sensitive equivalence:

```text
statement/block item position:
  { ... } == scope { ... }

expression position:
  { ... } is block expression;
  scope { ... } is explicit scope expression.
```

---

## 6. Implementation tasks by priority

### P0: remove contradictions / stabilize docs

1. Update stable docs from `pro_review11.md`.
2. Remove old `#` entity refs in stable docs.
3. Replace `play voice` and command-style audio examples.
4. Replace `face` canonical option with `look`.
5. Remove local `[hook]` examples.
6. Update IDs docs for voice logical ID policy.

### P1: finish parser/source style

1. Ensure flat fence parser is fully integrated, not only helper-level.
2. Ensure `BlockStyle::Flat` is preserved in all flat-parsed blocks.
3. Add `scope:` tests and `{}` vs `scope {}` context tests.
4. Add expression-form `thread` or explicitly reject it.

### P2: semantic checker

1. Add real lifetime environment with `LifetimeKey`.
2. Add capability/effect checks for writes to `'flow`, `'session`, `'global`.
3. Add safe `promote` and unsafe `promote_unchecked`.
4. Add MustDrop/typestate model.
5. Add thread capture checker.
6. Add concurrent write/axis conflict checker.
7. Add default parameter and currying checker.

### P3: runtime/VM

1. Add VM task group model.
2. Add line task group model.
3. Add thread cleanup/defer execution.
4. Add line finalization order.
5. Add cleanup policy execution.
6. Add deterministic merge of concurrent effect requests.
7. Add replay trace for thread/cleanup behavior.

### P4: tooling

1. LSP block-style code actions.
2. LSP resource rename/move/update refs.
3. CLI resource scan/check/fix.
4. CLI block style conversion.
5. Diagnostics for missing localized marks.

---

## 7. Short answer on lifetime readiness

Current design is **headed in the right direction**, because it has:

```text
- `'line.*` syntax in Expr;
- line focus guarantees;
- optional registry reads;
- lifetime assignment syntax;
- basic drop tracking for lifetime keys;
- line mark validation.
```

But it is **not yet sufficient** for the requested game-script lifetime model, because it lacks:

```text
- complete lifetime hierarchy;
- upper-lifetime write capability checks;
- safe promotion;
- unsafe-like promotion;
- global/session/flow mutation semantics;
- full MustDrop typestate;
- thread capture safety;
- deterministic conflict handling for concurrent writes.
```

The next design step should be to formalize `LifetimeKey`, `LifetimeScopeKind`, `promote`, `promote_unchecked`, and effect/capability rules for writes such as:

```awft
'flow.flags.seen_alice_intro <- true
'global.settings.skip_seen <- true
```
