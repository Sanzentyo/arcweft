# ADR-002: sideways run は engine transform を基本にする

## Status

Proposed

## Context

縦書き中の Latin run は sideways になることが多いです。OpenType には vertical alternates と pre-rotated glyph の両系統がありますが、雑に混ぜると二重回転や feature conflict が起こります。

## Decision

基本 path:

```text
- CJK upright: vertical alternates / vertical metrics を使う。
- Latin sideways: horizontal shaping を行い、engine の GlyphTransform で Rotate90Cw する。
- vrt2/pre-rotated glyph path は opt-in にする。
```

## Consequences

良い点:

```text
- Latin kerning を horizontal shaping として保てる。
- renderer transform と atlas bitmap cache を分離できる。
- feature policy が明確。
```

悪い点:

```text
- glyphon shader に affine quad transform が必要。
- clipping と hit-test は transformed quad を扱う必要がある。
```
