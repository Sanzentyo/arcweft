# Algorithm 02: shaping と vertical OpenType feature

## 1. 基本方針

縦書き shaping は「横組み shape → glyph ごとに回転」だけでは不足します。長期実装では orientation ごとに shaping run を分け、font feature policy を明示します。

```text
OrientedCluster[]
  ↓ split by font/script/style/orientation/feature-policy
ShapePlanRun[]
  ↓ ShapingBackend
ShapedGlyph[]
```

## 2. ShapePlanRun

```rust
pub struct ShapePlanRun<'a> {
    pub text: &'a str,
    pub cluster_range: ClusterRange,
    pub font_chain: &'a [FontCandidate],
    pub script: Script,
    pub direction: ShapeDirection,
    pub feature_policy: VerticalFeaturePolicy,
    pub orientation: ResolvedOrientation,
}
```

`ShapeDirection` は layout の inline direction と同一ではありません。

```rust
pub enum ShapeDirection {
    HorizontalLtr,
    HorizontalRtl,
    VerticalTtb,
}
```

## 3. VerticalFeaturePolicy

```rust
pub enum VerticalFeaturePolicy {
    Horizontal,
    VerticalAlternates,
    VerticalPreRotated,
    SidewaysEngineRotate,
}
```

推奨:

```text
- upright CJK: VerticalAlternates を使う。
- sideways Latin: Horizontal shape + engine rotate を使う。
- pre-rotated glyph path: font/engine が明示対応する場合だけ VerticalPreRotated を使う。
```

`vert`/`vrtr` 系と `vrt2` 系は混ぜません。

## 4. Feature decision

```text
if writing_mode == HorizontalTb:
    policy = Horizontal
else if resolved == Upright:
    policy = VerticalAlternates
else if resolved == SidewaysClockwise:
    policy = SidewaysEngineRotate
```

`VerticalPreRotated` は opt-in です。

```text
condition:
  - shaping backend が vrt2 path を明示サポート
  - font capability probe が vrt2 を確認済み
  - renderer が glyph-local transform 二重適用を避けられる
```

## 5. Font fallback

font fallback は cluster 単位ではなく run 単位最適化にします。

難しい理由:

```text
- 1 glyph だけ fallback すると baseline と em box が揺れる。
- CJK fallback font と Latin font の metrics が異なる。
- ruby text は別サイズで fallback する。
- vertical metrics がない font は synthesized metrics が必要。
```

Algorithm:

```text
for each style span:
  1. primary font candidate を選ぶ。
  2. cluster ごとに coverage bitset を評価。
  3. contiguous covered range を primary run にする。
  4. uncovered cluster は fallback chain で同じ処理。
  5. fallback boundary は shaping boundary にする。
```

最適化:

```text
- font coverage は Unicode block coarse bitset + exact cmap probe の二段。
- fallback result は (font_set_hash, style_font_request, cluster_text_hash) で cache。
```

## 6. Vertical metrics fallback

font が vertical origin / vertical advance を持つなら使います。ない場合は synthesized metrics を作ります。

```text
synthesized_vertical_advance = horizontal em size
synthesized_origin.x = glyph horizontal origin adjusted to center in em square
synthesized_origin.y = dominant baseline position
```

ただし punctuation は visual center だけでは不十分なので、`Tu` fallback adjustment を別に適用します。

## 7. ShapedGlyph

```rust
pub struct ShapedGlyph {
    pub font_key: FontKey,
    pub glyph_id: GlyphId,
    pub cache_key: GlyphCacheKey,
    pub cluster: ClusterRange,
    pub advance: Vector,
    pub offset: Vector,
    pub ink_bounds: Rect,
    pub orientation: ResolvedOrientation,
    pub feature_policy: VerticalFeaturePolicy,
}
```

`advance` は logical inline axis 上の進行量に変換済みにします。

```text
horizontal: advance = (x_advance, 0)
vertical:   advance = (0, y_advance) in logical area coordinates before physical mapping
```

## 8. Engine rotation の基準点

sideways run は run 全体を回すのではなく、glyph ごとに transform を持たせます。ただし spacing は run-level metrics から求めます。

```text
1. Latin run を horizontal shape する。
2. run horizontal advance を vertical inline length にする。
3. glyph origin は vertical line center を基準に置く。
4. each glyph local quad に Rotate90Cw を掛ける。
```

これにより kerning は horizontal shaping 結果を保ったまま、縦中に sideways run を置けます。

## 9. Cache key

```rust
struct ShapeCacheKey {
    text_hash: Hash,
    style_hash: Hash,
    font_set_hash: Hash,
    writing_mode: WritingMode,
    orientation_policy: TextOrientation,
    unicode_version: UnicodeVersion,
    feature_policy: VerticalFeaturePolicy,
}
```

Unicode table version と feature policy を key に入れないと、更新時に古い layout が残ります。
