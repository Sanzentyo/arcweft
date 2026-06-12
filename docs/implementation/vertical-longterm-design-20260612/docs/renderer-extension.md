# glyphon renderer extension 設計

## 1. 追加する型

`GlyphArea` は `TextArea` の sibling です。既存 `TextArea` を壊さず、新しい入口を足します。

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

`GlyphInstance` は 1 つの visual glyph quad を表します。`cluster` は render には不要ですが、debug、object-id、selection overlay、accessibility、Agent observation には必要です。

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

## 2. GlyphSource

```rust
pub enum GlyphSource {
    Text { cache_key: cosmic_text::CacheKey },
    Custom { id: CustomGlyphId },
}
```

長期では `Text` source を中心にします。`Custom` は inline icon、generated glyph、emoji fallback、特殊 mask などのために残します。

## 3. render pipeline への差分

既存 glyphon の大きな流れは維持します。

```text
rasterize if cache miss
  ↓
pack into atlas
  ↓
append glyph vertex item
  ↓
render pass samples atlas
```

差分は以下です。

```text
- TextArea path は Buffer::layout_runs() 由来
- GlyphArea path は external PlacedGlyph 由来
- GlyphToRender に transform を持たせる
- bounds clipping は transformed quad の AABB で行う
- exact clip が必要な場合は shader で local rect clip を併用する
```

## 4. clipping

縦書きや sideways glyph では、quad が回転します。長期では clipping を 2 段にします。

```text
1. CPU broad phase:
   transformed quad の axis-aligned bounding box と TextBounds の交差を見る。

2. GPU narrow phase:
   fragment shader が local glyph rect と clip rect を見て discard / alpha 0 にする。
```

90 度回転だけなら CPU で width/height swap して既存クリップへ寄せることもできますが、oblique / effects / glyph animation と組み合わせると affine が必要になります。

## 5. depth / metadata

`metadata: usize` は既存 glyphon の metadata-to-depth callback と同じ考え方を継続します。

arcweft 側では以下を pack します。

```text
metadata bits or table index:
  - layer id
  - text layout id
  - glyph index
  - object id / observation id
```

usize に詰め込みすぎるより、`metadata` は index とし、adapter 側に `Vec<GlyphMetadataRecord>` を持つ方が安全です。

## 6. upstream に出す順序

1. `GlyphArea`, `GlyphInstance`, `GlyphTransform`, `GlyphSource` を public module として追加。
2. 既存 `prepare` 内部を `prepare_text_area_internal` と `prepare_glyph_input` に分ける。
3. `prepare_glyph_areas` を追加。
4. `GlyphToRender` に transform を追加。
5. WGSL を affine transform 対応にする。
6. 既存 examples に pre-laid glyph example を追加。
7. 縦書き example は glyphon 本体ではなく external layout example として置く。

## 7. upstream に入れないもの

```text
- Japanese line breaking
- UAX #50 table
- ruby
- text-combine-upright
- arcweft SourceAnchor
- LayerTree / object id pass
```

これらは renderer ではなく layout / engine domain です。
