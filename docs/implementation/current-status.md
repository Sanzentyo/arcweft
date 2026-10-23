# Arcweft 現状一覧

確認日: 2026-10-24。基点 `a8ea56ed7a6293800dc05fafade3b6b687f44c55` に
Native ThreadHandle の21 Rust pathsの変更と既存14 tracked WIPを含めて検証した。
R2469 は Core の全53逆依存 package（名前付きケースは50 package）で
**7,182 PASS／52 FAIL／39既存 ignored**。受入済み Generic Match 151件と
今回の必須8件は全 PASS。Core 2,013件・Sema 1,328件・RuntimePlan 158件も全 PASS。
4,298固定入力・case set・225 executable の drift は0。
52失敗の識別子はR2458と一致するが、ThreadHandleの複合ケースはNativeを通過し、
AWBC loweringの未割当localで失敗する段階へ進んだ。
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
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | `850534b9e9bc8c8872f6528dd00050a5e6e7699e` の17入力／公開 blob と151件を確認。115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC 継続／保存が受入時の2実行とも PASS。完全 transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publication を含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。今回のR2469でも同151件が全 PASS。[今回入力の照合](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.acceptance-audit.json)。全体 goal の未達から、この受入を取り消さない。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **当該 cut の受入履歴あり・CLI 統合描画に再現失敗あり** | `86958911833f2509f391b70962773236e46a124e` の library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate は PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md)、[現行契約](../03-presentation/text-typesetting.md)。今回のR2469でも published JLREQ geometry／raw crop 8件が失敗し、hyphen 後の column が1169／1127に分かれる、後続本文が左へ進まないという具体的な診断を保持する。既存 layout 受入と CLI 統合描画の修復を区別し、PNG golden／hash と当時の実績は保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission／scheduler | **各 cut は公開済み・Program 結果保管／完了受付は検証済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。Program の値は共通 registry が保管し、私有の起動 proof を検査した Host／Web は退役 metadata のみを公開する。R2424 で Scheduler 17件、Host の実際の退役／公開2件と終端 session 2件、Web Source 認証1件が全 PASS。[必須・既存ケース照合](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.acceptance-audit.json)。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終受入は未確認。Host request の14 variant と Need producer の10 family は別の分類。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須 program lease と、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` の callee 退役処理。キュー切替前の修復入力で R2355 は8 library／2,463 PASS。現在のキュー移行全体の runtime 合格を意味しない。 |
| Native の original-source queue／runtime continuation・結果保管 | **Program の結果保管・退役は owner／consumer 検証済み** | 実際の返却 frame が値を cleanup／join／rollback 中も保持し、退役時に共通 registry の `Ready` へ移す。ローカル await が一度だけ値を取得し、`ReadyTransferred` をHost ACKで復活させない。R2424／R2426の結果を保持し、今回は実際のlive handleを親環境へ束縛した状態で、非Unit返却・各step後rollback・ACK・metadata再送・fallthrough失敗の3件も全PASS。Core全2,013件がPASS。[今回の照合](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.acceptance-audit.json)。公開Native saveとAWBCの受入は別項目。 |
| Thread の返り値／返却 handle | **意味解析とNativeのhandle受渡し・awaitを検証済み／全backend受入は未達** | 公開 `dd7ce52715d3beda046ad465063623ece4ea5092` の子task結果型・Return／Try所有境界を維持し、今回21 Rust pathsでstatement/valueを同じCPS loweringへ統合した。必須の型付きresult patternへ実際のaccepted receiptを束縛し、拒否／panicは起動公開前にjournalを戻す。Host NeedとProgram ThreadHandle、payload型の取り違えを拒否する新規Coreケースと、関数から返したhandleをawaitして実際のBool false／trueを取得する新規NativeケースがR2469で全PASS。既存の複合ケースもNative分岐は通過したが、次項のAWBCで失敗する。Compiler `flow_include` は13 PASS／1 FAIL。[必須8件と全151受入の照合](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.acceptance-audit.json)。 |
| AWBC の独立 Thread scheduling | **具体的な未達あり** | `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` の `FlowOp::Thread` はbodyを親frame内へinline loweringし、result bindingを作らない。R2469の関数からhandleを返す複合ケースは、Native完了後、`awbc_lower/expr.rs:78` の未割当local `1`でpanicした。独立Program kernel・child・結果・保存復元の接続が必要であり、Nativeの成功をこの境界の受入へ転用しない。[実診断](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.failure-recon.json)。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **広域実行に CLI 51件／Compiler 1件の失敗** | R2469はCoreの全53逆依存を選択。CLIは640 PASS／51 FAIL、Compilerは612 PASS／1 FAIL。52失敗識別子はR2458と一致し、新規・消失とも0。今回もtyped Presentation ABI未接続9件、JLREQ geometry／raw crop8件を再現し、hyphenのcolumnは1169／1127。ThreadHandleの失敗地点は前項のとおり進んだ。識別子だけで原因を同一視しない。[今回の全診断](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.failure-recon.json)、[件数・受入照合](acceptance-evidence/task-final-r2469-thread-handle-reverse-runtime.acceptance-audit.json)。 |
| 最終 workspace／Tier 2／Native-Web capture parity | **直近の workspace check／Clippy・構造gateは合格。最終受入は未達** | 最終Rust入力のR2470 check・R2471 Clippyはworkspace all-target／all-feature／locked、exit 0（警告あり）。公開FlowOp／Source grammar変更を受けR2472のcanonical構造gateを新規実行し、blocking 0。R2469は7,182 PASS／52 FAILで、全workspace runtime合格ではない。39既存ignored、Tier2、Native-Web capture parityの最終受入は未実行。R2420の76 doctestも当時の実績として保持する。[固定入力・owner disposition・実結果](convergence-evidence-history.md)。 |

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
