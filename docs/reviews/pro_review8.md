整理し直すと、Arcweftは **「Wasmランタイムを主ランタイムにする」よりも、「Arcweft独自IR/VMを正にし、Wasm・Cranelift・Rust codegenを用途別backendにする」** のがよいです。

## 結論

Arcweftは次の4系統に分けるべきです。

| 系統                                  | 目的                        | 実行方式                                                 | 使うもの                                                     |
| ----------------------------------- | ------------------------- | ---------------------------------------------------- | -------------------------------------------------------- |
| **Dev / authoring runtime**         | hot reload、LSP、debug、差分実行 | Arcweft VMが正、nativeのみCranelift JIT可                  | `arcweft-lang-vm`, optional `arcweft-lang-jit-cranelift` |
| **Native product: embedded bundle** | まず出すべき製品版                 | native Rust runtime + `.arftpack`/bytecode/assets埋め込み | VM、必要ならCranelift JIT                                     |
| **Native product: full AOT**        | スクリプトまでネイティブ化             | Arcweft IR → generated Rust crate → `cargo build`    | `arcweft-lang-codegen-rust`                              |
| **Web / browser**                   | ブラウザ配布                    | Rust runtimeをwasm化 + Arcweft bytecode/AOT wasm       | `wasm32-*`, JS host, WebGPU/WebAudio/DOM                 |
| **Wasm plugin / sandbox**           | Activity/pluginの隔離        | WasmtimeでWasm componentを実行                           | Wasmtime, WIT, wasm-tools                                |

既存docsの方向性ともかなり合っています。Arcweftの全体設計はすでに `CST → HIR → Typed Graph → IR → Bytecode/JIT/Bundle` というcompiler/tooling段階を置き、その下にSans I/O Coreを置いています。また `arcweft-core` は GPU、Audio、Servo、DOM、filesystem、network、WASM runtime、Cranelift runtime に直接依存しない、と明記されています。これは維持すべきです。

## まず決めるべき原則

**Arcweft VMを意味論の正本にする**のがよいです。

Cranelift JITも、Wasm出力も、generated Rustも、すべて **Typed IRから派生する最適化・配布用backend** にします。これにより、hot reload、GraphPatch、EntityId、Need、deterministic replay、debug bus、LSP diagnosticsを同じ意味論の上で保てます。

```text
.awft
  ↓
lossless CST
  ↓
HIR
  ↓
Typed Graph / Typed IR
  ↓
  ├─ VM bytecode             dev / default / web fallback
  ├─ Cranelift JIT           native-only pure kernels
  ├─ generated Rust          native full AOT
  ├─ generated Wasm module   browser / Wasmtime target
  └─ bundle metadata         graph, assets, shaders, IDs, save schema
```

WasmをArcweft内部IRの正本にする案も考えられますが、Arcweftの `flow`、`Need`、suspension boundary、stable ID、GraphPatch、dialogue line、choice、hot reload compatibilityをWasmの低レベルcontrol flowに早く落としすぎると、LSPや差分ビルドが難しくなります。Wasmは **配布形式・plugin sandbox・一部AOT backend** として扱うのが安全です。

## Craneliftの位置づけ

Craneliftは **Arcweft全体を動かすランタイムではなく、ネイティブコード生成backend** です。Cranelift自身も「target-independent IRを実行可能なmachine codeへ変換する低レベルcode generator」と説明されています。

Arcweft docsの `Cranelift JIT` 方針はかなり良いです。そこでは、VMが正規実行系で、JITはnative-only最適化backend、対象はpure/deterministicな関数に限定、flow control・effect発行・asset/audio/shader load・UI操作などは対象外、とされています。これはそのまま採用すべきです。

なので、Craneliftはこう使うのがよいです。

```text
対象:
  pure numeric function
  easing
  layout expression
  animation sampling
  audio envelope
  filter/map fusion
  shader parameter precompute

対象外:
  flow本体
  Need / await / select
  save/load
  effect発行
  UI操作
  asset/audio/shader I/O
  plugin call
```

つまり、**Craneliftは「ゲームスクリプト全体の実行系」ではなく、「Typed IR中の安全に切り出せるpure function accelerator」** です。nativeではVM fallbackを持ち、dev/testではVMとJITの同値性検査を必須にします。Arcweft docsにも `--compare-vm`、CLIF dump、asm dump、JIT failed時のVM継続がすでに書かれています。

一点だけ直すべきなのは、`arcweft-core` に `jit-cranelift` featureが予約されていることです。 依存ルールではcoreがCraneliftに直接依存しないと書かれているので、featureは `arcweft-core` ではなく、`arcweft-lang-jit-cranelift` または `arcweft-player-native` 側へ移す方がきれいです。crate mapにも `arcweft-lang-jit` が想定されています。

## Wasmtimeの位置づけ

Wasmtimeは **Arcweft native runtime内のWasm plugin / Activity host** として使うのが最も自然です。WasmtimeはWebAssemblyのstandalone runtimeで、Craneliftを使ってruntimeまたはahead-of-timeでmachine codeを生成でき、WASIもサポートします。READMEではRustコードを `wasm32-wasip2` にコンパイルしてWasmtimeで実行する例も示されています。

Arcweftでは、Wasmtimeをこう使うのがよいです。

```text
Native Arcweft Player
  ├─ Arcweft VM / JIT
  ├─ wgpu/audio/UI/device host
  └─ Wasmtime sandbox
       ├─ wasm Activity
       ├─ wasm plugin
       ├─ scripted external parser
       └─ tool/test/headless component
```

Wasmtimeを **Arcweft全体の主ランタイム** にするのはおすすめしません。理由は、Arcweftはwgpu、Audio、UI、DOM/Servo、device、Agent Debug Busなど大量のhost機能を持つため、結局Wasmtimeの外側にArcweft hostを作ることになります。しかもブラウザではWasmtimeをそのまま使うのではなく、ブラウザ自身のWebAssembly engineとJS host adapterで動かす必要があります。Arcweft docsでもweb buildはBrowser DOM、canvas/WebGPU/WebGL、Engine::step、WebAudio、DOM UI backendとして整理されています。

Wasmtimeは主にこの3用途です。

1. **native plugin sandbox**
   Rust/C/C++/他言語のActivityをWasm componentとして実行する。

2. **headless / CI / server runner**
   `arcweft_player_wasi.wasm` のようなWASI componentをWasmtimeで走らせる。

3. **AOT wasm artifactのnative実行**
   pluginや一部script moduleをWasmtimeで事前コンパイルして、native productで高速起動する。

## wasm-toolsの位置づけ

`wasm-tools` はruntimeではなく、**WebAssembly module/componentを検証・生成・解析・変換するcompiler/tooling層** です。READMEでも「CLI and Rust libraries for low-level manipulation of WebAssembly modules」と説明され、validate、parse、print、mutate、strip、objdump、component new、component witなどのサブコマンドが列挙されています。

Arcweftではこう使うべきです。

```text
arcweft-wasm-tools integration:
  wasmparser      plugin/module validation
  wasm-encoder    Arcweft IR -> wasm module generation
  wat/wast        test fixtures
  wasmprinter     debug dump
  wit-parser      WIT interface parsing
  wit-encoder     generated WIT
  wit-component   core wasm -> component
  wasm-metadata   producer/name/build metadata
```

特に重要なのは、**plugin ABIをWITで固定する**ことです。

```wit
package arcweft:activity;

interface activity {
  record frame-input { tick: u64, dt-nanos: u64 }
  record frame-output { commands: list<command> }

  mount: func(ctx: mount-context) -> result<mount-result, activity-error>;
  step: func(input: frame-input) -> result<frame-output, activity-error>;
  save: func() -> result<list<u8>, activity-error>;
  load: func(bytes: list<u8>) -> result<_, activity-error>;
}
```

このWITを `arcweft-plugin-wasm` / `arcweft-wasm-wasmtime` / browser JS adapterが共有します。`wasm-tools component wit` や `component new` を使って、WIT抽出、component化、CI検証を行うのがよいです。`wasm-tools` はWITやcomponent関連のサブコマンドとライブラリを持っています。

## Native完全コンパイルをどうするか

ここは2段階に分けるべきです。

### 1. まずは「native binary + embedded bundle」

最初の製品版はこれで十分です。

```text
arcw build native --embed-bundle
```

生成物:

```text
my_game.exe / my_game.app
  ├─ Rust native Arcweft runtime
  ├─ embedded .arftpack
  │    ├─ VM bytecode
  │    ├─ typed graph
  │    ├─ assets
  │    ├─ shaders
  │    ├─ text/localization
  │    ├─ save schema
  │    └─ wasm plugins
  └─ optional native backends
       ├─ wgpu
       ├─ audio
       ├─ filesystem/storage
       └─ Wasmtime plugin host
```

これは「スクリプトはnative machine codeではない」が、配布物としては完全なnativeアプリです。VMを正にできるため、debug、save compatibility、hot reload後の差分検証、JIT fallbackを保ちやすいです。

### 2. 次に「scriptまでAOT」するなら、まずgenerated Rust

スクリプトまで完全にネイティブ化したい場合、最初にやるべきは **Arcweft IR → Rust codegen → cargo build** です。

```text
.awft
  → HIR
  → Typed IR
  → generated Rust state machines
  → link with arcweft-runtime
  → native binary
```

理由は、Craneliftで直接flow/continuation/Need/borrowing/save/loadを全部AOTするより、Rust codegenの方が安全で、型・所有権・panic strategy・debug info・platform対応をRust toolchainに任せられるからです。

イメージ:

```rust
pub struct FlowOpening {
    pc: FlowPc,
    locals: FlowOpeningLocals,
}

impl CompiledFlow for FlowOpening {
    fn step(&mut self, ctx: &mut FlowCtx<'_>) -> FlowPoll {
        match self.pc {
            FlowPc::Start => { /* generated */ }
            FlowPc::AfterLine001 => { /* generated */ }
            FlowPc::ChoiceFirst => { /* generated */ }
        }
    }
}
```

これなら `save/load` も `pc + locals schema` として扱えます。release buildではこのgenerated Rustをリンクし、dev buildではVMを使う、という切り分けができます。

### 3. Cranelift AOTは後段の最適化

Craneliftでnative objectを直接出すAOTは、最初から全scriptに使わない方がよいです。使うなら、JITと同じく **pure function subset** から始めます。

```text
Typed IR pure fn
  → purity/effect check
  → type layout check
  → Cranelift IR
  → native object / cached compiled fn
```

flowやdialogueやNeedをCranelift AOT対象にするのは、VM、Rust codegen、save schema、effect systemが固まった後で十分です。

## Web / browserではどうするか

ブラウザ版では **Cranelift JITもWasmtimeも使わない** 前提にします。Arcweft docsでもwebではJIT無効、VM / wasm / browser wasm backendを使う、と書かれています。

推奨構成:

```text
dist/web/
  arcweft_player_st.wasm
  arcweft_player_mt.wasm
  loader.js
  game.arftpack
  assets/
```

Arcweft docsにもweb artifactとして `arcweft_player_st.wasm`, `arcweft_player_mt.wasm`, `loader.js` が想定されています。

webではまずこの構成が良いです。

```text
Rust Arcweft runtime → wasm
.awft scripts       → VM bytecode in .arftpack
browser host        → JS adapter for DOM/WebGPU/WebAudio/storage/input
```

その後、必要なら次の順で進めます。

```text
Phase Web-1:
  runtime.wasm + bytecode VM

Phase Web-2:
  pure functions only → generated wasm functions

Phase Web-3:
  selected flows → generated wasm state machines

Phase Web-4:
  wasm component plugin support via JS host adapter
```

ただし、browser pluginは最初からWasmtime component model相当を完全再現しようとしない方がいいです。まずは **Arcweft runtime wasm + JS loaderが外部 `.wasm` を instantiate し、限定されたhost importsだけ渡す** 形で十分です。

## カスタムbackend上で動くruntime

ここでいうbackendは、compiler backendではなく、render/audio/storage/deviceなどのruntime backendとして整理した方がよいです。

Arcweft coreはSans I/Oなので、

```rust
Engine::step(RuntimeStepInput) -> RuntimeStepOutput
```

だけを正にして、`RuntimeStepOutput` 内のcommands/effectsをbackendが実行します。既存docsでも `RuntimeStepOutput` は命令を実行せず、`Command` / `EffectRequest` / `TaskSpec` として返す、とされています。

```text
Arcweft Core
  ↓ RuntimeStepOutput
Host Backend
  ├─ Native wgpu
  ├─ Native audio
  ├─ Servo UI
  ├─ Browser DOM/WebGPU/WebAudio
  ├─ Headless test backend
  └─ Wasmtime plugin host
```

つまり、Arcweftは「ランタイムをWasmにする」のではなく、**coreをhost-neutralにして、backendごとにhost adapterを差し替える** のが正しいです。

## 最終的なcrate構成案

今のcrate mapをベースに、次のように分けるのがよいです。既存crate mapには `arcweft-lang-ir`, `arcweft-lang-vm`, `arcweft-lang-jit`, `arcweft-wasm`, `arcweft-wasm-wasmtime`, `arcweft-wasm-wasmi` がすでに想定されています。

```text
Language:
  arcweft-lang-syntax
  arcweft-lang-hir
  arcweft-lang-ir
  arcweft-lang-vm
  arcweft-lang-codegen-rust
  arcweft-lang-codegen-wasm
  arcweft-lang-jit-cranelift

Bundle:
  arcweft-bundle
  arcweft-precompile
  arcweft-build-cache

Runtime:
  arcweft-core
  arcweft-runtime
  arcweft-player-native
  arcweft-player-web
  arcweft-player-wasi

Wasm:
  arcweft-wasm-abi
  arcweft-wasm-tools
  arcweft-wasm-wasmtime
  arcweft-wasm-browser
  arcweft-plugin-wasm
```

`arcweft-core` はCranelift/Wasmtimeを知らない。`arcweft-player-native` が `arcweft-lang-jit-cranelift` と `arcweft-wasm-wasmtime` をfeatureで有効化する。`arcweft-player-web` はそれらを持たず、wasm化されたruntimeとJS host adapterだけを持つ。

## CLIも分ける

```bash
# 開発
arcw check
arcw dev --hot-reload
arcw run --vm
arcw jit check --compare-vm

# bundle
arcw build bundle
arcw build bundle --emit graph,bytecode,assets,schemas

# native
arcw build native --embed-bundle
arcw build native --embed-bundle --features native-jit
arcw build native --aot rust
arcw build native --aot rust --strip-debug

# web
arcw build web --mode single-thread
arcw build web --mode threads
arcw build web --mode both

# wasi / wasmtime
arcw build wasi --component
arcw run wasi --wasmtime

# plugin
arcw plugin build --target wasm32-wasip2
arcw plugin validate
arcw plugin wit
```

## 何をやめるべきか

避けるべきなのはこの3つです。

1. **WasmをArcweftの唯一の内部IRにすること**
   hot reload、GraphPatch、LSP、stable ID、Need、dialogue構造が扱いにくくなります。

2. **CraneliftでArcweft全体を最初から直接コンパイルすること**
   Craneliftは強力ですが、runtime semantics、effect、save/load、host capabilityを全部自前で背負うことになります。まずVMとRust codegenを正にした方がよいです。

3. **Wasmtimeをbrowserでもnativeでも共通の主runtimeにしようとすること**
   native plugin hostとしては良いですが、browserは別経路です。web docsもBrowser DOM、canvas/WebGPU/WebGL、WebAudio、DOM UI backendとして分けています。

## 推奨ロードマップ

最短で堅い順番はこれです。

```text
Phase 0:
  lossless CST / HIR / Typed IR
  VM bytecode
  .arftpack bundle

Phase 1:
  native player with embedded bundle
  web player with runtime.wasm + bytecode
  headless test runner

Phase 2:
  Wasmtime plugin host
  WIT ABI
  wasm-tools validation / component tooling

Phase 3:
  Cranelift JIT for pure functions
  VM equivalence check
  JIT cache / CLIF dump / asm dump

Phase 4:
  generated Rust AOT for scripts
  native full compile
  save schema compatibility

Phase 5:
  Arcweft IR -> wasm codegen for selected functions/flows
  browser-side AOT wasm modules
  optional Wasmtime AOT for native plugins
```

最終判断としては、**Arcweftの正本はVM/Typed IR、製品配布はembedded bundle、native高速化はCranelift、完全native化はgenerated Rust、sandboxと外部ActivityはWasmtime/WIT、browserはRust runtime wasm + JS host adapter** です。これが一番、ArcweftのSans I/O設計、Need、hot reload、GraphPatch、Web対応、native product buildを同時に壊さずに進められます。


できます。むしろ、この設計は 「JITコンパイルを備えたインタプリタ」 にかなり向いています。

ただし、最初から「全ArcweftスクリプトをJITで実行するVM」にするのではなく、こうするのが安全です。

Arcweft Bytecode VM / Interpreter
  ├─ flow / dialogue / choice / Need / effect はVMで実行
  ├─ pure function はVMで実行しつつプロファイル
  ├─ hotなpure functionだけCraneliftでJIT
  └─ JIT失敗・未完了・無効時は必ずVMへfallback

つまり、Arcweft VMがbaseline interpreterで、Cranelift JITがaccelerator tier になります。

これは既存のCranelift JIT方針とも一致しています。docsでも「VMが正規実行系」「JITはnative-onlyの最適化backend」「pure / deterministicな関数に限定」と書かれており、flow control、effect発行、asset/audio/shader load、UI操作、wasm callなどはJIT対象外にされています。 ￼

形としてはこう

pub struct ArcweftInterpreter {
    vm: BytecodeVm,
    jit: Option<CraneliftJit>,
    profiles: HotnessTable,
    cache: CompiledFunctionCache,
}
impl ArcweftInterpreter {
    pub fn call_function(&mut self, id: FunctionId, args: &[Value]) -> EvalResult {
        if let Some(compiled) = self.cache.get(id) {
            return compiled.call(args);
        }
        let result = self.vm.call_function(id, args)?;
        self.profiles.record_call(id);
        if self.should_jit(id) {
            self.enqueue_jit(id);
        }
        Ok(result)
    }
}

実際の運用はこうです。

初回:
  VMで実行
何度も呼ばれる:
  hotness counterが上がる
JIT条件を満たす:
  Typed IRをCranelift IRへlowering
  compile
  function pointerをcache
次回以降:
  compiled fnを呼ぶ
JIT pending:
  VMで続行
JIT failed:
  diagnosticを出してVM継続
hot reload:
  semantic hash / type layout hashが変わった関数のJIT cacheを破棄

docsにも、JIT pending中はVMで実行し、JIT ready後にframe boundaryで切り替え、JIT failed時はVM継続 + diagnosticという方針がすでに書かれています。 ￼

「インタプリタ + JIT」として成立する理由

成立します。理由は、Arcweft側にすでに以下の分離があるからです。

構文 / HIR / Typed IR
  ↓
bytecode VM
  ↓
実行

このVMに対して、

Typed IR function
  ↓
Cranelift lowering
  ↓
compiled native function

を横に足せば、典型的な tiered VM になります。

Tier 0: bytecode interpreter
Tier 1: Cranelift JIT for pure functions
Tier 2: optional generated Rust / AOT

なので、Arcweft runtimeはこう呼べます。

Arcweftは、baseline bytecode interpreterを持ち、native環境ではpure subsetをCranelift JITできるtiered interpreter / VM。

ただしJIT対象は絞るべき

最初にJITするべきなのはこれです。

score計算
layout計算
easing
animation sampling
audio envelope
filter/map pipeline
shader parameter precompute
pure helper function

逆に、これはVMのままにするべきです。

flow本体
dialogue進行
choice分岐
Need / await / select
effect発行
asset load
audio再生命令
UI操作
wasm plugin call
save/load

理由は、Arcweftのflowは単なる計算ではなく、継続、待機、effect、save互換性、hot reload、debug bus と密接に結びつくからです。ここまでいきなりJITすると、deopt、continuation mapping、save schema、effect isolationが必要になり、かなり重くなります。

flowもJITできる？

技術的にはできます。けれど、最初にやるべきではありません。

flowをJITするなら、こういう問題が出ます。

現在どのdialogue lineにいるか
choiceの途中でhot reloadされたらどうするか
await pending中のcontinuationをどう復元するか
save dataとcompiled codeの互換性をどう保つか
debuggerでstep実行できるか
GraphPatch後にpcをどう移すか

なので、初期方針はこうがよいです。

flow / continuation:
  VMで実行
flow内から呼ばれるpure function:
  JIT可
将来:
  flowをgenerated Rust state machineへAOT
  さらに必要ならflow subset JIT

ブラウザでは？

ブラウザでは native JITはしない 方針でよいです。docsにも「webではJIT無効。VM / wasm / browser wasm backendを使用」と書かれています。 ￼

ブラウザ版はこうです。

Rust Arcweft runtime → wasm
Arcweft script       → bytecode
実行                 → wasm内のVM interpreter
pure function高速化  → 事前にwasmへAOTするなら可

つまりブラウザでは、

JITつきインタプリタ

ではなく、

wasm化されたインタプリタ
+ 必要なら事前生成wasm関数

にします。

Wasmtimeとはどう関係する？

nativeでは2種類の「JITっぽいもの」が共存できます。

Arcweft script:
  Arcweft VM + Cranelift JIT
Wasm plugin:
  Wasmtimeがwasmを実行

ただし、これは混ぜない方がよいです。

Arcweft本体のスクリプトをWasmtimeで動かすのではなく、Arcweft scriptはArcweft VM/JITで動かす。外部ActivityやpluginだけWasmtimeで動かす。これが分かりやすいです。

Arcweft Runtime
  ├─ Arcweft VM
  ├─ Arcweft Cranelift JIT
  └─ Wasmtime plugin host

最小実装案

最初はこのくらいで十分です。

pub enum FunctionExecutor {
    Vm,
    JitPending,
    JitReady(CompiledFn),
    JitFailed(JitErrorSummary),
}
pub struct FunctionRuntimeEntry {
    pub hash: FunctionHash,
    pub layout_hash: TypeLayoutHash,
    pub purity: PurityInfo,
    pub executor: FunctionExecutor,
    pub calls: u64,
}

実行時はこう。

1. function call
2. JitReadyならcompiled fnを呼ぶ
3. それ以外ならVMで実行
4. calls += 1
5. callsが閾値を超え、pureで、layoutが対応可能ならJIT enqueue

JIT eligibilityはこれ。

pure
deterministic
no host effect
no allocation-heavy path
no string-heavy path
no plugin call
known primitive / POD layout
VM equivalence test可能

おすすめの最終設計

Arcweft Interpreter
  = Bytecode VM
Arcweft JIT
  = VMの一部関数呼び出しをCranelift compiled fnに差し替えるtier
Arcweft AOT
  = release用にgenerated Rust / wasmへ出す別backend
Arcweft Wasm Plugin Host
  = Wasmtimeで外部componentを実行する別系統

なので答えは、できます。
ただし設計名としては、「JITコンパイルを備えたインタプリタ」より「baseline VM + selective JIT tier」 と捉えるのがよいです。

最初に作るべき完成形はこれです。

Arcweft VMは常に正しい。
Cranelift JITはnative-onlyの高速化。
JIT対象はpure function subset。
JITできないものはVMで動く。
hot reload時はJIT cacheを捨てる。
webではJITせずVMかAOT wasm。

この形なら、Arcweftのhot reload、debug、save互換性、Wasm plugin、native完全コンパイルの全部を壊さずに、JITつきインタプリタとして成立します。

はい。今回のArcweftでは、AOTとJITを競合させるのではなく、役割を分けるのがよいです。

結論はこれです。

VM本体:
  native / wasm へAOTコンパイルする
.awftスクリプト:
  基本はArcweft bytecodeへコンパイルし、VMで解釈実行する
native:
  VM + optional Cranelift JIT
web:
  VMをwasm化して実行
  runtime JITは基本なし
  必要ならbuild時AOT wasm helperを使う

推奨方針

対象	native	web/browser	理由
Arcweft VM本体	Rust native binaryへAOT	arcweft_player.wasm へAOT	VMはportable runtimeとして作るのがよい
.awft本体 / flow / dialogue	bytecode VMで解釈実行	bytecode VMで解釈実行	hot reload、save互換、debug、GraphPatchを保ちやすい
pure helper関数	Cranelift JIT	まずはVM、必要ならbuild時wasm AOT	nativeだけJITが自然
release高速化	generated Rust AOT	generated wasm AOT	実行時JITより安定
plugin / Activity	Wasmtime / native plugin	browser wasm instantiate / JS host	本体VMとは別系統

Arcweft docs上でも、native/web buildは分かれており、web成果物として arcweft_player_st.wasm, arcweft_player_mt.wasm, loader.js が想定されています。これは「VM/player本体をwasmへAOTコンパイルし、bundleを実行する」設計と相性がよいです。 ￼

native上のインタプリタはどうするべきか

nativeでは、JIT付きインタプリタ にしてよいです。

Native Arcweft Player
  ├─ Arcweft Bytecode VM
  ├─ .arcpack / bytecode / assets
  ├─ wgpu / audio / storage backend
  ├─ optional Cranelift JIT
  └─ optional Wasmtime plugin host

実行モデルはこうです。

flow / dialogue / choice / Need / await
  → VMで実行
pure function
  → 最初はVMで実行
  → hotになったらCranelift JIT
  → JIT readyならcompiled functionへ差し替え
  → JIT失敗・未完了ならVM fallback

これは既存のCranelift JIT方針と一致しています。docsでは「VMが正規実行系」「Cranelift JITはnative-onlyの最適化backend」「pure / deterministicな関数に限定」とされ、flow control、effect発行、asset/audio/shader load、UI操作、wasm callなどはJIT対象外になっています。 ￼

nativeではこう切るのがよいです。

AOT:
  VM本体
  runtime
  renderer/audio/backend
  release用のgenerated Rust script, 将来的に
JIT:
  pure numeric/dataflow functions
  layout/easing/audio envelope/filter/map fusion
  dev/testではVM同値性チェック必須
Interpreter:
  flow本体
  dialogue
  choice
  Need/await
  save/loadに関わるcontinuation

つまりnativeのおすすめは、

baseline interpreter + selective Cranelift JIT

です。

最初からflow全体をJITしない方がいいです。flowには現在位置、save/load、hot reload、choice、await pending、debug step、GraphPatch mappingが絡むので、VMで持った方が安全です。

web上のインタプリタはどうするべきか

webでは、JIT付きインタプリタではなく、wasm化されたインタプリタ を基本にするべきです。

Browser
  ├─ loader.js
  ├─ WebGPU / WebGL
  ├─ WebAudio
  ├─ DOM / input / storage
  └─ arcweft_player.wasm
       ├─ Arcweft VM
       ├─ scheduler
       ├─ Need runtime
       └─ bytecode interpreter

webではこうです。

AOT:
  Arcweft VM本体 → wasm
  runtime/player → wasm
  必要ならpure helper → generated_helpers.wasm
Interpreter:
  .awft bytecodeをVMで実行
JIT:
  基本なし

ブラウザ上でも、理屈としては wasm-encoder でWasm bytesを生成して WebAssembly.compile/instantiate に渡す「dynamic wasm generation」はできます。wasm-encoder はRustでWebAssembly binaryを生成するcrateで、READMEでも Module を構築して finish() でWasm bytesを得る例が示されています。 ￼

ただしArcweftでは、web runtime JITは後回しがよいです。理由は、非同期compile、CSP、JS bridge、memory共有、module生成単位、Safari/iOS差などがあり、ゲームランタイムの安定性を落としやすいからです。

webで高速化したいなら、まずはこれです。

arcw build web
  ├─ arcweft_player.wasm
  ├─ game.arcpack
  ├─ generated_helpers.wasm
  └─ loader.js

つまり、webではJITよりAOT wasm helper が先です。

AOTが向いているもの

ArcweftでAOT向きなのは、安定していて、targetごとに事前生成できるものです。

VM本体
runtime/player
type layout
bytecode bundle
asset graph
shader reflection
save schema
release用flow state machine
pure helper functions

特にVM本体は必ずAOTです。

native:
  arcweft-lang-vm → native Rust binaryにリンク
web:
  arcweft-lang-vm → wasm32 targetでplayer.wasmへ入る

releaseでさらに高速化したい場合は、

Typed IR → generated Rust → cargo build

をnative AOTの本命にするのがよいです。Craneliftでflow全体を直接AOTするより、Rust codegenの方が安全で、debug info、platform対応、panic、link、最適化をRust toolchainに任せられます。

JITが向いているもの

JIT向きなのは、実行時に「hotかどうか」が分かる、かつ副作用がなく、失敗してもVM fallbackできるものです。

easing
layout expression
animation sampling
audio envelope
numeric scoring
filter/map pipeline
shader parameter precompute
pure helper function

JITに向かないものはこれです。

flow本体
dialogue進行
choice
Need / await / select
effect発行
asset load
audio再生命令
UI操作
save/load
plugin call

ここをJITすると、deopt、continuation復元、save互換、hot reload mappingが必要になります。Arcweft初期実装では重すぎます。

具体的な実行戦略

Dev native

.awft
  → CST / HIR / Typed IR
  → bytecode
  → VM実行
  → hot pure functionだけJIT
arcw dev
arcw dev --hot-reload
arcw run --vm
arcw run --vm --native-jit
arcw jit check --compare-vm

これは一番開発しやすいです。VMが正なので、JITを切っても同じ挙動になります。

Product native: 標準

native player
  + embedded .arcpack
  + bytecode VM
  + optional Cranelift JIT
arcw build native --embed-bundle
arcw build native --embed-bundle --native-jit

最初の製品版はこれがよいです。完全AOTではないですが、ユーザーから見ると単一nativeアプリです。

Product native: 完全AOT

.awft
  → Typed IR
  → generated Rust state machines
  → native binary
arcw build native --aot rust

これは後で入れるべきです。特に商用release、起動速度、難読化、配布サイズ、VM bytecodeを出したくない場合に向いています。

Web standard

arcweft_player.wasm
  + game.arcpack
  + assets
  + loader.js
arcw build web --mode single-thread
arcw build web --mode threads
arcw build web --mode both

既存docsにもweb build modeとして single-thread / threads / both が想定されています。 ￼

webではまずこれで十分です。

Web optimized

arcweft_player.wasm
  + game.arcpack
  + generated_helpers.wasm
arcw build web --aot helpers

ここで generated_helpers.wasm にはpure functionやlayout/easing系だけを入れます。flow本体はVMのままでよいです。

Web runtime dynamic compile

VMで実行
  → hot function検出
  → wasm-encoderでwasm生成
  → JSへ渡す
  → WebAssembly.instantiate

これは研究・advanced optionです。初期実装では不要です。

最終おすすめ

Arcweftでは、こう決めるのがよいです。

1. VM本体はnative / wasmへAOTコンパイルする。
2. .awftはまずArcweft bytecodeへコンパイルする。
3. native上のインタプリタは、bytecode VM + selective Cranelift JITにする。
4. web上のインタプリタは、wasm化されたbytecode VMにする。
5. web高速化はruntime JITではなく、build時AOT wasm helperから始める。
6. native完全コンパイルはgenerated Rust AOTを後で追加する。

一言でいうと、

native:
  AOT player + bytecode interpreter + optional JIT
web:
  AOT wasm player + bytecode interpreter + optional AOT wasm helpers
release ultimate:
  generated Rust / generated wasm AOT

です。

この設計なら、Arcweftのhot reload、debug、save互換、GraphPatch、native JIT、web配布、完全AOTの全部を同じTyped IRの上に載せられます。