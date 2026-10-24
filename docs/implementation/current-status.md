# Arcweft 現状一覧

確認日: 2026-10-25。最新の横断検証基点はmain `1f1c00210abb1a42389cc41b8fcc391e9d12d639`。
実装公開は `b90d3948990e64aa727047efb9762a7717b4bf24`。
[公開commit・tree・全pathの照合](acceptance-evidence/task-fanout-child-checkpoint.verified.json)。
Core 8 pathでNative childのpublication保持とAWBC Program suspension初期化を修復した。
最新のCore library単独検証は **1,991／1,991 PASS**、Clippyはall-target／all-featureで終了0・警告あり。
6,122固定入力の差分0、既存のtracked WIP 14件／untracked実ファイル6件の保持を確認した。
[今回の範囲・途中の失敗・検証原本](acceptance-evidence/task-fanout-child-acceptance.json)。
基本のResult集約・消費前検査は `8a4a62cf6ee5a901bea1127517dea7ecf84e6362` で公開済み。
その1,989件と今回の1,991件は別入力の実績で、件数を加算しない。
[前段の公開照合](acceptance-evidence/task-fanout-runtime-checkpoint.verified.json)。

最新のCore全53逆依存package通常選択は、R2843の一回の新規実行で
**7,269実行／7,219 PASS／50 FAIL／39既存ignored**、終了100、1,010.127秒。
失敗50件は前回とbinary／test識別子が完全一致し、新規失敗0。
Generic Matchは151／151 PASS、Coreはlibrary＋integration／binの2,044／2,044 PASS。
Core単独結果を再利用・加算していない。6,199固定入力、実行ファイル225件の前後hash差分0。
[実行集計・旧失敗・Generic Match照合](acceptance-evidence/task-fanout-child-reverse-acceptance.json)、
[package／target別の実行範囲](acceptance-evidence/task-fanout-child-reverse-coverage.json)。
この検証結果は `612cc7a51f9ac04669e62488a8f56fa5cc4ac8c1` で公開し、
[公開ref・全blob・保持入力を照合](acceptance-evidence/task-fanout-child-reverse-checkpoint.verified.json)した。
workspace check／Clippyも同じ基点のall-targets／all-features／lockedで終了0、警告あり。
過去の集計・途中の失敗・訂正理由は履歴と原本に保持し、最新件数へ重ねて加算しない。
全14 family・Tier2等の包括的な受入とは下表で区別する。

この文書を項目別の現状の唯一の一覧とする。現行の契約は
[仕様索引](../README.md)、過去の実行結果・失敗・公開full SHAは
[収束検証履歴](convergence-evidence-history.md) と [その他の検証履歴](evidence-history.md) に置く。
履歴中の古い現在形や計画、保持したuntrackedの2026-10-09 CLI修復メモを現在の残作業一覧へ転記しない。

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
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | 受入 `850534b9e9bc8c8872f6528dd00050a5e6e7699e`、公開 `f0e7ae9662b8f216d7f19dcbea3d4341fcfe7ca2`。17入力／公開blobと151件を受入時の2実行で確認し、115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC継続／保存がPASS。完全transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publicationを含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。最新R2843でも151／151 PASS。[全151件の終端照合](acceptance-evidence/task-fanout-child-reverse-acceptance.json)。受入を再開する新しい不具合はなく、全体goalの未達から受入を取り消さない。 |
| Collection／variant Map | **公開6 familyとArray反復化は対象条件で検証・公開済み** | Vec／Seq／Array／Slice／Option／Resultが公開catalog。Arrayのexact length、直接／retained-prefix、effect／source order、capture、途中保存復元を含む対象46件がPASSした。4,096要素入力も製品既定上限のままPASS。Need／Parser／Streamのschema候補を公開済みとはしない。[修復の検証履歴](convergence-evidence-history.md#array-map反復化の検証--2026-10-24)、[現行契約](../01-language/traits-seq-ranges.md)。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **当該 cut の受入履歴あり・CLI 統合描画に再現失敗あり** | `86958911833f2509f391b70962773236e46a124e` の library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate は PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md)、[現行契約](../03-presentation/text-typesetting.md)。同横断集計のCLI統合描画でも8件の失敗を再現した。具体的な診断は下記「CLIの具体的な観測」に集約する。既存 layout 受入と CLI 統合描画の修復を区別し、PNG golden／hash と当時の実績は保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission／scheduler | **各 cut は公開済み・Program 結果保管／完了受付は検証済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。Program の値は共通 registry が保管し、私有の起動 proof を検査した Host／Web は退役 metadata のみを公開する。R2424 で Scheduler 17件、Host の実際の退役／公開2件と終端 session 2件、Web Source 認証1件が全 PASS。[必須・既存ケース照合](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.acceptance-audit.json)。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終受入は未確認。Host request の14 variant と Need producer の10 family は別の分類。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須program leaseと、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` のcallee退役処理。キュー切替前の修復入力でR2355は8 library／2,463 PASS。最新R2843でも53逆依存の通常選択に新規失敗0。全体goalの未達を、この修復の未実装へ読み替えない。 |
| Native の original-source queue／runtime continuation・結果保管 | **Program の結果保管・退役は owner／consumer 検証済み** | 実際の返却 frame が値を cleanup／join／rollback 中も保持し、退役時に共通 registry の `Ready` へ移す。ローカル await が一度だけ値を取得し、`ReadyTransferred` をHost ACKで復活させない。R2424／R2426の結果を保持し、公開 `a74957e455597a0d3160af11c914e2e93f58d144` のR2469では実際のlive handleを親環境へ束縛した状態で、非Unit返却・各step後rollback・ACK・metadata再送・fallthrough失敗の3件も全PASS。公開Native saveとAWBCの受入は別項目。 |
| Thread の返り値／返却 handle | **結果受渡し・generic capture／nested joinは対象条件で検証・公開済み** | `dd7ce52715d3beda046ad465063623ece4ea5092` の結果型・Return／Try境界、`a74957e455597a0d3160af11c914e2e93f58d144` のNative CPS／accepted receipt束縛を維持する。型付きresult pattern、起動前の拒否／panic rollback、Host NeedとProgram ThreadHandleおよびpayload型の取り違え拒否も保持する。generic `T=bool`の複数回capture使用と、親return後のattached子孫joinを3 backend×false／trueで確認した。最新R2843はCore（library＋integration/bin）2,044／2,044、Compiler 617／617がPASS。[package集計と実行範囲](acceptance-evidence/task-fanout-child-reverse-coverage.json)。公開Native saveの対応は別項目。 |
| AWBC の独立 Thread scheduling | **実Source起動・結果／capture・nested join・各step公開保存復元は対象条件で検証・公開済み** | `StartProgram`、独立kernel、accepted handle、Task root、共有registryへの結果退役を実際に通過する。元Sourceと同じadmissionで値を復元し、VM awaitは消費前にNeed／Taskを型で検査する。ローカルProgram値をHost ACK待ちから切り離した修復後、全体／entry選択lowering×Bool false／true、1-op予算、各stepの公開save／restore、task／completion各1件がPASS。generic capture／nested joinも3 backendでPASSし、最新R2843の全53逆依存runtimeでも新規失敗0。cancelと全資源境界の包括的な受入とは分ける。 |
| Native AwaitMany の Source／operand admission | **起動経路を接続し、Core owner検証済み** | baseは元Source、完全なcapture群と評価済みsourceを検査してからcaptureを複製する。childも実際のitem／indexとcaptureを検査してから一時bindingを作る。TaskSpecのclass／priority／scope／policy／outcomeは元のtemplateが所有する。別lease・別endpoint・失敗済みbankの拒否をNative／AWBCのSource carrierで確認し、Core library全1,977件がPASS。未使用になった直接生成APIは削除した。実際のAWBC launch／restoreの切替と、全14 familyの最終受入はこの結果に含めない。[検証範囲と原本](acceptance-evidence/task-await-many-native-source-acceptance.json)。 |
| AWBC AwaitMany の Source起動／save／checkpoint | **元Sourceへの接続を公開済み・owner／逆依存検証済み** | 公開 `dec74c879c651705e41d23cfcf9b6b0f8dc50f71`。base／childを元Sourceで認証し、実際のframeの型bindingとoperandからproducerを導出してからrequest kernelを実行する。digestだけで生成・照合する旧3 APIは削除した。live／inertの保持値を同じbankで検査し、元型とexpected-keyの照合でnodesを二重課金しない。公開saveとsession復元はadmitted ProductのSourceを保持し、private checkpointは同じexecutable Arcでも別Source leaseや型を保ったitem改変を拒否する。debug labelはcapture参照前に検査する。前cutの53 package選択でCore library 1,982／1,982 PASSを実行し、横断実行に新規失敗0、Core単独doctest 75／75 PASS。[初回owner検証](acceptance-evidence/task-await-many-awbc-source-acceptance.json)、[後続の横断・doctest検証](acceptance-evidence/task-await-many-awbc-reverse-combined-execution.json)。全14 family・Tier2の最終受入は別。 |
| AwaitManyのResult集約／消費前検査 | **通常・独立Program childをowner／全53逆依存で検証済み** | 元のaggregate／child型からsequence保持とResult転置を選び、同じbankで型・資源・pattern／所有権を検査してからslotを消費する。Source位置によるErr選択、空入力、payload移動、scoped Resultのowner検査を維持した。今回、実際のProgram起動・子の結果binding／return・親のawaitをNative／AWBC／各step公開save/restore付きAWBCで実行した。Nativeのイベント消失とAWBCの初期化漏れは新しい具体的な再現から修復し、通常の集約受入やGeneric Matchを未実装に戻していない。Nativeのqueued progress／Readyはprivate rollback後も成立し、progressの再通知と未来cursorを拒否する。Core全件・Clippyに加え、R2843で新しい統合テストとNative rollbackテストもPASS。[失敗・修復・原本](acceptance-evidence/task-fanout-child-acceptance.json)。全53逆依存・workspace gateの新規実行は冒頭参照。scoped genericの独立childと異なる親子generationは今回の統合fixtureでは別途実行していない。 |
| DSL `traverse(...).parallel(...)` | **意味解析・lowering・first-class Need構築は未接続** | [採用されたDSL契約](../01-language/traits-seq-ranges.md#traversable)に対し、現cutでも意味解析／lowering／lazy Needの入口は未実装。最新R2843でも未知の `traverse` で既存2件が失敗した。R2800型検査の8 PASS／1 FAIL、fixture訂正と後続のCore全件成功は[履歴](convergence-evidence-history.md#awaitmanyのresult集約と消費前検査--2026-10-25)に保持する。Core内のResult集約受入は、このDSL契約の実行受入を意味しない。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| private fiber checkpoint | **Program handleのSource復元と別lease拒否は修復・owner検証済み** | R2525で、実際に受理されたProgram handleを持つcheckpointが「enclosing admitted Sourceが必要」と失敗した。checkpointに元executable leaseを保持し、復元先は同じleaseのadmitted Productに限定する。元Sourceと同一admissionを値mapperへ渡し、Product・activation・codegenの呼出側を移行した。R2535の新規Coreケースでhandleを取り出して破棄した候補から元状態へ復元でき、同内容の別leaseは元fiberを変えず拒否する。private checkpointは非Serde、公開wire DTOのJSON／cleanup検証は維持し、R2531のcompile-fail 2件もPASS。既存のdetached Bool入力rollbackも通過している。private checkpointの受入と、公開saveの全資源境界の包括的な受入は分ける。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **最新CLI選択は641 PASS／50 FAIL・新規失敗0** | R2843の50件は前回とbinary／test識別子が完全一致。Core全target 2,044／2,044、Compiler 617／617、修復済みの4,096要素Array Mapと低予算Native-AWBC parityもPASS。[実行・旧失敗照合](acceptance-evidence/task-fanout-child-reverse-acceptance.json)、[package集計](acceptance-evidence/task-fanout-child-reverse-coverage.json)。失敗の具体的な観測は下表に集約する。 |
| Coreの全53逆依存runtime | **最新cutの全target通常選択を新規実行・既存CLI 50失敗を保持** | R2842のcatalogとR2843の全終端case集合が一致。library 53、integration 161／bin 5 targetを揃え、7,269実行／7,219 PASS／50 FAIL／39既存ignored、終了100。named caseなしのlibrary 4／bin 4 targetはbuildだけ、integration 6 targetは全件ignored。Core単独結果の再利用・重複加算0。[target／case集合](acceptance-evidence/task-fanout-child-reverse-coverage.json)、[終端・固定入力](acceptance-evidence/task-final-r2843-fanout-child-reverse-runtime.terminal-audit.json)。 |
| Workspace gate／Tier 2／Native-Web capture parity | **最新workspace check／Clippyは終了0・警告あり。Tier 2とparityの包括受入は未実施** | 同じ基点のR2840 check／R2841 Clippyをworkspace／all-targets／all-features／lockedで新規実行し、各6,199固定入力の差分0。[check照合](acceptance-evidence/task-final-r2840-fanout-child-workspace-check.input-audit.json)、[Clippy照合](acceptance-evidence/task-final-r2841-fanout-child-workspace-clippy.input-audit.json)。R2795 Core doctest 75／75 PASSは以前の入力の証拠として保持し、今回の新規実行とはしない。今回の39既存ignored、Tier 2とNative-Web capture parityの包括受入は未実行で、Generic Match等の受入済み境界の判定には影響しない。 |

## CLIの具体的な観測

最新R2843でも同じ識別子で再現した既存50件を、診断／期待値の種類で分類する。下記の診断分類は以前の原ログに基づき、今回の失敗識別子との一致を確認した。根本原因や修復方針の確定ではなく、
fixtureの現行契約との整合も含めて調べる。全識別子と診断は
[分類](acceptance-evidence/task-array-map-reverse-cli-current-failure-classification.json) と
[原診断への対応](acceptance-evidence/task-final-r2759-array-map-cli-final-runtime.failure-recon.json) に保持する。
Source上限の旧1件はArray Map修復後の前cut横断実行でもPASSした。

| 観測した境界 | 件数 | 具体的な診断・期待値 |
|---|---:|---|
| typed Presentation ABI | 9 | 画像sampleの `Background` をCompilerが「typed Presentation command ABI待ち」と拒否する。 |
| JLREQ geometry／raw crop | 8 | 後続本文が左へ進まない、hyphen後のcolumnが1169／1127に分かれるという診断を再現する。 |
| View文法／観測 | 6 | typed View grammar／action名の拒否、rich-text object／run／depthの期待値不一致。 |
| 意味解析の入口 | 7 | HIR recovery 2件、未知の `traverse` 2件、final semantic analysis 3件。 |
| SourceのSemanticWork上限 | **0（旧1件は修復検証済み）** | 要素数分のlocals／命令展開をiteratorとVec蓄積値へ置換し、exact Arrayを構築する。4,096要素fixtureは製品既定上限のままPASSし、4096 callback／1 batch等の条件を維持した。低予算Native-AWBC parity、融合・copy・capture、途中保存復元を含む対象46件がPASS。[修復と実行証拠](convergence-evidence-history.md#array-map反復化の検証--2026-10-24)。 |
| verify出力の期待値 | 6 | solver／proof／promotion／thread-join／dependency／deferのreport期待値と一致しない。 |
| bench／profile／plan／scenario等の期待値 | 14 | 計測counter、task graph、scenario/assertion、unsafe一覧等が期待値と一致しない。 |

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
