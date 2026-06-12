# arcweft × glyphon 縦書き長期設計パッケージ

これは、glyphon を **layout 済み glyph stream を受け取れる renderer extension** として拡張し、arcweft 側で縦書きを含む text layout を所有するための長期設計パッケージです。

短期 MVP、`custom_glyphs` だけの回避策、`chars().join("\n")`、`TextArea` 全体の 90 度回転は意図的に対象外にしています。

## 含まれるもの

```text
.
├── docs/
│   ├── architecture.md
│   ├── arcweft-integration.md
│   ├── renderer-extension.md
│   ├── roadmap.md
│   ├── api/rust-api.md
│   ├── algorithms/*.md
│   ├── adrs/*.md
│   └── patches/glyphon-upstream-patch-sketch.md
├── crates/
│   ├── glyphon-layout-ext-api/
│   │   └── src/*.rs
│   └── arcweft-text-layout-design/
│       └── src/*.rs
└── examples/vertical_rl_dialogue_flow.md
```

## 設計の一文要約

**glyphon に縦書き組版を背負わせず、glyphon に `GlyphArea` / `GlyphInstance` という低レベル描画入力を追加する。arcweft は Sans I/O の text layout crate で縦書き・ルビ・縦中横・hit-test を確定し、native/web player adapter が glyphon extension に変換する。**

## Rust スケルトンの位置づけ

この ZIP 内の Rust crates は、そのまま production crate として使うものではなく、API 形状と難所アルゴリズムの責務境界をコンパイル可能な形で示すための設計スケルトンです。外部依存を入れず、`unsafe` を使わず、Arcweft の「低レイヤは Sans I/O」「typed API」「renderer adapter を分ける」前提に寄せています。

確認コマンド:

```bash
cargo fmt --all
cargo test --workspace
```

## 検証メモ

この作成環境には `cargo` / `rustc` が無かったため、実行検証は `VALIDATION.md` に未実行として記録しています。
