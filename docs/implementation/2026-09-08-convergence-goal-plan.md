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
