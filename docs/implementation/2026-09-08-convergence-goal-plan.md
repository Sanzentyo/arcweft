# 収束作業の再評価と実行 goal — 2026-09-08

## 確認した状態

- 対象: D:/git/arcweft の既存 main。
- 確認した受理済み Git SHA:
  64485364dcbc156c086ff6420f138d604a9d95d9
- fetch 後の HEAD/origin/main の差: 0/0。
- dirty/untracked: 658 件。index は空。
- レビュー ZIP: 71 個を列挙・SHA-256 計算。直下 inbox に ZIP はない。
- この記録は作業範囲と実行順序を定義する。Rust の実装完了や新たな
  テスト成功を示すものではない。

[9月1日の全体計画](2026-09-01-convergence-goal-critical-path.md)、
[9月7日の実装記録](2026-09-07-content-callable-continuation.md)、
[ジェネリック設計の記録](2026-09-07-generic-call-scope-contract.md)を、
現在の型推論・関数値・実行計画のソースと照合した。
本記録はこれらの実行順序・次作業の判断を更新する。過去の検証結果、
凍結済み ZIP、および未確認の実装範囲は書き換えない。

## 妥当性の判断

接続済みの収束系列を最後まで実装する方針は妥当である。
先行する型・意味情報・生成物を後続層が利用するため、次の依存順序を守る。

~~~text
Content/Fx・通常 callable の実装収束
  -> Generic Match C3/C5
  -> retained View .1.4
  -> RuntimePlan/task-plan .1.3.1
  -> scheduler/restore A-F
  -> 全体の検証・削除・main 反映

構造的 nominal C1-C6 の不足確認・実装
  -> scheduler/restore の前提として合流
~~~

Content/Fx を先に収束させる順序は、既存の未コミット変更との重なりを
解消するための実行順序である。Match -> View -> task-plan という
意味上の依存関係とは区別する。構造的 nominal の不足調査は早期に行い、
実装は所有境界が完結する区切りで統合する。

| 順序 | 作業 | 判断と完了する結果 |
|---|---|---|
| 0 | 現在の差分・受入条件・設計の照合 | 必須。既に成立している実装と不足を型付き API・動作で区別し、658 件を一括で破棄・再実装・コミットしない |
| 1 | ジェネリック、関数値、Content/Fx、ProjectCall の収束 | 最優先。再帰・後段で型が決まる部分適用・コールバック・attached default を含め、意味解析から native/AWBC まで実装し、全利用側と旧経路の削除を完了する |
| 2 | Generic Match C3/C5 | 妥当。受理済み設計と現在の実装の差だけを埋め、完全な transcript と exhaustive な行の atomic publication を証明する |
| 3 | retained View .1.4 | 妥当。Match の実装コミットを前提に既存依頼を更新し、設計を閉じてから operation、slot、capture、bundle/runtime/replacement を実装する |
| 4 | RuntimePlan/task-plan .1.3.1 | 妥当。実装済み View product に照合して設計を閉じ、一度だけの構築・seal・公開と実行側の利用まで完成させる |
| 5 | 構造的 nominal C1-C6 | 妥当。既存の record/variant/ADT 基盤を再利用し、exact join、到達型、AWBC、program-bound restore、ownership の不足だけを完成させる |
| 6 | scheduler/restore A-F | 妥当。上記の実装が揃った後、Sans-I/O scheduler、adapter、driver、保存・復元 transaction と旧経路削除を完成させる |
| 7 | 最終検証と main への反映 | 必須。全受入条件、実行・codec/golden・workspace・構造・該当 Tier 2 を確認し、対象差分を coherent な区切りで commit/push する |

### 前回のジェネリック設計をそのまま実装指示にしない点

現在のソースでは、同じ宣言パラメーターを Rigid と Bindable の両方の
map key にしようとする衝突、閉じていない継続の関数型を runtime 型に
投影する経路、呼び出し元の substitution layers、native の
Executable FunctionSite の同期 apply 拒否が残っている。
これらの修正が必要なことは、現在のソースと9月7日の失敗記録で裏付けられる。

一方、前回の設計が追加した「公開済みの多相継続を単相の関数型へ暗黙に
変換しない」という制約は、未実装のコールバック経路を避けることと
結び付けて説明されている。それだけでは、言語として採用する根拠にならない。

最初の工程で、通常の関数値、別名、カリー化、closure capture、
高階関数への受け渡し、効果・中断との整合を評価する。
多相性の全般的な拡張を自動で採用するのでも、実装を軽くするために
既存の利用を禁止するのでもなく、要求される利用側を満たす最終モデルを
選ぶ。スコープと callable の実行が互いの結果を変える部分は一緒に設計する。

前回の Free/Bound/Inference という区別、immutable な継続、
正規化された置換、有限インスタンス graph、決定的な上限は有力な方針である。
具体的な型配置、公開範囲、型の符号化、上限値についても、所有境界、
全利用側、型付き検証で妥当性を確認する。ZIP の READY というラベルや
原案どおりの型数を、実装の受入条件にはしない。

必要な訂正は現在の設計判断と実装記録に残し、意味規則が変わる場合は
維持対象の仕様も同期する。凍結済みパッケージの中身は直接編集しない。
実際に未設計の境界が判明した場合だけ、既存依頼を更新するか具体的な
補正依頼を作り、その解決を実装工程へ接続する。

## 今回の goal に含めない進め方

- 設計書や ZIP を作ったことだけで、実装工程や goal を完了とすること。
- 先行実装が未完の View/task-plan/scheduler に仮の表や型を置くこと。
- 受理済み設計を、現在ソースとの矛盾の証拠なく最初から再設計すること。
- 完了済みの固定 agent/model 指定削除を、引き続き主要作業にすること。
- 関連性を確認せず、71 個すべての ZIP を再実装・再依頼の対象にすること。
- この収束系列に必要でない機能追加、全面的な多相言語の拡張、
  無関係なリファクタやベンチマークを追加すること。

前段では対象外だった機能でも、後段の受理済み契約の必須条件なら、
その後段の工程で扱う。例えば前段の Need の非目標を、後段の task-plan
受入条件を免除する理由にはしない。

## goal の完了条件

1. 上表の実装対象を最新ソースと照合し、既存の正しい基盤を再利用した
   完全な producer/consumer の移行が main に反映されている。
2. 再帰ジェネリック、共有 prefix の異なる後段型、関数値、効果、
   attached default、source order、capture、停止・復帰を含む
   受入条件を、意味解析と native/AWBC の実行で確認している。
3. Match、View、task-plan、nominal、scheduler/restore の必須条件が
   それぞれ実際の生成物・実行・保存復元に結び付いている。
4. 置換された reader、fallback、重複 catalog、仮の owner、未使用の
   旧成功経路が残っていない。契約バージョンは 1 を維持している。
5. [テスト実行方針](test-execution-policy.md)に従う focused/changed-crate、
   workspace check、Clippy、test-workspace、必要な doctest、
   codec/golden、構造 gate、該当 Tier 2 が実施され、合格している。
   既存警告、適用外、外部条件による blocked を合格と混同しない。
6. 検証と設計変更の根拠、残作業なしという判断、完全な Git SHA を
   日付付きの実装記録に残し、対象変更を明示的に stage して cached diff
   を確認し、main に commit/push 済みである。
7. 必須の実装・検証が残る間は goal を complete にしない。
   設計資料作成や長時間実行、文脈の切り替わりだけで達成扱いにしない。

既存の main checkout で作業し、対象外のユーザー変更を保持する。
無関係な差分まで消して repository 全体を clean にすることは求めない。
新しい branch/worktree、強制 reset、無断の外部送信は行わない。
独立して進められる必要作業が残る間は進め、外部条件で詰まる場合は
実際の阻害要因と未実施範囲を記録する。

## この整理で実施した確認と未実施事項

実施: Git 状態・履歴・fetch、現行依頼の状態、上記の実装記録と設計、
該当する Rust ソース、関数値/カリー化仕様、テスト方針の照合。
Rust スキルと各適用 AGENTS の規約を確認した。

ZIP は列挙と SHA-256 計算を行った。今回、全71個の中身を再審査した
わけではない。採用する契約の詳細検証は、その実装工程の preflight で行う。

新しい Cargo テスト、workspace check、Clippy、runtime 実行は未実施。
9月7日の sema 3成功・1失敗および追加 compiler probe の失敗を、
今回の再実行結果として扱わない。この変更は実装計画の文書のみである。

この計画設定後の実装・実行検証は
[callable execution の継続記録](2026-09-08-callable-execution-continuation.md)
に記録する。上記の未実施事項は計画を記録した時点の状態であり、
継続記録の成功・失敗をこの計画の完了判定へ読み替えない。

その後の型参照・束縛スコープの移行状況は
[generic scope の移行記録](2026-09-08-generic-scope-migration.md)
を参照する。当該記録の時点では API 移行中でコンパイル未通過だった。
その後の現行検証は [constructor schema と nominal root の記録](2026-09-08-constructor-schema-and-nominal-roots.md)
を参照する。sema 全730テストは通過したが、native/AWBC の必須ケースと
後続工程は未完であり、先行する検証を全 goal の合格として扱わない。

続く [closure instance と関数値呼び出しの記録](2026-09-08-closure-instances-and-function-invocation.md)
では、root closure の意味情報を統一し、native の関数値呼び出しを共通の
実行フレームへ接続した。中断・再開を含む1,212ライブラリテストは通過したが、
実行マトリクスには14件の失敗が残る。全体の完了条件は変わらない。

さらに [呼び出し評価順と nominal AWBC の記録](2026-09-08-call-evaluation-order-and-nominal-awbc.md)
で、callee/capture と二項演算の評価順、nominal field の AWBC 読み書き、
HIR の不正レコード名の回復処理を修正した。旧 helper を前提とするテストは
実行結果の検証へ移行している。強めた named argument・pipe の再現ケースを
含めた未完項目は当該記録に残し、goal の受入条件から除外しない。

続く [block binding context の記録](2026-09-08-block-binding-context.md) では、
ブロック内の束縛推論を呼び出し元と同じ意味解析コンテキストへ統一した。
名前付き引数と pipe の強化ケースは native/AWBC で通過し、候補棄却時の
型情報の巻き戻しも確認した。現行の関連 integration は66成功・12失敗で、
関数スキーム・効果・後続工程を含む全体の完了条件は維持している。

[higher-order effect ownership の記録](2026-09-08-higher-order-effect-ownership.md)
では、引数 ABI の型参照と診断原因の保持を修正し、closure の関数型・効果
catalog・凍結済み推論結果の不一致を強化テストで確認した。一般の effect row
推論は未完である。現行 sema は737成功・7失敗、関連 integration は66成功・
14失敗であり、古い成功件数をこの状態の合格として扱わない。

続く [contextual effect hints の記録](2026-09-09-contextual-effect-hints.md) では、
宣言済みの効果境界を準備段階から参照し、効果変数を引数ヒントへ保持した。
未使用コールバックとカリー化の ABI は非空の効果を保持するようになったが、
本体で呼び出すコールバックの効果推論と実行側への全投影は未完である。
この改善を関数スキーム全体や後続工程の完了と扱わない。

[candidate completion と contextual source の記録](2026-09-09-candidate-completion-and-contextual-sources.md)
では、未束縛パラメーターの棄却と投影不変条件を区別し、原因の型付き情報を
保持した。コンストラクター同士が別々の型情報を補う例を受入検証へ追加し、
単なる引数順の入れ替えでは満たせない部分的な制約の受け渡しを特定した。
親子の推論・関数スキーム・実行情報の統合は引き続き同じ必須工程である。

[棄却呼び出しの値の証拠と分岐コンテキストの記録](2026-09-09-recovery-value-evidence.md)
では、第一候補の戻り値型を成功した値として回復する経路を削除し、
選択結果と値の有無を意味解析・実行計画の双方で検証するようにした。
分岐間の過剰な期待型も修正したが、相互に補完する引数の推論は未完である。
追加した CharacterDialogue の native/AWBC ケースは実行投影の不足で失敗し、
この残件も必須の実装として保持する。全体 goal の完了条件は変わらない。

[instance graph と型の依存関係の記録](2026-09-09-instance-graph-and-type-dependencies.md)
では、具体化の探索に件数・依存辺・作業量の上限と中断確認を追加し、
探索失敗時に前の受理済み世代を保持することを検証した。関数・クロージャー・
コンストラクターが保持する未選択ケースの型も同じ実行時型グラフへ接続し、
追加した native/AWBC 10件が通過した。型・定数・効果の訪問数と深さを含む
完全な上限管理、および関数スキーム等の18件の実行失敗は未完として保持する。

[型の具体化と作業予算の記録](2026-09-09-controlled-type-projection.md)では、
型・定数・効果の投影を反復処理へ移し、探索後の関数本体、クロージャー、
Content/Fx 等にも同じ作業予算を渡すようにした。上限超過時に以前の受理済み
世代を保持する検証も通過した。正規化・符号化とその前後のコピーを含む完全な
上限管理、sema 7件・native/AWBC 18件の既存失敗、および後続工程は引き続き
未完であり、goal の完了条件を狭めない。

[制御付き型符号化の記録](2026-09-09-controlled-type-encoding.md)では、
variant payload 内の独立したハッシュ計算も含めて型符号化を反復処理へ移し、
定数・効果とインスタンスキー生成にも同じ予算を接続した。既存の version 1 の
識別子を維持する検証と、符号化中の上限超過を確認している。実行時型の正規化、
周辺のコピー、未接続の符号化入口、既存の推論・実行失敗と後続工程は残る。

[型制約の正規化の記録](2026-09-09-constraint-projection.md)では、候補の型・定数の
置換を反復処理へ移し、長い置換列、深い型、途中の失敗でも予算とスコープを
維持する検証を追加した。sema は758成功・既知の7失敗、compiler library は
82成功、関連 integration は100成功・既知の18失敗である。親子の部分制約、
関数スキーム・効果、CharacterDialogue の値生成と全後続工程は引き続き必須。

[9月9日の実装チェックポイント](2026-09-09-convergence-checkpoint.md)は、
未コミット差分を蓄積し続けず自律的に commit/push するという追加指示に従い、
接続した実装・利用側・サンプル・証拠をまとめて記録する。進捗コミットと
goal 全体の完了は区別し、既知の失敗と検証の実際の状態を引き続き明記する。

[accepted Rust nominal の不足調査](2026-09-09-accepted-rust-nominal-gap-review.md)
では、受理済み C1-C6 と現行の登録・型・実行・復元・所有権 API を照合した。
既存の project nominal 対応と再利用できる部分を確認したが、Rust ADT の
厳密な catalog join と program に結び付いた復元を含む C1-C6 は未完である。
これは早期の不足調査の結果であり、callable の優先順位と scheduler 前の
nominal 実装完了条件を変更しない。

[Flow の効果公開の記録](2026-09-09-flow-effect-publication.md)では、解析済みの
効果を Flow と関数が共有する実行本体へ渡し、AWBC の署名まで保持するようにした。
明示された範囲、未使用の許可、部分適用の効果、および Flow 遷移のスコープを
native/AWBC で検証した。Agent REPL の失敗は解消したが、MCP のトレース生成は
その先の pattern binding の型不整合で失敗する。既存の sema 7件、残る callable
実行、後続工程は必須のまま保持し、goal 全体の完了とは区別する。

[ホスト呼び出しの型の記録](2026-09-09-host-call-type-authority.md)では、
引数・戻り値に実行計画で確定した型IDを保持し、AWBC の型の再生成を削除した。
呼び出し定義は型付きの行で共有し、各呼び出しの値と中断・再開を維持する。
MCP スクリプトは型検証を通過したが、CLI の観測レスポンスに必須の objects
配列がないため実行時の受理で失敗する。既知の callable 18件と後続工程を含め、
残件を免除せず次の実装へ進む。

[Agent の応答とバンドルの記録](2026-09-09-agent-host-response-admission.md)では、
宣言された Result 型の成功値をランナーが構築し、CLI の観測データと AWFB 出力、
生成コントローラIDの読み戻しを修正した。ソース・バンドル実行、トレース再生、
MCP の4テストが通過した。残る callable 18件に加え、Try の Agent型投影と
ネイティブ観測テストの HIR公開失敗を記録し、後続工程を含む goal は継続する。

[正規化された variant の選択の記録](2026-09-09-normalized-variant-selection.md)
では、Try のケース選択から限定的な型への変換を削除し、payload の型IDと
組み込みケースの構造を直接検証するようにした。runtime-plan の64テスト、
関連 compiler 22テスト、resource/attach/checkpoint の実行と MCP 4テストが通った。
callable 18件、capture の失敗とネイティブ HIR公開失敗、後続工程は未完のまま保持する。

[Agent enum の case authority の記録](2026-09-09-agent-enum-case-authority.md)
では、CaptureFormat・CaptureKind・PointerButton を既存の組み込み variant へ
統合し、実行計画の構築・値検査・パターン・native/pure・AWBC のケース選択を
共通化した。4 enum の全8ケースを native/AWBC の関数引数・Match・codec で
検証し、関連ライブラリ490件と capture ソースの check が通った。run に残る
スタックオーバーフローは意味解析の call graph 挿入で再現・特定しており、
引き続き修正する。既知の callable と後続工程の完了条件は維持する。

[call graph ノード格納の記録](2026-09-09-prepared-call-node-storage.md)では、
グラフとトランザクション差分がノードをヒープ上で所有し、マップの挿入・移動で
大きな候補データがスタックに積み重なる問題を修正した。スタック上限を変えず、
capture・attach・デバッグ記録の CLI テストが成功した。意味解析は760成功・
既知の7失敗で、ワークスペース check と Clippy も通った。残る callable 18件と
ネイティブ HIR公開失敗、後続工程を含む goal 全体は引き続き未完である。

[ルビのソース投影と生成式の記録](2026-09-09-dialogue-source-projection.md)では、
保持されたルビ略記のソース位置と、通常の content call に変換される式の
整合性を同じ生成定義で検証する。通常・入れ子・曖昧な候補での公開と、不正な
生成式の拒否を確認した。ネイティブ画像取得は HIR公開を通過し、ルビの描画用
テキストの所有関係で別の失敗に進んだ。回復中のクロージャー候補の失敗も含め、
残る実装・検証と全後続工程を引き続き必須とする。

[描画用テキストの所有関係の記録](2026-09-09-prepared-text-owner-projection.md)では、
話者名と本文の区別を観測データ・画像取得・描画順の共有処理に集約し、複数の
本文候補は型付きエラーとして扱うようにした。所有関係と View ID の４テスト、
MCP の４テストは通ったが、画像取得の最終ルビ画像は有効ピクセルがゼロになる。
CLI 全体で新たに確認した HTTP adapter の３件はテスト用 Flow schema の不足で
失敗しており、描画マスクとともに修正を続ける。goal 全体は引き続き未完である。

[HTTP adapter の Flow fixture の記録](2026-09-09-http-flow-fixtures.md)では、
テスト用の実行計画に必須の Flow schema を登録した。３件の HTTP テストと
CLI ライブラリ全166件が通過した。本文の表示時間の受け渡し、残る callable
および全後続工程は引き続き必須であり、goal の完了条件は変更しない。

[フレームの表示時間の記録](2026-09-09-frame-time-sampling.md)では、実行時の時計と
指定時刻の描画を明示し、本文・ルビ・Fx に同じ指定時刻を渡すようにした。
共有描画125件、元のネイティブ画像取得、WebAssembly の検査と単独ツールの
コンパイルが通過した。Tier 2 は次の画像サンプルの構文解析で停止し、追加の
typewriter テスト４件では描画前の Content 解析中にスタックオーバーフローが
発生する。これらと既知の callable 18件、回復中の構文候補、全後続工程を
引き続き必須とする。容量不足は生成物に限定した Cargo clean で回復した。

[式と候補データの所有関係の記録](2026-09-09-expression-payload-ownership.md)では、
準備中・確定後の式と、探索結果・意味解析の差分をヒープ上で所有する形に揃えた。
Content と Fx の入れ子で発生したスタックオーバーフローが解消し、ネイティブ
画像取得まで進む。型付き Fx の古い検証条件で４テストはまだ失敗しているため、
次の修正対象として保持する。直接取得したルビ画像は0秒で非表示、4秒で表示され、
位置と大きさも保たれた。sema 7件・callable 18件および全後続工程は未完であり、
この保持方法の修正を探索・実行全体の上限保証とは扱わない。

[typewriter 観測テストの記録](2026-09-09-typewriter-observation-fixture.md)では、
古い識別子の文字列比較を、組み込み Fx の定義・引数を構築する型付き API に
置き換えた。ルビ４件と通常の文字表示１件が最後まで通過し、指定時刻の表示、
配置維持、mask/object-id のピクセル検証が閉じた。残る構文候補・画像サンプル・
callable の失敗と全後続工程については、引き続き goal を継続する。

[パターンの終了境界と回復中のクロージャーの記録](2026-09-10-recovered-closure-projection.md)
では、パターンに割り当てられた領域全体の消費・残余入力の診断・正確なソース位置を
共通化し、回復中の候補も HIR 公開まで到達した。通常の不正パターンは引き続き
拒否する。有効なルビ表記は次の文書全体の実行可否判定で停止するため、
[候補内の構文回復と実行可否の設計依頼](../reviews/requests/2026-09-10-conditional-syntax-recovery-readiness.md)
に、候補の保持・診断・意味解析・キャッシュを一緒に閉じる必須作業を記録した。
設計依頼や HIR 公開の成功を、実行・AWBC と goal 全体の完了としては扱わない。

[条件付き構文回復の実装記録](2026-09-10-conditional-recovery-validation.md)では、
候補ごとの回復・ソース位置・capture と、選択後の意味情報・実行方式・キャッシュを
接続した。ルビ、capture の値と順序、候補内の Content を native/AWBC で最後まで
検証している。既知の sema 7件・callable 実行18件と画像サンプルの構文エラー、
後続工程は引き続き必須であり、この区切りを goal 全体の完了とは扱わない。

[未選択の呼び出しと実行計画の記録](2026-09-10-call-execution-admission.md)では、
tooling 用の拒否・曖昧な呼び出しに実行計画を与えず、コンパイラも同じ型付きの
実行可否判定を使うようにした。実行計画・効果・dialogue の利用関係を確定する
処理とそのテストは専用モジュールへ移した。意味解析の既知の失敗は、追加した
相関する通常呼び出しの受入テストを含めて9件、callable 実行は22件である。
親子の型制約・効果・関数値実行と全後続工程を引き続き必須とする。

## Callable native/AWBC 失敗数の監査訂正 — 2026-09-23

既存の記録と検証ログを照合した。確認時の `main` は
`233ac21d8664da4a71dbff4150bc5b78b2a568ab`、編集前の作業ツリーは dirty
（450 paths: 335 modified、6 deleted、109 untracked）である。上記の18→22は
`2026-09-10-call-execution-admission` 時点の有効な checkpoint だが、その後の
記録でさらに2つの native/AWBC case が追加され、最新の失敗 matrix は24件である。

根拠ログ:

- 18件: `.arcweft-local/validation/2026-09-10-recovered-closure-projection/test-workspace-final.log` — 54 passed / 18 failed。
- 22件: `.arcweft-local/validation/2026-09-10-call-execution-admission/final-compiler.log` と `workspace-tests.log` — 54 passed / 22 failed。2つの相関する通常呼び出し case が native/AWBC 各1件ずつ増えた。
- 24件: `.arcweft-local/validation/2026-09-10-final-call-diagnostics/compiler-full.log` — 57 passed / 24 failed、および `.arcweft-local/validation/2026-09-10-curried-group-effects/compiler-2.log` — 56 passed / 24 failed。後続の `2026-09-11-record-storage-admission/test-workspace-after-clean.log` も 57 passed / 24 failed と記録する。`compiler-1.log` の26件は `curried_terminal_effect` の2件を含む中間結果で、`compiler-2.log` ではその2件が通過して24件へ戻った。

以下は24件の失敗を構成する6 familyで、各caseは `::native` と `::awbc` の両方で実行される。

| Failure family | 実テスト名 | 主なownerと最終失敗 |
| --- | --- | --- |
| Contextual / correlated type evidence | `contextual_project_constructor_infers_an_unselected_case_parameter`; `contextual_constructor_sources_combine_complementary_type_evidence`; `contextual_project_unit_constructor_closes_with_a_later_argument`; `correlated_ordinary_call_closes_from_a_later_parent_argument`; `correlated_ordinary_calls_combine_complementary_parent_evidence` | `arcweft-lang-sema` final analysis/candidate constraints は final type または選択結果を確定できない。singular ordinary-call case は `arcweft-compiler` reachability で selected-call authority 欠落としても現れる。 |
| Inferred callback effect rows | `callback_with_inferred_effects`; `uninvoked_callback_with_inferred_effects` | `arcweft-lang-sema` の effect-row seal/inference に unknown row が残り、compiler/runtime-plan 投影もそれを拒否する。 |
| Shared-prefix generic scope | `shared_prefix_with_distinct_later_types` | `arcweft-lang-sema::types::GenericScope` の lexical binder depth が閉じず、`arcweft-compiler` の runtime semantic projection で失敗する。 |
| Function prefixes used as callbacks | `curried_prefix_as_callback`; `generic_prefix_as_monomorphic_callback` | Curried prefix は `arcweft-core` native/AWBC execution で `ProjectContinuation` を `Function` として使おうとして失敗する。Generic prefix は `arcweft-compiler` の selected-call reachability authority を欠く。 |
| Nonterminal prefix returned through callback | `callback_returns_a_nonterminal_prefix` | `arcweft-lang-sema` final analysis が式の admissible final type を確定できない。これは22件 checkpoint 後に加わった2件である。 |
| Character factory calls in both branches | `character_factory_branches_keep_both_selected_calls` | `arcweft-compiler/src/lower.rs` が Dialogue callable family に typed runtime intrinsic がないとして両backendを拒否する。 |

generic sema 修正後に回す最小の focused command は
`cargo test -p arcweft-compiler --all-features --test callable_execution` である。
この test target が各 family の native/AWBC case をまとめて検証する。直近の24件は
歴史的な失敗結果であり、現在進行中の Sema 変更に対してこの command は**未実行**。
全24件と goal の実行条件は未解決・必須のままであり、成功、期待拒否への変更、
skip、goal 完了を主張しない。

## CharacterDialogue factory/reconfigure producer gap — 2026-09-23

Read-only follow-up inspected `main` at
`233ac21d8664da4a71dbff4150bc5b78b2a568ab`; the shared working tree was dirty
with 420 paths. No source edits or Cargo commands were made. The two known
`character_factory_branches_keep_both_selected_calls::{native,awbc}` failures
are at the compiler-to-executable boundary: sema retains the selected
`CheckedCharacterDialogueFactory` and source-ordered patch, while
`lower.rs::runtime_call_target` falls through because runtime-plan and core
have no typed ordinary CharacterDialogue construction operation. The maintained
contract requires first-class immutable values; skipping the calls or emitting
an empty constant would lose source behavior.

After the current coherent commit/push, the next cut must connect the complete
owner chain: project the exact Factory/Reconfigure operation into a closed
runtime-plan expression through the existing source-row ANF; evaluate the
callee and every authored operand exactly once in source order, including a
value later cleared by a patch; and use a generation-bound Sans-I/O producer
owned by `arcweft-dialogue` to apply patches and encode the existing exact
opaque value. Core must carry only the closed operation and typed producer
boundary, with native/AWBC wire, verification, transcript, and execution kept
in sync. The producer inputs must come from accepted generation assembly of
real defaults, role payload types, custom-field registry, and View catalog.

The source contract also needs to distinguish logical Character declaration
membership from optional visual composition evidence: `pub character alice {}`
is valid without a visual manifest, so use explicit Absent/Present visual
evidence and validate looks only when present. Never fabricate an empty
manifest or zero digest. Dynamic `CharacterDialogue<Any>` values must retain
the actual constructed Character/config through application and display; the
current static-contract-only target is insufficient. The same cut includes
native/AWBC calls, captures, View/display, bundle generation, old-generation
pins, and save/restore re-admission.

Acceptance must observe produced values, not only successful compilation: run a
branching function for both conditions through native and AWBC, then assert the
returned opaque producer/semantic identity, 18-slot payload, selected Character
reference, defaults, and an explicit patch field. Keep the Dialogue branch and
selected-call authority intact. Add dynamic-target display and generation
admission coverage with the relevant consumers. The focused
`cargo test -p arcweft-compiler --all-features --test callable_execution` has
not been rerun; this design note is not implementation or validation evidence.

## CharacterDialogue generation and bundle bridge checkpoint — 2026-09-24

Inspected `main` at `48cc58f52e10f7ace58036d40a9647fa39e5cc5c` with a clean
working tree. The following coherent cuts were committed and pushed:

| Contract cut | Full Git SHA |
| --- | --- |
| Contextual function-value specialization and typed result-port evidence in Sema | `98254aea5870ad23c7dbc9ab41f84fc2a67102c9` |
| Read-only sealed Character inventory access for Compiler | `07142c973384b4081ff7165ad87b6b0a4bf4c0f2` |
| Bounded canonical v1 CharacterDialogue generation codec | `8a7c5d8683d9e1ca9807c40f220874fd9ac53a2b` |
| AWFB generation/package section and manifest/PNG/fingerprint admission | `9b36049ec276c82b228426535d1e1bc5fc19c458` |
| Dialogue-owned Core runtime-call backend adapter | `4a6191143d6990522197147d6067ddd2d37fc46c` |
| Compiler/RuntimePlan generation declaration from checked profile and complete logical Character inventory | `6e4c57ce2a86e8abbff2bedd1ee845ebee64abf2` |
| Profile/Agent AWFB handoff, typed package resource collection, and explicit rejection of lossy non-AWFB output | `48cc58f52e10f7ace58036d40a9647fa39e5cc5c` |

Observed passing validation at this checkpoint: Sema focused 8/8 and library
895/895; Dialogue codec focused 3/3, adapter focused 1/1, and library 52/52;
RuntimePlan library 79/79; Compiler Character focused 23/23 and library 112/112;
Bundle all-feature tests including its AWFB/PNG regression; CLI profile-to-AWFB
focused 1/1 and library 170/170. Changed-crate all-target/all-feature Clippy
completed with exit 0 for Sema, RuntimePlan, Compiler, Bundle, CLI, and
runtime-codegen; Dialogue all-target Clippy also exited 0. Existing warning
output remains and is not a strict-warning pass. The earlier Compiler Agent
JSON roundtrip was migrated to AWFB and verifies the generation digest.

This is a bridge checkpoint, not the factory/reconfigure acceptance: runtime
generation binding, actual Native/AWBC producer execution, dynamic target
display and Style admission, old-generation task pins, save/restore, StageLook,
and the broader callable/Match/View/task-plan/nominal/scheduler goal remain.
The focused `callable_execution` suite and workspace-wide final gates have not
been rerun for this checkpoint. The next cut is a Dialogue-owned binding from
the immutable declaration plus actual View/Style/Character resources and the
executable owner into one runtime schema; driver execution must select it by
the calling program generation.

## CharacterDialogue 実行・表示接続 checkpoint — 2026-09-24

既存 `main` checkout で次の契約単位を commit/push した。各 SHA は完全な Git
object ID であり、作業ツリーは最後の source cut `bdcc697742191836ab765b9551330d62636eb4be`
の push 直後に clean と確認した。

| 契約単位 | Full Git SHA |
| --- | --- |
| 宣言から実行プログラム所有の schema を束縛 | `98355fe1c0070a71b8e5883969c277fed419f28f` |
| Bundle の受理済み Character 資源と retained runtime image へ schema を接続 | `69e0309440f65be06575f5f04fc53fd557865c21` |
| Dialogue RichText 設定から text-model style への型付き投影 | `3d601b66766a2136a00b2cab32225ebf66ebb9ba` |
| Presentation 所有の再利用可能 Style パラメータ投影と Ruby 本文の除外 | `64a28a9c589b4788a285685b67679c75883ef85b` |
| Native/AWBC 実行テストへの実プログラム所有 producer 注入 | `4ed35f6e47dfe6f8e9d25a73007a7b16fb43e079` |
| 動的表示設定・View Style 選択・保持世代と置換状態遷移 | `bdcc697742191836ab765b9551330d62636eb4be` |

この地点で受理済み runtime schema が実際の opaque `CharacterDialogue` と
同じ executable owner を検査し、表示時に有効な View、Voice、RichText、
Style sheet、その他の設定を解決する。選択された Style sheet は会話 View の
root scope に View 既定値の後で入る。世代置換は View/Style の意味論変更を
検出し、古い表示と theme の所有世代を fiber 終了後も保持する。保持世代が
現行単一成果物の save 形式で表現できないときは、現行世代として偽装せず
型付きエラーで拒否する。

この cut で観測した gate: Compiler `evaluated_effects` 17/17、text-model
library 20/20、Dialogue library 60件と integration 4件、Presentation
library 131件と integration 47件、driver の世代/置換 focused 9/9、選択
Style/Clear focused 1/1、driver all-feature library 78件と integration
22+5+30件が通過した。対象 crate の all-target/all-feature Clippy は終了
コード0（既存 warning あり）、構造 gate は blocking 0、staged diff check
は通過した。workspace all-target/all-feature check、`callable_execution`
全 matrix、test-workspace はこの checkpoint では未実行である。

残る必須作業は、動的表示の実値による直接回帰テスト、StageLook の
旧参照移行、旧世代を含む保存復元の完全な世代表現、callable 全 matrix、
Match/View/task-plan/nominal/scheduler の各受入条件と最終 workspace gate
である。局所 gate の成功を収束 goal 全体の完了とは扱わない。

## Callable matrix と workspace HEAD の再測定 — 2026-09-24

上記の動的表示の直接回帰テストは、実 schema、2つの Character、受理済み
View/Style、RGBA RichText を用い、古い世代と Character 不一致の拒否も
確認して `c0bf0f374df376ffcd9d1e94e131af316cc0b3e7` として push した。
focused 1/1 と runtime-driver all-target/all-feature Clippy 終了コード0を確認した。

その HEAD 単体で `cargo check --workspace --all-targets --all-features` は
終了コード0（既存 warning あり）で通過した。ログは
`target/.arcweft-local/2026-09-24-workspace-all-target-check.log` にある。
`cargo test -p arcweft-compiler --all-features --test callable_execution` は
**81 passed / 6 failed**。失敗は Character factory の Native producer 未注入と
AWBC instruction 14 型不一致、generic prefix callback の両 backend、同じ
prefix の異なる後段型の両 backend の3 familyである。ログは
`target/.arcweft-local/2026-09-24-callable-execution-matrix.log`。これは既存の
24件という歴史的 checkpoint の更新であり、matrix の合格ではない。

## Callable source/specialization 統合 checkpoint — 2026-09-24

既存 `main` checkout で公開済み多相 callable の source、選択済み body、
特殊化後の継続 state を一つの実行契約に接続した。次の各 commit は push 済みで、
`4d58570285f3f2b2e7f06bb3fdb54a99b7af3784` の直後に working tree は clean と確認した。

| 契約単位 | Full Git SHA |
| --- | --- |
| Scoped callable 型を RuntimePlan/AWBC へ保持 | `f26cac6cd53843d6409afaaf57b51c17e7c54953` |
| Sema の callable 特殊化と body instance 証拠 | `a9336628d1e6e12208152457b5f2e75f6dbef3c8` |
| Native/AWBC の検証済み特殊化実行 | `38bc6a4335b6ccb0fa39ddceb9dc4d0d189adb3f` |
| Sema の generic source と alias 特殊化 | `0b32f15c8b31c78ba83c1d5c4a283caead3512e2` |
| Compiler/RuntimePlan の source demand、状態列、View capture ingress と全利用側接続 | `4d58570285f3f2b2e7f06bb3fdb54a99b7af3784` |

最新の観測: `callable_execution` は 90/90、Compiler lib は 112/112、
`flow_effects` は 5/5、`view_product` は 11/11、Core lib は 596/596、
Sema lib は 907/907、RuntimePlan lib は 79/79、runtime-accelerator lib は
91/91、runtime-codegen lib は 12/12。workspace all-target/all-feature
check と Clippy は終了コード0（既存 warning あり）、構造 gate は blocking 0、
cached diff check は通過した。ログは `target/.arcweft-local/2026-09-24-callable-*`、
`target/.arcweft-local/2026-09-24-flow-effects-focused.log`、
`target/.arcweft-local/2026-09-24-view-product-full.log` に保持した。

変更 crate 群の全テストは **未合格**。Core integration
`direct_suspension::cancellation_unwinds_nested_frames_and_scopes_once_in_lifo_order` が
`InvalidFrame` で 1 件失敗した（`2026-09-24-callable-changed-crates-tests-4.log`）。
取消し・保存復元境界の修正と再検証は scheduler/restore 工程の残件である。
workspace `test-workspace`、最終 doctest、codec/golden、該当 Tier 2 と
goal の他工程も未完了であり、この checkpoint は全体完了の証拠ではない。

## Callable 変更 crate gate 訂正 — 2026-09-24

**Supersedes:** 直前 checkpoint の `direct_suspension` 未合格記録。
`main` の `df1528dc14f8fe77a99f3523375bd0e5fadb8c81` を検査し、
working tree は clean。`InvalidFrame` は取消し実装ではなく、検証対象の
AWBC に宣言されていない lexical scope を fixture が保存していたことが原因。
scope 宣言、Enter/Exit 命令と保存 cursor を一致させたテスト修正を
`48359e2d09a1ee5d41807ecf0b9fbb6babb97057` として push した。
`direct_suspension` は 8/8 で通過した。

Sema の外部 API compile-fail fixture は、現在非公開の `CheckedMatchRef`
を内部証拠として検査し直し、重複した旧テストを削除した。
診断 stderr の更新は `df1528dc14f8fe77a99f3523375bd0e5fadb8c81`
として push し、`api_compile` は 14/14 で通過した。

`cargo test -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan
-p arcweft-compiler -p arcweft-runtime-codegen -p arcweft-runtime-accelerator
--all-features` は終了コード0で全テスト・対象 doctest が通過した。
ログは `target/.arcweft-local/2026-09-24-callable-changed-crates-tests-6.log`。
この gate は6 crate の証拠であり、workspace `test-workspace` および
goal の他工程の完了を示すものではない。

## Content/attached callable 統合 checkpoint — 2026-09-25

既存 `main` checkout で、工程1の Content 呼び出し・添付 default・generic
継続に関する統合を進めた。以下は push 済みの完全 SHA。`main` と
`origin/main` は最後の `cef1e68e85e857389c29d463425e2c0fd30af721`
で一致した。一方、この時点の working tree は Content 実行接続と回帰修正の
未コミット差分が残り、clean ではない。

| 契約単位 | Full Git SHA |
| --- | --- |
| HIR 外部 API の privacy 診断更新 | `24aa2d67c4ad079a5ccd21a170ca1cf3c59db36e` |
| HIR 添付 default 内 closure の子 scope 所有 | `cd20e039b7362b8a69e065f6a3c282e5589fc9fe` |
| generic 相互再帰の native/AWBC 回帰 | `e47cefda918f4ac9e5a77831806ad7e9619e3c3e` |
| generic 継続 snapshot/restore の型・世代拒否回帰 | `bd58057a7667c471c13409d5f1255c57a557c6af` |
| Sema 添付 default closure の捕獲と宣言所有 | `274977525074e49e9a8ce8182a2c29745e25d88f` |
| 通常 callable arrow と添付 Content ABI の分離 | `876ebc5fa1092025380fd4ef8dc2052173bff13d` |
| 標準 Dialogue runtime role の accepted nominal identity 統一 | `cef1e68e85e857389c29d463425e2c0fd30af721` |

検証済みの範囲: ABI 変更の core check、focused 30件、core all-target
Clippy 終了コード0（既存 warning あり）、Dialogue role identity、
compiler の既存 DialogueView profile 回帰、HIR all-feature テストは通過。
Content の新規 compiler→AWBC lowering 回帰は通過したが、native/AWBC の
実行結果確認はまだ進行中。Sema all-feature test は **908 passed / 2 failed**。
失敗は View `on_click` の2件で、accepted nominal 移行後の利用側を修正中。
この失敗を解消するまで Sema gate は未合格である。

`just test-workspace` は最初 D: の空き容量不足で中断した。提案されていた
`cargo clean` を1回実行して約268 GiBの旧ビルド成果物を削除したため、
以前の `target/.arcweft-local/` ログも現存しない。再実行では HIR の
compile-fail 診断3件の差分で停止し、上記 `24aa2d67...` で更新して focused
確認済み。修正後の workspace 全テスト、現 HEAD 単体の workspace check、
最終 Clippy、実行 backend の Content 受理は未完了。過去の局所合格を最終
受理へ繰り上げない。

**2026-09-25 訂正:** `cef1e68e...` 後の View `on_click` 2件は、
標準 modifier が DialogueAction の accepted nominal 登録前に旧 `Named` 型で
公開されていたことと、compiler の View capture が runtime carrier 付き
Record を snapshot owner として受けていなかったことが原因。選択済み
handler schema と所有権判定を修正した
`6d14a110a2ceb45ba2d44d054a766b35ffec26ba` は push 済み。
Sema all-feature lib は **911/911**、compiler の `on_click` と
DialogueView profile の focused テストは各1/1で通過した。これは上記
**908 passed / 2 failed** の現行状態を置き換える。A06 の native/AWBC 実行、
workspace 全 gate および後続工程は引き続き未完了。

**2026-09-25 Content 実行接続の検証追記:** 添付 Content 呼び出しの
到達辺を selected HIR から Compiler へ保持し、Dialogue の値スロットを
target 評価後に caller Flow で一度だけ評価する形へ接続した。native と
decode 済み AWBC の双方で `i64` / `String` の generic 呼び出し、
省略した本文の default、スロット値と `log.write` の回数・順序を確認した。
この A06 fixture は明示的な本文の default 回避を直接証明しないため、
別の core focused 回帰の証拠と区別する。添付 default の効果は callable
の公開効果行へ合成し、Sema all-feature lib 911/911 と compiler
`evaluated_effects` 18/18、core lib 601/601、RuntimePlan lib 79/79 を確認。
workspace all-target/all-feature check と Clippy は終了コード0（既存 warning
あり）。構造 gate も blocking violation 0 で通過した。変更 crate 群の
rustfmt は通過したが、workspace 全体の fmt は未編集の
`runtime-accelerator/src/compile.rs` の差分で未合格。
構造監査の `final_flow.rs` SIZE001/TEST001 は既存の閾値超え。今回の
変更は同じ final Flow lowering owner 内で、スロットの型付き local admission
と利用を一つの `FinalLoweringContext` に結ぶ。別の state authority や
I/O 層への依存を追加せず、旧 callback body lowering と capture 走査を除去し、
ファイル行数も純減したため、この区切りでは owner を維持する。

`just test-workspace` は Compiler lib 112/112、HIR lib 913/913、Sema などを
通過した後、Syntax の外部 API compile-fail fixture 5/23 の診断差分で停止。
この5件は import 拒否の診断位置だけの差分で、期待出力を更新して focused
23/23 の合格を確認し、`2dfa62739053d1f89faa37ffb28d3a7b29d60653` で
push 済み。workspace 再実行は Launch lib 41/43 で停止した。2件とも
fallback style fixture の旧 `layout` field が現在の `value` 契約に拒否される。
Launch fixture を確認・修正してから再実行するため、workspace 全テストは
引き続き未合格である。
ログ: `target/.arcweft-local/2026-09-25-content-*`。Content 実行接続は
`4f1beb729038103b7bdaa3aee6c2a13bf1ab1525` で push 済み。
Launch fixture、workspace 全テスト、goal の後続工程は未完了。

**2026-09-25 Launch fixture 訂正:** 旧 `{ layout, value }` を使用していた
2件の fallback style fixture を現行の strict `{ value }` 形へ更新し、
`d39ae35e4a9e9fdcab9ee172b3b41c24222d18d2` として push した。
Launch focused 2/2、lib 43/43、crate Clippy、fmt check は通過。
workspace 再実行と phase 1 の明示本文による default 回避の source→native/AWBC
証拠は引き続き進行中。

**2026-09-25 fmt 訂正:** Content 側の追加テストの整形漏れと
runtime-accelerator の対話 work-unit 計算の rustfmt 非安定な表記を修正し、
`7b22d4695d049e86cb4d9e878e8664533ec94813` で push した。
`cargo fmt --all -- --check` は終了コード0で通過。上記の workspace fmt
未合格記録を現行の結果として使わない。

**2026-09-25 workspace 再実行の現状:** Syntax と Launch の fixture 更新を含む
`just test-workspace` は LSP lib 217/221 で停止した。hover の3件は
`agent.observe` の選択先効果を fixture が提供できず、Dialogue hover の1件は
応答が null になった。現行の意味契約に照らした原因修正と再検証が必要で、
workspace 全テストは依然未合格。ログは
`target/.arcweft-local/2026-09-25-content-test-workspace-4.log`。

**2026-09-25 明示本文の追加証拠:** Sol Max の source surface 確認を受け、
Dialogue 内容内で空白なしの `#supplied()[provided]` を A06 に追加した。
添付 default 側は実行時に区別できる `supplied-default` ログを持つ。
native と canonical decode 済み AWBC の双方で表示が
`default / provided / second`、ログが省略呼び出し分の `default-enter` 4件
のみであることを focused 1/1 で確認した。この結果は上記の「A06 は明示本文
の回避を直接証明しない」という当時の記録を置き換える。
`6078fe816f2882c1571ffdf50b81e7177d219543` で push 済み。

**2026-09-25 LSP fixture 訂正:** hover 3件の fixture は選択 profile に
`native-file` adapter を指定し、提供される `fs.read` 効果を検証する形へ更新。
子 module Dialogue hover は final analysis に含まれない Test scenario の
式を問うていたため、通常 Flow 内の canonical `character[content]` に移した。
LSP 実装や HIR/Sema authority は変更していない。focused 7/7 と1/1、
LSP lib 221/221、crate fmt/Clippy は通過（既存 warning あり）。
`858fff451627f511972e4033a3557b9bd8f09754` で push 済み。
workspace 全テストはこの後に再実行する。

**2026-09-25 後続の実装・検証 checkpoint:** Tooling の public API 診断、
CLI の Windows 実行スタック、RuntimePlan `If` の Sema 型、server entry の
RouteWhole、player-native の Factory fixture をそれぞれ
`564db18f1639bf4f024b02e656da9883d2c3595c`、
`68f4590b6377778b37403caeedba8a9ea4eed99c`、
`264b6430e6954689b0d81bb85db3989cf92dc141`、
`a7c2840dbb07417c9b1bec9bcfcc5e77ffb57935`、
`0da05d1e048ff822a8ba0aec39875ef406df8734` として main へ push した。
この時点の `cargo test --workspace --lib --tests --exclude arcweft-cli
--quiet --no-fail-fast` は終了コード0。CLI の named integration は6対象中5対象が
合格し、`arcw_fixtures_check_run` の3集約テストは最初の未実装 fixture で停止する。
これらは workspace 全 gate の合格を意味しない。

`defer on completed/cancelled/failed` の source outcome、Block 本体、
欠落 body の回復を Syntax→HIR で型付き保持し、inline colon Dialogue 後の
`with:` を正しい plan 境界に接続した
`4b251bb79ea6e4843d1f4a0778d34a7abe4bfc7c` も push 済み。
HIR lib は長時間の既存 nominal shape limit 1件を除いて906 passed、
Syntax lib は682 passed、workspace fmt は終了コード0。CLI の
`current_pass/check` は、直前の新規回帰だった 011 を通過し 023 で停止する。
023 の `cancel on input(.SkipLine)` は現状の Syntax/HIR に型付き経路がなく、
式として回復されることを確認した。

実行側の `defer` は未完了。試作した RuntimePlan の静的 cleanup 配列への
書き込みは、未到達の登録も実行し、activation 中の失敗では cleanup を
飛ばし、activation local と affine 値を cleanup scope に渡せないため採用せず
作業ツリーから除いた。必要な契約は scope ごとの実行時登録・逆順解放と
捕捉値所有、失敗／取消時の子 scope と root activation の unwind、native と
AWBC 共通の work identity である。023 の `cancel on` と合わせ、工程1の
runtime acceptance は未完了。

**2026-09-25 取消入力 selector の型付き producer checkpoint:**

Supersedes: 直前の「023 の `cancel on` は Syntax/HIR で回復される」という
当時の観測。

`cancel on input(.SkipLine)` の専用 Syntax/HIR owner、braced/colon body、
回復、selector の source span、Sema の `InputActionId` と RuntimePlan の
Trigger fact を接続した。Thread body の子を HIR child edge と body projection
へ二重登録していた欠陥も、後者を authority として修正した。
`6a45bdf9ed62f3cdd83f639b6df9cc133c3c3463` を main へ push 済みで、
この時点の working tree は clean。Syntax lib 684/684、HIR lib は既知の
長時間 nominal limit 1件を除き 910 passed / 8 ignored、Sema lib 912/912、
RuntimePlan lib 79/79。変更 crate の all-target /
all-feature Clippy と workspace fmt は終了コード0（既存 warning あり）。
`current_check_fixtures_pass` は 019 の実行側 `defer` 未接続で失敗する。
023 を単独実行すると `.Skipped` の最終型が決まらず Sema で停止する。
子文付き `cancel` の簡約例は Sema と runtime semantic fact まで通り、
RuntimePlan が未完成の実行投影を明示的に拒否する。したがってこの commit
は source→checked fact の完了証拠であり、取消実行・戻り値選択・cleanup
の受理証拠ではない。workspace 全 gate はこの変更後に再実行していない。

**2026-09-25 取消時の行結果型:** 023 fixture は取消側だけに `out` があり、
通常経路は `()` になるため、維持仕様の同一行結果型へ直した。
`9af741b691229dfed5b28207dbc11e9414d01e3b` で push 済み。
Sema は通常経路の `out` と期待された `DialogueLine<R>` から結果型を確定し、
取消 body 内の対象 `out` を同じ型で検査する。型不一致と通常 `out` 不在の
取消結果は拒否する。`c4d80862887c90ae172a5b9e53dc7b2009727ad0` を push 済み。
Sema lib 914/914、workspace fmt、Sema all-target/all-feature Clippy は
終了コード0（既存 warning あり）。この変更後の workspace 全 gate、
019/023 の実行受理は未完了。Sema 変更前の CLI では、019 の集約検証と
修正後 023 の単独検証が RuntimePlan の `defer` 未接続で停止した。

構造 disposition: `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`
は production Sema の一式評価 owner。基準 commit の 185892 bytes / 4278
physical LOC から 188148 bytes / 4321 physical LOC へ 43行増えた。
追加した処理は既存の文式評価 walk に、同じ行結果 authority で型付けする
`out` の期待型を渡すもの。同じ `Analyzer` の facts・topology を使い、
別 state、I/O、逆向き依存、重複 traversal を追加しないため owner を維持する。

**2026-09-25 入力 action の実行経路 checkpoint:**

確認した main/`origin/main` は
`51bc16cf0940e8638018d49589b7d585c5c4ca98`、working tree は clean。
同 commit は authored dialogue View の action button から、観測した dialogue
occurrence の完全な token と型付き `InputActionId` を driver に渡し、core の
activation 固有の入力イベントとして native/AWBC 共通 reducer に接続する。
入力の epoch/sequence を保存し、古い activation や再送を取消に流用せず、
dialogue mark と action trigger を分離した。AWBC の schema marker は1のまま。
保存復元では未処理 action を保存 blocker に含め、復元した button の mount
provenance を sealed View の投影と照合する。

core の action順序・mark分離・snapshot replay・codec round trip の focused
4件、scene の button/semantic admission 16件、View projector 2件、driver の
activation/stale revision と復元 provenance 各1件が通過。`cargo check
--workspace --all-targets --all-features`、同 Clippy、workspace fmt、
`just structure-audit-gate` は終了コード0（Clippy は既存 warning あり、
構造 blocking violation 0）。`just test-workspace` は CLI fixture より前の
workspace lib/integration と CLI の先行 named test が通過したが、
`arcw_fixtures_check_run` で3/7失敗して終了コード1。019 は実行側 `defer`
未接続、spec run 011 は final expression type 未確定、spec check 022 は
nominal type resolution 未確定で停止した。後二者はこの commit で変更して
いない Sema に属するが、HEAD 単体での合格は未確認であり、回帰と断定しない。
WebAssembly 単独 check は transitive `getrandom 0.3.4` の `wasm_js` 設定不足で
止まり、web target の合格証拠にはしない。

この checkpoint は入力 trigger の producer/consumer 移行であり、
実行時に到達した `defer` の登録・LIFO unwind、取消 branch の結果選択・
transfer、および 019/023 の end-to-end 受理を完了したものではない。
次は line result `R` と取消 disposition の仕様章を整合させ、動的 cleanup と
native/AWBC の最終公開を同じ authority で実装する。

**2026-09-25 行結果契約の整合判断:**

`74625a45c` の main/`origin/main`、clean tree から維持仕様を再照合した。
受理済み `DialogueLine<R>` は非 escaping operation で、実行後に著者へ渡る
値は `R`（`out` がなければ `()`）。古い `LineOutcome` ラッパーは `R` を
保持できず、既存の tuple/handle binding と Sema/runtime の型経路にも
一致しないため現行仕様から除く。取消を著者が観測したい場合は通常の
`R = Result<T, LineCancel>` を選び、正常側と取消側で同じ `R` を `out`
する。`try` は通常の Result 伝播として扱う。cancel `continue` は
pending の正常 `R`、cancel `out` は同型の別 `R`、`goto`/`return` は
値を公開しない transfer とし、子 scope・行 scope の cleanup 完了後に
一度だけ公開または transfer する。cleanup 途中の失敗では再 unwind せず
終端を失敗にする。この判断を維持仕様の例へ反映したが、runtime の結果
選択と dynamic `defer` 実装は未完了。

**2026-09-25 CLI dialogue line fixture boundary:**

`main` の inspected SHA は `649e655981c12e1d8bfa5c1449866be46ad72c17`。
この切り分けは defer registration substrate の
`78c68948bd42538dd4dcc9171abe04ba05f07bf9`、compiled AWBC region で defer を
拒否する `a95cd92d629e8cb7e8d79f8a8e34608cd2baa28a`、contextual receiver の
Sema/compiler 修正 `649e655981c12e1d8bfa5c1449866be46ad72c17` の後に行った。
作業開始時点では 011 fixture だけが dirty だった。現在の unstaged 差分は
CLI run test、011/024/025 の spec fixture、009/010 の current fixture の移動、
および本記録で、他の dirty path は確認していない。

直接 source の CLI run は、accepted project-default Dialogue profile を
生成しても `RuntimePureAccelerator` に CharacterDialogue producer を接続しない。
さらに CLI step loop は `line_commands` を受け取って line outcome を返す host
loop を持たない。`009_dialogue_line.arcw` と `010_line_task_effects.arcw` の
`arcw run --mode drain --steps 16 --entry entry.main` はどちらも exit 0 だが
`final_status=failed runtime CharacterDialogue construction requires an accepted
generation producer` を出した。このため両 fixture と 011 を run から check へ
移した。011 は no-profile direct source で有効な VoiceHandle discard と
line-result tuple を保つ最小形にし、cue を String にした。StageAcquire、
ActorLook、scheduled cue は direct CLI fixture に登録済み Character manifest
がなく、元の cue を戻した check は `sema.final_analysis` で失敗した。
これらの contextual receiver 契約は、登録済み Character manifest を使う
Sema の `dialogue_line_plan_bindings_are_inferred_in_source_order` が検証する。
native/AWBC の line outcome progression と CLI line host は引き続き未実装で、
fixture の再分類はその end-to-end 受理を示さない。

CLI fixture runner の run assertions は process exit code のみで成功扱いしない。
各 runtime summary は terminal `done` または `return` status を要求し、
`process.exit(0)` で summary を出さない 001 CLI stdout fixture は正確な
`hello` 出力を確認する。`cargo test -p arcweft-cli --test
arcw_fixtures_check_run -- --nocapture` は両 run 集約が通過し、
`current_pass/run` の残る8件と `spec_should_pass/run` の8件を実行した。
CLI fixture integration target 全体は7件中5件通過、2件は check 集約で停止した。
`current_pass/check/019` は line
defer/runtime content lowering、`spec_should_pass/check/030` は Sema final type
resolution の未解決箇所であり、この fixture cut では変更していないが、全体 goal
では未完了のままである。正確な `arcw compile --emit check` は移動した
011/009/010 と修正した 024/025 すべてで exit 0、0 warning、0 obligation。
Sema contextual receiver cut の library test は `649e655` 時点で 916/916。

**2026-09-25 workspace compile checkpoint:** fixture 分類と実行成功判定を
`4c88b48e58430590099f09d21e4fa86fd07aa21c` まで main に push した clean
checkout で、`cargo check --workspace --all-targets --all-features --quiet` が終了コード0。
既存 warning は残る。これは workspace test、Clippy、019/030 の fixture 受理を
代替しない。

**2026-09-25 checked defer / collect convergence checkpoint:**

inspected `main`/`origin/main` は `a88c5bfacc4d926ada484ea1b71a10d2375ee726`、
working tree は clean。Sema の defer は outcome だけでなく Block body と自由ローカルの
型付き capture ABI を保持する（`a9a067100c1931c6de269c9ff17357eb74ec8ae9`）。
compiler→RuntimePlan の global/closed-instance fact に body、capture、checked effects
を投影・検証した（`67827583eae1686c44f775c1740c46bc08b01a0a`、
`2aef7f59084ccd6423c6cc77c462f0a9c9f2c37c`）。global defer body の executable
function site と capture input を行コンテンツ lowering より前に予約し、行 root の
到達文から `RegisterDefer(LineRoot)` を発行する
`a88c5bfacc4d926ada484ea1b71a10d2375ee726` を push 済み。
RuntimePlan 80/80 lib tests、changed-crate compiler check、RuntimePlan all-target/all-feature
Clippy、fmt は exit 0（既存 warning あり）。正確な
`current_pass/check/019_line_defer_cleanup.arcw` は `arcw check` exit 0、
0 warning、0 obligation。これは登録 site と静的 plan の受理であり、deferred body
の native/AWBC unwind、nested scope の登録・所有、失敗/取消結果選択は未完了。
`current_pass/check/023_dialogue_cancel_defer_on.arcw` は次の `Out` statement lowering
で失敗し、行コンテンツ handle も未発行である。現在の global defer 以外と defer
body 内 assertion は、未実装を成功扱いしないよう lowering で明示的に拒否する。

collection `collect` は Sema が Vec/Seq receiver item と destination `Vec<Item>` の
制約を接続し、919/919 lib tests を通過した
`d69a7e99d7ff322a2a626508c978af31547c1661`、compiler が選択済み
`CollectionMethodId::Collect` を `CoreIterCollect` へ写す
`0eda52ef6d9328a6ae9056ca502d07dd4b6f27b6` を push 済み。
`spec_should_pass/check/030_closure_pipeline_value_position.arcw` の `arcw check` は
exit 0、0 warning、0 obligation。`arcw run` はこの check fixture に公開 entrypoint
がないため bundle entrypoint 検証で停止し、実行受理の証拠ではない。

**2026-09-25 HEAD workspace gate:** 記録 commit
`4a98a5f8eea728f987a772b3c31560efa941cc8e` を含む clean main/
`origin/main` で `cargo check --workspace --all-targets --all-features --quiet` は
終了コード0（既存 warning あり）。`arcw compile --emit plan` で 019 の plan に
`RegisterDefer { owner: LineRoot }` があることも確認した。自由ローカル
`message: String` を捕捉する独立した line defer source の `arcw check` も
exit 0、0 warning、0 obligation。検証用の一時 source/plan 出力は削除し、
working tree は clean。defer body の runtime unwind と取消側 `out` は未完了。

**2026-09-25 dynamic defer child-work checkpoint:**

Sol Max に現行 native/AWBC state と save/restore の境界を照合してもらい、行 root
defer の実行は中断中の親 Flow の `FunctionCallFrame` に混ぜず、dialogue activation
が所有する子 work にする判断を採った。各登録の単調 ID、固定した exit filter、
1件ずつの LIFO dispatch、捕捉 affine 値の `LineScope`→`ChildScope` 移管、
filter 不一致時の明示 drop を一つの共有 state に置く。取消側 `out` は既に commit
した通常 `R` との結果選択が必要なため、別の型付き disposition 境界で閉じる。

`2bd7aded482b9ad19c7c5cd4d2e4b33b35a9a37c` は実 source の行 defer 2件を
RuntimePlan と検証済み AWBC へ下ろし、capture 数・outcome・codec 往復を確認する
compiler integration test を追加した（focused 1/1）。
`31b1514d6b214d2ea89d137d06f61cdb8781c825` は共有 activation が登録 ID を
発行し、snapshot の ID 順序と次 ID を検証する。`f7dde83d9d3b2c8aa88dbca4e5acdab68476222a`
は固定 exit の LIFO 選択・inflight ID・捕捉値の子 owner 移管と skip 時 release を
transactional な候補状態で実装した。Core lib 612/612、Core all-target/all-feature
Clippy、workspace all-target/all-feature check は exit 0（既存 warning あり）。
固定 exit の snapshot 往復、未対応の単独 inflight restore 拒否、affine skip release
の focused tests も通過した。inspected main/`origin/main` は後者の完全 SHA で
一致し、working tree は clean。

これらは共有状態遷移の証拠であり、native/AWBC が子 body を起動・再開・完了報告
する経路はまだ未接続。特に AWBC の既存 line-task child は suspend を受理しない。
inflight を含む activation-only snapshot は、子 fiber との厳密な照合が未接続の間
fail closed にしている。終了時の handle unwind は deferred stack/inflight が
空になるまで拒否する。nested/CurrentScope の登録と取消結果選択も未完了。

**2026-09-25 line-root defer executor checkpoint:**

Supersedes: 直前 checkpoint の「native/AWBC 子実行と inflight restore 未接続」という
実装状態。固定 exit と取消 `Out` の未完了判定は継続する。

inspected `main`/`origin/main` は `6f2e2178fdaec1e8f4e3ba7cd86b2ac830201da6` で
一致し、working tree は clean。native と decoded AWBC Product は共有 activation の
inflight 登録 ID を親 dialogue に所有させ、捕捉 packet を子 fiber に渡して LIFO で
実行する。条件不一致の body は実行せず、完了・失敗後に次の登録へ進む。AWBC 子は
HostCall/Need/Await/AwaitMany/budget 中断を保持・再開し、save/restore では登録 ID、
site、content、子の capture/handle packet を照合する。activation だけの inflight
snapshot と不一致の子は引き続き拒否する。

検証は core lib 613/613、AWBC Product focused 27/27、実 source の登録/capture/
codec と native/AWBC LIFO 実行の compiler focused 各 1/1、`cargo fmt --all -- --check`、
`cargo check --workspace --all-targets --all-features --quiet`、core all-target/all-feature
Clippy が終了コード 0（既存 warning あり）。compiler `evaluated_effects` target 全体は
20/21 で、既存の `evaluated_effect_operands_reach_awbc_from_final_checked_sources` が
RuntimePlan transaction type graph に semantic type がないとして失敗した。この cut で
変更した経路の focused tests は通過したが、target 全体の受理とは扱わない。

defer body の `log.info(...)` を Block の末尾式に置くと現行 lowering は `ReturnExpr`
として扱い、ログ効果を出さない。一方、効果文にすると body 内の自由変数が Sema の
defer capture ABI から漏れ、RuntimePlan 構築が lexical scope 不足で失敗する。
実行 fixture はこの未接続境界を混ぜないよう、定数引数の効果文で LIFO/フィルタを
検証した。自由変数付き末尾効果の実行、nested/CurrentScope、取消側 `Out` の結果選択、
defer 内の Product-owned Dialogue/Choice suspension は残る。

**2026-09-25 defer body capture convergence checkpoint:**

Supersedes: 直前 checkpoint の「効果文内の自由変数が defer capture ABI から漏れる」
実装状態。末尾効果呼び出しが `ReturnExpr` として実行されない問題は継続する。

inspected `main`/`origin/main` は `87188c4474034a9fa8096bd3498eb91a9a915ea5` で
一致し、working tree は clean。Sema の単一 `CheckedStructuralEdgeDraft` が選択済み
実行グラフの文・body の式エッジを順序付きで保持し、defer の自由変数収集はその checked
エッジを辿る。`defer { log.info(message); }` と body 内 `let captured = message` の双方で
外側の `message` を一度だけ capture し、body 内ローカルは capture しない。

検証は Sema focused 1/1、Sema lib 919/919、外側の `message` を使う実 source の
native/AWBC LIFO 実行 1/1、`cargo fmt --all -- --check`、workspace
all-target/all-feature check、Sema all-target/all-feature Clippy が終了コード 0（既存 warning
あり）。compiler `evaluated_effects` target 全体の別テストにあった RuntimePlan 型グラフ
失敗の受理証拠にはならない。Block 末尾の効果呼び出しはまだ効果として下りていない。

**2026-09-25 expression-owned evaluated-effect checkpoint:**

Supersedes: 直前 checkpoint の「Block 末尾の効果呼び出しは効果として下りていない」
実装状態、および line-root executor checkpoint に記録した compiler
`evaluated_effects` target の型グラフ失敗。nested/CurrentScope defer と取消側 `Out`
の結果選択は引き続き未完了。

inspected `main`/`origin/main` は
`4ba649355b0e478e882323e78842a047d4665004` で一致し、working tree は clean。
Sema の通常 evaluated effect は Call/Pipe の正確な expression root が一度だけ operation
を所有し、expression statement は site/application digest の型付き参照だけを持つ。
値位置の Block tail と文位置は同じ operation を実行する。Pipe の左辺と引数は source
order で一度ずつ評価し、`Unit` は継続値、`Never` は非継続として下ろす。closed
project-function instance でも expression payload が operation と必要な pipe fact を
保持する。dialogue callback effect の line-plan 所有は維持した。

`defer { log.info(message) }` と `defer { log.info(message); }` の両方で、自由ローカル
capture を伴う行 root deferred body が native/decoded AWBC で LIFO 実行される。
通常 Flow の Pipe tail と、通常 Project function の直接 Call/Pipe tail は、各 1 個の
effect operation を持つ RuntimePlan と検証済み AWBC を生成する。閉じた base enum
が選択 case からのみ到達する場合は、その typed variant owner の nominal identity/layout
を type graph に登録する。これにより既存の `drop(.Cancel)` / `.Stop` の型グラフ失敗も解消した。

検証: Sema lib 920/920、RuntimePlan lib 80/80、compiler lib 112/112、compiler
`evaluated_effects` 23/23、compiler 全 integration tests は
`RUST_MIN_STACK=16777216` で全 target 合格。workspace all-target/all-feature check、
workspace Clippy、変更 crate all-target/all-feature Clippy、fmt は終了コード 0
（既存 warning あり）。Windows の既定テストスレッド stack では compiler 全 integration
tests の `callable_origins_remain_distinct_across_a_branch` が compilation 中に process
exit `-1073741571`（stack overflow）で停止する。同じ単独テストと全 integration
targets は上記 stack 設定で合格した。既定 stack の失敗を合格扱いしない。

追加の native/decoded AWBC 実行証拠を
`f482f0065054674305579d8434605a7067685196` で main に push した。inspected
`main`/`origin/main` は同じ SHA、working tree は clean。通常 Flow の Pipe tail と
Project function の直接 Call/Pipe tail は、各 backend で `piped` ログを正確に 1 回出す。
compiler `evaluated_effects` target 23/23、同 test target の Clippy と fmt は終了コード0。

**2026-09-25 dialogue cancellation result-selection checkpoint:**

Supersedes: expression-owned evaluated-effect checkpoint の「取消側 `Out` の結果選択は未完了」
という実装状態。`continue` による pending `R` 維持、cancel handler 内から親 Flow への
`return`/`goto` 投影、nested/CurrentScope defer、保存用 DTO の完全な型到達性はなお未完了。

inspected `main`/`origin/main` は
`e824dbbd7f5d03aca0b749ae0bedc1a22882094b` で一致し、working tree は clean。
sema の checked output target が正確な DialogueLine application を保持し、compiler は
その application に属する cancel `out` だけを RuntimePlan へ投影する。通常結果を
activation に一度 commit した後、join された正確な取消 action の handler が同じ型の
値を返した場合だけ、公開前の結果を一度選び直す。native と AWBC は同じ activation
result authority を使い、AWBC では取消 handler 専用 function kind と verifier-checked
`R` signature で `Return(Some(R))` と fallthrough の `Return(None)` を区別する。
affine handle の旧結果の drop と新結果の child custody からの移譲は同じ activation
transaction に入り、選択 action は reducer/snapshot と照合する。native で handler 完了後に
Closed reducer を親が公開せず待ち続ける経路も閉じた。作者の通常 `return` を AWBC で
黙って pending 結果として扱わず、未接続の親制御移譲として拒否する。

検証: `023_dialogue_cancel_defer_on.arcw` の CLI check/verify 合格。core lib 615/615、
sema lib 920/920、RuntimePlan lib 81/81、compiler lib 112/112、compiler
`evaluated_effects` 24/24。新しい実 source は通常/取消の native と decoded AWBC で
実行され、AWBC の各ステップでメモリ上の snapshot/restore が通過した。workspace
all-target/all-feature check、変更 crate all-target/all-feature Clippy、fmt、cached diff
check は終了コード 0（既存 warning あり）。compiler 全 integration tests の既定 Windows
stack overflow はこの checkpoint で再実行しておらず、以前の失敗を合格に読み替えない。

保存用 `AwbcProductExecutorSaveSnapshot::from_live` から
`into_live_for_program` まで同じ実 source で試すと、取消前の tick 2 に対話 target の
semantic type が AWBC の型グラフにないとして失敗した。従ってメモリ上の復元成功を
保存用 DTO の受理とは扱わない。次はその型到達性と cancel `continue`/親制御移譲を
それぞれ所有境界で閉じる。

**2026-09-25 CharacterDialogue policy type reachability checkpoint:**

Supersedes: 直前 checkpoint の「保存用 DTO の型到達性未完了」という実装状態。
cancel `continue`、親 Flow `return`/`goto`、nested/CurrentScope defer は継続課題。

inspected `main`/`origin/main` は
`7acf4c5c50dd36b5452acfdd8cd89a689f8a92c0` で一致し、working tree は clean。
CharacterDialogue producer が Voice / InlineFailure / InlineFallback / FallbackStyle の
nominal schema、型 seed、variant domain、structural payload descendant を同一の
case specification から保持する。RuntimePlan は生成 fact の graph をそのまま
atomic admission へ渡し、binding は active program の owner/layout/cases を照合する。
手組みの dialogue、player-native、runtime-driver fixture も同じ graph を使う。
実 source の通常・取消 native/decoded AWBC 実行に加え、AWBC の各 tick で
SaveDTO 化、JSON 往復、exact program への復元が通過した。

検証: dialogue lib 61/61、RuntimePlan lib 81/81、compiler lib 112/112、compiler
`evaluated_effects` 24/24、player-native lib 44/44 と関連 integration 2/2・8/8、
runtime-driver 対象 1/1。workspace all-target/all-feature check と Clippy、fmt、
cached diff check は終了コード 0（既存 warning あり）。`just structure-audit-gate` は
本 cut の production owner 変更後に blocking violation 0 を確認した。
`policies.rs` は base 167 行から 866 physical LOC（32,343 bytes、production 731 行、
embedded tests 135 行）へ増えた。責務は producer-owned policy schema/projection/
binding に一貫し、RuntimePlan 側への依存逆転や重複 authority はないため維持する。

提案の `cargo clean` を checkout 内の `target` と確認して一度実行し、215,153 files、
254.4 GiB の古い成果物を削除した。clean 前の workspace test は容量不足を避けるため
ビルド中に停止し、clean 後に再実行した。再実行では workspace lib/integration 群と
CLI の先行 named tests が通過したが、最後の CLI `arcw_fixtures_check_run` は 5/7 で
終了コード 1。`024_stream_for_yield.arcw` は HIR staged arena validation、
`032_raw_dialogue_braces.arcw` は raw Content の checked attached body projection で失敗した。
この 2 件を workspace test 合格とは扱わない。維持仕様と owner を照合した結果、
通常関数の `for` の HIR/source freeze と、checked RawLiteral を消費する compiler
projection のそれぞれに不足がある。次の独立した cut で修正する。

**2026-09-25 Raw Content / ordinary `for` integration checkpoint:**

前 checkpoint の `032_raw_dialogue_braces.arcw` と `024_stream_for_yield.arcw` は、
それぞれ独立した owner の修正で CLI `compile --emit check` を通過した。Raw Content は
checked attached literal body を compiler が一度だけ読み、重複した checked text
authority を除去した (`b2b4702b0c3049c9933833ddc634a7a09db74794`)。通常関数の
`for` は source-backed Block を普通の statement scope として HIR/source freeze まで
保持し、Sema の candidate が反復情報を再計算する際は ledger の rollback/projection
を使って既存の公開事実を上書きできるようにした。公開文脈での重複事実拒否は維持する
(`cb7c7077cf611a5bc672270a71da63fe9c8f5dc1`)。

通常 `for` の検証は Syntax lib 685/685、HIR lib 912/912（既存 ignored 8）、
Sema lib 920/920、024 CLI check、workspace fmt check、cached diff check が終了コード0。
`026_headless_observation_calls.arcw` の実際に使う effect 上限と、spec 033〜035 の
未宣言の型を fixture に補い、4件それぞれを CLI check で確認した
(`f02cc464b262304ad8b7db907155f7ee09172af5`)。この時点の `main` と
`origin/main` は同 SHA、working tree は clean。

全体テスト合格は未達。CLI `current_check_fixtures_pass` は次の 025 で停止する。
`proof` の現行 grammar は固定 parameter group を要求し、本文の `check` は recovery。
`promote_unchecked` は Sema で placeholder の `Promoted` 型に留まり、compiler の
runtime intrinsic を持たない。Sol Max と照合し、この fixture を単に空の unsafe
block に置換することは受理証拠を損なうと判断した。30件の current-pass check
fixture の直接棚卸しでは 27件が通過し、025 のほか 027 の dialogue line plan が
syntax/HIR で失敗する。026 は上記修正で通過済み。CLI `spec_should_pass_check` は
042 の `bail` 呼出しに checked call fact がなく RuntimePlan lower で停止し、044 は
未定義型を補っても pattern の最終型付けで失敗する。これらを既存 warning や未実行の
workspace gate と混同しない。次は goal 本体の Content/Fx・call と取消継続の owner
接続を優先し、最終 gate の前に 025/027/042/044 を個別の契約として処理する。

**2026-09-25 Result-match fixture boundary correction:**

直前 checkpoint の「042 で停止」は fixture の対象外の未接続 `bail` 呼出しを
切り離したことで解消した。042 の `Result<i32, String>` の失敗枝を同じ型の
`Err("Input must be greater than 0")` にし、結果 `match` の受理を維持した
(`37d174fe814c3f0081f8f4628613b2a3b6d0f8c1`)。CLI spec check は 042 を
通過し、次の 044 の未定義 `GameEvent` で停止する。042 単独 CLI check は終了コード0。

`bail` 自体は Sol Max と compiler/native/AWBC の経路を照合した。
Sema の純粋関数の expression-site 分類を Flow-site に変えるだけなら compiler
`compile --emit check` は進むが、AWBC 検証は結果型を持つ function site の値なし
終端で `ResultShapeMismatch`。現行 core は `Bail` を flow failure に変換する一方、
維持言語契約は `Err(ArcError)` を返す。この差を AWBC の trap で覆うのは契約違反と
判断し、局所パッチと失敗する追加テストは撤回した。`bail`/`ensure` の型付き Result
carrier と native/AWBC の返却は独立した全層移行が必要。042 の `String` error 型を
その未実装経路の受理証拠に使わない。

044 を一時 source で分解すると、正しい payload-bearing enum 宣言を足した後も
`.ChoiceSelected { id }` は pattern 最終型が欠ける。payload-binding pattern に変えると
`Vec.pop_front()` は未接続 CapacityMethod intrinsic で止まり、Option source に替えると
effect call を expression arm に置いた `match` は runtime disposition を要求する。
この一連は Match/consumer migration の対象であり、fixture を単なる通過形へ弱めず
未編集のまま維持した。workspace test と spec fixture gate は引き続き不合格。

**2026-09-25 dialogue head / proof fixture checkpoint:**

直前 checkpoint の current-pass 025 は、`proof` の現行固定 parameter group と
supported expression body だけを検証する `025_proof_declaration.arcw` に整理した
(`e2f4e89fdfa87df87ec8ddfa502919e1d6ef67ab`)。以前の fixture にあった
`check no_lifetime_below`、値なし `promote_unchecked`、unsafe region は compiler-pass
受理証拠ではなかった。unsafe audit の HIR/verifier 専用検査は維持し、promotion の
型付き operand・lifetime・ArcError/Result return・native/AWBC 実装が完成するまで
compiler-pass の成功とは主張しない。025 単独 CLI check は終了コード0。

027 の syntax failure は、bracket dialogue call の後続 `with:` のコロンを
`speaker: content` の先頭コロンとして拾うことが原因だった。speaker-colon の探索を
先頭物理行に限定し、bracket call の `with:` plan projection を保持する回帰テストを
追加した (`171d60a1f591f0956420c85e826dbab32bc48c80`)。Syntax lib 686/686、
既存 HIR line-plan focused test 1/1、fmt と cached diff check は終了コード0。
元 027 fixture は syntax diagnostic を越えたが HIR recovery が続く。局所 source
切り分けでは `init:` は plan の Error item になり、`on mark(...) =>` では HIR arena
を通るものの、残る計画項目と後続の裸 `{ ... }` に recovery がある。
`on mark(...):` と bracket call に続く裸 lexical scope の一般接続は別の
producer/consumer 境界として未完了であり、fixture は未編集で維持した。
CLI `current_check_fixtures_pass` の全体合格、workspace test の合格証拠はまだない。

**2026-09-25 Dialogue 裸 scope / `on` 本文 checkpoint:**

確認した `main`/`origin/main` は
`eb27cf0136211825d7a075e9c191de935f3a9edc`。この時点の working tree は
`init` 全層移行中と 027 fixture の `out ()` 訂正が未コミットであり、clean ではない。
bracket dialogue の直後の裸 `{ ... }` を、添付 line plan ではなく次の Flow
Scope item に分割した (`219c51f9f7d889b503bdda389e29732d01342072`)。
`with { ... }` が添付 plan のまま残るテストを追加した
(`b7a8021896f247e7258c90e9dc81912324275557`)。CLI で単独検証を通した
`028_dialogue_bare_scope_after_call.arcw` も current-pass fixture に追加した
(`1c26791e10e91837377b32f97b9512adaea6ab5c`)。

`on mark(...):` と波括弧本文は、既存の `HirStmtKind::On` の順序付き本文・
scope に接続し、source-index の本文順序と回復を検証する
(`eb27cf0136211825d7a075e9c191de935f3a9edc`)。ハンドラ内 `defer` は
`CurrentScope` 登録に投影する。On focused syntax/HIR と、marker、入れ子の
`defer on failed:`、行レベル `out ()` を含む単独 CLI check/verify は終了コード0。
ただし compile-only の証拠であり、defer 本体の native/AWBC unwind は未確認。

027 の集約 current-pass check はこの時点では HIR staged arena validation で失敗。
`init:` の独立スコープ付き pre-reveal 実行は作業中で、当該 gate は未合格。
さらに維持仕様の `on mark` 本文内 `out` は、現行 Sema が nested output を
結果型へ集めず、core/native/AWBC と保存復元が取消 `InputActionId` 専用の選択状態を
持つため未接続。マーク起因の結果選択はハンドラ scope を終了させ、行そのものは
子処理と cleanup を待ってから一度だけ公開する契約として、後続 cut で閉じる。

## Dialogue Init / AWBC 実行 checkpoint — 2026-09-25

Supersedes: 直前の「027 は HIR staged arena validation で失敗」「Init は作業中」
という状態記述。確認した `main` と `origin/main` は
`386707b71d798348428c249685e52a576be12594` で一致し、この確認時点の
working tree は clean。後続の `on mark` 結果選択は別 cut として作業中。

bracket dialogue 後の裸 scope 分割が `for value in [true, false] { ... }` にまで
及んだ回帰を statement head の境界で修正した
(`bdc2910d7582111a36e507cb75de11dc3e7331b0`)。Syntax lib 696/696 と
Sema statement producer matrix 4/4 を確認した。型付き Init は独立の pre-reveal
scope、source-index、nested line `out` の結果型、後続項目の到達性、および
RuntimePlan の EnterScope → CommitDialogueResult → ExitScope に接続した
(`5242a0bfb2df3e6a3c3f2b6da83bd75176f6017d`)。focused Syntax 6/6、HIR
4/4、compiler 1/1（AWBC verification を含む）、Sema lib 923/923 が通過。

native Init は scoped defer の LIFO cleanup、affine handle/drop、早期 `out`、
EvaluatedEffect と HostCall continuation を接続済み
(`0d2c04bad4c7e7c00e9353a405767aed666ed924`)。core check、Init 5/5、defer
9/9、transaction rollback 1/1 と structure audit gate が通過。AWBC product は
HostCall の保存復元・再発行、activation effect の実行バッチ、CurrentScope の
defer と affine VoiceHandle capture/drop/release、`out` 後の tail skip、cleanup
後の reveal を接続した (`a6e435371641e4100bcf954a880c64684d055a49`)。
新規 acceptance 1/1、AWBC defer 10/10、core check、direct suspension 8/8 が
通過した。027 fixture の未定義 `.Completed` を Unit result `out ()` に訂正し
(`71152e7d43fee81e6e56551281dc221cb6d2bf7a`)、単独 CLI check/verify と
current-pass check 30件の直接 CLI 実行がすべて通過。全体 fmt check の残る
syntax test 書式差分も整えた (`386707b71d798348428c249685e52a576be12594`)
後、`cargo fmt --all -- --check` は終了コード0。

この checkpoint は Init と 027 の受理であり、維持仕様の `on mark` 本文内
`out` の実行選択、特に通常の `out` がない非 Unit 行、native/AWBC の mark
選択と保存復元は未完。generic child-fiber の nested defer も未接続。
workspace all-target/all-feature check、workspace test、Clippy と goal の後続工程は
この cut では未実施・未完了であり、局所通過を全体合格とは扱わない。

## `on mark` 行結果選択 checkpoint — 2026-09-25

Supersedes: 直前 checkpoint の「`on mark` 本文内 `out` は未完」という状態記述。
確認した `main`/`origin/main` は
`ebfd683fcca56a16e8dce4a39952687e8ba9ada2` で一致し、working tree は clean。
維持仕様にある通常 `out` を持たない非 Unit 行を受け入れ、到達した mark handler
の `out` が行結果を選ぶ。通常の行閉鎖時に結果が未選択なら公開せず失敗する。
複数の mark handler は静的な候補として認め、実行経路で二度目の mark 選択を
拒否する。取消 handler の `out` は公開前の mark 選択を置換できる。

共有 result state の source は exact `LineTaskWorkTag` であり、reducer graph、
consumed Mark、joined Action、実行 instance と restore を照合する。
native/AWBC は同じ選択 authority と affine `ChildScope → DialogueResult` 移譲を使い、
子 scope の cleanup 前に値を保持し、release を重複させない。AWBC は通常の
`Return` と別の selector terminator を schema-1 codec・verifier・VM・product・
snapshot に接続した。Sema は全 nested `out` の対象 application と結果型を
一致させ、RuntimePlan は Mark Action と取消 action にだけ selector を投影する。
HIR scope-graph テストの古い架空 source-backed owner は source-index 検査が
先に拒否していたため、実ソースの donor Flow owner を使う fixture に訂正した。

検証: native と decoded AWBC の On-only 実行 1/1（Mark で `Released`、
Mark 不在では結果公開なし）、core lib 627/627、AWBC focused 211/211、
Sema lib 925/925、RuntimePlan package 全 target、compiler package 全 target
（Windows では `RUST_MIN_STACK=16777216` を指定）、HIR lib 918/918
（既存 ignored 8）、
workspace `cargo check --all-targets --all-features`、workspace Clippy、fmt、
structure audit gate（0 blockers）、cached diff check は終了コード0。
Windows 既定 stack の `callable_origins_remain_distinct_across_a_branch` overflow は
以前と同じ単独テストで再現し、スタック設定下の合格と混同しない。

`RUST_MIN_STACK=16777216` での `just test-workspace` は HIR fixture 訂正後、
CLI `spec_should_pass_check_fixtures_pass_after_refactor` の
`044_pattern_binding_combo.arcw` で停止した。診断は nominal TypeId に完全な型が
ないという既知の Match/fixture 境界で、全 recipe の合格ではない。
generic child-fiber nested defer、残る callable/Match/View/task-plan/nominal/
scheduler・restore 工程と最終 workspace gate は引き続き必須。

## Inline enum record payload / statement match arm checkpoint — 2026-09-25

`c0f39f1547b57843e4381c13fbc7db9811f033d5` を `main` に push 済み。
`enum GameEvent { ChoiceSelected { id: i32 } }` の inline record payload を
syntax、HIR、source-index、project symbol、Sema の nominal schema と semantic
digest、RuntimePlan の admission まで型付きで保持した。statement `match` の
expression arm は checked Ignore continuation で effect を実行し、native と
decoded AWBC の両方で effectful/pure arm の破棄を確認した。record variant pattern
の compiler preflight も、checked `VariantPayload` owner に限って受け入れる。
native/decoded AWBC の inline-record pattern 実行 1/1 を確認した。

検証: syntax lib 699/699、HIR lib 918 passed（既存 ignored 8）、Sema lib
929/929、RuntimePlan package 全 target、compiler package 全 target
（`RUST_MIN_STACK=16777216`）、workspace all-target/all-feature check、
Clippy、fmt、structure audit gate（0 blockers）、cached diff check は終了コード0。
`just test-workspace` は非 CLI package 群を通過後、CLI
`spec_should_pass_check_fixtures_pass_after_refactor` の未編集 044 fixture で停止。
fixture 内の `GameEvent` 宣言が存在しないため nominal TypeId が未解決で、
今回の enum producer 回帰とは区別する。宣言を補った一時ソースの CLI check は
次に `Vec.pop_front()` の未接続 runtime intrinsic で停止した。Sol Max の調査では
同操作は `Option<T>` を返すだけでなく receiver の Vec を更新する必要があり、
AWBC `while`/`while let` にも別の CFG 実装不足がある。両者を型付きの独立した
変更として閉じ、044 の実行を検証する。元 fixture は未編集のまま。

## Vec.pop_front / AWBC condition-loop checkpoint — 2026-09-25

Supersedes: 直前 checkpoint の「044 は未宣言 `GameEvent` と `Vec.pop_front()` で停止」。
確認した `main`/`origin/main` は
`8b5503c376cf1650cefa786bcc03295ccfce2282` で一致し、working tree は clean。
044 fixture に inline `GameEvent` 宣言を補い、`compile --emit check` は
1 flow、warning/obligation とも 0 で通過した。CLI の集約 spec check も 044 を
通過し、次の 045 で停止した。

`Vec<T>.pop_front()` は checked local/parameter receiver の mutation fact を
静的 `VecPopFront` target に結び、RuntimePlan admission で `Vec<T> -> Option<T>`
を検証する。native は最も内側の束縛を更新し、sequence の Values/Dense/
TupleColumns/RecordColumns から先頭値を clone せずに移動する。AWBC schema-1
opcode、codec、verifier、VM は同じ更新を local/parameter register に行い、
保存復元は既存の sequence/register snapshot を使う。AWBC `while` と
`while let` は条件 header の再評価、pattern・guard・束縛、scope の終了、
`break`/`continue` の CFG に接続した。2要素を順に取り出す decoded AWBC
実行と一回目の処理後の snapshot/restore を確認した。

検証: focused core pop-front 4/4、AWBC loop 5/5、044 単独 CLI check、
workspace all-target/all-feature check、Clippy、fmt、structure audit gate
（0 blockers）、cached diff check は終了コード0。`RUST_MIN_STACK=16777216` の
`just test-workspace` は初回 AWBC opcode owner の期待順序で失敗し、定義順に
訂正して focused test 1/1 を確認。再実行では非 CLI のテストを通過し、CLI
`spec_should_pass_check_fixtures_pass_after_refactor` が 045
`dialogue_sugar_ruby_timed_cancel.arcw` の HIR required recovery で停止した。
全 recipe の合格ではない。

Sol Max と照合した残る境界: writable Vec receiver の維持契約には、既存の
assignment place が認める local/parameter を根にした直接 nominal field も含む。
この commit の実行対象は local/parameter のみなので、field place の typed
mutation は後続 cut で必須。index/deref/nested field は現行の checked writable
place 自体が受け入れない。`For` の `break`/`continue` は native/AWBC とも
未接続で、canonical `WhileNext`/`WhileLetNext` は sealed plan が拒否する。

## Inline timed cue producer checkpoint — 2026-09-26

確認した `main`/`origin/main` は
`171632a6dc5522525b23a4628e63d78511af153b` で一致する。
working tree は直接 field の `Vec.pop_front()` 移行中で dirty。
維持仕様が等価とする `with:` 内の `at(...): action()` を、同一物理行の
action でも typed callback block と Closure に投影した。後続の line-plan
`out` は別項目として保持し、空または壊れた inline body は正確な source span
で recovery する (`171632a6dc5522525b23a4628e63d78511af153b`)。
Syntax package は unit 701、public API 1、parser authority 3、doctest 2 が通過。
focused HIR projection と Syntax/HIR 対象 Clippy も終了コード0。

045 fixture 全体の check 合格は未達。旧 `alice.face`、expected type のない
`.Skipped`/`.Done`、CueHandle を返す callback に必要な Character manifest、
および `defer on cancelled` と Unit Flow の Type fact 欠落は、この parser
変更で解決したとは扱わない。これらを別の型付き consumer/fixture 境界として
調べ、timed cue と取消の受理証拠を維持する。

## Bare dialogue と attached plan の分類 checkpoint — 2026-09-26

確認した `main`/`origin/main` は
`c47a076fff725fda52aadfe8311121ab2e1fbc18` で一致する。
working tree は直接 field の `Vec.pop_front()` と 045 package fixture の
統合中で dirty。裸の `alice(...):` / bracket dialogue の文 owner が
aligned `with:` plan を含む場合、分類対象を plan 開始前の head に限定した。
従来は plan 内 `let actor = ...` の `=` を外側の Assignment と誤認し、
HIR required recovery になっていた。文の emission は full plan interval を
保ち、plan 内の Let と Out を source order で残す
(`c47a076fff725fda52aadfe8311121ab2e1fbc18`)。
focused parser/HIR 回帰、既存 nested cancel-body test、Syntax/HIR 対象 Clippy、
cached diff check は終了コード0。045 の profile 登録、timed cue、native/AWBC
実行、全体 fixture gate の合格はこの cut の証拠ではなく引き続き確認する。

## Writable nominal Vec field checkpoint — 2026-09-26

確認した `main`/`origin/main` は
`30430f9a01e354dfa2650d1615630e79a97622de` で一致する。
working tree は 045 CLI profile fixture とその受理側の作業中で dirty。
`Vec<T>.pop_front()` の writable place を local/parameter と直接 nominal
field にそろえた。Sema は exact project nominal schema と local-rooted field
selection を使い、HIR Path の二要素 field receiver を typed value として準備する。
compiler/RuntimePlan は sealed base local、nominal identity、field ID を投影・
検証し、native/AWBC は record 内の Vec を直接更新する。clone した一時値の
書き戻しは使わず、AWBC opcode と codec は version 1 のまま。
完全 project path と associated lookup の既存解決順を保つ。

検証: source-level compiler `pop_front` 5/5（native/decoded AWBC の反復取り出し、
local 回帰、nested/indexed receiver の拒否）、core nominal-field 2/2
（AWBC codec/verifier/VM/restore を含む）、変更 4 パッケージの all-target check、
workspace all-target/all-feature check、workspace Clippy、fmt、structure audit gate
（0 blockers）、cached diff check は終了コード0。`RUST_MIN_STACK=16777216` の
`just test-workspace` は非 CLI 群を通過後、045 fixture の
`AWF-EFX-007` (`dialogue.schedule` を選択 target が提供できない) で停止した。
全 recipe の合格ではない。Sol Max の照合では schedule/voice は engine 提供の
typed effect であり、adapter 固有の許可を fixture に足す問題ではない。

## 045 Dialogue profile 接続 checkpoint — 2026-09-26

確認した `main`/`origin/main` は
`75e347c2448cf5f157cceb795122c9af5ef96dcd` で一致する。working tree は
045 の CLI profile fixture と harness の統合中で dirty。次の 5 cut は個別に
commit/push 済み。

- `ce7af979391ac5ecdae9ccf78c3d926bc5bcea92`: target availability に
  engine 提供の control/dialogue/observation effect inventory を加え、adapter
  提供 effect と区別した。focused Sema test 1/1 と Sema all-target check が通過。
- `4434adf3dda0e10e42efabaea4fbebcae1cac559`: 重複していた
  `PresentationLifetime` opaque 登録を削除し、既存の閉じた enum を唯一の owner
  とした。focused Sema catalog と compiler projection test は各 1/1 通過。
- `fe27db1c56c7f5cf33c97162492d6b99fb07ad4d`: `at(anchor)(callback)`
  を二段の checked callable として保持し、terminal と exact prefix continuation
  を結合した。直接消費される prefix は Call 事実を残した fused disposition で
  二重実行を避ける。focused Sema line-plan test 1/1 通過。
- `a9873c59ffdac269d9fee943b39505d10f8f54ea`: profile loader は
  source の論理 Character と同じ公開 ID の visual manifest を読み込むとき、
  重複する外部 symbol だけを生成しない。manifest/look 登録は保持する。
  派生/明示/異なる ID の focused loader test 1/1 と HIR header test 1/1 通過。
- `75e347c2448cf5f157cceb795122c9af5ef96dcd`: `actor.look` の省略可能な
  `crossfade` を checked optional operand として保持し、省略時は RuntimePlan で
  typed 0ms Duration を生成。直接 line-plan の省略/120ms の compiler test 1/1、
  compiler/RuntimePlan all-target check が通過。

045 の実 profile check は display-name owner の接続を越えたが、timed callback の
closure body が親 line-plan の semantic scope からは見えず
`compiler.runtime_plan_lower` で停止した。後続の content handle 不在はその
line-plan 失敗から派生する。profile fixture 合格、native/decoded AWBC 実行、
workspace test recipe と最終 gate は未達であり、この checkpoint の合格証拠に
含めない。

## 045 callback / profile 受理と 049 境界 — 2026-09-26

確認した `main`/`origin/main` は
`ce3db75b4518548d12f83a3107ed3063bc8a9ad8` で一致し、working tree は clean。
Supersedes: 直前の 045 timed callback が semantic scope で停止するという観測。

- `cddddbfed15a733b1111e13fe2a02e3d55089ef5`: unresolved-dot の値 receiver が
  type-path-shaped expression になる経路でも、typed path segment の source span
  から外側の lexical local を HIR capture に記録する。source-index freeze と Sema
  の checked capture をそろえ、focused Sema test 1/1 が通過。
- `c00cf14ce992e2deffa4e84660ed5e4e7c06af73`: scheduled callback は閉じた
  closure scope/frame で RuntimePlan を下げ、個別 capture packet で実行する。
  activation 後に unscheduled task が使う copyable local だけを別の export として
  native/AWBC の reveal、後続 command、codec、verifier、snapshot に接続する。
  Core の export/custody、AWBC の codec/verifier/snapshot/lowering、native reveal の
  focused tests が通過。affine StageActorHandle は共有 export せず scheduled packet
  へ所有権を渡す。
- `a1edaf1f1e793aa4b5d39ad60f99961785008145`: CLI `check --profile` が
  profile topology の compilation context を使う。045 fixture と Character assets/
  profile sidecar を同梱し、temp fixture runner も companion assets をコピーする。
  045 の実 `arcw check --profile fixture` は 0 warning/obligation で通過。
- `ce3db75b4518548d12f83a3107ed3063bc8a9ad8`: closed environment enum
  の source type 名を exact nominal path として登録し、variant/domain の authority
  は既存の enum schema に保つ。`PresentationLifetime` と `DialogueVoice` の Match、
  既存 path 衝突拒否の focused tests が通過。

workspace all-target/all-feature check、workspace Clippy、fmt、staged diff check は
終了コード0。`RUST_MIN_STACK=16777216` での `just test-workspace` は非 CLI 群を
通過し、CLI 7 件中 6 件が通過。最後の fixture 集約は 045–048 を越え、
`049_await_context_option_boundaries.arcw` の
`hir.lower.project_publish` source-index 検証で停止した。Windows 既定 stack の
既知 `callable_origins_remain_distinct_across_a_branch` overflow も再現したが、
上記 stack 設定下では通過。049 の最小再現では直接の
`await ... with: pending` が HIR source-index で失敗し、plain await は HIR を通る。
049 と 045 の実 native/decoded AWBC 再生、および goal 全体の最終 gate は未達。

## 049 Await `with` 境界 checkpoint — 2026-09-26

`74930aabd4b93a3257f9b214bff844b07bedf9b2` を main へ push 済み。
Flow 文の Dialogue plan 区間検出が、同じ head にある `await ... with:` を
Dialogue 継続として先取りし、Await の `pending` body を式の範囲から除外していた。
top-level Await の `with` は Await 文境界に委ねるよう修正した。同行と次行の
`with:` の双方で、HIR Await が Pending branch、branch-local、nested body を
保持し、後続 Return と分離される focused test が通過。Syntax lib 702/702、
HIR lib 921 pass/8 ignored、fmt、cached diff check も通過した。

049 の fixture は HIR を越えた後、未宣言の `@asset:.bg.room` の Sema
値解決で停止する。fixture には asset/voice/state の宣言・manifest がなく、
entity reference の拒否自体は現行契約に沿う。型付き `Need` parameter への
単純な置換は Sema を越えるが、RuntimePlan Await が immediate host call を
要求して停止する。一般 extern capability call も manifest-owned host-call
contract がなく実行 plan を公開できない。fixture の受理内容と Await の
typed Need 実行モデルを合わせて閉じる必要がある。

個別の `compile --emit check` では 050、054、055 は通過。051 は Sema
expression type、052 は Sema fact/HIR family、053 は HIR arena coverage、
056 は extern capability host-call contract で停止した。これは 049 以降の
全体 fixture gate 合格を意味しない。

Sol Max との Await owner 照合: 現在の Core `RuntimeAwaitTargetSeed` は host
request template のみを持ち、RuntimePlan lowering は Await の直下にある
checked host call を要求する。`Need<T>` は plan の operational type だが
checked `RuntimeValue` には Need handle がない。AWBC は独立した NeedId
待機経路を持つ一方、native の `NeedWaiting` は再開を完結しない。
`Need<T>` local/parameter と pre-Await `.context` を受理するには、typed Need
handle、host-request/existing-handle の Await source、context frame の一回だけの
評価と失敗時付与、両 engine の待機・復帰、codec/verifier/snapshot を同じ契約で
接続する必要がある。049 の fixture だけを弱めてこの境界の完了とはしない。
また 051 の停止 owner は `[1i32, 2i32, 3i32]` で、Sema の compact numeric
sequence が期待 `Array<i32, 3>` にかかわらず `Vec<i32>` を生成する。
052 の `text_key=@super.super.intro_text` は HIR の text-key coordinate が
絶対 `text.*` のみを解決する境界で止まる。これらは別の acceptance cluster。

## 053 rich-text / typed Need checkpoint — 2026-09-26

確認した `main`/`origin/main` は
`8ef222574b7f32030be0111617d4affbddc665fe` で一致する。working tree は
053 fixture と対応する維持仕様例の修正中で dirty。次の三 cut を個別に push 済み。

- `7a10931d8548738ecef572c8d2e9793b5fc541fe`: record field の `:` 後に
  より深い indent の改行値を許し、dedent では欠損値として回復する。053 の
  multiline `Transform2D.translate_y` を HIR に渡せるようにした。Syntax lib
  703/703、Syntax Clippy、fmt、cached diff check が通過。
- `c41081aa56f5c003b029949b2031c94153ee4d48`: Core の Need handle を
  `RuntimeValue::Need(NeedId)` として保持し、AWBC の String 代用を拒否する。
  affine ownership、snapshot/restore、外部利用側、direct suspension の入力を
  同じ型へ移行。Core の Need focused tests と `direct_suspension` 8/8、
  workspace all-target/all-feature check と Clippy、fmt、cached diff check が通過。
  これは Await の既存 Need source / native 復帰を完成させた証拠ではない。
- `8ef222574b7f32030be0111617d4affbddc665fe`: Fx sampler の body を
  accepted standard `Transform2D` の型付き record として検証し、別 nominal と
  project shadowing を拒否。`FxSampleContext` の closure pattern を seed。
  Sema focused test 2/2、crate check/Clippy、workspace all-target/all-feature
  check/Clippy、fmt、cached diff check が通過。

`RUST_MIN_STACK=16777216` の `just test-workspace` は上記 Core の旧 String
代用を使う `direct_suspension` 8件で一度停止した。テスト入力も型付き Need へ
移行した再実行では非 CLI 群が通過し、CLI 7件中6件が通過、既知の未編集 049
fixture の `sema.final_analysis` value resolution で停止した。レシピ全体の
合格ではない。053 は HIR を越えたが、非空 `sample = |ctx| Transform2D { ... }`
の式・call が通常の checked-expression facts を持たず Fx sealer で
`InvalidBody` となる。Fx body に並列の通常 fact を公開せず、既存の
`CheckedFxDefinitionCatalog` と value-program 命令/verifier に、一時的な
型付き sampler 式証拠を接続することが次の直接作業。維持仕様と fixture の
`Fx.text(weight = .strong)` は現在の numeric weight 契約と異なるため、
`700` への訂正が working tree に残る。053、049、および goal 全体の最終
受理は未達。

## 053 callable edge / Fx sampler checkpoint — 2026-09-26

Supersedes: 直前の「053 rich-text / typed Need checkpoint」の working-tree
状態と Fx sampler の次作業判断。確認した `main`/`origin/main` は
`10936d084feefcd93727bf8695000a948699c581` で一致し、working tree は
053 fixture の `.arcw`、companion profile と assets の未統合変更で dirty。

`d150653b13007b45e4390f2420bf9461679c7e6e` で Fx body の式を一時的な
型付き sampler 値として検証し、通常の final-analysis facts を並列公開せず
`sin(ctx.time * speed + ctx.ordinal_phase()) * amplitude` を受理した。
Sema の `project_fx_` focused tests 11/11、workspace check/Clippy、
structure gate と fmt が通過した。`just test-workspace` は非 CLI 群と CLI
7件中6件が通過し、未完の 049 fixture で停止した。Fx sealer
`fx_definition.rs` の SIZE001 は、Fx 定義の型検証と value-program 構築を
同じ owner に保持する実装（測定時 2507 行、基底 2282 行）として review。
独立した authority や依存逆転は見つからず、LOC だけを減らす分割は行わない。

`10936d084feefcd93727bf8695000a948699c581` は dialogue の `id` と
`text_key` の意味論専用引数を checked child edge に結び、rest spread の
whole-container / fixed literal element source も HIR と照合する。
Sema dialogue edge と TextProxy の focused tests、compiler rest-spread
focused test、workspace all-target/all-feature check/Clippy が通過。
`RUST_MIN_STACK=16777216` の `just test-workspace` は非 CLI 群を通過し、
CLI の未編集 049 fixture の `sema.final_analysis` で 6/7 の後に停止した。
stack 指定なしの同レシピは compiler `callable_execution` で stack overflow
したため、全体 gate 合格とは記録しない。

053 の最初の dialogue application は companion manifest の `smile` 登録と
character look 型では停止していない。Sol Max の最小再現で、
`InlineFailure.fallback("?")` の返り型 `InlineFailure` と dialogue schema の
`inline_error` 期待型 `InlineFailurePolicy` が一致しないと確認した。維持仕様は
前者を canonical value とする。schema 訂正後も compiler の opaque producer
と実行値の接続が必要であり、053 fixture の受理は未達。049 と goal 全体も未達。

## 053 dialogue operation / policy checkpoint — 2026-09-26

Supersedes: 直前 checkpoint の `InlineFailure` schema 不一致についての現在状態。
確認した `main`/`origin/main` は
`331385ab3a3c0a3ab0f8f24d581408d109051e40` で一致する。working tree
には未完の 053 fixture `.arcw` と companion TOML/assets のみが残る。

同 SHA で、選択済み call の意味論専用 child と accepted type owner を HIR/Sema
から compiler の到達性へ接続した。CharacterDialogue の policy graph と
`InlineFailure.fallback(String)` を compiler/runtime-plan/native/AWBC まで接続し、
同じ String を使う二つの dialogue の native/AWBC 一致と AWBC codec 往復を
検証した。dialogue point action は evaluated effect または選択済み Unit call
として保持し、正確な application digest・effects・captures と callback role を
runtime-plan に渡す。通常 call の結果は内部実行に保持する。Sema の拒否 call は
tooling evidence として残し、位置引数から open effect field identity を
捏造しないことを確認した。

focused Sema/HIR/compiler/Core/runtime-plan と native/AWBC tests、workspace
all-target/all-feature check、workspace Clippy、fmt、cached diff check、
structure audit は通過。structure audit は 2622 files / 96 packages /
331 review triggers / 0 blocking violations。今回増えた `final_flow.rs` は
選択済み dialogue action の実行 lowering、`semantic_facts.rs` は同一世代の
checked fact join、`final_expr.rs` は値の lowering、`awbc_lower/expr.rs` は
AWBC 命令 lowering をそれぞれ保持し、並列 authority の増設は認めない。
`RUST_MIN_STACK=16777216` の `just test-workspace` は非 CLI 群を通過し、
CLI 7 件中 6 件が通過。未編集の 049 fixture の `sema.final_analysis` value
resolution で停止したためレシピ全体は失敗扱いとする。

053 fixture 全体は未受理。`InlineFailure.discard` の完全修飾値は HIR Select
が type 名を値扱いする境界で止まり、`fmt(...)` は `DisplayText` trait を
返して runtime Content にならず、`rgb("#ffffff")` は compile-time Color に
留まる。Sol Max と照合した次の契約は、`fmt` の結果を型付き Content とし、
formatted run を Content / text model / native / AWBC に接続すること、Color
は単一の sRGB RGBA8 runtime value として literal `rgb` を検証済み定数から
残余化すること。これらと 049、および goal 全体の最終受理は未達。

## Qualified enum value checkpoint — 2026-09-26

Supersedes: 直前 checkpoint の「完全修飾 `InlineFailure.discard` は HIR Select
で止まる」という現在状態。`main`/`origin/main` の確認済み SHA は
`05206d5f763072ca924120be0f08b08e3d2a85ff`。working tree には
未完の Color runtime 移行と 053 fixture/companion が残る。

同 SHA で、`Type.Case` の Select を accepted environment/project enum の
静的 qualifier として解決する。HIR の選択済み semantic/runtime 両グラフは
qualifier を実行 child にせず、case expression と正確な owner/case 照合を保持。
`InlineFailure.discard`、`InlineFallback.value_plain`、project enum の focused
Sema regression 1/1、HIR/Sema check、compiler all-target check、fmt と cached
diff check は通過。native/AWBC での完全修飾値の実行と Color/053/049 の全体
fixture gate はまだ未検証または未達。

## Runtime Color checkpoint — 2026-09-26

Supersedes: 直前 checkpoint の「Color は compile-time に留まる」という現在状態。
`71c3d69105a8e251cc21b279ac9ace50f968e7cd` を `main` へ push 済み。
チェック済み `rgb` の RGBA8 を単一の `RuntimeColor` / `RuntimeValue::Color`
として実行側へ渡す。選択済み builtin application の digest を保持する
`ResidualValue` を HIR/Sema/runtime-plan/compiler に通し、通常関数の引数・
返り値、native、AWBC の型・定数・codec・verifier・snapshot・admission を
同じ Color に接続した。短い `#rgb` / `#rgba` と長い `#rrggbb` /
`#rrggbbaa` は一つの parser で RGBA8 に正規化し、不正値を拒否する。

focused compiler native/AWBC Color 往復 2/2、dialogue point Color 1/1、
Core Color tests 3/3、Sema parser test、workspace all-target/all-feature
check、workspace Clippy、fmt、cached diff check が通過。structure audit は
2624 files / 96 packages / 331 review triggers / 0 blocking violations。
増えた `semantic_facts.rs` は同一世代の残余値と選択済み application の照合、
`final_expr.rs` はその値の lowering、`compiler/lower.rs` は sema の正確な
Color から runtime facts への投影を各 owner 内で行い、並列 Color 型や
fallback resolver を設けていない。

`RUST_MIN_STACK=16777216` の `just test-workspace` は非 CLI 群と CLI の
先行 6 件が通過した。最後の CLI fixture 群は既知の未完 049
`049_await_context_option_boundaries.arcw` の ExprId slot 23 で
`sema.final_analysis` value resolution に失敗し、レシピ全体は未合格。
053 の `fmt` Content 化と 049 fixture、goal 全体の受理は未達。

## fmt checked-call / ArcError owner checkpoint — 2026-09-26

Supersedes: 直前 checkpoint の「`fmt` は `DisplayText` を返す」と「049 は
fixture の参照不足だけ」という現在状態。`main` に以下を push 済み。

- `e5dbf2cfa3164e62df3c9b57c9f4ce5b68367f7c`: 標準 `fmt` を受理済み
  `DialogueContent` 型へ変更し、文書化された named options と policy alias の
  排他を選択済み `CheckedCallApplication::format_call()` で一度だけ確定。
  通常 call と dialogue 内の call は同じ fact を使う。結果が exact Content
  の補間は `ContentValue` として seal し、compiler は Content slot /
  `ContentInsert` へ投影する。focused Sema `fmt_` 7/7、Sema/Compiler
  all-target check、fmt と cached diff check が通過した。
- `aaab0ed0271177e06ce64fa48d07888c3ef05cfb`: 既存 `std.arc_error`
  owner の version 1 payload に型付き Content message、元の error value、
  structured trace と snapshot admission を追加。`SourceCoordinate` は元の
  document revision と byte range を保存し、UTF-8 検証済み
  `SourceAnchor` を復元時に偽造しない。Core ArcError 3/3 と source coordinate
  2/2 が通過した。

両 commit を含む workspace all-target/all-feature check と Clippy、fmt、
diff check は warning ありで通過。structure audit は 2625 files / 96
packages / 333 review triggers / 0 blocking violations。新規
`value/arc_error.rs` の SIZE001/TEST001 は、version 1 payload の typed
encode/decode・limits・nested cause 検証を単一 owner に保持した 1291 行
（うち embedded tests 98 行）として review した。source layout 自体は
blocking violation ではなく、責務が分かれるまで行数だけの分割はしない。

049 は未宣言 asset/voice を fixture 引数へ移すと、`Result` / `Option`
`.context` の shared call 解決・実行不在が露出する。維持仕様上は
`Result/Option.context` の `ArcError` 型・native/AWBC 実行を先に閉じ、
pre-await `Need<Result<T,E>>.context` は Ready payload だけを変換しつつ
Pending/cancel/save/restore を保つ派生 Need が必要。053 は `fmt` の
recoverable formatted run、Core/native/AWBC の共通 formatter、動的 policy、
一般の `DisplayText` 適合判定が未実装。両 fixture は作業ツリーに残り、
この時点の `just test-workspace` 再実行と goal 全体の受理は未達。

## Formatted Content lower-layer checkpoint — 2026-09-26

Supersedes: 直前 checkpoint の「formatted run を Content / text model に
接続すること」という未実装状態のうち、Core と text-model の契約部分。
Inspected `main`/`origin/main` SHA:
`88a81e15f7376640092fe87f43b3a3c3109aae91`。working tree には 049 の
Sema/compiler/intrinsic と 049/053 fixture の未完差分が残る。

同 SHA で version 1 の Content envelope に型付き `Formatted` binding を追加。
成功値は Text または nested Content と任意の Runtime Color、失敗値は
理由と任意の事前計算済み plain text を保持する。`inherit` / `on_error` /
`fallback` / `discard` の閉じた選択を Core/AWBC/runtime-plan に通し、
text-model が source/value source と policy を解釈する。Core は nested
Content の artifact/depth、`on_error` の runtime graph budget、codec と
verifier を検査する。検証済み一スロット manifest から plain-text Content
を作る constructor は 049 の ArcError message 用にも利用できる。

Core focused tests 2/2、dialogue policy decoder 1/1、text-model focused
tests 5/5、変更した Core/dialogue/text-model/render-text の all-target check と
Clippy、workspace all-target/all-feature check、`cargo fmt --all -- --check`、
staged diff check が通過した。Clippy と workspace check は既存 warning あり。
workspace Clippy と `just test-workspace` はこの cut 後に未実行。
`fmt` の生成 template、native/AWBC の formatted run、DisplayText witness と
recoverable operand 評価は未実装であり、この commit は 053 fixture の受理を
意味しない。049 の context 実行と pre-await Need transform も未達。

構造 review は `05d4450e9e694eab785d26a331e8ba55f872b059` の dirty tree で
`just structure-audit-gate` を実行し、2625 files / 96 packages / 333 review
triggers / 0 blocking violations。`arcweft-core/src/value/opaque.rs` は
119415 bytes / 3066 physical LOC（base 2518、増分 548、embedded tests 799）。
増分は既存 version 1 Content envelope の `Formatted` payload、codec、admission
と同じ owner に属する。既存の opaque role/handle 群はこの cut で状態や
依存を増やさず、新たな I/O・逆向き依存・二重 authority はない。現段階では
Content の encode/decode/budget と一緒に保ち、行数だけの分割はしない。
`arcweft-text-model/src/content.rs` は 73152 bytes / 1939 physical LOC
（base 1431、増分 508、embedded tests 411）。増分は既存 template/catalog と
materializer が解釈する Formatted policy と検証に限られ、Core の wire
payload を再定義せず、dialogue policy への既存方向の依存に収まる。
template と materializer の責務は同じ Content contract の生成と消費であり、
独立 state や重複 traversal を増やしていないため、ここも現時点では保持する。

## fmt runtime decision under implementation — 2026-09-26

Inspected `main`/`origin/main` SHA:
`ebd4803a0b4fc14de9a6b248bf99333e67c7ff86`。049 の context と 053 fixture は
引き続き dirty。これは実装受理ではなく、Sol Max と現行 owner を照合した
次の実装境界の記録である。

選択済み `CheckedFmtCall` を唯一の引数・policy authority とし、通常式の
`fmt(...)` と dialogue 内 `fmt(...)` を同じ可到達 call inventory に載せる。
各 call に単一 `Formatted` slot の canonical Content template を一つ発行し、
slot の source/value-source は checked source range から有界に保持する。
`ExprId` の debug 表記を fallback text に使わない。operand は C1 の source
order で評価し、失敗を Content 構築前に型付き result として回収する。
native では同期的 value-site、AWBC では dialogue terminator より前に
operand が評価されるため、完成後の `MakeDialogueContent` だけで例外を
捕捉しても契約を満たさない。純粋かつ非中断の DisplayText 呼び出しだけを
許し、言語式/formatter の回復可能失敗と artifact/schema/budget/cancel の
致命的失敗を分離する。

現行 `TypeKind::DisplayText` は trait 適合 witness ではなく、plain 補間も
exact Content 以外を十分に選別していない。Sema が builtin または選択済み
project impl の適合 fact を seal し、compiler/runtime-plan へ渡す。
builtin のみの fixture 受理を一般の DisplayText 実装完了と呼ばない。
この段階では template 生成、実行、witness の検証は未実行・未達。

## Result/Option context runtime checkpoint — 2026-09-26

`main` に `8a540f312759e2a9e976f627a42003ff61f09a2a` を push した。
`Result.context` / `Option.context` と lazy callback variant は、選択済み
call fact から exact `ArcError` を返し、失敗時だけ message を Content に
変換する。native と Product AWBC は同じ Core constructor を使い、原因値と
構造化 frame を保持する。RuntimePlan は到達する context call にだけ
canonical 一スロット Content template を発行し、bundle は AWBC 行と
text-model 本文の完全一致を確認してから実行用 proof を渡す。standalone
実行では String message に proof がなければ失敗し、既存 Content message は
直接受理する。schema/codec/verifier と bundle/session replacement 経路も
同じ契約へ移した。

Core native/Product AWBC の context focused tests 10/10、Sema focused
tests 2/2、bundle canonical admission 1/1、変更 crate の all-target check、
workspace all-target/all-feature check と Clippy、`cargo fmt --all -- --check`、
cached diff check が通過した。workspace check/Clippy は warning あり。
`just structure-audit-gate` は 2628 files / 96 packages / 333 review triggers /
0 blocking violations。今回の大きな Core 変更は既存の ArcError/Content
owner と専用 test child modules に置き、別の authority や逆向き依存を
追加していない。

049 fixture の focused CLI は `Await operand ExprId(slot 39) is not a typed
host call` で停止した。`asset.image` / `voice.load` は registered
`Need<Result<...>>` producer だが、現行 Await target は Host request のみ
である。必要な typed producer start / Need handle / 共通 Await と両 engine の
待機・復帰は `c0936def8915ff3614a401fbad3e7fd42ecb604b` の
[Need/Await 境界記録](2026-09-26-need-await-producer-boundary.md) に整理した。
receiver の evaluated-child traversal、049 の fixture/CLI test、および 053 の
fixture/companion は次の cut のため未commitで保持した。ArcError frame の
実 source coordinate、pre-Await `Need<Result<T,E>>.context`、049/053 の
実 native/decoded AWBC 受理、`just test-workspace` と goal 全体の最終 gate は
未達。`just test-workspace` は既知の focused 049 失敗があるため、この cut
では再実行していない。

## Typed Need producer schema checkpoint — 2026-09-26

`main` に `a429eddd108d8fb581e15e8aeeec508d9cfb457f` を push した。
Core に typed `NeedProducerOperation::AssetLoad` と image/voice kind を
追加し、Sema の選択済み `CallableValidator::NeedProducer` role に operation
と `TaskPolicy` を保持する。`asset.image` と `voice.load` だけを明示登録し、
汎用の `Need<T>` 戻り値や callable 名から producer を推測しない。payload
`T` は選択済みの具体化された `Need<T>` result の owner とし、role 内に
重複保存しない。validator digest は operation/kind/policy を含む。

Sema の登録/digest focused tests 2/2、Core/Sema all-target check、workspace
all-target/all-feature check、変更 crate all-target Clippy、fmt、cached diff
check が通過した。check/Clippy は warning あり。compiler/runtime-plan の
selected producer fact、Need handle の生成・待機、native/AWBC 実行と復元は
この SHA では未実装であり、049 fixture は引き続き未受理。receiver traversal、
049/053 fixture など次の cut の差分は保持している。

## Selected Need producer fact checkpoint — 2026-09-26

`main` に `8d755d99f8f4e6b927606048a8514a81d85aa162` を push した。
compiler は選択済み `NeedProducer` role と Sema admission を checked call の
source-order 引数へ照合し、runtime-plan は operation/policy、具体化済み
`Need<T>` result、admission を単一の `RuntimeResolvedNeedProducer` fact に
保持する。call expression type との完全一致、引数 coordinate/type、Host
dispatch の manifest contract と Suspend mode を検証し、普通の extern
`Need<T>` Host call に producer role を捏造しない。

標準 producer projection と manifest-backed Host の focused compiler tests
各 1/1、compiler/runtime-plan all-target check、workspace all-target/all-feature
check、変更 crate all-target Clippy、fmt、cached diff check が通過した。
check/Clippy は warning あり。producer start、Need handle、Await の実行、
AWBC codec/snapshot と 049 fixture 受理はまだこの SHA に含まれない。

## AWBC Need Await typed suspension checkpoint — 2026-09-26

`main` に `92a9b91fbdf9bbd35574b5c9362ab47bb701755a` を push した。
既存の外部 Need Await に対し、suspended target と save DTO が発生元の
register と exact item `T` を保持する。復元時に register/type/NeedId の
一致を検証し、main/deferred Product 経路は `Ready` payload が `T` に
入ることを確認してから AwaitReady を発行・束縛する。verifier も Await
binding pattern を `Need<T>` の `T` で検証する。

不正な Ready focused test 1/1、save DTO round-trip/型改ざん 1/1、
direct suspension 8/8、Core all-target check、workspace all-target/all-feature
check、Core all-target Clippy、fmt と cached diff check が通過した。
check/Clippy は warning あり。これは外部 Need の既存経路の安全性 cut で、
StartNeed producer registry・task event link・Product restore の移行と
049 fixture の実行受理はまだ含まれない。

## Loaded audio handle type checkpoint — 2026-09-27

`main` に `88f89da1622f8a724747358341ed5d48b6a3dd50` を push した。
`voice.load` の成功型を、line の再生中 lease `VoiceHandle` から標準 opaque
資源型 `AudioHandle` (`std.audio_handle`) に分けた。`VoiceHandle` の
line 操作と affine ownership は変更していない。選択済み標準 callable の
`Need<Result<AudioHandle, VoiceError>>` 型、accepted nominal、Core opaque
owner、関連する言語・音声・例示文書を揃えた。実 bundle 資源を検証して
`AudioHandle` を発行する host adapter はこの commit に含まれず、Need
producer の統合差分として作業中である。

Core/Sema focused tests 5 件、両 crate の all-target/all-feature check と
Clippy、変更 Rust の rustfmt、cached diff check が通過した。check/Clippy は
既存 warning あり。shared runtime の workspace gate と 049 fixture は、
未統合の producer/continuation 差分が残るため、この checkpoint の受理証拠
としては未実行・未達である。push 後の working tree は Need producer の
native/AWBC/driver/host/CLI 実装と 049/053 fixture が dirty のままであり、
この型訂正だけを hunk 単位で stage した。

## Typed Need / bundle asset 統合 checkpoint — 2026-09-27

`main` に `45d9d2251b644c0906136b1a2497b8fb15371b34` を push した。
選択済み manifest の `Need<T>` を typed producer plan として native/AWBC へ接続し、
task publication と Await の復帰、bundle asset の内容 identity と
Image/Audio handle、generation 別の asset context、save/restore の検証まで
統合した。標準 FS の manifest-backed Custom request は native adapter が
契約と引数を照合して処理し、adapter が拒否した要求は終端失敗として公開する。
`Need` の停止モードは effect row ではなく manifest と一致する callable の
最終戻り型から決める。producer lowering は通常 host-call lowering に先行する。

変更後の workspace all-target/all-feature check と Clippy、fmt、
structure-audit-gate（97 packages、blocking violation 0）、cached diff check は
通過した。Clippy/check には既存 warning がある。focused validation は
compiler lib 115/115、native task 20/20、Sema の新規 collection 4/4、
Core の AWBC role/verifier 回帰、CLI run fixture 全件、native headless asset
実行、runtime-host/driver/Core の関連 test が通過した。
`just test-workspace` は CLI linker の PDB 上限で停止したため、
`cargo clean -p arcweft-cli` 後に `CARGO_PROFILE_TEST_DEBUG=0` と
`CARGO_INCREMENTAL=0` で CLI の各 command を実行した。fixture test は
7/8 で、051 の `CapacityMethod` runtime intrinsic 不足により未合格。
053 companion manifest/assets の直接 check も `fmt` Content の typed runtime
lowering 不足で未合格。これらを全体 gate の成功とは扱わない。

push 後の working tree は 053 fixture `.arcw` と companion TOML/assets のみ
dirty。051 の容量 method、053 の一般 formatter 実行、公開 CLI の typed
`Ref<Asset>` 引数、その他の goal 受入条件は引き続き必須の残件である。

## CapacityMethod 型契約 checkpoint — 2026-09-27

`main` に `810655d4198eac3520b59e42e613f92fd1683f8d` を push した。
`with_capacity`、`reserve`、`shrink_to` の引数を一つの checked `usize` に
閉じ、`Vec<T>.push` は `T` を要求する。`push`/`pop`/`pop_front` は Vec に
限定し、仕様にない String/Bytes の `push` 受理、任意個・未型付けの容量引数、
到達不能な `collect` schema を除いた。直接構築する CapacityMethod identity
も同じ family/arity を検証する。LSP の旧三引数署名期待と維持仕様を更新し、
`&mut self` は現行の local/直接 nominal field place を要求し、binding の
`mut` 注記を別物として扱う現行契約を明記した。

Sema の容量 method focused test 3/3、署名 query 1/1、LSP native/署名
projection parity 1/1、fmt、cached diff check が通過した。この commit は
型契約の訂正であり、runtime の constructor/容量 hint/Vec push・pop は
未統合。051 fixture の runtime 受理や workspace gate の合格は主張しない。
053 fixture は引き続き別の active diff である。

## CapacityMethod constructor / 容量 hint 実行 checkpoint — 2026-09-27

`main` に `b6e42b6cc86a39e9c8871de499d1d8f6ae2816aa` を push した。
選択済み CapacityMethod を Vec/String/Bytes の typed runtime intrinsic へ
投影し、`with_capacity` は空の該当 collection を返し、`reserve`、
`shrink_to`、`shrink` は checked `usize` と writable receiver を検証して
Unit を返す。容量値そのものは言語から観測できない。native/pure と AWBC
product host が同じ評価を利用し、AWBC verifier は family、arity、型、
effect row を検証する。Unit の式文は runtime-plan が discard binding として
一度だけ評価する。

Vec/String/Bytes の canonical source constructor と hint の native/AWBC
6/6、compiler callable execution 102/102、runtime-plan lib 89/89、
Core 容量 focused 3/3、変更 crate の all-target/all-feature check、fmt、
cached diff check が通過した。check/test に既存 warning がある。
`String::with_capacity` / `Bytes::with_capacity` は受理済み source
契約外であり、canonical form は `String.with_capacity` /
`Bytes.with_capacity`。Vec `push`/`pop` の実更新、051 fixture、workspace
test gate はこの SHA では未達・未実施である。push 後の dirty は Vec
更新の作業差分と 053 fixture companion で、別の goal は含まれない。

## Vec 更新と 051 fixture checkpoint — 2026-09-27

`main` に `f091d95086ddc5144ef79c62f5f68aa299016b0e` を push した。
選択済み `Vec.push(T)` / `Vec.pop()` の checked local または直接 nominal
field place を runtime-plan の単一 mutation fact とし、引数を一度評価して
更新する。Core native/pure と AWBC の専用 `VecPush` / `VecPop` 命令が同じ
Vec sequence 値を更新する。AWBC codec/verifier/VM は値型、place、宛先、
定長 Array の repeat 長を検証する。051 で露出した compact numeric
`Array<T, N>` literal の checked 要素型・長さ投影も閉じ、runtime-codegen
と accelerator の新 opcode/expr 利用側を更新した。

CLI `compile --emit check` は 051 fixture を 1 flow、warning 0、obligation
0 で受理した。native/AWBC callable execution 110/110、Core lib 687/687、
runtime-plan lib 90/90、workspace all-target/all-feature check、AWBC の
codec/verifier/VM focused test と forged fixed-Array 長不一致拒否、fmt、
cached diff check が通過した。check/test は既存 warning あり。Core と
runtime-plan の focused Clippy は Array 延長前に warning ありで通過したが、
この SHA 全体に対する再実行はしていない。workspace test gate も未実施。
push 後の dirty は 053 formatter fixture と companion TOML/assets のみ。
053 の typed formatter と公開 CLI typed `Ref<Asset>`、後続工程の受入条件は
引き続き未完である。

## 053 formatter 接続の設計判断 — 2026-09-27

`main` の `de6562ee2224bf1dc98f7c72cb800270fcde7ec1` を確認した。
053 の companion manifest/assets を一時的に `target/` へ配置した直接
`arcw check` は、選択済み `fmt` call に typed runtime formatter lowering が
ないため失敗した。053 fixture 本体と companion は未統合差分として保持する。

Sema の `CheckedFmtCall` を primary value、選択済み named option、failure
policy と source-order の authority とする。表示可能型の witness を同じ
Sema 境界で封印し、通常の `#[expr]` 補間にも適用する。compiler の `Format`
target は安定した checked call 座標と typed operand role を保持し、call
projection の段階では dense template ID を持たない。project/closure instance
の発見と materialization 後、生存する formatter call を完全な実行 scope と
安定座標で整列し、既存 dialogue fragment の後に一スロット
`FormattedInsert` template を割り当てる。plain-text context template は
その後に置く。同じ identity/digest を RuntimePlan manifest、text-model
catalog、AWBC へ登録する。

Core の `FormatContent` は入力式を authored source order で一度ずつ評価し、
回復可能な式・formatter failure を既存の Formatted outcome/policy に保持して
artifact-bound Content を返す。native/pure と AWBC はこの評価契約を共有し、
AWBC では値 register が完成する前に回復可能 failure を扱える verified な
子式境界が必要である。artifact/schema/budget/cancel の失敗は fallback に
変換しない。ここは設計決定であり、この時点で 053 の実行受理・native/AWBC
parity・workspace test gate は未達。project `DisplayText` conformance は
現行実装に選択証拠がなく、同じ工程で接続が必要な残件である。

## 053 checked witness / template 投影 checkpoint — 2026-09-27

`main` に `26df8123ab550a12aeba6e11a743a956c843062b` を push した。
Sema は通常補間と `fmt` の表示可能性を型付き witness で保持し、exact
Content、Core が表示できる scalar、`fmt` の `Option<scalar>` を区別する。
generic 関数内の開いた型は保留し、閉じた executable instance の具体型で
再検証する。既に却下された call は補間の source 証拠を保持したまま
`witness: None` として既存の拒否診断を失わない。

compiler は選択済み `CheckedFmtCall` と authored source を Format target に
投影し、global/project-function/closure の生存 call を完全な lexical scope と
安定座標で収集する。dialogue fragment 後に dense ID を割り当てた canonical
`FormattedInsert` template を RuntimePlan manifest と text-model catalog
の双方へ登録する。RuntimePlan は選択 call と template の一対一対応を検査する。

Sema `content_callables` 35/35、compiler lib 115/115、runtime-plan lib
90/90、compiler/runtime-plan all-target Clippy、`cargo fmt --all`、staged
diff check が通過した。push 済み HEAD の workspace all-target/all-feature
check も既存 warning ありで通過した。Clippy は既存 warning あり。compiler tests の
generic dialogue 3 回帰は、開いた generic を早期拒否していた問題を
閉じた instance での検査に移して解消した。053 fixture はまだ `fmt`
実行式の Core/native/AWBC 評価がないため未受理。project `DisplayText`
conformance と formatter の動的 policy/style/locale 等も未完である。

## 053 typed fault 前提 cut — 2026-09-27

`d1093c1fa7c2a6d20976a67de1b1c05b337a9af8` は、選択済み `fmt`
call の primary operand が `CheckedFmtCall` の source と一致し、閉じた
runtime type が Sema の display witness に適合することを RuntimePlan
semantic-fact transaction で検証する。runtime-plan 90/90、compiler
116/116、runtime-plan all-target Clippy と cached diff check が通過した。

`37671e5462b44b9bb990a739b1e515f690800fa0` は整数のゼロ除算を
panic から `RuntimeEvalError::RecoverableExpression(DivisionByZero)` に
変更した。signed/unsigned 全幅、pure scalar、AOT i64、AWBC `Binary`
は同じ typed error を通す。float の IEEE 除算と signed 最小値 / -1
の wrapping 結果は維持する。Core lib 690/690、workspace
all-target/all-feature check、Core all-target Clippy、fmt と cached diff
check が既存 warning ありで通過した。

この cut は formatter の recoverable 子式境界の前提であり、053 fixture の
実行受理ではない。`evaluate_runtime_call` の String 値への失敗変換、
AWBC VM の残る `VmError::Runtime(String)` と nested helper の trap/
budget/cancel の文字列化、`Format` 専用 expression と verified AWBC
continuation は引き続き未実装である。

`332ed89175f0399ca08e1391126e9d8389fcf710` では native と Product
AWBC が共有する `evaluate_runtime_call` を typed `Result` に変えた。
組み込み iterator/Option/index/string/float、math backend、外部 callable
の失敗を String 成功値へ変換せず、未受理 backend も typed error へ返す。
`Option.unwrap(None)` の回帰テスト、Core lib 691/691、workspace
all-target/all-feature check、Core all-target Clippy、fmt、cached diff
check が既存 warning ありで通過した。nested AWBC pure-helper の
trap/budget/cancel の文字列化と `Format` 専用式はまだ未完である。

`a83dfeed66bc128c41586c504f89bca90d4049ca` は nested AWBC pure
helper の非値終了を `VmNestedPureExit` へ移し、trap code・budget safe point・
cancel・suspend を in-memory の型付き値で保持する。Core lib 691/691、
workspace all-target/all-feature check、Core all-target Clippy、fmt、cached
diff check は既存 warning ありで通過した。formatter が受け取る回復可能な
`VmError::Evaluation` とは区別できるが、残る `VmError::Runtime(String)` の
message-based trap 分類と protected formatter operand は未完である。

`b1cf3a9f1750aeb4b45cac903c062cdae445633c` は selected expression
failure を `Option.unwrap(None)` と source index bounds に拡張する。
canonical な非 Option variant を `unwrap` に渡した場合は panic せず fatal
な型エラー、index の target/引数形の不一致も fatal のままとした。Core lib
692/692、workspace all-target/all-feature check、Core all-target Clippy、
fmt、cached diff check が既存 warning ありで通過した。formatter style 等の
失敗はこの expression error に混ぜず、専用の formatted outcome で扱う。

`dfcb8f08f30b6ccb2e080c053630af36866beb37` は AWBC の失敗由来を
型付きで保持する。compact pure helper の `RuntimeEvalError` を文字列化せず
`VmError::Evaluation` へ渡し、実際の pattern mismatch と dynamic Flow target
lookup を専用 error にし、`Runtime(String)` の部分文字列から trap code を
推定する経路を削除した。nested pure helper と trait method の非値 exit も
共通の in-memory `VmNestedCallExit` へ投影する。変更後の Core lib 692/692、
Core all-target check と all-feature Clippy correctness、rustfmt、cached diff
check が既存 warning ありで通過した。独立 fiber に入る既存 helper/trait の
budget と中断の同一 fiber 継続は未完であり、formatter の protected operand
実行受理をこの commit だけで主張しない。

## 053 typed formatter substrate — 2026-09-27

`b13f0b8b7f55949f16bc460f558c13e29458b0c3` は既存 `main` の
`f8ddcd2aef1ee47016cbf7e283f41ccd0552e2d2` を起点に、selected `fmt`
call を exact scoped template と source-ordered typed operand に投影し、Core
`FormatContent` の builder/Engine/pure evaluator と AWBC v1 opcode/verifier/
codec/VM に接続した。AWBC は同一 fiber の各 operand 継続で recoverable な
式失敗だけを保持し、後続 operand を評価する。途中状態は snapshot、restore
validation と retained-value visitor に含めた。Core の共通 formatter outcome
builder が native/pure/AWBC の結果と失敗 policy を作る。053 check fixture の
manifest/assets も同じ commit に入れた。push 後の working tree は clean。

Core lib 707/707、runtime-plan lib 90/90、AWBC VM focused 3/3、053
companion manifest/assets による公開 CLI check、workspace all-target/all-feature
check と Clippy、fmt、cached diff check は既存 warning ありで通過した。
`just structure-audit-gate` は 2640 files/97 packages、blocking 0 で通過。
`just test-workspace` は sema lib 968件中2件で失敗した。
`contextual_effect_rows_survive_value_expression_boundaries` の
`ExpressionTypeUnavailable` と capacity の expected `Unchecked` 対 actual
`Exact(USize)` であり、どちらも本 cut では未編集の sema owner。原因を別途
追跡中で、workspace test の合格とは記録しない。

構造 review: `crates/arcweft-core/src/awbc/fiber.rs` はこの cut の起点
4201 physical LOC から 4892 LOC（180957 bytes、+691）となり、SIZE001 /
TEST001 が出た。生存 frame、return continuation、snapshot codec と復元時
検証を同じ fiber state machine に閉じ込める既存 owner の責務内であり、
formatter だけの別 side table や重複した保存経路は導入していない。この
責務の一体性を維持する判断である。新しい dependency direction 違反は
scanner で検出されなかった。

この commit は 053 全受理ではない。`style="number"` の決定的な数値整形、
locale/currency data、project `DisplayText` conformance、
`InlineFallback.value_plain` の適切な失敗、AWBC の pure-helper/trait-method
別 fiber 経路の同一継続化は残る。

## 053 follow-up と workspace gate — 2026-09-27

Supersedes: 直前の 053 substrate note に記録した `just test-workspace` の
2件失敗は、現時点の gate 状態としては解消した。失敗の観測自体は履歴として
残す。確認した HEAD は `edf3a37cb777953ec30e957b8fb356ba7f24b67e`、
working tree は clean。

`7d99658768bb7390e9684c1ff3a6cfe213d65530` は Capacity の選択済み
`USize` schema に古い `Unchecked` assertion を合わせ直した。focused 1/1。
`edf3a37cb777953ec30e957b8fb356ba7f24b67e` は contextual effect row
を持つ sequence/array の子 closure を、未解決の contextual shape だけで
直接拒否せず、完全な期待型がある場合にだけ container を確定比較する。
array repeat と長さ不一致拒否を含む focused 2/2、sema changed-crate
check/Clippy を通した。

`de6d314b460eef36b4d2ba486fee8dd7b531db82` は評価済み primary の
unstyled text を formatter option failure 時の `value_plain` に保持し、
値を得られなかった `InlineFallback.value_plain` を空表示へ黙って落とさず
typed materialization error とした。Core と text-model の focused test
各1/1、workspace check/Clippy を通した。

これらの push 後、`just test-workspace` の全レシピ、workspace
all-target/all-feature check と Clippy、`cargo fmt --all -- --check`、
`git diff --check` は既存 warning ありで終了コード 0。前述の structure gate
も、この一連で dependency/owner 変更を追加しておらず、blocking 0 の
証拠を維持する。053 の残る locale/currency、project `DisplayText`、
数値 style、AWBC nested helper/trait の同一 fiber 化は未受理のまま。

## 053 formatter の同一 fiber 呼び出し — 2026-09-27

Supersedes: 直前の follow-up note にある AWBC helper/trait の別 fiber 経路は
解消した。確認した HEAD は `4e8a481fae71c0cb025b4c96437f53024cc3e13a`、
push 後の working tree は clean。

`f2347a082b6b34d6536d19963b40cb4eed0692ef` は primary が先に評価済みで
後続 formatter option の式が回復可能に失敗した場合、その primary の plain
表示を `value_plain` に保持する。primary 自体が失敗したときは値を偽造しない。
Core focused 2/2 と変更 crate Clippy、fmt、diff check が通過した。

`4e8a481fae71c0cb025b4c96437f53024cc3e13a` は AWBC の未加速 pure
helper と trait method を呼び出し元 fiber の検証済み frame に入れ、同じ
budget/cancel/save/restore と formatter protected operand の継続で実行する。
typed return continuation は instruction site、対象 function、戻り先 register、
可変 receiver の返却 slot を program と照合する。callee に pending cleanup が
ある回復可能エラーは formatter へ強制的に回収せず、通常 unwind に渡す。
backend 加速がない helper の fallback 統計は一回だけ課金する。別 fiber の
trait method executor と helper fallback 呼び出しは削除した。

Core lib 714/714、`just test-workspace` 全レシピ、workspace all-target/all-feature
check と Clippy、`cargo fmt --all -- --check`、cached diff check は終了コード 0
（既存 warning あり）。`just structure-audit-gate` は 2641 files / 97 packages /
339 review triggers / blocking 0。`awbc/fiber.rs` の SIZE001/TEST001 は継続し、
増分は frame return、snapshot、program validation という同じ live-fiber
所有境界に収まるため、行数だけを理由に別 state を作らなかった。

053 全受理ではない。`Option/Result.with_context` の lazy callback は formatter
operand から到達可能だが、Product host でなお別 fiber に入る。project
`DisplayText` の選択済み適合・実行、session locale と固定書式データを用いた
number/currency 整形も未完了である。

## 053 locale identity の単一化 — 2026-09-27

確認した HEAD は `bbef315cf0e34a64a60f36a6eb5a980eedfdcc01`、push 後の
working tree は clean。Core の独立 `LocaleId`/canonicalizer を削除し、
Dialogue、render-text、resource model/manifest の locale 値を
`arcweft-id::LocaleTag` に統合した。authored text の正規化は明示的な
`canonicalize`、型付き値と serde 復元は canonical spelling を要求する
`try_new` に分けた。LocaleTag owner の後続 subtag 重複検査から primary
language を除き、`de-de` → `de-DE` を受理しつつ `en-US-us` 等の真の
後続重複は拒否する。

6 変更 crate の tests・all-target check/Clippy、workspace
all-target/all-feature check/Clippy、`just test-workspace`、fmt と cached diff
check は終了コード 0（既存 warning あり）。`just structure-audit-gate` は
2640 files / 97 packages / 339 review triggers / blocking 0。旧 Core owner を
消して既存の `arcweft-id` owner を使ったため、依存辺や重複 authority は
増やしていない。

この cut は runtime locale 選択の受理ではない。維持仕様の root `[locale]`
は現行 manifest decoder に未接続で、Character catalog の default active
locale は別 authority のまま。session locale、formatter data、number/currency
整形と保存・replay の一貫性を次の境界で閉じる。

## 053 lazy context callback の同一 fiber 化 — 2026-09-27

Supersedes: formatter operand から到達できた `Option/Result.with_context`
callback の別 fiber 経路は解消した。確認した HEAD は
`79538180640e4385ce1bae0ff8b7280a1e0517f8`、push 後の working tree
は clean。

Core `RuntimeArcError` の begin/finish が Result/Option の成功 branch と
失敗・欠損 cause を一度だけ選び、native と AWBC が共有する。AWBC の四つの
context intrinsic は VM が処理し、lazy callback は成功 branch では呼ばず、
失敗 branch だけ現在の fiber に入れる。attached default は default body の
戻り値を target call に渡す二段継続とし、従来 Product host が default 値を
最終 message として扱った差異を除いた。Product host の nested callback
evaluator は削除し、独立した root pure/choice 実行だけが別 root fiber を持つ。

return continuation と v1 snapshot は pending cause と default 結果を保持し、
restore 時に instruction site、元 receiver、callback state と選択 body を
program に照合する。verifier は context の receiver/result/message、空 effect、
callback contract と default/target body の空 effect・非中断を要求する。
formatter 内の callback 評価失敗は protected operand が回収し、Content proof
欠落は致命的 trap のまま後続 Style を実行しない。

Core lib 719/719、新 AWBC focused 8/8、`just test-workspace`、workspace
all-target/all-feature check と Clippy、fmt と cached diff check は終了コード 0
（既存 warning あり）。`just structure-audit-gate` は 2641 files / 97 packages /
339 review triggers / blocking 0。`awbc/fiber.rs` と VM の増分は既存の
live-fiber return/snapshot/verification owner に置き、別の継続 authority は
作っていない。053 の project `DisplayText` と locale/number/currency 書式は未受理。

## 053 project locale・数値書式の統合 — 2026-09-27

Supersedes: 上の locale identity checkpoint で未接続だった root `[locale]`、
Character catalog の別 active policy、number/currency 書式はこの cut で接続した。
確認した commit は `3a2c8e84c77e4a20ca75ee4491c5ef15219d2f76`。
main へ push 後、working tree は clean。

root `[locale]` の source/default/fallback を typed `ProjectLocaleSpec` として厳密に
decode し、source span を保持して compiled project、bundle manifest、session
まで渡す。未公開の profile-scoped Character locale policy と policy digest は削除した。
Character 表示名は session active → project fallback → record source または
project source → base → declaration の順に解決する。active locale は host override
または bundle default から決め、hot swap で維持し、v1 save/restore と root replay
trace に保持する。bundle identity には project locale、replay には ICU/CLDR data
identity も含める。View `Localized` の active/fallback 投影は retained View 工程に残る。

Core の native/pure/AWBC formatter は同じ固定 ICU/CLDR data と typed locale context
で整数・浮動小数・通貨を整形し、AWBC は formatter 開始時の context を継続と v1
snapshot に捕捉する。runtime-host は executor と pure accelerator の両方へ context
を設定する。Core 729 tests、Character 56 tests、bundle codec 8 tests、launch
43 tests、manifest-model 17 tests、compiler 関連 61 tests と driver の locale
save/hot-swap focused tests が通過。workspace all-target/all-feature check、Clippy、
`just test-workspace` 全レシピ、fmt、cached diff check は終了コード 0（既存 warning
あり）。`just structure-audit-gate` は 2641 files / 97 packages / 339 review triggers /
blocking 0。触れた bundle facade、runtime-host runner、runtime-plan semantic facts の
既存 SIZE trigger は、それぞれ manifest 契約、runner context、既存 fact owner の
責務内の変更であり、別 state や逆向き依存を加えていない。

053 の一般 project `DisplayText` conformance と fixture 全体の受理は未達。
標準 trait/`DisplayContext`/`DisplayError`、閉じた impl の選択証拠、runtime-plan
method ID、pure trait call と AWBC formatter continuation を一つの typed witness
で接続する必要がある。既存 `TypeKind::DisplayText` record atom を trait の代用に
残さず、RAG/Agent consumer を同時に移行する。

## 053 selected DisplayText と formatter attempt の統合 — 2026-09-27

Supersedes: 直前 checkpoint の project `DisplayText` 未接続。確認した commit は
`7e9f72946f459c31de60c0ec4ef9730a1f443d67`。main へ push 後の working tree
は clean。標準 `DisplayText` trait、`DisplayContext`、`DisplayError`、標準
scalar/Content と明示的 Option の witness、閉じた project impl の選択済み method
instance を sema・HIR・compiler・runtime-plan・Core native/pure/AWBC へ接続した。
source-order の `fmt` operand を型付き attempt manifest に保持し、同じ fiber で
recoverable failure と非局所制御を処理する。AWBC v1 codec・verifier・snapshot/restore
にも attempt state を含め、途中復元と forged state 拒否を検証した。context の
locale/style/currency、generic impl の複数閉じた instance、nested closure、
project `DisplayText` の成功・失敗と後続 style 評価を検証した。

`cargo fmt --all -- --check`、workspace all-target/all-feature check と Clippy、
`just test-workspace` 全レシピ、`git diff --cached --check`、
`just structure-audit-gate` は最終差分で終了コード 0（既存 warning と size review
trigger あり、blocking 0）。focused sema DisplayText 10/10、runtime-plan lib 90/90、
Core format attempt 5/5、AWBC format-content 14/14、compiler の closed generic
拒否 1/1 と関数値 branch の native/AWBC 2/2 も通過した。途中の workspace run は
closed generic の診断が旧判定に届かず 1 件失敗し、witness 欠落の診断へ統合して修正。
別 run の LSP rustc ICE は LSP 単体 221/221 と最終 workspace run では再現せず、
compiler branch の stack overflow は fmt call clone を再帰 lowering frame から
隔離して解消した。

構造 review では AWBC fiber の format attempt live/snapshot state と verifier の
attempt abstract state を各 `format.rs` 子 owner に抽出した。runtime-plan の
`format_attempt.rs`、`trait_method.rs`、semantic display 子 owner と合わせ、
親の frame/save orchestration と dispatcher に別 authority を作っていない。
053 全受理は未達。affine `VoiceHandle` の callback capture → `Content` → `fmt` は
sema で受理されるが、一般 local read の Copy/Move と native/AWBC の consuming
transfer が未接続である。次の cut は generation-bound な local-use 判定から
単一の Core read mode、runtime token 一意性、途中 save/restore まで閉じる。

## 053 affine local-use と実行時 custody の統合 — 2026-09-28

Supersedes: 直前の 053 記録にある「一般 local read と consuming transfer が未接続」
という現状説明。確認した code commit は
`6cb610f536e186baade097afa8d4cc11445ca53d`。main へ fast-forward push 後、
この証拠追記前の working tree は clean。起点は
`2ab12d9327bda3ad1c4af1752baa142fbe082c63`。169 ファイルの統合 commit として
sema → runtime-plan → Core native/AWBC → Product/driver/host/CLI の所有権契約を同時に接続した。

Sema は受理済み HIR 世代と closed instance に結び付いた local-use catalog で
Copy/Move/Borrow、guard、capture、関数入口の実値 Copy 義務を選ぶ。compiler と
runtime-plan はその行を単一の `RuntimeLocalReadMode` と AWBC の所有権命令へ投影する。
Core は affine 値の暗黙 clone を拒否し、Need、line/child packet、callback、
ProjectCall、formatter operand、root event、Pure backend input の移譲を一つの
live owner に収めた。AWBC v1 の verifier、codec、VM、途中 snapshot/restore は
同じ所有権状態を検証する。Await progress は保留中の Need を observer 継続の
register に戻し、Ready では戻さない。map と built-in For は `SequenceNext`、
一度だけの tuple/array 分解、借用 pattern test を使い、反復ごとに source を
再消費しない。native iterator は残り要素だけを保持し、affine item を move する。
この cut が置き換えた複製・再読経路は残していない。

最終差分で `cargo fmt --all -- --check`、workspace all-target/all-feature check と
Clippy、`just test-workspace` 全レシピ、cached diff check が終了コード 0
（既存 warning あり）。focused Core lib 772/772、runtime-plan lib 90/90、
AWBC 288/288、runtime-plan map parity 5/5、iterator witness 3/3、
Sema local-use 33/33、CLI fixture 8/8 も通過した。Core の iterator 残余値の
live admission と途中 snapshot 往復、affine `SequenceNext` の空分岐と codec、
Need progress 再 Await と失敗時無変更を直接検証した。`just structure-audit-gate` は
2654 files / 97 workspace packages / blocking 0。途中の workspace run は
capture、defer、policy 再利用、Core fixture、LSP adapter、map/For、CLI fixture の
回帰を順に露出したが、最終 run は通過した。任意の `-D warnings` Clippy は
未変更の依存 crate 警告で失敗し、通常の必須 Clippy 成功とは混同しない。
MCP/capture/visual/production-limit の Tier 2 family と executable doctest は
本 cut で変更しておらず未実行。変更した codec と保存契約の往復は上記テストで検証した。

構造 review の実測は起点 HEAD → code commit の物理行数で、bytes は現在値。
全行は workspace package の維持 Rust owner。`tests.rs` と
`final_analysis/tests/local_use.rs` は test-only、他は production。
括弧内は production file に埋め込まれた test 行数で、`—` は該当なし。

| path | LOC (base → current) | bytes | embedded test LOC |
|---|---:|---:|---:|
| `crates/arcweft-core/src/awbc/fiber.rs` | 5597 → 6834 | 254456 | 877 |
| `crates/arcweft-core/src/awbc/product_step.rs` | 3849 → 5214 | 210085 | — |
| `crates/arcweft-core/src/awbc/product_step/line.rs` | 2865 → 4162 | 176763 | 74 |
| `crates/arcweft-core/src/awbc/product_step/suspension.rs` | 1904 → 2817 | 114368 | — |
| `crates/arcweft-core/src/awbc/product_step/tests.rs` | 4187 → 5335 | 203593 | test-only |
| `crates/arcweft-core/src/awbc/tests.rs` | 7512 → 7889 | 282928 | test-only |
| `crates/arcweft-core/src/awbc/verify/code.rs` | 4170 → 5693 | 221980 | — |
| `crates/arcweft-core/src/awbc/vm.rs` | 3960 → 5263 | 216094 | — |
| `crates/arcweft-core/src/engine.rs` | 2541 → 3944 | 154595 | 62 |
| `crates/arcweft-core/src/engine/dialogue.rs` | 2811 → 3197 | 138377 | 638 |
| `crates/arcweft-core/src/engine/dialogue/store.rs` | 1016 → 1676 | 62276 | 501 |
| `crates/arcweft-core/src/line_task/activation.rs` | 493 → 910 | 35585 | — |
| `crates/arcweft-core/src/line_task/handle.rs` | 4643 → 5614 | 216403 | 666 |
| `crates/arcweft-core/src/pattern.rs` | 3172 → 4052 | 154630 | 915 |
| `crates/arcweft-core/src/root.rs` | 1132 → 1304 | 48116 | — |
| `crates/arcweft-core/src/task/producer.rs` | 2148 → 3066 | 111904 | 433 |
| `crates/arcweft-core/src/value/callable/application.rs` | 217 → 1206 | 48661 | — |
| `crates/arcweft-core/src/value/opaque.rs` | 3300 → 3853 | 151930 | 825 |
| `crates/arcweft-core/src/value/ownership/slot.rs` | 938 → 1556 | 48811 | 146 |
| `crates/arcweft-lang-sema/src/final_analysis/local_use.rs` | 0 → 2659 | 103448 | — |
| `crates/arcweft-lang-sema/src/final_analysis/tests/local_use.rs` | 0 → 1076 | 32491 | test-only |
| `crates/arcweft-runtime-plan/src/final_flow.rs` | 8236 → 8578 | 354253 | 398 |
| `crates/arcweft-runtime-plan/src/semantic_facts.rs` | 12891 → 13271 | 514158 | — |

判断: Core の AWBC fiber/VM/verifier と ProductStep の親・line・suspension は
生存 frame、所有権転送、検証、保存、host progression のそれぞれ既存の状態機械 owner。
test-only file と埋め込み test は同じ owner の境界を検証しており、別の実行 catalog
を作っていない。Engine/Root/dialogue/line_task/task は native の順序と一度だけの
child custody、Pattern/value/callable/opaque/slot は typed 値と owner token の
責務に留まる。Sema の新規 owner は一つの generation-bound checker、runtime-plan
の大きな親は selected final flow と semantic facts の既存集約点で、独立した
projection は子 module に分けている。今回の増分に混在 I/O、逆向き Cargo 依存、
並行する live-owner table、公開 API を分割のためだけに拡げる理由は見つからず、
行数だけによる分割はしない。未変更の historical size trigger は本 cut の
blocking finding ではない。

この cut は 053 全体または収束 goal 全体の完了宣言ではない。次に残る
affine witness `Iterator::next` の state clone と native/pure filter の item clone は
実際の所有権境界として別途閉じる。後続の View、RuntimePlan/task-plan、
scheduler/restore 工程も goal の受入条件に従い継続する。

## Affine iterator と未使用 Filter IR の整理 — 2026-09-28

Supersedes: 直前 checkpoint の witness iterator と filter clone の残課題。
確認した code commit は `4fbc19771227fcd9871fd269a2bc73f6a5190f6e`。
main へ fast-forward push 後、working tree は clean。

Native の selected `Iterator::next` は affine state を一度だけ MutRef method へ
移し、更新された receiver を iterator へ戻す。戻り値は canonical な typed
`Option` case で判定し、`Some` の item 自体を渡す。affine `Vec<Need<i64>>` の
先頭取り出し、残余 state の同一実体、typed snapshot/restore 後の再開と終端、
affine receiver の Copy 拒否を focused test で検証した。

Core の `RuntimeExprSeedKind::Filter` には workspace 内の producer がなく、
native/pure は item を複製し、AWBC は predicate を一度だけ先に評価して
未実装の `seq.filter` intrinsic へ渡していた。未公開の孤立した IR 経路として
seed・実行 variant と全 consumer を削除した。言語の通常 callable `filter`
契約は変更していない。

`cargo fmt --all -- --check`、Core lib 774/774、runtime-plan lib 90/90、
runtime-accelerator lib 62/62、変更 3 crate の all-target/all-feature Clippy、
`git diff --cached --check` が終了コード 0。Clippy は多数の警告を出したが、
今回の必須実行結果は成功。workspace 全体の test recipe・構造 gate はこの小さな
Core cleanup cut では再実行していない。053 fixture 全体および後続工程の
受入条件は依然未完了。

## Affine intrinsic 引数の所有権移譲 — 2026-09-28

確認した code commit は `b574a28202e301e78ab0e1c9f2d79747229a609d`。
main へ fast-forward push 後、working tree は clean。

Core の `collect`、built-in `into_iter`、`Iterator::next`、`Option::unwrap`
は、native・pure・AWBC が既に所有していた引数を、借用スライス経由で再び
clone していた。共通の typed intrinsic dispatcher は単一引数を move し、
観測だけの `Option::is_some` は借用する。`evaluate_runtime_call` と
`VmHost::call_intrinsic` は所有済み `Vec<RuntimeValue>` を受け渡し、他の借用型
handler と pure-helper の fallback 契約は維持した。これにより AWBC の
`CallIntrinsic` が register から取り出した affine 値もそのまま host に渡る。

`Need` の同じ割当が direct `Vec.into_iter` → `next` → `unwrap` → `collect` を
通ることと、pure helper 内の `collect` を直接検証した。Core lib 776/776、
変更 3 crate の `cargo check --tests` と all-target/all-feature Clippy、
compiler の native/decoded AWBC 共通 host テスト 1/1、fmt と cached diff check
が終了コード 0。Clippy は警告を出したが失敗はない。workspace 全体の recipe と
構造 gate はこの cut では再実行していない。`CoreIndex` の値取得には別途
affine item を clone しうる経路が見つかったため、次の所有権判断として残す。
053 fixture 全体と後続工程の受入条件は未完了。

## CoreIndex の Copy 契約 — 2026-09-28

確認した code commit は `9f04fea14c2586e57fc974dabca152d090ec158f`。
main へ fast-forward push 後、working tree は clean。

`target[index]` は保持された collection から独立した値を返すため、選択 item
に Copy を要求する。Sema の generation-bound local-use seal は選択済み式辺を
辿り、閉じた generic instance でも Copy 証拠がない item を拒否する。
`CoreIndex` の直接呼び出しは affine sequence 全体を `value_at` より前に拒否し、
AWBC verifier は exact な sequence/array または String、整数 index、
matching Copy result と pure effect を要求する。実行側のない Map/Range の
index 型受理も削除した。affine collection から選択した要素だけを取り出して
残りを暗黙に捨てる経路は作っていない。

Core lib 778/778、Sema lib 1016/1016、compiler の native/decoded AWBC 実行
1/1、変更 crate の all-target/all-feature Clippy、fmt、cached diff check は
終了コード 0。Sema 全体の初回は選択済み postfix 子を二重に辿る回帰 7 件と、
旧 fixture の affine `Stream` index 1 件で失敗した。前者は選択済み式辺の走査へ
訂正し、後者は Copy 拒否を明示するテストへ移して再実行で全通過した。
`just structure-audit-gate` は 2656 files / 97 packages / 348 review triggers /
blocking 0。既存の大きな local-use owner は 2688 physical LOC / 104734 bytes、
AWBC verifier owner は 5754 physical LOC / 224154 bytes。今回の追加はそれぞれ
既存の所有権 seal と命令署名検証の責務内で、別 state、重複 authority、依存逆転を
増やしていないため、前 checkpoint の cohesion 判断を維持する。

053 check fixture 群は直前の clean SHA
`645e217d4cf22a0fb66ad8b0d3b81572a2404203` で focused CLI test 1/1 が通過。
この証拠は check 受理であり、053 の native/decoded AWBC 実行を示さない。
後続の View、task-plan、scheduler/restore と goal 全体の受入は未完了。

## Match C3 scope transcript の座標化 — 2026-09-28

確認した code commit は `e6e769ac5f02a48fec1d998627b7bf6511ff17ae`。
main へ fast-forward push 後、working tree は clean。起点は
`fd50ad9d36afe503ee28c8c121e23b8aa14458ce`。

受理済み [Match 設計](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/README.md)
は scope label の source spelling を transcript atom にしない契約だが、
現行 `CheckedStatementPayload::Scope` と `CheckedExpressionResolution::Scope` は
Named の名称を digest に書いていた。両 writer を Named/Anonymous tag と
同じ owner の accepted-rooted 座標に統一した。checked scope 名の compiler・
runtime 投影は保持し、凍結された設計 mirror と manifest は編集していない。
設計時の resolution 列挙に live `Scope` がない不一致は、この現行 source と
受理済み座標契約の照合として記録する。

statement/expression の label 名変更では Match digest が等しく、owner 配置と
Named/Anonymous の変更では異なる focused test 3/3 が通過。Sema lib 全体
1019/1019、`cargo check -p arcweft-lang-sema`、all-target/all-feature Clippy、
fmt、cached diff check は終了コード 0。`just structure-audit-gate` は
2656 files / 97 packages / 348 review triggers / blocking 0。
semantic transcript owner は 4351 physical LOC / 172437 bytes。既存の
単一 transcript writer と同じ責務内で座標 atom とそのテストを追加し、並行する
digest authority や逆向き依存を作っていない。

Match C5 の完成品 query は現在 sema 内部だけで、compiler に届く公開契約と
受理済み result schema の残項目は未完。C3 の全 resolution/body 差分テスト、
View 以降の工程も未完として継続する。

## Match C5 完成品 query の公開 — 2026-09-28

Supersedes: 直前 checkpoint の C5 公開契約未完という記述。確認した code commit は
`c07a496ae47d6e019b5a0169c778e8c5cdf40338`。main へ fast-forward push 後、
working tree は clean。

`FinalSemanticAnalysis::checked_match` は exact な HIR・symbol generation を検証し、
一つの Match subtree と nested Match を限界付きで転記する。成功時だけ、accepted-rooted
path、scrutinee の checked digest/type、source-ordered arm 座標・guard・result・binding、
version 1 の transcript digest と byte length、網羅性と到達不能 alternative を持つ
`CheckedMatch` を返す。非網羅、世代不一致、限界、cancel、欠落・不正 evidence は型付き
error とし、部分的な完成品を公開しない。非網羅 witness と Or alternative は sema の
既存 coverage matrix に対する read-only view で辿れる。旧二段階の内部 query は削除した。

外部 API の世代拒否、完全な成功結果、bool/record witness、Or alternative 座標の
統合テスト 5/5、Sema lib 1022/1022、C3 の raw HIR ID・span・書式・数値 radix
不変性、nested Match と block body の意味差分テスト 3/3 が通過。
`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
Clippy とテストには既存の警告があるが失敗はない。`just structure-audit-gate` は
2658 files / 97 packages / 348 review triggers / blocking 0。

`arcweft-lang-sema/src/final_analysis/semantic_transcript.rs` は production owner で、
4351 → 4739 physical LOC、186168 bytes、埋め込み test なし。増分 388 LOC は
公開 result/view と同じ bounded transcript transaction に属する。内部 matrix と
公開 view の authority は一つで、別の coverage state、逆向き依存、I/O は増えていない。
外部 API テストは別 file に置いた。既存の大きな owner のまま保持する判断は、
この責務と依存境界に基づくもので、行数だけを理由に分割していない。

Match C3 の T01/T06 全 resolution/body 差分 matrix はまだ完走していない。
View、task-plan、scheduler/restore と goal 全体の受入も未完として継続する。

## Match C3 statement-owned body transcript の修正 — 2026-09-28

確認した code commit は `4693bffbd2f79bed87201efcf0181aaae3a18a3c`。
main へ fast-forward push 後、working tree は clean。

`statement_digest_at_with_state` は直接の `BodyItem` edge を順序 marker のまま保持し、
`HirStmtKind::body_projections()` の各 body を既存の generation-bound body writer で
転記する。accepted-rooted body 座標と digest を source order で statement digest に
入れる。これまで `if` 等の statement 内の body が Match transcript に届かず、
内部の意味変更でも外側の Match digest が同じになりえた。

新規回帰は修正前に失敗し、修正後は内部 literal の変更、非空/空 body、statement
順序の各差分で外側 Match digest が異なる。Sema lib 1023/1023、公開 Match query
5/5、`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
Clippy/test の警告はあるが失敗はない。新しい owner/API/依存方向はなく、既存
transcript transaction の body writer を再利用した。構造 gate はこの 35 LOC の
同一 owner 修正では再実行していない。

C3 には expression の演算子・mode・数値などの HIR shape atom、pattern の
sequence-rest atom、checked call passing、Choice の checked plan field 等の不足が
残る。T01/T06 の完全受入、View 以降の工程と goal 全体は未完として継続する。

## Match C3 HIR shape atom と compact numeric producer — 2026-09-28

Supersedes: 直前 checkpoint の HIR shape/sequence-rest atom 未実装という記述。
確認した code commit は `fee984818350656c92e04ffe10dad5787f80bfb0`。
main へ fast-forward push 後、working tree は clean。

Match transcript の exhaustive expression-shape writer は HIR が所有する閉じた tag
で binary/unary/borrow operator、Thread/computation mode、ForSynthetic kind、
placeholder/call form/Await branch を記録し、Range の inclusive/endpoint、
tuple/bracket の個数、compact numeric sequence の個数と canonical magnitude も
記録する。pattern の BracketSequence は absent/unbound/bound rest を区別する。
既存 checked type、child/body、record field、callable join、Match product の
authority は複製しない。radix など authored spelling は digest に入れない。

compact numeric sequence の各値が選択された整数要素型へ収まるかは、Match query
だけでなく Sema expression producer が検証する。u8/i8/u128 の上限ちょうどと
一つ超過を型付き error で検証した。binary Add/Subtract、Range inclusive、
numeric 値差分と radix 不変、`[true]`/`[true, ..]` および `[]`/`[..]` の
pattern digest 差分は、修正前の衝突を含め focused test で確認した。

HIR lib 924/924（8 ignored）、Sema lib 1027/1027、公開 Match query 5/5、
`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
`just structure-audit-gate` は 2658 files / 97 packages / 348 review triggers /
blocking 0。Clippy/test の既存警告はあるが失敗はない。

production owner `semantic_transcript.rs` は 4774 → 4904 physical LOC / 192499 bytes。
増分は同じ bounded transcript writer と shape atom の分岐で、別 state や逆向き依存、
I/O を導入しない。HIR `expr/basic.rs` は 553 → 617 physical LOC / 15559 bytes で、
閉じた HIR enum tag をその owner に置いた。既存の大きな transcript owner は
この責務のまとまりを保ち、行数のみを理由に分割しない。

checked call passing と Choice の plan key/cancel trigger は未転記。full Option の
`label(text_key=...)` は現行 checker 自体が拒否しており、受理前の checked owner
拡張が必要。T01/T06 の全 live family/root matrix、View 以降の工程、goal 全体の
受入は未完として継続する。

## Match C3 selected call passing と安定 join — 2026-09-28

確認した code commit は `e91240c92767da6b5bb556eea779c027098a301d`。
main へ fast-forward push 後、working tree は clean。

Match の call writer は exact site と accepted-rooted expression 座標を照合した
selected application の source-ordered `CheckedCallArgumentPassing` を転記する。
positional/named/spread の閉じた tag は既存 application encoder と共有する。
選択済み callable join の runtime digest は解析世代と catalog revision を含むため、
その bytes は変えずに、同じ join から accepted declaration ID を使う transcript 専用
投影を発行する。非選択 candidate 一覧を含む application 全体の digest や raw 引数名は
Match に入れない。

同じ selected callable/slot の positional と named call は Match digest が異なり、
named call の空白・整数 radix 変更では等しい。後者は修正前、既存 join digest の
generation-bound ID により失敗したため、安定投影の必要性を実証した。Sema lib
1028/1028、focused transcript 8/8、`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
既存警告はあるが失敗はない。新しい crate/依存方向、公開 runtime contract、
大幅な owner growth はなく、構造 gate は本 cut では再実行していない。

C3 には project callable value writer の generation-bound ID と method selection
の generation-bound join、Choice の checked plan key/cancel trigger が残る。
T01/T06 の全 live family/root matrix、View 以降、goal 全体は未完として継続する。

## Match C3 callable value / method の安定 identity — 2026-09-28

Supersedes: 直前 checkpoint の project callable value と method selection に残る
generation-bound digest の記述。確認した code commit は
`e017c1a84e5394e0726c99ea7161691107d8835c`。main へ fast-forward push 後、
working tree は clean。

project callable value writer は実行用 `CheckedCallableId` の解析世代 digest の代わりに
accepted declaration ID を記録する。`CheckedMethodSelection` は既存の runtime 用
join digest を保持しつつ、同じ selected join から seal 時に transcript 用 digest も
発行する。Project は accepted declaration、Environment は durable な構造的
`EnvironmentCallableId`、Standard は catalog version と構造的 ordinal を用い、
Detached は project Match transcript への混入を拒否する。transcript 時の再解決や
並行する callable catalog は作らない。

project callable value の空白・無関係な前置宣言不変テストと、`Select(Method)` を実際に
通る View modifier のテストが通過した。後者は runtime digest が source revision で
変わる一方、stored stable method digest と Match digest は等しく、別 modifier では
双方が異なることを検証する。Sema lib 1030/1030、focused transcript 10/10、
`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
既存警告はあるが失敗はない。新 crate/依存方向や大幅な owner growth はなく、
構造 gate は本 cut では再実行していない。

C3 の compact Choice plan key/cancel trigger と T01/T06 全 live family/root matrix は
まだ未完。View 以降の工程と goal 全体も継続する。

## Match C3 compact Choice plan の seal — 2026-09-28

Supersedes: 直前 checkpoint の compact Choice plan key/cancel trigger 未実装という記述。
確認した code commit は `5534fed88d5ad1ab767924528951e0a0c8902a8d`。
main へ fast-forward push 後、working tree は clean。

`CheckedChoice` は plan の不在と空 plan を区別し、source 順の Assignment、Timeout、
Cancel、OnSelect 行を保持する。assignment key は Window/Layout/DefaultFocus の
閉じた enum とし、未知・recovered key を拒否する。Cancel は既存の typed
`CheckedTrigger` を使い、Input/Event/Signal/Timeout/Select/Task/Scope/Expression の
受理された family を seal する。Mark/recovery は拒否する。既存 option ID と goto
target は引き続き同じ checked authority に置き、plan の値・pattern・body は HIR の
typed child edge と body digest から転記する。validator は plan の有無、行数、順序、
family、key、trigger を exact に照合する。

Choice producer は plan assignment 値、timeout Duration、expression trigger Bool、
signal target/payload を検査し、その expression effects を Choice effect row へ集約する。
Event cancel pattern は Entry の checked event 型、Input/Task/Scope 等は既存 ingress
型で seed する。Signal payload binding は cancel body 内から参照できる。
Match transcript は closed plan 行と compact action の意味を記録する。

focused transcript 11/11、ingress 6/6、statement contextual 22/22 が通過。
plan 不在/空、同じ child 値の window/layout、trigger family、書式不変、未知 key、
Duration/Bool/Signal 型不一致拒否、Entry event 型一致、`control.spawn` の Choice
effect 伝播を確認した。Sema lib 1032/1032、
`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
`just structure-audit-gate` は 2658 files / 97 packages / 348 review triggers /
blocking 0。既存警告はあるが失敗はない。

production owner `final_analysis/analyzer/expressions.rs` は 4750 → 4941 physical LOC /
214098 bytes、`final_analysis/model.rs` は 3057 → 3128 physical LOC / 103440 bytes。
増分は既存 Choice producer とその checked fact に集中し、独立 plan catalog、逆向き
依存、I/O を作っていない。大きい owner は一つの final-analysis transaction の責務を
保っており、行数のみを理由に分割しない。

full Option の label/text_key は現行 checker が受理していないため、この compact
Choice cut の完了証拠には含めない。C3 の明示 Call 型引数と T01/T06 の全 live
family/root matrix、View 以降、goal 全体は未完として継続する。

## Match C3 明示 Call 型引数の転記 — 2026-09-28

Supersedes: 直前 checkpoint の明示 Call 型引数未転記という記述。確認した code commit は
`6dd8ed3e66d3263ffdab4f3d96ad73d6f5a5cf42`。main へ fast-forward push 後、
working tree は clean。

Call の HIR invocation は explicit type application の不在/存在、source 順の個数と
各 TypeId を保持する。Match writer は TypeId を lookup のみに使い、final analysis の
checked `TypeKind::semantic_identity_digest()` を転記する。通常 Call と attached-content
ContentCall は同じ helper を通り、raw ID と DirectAngle/Turbofish の綴りは書かない。
欠落・不正な引数、未閉鎖の application、checked type 欠落は成功 transcript を作らず
拒否する。join/application の選択 authority は変更しない。

同じ selected generic join を持つ推論 `identity(1i64)` と明示
`identity::<i64>(1i64)` の Match digest が異なり、明示 call の空白変更では等しい。
DirectAngle の generic method は受理されることを確認したが、現行 Sema で同一 callee
に両表記を受理する fixture は見つからず、DirectAngle/Turbofish 相互の等価性は
未検証。malformed application と checked 型不一致の拒否を確認した。

focused transcript 12/12、Sema lib 1033/1033、
`cargo check --workspace --all-targets --all-features`、
`cargo clippy --workspace --all-targets --all-features`、`just test-workspace`
（308 件の test result 群、失敗 0）、fmt、cached diff check は終了コード 0。
既存警告はあるが失敗はない。2 file の同一 writer/受理テスト変更で新たな owner や
依存方向を増やしていないため、構造 gate はこの cut では再実行していない。

T01/T06 の全 live family/root matrix、View 以降、goal 全体は未完として継続する。

## Match T06 受理済み body root の証拠 — 2026-09-28

確認した code commit は `14c9fa5d792139838f673616c890dfba4aad387d`。
main へ fast-forward push 後、working tree は clean。

Predicate、Proof、Flow の受理済み body に Match を置き、各 root で arm body の
型を保つ意味変更が transcript digest を変えることを確認した。先行する無関係な宣言と
書式変更で raw Match ExprId と span が変わっても、checked meaning の digest は
維持される。Flow は local initializer 内、Proof は expression body 内の Match を使う。

focused `checked_match_transcript` 18/18、Sema lib 1036/1036、fmt、
`git diff --cached --check` は成功。changed-crate Clippy は終了コード 0 だが既存警告が
あり、baseline 比較はしていない。テストのみの cut なので workspace gates と構造 gate
は再実行していない。

impl/inherent、dialogue、Await、attached default の候補はこの cut で受理を証明
できず、テストには含めていない。通常/View project parameter default は現行 builder
が拒否する。T01/T06 の全 live family/root matrix、View 以降、goal 全体は未完として
継続する。

## Match T01 nested Value/Select tag の閉鎖 — 2026-09-28

確認した code commit は `789802c63d84066b4e81f9e7cadc46d71f16586b`。
main へ fast-forward push 後、working tree は clean。

live `CheckedValueResolution` の8 variant と `CheckedSelectResolution` の5 variant は
各 owner が閉じた u16 tag を定義し、Match transcript がその tag を payload より前に
転記する。top-level Value は再帰 Value writer に統合したため、Value node ごとに
tag を一度だけ書く。Select の DialogueView/AgentField にあった局所的な byte tag は
重複 authority として削除した。既存受理 source で Local/ProjectCallable と同型の
別 record field の意味差、Method/Field の live tag を検証した。

focused semantic transcript 21/21、Sema lib 1037/1037、workspace all-target/all-feature
check と Clippy、fmt、cached diff check は成功。`just test-workspace` の初回は
project-loader の `release_remote_publish_file_mirror_archive_verifies_after_publication`
1件が staging path 不在で失敗した。同テストの単独再実行は成功し、workspace 全体の
再実行も 308 件の test result 群で失敗 0、終了コード 0。原因の確定や再現性の解消は
この cut では行っていない。既存警告はあるが上記の最終 gate は成功した。
既存 owner 内の tag/writer 修正で新たな依存方向や owner を増やしていないため、
構造 gate は再実行していない。

T01 の全 live family、T06 の残 root、View 以降と goal 全体の受入は未完として継続する。

## Match T06 dialogue/attached default の受理証拠 — 2026-09-28

確認した code commit は `420e1f8e57484f029e410cadab1fc3bd25f3770f`。
main へ fast-forward push 後、working tree は clean。

受理済み dialogue On handler と attached-content default block に expression Match を
置き、型を保つ arm の意味変更で digest が変わり、先行宣言による raw ExprId/span 変更
では digest が維持されることを確認した。focused 13/13、Sema lib 1039/1039、fmt、
changed-crate Clippy と cached diff check は成功。テストのみの cut なので workspace
と構造 gate は再実行していない。

Await Pending は受理済み基底に expression Match を挿入すると HIR publication が
`HirInvariantFailure::InvalidSourceIndex` で失敗し、今回の受理証拠には含めない。
statement Match 形式は expression Match owner を生成しない。原因の owner/source-index
調査と修正を継続する。通常 function parameter default は現行文法が禁止する一方、
View parameter default は retained View 契約が要求するが現在の builder/compiler が
拒否しており、View 工程で型・効果・実行側を一体で移行する必要がある。
T01/T06 全 matrix と goal 全体は未完として継続する。

## Match T06 実装 method body の受理証拠 — 2026-09-28

確認した code commit は `f755b33c2ff97ceedd9f35e8d2c3a559f1678b73`。
main へ fast-forward push 後、working tree は clean。

受理済み DisplayText trait 実装 method と inherent method の body に expression Match
を置き、同型の arm 意味変更で digest が変わり、先行宣言による raw ExprId/span 変更では
digest が維持されることを確認した。inherent method は no-Match 宣言/body の受理を
先に確認している。free function からの `number.get()` は現行 Sema で
`CallResolutionFailed` となるため、body-root テストの実行条件に加えなかった。

focused 15/15、`cargo test -p arcweft-lang-sema` と lib 1041/1041、changed-crate
Clippy、fmt、cached diff check は成功。既存警告あり。テストのみの cut なので
workspace と構造 gate は再実行していない。Await Pending と残る T01/T06 matrix、
View 以降と goal 全体は未完として継続する。

## Match T06 Await Pending の受理訂正 — 2026-09-28

Supersedes: 直前の T06 dialogue/attached default 記録にある Await Pending は
expression Match を挿入すると HIR `InvalidSourceIndex` で拒否されるという判断。
確認した code commit は `9ad4df2c1177c8b7d708a31e412315069846d9fb`。
main へ fast-forward push 後、working tree は clean。

失敗候補は Match arm を一行で区切らず parser が `ParseStatus::Recovered` とした
source だった。改行で arm を区切ると HIR と Sema の双方が受理する。HIR 回帰テストは
Await Pending の Thread body 内 Let initializer が Match expression owner と source
span を持つことを確認する。Sema 回帰テストは arm の意味変更で digest が変わり、
先行宣言と数値表記を変えても digest が維持されることを確認する。

HIR lib 925 成功・8 ignored、Sema lib 1042/1042、focused HIR/Sema、changed-crate
Clippy、fmt、cached diff check は成功。テストのみの cut なので workspace と構造 gate
は再実行していない。回復形の Await owner は現行 source-index invariant が型付き
`InvalidSourceIndex` として拒否するが、正当な Await/Match の受理を妨げないため今回の
production 変更対象には含めていない。T01/T06 の残 matrix、View 以降と goal 全体は
未完として継続する。

## Match T01 pattern family の受理差分 — 2026-09-28

確認した code commit は `a3f140e6689d712c16c30629d71cd145c721c159`。
main へ fast-forward push 後、working tree は clean。

既存の受理済み Match source を使い、各対象 pattern subtree が checked Match arm に
到達することを HIR constructor と checked pattern resolution/type の両方で確認した。
tuple/Or、Result payload variant と Choice typed binding、Vec sequence/rest、project
enum record-variant payload の4 family で、対象 owner の意味変更が Match digest に届く。
既存の exact/rest sequence 差分テストも維持した。合成 checked fact や frozen 設計の
固定 variant 数をテストに持ち込んでいない。

focused 4/4、Sema lib 1046/1046、`cargo test -p arcweft-lang-sema`、changed-crate
Clippy、fmt、cached diff check は成功。既存警告あり。テストのみの cut なので
workspace と構造 gate は再実行していない。T01 の残る live expression/value/select/
pattern/statement family、T06 の残 root、View 以降と goal 全体は未完として継続する。

## Match T01 Pipe/Try expression の受理差分 — 2026-09-28

確認した code commit は `7545ae34fd13a9126f0fdafdae7200de6d4ca815`。
main へ fast-forward push 後、working tree は clean。

受理済み Match arm 直下の Pipe と prefix Try について、HIR family と checked
resolution を同一 owner で確認した。Pipe は placeholder 2箇所を持つ checked binding
を保持し、同型の別 callable source へ替えると Match digest が変わる。Try は Result
carrier と CarrierBlock boundary を保持し、同型の別 local operand へ替えると digest
が変わる。source 名だけで解決を推測せず checked fact を検査している。

focused 2/2、Sema lib 1048/1048、`cargo test -p arcweft-lang-sema`、changed-crate
Clippy、fmt、cached diff check は成功。既存警告あり。テストのみの cut なので
workspace と構造 gate は再実行していない。T01 の残る live family、T06 の残 root、
View 以降と goal 全体は未完として継続する。

## Match T01 pattern corpus pilot — 2026-09-28

確認した code commit は `83a1150f0e20c4a80edbd82e6b7b282bac0fb814`。
main へ fast-forward push 後、working tree は clean。

test-only の accepted Match path-prefix collector を追加し、別宣言の checked pattern を
対象 Match の証拠に数えないことを検証した。既存5件の受理済み source を表に再利用し、
live HIR pattern shape と checked pattern resolution を wildcard のない exhaustive
classifier で分類する。現時点の受理証拠は HIR shape 9/13、checked resolution 5/6。
MutableBinding、EntityReference、WholeBinding と checked Entity は Pending、Error は
RejectOnly。到達不能と断定した family はない。既存の意味差分テストは維持した。

focused pattern 6/6、corpus 2/2、Sema lib 1050/1050、changed-crate Clippy、fmt、
cached diff check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate
は再実行していない。Pending pattern と残る T01/T06、View 以降と goal 全体は未完として
継続する。

## Match T01 accepted pattern corpus の閉鎖 — 2026-09-28

Supersedes: 直前 pilot の MutableBinding、EntityReference、WholeBinding、checked Entity
を Pending とした分類。確認した code commit は
`8d9737e418fbdfad44485b3af14bf07517cb691e`。main へ fast-forward push 後、
working tree は clean。

3つの HIR pattern shape は全て受理済み Match root 配下に到達し、MutableBinding と
WholeBinding は checked Structural、EntityReference は checked Entity として閉じた。
`mut selected` と通常 binding、別の accepted Flow entity ID、whole binding の有無
それぞれで Match digest が変わる。pattern corpus の positive は live HIR 12/13、
checked resolution 6/6。残る HIR Error は RejectOnly で、受理 family と数えない。

focused pattern 8/8、`cargo test -p arcweft-lang-sema`（lib 1051/1051、trybuild
14 と integration）、changed-crate Clippy、fmt、cached diff check は成功。既存警告あり。
テストのみの cut なので workspace と構造 gate は再実行していない。T01 の残る
expression/value/select/statement と T06 の残 root、View 以降、goal 全体は未完として
継続する。

## Match T01 expression/value/select corpus pilot — 2026-09-28

確認した code commit は `9e781d468e011903bccaac99b146dbbe56818b56`。
main へ fast-forward push 後、working tree は clean。

test-only の accepted Match path-prefix collector と、live HIR expression shape、
checked expression resolution、Value、Select の exhaustive classifier を追加した。
既存の意味差分テストと共有する8件の受理済み source row からのみ集計し、別宣言の
checked expression を誤算入しないことも検証した。現時点の直接証拠は HIR shape
16/38、checked expression 12/33、Value 2/8、Select 3/5。残りは Pending、
HIR Error は RejectOnly、到達不能と断定した family はない。

acceptance module 32/32、Sema lib 1053/1053、changed-crate Clippy、fmt、cached
diff check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。Registered/Entry/ProjectItem Value 等の既存受理 source は次の
行の候補で、まだこの pilot の証拠には数えない。T01/T06 の残り、View 以降と goal
全体は未完として継続する。

## Match T01 Entry/ProjectItem Value の受理差分 — 2026-09-28

確認した code commit は `2eb2bc2ebc086753fd2cfd5000b2a73d864eae21`。
main へ fast-forward push 後、working tree は clean。

既存 corpus helper を fixture 入力へ対応させ、Flow 内の2つの Entry と external/
retained Character の2つの ProjectItem を、受理済み Match path 配下の checked
Value として確認した。各 arm の値は同じ `Ref<Entry>` / `Ref<Character>` 型を保つが、
Entry binding digest または ProjectItem semantic ID が異なる。片方の arm の owner
を替えると Match digest が変わる。HIR EntityReference shape も受理済み行へ移した。

acceptance module 34/34、Sema lib 1055/1055、changed-crate Clippy、fmt、cached
diff check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。Registered Value は custom environment が必要なためこの cut に
含めず、残る T01/T06、View 以降と goal 全体は未完として継続する。

## Match T01 Registered Value の受理差分 — 2026-09-28

Supersedes: 直前 checkpoint の Registered Value は未着手という記述。確認した code
commit は `6bd9dcaf4998b648d27e0b79aa696d0081928657`。main へ fast-forward
push 後、working tree は clean。

一つの `TypeCheckEnv` に同型 `i32` の2つの registered symbol を置き、受理済み
Match path 配下の `CheckedValueResolution::Registered` がそれぞれの環境 binding
ID を保持することを確認した。選択する binding を替えると registered semantic ID と
Match digest が変わる。同じ binding のまま無関係な先行宣言を追加すると両者は維持される。
synthetic `from_bytes` ID や型差ではなく実解析の checked fact を検査した。

acceptance module 35/35、Sema lib 1056/1056、changed-crate Clippy、fmt、cached
diff check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。残る T01/T06、View 以降と goal 全体は未完として継続する。

## Match T01 ProgressField の受理到達 — 2026-09-28

確認した code commit は `cc815851a5cd243ac8cccabd4bbb1d78e4bdb87a`。
main へ fast-forward push 後、working tree は clean。

受理済み Await Pending body の Match arm 内で `progress.ratio` と
`progress.label` がそれぞれ checked `Select::ProgressField` として到達する。
前者は F32、後者は Option<String> で、両方とも2 armに exact field fact がある。
Match digest も異なるが型も異なるため、この比較だけを field atom 単独の転記証拠
とは扱わない。同条件の writer 検証を次の cut で行う。

Sema lib 1057/1057、focused acceptance、changed-crate Clippy、fmt、cached diff
check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。T01/T06 の残り、View 以降と goal 全体は未完として継続する。

## Match T01 ProgressField atom の writer 証拠 — 2026-09-28

確認した code commit は `122e26ff811544570cae84f8bb11c8f3bac60861`。
main へ fast-forward push 後、working tree は clean。

ProgressField の transcript payload は重複した Ratio/Label byte mapping をやめ、
owner の `ProgressField::semantic_tag()` を使う。受理済み Pending/Match fixture から
実際の checked Ratio/Label selection を取得し、同じ analysis、owner coordinate、
hasher prefix、型入力なしで writer に転記すると、同じ byte 長で異なる payload
digest になる。直前 cut の Match-root 到達証拠と合わせて field atom を検証した。

focused semantic transcript 42/42、Sema lib 1058/1058、changed-crate Clippy、
fmt、cached diff check は成功。既存警告あり。owner tag に同じ値を移す内部 writer
整理で公開形や依存関係を変えていないため、workspace と構造 gate はこの cut で
再実行していない。T01/T06 の残り、View 以降と goal 全体は未完として継続する。

## Match T01 contextual Value と lifetime Variant の受理証拠 — 2026-09-28

確認した code commit は `72d76102a2e960336ab95da40eb9c9f9aa10f563`。
main へ fast-forward push 後、working tree は clean。

受理済み Dialogue line-plan 内の Match arm に direct `line.voice_handle()` と
`akane.stage.acquire(scope=line)` を置き、checked LineContext / CharacterField(Stage)
の exact owner・型を確認した。Stage call の `scope=line` は checked
`PresentationLifetime::line` Variant で、BuiltinClosed owner、型、ordinal、case
名を両 arm で確認した。Akane と alternate の Stage owner 変更で digest は変わるが、
StageApi と Match result の exact Character 型も変わるため field atom 単独の差とは
主張しない。先行宣言で raw owner/span が変わっても digest は維持される。

途中の簡略 fixture は未束縛 `cue`、別の local workaround は HIR recovery を起こし
失敗した。最終差分では元の受理済み line-plan binding と direct `scope=line` を保持した。
focused 2/2 と corpus matrix 1/1、Sema lib 1060/1060、changed-crate Clippy、fmt、
cached diff check は成功。既存警告あり。テストのみの cut なので workspace と構造
gate は再実行していない。T01/T06 の残り、View 以降と goal 全体は未完として継続する。

## Match transcript 受理テストの owner 分割 — 2026-09-28

確認した code commit は `b83ed9f671cd3a59410a387ec0e163f2fe550d55`。
main へ fast-forward push 後、working tree は clean。

`semantic_transcript_acceptance.rs` が 98,536 bytes / 2,642 physical LOC に達し、
expression corpus pilot の一 cut だけで 451 LOC 増えたため、構造 policy の owner
review を行った。単一ファイルには body-root、expression behavior、pattern、
expression/Value/Select corpus の独立に変化する受理テスト責務が同居していた。
共通 Match 観測 helper を親121 LOC / 4,760 bytes に残し、body_roots 440 LOC /
11,276 bytes、expression_shapes 409 LOC / 16,574 bytes、patterns 649 LOC /
24,399 bytes、expression_corpus 1,036 LOC / 41,900 bytes の test-only 子 module
へ分けた。後者は accepted-path collector、網羅表、Value/Select owner fact を一つの
受理境界として保つ。新たな本番 owner、状態、I/O、Cargo edge、公開 API はない。
statement corpus は別子 module に置ける構造になった。

移動前後の acceptance 38/38、Sema lib 1060/1060、fmt、changed-crate Clippy、
`just structure-audit-gate`、cached diff check は成功。構造 gate は review trigger
のみ、blocking 0。既存警告あり。T01/T06 の残り、View 以降と goal 全体は未完として
継続する。

## Match T01 statement corpus pilot — 2026-09-28

確認した code commit は `639613a2292f61c7c84a4cd7a2c0904913fb0dd3`。
main へ fast-forward push 後、working tree は clean。

test-only `statements.rs` で live HIR statement 32形状と checked payload 15 family を
wildcardなしで分類した。accepted Match root の semantic coordinate 以下にある
checked statement だけを集計し、既存 arm block の Let と nested If+Let は
Structural payload として受理した。別宣言の checked Return は集計から除外する。
既存 body-root source builder を親 module へ共有し、重複 fixture を作らなかった。

focused corpus 2/2、acceptance 40/40、Sema lib 1062/1062、changed-crate Clippy、
fmt、cached diff check は成功。既存警告あり。テスト専用の新子 module は直前の
構造レビューに沿い、本番 API や依存は変えない。Assign、Assertion、Wait 等は
Pending のまま。T01/T06 の残り、View 以降と goal 全体は未完として継続する。

## Match T01 Assign/Assertion statement の受理差分 — 2026-09-28

確認した code commit は `ed7142cf31abc021674bbf1c3371cef16a8cd7d3`。
main へ fast-forward push 後、working tree は clean。

受理済み expression Match arm block 内の Assign と Assertion を HIR shape と
checked payload の両方で確認し、statement corpus の Accepted 行へ移した。
Assign は同じ Bool 型の `Flags.left` / `Flags.right` を選ぶと checked field ID と
宣言 ordinal が異なり、Match digest も変わる。Assertion は同じ
`Runtime(AlwaysGuard)` disposition のまま別の Bool 条件を選ぶと digest が変わる。

focused 60件、Sema lib 1064/1064、changed-crate Clippy、fmt、cached diff check
は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は再実行
していない。Wait 以降の statement family、T01/T06 の残り、View 以降と goal
全体は未完として継続する。

## Match T01 Defer/For statement の受理差分 — 2026-09-28

確認した code commit は `3cf692ae102fdfca4a258015f5d4e783fccb0df7`。
main へ fast-forward push 後、working tree は clean。

Flow 内 expression Match arm block に Defer と For を置き、HIR shape と checked
Defer / Iteration payload を accepted path 配下で確認した。Defer は Bool capture の
owner を変えると checked capture coordinate と Match digest が変わる。For は
`Builtin { family: Vec, item: Bool }` を保ったまま iterable の値を変えると digest
が変わる。focused Match 62件、Sema lib 1066/1066、changed-crate Clippy、fmt、
cached diff check は成功。Clippy の既存警告あり。テストのみの cut なので workspace
と構造 gate は再実行していない。

Wait 候補は Final Sema analyze を通ったが、expression Match arm の式 block では
parser が `FunctionItem` 文脈を使い、HIR Wait statement ではなく ExpressionStatement
となる。checked Match query の `MissingChildEdges` はこの候補に出たが、Wait payload
への到達証拠ではない。Wait は Pending のまま、Choice/Await/dialogue 等の特殊 nested
body を含む到達可能性を調べる。一般の arm block に Flow 文脈を伝播させるのは式評価
中の suspension を許す言語拡張になるため、この cut では行わない。T01/T06 の残り、
View 以降と goal 全体は未完として継続する。

## Match T01 Return/IfLet statement の受理差分 — 2026-09-28

確認した code commit は `ad7853e12f1fc5a819a96acb8766b07a279f1f74`。
main へ fast-forward push 後、working tree は clean。

通常 function の expression Match arm block に Return と IfLet を置き、各 HIR
shape と checked Structural payload の正確な組を accepted path 配下で確認した。
同型 i64 return literal と同型 Bool IfLet 入力の意味変更がそれぞれ Match digest に
届く。初期の LetElse/IfLet/While 候補の `InvalidArenaCommit` は Rust 行継続で
`.arcw` 字下げが崩れた fixture によるもので、raw multiline helper へ直すと
IfLet は受理された。LetElse/While/WhileLet は修正後未検証なので Pending のまま。

focused Match 64件、Sema lib 1068/1068、changed-crate Clippy、fmt、cached diff
check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。T01/T06 の残り、View 以降と goal 全体は未完として継続する。

## Match T01 LetElse statement の受理差分 — 2026-09-28

確認した code commit は `20cd53099da71309e0a886fee86fb5f74e4d7674`。
main へ fast-forward push 後、working tree は clean。

正しい raw multiline source の expression Match arm block に LetElse を置き、
checked Structural payload と else body 内 Return/Structural を accepted path 配下で
確認した。Bool initializer を変えると Match digest が変わる。focused 65件、
Sema lib 1069/1069、changed-crate Clippy、fmt、cached diff check は成功。
既存警告あり。テストのみの cut なので workspace と構造 gate は再実行していない。

同じ Ordinary arm block に直接置いた While/WhileLet は HIR `thread_control.rs` の
`require_thread_statement_context` で `InvalidArenaCommit` となり、前段の不正な
字下げだけが原因ではなかった。T01 ではまず既存 Thread body を Match の下に置く
受理証拠を試す。普通の block に While を受理させる変更は body owner と利用側の移行
が必要で、この cut に混ぜない。While/WhileLet と T01/T06 の残り、View 以降と goal
全体は未完として継続する。

## Match T01 Thread 内 While/WhileLet の受理証拠 — 2026-09-28

確認した code commit は `a3e4aab38cdd14c8f955e94e11a4b33e08620624`。
main へ fast-forward push 後、working tree は clean。

expression Match arm の値 block に `thread { while ... {} }` と
`thread { while let ... {} }` を置き、Thread body 下の HIR While/WhileLet と外側
Expression statement がいずれも checked Structural payload として Match path 配下
に到達することを確認した。同型 Bool 入力の変更がそれぞれ Match digest に届く。
普通の arm block 直下は HIR Thread 文脈ゲートで拒否されるままであり、この受理証拠は
その契約を変更しない。

focused Match 67件、Sema lib 1071/1071、changed-crate Clippy、fmt、cached diff
check は成功。既存警告あり。テストのみの cut なので workspace と構造 gate は
再実行していない。Wait/Suspension 等の残る T01/T06、View 以降と goal 全体は
未完として継続する。

## Match T01 dialogue Wait/Trigger/ControlTransfer の受理証拠 — 2026-09-28

確認した code commit は `2ef350f4d7ce7da82a70b0ca26cfbb6fc6aa8329`。
main へ fast-forward push 後、working tree は clean。

外側の expression Match arm に Dialogue application 全体を置き、その line-plan の
`wait(1s)` を FlowItem/Thread statement として受理した。accepted Match path 配下に
Wait/Suspension、On/Trigger、Out/ControlTransfer、Expression/Structural の正確な
HIR/checked 組があり、Match result は String。`wait(1s)` と `wait(2s)` で digest が
変わる。以前の `InvalidEvidence` は Match を on-mark body の内側へ置き、wait を
ordinary arm block にしてしまった fixture 配置が原因だった。

`cargo test -p arcweft-lang-sema` は全件成功（lib 1072件を含む）。fmt、cached diff
check、changed-crate Clippy も成功。既存警告あり。テストのみの cut なので workspace
と構造 gate は再実行していない。T01/T06 の残り、View 以降と goal 全体は未完として
継続する。

## Match T01 dialogue CancelRule の受理証拠 — 2026-09-28

確認した code commit は `08875ff79263a6c9012f1a8dcfb9081aca707aab`。
main へ fast-forward push 後、working tree は clean。

外側 Match arm の Dialogue application line-plan に `cancel on input(...)` を置き、
HIR CancelRule と checked Trigger が accepted path 配下にあることを確認した。
キャンセル規則なしとの差、同じ String result と Trigger family のまま `.SkipLine` と
`.BackToTitle` の InputAction ID を替えた差で Match digest が変わる。compact Choice
plan の cancel は expression-owned `HirChoicePlanItem::Cancel` であり、statement
CancelRule の受理証拠には数えない。

statement corpus 13/13、Sema lib 1073/1073 と crate integration/doc suites、
changed-crate Clippy、fmt、cached diff check は成功。既存警告あり。テストのみの
cut なので workspace と構造 gate は再実行していない。T01/T06 の残り、View 以降と
goal 全体は未完として継続する。

## Match T01 SourceLocale/Include statement の受理証拠 — 2026-09-28

確認した code commit は `57270ad5e57b22af824ad537f07c6436e9883753`。
main へ fast-forward push 後、working tree は clean。

外側 Match arm の nested Thread に SourceLocale と Include を置き、受理済み
HIR statement と exact checked payload を Match path 配下で確認した。
`en-US`/`ja-JP` の locale 差と `shared`/`alternate` の Flow target 差は
checked fact と Match digest の双方で観測された。Scope は named identity の
transcript writer が名前を落とす欠落を発見したため Pending のままとし、別の
本番修正で閉じる。

`cargo test -p arcweft-lang-sema` は lib 1075件と integration/doc suites を含め
全件成功。changed-crate Clippy、fmt、cached diff check も成功し、既存警告あり。
テストのみの cut なので workspace と構造 gate は再実行していない。T01/T06、
View 以降と goal 全体は未完として継続する。

## Match T01 named scope identity の transcript 修正 — 2026-09-29

確認した code commit は `bb937ea678170b79bdc7abd8f64b3cfbf3727750`。
main へ fast-forward push 後、working tree は clean。

`CheckedScopeIdentity::Named` に残る検証済み DeclarationName を共通
`write_scope_identity` が省いていたため、`scope local` と `scope scene` の
Match digest が衝突していた。DeclarationName の owner API で canonical bytes を
発行し、statement Scope と NamedBlock expression の双方へ length-framed で記録した。
両方を T01 corpus Accepted へ移し、name の意味差、anonymous 差、整形不変性を
実際の checked owner と Match digest で確認した。旧「scope label 無視」テストは
契約と矛盾するため新しい意味差分期待へ更新した。

focused 3件、acceptance module 55/55、workspace all-target/all-feature check と
Clippy、`just test-workspace`、`just structure-audit-gate`（blocking 0）、fmt、
cached diff check は成功。workspace test 初回は旧期待テスト1件で失敗し、修正後の
full rerun が成功した。既存 Clippy 警告と構造 review triggers 348件あり。
T01/T06、View 以降と goal 全体は未完として継続する。

## Match T01 Flow/control statement corpus — 2026-09-29

確認した code commit は `76304e636aee797bcbdbc79fe3ee87cdf6ebd2c5`。
main へ fast-forward push 後、working tree は clean。

通常 Flow の Match arm 配下で Goto/Structural、Yield/Yield、Break と
Continue/ControlTransfer を受理し、exact HIR/checked payload の組を T01 corpus
Accepted へ移した。Goto target、同型 Bool の Yield operand、Break と Continue の
操作差で Match digest が変わる。後二者は checked LoopExpression target family と
同じ安定 body coordinate も確認した。loop expression 自身の
Expression/Structural statement も同じ Match path に含まれる。

focused bundle と family matrix、`cargo test -p arcweft-lang-sema` 全件、
changed-crate Clippy（全 target/feature）、fmt、cached diff check は成功。
既存警告あり。テストのみの cut なので workspace と構造 gate は再実行していない。
Signal/LifetimeSet/statement-Match/Close/UnsafeLifetime 等と T01/T06、View 以降、
goal 全体は未完として継続する。

## Match C3 call generation を意味 digest から分離 — 2026-09-29

確認した code commit は `a9ea82b0680aad576b513de02746704064ba7ca9`。
main へ fast-forward push 後、working tree は clean。

`CheckedCallApplicationDigest` は選択済み catalog/source 世代を含む実行・検証用
identity であり、無関係な source 追加で変わる。Match transcript の
EvaluatedEffect statement、dialogue 即時/遅延 Call、ContentResult/Emission と Fx
outer/inner edge がその bytes を意味 atom として混入させていた。これを除き、既存の
checked child expression、stable selected callable join、引数・effect operation、
Fx の typed definition/binding payload を使用する。Fx semantic digest sealer からも
raw call application だけを除き、runtime carrier/sealed ref の厳密な世代照合は維持した。
受理済み project Fx は Function/Existing 宣言なので、その安定 declaration ID は保持した。

Match 配下の受理済み6回帰で、末尾の無関係宣言を追加すると runtime application
digest は変わるが Match digest は保たれ、同型の callable/operand/Fx binding 変更は
Match digest に届くことを確認した。EvaluatedEffect の exact statement payload、
dialogue Content/Delay(120ms) site、builtin/project Content、project Fx Content、ViewFx と
Fx 自身の semantic digest も確認した。focused 6/6、Sema lib 1084/1084、workspace
all-target/all-feature check と Clippy、`just test-workspace`、
`just structure-audit-gate`（blocking 0）、fmt、cached diff check は成功。
既存警告と size review trigger は残る。T01/T06、View 以降と goal 全体は未完として継続する。

## Match T01 dialogue/Content/View expression corpus — 2026-09-29

確認した code commit は `dcc7e1612984e9449b365912324ca863fb3c3bd2`。
main へ fast-forward push 後、working tree は clean。

直前の generation-invariance 回帰で受理した5種類の Match-root source を共有し、
各 accepted path 配下の HIR expression shape と checked resolution の完全な集合を
corpus 行ごとに固定した。dialogue 即時/遅延 Call、builtin/project Content、
project Fx Content、View Fx の到達により、AttachedContentApplication、
PostfixBracket、DialogueApplication、ContentApplication、ViewFxApplication、
CompileTimeScalar 等を実測に基づき Accepted へ移した。厳密な集合照合は明示 row
metadata で指定し、表示名の文字列規則には依存しない。意味差と世代不変性は直前の
同じ fixture の回帰を再利用した。

focused corpus、Sema lib 1084/1084 と UI/integration/doctest、changed-crate Clippy、
fmt、cached diff check は成功。既存警告あり。テストのみの cut なので workspace と
構造 gate は再実行していない。Effect resolution の declaration-only root 証拠は
別の witness として Pending に残す。T01/T06、View 以降と goal 全体は未完として継続する。

## Match T01 collection expression と Effect root の証拠 — 2026-09-29

確認した code commit は `7e0f0bc03134f684b49c8a287bab4da41d8558e5`。
main へ fast-forward push 後、working tree は clean。

受理済み Match path 配下で Range、一般 BracketSequence、ArrayRepeat、Vec Index の
HIR shape と checked resolution の完全な集合を確認し、同型の sequence 要素、repeat
値、index 選択の変更が Match digest に届くことを確かめた。同一 Function に
`effects { fs.read }` と body Match を置く witness は、checked Effect fact が
DeclarationContract path にあり、Match の DeclarationBody path 配下には入らないことを
確認した。Effect resolution は唯一の declaration effect-root producer と合わせ、
T01 の Match 子孫では ProvenUnreachable とした。Record/RecordLiteral/ShortVariant/Nominal
は本 cut に受理証拠がないため Pending のまま。

focused corpus 11/11、`cargo test -p arcweft-lang-sema` 全件、changed-crate Clippy、
fmt、cached diff check は成功。既存警告あり。テストのみの cut なので workspace と
構造 gate は再実行していない。

## Match T06 Choice scope の訂正 — 2026-09-29

凍結 Match 設計の `CUTS_TESTS_AND_DELETION.md` C1/C3/T06 は HIR Choice body root の
path/topology と、受理済み Match transcript の意味差を対象とする。同 C5 item 4 は
runtime/wire/persistence/task-plan consumer の新設を明示的に除外する。従って現行
Sema が拒否する非compact Choice 全体の native/AWBC 実行移行を、本工程の完了条件へ
追加しない。非compact HIR role は C1 の構造証拠と現在の拒否境界で確認し、T06 の
正例は受理済み compact Choice の Timeout/Cancel/OnSelect body・pattern root と
source order を差分検証する。これらの正例は未検証であり、T06 は未完のまま。
View parameter default も現行の別 producer/consumer gap として保持する。

## Match T06 compact Choice plan body roots — 2026-09-29

確認した code commit は `2b61eee27a429531e96915df44c717e2b9023c8c`。
main へ fast-forward push 後、working tree は clean。

受理済み compact Choice を Match arm に置き、`with` plan の Timeout body、Cancel
trigger pattern/body、OnSelect pattern/body が source-order ordinal を保つ typed role と
Match 子孫の accepted coordinate に到達することを確認した。各 body の値だけを変更すると
checked Choice の public/option ID、Goto target、plan rows は同じまま Match digest が
変わる。plan 行順の変更でも digest が変わり、整形・無関係な先行宣言による raw ID/span
変更では digest が保たれる。Cancel/OnSelect pattern の checked 型も検証した。

focused test と Sema lib 1087/1087、changed-crate Clippy、fmt、cached diff check は
成功。既存警告あり。テストのみの cut なので workspace と構造 gate は再実行していない。
T06 には View parameter/default/body/value の明示matrix、とりわけ現行非受理の
View default が残る。T01、View 以降と goal 全体も未完として継続する。

## チャット引き継ぎと実行 goal の再設定 — 2026-09-30

前チャット `01a079db-bda6-7c73-8f3b-99a7747d54f3` の履歴と現在の Git を照合し、
チャット `01a0f2b3-0f85-7bf1-b952-a20276132d1f` へ作業を引き継いだ。
確認した HEAD は `6905cc99b59d24d53a6ab546a4d3021ffd18a63a`。既存 `main` で
fetch 後の HEAD/origin/main の差は 0/0、index は空。working tree には
`crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/statements.rs`
だけ未コミット差分があり、Signal、LifetimeSet、UnsafeLifetime、Close、
statement-Match の受理 corpus と exact payload/digest 差分検証を保持した。
この時点では当該差分の検証を開始した段階であり、合格とは扱わない。

新チャットには goal が存在しなかったため、上記「goal の完了条件」を維持する
active goal を設定した。直近は Generic Match C3/C5、T01 の live family ごとの
受理・到達不能・拒否証拠、および T06 の body-root path/digest を閉じる。
その後は retained View .1.4、RuntimePlan/task-plan .1.3.1、構造的 nominal
C1-C6、scheduler/restore A-F の依存順と最終検証を維持する。
引き継ぎや個別テストの成功を全体 goal の完了に読み替えない。

2026-09-30 のユーザー指示を後続チャットにも引き継ぐ。Sol を指定する場合は
必ず `gpt-6.1-sol` を使用し、Sol Max はその reasoning effort `max` とする。
Astra Max (`gpt-6-astra` / `max`) への助言依頼は、goal の区切り等で非常に難しい
設計判断に不安がある場合に限り許可されている。その場合は履歴を引き継がない
新しい context とし、必要な設計事実・契約・相談点だけを渡す。
この例外を通常の実装委任や常時レビューへ拡張しない。

保持した statement corpus は focused test 18/18、`cargo test -p arcweft-lang-sema`
全件（lib 1088件、API/integration 24件）、
`cargo clippy -p arcweft-lang-sema --all-targets --all-features`、
`cargo fmt --all -- --check`、diff check を通過した。最初の fmt check は失敗し、
当該テストファイルの整形後に再実行して成功した。Clippy は警告を残して成功しており、
新しい table-driven test にも `too_many_lines` 警告がある。lint の抑制は追加していない。
本 cut はテストと引き継ぎ記録だけなので workspace と構造 gate は再実行していない。
検証対象のテストファイル blob は `0602f31badbc84441d2277d48334ddafa4e7814a`。

現行 inventory は expression/value/select 側が Accepted 54、Pending 28、
ProvenUnreachable 1、RejectOnly 1、statement 側が Accepted 41、Pending 5、
RejectOnly 1。family disposition の件数であり、実装全体の完成率ではない。
次は statement Choice/Select/ProofCall、EvaluatedEffect/Select payload の
残件と expression/value/select 側の Pending を live owner/producer から確定し、
View parameter/default/body/value の未完境界へ進む。

上記の引き継ぎと検証済み statement corpus は
`16dcf822dc72f68fa1e868764bca96864a0d33c7` で main へ fast-forward push 済み。
push 後は working tree clean、HEAD/origin/main の差は 0/0。
goal は作成時には active だったが、そのターンが
`Selected model is at capacity. Please try a different model.` で失敗した後、
goal API の取得結果は blocked になっていた。実装上の阻害条件を確認して
blocked に変更したものではなく、goal を complete にもしていない。
継続ターンでは上記検証・統合を完了したが、公開されている goal 更新ツールには
active への再開操作がないため、自動継続の再開にはユーザーまたはシステム側の操作が必要。

## Match T01 statement family の残件解消 — 2026-10-01

確認した base は `974de216c5c0e89b1fa0c2fde4e9f67302a48120`、既存 main の
working tree は clean だった。goal はユーザー/システムによる再開後に active を
返しており、9月30日の容量エラーによる停止状態は解消している。

code commit `d2325f46ea98e160907d964d60d6175a29a1cfde` を main へ
fast-forward push 済み。push 後の working tree は clean。
Match arm 下の Thread で Choice statement と Select operand/branch statement を
受理し、EvaluatedEffect expression statement を含む exact HIR/checked payload の組を
確認した。同型の Goto target、Select operand/branch source、log operand の変更は
Match digest を変える。Select の Bind/Frame head と枝順も checked fact に保持され、
枝順の変更が digest に届く。

ProofCall を Match arm で受理する最初の候補は失敗した。parser の唯一の分類元は
宣言 ProofBlock の文脈でのみ ProofCall を生成し、expression block の文法は
FunctionItem 文脈なので、proof 内の Match arm の同じ call は普通の Expression になる。
同じ proof に direct ProofCall と Match 内の call を置いた witness を受理し、direct
ProofCall の checked path が Match path 外であることを確認した。direct call の
選択先を変更しても Match digest は同じで、arm 内の選択先変更では変わる。
この producer 境界と実行可能な witness に基づき ProofCall は ProvenUnreachable とした。

現行 statement inventory は Accepted 45、ProvenUnreachable 1、RejectOnly 1、
Pending 0。使わなくなった Pending disposition と dead-code lint 抑制を削除した。
これは T01 statement 軸の証拠の閉包であり、expression/value/select と pattern 軸、
T06 View default、および後続工程の完了を意味しない。

focused statement 20/20、`cargo test -p arcweft-lang-sema` 全件（lib 1090件、
API/integration 24件）、changed-crate Clippy（all target/feature）、fmt、cached diff
check は成功。Clippy 警告は残る。テストのみで責務・公開 API・依存は変わらず、
workspace と構造 gate は再実行していない。次は通常式・nominal construction 等の
expression corpus の残件を確認し、T06 View の producer/consumer 境界へ進む。

## Match T01 通常の制御式・record・借用・合成式 corpus — 2026-10-01

確認した base は `5f440fe93a68a6a68af7b3b86e4a396d010d655f`、working tree clean。
code commit `95f55eef108cacbfb54133d21e631c796c76e870` を main へ
fast-forward push し、push 後は working tree clean、HEAD/origin/main の差は 0/0。

Match 配下の Unary、If、IfLet、Loop、ComputationBlock、Record、RecordLiteral、
Borrow、Dereference、ForSynthetic の10 shape と checked Nominal resolution を
9つの受理済み source で検証し、Accepted へ移した。各行は accepted path 全体の
HIR shape/checked resolution 集合を厳密に比較する。同型の operand/branch/break/carrier
tail/record field/borrowed value/iterable の変更が Match digest に届く。
先行する無関係な宣言と整形変更は raw Match ExprId/span を変えるが、digest を保つ。

最初の nominal 型比較は revision を含む raw TypeKind の不一致で失敗したため、
owner API の `semantic_identity_digest` で同じ意味上の型を比較するよう修正した。
Match arm 直下の `{ first = ... }` はブロックとして解析され、候補 source は
RecoveredModule で失敗した。既存の式文法で括弧により record literal を明示した
`({ first = ... })` は受理され、RecordLiteral shape と Nominal payload を保持する。
文法や型契約を今回のテストに合わせて変更したものではない。

expression corpus 12/12、`cargo test -p arcweft-lang-sema` 全件（lib 1091件、
API/integration 24件）、changed-crate Clippy（all target/feature）、fmt、cached diff
check は成功。Clippy 警告は残る。テストのみで責務・公開 API・依存は変わらず、
workspace と構造 gate は再実行していない。

現行 expression/value/select inventory は Accepted 65、Pending 17、
ProvenUnreachable 1、RejectOnly 1。statement inventory は前 cut の
Accepted 45、ProvenUnreachable 1、RejectOnly 1、Pending 0 を維持する。
次は既存の受理例を持つ Await、implicit callable/parameter、dialogue coordinate と
character factory/reconfigure、compile-time enum/style/type value、constant、AgentField、
LifetimePath/ShortVariant の17残件を accepted Match path で確定する。
T06 View parameter/default/body/value の完全な matrix と View default の移行、
retained View、task-plan、nominal、scheduler/restore、最終検証は引き続き未完であり、
goal は active のまま継続する。

## Match T01 contextual expression の閉包 — 2026-10-01

確認した base は `da470eb3a224649c974343e90dcaeb6662c8188d`、既存 main の
working tree は clean だった。code commit
`be114b5284f2b54efcb1f3f5db63e91e39877fb0` を main へ fast-forward push 済み。
push 後は working tree clean、HEAD/origin/main の差は 0/0。

12の source/differential 行を shared expression corpus へ接続し、Await、implicit
callable/parameter、ShortVariant、StageLook、dialogue の line/text-key coordinate・
line reference・Character factory/reconfigure、Object の nominal TypeValue と public-ID
Constant、builtin Fx の CompileTimeEnum、AgentField を Match 子孫で受理した。
各行は accepted path 全体の HIR shape/checked resolution 集合を固定する。
Need operand、implicit body、variant case、dialogue coordinate/reference/locale、Agent field、
Object type/ID、Fx speed の同型差が digest を変え、整形・無関係な先行宣言で raw
Match ExprId/span が変わっても digest は保たれる。StageLook 行の差分は locale を変え、
登録済み `.normal` look 自身は同じである。

Agent 候補の裸の Observation 型名は TypeResolutionFailed、tick/frame_id を同じ
u64 結果として扱う候補は ExpressionTypeUnavailable で失敗した。実際の登録環境の
Observation 値を使い、両方 String の state_hash/render_hash を比較する fixture に修正した。
closed Fx enum の候補にも未受理の constructor/context や不適合な phase があり失敗した。
受理証拠は既存の Content Fx producer と schema が定義する
`#fx(wave(phase=.glyph_transform, speed=...))` で確定した。テストに合わせた本番変更や
合成 checked fact の注入は行っていない。

LifetimePath は Match の直接の child であることを HIR API で確認したうえで、唯一の
値 checker による ExpressionTypeUnavailable と final report 非公開を確認し RejectOnly
へ移した。StyleValue は Style declaration の直接の property root にのみ発行される。
同じ project に Style Color property と function Match を置いた witness を受理し、
StyleValue の checked path が Match path 外であることと、Match corpus に入らないことを
確認した。この producer 境界に基づき ProvenUnreachable とした。

expression/value/select 軸は Accepted 80、ProvenUnreachable 2、RejectOnly 2、
Pending 0。使わなくなった Pending disposition を削除した。statement 軸は前 cut の
Accepted 45、ProvenUnreachable 1、RejectOnly 1、Pending 0 を維持する。既存 pattern
inventory の exhaustive `of` と受理 matrix も残しており、family disposition の未確定行はない。
この閉包を Generic Match C3/C5 全体や T06、後続工程の完了とは扱わない。

expression corpus 15/15、`cargo test -p arcweft-lang-sema` 全件（lib 1094件、
API/integration 24件）、changed-crate Clippy（all target/feature）、fmt、cached diff
check は成功。Clippy 警告は残る。テストのみで責務・公開 API・依存は変わらず、
workspace と構造 gate は再実行していない。

新しい `contextual_expressions.rs` は arcweft-lang-sema の test module、16,576 bytes、
498 physical LOC（base 0）。300-LOC growth trigger に対する責務レビューでは、
enclosing context が選択する checked family の source fixture と意味差/到達境界の証拠を
一つの test owner に分けたと判断した。parent の inventory・座標 traversal・observation を
再利用し、独自の registry、source resolver、fact sealer、production state は持たない。
module fan-in は parent expression corpus のみで、fan-out は既存 fixture/analyzer/HIR/
checked-coordinate API と既存 Character/Agent 型である。外部公開を増やさず、
`pub(super)` の corpus-row 接続だけを追加した。Cargo dependency graph は未変更なので
scanner による再計測は不要と判断した。

次は T06 View parameter/default/body/value の全 matrix と、現行 builder/compiler が
拒否する View default の producer/consumer 移行を進め、C3/C5 の残る受入条件を照合する。
retained View .1.4、task-plan .1.3.1、nominal C1-C6、scheduler/restore A-F と
全体検証は未完のまま保持し、goal は active で継続する。

## Match T06 View roots / declaration defaults — 2026-10-01

Inspected base: `de7aed2d7dc39d8bd7453fe81c9aefb52015f1d7`、既存 `main`、開始時
clean。今回の変更は semantic default authority と T06 の受入証拠であり、goal 全体と
retained View `.1.4` の実行接続は未完。

Supersedes: 直前までの「View default は builder が拒否する」という現行状態。
View は `Defaulted` parameter を登録し、登録済み callable schema の型で default を
contextual check する。通常 function parameter default の拒否は維持する。raw type
annotation には omitted function effect row が残るため、登録済み parameter 型を
expected authority とした。default row は expected/result 型 identity を別々に保持する。

添付本文専用だった checked default 型・capture 型・式 digest を
`CheckedDeclarationDefault` family に統合した。View parameter defaults は最終
`CheckedCallableFacts` に parameter coordinate 順で入り、添付本文 row と同じ
interface sealing transaction で complete batch を検証・公開する。式 digest は
interface を seal する前の既存 acyclic transcript から導き、default から callable
interface を再帰的に hash しない。domain/version は `v1`。旧型 alias・旧 reader は
残さない。RuntimePlan の変更は既存の添付本文 default consumer の型名移行だけで、
C5 が禁止する新しい View runtime/wire/task-plan consumer は追加していない。

Default の free inputs は既存の selected execution inventory と、明示 closure / implicit
callable の checked capture から導く。先行 parameter だけを許可し、自己参照と
後続参照を拒否する。後続参照には既存 lexical lookup で
`ExpressionTypeUnavailable` になるものもあり、受理した capture の順序検査と区別する。
View default は外部効果を実行せず、non-suspending とする。Thread を作る default と
Await default は effects 検査で拒否する。Await のこの witness は effects が先に失敗する
ため、独立した suspension-only rejection の証明としては扱わない。

`view_roots` の matrix は View parameter pattern と binding、３つの authored View value
root、Match の位置、handler closure capture を accepted topology と checked coordinate
で照合する。Fx input または handler capture の参照先変更、View value の順序変更は
Match digest を変え、隣接 Text の変更、整形と無関係な先行宣言による raw ID/span の
変更は Match の意味を変えない。`view_defaults` は String、純粋な project call、tuple、
nominal、contextual enum、明示/暗黙 callable、先行入力の連鎖、default を持つ View の
Fx binding を受理する。default 内 Match の parameter-default path、式/interface digest
の意味変化と source-revision invariance、および型不一致・自己/後続/latent forward
input・効果実行の拒否を実行証拠にした。

構造: production の default derivation と共通 free-input projection を
`final_analysis/declaration_defaults.rs` にまとめた。これは新しい parallel capture walk
ではなく、旧 report 内の添付本文 capture producer を移し、View も同じ producer を
使うもの。331 physical LOC / 14,146 bytes、embedded tests 0、fan-in は report、fan-out
は既存 HIR/sema typed facts と transcript のみで I/O/dependency 追加なし。
`checked_catalog.rs` は 2,730 → 2,838 LOC / 104,427 bytes、report は 2,572 → 2,403 LOC /
96,807 bytes。前者は checked callable/default/interface の単一 authority と transactional
publication の cohesive owner、後者は final report の assembly owner として保持する。
test leaf は `view_defaults.rs` 198 LOC / 7,416 bytes、`view_roots.rs` 272 LOC /
9,667 bytes。いずれも runtime API を広げない。

検証: focused View matrix ６件、最終 source の `cargo check --workspace --all-targets
--all-features`、同 Clippy、`cargo fmt --all --check`、diff check が合格。
`just test-workspace` は全 recipe が終了コード 0、sema lib 1,100 件を含めて合格した。
集計は 308 test suites、7,125 passed / 0 failed / 24 ignored。
Windows の既知の compiler test stack 条件に合わせ、workspace test に
`RUST_MIN_STACK=16777216` を指定した。最初の workspace run は追加 fixture の raw
string 編集ミスで compile 失敗したが、修正後に全 recipe を再実行して合格した。
Clippy は warnings あり（既存 owner、移した capture producer、大きい共通 error 等）。
`just structure-audit-gate` は 97 packages、blocking violations 0、review triggers 348。
その後の小さい検証/fixture 修正は dependency、API、owner/test boundary を変えないため
構造結果を再利用し、上記の最終 physical LOC/bytes と owner disposition を記録した。

実行側の残り: compiler の default boundary は `ViewLower / compiler.view.lower` で
fail closed を維持し、２つの compiler fixture を registration rejection から更新した。
runtime default 評価の合格証拠ではない。ユーザー定義 View call の追加 probe は
`ProjectItem(View)` に解決され、`CallTargetFacts` は selected application を持たなかった。
これは `.1.4` の production producer 移行対象であり、その probe を受理テストとして
残したり、成功として数えたりしない。

次の設計方針: ユーザーの条件付き許可に従い、この難しい layer/value boundary だけ
fresh-context の `gpt-6-astra` / Max に advice を求めた。advice agent は編集・検証をして
いない。採用する方向は既存 `RuntimePureProgram` の root/input ABI を checked expression
と declaration default へ一般化し、View の一般値に既存 `RuntimeValue` / semantic type
を用いること。Fx evaluator は actual Fx ABI に限定し、View-owned fragment program と
一般 data slot の役割を分ける。scalar mount storage と named runtime parameter の二重
authority を消し、supplied/defaulted provenance と依存 revision から省略値を再計算する。
Need の affine guard は弱めず、Cut-5 の handle/current-owner admission、observer と
replacement/save transaction を `.1.3.1` と接続する。scalar Await、I32-count Repeat、
name-based BindLocal は typed operation/slot の consumer が完成した切替で削除する。

Delivery: code/evidence commit は `6b60aa38624ebea505aaa1859a939e00330ab120`。
commit 後の `main` working tree は clean で、検証した source と一致した。この View の
T06 semantic matrix を閉じ、T01 Pending 0 と合わせて C3/C5 の残る受入照合から `.1.4`
の producer/value/runtime migration へ継続する。`.1.3.1`、nominal、scheduler/restore と
最終全体検証も残り、goal は active のまま保持する。

### 2026-10-01 — retained View callable producer

Inspected base: `c24d76b360ad3311601a071abdc432c21aa3a086`、`main == origin/main`、
着手時 clean。以下は同じ checkout の in-scope dirty patch を検証する cut。
前 goal turn は T06/default authority を source と実行証拠で進めた progress。
今回も goal の全範囲を維持し、`.1.4` の実行側を完了扱いにはしない。

Supersedes: 直前の user View call probe が `ProjectItem(View)` / NonCallable に
落ちる現行状態。retained owner が package/module/name から callable identity を
一度だけ生成・保持し、View の非 binding callable row は、その owner/item/module
に一致する関係を消費する。scope binding は一つの Retained target のまま。
value projection は通常・import・ambiguous・inaccessible の各選択前に同じ関係を
投影し、登録済み callable catalog の project binding もこの API を使う。
entity reference は同じ owner の public ID を保持する。

View callee は `CompileTimeCallableType::ProjectView` の semantic token、呼び出し結果は
`ViewValue`、checked execution は `RetainedView` とした。registered schema/result と
execution family を pending/final catalog の両段階で照合し、interface/transcript の
閉じた encoder に同じ role を記録する。すべての domain/version は `1`。
通常の function arrow、Core function allocation、runtime function root に投影しない。
所有権 classifier も token を MissingRuntimeSnapshotOwner として拒否する。

直接・import・修飾 path は既存 exact project resolver と binding provenance を使い、
local 別名・block 結果は token の exact declaration で同じ resolver に入る。
resolver の binding evidence だけを optional にし、引数 mapper、candidate constraint、
選択・execution projection・final seal は既存の共通経路を使う。
selected callee staging は token と元の local/structural resolution を保持する。
builtin View head の手前でも通常の local/project lookup と visibility を尊重する。
非 builtin の Select/dot/associated callee を View checker が先に値評価する経路を
削除し、共通 callee checker に統合した。これは qualified module receiver を誤って
値評価していた failure の修正でもある。

ユーザーの条件付き許可に従い、この難しい callable/value boundary だけ
fresh-context `gpt-6-astra` / Max へ advice を求めた。advice agent は編集・検証・Git
操作をしていない。今後 Sol を指定する相談/チャットには `gpt-6.1-sol` を使う。
runtime function と retained entity のどちらかへ無理に統合せず、既存 signature と
typed retained execution を組み合わせる判断を採用した。

証拠: focused sema 5 tests が直接・named・default omission・local/block alias・import・
qualified path・project/local Text shadowing を受理し、必須/未知/型不一致/余分な supply
を拒否する。source order と parameter destination の入替、compact spread の exact
element projection を production execution facts で照合する。Match transcript は選択先
変更に反応し、整形・無関係な先行宣言による raw allocation の変更で不変。
HIR regression は単一 binding、owner/callable join、entity identity、private View の
inaccessible candidate projection を検査する。compiler fixture は user View call も
`ViewLower / compiler.view.lower` で fail closed するよう拡張した。

構造: 新しい dependency、I/O、parallel symbol index、名前ベース resolver はない。
既存の HIR symbol owner、sema type algebra、checked callable catalog、callee transaction
の責任に配置し、Core callable carrier を増やさない。新しい test leaf は parent の
acceptance fixture を使う 205 LOC / 7,346 bytes（test-only、production dependency なし）。
主要 owner の現物測定は以下。embedded test の新規追加は既存 HIR test leaf と新しい
sema test leaf、compiler integration fixture に属する。

| Owner | Base physical LOC | Current physical LOC | Current bytes |
| --- | ---: | ---: | ---: |
| HIR `symbol/identity.rs` | 1,270 | 1,290 | 40,699 |
| HIR `symbol/table.rs` | 2,058 | 2,082 | 76,873 |
| HIR `symbol/table/publication.rs` | 1,032 | 1,028 | 38,750 |
| HIR `symbol/tests/symbol_projection.rs` | 1,071 | 1,114 | 40,536 |
| sema `callable/checked_catalog.rs` | 2,838 | 2,862 | 105,380 |
| sema `callable/resolver/preparation.rs` | 830 | 862 | 35,805 |
| sema `final_analysis/analyzer/calls.rs` | 5,756 | 5,775 | 251,151 |
| sema `final_analysis/analyzer/expressions.rs` | 4,941 | 4,919 | 212,869 |
| sema `types.rs` | 1,799 | 1,812 | 60,325 |
| compiler `tests/view_product.rs` | 1,120 | 1,121 | 40,080 |

上限を超える既存 catalog/call/expression owner は、それぞれ sealed callable authority、
atomic candidate transaction、checked expression-family dispatch の cohesive owner として
保持する。今回追加する state cluster はなく、共通 callee boundary を移行して既存の
重複先行評価を削除する。ファイル分割のための API widening はしない。
`just structure-audit-gate` は 97 packages、blocking violations 0、review triggers 348。

検証: production patch の `cargo check --workspace --all-targets --all-features`、
同 Clippy、format/diff check、focused sema 5 tests は合格。Clippy は既存の大きい
owner 等の warnings あり。初期 focused 試行の型/API・fixture 記述ミスと、発見した
callee staging/qualified receiver failure は修正後に再実行して合格した。
最初の `just test-workspace` は完了結果の取得前に execution handle が消え、実際の
cargo/just/rustc process も終了していた。合格扱いにせず元ログを保持し、
`RUST_MIN_STACK=16777216` で再実行した。２回目は新 HIR fixture が暗黙 public ID を
`view.child.Public` と推測していたため 925 passed / 1 failed / 8 ignored で失敗。
published retained owner の public ID を使う exact join の検査へ修正し、focused HIR
regression 1 test は合格。修正後の全 recipe は終了コード 0 で完了した。
308 suites、7,131 passed / 0 failed / 24 ignored、sema lib 1,105 件、HIR lib
926 passed / 0 failed / 8 ignored を含む。成功ログは
`%TEMP%/arcweft-1001-view-call-workspace-test-final2.log`、対応する `.exit` は `0`。
最終 test fixture の修正も含めた workspace check/Clippy/format の再確認も合格。
最終ログは `%TEMP%/arcweft-1001-view-call-workspace-check-final.log` と
`%TEMP%/arcweft-1001-view-call-clippy-final.log`。構造結果は API/dependency/owner が
同じため再利用し、修正した test leaf の現物 bytes は上表へ反映した。

残り: この cut は semantic callable producer の移行。View instructions、一般値
slot と expression/default の `RuntimePureProgram` root/input ABI、nested CallView、
Need observer と replacement/save の Cut-5 は未完了。compiler fixture の拒否は
runtime 実行の合格証拠ではない。次は checked input authority と runtime reachability
を一般 expression root に接続し、最終 typed consumer で scalar-only mount/value と
旧 Await/Repeat/BindLocal を削除する。`.1.3.1`、nominal、scheduler/restore と最終
全体受入検証も残るため、goal は active を保持する。

次の consumer join で移行する現行境界: `compiler/view.rs::prepare_authored_view` は
DerivedFromName の View ID を module/name から再生成しており、HIR header/publication
の public ID と異なる。新 HIR regression の失敗でこの差を確認した。maintained View
contract の module-scoped identity を final typed publication に閉じ、compiler はその
published identity を消費する必要がある。module/name 再生成を最終 consumer に残さない。

### 2026-10-01 — final retained View identity and UI audit intake

Inspected base: `eb8fee0a33b506a24ecafe625dd11d1b571dfb0b`、`main == origin/main`、
着手時 clean。同じ checkout でこの identity cut の source/test/doc dirty patch を検証。
Supersedes: 直前の HIR/global identity と compiler/module identity の不一致。
Final HIR retained header が canonical module/name から `view.<module>.<name>` を
一度生成し、explicit public ID は維持する。global identity の他 retained family は
既存契約を維持する。namespace derivation は ID owner の同じ PublicId grammar を使う。
module segment `std` は `view.std.Card` の nested component として許される。

freeze の ItemValidationContext は実際の canonical module を持ち、同じ header
projection と全体一致を検査する。DerivedFromName を無条件で認める旧 validation、
別 public-ID-issue 比較、compiler の module/name 再生成、ViewId の module/name
constructor を削除した。compiler は published PublicId を ViewId の typed family
conversion へ渡す。二重 identity index、名前の fallback、旧 signature alias はない。

受入証拠: HIR は別 module の同名 Card を `view.a.Card` / `view.b.Card` に分離し、
entity/callable/source owner の exact join を検査する。別 module で生成した header の
差込みは final freeze が拒否する。compiler fixture は同名 Card、明示 `view.authored`、
複数 root/module を含み、final HIR、retained symbols、bundle definition の ID 集合を
一致させる。body span の順序は final HIR の canonical module/source inventory で検査
する。bundle definition の ID-sorted lookup order は source order の正本ではない。

構造: production owner は既存 ID grammar、HIR declaration projection/freeze、compiler
product join、View typed ID。依存追加、I/O、parallel schema/state はない。validation
context の module field 追加は実際の freeze invariant に必要で、物理分割のための API
widening ではない。大きい retained/item projection/module/view compiler owner はそれぞれ
declaration algebra、attached-to-final freeze validation、module atomic publication、
accepted View lowering の cohesive owner として保持する。test は同じ owner の既存
leaf/integration fixture に置く。各 file の growth は 300 LOC 未満。

| Owner | Base physical LOC | Current physical LOC | Current bytes | Classification |
| --- | ---: | ---: | ---: | --- |
| id `src/lib.rs` | 708 | 763 | 24,117 | production + embedded tests |
| HIR `item/retained.rs` | 1,544 | 1,569 | 48,392 | production |
| HIR `source_index/item_projection.rs` | 1,494 | 1,437 | 55,875 | production |
| HIR `module.rs` | 2,122 | 2,123 | 85,692 | production |
| project-loader `environment.rs` | 937 | 938 | 32,672 | production |
| compiler `src/view.rs` | 1,426 | 1,411 | 57,065 | production |
| View `view/identity.rs` | 454 | 415 | 13,221 | production + embedded tests |
| HIR lowering `tests/view.rs` | 454 | 506 | 16,930 | test leaf |
| HIR `item/tests.rs` | 1,021 | 1,078 | 37,196 | test leaf |
| HIR `symbol/tests/symbol_projection.rs` | 1,114 | 1,153 | 42,179 | test leaf |
| compiler `tests/view_product.rs` | 1,121 | 1,182 | 42,054 | integration tests |

embedded test LOC は id 192、View identity 130。fan-in/out と layer direction は
最終 `just structure-audit-gate` で再確認し、97 packages、348 review triggers、
blocking violations 0。ログは `%TEMP%/arcweft-1001-view-identity-structure-final.log`。

最終 `cargo check --workspace --all-targets --all-features`、同 Clippy、format/diff check
は合格。ログは `%TEMP%/arcweft-1001-view-identity-check-final2.log` と
`%TEMP%/arcweft-1001-view-identity-clippy-final2.log`。Clippy warnings あり。
focused HIR View tests は 16 passed、compiler canonical-module/source-order fixture は
1 passed。初期試行の constructor 名/analysis lease API の誤認を修正し、compiler の
ID-sorted lookup iteration を source order とみなす assertion も final HIR inventory を
使う証明へ修正した後に合格した。この fixture 修正では production の final ID
authority を変更していない。
最終 `just test-workspace` は `RUST_MIN_STACK=16777216` で終了コード 0。
308 suites、7,134 passed / 0 failed / 24 ignored、HIR lib は 929 passed / 8 ignored、
sema lib は 1,105 passed。成功ログは
`%TEMP%/arcweft-1001-view-identity-workspace-test.log`、対応する `.exit` は `0`。
最終 Rust source/test の変更後に全 recipe を実行し、その後の変更はこの evidence と
View chapter の prose のみ。changed content/link/format と staged diff を確認した。
GPU benchmark、UI audit の実行時反例/未達 consumer はこの identity cut の合格証拠に
含めない。fetch 後も `origin/main` は inspected base と一致した。

UI audit intake: ユーザー指定の [UI監査結果まとめ](chatgpt-conversation://6abdc93b-4880-83e8-b6fa-a4b71236660b)
を read_thread で取得した。監査の固定 base は上記 SHA と一致する。取得できた本文は
20,000 characters（sections 1–8 と section 9 冒頭、tool による末尾 truncation、older
page なし）。取得できない末尾を読了扱いにはしない。以下は主要指摘を現物に照合した
intake であり、外部静的監査を runtime/GPU 測定結果として扱わない。

| Audit boundary | 現行 evidence / goal の受入条件 |
| --- | --- |
| definition vs occurrence identity | この cut は definition ID の final authority を閉じる。`BundleViewInstancePathSegment` と replacement/reconcile は依然 instruction index を使う。compiler-accepted stable site + typed item path を state/style/resource/geometry/paint/input/Agent へ通し、前方挿入・並替・direct repeated Text/Button で同一性を検証する。今回で node identity 完了とはしない。 |
| live replacement | `session/hot_swap.rs` は View/Style 変更を CodeGenerational とし旧 presentation を維持する。lower replacement は StyleProgramChanged と AWBC-less candidate catalog の制約を持つ。handler runtime/Style/text/resource/state migration を完全 candidate に含め、表示・input/action/focus/capture/editing を同じ publication で commit する。 |
| general values/defaults/Need | 既採用 RuntimeValue/RuntimePureProgram と free-input ABI の移行に統合する。per-definition storage、supplied/default provenance と dependency revision、typed Repeat item/binding/key、ordinary Need Match/observer、typed Binding/event payload を閉じ、scalar storage/named mirror/旧 Await/count Repeat を consumer 切替で削除する。 |
| cache/fuel transparency | `ViewMountState::evaluate` の hit は budget を消費せず、from_snapshot は cache を空にする。canonical semantic fuel を hit/miss/backend で一致させる。warm/cold/evicted/restored が同じ limit で同じ成功/診断/publication を返す実行 fixture が必要。監査の 17-mount 反例は未実行で、source-level counterexample として扱う。 |
| retained assets/frame transaction | Glyphon candidate fork は project font bytes を clone して font database/system を再構築し cache も clone する。immutable assets の共有と disposable derived cache を semantic state transaction から分離する。frame preparation の費用が毎回 font inventory 総量へ比例しない実行証拠を必要とする。OS fallback の問題とは扱わない。 |
| geometry/text/GPU resources | node ごとの resource 全検索と ancestor 再走査を typed product join と段階別 invalidation へ置換する。既存 text layout を actual intrinsic measurement と paint/hit/selection に共有する。direct renderer の image upload/resource lifetime を retained asset に合わせ、content/geometry/input parity と upload/work counters を検証する。 |
| virtualization | range planner 単体を complete としない。Scroll occurrence と同じ identity で window evaluation/layout/paint を接続し、off-window state policy、focus/IME/capture pin、variable-height anchor correction の deterministic commit を検証する。 |
| logical clock/numeric profile | FxLogicalTime は f32 秒を累積している。Core 共通の整数 timestamp と activation 差分を正本にし sampler 境界で変換する。長時間/同総 dt の分割差 fixture、CPU state/geometry profile と GPU pixel 許容差の別検証を必要とする。監査の算術再現をアプリ測定とは数えない。 |
| restored external invocation | dispatch は route/event/target を照合し invocation raw_epoch を照合しない。通常 player での実害は未実証。restore/replacement の publication incarnation と外部 interaction lease を論理 replay ID から分離し、保持された古い invocation を拒否する lower API fixture で閉じる。内部 queue clear だけを外部寿命の証拠としない。 |

この受入条件を `.1.4`、`.1.3.1` の owner/transaction と scheduler/restore A–F に
接続し、既存 goal の全範囲を維持する。stable View chapter の target を同期した。
cache を保存して意味論差を隠す、instruction/source 位置から移行を推測する、scalar
旧系へ個別回避策を追加する方向は採用しない。根本 boundary を consumer ごと移行する。
性能については変更波及に応じた work/asset reuse を測り、常に O(変更数) との未実証の
保証はしない。次は general expression/default の complete checked input authority と
runtime reachability を接続する。runtime 実行、UI audit の未達、`.1.3.1`、nominal、
scheduler/restore、最終全体検証が残るため goal は active のまま。

### 2026-10-01 — checked executable local-input migration (in progress)

Inspected base: `5acefacb629f5b286afc498340adceff109d883b`、main、着手時 clean。
直前の goal turn は definition identity の修正・全検証・push を完了した progress。
この cut は同じ checkout の in-scope dirty patch。まだ commit/push していない。

baseline の View default `Label { value }` は、先行 parameter を読むにもかかわらず
default capture 0 を返すことを実行再現した。共通 free-local collector と prepared/final
record source の射影を接続した後、単純 default/block/local exclusion/explicit closure/
field selection は合格し、その時点の sema lib は 1,106 passed。後続変更があるので
最終 source の合格とは数えない。`(_, Label { value })` は default dependency 1 でも
implicit callable 自身の capture packet が 0 であることを追加 assertion で実行再現。
named Call root の `choose(_, ...)` は placeholder region の成立条件から外れる fixture
だったため、新しい call-root placeholder 仕様を導入せず valid Tuple root で切り分けた。

ユーザーの条件付き許可に従い、この大きい input-site/C1/transaction/runtime boundary
だけ fresh-context `gpt-6-astra` / Max へ設計助言を求めた。助言 agent は読取のみ。
採用する final model は既存 Expression/RecordField/Capture/StatementCapture site を
共通 input projection の責任へ移し、site/source binding type/origin/access を保持する。
open generic の Copy/Move は closed instance の local-use admission に残す。
C1 structural owner の全 record slot を閉じ、C2 は runtime field と stable source を
同じ slot に join する。expression-only implicit ledger/occurrence/expected-use を
site に移行し、candidate journal/rollback/extract/apply/drain と最終検証も同時に揃える。
semantic order は selected input traversal が持ち、candidate の発見順を正本にしない。
runtime は同じ binding frame と site の ownership proof を読む。default 専用の別 collector、
field と local iterator の位置合わせ、C1/C2 の独立 identity 再生成は削除する。

現時点で migration は未完了。追加した implicit packet regression は失敗しており、
C1 slot/phase と site projection の API 移行途中なので compile も再確認が必要。
この状態を coherent cut として公開しない。残りの producer/consumer を完成させ、
mixed fields、複数/重複 binding、generic、defer/callback、candidate atomicity、stable
identity、runtime result の証拠を閉じてから必要な全体検証と commit/push を行う。

### 2026-10-01 — accepted scope extension: canonical declaration identities

User steering / reference: ChatGPT `6abdea2a-433c-83e9-8ea6-b4e32e3d7217`
(`関数分離の理由比較`)。2 turns を read_thread で読み、提案を現在の local source と照合。
Supersedes: `5acefacb629f5b286afc498340adceff109d883b` の View-only module identity
policy を最終契約として保持する判断。他の既存 goal acceptance はすべて維持する。

すべての authored declaration family の implicit PublicId を
`<family>.<canonical-module-path>.<declaration-name>` に統一する。root module は空
namespace。explicit PublicId はその値をそのまま保持し、移動に依存しない identity
が必要な場合は作者が explicit ID を選ぶ。import / alias / re-export / symbol / sema /
compiler / bundle / runtime は final HIR が一度確定した typed ID を参照し、名前から
再生成しない。Flow の独立な name-only constructor も同じ module-aware authority
へ移す。catalog-owned Asset は実際の owner/virtual path を保持し、架空の source
module を足さない。old global implicit ID の dual reader/fallback は残さない。

進行中の local-input/runtime callable cut を検証・保存した直後に、この移行を
生成、参照、全 consumer、同名 cross-module/explicit ID/移動・source revision の
tests、maintained specifications まで完了する。まだ実装完了の証拠ではない。
goal は active、予算の追加や新しい goal への置換は行わない。

### 2026-10-01 — local-input cut: final implementation and ownership review

Base remains `5acefacb629f5b286afc498340adceff109d883b`, existing main; all current dirty changes belong to this cut. Supersedes the in-progress implementation status above; the final workspace recipe passes.

The final projection carries `CheckedLocalUseSite`, binding type, stable origin when issued, and access. Defaults consume the complete eager expression/statement inventory; explicit closure, implicit callable, deferred body and local-use admission reuse the projection. C1 owns all authored record slots; C2 atomically joins runtime field placement and stable source to those same slots. The expression-only capture ledger and occurrence coordinate are replaced. `CheckedLocalInputCoordinate` distinguishes expression, record field and capture creation roles. Record shorthand and explicit fields interleave by selected source position; aggregate packets follow first semantic use. Candidate journal order remains rollback bookkeeping and is excluded from semantic replay equality.

Runtime executes an admitted implicit body through a named execution context. Ordinary expression/default/return evaluation creates the callable; captured bindings use the existing lexical frame and exact `checked_local_read(site, local)` ownership admission. No capture ExprId overrides, source-name reconstruction, or parallel runtime binding frame were added.

The first workspace recipe failed on two new acceptance cases: open generic record shorthand was rejected, and a later View parameter in shorthand caused HIR `InvalidLocalTimeline`. Repairs distinguish completely bound declaration parameters from runtime concrete types (`apply_bound` versus `apply_resolved`); an open record keeps its C1 schema until closed-instance runtime projection. `UnresolvedShorthand` retains the authored name/site, freeze proves that no visible binding exists, and sema rejects it. Both ordinary and dialogue candidate producers/freezers were migrated. A mutation replacing a visible local with unresolved input fails atomic HIR freeze. The second workspace recipe exposed the old C05 expectation of a lowering invariant failure; that test now proves the same no-fabricated-local rule through the typed unresolved site, and its focused rerun passes.

Focused evidence after repairs: View defaults 10/0, HIR unresolved-input/freeze 1/0, native plus decoded AWBC input/frame cases 8/0. These include a returned `Box<T>` packet, mixed fields, field base type, and two closed generic frames. Earlier unknown-effect fixtures lacked an explicit closed callable effect row; the original generic-record probe was retained and repaired rather than excluded. Final workspace check/Clippy, format and diff checks pass; Clippy retains warnings. Final `just test-workspace` exits 0: 308 suites, 7,153 passed, 0 failed, 24 ignored, with `RUST_MIN_STACK=16777216`. The final structure gate passes: 97 packages, 350 review triggers, 0 blocking findings. Failed first and second recipe logs are retained in the local temporary receipt files; their repairs are described above. Tier 2 device/render/scheduler/protocol families are outside this compiler input cut; no native pixel or performance claim is made.

Exact physical measurements at this dirty cut (bytes are current file bytes; embedded test LOC follows the canonical scanner convention). All listed production owners are handwritten; test rows contain handwritten test source. No generated, benchmark, example, tool or facade owner was introduced.

| path / owning crate | classification | bytes | physical LOC (base → current) | embedded test LOC |
|---|---|---:|---:|---:|
| `crates/arcweft-compiler/tests/callable_execution.rs` | test | 33148 | 1139 → 1202 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/expression_lowering.rs` | production | 104871 | 2419 → 2422 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests.rs` | test | 114534 | 3145 → 3207 | 0 |
| `crates/arcweft-lang-hir/src/source_index/expression_manifest/projection.rs` | production | 50064 | 1262 → 1281 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs` | production | 252581 | 5775 → 5803 | 156 |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/evaluated_effects.rs` | production | 100761 | 2179 → 2190 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs` | production | 213473 | 4919 → 4934 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/state.rs` | production | 122289 | 3116 → 3218 | 600 |
| `crates/arcweft-lang-sema/src/final_analysis/analyzer/tests.rs` | test | 65646 | 1748 → 1748 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/free_capture.rs` | production | 24735 | 87 → 646 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/local_use.rs` | production | 105304 | 2688 → 2693 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/match_edges.rs` | production | 76020 | 1709 → 1827 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/model.rs` | production | 104403 | 3153 → 3154 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/model/capture.rs` | production | 47239 | 1167 → 1267 | 381 |
| `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs` | production | 93082 | 2247 → 2269 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/prepared.rs` | production | 52882 | 1600 → 1603 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/report.rs` | production | 97206 | 2403 → 2411 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/statement_effects.rs` | production | 53563 | 1354 → 1371 | 0 |
| `crates/arcweft-lang-sema/src/semantic_coordinate.rs` | production | 76327 | 2105 → 2155 | 73 |
| `crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs` | production | 52426 | 1210 → 1271 | 341 |
| `crates/arcweft-runtime-plan/src/final_expr.rs` | production | 134746 | 3286 → 3318 | 0 |
| `crates/arcweft-runtime-plan/src/final_flow.rs` | production | 354240 | 8578 → 8578 | 398 |

Ownership disposition: `free_capture.rs` grows 559 physical LOC (87 → 646) and stays one private local-input projection, ordering and root-bound capture collection boundary; its state is transaction-local and owns no I/O or persistent duplicate read index. The new coordinate role is a real semantic consumer boundary, not API widening for file splitting. `model/capture.rs` (1,267 LOC, 381 embedded test LOC) crosses SIZE001 and remains the terminal capture proof, identity encoder and validation owner; its tests mutate that exact boundary. HIR expression lowering and source projection retain typed lexical resolution/freeze, including ordinary/dialogue producer parity; their existing tests follow that owner.

For the touched upper-trigger owners, retain cohesion explicitly: analyzer calls/evaluated_effects/expressions issue selected facts, state owns the candidate transaction, local_use owns closed Copy/Move/Borrow admission, match_edges owns selected C1 edges and record phase transition, nominal_schema owns C2 projection, prepared/model/report own their existing phase carriers/publication, statement_effects owns eager inventories, semantic_coordinate and catalog own byte grammar/issuance respectively, runtime final_expr/final_flow own admitted lowering/site definition. Each edit follows that existing responsibility; no unrelated state cluster, duplicate semantic authority, second AST walk, compatibility reader or I/O boundary was added. The record side maps, default-only collector, expression-only expected uses and positional local/field iterator join were deleted. Existing fixture and production owner boundaries suffice; arbitrary physical decomposition would widen APIs without changing an actual owner.

Cargo metadata (`--no-deps --all-features`) direct workspace fan-in/out: `arcweft-lang-hir` 11/9, `arcweft-lang-sema` 11/18, `arcweft-runtime-plan` 10/14. Manifests, lockfile, features and dependency direction are unchanged. Syntax/HIR → sema → runtime-plan remains the direction; consumers do not issue semantic local identities.

Remaining goal work: canonical implicit IDs for all declaration families are the next accepted priority; general checked expression/default root ABI and retained RuntimeValue/View execution, UI audit acceptance, .1.3.1, remaining nominal and scheduler/restore work continue afterward. This cut does not establish that the full goal is complete.

### 2026-10-01 — canonical declaration identities: final implementation

Base: `042f5e198b3eb6636c98029df9058be112d0591c`, existing main; that local-input cut was committed and pushed with matching remote HEAD. The current dirty paths belong to the accepted declaration-identity scope extension. Supersedes the View-only policy; the full convergence goal remains active.

`DeclarationIdentityFamily::derive_public_id(namespace, name)` is the sole implicit-ID grammar. The name-only overload, namespace alias and HIR family-switch helper are deleted. Every authored family uses canonical module plus declaration name/path. Explicit IDs retain their accepted values. Asset remains catalog/virtual-path owned.

Actual producers include more than retained headers: Flow now issues and stores its accepted publication on final HIR; symbols and dialogue-line owners borrow it instead of deriving it again. Flow lookup no longer grants a module-local shortened public-ID alias, and all accepted Flow IDs share global collision rejection. Proof now issues an implicit ID, carries it through the existing staged authored-return header into final HIR, and publishes the same value on its callable symbol. Alias/ID lookup joins one callable, visibility remains enforced, and implicit/explicit collisions fail. No independent Proof ID lookup table is added.

Style had a separate reference-based path, not a retained header. It now owns one typed retained PublicId, including origin and complete recovered reference shape/issue. Bare dotted names preserve their declaration path after the canonical module. Source freeze uses the owning projection with the actual module; sema and compiler consume the accepted ID. Their separate Style PublicId readers are deleted. Authored token IDs keep their distinct relative-token meaning. The unused name-only semantic Flow constructor is deleted.

Focused evidence: 60 final-HIR family/module/explicit-ID cases pass; retained family import/re-export and canonical Flow reference/collision cases pass; Proof owner/return tests 28/0 and public-ID alias/visibility/collision 1/0 pass. The compiler project test preserves distinct Character/Style/Flow IDs from root and child in Agent graph, runtime Flow labels, Style sheets and decoded AWBC bindings. Its first AWBC probe had no entry and correctly failed `MissingEntrypoint`; the same fixture now has an explicit entry, and its rerun passes 1/0. Initial matrix fixture failures were invalid Character/View header spellings; fixtures were corrected to their existing grammar without changing acceptance. The final complete workspace recipe passes, as recorded below. No GPU/render-performance claim is made.

Ownership disposition: final_lowering and callable own the existing Proof header/body transaction; final retained/Flow/Style producers own identity issuance, their source projections authenticate it, symbol identity/table/publication own visibility and collision/lookup, and line_identity/sema index/compiler Style consume published IDs. No source-module invention, I/O, dependency change, public API widening for file splitting, duplicate accepted-ID index, legacy reader or fallback was added. The new SIZE001 test-owner trigger (`item_lowering/tests.rs`, 2,558 LOC) remains the shared lowering-fixture and cross-family acceptance owner; its added matrix exercises the same production projection/freeze boundary. Existing larger production owners remain cohesive: final_lowering 1,485, callable 1,345, retained 1,551, item projection 1,444, Flow projection 1,661, symbol identity 1,301 and symbol table 2,083 LOC. `arcweft-id/lib.rs` owns the portable value grammar and shrinks to 755 LOC. No owner grows by 300 LOC. All source is handwritten; no generator, benchmark, example or tool owner changed.

Canonical structure gate: 97 packages, 351 review triggers, 0 blocking violations. Workspace normal dependency fan-in/out: id 32/0, HIR 10/3, sema 8/14, compiler 3/24; development edges respectively 3/0, 1/0, 3/0, 1/9. Manifests, lockfile, features and dependency direction are unchanged. Exact changed-owner measurements follow (base is the full SHA above; bytes and physical LOC are from the current checkout, embedded test LOC from the canonical scanner).

| path / owner | class | bytes | physical LOC (base → current) | embedded test LOC |
|---|---|---:|---:|---:|
| `crates/arcweft-compiler/src/project/tests.rs` / arcweft-compiler | test | 81895 | 2269 → 2322 | 0 |
| `crates/arcweft-compiler/src/style.rs` / arcweft-compiler | production | 13800 | 375 → 372 | 0 |
| `crates/arcweft-id/src/lib.rs` / arcweft-id | facade | 23796 | 763 → 755 | 192 |
| `crates/arcweft-lang-hir/src/final_lowering.rs` / arcweft-lang-hir | production | 57939 | 1479 → 1485 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/callable.rs` / arcweft-lang-hir | production | 54632 | 1305 → 1345 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/flow.rs` / arcweft-lang-hir | production | 39095 | 953 → 954 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/retained.rs` / arcweft-lang-hir | production | 18017 | 427 → 434 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/style.rs` / arcweft-lang-hir | production | 28015 | 582 → 647 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/tests.rs` / arcweft-lang-hir | test | 90616 | 2477 → 2558 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/tests/proof.rs` / arcweft-lang-hir | test | 44279 | 1285 → 1285 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/tests/style.rs` / arcweft-lang-hir | test | 31560 | 869 → 913 | 0 |
| `crates/arcweft-lang-hir/src/final_lowering/item_lowering/tests/style_freeze.rs` / arcweft-lang-hir | test | 12598 | 346 → 346 | 0 |
| `crates/arcweft-lang-hir/src/item/flow.rs` / arcweft-lang-hir | production | 19699 | 594 → 614 | 0 |
| `crates/arcweft-lang-hir/src/item/flow/tests.rs` / arcweft-lang-hir | test | 19428 | 627 → 639 | 0 |
| `crates/arcweft-lang-hir/src/item/host.rs` / arcweft-lang-hir | production | 27620 | 997 → 1006 | 0 |
| `crates/arcweft-lang-hir/src/item/host/tests.rs` / arcweft-lang-hir | test | 18195 | 572 → 589 | 0 |
| `crates/arcweft-lang-hir/src/item/retained.rs` / arcweft-lang-hir | production | 47639 | 1569 → 1551 | 0 |
| `crates/arcweft-lang-hir/src/line_identity/sites.rs` / arcweft-lang-hir | production | 12578 | 340 → 339 | 0 |
| `crates/arcweft-lang-hir/src/source_index/item_projection.rs` / arcweft-lang-hir | production | 56085 | 1437 → 1444 | 0 |
| `crates/arcweft-lang-hir/src/source_index/item_projection/flow.rs` / arcweft-lang-hir | production | 59028 | 1658 → 1661 | 0 |
| `crates/arcweft-lang-hir/src/source_index/item_projection/proof.rs` / arcweft-lang-hir | production | 13651 | 434 → 421 | 0 |
| `crates/arcweft-lang-hir/src/source_index/item_projection/style.rs` / arcweft-lang-hir | production | 41308 | 1134 → 1136 | 0 |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` / arcweft-lang-hir | production | 41170 | 1290 → 1301 | 73 |
| `crates/arcweft-lang-hir/src/symbol/table.rs` / arcweft-lang-hir | production | 76917 | 2082 → 2083 | 0 |
| `crates/arcweft-lang-hir/src/symbol/table/publication.rs` / arcweft-lang-hir | production | 39282 | 1028 → 1046 | 0 |
| `crates/arcweft-lang-hir/src/symbol/table/retained.rs` / arcweft-lang-hir | production | 16764 | 433 → 433 | 0 |
| `crates/arcweft-lang-hir/src/symbol/tests/symbol_projection.rs` / arcweft-lang-hir | test | 47899 | 1153 → 1294 | 0 |
| `crates/arcweft-lang-sema/src/effect_model.rs` / arcweft-lang-sema | production | 10736 | 431 → 426 | 50 |
| `crates/arcweft-lang-sema/src/project_index/final_projection.rs` / arcweft-lang-sema | production | 30626 | 842 → 813 | 0 |
| `crates/arcweft-lang-sema/src/final_analysis/tests.rs` / arcweft-lang-sema | test | 355970 | 10226 → 10228 | 0 |
| `crates/arcweft-lsp/src/session/tests.rs` / arcweft-lsp | test | 68086 | 2030 → 2030 | 0 |

The first full recipe failed on the old same-named Flow fixture: the child still referenced `@flow.opening` and expected both public labels to be equal. The fixture now references `@flow.child.opening`, asserts the two canonical IDs, and preserves both self-goto relation assertions. The 10,228-LOC existing sema test owner grows by 2 LOC; this changes its established project-index identity/relation acceptance case, with no new fixture infrastructure or unrelated test coupling. Its existing shared semantic harness remains the owner; splitting this expectation update would only duplicate that harness. Final recipe rerun uses the complete current source, including Proof/Style authority and the compiler codec case.

The second recipe passes the final HIR (935/0/8 ignored), sema (1,115/0) and compiler cases, then fails the old LSP hover expectation `@character.child_speaker`. Actual hover already consumes the final typed ID `@character.side.child_speaker`. Only that expected literal is migrated; the same selected CharacterDialogue/result and no-root-character assertions remain. The 2,030-LOC LSP session test owner retains its existing accepted-project/hover harness; no protocol, URI, observe, capture, rendering or production LSP path changed. A third complete recipe runs after this expectation repair. Failed recipe logs remain in `%TEMP%/arcweft-1001-canonical-id-workspace-{first,second}-failed.log`.

Final receipt: third `just test-workspace` exits 0, 308 suites, 7,158 passed, 0 failed, 24 ignored, with `RUST_MIN_STACK=16777216`. HIR 935/0/8 ignored and sema 1,115/0 are included. The repaired LSP hover also passes a focused all-feature run 1/0. Workspace check and Clippy (`--all-targets --all-features`) pass; Clippy retains warnings. Format and staged diff checks pass. Structure-gate evidence above is reused for the final literal-only sema/LSP fixture repairs: ownership, dependencies and APIs are unchanged, and their exact measurements are recorded. Logs use `%TEMP%/arcweft-1001-canonical-id-{workspace,check,clippy,structure}.log`; workspace `.exit` is `0`. Tier 2 device/GPU/render/observe/protocol work is outside this identity cut.

The accepted canonical-ID scope is complete. Remaining convergence goal: connect complete checked expression/default input authority to the existing RuntimePureProgram/RuntimeValue ABI, replace scalar View storage/evaluation and finish retained View .1.4 with UI audit acceptance, then .1.3.1, remaining nominal and scheduler/restore acceptance. Read-only preparation confirms `compiler/view.rs::prepare_authored_view` still rejects defaults, and View value inventory/mount state still use scalar Fx schemas; Core already owns typed RuntimePureProgram bindings and Value input/output ABI. These are next implementation boundaries, not completion evidence. The full goal remains active.

### 2026-10-01 — expression input evidence and shared eager execution regions

Base: `e818c8bc7165e5005647f51cf87fe7b5a62d01a5`, existing main, initially clean and matching the pushed canonical-ID cut. Current edits belong to the general expression/default input boundary. Supersedes the preparatory proposal to persist every expression's expanded subtree: the final report now keeps a shared direct-edge DAG, with linear node/edge storage, and expands only a requested root. The existing effect fold issues those edges; no new HIR walker or scope reconstruction issues execution policy. Prepared suspension/control rows are promoted only after final expression/statement owners match exactly; foreign edges and cycles reject publication. The old expanded suspension map and its default-interface argument are removed.

`checked_expression_input_abi` issues root-bound source evidence through the existing free-local collector and local-use authority. It preserves stable binding origins, accepted types, every read/capture-transfer occurrence, its stable site coordinate and mode, result/effects/suspension/call-control summaries, and the existing callable/opaque Copy ingress obligations. Inputs and occurrences sort by accepted coordinates, not raw LocalId/ExprId. Inner bindings are excluded by their accepted root paths. This is input evidence, **not yet complete extracted-program admission**: source-catalog gaps fail closed, and it does not prove closed generic execution, external-place legality, boundary-relative control safety, or runtime unrestricted duplication.

Executable counterexamples repaired: the prepared effect fold previously recognized only completed implicit-callable resolutions, so an owner-bound implicit callable's creation row included its latent body. Both phases now use the owning prepared classification. Defer cleanup still contributes effect/suspension dependencies while its body is excluded from the creating frame's eager DAG; the statement's creation-capture packet is its input. The same final DAG now supplies declaration-default captures.

Field identity and evaluation source are now distinct owning contracts: `CheckedFieldSelection` remains the schema identity used by places, and `CheckedFieldAccess` owns a `Binding(LocalId)` or `Expression(ExprId)` receiver. Prepared project fields retain that same discriminant; ordinary environment field selects use expression receivers. Final publication authenticates receiver owner/type against the actual selected HIR source and accepted local/expression facts; stable transcripts encode the accepted binding/expression coordinate. Only a Binding receiver issues an intrinsic local source; an Expression receiver reads its selected child. Writability remains on CheckedMutablePlace. This removes the default-field false parent read and does not infer receiver execution from mutable-place presence. Compiler field projections consume the access's schema selection.

The fresh-context `gpt-6-astra` Max advice request initially failed with capacity. A later read-only continuation of that same advice task succeeded; no other model fallback, code delegation or agent mutation was used. Advice was checked against current source and incorporated into the maintained View target and the following dependency order:

1. Extend the **existing** local-use authority to ValueTransfer versus PlaceAccess, with exact typed replacement/mutation sites. The execution DAG must likewise distinguish value and place evaluation. Assignment LHS and selected in-place receiver branches currently skip value-read issuance; do not invent Copy/Move rows for those places. Pure extraction rejects external places while permitting root-internal mutation.
2. Seal explicit root intent `EvaluateValue` versus `InvokeBody` under one monomorphic or authenticated closed-instance execution context. A default returning a callable is EvaluateValue; implicit body invocation may use the same ExprId with a distinct body resolution/region. Reuse `checked_local_uses_for_instance` with the same declaration, HIR generation and frozen substitution. Preserve the exact ingress Copy obligation, rather than assuming Copy mode proves an unrestricted runtime carrier.
3. Check typed return/out/break targets relative to the extraction boundary. The existing selected-call control summary is not this proof; ordinary pure project calls must not be rejected merely because that summary says FlowRequired. Internal loop exits remain legal.
4. Replace closure/CaptureId/View-u16-specific RuntimePureProgramFact admission with that checked root, canonical input slots, closed result and execution context. View props/default/Repeat mappings project onto the generic slots. Keep root admission independent of PureHelper body shape: current pure callable evaluation rejects Executable function-site bodies, and lower_function_block admits only Let/Assign/Expression statements. Complete the owning executable runner for general deterministic defaults. Core pure-program ingress currently clones borrowed arguments and checks type, so broader RuntimeValue inputs require owned transfer or checked unrestricted duplication.
5. Finish per-definition RuntimeValue View storage/default execution, fragment construction and retained UI audit acceptance, then the other existing goal conditions. The scalar View inventory and compiler default rejection remain present; this cut does not claim their completion.

Validation so far: final sema all-feature library run passes 1,123/0, including seven input-ABI cases and the DAG publication rejection case. Existing View default cases now reconcile their captures with the general input proof, including String/record/tuple/Match/callable results, shorthand, earlier-parameter chains and stable source revisions. New cases cover every repeated read, inner-local exclusion, explicit/implicit creation latency, cleanup capture exactly once, source/arena revision invariance, foreign-generation rejection, open generic source rejection, and retained Copy ingress obligations. The first focused test exposed implicit latency; the initial full sema run exposed the field-parent read mismatch, both repaired at their owners. An initial callable-Copy fixture was a single-transfer View default (Move); the final fixture exercises the existing repeated function-parameter ingress and preserves the Copy obligation assertion. Failed logs remain under `%TEMP%/arcweft-1001-expression-input-*`. Workspace check and Clippy all-targets/all-features pass with warnings. The complete workspace recipe and structure gate are currently running; no completion claim relies on an unfinished check. Rust documentation comments added afterward describe the same input-proof limits and do not change runtime behavior.

Ownership disposition: execution_regions owns compact DAG promotion/expansion and its malformed-publication tests; expression_inputs owns the borrowed source-input proof and obligation join, not a second stored read index or runtime admission model. field_access owns expression receiver evaluation separately from field schema and writable-place evidence. free_capture retains the sole free-binding filter/type/origin validation. The existing analyzer/effect fold issues selected facts and dependencies, nominal_schema joins field schema/source, report owns atomic publication/default interface sealing, validation owns source authentication, and semantic_transcript owns stable byte encoding. The compiler lower owner only migrates its field identity projection. No I/O, dependency/feature/manifest change, compatibility reader, generator or benchmark was added. All contract markers remain 1. The larger touched owners retain their existing cohesive phase responsibilities; no owner grows by 300 LOC, and the new small field-access module is a real shared schema/value-source boundary, not a physical split requiring convenience APIs. Exact current measurements and final receipts follow after the pending checks.

Structure gate passes: 97 workspace packages, 351 review triggers, 0 blocking violations. Normal dependency fan-in/out is sema 8/14 and compiler 3/24; development edges are sema 3/0 and compiler 1/9. No new size trigger is introduced. The touched upper-trigger owners keep their established responsibilities: compiler lower owns the full admitted-runtime projection transaction; analyzer expressions/issues selection, items closes callable contracts, model/prepared/report carry the existing phases, nominal_schema owns nominal/schema joins, semantic_transcript owns its bounded canonical encoder, statement_effects owns prepared/final execution folds, and validation authenticates final facts. Embedded tests continue to exercise their owning transaction/encoding boundaries. Small new execution_regions/expression_inputs modules separate actual DAG state from borrowed input proof issuance; field_access is a shared schema contract. This is deliberate ownership, with no API widening for arbitrary physical decomposition.

| path (`crates/` prefix) / owner | class | bytes | physical LOC (base → current) | embedded test LOC |
|---|---|---:|---:|---:|
| `arcweft-compiler/src/lower.rs` / compiler | production | 403948 | 9519 → 9520 | 0 |
| `arcweft-lang-sema/src/final_analysis.rs` / sema | facade | 13633 | 245 → 251 | 0 |
| `arcweft-lang-sema/src/final_analysis/analyzer/callable_effect_graph.rs` / sema | production | 23271 | 592 → 591 | 0 |
| `arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs` / sema | production | 214027 | 4934 → 4945 | 88 |
| `arcweft-lang-sema/src/final_analysis/analyzer/items.rs` / sema | production | 67097 | 1611 → 1608 | 0 |
| `arcweft-lang-sema/src/final_analysis/declaration_defaults.rs` / sema | production | 11660 | 282 → 278 | 0 |
| `arcweft-lang-sema/src/final_analysis/error.rs` / sema | production | 42097 | 1118 → 1122 | 64 |
| `arcweft-lang-sema/src/final_analysis/execution_regions.rs` / sema | production | 8217 | 0 → 228 | 51 |
| `arcweft-lang-sema/src/final_analysis/expression_inputs.rs` / sema | production | 8790 | 0 → 238 | 0 |
| `arcweft-lang-sema/src/final_analysis/free_capture.rs` / sema | production | 25423 | 646 → 669 | 0 |
| `arcweft-lang-sema/src/final_analysis/input.rs` / sema | production | 5225 | 148 → 148 | 0 |
| `arcweft-lang-sema/src/final_analysis/model.rs` / sema | production | 104759 | 3154 → 3163 | 0 |
| `arcweft-lang-sema/src/final_analysis/model/field_access.rs` / sema | production | 1067 | 0 → 39 | 0 |
| `arcweft-lang-sema/src/final_analysis/nominal_schema.rs` / sema | production | 93308 | 2269 → 2278 | 0 |
| `arcweft-lang-sema/src/final_analysis/prepared.rs` / sema | production | 54377 | 1603 → 1639 | 0 |
| `arcweft-lang-sema/src/final_analysis/report.rs` / sema | production | 97392 | 2411 → 2411 | 0 |
| `arcweft-lang-sema/src/final_analysis/semantic_transcript.rs` / sema | production | 200952 | 5078 → 5093 | 332 |
| `arcweft-lang-sema/src/final_analysis/statement_effects.rs` / sema | production | 55233 | 1371 → 1410 | 0 |
| `arcweft-lang-sema/src/final_analysis/statement_seal.rs` / sema | production | 27224 | 619 → 623 | 0 |
| `arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance.rs` / sema | test | 5754 | 153 → 155 | 0 |
| `arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/expression_inputs.rs` / sema | test | 8594 | 0 → 244 | 0 |
| `arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/view_defaults.rs` / sema | test | 16606 | 398 → 430 | 0 |
| `arcweft-lang-sema/src/final_analysis/validation.rs` / sema | production | 129794 | 3069 → 3093 | 0 |

Measurements are current file bytes/physical lines; embedded test LOC and package graph come from the canonical scanner. Reports/logs are `%TEMP%/arcweft-1001-expression-input-structure{/, .log, .exit}`. The namespace cut's historical measurements/evidence are retained above. Tier 2 device/GPU/render/observe/protocol work is outside this source-input cut, and no allocation/GPU benchmark result is claimed.

Final receipt: `just test-workspace` exits 0, **308 suites, 7,166 passed, 0 failed, 24 ignored**, with `RUST_MIN_STACK=16777216`. Workspace check and Clippy all-targets/all-features exit 0. Final sema all-feature library evidence is 1,123/0. Afterward, documentation backticks and three equivalent test method references replace avoidable doc/redundant-closure lint warnings; the seven input-ABI cases pass again 7/0 and sema Clippy all-targets/all-features passes again with the existing shared large-error/other warnings retained. No production execution decision, fixture value/assertion, manifest, feature or dependency changes in that lint cleanup. The workspace behavioral/check receipts and structure graph are therefore reused for those edits; table bytes are refreshed directly, and physical/embedded-test LOC is unchanged. Format and diff checks pass. Logs and exit receipts use `%TEMP%/arcweft-1001-expression-input-{workspace,check,clippy,structure}` plus `focused-lint-final` and `clippy-lint-final`.

The input-source/field-receiver/DAG foundation is validated. The full convergence goal stays active: next implement typed ValueTransfer/PlaceAccess and explicit closed-context EvaluateValue/InvokeBody admission, finish the deterministic executable program runner and RuntimePureProgram ingress, and connect the RuntimeValue View/default consumer. Those remaining conditions are not discharged by this cut or its commit boundary.

### 2026-10-01 — local access and general assignment: initial migration (ownership rule superseded below)

Base: `2ef441c6791a2aba4ec8ab18c593eb0aea6cdf03`, existing main. This section records uncommitted implementation progress; no final workspace/structure receipt or delivery is claimed yet. The full goal remains active.

The local-use catalog now owns one `CheckedLocalAccess` sum: value transfer (Copy/Move/Borrow) or exact place access (Replace/Mutate). Source coordinates distinguish Place at tag 4 within the existing version-one grammar. The existing eager fold issues Value/Place/Statement membership; places do not traverse value receivers. The expression-input and default-input collectors consume that same inventory. Global and selected closed-instance runtime views project the same catalog, and assignment/capacity lowering requires its exact place certificate. Creation captures preserve latent Reassign requirements without inventing an eager write.

Whole mutable-local assignment joins the existing final `CheckedAssignment` place authority. Prepared statements retain target/value/type until the final target is sealed, replacing the field-only prepared model. Runtime assignment facts and native expression/Flow/AOT operations now carry a general typed place; the old field-only operation is deleted. AWBC opcode 0x11 is likewise `Assign { place, value }`, with its version-one codec evolved in place. Local targets may be empty; field targets require an initialized nominal base. Its verifier carries the value's Copy proof into the replaced local.

Exceptionally difficult ownership-boundary advice was requested from fresh-context Astra Max (`affine_place_scope_advice`) under the user's conditional authorization. The advice was read-only. It identified lost declaration scope after Move and missing displaced-value cleanup. Native environments now store declaration slots with optional contents; initialized binding packets remain distinct transfer data. Nearest-slot lookup cannot fall through a vacant inner slot. Both native rollback users retain vacant slots and scope identity. Public native session persistence remains unsupported; do not infer native save support from rollback evidence.

Assignment returns its displaced carrier or retains a rejected supplied carrier. A transaction-local typed drop authorization journals only displaced handle graphs; explicit drop/scope-exit policies keep their separate boundary meaning. Native parent/child and activation owners, and AWBC product/activation owners, reconcile that evidence through the existing ledger. AWBC owns a DiscardedValue packet until the product consumer admits its exact graph. No blanket assignment permission to discard unrelated handles is issued, and Need disappearance does not invent producer cancellation. Journal preparation avoids cloning its growing token map; validation queries the actual before/after inventories without cloning them.

Source availability also rechecks mutation owners after value operands, and borrowed receiver loans reject consumption/replacement during operand evaluation. Partial calls retain receiver evaluation while withholding terminal body effects.

Current evidence: input ABI 9/0; local-use 40/0, including closed-instance place issuance, move-then-reinitialize, mutation after move, argument-induced receiver invalidation and borrowed actor invalidation; native slot/rollback tests 9/0 before the final discard-journal additions; exact nested displaced-graph/retained-new-owner ledger case 1/0. Core/sema checks passed at their recorded intermediate states. Pure-helper local replacement passes native and decoded AWBC (2/0), and the migrated field ordinal/type codec case passes. The first affine-Vec source fixture used an invalid empty literal; the constructor repair then exposed existing nested Need runtime-type rejection. The fixture now uses representable affine Content and passes native and decoded AWBC 2/0, rather than weakening Need admission. Logs use `%TEMP%/arcweft-1001-place-*` and `arcweft-1001-general-place-*`.

Required before this migration is a delivered cut: finish native/decoded AWBC and AWBC checkpoint/save-restore acceptance for reinitializable empty slots; test rejected-write rollback and exact resource cleanup on the real execution owners; complete all-feature sema/Core and required workspace check/Clippy/test recipes; update actual structure/byte/physical-LOC/embedded-test/dependency evidence; inspect the complete staged diff, commit and push. Review native activation journal handling and transient pure/helper ownership propagation as part of those execution checks. No failed or unavailable receipt is a completion claim. Explicit root intent, closed root context, general program/owned ingress, retained View/UI audit, .1.3.1 and the remaining goal rows still follow.

### 2026-10-01 — user-directed terminal move contract: active correction

The user's explicit instruction supersedes the earlier reinitialization decision:
move ends the availability of the same local declaration/generation, and assignment
after move is forbidden. The referenced conversation `6abe4a29-83ac-83e8-a299-3e95430165fd`
(テスト再実行計画) was read in full as supporting advice. No additional Astra consultation
is needed. Shadowing initializes a new declaration; it never restores the old one.
Both Replace and Mutate require a live owner, including after right-hand-side/operand
evaluation and at reachable branch joins. Active receiver loans reject consuming or
replacing the owner while evaluating arguments. Slot identity survives for nearest
scope lookup and rollback, but absent contents grant no write permission.

The source checker no longer clears moved evidence at assignment. Native assignment
rejects an absent target and retains rejected input; successful assignment always
returns one displaced live carrier. AWBC opcode 0x11 requires an initialized target
in static verification and guards target liveness before taking the incoming register
at execution. Live replacement, exact displaced-resource cleanup, and the typed access
catalog remain. The previous positive move/reinitialization tests are replaced by
rejection tests and independent shadowing/live-replacement tests. Snapshot/checkpoint
tests exercise the VM guard after restoration independently of static admission; an
invalid program is never published as an executable product. Native activation also
reconciles the retained incoming graph's owner and journals the previous graph's
release command; release is pending host acknowledgment rather than prematurely
reported as Released.

Earlier passing reinitialization receipts do not validate this contract. The two full
workspace attempts ended with a stale native scope fixture expectation, and an
intermediate Clippy/Core run failed on a newly added test's missing token qualification.
The initial terminal-contract tests exposed a malformed branch fixture and scope-yield/
asynchronous-release test expectations (sema 41/1; Core 783/2); those are being repaired
without weakening liveness checks. Final all-feature owner tests, workspace check,
Clippy and full test recipe, structural evidence, staged review and commit/push remain
required. The full convergence goal remains active; this instruction changes ownership
acceptance, not its remaining UI/runtime/scheduler scope. Transient pure/helper owning
ingress and root admission are still tracked for the ensuing general runner migration.


#### Local-access migration structural owner inventory

Base: 2ef441c6791a2aba4ec8ab18c593eb0aea6cdf03. Current uncommitted main after the terminal-move correction. Rust paths are relative to crates/ where shown by the scanner; bytes and physical LOC are measured from current files, and embedded test LOC/roles come from the canonical scanner. No manifests, dependencies, features or package boundaries are changed.

| Rust path / owner | Role | Bytes | Physical LOC before → current | Embedded test LOC |
|---|---|---:|---:|---:|
| crates/arcweft-compiler/src/lower.rs / arcweft-compiler | production | 404114 | 9520 → 9524 | 0 |
| crates/arcweft-compiler/tests/callable_execution.rs / arcweft-compiler | test | 33978 | 1202 → 1240 | 0 |
| crates/arcweft-compiler/tests/generic_continuation_snapshot.rs / arcweft-compiler | test | 21255 | 504 → 590 | 0 |
| crates/arcweft-core/src/aot.rs / arcweft-core | production | 14842 | 442 → 439 | 0 |
| crates/arcweft-core/src/awbc/codec/code.rs / arcweft-core | production | 84451 | 2269 → 2263 | 359 |
| crates/arcweft-core/src/awbc/parity.rs / arcweft-core | production | 9535 | 271 → 274 | 0 |
| crates/arcweft-core/src/awbc/product_step.rs / arcweft-core | production | 210074 | 5214 → 5212 | 0 |
| crates/arcweft-core/src/awbc/product_step/dialogue.rs / arcweft-core | production | 18681 | 461 → 461 | 0 |
| crates/arcweft-core/src/awbc/product_step/line.rs / arcweft-core | production | 177257 | 4162 → 4173 | 74 |
| crates/arcweft-core/src/awbc/product_step/suspension.rs / arcweft-core | production | 114808 | 2817 → 2824 | 0 |
| crates/arcweft-core/src/awbc/schema.rs / arcweft-core | production | 112789 | 3546 → 3545 | 0 |
| crates/arcweft-core/src/awbc/tests.rs / arcweft-core | test | 282939 | 7889 → 7890 | 0 |
| crates/arcweft-core/src/awbc/tests/local_assignment.rs / arcweft-core | test | 6989 | 0 → 199 | 0 |
| crates/arcweft-core/src/awbc/verify/code.rs / arcweft-core | production | 224598 | 5754 → 5759 | 0 |
| crates/arcweft-core/src/awbc/vm.rs / arcweft-core | production | 217525 | 5263 → 5291 | 0 |
| crates/arcweft-core/src/engine.rs / arcweft-core | production | 155050 | 3944 → 3951 | 62 |
| crates/arcweft-core/src/engine/aot.rs / arcweft-core | production | 8026 | 203 → 197 | 0 |
| crates/arcweft-core/src/engine/dialogue.rs / arcweft-core | production | 145529 | 3197 → 3376 | 721 |
| crates/arcweft-core/src/engine/dialogue/store.rs / arcweft-core | production | 65798 | 1676 → 1751 | 575 |
| crates/arcweft-core/src/engine/eval.rs / arcweft-core | production | 63688 | 1577 → 1568 | 124 |
| crates/arcweft-core/src/engine/flow.rs / arcweft-core | production | 66093 | 1666 → 1660 | 102 |
| crates/arcweft-core/src/line_task.rs / arcweft-core | production | 70999 | 2107 → 2109 | 0 |
| crates/arcweft-core/src/line_task/activation.rs / arcweft-core | production | 35676 | 910 → 912 | 0 |
| crates/arcweft-core/src/line_task/drop_authorization.rs / arcweft-core | production | 3058 | 0 → 91 | 0 |
| crates/arcweft-core/src/line_task/handle.rs / arcweft-core | production | 216664 | 5614 → 5622 | 666 |
| crates/arcweft-core/src/plan.rs / arcweft-core | production | 56345 | 1599 → 1598 | 0 |
| crates/arcweft-core/src/plan/construction/lower.rs / arcweft-core | production | 241813 | 5707 → 5692 | 269 |
| crates/arcweft-core/src/plan/construction/seed.rs / arcweft-core | production | 91089 | 2868 → 2870 | 0 |
| crates/arcweft-core/src/plan/entry_inventory.rs / arcweft-core | production | 60039 | 1559 → 1559 | 0 |
| crates/arcweft-core/src/plan/flow_ops.rs / arcweft-core | production | 4332 | 114 → 114 | 0 |
| crates/arcweft-core/src/pure.rs / arcweft-core | production | 129263 | 3423 → 3416 | 134 |
| crates/arcweft-core/src/tests/flow.rs / arcweft-core | test | 72668 | 1914 → 1915 | 0 |
| crates/arcweft-core/src/value.rs / arcweft-core | production | 138654 | 3785 → 3802 | 0 |
| crates/arcweft-core/src/value/env.rs / arcweft-core | production | 31416 | 695 → 886 | 306 |
| crates/arcweft-core/src/value/expression_literals.rs / arcweft-core | production | 8353 | 190 → 190 | 43 |
| crates/arcweft-core/src/value/expression_locals.rs / arcweft-core | production | 14427 | 353 → 351 | 0 |
| crates/arcweft-core/src/value/nominal_record.rs / arcweft-core | production | 24703 | 728 → 727 | 233 |
| crates/arcweft-lang-sema/src/callable.rs / arcweft-lang-sema | production | 14860 | 247 → 247 | 0 |
| crates/arcweft-lang-sema/src/callable/checked_application.rs / arcweft-lang-sema | production | 175613 | 4735 → 4733 | 0 |
| crates/arcweft-lang-sema/src/callable/identity.rs / arcweft-lang-sema | production | 60986 | 1945 → 1961 | 0 |
| crates/arcweft-lang-sema/src/final_analysis.rs / arcweft-lang-sema | production | 13769 | 251 → 253 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/callable_effect_graph.rs / arcweft-lang-sema | production | 23538 | 591 → 596 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs / arcweft-lang-sema | production | 284135 | 6845 → 6867 | 546 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/items.rs / arcweft-lang-sema | production | 67091 | 1608 → 1608 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/pending_effects.rs / arcweft-lang-sema | production | 12549 | 283 → 283 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/statements.rs / arcweft-lang-sema | production | 39827 | 967 → 999 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/declaration_defaults.rs / arcweft-lang-sema | production | 12177 | 278 → 291 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/error.rs / arcweft-lang-sema | production | 42101 | 1122 → 1122 | 64 |
| crates/arcweft-lang-sema/src/final_analysis/execution_regions.rs / arcweft-lang-sema | production | 9994 | 228 → 274 | 51 |
| crates/arcweft-lang-sema/src/final_analysis/expression_inputs.rs / arcweft-lang-sema | production | 10367 | 238 → 275 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/free_capture.rs / arcweft-lang-sema | production | 26042 | 669 → 688 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use.rs / arcweft-lang-sema | production | 110944 | 2693 → 2839 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use/access.rs / arcweft-lang-sema | production | 2045 | 0 → 68 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/model/capture.rs / arcweft-lang-sema | production | 47297 | 1267 → 1268 | 381 |
| crates/arcweft-lang-sema/src/final_analysis/prepared.rs / arcweft-lang-sema | production | 54636 | 1639 → 1646 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/prepared/statement.rs / arcweft-lang-sema | production | 6761 | 249 → 231 | 23 |
| crates/arcweft-lang-sema/src/final_analysis/report.rs / arcweft-lang-sema | production | 97392 | 2411 → 2411 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/statement_effects.rs / arcweft-lang-sema | production | 58147 | 1410 → 1484 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/statement_seal.rs / arcweft-lang-sema | production | 26642 | 623 → 610 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/display_text.rs / arcweft-lang-sema | test | 9429 | 294 → 299 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/local_use.rs / arcweft-lang-sema | test | 41355 | 1132 → 1317 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/expression_inputs.rs / arcweft-lang-sema | test | 12147 | 244 → 344 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/view_defaults.rs / arcweft-lang-sema | test | 16617 | 430 → 430 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/validation.rs / arcweft-lang-sema | production | 130217 | 3093 → 3101 | 0 |
| crates/arcweft-lang-sema/src/semantic_coordinate.rs / arcweft-lang-sema | production | 76397 | 2155 → 2157 | 73 |
| crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs / arcweft-lang-sema | production | 52570 | 1271 → 1274 | 341 |
| crates/arcweft-runtime-accelerator/src/compile.rs / arcweft-runtime-accelerator | production | 56740 | 1655 → 1655 | 0 |
| crates/arcweft-runtime-codegen/src/awbc_region.rs / arcweft-runtime-codegen | production | 14359 | 403 → 403 | 0 |
| crates/arcweft-runtime-plan/src/awbc_lower/expr.rs / arcweft-runtime-plan | production | 91007 | 2379 → 2369 | 0 |
| crates/arcweft-runtime-plan/src/awbc_lower/flow.rs / arcweft-runtime-plan | production | 150788 | 3922 → 3909 | 0 |
| crates/arcweft-runtime-plan/src/awbc_lower/trait_method.rs / arcweft-runtime-plan | production | 9602 | 291 → 269 | 0 |
| crates/arcweft-runtime-plan/src/final_expr.rs / arcweft-runtime-plan | production | 135599 | 3318 → 3332 | 0 |
| crates/arcweft-runtime-plan/src/final_flow.rs / arcweft-runtime-plan | production | 354262 | 8578 → 8578 | 398 |
| crates/arcweft-runtime-plan/src/final_flow/line_plan.rs / arcweft-runtime-plan | production | 53681 | 1352 → 1352 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts.rs / arcweft-runtime-plan | production | 516824 | 13271 → 13332 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts/project_function.rs / arcweft-runtime-plan | production | 103791 | 2798 → 2812 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts/tests.rs / arcweft-runtime-plan | test | 120391 | 3342 → 3346 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts/type_dependencies.rs / arcweft-runtime-plan | production | 13818 | 388 → 388 | 0 |

The final scanner reports 97 packages, 351 ownership review triggers, and zero
blocking violations (1,468,711 Rust physical lines). The touched owners remain the
semantic local-access checker/catalog, prepared/final execution proof projection,
runtime place lowering, native environment/ledger transactions, and AWBC
codec/verifier/VM/product transactions. New private modules own local-access algebra,
exact transaction drop authorization, and AWBC assignment tests; there is no second
live-value authority, source reconstruction, package split, or compatibility reader.
Large existing owners and the shared actor lifecycle fixture remain ownership review
triggers, not acceptance gates based on source placement. Input evidence is borrowed
from the one catalog; cleanup metadata is not duplicated live storage. Assignment
preflight does not clone the growing authorization map or the whole value graph.
No allocation/GPU benchmark or native public session persistence support is claimed.

Both the complete workspace dependency-edge report and package fan-in/fan-out report
are byte-identical to the preceding input-evidence cut. SHA-256 values are
`7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42`
(dependency edges) and
`6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7`
(package metrics). No Cargo manifests, lockfile, features, or dependencies changed.
Reports use `%TEMP%/arcweft-1001-place-terminal-final-structure`.

Final owner receipts: sema all-feature library 1,133/0; Core all-feature library
785/0; compiler native/decoded AWBC callable cases 123/0 and continuation/AWFB
save cases 5/0. Workspace check and Clippy all-targets/all-features exit 0. Core
Clippy all-targets/all-features passes after removing an unused private authorization
merge and qualifying equivalent test defaults. Existing warning categories remain;
no broad lint suppression is added. These source-neutral cleanup edits preserve the
workspace Clippy receipt, while the current Core tests/check/Clippy validate their
actual compiled form. Format and diff checks pass. The final just test-workspace receipt exits 0: 308 suites, 7188 passed, 0 failed, 24 ignored, with RUST_MIN_STACK=16777216. Logs and exit receipts use %TEMP%/arcweft-1001-place-terminal-*. Earlier fixture/compiler/expectation failures are retained in their logs and are superseded by these passing receipts. This candidate is fully validated for the local-access/terminal-move cut; explicit staged review and fast-forward delivery follow. The full convergence goal remains active, including closed root admission, owned general-program ingress and execution, retained View/UI audit, nominal/runtime-plan and scheduler/restore acceptance.

Delivery receipt: implementation commit
`3b06ff85dda1b9d6eba46c13c9b4d0d0abc7f179` (parent
`2ef441c6791a2aba4ec8ab18c593eb0aea6cdf03`) was pushed non-forced to
origin/main; the full remote SHA was observed to match, and the implementation
checkout was clean afterward. Move is terminal for the same declaration;
shadowing remains a distinct initialization. The full convergence goal remains
active. Next continue closed EvaluateValue/InvokeBody admission, general owned
program ingress and execution, then retained View/UI and the remaining accepted
runtime/nominal/scheduler rows. The preceding historical reinitialization proposal
is superseded, and no unresolved failing required check remains for this delivered
local-access/terminal-move cut.

### 2026-10-02 — Shared callable boundaries and body emission

Inspected base `c5a4d22a1c968689dd86893542b1fcdf0b5fdeda` on the existing
main checkout; it was clean at continuation. This candidate changes source,
contract tests and the maintained Return chapter. The delivered terminal-move
contract remains authoritative: no Read/Borrow/Replace/Mutate after moving the
same declaration; shadowing initializes a distinct declaration. No branch,
worktree, dependency, feature, compatibility reader or contract version changed.

Fresh-context Astra Max advice was read-only and limited to the uncertain
closed execution-root/callable-boundary design. It informed the following
producer decisions; the source and tests establish their actual behavior:

- Return and Try use one checked callable-frame algebra for declarations,
  explicit closures and selected implicit callables. Carrier blocks are separate
  Try receivers and do not receive Return. Replaced Try-only frame type names
  are deleted, with compiler, validation and transcript consumers migrated.
- HIR issues structural Return context. The final statement producer combines
  that context with accepted coordinates and selected callable facts before
  recursively folding latent bodies. It consumes one affine prepared Return
  proof, rather than looking up a parent closure while that parent is absent
  from the recursive completion map. Rejected call values retain their tooling
  diagnostics and supply no fabricated result-type evidence.
- The canonical expression-use index supplies the nearest implicit region in
  parent depth, excluding the expression whose value is being returned. A
  returned callable value therefore does not invoke itself. Impl methods use
  the accepted declaration root, without a first-source-item symbol search.
- The existing execution DAG now owns distinct Body nodes. An implicit body
  projects selected children without a Body-to-own-Value edge, so creation
  does not repeat captured inputs and the graph remains acyclic. Statement
  control requirements join the same execution fold; FlowRequired describes
  emission needs independently of effects or suspension.
- A block ending in Return has no normal completion (Never). Callable result
  typing preserves a declared/contextual result or infers the terminal Return
  value. Runtime closure validation accepts divergent bodies against their
  closed result contract. Explicit and implicit bodies containing Return use
  the existing executable function-site runner in native and AWBC. Implicit
  sites carry their body control role and closed invocation effects; admitted
  lexical frame and synthetic-parameter bindings are reused. Assertions in an
  implicit frame derive identities from that callable's semantic identity.

This is a callable/control substrate cut, not completion of general program
admission or the UI audit. Still required: opaque EvaluateValue/InvokeBody root
admission; exact closed generic-context authentication; canonical formal,
synthetic and free-local input slots; boundary-relative Return/Loop/Try proof;
owned RuntimeValue ingress and deterministic general-program execution; deletion
of closure/capture/u16-only PureProgram assembly and helper forcing; retained
View general values, patch/cache/resource/virtualization/clock/interaction-lease
acceptance; and the remaining nominal/runtime-plan/scheduler rows above. Const
phase-fence acceptance and full control-flow divergence inference are not claimed.
The full goal is unfinished.

Goal-tool observation: the saved goal currently reports blocked. Creating a
replacement with the reconciled objective was rejected because an unfinished
goal already exists; the exposed status tool cannot resume or edit its objective.
The implementation continues against this durable plan. This metadata condition
is not an implementation blocker and does not justify marking the full goal
complete or resetting its history.

Validation before delivery: native and decoded-AWBC terminal Return cases 4/0;
sema all-feature library 1,138/0 after repairing the retained latent-closure-row
regression; HIR all-feature library 936/0 with 8 ignored; workspace check and
Clippy all-targets/all-features exit 0. The final workspace test receipt follows
below once complete. The earlier owner/workspace failure for the missing latent
closure row is retained in the 1001-return logs and is superseded for sema by the
passing final source receipt. RUST_MIN_STACK=16777216 is used for tests.

The final structural scan reports 97 packages, 351 ownership review triggers,
zero blocking violations and 1,469,676 Rust physical lines. No Cargo graph change
occurred: dependency-edge SHA-256 remains
7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42 and
package fan-in/fan-out SHA-256 remains
6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.
Reports use %TEMP%/arcweft-1002-return-final-structure.

Touched ownership remains HIR topology/context queries; sema callable-frame,
statement, effect/control and execution-DAG authorities; compiler projection;
and runtime-plan function-site admission/lowering. The new callable-boundary
module owns the shared frame algebra and its generation-bound Return issuer.
The new tests own Return-boundary acceptance. Existing large owners remain
cohesive algorithm/transaction review triggers, with no new I/O, package split,
public visibility introduced solely for file splitting, source reconstruction,
or alternate runtime interpreter. Prepared closure-effect aggregates remain
transaction-local diagnostics/inference inputs, while final Body edges belong
to the one published execution DAG. No allocation or GPU benchmark is claimed.

Exact current measurements (base physical LOC compared with the complete file):

| Path / owner | Classification | Bytes | Base → current physical LOC | Embedded test LOC |
| --- | --- | ---: | ---: | ---: |
| crates/arcweft-compiler/src/lower.rs / arcweft-compiler | production | 404839 | 9524 → 9534 | 0 |
| crates/arcweft-compiler/tests/callable_execution.rs / arcweft-compiler | test | 34466 | 1240 → 1266 | 0 |
| crates/arcweft-lang-hir/src/final_project.rs / arcweft-lang-hir | production | 39498 | 1054 → 1054 | 0 |
| crates/arcweft-lang-hir/src/final_project/semantic_paths.rs / arcweft-lang-hir | production | 245853 | 6469 → 6585 | 0 |
| crates/arcweft-lang-hir/src/final_project/semantic_paths/tests.rs / arcweft-lang-hir | test | 37705 | 1006 → 1050 | 0 |
| crates/arcweft-lang-sema/src/final_analysis.rs / arcweft-lang-sema | production | 13804 | 253 → 253 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer.rs / arcweft-lang-sema | production | 42568 | 1062 → 1062 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/callable_effect_graph.rs / arcweft-lang-sema | production | 24678 | 596 → 625 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs / arcweft-lang-sema | production | 216350 | 4945 → 4997 | 88 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/items.rs / arcweft-lang-sema | production | 67248 | 1608 → 1612 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/error.rs / arcweft-lang-sema | production | 42602 | 1122 → 1132 | 64 |
| crates/arcweft-lang-sema/src/final_analysis/execution_regions.rs / arcweft-lang-sema | production | 11511 | 274 → 307 | 51 |
| crates/arcweft-lang-sema/src/final_analysis/model.rs / arcweft-lang-sema | production | 102425 | 3163 → 3086 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/owner_bound_resolution.rs / arcweft-lang-sema | production | 36369 | 884 → 887 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/prepared/statement.rs / arcweft-lang-sema | production | 7007 | 231 → 235 | 23 |
| crates/arcweft-lang-sema/src/final_analysis/report.rs / arcweft-lang-sema | production | 97851 | 2411 → 2423 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs / arcweft-lang-sema | production | 201544 | 5093 → 5110 | 332 |
| crates/arcweft-lang-sema/src/final_analysis/statement_effects.rs / arcweft-lang-sema | production | 61275 | 1484 → 1565 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/statement_seal.rs / arcweft-lang-sema | production | 30517 | 610 → 696 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests.rs / arcweft-lang-sema | test | 356022 | 10228 → 10228 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance.rs / arcweft-lang-sema | test | 5841 | 155 → 157 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/statements.rs / arcweft-lang-sema | test | 61618 | 1655 → 1655 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/statement_producers.rs / arcweft-lang-sema | test | 22108 | 702 → 702 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/validation.rs / arcweft-lang-sema | production | 130342 | 3101 → 3103 | 0 |
| crates/arcweft-lang-sema/src/project_index/final_projection.rs / arcweft-lang-sema | production | 30806 | 813 → 815 | 0 |
| crates/arcweft-lang-sema/src/semantic_coordinate.rs / arcweft-lang-sema | production | 76733 | 2157 → 2165 | 73 |
| crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs / arcweft-lang-sema | production | 54197 | 1274 → 1318 | 341 |
| crates/arcweft-runtime-plan/src/assertion_lower.rs / arcweft-runtime-plan | production | 11243 | 273 → 296 | 42 |
| crates/arcweft-runtime-plan/src/final_expr.rs / arcweft-runtime-plan | production | 135751 | 3332 → 3337 | 0 |
| crates/arcweft-runtime-plan/src/final_flow.rs / arcweft-runtime-plan | production | 357885 | 8578 → 8648 | 398 |
| crates/arcweft-runtime-plan/src/final_flow/line_plan.rs / arcweft-runtime-plan | production | 53697 | 1352 → 1353 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts.rs / arcweft-runtime-plan | production | 517134 | 13332 → 13339 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts/project_function.rs / arcweft-runtime-plan | production | 103928 | 2812 → 2815 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/model/callable_boundary.rs / arcweft-lang-sema | production | 8717 | 0 → 215 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/return_boundaries.rs / arcweft-lang-sema | test | 4805 | 0 → 133 | 0 |

Final source receipt: `just test-workspace` exits 0 with 308 suites,
7,198 passed, 0 failed and 24 ignored. Workspace check and Clippy on the final
source, all-targets/all-features, both exit 0. Format and diff checks pass.
The earlier complete workspace run failed only the retained latent-closure-row
fixture; it is superseded by this final workspace receipt. Logs/exit receipts
use %TEMP%/arcweft-1002-return-test-workspace-final,
%TEMP%/arcweft-1001-return-workspace-final-{check,clippy}, and
%TEMP%/arcweft-1002-return-fmt-check. Existing warning categories remain.
This candidate is validated for the shared callable/Return/body-emission cut;
explicit staged review and non-forced main delivery follow. Full convergence
acceptance above remains unfinished, including general closed root admission,
owned program execution and retained View/UI acceptance.

### 2026-10-02 — Accepted callable instance provenance

Inspected and continued delivered main
`d8a5b78404625d57c3b91fa1b31578f834f2594d`; its non-forced push and
matching remote SHA were observed, followed by a clean working tree. The
shared Return/callable/body-emission receipt immediately above belongs to
that delivered commit. This next candidate retains that cut and the terminal
Move contract. The goal UI still reports blocked; the API rejected a new goal
while an unfinished goal exists and has no resume/objective-edit operation.
The pending app-side resume question does not block independent source work.

At this design boundary, fresh-context Astra Max provided read-only advice.
Source tracing confirmed a missing admission seam: a frozen generic solution
was a flat semantic substitution, but its local-use issuer validated only the
current report/project/symbols. It did not authenticate the supplied instance.
This is a public-boundary gap; it is not evidence that the ordinary compiler
had actually mixed worlds.

The accepted report now owns public source selectors. It retrieves its own
application/join/catalog instead of accepting independently paired evidence.
The old free selector exports are removed and compiler/test consumers migrate.
Selected calls retain their exact lexical declaration from accepted topology;
a supplied enclosing instance must match that declaration and authority even
when the call's types were already closed.

One private callable-authority lease retains existing generation and registered
catalog authority. Generation equality includes the exact HIR allocation;
registered allocation equality uses Arc::ptr_eq, matching the existing catalog
admission contract. A shallow catalog clone retaining both authorities is valid;
a separately allocated equal-digest catalog is not. Leases are propagated through
runtime selections, declaration values, continuations, specialized closed bodies
and DisplayText templates/conformances. They do not enter stable transcripts,
instance digests, codecs or version markers. The lower generic solver remains
portable, and context-free declaration-value normalization remains unchanged.
The local-use issuer rejects a foreign project-function or DisplayText instance
before applying its substitution.

New behavior coverage checks equivalent rebuild rejection with stable identities,
registered allocation versus shared clones, wrong same-world lexical enclosing
instances, foreign catalog closing/continuation/specialization, and DisplayText
provenance. The current sema owner run passed 1,144 tests before the final
input-call authority check was added; full workspace validation is running on
that final source. The initial compiler check caught the report Result/Option
API mismatch and a removed application binding needed by a later consumer;
both were repaired. Neither failed check is a passing receipt.

Ownership remains sema -> runtime-plan/verify -> compiler/tooling. The existing
checked catalog owns the private lease; report/project_functions owns only
report-bound issuing APIs; join and source retain their existing projection
responsibilities. Display templates hold the lease rather than Arc<CheckedCatalog>,
so final interface publication's unique Arc mutation remains valid. No source
reconstruction, alternate resolver, fallback global transfer seal, dependency
or feature change is introduced. General closed root/context admission and owned
program execution remain the next unfinished acceptance; provenance alone does
not satisfy the full convergence goal or the retained View/UI audit.


Structure receipt: canonical gate exits 0, 97 workspace packages, 351 review
triggers, 0 blocking violations, 1,470,076 Rust physical LOC. Report:
%TEMP%/arcweft-1002-instance-structure. Existing upper-size owner dispositions
for compiler lower, sema checked_catalog/join/model/local_use/report remain
applicable: this cut only authenticates their existing projection and publication
responsibilities. New issuing API and allocation-boundary tests follow their
respective report and checked-catalog owners. No Cargo dependencies/features change.

| Path / crate | Class | Bytes | Base → final physical LOC | Embedded tests |
|---|---|---:|---:|---:|
| crates/arcweft-compiler/src/lower.rs / arcweft-compiler | production | 403730 | 9534 → 9512 | 0 |
| crates/arcweft-compiler/src/lower/project_instances/callables.rs / arcweft-compiler | production | 24576 | 633 → 633 | 0 |
| crates/arcweft-compiler/src/lower/project_instances/tests.rs / arcweft-compiler | test | 24453 | 669 → 668 | 0 |
| crates/arcweft-compiler/tests/project_function_instances.rs / arcweft-compiler | test | 17087 | 509 → 503 | 0 |
| crates/arcweft-lang-sema/src/callable.rs / arcweft-lang-sema | production | 14909 | 247 → 252 | 0 |
| crates/arcweft-lang-sema/src/callable/checked_catalog.rs / arcweft-lang-sema | production | 106633 | 2862 → 2900 | 0 |
| crates/arcweft-lang-sema/src/callable/join.rs / arcweft-lang-sema | production | 82463 | 2098 → 2146 | 0 |
| crates/arcweft-lang-sema/src/callable/join/source.rs / arcweft-lang-sema | production | 27023 | 648 → 664 | 0 |
| crates/arcweft-lang-sema/src/checked_rich_text/model.rs / arcweft-lang-sema | production | 35487 | 1076 → 1086 | 143 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/display.rs / arcweft-lang-sema | production | 13953 | 312 → 315 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use.rs / arcweft-lang-sema | production | 111424 | 2839 → 2850 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/report.rs / arcweft-lang-sema | production | 97874 | 2423 → 2424 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/display_text.rs / arcweft-lang-sema | test | 11026 | 299 → 346 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/higher_order_effects.rs / arcweft-lang-sema | test | 22628 | 707 → 692 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/local_use.rs / arcweft-lang-sema | test | 42889 | 1317 → 1356 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/project_callable_source.rs / arcweft-lang-sema | test | 11670 | 272 → 326 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/project_specialization.rs / arcweft-lang-sema | test | 13574 | 308 → 378 | 0 |
| crates/arcweft-lang-sema/src/callable/checked_catalog/authority_tests.rs / arcweft-lang-sema | test | 992 | 0 → 25 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/report/project_functions.rs / arcweft-lang-sema | production | 3029 | 0 → 77 | 0 |

Final validation on unchanged candidate Rust bytes: just test-workspace exits 0
with 308 suites, 7204 passed, 0 failed and 24 ignored.
Workspace check and Clippy, all-targets/all-features, both exit 0; format, diff
and structural gate pass. Existing warning categories remain. Receipts use
%TEMP%/arcweft-1002-instance-{test-workspace,workspace-check,workspace-clippy,fmt}.
The final workspace run supersedes the pre-final sema owner run and covers the
added input-call authority check. Dependency edge and package/fan metrics are
unchanged from the preceding cut (SHA256
7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42 and
6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7).
Explicit stage review and non-forced main delivery follow. General closed
execution context/root and owned runner migration remain unfinished; no GPU
or allocation-performance claim is made from these checks.

Delivery receipt: source commit
`756764a81ff186814d5758820d7d64065aeca01f` was committed after the explicit
21-path staged review and pushed non-forced to origin/main. The remote full
SHA matched that commit and the working tree was observed clean. The 7,204-pass
workspace, check, Clippy, format and structure receipts above apply to its exact
Rust bytes. This delivery receipt is documentation only and reuses those passes.
The active source decision remains terminal Move for the same LocalId/generation,
with new shadow declarations independent and active receiver loans protected.
Full convergence remains unfinished. The app-side goal is still blocked, and the
pending resume question concerns that external execution state, not permission
for source work. Subsequent implementation continues at the general closed
execution context/root, canonical invocation inputs and owned runner boundary.

### 2026-10-02 — Closed execution context and owned input certificates

Continuation observed goal status active. Inspected main
`7dcab5928b567e5459c619b0132887743e16f4fd` with a clean starting checkout;
this candidate continues the accepted-instance authority cut. The earlier
app-side blocked/resume limitation is historical and no longer applies.

FinalSemanticAnalysis now issues an opaque CheckedClosedExecutionContext for
one accepted lexical owner. It validates the actual project/symbol lease and
joins the selected frozen instance with the local-use seal internally. Project
and DisplayText instances must match both the report's authority and the source
owner. Uninstantiated contexts do not provide global fallback for declaration
type/const parameters. The context is reusable for value roots within its exact
owner, and rejects another owner's source.

The existing checked_expression_input_abi report entry is removed; issuance
belongs to this context. Input/result types close through its environment.
Requested-root input occurrences, transfer/place certificates and ingress Copy
requirements are owned snapshots, retaining the callable-authority lease and
closed instance identity. validate_for rejects exchanging a snapshot between
instances, declarations or equivalent rebuilds. This avoids borrowing a temporary
instance catalogue when a caller owns the extracted input proof. No lease address
or lookup ID is added to canonical identities/encoding; version markers remain 1.
The existing free-local collector remains the sole origin/containment authority.
Its temporary requested-root type map closes already authenticated sources and
stores no persistent duplicate read index.

The value-evaluation context must distinguish latent callback contracts from
executing a declaration's full body. An initial check of every signature effect
parameter rejected six valid View callback defaults. Those effects are not
executed by constructing the default's callback value. Their actual input/result
types still must close. The callable-Copy ingress fixture genuinely contains a
free effect parameter, and now explicitly uses its selected closed function
instance rather than unbound global evidence. This is not an effects admission
exception: general program admission still must close its actual operational
effects and prove boundary-relative control. The current ABI remains input proof,
not permission to execute an extracted program or consume Return.

Instance local-use publication now enumerates the accepted declaration's
expression/pattern/local inventory instead of scanning all project facts for
its receiver, scheduled callback, dialogue effect and Copy evidence. HIR exposes
existing root-owned map keys; selected semantic facts are still required. This
is an ownership-bounded iteration change, without another HIR walker, resolver
or side index. Global publication retains the complete selected project scan.

Behavior coverage: closed i64 Copy versus Need<i64> Move for the same source;
wrong declaration/foreign instance rejection; owner-bound uninstantiated contexts;
stable input coordinates/results across equivalent rebuilds with distinct
admission leases; snapshots cannot swap instance Copy proofs. Existing default,
field/place, capture/cleanup, source revision and repeated-read cases migrate to
the context. Final sema all-feature library receipt after scoped iteration is
1,148 passed, 0 failed. The initial context compile rejected an ExprId field named
source (thiserror treats that name as an error cause); renamed owner. An unintended
negative-test edit was restored. Intermediate sema runs failed 7 and then 1
fixtures described above; both are superseded by the final owner pass.

Final validation: workspace check and Clippy all targets/all features, format,
and complete workspace recipe all exited 0. The complete workspace receipt is
308 suites, 7,208 passed, 0 failed, 24 ignored. Logs/exit receipts use
%TEMP%/arcweft-1002-context-{workspace-check,workspace-clippy,test-workspace}.
Canonical structure gate passed: 97 packages, 351 review triggers, 0 blocking
violations, 1,470,603 Rust physical LOC; report
%TEMP%/arcweft-1002-context-structure. No dependency/feature changes.

Still unfinished: EvaluateValue versus InvokeBody opaque root admission,
canonical invocation formals/destructuring/unused and synthetic/pipe-left slots,
closed operational effect and relative Return/Loop/Try/Const evidence, replacement
of closure/CaptureId/u16/PureHelper-specific program facts, the owned general
runner and retained View/UI consumers. The full convergence goal remains active.
This candidate is not an assertion that those downstream acceptances are met.


Ownership review: the closed context owns only authenticated lexical environment
and its selected local-use evidence. Requested-root input snapshots own only
selected certificates/types; the transient closure map is not a stored index.
HIR path index remains the single body-owner authority. Existing cohesion
dispositions for semantic_paths, local_use, free_capture and tests remain
applicable; their changed responsibilities remain within those owners. No Cargo
dependency/feature, unsafe boundary or contract version marker changes.

| Path / crate | Class | Bytes | Base → final physical LOC | Embedded tests |
|---|---|---:|---:|---:|
| crates/arcweft-lang-hir/src/final_project/semantic_paths.rs / arcweft-lang-hir | production | 246256 | 6585 → 6595 | 0 |
| crates/arcweft-lang-sema/src/final_analysis.rs / arcweft-lang-sema | production | 13917 | 253 → 255 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/expression_inputs.rs / arcweft-lang-sema | production | 11571 | 275 → 306 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/free_capture.rs / arcweft-lang-sema | production | 27503 | 688 → 723 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use.rs / arcweft-lang-sema | production | 112355 | 2850 → 2876 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests.rs / arcweft-lang-sema | test | 356084 | 10228 → 10230 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance.rs / arcweft-lang-sema | test | 6320 | 157 → 175 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/expression_inputs.rs / arcweft-lang-sema | test | 12544 | 344 → 354 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/return_boundaries.rs / arcweft-lang-sema | test | 4757 | 133 → 130 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/view_defaults.rs / arcweft-lang-sema | test | 16574 | 430 → 428 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/execution_context.rs / arcweft-lang-sema | production | 6263 | 0 → 156 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/execution_context.rs / arcweft-lang-sema | test | 8408 | 0 → 242 | 0 |

### 2026-10-02 — Rust-compatible move semantics; one-shot Astra consultation

The latest direct user instruction supersedes the earlier terminal-Move policy.
Whole-place assignment after moving its value is valid, like Rust's
`let mut x = String::new(); let y = x; x = String::new();`.
Statically decidable initialization, move and borrow obligations must be solved
by the checker. Declaration identity and current value availability remain
distinct; assigning a new value does not revive the consumed value. Existing
terminal-Move statements and receipts above describe historical implementation,
not the current target contract. The full convergence goal remains active.

One fresh-context Astra Max consultation is explicitly requested for this
change. Dispatch marker: task `/root/rust_move_semantics_once_20261002`, state
`completed`, model `gpt-6-astra`, effort `max`, history `none`. This marker
was written before dispatch and updated with the returned canonical task name. Do not repeat the
consultation after compaction; reconcile the live agent inventory if dispatch
outcome is uncertain. The consultation is read-only, with no edits, tests or
delegation. Root retains implementation and validation ownership.

Consultation completed once. Its read-only advice confirms whole-place Assign
and optional displaced values, and requires a temporary ownership CFG issued
by the existing checked evaluation traversal. Solve initialization and eligible
Copy obligations over typed events to a fixed point; retain actual condition,
backedge, Break and Continue destinations. Do not rerun the HIR traversal to
simulate iterations. Protect in-place receiver reservations against operand
move/assignment even if an operand restores initialization before the call.
Field reads currently consume their aggregate and remain a real Rust-parity gap:
canonical move paths and partial place storage must preserve initialized
siblings, complete-value admission and exact remaining-value cleanup. No new
syntax for arbitrary dereference/indexed/nested writable places is implied.
Derive old-value disposition after RHS (absent/present/conditional); conditional
occupancy is a drop flag, not a runtime source-legality decision. Keep all
contracts at 1. Native rollback is supported; public native session persistence
remains unsupported. Full Rust Move parity is not yet claimed.

The preceding context/input cut was delivered as
`8cedccd53b4e58369c5e5b0ea778946544137f8a`; origin/main matched it and the
checkout was clean before Rust-move edits. Current Rust-move WIP preserves
declaration slots, allows whole-local assignment, returns optional displaced
values and updates native/AWBC restore evidence. Narrow tests passed: sema local
use 44, AWBC assignment 3, native environment 10. Initial core test compilation
exposed one stale mandatory-displaced test consumer; migrated it. Shared final
validation and ownership CFG/receiver/partial-path implementation remain pending.

Current candidate extends the same selected evaluation fold with a transient
ownership CFG (`local_use/flow.rs`). Each occurrence is issued once, then typed
Access/Bind/Synthetic events are solved by a worklist. Local initialization uses
Initialized/Uninitialized/MaybeInitialized independently of move-site diagnostic
provenance. Copy ingress demands grow monotonically and final rows are published
only after re-solving the event graph. The old unconditional repeated-loop move
ban and freshness sets are deleted. Loop frames are keyed by the existing checked
body coordinate, not source labels. While/WhileLet/For exits and real Break and
Continue edges are retained; Loop has no artificial zero-iteration exit. Fresh
pattern and pipe bindings are explicit initialization events. Carrier-block Try
residuals and dialogue Out use their exact checked owner exits; callable residuals
leave their frame and do not poison the successful write. Implicit Try/Pipe bodies
use their retained selected body payload, not their callable-creation shell.

Receiver reservations cover both selected borrowed and in-place receivers through
operand evaluation. The owning receiver admission is distinguished from another
overlapping mutation; operand move-and-restore does not erase a reservation.
Assignment with an executable RHS now uses the existing FinalFlowLowerer value
continuation and writes only after the call result is returned. This fixes the
previous missing flow-owned projection for `items = identity(items)`.

Evidence before the final initialization-enum cleanup: sema all-feature library
1,153 passed, 0 failed; compiler native/decoded-AWBC reinitialization cases 4 passed;
native environment 10 and AWBC assignment/restore 3 passed. Earlier fixture runs
used unsupported ordinary-fn While lowering, unnamed block arguments, or an
ambiguous unparenthesized block condition; final loop fixtures exercise the existing
Flow surface, named capacity operands and parenthesized condition. Ordinary-fn
While admission remains an existing downstream language gap, not new evidence.
The flow-event extraction had one compile error from an unreplaced `state` receiver;
corrected. A first Try event implementation missed the retained implicit-callable
Try body; corrected with its typed payload. No broad source fallback is introduced.

The first whole-workspace recipe failed at REPL `api_compile`: trybuild reported
that the private-source-module fixture compiled successfully. No REPL source or
fixture was changed. Its isolated rerun passed (1 test, E0603 fixture accepted as
compile-fail). Cause is not established; do not call it a known baseline issue.
Final whole-workspace rerun session 86044 uses
%TEMP%/arcweft-1002-rust-move-final-workspace.{log,exit}. Final check/Clippy session
46100 uses final-check/final-clippy receipts. These started before the equivalent
initialization-enum cleanup; reconcile final bytes and actual exits before claiming
acceptance. Current final sema session 45228 retains rust-move-sema receipts.
Latest source is still uncommitted; no Rust-move delivery is claimed.

Structure gate before the final residual/event refinements passed: 97 packages,
351 review triggers, 0 blocking, 1,471,063 Rust physical LOC. Dependency-edge and
package metrics hashes match the prior cut. Re-measure final changed owners.
The new flow module owns only temporary static dataflow, not another HIR walker,
runtime resolver, persisted index or public CFG. Existing local-use/effect/lowering,
VM/verifier and environment cohesion dispositions remain applicable. Source
certificates remain generation/instance-bound; no dependency, feature, unsafe or
contract version changes. The one-shot consultation remains completed; do not
dispatch another Astra after compaction. Remaining Rust parity includes canonical
field move paths/shared partial storage and statically derived old-value
dispositions. Full general program/root/owned-runner/UI convergence remains active.

Final candidate also isolates each latent callable body from the enclosing
receiver reservations, Loop/Carrier/Output targets and guard scopes. Creation
captures are checked before entering that separate frame. Its owned Copy capture
can mutate its own frame without invalidating the caller's receiver. The added
regression and full sema library pass: 1,154 passed, 0 failed.

Final-byte validation receipts (rust-move-delivery logs/exit files under %TEMP%):
workspace check all targets/all features, workspace Clippy all targets/all
features, format and canonical structure gate all exited 0. Complete
`just test-workspace`: 308 suites, 7,218 passed, 0 failed, 24 ignored, exit 0.
The previous full rerun passed 7,217 before the added callable-frame regression;
the delivery run supersedes it. REPL compile-fail passed in both reruns; its first
failure remains recorded above rather than silently counted as a pass.

Final structure: 97 packages, 351 review triggers, 0 blocking violations,
1,471,233 Rust physical LOC. Dependency edges/package metrics retain SHA256
7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42 and
6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.
This is static ownership/source/runtime/codec evidence, not GPU/performance
or full Rust field-move acceptance. Explicit staged review and main delivery
follow; the convergence goal remains active after this coherent cut.

| Path / crate | Class | Bytes | Base → final physical LOC | Embedded tests |
|---|---|---:|---:|---:|
| crates/arcweft-compiler/tests/callable_execution.rs / arcweft-compiler | test | 35324 | 1266 → 1299 | 0 |
| crates/arcweft-core/src/awbc/tests/local_assignment.rs / arcweft-core | test | 7248 | 199 → 203 | 0 |
| crates/arcweft-core/src/awbc/verify/code.rs / arcweft-core | production | 224591 | 5759 → 5759 | 0 |
| crates/arcweft-core/src/awbc/vm.rs / arcweft-core | production | 218120 | 5291 → 5306 | 0 |
| crates/arcweft-core/src/engine/dialogue/store.rs / arcweft-core | production | 65938 | 1751 → 1757 | 581 |
| crates/arcweft-core/src/value/env.rs / arcweft-core | production | 31175 | 886 → 875 | 297 |
| crates/arcweft-lang-sema/src/final_analysis/local_use.rs / arcweft-lang-sema | production | 120315 | 2876 → 3038 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use/access.rs / arcweft-lang-sema | production | 2095 | 68 → 69 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/local_use/flow.rs / arcweft-lang-sema | production | 9249 | 0 → 298 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/local_use.rs / arcweft-lang-sema | test | 47466 | 1356 → 1456 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/semantic_transcript_acceptance/expression_inputs.rs / arcweft-lang-sema | test | 12543 | 354 → 354 | 0 |
| crates/arcweft-runtime-plan/src/final_expr.rs / arcweft-runtime-plan | production | 135942 | 3337 → 3342 | 0 |
| crates/arcweft-runtime-plan/src/final_flow.rs / arcweft-runtime-plan | production | 358534 | 8648 → 8665 | 398 |

Delivery receipt: `4b076ec754b45a86a28a6fa185131ccd9ef49136` committed the
explicitly reviewed 16-path candidate and pushed non-forced to origin/main.
The full remote SHA matched and the checkout was observed clean before this
documentation update. Final 7,218-pass workspace, 1,154-pass sema, check/Clippy,
format and structure receipts apply to those exact source bytes. The requested
one-shot Astra consultation is completed and committed in this record; do not
repeat it after compaction. This cut closes whole-local initialization and its
static control-flow foundation, not the full convergence or partial-field goal.

### 2026-10-02 — Partial place migration in progress (uncommitted)

Base main/origin is `4b076ec754b45a86a28a6fa185131ccd9ef49136`.
The checkout contains this in-scope migration and the previous delivery receipt;
no new commit/push is claimed. The one-shot Astra consultation above remains
**completed**; do not dispatch it again after compaction.

A new field regression reproduced `pair.other` consuming an affine aggregate
and rejecting the later sibling. It now passes (1/1 sema owner test). A separate
negative regression rejects whole-place use and moved-child use after a partial
move (1/1 pass). Source uses admitted `RichTextStyle` fields; no unsupported
resource leaf is smuggled into the project nominal schema.

WIP adds schema-selected local move paths to the existing local-use authority
and temporary ownership CFG, leaf Copy classification, prefix overlap for
receiver loans, and field assignment initialization. Runtime local reads carry
that path through plan construction; storage is a shared typed tree of complete
values, vacant cells, and partial record headers/children. Native declaration
slots, AWBC registers, rollback and inert snapshots are being migrated together.
`ReadPlace` is added to version-1 AWBC with typed mode/field IDs; its verifier
tracks moved child paths, and the VM retains siblings. Ownership inventories
include surviving children. Public environment snapshots preserve slot storage.
Old field-write helpers have been removed. Scalar AOT/JIT and compiled AWBC
regions explicitly exclude projected place reads until their actual scalar
subset supports them.

Validation is not complete: core all-target/all-feature check passed before the
latest new tests; workspace check identified codegen/JIT consumer repairs, now
edited but not rerun. New AWBC regression initially failed to compile because
it used guessed `Named`/`snapshot` APIs; `Record` is corrected and the snapshot
API repair remains to finish. No workspace suite/lint/structure gate pass is
claimed for these bytes. Full owner tests, codec/partial snapshot corruption and
resource cleanup coverage, native/decoded compiler coverage, final validation,
maintained contract updates and staged review/delivery remain required.

Rust reference evidence: [Partial moves](https://doc.rust-lang.org/rust-by-example/scope/move/partial_move.html)
confirms that initialized fields survive a partial move and whole use is blocked;
indivisible destruction owners do not admit partial moves. The implementation
retains opaque producer storage as indivisible.
2026-10-02 12:25 JST continuation: the partial migration remains uncommitted.
The latest full sema run passed 1,157/1,157 after repairing View default input
projection. A subsequently added disjoint sibling/receiver-loan regression passed
1/1; its first insertion accidentally landed inside another raw fixture and was
corrected before rerunning. The shared prepared/final effect fold and free-input
collector now derive the same typed local/record-address root, so a View default
field receiver is not evaluated as a second whole-parent transfer.

Core `partial` owner selection passed 13/13, including actual StageActor cleanup
reconciliation, inert codec/snapshot/checkpoint replay, malformed partial shapes,
and atomic rejection of corrupt headers/child types. A focused native + decoded
AWBC compiler field-read case passed 1/1. These passes precede the latest common
address-factory/move-path owner cleanup and initialized-field assignment fast
path, so final required checks are still pending. The complete sema/partial/core
logs and `.exit` files are under `%TEMP%/arcweft-1002-partial-*`.

Workspace checks exposed and repaired the codegen/JIT subset admission, a
runtime-plan test's register inventory, and two compiler snapshot-test uses of
old Option-register APIs. The last workspace run failed on those last two tests;
there is no workspace pass for the current bytes yet. Maintained block-scope and
AWBC storage/opcode contracts have been updated in place at version 1. Never
publish this WIP or mark the convergence/Rust parity goal complete until its
remaining owner/integration checks and review are closed. The one-shot Astra
consultation is still completed; do not repeat it.

### 2026-10-02 — Partial-place candidate review and structure evidence

Inspected base remains `4b076ec754b45a86a28a6fa185131ccd9ef49136` on
existing main; 51 tracked paths and the new place_storage owner are in-scope dirty
changes. This supersedes the earlier WIP statements about unresolved API compile
repairs: workspace check all targets/all features and Clippy all targets/all
features now exit 0. Format and diff checks exit 0. The complete workspace test
run is still pending, so no final delivery or full Rust parity is claimed.

Reviewed the complete production/test/contract diff, including the untracked
storage module. `RuntimePlaceStorage` owns the shared initialized/partial storage
state, mapping to inert snapshots, separable record projection/restoration and
remaining-owner inventory. Its private record header and state enum retain the
complete-value invariant. Public read-only slot snapshots preserve declaration
identity and partial state; no alternate copied availability authority is stored.
The semantic `MovePath` is temporary CFG state keyed by admitted field identities.
The existing local-use catalog is the sole published read/write certificate.
The common typed field-address grammar feeds prepared/final effects, free-input
projection and transfer issuance; no consumer reconstructs paths from spelling.

Cohesion disposition for touched upper-trigger owners: AWBC schema/codec own one
instruction/wire algebra; verifier owns fixed-point type/ownership admission;
VM owns instruction execution; fiber owns live/rollback/snapshot state and exact
program admission. Product line/suspension owners retain their respective handle
custody and resume transactions. Value and plan seed/lower own the admitted value
algebra and seed-to-executable conversion. Pure/engine evaluators and scalar
codegen retain their existing execution responsibilities; scalar subsets reject
unsupported projected reads explicitly. Sema local-use owns CFG/loan issuance;
model/prepared/capture retain their phase-specific contracts; analyzer call,
expression and evaluated-effect owners issue those contracts; statement effects
fold the selected graph. Runtime-plan final-expression and AWBC-expression
owners consume the final certificates. New storage tests follow storage, schema,
cleanup and atomic restore boundaries. Existing tests remain with their execution
owners. No new dependencies, features, I/O, unsafe, generated source, facade split
or API widening solely for file size was introduced. The new 511-LOC storage owner
is a cohesive state boundary; all other owners grow less than 300 physical LOC.

Canonical structure gate exits 0: 97 packages, 351 review triggers, 0 blocking
violations, 1,472,982 Rust physical LOC, 2,554 Rust files and 2,684 scanned files.
A non-source report retained under `%TEMP%/arcweft-1002-partial-structure-review`
provides exact file/embedded-test metrics and Cargo graph measurements. Normal
fan-in/out: core 30/7, sema 8/14, runtime-plan 5/10, compiler 3/24,
runtime-codegen 1/1, JIT 2/1, CLI 0/53. Development fan-in/out respectively:
3/5, 3/0, 5/0, 1/9, 0/0, 0/0, 0/3. Dependency/package metric SHA256 remain
7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42 and
6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.

Exact current measurements below use the scanner's embedded-test calculation;
base LOC is the complete file at the inspected SHA, not diff additions.

| Path / owning crate | Class | Bytes | Base → candidate physical LOC | Embedded test LOC |
|---|---|---:|---:|---:|
| crates/arcweft-cli/src/app/jit.rs / arcweft-cli | production | 58644 | 1638 → 1640 | 0 |
| crates/arcweft-compiler/src/project/tests.rs / arcweft-compiler | test | 82081 | 2322 → 2331 | 0 |
| crates/arcweft-compiler/tests/callable_execution.rs / arcweft-compiler | test | 35768 | 1299 → 1314 | 0 |
| crates/arcweft-core/src/awbc/codec/code.rs / arcweft-core | production | 85654 | 2263 → 2297 | 359 |
| crates/arcweft-core/src/awbc/fiber.rs / arcweft-core | production | 257034 | 6834 → 6904 | 877 |
| crates/arcweft-core/src/awbc/product_step/line.rs / arcweft-core | production | 177679 | 4173 → 4186 | 74 |
| crates/arcweft-core/src/awbc/product_step/suspension.rs / arcweft-core | production | 114982 | 2824 → 2824 | 0 |
| crates/arcweft-core/src/awbc/product_step/tests.rs / arcweft-core | test | 203624 | 5335 → 5335 | 0 |
| crates/arcweft-core/src/awbc/schema.rs / arcweft-core | production | 113257 | 3545 → 3563 | 0 |
| crates/arcweft-core/src/awbc/tests.rs / arcweft-core | test | 283165 | 7890 → 7891 | 0 |
| crates/arcweft-core/src/awbc/verify/code.rs / arcweft-core | production | 230135 | 5759 → 5899 | 0 |
| crates/arcweft-core/src/awbc/vm.rs / arcweft-core | production | 219307 | 5306 → 5332 | 0 |
| crates/arcweft-core/src/engine/dialogue/store.rs / arcweft-core | production | 69794 | 1757 → 1846 | 670 |
| crates/arcweft-core/src/engine/dialogue.rs / arcweft-core | production | 145695 | 3376 → 3381 | 721 |
| crates/arcweft-core/src/engine/eval.rs / arcweft-core | production | 63687 | 1568 → 1568 | 124 |
| crates/arcweft-core/src/plan/construction/lower.rs / arcweft-core | production | 242356 | 5692 → 5704 | 269 |
| crates/arcweft-core/src/plan/construction/seed.rs / arcweft-core | production | 91556 | 2870 → 2894 | 0 |
| crates/arcweft-core/src/pure.rs / arcweft-core | production | 129325 | 3416 → 3420 | 134 |
| crates/arcweft-core/src/value/place_storage.rs / arcweft-core | production | 17425 | 0 → 511 | 105 |
| crates/arcweft-core/src/value.rs / arcweft-core | production | 138864 | 3802 → 3818 | 0 |
| crates/arcweft-lang-jit-cranelift/src/lower.rs / arcweft-lang-jit-cranelift | production | 61266 | 1511 → 1516 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs / arcweft-lang-sema | production | 252825 | 5803 → 5809 | 156 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/evaluated_effects.rs / arcweft-lang-sema | production | 101015 | 2190 → 2196 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs / arcweft-lang-sema | production | 216556 | 4997 → 5003 | 88 |
| crates/arcweft-lang-sema/src/final_analysis/local_use.rs / arcweft-lang-sema | production | 123115 | 3038 → 3115 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/model/capture.rs / arcweft-lang-sema | production | 47504 | 1268 → 1273 | 381 |
| crates/arcweft-lang-sema/src/final_analysis/model.rs / arcweft-lang-sema | production | 103143 | 3086 → 3108 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/prepared.rs / arcweft-lang-sema | production | 55016 | 1646 → 1656 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/statement_effects.rs / arcweft-lang-sema | production | 61715 | 1565 → 1578 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/tests/local_use.rs / arcweft-lang-sema | test | 49625 | 1456 → 1530 | 0 |
| crates/arcweft-runtime-plan/src/awbc_lower/expr.rs / arcweft-runtime-plan | production | 91778 | 2369 → 2385 | 0 |
| crates/arcweft-runtime-plan/src/awbc_lower/tests.rs / arcweft-runtime-plan | test | 74699 | 2059 → 2059 | 0 |
| crates/arcweft-runtime-plan/src/final_expr.rs / arcweft-runtime-plan | production | 137230 | 3342 → 3371 | 0 |

The first full partial-place workspace run stopped after 89 suites: 1,017 passed,
1 failed, 0 ignored, exit 1. `nominal_field_awbc_roundtrip_rejects_wrong_ordinals_and_replacement_types`
still searched for the replaced `ProjectRecord` instruction, so its corruption
fixture could not find the read. It now corrupts the admitted `ReadPlace` field
path and requires typed rejection of the nonexistent child; wrong-write and
wrong-value rejection assertions remain. The focused repair and final full rerun
are pending. This failure is not counted as a complete-workspace pass.

The second workspace rerun reached 110 suites: 1,957 passed, 1 failed,
0 ignored, exit 1. `fiber_checkpoint_and_serde_preserve_cleanup_stacks`
constructed cleanup entries for effect 0 without any effect plan. Atomic restore's
program admission correctly rejected that invalid fixture. The test now supplies
an admitted Log effect/signature/String type and validates both its program and
live cleanup state before snapshotting. Focused iterations exposed missing Log
static arguments and the default Unit type occupying ordinal 0; those fixture
errors were corrected, rather than weakening restore validation. The final focused
cleanup/checkpoint test passes 1/1, exit 0. The compiler ordinal corruption repair
also passes 1/1. Third full rerun is in progress; its latest Clippy and format
checks exit 0. Original failures are retained as failed evidence.

### 2026-10-02 — Close partial storage across scheduled child custody (WIP)

Base remains `4b076ec754b45a86a28a6fa185131ccd9ef49136`; no commit/push
has occurred. The reviewed 52-path candidate was explicitly staged. Its third
full workspace run passed 308 suites / 7,232 tests, 0 failed, 24 ignored, exit 0;
check/Clippy/format/structure also passed. Those receipts apply to that candidate,
not the subsequent custody repair. Its final structure had 1,473,015 Rust physical
LOC, with compiler callable_execution 35,895 bytes / 1,315 LOC and core AWBC tests
284,292 bytes / 7,923 LOC after the two fixture repairs.

Final consumer inspection disproved the earlier assumption that every flat
`RuntimeLocalBinding` forest was cleanup-only. Native child completion passed
`env.into_bindings()` to scheduled custody, whose typed admission requires unique
declaration IDs. A partial root with two surviving fields became two bindings
with the same ID and was rejected. AWBC completion/cancellation extracted only
complete parameter values, omitting partial parameter owners. This is an in-scope
producer/consumer defect, not a deferred follow-up or external blocker.

The current repair retains declaration identity and the shared storage tree in
`RuntimeLocalSlot<T>` for live and inert images. Native child completion exports
slots, and AWBC exposes borrowed/owned parameter storage in sealed positional
order. One helper validates every parameter's exact schema before any transfer;
owned extraction uses mem::take and no longer clones the frame. Incoming capture
packets remain complete bindings; terminal LineScope custody, its snapshot,
completion preflight/commit, ownership walks and cancellation all use returned
slots. Native rollback also maps the same slot carrier. The obsolete flat export
and Option-parameter extraction APIs are removed. Contract versions remain 1;
no legacy reader or malformed value placeholder is introduced.

The two new regressions cover unique-ID custody admission with two surviving Need
owners and snapshot replay, plus verified AWBC partial-parameter borrow/transfer.
The core `partial` selection passes 15/15 on the latest bytes. Initial test builds
failed on missing explicit private-test imports and a guessed ownership enum;
the real AwbcFunctionInputOwnership is a struct and its default admits Owned.
Those test-only failures were repaired and remain recorded as failures. The
previous core compile pass precedes the latest Env rollback/test changes; current
workspace check, Clippy, full suite and structure evidence is being rerun under
`%TEMP%/arcweft-1002-slot-custody-*`. Current source is frozen for those runs.

Reviewed the complete eight-owner custody diff. The scheduled handle owner keeps
its existing Packet/ChildFiber/LineScope state machine, token custody, typed
preflight/commit and inert codec boundaries; its tests mutate those same states.
Engine and product-step remain the native/AWBC reducers, not new storage owners.
The generic slot is a shared declaration carrier with private fields and controlled
mapping; its API is required by transport/snapshot roles, not file splitting.
The parameter extraction helper owns exact input-storage admission and mutation.
No new dependency, feature, I/O or unsafe boundary is added. Structural gate
passes: 97 packages, 351 review triggers, 0 blocking violations, 1,473,184 Rust
physical LOC / 2,554 Rust files / 2,684 scanned files. Production owners grow
less than 300 LOC apart from the already justified new storage owner. The
local_assignment test owner grows 322 LOC (203 to 525): it remains the cohesive
whole/field assignment, child availability, typed parameter custody and
codec/checkpoint rejection fixture boundary, within the ordinary module size
range. No unrelated test infrastructure or state cluster was added. Exact metrics:

| Path (all arcweft-core) | Class | Bytes | Base → current physical LOC | Embedded tests |
|---|---|---:|---:|---:|
| crates/arcweft-core/src/awbc/fiber.rs | production | 256047 | 6834 → 6883 | 877 |
| crates/arcweft-core/src/awbc/product_step/line.rs | production | 177645 | 4173 → 4184 | 74 |
| crates/arcweft-core/src/awbc/product_step.rs | production | 210182 | 5212 → 5215 | 0 |
| crates/arcweft-core/src/awbc/tests/local_assignment.rs | test | 18990 | 203 → 525 | 0 |
| crates/arcweft-core/src/engine.rs | production | 155371 | 3951 → 3960 | 62 |
| crates/arcweft-core/src/line_task/handle.rs | production | 221151 | 5622 → 5737 | 737 |
| crates/arcweft-core/src/value/env.rs | production | 33652 | 875 → 941 | 374 |
| crates/arcweft-core/src/value.rs | production | 139765 | 3802 → 3849 | 0 |

The one-shot Astra consultation remains completed; do not repeat after compaction. Static after-RHS previous-value disposition and the full general execution-root/UI convergence remain pending. This record does not claim complete Rust move/borrow parity or native public-session save support.

2026-10-02 14:18 JST final custody candidate receipts: complete
`just test-workspace` passes 308 suites, 7,234 passed, 0 failed, 24 ignored,
exit 0. Workspace all-target/all-feature check and Clippy, format check,
canonical structure gate and cached diff check exit 0. The latest
slot-custody-workspace logs/exit files apply to the current frozen source;
the earlier 7,232-pass candidate is superseded. Clippy/test builds retain
warnings; no warning-free claim is made. Matching device/GPU, stdio/MCP,
readback/auxiliary attachment and production-limit paths were not changed;
no new Tier-2 or performance measurement is claimed for this storage cut.

All 55 in-scope paths are explicitly staged and match the reviewed working
files, including the new storage module. Full diff review covers the initial
record-place migration and the additional custody repair. Fetch confirms
HEAD/origin/main both remain `4b076ec754b45a86a28a6fa185131ccd9ef49136`
before committing. The structure graph hashes remain the two recorded hashes;
final source has 1,473,184 Rust physical LOC with 351 review triggers and no
blocking violation. This closes the record-local partial storage/custody cut.
Static after-RHS old-value dispositions, complete general execution roots and
owned input/runner migration, precise synthetic/capture boundaries, and the
retained View/runtime-plan/scheduler convergence remain active acceptance.
Do not mark the full goal or complete Rust parity achieved. The requested
one-shot Astra consultation is completed; never dispatch it again after compaction.

Delivery receipt: `7190f3582d48e0cac33f0ba39bd9d54e58b2b811` commits the
reviewed 55-path record-place/custody migration and was pushed non-forced to
origin/main. The full remote SHA matched and the checkout was observed clean.
Final 7,234-pass workspace, check/Clippy/format/structure receipts above apply to
that source. The one-shot Astra consultation remains completed and must not be
repeated. Full convergence remains active.

The next private storage-walk adjustment removes the traversal-stack allocation
for vacant/complete roots; a stack is populated only when visiting partial child
cells. Borrowed and owned inventory keep the same deterministic order and
complete-value grouping. The existing nested remaining-owner test now also checks
the borrowed inventory order. No public/serialized/type/ownership contract,
dependency or scheduling change is made. Focused/core validation and lint are
required; no GPU or measured performance claim is inferred from this source edit.

Storage-walk final evidence: core all-feature library tests pass 796/796,
core all-target/all-feature check and Clippy exit 0, format and diff checks exit 0.
This is an isolated traversal implementation adjustment; public contract/schema,
features/dependencies and integration boundaries are unchanged from the tested
7190f3582d48e0cac33f0ba39bd9d54e58b2b811 cut. Its broader integration/structure
receipts are reused for those unchanged boundaries, not represented as a new
full workspace run of these source bytes. The earlier storage cohesion review
remains applicable. No stack allocation occurs for vacant/complete inventory
roots in this implementation; output collections still allocate when returning
values. Actual runtime/GPU speed was not measured. General root/owned-runner and
static old-value disposition acceptance remains active; Astra was not reconsulted.

Storage-walk measurement at base 7190f3582d48e0cac33f0ba39bd9d54e58b2b811: arcweft-core/src/value/place_storage.rs, handwritten production/test owner, 17,766 bytes, 511 → 520 physical LOC, 112 embedded test LOC; responsibility and dependency disposition unchanged.

Delivery receipt: `15d7c14dff115634baca21d3a092ca3706d8aef2` commits the two-path private place-walk adjustment and was pushed non-forced to origin/main; the full remote SHA matched and the checkout was clean before continuing. The core 796-pass/check/Clippy receipts above apply to those source bytes.

### 2026-10-02 — General body execution roots in progress

Inspected main HEAD is `15d7c14dff115634baca21d3a092ca3706d8aef2`. Six tracked sema files are dirty, all from this continuation: final_analysis.rs and its analyzer/callable_effect_graph.rs, execution_context.rs, execution_regions.rs, report.rs, statement_effects.rs. No other WIP was observed. Current changes add declaration body keys to the existing selected execution DAG and distinguish EvaluateValue from InvokeBody at the report-issued context boundary. Parameter defaults remain separate execution phases. Empty declaration bodies have roots without an invented ExprId.

Intermediate sema validation before context generalization passed 1,158/1,158 library tests and all-target/all-feature check. The latest context check then failed E0308 because the generalized source argument was accidentally applied to private execution_source_scope instead of public checked_execution_context. The exact signatures were repaired; arcweft-1002-general-root-context-repair.log/.exit records all-target/all-feature check exit 0. Warnings remain. A preceding temporary operation-key check failed E0507 on copying a now-shared body key and was repaired with a clone; these failures are not represented as passing checks.

This is uncommitted producer work, not completed general program admission. Exhaustive declaration-body publication, full formal/free/synthetic input ABI, intent/result/control authority, runtime-plan binding and actual owned native/AWBC runner consumers remain to close before delivery. Static after-RHS old-value disposition and remaining move/borrow precision also remain acceptance. The active convergence goal is unchanged. The explicitly requested one-shot Astra consultation is already completed under /root/rust_move_semantics_once_20261002, gpt-6-astra/max with no inherited context; NEVER repeat or resume that consultation after compaction.

General-root continuation evidence (same base main `15d7c14dff115634baca21d3a092ca3706d8aef2`, still uncommitted): the expression-only input ABI is replaced by checked_execution_input_abi and CheckedExecutionInputAbi. Value creation, callable-body invocation and declaration-body invocation retain distinct stable coordinates. Shared DAG publication now requires the exhaustive selected declaration body keyset and exact body children. Its corruption test first publishes a valid complete fixture, then checks missing-body, cycle and foreign-edge rejection, avoiding a false-positive missing-body rejection in the cycle test.

Formal layout owns complete signature groups, parameter patterns, unused bindings, attached content and implicit parameters. The context retains the same authenticated HIR view for direct closure parameter projection. Source parameter origins are indexed once per queried owner rather than scanned per parameter; binding coordinates are authenticated and sorted, never sorted by an error-swallowing optional coordinate. Actual execution membership remains owned by the shared selected DAG. Synthetic `_` and pipe accesses retain local-use and Copy ingress evidence. Body effects exclude default phases and preserve latent implicit-call effects. Body admission now checks the body key without expanding the DAG just to establish scope. The obsolete private expression-only admit_source method is removed.

Validation: intermediate seven new body/input tests plus the complete sema library pass 1,165/1,165; after the formal-layout owner is extracted to execution_inputs/parameters.rs, sema again passes 1,165/1,165. The first new owner test run had two fixture failures: a normal empty fn has an HIR block expression, and ordinary project-fn defaults are unsupported by registration. Final fixtures use an empty Flow and supported View defaults; 5/5 owner tests then pass. Earlier input API check had one unavailable selected-graph helper and four stale type/path test consumers; all were migrated. General-root workspace all-target/all-feature check and sema Clippy pass before the subsequent displacement work, with warnings retained. The corresponding general-root structure gate passes: 97 packages, 351 review triggers, 0 blocking, 1,474,408 Rust physical LOC. Graph hashes retain the prior two SHA256 values. That structure receipt predates the parameter-owner extraction and displacement work, and is not a final-source measurement.

Ownership disposition: execution input evidence remains one report/context-bound authority. Its formal-layout child owns signature arity/pattern/binding authentication; the parent owns selected execution occurrences and ingress certificates. This is a real producer/API responsibility boundary, with no public widening solely for file splitting, no persisted side index and no alternate HIR execution walker. The existing effect-fold/report/local-use owner dispositions remain applicable to their extended root and certificate responsibilities. General runtime-plan program binding and owned native/AWBC runner migration remain incomplete; the old RuntimePureProgramFact/RuntimePureProgramCaptureFact and helper-only consumers are still present. No new executable general-program admission, commit/push, GPU/device performance, Tier-2 or full convergence claim is made.

### 2026-10-02 — Static post-RHS cleanup contours in progress

The same selected ownership CFG now returns a transient solution containing violations and post-RHS assignment cleanup contours. Only the final accepted Copy/availability solve seals these onto CheckedLocalPlaceAccess. Reachable contours distinguish initialized, uninitialized and conditional roots plus typed moved-child facts; unreachable writes are explicit. Moved-child schema evidence comes from the owning move occurrence, not source reconstruction or a separate persistent index. Mutate accesses have no displacement. Native/AWBC instruction/seed/cleanup consumers have NOT yet been migrated to these contours.

Sema local-use owner tests pass 55/55, including definite old values, move then reinitialization, nested writes, conditional joins, RHS consumption and conditional RHS consumption. A whole-record replacement regression retains its moved child's exact field coordinate and uninitialized state. Complete sema library passes 1,166/1,166, 0 failed. Receipts: arcweft-1002-displacement-local-use-tests and displacement-sema-tests logs/exit files under %TEMP%; sema all-target/all-feature check also passes before adding the final test assertions.

AWBC verifier preparation replaces bool initialization and union-only moved-child flags with a three-state join that retains definite absence separately from conditional absence. Reads still require definite initialization. Review found fresh result/output vacancy checks also must require definite absence; the former boolean model could treat a conditionally occupied slot as vacant. SequenceNext, line-operation and format-result vacancy checks now reject that case. New typed CFG regression sequence_output_cannot_overwrite_a_conditionally_initialized_slot passes 1/1 and includes a positive case that clears the slot on every incoming path. Core all-target/all-feature check and 796/796 library tests passed before the stricter vacancy checks/new regression. They are intermediate receipts, not final-source full-core validation.

Current shared validation is running: session 31303 workspace check, 24084 workspace Clippy, 56016 just test-workspace; receipts use %TEMP%/arcweft-1002-displacement-workspace-{check,clippy,tests}.log/.exit. Inspect their actual handles/exits on continuation; do not restart solely from a timeout. New core verifier code and Sema displacement contours are uncommitted producer/verification work. Complete runtime assignment seed/native/AWBC/codec/verifier consumption of post-RHS contours, appropriate integration checks and review before delivery. General program/root/owned-runner and retained UI/runtime-plan/scheduler acceptance remain active. The one-shot Astra consultation is ALREADY COMPLETED; never repeat it after compaction.

Latest frozen-source receipts: workspace all-target/all-feature check exits 0 (session 31303 completed); workspace Clippy all-target/all-feature exits 0 (session 24084 completed, warnings retained). The full workspace test session 56016 remains authoritative and live; its current output is the compile-fail fixture family and no exit receipt exists yet. Do not claim the recipe passed or restart it. Format and diff checks pass. Latest structure gate exits 0: 97 packages, 351 review triggers, 0 blocking, 1,474,803 Rust physical LOC, 2,556 Rust files and 2,686 scanned files. Graph hashes retain the recorded values.

Final measured owners at base 15d7c14dff115634baca21d3a092ca3706d8aef2: core/awbc/verify/code.rs 232,069 bytes, 5,899 -> 5,953 physical LOC; core/awbc/tests/local_assignment.rs 23,013 bytes, 525 -> 638 test LOC; sema/final_analysis/execution_inputs.rs 25,021 bytes, 593 production LOC, replacing the 306-LOC expression_inputs.rs; its formal-layout child parameters.rs 13,595 bytes, 281 production LOC; new tests/execution_inputs.rs 13,028 bytes, 355 test LOC; local_use/access.rs 6,590 bytes, 141 -> 205 production LOC; local_use/flow.rs 14,284 bytes, 342 -> 415 production LOC. No embedded tests in these production owners. The formal child decomposition follows signature-layout authentication, while the parent follows execution occurrence/ingress issuance; source access, final authority and dependency direction are unchanged. The core verifier remains one coherent opcode/terminator/CFG/copy/initialization authority; its existing upper-size disposition remains applicable, with no I/O or duplicate persisted state introduced.

No new delivery or completion claim: all current changes remain uncommitted, including Sema body/input and post-RHS contour producers plus precise AWBC initialization/vacancy validation. Actual assignment metadata/codec/seed/native/AWBC cleanup consumption and general program/root/owned-runner migration remain required. The full convergence objective stays active. Astra's requested one-shot Rust move consultation remains completed and MUST NOT be repeated.

### 2026-10-02 — Post-RHS assignment consumer migration

Base remains main 15d7c14dff115634baca21d3a092ca3706d8aef2; dirty paths are the in-scope general execution-input producer, static displacement producer and their assignment consumers. Supersedes the live-session status above: displacement workspace check/Clippy/test sessions 31303/24084/56016 all completed with exit 0; the full recipe ran 308 suites, 7,243 passed, 0 failed, 24 ignored. Those receipts precede this assignment contract migration and do not validate its bytes.

RuntimeAssignmentSeed/RuntimeAssignment now carry the sealed post-RHS contour on the owning Assign. Runtime-plan requires the exact checked access displacement; construction resolves every child path against its assigned type and canonicalizes defining-order field coordinates. Native expression, flow, AOT and dialogue assignment consumers use it. Runtime-owned internal destination writes use an explicitly dynamic contour through the same assignment transaction; no source assignment fallback exists. Native preflight retains incoming value and existing storage on mismatch, and authorizes cleanup of only remaining old owners.

AWBC Assign retains opcode 0x11 and contract version 1, adding the same typed initialization/child contour to its canonical wire payload. Its verifier checks exact static initialization after the CFG fixed point, including joins/backedges, child schema paths and coverage of partial moves. A conditional annotation cannot replace a definite bytecode fact. VM validates the old storage/handle graph before consuming the RHS register, then publishes discarded remaining owners through the existing cleanup transaction. Native slots and AWBC registers/snapshots continue using the shared partial storage/drop flags; no parallel storage, legacy codec or source reconstruction is introduced.

Intermediate failures were missing displacement fields in four AWBC fixture constructors, then a new native regression accidentally placed outside its test module. Constructors and the test boundary are corrected, along with two compiler instruction-pattern consumers. Core library after the initial bridge passes 797/797; after native input-retention and forged-codec-contour regressions it passes 799/799. These are intermediate receipts (contour-core-tests and contour-core-final-tests); later compiler integration cases and the extra forged conditional annotation still require final shared validation. New native/decoded-AWBC cases cover conditional RHS consumption and whole-record replacement after multiple partial moves. The full convergence goal remains active. General program/owned-runner, retained UI/runtime-plan and scheduler acceptance remain unresolved. Astra one-shot Rust move advice is already completed and MUST NOT be repeated.

Integration correction: the initial exact-contour compiler run failed 4/136. Conditional source assignments were distributed into both continuation branches, where bytecode dataflow was definite. An earlier synthetic control thunk also eagerly moved enclosing free locals before branch selection. These are different boundaries: value-producing If/IfLet/Match now emit CFG in the same lexical frame, while prepared conditional cleanup facts are narrowed by the same AWBC fixed-point authority after continuation distribution. Definite source facts cannot change; wire/product verification accepts only exact finalized facts. Sealing records transient instruction updates and applies them only after successful structural/code validation. Construction permits no public entrypoint yet (the existing parameterized-Flow fixture installs its entry later); final product admission still requires its configured entry policy. No old wire reader or second flow interpreter exists.

The open function instruction range/entry safe point now has one emission owner in AwbcInventory, shared by flow/expression/trait/pure-helper builders. Individual body builders retain their control/suspension flags and block-range identity, not copied instruction cursors. Obsolete Control pending thunks, synthetic control callable type/state factories and the pending synthetic state table are deleted. Protected fmt operands remain genuine independent invocations and retain their separate capture authority.

Pattern Guard is a typed BindPattern mode at version 1. Lowering projects only actual guard-used bindings from the original pattern. Bytecode requires producer-proven unrestricted values for every projected target; VM preflights all projected copies before writes and retains the candidate. On success, the guard scope exits and the original pattern moves into body bindings. Match/IfLet value roots, inline value control, Flow guarded candidates and while-let guarded bodies share this behavior. Native/decoded-AWBC regression exercises both guard outcomes with an affine tuple and a Copy field. Its initial fixture used Rust `if` instead of Arcweft `when`; source grammar was corrected. Intermediate inline-control run still failed 4/136 before adding the required post-continuation static refinement. The corrected compiler callable suite passes 136/136 (contour-sealed-callable-tests), including move/reinitialization, RHS self-transfer, both conditional RHS paths, both guard paths and existing control/call/nominal tests.

A proposed compiler partial-record fixture using Vec<Content> fields failed earlier during nominal schema projection: accepted standard DialogueContent was treated as requiring structural Rust ADT metadata. That fixture is removed because this cut does not establish general nonstructural nominal leaf admission. This is unresolved C6/general runtime-type acceptance, not a claimed partial-record source pass. Actual whole-partial replacement is covered at native storage/cleanup and decoded AWBC boundaries, including rejection of omitted moved-child evidence and retention of only remaining owners. No RuntimeTypeSchema fallback or unsupported leaf was disguised as a structural record. General program/owned-runner and remaining nominal/UI/scheduler convergence work is still required.

Final shared checks now running on the frozen source: contour-workspace-tests session 43358 (just test-workspace), contour-final-workspace-check session 1646, contour-final-structure session 96428; inspect actual handles and exit receipts. Compiler 136/136 receipt predates the final entrypoint-construction policy and atomic-sealing regression, which are included in these shared checks. Do not claim an earlier workspace pass validates this expanded source. The one-shot Astra consultation remains completed and MUST NOT be repeated.

Ownership/measurement disposition on frozen source at 15d7c14dff115634baca21d3a092ca3706d8aef2: canonical structural gate passes (exit 0), 97 packages, 351 review triggers, 0 blocking violations, 1,475,983 Rust physical LOC, 2,559 Rust files, 2,689 scanned files. Dependency/package CSV SHA256 remain 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42 and 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7. Production workspace fan-in/fan-out are unchanged: core 30/7, sema 8/14, runtime-plan 5/10 (development: 3/5, 3/0, 5/0).

Measured current handwritten production owners (bytes; physical LOC at base -> current; embedded tests): core/awbc/verify/code.rs 233,964; 5,899 -> 5,996; 0; its assignment child 5,295; new -> 150; 0. Core/value/place_displacement.rs 2,783; new -> 90; 0; place_storage.rs 22,367; 520 -> 638; 112; env.rs 38,624; 941 -> 1,073; 482. Runtime-plan/awbc_lower/expr.rs 87,711; 2,385 -> 2,300; 0; its value_control child 7,137; new -> 183; 0. Flow.rs 149,861; 3,909 -> 3,893; 0; inventory.rs 98,475; 2,457 -> 2,399; 0; pattern.rs 35,312; 812 -> 881; 0. Earlier general input/formal/local-use owner measurements and responsibility review remain applicable (no later edits to those source owners).

The verifier child owns one post-fixed-point assignment admission/sealing rule and borrows the sole FlowState; it has no independent solver or persistent fact index. The storage child owns the shared source/runtime contour grammar; storage validates its own drop flags and inventories its own remaining values. The value-control child owns lexical-frame branching/result joins and borrows the same inventory/frame as other expressions. Inventory owns the only open instruction-range cursor and table publication; flow owns structured suspension/loop/control flags; pattern owns schema-directed guard binding projection. These are cohesive state/API boundaries; none widens APIs merely for file placement, introduces I/O or changes dependency direction. Existing upper-size justification for the full opcode/CFG verifier and structured flow lowerer remains applicable. Tests follow ownership/codec/flow boundaries, not source spelling. No forced decomposition of unchanged unrelated owners is warranted by their existing numerical triggers.

Workspace all-target/all-feature check and Clippy complete with exit 0 (contour-final-workspace-check/clippy receipts). Warnings are retained, including nonblocking argument-count/map-key iteration advisories in the assignment predicate; this is not a warning-free claim. Full workspace tests remain live in session 43358; an observed partial summary is not a completion receipt. The shared semantic/wire/CFG migration requires that recipe to complete before commit/push. Specialized GPU/device/performance, stdio/MCP/capture/readback, production-limit/Tier-2 execution and native public-save acceptance are not newly exercised by this cut; remaining milestone acceptance stays in the active goal.

Required full-recipe failure/correction: contour-workspace-tests (session 43358) exits 1 at runtime-plan awbc_product_parity::product_awbc_standard_map_helpers_match_structured_results. StandardMap was another former control-thunk caller, missing from the new inline dispatcher. The old shared check/Clippy/structure receipts are therefore intermediate, not delivery acceptance. Standard sequence/array/Option/Result map lowering now produces a result register in the same frame; sequence maps use a real temporary rather than an implicit initialized RuntimeState slot, variant results join instead of returning from the enclosing function. Operand evaluation order is preserved. One map operation owns a temporary lexical scope: its result moves to the parent register before the scope closes, and unused callback/source owners are cleaned at the operation boundary. No synthetic control callable/state or fallback is restored. Existing runtime-plan awbc_product_parity passes 5/5 after this repair (map-control-parity-tests receipt), including the failed standard-map family matrix.

Final shared checks are re-running sequentially in session 75443: just test-workspace, workspace all-target/all-feature check, workspace all-target/all-feature Clippy; actual returned session must be reconciled with map-final-workspace-{tests,check,clippy} logs/exit files. Source is frozen after the standard-map repair; docs-only evidence updates do not change those source bytes. Refresh only the changed map-owner measurements and final structural receipt after this repair. Do not publish until the required full recipe completes successfully. Astra remains completed; do not repeat.

Map-final structure refresh completes with exit 0: aggregate counts and dependency/package hashes are unchanged from the preceding contour structural receipt. The map repair moves 31 physical LOC from the expression parent to its value-control child: expr.rs is now 87,247 bytes, 2,385 -> 2,269 physical LOC; value_control.rs is 8,283 bytes, new -> 214 physical LOC; both have 0 embedded test LOC. The same expression-family/control-result ownership disposition applies. Root schema, feature set, package/dependency graph and other owner measurements are unchanged. The final code freeze is the staged patch plus these two inspected map-repair hunks; they will be explicitly restaged before delivery.

Delivery acceptance on final frozen source: sequential session 75443 completes with exit 0. just test-workspace passes 308 suites, 7,252 tests passed, 0 failed, 24 ignored; core library 802/802, sema library 1,166/1,166, compiler callable integration 136/136 are included. Workspace check and Clippy with --all-targets --all-features both exit 0 after the map repair. Final structural gate exits 0 with the recorded map-final measurements; format, working and cached diff checks pass. Receipts are %TEMP%/arcweft-1002-map-final-workspace-{tests,check,clippy}.log/.exit and map-final-structure.log/.exit plus its CSVs. Earlier required-recipe failure is repaired, not ignored or treated as a pass. Warnings/24 ignored tests remain explicit; no new specialized-device/performance/Tier-2/native-public-save claim is made.

Reviewed/staged 59 explicit paths at main parent 15d7c14dff115634baca21d3a092ca3706d8aef2, including final map-repair hunks and maintained contract/evidence updates. No unstaged/unrelated WIP or Cargo/dependency change is observed. Remote refs/heads/main is rechecked at the same full parent before publishing a normal fast-forward. This coherent cut delivers general execution body/input evidence and static post-RHS assignment/guard/lexical-control ownership consumption. It does not complete the full convergence objective: general runtime program binding/owned-runner migration, nonstructural nominal leaves/C1-C6 acceptance, retained UI/live hotpatch/cache/frame/font work and remaining runtime-plan/scheduler milestone checks stay active. Resume these dependencies from the actual delivered HEAD without redefining or completing the goal. Sol choices remain gpt-6.1-sol. Astra one-shot Rust move consultation was already completed and MUST NOT be repeated after compression or delivery.

### 2026-10-02 deterministic extraction and borrowed ingress progress

Continuation base is main 88304db26bd4de6f3cf4a0cf19e635393da852b9, also the observed remote main at the start. The inherited View prose correction was the only initial dirty path. Current source adds extraction admission to the existing final execution/input authority and requires it at the compiler/runtime-plan View program bridge. The full convergence goal remains active; no one-shot Astra consultation is repeated.

CheckedClosedExecutionContext now issues CheckedDeterministicProgram from its selected value/body ABI. It requires empty actual effects and non-suspending execution, rejects writes to root-external bindings, and checks Return, Output, Loop and Try targets relative to each executing frame. Pure project calls and internal local mutation/control remain legal even when the existing emission classification requires Flow. Defer contributes its previously folded effects/suspension; independent cleanup control and external place accesses are checked through its checked body dependencies, without making cleanup reads eager creation inputs. Temporary sets contain only selected membership/visited evidence and are discarded after admission; no independent semantic index, HIR walker or interpreter is introduced. The ABI stays bound to the same registration/closed instance, while runtime publication authenticates its accepted HIR lease. The current View fact still has its closure/capture/parameter projection; this cut does not replace it with the complete general runtime-program root/input model.

The borrowed core program adapter validates arity and all selected input types, then checks the complete transitive value graph for Copy before cloning any argument or invoking the backend. The same private type-validation rule is shared with owned helper binding construction. A regression places Need inside a plain opaque tuple: rejection retains the caller's value and does not call the backend; the corresponding unrestricted opaque tuple executes. This does not establish a general owned native/AWBC program runner or its cleanup/custody/error-return semantics. No language String Copy policy or ownership syntax is changed.

The View maintained chapter now follows the current Rust-compatible contract: whole-local assignment reinitializes the same declaration after Move, partial field assignment can restore a moved child, and a wholly uninitialized owner cannot be partially updated. Post-RHS cleanup and borrower protection remain required. Initial compile used a nonexistent analysis-view generation accessor and was corrected to the existing accepted-HIR lease API. Early control/assignment/defer regression spellings failed HIR admission or operand typing; fixtures now exercise admitted Flow control, value-block assignment and conditional cleanup Return without weakening production or rejection assertions. The final narrow extraction/input run passed 12/12, and borrowed core program tests passed 3/3 before the final shared check. Source after the membership-query/capture-set adjustment is frozen for the shared validation run; its exact receipts supersede these intermediate passes.

Ownership review: the new 198-LOC deterministic_program.rs owns extraction policy over the same context and typed catalogs. The input ABI owns generation validation and selected membership queries; the private callable lease owns its HIR admission. Runtime semantic facts own atomic program publication, and compiler projection owns producer/consumer composition. The core helper adapter owns ingress validation and delegates to the existing value ownership authority. No dependency/features/Cargo changes, I/O or authority reconstruction are introduced. The existing cohesive large-owner dispositions for compiler/lower.rs (404413 bytes, 9512 -> 9524 physical LOC, production), core/pure.rs (129666 bytes, 3420 -> 3434 physical LOC, production with existing tests), callable/checked_catalog.rs (107043 bytes, 2900 -> 2912 physical LOC, production with existing tests), and runtime-plan/semantic_facts.rs (518637 bytes, 13339 -> 13376 physical LOC, production with existing tests) remain applicable: their added clauses belong to their existing projection/evaluation/admission responsibilities. execution_inputs.rs is 25805 bytes, 593 -> 612 physical LOC; its standalone owner tests are 20057 bytes, 355 -> 550 physical LOC. The new proof module is 8306 bytes and has no embedded tests. Structural scanner evidence will provide the final graph/counts; unchanged unrelated numerical triggers do not constitute new blocking defects.

Shared validation is running in sequential session 10034: final workspace check, just test-workspace, then workspace Clippy, with receipts %TEMP%/arcweft-1002-admission-final-{check,tests,clippy}.log/.exit. Check has exited 0 on final frozen Rust source; tests and Clippy are pending. Do not restart a confirmed live session solely after compression. General root/input runtime binding and owned execution, nonstructural nominal leaves, retained UI/live patch/cache/frame/font and runtime-plan/scheduler acceptance remain unresolved. The requested Astra Rust move consultation is ALREADY COMPLETED and MUST NOT be repeated.

Final validation and leaf representation repair: sequential session 10034 completed with exit 0. just test-workspace passed 308 suites, 7258 tests passed, 0 failed, 24 ignored. Workspace check and Clippy with --all-targets --all-features both exited 0. Clippy identified the new admission error's large inline context payload; it is now boxed at the Sema and compiler error boundaries, preserving structured source/display and automatic propagation. The redundant must_use on the new Result-returning fact constructor is removed. This last leaf edit changes error representation and a lint annotation, not extraction/control/input execution behavior or any codec/runtime schema. Reuse the complete workspace behavioral pass; the affected Sema input/admission tests are rerun (12/12, including boxed foreign-context propagation), compiler View product tests are rerun (11/11), and final workspace check/Clippy both exit 0 on the delivered bytes. Their receipts are %TEMP%/arcweft-1002-admission-carrier-{tests,view-tests,check,clippy}.log/.exit, sequential session 16157 completed. No new deterministic_program lint remains; other workspace warnings remain explicit. No new device/GPU/performance/Tier-2/public-save claim is made.

Final structural refresh exits 0: 2690 files scanned, 2560 Rust files, 1476604 physical Rust LOC, 97 packages, 351 review triggers, 0 blocking violations. Dependency CSV SHA-256 is 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42; package metrics CSV SHA-256 is 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7, both unchanged. Receipt: admission-carrier-structure.log/.exit and its report directory in %TEMP%. Updated measurements supersede the preceding interim values: compiler/lower.rs 404462 bytes, 9526 physical LOC, 0 embedded test LOC; deterministic_program.rs 8493 bytes, 204 physical LOC, 0 embedded tests; standalone execution_inputs tests 20335 bytes, 555 physical LOC; runtime-plan/semantic_facts.rs 518621 bytes, 13375 physical LOC, 0 embedded tests. Other touched-owner values and responsibility dispositions are unchanged. core/pure.rs contains 134 embedded test LOC; checked_catalog.rs and compiler/lower.rs have 0 embedded test LOC. No graph/feature/dependency change is present.

The 13 source/test/specification/evidence paths are explicitly staged and reviewed for the coherent extraction/borrowed-ingress cut. Formatting and working diff checks pass; remote main is reobserved at the exact parent 88304db26bd4de6f3cf4a0cf19e635393da852b9 before delivery. Required general runtime-program root/input/owned-runner migration, nominal nonstructural leaves, retained UI/live patch/cache/frame/font and runtime-plan/scheduler convergence acceptance still remain. This cut is progress, not full convergence or full Rust ownership-model completion. Continue from the actually delivered main without replacing the goal by this cut. Sol remains gpt-6.1-sol; the one-shot Astra Rust move consultation is completed and MUST NOT be repeated after compression.

### 2026-10-02 program function-frame migration and nextest selection

Continuation base is main 6ad5f2259963027222840da8bac354443693aaf5, also the observed remote main. The initial dirty state is the in-scope program binding/test-workflow migration; no unrelated WIP, branch, worktree or Cargo dependency change is observed. The full convergence goal remains active. The latest user direction requires cargo nextest and effective reverse-dependency coverage. cargo-nextest 0.9.146 (8af696ddcce8fff2962d6a5168b6d138b8616a35) is installed through Cargo's locked installation; the source-install receipt exits 0. Tests now use nextest; existing historical Cargo receipts remain historical. The one-shot Astra Rust move consultation is ALREADY COMPLETED under /root/rust_move_semantics_once_20261002 and MUST NOT be repeated after compression.

RuntimePureProgramBinding now selects an existing plan-owned FunctionSite; AwbcPureProgramBinding selects its ordinary AWBC Function. The seed issuer, semantic signature, effect-free admission, wire codec, product verifier, field-default request, Standard View handler, bundle cross-section join and driver accepted handler all consume that same reference. Program wrappers no longer publish synthetic PureHelper rows. Genuine Fx/scalar/AOT helpers retain their separate actual use. Function-site capture/parameter pattern binding and the existing native evaluator/AWBC VM remain the execution owners. Borrowed input type and deep Copy validation precede all cloning or execution. Input source order is already checked by validate_function_input_bindings: contiguous captures precede contiguous parameters. No parallel interpreter, fake Flow, version increment or legacy reader is added.

The first targeted nextest run failed four tests: two Rust-default producers and one View handler could not resolve a program-only function site to AWBC; the field-default rejection fixture still tampered with the obsolete helper table. Program sites now join the same pending function-site queue as callable-state sites, so program-only bodies are reserved and emitted through the existing emitter. The fixture now rejects missing function rows and wrong function kinds, and checks absent/invalid/nonempty effect sets. No assertion was weakened. The repaired targeted run passed 61/61, with 1143 tests skipped by its explicit filter; receipt program-sites-nextest-repair.log/.exit exits 0. A final extra nonempty-effect fixture case and unused-import cleanup are included in the subsequent reverse-dependency run/check source.

just test-workspace now uses nextest over its existing library/integration/CLI surface. just test-affected crate applies rdeps(=crate) to the same surface, with empty branch selection allowed and never counted as behavioral evidence. The test execution policy requires recording actual selected counts/packages, graph-external semantic owner tests where needed, and Cargo's supported doctest runner for doctests only. Current shared validation is sequential session 97917: just test-affected arcweft-core, workspace all-target/all-feature check, workspace all-target/all-feature Clippy. Receipts use %TEMP%/arcweft-1002-program-sites-{rdeps,final-check,final-clippy}.log/.exit. At this note, reverse-dependency building is running; do not represent it or later queued checks as a pass, and do not restart solely after compaction. Cargo metadata identifies 50 workspace packages in the candidate reverse closure; the executed nextest selection will establish actual test coverage.

Structural gate exits 0: 2690 scanned files, 2560 Rust files, 1476620 Rust physical LOC, 97 packages, 351 review triggers, 0 blocking violations. Dependency CSV SHA256 remains 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42; package metrics SHA256 remains 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7. Exact owner bytes/LOC/classification/embedded-test values are in program-sites-structure/file_metrics.csv. Existing large owners remain cohesive: core plan/schema/codec own the reference contract, construction and entry_inventory own issuance/admission, AWBC structure owns validation, product_step owns VM dispatch, runtime-plan final_flow and awbc_lower own producer projection, and bundle/driver own cross-section acceptance and dispatch. No new state cluster, copied semantic authority, dependency direction, I/O ownership or API widening for file splitting is introduced. Largest touched-owner line changes are final_flow 8665 -> 8677, construction 3372 -> 3380, verifier structure 3523 -> 3533; entry_inventory decreases 1559 -> 1549. Other touched large-owner responsibility/graph dispositions remain applicable. The scanner predates the final unused-import-only inventory cleanup; its behavior/dependency evidence is reused, with final leaf measurements to be recorded if changed.

This is a complete migration of the runtime program reference, not completion of general program admission or owned execution. RuntimePureProgramFact still represents closure/CaptureId/u16 inputs, and the live producer emits expression bodies. The native borrowed site evaluator still rejects executable function bodies. General selected-root/formal/free-input publication and actual owned native/AWBC runner custody/cleanup remain required, followed by nominal nonstructural leaves, retained UI/live patch/cache/frame/font and runtime-plan/scheduler acceptance. No full Rust ownership parity, device/GPU/performance, Tier 2 or native public-save completion is claimed. Continue these accepted dependencies from the delivered main without replacing or completing the goal.
Nextest runner-resource adjustment: the first reverse-dependency branch completes 4553/4553 tests across 184 binaries, 16 skipped, with 24 slow tests. trybuild compile-fail owners share target/tests/trybuild, and their cross-process fixture/Cargo locks caused substantial waiting under nextest's default process concurrency. The local trybuild 1.0.116 run/cargo implementation confirms both the per-owner .lock and common target path. .config/nextest.toml now assigns api_compile/public_api/compile_fail/derive_trybuild binaries and the two HIR project_symbols trybuild tests to the cargo-fixtures group (max-threads = 1). Ordinary tests retain default concurrency. This configuration is added after the first branch's scheduling began; that pass is not represented as exercising the new scheduling override. Subsequent CLI phases read the final config, and actual grouped owner membership/execution will be verified separately. Official configuration authority is https://nexte.st/docs/configuration/test-groups/. No broad test suppression, timeout increase, changed assertions, Cargo jobs/env override or retries are added. Session 97917 remains live for CLI phases and queued final check/Clippy; inspect its real results on continuation.
Final acceptance for this coherent reference/test-workflow cut: sequential session 97917 completes exit 0. The three rdeps(=arcweft-core) nextest branches pass 4553 + 169 + 24 = 4746 tests, 0 failed. The first branch records 16 skipped tests and 116 excluded binaries; later CLI branches record 0 skipped. Tests execute in 47 packages from the 50-package candidate closure; arcweft, arcweft-bundle-assets and arcweft-render-web have no executed test cases in this matrix. Full workspace all-target/all-feature check and Clippy both exit 0; warnings remain. The original four targeted failures are repaired, not ignored. Final grouped-owner verification uses cargo nextest run -p arcweft-core -p arcweft-lang-sema --test api_compile -E 'group(=cargo-fixtures)' and passes 15/15, 0 skipped, 31.447 seconds, session 18870 exit 0. It exercises actual configured group membership and serialized owner execution rather than weakening/removing compile-fail cases. Receipts are program-sites-{rdeps,final-check,final-clippy,groups}.log/.exit in %TEMP%. Borrowing, Move reinitialization, moved-field restoration and native/AWBC program parity owner regressions are included in the passed reverse closure. Full Rust ownership parity and the complete convergence objective remain unachieved.

Final structural leaf reconciliation: awbc_lower/inventory.rs is 98425 bytes, 2398 physical LOC, 0 embedded test LOC after unused-import-only cleanup. The scanner's global LOC and dependency hashes remain valid; the cleanup changes no behavior, ownership, dependency or physical LOC. The added nextest configuration changes test scheduling only. Reuse the complete reverse-closure behavioral pass after that import-only edit, with final workspace check/Clippy and grouped owner execution on final source. No new device/GPU/performance/Tier-2/public-save claim is made. Reviewed explicit staging is limited to the 28 source/test/configuration/policy/evidence paths for this goal, at parent 6ad5f2259963027222840da8bac354443693aaf5. Format/diff/staged review and remote-main observation must precede the normal fast-forward push. The one-shot Astra consultation remains completed; never repeat it. Continue the full accepted remaining convergence dependencies from the delivered main.

Executed package set: arcweft-adapter-desktop, arcweft-adapter-sema, arcweft-agent-mcp, arcweft-agent-mcp-client, arcweft-agent-policy, arcweft-agent-protocol, arcweft-agent-repl, arcweft-agent-runner, arcweft-browser-bench, arcweft-bundle, arcweft-cli, arcweft-compiler, arcweft-core, arcweft-debug-model, arcweft-debug-sqlite, arcweft-dialogue, arcweft-glyphon, arcweft-host-adapter, arcweft-lang-jit-cranelift, arcweft-lang-sema, arcweft-launch, arcweft-lsp, arcweft-player-native, arcweft-player-scene, arcweft-player-web, arcweft-project, arcweft-project-loader, arcweft-rag, arcweft-render-text, arcweft-render-wgpu, arcweft-resource-manifest, arcweft-resource-model, arcweft-runtime-accelerator, arcweft-runtime-codegen, arcweft-runtime-driver, arcweft-runtime-host, arcweft-runtime-plan, arcweft-runtime-scheduler, arcweft-save, arcweft-test, arcweft-text-layout, arcweft-text-model, arcweft-tooling, arcweft-verify, arcweft-verify-lsp, arcweft-verify-oxiz, arcweft-verify-z3.

### 2026-10-03 — admitted program roots and complete input ABI (validated substrate cut)

Observed cut base: main 18889d4140de84bbebf398c3daa47d89c59c9407, initially clean. Current dirty paths belong to the admitted-program producer/consumer migration. The full convergence goal remains active; the earlier acceptance conditions are retained. The single context-free Astra consultation rust_move_semantics_once_20261002 is already completed and recorded at 4b076ec754b45a86a28a6fa185131ccd9ef49136. Never repeat that consultation, including after compaction.

Supersedes the current implementation limitation that RuntimePureProgramFact is a closure/CaptureId/u16 model. Its final input is now the same sealed CheckedDeterministicProgram admission issued in sema, canonical free inputs followed by every full formal parameter, normalized result and selected body kind. HIR reachability distinguishes Value creation, CallableBody invocation and declaration identity plus body role. Declaration projections include formal patterns and attached-content bindings while excluding default-expression evaluation. The ABI retains its accepted topology Arc. Compiler View handlers issue the admission once and publish captures in its canonical input order; the general compiler projection no longer accepts the private View-handler schema. Replaced closure-specific variants and readers are removed in place, with digest domains/version markers kept at 1.

Runtime-plan reservation/definition uses existing ordinary function-site inputs, pattern lowering, scoped semantic catalogs and expression/executable lowering. Closed program proofs select an exact matching instance identity and executable partition; they do not borrow a different instance's or global catalog. Ordinary expression-compatible declaration layouts use the existing function-block lowerer. Executable layouts use the existing Flow lowering continuation and real body children, without a synthetic Flow declaration or another interpreter. AWBC library lowering can verify admitted program-only artifacts without requiring an unrelated entry table; selected-entry products retain their entry requirement and all other verifier checks.

Interim evidence, superseded by the final receipts below: 4 compiler program tests pass through nextest, including native/AWBC execution of unused scalar formals, destructuring tuple formals, and two actually emitted generic instances (i64/u64), plus creation/invocation ABI separation. The capture-order regression passes 1/1 with mixed DialogueView/String inputs whose first-use order differs from declaration order. Earlier build failures and the native classification/AWBC entry-policy failures were diagnosed and repaired, not skipped. The implicit-body/synthetic-ingress regression and final transitive closure are still running; do not treat them as passed until their logs are inspected. An initially empty HIR exact-name filter was corrected to project::tests and executed 1/1; the empty selection is not evidence.

Resolved cargo metadata and cargo tree --workspace --invert arcweft-lang-hir select 23 workspace package candidates. Use nextest list before the affected run, retain the existing workspace/CLI matrix and the configured serialized cargo-fixtures group, and report the actual executed package/test set. No Cargo jobs/env override, retries, weakened assertions or blanket test exclusions are added.

Remaining acceptance: this substrate does not yet emit every general View expression/default producer or register a closed program-only generic root that has no ordinary emitted instance. The closed-instance test uses genuine ordinary calls to emit both instances and proves exact scope/frame selection, not program-only instance discovery. Retaining/registering the admitted closed environment as a real graph root remains required; do not select an arbitrary call site or reconstruct substitution from source/signature. The owned native/AWBC runner for executable bodies and affine ingress/custody/cleanup remains required; borrowed adapters still reject executable control transfer. General callable creation outside ordinary reachability, nominal nonstructural leaves, retained UI/default/Need/stable identity/live patch/frame/cache/font/resource acceptance, and runtime-plan/scheduler/restore acceptance remain open. No full Rust ownership parity or device/GPU/performance/Tier-2/public-save completion is claimed.

Final cut evidence: source was frozen before the complete repeat. just test-affected arcweft-lang-hir passes all three nextest branches: 3681 + 169 + 24 = 3874 tests, 0 failed. The first branch executes across 99 binaries, with 8 skipped tests and 201 excluded binaries; CLI branches skip 0. All 23 resolved workspace candidates have executed passing tests. The initial broad run is NOT a pass: it stopped at 3674 passed / 1 failed / 5 not run because editing while a trybuild fixture was compiling mixed old sema metadata with the newly edited compiler. This was an agent sequencing error; assertions and fixture expectations were not changed. The frozen repeat includes the repaired LSP compile-fail owner and every remaining case. Receipts: arcweft-1003-program-fixed-list.json/.log and arcweft-1003-program-fixed-rdeps.log/.exit in %TEMP%. The final semantic-seal owner bundle passes 81/81, and the frozen reverse closure includes all six compiler program cases, the View capture-order regression, and the HIR explicit-body-role rejection. Implicit ingress uses an explicitly empty effect row, consistent with existing compiler callable fixtures; the omitted annotation row observed during fixture development was unresolved and is not claimed repaired here.

Both cargo check --workspace --all-targets --all-features and cargo clippy --workspace --all-targets --all-features exit 0. Warnings remain, including review-size/large-error warnings in maintained owners; no zero-warning claim is made. Final import-only/semicolon cleanup uses explicit production imports and adds no new dependency or behavior. Its initially omitted test import is repaired. Nextest list selects the exact 7 HIR/compiler owners before execution; all 7/7 pass. The complete workspace check and Clippy repeat on these final bytes and exit 0. Reuse the 3874-test behavioral pass after this non-behavioral cleanup, rather than labeling it a second full run. Receipts: program-import-owner-{list,final}.log/.exit, program-import-check.log/.exit, program-import-clippy.log/.exit.

Structural gate exits 0 on the final behavioral source: 2691 files, 2561 Rust files, 1477893 Rust physical LOC, 97 workspace packages, 351 review triggers, 0 blocking violations. The final explicit-import cleanup adds 16 physical LOC only; final Rust total is 1477909 and programs.rs is 483 physical LOC. Final leaf metrics and graph hashes are recorded below. Graph/ownership findings are unchanged by imports and semicolons. Responsibility disposition: HIR retains the accepted topology and the exhaustive root grammar; sema owns admission and input/transfer certificates; runtime-plan owns signature authentication, exact executable-catalog selection, ordinary function-site reservation and body lowering; compiler owns the registered-world normalization and View-domain capture/schema join. The new compiler programs leaf contains one projection and its cross-backend/rejection tests. final_flow's 276-LOC growth keeps one lowering algorithm and its existing control continuation, not a second interpreter. semantic_facts retains its existing atomic typed vocabulary/staging/admission responsibility and gains 68 LOC. No I/O, dependency direction change, duplicate normalized authority, source reconstruction, or public visibility widening solely for file splitting is introduced. Upper-size owners remain explicitly justified by these cohesive boundaries; file placement is not a behavior gate.

The current implementation is a completed producer/consumer substrate cut, not completion of every general root or owned execution. Next, retain/register the admitted closed environment and a program-owned executable partition/frame inventory independently of ordinary call reachability, including full group and monomorphic declarations already owned by ordinary instance catalogs. Do not choose the first matching call/instance or use signature-only fallback. Then connect the owned native/AWBC root runner and remaining general View/default producers, followed by the retained UI and scheduler/restore/nominal acceptance already listed. Keep the full goal active and never repeat the completed Astra one-shot.
arcweft-compiler: production workspace fan-in/fan-out 3/24, development 1/9.
arcweft-lang-hir: production workspace fan-in/fan-out 10/3, development 1/0.
arcweft-lang-sema: production workspace fan-in/fan-out 8/14, development 3/0.
arcweft-runtime-plan: production workspace fan-in/fan-out 5/10, development 5/0.
dependency_edges.csv SHA-256: 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42.
package_metrics.csv SHA-256: 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.
crates/arcweft-compiler/src/lower.rs — 404487 bytes, 9531 physical LOC; production owner; embedded test LOC are 434 for programs.rs, 398 for final_flow.rs, and 0 for the other listed leaves.
crates/arcweft-compiler/src/lower/programs.rs — 18001 bytes, 483 physical LOC; production owner; embedded test LOC are 434 for programs.rs, 398 for final_flow.rs, and 0 for the other listed leaves.
crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs — 67899 bytes, 1812 physical LOC; production owner; embedded test LOC are 434 for programs.rs, 398 for final_flow.rs, and 0 for the other listed leaves.
crates/arcweft-runtime-plan/src/final_flow.rs — 374214 bytes, 8953 physical LOC; production owner; embedded test LOC are 434 for programs.rs, 398 for final_flow.rs, and 0 for the other listed leaves.
crates/arcweft-runtime-plan/src/semantic_facts.rs — 522302 bytes, 13443 physical LOC; production owner; embedded test LOC are 434 for programs.rs, 398 for final_flow.rs, and 0 for the other listed leaves.

### 2026-10-03 — owned execution environment and shared local-use authority

Observed base: clean main d66485aad40bd56f062441eadceb431cc210f1ed. The previous goal turn is progress: it changed and validated the admitted root/input producer-consumer boundary. This continuation closes the lifetime of its frozen environment; the full convergence goal remains active. The completed Astra one-shot rust_move_semantics_once_20261002 must never be repeated, including after compaction.

CheckedExecutionEnvironment now owns the authenticated lexical scope, registered callable authority, frozen ProjectFunction or DisplayText solution and the complete selected local-use authority together. Context and ABI share its Arc; ABI no longer copies a separate authority/instance identity. The retained environment can instantiate types and expose its exact transfer/Copy/place certificates after the originating context, instance solution or semantic report is dropped. The admission still authenticates HIR, registered callable authority and instance membership. FinalSemanticAnalysis owns its global catalog in an Arc, and compiler runtime input/global closure catalogs clone that Arc instead of copying the complete certificate maps per root.

CheckedLocalUseAuthority is now sema-owned and used directly by runtime-plan and compiler. The old RuntimeClosedLocalUseCatalog type, implementation and runtime-plan export are deleted; there is no compatibility alias or second reader. Runtime program scope selection checks the complete selected authority in addition to instance identity, so a same-shaped or same-identity catalog cannot silently replace the admission's certificates. This is an immutable semantic environment; it does not clone or retain live RuntimeValue resources.

Behavior evidence: nextest owner selection for execution_inputs plus compiler programs executes 20/20 passing cases. New tests prove generic substitution and exact transfers survive dropping the context/solution, and two admissions share both one context environment and the report's global Arc. The DisplayText environment test independently passes 1/1 after dropping the report and retains its exact method/conformance/type. The first owner build caught a test dereferencing a value-returning transfer accessor; that expression is repaired, not skipped. No other assertion or fixture expectation is weakened.

Resolved cargo metadata and cargo tree --workspace --invert arcweft-lang-sema select 22 workspace candidates. Each of the three affected nextest surfaces is listed before execution. With source frozen, just test-affected arcweft-lang-sema passes 2740 + 169 + 24 = 2933 tests across all 22 packages, 0 failed and 0 skipped. The first branch executes 95 binaries and excludes 205 binaries; CLI branches execute 2 and 6 binaries respectively. Complete workspace all-target/all-feature check and Clippy both exit 0; warnings remain. Structural gate exits 0: 2692 files, 2562 Rust files, 1478142 Rust physical LOC, 97 packages, 351 review triggers, 0 blocking violations. Receipts: arcweft-1003-environment-{metadata,list,rdeps,final-check,final-clippy,structure} files in %TEMP%; owner receipts are program-environment-repair-owner and program-display-environment. No Cargo jobs/environment override, retry policy or new exclusion is introduced.

Ownership disposition: the local-use authority implementation moves to its actual sema owner without changing its certificate vocabulary or introducing a dependency edge. Its new leaf delegates access to the two existing sealed catalogs. execution_context remains the one issuer of an authenticated closed environment; execution_inputs consumes it and retains it. runtime-plan validates and consumes the same sema authority, and compiler remains responsible for runtime projection. These changes preserve syntax/HIR/sema/runtime-plan direction and Sans I/O. Source/file placement is a review aid, not acceptance evidence. The prior large-owner cohesion dispositions remain applicable; this cut adds no unrelated state cluster. Source and Cargo graph measurements follow below.

Remaining acceptance is unchanged: compiler still must materialize a program-owned executable catalog/frame from this retained environment independently of ordinary call reachability, including closed program-only generic roots, full group chains and monomorphic declarations already present in ordinary catalogs. Then wire the owned native/AWBC root runner, affine ingress/custody/cleanup and the remaining general View/default producers. Retained UI, nominal nonstructural leaves, scheduler/restore and all prior acceptance remain required. This environment/certificate lifetime is verified; independent program frame emission, complete Rust ownership parity, device/GPU/performance measurements and full convergence are not claimed complete. No external blocker exists.
crates/arcweft-lang-sema/src/final_analysis/execution_context.rs — 10859 bytes, 284 physical LOC, production owner, 0 embedded test LOC.
crates/arcweft-lang-sema/src/final_analysis/local_use/authority.rs — 3700 bytes, 102 physical LOC, production owner, 0 embedded test LOC.
crates/arcweft-lang-sema/src/final_analysis/report.rs — 99263 bytes, 2462 physical LOC, production owner, 0 embedded test LOC.
crates/arcweft-runtime-plan/src/semantic_facts.rs — 522790 bytes, 13452 physical LOC, production owner, 0 embedded test LOC.
crates/arcweft-runtime-plan/src/semantic_facts/project_function.rs — 100326 bytes, 2717 physical LOC, production owner, 0 embedded test LOC.
dependency_edges.csv SHA-256: 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42.
package_metrics.csv SHA-256: 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.

### 2026-10-03 — one sealed executable type/catalog projection

Observed base: clean main bd395bc30c99e29cac0c23852049ff006694da8b; origin/main matches. The previous goal turn is progress: it delivered the owned admission environment and shared sema-owned certificates. This continuation consolidates the complete executable catalog producer before introducing independent program frames. The original goal and all remaining acceptance stay active. The completed rust_move_semantics_once_20261002 Astra consultation must not be repeated after compaction.

RuntimeExecutableInstantiation now projects every expression, pattern, local and authored type from CheckedExecutableRuntimeFactPartition. Ordinary project-function instances, explicit closures (including selected-method closures) and DisplayText methods consume this same operation. Their duplicate HIR owner traversals and separate type-projection loops are deleted. Semantic-only expressions remain explicit rows; closed type substitution still uses the selected instance's original graph work ledger. Parameter source TypeIds retain their checked ABI's inferred callback effect rows rather than replacing them with an authored annotation that may omit an effect clause.

The complete producer is named runtime_executable_semantic_facts. It receives the sealed partition and selected lexical environment and generates its type projection internally, alongside expression/pattern/statement/capture facts and local-use authority. Callers no longer pass a separately constructed complete type catalog. This directly removes the producer boundary where type coverage could drift from the semantic partition. The existing runtime catalog's typed validation and complete family vocabulary remain intact; no parallel program interpreter, fallback resolver, schema/version change or compatibility path is added.

Validation: cargo check -p arcweft-compiler --all-targets exits 0. The initial check found the extracted implementation's missing HIR TypeId import and private child-method visibility; both are repaired before the frozen-source test run. Cargo resolved metadata plus cargo tree --workspace --invert arcweft-compiler selects 7 workspace packages, and the actual nextest PASS receipts confirm all 7 executed. Each affected surface is listed first with rdeps(=arcweft-compiler): 732 library/integration tests across 45 binaries (255 binaries excluded), 169 CLI tests across 2 binaries and 24 CLI integration tests across 6 binaries. just test-affected arcweft-compiler passes all 925, 0 failed and 0 skipped. Existing coverage includes full formal/destructuring program inputs, exact signed/unsigned generic instance selection, ordinary closure input rejection, selected DisplayText methods, and body/nested-closure shared depth bounds. No test assertion, fixture, feature, exclusion, retry policy or Cargo jobs/environment setting is changed.

The compiler-private producer consolidation changes no shared semantic/public/serialized shape or Cargo setting. Validation therefore uses its complete reverse dependency test closure and cargo clippy -p arcweft-compiler --all-targets --all-features, which exits 0; compiler has no package features. cargo fmt --all --check exits 0. Warnings remain, including the existing large projection-error vocabulary at the new method; no zero-warning or fresh whole-workspace check claim is made. Structural gate exits 0: 2693 files, 2563 Rust files, 1478077 Rust physical LOC, 97 packages, 351 review triggers, 0 blocking violations. Receipts are arcweft-1003-partition-{metadata,candidates,list-*,selected-packages,executed-packages,rdeps,check,clippy,structure} in %TEMP%.

Ownership disposition: executable_types is a stateless compiler-owned implementation of the existing lexical projection context, with explicit narrow imports and no I/O or dependency edge. It replaces three implementations of one invariant, and both type and semantic projection now use the sema-sealed owner inventory. Its pub(super) method is private to the existing compiler owner; no facade/public API is widened for file placement. lower.rs retains its existing inversion/payload-factory responsibility and prior cohesion disposition while shrinking from 9532 to 9426 physical LOC. display_text.rs retains conformance selection and method publication while shrinking from 477 to 432 physical LOC. Neither acquires another state cluster. All three changed Rust leaves are production, with 0 embedded test LOC in this extracted projection boundary.

Remaining acceptance: admitted programs still need their own semantic catalog and frame instead of selecting an already-emitted ordinary catalog. In particular, add and resolve an end-to-end case where a monomorphic declaration is also ordinarily invoked (its rows are currently instance-owned), and a closed generic program whose selected instance is only evidenced in an ordinarily unreachable call. Use the unified producer and the admission's retained environment, preserve exact root/body role and full argument-group coordinates, and extend real lexical/frame identities for programs and nested callables. Then finish the owned native/AWBC runner, static initialization/loan/cleanup and custody boundaries, general retained View/default producers, nominal leaves and scheduler/restore acceptance. The type/catalog producer is consolidated and validated; independent program catalog/frame emission and the full convergence goal are not complete. There is no external blocker.

crates/arcweft-compiler/src/lower.rs — 399001 bytes, 9426 physical LOC, production owner; no embedded tests in the changed boundary.
crates/arcweft-compiler/src/lower/display_text.rs — 16410 bytes, 432 physical LOC, production owner, 0 embedded test LOC.
crates/arcweft-compiler/src/lower/executable_types.rs — 3959 bytes, 86 physical LOC, production owner, 0 embedded test LOC.
dependency_edges.csv SHA-256: 7ED2CD7BDA7DC6A879DD5684072D06357883E1B185C6A00343A7D7C27CB7AC42.
package_metrics.csv SHA-256: 6242C34313F5C79FA7B8ADC1EE1732C9319F02BDDEBD5FB78054B0A8B6A0DFE7.

### 2026-10-03 — WIP: independent admitted program catalogs and frames

Observed base: clean main a167ccea8211016eb236cf31f29019e5de2b2949, inspected before editing. Previous goal turn is progress: the complete executable type/catalog producer was unified and delivered. Current working tree has an in-scope producer/consumer migration in compiler, sema and runtime-plan; it is not staged, committed or published. The full goal remains active. The completed Astra consultation rust_move_semantics_once_20261002 must never be repeated, including after compaction.

Program scope no longer searches ordinary function/closure/method catalogs. RuntimePlanSemanticFactInput stages one exact executable catalog per RuntimePureProgramId; RuntimePlanSemanticFacts authenticates its root, reachability and complete local-use authority against the retained admission. Program catalogs join complete executable visitors, including nested closures, implicit callables, normalized type dependencies and checked callee-instance references. Scope, closure lexical owner, formatter scope, method use and frame maps have explicit Program identities. Each program gets its own HIR locals, source-operand ANF locals and ControlLocals before body lowering. The complete catalog producer remains shared with ordinary functions and methods.

Sema issues runtime_program_fact_partition from the admitted ABI and exact accepted HIR root. It records input_locals separately from declarations, preserving free-binding identity without redeclaring the parent local. Type projection and runtime catalog validation consume that sealed inventory. CheckedExecutionEnvironment validates nested callable owners against its exact HIR allocation, callable generation/catalog/standard and lexical declaration/item. The catalog validator carries Callable or Program lexical authority rather than assigning a fake callable identity to a program. Ordinary global rows shared with a program expression are retained; independent program rows do not steal the ordinary parent's locals or expression metadata.

The same ProjectInstantiationSession retains admitted environment Arcs and charges registration against its work ledger. Program bodies discover selected callee and specialization dependencies before graph sealing, using their frozen enclosing function solution. ProjectInstanceNode selects the exact definition partition from admitted ordinary/program root sets, rejects conflicting inventories when both contain it, and rejects absent definitions. Discovery, dialogue-owner collection and callee materialization use this membership. Runtime validation authenticates a partition against either admitted reachability generation and counts checked calls in every executable catalog, so program-only callees are not incorrectly rejected as unreferenced.

Evidence so far: the initial new monomorphic dual-use regression fails with a missing accepted-local seed at the old global scope. After independent frame migration, the program owner selection passes 8/8 cases through native/AWBC, including new ordinarily-unreachable i64/u64 identity instances. New tests then prove captured-body ingress uses supplied base=41 rather than the ordinary Flow's base=1, and inspect actual ordinary reachability before checking generic roots. The 10-test owner run has 9 passing cases and one failing transitive-call case. Its earlier failures Facts(InvalidProjectFunctionInstance) and Facts(UnreferencedProjectFunctionInstance) are repaired by authenticating both admitted root generations and visiting program callee references. The latest focused transitive run now reaches native execution and fails UnsupportedPure: an executable runtime function requires function-call control transfer. This is a remaining runner boundary, not a test to weaken or skip. Receipts are arcweft-1003-program-catalog-{regression,owner,transitive,check} logs/list/exit files in %TEMP%. Several check iterations pass cargo check -p arcweft-compiler --all-targets; no broad reverse-dependency, workspace lint/check or structural acceptance is claimed for this WIP.

Next concrete action: connect admitted program execution to real native/AWBC function machinery with owned inputs, caller-value retention on rejected entry, real return continuation, custody/cleanup and snapshot/rollback state. Keep the transitive test's expected execution result. Native pure/callable.rs rejects Executable function-site bodies; pure/program.rs uses that borrowed evaluator. Engine already has new_with_shared_plan and start_function_site_call with FunctionCallFrame/FunctionReturnContinuation, while AWBC product_step.rs uses run_function for borrowed programs and entry-selected constructors for product fibers. Add a real program execution root; do not synthesize a Flow or add a second body interpreter. Compiler must select executable lowering when projected calls require it even if admitted semantic control is expression-compatible.

Before delivery, finish program-scoped formatter/Content projection and DisplayText consumers (scope variants exist, but compiler template/dialogue/method collectors still need program roots), full argument-group behavior and meaningful nested-closure execution/rejection tests. Review the complete diff, then freeze Rust before nextest and trybuild. Validate sema's complete reverse dependency closure with cargo metadata and cargo tree --invert, list each nextest surface first, repair all affected failures, complete all-feature workspace check/Clippy, fmt and structural/ownership gates. No source edits during nextest. Only publish a coherent verified result. Owned root execution, complete Rust ownership parity, general View/default producers, nonstructural nominal leaves, retained UI and scheduler/restore acceptance remain incomplete. No external blocker exists.

### 2026-10-03 — WIP continuation: native program frames and scoped formatting

Observed main and origin/main remain a167ccea8211016eb236cf31f29019e5de2b2949. The in-scope migration remains unstaged and unpublished. The convergence goal is active. The completed rust_move_semantics_once_20261002 consultation must not be repeated.

Native program activation now preflights complete FunctionSite inputs and patterns while retaining rejected owned inputs, then enters a real FunctionCallFrame with a Program return continuation. Expression bodies also enter this frame without invoking a backend during activation. The ordinary budgeted engine dispatcher executes pending program operations without inventing a Flow. A completed result is moved out once through take_program_result; inert rollback images encode it through the program-owned value snapshot authority. NativeProgramResult is a real owned-slot domain, included in parent-fiber line-handle reconciliation and both ownership codecs. This prevents a value moved from its local into the result owner being mistaken for a discarded value. The borrowed program adapter checks transitively unrestricted inputs before copying them into this same frame path.

Program reservation selects executable lowering when selected calls need function control transfer. Program-only generic calls now execute their transitive instances through native and AWBC with the expected result 42. Program roots also participate in compiler formatter-template, DisplayText method-use and dialogue/Content collection, with their retained closed environments. Multiple argument groups and nested closures use the program frame. The complete executable catalog producer remains shared; no additional body interpreter or fake callable/Flow identity is introduced.

Actual nextest evidence: program/owned-input/rollback/ownership-codec selection passes 21/21. The earlier formatter regression failed because its test plan was not artifact-bound; the fixture now uses the existing explicit artifact binding before Content construction, and the execution assertion is preserved. A further program-only custom DisplayText test uncovered an AWBC producer/verifier mismatch: method frames were concretely typed while signatures were Dynamic. Trait method signatures now use their actual input locals and result type, and the unused Dynamic-signature helper is deleted. The verifier and fixtures use the emitted standard nominal identities standard::DisplayContext and standard::DisplayError. The initial std.DisplayContext guess was incorrect; the final identities are taken from AcceptedNominalOwnerId::source_label and AcceptedNominalId::source_label. The custom program plus existing AWBC formatter/restore/rejection selection passes 15/15. Prior failures remain in the %TEMP% receipts; no assertion, ignore, retry, feature or budget override was used to conceal them. Current logs are arcweft-1003-program-{repair,display-final} with matching list logs. Earlier intermediate return/integration/content/display logs describe their own source bytes and are not final broad acceptance.

Resolved default and all-feature Cargo metadata plus cargo tree --workspace --invert arcweft-core select the same 50 workspace reverse-dependency candidates for the union of core, sema, runtime-plan and compiler. Rust source is frozen while the workspace library/integration and existing separate CLI surfaces are listed/executed. The full reverse-dependency run, all-feature workspace check/Clippy and final complete diff review remain pending at this note. cargo fmt --all --check passes. Structural gate passes: 2694 scanned files, 2564 Rust files, 1479324 Rust physical LOC, 97 packages, 351 review triggers and 0 blocking violations. Receipts: arcweft-1003-program-{metadata,allfeatures-metadata,rdeps-packages,allfeatures-rdeps-packages,rdeps-list,cli-list,structure} in %TEMP%.

Ownership disposition: engine/program is an implementation of the existing Engine state owner and FunctionSite ABI, with no I/O, dependency edge or independent interpreter. Engine owns the live result; its rollback type owns only inert snapshots, and FlowFiberStatus retains its existing non-owning terminal label. The owned-slot and codec additions belong to the existing ownership identity vocabulary. Sema remains the sole issuer of program partitions and local-use authority. Runtime-plan authenticates complete catalogs and issues independent frame coordinates; compiler projects the retained environment and uses the shared catalog factory. Scoped formatting uses the same existing template allocator and method publisher. The previous large-owner cohesion dispositions remain applicable; this migration does not split those owners through widened facade APIs or add unrelated state clusters.

Remaining acceptance: complete the owned AWBC program-root identity/activation and custody boundary, persistent save/restore of native program state/results, live line-handle ingress/return/export and cleanup ownership, program Content/DisplayText cases beyond the current formatter test, and the remaining general View/default, nonstructural nominal and scheduler/restore work. The synchronous borrowed adapters still have their explicit bounded execution surface; this work does not claim complete Rust ownership parity, live host-resource custody, durable program save support, performance measurements or full convergence. No external blocker exists, and this note does not authorize publishing unvalidated WIP or mark the goal complete.

Broad validation progress: the first source-frozen library/integration surface lists 4573 tests in 172 test binaries from 46 packages. The 50 graph candidates additionally contain the separately tested CLI and three packages without selected test cases (arcweft, arcweft-bundle-assets and arcweft-render-web); they remain covered by workspace compile/lint checks. The first run ends with 4401 passed, 1 failed, 16 skipped and 171 not run after fail-fast. The failing runtime-plan awbc_product_parity test observes a real migration regression: the new native frame path returns a Dense Seq for standard map while the pure/AWBC path and existing expected value use Values. Engine's standard map incorrectly used the literal-output constructor. It now uses the same mapped-values constructor as pure execution; the unchanged failing test passes 1/1 after the repair. Initial failure receipt is program-rdeps.log; repaired receipt is program-map-repair.log with a fresh list. Source is again frozen for a fresh complete reverse-dependency list/run (program-rdeps-final-{list,run} receipts, actual run filename program-rdeps-final.log), workspace all-feature final check and Clippy. The earlier all-feature workspace check passed before the map repair and is not relabeled as validation of the repaired bytes. CLI library/bin list selects 169 tests; the existing six integration binaries are listed separately. Complete broad acceptance remains pending until these actual commands finish.

Final validation receipts for this WIP: the fresh reverse-dependency surface passes 4573/4573, 0 failed, with 16 existing skipped cases and 3 slow compile-fail owners. It takes 519.772 seconds, including serialized trybuild owners; Rust is not edited during the run. The two separately listed CLI surfaces pass 169/169 and 24/24, each with 0 skipped. Total executed tests are 4766, all passing. Final workspace all-target/all-feature check and Clippy both exit 0 after the map repair; warnings remain. Final fmt and git diff --check exit 0. The structural gate receipt is reused for the unchanged ownership/dependency boundary; the small map-constructor repair adds no state, API, dependency or new owner. Logs are program-rdeps-final.log, program-cli.log, program-cli-integration.log, program-workspace-final-check.log and program-workspace-clippy.log, with their fresh list receipts in %TEMP%. The earlier broad failure is retained as a failure, not relabeled as success. No jobs/environment override, default-filter bypass, new test exclusion, assertion weakening or retry setting is introduced.

Current source measurements (physical lines including blank lines): compiler lower/programs.rs is 25451 bytes / 680 LOC (shared producer and its embedded program execution tests); core engine/program.rs is 6088 bytes / 153 LOC (native activation and result ownership, no embedded tests); runtime-plan final_flow.rs is 380360 bytes / 9098 LOC and semantic_facts.rs is 523656 bytes / 13486 LOC (existing cohesive lowering/validation owners and prior dispositions). Full changed-Rust measurements are retained in program-owner-measurements.json in %TEMP%. Current main/origin remain a167ccea8211016eb236cf31f29019e5de2b2949. Source and operational note are not staged, committed or pushed: the owned root/custody/persistent-state acceptance listed above still requires implementation and complete review. These passes verify the current substrate and affected consumers, not completion of that remaining work or the convergence goal. Next continuation should start with the actual AWBC FiberState root/entry constructors and program snapshot ownership; do not repeat the completed Astra consultation or redo unchanged validation without new evidence.

### 2026-10-03 — WIP continuation: authenticated AWBC program origin and owned restore

Observed main remains a167ccea8211016eb236cf31f29019e5de2b2949 with the prior in-scope migration preserved. This turn continues the actual owned AWBC runner instead of treating the prior broad tests as completion. The Astra one-shot remains completed and must not be repeated.

Supersedes the preceding note's added requirement for public native program/session persistence: that requirement over-expanded the accepted executor contract. docs/02-runtime/runtime-step-and-executors.md exposes persistent snapshots only for tiers with a defined contract; executor.rs currently defines the public session snapshot for AWBC Product and returns UnsupportedTier for native/structured tiers. Earlier accepted goal evidence at lines 3857 and the Rust-move record likewise explicitly distinguishes native rollback from public native session persistence. Preserve native rollback and complete the AWBC persistent boundary; do not invent native public-session save support as an additional goal gate. This correction preserves the original goal and actual supported contracts.

AWBC fibers and inert snapshots now carry AwbcFiberRoot (Entry, Program, internal Function, Empty) instead of a mandatory entry field. Program and function origins authenticate their original root function; restore checks it independently of the current nested-callee cursor. The old entry-0 empty-table validation exception is deleted. Entry/route constructors still authenticate their actual targets, and child constructors retain their actual origin. Foreground runtime-driver restore rejects a non-Entry snapshot before dropping its current owner. Product owned program activation checks all arguments before transfer, retains rejected inputs, executes through the existing budgeted Product VM and retains the returned value in its fiber terminal before line-handle reconciliation. ProgramResult replaces the earlier unreleased NativeProgramResult diagnostic domain so both native and AWBC use one result-storage vocabulary; tag and contract versions remain 1-era shapes without a legacy reader.

An AWBC program with no Entry table now executes and crosses inert JSON save/restore with an affine Need input and return value, taken exactly once. The unchanged standard-map parity case also passes. Initial failures identified entrypoint requirements at activation and Need registry restore; root selection owns that policy, while component restore verifies complete AWBC structure without requiring a foreground Entry. Core/runtime-driver all-target checks pass before the final test additions. The first test compile lacked runtime-plan's serde_json dev dependency; it now inherits the existing workspace dependency for actual snapshot codec coverage. Owner receipts are awbc-program-restore-{list,run} (actual run filename awbc-program-restore.log) in %TEMP%, with 2/2 passing. A forged Entry(0) origin rejection with rollback-to-original-owner is added and awaits the wider owner run. The initial wider list command duplicated --lib and failed without executing; corrected selection is running. No Rust source is edited while nextest/trybuild runs.

Required next: complete the running AWBC/program owner validation, repair any consumer failures, then broaden through the resolved reverse dependencies of core/runtime-plan/runtime-driver/compiler and the applicable workspace checks. Review Empty-origin terminal invariants and all saved-root consumers. Finish real line-handle ingress/return/export custody and the remaining general View/default, nominal and scheduler/restore acceptance before full goal completion. No commit/push or full new broad-pass claim is made for these new bytes; no external blocker exists.

Continuation evidence: the corrected narrow owner run passes 224/224 tests. Its `test(awbc_product_parity)` term matches test names rather than the integration binary name, so it does not establish the newly added forged-origin integration case; that case remains selected by the forthcoming complete reverse-dependency run. This corrects the overly broad commentary claim about that rejection. Empty-origin validation now admits only the inert terminal shape with no frames, streams, suspension, result value or executable cursor; it does not validate a fictitious function-zero cursor. The new empty restore/rejected-value regression passes 1/1. Its first list build identified a test using the in-memory snapshot instead of the inert save projection; the test now uses the actual rollback image's save projection.

Fresh locked Cargo metadata and `cargo tree --workspace --invert arcweft-core --edges normal,build,dev` select the union of core, sema, runtime-plan, compiler and runtime-driver consumers. Default and all-feature resolved closures both contain the same 50 workspace packages; the new serde_json dev edge does not change that workspace closure. Receipts are `arcweft-1003-root-{metadata,allfeatures-metadata,rdeps-packages,allfeatures-rdeps-packages,core-tree}` in `%TEMP%`. The complete source-frozen nextest list is building. Review additionally identified that retained Program return values need their exact root signature check, and a valueless return must retain a matching Unit status label; these fixes and meaningful restore rejection tests are next after the list terminates. The current bytes are uncommitted WIP on the observed base, and the goal remains active.

The first complete list finishes with 4575 selected names from 46 packages and 172 binaries. Source was then unfrozen for the actual restore repairs: the saved root must equal the selected invocation even if a different root variant names the same function, and a retained Program return must match its exact signature result type. Supersedes the preceding prospective valueless-return wording: verified program bindings require a typed result, including Unit, so an absent result is rejected; no Unit fallback is synthesized. Independent activation, task, defer and callback children now carry their own exact Function root. Their existing typed child owner retains the parent relationship rather than borrowing the parent's program root for a different function. Prepared input authority exposes its actual function coordinate for that construction.

The fresh repaired owner list explicitly includes both integration regressions by test name. Its run passes 47/47: Need ingress/result survives JSON save/restore, unknown Entry and same-function Function origin substitution are rejected with original-owner rollback, a Bool substituted for the Need result is rejected, Unit survives restore and post-take restore does not recreate a result, and the existing Product dialogue/task/defer/Need/context owner tests remain passing. Receipts: `arcweft-1003-root-repair-{list,run}.log`. Structural gate exits 0 on these source bytes: 2695 files, 2565 Rust files, 1479905 physical Rust LOC, 97 packages, 351 review triggers, 0 blocking violations. The new activation leaves belong to their existing Engine/Product owners; fiber owns origin/frame validation, the shared dialogue registry retains ledger custody, and runtime-driver retains foreground admission. No layer edge or second authority is added; serde_json is dev-only and workspace-inherited. The prior large-owner cohesion dispositions remain applicable.

Source is frozen again for fresh default-feature nextest over the resolved 50-package union: library/integration packages are explicitly selected from metadata with the same five `rdeps` predicates, and CLI retains its existing separate surfaces. Required new integration names and a nonzero list are checked before the run; `--no-tests fail` is used. Workspace all-target/all-feature check and Clippy are queued against the same bytes. Current remote main is re-observed through `git ls-remote` as a167ccea8211016eb236cf31f29019e5de2b2949, matching local main/HEAD. No staging, commit or push has occurred; live line-handle ingress/export custody and the other goal acceptance remain open.

Final evidence for this continuation: the fresh resolved library/integration list selects 4576 runnable cases in 172 binaries from 46 packages. The complete run passes 4576/4576, 3 slow cases, 16 existing ignored cases skipped. Separate CLI library/bin list and run pass 169/169; the maintained six CLI integration binaries list and pass 24/24. The disjoint broad total is 4769 passed, 0 failed, 16 skipped; the overlapping 224 and 47 owner checks are not added again. All 50 graph candidates are covered: 47 packages execute tests including CLI; arcweft, arcweft-bundle-assets and arcweft-render-web have no selected cases and remain included in workspace compilation/lints. Workspace all-target/all-feature check and Clippy both exit 0 with warnings; fmt --all --check and git diff --check also exit 0. No Rust source is edited during list, execution or trybuild. No Cargo jobs override, feature workaround, new test exclusion or retry setting is used. The check/Clippy process was queued against the same frozen source, and all commands are now terminal.

Receipts in `%TEMP%`: `arcweft-1003-root-final-{list,run}.log`, `root-cli-{list,run}.log`, `root-cli-integration-{list,run}.log`, `root-workspace-{check,clippy}.{log,exit}`, and `root-structure.log` (all CLI/check/structure names have the same `arcweft-1003-` prefix). The validated 43 source/Cargo file hashes, including both new leaves, are recorded in `arcweft-1003-root-validated-source-hashes.json`, SHA-256 000C281E9DC2BFFB678A6422EF37C6EAFA7339BA90685034552CC68EE693DEA4. This evidence describes uncommitted source on base a167ccea8211016eb236cf31f29019e5de2b2949, not a new commit or full goal completion.

Next concrete acceptance is live resource custody: Engine/Product standalone program activation currently receives values while initializing an empty dialogue registry, and raw result take moves a value without transferring its published ledger obligations. Work from the actual sole registry owners in `crates/arcweft-core/src/line_task/activation.rs` and `handle.rs`, and the new owned activation/result boundaries. Establish a typed custody-bearing ingress/export path or reject unavailable custody before consuming inputs; do not clone live ledgers/value owners, fabricate handles/IDs, or add a second ownership authority. Cover actual issued line handles, rejection with original owner intact, return/export, cleanup and restore in both executors. Reuse the broad receipts for unchanged bytes and rerun the changed owner/consumer scope when that implementation alters them. General View/default, nonstructural nominal and scheduler/restore acceptance remain active; public native persistence remains outside the accepted contract. The Astra one-shot is complete and must not be repeated. No commit/push is claimed for the still-open custody boundary.

### 2026-10-03 — validated program catalog/root delivery cut

The user explicitly requests more frequent Git commits because the dirty state has accumulated. Supersedes this WIP section's decision to hold the entire validated substrate until the separate live-resource extension is complete. The coherent delivery unit is independent admitted-program catalogs/frames, program-only dependency discovery, exact DisplayText signatures, owned native/AWBC activation and authenticated AWBC origin/result restore. The complete consumer migration and new regressions have passed the 4769-test reverse-dependency selection and the required workspace/format/structure checks above. Before staging, the 43 source/Cargo hashes are compared with the validated receipt; only this operational documentation has advanced. Review includes the two new source leaves and all affected consumers/fixtures. The observed main base and remote are a167ccea8211016eb236cf31f29019e5de2b2949.

This is delivery of the verified catalog/root behavior, not completion of general View, live host-resource custody or the full goal. The live-resource ingress/export extension remains the next implementation step; no claim of working detached line-handle transfer or public native persistence is made. Continue with smaller validated commits for subsequent coherent changes instead of accumulating independent completed work behind the full convergence milestone. No Rust byte has changed since the broad pass, so those receipts are reused for the exact staged source; documentation whitespace and staged inventory are checked separately.

### 2026-10-03 — detached input/result custody and published registry transfer

The preceding validated 45-file delivery is committed and pushed as 2ef48432c76b1436278b245bd471ce0820d20c79; local and remote main match and the worktree was clean before this continuation. That is authoritative progress, not full goal completion. The user asks for frequent coherent commits, so finish and publish this new boundary unit separately after its checks. The Astra one-shot remains completed and must not be repeated.

RuntimeValue's existing exhaustive affine-handle traversal now owns detached-custody validation and a typed token/path error. Both native and Product owned program constructors borrow-check every raw input against that rule before consuming the packet. Result take is fallible in both executors: it retains a resource-bearing value and its ledger, and moves unrestricted/external-Need results only after the same classification succeeds. Existing borrowed adapters and all result-taking tests are migrated directly to the unreleased Result API; there is no alias or old reader. This is an external execution boundary, not a new language restriction on Rust-compatible reinitialization or ordinary in-context ownership transfer.

The shared dialogue registry has a non-Clone published transfer phase containing its existing published entries. Borrowed preflight rejects any active or in-flight entry with the original registry returned; commit moves the complete ledger, revisions, issuance and pending-command history. Each existing executor store receives that same phase while selecting its own frame type; no active frame is cast, cloned or reconstructed from source. The new shared fixture issues a real StageActor handle and publishes it through the actual line lifecycle. Native and AWBC result refusal tests retain a published registry under the exact ProgramResult slot. Native rollback is unchanged by two refused takes; Product inert save/owned restore retains the value and published metadata.

Owner evidence: the initial core check fails because the new diagnostic tried to Display a Debug-only RuntimeValuePath; it now uses the existing typed path's Debug representation. The first test list catches a fixture Vec where a Box slice is required and a mistaken enum use for the existing AwbcFunctionInputOwnership struct; those fixture shapes are corrected. Initial run: 5 passed, 2 failed because an Ordinary entry block must use CallableBoundary; that fixture is repaired. The repaired detached selection passes 7/7. Published-registry fixture integration then catches ProductDialogueStore's wrapper requirement; its owning adapter now receives the same published phase. The current complete owner selection lists and passes 8/8, including issued native/AWBC input rejection, nested token/path diagnostics, published-ledger result refusal and Product save/restore, active/in-flight phase refusal, and existing Need/Bool result semantics. Receipts: `arcweft-1003-custody-repaired-{list,run}.log` in `%TEMP%`; earlier failed receipts are retained and not relabeled as passes. No live RuntimeValue clone is used by the new issued-resource fixtures.

Remaining in this unit: resolved reverse-dependency coverage, workspace check/Clippy and structural/format review, then explicit staging, commit and normal push. Complete execution-context/owned-input packet adoption of the published phase remains required before claiming general live-resource transfer; the raw boundary now refuses missing custody instead of consuming an unowned handle. The convergence goal retains general View/default, nominal and scheduler/restore requirements, with native rollback distinct from unsupported public native persistence. No external blocker exists.

Final validation supersedes the pending checks above: the unchanged resolved core/runtime-plan reverse-dependency closure contains 50 workspace packages under both default and all-feature graphs. Fresh source-frozen nextest list/run passes 4582 library/integration cases, 4 slow and 16 existing ignored cases skipped. Separate CLI surfaces list/pass 169 library/bin and 24 integration cases. Disjoint total: 4775 passed, 0 failed, 16 skipped; the overlapping 8 owner cases are not counted again. The first CLI list incorrectly requested unsupported `libtest` output and exited 2 before selecting tests; `oneline` repairs only that command, and both subsequent lists and runs exit 0. Workspace all-target/all-feature check and Clippy exit 0 with warnings. The newly added published-phase/store adapters have unused-production warnings until the next execution-context adoption; no suppression is added and this component is not claimed to be a working live-resource invocation path. Format, diff whitespace and structure checks pass; structure scans 2696 files, 2566 Rust files and 97 packages, with 351 review triggers and no blocking findings. The additions stay on the existing value/registry/executor owners, with the shared issued-resource test fixture as the only new source leaf.

Receipts in `%TEMP%`: `arcweft-1003-custody-rdeps-{list,run}.log`, `custody-cli-repaired-list.log`, `custody-cli-run.log`, `custody-cli-integration-{list,run}.log`, `custody-workspace-{check,clippy}.{log,exit}` and `custody-structure.log` (all shortened names retain the `arcweft-1003-` prefix). The 16 changed source hashes are recorded in `arcweft-1003-custody-validated-source-hashes.json`, SHA-256 33FBF5C0D59CE9DE8850AD7F1492622905E3EA7A1958B2EAE5E0793CC9BD457D, and match after all runs finish. All validation commands are terminal. Local/remote main were reobserved at 2ef48432c76b1436278b245bd471ce0820d20c79 before staging. Publish this checked boundary separately; the full convergence goal remains active, and the next concrete work is custody-bearing invocation/export with the actual producer/Need context retained, followed by the existing View, nominal and scheduler acceptance. Rust-compatible move/reinitialization and borrow-checking direction remains authoritative. No further Astra consultation is authorized for the completed one-shot, including after compaction.

### 2026-10-03 — program continuation retains complete execution custody

Observed clean base: main f31ef0938b9a00af2f5ef7935ca0fb683e9eba7a, the pushed detached-boundary delivery. The previous turn is progress. The convergence goal remains active; no new Astra consultation or delegation occurs.

Both executors now consume their completed owner into another admitted program through the common RuntimeProgramInput grammar: one PreviousResult plus ordered detached inputs. The retained executor is the execution context; a copied fragment or reconstructed resource environment is not introduced. Complete ABI/pattern and input-custody checks borrow the packet before placement. Published-registry phase adapters now have actual production callers. Ledger revisions/issuance/command history, Need producers and publication state, generation, streams/observations and allocation frontiers remain on the same executor. AWBC root frames advance the existing frame identity frontier; AwaitMany ordinal, stream state, line cursor and budget are retained. Ready-frame admission rejects affine leftovers and unfinished cleanup instead of discarding them. Native and Product activation still execute no host/backend work.

The sole resource traversal now also classifies nested producer-owned Needs for detached export. The owning registry determines whether a Need carries execution custody; unregistered external identities remain admissible. Detached continuation inputs use the same check, and the old result remains in its executor until it moves to the selected parameter/capture slots. Shared parent reconciliation now authenticates unchanged slots rather than skipping their ledger proof; genuine no-ops leave revision unchanged. Failed placement/reconciliation drops the candidate before restoring the original executor and input packet from inert images.

Meaningful failure testing finds that Product rollback previously reset unfinished Need submission state through the ordinary save-resume path. NeedProducerRestorePolicy now owns Resume versus Rollback behavior in the existing registry restoration authority, with one decoder and unchanged version-1 shapes. Native rollback and Product rollback preserve the original accepted host frontier; ordinary Product restore still re-ensures restartable work. A final-revision ledger forces rejection after candidate placement and proves complete executor/input rollback, including producer metadata and allocation state, in both executors.

Owner evidence currently passes 21/21 nextest cases, including actual issued StageActor movement, detached prefix inputs, internal Need export refusal and continued execution, repeated return/rollback/restore, fresh AWBC frame IDs, duplicate/ABI rejection, exact no-op owner validation, and late transaction rollback. The initial list fails on a missing Arc qualification; that fixture is repaired. Initial run: 10/11 pass, AWBC restore rejects an unregistered test producer plan. The fixture now includes its actual verified task-plan row. A subsequent 7/11 run exposes a noncanonical fixture string table, repaired with the existing canonicalizer. A 12/14 run identifies completed root frames and expected resume re-ensure behavior; the readiness rule is repaired and the restore test verifies exact dispatch identity/frontiers plus re-ensure rather than incorrectly demanding unchanged submission. The first revision-failure fixture cannot serialize keyed ledger metadata as JSON; it is replaced by a crate-private test-only revision mutation on the same owner, with no new production API or live-value clone. The resulting 15/16 run then exposes the real Product rollback submission-state bug; typed restore policy fixes it. Earlier failed receipts remain failed. Final owner receipts: `arcweft-1003-continuation-policy-owner-{list,run}.log` in `%TEMP%`. This is inert native rollback and Product restore evidence, not a claim of public native persistence or complete public line-ledger JSON codec coverage.

Fresh locked metadata and inverted core dependency tree yield the same 50 workspace candidates for default and all-feature graphs, unchanged from the preceding cut. Source is frozen after the 21-case pass for full nextest reverse coverage and workspace all-target/all-feature check/Clippy. Structural gate passes: 2697 files, 2567 Rust files, 1481776 Rust physical LOC, 97 packages, 351 review triggers, 0 blocking violations. The new program_invocation leaf owns only the common ephemeral input/failure grammar and inert input rollback; existing engine, fiber, registry and producer owners retain behavior. There is no dependency-edge change, second executable catalog, legacy reader or version increase. Required broad checks and explicit commit/push remain pending. General View/default consumers, retained UI audit acceptance, nominal C1-C6 and scheduler/restore remain required; this owner-preserving continuation does not close those gates. No external blocker exists.

Final cut evidence supersedes the pending validation above: the source-frozen reverse list selects 4591 runnable cases across 172 binaries and 46 packages. The complete run passes 4591/4591, 3 slow, 16 existing ignored cases skipped. The disjoint CLI library/bin and six integration lists/runs pass 169/169 and 24/24 respectively. Broad total: 4784 passed, 0 failed, 16 skipped; the overlapping 21 owner cases are not added again. All 50 resolved candidates remain covered by workspace compilation/lints; the same three packages with no selected test cases remain included. Workspace all-target/all-feature check and Clippy both exit 0 with warnings, including large error variants inherited through the new diagnostic wrapper; no suppression or Cargo concurrency/retry/feature workaround is added. Format and diff whitespace checks exit 0. All commands are terminal, and no Rust source is edited during any list/run/trybuild interval.

Receipts in `%TEMP%`: `arcweft-1003-continuation-{metadata,allfeatures-metadata}.json`, `continuation-core-tree.txt`, `continuation-{rdeps,allfeatures-rdeps}-packages.txt`, `continuation-rdeps-{list,run}.log`, `continuation-cli-{list,run}.log`, `continuation-cli-integration-{list,run}.log`, `continuation-workspace-{check,clippy}.{log,exit}` and `continuation-structure.log` (all shortened names retain the `arcweft-1003-` prefix). The 15 changed source hashes in `arcweft-1003-continuation-source-hashes.json` have SHA-256 8B2DDBDDB71D7114993B64045D4FF4794C4BFD530A3FC00014F572A37C71ECC7 and match after all checks. Local and remote main are reobserved at f31ef0938b9a00af2f5ef7935ca0fb683e9eba7a before staging. Review covers the complete changed source and the new grammar leaf; explicit staging includes only this checked continuation/rollback unit and its maintained contract/evidence.

This closes completed-program continuation on the same executable lease. It does not claim arbitrary Entry/retained-View ingress, in-context nested program calls, cross-artifact resource migration or full general View support. The next consumer work must use the same typed ABI and retain the actual caller/producer custody instead of extracting bare values or manufacturing a second executor environment. Preserve the full existing convergence acceptance, including retained UI, nominal and scheduler/restore work. Continue with coherent validated commits as requested; the goal remains active.

### 2026-10-03 — View default pipeline WIP and independent cache-budget cut

Observed base is b7d79698d0a34deb59a326ccf82bb4c4aa152cc4 on existing main. The working tree contains this continuation's owned View default producer/codec/mount migration; no commit of these new defaults is claimed. Compiler and driver library checks pass, but the positive compiler-to-AWBC-to-mount matrix remains failing for a callback parameter whose omitted effect row is a declaration-owned Free generic effect parameter. It is not an Inference reference and must not be replaced with an empty row or narrowed globally from the default. View generic invocation/specialization needs its actual typed authority. No new Astra consultation is made; the Rust move one-shot remains consumed.

The WIP replaces scalar default indices with exact RuntimePureProgramId/input/result descriptors and shares handler/default admissions. String literal, pure project function, tuple with a preceding String input, nominal record and enum defaults have reached executable mount/restore in the same positive matrix; callback cases remain required failures, not ignored or deleted. The initial test fixture omitted standard dialogue AWBC installation; that fixture is corrected and the original strict parameter-runtime-type cross-join is retained. Product Return is a normal terminal notification; external requests/effects remain rejected. An attempted MaySuspend-flag gate is removed because existing verified program emission conservatively sets it for a pure project call. Actual typed default admission and execution remain the purity boundaries.

Independent, complete owner behavior is available for scalar cache fuel: FxEvaluationBudget::charge_operations charges cached View value programs the same validated instruction cost as cold execution; a fresh/cached/restored regression preserves identical failure, remaining fuel and successful reuse cost. Its narrow nextest check passes. These two source files can form a separate coherent cut: crates/arcweft-presentation/src/fx/program.rs and crates/arcweft-view/src/value_program.rs (test embedded in the latter). Do not accidentally stage the unfinished default/type/generic migrations or the new default paragraph in the maintained View chapter with that cut.

Current default/all-feature locked metadata both select 57 workspace packages from the union of presentation, core, View, bundle, compiler and driver reverse dependencies. Receipts use the %TEMP% arcweft-1003-view-default- prefix. Source-frozen complete library/integration nextest list is building; full execution must retain the known callback acceptance failure and use --no-fail-fast to inspect independent consumers. Separate maintained CLI surfaces remain to run. Workspace all-target/all-feature check and Clippy are queued in one sequential process. Rust source hashes (33 paths) are saved in view-default-source-hashes.json. Do not edit Rust while the list/run/trybuild is alive. No new Cargo job limit, feature workaround, retry or test exclusion is used. A list/build, a queued check, and this note are not broad pass evidence.

Required next: finish current live validation, record actual failures/counts, review and deliver the independent cache-budget cut if its consumer evidence establishes it. Continue the unfinished View default generic specialization and retained execution custody/Need work using the existing typed authority; preserve all full convergence acceptance. The goal remains active and no external blocker exists.

Validation continuation: workspace check and Clippy with --workspace --all-targets --all-features both exit 0 with warnings. Separate, disjoint maintained CLI slices finish 169/169 library/bin tests and 24/24 integration cases; their lists precede execution, and the two CLI commands are intentionally independent with default Cargo coordination. The resolved library/integration list selects 6652 runnable cases and explicitly includes the cache cold/warm/restore regression, the function assignment contract regression and the required generic-default positive case. The run uses --no-tests fail and --no-fail-fast; at this point it is still completing serial public-API compile-fail tests. Its only observed failure is the unfinished generic View default, which is not part of the cache-budget staging scope. The cache regression passes in this complete run. All 33 captured Rust source hashes remain unchanged.

Structural audit on the observed working tree exits 0: 2698 files, 2568 Rust files, 1482546 physical Rust LOC, 97 packages, 351 review triggers, 0 blocking violations. For the independent cache cut, presentation/src/fx/program.rs is 62221 bytes / 1790 physical LOC (base 1771); its existing typed instruction grammar/evaluation owner also owns shared fuel accounting. View/src/value_program.rs is 30339 bytes / 941 physical LOC (base 895); its mount owns input revisions and the derived cache, and inert save still omits that cache. The added test follows that same cold/cache/restore behavior. Neither cut changes a Cargo dependency, adds another cache state owner, or widens an API for file splitting. Existing larger-owner dispositions remain applicable. The broader unpublished default migration still needs its generic contract and execution-custody acceptance, regardless of successful compilation or the scanner's structural result.


Final observed-working-tree receipts: reverse-dependency library/integration execution exits 100 after 6652 tests, with 6651 passed (6 slow), 1 failed and 24 existing ignored cases skipped. The sole failure is compiler_defaults_execute_general_values_and_refresh_preceding_inputs, at its required generic callback default. The two separate CLI surfaces add 193 passes; the disjoint broad total is 6844 passed, 1 failed and 24 skipped. This is explicitly not a passing whole-default migration or a clean-HEAD run: the tree contains its uncommitted producer/consumer WIP. The cold/warm/restored cache case passes against unchanged source bytes; no failed case is excluded, ignored or asserted weaker, and no runner retry override is used. The independent staged cut contains only the two cache/fuel files and this evidence note. The stable View evaluation-cost contract already requires equal semantic fuel for cache hits, so the unpublished default-ABI chapter edits are not included. No Rust source changed during the complete list/run/trybuild intervals, and all validation sessions are terminal. Required continuation is the generic View invocation/specialization contract and original remaining goal acceptance.

### 2026-10-03 — typed View defaults and declaration input contracts

Observed main and remote base: b72f59516f8e6097436dcc3594728eec59b8354f. The working tree contains this goal's default producer/codec/mount migration. The previous generic callback failure is superseded by a passing seven-case compiler/AWBC/codec/mount/restore matrix: String, pure project call, dependent tuple, nominal record, enum, explicit closure and implicit closure. The declaration's existing generic binder now closes the retained parameter schema; closed subterms keep their root semantic identities. Core checks effect constraints jointly, including contravariant callback inputs, and quantifies function-local rows before declaration input rows. Invalid program identity, invalid supplied values and forged saved callback values are rejected. A nested Need in borrowed root input is rejected before cloning or changing retained state. The latest source additionally checks monomorphic saved values; broad validation is still pending at this note's creation.

Fresh locked default/all-feature metadata both select 53 workspace packages from core, View, bundle, compiler, runtime-driver, sema and runtime-plan transitive reverse dependencies. Maintained CLI surfaces are separate. All Rust source is frozen through nextest list/run/trybuild, with 43 source hashes in `%TEMP%/arcweft-1003-default-cut-source-hashes.json`. Owner receipts `default-cut-owner-{list,run}.log` pass 3/3; the ingress repair `default-cut-ingress-{list,run}.log` passes 1/1. All shortened receipt names have prefix `arcweft-1003-`. These overlapping narrow cases will not be added to the broad total.

Structural gate exits 0: 2700 files, 2570 Rust files, 97 workspace packages, no blocking violations. The new Core parameter-contract leaf (14961 bytes / 397 LOC, including its joint-variance regression) owns only binding of retained input effects against the existing executable type table and decision algebra. Sema's parameter-contract leaf (3545 bytes / 95 LOC) uses the existing declaration quantifier; it introduces no parallel type representation. Driver evaluator (96712 bytes / 2323 LOC; base 2026), compiler View lowering (66167 bytes / 1640 LOC; base 1451) and bundle View codec (69417 bytes / 1814 LOC; base 1695) retain their existing hydration, checked publication and wire-validation responsibilities. No dependency edge, I/O owner, version marker or compatibility reader is added. The large evaluator keeps one atomic hydration boundary; cache entries are inert evidence and the parameter map owns live values.

This unit does not close all retained UI acceptance. Borrowed ingress and default caching admit copyable values; affine values and execution-owned Needs still require actual retained execution custody. Generic callback captures in later defaults and higher-rank function input contracts remain fail-closed until their authentic scoped program ABI is implemented. Nested general View calls and reactive general-value consumers remain on the existing goal. Native rollback remains distinct from unsupported public native persistence. Do not mark the convergence goal complete. No new Astra consultation is performed; the Rust-move one-shot remains consumed, including after compaction. The next delivery is a reviewed, validated commit/push of this unit, preserving all remaining goal acceptance.

Final evidence supersedes pending validation above: the source-frozen reverse list/run selects 4807 cases across 199 binaries and 49 packages, and passes 4807/4807 (4 slow), with 16 existing ignored cases skipped. Separate maintained CLI lists/runs pass 169/169 library/bin and 24/24 integration cases. Disjoint total: 5000 passed, 0 failed, 16 skipped; overlapping owner receipts are not counted again. The 53 graph candidates also contain the separately tested CLI and three packages without selected cases (arcweft, arcweft-bundle-assets and arcweft-render-web). Workspace all-target/all-feature check and Clippy both exit 0 with warnings. Format, diff whitespace and structural gate exit 0. All sessions are terminal. All 43 frozen source hashes match; the receipt JSON has SHA-256 A577659EC1708D3A0B48817269D1CF63257831DD1D52A201E310E77DEA7CB7F3. No assertion weakening, ignored-test addition, retry override, source edit during list/run/trybuild, feature workaround or Cargo concurrency override occurs. Receipts in `%TEMP%` use `arcweft-1003-default-cut-`: `{metadata,allfeatures-metadata}.json`, `{metadata,allfeatures-metadata}-packages.txt`, `core-tree.txt`, `rdeps-{list,run}.log`, `cli-{list,run}.log`, `cli-integration-{list,run}.log`, `workspace-{check,clippy}.{log,exit}`, `fmt.log`, `structure.log` and `source-hashes.json`. Local/remote main are reobserved at b72f59516f8e6097436dcc3594728eec59b8354f before delivery. Publish effect quantification independently, then the reviewed default producer/consumer migration, without rerunning unchanged bytes. This is a tested partial goal delivery, not closure of retained execution custody or the full convergence acceptance.

Delivery split: the independent effect quantifier and its regression/evidence are committed and pushed as 776c90396e53392f81f3def3f0c9e7ed31e56085. The remaining checked default producer/consumer unit is staged separately on that parent. Committing the unchanged tested sources preserves the above receipts. Continue the original goal with scoped generic capture ABI and actual retained input/Need execution custody; preserve the remaining View, nominal and scheduler/restore acceptance. The goal stays active, and the one-shot Astra consultation is not repeated.
