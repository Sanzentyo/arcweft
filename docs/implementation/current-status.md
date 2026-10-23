# Arcweft 現状一覧

確認日: 2026-10-23。資料監査の基点となった既存 `main` は
`7b1f8f5deb64db1274632c6ec2a7b8af69147420`。
調査開始時の14件の tracked WIP、既存の untracked 修復メモ等は保持した。
資料整理は `71a00e3eec1b43067fb9f64676b9158f7b44c113` として公開した。
Native child queue 移行は `87bd6493162454d19f57c6341d9db3020b41847f` として公開した。
queue reader／Await 移行は `d64d236091748a13b7b3f25f535f08a53e0dec23` として公開した。
元コード fixture と空 range の処理は `a5ad3b4ae1cf0cb3b5461490878e08c082213355` で公開・検証した。
その後の独立 root cursor 除去と R2370 の失敗を下記に反映した。

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
| Native の original-source queue／runtime continuation 移行 | **前段 Core cut は検証済み・root 切替はコンパイル要修復** | `a5ad3b4ae1cf0cb3b5461490878e08c082213355` では R2368 Core library 1,955/1,955 PASS、R2369 Core all-target Clippy exit 0（警告あり）。[実行監査](acceptance-evidence/task-final-r2368-native-original-fixture-owner-runtime.terminal-audit.json)。後続の WIP で独立 `NativeFlowExecutionFrame.cursor` を削除し、Flow／Goto／関数入口を `FunctionRoot` work に統合した。R2370 は復帰・rollback 等の旧フィールド参照22 errors、exit 101。[失敗ログ](acceptance-evidence/task-final-r2370-native-unified-function-root-check.log)。同利用側と runtime 専用の旧 `FlowOp` variant の移行、および全 reverse53 の受入はこの前段 PASS に含めない。 |
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

## 直近の検証入力を取り違えないための記録

| 検証 | 実際の結果 | 適用範囲 |
|---|---|---|
| R2342 Core 逆依存 | 7,165 PASS／52 FAIL／39 skipped、exit 100 | 53 package を選択。host 上に named test がない3 package は合格件数に含めない。3,976入力・225 executable と case set の drift は0。 |
| R2355 original code position 修復 | 2,463 PASS／0 FAIL／0 skipped、exit 0 | 8 library。3,977入力・8 executable・case set の drift は0。現キュー WIP より前の入力。[terminal audit](acceptance-evidence/task-final-r2355-native-code-position-owner-runtime.terminal-audit.json)。 |
| R2356 workspace all-target／all-feature／locked check | exit 0 | キュー切替前の入力。[receipt](acceptance-evidence/task-final-r2356-native-code-position-workspace-check.json)。 |
| R2359 Core all-target／all-feature／locked Clippy | exit 0、既存警告あり | キュー切替前の入力。警告なしとは言わない。[receipt](acceptance-evidence/task-final-r2359-native-code-position-final-core-clippy.json)。 |
| R2361 `cargo check -p arcweft-core --lib --all-features --locked` | **exit 101、33 errors／24 warnings** | `7b1f8f5deb64db1274632c6ec2a7b8af69147420` の source dispatcher 移行入力。runtime 実行は始めていない。[receipt](acceptance-evidence/task-final-r2361-native-work-producer-migration-check.json)。 |
| R2362 同 Core library check | **exit 101、29 errors／25 warnings** | child queue 移行の途中。誤った `DialogueActivationFrame.group` 参照の E0609 を含む。[receipt](acceptance-evidence/task-final-r2362-native-task-root-producer-check.json)。 |
| R2363 同 Core library check | **exit 101、28 errors／24 warnings** | 正しい `task_group` 参照へ修正し、検証済みの空 task body をキューへ積まない形にした入力。上記 E0609 は解消。runtime 実行なし。[receipt](acceptance-evidence/task-final-r2363-native-task-root-correction-check.json)。 |
| R2364 同 Core library check | **exit 0、151 warnings** | queue reader／join／numeric Map／dialogue activation／Await source 移行の入力。テストや runtime の成功を意味しない。[receipt](acceptance-evidence/task-final-r2364-native-source-readers-await-check.json)。 |
| R2365 Core all-target／all-feature／locked Clippy | **exit 101、lib test の34 compile errors／79 warnings** | library の検査とテストのコンパイルを区別する。nextest／runtime は未実行。[receipt](acceptance-evidence/task-final-r2365-native-queue-readers-core-clippy.json)。 |
| R2366 Core all-target／all-feature／locked check | **exit 101、lib test の1 compile error／31 warnings** | formatter fixture の空入力配列の型推論で失敗。その後、slice 型を明示して修復。[receipt](acceptance-evidence/task-final-r2366-native-original-fixture-check.json)。 |
| R2367 Core library nextest list | **exit 0、1,955件選択／除外0** | 全件と、join／scope／Await／formatter／affine literal の必須11件を確認。[receipt](acceptance-evidence/task-final-r2367-native-original-fixture-owner-catalog.json)。 |
| R2368 Core library nextest run | **1,955 PASS／0 FAIL／0 skipped、exit 0、33.712秒** | 全 feature、3,988入力／1 executable／case set の drift 0。保持14 tracked WIP を含む固定 working tree で実行。[terminal audit](acceptance-evidence/task-final-r2368-native-original-fixture-owner-runtime.terminal-audit.json)。 |
| R2369 Core all-target／all-feature／locked Clippy | **exit 0、警告あり** | lib test の警告は3,163（2,466重複）。警告ゼロとはしない。R2368 と同じ source。[receipt](acceptance-evidence/task-final-r2369-native-original-fixture-core-clippy.json)。 |
| R2370 Core library all-feature／locked check | **exit 101、22 errors／25 warnings** | 独立 root cursor を除去した後続 WIP。復帰・rollback 等の旧参照に E0609。R2368 の入力と異なり、runtime は未実行。[receipt](acceptance-evidence/task-final-r2370-native-unified-function-root-check.json)。 |

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
