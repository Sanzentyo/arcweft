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
