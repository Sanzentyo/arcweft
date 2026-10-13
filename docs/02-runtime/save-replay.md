# save / replay / hot reload

## SaveSnapshot

```rust
pub struct SaveSnapshot {
    pub bundle_hash: BundleHash,
    pub program_version: Version,
    pub state: Value,
    pub logical_time: LogicalTime,
    pub active_scene: SceneId,
    pub flow_fibers: Vec<FlowFiberSnapshot>,
    pub active_activities: Vec<ActivitySnapshot>,
    pub audio_state: AudioSnapshot,
    pub awaited_tasks: Vec<AwaitedTaskSnapshot>,
}
```

## 未完了 task

生 Future は保存しない。保存するのは task key と continuation。

```rust
pub struct AwaitedTaskSnapshot {
    pub task_id: TaskId,
    pub key: TaskKey,
    pub class: TaskClass,
    pub request: HostTaskRequest,
}
```

load 時は `ensure_task` で再登録。

Restartable dispatch の復元では、保存された完全な task correlation、logical epoch、
dispatch sequence と publication frontier を保持する。復元先には新しい native host
dispatch owner を用意し、再登録時に新しい `HostTaskAttempt` を発行する。
adapter と worker はその寿命証拠を返す。
保存前の worker packet は、公開座標と次の publication revision が一致していても
復元後の attempt と一致しないため、scheduler・frontier・統計を更新する前に拒否する。
稼働中の同一 dispatch の Join は既存 attempt を共有する。この host 内の証拠は保存・
digest・公開イベント順序には含めず、成功した publication は従来の決定的な座標を使う。

## ReplayTrace

```rust
pub struct ReplayTrace {
    pub engine_version: String,
    pub bundle_hash: BundleHash,
    pub initial_state: StateSnapshot,
    pub frames: Vec<RecordedFrameInput>,
    pub task_responses: Vec<RecordedTaskResponse>,
    pub audio_events: Vec<RecordedAudioEvent>,
    pub agent_actions: Vec<RecordedAgentAction>,
}
```

## Hot reload

```text
incoming patch
  → parse
  → typecheck
  → contract check
  → shader validate
  → wasm/rust ABI check
  → state compatibility check
  → dry-run current continuation
  → commit at frame boundary
```

## 保存形式の契約

Arcweft が所有する save・snapshot・codec の contract version は `1`。
未公開の内部形式は同じ version のまま更新し、旧形式の reader や migration 経路を
並置しない。host 内の worker 寿命証拠は保存形式に含めない。

retained View の root binding・mount parameter・local state は、選択された
runtime program の既存 Core value snapshot に投影して保存する。JSON decode
では live value を構築せず、artifact/generation の照合後に同じ program authority
で復元する。callable state と capture は復元先の immutable executable に束縛し、
nominal header はその型グラフで検証する。View の in-memory snapshot に独立の
serde reader を残さず、mount identity・型付き slot・axis seed の検証を既存 View
restore で実行してから session を公開する。

Core の inert value graph は、live/inert 共通の borrowed logical view と
canonical visitor の counting sink を使い、live value を構築する前に
depth・node・collection・string・validation-work・encoded-byte を検証する。
予算は live admission の既存 authority を共有し、dense の各要素と columnar の
論理行、predicate と callable の retained 値を計上する。dense numeric の
scalar atom の深さは選択された resource policy に従い、値 carrier の nesting
上限はそのまま維持する。durable root は選択された role、root rollback は
元の root policy の上限を構築前に適用する。型付き保存場所では、その owner が
保持する expected type を original program の型行に照合し、同じ
borrowed view の descendant を inert のまま検証する。resource preflight の
残りの validation-work allowance を型検証へ渡し、phase ごとに上限を再発行しない。
関数の型関係と effect predicate、nominal の元 instantiation の照合、および
そこから呼ぶ value relation も、この同じ allowance を使用する。Choice の
不成立 branch は effect obligation を巻き戻すが、消費 work と hard resource
failure は巻き戻さない。validation-work・validation-depth の上限超過は
値の位置を保持した資源エラーとして返す。
durable root は選択された state role の型をこの構築前検証に渡す。
inert callable と Iterator Witness の照合は選択中の original program を使い、
その検証は live lease や Copy capability を発行しない。live 値の admission は
既存 lease がその executable と一致することを引き続き要求する。
unrestricted の保存場所では、同じ traversal が Need と
affine opaque の全 descendant を拒否する。line-handle token inventory もこの
traversal を使い、空の columnar storage の再帰 metadata を検証から除外しない。
columnar の保存された column length・field identity・field name の不整合を論理行の
読み取り前に拒否する。借用ビューからの Copy・custody 操作には live value
または live sequence row が発行する capability を要求し、inert view を
live capability に昇格させない。line-token の読み取りでは live payload を
作り直さず、同じ借用ビューで固定幅の token field を検証する。
Core value／sequence／predicate の内部 child は、構築前検証後の private な
構築経路を使う。task request など別の owning codec の検証境界は保持する。

inert nominal／variant に保持された宣言 effect 解の row と label も、元の
immutable instantiation 上で同じ collection・string・depth・validation-work
予算を検証する。metadata は logical payload node を増やさず、元の型を照合し
effect row を複製する前に共有 work allowance を消費する。
宣言 effect 解の matcher は、destination と元 provenance のどちらでも、
元の immutable row・label をこの予算で検証してから formula の作業領域へ
複製する。資源エラーを内部の admissibility 判定へ落とす際も、最初の
hard failure を共有 allowance に保持し、型の不一致へ置き換えない。

AWBC fiber の capture／restore は、全 frame の function・frame layout・
register 数・宣言 effect 解の context を original AWBC program に照合してから
metadata を複製し、live value を構築する。live frame の検証も同じ owning
frame-layout 検証を使う。続いて全 frame の register place を、その layout の
元の型行と frame の immutable destination binding に照合してから、最初の
値を複製・構築する。Native lexical environment も全 function scope と
元の local declaration を同じ順序で検証し、live／inert の scope carrier と
place relation を共有する。layout と local declaration の既知の型IDは元の
inventory と一緒に直接渡し、semantic IDを経由して選び直さない。semantic role
からの照合は同じ original inventory の一意な型行を選び、曖昧な行を拒否する。
どちらも保存対象のスロット inventory 内で
logical-node・完全値の encoded-byte・relation work を累積する。
空き place は値を構築せず、部分レコードは元の nominal identity・layout・
field inventory と independent provenance を検証する。destination の型行と
元 provenance の型行が異なる場合も、既存の nominal argument／effect 関係を
満たすことを要求し、残る子は destination の field 型で検証する。
部分ヘッダの名前・instantiation・深さ・子数も複製前に制限する。完全値の
canonical byte 計上は、部分 place や frame wire 全体の byte 計上を代替しない。
継続状態と session の総予算は、それぞれの owner の継続契約を要求する。

Core の live value capture は、DTO を組み立てる前に借用値の snapshot grammar
を traversal し、depth・node・collection・string・validation-work と、その
grammar の encoded-byte 上限を検証する。program-bound capture は選択された
`RuntimeSchemaLimits` を受け取り、既定入口は engine の既存 policy を使う。
iterator／opaque を直接保存する入口も同じ grammar を使う。空の columnar
storage の metadata と宣言 effect 解も共有 work allowance を消費する。
owned snapshot は affine opaque の custody を保持し、unrestricted capture は
複製前にその所有権を拒否する。Need descriptor 全体や frame／session の総予算は
各 owner が接続する検証境界であり、値の grammar の byte 計上とは区別する。

Iterator witness の live receiver は original executable lease を保持する。
`next` の method identity、唯一の mutable receiver parameter、builtin Option の
item 型をその program の型・signature・frame layout から読み、receiver state と
保存場所の Iterator item 型を照合する。context-free decode はこの lease を発行
しない。Range iterator は元の整数幅と end、current、inclusive、done を保持し、
inclusive な最大値を返した後の `end + 1` を正当な終端カーソルとして扱う。
live／snapshot admission は同じ grammar で Witness の state と Range の全 header
を encoded-byte budget に含め、偽造 bound・cursor・早すぎる done を拒否する。

durable root の保存値も Core DTO を使う。active entry の identity・kind・binding
から original program の stateful roles を選び、state/event identity と layout を
照合してから復元する。state payload はその role の original 型グラフと root
limits で検証する。live root state と in-memory root snapshot の serde reader を
残さず、保存 wire の version は `1` を維持する。

Product の queued `Ready` は、保持された task admission の完全な correlation から
outcome を選び、既存の `BoundTaskOutcome` で検証してから保存・復元結果を公開する。
program outcome は選択中の immutable executable に束縛し、standalone outcome は
既存の有限 predicate を使う。欠落・不一致・重複 admission は復元を拒否する。
この検証は inert save と直接の live snapshot restore の両方に適用する。

固定 byte-array atom の `TaskId` を JSON object key にしない。
`task_publications` の version `1` JSON は、task identity の昇順の
`[TaskId, TaskPublicationCursor]` row 配列として保存する。
decode は重複・順序逆転・旧 object grammar を拒否し、別の reader を残さない。

表示フレームの effective configuration は、元の generation-admitted
`CharacterDialogue` と executable lease を保持する。Style／RichText／custom／
capture を失う表示用の field projection から保存値を再構成しない。
`LineDisplayFrameSnapshot` と stage observation は、同じ display/source metadata
に Core の inert value carrier を接続する。セッションの dialogue store と View
text output、bundle の retained display-frame resource もこの carrier を使う。
live frame／stage の Serialize はこの wire を生成し、live configuration の
context-free Deserialize は提供しない。

復元は original executable と generation schema で CharacterDialogue の全 role
と custom field を再検証し、config digest・Character identity・元の Style と
stylesheet の一致を確認する。generation schema を持つセッションを standalone
configuration に downgrade する保存値は拒否する。正当な standalone 描画入力は
program-owned descendant を構築できない Core の共通 decoder を使う。
保存・復元・observation の contract version は引き続き `1`。

表示フレーム内の role/custom／Content callback／fallback Style も、既存の
session catalog validator に渡してから facade を公開する。表示文字列から
Content を再構成せず、元の Content envelope と捕捉値を保持する。

直接の live session snapshot も、wire の frame producer と同じ original
program／generation admission を通してから presentation をコピー・公開する。
dialogue 履歴と View の Character display name／Content frame は同じ owning
traversal で検証する。fallback Style は保存側も元の generation role で検証する。
Content の owning constructor と borrowed decoder／evaluated-binding producer は、
failure policy と callback の affine descendant をコピー前に拒否する。
既存の Content node budget は callback の元の retained value graph も計上する。

Snapshot の値 admission は既存の logical-value visitor と byte count を使い、
encoded-byte 上限を graph 全体で検証する。検証だけの caller は transcript を
割り当てない。Need の要求値は元の借用値を走査し、outcome 型とは分けて同じ
node・depth・collection・string・byte 予算に計上する。Need の outcome identity は
Native と AWBC の両方で original program の型行に照合する。

Range の幅は元の bound に基づく owning value の検証を Native と AWBC が共有する。
Values iterator の snapshot は残りの各要素を original item 型で検証する。
snapshot の byte 計上は Range の bound と inclusive flag、Values iterator の長さ、
callable の state/header と retained 値を含む。Engine の専用 iterator rollback
復元も private constructor の前に Core の同じ graph policy を適用する。

strict typed JSON save は、型付き DTO を構築する前に JSON codec の共通 parser
と既存 DecodeBudget で input・depth・node・collection・string を検証する。
同じ parser の検証出力は value graph を保持せず、duplicate key の照合に必要な
key だけを保持する。未知 field・重複 key・末尾 data の拒否を維持し、検証後の
型付き decoder は元の bytes を読む。wide integer と float bit を検証のために
再エンコードしない。早期拒否の構造化 kind・path は decode context が保持する。

## Agent replay

Agent actions are replayed as semantic actions when possible.

```bash
arcw agent script replay traces/bug.arcwx --json
```
