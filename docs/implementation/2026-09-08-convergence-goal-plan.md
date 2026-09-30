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
