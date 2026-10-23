# Arcweft 現状一覧

確認日: 2026-10-24。Thread 境界の公開入力
`dd7ce52715d3beda046ad465063623ece4ea5092` を既存14 tracked WIP とともに検証した。
R2452 の23逆依存 package は **4,756 PASS／58 FAIL／31既存除外**。
受入済み Generic Match 151件と新規 Thread 6件は全 PASS。
その後、追加6失敗と新規 Await 回帰テストは R2454 の **7/7 PASS** で限定修復を確認した。
広域実行の58失敗を、その後の入力の失敗件数へ転用しない。
公開 SHA・固定入力・実際の途中失敗と修復は [検証履歴](convergence-evidence-history.md) に保持する。

この文書を項目別の現状の唯一の一覧とする。現行の契約は
[仕様索引](../README.md)、過去の実行結果・公開 SHA は
[収束検証履歴](convergence-evidence-history.md) と
[その他の検証履歴](evidence-history.md) に置く。
履歴中の古い現在形・計画・当時の失敗を、この一覧より優先しない。
保持した untracked の2026-10-09 CLI 修復メモも当時の handoff であり、
現在の失敗件数・残作業の一覧ではない。元の分類 JSON は
[検証履歴の原本](acceptance-evidence/2026-10-09-cli-failure-classification.json)へ移した。

## 判定の扱いと訂正

受入・公開済みの項目を、全体 goal が未完了という理由で未完了へ戻さない。
再開する場合は、該当する受入条件、新しい失敗ケース／再現手順、検証入力
または原因箇所を記録し、既存の受入実績と追加の修復範囲を分ける。
後続の機能・保存方式・別バックエンドの未達は、先行する完了項目の未達ではない。

Generic Match の後続メモにあった「未完了」「C3/C5 が残る」という列挙は
誤りだった。完了監査を再確認せず、全体 goal の残作業文を繰り返したことが原因で、
具体的な新規不具合による再開ではなかった。これらの列挙と重複した現状説明は削除し、
受入結果・失敗を含む実際の検証履歴と full SHA を保持した。

## 項目別の状態

| 項目・境界 | 状態 | 根拠と、別途区別する範囲 |
|---|---|---|
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | `850534b9e9bc8c8872f6528dd00050a5e6e7699e` の17入力／公開 blob と151件を確認。115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC 継続／保存が受入時の2実行とも PASS。完全 transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publication を含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。R2452 の公開入力でも同151件が全 PASS。[公開入力の照合](acceptance-evidence/task-final-r2452-thread-boundary-reverse-runtime.acceptance-audit.json)。全体 goal の未達から、この受入を取り消さない。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **当該 cut の受入履歴あり・CLI 統合描画に再現失敗あり** | `86958911833f2509f391b70962773236e46a124e` の library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate は PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md)、[現行契約](../03-presentation/text-typesetting.md)。R2424 の published JLREQ geometry／raw crop 8件は、hyphen 後の column が1169／1127に分かれる、後続本文が左へ進まないという具体的な失敗。既存 layout 受入と CLI 統合描画の修復を区別し、PNG golden／hash と当時の実績は保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission／scheduler | **各 cut は公開済み・Program 結果保管／完了受付は検証済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。Program の値は共通 registry が保管し、私有の起動 proof を検査した Host／Web は退役 metadata のみを公開する。R2424 で Scheduler 17件、Host の実際の退役／公開2件と終端 session 2件、Web Source 認証1件が全 PASS。[必須・既存ケース照合](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.acceptance-audit.json)。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終受入は未確認。Host request の14 variant と Need producer の10 family は別の分類。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須 program lease と、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` の callee 退役処理。キュー切替前の修復入力で R2355 は8 library／2,463 PASS。現在のキュー移行全体の runtime 合格を意味しない。 |
| Native の original-source queue／runtime continuation・結果保管 | **Program の結果保管・退役は owner／consumer 検証済み・言語側の返却 handle 統合は未達** | 実際の返却 frame が型付き値を cleanup／join／rollback 中も保持し、退役時に共通 registry の `Ready` へ移す。Host を介さずにローカル await が参照し、`AlwaysStart` は一度だけ値を移して `ReadyTransferred` を残す。Host ACK は取得済みの値を復活させない。空 body／暗黙 Unit も元の結果契約を通し、非Unitの fallthrough は失敗 task として保持する。R2417／R2424 の5 library **2,158件**が PASS（Core **1,959件**）。最終の通常Host event拒否強化後も R2426 で同2,158件が全 PASS。新規3件は非Unit返却・各step後rollback・一度の移動／ACK・resume時metadata再送・fallthrough失敗を検証。[実行原本](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.log)。保存 schema は origin を区別した version 1。これを次項の ThreadHandle／AWBC scheduling の受入とは呼ばない。 |
| Thread の返り値／返却 handle | **意味解析・Native statement の結果契約は検証済み／返却 handle 実行は未達** | 公開 `dd7ce52715d3beda046ad465063623ece4ea5092` では `CheckedThread` が結果型を所有し、Return／Try は元の子 task 境界を保持する。R2452 の新規 Thread 6件と Sema library 1,303件は全 PASS。Compiler `flow_include` は12 PASS／1 FAILで、実ソース Bool 退役・capture・quota は合格し、関数から handle を返して await する既知ケースが失敗した。`final_expr.rs` の Thread pure lowering 拒否と Core Await の Need-only projection は未置換。[実結果と必須ケース照合](acceptance-evidence/task-final-r2452-thread-boundary-reverse-runtime.acceptance-audit.json)。手製の Unit 投影だった RuntimePlan fixture は checked ThreadHandle の型・digest へ揃え、R2454 で PASS。 |
| AWBC の独立 Thread scheduling | **具体的な未達あり** | `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` の `FlowOp::Thread` は body を同じ frame の scope 内へ inline lowering する。独立 task/kernel の起動を証明しておらず、Native の program task 経路の成功を AWBC scheduling の成功へ読み替えない。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **公開入力の広域実行は58失敗。その後の追加6差分は限定再検証で修復済み** | R2452 は CLI 53・Compiler 1・HIR 2・Sema 1・RuntimePlan 1の失敗。R2424 の52失敗はすべて残り、追加は同じ Await/context fixture の CLI 2件と、型投影／compile-fail fixture 4件だった。これら6件と新規回帰1件は修復後の R2454 で全 PASS。追加分の修復後に、広域の失敗件数を再計数したとは主張しない。typed Presentation ABI 未接続9件、JLREQ geometry／raw crop 8件、返却 ThreadHandle 等の実診断は保持する。[広域診断](acceptance-evidence/task-final-r2452-thread-boundary-reverse-runtime.failure-recon.json)、[限定再検証](acceptance-evidence/task-final-r2454-await-observer-regression-runtime.terminal-audit.json)。 |
| 最終 workspace／Tier 2／Native-Web capture parity | **直近の workspace check／Clippy は合格。最終受入は未達** | 追加6差分の修復入力で R2455 check・R2456 Clippy は all-target／all-feature／locked、exit 0（警告あり）。所有者・依存・API の変わらない構造は R2447 blocking 0 と今回の計測を再利用した。R2452 の広域58失敗と、その後の R2454 限定7 PASS は上記の別入力。R2420 の76 doctest は当時の入力の実績であり、今回の再実行とは呼ばない。Tier2とNative-Web capture parityの最終受入も未確認。[固定入力ごとの実績](convergence-evidence-history.md)。 |

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
