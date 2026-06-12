# ADR-001: `TextArea` に `writing_mode` を足さず、`GlyphArea` を追加する

## Status

Proposed

## Context

glyphon の既存 API は `TextArea { buffer, left, top, scale, bounds, default_color, custom_glyphs }` を中心にしています。`TextArea` に `writing_mode` を足すと、一見自然ですが、layout の責務が renderer に流れ込みます。

## Decision

`TextArea` は既存 path として残し、layout 済み glyph を受ける `GlyphArea` を追加します。

```text
TextArea  = glyphon/cosmic-text managed layout path
GlyphArea = external pre-laid glyph path
```

## Consequences

良い点:

```text
- glyphon は renderer のまま維持できる。
- arcweft が組版意味論を所有できる。
- 縦書き以外の ruby/数式/inline object も同じ path に乗る。
```

悪い点:

```text
- glyphon 本体の private fields を触る必要があるため upstream patch または fork が必要。
- external layout engine は shaping/cache key と glyphon cache の橋渡しを設計する必要がある。
```

## Rejected alternatives

```text
- chars().join("\n")
- TextArea 全体回転
- custom_glyphs だけで本文全部を描く
- glyphon を捨てて独自 atlas renderer を作る
```
