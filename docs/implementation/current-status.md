# Arcweft 現状一覧

確認日: 2026-10-23。資料監査の基点は既存 `main` の
`7b1f8f5deb64db1274632c6ec2a7b8af69147420`。
今回の検証入力は `2cfd5197813492653b004f0375116eb419cbd60e` と後続のテスト fixture 変更。
資料整理は `71a00e3eec1b43067fb9f64676b9158f7b44c113` として公開済み。
調査開始時の14件の tracked WIP と既存の untracked 修復メモ等は保持した。
途中の公開履歴と固定入力ごとの実行結果は検証履歴を参照する。

この文書を項目別の現状の唯一の一覧とする。現行の契約は
[仕様索引](../README.md)、過去の実行結果・公開 SHA は
[収束検証履歴](convergence-evidence-history.md) と
[その他の検証履歴](evidence-history.md) に置く。
履歴中の古い現在形・計画・当時の失敗を、この一覧より優先しない。

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
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | `850534b9e9bc8c8872f6528dd00050a5e6e7699e` 時点の監査で、17入力／公開 blob と151件を確認。115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC 継続／保存が2実行とも PASS。完全 transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publication を含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。後述のキュー移行のコンパイル失敗から、この受入を取り消さない。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **実装・受入履歴あり** | `86958911833f2509f391b70962773236e46a124e` の full-checkout 記録は library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate が PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md) は package 環境の Cargo 未実行も別に記録する。現在の source の planner/係数と [現行契約](../03-presentation/text-typesetting.md) を照合した。旧着手手順や完了済み JLREQ を残作業として再掲せず、実 PNG golden とハッシュを履歴・fixture に保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission | **各 cut は公開済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終公開／受入 receipt は今回確認できていない。`HostTaskRequest` の14 variant と `NeedProducerFamily` の10 tag は異なる分類であり、「14 family」という略記から未実装数を推定しない。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須 program lease と、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` の callee 退役処理。キュー切替前の修復入力で R2355 は8 library／2,463 PASS。現在のキュー移行全体の runtime 合格を意味しない。 |
| Native の original-source queue／runtime continuation 移行 | **root キューの Core 検証済み・全 consumer の最終受入は未確認** | 独立 root／resume cursor を除去した本番側と元コードのキューへ揃えた fixture で、R2374 Core library **1,955/1,955 PASS**、R2375 Core all-target Clippy exit 0（警告あり）。[実行監査](acceptance-evidence/task-final-r2374-native-root-queue-owner-runtime.terminal-audit.json)。R2372 の旧フィールドを使う17件のコンパイルエラーは修復済み。欠落・重複・別 owner、停止復帰、join、formatter、Await を含む。旧 runtime-only `FlowOp` variant の削除と全 reverse53 の受入は、この Core PASS に含めない。 |
| Thread の返り値／返却 handle | **具体的な未達あり** | `crates/arcweft-runtime-plan/src/final_expr.rs` は `HirExprKind::Thread` を pure runtime expression として拒否する。返却された ThreadHandle の compiler 受入を、statement Thread の既存実装で代替しない。これは Generic Match の未達ではない。 |
| AWBC の独立 Thread scheduling | **具体的な未達あり** | `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` の `FlowOp::Thread` は body を同じ frame の scope 内へ inline lowering する。独立 task/kernel の起動を証明しておらず、Native の program task 経路の成功を AWBC scheduling の成功へ読み替えない。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **直近の全閉包実行に失敗あり** | R2342 は7,217実行、7,165 PASS／52 FAIL／39既存除外。失敗内訳は CLI 51・Compiler 1。失敗ケースの完全な一覧は [terminal audit](acceptance-evidence/task-final-r2342-joined-frame-reverse-runtime.terminal-audit.json)。後の狭い PASS を加算して、この全閉包を合格としない。 |
| 現 HEAD の最終 workspace／Tier 2／Native-Web capture parity | **新たな全体合格なし** | Core library の今回の PASS と、未再実行の workspace／Tier 2／parity を区別する。以前の検証の結果はそれぞれの固定入力の履歴として保持し、既に受入済みの各機能を再実装対象へ戻さない。 |

Generic Match の151件は run `119cbafe-0a01-419e-841f-b919f03d9578` と
`c99c9fb1-8db4-48d9-a3d2-df5f41144aea` の実績。
資料監査の基点 `7b1f8f5deb64db1274632c6ec2a7b8af69147420` では、後続の owner 移行により
17入力中9 blob が変わっていた。これを現 HEAD の151件再実行とは呼ばない。
資料監査では Never・uninhabited empty Match・let-else・nested non-exhaustive atomicity
の実装／テストを照合し、同機能を再開する
具体的な新規不具合は確認していない。

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
