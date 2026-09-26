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
