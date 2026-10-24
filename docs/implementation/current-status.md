# Arcweft 現状一覧

確認日: 2026-10-24。今回の実装・検証の基点は main `d741187573117bce556f453018593cb482f878ea`。
既存のtracked WIP 14件とuntracked 6件を保持し、変更入力を固定して検証した。
後続cutのowner実行を、以前の逆依存横断集計へ加算しない。

Array Map収束時の、Core全53逆依存packageの横断集計は
**7,254実行／7,204 PASS／50 FAIL／39既存ignored**。
library 5,474件とintegration／bin 1,780件を重複なく集約し、
全166 integration／bin targetsを照合した。
[実行集計と再利用範囲](acceptance-evidence/task-array-map-reverse-combined-execution.json)。

全targetの初回実行は公開基点 `d3f81dbda05299f6e5d5d912b25e3efad256ce3c` と
既存14 tracked WIPを保持した5,344固定入力。その後、Array Mapの旧展開を前提とする
試験1件を訂正し、CLI457件と他のCLI integration43件を再実行した。製品入力と、再利用したtest executableは差分0。
最終失敗は下表の既存CLI 50件で、追加失敗0。修復済みの4,096要素Array MapもPASS。
初回の追加失敗と訂正理由は履歴へ保持し、上限や受入条件を緩めていない。

後続のNative／AWBC AwaitMany Source接続は別cutとして扱う。
その検証範囲、共有gate、結果の再利用範囲、未実行の受入は下表に記す。

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
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | `850534b9e9bc8c8872f6528dd00050a5e6e7699e` の17入力／公開 blob と151件を確認。115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC 継続／保存が受入時の2実行とも PASS。完全 transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publication を含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。同横断集計でも151ケースがすべてPASS。[横断集計の照合](acceptance-evidence/task-array-map-reverse-generic-current-execution.json)。全体 goal の未達から、この受入を取り消さない。 |
| Collection／variant Map | **公開6 familyとArray反復化は対象条件で検証・公開済み** | Vec／Seq／Array／Slice／Option／Resultが公開catalog。Arrayのexact length、直接／retained-prefix、effect／source order、capture、途中保存復元を含む対象46件がPASSした。4,096要素入力も製品既定上限のままPASS。Need／Parser／Streamのschema候補を公開済みとはしない。[修復の検証履歴](convergence-evidence-history.md#array-map反復化の検証--2026-10-24)、[現行契約](../01-language/traits-seq-ranges.md)。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **当該 cut の受入履歴あり・CLI 統合描画に再現失敗あり** | `86958911833f2509f391b70962773236e46a124e` の library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate は PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md)、[現行契約](../03-presentation/text-typesetting.md)。同横断集計のCLI統合描画でも8件の失敗を再現した。具体的な診断は下記「CLIの具体的な観測」に集約する。既存 layout 受入と CLI 統合描画の修復を区別し、PNG golden／hash と当時の実績は保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission／scheduler | **各 cut は公開済み・Program 結果保管／完了受付は検証済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。Program の値は共通 registry が保管し、私有の起動 proof を検査した Host／Web は退役 metadata のみを公開する。R2424 で Scheduler 17件、Host の実際の退役／公開2件と終端 session 2件、Web Source 認証1件が全 PASS。[必須・既存ケース照合](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.acceptance-audit.json)。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終受入は未確認。Host request の14 variant と Need producer の10 family は別の分類。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須 program lease と、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` の callee 退役処理。キュー切替前の修復入力で R2355 は8 library／2,463 PASS。現在の通常runtime検証は下記の全53逆依存runtimeにまとめる。 |
| Native の original-source queue／runtime continuation・結果保管 | **Program の結果保管・退役は owner／consumer 検証済み** | 実際の返却 frame が値を cleanup／join／rollback 中も保持し、退役時に共通 registry の `Ready` へ移す。ローカル await が一度だけ値を取得し、`ReadyTransferred` をHost ACKで復活させない。R2424／R2426の結果を保持し、公開 `a74957e455597a0d3160af11c914e2e93f58d144` のR2469では実際のlive handleを親環境へ束縛した状態で、非Unit返却・各step後rollback・ACK・metadata再送・fallthrough失敗の3件も全PASS。公開Native saveとAWBCの受入は別項目。 |
| Thread の返り値／返却 handle | **結果受渡し・generic capture／nested joinは対象条件で検証・公開済み** | `dd7ce52715d3beda046ad465063623ece4ea5092` の結果型・Return／Try境界、`a74957e455597a0d3160af11c914e2e93f58d144` のNative CPS／accepted receipt束縛を維持する。型付きresult pattern、起動前の拒否／panic rollback、Host NeedとProgram ThreadHandleおよびpayload型の取り違え拒否も保持する。generic `T=bool`の複数回capture使用と、親return後のattached子孫joinを3 backend×false／trueで確認した。横断JSONのpackage集計では `arcweft-core`（library＋integration/bin）2,029／2,029、`arcweft-compiler` 617／617がPASS。[横断集計](acceptance-evidence/task-array-map-reverse-combined-execution.json)。Core library-onlyのfresh runと後続Native Source cutは別項目に示す。公開Native saveの対応は別項目。 |
| AWBC の独立 Thread scheduling | **実Source起動・結果／capture・nested join・各step公開保存復元は対象条件で検証・公開済み** | `StartProgram`、独立kernel、accepted handle、Task root、共有registryへの結果退役を実際に通過する。元Sourceと同じadmissionで値を復元し、VM awaitは消費前にNeed／Taskを型で検査する。ローカルProgram値をHost ACK待ちから切り離した修復後、全体／entry選択lowering×Bool false／true、1-op予算、各stepの公開save／restore、task／completion各1件がPASS。generic capture／nested joinも3 backendでPASSし、同横断集計の逆依存通常選択で新規失敗はない。cancelと全資源境界の包括的な受入は未確認であり、上記の受入条件とは分ける。 |
| Native AwaitMany の Source／operand admission | **起動経路を接続し、Core owner検証済み** | baseは元Source、完全なcapture群と評価済みsourceを検査してからcaptureを複製する。childも実際のitem／indexとcaptureを検査してから一時bindingを作る。TaskSpecのclass／priority／scope／policy／outcomeは元のtemplateが所有する。別lease・別endpoint・失敗済みbankの拒否をNative／AWBCのSource carrierで確認し、Core library全1,977件がPASS。未使用になった直接生成APIは削除した。実際のAWBC launch／restoreの切替と、全14 familyの最終受入はこの結果に含めない。[検証範囲と原本](acceptance-evidence/task-await-many-native-source-acceptance.json)。 |
| AWBC AwaitMany の Source起動／save／checkpoint | **起動・保持handle・保存復元を元Sourceへ接続し、Core owner検証済み** | base／childを元Sourceで認証し、実際のframeの型bindingとoperandからproducerを導出してからrequest kernelを実行する。行のdigestだけで生成・照合する旧3 APIは削除した。live／inertの保持値を同じbankで検査し、元型とexpected-keyの照合で値のnodesを二重課金しない。公開saveはadmitted Productを受け取り、sessionの保存・復元も同じSourceを保持するbankを使う。private checkpointは同じexecutable Arcでも別Source leaseを拒否し、型を保ったitem改変も拒否する。debug labelの上限はcapture参照前に共通APIで検査する。最終Core libraryは1,982／1,982 PASS、skip 0。逆依存53のruntime全体・Generic Match 151件・Tier2の新規実行はこのcutに含めない。[検証範囲と原本](acceptance-evidence/task-await-many-awbc-source-acceptance.json)。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| private fiber checkpoint | **Program handleのSource復元と別lease拒否は修復・owner検証済み** | R2525で、実際に受理されたProgram handleを持つcheckpointが「enclosing admitted Sourceが必要」と失敗した。checkpointに元executable leaseを保持し、復元先は同じleaseのadmitted Productに限定する。元Sourceと同一admissionを値mapperへ渡し、Product・activation・codegenの呼出側を移行した。R2535の新規Coreケースでhandleを取り出して破棄した候補から元状態へ復元でき、同内容の別leaseは元fiberを変えず拒否する。private checkpointは非Serde、公開wire DTOのJSON／cleanup検証は維持し、R2531のcompile-fail 2件もPASS。既存のdetached Bool入力rollbackも通過している。private checkpointの受入と、公開saveの全資源境界の包括的な受入は分ける。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **前回の横断通常選択は641 PASS／50 FAIL・新規失敗0** | 全失敗が既存CLI識別子と一致する。旧51件中の4,096要素Array MapはPASSし、残る50件を今回再現した。旧Compiler失敗もPASS、Compiler全617件はPASS。[実行・旧失敗との照合](acceptance-evidence/task-array-map-reverse-combined-execution.json)。診断の分類は下表に一度だけ記載する。 |
| Coreの全53逆依存runtime | **Array Map cutの全target照合を保持・直近Core libraryは全PASS** | library 53 targetとintegration 161／bin 5 targetを確認。libraryはCore library-only 1,976件の再実行と、byte hash差分0の他52バイナリ3,498件の成功を再利用した。[libraryの再利用範囲](acceptance-evidence/task-array-map-reverse-library.final-summary.json)、[integrationの終端・target照合](acceptance-evidence/task-array-map-reverse-integration-summary.json)。Array構築4件を追加し、旧展開前提の1件をconstant-inventory／exact Array長の検証へ置換した。他のcase setは差分0。named caseなしのlibrary 4／bin 4 targetはbuildだけとして扱う。 |
| 最終 workspace／Tier 2／Native-Web capture parity | **直近workspace check／ClippyはPASS・横断runtimeにCLI失敗履歴あり・Tier2は未実行** | R2788 workspace check／R2789 workspace Clippyはall-target／all-feature／lockedで終了0、警告あり。後続のCore限定のpanic・記法・fixture整理はR2790全target Clippy終了0・変更行警告0、R2792 library 1,982／1,982 PASS。workspace gate後にconsumer APIや非Coreソースは変更していない。[今回の範囲](acceptance-evidence/task-await-many-awbc-source-acceptance.json)。R2771／R2772、R2661／R2662、R2760、R2663構造gate blocking 0、R2531非Serde doctest 2件等の実績は履歴に保持する。39既存ignored、Tier2とNative-Web capture parityの包括的な受入は未実行。この未実行を、受入済みのGeneric Matchや個別runtime境界の未実装へ読み替えない。 |

## CLIの具体的な観測

同横断集計で再現した50件を、診断／期待値の種類で分類する。根本原因や修復方針の確定ではなく、
fixtureの現行契約との整合も含めて調べる。全識別子と診断は
[分類](acceptance-evidence/task-array-map-reverse-cli-current-failure-classification.json) と
[原診断への対応](acceptance-evidence/task-final-r2759-array-map-cli-final-runtime.failure-recon.json) に保持する。
Source上限の旧1件は修復後の今回実行でもPASSした。

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
