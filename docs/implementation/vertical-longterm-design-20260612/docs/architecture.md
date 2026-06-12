# 長期アーキテクチャ: glyphon extension としての縦書き

## 0. この設計で解くこと

目的は「glyphon に縦書きモードを足す」ではなく、**glyphon に layout 済み glyph stream を受け取れる正式な入口を足す**ことです。

arcweft の長期 text stack は次のように分けます。

```text
.arcw / RichTextDocument / TypesetBlock source
  ↓
arcweft-render-text
  - deterministic, serializable rich-text sidecar
  - writing_mode / vertical_latin / ruby_position などの authored intent
  ↓
arcweft-text-layout
  - Sans I/O layout engine
  - grapheme / bidi / script / font fallback / writing-mode / ruby / text-combine
  - hit-test / selection / Agent observation metadata
  ↓
arcweft-glyphon
  - glyphon extension inputへ変換する native/web adapter
  ↓
glyphon-layout-ext / upstream glyphon
  - GlyphArea / GlyphInstance を atlas + wgpu pipeline で描画
```

この分割で、glyphon は GPU renderer のまま保たれ、arcweft は文書意味論・組版意味論を所有できます。

## 1. 絶対に避ける長期形

### 1.1 `TextArea` に `writing_mode` を足すだけ

これは API だけを見ると楽ですが、実際には glyphon の責務が以下まで広がります。

```text
- Unicode grapheme cluster segmentation
- Vertical_Orientation table
- font fallback と script segmentation
- vertical OpenType feature policy
- vertical metrics fallback
- ruby layout
- text-combine-upright
- 禁則 / 追い込み / ぶら下げ
- caret / selection / hit-test
- Agent observation mapping
```

glyphon の性格は `Buffer` から出た glyph を atlas に積み、wgpu で描く renderer です。長期的にここへ組版を押し込むと、arcweft の presentation layer と glyphon の責務が絡みます。

### 1.2 文字列を改行で縦にする

これは表示だけのハックで、句読点、括弧、長音符、欧文 sideways run、数字の縦中横、行頭行末禁則、ルビ、caret、selection、hit-test、source map を壊します。

### 1.3 TextArea 全体を 90 度回す

日本語縦書きは「横書きの回転」ではありません。漢字・かなは正立、欧文は mixed では sideways、約物は縦組み用 glyph へ置換または位置調整が必要です。

## 2. 長期の最終 API 境界

### 2.1 glyphon 側へ追加する public API

```rust
pub struct GlyphArea<'a> {
    pub glyphs: &'a [GlyphInstance],
    pub left: f32,
    pub top: f32,
    pub scale: f32,
    pub bounds: TextBounds,
    pub default_color: Color,
}

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

pub enum GlyphSource {
    Text { cache_key: cosmic_text::CacheKey },
    Custom { id: CustomGlyphId },
}
```

`TextArea<'a>` は既存 API として残し、`GlyphArea<'a>` を sibling として足します。`TextArea` を壊さず、横書き従来用途はそのままです。

### 2.2 renderer 側の入口

```rust
impl TextRenderer {
    pub fn prepare_glyph_areas<'a>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        atlas: &mut TextAtlas,
        viewport: &Viewport,
        glyph_areas: impl IntoIterator<Item = GlyphArea<'a>>,
        cache: &mut SwashCache,
    ) -> Result<(), PrepareError>;
}
```

最終的には `TextArea` と `GlyphArea` を混在できる `RenderArea` も足せます。ただし upstream へ出す最小差分は `prepare_glyph_areas` が良いです。

## 3. arcweft 側の crate 境界

```text
crates/arcweft-render-text
  既存。RichTextDocument / presentation sidecar。
  Sans I/O、serializable、authoring intent。

crates/arcweft-text-layout
  新規。layout algorithm 本体。
  Sans I/O。GPU、glyphon、filesystem、wall-clock へ依存しない。

crates/arcweft-glyphon
  新規または player-native 側 adapter。
  arcweft-text-layout の PlacedGlyph を glyphon::GlyphArea へ変換。
  glyphon / wgpu / SwashCache / TextAtlas を持つ。
```

`arcweft-core` は text layout や renderer に依存させません。core は runtime/data core のままです。

## 4. データフロー

```text
LineDisplayFrame
  ├─ text: String
  ├─ display_map: source/ruby/control mapping
  └─ RichTextPresentation(layout, transform, effects...)
       ↓
Style cascade / run building
       ↓
ParagraphLayoutInput
       ↓
Cluster segmentation
       ↓
Bidi + script + language + font fallback segmentation
       ↓
Vertical orientation classification
       ↓
Text-combine detection
       ↓
ShapingPlanRun[]
       ↓
ShapingBackend::shape_run
       ↓
GlyphRun[] with metrics
       ↓
Line breaking / column breaking / ruby collision resolution
       ↓
LaidOutText
  ├─ glyphs: PlacedGlyph[]
  ├─ line_boxes: LineBox[]
  ├─ hit_map: HitMap
  └─ observations: TextRunObservation[]
       ↓
arcweft-glyphon bridge
       ↓
glyphon::GlyphArea[]
       ↓
glyphon atlas / wgpu vertices
```

## 5. 座標系

縦書きは内部では physical x/y だけで扱わない方が良いです。

```text
logical inline axis:
  horizontal-tb: left → right
  vertical-rl:   top → bottom
  vertical-lr:   top → bottom

logical block axis:
  horizontal-tb: top → bottom
  vertical-rl:   right → left
  vertical-lr:   left → right
```

内部 layout は `LogicalPoint { inline, block }` を基準に行い、最後に physical へ写像します。hit-test、selection、scroll、Agent observation まで同じ変換を使うことが重要です。

## 6. glyphon renderer の変更点

`GlyphArea` 入力を実現するには、glyphon 内部を次のようにリファクタします。

```text
現在:
  prepare(TextArea)
    for run in buffer.layout_runs()
      for glyph in run.glyphs
        physical = glyph.physical(...)
        prepare_glyph(...)
        glyph_vertices.push(...)

変更後:
  prepare(TextArea)
    TextArea → internal PreparedGlyphInput iterator
    prepare_prelaid_glyphs(...)

  prepare_glyph_areas(GlyphArea)
    GlyphArea → internal PreparedGlyphInput iterator
    prepare_prelaid_glyphs(...)
```

`prepare_glyph` と atlas allocation は既存ロジックを共有します。変えるのは **glyph の供給元** と **quad transform** です。

## 7. transform の扱い

長期的には `GlyphTransform::Affine([f32; 6])` を採用します。ただし public API は初期段階で以下の enum を持つと安全です。

```rust
pub enum GlyphTransform {
    Identity,
    Rotate90Cw,
    Rotate90Ccw,
    Affine(Affine2),
}
```

renderer 内部ではいずれも affine に正規化します。既存 glyphon は instanced quad 的なデータを持っているため、最小変更は `GlyphToRender` に affine 係数を追加し、WGSL 側で corner を変換する方式です。

## 8. cache key 方針

transform は raster cache key に入れません。glyph bitmap は同じで、回転や移動は vertex transform だからです。

ただし以下は raster cache key に影響します。

```text
- font id
- glyph id
- font size
- subpixel bin
- cache key flags
- vertical feature により置換後の glyph id
- color glyph / mask glyph content type
```

layout cache key には text hash、style span hash、font set hash、locale、writing mode、text orientation、constraint、ruby policy、text-combine policy、Unicode data version、layout engine version を入れます。

## 9. long-term acceptance criteria

- `GlyphArea` は `TextArea` と同じ renderer / atlas / viewport で描ける。
- 縦書きの layout は `arcweft-text-layout` が Sans I/O で再現可能。
- 1 glyph ごとに source range と cluster id を保持する。
- line/column/hit-test は logical axis で行い、physical は最後に変換する。
- `RichTextWritingMode::VerticalRl` と `RichTextVerticalLatinMode::{Mixed,Upright,Sideways}` を自然に bridge できる。
- Agent observation が bbox/source/ruby/style を返せる。
- renderer-specific dependency は adapter crate へ閉じ込める。
