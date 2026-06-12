# arcweft への統合設計

## 1. 現状の活かし方

arcweft には既に `arcweft-render-text` があり、`LineDisplayFrame`、`RichTextDocument`、`RichTextDisplayMap`、`RichTextRubyAnnotation`、`RichTextPresentation` が存在する前提で設計します。

特に `RichTextLayout` は以下に相当する意図を既に持っています。

```text
- writing_mode: HorizontalTb / VerticalRl / VerticalLr
- direction: Auto / Ltr / Rtl
- vertical_latin: Mixed / Upright / Sideways
- ruby_position: Auto / Over / Under / InterCharacter
- column_gap
```

したがって、新しい text layout engine は authoring syntax からやり直さず、`LineDisplayFrame` と `RichTextPresentation` を入力にします。

## 2. 新規 crate

```text
crates/arcweft-text-layout
```

責務:

```text
- layout input model
- style cascade result model
- Unicode segmentation / orientation
- shaping plan generation
- line / column layout
- ruby and text-combine layout
- hit-test / selection map
- Agent observation map
```

依存して良いもの:

```text
- arcweft-render-text
- arcweft-id / arcweft-source など純 data crate
- serde / thiserror
- optional unicode data crate / generated table
```

依存してはいけないもの:

```text
- glyphon
- wgpu
- filesystem
- network
- wall-clock
- native windowing
```

## 3. glyphon adapter crate

```text
crates/arcweft-glyphon
```

責務:

```text
- arcweft-text-layout::LaidOutText を glyphon::GlyphArea へ変換
- glyphon FontSystem / SwashCache / TextAtlas の lifetime を扱う
- wgpu render pass へ接続
- object-id / depth / layer metadata の metadata packing
```

`arcweft-glyphon` は player-native あるいは renderer adapter として高レイヤに置きます。`arcweft-core`、`arcweft-lang-*`、`arcweft-render-text` からは参照しません。

## 4. LayerTree との接続

text layout の結果は `LayerContent::Text` または `LayerContent::RichText` の CPU-side prepared payload として layer に乗ります。

```rust
pub struct PreparedTextLayer {
    pub layout_id: TextLayoutId,
    pub laid_out_text: LaidOutText,
    pub hit_region: TextHitRegion,
    pub observations: Vec<TextRunObservation>,
}
```

LayerTree は描画順、clip、input routing、object id pass を担当します。text layout は「どこが hit したか」「どの source range か」を返すだけで、入力ルーティング自体は LayerTree に任せます。

## 5. Need と cache

`TypesetBlock` 級の長い文書は `Need<T, E>` に載せます。dialogue RichText は毎 frame に関わるため、短い場合は同期 layout、長い場合は cache hit 前提にします。

```text
RichText dialogue:
  - short text
  - typewriter reveal
  - style/effect changes
  - frame-local layout cache

TypesetBlock:
  - long document
  - lazy precompile
  - persistent cache key
  - Need progress reporting
```

`arcweft-text-layout` は Sans I/O なので、cache store そのものは host adapter が持ちます。

## 6. Agent observation

layout は run/glyph ごとに以下を返します。

```rust
pub struct TextRunObservation {
    pub text: String,
    pub logical_range: TextRange,
    pub physical_bounds: Rect,
    pub baseline: LogicalLine,
    pub ruby: Option<String>,
    pub style: TextStyleSummary,
    pub source: Option<SourceAnchor>,
}
```

重要なのは、Agent 観測の bbox を renderer の pixel readback から推定するのではなく、layout engine の幾何情報から直接出すことです。object-id pass は補助であり、source map は layout が正本です。
