# Crate and workspace boundaries

The current workspace member list is `[workspace].members` in [Cargo.toml](../../Cargo.toml); member manifests under `crates/` define package names. This chapter records cross-crate ownership and dependency contracts, not a duplicate workspace inventory. A target boundary without a current package name does not imply that a workspace crate already exists.

## 依存ルール

- `arcweft-manifest-model` は source/HIR/backend に依存しない共通 identity と
  canonical encoding を所有する。標準 callable provider の `StandardEnvironmentId`
  もここで一つの型と provider code を定義し、Sema catalog と runtime linkage が共有する。
- `arcweft-id::presentation` は presentation occurrence の `PresentationHandleId`
  と canonical text の検証を所有する。driver、retained View、snapshot、描画 adapter
  は同じ型を使い、driver 経由の identity 定義や公開 alias を持たない。View・asset・
  image・target・layer の public ID と、実行時 occurrence の identity を区別する。
- `arcweft-core` は runtime/data core に限定し、dialogue / presentation / syntax / verifier / CLI / LSP / wgpu / audio / Servo / DOM / filesystem / network / Cranelift / Wasmtime に依存しない。
- `arcweft-interaction-model::presentation` は image の登録データ、fit、検証済み alignment、
  playback、fixed-point transformと `PresentationResourceKind` の共有domainを所有する。
  登録データは `arcweft-layout` の Sans I/O placement 値を直接保持する。
  resource familyはSemaの型constructorとdriverの保持recordが直接共有する。
  Core の実行契約と
  bundle、presentation、retained View、renderer は同じ値型を使い、各層での
  再定義・変換・互換 alias を持たない。実行 command とその評価は Core、
  retained state と描画処理はそれぞれの上位 owner に置く。
- `arcweft-core::awbc` は AWBC executable-table の Sans I/O data/verifier
  境界を所有する。product encoding、filesystem、network、signing、runtime
  lowering、VM 実行、JIT/AOT backend、patch materialization は所有しない。
- 広い application prelude は facade crate `arcweft` が提供する。低レイヤ crate に便利 re-export を置かない。
- `arcweft-core::presentation` は実行中の presentation handle の発行、元programに
  結び付くledger、bodyのcustody、scope exitのcleanupを所有する。
  `arcweft-presentation` は表示側のtyped target/slot address、layer、hit/focus等の
  Sans I/O data/modelを所有し、Coreの実行grantを再発行しない。renderer、
  filesystem、asset loading、windowing、clockには依存しない。
- Data-format crate は Sans I/O を保つ。manifest、schema、bytecode、bundle、save snapshot は構造体と bytes/string codec までを担当し、path read/write、network、clock、backend resource 確保は CLI / build / player adapter に置く。
- `arcweft-data` は format-neutral な `Value`、type shape、Encode / Decode、decode limits、codec registry を所有する builtin data contract とする。`arcweft-codec-json`、`arcweft-codec-toml`、`arcweft-codec-yaml`、`arcweft-codec-msgpack`、`arcweft-codec-cbor`、`arcweft-codec-csv`、`arcweft-codec-arrow`、`arcweft-codec-avro`、`arcweft-codec-binary` は concrete external format adapter crate として分離し、`arcweft-save`、`arcweft-config`、`arcweft-http-codec` はこの registry 境界を使う。
  typed JSON save の構築前検証は `arcweft-save -> arcweft-codec-json -> arcweft-data` の同じ parser／DecodeBudget を使う。型付き decode は元の payload bytes を保持する。
- `arcweft-data-derive-support` は Rust data 属性の構文と検証を所有する
  macro 内部用 crate とし、`arcweft-data-derive` と
  `arcweft-rust-abi-macros` が共有する。依存は `syn` / `quote` /
  `proc-macro2` に限定し、runtime graph や登録済み callable を所有しない。
  `arcweft-rust-abi` → `arcweft-data` は実在する `Bytes` 型の ABI 型登録に
  用いる。
- `arcweft-core::entry` は data の `TypeShape` から `RuntimeTypeSchema` への
  context-free な変換を所有し、sema と runtime adapter は同じ `From` 実装を使う。
  依存方向は core → data とする。この変換は reflection metadata を保持し、
  nominal identity の発行や未解決の参照を含む graph の admission は行わない。
- `arcweft-core::effect_row` は有限 effect set、canonical membership DAG、
  その predicate と制御付き置換・量化の共通代数を所有する。意味解析と実行時型の
  検証は、各層の型付き参照をこの同じ代数へ渡す。宣言・application scope、
  推論変数の eligibility、候補制約、未確定の annotation は sema に残し、
  代数自体は HIR、solver transaction、I/O に依存しない。
- `arcweft-bundle` は bundle data model と deterministic codec entrypoints を所有する。JSON は `.awfb` 互換の default codec、TOML/YAML/MessagePack/CBOR/Avro は explicit alternate artifact format とする。Avro bundle artifact は stable JSON payload を Avro Object Container envelope に包む。
  Avro の container framing、metadata 順序、content-derived sync marker と
  借用 datum の書き込みは `arcweft-codec-avro::container` が所有する。
  bundle と runtime accelerator はこの Sans I/O owner に依存し、それぞれの
  envelope field grammar を渡す。独立した第三者 Writer は構築しない。
- `arcweft-bundle-assets` は generation ごとの exact bundle identity に束縛された image / voice catalog、実バイトの decode・metadata validation、typed runtime handle/error の生成と保存値の再検証を所有する portable runtime boundary とする。bundle の artifact model / codec は `arcweft-bundle`、host I/O と backend resource 化は native / web adapter に残す。
- `arcweft-bundle::resource_codec` は product resource section の共通
  compact codec contract を所有する。section magic/schema、decode budgets、
  string/public-id table、既存 AWFB section kind との対応、patch
  compatibility class までを扱い、runtime capability availability、
  renderer/backend limits、filesystem/network fetch、signing key access は扱わない。
- `arcweft-project::persistent_object` は compiler-private `.awbo` object
  envelope/key/payload contract を所有する。cache record filesystem layout、
  object storage、lock、watch policy、CLI explain output は
  `arcweft-project-loader` / CLI adapter 側に置く。
- View / shader / audio / Activity は `Command` / `TaskSpec` / `Need` / `EffectRequest` を介する。
- Hook は phase ごとの構造化 action を返し、直接 host API を呼ばない。
- Memoization は pure computation または TaskKey deduplication に限定し、cache は決定性に影響してはならない。
- Rust `unsafe` は named boundary に閉じ込め、安全性の invariant を文書化する。現在の workspace では Windows TSF FFI は [desktop-native](../../crates/arcweft-desktop-native/src/text_input/windows_tsf/unsafe_com.rs)、native JIT 呼び出しは [lang-jit-cranelift](../../crates/arcweft-lang-jit-cranelift/src/native_call.rs) が所有する。
- `arcweft-agent-protocol` は CLI / MCP / test / LLM が共通利用する。
- `arcweft-agent-mcp` は `arcweft-agent-protocol` の observation/resource を
  MCP `resources/read` / tool result 互換の Sans I/O JSON shape へ変換する。
  stdio、HTTP、auth、session lifecycle、renderer readback は持たない。
- `arcweft-lang-syntax` は rowan-compatible な lossless CST と surface parser を所有する。`SyntaxKind`、`TokenKind`、green tree、`SyntaxNode`、source text / line index、error-tolerant `ParsedSource`、surface AST、expression/type/pattern parsing、syntax lint をここに集約する。HIR lowering、semantic checks、runtime-plan lowering は持たない。
- `arcweft-lang-hir` は qualified arena ID、transactional `HirDatabase`、
  attached `ParsedSource` からの final lowering、module-preserving
  `HirProject` を所有する。semantic passes、verifier、compiler、CLI、LSP は
  accepted project/viewを入力境界にし、detached loweringやflattened linked HIRを
  構築しない。
- `arcweft-lang-hir` は syntax/CST 由来の typed ID context も所有する。
  `arcweft-tooling`、CLI、LSP は dialogue ID や choice ID を独自 scan
  せず、この context から edit / hint / code action を作る。
- `arcweft-lang-sema` は name registry、symbol use collection、reference validation、typecheck readiness、minimal type checking を所有する。
- `arcweft-compiler` は source text から parse / HIR / typecheck /
  runtime-plan lowering / line display catalog までを束ねる Sans I/O driver
  API を所有する。CLI の profile / diagnostics / filesystem selection は
  CLI 側に残し、player host は source developer mode でこの driver を使う。
  完成した semantic generation は `ProjectAnalysisLease` が exact HIR /
  source / symbol ancestor、registered world、final report、semantic index を
  一体で保持する。`CompiledProject` はこの lease と検証・実行成果物を所有し、
  `ProjectCompilationLease` は完成済みの HIR / analysis / compiled phase を表す。
  LSP の accepted snapshot と compiler error はこの phase lease を保持するため、
  完成した解析は後続の実行 admission が失敗しても利用できる。
- `arcweft-runtime-plan` は checked HIR から `arcweft-core` の `RuntimePlan` / line task graph へ lowering する。
- `arcweft-runtime-codegen` は full-script AOT/JIT の executor policy、
  safe-region runtime-code IR、frame layout、cache key、structured compiled
  step exit contract を所有する。Cranelift lowering、executable memory、
  object loading、Wasm AOT backend、host I/O は adapter crate 側に置く。
- `arcweft-verify` は Sans I/O の検証中核で、proof obligation、audit manifest、SMT problem、tool diagnostics schema を所有する。ファイルI/O、process起動、watch、editor transport は持たない。
- `arcweft-verify-z3` は外部 Z3 process adapter、`arcweft-verify-oxiz` は pure Rust OxiZ adapter とする。solver依存は `arcweft-verify` や `arcweft-core` に入れない。
- `arcweft-verify-lsp` は transportなしの LSP helper crate とし、
  `arcweft-verify` report から diagnostics / code actions を作る。
  Source-aware position conversion is exposed through a mapper trait; stdio,
  client state, file watching, and request dispatch stay outside this crate.
- `arcweft-lsp` は `lsp-server` based stdio transport crate とし、
  initialize/shutdown、client capability negotiation、FULL text sync document
  cache、publish diagnostics、request dispatch、workspace command routing を
  所有する。Rust metadata generation and profile/adapter file I/O remain
  build/CLI/adapter responsibilities and are read as resolved metadata by the
  transport.
- `arcweft-test` は `test` / `bench` 宣言を HIR から Sans I/O manifest に変換する。ファイルI/O、clock、renderer/audio driving、benchmark timers、headless player 実行は CLI / player / adapter crate に置く。
- `arcweft-text-layout` は Sans I/O の文字組版を所有する。shaping/layout data、
  論理軸、縦書き、ruby、text-combine、改行・列分割、hit/capture geometry を
  renderer に依存せず決定する。glyphon、WGPU、filesystem を依存先に持たない。
- `arcweft-glyphon` は共通 layout を glyphon `GlyphArea` へ変換する adapter。
  glyphon/WGPU の型、atlas/cache の寿命、描画座標への調整をこの境界より上に置く。
  renderer が文字組版や列分割を再計算する第二の authority は作らない。
- `arcweft-render-wgpu` は native/Web が共有する `SharedRenderer`、View
  compositor、prepared text submission、Color/ObjectId/Mask attachment と
  offscreen readback を所有する。Text layout、Fx semantics、Agent object
  identity は所有しない。`arcweft-cli` の Agent observe も通常 player と
  同じ `PreparedFrame` をこの shared capture API へ渡す。
- `arcweft-layout` は Sans I/O の presentation-layer crate とし、design
  viewport、output viewport、raw/contain/cover/stretch fit transform、inverse
  mapping、layout unit expression、safe-area evaluation context、text overflow
  policy、text fitting result/diagnostic data contractsを所有する。WGPU、
  glyphon、filesystem、player/CLI adapterには依存しない。
- `arcweft-player-native` は native product/player host であり、bundle /
  bytecode execution、scheduler/input/audio/window lifecycle、shared WGPU
  surface/capture orchestration を所有する。source direct execution は developer mode であり、
  `dev-source` feature 配下に閉じ込める。product player の正本は `.awfb` /
  bytecode bundle input とする。
- View は最初から細かい public crate family に分割せず、当面は
  `arcweft-view` が View registry、generational Entity、retained
  fragment、reactivity、style/layout integration、semantic View nodes を所有する。
  checked / retained Match の共通定義 grammar と canonical transcript writer は
  `arcweft-core::pattern` が所有し、View は Core の inert source data を保持する。
  checked admission の発行権限は sema、product の source/revision 検証は bundle に残す。
  Raw input routing、LayerTree、HitTree、focus、modal、pointer capture は
  `arcweft-presentation` 側の Sans I/O data/model とする。
- `arcweft-launch` は `arcw.toml` launch profiles を typed data と TOML codec
  として所有する Sans I/O crate とする。ファイル探索、current directory、
  process 環境、network binding、adapter execution は CLI / player adapter 側の
  責務。
- full-document parse の public authority は、one-session
  `SyntaxDatabase::parse_initial` / `reparse` が返す revision-bound
  `incremental::ParsedSource` とする。`ParsedSource` は exact document lease、
  lossless CST、attached typed handles、diagnostics、line index を一体で所有し、
  compiler、project-loader、LSP はその同じ snapshot を借用する。detached
  `TypedSyntaxTree`、whole-document parse facade、raw textからのdocument identity
  捏造、range/source検索によるnode再発見は置かない。unbound expression/type/
  pattern/statement fragmentsは、bound document APIと取り違えられない明示的な
  fragment ownerを通す。delimiter recovery、top-level punctuation、keyword /
  binding split、multi-token punctuation sequenceなどの構文走査はlossless
  grammar/CST helperへ集約し、`split_top_level`型のad hoc parserを拡張しない。
- Cranelift は `arcweft-lang-jit-cranelift` の native-only 最適化 backend に閉じ込める。`arcweft-core` に `jit-cranelift` feature や Cranelift 依存を置かない。
- Wasmtime plugin/activity sandbox は optional native adapter の target で、Arcweft runtime の主実行系ではない。WIT ABI と Wasm validation/generation/inspection も adapter/tooling 境界に置き、これらを Core の依存先に置かない。現在の workspace はこれらの target に package 名を割り当てていない。

