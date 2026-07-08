結論としては、大きく世界観や DSL を変える必要はありません。今の Arcweft 仕様には、Sans I/O core、`Need`、`lazy use`、GraphPatch、`EntityId + SemanticHash`、`stable_counter` など、incremental build / hot reload 向きの部品がすでにあります。特に、hot reload は「parse → typecheck → contract/shader/ABI/state compatibility check → current continuation dry-run → frame boundary commit」と明文化済みなので、この方向は維持でよいです。

ただし、早めに変えておいた方がいい点はあります。

## 1. ID は「行番号・出現順」由来にしない

一番重要です。現在の ID 仕様は、`EntityId / PublicId / DisplayId / SemanticHash` の 4 層で、`PublicId` は rename 可能、履歴・RAG・GraphPatch は主に `EntityId + SemanticHash` を使う方針になっています。さらに `seq` は registry で保持し、挿入時に既存 ID をずらさない、と書かれています。これは正しいです。

変えるなら、`sequence = "stable_counter"` という名前をもう少し誤解しにくくして、実装上は `stable_slot` に寄せるのがよいです。

```toml
[id.rules.say]
pattern = "say.{flow}.{slot:03}"
slot = "stable_registry_slot"
scope = "flow"
renumber_on_format = false
```

`counter` という名前だと、formatter や挿入で再採番される実装になりがちです。hot reload では、台詞や choice option の ID がずれると、voice、localization、save/replay、GraphPatch、memo cache が全部壊れます。

おすすめはこの割り振りです。

```text
EntityId:
  ent:01J... のような opaque / rename-stable ID

PublicId:
  #say.opening.001
  #choice.opening.listen
  #flow.opening
  など、ユーザーが見る階層 ID

SemanticHash:
  canonical HIR から作る
  whitespace / コメント / source range / PublicId 文字列 rename は基本除外
  参照は PublicId 文字列ではなく解決済み EntityId で hash

DisplayId:
  LSP inlay hint / UI 用
```

`PublicId` の自動生成は許可してよいですが、生成結果は `.arcweft/entities.toml` に保存して、次回以降は registry を正にします。初回だけ source anchor や近傍 hash で推定し、確定後は行番号や byte offset から再生成しない方が安全です。

## 2. parser は「失敗したら Err」ではなく、壊れた CST/AST も返す

現状の実装は `parse_source(...) -> Result<SyntaxTree, Vec<ParseError>>` で、内部も `SourceLine` ベースの parser になっています。 これは MVP としては十分ですが、hot reload / LSP / incremental build では少し弱いです。

実装ガイドでは、`rowan` 互換の lossless CST、コメント・空白・ID・`[[...]]` link・`#...` reference の保持、recovery parser を優先する方針がすでに書かれています。 ここは仕様として強制した方がいいです。

API は例えばこうです。

```rust
pub struct ParsedSource {
    pub green: GreenNode,
    pub ast: SyntaxTree,
    pub errors: Vec<ParseError>,
    pub file_hash: Hash,
    pub line_index: LineIndex,
}
```

`Err(Vec<ParseError>)` で tree を失うと、編集中の hot reload preview、LSP diagnostics、GraphPatch preview、ID materialize が止まります。壊れた部分は `ErrorNode` / `RawItem` / `MissingToken` として残し、正常な subtree は再利用できるようにするのがよいです。

## 3. build cache は `file` 単位ではなく `entity / item` 単位にする

今の architecture は `CST → HIR → Typed Graph → IR → Bytecode/JIT/Bundle` という段階分けです。 これをそのまま incremental build の cache key に落とすとよいです。

おすすめの hash 分割はこれです。

```text
FileHash:
  raw source

ParseHash:
  token/CST hash

ItemInterfaceHash:
  exported name, kind, signature, type, effect set, visibility, ABI

ItemBodyHash:
  body HIR

SemanticHash:
  canonical typed HIR

DependencyHash:
  resolved EntityId deps + interface hashes + asset/shader/layout deps

ArtifactHash:
  target/profile/toolchain/build-mode を含む
```

これにより、たとえば台詞本文だけの変更なら、型情報や他 flow の compile を飛ばせます。`pub fn` の signature が変わった場合だけ依存 item を再 typecheck します。

`.arcweft/build-cache/` には、最低限このような record を保存するとよいです。

```toml
[[item]]
entity = "ent:01J8X6K9XW4M9F2D7A1R8QZ6CN"
public = "say.opening.001"
kind = "Say"
source = "game/routes/opening.awft"
item_interface_hash = "b3:..."
semantic_hash = "sem:b3:..."
dependency_hash = "b3:..."
artifact_hash = "b3:..."
```

## 4. `lazy use` は今の仕様を維持し、summary / body を分離する

`lazy use` は「export summary だけを読み、body parse/typecheck/compile/load は初回使用まで遅延」と定義されています。これは incremental build と hot reload にかなり向いています。

ここで追加したい仕様は、module summary の形式です。

```rust
pub struct ModuleSummary {
    pub module: ModulePath,
    pub exports: Vec<ItemSummary>,
    pub interface_hash: Hash,
    pub source_hash: Hash,
}

pub struct ItemSummary {
    pub entity_id: EntityId,
    pub public_id: PublicId,
    pub kind: ItemKind,
    pub signature_hash: Hash,
    pub effect_hash: Hash,
    pub visibility: Visibility,
}
```

`lazy use` 先の body が変わっても、interface hash が同じなら import 側は rebuild しなくてよい、というルールを固定できます。

## 5. script 文法は「別 script 言語なし」のままでよいが、sugar の canonical lowering を固定する

Arcweft は別の `script` item を定義せず、普通の VN 記述を `flow` grammar の一部として扱う設計です。compact scenario statement、character method call、typed statement がすべて `FlowItem` になる方針も明記されています。 これは hot reload 的には良いです。別 script lowering phase があると、差分の境界が増えて壊れやすくなります。

ただし、sugar は必ず canonical HIR に落としてから hash するべきです。

```awft
alice: おはよう。[p]
```

と

```awft
alice.say()[
    おはよう。[p]
]
```

は同一の HIR にする。`alice(id=#say.opening.greeting, face=smile, voice=auto):` も、canonical method call に落としてから `SemanticHash` を作る。

また、古い `alice #say... @smile voice auto:` 形式は、すでに deprecated とされています。これは早めに formatter migration 対象にして、stable 版では消す方がよいです。複数の同義文法が残ると、incremental parser と formatter の round-trip が難しくなります。

## 6. choice option / dialogue line / hook / memo にも必ず stable child entity を持たせる

`flow` 自体だけでなく、以下も entity にした方がよいです。

```text
Flow
Fragment
Say line
Choice
ChoiceOption
AwaitWith branch
Hook
MemoFn
UI subtree root
Shader block
Asset ref
Audio cue
```

Graph node の種類にはすでに `Flow`, `Say`, `Choice`, `ChoiceOption`, `Await`, `AssetRef`, `ShaderRef`, `ViewPanel`, `AudioCue` などが含まれています。 したがって、Graph 用だけでなく build / reload の単位としても同じ entity を使うのが自然です。

特に choice option は、ラベル文字列や option の出現順から ID を作らない方がよいです。

```awft
@choice #choice.opening.first {
    #choice.opening.listen "聞いてみる" -> #flow.alice_intro
    #choice.opening.silent "黙っている" -> #flow.quiet_intro
}
```

これは良いです。省略を許す場合も、registry に stable slot を保存します。

```awft
@choice #choice.opening.first {
    "聞いてみる" -> #flow.alice_intro   // 初回だけ #choice.opening.first.001 を推定
}
```

その後は LSP code action で明示 ID を挿入する運用が安全です。

## 7. hot reload には compatibility matrix を足す

今の hot reload pipeline はよいですが、何を frame boundary commit できて、何を reload 不可にするかを仕様化した方がいいです。

おすすめはこの分類です。

```text
Safe hot reload:
  dialogue text
  localized text
  face / bg / staging parameter
  pure memo body
  hook body, effect set が同じ場合
  shader body, interface が同じ場合
  View layout, state schema が同じ場合

Conditional hot reload:
  flow body
    current continuation が old→new に mapping できる場合のみ
  function body
    signature / effect / ABI が同じ場合のみ
  choice options
    current selected id / save data と互換なら可

Needs migration:
  state field add/remove/rename
  enum variant change
  save schema change
  public function signature change
  Activity ABI change

Reload rejected / restart required:
  Rust dylib ABI incompatible
  WASM import/export incompatible
  core feature flag change
  target/profile/build-mode change
```

`FlowFiber` は `pc: ProgramCounter` と `continuation: ContinuationId` を持つ設計なので、各 `FlowItem` / branch / line に stable `ContinuationId` を持たせると、現在位置を保った hot reload ができます。

## 8. memo / JIT cache はすでに良いので、function hash と dependency snapshot を厳密に使う

Memo cache schema には `function_semantic_hash`, `args_hash`, `dependency_snapshot`, `value_hash` があり、hot reload patch committed 時には affected function/entity/layer/UI subtree caches を invalidate する方針があります。 これはそのままでよいです。

追加するなら、`SemanticHash` だけでなく `InterfaceHash` を持つと便利です。

```rust
pub struct FunctionCacheKey {
    pub entity: EntityId,
    pub interface_hash: Hash,
    pub semantic_hash: Hash,
    pub type_layout_hash: Hash,
    pub args_hash: Hash,
    pub dependency_hash: Hash,
    pub target: TargetProfile,
}
```

body だけ変わった場合は dependent interface users を rebuild しない。signature が変わった場合だけ dependent users を rebuild する、という切り分けができます。

## 9. CLI / scripts は今のうちに dev/watch 系を予約しておく

現在の CLI 実装はまだ stub です。 なので、今のうちにコマンド名を予約しておくと後で破壊的変更が減ります。

おすすめはこのあたりです。

```bash
arcw check
arcw check --watch --emit json

arcw build
arcw build --incremental
arcw build web --mode single-thread
arcw build web --mode threads
arcw build web --mode both

arcw dev
arcw dev --hot-reload
arcw dev --hot-reload --dry-run
arcw dev --hot-reload --reject-unsafe

arcw ids infer
arcw ids materialize
arcw ids rename #say.opening.001 #say.opening.greeting
arcw ids repair
arcw ids gc

arcw graph build
arcw graph patch preview patch.json
arcw graph patch apply patch.json

arcw cache explain #flow.opening
arcw cache invalidate #flow.opening
```

特に `ids materialize` は重要です。開発中は implicit ID を許可し、release build や localization extraction では ID materialize 済みを要求すると、書きやすさと安定性を両立できます。

## 10. 変更するなら優先順位はこれ

P0 は、lossless CST + error-tolerant parser、opaque `EntityId` + registry、`stable_slot` ID、item/entity 単位の build graph です。ここを後回しにすると、後で hot reload 対応時に DSL と保存形式を大きく直すことになります。

P1 は、hot reload compatibility matrix、ContinuationId mapping、canonical sugar lowering、GraphPatch と text edit の同一 pipeline 化です。

P2 は、CLI watch/dev/cache explain、formatter migration、release build 時の ID materialize 強制です。

一言でまとめると、Arcweft の現仕様は方向性としてかなり合っています。変えるべきなのは「構文そのもの」よりも、「ID を絶対に揺らさない」「parser が壊れた入力でも tree を返す」「hash を interface/body/dependency に分ける」「hot reload 可能な変更と不可な変更を仕様化する」の4点です。

おすすめは **Parser では lifetime 構文だけを読む、HIR で scope graph を作る、実際の escape / borrow-cross-boundary 診断は typecheck 後段の borrowck で持つ**、です。

Arcweft 仕様上、`await` / `yield frame` / `select` / `spawn` / `defer` / `lazy let capture` は suspension boundary で、`&'frame T`、`&'lease T`、`&mut T` は boundary を跨げない、とすでに定義されています。 また line plan では `with:` が lexical scope を作り、`out` された値だけが外へ出られ、borrowed value は `out` や `at` / `await` / `yield` / cancellation boundary 越えが禁止されています。 なので診断は型・effect・scope が見えた後に出すのが自然です。

## 役割分担

### Parser / CST / AST

Parser はここまでで止めるのがよいです。

```awft
&'frame [u8]
&'lease mut Buffer
borrow bg.pixels() as pixels: &'asset [Rgba8] { ... }
alice.say()[ ... ] with: ...
```

Parser が持つべき責務は、

```text
- lifetime token `'frame` を lifetime label として読む
- `&'a T`, `&'a mut T`, `&mut T`, `&T` を TypeRef::Borrow として読む
- borrow block の形を読む
- line plan / with: / with { ... } / out / at / cancel の構文を読む
- span と source range を正確に残す
- 構文エラーだけ出す
```

です。

Parser で出してよい診断は、例えばこれです。

```text
E_PARSE_LIFETIME_EXPECTED
  `&' [u8]` のように `'` の後に識別子がない

E_PARSE_BORROW_BINDING
  `borrow expr as name { ... }` のように型注釈が欠けている

E_PARSE_WITH_BLOCK
  `with:` の indent が壊れている
```

逆に Parser で出さない方がよいものは、

```text
unknown lifetime
borrow crosses await
line handle escape
`alice[...]` が dialogue call か index か
```

です。特に `alice[...]` は、仕様上も parser が判断できない場合は generic `PostfixBracket` CST node として残し、HIR lowering が型で `DialogueContentCall` か `IndexExpr` に解決する方針になっています。

### HIR lowering

HIR は「診断の本体」ではなく、**診断に必要な構造を作る場所**にします。

HIR でやることは、

```text
- dialogue sugar を canonical DialogueLine に正規化
- `with:` を canonical with block に正規化
- borrow block に BorrowRegionId を振る
- line plan に LineScopeId を振る
- `at`, cancel branch, await, yield, select, spawn, defer を boundary node として記録
- lifetime name を RegionRef に解決する
- capture set / out expression / local bindings を記録する
```

です。

例えば HIR はこういう構造を持つとよいです。

```rust
pub enum RegionKind {
    Static,
    Frame,
    Flow,
    Scene,
    Asset,
    Lease,
    LexicalBlock(BlockId),
    BorrowBlock(BorrowRegionId),
    Line(LineScopeId),
    Inferred,
}

pub struct BorrowBlockHir {
    pub id: BorrowRegionId,
    pub source: ExprId,
    pub binding: BindingId,
    pub binding_ty: TypeId,
    pub body: Vec<HirStmt>,
    pub span: SourceSpan,
}

pub struct LinePlanHir {
    pub line_scope: LineScopeId,
    pub locals: Vec<BindingId>,
    pub items: Vec<LinePlanItemHir>,
    pub out: Option<ExprId>,
    pub boundaries: Vec<BoundaryId>,
}
```

HIR で出してよい診断は、名前解決レベルのものです。

```text
E_UNKNOWN_LIFETIME
  `&'fram [u8]` のように lifetime 名が解決できない

E_LIFETIME_NOT_IN_SCOPE
  `fn f(xs: &'a [u8])` で `'a` が宣言されていない

E_BUILTIN_LIFETIME_SHADOW
  `fn f<'frame>(...)` のように builtin lifetime を shadow した

E_LINE_LOCAL_NOT_VISIBLE
  line plan 内の `local_color` を line 外から参照した
```

ただし、`borrow crosses await` や `line handle escape` は HIR ではまだ出さない方がいいです。`Expr` の型が `&'frame [u8]` なのか、owned `ImageHandle` なのか、`ScopedHandle<'line>` なのかが確定していないからです。

### Typecheck / effect check / borrowck

ここが本命です。

Typecheck 後段に小さな borrow checker を置き、HIR が作った scope graph と boundary 情報を使って診断します。

```text
type inference
  -> effect inference
  -> region inference
  -> borrow / escape check
```

ここで見るべきものは、

```text
- 各値の型
- 各 borrow の origin region
- 各 binding の live range
- suspension boundary
- line scope
- `out` で外へ出る値
- closure / at / cancel branch capture
- handle の scope parameter
```

診断はここに集約します。

```text
E_BORROW_CROSSES_SUSPEND
  &'frame T / &'lease T / &mut T が await, yield, select, spawn, defer を跨ぐ

E_BORROW_ESCAPE
  borrow block 内で作った参照が block 外へ出る

E_LINE_BORROW_ESCAPE
  line plan 内の borrowed value を out した

E_LINE_HANDLE_ESCAPE
  StageLease<'line>, VoiceHandle<'line>, ScheduledCueHandle<'line> などを detach / retarget せず line 外へ出した

E_CAPTURE_CROSSES_BOUNDARY
  at(...) / cancel / spawned task が短命 borrow を capture した

E_MUT_BORROW_ALIAS
  &mut T と他 borrow が重なった
```

## `line handle escape` は typecheck 側に寄せる

これは特に HIR ではなく typecheck 側がよいです。

理由は、`line handle` かどうかは構文ではなく型で決まるからです。

```awft
let actor = alice.stage.acquire(scope=line)
out actor
```

これは見た目だけでは、`actor` が owned value なのか、`StageLease<'line>` なのか、`DetachedStageHandle` なのか分かりません。型が付いて初めて判断できます。

型としては、こう持つと分かりやすいです。

```rust
pub enum HandleScope {
    Line(LineScopeId),
    Flow,
    Scene,
    Global,
    Detached,
}

pub struct HandleTy {
    pub kind: HandleKind,
    pub scope: HandleScope,
    pub drop_policy: DropPolicy,
}
```

そして `out` の型を見て、

```text
HandleScope::Line(current_line)
  -> line 外へ out するなら error または warning

HandleScope::Detached
HandleScope::Global
HandleScope::Scene
  -> escape 可

&'frame T
&'lease T
&mut T
  -> escape 不可
```

にします。

例えばこれは error がよいです。

```awft
let actor = alice.say()[聞いて。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    out actor
```

診断:

```text
E_LINE_HANDLE_ESCAPE:
  `actor` has type `StageLease<'line>` and cannot escape this line.
  Use `actor.detach()`, acquire with `scope=scene`, or keep it inside the line plan.
```

これは OK です。

```awft
let actor = alice.say()[聞いて。[p]]
with:
    let actor = alice.stage.acquire(scope=line)
    out actor.detach()
```

## `borrow scope` は HIR で構造化、判定は borrowck

`borrow` block は HIR で region を作るのがよいです。

```awft
borrow bg.pixels() as pixels: &'asset [Rgba8] {
    let average = pixels.average_color()
}
```

HIR では、

```text
BorrowRegionId = br0
binding pixels: &'asset [Rgba8]
body region = br0
```

を作るだけ。

その後 borrowck が、

```text
pixels の live range は br0 内に収まるか
pixels が await / yield / at / cancel / spawn に capture されていないか
pixels が out / return / global store されていないか
```

を判定します。

例えばこれは typecheck/borrowck error です。

```awft
let leaked =
    borrow bg.pixels() as pixels: &'asset [Rgba8] {
        out pixels
    }
```

```text
E_BORROW_ESCAPE:
  `pixels` is borrowed from `bg.pixels()` and cannot escape the borrow block.
```

これも error です。

```awft
borrow bg.pixels() as pixels: &'asset [Rgba8] {
    await do_something() with {
        pending p => scene #scene.loading {}
    }
    use_pixels(pixels)
}
```

```text
E_BORROW_CROSSES_SUSPEND:
  `pixels: &'asset [Rgba8]` is live across `await`.
```

## `&'frame [u8]` の lifetime syntax は parser で薄く、HIR で解決

`&'frame [u8]` は TypeRef ではこう持つとよいです。

```rust
pub enum TypeRef {
    Borrow {
        mutability: Mutability,
        lifetime: Option<LifetimeName>,
        pointee: Box<TypeRef>,
        span: SourceSpan,
    },
    Slice(Box<TypeRef>),
    Path(TypePath),
    // ...
}
```

Parser は `LifetimeName("'frame")` を入れるだけ。

HIR で、

```rust
pub enum RegionRef {
    Builtin(BuiltinRegion::Frame),
    Param(LifetimeParamId),
    Inferred(InferRegionId),
}
```

に解決します。

`&[u8]` の lifetime 省略は、HIR では `RegionRef::Inferred(_)` にして、typecheck で elision rule を適用するのがよいです。

## 最終案

一番きれいなのはこれです。

```text
Parser:
  構文を lossless に読む
  lifetime / borrow / line plan の shape を保持
  syntax error だけ出す

HIR:
  sugar を canonical form に落とす
  scope graph / region graph / boundary graph を作る
  lifetime name resolution と visibility 診断を出す

Typecheck + borrowck:
  型、effect、region、handle scope を解く
  borrow crosses await
  borrow escape
  line handle escape
  capture crosses boundary
  を出す
```

つまり、`line handle escape` と `borrow scope` の本質的な診断は **typecheck 後段の borrowck** に置くのがよいです。HIR はそのための `LineScopeId`、`BorrowRegionId`、`BoundaryId`、`CaptureSet`、`OutExpr` を作る場所にするのが、incremental build / hot reload / LSP のどれにも一番強いと思います。
