# Validation

この設計パッケージは Rust API スケルトンを含みます。

作成環境では `cargo` / `rustc` がインストールされていなかったため、以下は未実行です。

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
```

利用側では、Arcweft workspace の toolchain に合わせて上記を実行してください。

期待する検証観点:

```text
- crates/glyphon-layout-ext-api が unsafe なしでビルドできること
- crates/arcweft-text-layout-design が unsafe なしでビルドできること
- vertical mixed text の ASCII が Rotate90Cw になること
- DP line breaker が max_inline を超えた paragraph を分割すること
- GlyphArea.visible_glyphs が transformed AABB と bounds で broad clipping すること
```
