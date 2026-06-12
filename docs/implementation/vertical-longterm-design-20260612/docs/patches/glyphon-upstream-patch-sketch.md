# glyphon upstream patch sketch

これは実 patch ではなく、長期で upstream/fork に入れる差分の設計メモです。

## 1. public module

```text
src/glyph_area.rs
```

追加:

```rust
pub struct GlyphArea<'a> { ... }
pub struct GlyphInstance { ... }
pub enum GlyphSource { Text { cache_key: CacheKey }, Custom { id: CustomGlyphId } }
pub enum GlyphTransform { Identity, Rotate90Cw, Rotate90Ccw, Affine([f32; 6]) }
```

`lib.rs` で re-export。

## 2. TextRenderer API

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

後で `PrepareContext` に寄せる。

## 3. Internal refactor

既存 path:

```text
TextArea -> LayoutGlyph -> prepare_glyph -> glyph_vertices.push
```

新 path:

```text
GlyphArea -> GlyphInstance -> prepare_glyph_instance -> glyph_vertices.push
```

共通化:

```rust
struct PreparedGlyphQuad {
    cache_key: GlyphonCacheKey,
    atlas_source: AtlasSource,
    origin: Point,
    ink_bounds: Rect,
    transform: Affine2,
    color: Color,
    metadata: usize,
}
```

## 4. Vertex format

既存 vertex が `min/max` を前提にしている場合、affine には不十分です。

追加候補:

```rust
#[repr(C)]
struct GlyphVertex {
    position: [f32; 2],
    texcoord: [f32; 2],
    color: u32,
    metadata: u32,
}
```

各 glyph quad で 4 vertices + index 6 にするか、instance data に affine を持たせるかを比較します。

推奨:

```text
- 既存 batching を保つなら instance data + unit quad。
- clipping/debug pass を強くするなら expanded 4 vertices。
```

## 5. Clipping

CPU:

```text
transformed_quad_aabb.intersects(bounds)
```

GPU:

```wgsl
let local = inverse_transform * world_position;
if local.x < clip_min.x || local.x > clip_max.x { discard; }
```

## 6. Tests

```text
- prepare_glyph_areas accepts empty iterator
- identity transform matches TextArea path for pre-laid horizontal glyphs
- Rotate90Cw swaps visual extents
- bounds clip rejects transformed out-of-bounds glyph
- metadata is preserved to vertex/object-id path
```
