# Arcweft 現状一覧

確認日: 2026-10-25。今回の照合基点は公開main `c985b52c024f049113be24933e683428920e53b6`。
この基点からのCore 11 source path変更はR2926で全2,074件を検証した。
最新の終了済み横断実行R2908は、前段のCore公開 `4a626c6fd1ef3c43a400b983fc118623d689b18f`
と当時の保持WIPからなる3,980非文書入力の結果であり、新しいCore変更の横断成功には転用しない。
新入力の全53逆依存はR2928 catalogを構築中で、runtimeの終端は未確認。
元のtracked WIP 14件／untracked実ファイル6件を保持している。
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
| Generic Match `.1.2` C3/C5 | **受入・公開済み** | 受入 `850534b9e9bc8c8872f6528dd00050a5e6e7699e`、公開 `f0e7ae9662b8f216d7f19dcbea3d4341fcfe7ca2`。17入力／公開blobと151件を受入時の2実行で確認し、115 transcript・24 coverage・5公開 query・7 Native/decoded-AWBC継続／保存がPASS。完全transcript、uninhabited-domain、Never／let-else、exhaustive-only atomic publicationを含む。[受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)、[公開記録](acceptance-evidence/accepted-generic-phase-note-publication.json)。最新R2908でも151／151 PASS。[全151件の終端照合](acceptance-evidence/task-presentation-owner-reverse-acceptance.json)。受入を再開する新しい不具合はなく、全体goalの未達から受入を取り消さない。 |
| Collection／variant Map | **公開6 familyとArray反復化は対象条件で検証・公開済み** | Vec／Seq／Array／Slice／Option／Resultが公開catalog。Arrayのexact length、直接／retained-prefix、effect／source order、capture、途中保存復元を含む対象46件がPASSした。4,096要素入力も製品既定上限のままPASS。Need／Parser／Streamのschema候補を公開済みとはしない。[修復の検証履歴](convergence-evidence-history.md#array-map反復化の検証--2026-10-24)、[現行契約](../01-language/traits-seq-ranges.md)。 |
| Presentation引数schema／resource型 | **引数role・resource型移行は検証・公開済み** | `0b5ac5491d245a5386ca1b44547091e50bcb1955`。全11 callableの宣言済みparameterに型付きroleを保持し、UncheckedSupplyもconsumerをversion 1 transcriptへ含める。Character固有lookの型、異なるtarget／metadata role、別familyでのconsumer拒否を対象4件と当時の全22逆依存で検証した。[roleのowner検証](acceptance-evidence/task-presentation-role-owner-acceptance.json)、[当時の横断結果](acceptance-evidence/task-presentation-role-reverse-acceptance.json)。後続の `fb04fd878622fac57a22db7e2c005ee6daa829ea` でresource kindと再帰型の `Handle<T>`／`SlotRef<T>`を36 pathで共有化し、対象13／13とR2864の全62逆依存で新規失敗0。[型モデルの検証](acceptance-evidence/task-presentation-handle-owner-acceptance.json)。実行ABIの受入は次の行と区別する。 |
| Core Presentation token／custody・command ABI | **token・ledger・scope grant・Engine所有・Native cleanup／private rollbackは公開済み。Handle保持グラフとbody移動は追加Core検証済み、Source／command ABIは未接続** | `9038c82a7a0aff5650bc635d74ff21f7e7cba69e` までのscope grantと、後続Engine所有の受入を維持する。今回、元ledger・回収待ちheaderと全保持値のHandleを同じ資源visitorで照合し、重複・未発行／解放済みtokenを拒否した。body登録も既存のlive frame・継続・rootと候補を同じbankで検査してから発行し、失敗時は候補を返して既存状態を保持する。scopeからdetachしたleaseには一意な保持Handleが必要で、bank再開は観測済み集合を保持する。R2926は新規4件を含む **2,074／2,074 PASS**、3,981非文書入力・13実行ファイルの差分0。[追加検証の範囲](acceptance-evidence/task-presentation-custody-acceptance.json)、[終端照合](acceptance-evidence/task-final-r2926-presentation-custody-rejection-core-runtime.terminal-audit.json)。これは実際のtyped frame／body capture／private rollbackの検証であり、Source命令のschema認証、Dialogue activationの独立bank、AWBC ledger、command lowering／driver接続の受入ではない。CompilerのPresentation候補と一般Handleのruntime投影は現在も拒否する。 |
| Choice の authored source/index 関係 | **当該境界は完了・公開済み** | `5931e15eb9bf92c8b849216406687c44e348497b` を基点とする閉包記録。owner 13・compiler 1・RAG 1、reverse22 は3,930/3,930。動的な Choice の task lifecycle／scheduling は、この静的関係の受入とは別。 |
| Dense numeric の単一 authority | **当該 cut は完了・公開済み** | `01af81001e1a7a2ac7ed70e2846371ad1cdb3c08`。reverse53 は5,977 PASS／88 FAIL／39既存除外、追加の fixture leaf 9件 PASS。閉じた authority の受入と、保持された CLI ingress・画像 parity の失敗を混同しない。 |
| 縦書きの共通 layout／`balanced_v1` | **当該 cut の受入履歴あり・CLI 統合描画に再現失敗あり** | `86958911833f2509f391b70962773236e46a124e` の library 24・document 12・corpus 3・glyphon 6件と当時の統合 gate は PASS。[検証原本](acceptance-evidence/vertical-break-validation-2026-07-14.md)、[現行契約](../03-presentation/text-typesetting.md)。同横断集計のCLI統合描画でも8件の失敗を再現した。具体的な診断は下記「CLIの具体的な観測」に集約する。既存 layout 受入と CLI 統合描画の修復を区別し、PNG golden／hash と当時の実績は保持する。 |
| retained View の callable-owner 認証 | **当該境界は完了・公開済み** | `0403def0b856ddbb4ab19b43e822d0bcc8de1ce7`。479件の重複を除いた PASS と6共有 gate の記録。View `.1.4` 全体、task capability、別の operation の受入まで一括完了とする証拠ではない。 |
| retained View の operation／slot／capture 全体 | **実装済みの範囲あり・全体の最終受入は未確認** | compiler の Match／conditional／repeat／local lowering、mount cache／snapshot、上記 owner 認証、後続 Source/resource/restore の実装・検証履歴がある。R1661 は対象2,597件 PASS。`.1.4` 全体の最終受入記録を今回確認できていないという範囲であり、これらの完了 cut を再び未実装に戻さない。現行仕様に必須と確認できない source-shape 制限を、新規不具合とも断定しない。 |
| Need／TaskPlan の共通 seal・Source admission／scheduler | **各 cut は公開済み・Program 結果保管／完了受付は検証済み・全14の最終受入は未確認** | sealed `RuntimeTaskPlanTable`／非Serde `RuntimeTaskPlanRef`、元 program の Source lease と expected-key lookup がある。`748c8da92b6d7c5a2f9bddbac068f4b5948d0aa1` の Program capture role は R2275、8 owner／2,451 PASS。Program の値は共通 registry が保管し、私有の起動 proof を検査した Host／Web は退役 metadata のみを公開する。R2424 で Scheduler 17件、Host の実際の退役／公開2件と終端 session 2件、Web Source 認証1件が全 PASS。[必須・既存ケース照合](acceptance-evidence/task-final-r2424-program-state-core-reverse-runtime.acceptance-audit.json)。全 producer/builder/decoder/consumer の F/Q/C/E14・private-codec・View capability をまとめた最終受入は未確認。Host request の14 variant と Need producer の10 family は別の分類。 |
| 構造的 nominal | **複数の境界は公開済み・最終 C6 監査は未確認** | record construction `c4ec6d1109ca37b222f7238fd345b3a17caea612`、snapshot/program authority `faff95041f0d36bd492918cce57bc65c0ef428e2`、private Variant carrier `1c994218a25444e14b58e1111f65581f8bbdd73a`、bounded task-image source `4c8844d70baf81ae7bb031766360738050875b35` が公開されている。最後の cut は Core 1,497 PASS、逆依存実行は6,633 PASS／57保持された FAIL。C1–C6 を一括して未実装とはしない。ownership-success／旧経路削除までの最終 C6 受入を確認できていないことと、個別境界の具体的な不具合を区別する。 |
| 元 executable lease・Native root/callee 座標 | **当該修復は公開・検証済み** | `6a86816246cd8e1bfae8f8250324cdbf0a2c559a` の必須program leaseと、`1be0eb4d09a958b099225a1c7fbbd506afe39d26` のcallee退役処理。キュー切替前の修復入力でR2355は8 library／2,463 PASS。前段R2843でも53逆依存の通常選択に新規失敗0。全体goalの未達を、この修復の未実装へ読み替えない。 |
| Native の original-source queue／runtime continuation・結果保管 | **Program の結果保管・退役は owner／consumer 検証済み** | 実際の返却 frame が値を cleanup／join／rollback 中も保持し、退役時に共通 registry の `Ready` へ移す。ローカル await が一度だけ値を取得し、`ReadyTransferred` をHost ACKで復活させない。R2424／R2426の結果を保持し、公開 `a74957e455597a0d3160af11c914e2e93f58d144` のR2469では実際のlive handleを親環境へ束縛した状態で、非Unit返却・各step後rollback・ACK・metadata再送・fallthrough失敗の3件も全PASS。公開Native saveとAWBCの受入は別項目。 |
| Thread の返り値／返却 handle | **結果受渡し・generic capture／nested joinは対象条件で検証・公開済み** | `dd7ce52715d3beda046ad465063623ece4ea5092` の結果型・Return／Try境界、`a74957e455597a0d3160af11c914e2e93f58d144` のNative CPS／accepted receipt束縛を維持する。型付きresult pattern、起動前の拒否／panic rollback、Host NeedとProgram ThreadHandleおよびpayload型の取り違え拒否も保持する。generic `T=bool`の複数回capture使用と、親return後のattached子孫joinを3 backend×false／trueで確認した。最新R2908でも当該受入ケースはPASS。[package集計と失敗集合](acceptance-evidence/task-presentation-owner-reverse-acceptance.json)。公開Native saveの対応は別項目。 |
| AWBC の独立 Thread scheduling | **実Source起動・結果／capture・nested join・各step公開保存復元は対象条件で検証・公開済み** | `StartProgram`、独立kernel、accepted handle、Task root、共有registryへの結果退役を実際に通過する。元Sourceと同じadmissionで値を復元し、VM awaitは消費前にNeed／Taskを型で検査する。ローカルProgram値をHost ACK待ちから切り離した修復後、全体／entry選択lowering×Bool false／true、1-op予算、各stepの公開save／restore、task／completion各1件がPASS。generic capture／nested joinも3 backendでPASSし、前段R2843の全53逆依存runtimeでも新規失敗0。cancelと全資源境界の包括的な受入とは分ける。 |
| Native AwaitMany の Source／operand admission | **起動経路を接続し、Core owner検証済み** | baseは元Source、完全なcapture群と評価済みsourceを検査してからcaptureを複製する。childも実際のitem／indexとcaptureを検査してから一時bindingを作る。TaskSpecのclass／priority／scope／policy／outcomeは元のtemplateが所有する。別lease・別endpoint・失敗済みbankの拒否をNative／AWBCのSource carrierで確認し、Core library全1,977件がPASS。未使用になった直接生成APIは削除した。実際のAWBC launch／restoreの切替と、全14 familyの最終受入はこの結果に含めない。[検証範囲と原本](acceptance-evidence/task-await-many-native-source-acceptance.json)。 |
| AWBC AwaitMany の Source起動／save／checkpoint | **元Sourceへの接続を公開済み・owner／逆依存検証済み** | 公開 `dec74c879c651705e41d23cfcf9b6b0f8dc50f71`。base／childを元Sourceで認証し、実際のframeの型bindingとoperandからproducerを導出してからrequest kernelを実行する。digestだけで生成・照合する旧3 APIは削除した。live／inertの保持値を同じbankで検査し、元型とexpected-keyの照合でnodesを二重課金しない。公開saveとsession復元はadmitted ProductのSourceを保持し、private checkpointは同じexecutable Arcでも別Source leaseや型を保ったitem改変を拒否する。debug labelはcapture参照前に検査する。前cutの53 package選択でCore library 1,982／1,982 PASSを実行し、横断実行に新規失敗0、Core単独doctest 75／75 PASS。[初回owner検証](acceptance-evidence/task-await-many-awbc-source-acceptance.json)、[後続の横断・doctest検証](acceptance-evidence/task-await-many-awbc-reverse-combined-execution.json)。全14 family・Tier2の最終受入は別。 |
| AwaitManyのResult集約／消費前検査 | **通常・独立Program childをowner／全53逆依存で検証済み** | 元のaggregate／child型からsequence保持とResult転置を選び、同じbankで型・資源・pattern／所有権を検査してからslotを消費する。Source位置によるErr選択、空入力、payload移動、scoped Resultのowner検査を維持した。前段のCore cutで、実際のProgram起動・子の結果binding／return・親のawaitをNative／AWBC／各step公開save/restore付きAWBCで実行した。Nativeのイベント消失とAWBCの初期化漏れは新しい具体的な再現から修復し、通常の集約受入やGeneric Matchを未実装に戻していない。Nativeのqueued progress／Readyはprivate rollback後も成立し、progressの再通知と未来cursorを拒否する。Core全件・Clippyに加え、R2843で新しい統合テストとNative rollbackテストもPASS。[失敗・修復・原本](acceptance-evidence/task-fanout-child-acceptance.json)。後続R2864の全62逆依存でも当該ケースはPASS。横断範囲とworkspace gateは各行を参照。scoped genericの独立childと異なる親子generationは今回の統合fixtureでは別途実行していない。 |
| DSL `traverse(...).parallel(...)` | **意味解析・lowering・first-class Need構築は未接続** | [採用されたDSL契約](../01-language/traits-seq-ranges.md#traversable)に対し、現cutでも意味解析／lowering／lazy Needの入口は未実装。最新R2908でも既存2件が同じ識別子で失敗した。R2800型検査の8 PASS／1 FAIL、fixture訂正と後続のCore全件成功は[履歴](convergence-evidence-history.md#awaitmanyのresult集約と消費前検査--2026-10-25)に保持する。Core内のResult集約受入は、このDSL契約の実行受入を意味しない。 |
| 公開 executor save | **バックエンドごとに異なる** | `crates/arcweft-core/src/executor.rs` の `snapshot_for_save` は Product AWBC を扱う一方、RuntimePlanVm／StructuredAot は `UnsupportedTier` を返す。Product の既存 save/restore の成功は保持し、Native 公開 save の未対応を別項目として扱う。 |
| private fiber checkpoint | **Program handleのSource復元と別lease拒否は修復・owner検証済み** | R2525で、実際に受理されたProgram handleを持つcheckpointが「enclosing admitted Sourceが必要」と失敗した。checkpointに元executable leaseを保持し、復元先は同じleaseのadmitted Productに限定する。元Sourceと同一admissionを値mapperへ渡し、Product・activation・codegenの呼出側を移行した。R2535の新規Coreケースでhandleを取り出して破棄した候補から元状態へ復元でき、同内容の別leaseは元fiberを変えず拒否する。private checkpointは非Serde、公開wire DTOのJSON／cleanup検証は維持し、R2531のcompile-fail 2件もPASS。既存のdetached Bool入力rollbackも通過している。private checkpointの受入と、公開saveの全資源境界の包括的な受入は分ける。 |
| Avro container の決定性・共通出力 | **当該実装 cut は公開・検証済み** | `80e4845d9a23d097a4e60b6a46071c3d2545da93`、lint 整理 `3c7bae39d31250647811a4e2c0b91a08dfd99374`。4 owner の520件と対象33件は PASS。全20 Avro 逆依存／以前の Data 71逆依存の最終受入と、他形式の資源上限はこの結果から推定しない。 |
| その他の resource limit／data codec | **owner ごとの検証実績を保持・横断最終受入は未確認** | 値・type/source・frame/session admission や形式別 codec の検証記録を履歴に保持する。Avro の結果を JSON／Raw／Arrow／Parquet に転用せず、全体 goal に含まれることだけで個別の上限を未実装にも戻さない。新しい未達を挙げる際は形式・入力・上限・期待拒否点と実測結果を示す。 |
| CLI・全 consumer の統合受入 | **最新CLIは640 PASS／51 FAIL。追加の起動待ち1件は同条件の単独再確認で非再現** | R2908では従来のCLI失敗50識別子がすべて再現し、stdio fixtureのREADY待ち失敗が1件加わった。Core 2,070／2,070、Sema 1,334／1,334、runtime-driver 177／177がPASS。Compilerは616 PASS／1 FAILで、拒否診断の型名表記を期待値2行で訂正した後、R2914の対象1件（compile-fail fixture 8例を含む）はPASS。[全package・失敗集合](acceptance-evidence/task-presentation-owner-reverse-acceptance.json)。追加2件の具体的な観測は下表に記す。単独再確認を全体再実行として合算しない。 |
| Coreの全53逆依存runtime | **R2908終端：7,310実行／7,258 PASS／52 FAIL／39 ignored** | Cargo逆依存53 packageの全target／全feature／locked、終了100、1,836.569秒。221 test binariesと6 non-test binariesを構築し、named caseは50 packageで選択、`arcweft`／`arcweft-bundle-assets`／`arcweft-render-web` はbuildのみ。catalogと終端case集合が一致し、3,980非文書入力・227実行ファイルのhash差分0。前回失敗50件に追加識別子2件。[終端照合](acceptance-evidence/task-final-r2908-presentation-owner-reverse-runtime.terminal-audit.json)、[範囲・package集計・Generic 151件](acceptance-evidence/task-presentation-owner-reverse-acceptance.json)。前段の共有型全62逆依存R2864は別入力の履歴であり、この53 packageの件数へ加算しない。 |
| Workspace gate／Tier 2／Native-Web capture parity | **Handle custody追加後のworkspace check／Clippyは終了0・警告あり。包括受入は未確認** | R2922 check／R2923 Clippyはworkspace／all-targets／all-features／lockedで終了0。その後の差分はCoreのcfg(test) assertion 2箇所に限り、最終入力のR2927 Core Clippy全targetも終了0・警告あり。R2924構造gateは3,219 files／97 packages、blocking 0・review trigger 378。[範囲・再利用の限界](acceptance-evidence/task-presentation-custody-acceptance.json)、[所有境界](acceptance-evidence/task-presentation-custody-ownership-review.json)。新入力の全53逆依存runtimeはまだ終端未確認。過去のCore doctest等を新入力の実行へ転用せず、Tier2／Native-Web capture parityの包括受入とも分ける。 |

## CLIの具体的な観測

最新R2908でも同じ識別子で再現した既存50件を、診断／期待値の種類で分類する。下記の診断分類は以前の原ログに基づき、今回の失敗識別子との一致を確認した。根本原因や修復方針の確定ではなく、
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
| stdio fixtureの起動待ち（R2908で追加） | 1 | `stdio_transport_times_out_and_retains_bounded_stderr_tail` がrequest送信前のREADY待ち2秒で失敗。子processは未終了、stderr空だった。同じ実行ファイル・入力・2秒deadline／250ms request timeout／32byte tailのR2912は1／1 PASS。単独で再現しなかった事実だけを記し、負荷等を原因と断定しない。[同条件再確認](acceptance-evidence/task-presentation-owner-stdio-readiness-recheck.json)。 |

R2908のCompiler追加失敗は `removed_compiler_apis_are_unavailable`。
`runtime_capability_policy_absent.stderr` のvariant名が完全修飾表記と一致しなかったため、
期待値の2行を訂正した。元のfixtureとE0560／E0559の拒否条件は保持し、同じCompiler実行ファイルの
R2914は1／1 PASS（compile-fail 8例）。[変更と再確認の範囲](acceptance-evidence/task-presentation-diagnostic-recheck.json)。
R2908の52 FAILを後続成功で書き換えず、Generic Matchその他の受入取消しには結び付けない。

旧ロードマップ、着手前の実装ガイド、重複 feature matrix、完了した runtime-boundary
移行手順、旧レビュー依頼・設計 mirror は、現行契約への整理と検証履歴の保全後に削除した。
削除理由と保全方法は [資料整理記録](documentation-cleanup.md) を参照する。
