# Rust API 設計

この章は、長期で upstream glyphon または fork に入れる API と、arcweft 側の Sans I/O layout API を分けて定義します。

## 1. glyphon extension API

### 1.1 方針

既存の `TextArea<'a>` は維持し、`Buffer::layout_runs()` 由来の標準 path として残します。追加するのは sibling API です。

```rust
pub struct GlyphArea<'a> {
    pub glyphs: &'a [GlyphInstance],
    pub left: f32,
    pub top: f32,
    pub scale: f32,
    pub bounds: TextBounds,
    pub default_color: Color,
}
```

`GlyphArea` の責務は「配置済み glyph を描く」だけです。縦書きか横書きか、ルビか本文か、text-combine か inline icon かは区別しません。

### 1.2 GlyphInstance

```rust
pub struct GlyphInstance {
    pub source: GlyphSource,
    pub origin: Point,
    pub advance: Vector,
    pub ink_bounds: Rect,
    pub transform: GlyphTransform,
    pub color: Option<Color>,
    pub metadata: usize,
    pub cluster: Option<TextCluster>,
}
```

- `origin`: glyph local coordinate の基準点。通常は baseline origin。
- `advance`: caret/hit-test/debug のための進行 vector。描画 quad の位置決定そのものは `origin + transform + ink_bounds` で行う。
- `ink_bounds`: rasterized glyph の local bounds。atlas rectangle とは別。
- `transform`: glyph-local から area-local への affine 変換。
- `metadata`: layer object id、run id、effect id などを pack するための renderer-opaque 値。
- `cluster`: renderer は不要だが、debug pass、Agent observation、selection overlay 生成で必要。

### 1.3 GlyphSource

```rust
pub enum GlyphSource {
    Text { cache_key: cosmic_text::CacheKey },
    Custom { id: CustomGlyphId },
}
```

本命は `Text` です。`Custom` は次の用途に限定します。

```text
- inline icon / SVG fallback
- color emoji や platform-specific emoji fallback
- generated glyph / game-specific symbol
- mask glyph / object-id only glyph
```

### 1.4 Prepare API

最終 API は、引数が多すぎる `prepare` を増やすのではなく context 化します。

```rust
pub struct PrepareContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub font_system: &'a mut FontSystem,
    pub atlas: &'a mut TextAtlas,
    pub viewport: &'a Viewport,
    pub cache: &'a mut SwashCache,
}

impl TextRenderer {
    pub fn prepare_glyph_areas<'a>(
        &mut self,
        ctx: &mut PrepareContext<'_>,
        areas: impl IntoIterator<Item = GlyphArea<'a>>,
    ) -> Result<(), PrepareError>;
}
```

互換性を重視するなら、最初は `TextRendererGlyphAreaExt` trait として外部 crate で API shape を定義し、glyphon 本体に private field access が必要になった時点で upstream patch にします。

## 2. arcweft-text-layout API

### 2.1 入力

```rust
pub struct ParagraphLayoutInput<'a> {
    pub text: &'a str,
    pub runs: &'a [StyleRun],
    pub ruby: &'a [RubyAnnotation],
    pub box_size: Size,
    pub style: TextLayoutStyle,
}
```

`LineDisplayFrame` から以下を flatten して作ります。

```text
- visible text
- byte range → authored node/source map
- style cascade result
- ruby annotations
- control markers / hard breaks
- reveal boundary policy
```

### 2.2 Style

```rust
pub struct TextLayoutStyle {
    pub writing_mode: WritingMode,
    pub text_orientation: TextOrientation,
    pub direction: InlineDirection,
    pub line_break: LineBreakPolicy,
    pub ruby_position: RubyPosition,
    pub text_combine: TextCombinePolicy,
    pub column_gap: f32,
}
```

`WritingMode` と `TextOrientation` は renderer option ではなく、layout model の第一級属性です。

### 2.3 出力

```rust
pub struct LaidOutText {
    pub glyphs: Vec<PlacedGlyph>,
    pub lines: Vec<LineBox>,
    pub columns: Vec<ColumnBox>,
    pub hit_map: HitMap,
    pub observations: Vec<TextRunObservation>,
}
```

`LaidOutText` は完全に Sans I/O です。GPU handle、font texture、atlas rect、window scale factor を持ちません。

### 2.4 glyphon adapter

```rust
impl LaidOutText {
    pub fn to_glyph_area<'a>(&'a self, area: AreaPlacement) -> GlyphArea<'a>;
}
```

ただし、本当に `GlyphArea<'a>` を作るには shaping backend が生成した `cache_key` が必要です。したがって production では `PlacedGlyph` に `GlyphSource` 相当の renderer-independent key を保持し、`arcweft-glyphon` が glyphon-specific `GlyphSource` へ変換します。

## 3. 重要な lifetime 方針

`GlyphArea<'a>` は glyph slice を借用します。frame ごとの temporary Vec を render prepare 完了まで生かせばよいです。atlas cache は glyphon 側が保持します。

```text
FrameBuilder owns Vec<GlyphInstance>
  ↓ borrowed by GlyphArea
TextRenderer::prepare_glyph_areas consumes borrowed view
  ↓ uploads vertices / atlas requests
FrameBuilder can be dropped after prepare
```

## 4. versioning 方針

- `GlyphArea` は縦書き専用名にしない。
- `GlyphTransform` は最初から affine まで表現できる型にする。
- `GlyphSource::Text` は当初 `cosmic_text::CacheKey` でもよいが、将来 `glyphon::GlyphCacheKey` newtype に包む。
- `cluster` は optional にせず持たせたいが、icon/custom glyph を考えると `Option` が現実的。
