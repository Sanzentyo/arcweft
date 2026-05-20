According to a document from 2026-05-16, arcweft 関連の質問では、まず arcweft の理念や構造を把握・分析する前提があります。
以下はその前提と、現在の `Need` / `Source` / `Stream` / `Seq` / scheduler / line plan / `yield` 仕様を踏まえた設計書です。

---

# Arcweft Generator / Yield / Stream 設計書

## 1. 概要

Arcweft には既に `yield` が存在する。ただし、本設計では `yield` を汎用 coroutine / 汎用 generator の入口として広げるのではなく、以下の3種類の明示的な生成コンテキストに限定する。

```text
seq { ... yield ... }
  pure lazy sequence

stream { ... yield ... }
stream fn ... -> Stream<T, E> { ... yield ... }
  stream transform / granted-port transform

source @source.id: Source<T, E> { ... yield ... }
  permissioned, replayable, backpressure-aware live source
```

既存仕様では、`yield expr` は `Source<T, E>` / generator-like streams 用として予約され、dialogue line plan では使わないことが明記されている。
また、Arcweft は device abstraction として汎用 generator を主役にせず、`Need<T,E>`、`Stream<T,E>`、`Watch<T>`、generator syntax を分ける方針を既に採っている。

本設計の目的は、既存の `yield` を以下の観点で整理し、コンパイラ・runtime・scheduler・文法サマリへ一貫して反映することである。

```text
- `yield` の許可位置を明確化する
- `Seq` と `Source` を混同しない
- device acquisition を generator に隠さない
- `Need` / `await with` / scheduler と衝突しない
- `thread` / `together` / `.parallel(...)` と役割を分離する
- replay / determinism / backpressure / privacy を維持する
- line plan の `out` と stream の `yield` を混同しない
```

---

## 2. 現行仕様との関係

### 2.1 既に存在する要素

現行仕様では、関数種別に `stream fn` が含まれている。文法サマリの `FunctionKind` には `fn`、`task fn`、`dialogue fn`、`stream fn` が定義されている。

実装上も、AST には `FunctionKind::Stream` が存在する。
さらに `Stmt::Yield(Expr)` も存在するため、`yield` は構文要素として既に導入済みである。

一方で、canonical grammar としての `grammar.md` には `YieldStmt := 'yield' Expr` が明示されていない。`grammar.md` は「現在の Arcweft surface grammar の canonical summary」とされているため、この不足は仕様の穴として扱う。

### 2.2 `Seq` と `Source` の分離

`Seq<T>` は pure lazy sequence であり、`Source<T,E>` は live external stream である。`Source` は permission、privacy、backpressure、cancel、record/replay を持つため、`Seq` へ暗黙変換しない。

したがって、`yield` を導入する場合も、以下を分ける。

```text
seq yield
  pure / deterministic / synchronous lazy sequence

stream yield
  stream transform / state machine

source yield
  live source event emission
```

### 2.3 `Need` / scheduler との関係

Arcweft の `Need<T,E>` は `T` へ暗黙変換しない。flow で使用するには、`try await ... with { pending ... }` のように、待機時の表示や失敗時の振る舞いを明示する。

scheduler についても、runtime adapter task は `ensure_task` を通して作られ、同一 key の task は join される。source-level `thread` は VM-scoped fiber であり、OS task を直接作るものではない。

したがって、generator / `yield` は task 起動や permission prompt を隠してはならない。

---

## 3. 設計原則

## 3.1 `yield` は生成であり、並列実行ではない

`yield` は item を生成し、現在の generator-like state machine を suspension する。

```text
yield:
  item production

thread:
  scoped VM child task

together:
  same-tick effect grouping

traverse(...).parallel(limit = N):
  Need / Task の並列実行

select / poll:
  Source / Stream consumption
```

line plan の `together { ... }` は thread primitive ではなく、同一 timeline tick に effect request をまとめるものとされている。長時間実行する処理は `Need<T,E>` と explicit pending handling を使う。

## 3.2 device を generator で開かない

device stream には permission、hotplug、disconnect、backpressure、replay、privacy があるため、普通の lazy generator として扱わない。現行仕様でも、device stream は permissioned, cancelable, backpressure-aware source of timestamped events と定義されている。

禁止例:

```awft
stream fn unsafe_open_mic() -> Stream<AudioFrame, AudioError> {
    let mic = capture.microphone(@capture.player_microphone) // error
    for frame in mic.frames() {
        yield frame
    }
}
```

許可例:

```awft
let mic =
    try await capture.microphone(@capture.player_microphone) with {
        pending p => {
            scene.show(@scene.permission_wait)
            progress.set(p.ratio)
        }
        denied _ => return Ok(FlowExit::Goto(@flow.mic_optional))
    }

let levels = rms_level(mic.frames())
```

## 3.3 `out` と `yield` を混同しない

`out expr` は line plan、cue block、content scope から値を外へ出す control transfer である。`yield expr` は Source / generator-like stream 用であり、dialogue line plan では使わない。

```awft
alice[長い台詞です。[p]]
with 'line {
    cancel on input .SkipLine:
        text.flush(mode = .Instant)
        out 'line .Skipped
}
```

line plan の値は `out`。stream item は `yield`。

---

## 4. 用語定義

| 用語                  | 意味                                                                     |
| ------------------- | ---------------------------------------------------------------------- |
| `Seq<T>`            | pure lazy sequence。外部入力、permission、wall-clock、backpressure を持たない       |
| `Stream<T,E>`       | ordered stream transform。主に既存 stream / granted port を処理する              |
| `Source<T,E>`       | live external source。permission、privacy、backpressure、cancel、replay を持つ |
| `Need<T,E>`         | 非同期 acquisition / realization。暗黙 force しない                             |
| `yield`             | `Seq` / `Stream` / `Source` item を生成する statement                       |
| suspension boundary | borrow が跨げない境界。`await`、`yield`、`thread` など                             |
| generator context   | `yield` が有効な構文・型チェック文脈                                                 |
| source handler      | `source` block 内の `on item` / `on error` / `on disconnected` など        |

---

## 5. Surface Syntax

## 5.1 `seq { ... }`

`seq` block は pure lazy sequence を作る。

```awft
let visible_choices = seq {
    for c in opening_choices() {
        if c.enabled {
            yield choice_to_view(state)(c)
        }
    }
}
```

型:

```text
seq { yield e }
  e: T
  result: Seq<T>
```

制約:

```text
- pure only
- `await` 禁止
- `Need` force 禁止
- device / Source acquisition 禁止
- signal write / log / command 発行は禁止
- borrowed value が yield boundary を跨ぐことは禁止
```

## 5.2 `stream { ... }`

`stream` block は stream transform を作る。

```awft
let levels = stream {
    for frame in mic.frames() {
        yield rms(frame)
    }
}
```

型:

```text
stream { yield e }
  e: T
  result: Stream<T, E>
```

`E` は以下のいずれかで決定する。

```text
- expected type から決定
- source expression の error type から決定
- 明示 annotation から決定
```

例:

```awft
let levels: Stream<f32, AudioError> = stream {
    for frame in mic.frames() {
        yield rms(frame)
    }
}
```

## 5.3 `stream fn`

`stream fn` は named stream transform を定義する。

```awft
stream fn rms_level(
    frames: Stream<AudioFrame, AudioError>,
) -> Stream<f32, AudioError> {
    for frame in frames {
        yield frame.samples
            .seq()
            .map(|s| s * s)
            .mean()
            .sqrt()
    }
}
```

制約:

```text
- 返り値は Stream<T,E> または Source<T,E>
- 各 yield expr は T に型付けされる
- device acquisition 禁止
- permission prompt 禁止
- borrowed callback buffer が yield を跨ぐことは禁止
```

## 5.4 `source` block

`source` block は live source を宣言する。

```awft
pub source @source.face_camera_frames: Source<VideoFrameHandle, CaptureError> {
    from capture.camera(@capture.face_camera)
    backpressure = latest
    replay = hash_only
    privacy = transient

    on item frame => yield frame
    on disconnected => signal.set(@signal.camera_connected, false)
    on error e => log.warn("camera stream error {err:?}", err = e)
}
```

`source` block は free-form coroutine ではない。現行仕様でも、source block は declarative / policy-driven であり、compiler は backpressure policy、replay policy、borrow crossing などを検査する。

必須 policy:

```text
from
backpressure
replay
privacy
```

`privacy` は build target や product policy により default を持ってよいが、手書き source では明示を推奨する。

---

## 6. Canonical Grammar 追加案

`grammar.md` に以下を追加する。

```text
YieldStmt :=
    'yield' Expr

ComputationBlock :=
    ('result' | 'task' | 'seq' | 'stream') Block

StreamFnDecl :=
    Visibility? 'stream' 'fn' Ident GenericParams? ParamGroup+
    '->' StreamReturnType
    WhereClause?
    Contract*
    Block

StreamReturnType :=
    'Stream' '<' Type ',' Type '>'
  | 'Source' '<' Type ',' Type '>'

SourceDecl :=
    Visibility? 'source' SourceId? Ident? ':' SourceType SourceBlock

SourceId :=
    EntityRef
  | RelativeId
  | FamilyRelativeEntityRef

SourceType :=
    'Source' '<' Type ',' Type '>'

SourceBlock :=
    '{' SourceBlockItem* '}'

SourceBlockItem :=
    SourceHeader
  | SourceHandler
  | ContractClause

SourceHeader :=
    'from' Expr
  | 'backpressure' '=' BackpressurePolicy
  | 'replay' '=' ReplayPolicy
  | 'privacy' '=' PrivacyPolicy

BackpressurePolicy :=
    'latest'
  | 'bounded' '(' 'capacity' '=' IntLiteral ',' 'overflow' '=' OverflowPolicy ')'
  | 'blocking_not_allowed'

OverflowPolicy :=
    'drop_oldest'
  | 'drop_newest'
  | 'error'
  | 'coalesce'

ReplayPolicy :=
    'full'
  | 'hash_only'
  | 'summary'
  | 'event_only'
  | 'none'

PrivacyPolicy :=
    'transient'
  | 'redacted'
  | 'recordable'
  | 'private'

SourceHandler :=
    'on' SourceEventPattern '=>' SourceHandlerBody

SourceEventPattern :=
    'item' Pattern
  | 'error' Pattern
  | 'progress' Pattern
  | 'disconnected'
  | 'permission_revoked'
  | 'end'

SourceHandlerBody :=
    YieldStmt
  | ExprStmt
  | Block
```

既存の `Stmt` grammar には以下を追加する。

```text
Stmt :=
    ...
  | YieldStmt
```

ただし `YieldStmt` は parser では受けるが、semantic checker で generator context 外を拒否する。

---

## 7. 許可コンテキストと禁止コンテキスト

## 7.1 許可

```text
- seq block
- stream block
- stream fn body
- source handler
```

## 7.2 禁止

```text
- ordinary fn
- task fn
- dialogue fn
- parser
- flow body
- dialogue line plan
- line-plan thread / at / together / on / cancel / defer
- dialogue text
- memo fn
- hook
- reducer
- view
```

## 7.3 診断

flow 本体:

```awft
pub flow @flow.opening opening(state: GameState) {
    yield state
}
```

診断:

```text
error: `yield` is only valid in `seq`, `stream`, or `source` contexts
help: use `return` to leave a flow
help: use `out` only inside line-plan/content output scopes
```

line plan:

```awft
alice[テスト[p]]
with {
    yield .Done
}
```

診断:

```text
error: `yield` cannot be used in a dialogue line plan
help: use `out .Done` to produce a line result
```

ordinary function:

```awft
fn numbers() -> Seq<i32> {
    yield 1
}
```

診断:

```text
error: `yield` requires an explicit generator block
help: write `seq { yield 1 }`
```

---

## 8. 型チェック設計

## 8.1 GeneratorContext

semantic checker に `YieldContext` を導入する。

```rust
#[derive(Clone, Debug)]
pub enum YieldContext {
    None,

    Seq {
        item_ty: Ty,
    },

    Stream {
        item_ty: Ty,
        error_ty: Ty,
    },

    Source {
        item_ty: Ty,
        error_ty: Ty,
        source_policy: SourcePolicyRequirement,
    },
}
```

checker は traversal 中に context stack を持つ。

```rust
struct CheckContext {
    yield_stack: Vec<YieldContext>,
    suspension_boundaries: Vec<SuspensionBoundary>,
    effect_context: EffectContext,
}
```

`yield expr` を見た時:

```rust
fn check_yield(expr: &Expr, ctx: &mut CheckContext) -> Result<Ty, Diagnostic> {
    match ctx.current_yield_context() {
        YieldContext::None => error_yield_outside_context(),

        YieldContext::Seq { item_ty } => {
            let actual = check_expr(expr, ctx)?;
            unify(actual, item_ty)?;
            check_pure_context(expr)?;
            mark_suspension_boundary(SuspensionKind::Yield);
            Ok(Ty::Unit)
        }

        YieldContext::Stream { item_ty, .. } => {
            let actual = check_expr(expr, ctx)?;
            unify(actual, item_ty)?;
            check_no_forbidden_borrow_crossing(ctx)?;
            mark_suspension_boundary(SuspensionKind::Yield);
            Ok(Ty::Unit)
        }

        YieldContext::Source { item_ty, .. } => {
            let actual = check_expr(expr, ctx)?;
            unify(actual, item_ty)?;
            check_source_policy_complete(ctx)?;
            check_no_forbidden_borrow_crossing(ctx)?;
            mark_suspension_boundary(SuspensionKind::Yield);
            Ok(Ty::Unit)
        }
    }
}
```

## 8.2 `yield` の型

`yield expr` statement 自体の型は `Unit` とする。

```text
yield e: Unit
```

ただし、周辺 generator context の item type に制約を与える。

```text
yield e inside Seq<T>       => e: T
yield e inside Stream<T,E>  => e: T
yield e inside Source<T,E>  => e: T
```

## 8.3 `seq` block の型推論

```awft
let xs = seq {
    yield 1i32
    yield 2i32
}
```

推論:

```text
yield 1i32 => item_ty = i32
yield 2i32 => item_ty = i32
result     => Seq<i32>
```

異なる型の `yield`:

```awft
let xs = seq {
    yield 1i32
    yield "x"
}
```

診断:

```text
error: yielded item types do not match
note: first yield has type i32
note: this yield has type String
```

## 8.4 `stream fn` の戻り型

`stream fn` は明示戻り型を要求する。

```awft
stream fn numbers() -> Stream<i32, Unit> {
    yield 1i32
}
```

戻り型なし:

```awft
stream fn numbers() {
    yield 1i32
}
```

診断:

```text
error: `stream fn` must declare `-> Stream<T, E>` or `-> Source<T, E>`
help: write `stream fn numbers() -> Stream<i32, Unit>`
```

## 8.5 `yield` なし `stream fn`

原則エラー。

```awft
stream fn empty() -> Stream<i32, Unit> {
    ()
}
```

診断:

```text
error: `stream fn` does not yield any item
help: return an explicit `empty_stream<i32, Unit>()` if this is intended
```

---

## 9. Suspension Boundary / Borrow ルール

`yield` は `await` と同じく suspension boundary である。現行仕様では `await`、`yield frame`、`select`、`thread`、`defer`、`lazy let capture` が suspension boundary とされ、`&'frame T`、`&'lease T`、`&mut T` は boundary を跨げない。

禁止:

```awft
stream fn bad<'frame>(bytes: &'frame [u8]) -> Stream<u8, Unit> {
    yield bytes[0]
}
```

ただし、この例は単に値をコピーして yield するなら許可できる場合がある。問題は borrow が continuation state に保持される場合である。

より明確な禁止例:

```awft
stream fn bad<'frame>(frames: Stream<&'frame AudioFrame, AudioError>)
    -> Stream<&'frame [f32], AudioError>
{
    for frame in frames {
        let samples = frame.samples()
        yield samples // error: borrowed frame-local data crosses yield
    }
}
```

許可:

```awft
stream fn ok(frames: Stream<AudioFrame, AudioError>)
    -> Stream<Bytes, AudioError>
{
    for frame in frames {
        let owned = frame.samples().to_bytes()
        yield owned
    }
}
```

checker は以下を検査する。

```text
- yield 後も continuation に残る local captures
- yielded value の lifetime
- stream/source item の ownership
- callback buffer / borrowed device frame の escape
- &mut borrow が suspension boundary を跨ぐか
```

---

## 10. Effect ルール

## 10.1 `seq` は pure

`seq` block 内では以下を禁止する。

```text
- await
- try await
- Need force
- signal.set
- metric.set
- log.*
- event.emit
- device acquisition
- source polling
- thread
- together
- wall-clock access
- random without deterministic seeded capability
```

理由: `Seq<T>` は pure lazy sequence であり、`Source<T,E>` とは別物だからである。

## 10.2 `stream` は transform

`stream` block / `stream fn` は、既に取得済みの stream/source/port を処理できる。

許可:

```text
- for item in Stream<T,E>
- select / poll の stream-safe form
- pure map/filter/fold-like transform
- owned handle の利用
```

禁止:

```text
- capture.microphone(...)
- capture.camera(...)
- device.usb(...)
- permission prompt
- UI pending display
- OS thread creation
```

device acquisition は `Need` と `try await ... with` で行う。

## 10.3 `source` は policy-backed effectful source

`source` block は effectful だが、free-form effect ではない。

必須:

```text
- from
- backpressure
- replay
- privacy
```

許可:

```text
- on item ... => yield ...
- on error ... => log / signal / error handling
- on disconnected ... => signal / cleanup
```

禁止:

```text
- source handler から別 device を開く
- source handler から user-visible permission flow を隠す
- unbounded buffering
- replay policy 未指定
```

---

## 11. 並列処理との統合

## 11.1 `.parallel(limit = N)` は `Need` / `Task` の合成

現行仕様では、effectful map は `traverse` を使い、並列実行は `.parallel(limit = 4)` のように表現する。

```awft
let images =
    await image_paths
        .traverse(asset.image)
        .parallel(limit = 4) with {
            pending p => {
                scene.show(@scene.loading)
                progress.set(p.ratio)
            }
        }
```

これは generator とは別機能である。

```text
generator:
  item production

parallel traverse:
  bounded task scheduling
```

## 11.2 `yield` は task completion order を決めない

scheduler は task events を frame boundary で正規化する。現行仕様では、task event は `logical_epoch`, `task_id`, `sequence` で sort される。

複数 stream/source を合流する場合は、merge policy を明示する。

```awft
let merged =
    merge(
        camera.frames(),
        fixture.frames(),
        order = .FrameBoundaryThenSourceId,
        on_error = .Propagate,
    )
```

policy なし merge は禁止。

```text
error: merging multiple sources requires an ordering policy
help: add `order = .FrameBoundaryThenSourceId`
```

## 11.3 `together` との関係

`line plan` の `together { ... }` は same-tick effect grouping であり、long-running work でも stream generation でもない。

禁止:

```awft
alice[...]
with {
    together {
        yield .A
        yield .B
    }
}
```

診断:

```text
error: `yield` cannot be used inside `together`
help: `together` groups same-tick effects; use `out` for line results
```

## 11.4 parallel conflict

現行 line-plan lowering では、`together` は parallel boundary を保持し、同じ signal や line `out` へ複数 child が書くような deterministic conflict を拒否する。

stream/source merge でも同様に、複数 producer の同時 item には ordering policy が必要である。

---

## 12. Runtime Lowering

## 12.1 Flow runtime へ `Yield` を入れない

現行 core の `FlowOp` は flow execution 用であり、`Await`、`Dialogue`、`Choice`、`Loop`、`For` などを持つ。
`yield` は flow 本体で禁止するため、`FlowOp::Yield` は追加しない。

## 12.2 Stream IR

`stream fn` / `stream {}` は専用 IR に lower する。

```rust
pub struct StreamPlan {
    pub id: StreamRuntimeId,
    pub item_ty: TypeId,
    pub error_ty: TypeId,
    pub ops: Vec<StreamOp>,
}

pub enum StreamOp {
    Let {
        pattern: RuntimePattern,
        expr: RuntimeExpr,
    },

    ForNext {
        pattern: RuntimePattern,
        source: StreamExpr,
        body: Vec<StreamOp>,
    },

    Yield {
        expr: RuntimeExpr,
    },

    If {
        condition: RuntimeExpr,
        then_ops: Vec<StreamOp>,
        else_ops: Vec<StreamOp>,
    },

    Match {
        scrutinee: RuntimeExpr,
        arms: Vec<StreamMatchArm>,
    },

    Select {
        branches: Vec<StreamSelectBranch>,
        policy: SelectPolicy,
    },

    Close {
        source: StreamExpr,
    },

    Return,

    Noop,
}
```

## 12.3 Source IR

`source` block は `SourcePlan` に lower する。

```rust
pub struct SourcePlan {
    pub id: SourceRuntimeId,
    pub item_ty: TypeId,
    pub error_ty: TypeId,
    pub from: SourceFrom,
    pub policy: SourcePolicy,
    pub handlers: Vec<SourceHandlerPlan>,
}

pub struct SourcePolicy {
    pub backpressure: BackpressurePolicy,
    pub replay: ReplayPolicy,
    pub privacy: PrivacyPolicy,
}

pub enum SourceHandlerPlan {
    Item {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Error {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Progress {
        pattern: RuntimePattern,
        ops: Vec<SourceOp>,
    },
    Disconnected {
        ops: Vec<SourceOp>,
    },
    PermissionRevoked {
        ops: Vec<SourceOp>,
    },
    End {
        ops: Vec<SourceOp>,
    },
}

pub enum SourceOp {
    Yield(RuntimeExpr),
    Effect(LineEffectRequest),
    SignalWrite(RuntimeAssignment),
    Log(RuntimeLog),
    Close(SourceRuntimeId),
    Noop,
}
```

## 12.4 SourceEvent

現行 device stream 仕様では `SourceEvent<T,E>` に `source`, `sequence`, `kind` があり、`kind` は `Item`, `Progress`, `Disconnected`, `PermissionRevoked`, `Error`, `End` を持つ。

runtime adapter は device callback を直接 DSL に渡さず、必ず `SourceEvent` queue に変換する。

```text
native callback
  -> owned frame/audio packet
  -> bounded ring buffer
  -> SourceEvent queue
  -> frame-boundary normalization
```

---

## 13. Backpressure / Replay / Privacy

## 13.1 Backpressure

policy:

```rust
pub enum BackpressurePolicy {
    LatestOnly,
    BoundedQueue {
        capacity: usize,
        on_overflow: OverflowPolicy,
    },
    BlockingNotAllowed,
}

pub enum OverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
}
```

使い分け:

```text
camera preview:
  latest

USB protocol:
  bounded(capacity = N, overflow = error)

metering / audio level:
  coalesce

private live capture:
  latest + hash_only / none
```

## 13.2 Replay

policy:

```text
full
  exact payload or fixture id

hash_only
  item hash + summary

summary
  selected redacted summary

event_only
  connection/disconnect/error only

none
  product mode for private capture
```

source block は replay policy を必須にする。

```awft
pub source @source.test_camera_frames: Source<VideoFrameHandle, CaptureError> {
    from fixture.video("fixtures/camera/front_cam.webm")
    backpressure = bounded(capacity = 8, overflow = error)
    replay = full
    privacy = recordable

    on item frame => yield frame
}
```

## 13.3 Privacy

privacy policy:

```text
transient
  runtime-only; replay は hash/summary 以下

redacted
  replay stores redacted metadata

recordable
  full replay allowed

private
  no item payload recording
```

`privacy = private` かつ `replay = full` は禁止。

```text
error: `privacy = private` is incompatible with `replay = full`
help: use `replay = hash_only`, `summary`, `event_only`, or `none`
```

---

## 14. Cancellation / Close / End

## 14.1 Source close

source / stream consumption は明示 close を持つ。

```awft
select {
    audio = frames.next? => {
        signal.set(@signal.voice_level, audio.rms)
    }

    event .Back => {
        close frames
        return Ok(FlowExit::Goto(@flow.title))
    }
}
```

現行 device stream 仕様でも、source acquisition 後の consumption には `select`, `poll`, Activity input ports, signals を使う例がある。

## 14.2 Cancellation propagation

source/stream は cancel scope を持つ。

```text
flow cancel
  -> active source subscriptions close
  -> pending SourceEvent discarded according to policy
  -> replay records cancellation boundary

line cancel
  -> line-local source subscriptions close
  -> line defer runs
```

line plan の cleanup は `defer` で表現され、scope exit / cancellation で実行される。

## 14.3 End semantics

`SourceEventKind::End` を受けた source は closed state に入る。

```text
next? on ended source:
  returns None / End depending on API shape

for item in source:
  loop terminates

select branch:
  end pattern branch may fire if specified
```

推奨 syntax:

```awft
select {
    frame = frames.next? => {
        render(frame)
    }

    end frames => {
        log.info("camera stream ended")
        return Ok(FlowExit::Done)
    }
}
```

---

## 15. Error Handling

## 15.1 Stream error

`Stream<T,E>` は item stream と error type を持つ。

```text
yield e:
  emits item T

error propagation:
  propagates E through stream state machine
```

推奨 API:

```awft
let reports =
    usb.interrupt_in(@usb.custom_pad, endpoint = @usb.ep.input)
        .map(parse_custom_pad_report)
        .filter(_.is_ok())
        .map(_.unwrap())
        .coalesce_latest()
```

`Source<T,E>` adapters は pure `Seq<T>` にならず、timestamp、error、disconnect、backpressure semantics を保持する。

## 15.2 `try` / `?`

`yield expr?` は以下のように扱う。

```awft
stream fn parse_reports(raw: Stream<Bytes, UsbError>)
    -> Stream<Report, UsbError>
{
    for packet in raw {
        yield parse_report(packet)?
    }
}
```

意味:

```text
parse_report(packet): Result<Report, UsbError>
? propagates UsbError as stream error
yield receives Report
```

`Need<Result<T,E>, TaskError>` は先に `await ... with` が必要であり、暗黙に `T` へならない。

---

## 16. Parser / AST / HIR 変更

## 16.1 Parser

現状 `Stmt::Yield(Expr)` は AST にあるため、parser が既に拾えるなら grammar doc を追従する。拾えていない場合、`parse_stmt` に以下を追加する。

```rust
if let Some(rest) = trimmed.strip_prefix("yield ") {
    return Stmt::Yield(parse_expr_lossy(rest.trim()));
}
```

ただし、parser は文脈チェックをしない。

```text
parser:
  accepts syntax

semantic checker:
  rejects invalid context
```

## 16.2 AST

既存:

```rust
pub enum Stmt {
    ...
    Yield(Expr),
    ...
}
```

変更不要。

ただし、source-specific syntax を AST に持たせる必要がある。

```rust
pub struct SourceItem {
    visibility: Option<Visibility>,
    id: Option<IdRef>,
    name: Option<String>,
    source_ty: TypeRef,
    headers: Vec<SourceHeader>,
    handlers: Vec<SourceHandler>,
    range: TextRange,
}
```

現状 `SourceItem` が body を raw / statements として保持している場合、Phase 1 では raw preserve でもよいが、semantic check と lowering のために structured source item へ移行する。

## 16.3 HIR

```rust
pub enum HirComputationBlockKind {
    Result,
    Task,
    Seq,
    Stream,
}

pub enum HirStmt {
    ...
    Yield {
        expr: HirExpr,
        context: YieldContextId,
    },
}
```

`yield` は HIR では必ず resolved context を持つ。`YieldContext::None` のまま HIR へ進めない。

---

## 17. Runtime / Core との境界

Arcweft core は副作用を実行しない。`Engine::step(RuntimeStepInput) -> RuntimeStepOutput` は deterministic な data を受け取り、次 state と request を返す。core は filesystem、network、wall-clock、GPU/audio/device handle を保持しない。

この原則に従い、source/generator も次のように分離する。

```text
compiler:
  source/stream plan を作る

core:
  SourceEvent を frame input として受け取る
  stream state machine を deterministic に進める
  effect request を出す

adapter:
  device callback
  OS / web API
  actual worker
  native timestamps
  permission prompt
```

---

## 18. 実装フェーズ

## Phase A: 仕様整備

* `grammar.md` に `YieldStmt` を追加
* `source` block grammar を追加
* `control-transfer-return-out-yield.md` に許可/禁止 matrix を追加
* `streams-generators.md` と `device-streams.md` の用語を統一
* line plan では `yield` 禁止、`out` 使用を明記

## Phase B: Semantic Checker

* `YieldContext` stack を導入
* `yield` context check
* `seq` pure effect check
* `stream fn` return type check
* `source` policy completeness check
* suspension boundary borrow check を `await` / `thread` と共有

実装ガイド上も、`await` / `yield` / `thread` は suspension boundary として mark され、borrow crossing は compile error とされている。

## Phase C: Runtime Lowering

* `LineEffectRequest::Yield` を通常 lowering から除外
* line plan の `Stmt::Yield` は semantic error にする
* `StreamPlan` lowerer を追加
* `SourcePlan` lowerer を追加
* `FlowOp` には `Yield` を追加しない

現在 runtime-plan lowering には `Stmt::Yield` を `LineEffectRequest::Yield` へ落とす経路があるため、これは仕様とズレている。

## Phase D: Scheduler Integration

* source event queue
* frame-boundary normalization
* merge ordering policy
* backpressure handling
* replay recording
* cancellation propagation

## Phase E: Tests

### Parser tests

```text
ok_parse_yield_stmt
ok_parse_seq_yield
ok_parse_stream_fn_yield
ok_parse_source_handler_yield
```

### Semantic ok

```text
seq_yield_infers_item_type
stream_fn_yield_matches_return_item
source_yield_matches_source_item
stream_transform_granted_port
```

### Semantic error

```text
yield_in_flow_is_error
yield_in_line_plan_is_error
yield_in_task_fn_is_error
yield_in_dialogue_fn_is_error
yield_in_memo_fn_is_error
yield_type_mismatch
stream_fn_missing_return_type
stream_fn_without_yield
source_missing_backpressure
source_missing_replay
source_opens_device_inside_handler
borrow_crosses_yield
private_source_full_replay_is_error
merge_without_order_policy_is_error
```

### Runtime tests

```text
stream_state_machine_yields_in_order
source_events_sorted_by_sequence
backpressure_latest_drops_old_frames
bounded_queue_error_on_overflow
replay_hash_only_records_hash_not_payload
cancel_closes_source_subscription
```

---

## 19. 仕様例

## 19.1 Pure `Seq`

```awft
fn enabled_choice_labels(state: GameState) -> Seq<String> {
    seq {
        for c in opening_choices() {
            if choice_available(state)(c) {
                yield c.label
            }
        }
    }
}
```

## 19.2 Stream transform

```awft
stream fn rms_level(
    frames: Stream<AudioFrame, AudioError>,
) -> Stream<f32, AudioError> {
    for frame in frames {
        let rms =
            frame.samples
                .seq()
                .map(|s| s * s)
                .mean()
                .sqrt()

        yield rms
    }
}
```

## 19.3 Source declaration

```awft
pub source @source.player_mic_frames: Source<AudioFrameHandle, CaptureError> {
    from capture.microphone(@capture.player_microphone)
    backpressure = bounded(capacity = 8, overflow = drop_oldest)
    replay = hash_only
    privacy = transient

    on item frame => yield frame

    on disconnected => {
        signal.set(@signal.mic_connected, false)
        log.warn("microphone disconnected")
    }

    on error e => {
        signal.set(@signal.mic_error, e)
        log.warn("microphone error {err:?}", err = e)
    }
}
```

## 19.4 Flow consumption

```awft
pub flow @flow.listen listen(state: GameState) -> Result<FlowExit, FlowError> {
    let mic =
        try await capture.microphone(@capture.player_microphone) with {
            pending p => {
                scene.show(@scene.permission_wait)
                text.show("マイクの許可を待っています")
                progress.set(p.ratio)
            }

            denied _ => return Ok(FlowExit::Goto(@flow.mic_optional))
        }

    let levels = rms_level(mic.frames())

    select {
        level = levels.next? => {
            signal.set(@signal.voice_level, level)
        }

        event .Back => {
            close levels
            return Ok(FlowExit::Goto(@flow.title))
        }
    }
}
```

## 19.5 Line plan では `out`

```awft
let outcome = alice.say(voice=auto)[
    聞いて。[p]
]
with 'line {
    cancel on input .SkipLine {
        text.flush(mode = .Instant)
        out 'line .Skipped
    }
}
```

---

## 20. Open Questions

### 20.1 `Stream<T,E>` と `Source<T,E>` の名前

一部ドキュメントでは `Source<T,E>`、一部では `Stream<T,E>` が出る。最終的には以下のどちらかに寄せる。

案 A:

```text
Source<T,E>:
  live external source

Stream<T,E>:
  derived transform
```

案 B:

```text
Stream<T,E>:
  public API の統一名

Source<T,E>:
  runtime/source declaration internals
```

現行の仕様文脈では、`Seq` と live `Source` の分離が強く示されているため、案 A が分かりやすい。

### 20.2 `stream fn -> Source<T,E>` を許すか

`stream fn` は基本 `Stream<T,E>` を返す方が読みやすい。`Source<T,E>` は permission/backpressure/replay policy を持つため、原則 `source` declaration に限定する。

推奨:

```text
stream fn -> Stream<T,E>
source    -> Source<T,E>
```

例外的に generated code では `stream fn -> Source<T,E>` を許してもよいが、手書き authoring では非推奨にする。

### 20.3 `yield` を expression にするか

本設計では statement に限定する。

```text
yield expr
```

以下は採用しない。

```awft
let ack = yield item
```

理由:

```text
- generator consumer から send-back する設計は複雑
- scheduler / replay / cancellation の責務が曖昧になる
- Arcweft の authoring 用途では不要
```

---

## 21. 最終方針

Arcweft は `yield` を持つ。ただし、それは汎用 generator ではない。

```text
`yield` is valid only in explicit generation contexts.

- `seq { yield ... }`
  pure lazy sequence

- `stream { yield ... }` / `stream fn`
  stream transform

- `source { on item ... => yield ... }`
  policy-backed live source

`yield` is a suspension boundary.
`yield` does not open devices.
`yield` does not create OS threads.
`yield` does not hide permission prompts.
`yield` does not replace `Need`, `await with`, `thread`, `together`, or `out`.
```

この設計により、現在の Arcweft の Sans I/O runtime、deterministic scheduler、Need-based acquisition、line-plan cancellation、device replay policy と矛盾せずに、generator-like な記述性だけを安全に取り込める。
