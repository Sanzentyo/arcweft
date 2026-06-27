# AGENTS.md 方針準拠監査メモ - 2026-06-27

このメモは、GitHub コネクターで `Sanzentyo/arcweft` の最新方針文書と検索可能な実装を読み、AGENTS.md に反していそうな実装を漏れが後から分かる形で記録したものです。

## ステータス凡例

- ✅: この監査で確認済み。
- ⚠️: 方針違反候補。即修正または追加分類が必要。
- ❌: この監査では未達。後続作業で必ず潰すこと。
- 🟦: 現時点では許容と判断。ただし再利用・拡張時に注意。

## 監査制約 / 未達を後で見落とさないための明示

- ❌ ローカル checkout は未達。`git clone` は環境の名前解決制限で失敗したため、GitHub コネクター経由の個別 fetch / code search に限定した。
- ❌ 完全な repository tree walk は未達。GitHub コネクターでは root directory の再帰 listing を取得できず、検索語ベースの総当たりになった。
- ❌ `cargo fmt` / `cargo check` / `cargo clippy` / `tools/structure-audit.rs` の新規実行は未達。前回実装メモにある 2026-06-27 時点の検証成功記録は参照したが、この監査では再実行していない。
- ❌ `#[allow]` は検索結果が多く、全件の正当性分類までは未達。下の A1 / F1 に follow-up を固定した。
- ⚠️ GitHub code search の結果は検索 index の commit 表示に依存する。個別 `fetch_file` は default branch を読んだが、検索結果と fetch 結果の厳密な同一 ref 固定は未達。

## 読み込み・方針把握チェックリスト

- ✅ `Rust Skill.txt` を全文確認した。反映観点: `unsafe` / `Box::leak` / `mem::forget` 回避、安易な `#[allow]` 回避、unstable feature の慎重化、`mod.rs` 回避、公開 API / `lib.rs` 設計、clippy/fmt、serde/cpal ルール。
- ✅ `AGENTS.md` を最新取得して確認した。重要方針: crate boundary、Sans I/O、typed API、ad hoc helper / conversion の抑制、Arcweft-owned enum/type に振る舞いを足す、互換 shim 非推奨、構造監査、docs 更新。
- ✅ `docs/README.md`、`docs/00-overview/architecture.md`、`docs/00-overview/decisions.md`、`docs/00-overview/runtime-boundary-refactor.md`、`docs/00-overview/crate-map.md` を確認した。
- ✅ root `Cargo.toml` を確認した。workspace は `edition = 2024` / `rust-version = 1.96`、workspace lint は `unsafe_code = "forbid"`。
- ✅ 2026-06-27 の product catalog resource codec 実装メモを確認した。同メモでは `cargo fmt`、bundle tests、`cargo check`、`cargo clippy`、`tools/structure-audit.rs`、`git diff --check` が通っている記録がある。

## 総当たり検索観点チェックリスト

### A0: 方針文書 / 実装状態文書

- ✅ `AGENTS.md`、architecture、crate-map、runtime-boundary-refactor、seq-02.3/02.4 implementation note を確認。
- ✅ 実装状態を置く場所は `docs/implementation/` が妥当。このファイルも同配下に作成。

### A1: `unsafe` / leak / forget / unstable / allow

- ✅ 検索語: `unsafe`, `unsafe fn`, `unsafe_code`, `Box::leak`, `std::mem::forget`, `mem::forget`, `#[allow`。
- ✅ `Box::leak` / `mem::forget` は production code hit を確認できず、docs audit だけに見えた。
- ✅ root workspace は `unsafe_code = "forbid"`。
- ⚠️ `#[allow]` は production code に複数 hit。例: `crates/arcweft-core/src/audio.rs`, `crates/arcweft-core/src/engine/audio.rs`, `crates/arcweft-runtime-codegen/src/artifact.rs`, `crates/arcweft-agent-protocol/src/serde_helpers.rs`, `crates/arcweft-audio-core/src/graph.rs`, `crates/arcweft-lang-hir/src/lower.rs`, `crates/arcweft-core/src/awbc/vm.rs`, `crates/arcweft-runtime-driver/src/display.rs`, `crates/arcweft-render-native/src/renderer.rs` など。
- ❌ `#[allow]` の全件分類は未達。F1 として修正手順を固定。

### A2: `mod.rs` / public API / re-export

- ✅ 検索語: `mod.rs`, `pub use`。
- ✅ `mod.rs` は production path としては code search に見えず、docs / audit CSV 由来に見えた。
- 🟦 `crates/arcweft/src/lib.rs` の `pub use arcweft_*::*` は namespaced facade crate 内に閉じており、AGENTS.md の facade 方針と一致。
- ❌ 完全 tree walk ではないため、実ファイル名 `mod.rs` の全件不存在確認は未達。

### A3: crate boundary / Sans I/O / backend dependency

- ✅ 検索語: `std::fs`, `std::time`, `ureq`, `cranelift`。
- 🟦 `std::fs` hit は CLI / project-loader / native desktop / runtime-host / tools / tests が中心で、明白な `arcweft-core` / syntax / data-format crate 違反は検索上確認できなかった。
- 🟦 `ureq` hit は `arcweft-project-loader` と workspace dependency に見え、core への混入は確認できなかった。
- 🟦 `cranelift` hit は概ね `arcweft-lang-jit-cranelift` と docs / CLI jit に見え、core dependency への混入は確認できなかった。
- ⚠️ `crates/arcweft-core/Cargo.toml` に `render-wgpu`, `ui-servo`, `audio`, `capture-camera`, `usb`, `mcp` の空 feature がある。依存は空でも、core が adapter/domain feature 名を所有しているように見える。F2 で修正候補化。
- ⚠️ `crates/arcweft-core/src/lib.rs` が `pub mod audio;` を公開し、`arcweft-core/src/audio.rs` と `engine/audio.rs` が audio/capture command 評価を持つ。現状は typed request data / Sans I/O と見えるが、AGENTS.md の「core は audio / camera / USB / MCP / OS adapter に依存しない」方針との境界確認が必要。F3 で分離案を固定。

### A4: parser / CST / ad hoc split

- ✅ 検索語: `split_top_level`, `@bg`, `@show`, `collect_logical_block_items`。
- ✅ 旧 `@bg` / `@show` は docs と `spec_should_fail` fixture が中心で、production parser に旧コマンド受理が残っているとは確認できなかった。
- ⚠️ `split_top_level` 系は `arcweft-lang-syntax` 内に多数残存。多くは `crate::cst` helper に集約されており即違反ではないが、AGENTS.md / crate-map の「これ以上 ad hoc parser を拡張しない」方針上、今後の拡張禁止と CST-owned parse event への移行計画を明示すべき。
- ⚠️ `collect_logical_block_items` は `CstPunctuationScan` を使う実装に改善済みだが、まだ `body.lines()` を走査している。runtime-boundary-refactor の checklist は完了扱いだが、完全な CST item event 化までは未達の可能性がある。F4 で修正候補化。

### A5: runtime boundary obsolete names

- ✅ 検索語: `arcweft_core::frame`, `FlowFiber.frames`, `external_values`, `line_effects`, `task_requests`, `SourceEvent<String, String>`, `StreamEvent<String, String>`。
- ✅ `arcweft_core::frame`, `FlowFiber.frames`, `external_values` は docs / refactor checklist 由来だけに見えた。
- ✅ `RuntimeStepOutput` は `effects: RuntimeEffectBatch` と `requests: HostRequestBatch` を持ち、top-level `line_effects` / `task_requests` ではない。
- 🟦 `line_effects` は `RuntimeStepStats` の counter と CLI/reporting hit が中心で、旧 output field ではない。
- ⚠️ `task_requests` は CLI/runtime-host/reporting に残存 hit。旧 field 名ではない可能性が高いが、user-facing JSON 名や observation schema に旧語彙が残るなら `host_task_requests` / `host_requests.tasks` へ寄せる。

### A6: helper / conversion discipline

- ✅ 検索語: `_to_`, `convert_error`, `map_parse_error`。
- ✅ AGENTS.md が例示する `convert_error` / `map_parse_error` は AGENTS.md 本文だけに見えた。
- ⚠️ `crates/arcweft-render-wgpu/src/convert.rs` に `usize_to_f32`, `u32_to_f32`, `u64_to_f32`, `f32_floor_to_i32`, `f32_ceil_to_i32`, `f32_to_u8_nonnegative` がある。bounded conversion policy として意図は明確だが、名前は endpoint 型変換 helper そのもの。F5 で修正候補化。
- 🟦 `arcweft-core/src/engine/audio.rs` の `evaluate_audio_i16` などは Engine の domain-specific 評価 method であり、現時点では free-standing conversion helper ではない。ただし `#[allow(clippy::too_many_lines)]` と併せて分割候補。

### A7: compatibility layer / old alias

- ✅ 検索語: `compat`, `legacy`。
- ⚠️ `crates/arcweft-codec-binary/src/lib.rs` の `BincodeCompatCodec` / feature `bincode-legacy` は明示的な外部 interop boundary として実装されている。未完成 compiler/parser の互換層ではないので即違反とは断定しないが、AGENTS.md の「古い互換名を残さない」方針と誤読されやすい。F6 で rename / docs 補強案。
- 🟦 seq-02.3/02.4 note では product JSON fallback を消し、migrated catalog families を compact owner codec に寄せたことが記録されている。

### A8: stringly API / one-off labels

- ✅ CLI `output.rs` の `effect_label`, `source_policy_summary` を確認。
- 🟦 `format!("{:?}")` や string labels は CLI/reporting の表示境界なら許容。ただし runtime-plan / core / protocol へ伝播するなら typed summary enum へ移す。
- ⚠️ `RuntimeAudioCommand::operation_name()` は `&'static str` を返す。CLI/log label なら許容だが、protocol routing や capability dispatch に使い始めるなら typed enum / serde tag を使う。

### A9: structural audit / file size

- ✅ 2026-06-27 seq-02.3/02.4 note は `tools/structure-audit.rs` 実行済み、0 errors / 106 warning-level findings と記録している。
- ❌ この監査では structure-audit を再実行していない。巨大ファイル・責務過多の最新状態は未達。
- ⚠️ 既知: `crates/arcweft-bundle/src/product.rs` は tests 込みで大きいが、前回 note では changed production file warning threshold 未満、`resource_codec/runtime.rs` の warning-level size は pre-existing とされている。

## 方針違反候補と具体的修正方法

### F1: `#[allow]` の全件棚卸し不足

Status: ⚠️ / ❌ 全件分類未達

修正方法:

1. `rg -n --glob '*.rs' '#\!?\[allow' crates tools tests` をローカル checkout で実行し、production / test / generated / temporary に分類する。
2. production の broad allow は原則削除する。
3. `large_enum_variant` は Box 化または enum の payload 分離で解消できないか確認する。Arcweft-owned enum なら enum 本体へ domain behavior / storage policy を足す。
4. `too_many_lines` は state object / small inherent methods / domain enum method へ切る。private helper に逃がすだけの分割は避ける。
5. 残す allow は item 最小スコープに限定し、直上 comment に「なぜ設計上必要か」「いつ消せるか」を書く。

### F2: `arcweft-core` の adapter/domain 名 feature

Status: ⚠️

対象:

- `crates/arcweft-core/Cargo.toml`: `render-wgpu`, `ui-servo`, `audio`, `capture-camera`, `usb`, `mcp`。

修正方法:

1. 空 feature が downstream feature unification のためだけなら、`arcweft-core` から削除する。
2. feature gate が必要なら adapter crate 側 (`arcweft-render-wgpu`, `arcweft-player-native`, `arcweft-agent-mcp`, `arcweft-audio-*`) に移す。
3. core が持つべきものは backend 名ではなく typed data capability のみ。たとえば `RuntimeHostCallRequest`, `HostRequestBatch`, typed input/source/request model へ寄せる。
4. 削除後に `cargo tree -e features -p arcweft-core` で core に backend feature 名が残らないことを確認する。

### F3: core audio/capture command boundary の確認と分離

Status: ⚠️

対象:

- `crates/arcweft-core/src/audio.rs`
- `crates/arcweft-core/src/engine/audio.rs`
- `crates/arcweft-core/src/lib.rs` の `pub mod audio;`

修正方法:

1. 現状は Sans I/O の typed command 評価に見えるため即削除ではなく、責務を明文化する。
2. `RuntimeAudioCommand` が runtime core の汎用 host request で表現できるなら、`RuntimeHostCallRequest` / `HostRequestBatch.audio` の typed boundary に統合する。
3. audio/capture 固有 ID・constraints は `arcweft-interaction-model` または `arcweft-audio-core` 側に所有させ、core は opaque payload / typed request envelope だけにする。
4. `Engine::evaluate_audio_command` の巨大 match は `RuntimeAudioCommand` の inherent lowering method、または audio request builder 側へ移す。ただし extension trait や ad hoc wrapper ではなく、Arcweft-owned type の impl に置く。
5. core が device/microphone availability を判断しないことを source gate test で固定する。

### F4: parser の `split_top_level` / line-based logical item residual

Status: ⚠️

対象:

- `crates/arcweft-lang-syntax/src/parser/helpers.rs`
- `crates/arcweft-lang-syntax/src/parser/items.rs`
- `crates/arcweft-lang-syntax/src/parser/statements.rs`
- `crates/arcweft-lang-syntax/src/parser/choice.rs`
- `crates/arcweft-lang-syntax/src/cst/punctuation.rs`

修正方法:

1. 既存 `split_top_level_*` 利用箇所を「CST helper の薄い呼び出し」か「parser 固有 ad hoc scan」か分類する。
2. 新規構文対応では `split_top_level_*` を増やさず、CST token/event collector を先に拡張する。
3. `collect_logical_block_items` は `body.lines()` ではなく CST block item event を返す API に置き換える。
4. fixture を source-of-truth にし、multi-line defaults / raw strings / comments / nested delimiters / method-chain continuation の parser/HIR/sema/CLI fixture を共有する。
5. `runtime-boundary-refactor.md` の完了チェックと実装状態がずれる場合は、同 doc か別 implementation note に「CST event 化は未達」と明示する。

### F5: render-wgpu numeric conversion helper naming

Status: ⚠️

対象:

- `crates/arcweft-render-wgpu/src/convert.rs`

修正方法:

1. 型 endpoint 名 (`usize_to_f32`) ではなく domain policy 名へ変更する。例: `saturating_usize_as_f32`, `saturating_u32_as_f32`, `pixel_floor_as_i32`, `pixel_ceil_as_i32`, `nonnegative_alpha_byte`。
2. 複数 crate で同じ変換が必要なら free helper を増やさず、owned newtype (`LayoutPx`, `DevicePx`, `NormalizedAlpha`) または既存 Arcweft-owned boundary type の inherent method に寄せる。
3. 変換失敗時の saturating / zeroing policy を doc comment と tests で固定する。

### F6: `BincodeCompatCodec` / `bincode-legacy` の naming と外部 interop 明確化

Status: ⚠️

対象:

- `crates/arcweft-codec-binary/src/lib.rs`
- `crates/arcweft-codec-binary/Cargo.toml`

修正方法:

1. internal compatibility layer ではなく外部 format adapter であることを明確にするため、`BincodeCompatCodec` を `BincodeInteropCodec` へ rename する。
2. feature 名も `bincode-legacy` ではなく `bincode-interop` 等に変更する。既存ユーザー互換が必要なら migration note を docs/implementation に残し、unfinished compiler/parser の old alias とは別物だと明記する。
3. primary Arcweft binary format (`ArcweftBinaryCodec`) が bincode に依存しないことを source gate test で固定する。

### F7: runtime obsolete vocabulary の残存候補

Status: ⚠️ / 🟦

対象:

- `task_requests` hit in CLI/runtime-host/reporting.
- `line_effects` hit in stats/reporting.

修正方法:

1. runtime output struct field と serialized report field を分けて分類する。
2. runtime semantic boundary は `RuntimeEffectBatch` / `HostRequestBatch` を正本とする。
3. user-facing JSON に旧語彙が残るなら `host_task_requests` / `effects.line` / `requests.tasks` へ rename する。後方互換 alias は作らない。
4. `arcweft_core::frame`, `FlowFiber.frames`, `external_values` は code search 上 production hit なし。local `rg` で再確認する。

## 次回必須コマンド

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check

rg -n --glob '*.rs' '#\!?\[allow|#!\[feature|unsafe\s*\{|unsafe fn|Box::leak|mem::forget' crates tools tests
rg -n --glob '*.rs' 'split_top_level|collect_logical_block_items|_to_|compat|legacy|task_requests|line_effects|external_values|FlowFiber\.frames|arcweft_core::frame' crates tools tests
rg -n --glob 'Cargo.toml' 'render-wgpu|ui-servo|capture-camera|usb|mcp|cpal|wgpu|cranelift|wasmtime|ureq|reqwest' crates Cargo.toml
```

## 監査結論

この監査で、`Box::leak` / `mem::forget` / production `mod.rs` / obsolete `arcweft_core::frame` などの明確な即時違反は検索上確認できなかった。一方で、AGENTS.md 方針に反していそうな実装候補として、次を優先して修正・分類する。

1. `#[allow]` の全件棚卸しと削減。
2. `arcweft-core` の backend/domain feature 名削除または adapter 移管。
3. core audio/capture command boundary の責務明文化と、必要なら interaction/audio crate 側への分離。
4. parser の line-based / split_top_level 系を CST event 化へ進める。
5. render-wgpu numeric conversion helper を domain policy 名または owned type method へ寄せる。
6. `BincodeCompatCodec` / `bincode-legacy` を external interop 名へ寄せる。
7. `task_requests` / `line_effects` が旧 semantic field として残っていないか local rg で最終確認する。
