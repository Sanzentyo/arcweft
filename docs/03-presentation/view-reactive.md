# Game Native View

Game Native View は SwiftUI 風の宣言的・リアクティブ View。HTML/CSS とは別に、ゲーム画面、選択肢、HUD、dialogue View、debug overlay、Agent 観測に使う。

## View

```arcw
pub view SettingsPanel(
    config: Binding<Config>,
    props: SettingsProps,
    close_action: DialogueAction,
) {
    local state tab: SettingsTab = .Audio

    Column(spacing = 16) {
        Text("Settings").font(.title)

        Picker(
            value = bind(tab),
            options = [
                PickerOption(id=.Audio, label="Audio"),
                PickerOption(id=.Text, label="Text"),
                PickerOption(id=.Video, label="Video"),
            ],
        )

        match tab {
            .Audio => AudioSettings(config = config.audio)
            .Text  => TextSettings(config = config.text)
            .Video => VideoSettings(config = config.video)
        }

        Button("閉じる")
            .agent_target(@view.settings.close)
            .on_click { close_action }
    }
    .padding(24)
    .background(.panel)
    .corner_radius(16)
}
```

裸の View 宣言名は、その宣言を特定する semantic callable token として解決する。
直接名、import、修飾パス、local な別名の呼び出しは共通の selected-call 型検査と
引数束縛に従い、結果は `ViewValue` になる。宣言は retained View execution を持ち、
Core の通常関数値や実行 frame へ投影しない。`@view.*` は同じ retained owner の
public identity を参照する entity reference として保持する。local/project の名前が
builtin View head と同じ場合も、通常の shadowing と visibility に従う。

## Parameter defaults

View の parameter default は宣言型を expected type として検査する通常の式であり、
String、nominal、tuple、関数値を scalar へ縮約しない。default が読むことのできる
parameter は宣言順で先行するものだけで、closure や implicit callable の capture
にも同じ制約を適用する。自己参照と後続 parameter への参照は拒否する。
record shorthand や nominal field の base、callback の生成時 capture も同じ
checked free-input authority から依存として保持する。default 内の local binding
は、その default の外部 parameter 依存には含めない。
default の評価は外部効果を実行せず、中断しない。純粋な関数呼び出しと local な
計算は通常の意味論に従う。

呼び出し側の supplied argument は authored order で一度ずつ評価し、parameter は
宣言順で束縛する。supplied value があればそれを使い、省略された場合だけ default
を評価する。省略値は先行入力から導く props であり、入力が変われば再計算する。
同じ program と依存入力 revision の cache は再利用できる。default の checked
coordinate、型、効果・中断・control row、式 transcript と全 free parameter input
は宣言の checked callable authority に属し、添付本文 default と同じ型で保持する。

bundle の default record は `RuntimePureProgramId`、canonical free-input order の
parameter coordinate／semantic type、および result semantic type を保持する。
Product AWBC の verified program binding と正確な input／result ABI を照合し、
生成された result 型と宣言型の適合を共通の型グラフで検証する。関数値は
引数の反変性、戻り値の共変性、効果契約を保持する。関数値が自身で宣言する
型・配列長 parameter は、その binder の所有元と slot を比較する rigid な参照
として扱う。外側の効果 scope が異なっても対応する local binder を比較できるが、
別 slot や外側の binder、固定配列長への置き換えは同一視しない。scalar value inventory の
index へ変換しない。mount は通常の `RuntimeValue` を
保持し、default の cache key は同じ executable owner と先行入力の inert value
から作る。cache の有無や restore 前後によって成功・失敗が変わらないよう、
cache hit も初回実行と同じ論理 operation cost を評価予算から消費する。

`Text` の String 式も同じ `ViewExpressionProgram` と通常の Core pure program
を使う。checked expression transcript から意味を含む program identity を導き、
その式が読む View parameter／派生 local と canonical input ABI を照合する。内部の local、
純粋な呼び出し、Match は Core の通常の式意味論に従う。入力の変更は再評価し、
cache は executable owner・program・ordered input snapshot に結び付ける。
result も inert snapshot として保存し、restore 時は派生 cache を破棄する。
String 以外の結果を文字列へ暗黙変換しない。Dialogue の型付き表示は専用の
projection を保つ。affine な入力・結果は resource owner の接続が必要であり、
copy として複製しない。

通常の block 内の `let` は派生値であり、retained な `local state` と区別する。
pattern の実行と型付き出力は Core の checked binding program が所有する。
View は program identity と canonical output ordinal で local を識別し、parameter の
ordinal や名前付き scalar slot へ変換しない。nested scope と shadowing は通常の
lexical binding に従う。派生 local は frame の scope を出ると破棄し、snapshot には
含めず、restore 後は同じ入力と program から再構築する。

View の `if` 式・statement、`else if` は通常の checked bool 式を条件に使う。
条件は同じ Core program／input ABI／評価予算で評価し、選択した arm だけを実行する。
各 arm は独立した lexical region を持ち、arm の local を他の arm や後続の region
から読めない。bundle は enclosing region 内の arm 範囲と scope を検証し、
Core signature の結果が bool であることを確認する。失敗した arm は部分的な frame
を公開せず、cache hit／cold restore でも評価結果と operation cost を保つ。

`Button` の label は String、enabled は bool であり、省略時の宣言値はそれぞれ
空文字列と true。位置引数と名前付き引数は同じ typed role に束縛し、未知・重複・
型不一致の引数を拒否する。supplied expression は authored order で評価し、
省略された宣言値はその後に補う。bundle は `ViewActionButtonInput` の順序付き
record で label source と enabled value を保持し、別の並び順 table を作らない。
各 program は Text と同じ input ABI、結果型、pure effect と executable owner
の検証を受ける。失敗した control は部分的な mount を公開しない。

mount の `action_buttons` は評価済みの label／enabled を持つ。セッションはこの
値を描画・入力・観測へ投影し、静的な初期値で上書きしない。disabled の Button
は activation route を公開せず、以前の route を失効させる。restore 後も retained
parameter と同じ式から control value を再評価する。

builtin element の x／y／width／height は registered Length 型で検査し、受理した
compile-time scalar fact をその要素の inline Style patch へ投影する。現在の layout
profile は px の固定小数点 thousandths を使い、pt／em は換算規則を持たない
ため拒否する。width／height は border-box 寸法。x／y の指定は Position.Absolute
と物理 Left／Top を与え、包含領域の原点からの位置とする。省略した軸や寸法は
既存の Style/layout 規則に従う。これらの入力は新しい runtime scalar interpreter
や別の bounds table を作らず、既存の cascade、geometry、paint、hit-test 境界を使う。

## Binding

Binding は直接 state を破壊的に書き換えず、lens + event/command。

## Bundle execution contract

View program bundle は単一の暗黙 root や index-only child span を持たない。各
View 宣言を、次の閉じた定義 record として保持する。

- package/module scoped `public_id`
- 共通 instruction inventory 内の半開区間 `body`
- authored order の parameter schema（ordinal、name、semantic type、optional な
  scalar runtime type／definition-scoped value slot、typed default program）
- mount-state schema hash

`CallView` は対象 definition ID と、parameter ordinal/name に結び付いた
`ViewValueProgramId` を保持する。必須引数の欠落、未知の引数、型不一致、重複
binding、未知の View は bundle 作成または decode 時の structured failure であり、
no-op へは落とさない。空の View body は長さ 0 の正規 span として有効である。
同名 parameter でも View definition が異なれば別 slot であり、型を共有・推測
しない。local と repeat ordinal の state slot も definition ID で scope される。

flow から mount された View を root とし、View body 内の nested View call を
再帰的にたどった到達可能な定義だけを bundle に含める。`mod game.opening` 内の
`Child(...)` は `view.game.opening.Child` へ解決される。module path の区切りは
`.` である。

名前から導く View の public ID は、final HIR の header projection が canonical module
と宣言名から一度生成する。これは [全宣言familyの共通規則](../01-language/ids-and-references.md)
であり、明示 ID は authored identity のまま保持する。最終 freeze、
retained symbol、sema entity/callable join、compiler、bundle は同じ published identity
を照合・消費し、consumer が module/name から別の ID を再生成しない。

## Retained execution and mount identity

Runtime-driver は live presentation handle ごとに root View occurrence を一つ保持し、
nested call と keyed repeat は structural path で子 occurrence を識別する。同じ
View definition を main/side panel など複数の handle が同時に参照してよく、各
occurrence は別の monotonic `ViewMountId`、activation logical time、deterministic
seed、parameter/state revision、TextInput 値、Fx instance identity を持つ。resource
ID は definition identity であり、単一 owner を表さない。

定義内の構築 site は compiler が受理した typed inventory で識別し、動的な
occurrence は mount、site、型付き repeat key の path で識別する。instruction index
は受理世代内の実行位置であり、replacement を跨ぐ同一性には使わない。state、
style、text/image/control resource、geometry、paint、hit-test、focus、handler、Agent
observation は同じ node occurrence を参照する。直接繰り返した要素も item key を
含むため、同じ mount/source から生成した複数要素を一意に束縛できる。
構造編集による対応が曖昧な場合は explicit key または検証済みの対応計画を必要とし、
それがなければ決定的な reset/rejection とする。source 文字列や位置から推測しない。

View fragment program は構造の構築を所有し、一般 expression/default は既存の
`RuntimePureProgram` と `RuntimeValue` を使う。checked root と complete free-input
ABI から parameter、state projection、local、repeat item を型付きで渡す。Fx evaluator
への scalar projection は実際の `ApplyFx` ABI 境界に限定する。未初期化 slot、
型不一致、非有限値、budget 超過は structured diagnostic にする。
placeholder 値を実行値として使わない。context time は mount activation から
の logical seconds、ordinal は対象内の logical instruction/item index である。

checked root は expression の値生成と callable body の呼び出しを区別する。
宣言本体の root は宣言 identity と body role を組にして保持し、同じ accepted
HIR topology の body projection を使う。実行 ABI は正規順の free input と、
未使用・wildcard・分割 pattern を含む全 formal parameter を保持する。添付本文
parameter も formal input に含め、parameter default の評価は本体から分離する。
compiler と実行 plan は同じ admission を使い、閉じた instance ではその instance
identity と executable partition に一致する型・所有権 catalog を選ぶ。program
binding は通常の function frame を参照し、別の scalar helper ABI を作らない。
admission は閉じた置換と local-use 証拠を一つの所有された environment に保持する。
元の context、solution、semantic report の破棄後も、その環境から型と転送証拠を
参照できる。global catalog は report と admission で共有し、root ごとに複製しない。
実行 catalog の照合では instance identity と、この完全な証拠 authority の双方を
検証する。元の HIR 世代と登録済み callable authority の検証も維持する。
callable を返す default は値生成であり、capture の転送だけをその frame の入力に
含める。latent body と defer の cleanup body は別の実行境界で、cleanup の効果・
中断条件は保持する。field access は field identity と receiver の評価元を別々に
持ち、直接 binding の read と receiver expression の評価を二重に数えない。
宣言の型スコープに属する nominal record／enum の生成では、その実行 frame の不変な
型束縛を値の生成元として保持する。別 frame への転送と snapshot 復元では、
同じ executable の型表で生成元を検証し、転送先とは別の環境で nominal 引数を
比較する。case の選択と call の入力・結果・spread の型投影にも、この完全な
実行環境を使う。スコープの幅が同じでも転送先の束縛で生成元を代用しない。部分 move
後の record header もこの証拠を保持し、field の再初期化で失わない。
move した値の読み取り・借用は未初期化として拒否する。whole-local への代入は
同じ宣言を再初期化できる。部分 move は field ごとに追跡し、その field への
代入で復元できるが、owner 全体が未初期化のときは部分更新を拒否する。RHS 後の
旧値の状態を静的に求め、残っている所有値だけを cleanup する。mutation は
初期化済みの対象を必要とし、借用中の owner の消費・置換を拒否する。詳細は
[block scopes](../01-language/block-scopes.md) の所有権・部分 move 契約に従う。
shadowing は新しい local generation の初期化として扱う。
値の Copy/Move/Borrow と place の replacement/mutation はそれぞれの checked
access 証拠を必要とする。純粋な program への抽出は root 外の place mutation と
root 外を対象にする制御移動を拒否し、root 内の local mutation と loop exit は
通常の型付き実行契約で検証する。generic の入力・result・access は同一の宣言・
HIR 世代・確定済み置換で閉じる。Copy mode だけで深い複製可能性を仮定せず、
実際に渡す callable/opaque carrier の ingress 条件を検証する。
glyph-target sampler の ordinal は Fx application ごとに最初の対象 glyph を 0
として rebase し、文書全体の glyph index や UTF-8 byte offset を渡さない。
reduce-motion 時は sampler time を 0 に固定する。

Reactive Need observation is an ordinary checked `match`, not a retained
`Await` discriminant. The temporal owner has `NotStarted`, `Pending(Progress)`,
`Ready(T)`, and `Cancelled`; a fallible payload is matched as
`Ready(Result::Err(error))`. There is no Need-owned `error` or `denied` branch.
`Branch`, `Match`, keyed `Repeat`, nested `CallView`, `BindLocal`, and `ApplyFx`
run under the same frame operation/value budget. Unknown state, invalid pattern
ownership, or branch span mismatch is a typed diagnostic rather than a no-op.

評価結果は mount-scoped target/image、typed text source、Fx application を保持
する。plain text 以外の localized/RichText/display-frame source を文字列へ黙って
潰さない。実際の scene resource は typed node occurrence に束縛され、同じ
authored control を別 mount または repeat item で独立に操作できる。画像と scroll
element も同じ occurrence authority を持ち、後段の source/target 文字列検索で
対応関係を再構築しない。

View text bundle は source record と実体 store を分離する。localized store は
`(TextKey, locale)` に対する `RichTextDocument`、rich-text store は document ID に
対する `RichTextDocument`、display-frame store は frame ID に対する
`LineDisplayFrame` と stage index を保持する。参照先や stage が存在しない場合は
`VIEW014` から `VIEW017` の typed diagnostic で mount 評価を失敗させる。空文字、
debug 表現、既定 locale、plain text への暗黙 fallback は使わない。locale 未指定の
source だけは、bundle compile 時に同じ `TextKey` の canonical display catalog が
存在すれば、その document を初期 store として materialize してよい。

Player は評価済みの typed value を `ResolvedTextDocument` に解決し、通常の
`TextLayout` と frame-local `PreparedTextBatch` へ直接追加する。vertical writing、
ruby、text-combine、locale、run source、selection/scroll clip はこの境界まで型付きで
保持し、View 専用の string block や二度目の layout は作らない。

paint IR は `Element`、`Text`、`Image` と nested `Mount` の authored order を保持
する。nested mount は親の `Mount` slot で再帰展開するため、親の前後へ後置されない。
View 所有 image は通常 image pass から View scene resource table へ移し、crop UV、
affine transform、opacity を保持した `Image` primitive として同じ painter sequence に
置く。したがって Text/Image/element/child View の相対順序は native、Web、headless
で一つの scene contract になる。

save/load は整数 logical timestamp、mount allocator cursor、root bindings、occurrence
path、整数 activation time、seed、typed parameter/state value と revision、供給/default
provenance、state 初期化状態を保存する。handler、Fx projection、snapshot は同じ
値の正本を読む。sampler に渡す秒数は整数差分から変換し、浮動小数秒を累積しない。
restore は program/schema/type/allocator と、保存済み
presentation frame が retained mount table の handle/path/View/mount identity に一致
することを代入前に検証する。

replacement は View/general expression、正確な handler runtime 世代、Style、text と
resource、field 単位の state migration を一つの候補にする。同じ field identity/型の
値は保持し、追加 field は initializer を一度実行し、削除 field の値と依存を解除する。
型変更は検証済み pure migration または明示 rejection とする。host の資産準備後に
geometry/paint/hit/action と focus/capture/editing state を同じ公開境界で commit する。
source 位置から migration を推測しない。restore/replacement は外部 interaction lease
の有効期間も更新し、巻き戻した論理 ID に古い invocation が対応付くことを拒否する。

```arcw
Slider(value = bind state.config.master_volume, range = 0.0..1.0)
```

展開:

```rust
Binding<f32> {
    get = .config.master_volume,
    set = |v| GameEvent.View(.SetMasterVolume { value = v }),
}
```

## Evaluation cost and derived resources

cache は破棄、容量変更、save/load によって意味が変わらない派生情報とする。
同じ評価は interpreter、cache hit、最適化 backend で同じ canonical semantic fuel を
消費し、成功/診断と公開結果が一致する。物理的な実行命令数と CPU/GPU 処理時間は
別の性能指標とする。フレームを跨ぐ work queue は順序と commit 状態を持つ正式な
決定的状態として扱う。

不変 program/font/image 資産は inventory/revision で共有し、transaction のために
フレームごとに font bytes、font database、資産全体を再構築しない。派生 cache は
完全な key を持ち、意味上の state と分ける。value/structure、style、measure、
place/clip、paint/hit の各段階は変更とその波及範囲で invalidation を伝播する。
schema/storage は definition と実際の free input に対応し、別 View の slot 追加で
無関係な mount の値を reset しない。

Text intrinsic measure は既存の text layout を content、font inventory、typography、
locale、writing direction、available constraints から生成し、同じ結果を paint/hit/
selection に渡す。CPU の state/branch/action/item identity は同じ入力列と資産から
一致し、論理 geometry/hit は固定した数値/font/layout profile で一致する。GPU pixel
は固定環境の golden と platform 間の許容差を別々に検証し、装飾的な差を state の
決定へ逆流させない。

## Reactive dependencies

view 評価中に読んだ依存を記録する。

```rust
pub enum ViewDependency {
    StatePath(StatePathId),
    Signal(EntityId),
    LocalState(ViewLocalId),
    Environment(EnvironmentKey),
    Resource(AssetId),
    Font(FontId),
    Locale,
}
```

変更時は該当 view だけ invalidated。

## View / Modifier

```arcw
Text("聞いてみる")
    .font(.body)
    .padding(x = 24, y = 12)
    .background(.button)
    .corner_radius(8)
    .transition(.fade(duration = 120ms))
    .animation(.spring, value = is_selected)
```

## Reactive `Need` match

View does not implicitly force a `Need`. It uses the ordinary `match` grammar,
then projects that checked expression into the retained View subscription and
branch owner:

```arcw
match load_avatar(user) {
    .pending(progress) => SkeletonCircle(progress = progress)
    .ready(.Ok(image)) => Image(image)
    .ready(.Err(error)) => ErrorMessage(error)
}
```

`AwaitView` is not a retained parser, HIR, formatter, or LSP surface. Outside a
View context, `await` remains continuation suspension rather than reactive
branch selection. Need itself owns no domain-error or denial branch; those are
represented by the Ready payload or by a typed admission result. Cancellation
remains a separate control outcome.
```

## Retained list virtualization

Virtualized lists are addressed by mount occurrence, not only by View program.
The implemented range/save substrate therefore keeps independent source
inventory, viewport, offset, and materialized-window state for two mounts of
the same program. Typed child-local state belongs to the future evaluator and
must use the same mount/key identity; the range planner does not claim to
serialize an opaque child state value.

The Sans I/O range contract consumes a finite ordered item set. Every item has
a stable key and a resolved non-zero primary-axis extent in logical
milli-pixels. It produces one half-open materialized window plus a complete
range table. Items outside the window remain in the table and are reported as
non-materialized. Leaving the window does not discard their stable range
identity. Whether concrete child-local state is retained, pruned, focused, or
temporarily materialized is an evaluator policy and is not invented by this
Sans I/O planner.

Live source replacement preserves a key-relative scroll anchor when source
order changes. Save/load instead restores the exact finite inventory and
absolute offset; its derived anchor is an integrity check, so contradictory or
tampered offset/anchor pairs are rejected rather than silently normalized.

`LazyRow` and `LazyColumn` authoring must not be implemented as eager Row/Column
aliases. The grammar becomes available only when the typed View evaluator can
provide finite keyed values and the layout layer can resolve off-window extents
under one deterministic measurement policy. That evaluator must also allocate
an occurrence-specific actionable Scroll identity; the current player and
Agent action path still addresses authored Scroll strings and cannot
independently route two mounts of one authored Scroll.

## Agent output

View node は bbox / polygon / mask / action target を持つ。

```rust
pub struct ViewNode {
    pub entity: Option<EntityId>,
    pub role: ViewRole,
    pub label: Option<String>,
    pub bbox: BBox,
    pub polygon: Option<Polygon>,
    pub actions: Vec<ActionTarget>,
}
```

## Retained dependency tracking and local handlers

View は dependency tracking によって必要部分だけ再評価される。派生値は
ordinary pure function として記述し、再利用と invalidation は View evaluator
が実際の read dependency から決める。

```arcw
fn visible_choices(state: GameState) -> Vec<ChoiceView> {
    opening_choices()
        .filter(choice_available(state))
        .map(choice_to_view(state))
        .collect<Vec<ChoiceView>>()
}

view ChoiceList(state: GameState) {
    let choices = visible_choices(state)

    Column {
        for choice in choices key = choice.id { ChoiceButton(choice) }
    }
}
```

入力処理は `.on_click { ... }` など対象 View node の modifier に置く。
View 全体の invariant は compiler/test が retained tree と action inventory を
直接検査し、global callback を挟まない。
