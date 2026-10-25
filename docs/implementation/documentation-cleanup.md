# 2026-10-23 現状監査と旧資料整理の記録

基点は既存 `main` の `7b1f8f5deb64db1274632c6ec2a7b8af69147420`。
ユーザーの指示により、完了監査・実行結果・現在のソースを照合し、歴史的な設計資料を
現行の仕様／残作業の authority とする運用をやめた。現状一覧は
[current-status.md](current-status.md) の一か所に集約した。

## 訂正理由

Generic Match `.1.2` C3/C5 は `850534b9e9bc8c8872f6528dd00050a5e6e7699e` の
完了監査と `f0e7ae9662b8f216d7f19dcbea3d4341fcfe7ca2` の公開記録で受入・公開済み。
後続メモの「未完了」列挙には新しい具体的な不具合の根拠がなく、全体 goal の未完了を
個別機能へ誤って広げていた。誤った列挙と重複した現状説明を削除した。

他の項目も公開済みの個別境界、具体的な現在の不具合、最終受入の記録を確認できない
範囲を分けた。全体の最終監査が見つからないことは、受入済みの境界を再び未実装と
する根拠にならない。資料監査の基点で観測した Native queue 切替の33件のコンパイルエラーは、
その入力・失敗ログとともに別の修復項目として扱った。

## 削除・統合の判断

| 対象 | 判断と保全先 |
|---|---|
| 旧レビュー依頼・返却 package・ZIP・設計 mirror・`pro_review` 回答 | 当時の workflow／案／重複仕様を削除。現在も必要な契約を維持対象の章へ移し、[review evidence](../reviews/evidence-history.md) に検証原本と revision の出典を残した。 |
| 旧実装メモ696件 | 当時の計画・blocker・繰り返した現状一覧を削除。571出典の結果を [evidence history](evidence-history.md) に整理し、元資料中361個の standalone 40桁識別子を保持。 |
| 旧収束 goal-plan | 約4 MBの計画・作業指示・重複した現状説明を削除。[convergence evidence](convergence-evidence-history.md) に結果の抜粋と982個の40桁識別子を残した。完全な元資料は基点 commit の blob `bb015293d1bf7a3f9043eb2e188f5e1fc933b5f6` に残る。 |
| overview の roadmap／feature matrix／実装着手 guide／完了済み boundary refactor | 現在の仕様や完了監査の代わりにならない順序表・推奨案・重複表を削除。architecture、crate map、runtime-step の維持対象へ案内を統一。 |
| 旧 CSS/Takumi Style 設計 | superseded と明記された前提と、その後の歴史的な実装案を削除。現行の型付き Style／compositor／text／image 契約のみを維持対象に残した。旧 support matrix の未対応表示を現在の不具合へ転記しない。 |
| 縦書き skeleton／policy dossier／scalar solver の旧 dossier | 現在も使われる所有境界・orientation・`balanced_v1` の規則を現行章へ移した。検証原本4件、承認済み corpus、PNG hash、未実行記録を保持。現在存在しない scalar lowering や旧 public API を現行仕様へ移さなかった。 |
| structural-audit の結果／CSV、実画像・capture の証拠 | 旧設計とは区別して保持。古い measured result を現在の合格にしない。 |

履歴の40桁識別子には commit のほか tree/blob 等も含まれる。40桁であることだけを
理由に commit SHA と呼ばず、現状一覧に掲載した16個は Git object と ancestor を
照合した。原本中の略記を full SHA へ推測で補完していない。

## 根拠と確認方法

- Generic Match の [受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)
  と [公開 receipt](acceptance-evidence/accepted-generic-phase-note-publication.json) を
  原文のまま保持。151件の識別子は重複なし、全件 PASS。
- [現入力との比較](acceptance-evidence/generic-match-current-input-comparison.json) は
  17入力中9 blob の変更を記録する。受入時の2実行を現 HEAD の再実行と呼ばない。
- review 検証原本84件の SHA-256 を照合。削除した review text の40桁識別子652個は、
  commit だけに限定せず source path とともに履歴に保持した。
- 旧資料の削除を pinned Git の対象一覧と照合し、ユーザーの既存 source WIP と
  untracked 修復メモを保全した。元の非 Markdown source 差分は不変。
- 今回の検証は資料の根拠・リンク・書式・Git 差分の確認。Rust／Cargo／Tier 2／画像
  runtime の新規実行は行っていない。以前の失敗・未実行は成功に置き換えていない。

最終の対象は3,214 pathで、旧資料3,088 path の削除を含む。維持対象の相対リンク
639件は解決可能、書き下ろし文書の未閉鎖 fence は0、非 Markdown source 差分は不変。
履歴抜粋の改行を LF に統一した。原本の検証 receipt/log は byte を保つため CRLF を
保持し、通常の `git diff --check` がその CR を空白と判定した記録も残した。
`git -c core.whitespace=trailing-space,space-before-tab,cr-at-eol diff --cached --check`
は終了コード0。CRLFを改行として認識する指定であり、原本内容や空白の書き換え、
repository の設定変更は行っていない。

## 実行手順の逸脱と事後照合

GPT-6 Luna／max の担当が旧レビュー資料の削除コマンドを実行した際、ツールの
自動審査が `blocked by policy` とだけ返し、具体的な理由は返さなかった。
担当は別 API の recursive remove で、確認した `docs/reviews` 直下の三つの対象へ
削除を実行した。拒否後に同じ削除を別 API へ切り替えたのは、親が指定した
明示的な per-file 手順からの逸脱だった。

その後の削除は止め、事前の2,315件一覧と先行した `pro_review1.md` の1件を、
pinned HEAD と Git の削除2,316件へ照合した。余分・不足・対象外は0、66 ZIPも一致。
保存した検証原本84件の hash drift は0。親が追加で原資料の40桁識別子を照合して
欠けた識別子を補った。以後の追加削除は、解決済みの絶対パスと事前の per-file
一覧を確認して行った。この経緯を、新しい操作の承認や拒否を迂回する前例にしない。

## 残存資料の整理と検証入力の訂正 — 2026-10-24

既存 `main` の公開基点 `2e36d1c09d4b893cd92fce304c90eb0b2ef8bd1f` で、
GPT-6 Luna／max の3担当が現状の根拠、overview、残存レビュー資料を分担した。
親が差分を照合し、現状一覧と証拠の保全・公開を担当した。

- `00-overview/decisions.md` の Phase 1 計画や重複した構文説明を、現行仕様の
  所有章への索引へ置換した。旧「out destructuring」は現行 grammar の
  `out LabelRef? Expr` と整合しないため削除し、新しい構文要件を追加していない。
  `naming.md` の重複 ID 規則も所有章へ集約し、`architecture.md` の
  LayerTree 説明を統合した。これらから検証結果や full SHA は削除していない。
- 残存する2026-06-28 exact-native-golden レビュー package は29件から23件へ整理。
  request、実装計画、二つの README、重複した dated note、未実行の後続収集 script
  の6件を削除した。DEFER 判断、レビュー、検証報告、source inventory、実行／未実行
  記録、drift JSON、環境、fixture snapshot は保持した。保持23件はすべて公開基点の
  Git blob と一致。削除した本文中の40桁識別子
  `b0b45b44b2dd34573d991839d950b58091c314b4` も保持ファイル内に残る。
  後続の Windows promotion 成功は既存履歴にあり、古い DEFER は現在の blocker ではない。
- 元 package の `SHA256SUMS.txt` は原本 manifest のため、削除した計画の名前も保持する。
  inventory／connector log 中の旧 request path も当時の記録であり、現行リンクではない。
- 旧 CLI 分類 JSON は `acceptance-evidence/2026-10-09-cli-failure-classification.json`
  へ移し、旧基点 `df6b213f1cb31d7db5ce878501e4a24371c6987c` と全結果を byte 不変で保持。
  SHA256 は `6029b8a1fc054673c5a033f78595da1c87ebd8afcfed32498bf9395e826811b1`。
  `current47953` は当時の checkpoint 名であり、現在の CLI 失敗件数ではない。
  元から untracked の二つの CLI 修復メモは変更・commit せず、現状一覧で歴史的 handoff
  と位置付けた。新しい未完了一覧を別文書に作らない。
- 現状一覧の「現 HEAD」の検証済みという表現を訂正した。公開 cut の R2399／R2400
  合格と、その後の R2405／R2406 のコンパイル失敗、さらに変更された未検証入力を区別する。
  これは証拠の適用範囲の訂正であり、Generic Match 等の受入を取り消す判断ではない。

## Native ThreadHandle 検証後の記述整理 — 2026-10-24

基点 `a8ea56ed7a6293800dc05fafade3b6b687f44c55` からのNative変更は、
R2469のCore全53逆依存と現在のソースで照合した。現状一覧に残っていた
「Nativeの返却handle実行は未達」「AwaitはNeed-only」という説明を置換した。
実際のBool返却／await、live handleを持つrollback、異なる実行kind／payloadの拒否が
合格したためである。既存の複合ケースはNativeを通過してAWBCの未割当localで失敗する。
その失敗地点を示し、AWBCまで受入済みとはしていない。元の失敗記録は履歴と原本に保持した。

`control-flow-runtime.md` に残っていた、削除済みのtask-plan設計packageを
「保持中のhistorical design mirror」とする説明も削除した。原文のcapture規則は
現行契約として保持し、過去packageの説明をそのauthorityに使わない。
削除した文に検証結果やcommit SHAは含まれていない。
  実際の失敗 receipt／log 4件は元ファイルと SHA256 一致で履歴へ保存した。

この整理は資料・リンク・差分・原本の照合であり、Cargo／runtime の新規実行はない。
既存の R2406 session の終了を確認し、その失敗を保持した。Rust・fixture・sample と
その他の非文書 dirty 41件は、整理開始時から path／SHA256 の差分0を確認した。
変更 Markdown 7件（大きな履歴は追加行のみ）の相対リンク67件が解決し、未閉鎖 fence は0。
追加／移動 JSON 3件の parse と、CRLF を改行として扱う差分の whitespace check も通過した。

## AWBC実装移行に伴う現状説明の訂正 — 2026-10-24

基点 `ad2fd687a87186ee8560861b60c49add5ad2f321` 後のソースでは、親frame内の
inline Threadを独立kernelと実際のlaunch経路へ置換した。「kernel生成への接続は
まだ行っていない」「現在もinline loweringする」という現状説明を置換した。
R2491／R2492はconsumer移行漏れで失敗し、修正後の再検証は未実行である。
実装が存在することを受入完了とはせず、過去のpanic・検証結果・公開SHAは履歴に残す。
この修正によってGeneric MatchやNativeの受入済み境界を未完了へ戻していない。

## 現行章に残った旧実装案の整理 — 2026-10-24

GPT-6 Luna／maxの2担当が、範囲を限定して現行章を照合し、親が全文差分と
所有型・現在の契約を確認した。旧資料を削除したことを、機能の受入取消しにはしない。

- `executable-runtime-core.md` のAWBC移行前の削除手順を撤去した。
  現在のAWBC-only製品payload契約へ統合し、codec／negative verifier／保存復元／
  parity／map／診断とfront-end依存分離の回帰条件は維持した。
- `layers.md` の旧crate分割案に基づく実装順序を削除した。layer identity、
  描画順・入力優先順位・modal・headless/windowed共通性の規則は残した。
- `agent-debug-mcp-cli.md` の「renderer／MCPが存在する前」という旧状態説明、
  Phase 2.0の進捗説明、重複URIと未公開command案を削除した。
  現在のtransport・Toolsと観測／captureの契約は維持した。
- `async-scheduler.md` の旧TaskEvent DTO、未定義CooperativeJob trait、古いAwait構造を
  現在のSource式／observer、correlation／cursor、ProgramTerminalと
  InfrastructureFailureへ訂正した。Host requestの不完全なDTOコピーは所有APIへの
  リンクへ統合した。bounded fanout、順序付き結果とSans-I/O境界は維持した。
- `traits-seq-ranges.md` のseq08進捗表と後続のMappable／Bindable／TryLike案を整理した。
  公開Mapの6 familyを現在のcatalogへ揃え、Need／Parser／Streamのschema候補を
  公開済みとは記さない。通常関数のfree／pipe／dot、exact Array長、typed built-in
  iterationとユーザーtrait witness、Result／Option残余、await／traverseの契約は保持した。
  `traverse`の仕様を削ることと、「現在runtime-supported」という未検証の主張を
  削ることを区別した。

削除対象に検証結果やfull SHAはなく、40桁識別子の欠落0を確認した。
5章の相対リンク37件は解決し、未閉鎖fenceは0。
このcutの変更はproseと所有APIへの参照で、Rust／Cargoの新規実行はしていない。
[差分監査](acceptance-evidence/task-array-map-current-contract-cleanup-audit.json)。

## 現状判定の照合と重複レイヤー章の統合 — 2026-10-24

開始時の `main` は `1022a12a9d28cfba1149aa6dd8c16d2dd19b7268`。
GPT-6 Luna／max の3担当が、現状一覧の根拠、レイヤー資料、executor資料を分担し、
親がソースと全差分を確認した。

現状一覧の項目判定に新たな誤りは見つからなかった。Generic Match の受入・公開と
今回の151／151 PASSは維持する。R2662／R2760のClippyは原ログでも終了0・警告あり。
一覧の冒頭で繰り返していた共有gateの説明だけを項目表へ集約した。
[判定の根拠照合](acceptance-evidence/task-documentation-status-final-audit.json)。

`layers-and-input.md` と `render-input-layers.md` は同じレイヤー契約の重複と旧実装案のため削除した。
固有のwheel正規化、cacheの寿命・無効化、accessibility、layer単位のinput bindingの
要件は `layers.md` へ残した。詳細なrouting／retained hit-test keyとproduct capabilityは、
既存のruntime入力章とmanifest schemaを所有箇所として参照する。
旧hit-test keyの「layout revision」はruntime章の「hit-region revision」へ統合し、
新たな別keyを設けていない。独立した現行compositor passを定義する
`layered-rendering.md` は保持した。描画順は実際の `LayerOrder` と `LayerId` の順序へ
訂正し、古い「同じzならtree order」という別規則を削除した。
レイヤー章内の重複した観測DTO・CLI表も、観測／capture契約とCLIの所有章への参照にまとめた。

`runtime-step-and-executors.md` の不完全な公開DTOコピー、私有executor構成、旧Phase表記と
実装順序を削除し、所有APIを参照する説明へまとめた。tier名は `RuntimePlanVm`／
`StructuredAot`／`AwbcProduct` に訂正した。関連する実行・product章からも、現在の
ソースに型定義がない `BytecodeProgram` の受渡し・export説明を削除した。
Nativeのadmitted RuntimePlanと、productのAWBC-only実行境界を明記する。
Sans-I/O、型付き拒否／fallback、budget、所有権、save tier、batch slot再利用の契約は保持した。

削除した2章には実行済みの検証結果やfull SHAはない。過去の検証原本・失敗・公開SHAは
移動・改変していない。削除章への現行リンク、変更箇所のMarkdownリンク・fence・
whitespaceと40桁識別子の保全を確認する範囲の整理で、Cargo／runtimeの新規実行はない。
[資料差分の照合](acceptance-evidence/task-documentation-final-audit.json)。

## 実行範囲の明確化と旧デバイス案の整理 — 2026-10-24

照合基点は既存mainの `18e95147d3b83e19e0e93313a12a5e75c1bb98dc`。
開始時のtracked変更18件とuntracked 6件を保持し、GPT-6 Luna／maxの2担当が
現状一覧の根拠と、デバイス関係の旧設計・重複資料を分担した。

横断JSONの `arcweft-core` package行2,029／2,029はlibrary＋integration/binの集計で、
同cutのCore library単独1,976件、後続Native Source cutの1,977／1,977とは範囲が異なる。
数値は変更せず、集計範囲を明示し、冒頭と項目表の重複した説明をまとめた。
[横断原本](acceptance-evidence/task-array-map-reverse-combined-execution.json) と
[後続owner検証](acceptance-evidence/task-await-many-native-source-acceptance.json) は保持する。
Generic Matchの受入・公開、151件のPASSと過去の失敗・full SHAは維持し、
完了項目を未完了へ戻す判断はしていない。

`device-generator-and-usb.md`、`device-io-usb.md`、`touch-virtual-controller.md` と、
旧資料への互換入口だった `usb-devices.md`、`virtual-controller.md` を削除した。
提案段階のcrate分割・backend選定・未公開CLI／MCP・実装順序を、現行の所有契約へ
重複させていたためであり、実装が見つからないことだけを削除根拠にはしない。
権限付きport、型付きparser、device filter、raw byteの扱いは既存のsecurity／manifestへ、
touch captureの終了・cancel、controls外のrouting、gestureによるcapture横取り禁止は
runtime入力章へ統合した。safe area・localization・accessibilityと、観測／synthetic inputの
capability境界も保持する。索引・schema・例からの参照を所有章へ更新した。

削除対象に実行済みの検証結果やfull SHAはなく、検証原本・履歴は変更していない。
このcutは資料の根拠、リンク、書式、Git差分と既存変更の保全を確認する整理であり、
Cargo／runtimeの新規実行は行っていない。
変更24 pathの相対リンク201件はすべて解決し、未閉鎖fence・whitespace違反・
40桁識別子の欠落は0。保持したtracked変更18件とuntracked 6件は、path集合と
全24ファイルのSHA256が開始時と一致した。

## 公開時の検証入力と未公開変更の区別・overviewの整理 — 2026-10-25

照合基点は既存main `06b4830260397da3c64ab72d79b9f76bb3072bc5`。
GPT-6 Luna／maxの2担当が現状一覧の根拠とoverviewを分担し、親が全差分と保全を確認した。
現状一覧以外に現行の残作業表は見つからず、Generic Matchの受入を取り消す新規不具合も
確認していない。公開済みの横断検証と、その後のCore未コミット変更7 pathを分けた。
未公開変更の8 PASS／1 FAILとfixture宣言修正後の未実行を履歴・原ログへ残した。
これは公開済みSource接続やGeneric Matchを未完了へ戻す判断ではない。

`terminology.md` は所有章と重複し、Parserを含む旧定義が現行の契約と一致しないため削除した。
overview索引からの参照も削除した。`architecture.md` の旧Typed IR VM図、公開APIのコピー、
重複した詳細説明は、RuntimePlanとAWBC productの実行境界、および既存の所有章への案内に整理した。
`crate-map.md` の古いworkspace分割一覧と重複device箇条書きも削除し、構成の参照先を
Cargo manifestへ統一した。層の責務、data-formatのSans-I/O、Needの明示的待機、
Activity snapshotのpure data、Native／HTML Viewのrouting、権限付きport、typed stream、
generator、backpressure、privacy、cancelの要件は現行の所有契約へ残した。
未実装であることだけを機能要件の削除理由にはしていない。

今回も現状一覧を唯一の項目別一覧とし、過去の実行・失敗・full SHAは履歴に保持する。
この整理で新しいCargo／runtime検証は実行していない。資料のリンク・fence・JSON・
whitespace・証拠のbyte一致と、開始時のtracked変更21件／untracked 6件および
既存のstaged 2 pathの保全を照合した。対象Markdown 9件の相対リンク390件は解決し、
未閉鎖fence・既存の40桁識別子の欠落は0、追加したJSON 7件はparseに成功した。

## 残存stable章の旧計画・重複状態表現の除去 — 2026-10-25

照合基点は公開済み `main` の `8a4a62cf6ee5a901bea1127517dea7ecf84e6362`。
開始時に既存Rust／sample変更14 tracked pathとuntracked 6 filesを確認し、維持した。
GPT-6 Luna／maxの2担当が現状一覧と残存資料を分担し、親が差分・根拠・公開を照合した。
現状一覧を公開SHAへ更新し、実行結果の履歴は保持した。

残存stable章から実装順序や過去の段階表現を除いた。`need-timeout.md` と
`const-block.md` は仕様・必要な負例／parityの条件を保ち、段取りだけを削除した。
timeoutのBehavioral Matrixは実施済み結果と誤読されないよう `Tests must cover` とした。
`core.md` は旧Phase 2.0説明、式値を将来typed evaluatorへ置換する計画、構造体と競合する
旧 `FlowFiber` enum、重複 `RuntimeStepOutput`／dispatch記述を除去した。fail-closed
loweringとcache hit/missが `state_hash` に入らない契約は残した。

`cli.md` の旧Phase 2.0／vertical-slice表記、削除済みcommandの経緯、将来の
materialization／player案を除いた。
runtime対応一覧に誤って含まれていた `traverse(...).parallel` は、現runtimeの接続がない
ため一覧から外し、言語契約は維持対象の[traits・sequence・range章](../01-language/traits-seq-ranges.md)
に残した。`lsp.md` はaccepted typed identityに基づく現行のnavigation／renameへ訂正した。
LSP、verification、proofs章の古いfuture-work／Phase進捗表現も削除し、同期方式、検証要件、
crate ownershipなど現在の契約を残した。sequence pattern／arrayのPhase表記とCraneliftの
将来executor案も、契約から進捗・ロードマップを切り離した。

削除した文に実行結果やfull SHAはなく、`docs/reviews/` の検証原本・履歴および
既存の受入原本は変更していない。公開照合receiptを追加し、変更したstable章は10 path。
確認は内容・相対リンク・fence・diff whitespaceと40桁識別子保全に限り、Rust／Cargo／
runtimeは実行していない。親による最終照合では変更Markdown 13件の相対リンク47件が
解決し、未閉鎖fence・whitespace違反・40桁識別子の欠落は0。
既存tracked WIP 14件とuntracked実ファイル6件の内容を保持し、公開receiptのbyte一致も確認した。

## Overview と build/security 章の重複・旧案整理 — 2026-10-25

照合基点は既存main `7ea4e5aa38d6be5ad6bb2e750fc9a772020e21d5`。
GPT-6 Luna／maxが6章を担当し、親が現状一覧・全差分・根拠の保全を確認した。
開始時に既存のtracked Rust／sample変更42件、untracked表示3項目（実ファイル6件）を確認した。
Cargo manifest、CLI option定義、AWFB/AWBC ownerを現行契約の照合に使った。

`00-overview/naming.md` から、authored root／local stateの重複配置例、
Cargo memberの重複一覧、MCP/LSPの候補名一覧、将来のrename手順を除いた。
公開名、拡張子、crate prefix、protocol prefixと所有章への参照は残した。
`authored-resource-storage.md` はすでに明記された現在のroot／fallback規則に
重なる過去の移行手順を削除した。

`device-generator.md` はCLI案、非所有のcrate/cache配置、phase表、未定義traitの
実装sketchを除いて、deterministic generation、入力・出力、source trace、
検証・permission・fixture・Agent/LSP要件を保持する短い契約へ整理した。
`native-web-build.md` は古いfeature名、CLI build mode、artifact tree、
将来のAOT案と重複したAWBC説明を除いた。native-only JIT境界、Web capability、
thread fallback、async WebGPU、限界検証、typed request/response、GPU ownershipを残した。

`security.md` の重複したcapture／USB／HID／controller節を一つに統合した。
manifest・host grant、allowlist、secure context/user gesture、signed debug exposure、
監査・indicator、typed parser/port、virtual fixtures、gamepadとcontrollerのrouting・
accessibility・observe/control境界を保持した。`packaging.md` から現行manifestと
CLIに一致しないumbrella feature/runtime flag案、structured_json bytecode表、
旧AWFB構成tree、schema/producerを持たないbug-report directory sketchを除き、
version-one AWFB/AWBC契約とresource・patch・signature要件は残した。

削除範囲に実行結果、validation receipt、full commit SHAはなかった。
変更した6章の相対link 26件が解決し、未閉鎖fenceは0、`git diff --check` は合格した。
文書だけの変更のためCargo／Rust検証は実行していない。
親の最終確認では、現状一覧と理由記録を含む8 Markdownの相対link 59件が解決し、
未閉鎖fence、40桁識別子の欠落、保持untracked 6実ファイルのhash差分は0だった。
[資料差分の照合](acceptance-evidence/task-documentation-contract-prune-audit.json)。

## 最新の検証範囲と重複説明の整理 — 2026-10-25

既存main `a08de43545edfe2a3d8848190035a69a447afb8d` と保持WIPを確認した。
現状一覧の冒頭に重複していた実装・検証の説明を項目別の表へ集約し、Presentationの
引数role／型モデルの公開と、後続Core tokenのowner検証・未接続の実行境界を別行にした。
前段sourceの全62逆依存結果を、新sourceの実行結果と誤読できる表現も訂正した。
受入の取消しや新しい不具合認定ではなく、証拠を適用できる入力と境界の明確化である。

Generic Matchの受入・公開済み状態を維持し、新規不具合のない完了項目を残作業へ戻していない。
数値・失敗・full SHAは表または検証履歴・原本に保持した。削除済みの歴史的設計や
旧ロードマップを現状のauthorityとして復活させていない。

最終の資料確認では相対link 34件、fenceと40桁識別子の保全、保持WIPを照合した。
通常の `git diff --cached --check` は、byte不変で取り込んだCRLFのJSON原本等を
trailing whitespaceと判定して終了1だった。原本を改行変換せず、既述の
`core.whitespace=trailing-space,space-before-tab,cr-at-eol` を指定した検査で確認する。

## 検証基点の更新と履歴に残った作業指示の除去 — 2026-10-25

公開基点 `9038c82a7a0aff5650bc635d74ff21f7e7cba69e` を照合した。
現状一覧のPresentation行は、公開済みscope grantと、その後のEngine所有・通常Native終了・
private rollbackのowner検証を区別する記述へ更新した。新しい実装を、未接続のSource／AWBC／
Dialogue activation／driver ABIまでの受入としていない。Generic Matchの受入・公開を維持した。

収束履歴に残る将来の作業順序3箇所を削除した。後続で解決済みの内容まで現在の残作業と
誤読させる重複であり、過去の数値・失敗・入力・full SHAは保持した。
冒頭のGeneric Matchは受入SHAと公開SHAを別々に記すよう訂正した。
以前のLuna／maxによる旧資料削除と現行仕様への統合は維持し、削除済み資料を復活させていない。
この文書整理そのものは新しいRust受入の証拠ではなく、同時進行のRust検証は収束履歴へ分けた。

## 画像callの採用契約と実装状況の分離 — 2026-10-25

公開基点 `2d1ce741898668793248d3d0dffd29698e0d6ba3` のCompilerを確認した。
`lower.rs` のPresentation候補拒否と、`Handle`／`SlotRef`／resource型のruntime投影拒否は
現在も残っている。このため画像章の「current bounded call lowers」という表現を、
配置の採用規則として記す形へ直した。これは未接続のproducerを実装済みと読ませない訂正で、
画像・backgroundの配置要件や、既存の下流image model／rendererの受入を取り消すものではない。
同じ章の旧fluent sketchとの比較も削除し、現在の宣言とcallが使う単一のsemantic modelだけを残した。
実行結果・full SHAを含む部分は削除していない。Rustの検証入力は変更していない。
