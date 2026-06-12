# vertical-rl dialogue flow example

入力:

```text
吾輩は猫である。ABC 123「縦書き」
```

style:

```rust
TextLayoutStyle {
    writing_mode: WritingMode::VerticalRl,
    text_orientation: TextOrientation::Mixed,
    direction: InlineDirection::Auto,
    ruby_position: RubyPosition::Auto,
    text_combine: TextCombinePolicy::Digits { max_digits: 3 },
    column_gap: 8.0,
}
```

pipeline:

```text
1. cluster segmentation
2. UAX #50 orientation
3. text-combine candidate: 123 -> group
4. shape upright CJK with vertical alternates
5. shape ABC horizontally, place as sideways run
6. shape text-combine group horizontally, compress into 1em
7. line break by vertical inline height
8. column placement from right to left
9. produce GlyphArea
```

conceptual visual order:

```text
rightmost column:
  吾
  輩
  は
  猫
  で
  あ
  る
  。
  ABC(sideways)
  123(text-combine)
  「
  縦
  書
  き
  」
```

`GlyphInstance` examples:

```rust
GlyphInstance {
    source: GlyphSource::Text { cache_key },
    origin: Point::new(column_x, y),
    transform: GlyphTransform::Identity,
    cluster: Some(TextCluster { logical_range, cluster_index }),
    ..
}

GlyphInstance {
    source: GlyphSource::Text { cache_key_for_a },
    origin: Point::new(column_x, y),
    transform: GlyphTransform::Rotate90Cw,
    cluster: Some(TextCluster { logical_range: abc_range, cluster_index }),
    ..
}
```

重要:

```text
- ABC は 3 glyph を 1 run として horizontal shaping する。
- 123 は text-combine group なので 1em inline advance。
- 「」は vertical alternate または fallback adjustment。
- reveal 中も line break は全文固定。
```
