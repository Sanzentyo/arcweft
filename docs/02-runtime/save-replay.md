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

retained presentation の display frame と stage は、表示・source metadata の
既存 serde grammar を借用して同じ group の depth・collection・string・work
上限を検証する。metadata を仮の RuntimeValue に変換せず、実際の wire-byte
上限は enclosing codec が所有する。CharacterDialogue の original executable
と generation、元の Style role、Content の入れ子と callback capture は、
frame の複製や inert value の live 復元より前に検証する。Content は所有型の
codec fields を直接 borrowed logical view へ投影し、検証用の live envelope を
複製しない。保存の value projection もこの view を利用し、既存の型付き
nominal・callable・Need の保存境界を維持する。
durable root は選択された state role の型をこの構築前検証に渡す。
inert callable と Iterator Witness の照合は選択中の original program を使い、
その検証は live lease や Copy capability を発行しない。live 値の admission は
既存 lease がその executable と一致することを引き続き要求する。
expected type を持たない program-bound value capture／restore も、資源走査を
完了してから nominal の元の型行と construction binding、callable の元 state
と retained 型、Iterator Witness の next-method と state 型、DataShape と
Need outcome の元の型座標を検証する。この照合を全 descendant に終えてから
最初の DTO projection／live constructor に進む。program 参照のない graph は
資源走査で判定し、不要な型照合走査で validation-work を再消費しない。
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
frame-layout 検証を使う。`FiberState`・`FiberFrame`・return point／continuation は
一つの storage-parameterized owning grammar を使い、保存形式はその inert
specialization とする。live frame と default の pending input owner は Clone
も serde も持たない。root・cursor・frame instance・return destination・scope stack
と format の復帰先は、全 frame header を検証した後、値の構築より前に照合する。
続いて全 frame の register place を、その layout の
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

Product が保存 Fiber を検証する場合も inert storage を借用し、検証だけを目的とした
live Fiber の構築・複製を行わない。terminal の return summary は、数値 shape・
sequence の物理 storage kind・元 program の callable arity を含む共通 borrowed
label projection と照合してから復元する。文字列 label は元の値を借用する。

Product image は root／child／dialogue の fiber、対話 ledger と frame、Need registry、
queued publication、観測値を同じ original program の累積 admission で検証する。
save 変換の公開入口も同じ image owner の検証を通してから inert 値を複製する。
対話 frame と phase／closing／pending operation は live／inert 共通の owning grammar
を使い、inert specialization だけが Clone と serde reader を持つ。registry の元の
activation key、phase と結果、元 activation signature の capture、reveal 後の input、
content template の value slot と callback capture ABI を、live 値の構築前に照合する。
検証用に DialogueStore や中間の live frame snapshot を構築しない。全体の検証後は
既存の admitted projection を使い、各値の変換時に資源枠を更新しない。
一時的な `Transitioning` phase は codec と復元の両方で拒否する。
activation 中の fiber root は、元 content group の activation function と一致させる。
別の root として単体検証を通る fiber でも、この対応を置き換えられない。

Native の fiber／frame、control kind と function return、formatter、Await／AwaitMany、
対話 frame／scope／pending operation は、それぞれ同じ owning grammar の live／inert
特殊化を使う。`RuntimeEnv` と `RootRuntime` も同じ構造を共有し、live owner に
無条件 Clone を与えない。AwaitMany の in-flight handle も inert descriptor に変換する。
Need publication の observer は、選択対象の通知群を元 program と Await handle の
outcome 型で、同じ資源枠を使ってすべて検証してから Copy／move する。
後続通知の拒否時にも先行する affine Ready、queue、観測済み cursor を保持する。
保存用 publication は affine 値も inert のまま保持できる。

Root の capture／restore は、状態値だけでなく queued event と committed command を
最初の値の変換前に検証する。元 executable の entry から binding／roles／policy を
借用し、保存側の metadata、nominal/layout、event sequence、command transition/index、
constructor／target／payload schema を照合する。親の累積資源枠に加え、entry 固有の
資源枠も Root 全体で累積する。資源検査後の schema relation と replay-safe 条件は
既存の borrowed value visitor を使い、Agent／predicate も live に戻して検証しない。
Root の値 inventory には、確定済み command の payload も含める。
durable Root snapshot と session snapshot も、それぞれ一つの owning grammar の
live／inert 特殊化を使う。live snapshot の無条件 Clone と直接の JSON reader／writer
は与えず、Root の entry／layout／type／資源検証は保存形態間で同じ実装を使う。
復元時の Root は、候補 session から所有権をそのまま移す。

対話の line-task reducer と execution lane も、それぞれ一つの owning grammar の
live／inert 特殊化を使い、Product wire は Core の inert 型をそのまま使う。
reducer の collection／work 予算を親の資源枠で検査してから、元 content group の
node／phase／scheduled scope と、activation の結果・取消・journal custody を
借用参照で照合する。検証用 snapshot の構築や、保存値を live に戻す処理は行わない。
Native は全 active frame の inbox と reducer を検査してから、最初の frame 値を変換する。
activation／content／task group は元 program と対応させる。Product の reducer wire の
node は Core の型付き identity を保持し、version 1 の形を直接更新する。
旧 reducer DTO とその変換・reader は保持しない。

attached default の実行中に停止した callable の pending packet は、Native
rollback と AWBC fiber で一つの inert carrier を使う。元の callable state の
default capture と後続 transition が、各 retained／argument slot の値の有無を
決める。全 slot の presence mask を確認してから、残る値を state が保持する
original 型IDで検証し、最初の値を複製・構築する。移動済み slot は空のまま
保持する。default の復帰先はその state の original body と照合する。
AWBC は全 frame の register と pending packet に同じ累積 value resource
allowance を渡す。Native の packet も同じ入力 grammar と value relation を使う。
packet の decode は executable lease を発行せず、復元は外側の保存 owner が
選択した元の immutable program を要求する。pending packet の value byte count は
frame／session wire 全体の byte grammar の検証を代替しない。

lazy context の Result cause／Option absence も、一つの generic carrier で
live と inert を表す。serde は Core の inert value projection だけを読み書きし、
live packet の context-free reader／writer と別の AWBC context DTO を残さない。
capture／restore は元の caller・return cursor・intrinsic signature・callback
state／body を照合してから、signature の source 型行が保持する builtin case の
payload tuple と元の field 型IDを選ぶ。source／tuple の scope と cause の型は、
candidate provenance から独立した caller の immutable binding で検証する。
この照合は通常の live return 検証と同じ owner を使い、全 frame の register と
pending 値の累積 resource admission に接続する。保存された実際の cause だけが
value node と encoded-byte を消費し、absence は値を構築しない。source の metadata
work は同じ allowance を消費する。型検査のために receiver／Tuple／Unit を
合成せず、source／cause の資源・型エラーを構造付きで保持する。

formatter の instruction state と nested Flow attempt は、共通の source-order
staging grammar を live／inert に使う。inert projection だけが private codec を
持ち、live state の直接 serde reader／writer と別 snapshot DTO を残さない。
Flow attempt の進行は `Idle`／`Evaluating(parameter, scope-depth, failure)` で
表し、parameter の無い active failure／scope-depth は構築できない。owned 契約は
version `1` のままこの shape に揃え、旧 reader を並行して残さない。
capture／restore は元 instruction の operand manifest／function signature が保持
する元の型IDと frame の immutable binding を使い、全 frame の register／pending
値と同じ allowance で staged 値を検証してから projection／構築を始める。
未到達 slot と metadata の work、locale／failure の string 上限、active attempt の
completion instruction 検索もこの検証に含める。metadata は value node／byte として
合成せず、frame wire 全体の byte grammar の検証とは区別する。

lexical scope・cleanup・scoped defer は一つの generic grammar を live／inert に使う。
private codec capability は Core の inert value carrier だけに実装し、live serde
reader／writer と別の AWBC scope DTO は持たない。scoped defer は登録した元の
instruction cursor を保持し、RegisterDefer の site・outcome・CurrentScope・capture
register inventory と original verifier の lexical scope stack に照合する。
実際の capture graph は一度だけ資源計上し、元の frame slot 型IDと defer signature
型IDの両方を immutable frame binding で検証する。Copy 値の重複 capture は許可し、
affine 値の重複は拒否する。cleanup は元の RegisterCleanup instruction cursor と
引数だけを保持し、key／effect plan／argument register inventory は元 instruction
から借用する。保存済みの複製 key／effect は持たない。元 frame slot と effect
signature の型IDを immutable binding で照合し、登録地点の lexical scope を検証する。
key による取消と terminal unwind は全対象 inventory の admission を先に完了し、
後続 packet の不正で先行 packet を除去・dispatch しない。key／failure の文字列、
collection count、registration identity、in-flight／release の site・token inventory
も全 frame の register／pending／formatter と同じ allowance で、最初の projection／
構築より前に検証する。VM の cleanup と scoped defer の producer も同じ owning
input rule を使って metadata コピーと registration commit より前に検証する。
LineRoot defer は line owner が管理し、producer の無い frame root_defers は残さない。
VM の instruction input transfer は frame owner に置き、全入力の元 layout 型行・
immutable binding・資源上限・Copy custody を検証してから、重複 Copy register の
必要なコピーをすべて準備する。各 source register は最後の occurrence で一度だけ
移動する。後続入力や Copy の検証失敗は、先行 register の移動より前に返す。
これらの value byte count は frame／session wire 全体の byte admission を代替しない。

terminal result は `FiberTerminalValue<V>` の単一 grammar を使い、inert carrier
だけに codec を与える。保持中の payload、payload の無い completion、caller／line
owner へ転送済みの状態を別の variant で表す。転送を `Returned(None)` で表さない。
snapshot projection／restore は status・suspension・terminal・return label の形を
構築前に検証し、terminal graph と label／trap message は全 frame と同じ allowance
に入れる。通常の result は元の root frame signature と immutable binding、frame を
切り離した Program result は元の pure-program binding の signature が型 authority
となる。dialogue result は verifier と共通の元 program の line-group ownership に
照合する。payload の無い状態を検証するための synthetic value は構築しない。

stream queue は `FiberStreamState<V>` の単一 grammar を使い、inert carrier だけに
codec を与える。非 Empty fiber の stream inventory は元 program の stream table と
同じ長さで、各 ordinal の plan ID がその ordinal に一致する。欠落・重複・並び替え・
未知 ID と、emitted count より長い queue を、queue descendant の検証前に拒否する。
queue item の実 graph は全 frame と共通の allowance に一度だけ計上し、再帰的 Copy
custody と元 stream plan の item 型IDを、projection／live 構築前に検証する。
VM の yield は同じ indexed inventory、closed 状態、checked emission counter、queue
資源上限、item 型と元 register layout を検証してから Copy と transfer を行う。
queue の欠落時に item を捨てたり、counter を飽和させたりしない。close は同じ indexed
owner を使う。これらの value byte count は fiber／session wire 全体の検証と区別する。

対話の activation・result・scheduled custody・defer registration は、live と inert が
同じ所有型の grammar を使う。inert specialization にだけ serde reader と Clone を
与え、live の capture packet や activation owner を複製しない。registry は全 entry の
identity・command journal・custody と retained 値を借用して検証してから、最初の
frame copy／live line 構築へ進む。戻り値は元 Native／AWBC の型ID、defer capture は
元 site が指す function の入力順・arity・型と Unit 戻り値に照合する。値・部分場所の
header・型関係は同じ resource bank を使い、途中の failure を後続 phase で消さない。
部分場所の header traversal は schema の metadata depth／work を使い、完全値の
nesting／sequence 上限を代用しない。残る initialized 値は部分場所の元の深さで計上する。
affine handle の kind・token・物理 storage path は共通の borrowed value view から
検査し、inert の token 検査のために live handle を構築しない。

AwaitMany の進捗は `FiberAwaitManyState<V, N>` と `FiberAwaitManyInFlight<N>` の
単一 grammar に保持し、Core の inert value／Need carrier だけに codec を与える。
元の到達済み AwaitMany terminator・resume point・plan・binding を構築前に照合し、
launch index、in-flight index の一意性と上限、index に対応した pending／result の
partition を検証する。capture は元 frame slot、base request signature、child request
signature の型行を immutable frame binding で照合する。item／result の元の型行を
検証し、capture／item の再帰的 Copy 条件、collection count、空 result slot の work
も共通 allowance に入る。result はその所有権を保持する。
producer key は元 AWBC task-plan の借用 argument view から導出し、保存 request の
clone や検証用の TaskSpec／RuntimeValue を構築しない。Need の origin・policy・outcome・
correlation の規則は共通 owner に置き、inert request の live 構築より前に適用する。
generation rebind は future launch に適用するため、collector と既存 child の個々の
receipt は保持し、両者の generation が同じであることは要求しない。VM は source と
capture の batch 全体を検証してから capture をコピーし、source register を一度移動する。
TaskSpec<P> と HostTaskRequest<P> は live payload と inert payload の共通 grammar を
所有し、HTTP／Wasm の payload-bearing request も同じ型を使う。別の saved spec／
request enum と reader は持たない。Custom の optional named arguments と manifest
contract は、この共通 request grammar の default／omission 規則に揃える。

TaskSubmission<P>、TaskEvent<P>、RuntimeNeedProducerState<P> と producer registry の
argument／launch は同じ storage-parameterized owner を使う。live event と Ready
state に Clone や serde reader は与えず、保存用 specialization だけが inert 値を
保持する。Native rollback と AWBC Product の in-memory Need registry は同じ inert
grammar を使い、wire の site／plan digest は verified program の元の plan 行を選ぶ
locator とする。argument の型・Copy 条件、元 TaskSpec、Ready の outcome、正確な
correlation、join／ordinal／invocation frontier を live 投影より前に検証する。
JoinSameKey の Ready は再帰的に Copy を満たし、AlwaysStart の affine Ready は
その値を借用して検証し、復元後も一度だけ waiter に移動する。

queued event と Need state の outcome 照合は enclosing retained admission の
node／byte／relation-work bank を共有する。有限 standalone 型は同じ descriptor
visitor と RuntimeCheckedType の既存 matcher で検証し、Choice の次の候補に進んでも
最初の work failure を保持する。program outcome は元の executable lease と型行を
借用し、同じ scalar 型を宣言する別 program を authority として受け入れない。

Need の resource traversal は要求値だけでなく correlation／origin、producer の
retained fields、generation、class、priority、cancel scope、outcome、debug label と
全14 Host request family の metadata を計上する。path／text／id／operation／header／
env／argument name と raw byte buffer の長さを、live・inert で同じ owner が検証する。
standalone outcome の有限型も、collection／depth／node／string と共有 work の制限を
検証してから semantic identity を計算・複製する。program-owned 型を standalone
predicate に置き換えない。Need semantic identity の canonical transcript は変えない。

保存 spec の projection／restore は descriptor と全 request value を一つの allowance
で preflight し、program outcome の元の型行と全 request value の program storage
を照合してから値の変換を開始する。最初の payload の変換で上限を再発行しない。
Need carrier 構築と launch journal の admission
にも同じ規則を接続し、submission の元の handle correlation を live request 構築前に
照合する。descriptor failure は後続 value admission にも同じ first error として残る。
この計上は retained storage の logical snapshot grammar であり、frame／session の
wire 全体の byte count と各 program の request type／expected-key／View capability の
admission は、それぞれの owning boundary で別に成立させる。

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

observation は `RuntimeObservationState<P>` の単一 grammar を使い、保存値は
Core の inert value specialization とする。live observation の serde reader は
持たない。signal／metric の名前と値、log／event／call の文字列と行 inventory を
同じ retained admission で検証してから最初の payload を複製・構築する。
Copy と元 program の lease は inert のまま照合し、構築後の所有権検査へ延期しない。
Product の main／child Fiber と observation も、その owner の同じ資源枠へ接続する。
復帰 metadata の declaration／capture／operand 照合は同じ validation-work を使い、
payload を metadata 検査で重複して計上しない。value が空の継続も work 上限を消費する。
資源枠への接続は Copy／実行 capability を発行せず、外側の wire byte grammar を
検証する責務は enclosing codec に残る。

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
session catalog validator に借用値のまま渡してから live executor／View の復元を
開始する。wire は inert root／Product／View／presentation、in-memory snapshot は
同じ各 owner の保持値を走査する。Copy・型・資源の借用 admission が catalog
callback に先行し、catalog の拒否は既存 session の owner を移動・破棄しない。
各復元入口は catalog を一度検証し、復元後に同じ callback を再実行しない。表示文字列から
Content を再構成せず、元の Content envelope と捕捉値を保持する。
カタログ検証の callback は Core の read-only `RuntimeBorrowedValue` を受け取る。
この参照は既存の logical codec の tuple・sequence・scalar・opaque header と
payload を公開し、子の live／inert provenance を保持する。表示フレームの
typed Content を検証用 carrier に複製せず、同じ asset-handle codec が live と
inert の両方を検査する。参照の取得は executable lease や復元の許可を発行しない。

Style の reusable property codec もこの参照を使い、元の generation role が
要求する outer／payload 型と、presentation catalog の property domain を確認する。
保存データの property も同じ grammar で列挙値・単位・数値範囲・PublicId を検証する。
property の所有値を作る decoder は全 field の借用検証後にのみ文字列・ID・row を構築する。

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
