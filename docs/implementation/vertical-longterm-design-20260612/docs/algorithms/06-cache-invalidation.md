# Algorithm 06: cache と invalidation

## 1. cache の種類

```text
- Unicode table cache: build-time generated, versioned
- font coverage cache: font + codepoint range
- shaping cache: text/style/font/orientation/features
- layout cache: paragraph/style/box/locale/reveal-policy
- glyph atlas cache: glyphon/sw ash cache side
- observation cache: layout result derived
```

`arcweft-text-layout` は Sans I/O なので GPU atlas cache を持ちません。

## 2. LayoutCacheKey

```rust
pub struct LayoutCacheKey {
    pub text_hash: Hash,
    pub display_map_hash: Hash,
    pub style_hash: Hash,
    pub box_size_hash: Hash,
    pub locale: Locale,
    pub font_set_hash: Hash,
    pub unicode_version: UnicodeVersion,
    pub algorithm_version: AlgorithmVersion,
}
```

`algorithm_version` は重要です。line break penalty や ruby collision policy を変えた時に古い layout を捨てるためです。

## 3. Typewriter reveal

reveal boundary を key に入れると毎文字 layout が走ります。長期では入れません。

```text
full layout cache key: reveal independent
frame visibility: reveal cluster index only
```

```rust
pub struct RevealMask {
    pub visible_until_cluster: u32,
    pub fade_window: Option<ClusterRange>,
}
```

renderer は invisible glyph の vertex を省くか alpha 0 にします。ただし layout は全文で固定します。

## 4. Incremental layout

ノベルゲームの dialogue では paragraph は短いですが、TypesetBlock では長文になります。

```text
dirty input:
  text range changed
  style range changed
  box size changed
  font availability changed
```

Incremental strategy:

```text
1. paragraph boundary で invalidation。
2. paragraph 内は cluster index range で shaping invalidation。
3. line break は affected paragraph 全体を再計算。
4. page/column flow は変更 paragraph 以降を再配置。
```

## 5. font probe cache

vertical fallback adjustment は font dependent です。

```rust
pub struct VerticalFontProbe {
    pub font_key: FontKey,
    pub has_vert: bool,
    pub has_vrtr: bool,
    pub has_vrt2: bool,
    pub has_vertical_metrics: bool,
    pub punctuation_adjustment_profile: ProfileId,
}
```

この probe は font file hash と font face index で cache します。

## 6. glyphon atlas invalidation

`GlyphArea` 追加後も atlas lifecycle は glyphon 側に残します。

```text
arcweft-text-layout:
  cache_key を持つ glyph stream を作る

glyphon:
  cache_key が atlas にあるか確認
  miss なら rasterize/pack
```

`GlyphTransform` は atlas key に入れません。同じ glyph bitmap を別 transform で描けるからです。

## 7. object-id / debug pass

object-id pass は glyph bitmap cache と独立です。

```text
color pass vertex: color + atlas uv + transform
oid pass vertex: object_id + transform + same quad geometry
```

layout cache invalidation で observation id が変わると object-id metadata も変わるため、vertex buffer は再生成します。atlas は維持できます。
