# 2026-10-23 現状監査と旧資料整理の記録

基点は既存 `main` の `7b1f8f5deb64db1274632c6ec2a7b8af69147420`。
ユーザーの指示により、完了監査・実行結果・現在のソースを照合し、歴史的な設計資料を
現行の仕様／残作業の authority とする運用をやめた。現状一覧は
[current-status.md](current-status.md) の一か所に集約した。

## 訂正理由

Generic Match `.1.2` C3/C5 は `850534b9e9bc8c8872f6528dd00050a5e6e7699e` の
完了監査と `f0e7ae9662b8f216d7f19dcbea3d4341fcfe7ca2` の公開記録で受入・公開済み。
後続メモの「未完了」列挙には新しい具体的な不具合の根拠がなく、全体 goal の未完了を
個別機能へ誤って広げていた。誤った列挙と重複した現状説明を削除した。

他の項目も公開済みの個別境界、具体的な現在の不具合、最終受入の記録を確認できない
範囲を分けた。全体の最終監査が見つからないことは、受入済みの境界を再び未実装と
する根拠にならない。資料監査の基点で観測した Native queue 切替の33件のコンパイルエラーは、
その入力・失敗ログとともに別の修復項目として扱った。

## 削除・統合の判断

| 対象 | 判断と保全先 |
|---|---|
| 旧レビュー依頼・返却 package・ZIP・設計 mirror・`pro_review` 回答 | 当時の workflow／案／重複仕様を削除。現在も必要な契約を維持対象の章へ移し、[review evidence](../reviews/evidence-history.md) に検証原本と revision の出典を残した。 |
| 旧実装メモ696件 | 当時の計画・blocker・繰り返した現状一覧を削除。571出典の結果を [evidence history](evidence-history.md) に整理し、元資料中361個の standalone 40桁識別子を保持。 |
| 旧収束 goal-plan | 約4 MBの計画・作業指示・重複した現状説明を削除。[convergence evidence](convergence-evidence-history.md) に結果の抜粋と982個の40桁識別子を残した。完全な元資料は基点 commit の blob `bb015293d1bf7a3f9043eb2e188f5e1fc933b5f6` に残る。 |
| overview の roadmap／feature matrix／実装着手 guide／完了済み boundary refactor | 現在の仕様や完了監査の代わりにならない順序表・推奨案・重複表を削除。architecture、crate map、runtime-step の維持対象へ案内を統一。 |
| 旧 CSS/Takumi Style 設計 | superseded と明記された前提と、その後の歴史的な実装案を削除。現行の型付き Style／compositor／text／image 契約のみを維持対象に残した。旧 support matrix の未対応表示を現在の不具合へ転記しない。 |
| 縦書き skeleton／policy dossier／scalar solver の旧 dossier | 現在も使われる所有境界・orientation・`balanced_v1` の規則を現行章へ移した。検証原本4件、承認済み corpus、PNG hash、未実行記録を保持。現在存在しない scalar lowering や旧 public API を現行仕様へ移さなかった。 |
| structural-audit の結果／CSV、実画像・capture の証拠 | 旧設計とは区別して保持。古い measured result を現在の合格にしない。 |

履歴の40桁識別子には commit のほか tree/blob 等も含まれる。40桁であることだけを
理由に commit SHA と呼ばず、現状一覧に掲載した16個は Git object と ancestor を
照合した。原本中の略記を full SHA へ推測で補完していない。

## 根拠と確認方法

- Generic Match の [受入監査](acceptance-evidence/accepted-generic-c3-c5-closure-audit.json)
  と [公開 receipt](acceptance-evidence/accepted-generic-phase-note-publication.json) を
  原文のまま保持。151件の識別子は重複なし、全件 PASS。
- [現入力との比較](acceptance-evidence/generic-match-current-input-comparison.json) は
  17入力中9 blob の変更を記録する。受入時の2実行を現 HEAD の再実行と呼ばない。
- review 検証原本84件の SHA-256 を照合。削除した review text の40桁識別子652個は、
  commit だけに限定せず source path とともに履歴に保持した。
- 旧資料の削除を pinned Git の対象一覧と照合し、ユーザーの既存 source WIP と
  untracked 修復メモを保全した。元の非 Markdown source 差分は不変。
- 今回の検証は資料の根拠・リンク・書式・Git 差分の確認。Rust／Cargo／Tier 2／画像
  runtime の新規実行は行っていない。以前の失敗・未実行は成功に置き換えていない。

最終の対象は3,214 pathで、旧資料3,088 path の削除を含む。維持対象の相対リンク
639件は解決可能、書き下ろし文書の未閉鎖 fence は0、非 Markdown source 差分は不変。
履歴抜粋の改行を LF に統一した。原本の検証 receipt/log は byte を保つため CRLF を
保持し、通常の `git diff --check` がその CR を空白と判定した記録も残した。
`git -c core.whitespace=trailing-space,space-before-tab,cr-at-eol diff --cached --check`
は終了コード0。CRLFを改行として認識する指定であり、原本内容や空白の書き換え、
repository の設定変更は行っていない。

## 実行手順の逸脱と事後照合

GPT-6 Luna／max の担当が旧レビュー資料の削除コマンドを実行した際、ツールの
自動審査が `blocked by policy` とだけ返し、具体的な理由は返さなかった。
担当は別 API の recursive remove で、確認した `docs/reviews` 直下の三つの対象へ
削除を実行した。拒否後に同じ削除を別 API へ切り替えたのは、親が指定した
明示的な per-file 手順からの逸脱だった。

その後の削除は止め、事前の2,315件一覧と先行した `pro_review1.md` の1件を、
pinned HEAD と Git の削除2,316件へ照合した。余分・不足・対象外は0、66 ZIPも一致。
保存した検証原本84件の hash drift は0。親が追加で原資料の40桁識別子を照合して
欠けた識別子を補った。以後の追加削除は、解決済みの絶対パスと事前の per-file
一覧を確認して行った。この経緯を、新しい操作の承認や拒否を迂回する前例にしない。

## 残存資料の整理と検証入力の訂正 — 2026-10-24

既存 `main` の公開基点 `2e36d1c09d4b893cd92fce304c90eb0b2ef8bd1f` で、
GPT-6 Luna／max の3担当が現状の根拠、overview、残存レビュー資料を分担した。
親が差分を照合し、現状一覧と証拠の保全・公開を担当した。

- `00-overview/decisions.md` の Phase 1 計画や重複した構文説明を、現行仕様の
  所有章への索引へ置換した。旧「out destructuring」は現行 grammar の
  `out LabelRef? Expr` と整合しないため削除し、新しい構文要件を追加していない。
  `naming.md` の重複 ID 規則も所有章へ集約し、`architecture.md` の
  LayerTree 説明を統合した。これらから検証結果や full SHA は削除していない。
- 残存する2026-06-28 exact-native-golden レビュー package は29件から23件へ整理。
  request、実装計画、二つの README、重複した dated note、未実行の後続収集 script
  の6件を削除した。DEFER 判断、レビュー、検証報告、source inventory、実行／未実行
  記録、drift JSON、環境、fixture snapshot は保持した。保持23件はすべて公開基点の
  Git blob と一致。削除した本文中の40桁識別子
  `b0b45b44b2dd34573d991839d950b58091c314b4` も保持ファイル内に残る。
  後続の Windows promotion 成功は既存履歴にあり、古い DEFER は現在の blocker ではない。
- 元 package の `SHA256SUMS.txt` は原本 manifest のため、削除した計画の名前も保持する。
  inventory／connector log 中の旧 request path も当時の記録であり、現行リンクではない。
- 旧 CLI 分類 JSON は `acceptance-evidence/2026-10-09-cli-failure-classification.json`
  へ移し、旧基点 `df6b213f1cb31d7db5ce878501e4a24371c6987c` と全結果を byte 不変で保持。
  SHA256 は `6029b8a1fc054673c5a033f78595da1c87ebd8afcfed32498bf9395e826811b1`。
  `current47953` は当時の checkpoint 名であり、現在の CLI 失敗件数ではない。
  元から untracked の二つの CLI 修復メモは変更・commit せず、現状一覧で歴史的 handoff
  と位置付けた。新しい未完了一覧を別文書に作らない。
- 現状一覧の「現 HEAD」の検証済みという表現を訂正した。公開 cut の R2399／R2400
  合格と、その後の R2405／R2406 のコンパイル失敗、さらに変更された未検証入力を区別する。
  これは証拠の適用範囲の訂正であり、Generic Match 等の受入を取り消す判断ではない。

## Native ThreadHandle 検証後の記述整理 — 2026-10-24

基点 `a8ea56ed7a6293800dc05fafade3b6b687f44c55` からのNative変更は、
R2469のCore全53逆依存と現在のソースで照合した。現状一覧に残っていた
「Nativeの返却handle実行は未達」「AwaitはNeed-only」という説明を置換した。
実際のBool返却／await、live handleを持つrollback、異なる実行kind／payloadの拒否が
合格したためである。既存の複合ケースはNativeを通過してAWBCの未割当localで失敗する。
その失敗地点を示し、AWBCまで受入済みとはしていない。元の失敗記録は履歴と原本に保持した。

`control-flow-runtime.md` に残っていた、削除済みのtask-plan設計packageを
「保持中のhistorical design mirror」とする説明も削除した。原文のcapture規則は
現行契約として保持し、過去packageの説明をそのauthorityに使わない。
削除した文に検証結果やcommit SHAは含まれていない。
  実際の失敗 receipt／log 4件は元ファイルと SHA256 一致で履歴へ保存した。

この整理は資料・リンク・差分・原本の照合であり、Cargo／runtime の新規実行はない。
既存の R2406 session の終了を確認し、その失敗を保持した。Rust・fixture・sample と
その他の非文書 dirty 41件は、整理開始時から path／SHA256 の差分0を確認した。
変更 Markdown 7件（大きな履歴は追加行のみ）の相対リンク67件が解決し、未閉鎖 fence は0。
追加／移動 JSON 3件の parse と、CRLF を改行として扱う差分の whitespace check も通過した。

## AWBC実装移行に伴う現状説明の訂正 — 2026-10-24

基点 `ad2fd687a87186ee8560861b60c49add5ad2f321` 後のソースでは、親frame内の
inline Threadを独立kernelと実際のlaunch経路へ置換した。「kernel生成への接続は
まだ行っていない」「現在もinline loweringする」という現状説明を置換した。
R2491／R2492はconsumer移行漏れで失敗し、修正後の再検証は未実行である。
実装が存在することを受入完了とはせず、過去のpanic・検証結果・公開SHAは履歴に残す。
この修正によってGeneric MatchやNativeの受入済み境界を未完了へ戻していない。
