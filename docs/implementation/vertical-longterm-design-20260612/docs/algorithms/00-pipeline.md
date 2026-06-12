# Algorithm 00: 全体 pipeline

縦書きは一つの巨大関数で実装しません。難所を分離し、各段階の入出力を固定します。

```text
1. RichText flatten
2. Style cascade
3. Unicode segmentation
4. Orientation resolution
5. Font fallback plan
6. Shaping plan generation
7. Glyph shaping
8. Inline item construction
9. Line breaking
10. Column layout
11. Ruby / text-combine refinement
12. Glyph placement
13. Hit map / observation map
14. glyphon GlyphArea conversion
```

## 1. RichText flatten

`RichTextDocument` は node tree ではなく ordered node stream です。layout engine へ入れる前に visible text と source map を作ります。

```rust
struct FlattenedText {
    text: String,
    runs: Vec<StyleRun>,
    ruby: Vec<RubyAnnotation>,
    controls: Vec<ControlMarker>,
    source_map: Vec<SourceSegment>,
}
```

注意点:

```text
- ruby base は本文 text に入る。
- ruby text は本文 text には入れない。
- hard break は U+000A ではなく ControlMarker として持つ。
- typewriter reveal の boundary は byte index ではなく cluster index で持つ。
```

## 2. Style cascade

`StyleRun` は authored span の重なりを解消した non-overlap run にします。

```text
input: nested StyleStart / StyleEnd / base_styles
output: sorted non-overlap StyleRun[]
```

実装は sweep-line です。

```text
1. start/end event を byte index に集める。
2. byte index 昇順で event を処理する。
3. active style stack から computed style を作る。
4. 次の event までを StyleRun にする。
```

同じ byte index に start/end がある場合は、以下の順で安定化します。

```text
1. end event
2. start event
3. control marker
```

これにより空 range style と control 周辺の揺れを減らします。

## 3. Segmentation

production では UAX #29 grapheme cluster を使います。設計スケルトンでは char 単位ですが、API は cluster 単位前提です。

```rust
struct TextCluster {
    range: ByteRange,
    text: SmallString,
    style_run: StyleRunId,
}
```

cluster 単位にする理由:

```text
- variation selector
- combining mark
- emoji sequence
- surrogate-like conceptual unit
- caret が入れない箇所の保護
- UAX #50 が grapheme cluster 単位で orientation を扱うため
```

## 4. Orientation resolution

各 cluster に `VerticalOrientation` と resolved orientation を付けます。

```text
Unicode Vertical_Orientation + style.text_orientation + text-combine policy
  ↓
ResolvedOrientation::Upright | SidewaysClockwise | SidewaysCounterClockwise
```

## 5. Shaping

orientation と font/script が同じ cluster を run にまとめ、shape plan を作ります。

```text
cluster[]
  ↓ group by script/font/style/orientation/feature-policy
ShapePlanRun[]
  ↓ shaping backend
ShapedGlyph[]
```

## 6. Inline item construction

line breaker は glyph を直接扱いません。`InlineItem` を扱います。

```rust
enum InlineItemKind {
    GlyphCluster,
    RubyBaseGroup,
    TextCombineGroup,
    InlineObject,
    HardBreak,
}
```

各 item は inline advance、break class、stretch/shrink、penalty、cluster range を持ちます。

## 7. Line breaking

MVP では greedy、長期では dynamic programming を使います。詳細は `03-line-breaking-columns.md` を参照。

## 8. Column layout

`WritingMode::VerticalRl` の場合:

```text
inline axis: top → bottom
block axis: right → left
```

line break 後の line は vertical column です。box の右端から `column_advance + column_gap` ずつ左へ進めます。

## 9. Ruby / text-combine refinement

ruby と text-combine は line break 前から item として存在させますが、正確な配置は line/column 決定後です。

```text
line decided
  ↓
ruby overhang/collision resolution
  ↓
base group adjustment
  ↓
final glyph placement
```

## 10. Hit map

glyph bbox だけでは caret が作れません。cluster advance zone と ink bbox を分けて持ちます。

```rust
struct HitCell {
    cluster_range: ClusterRange,
    caret_before: Point,
    caret_after: Point,
    advance_zone: Polygon,
    ink_zone: Rect,
}
```

pointer hit-test はまず advance zone、次に nearest caret、最後に ink fallback の順にします。
