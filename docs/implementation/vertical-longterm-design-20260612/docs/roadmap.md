# 長期ロードマップ

## Phase 1: glyphon API 変形の足場

- `GlyphArea` / `GlyphInstance` の public API を upstreamable な形で定義する。
- 既存 `TextArea` path を壊さない。
- `prepare_glyph_areas` が horizontal pre-laid glyph を描ける。
- transform は `Identity` のみでも可。ただし型には `GlyphTransform` を入れておく。

## Phase 2: affine quad renderer

- `GlyphToRender` に affine 係数を追加。
- WGSL vertex shader で local quad corner を transform する。
- CPU broad clipping + shader local clipping を入れる。
- rotation regression tests を追加する。

## Phase 3: arcweft-text-layout crate

- `LineDisplayFrame` → `ParagraphLayoutInput` 変換。
- logical axis model。
- cluster/source/ruby map。
- minimal vertical-rl line/column layout。
- `LaidOutText` と `HitMap`。

## Phase 4: shaping backend

- `ShapingBackend` trait を作る。
- cosmic-text / rustybuzz / swash のどれを使っても `ShapedGlyph` へ落とす。
- vertical feature policy を `ShapePlan` に持たせる。
- `vert`/`vrtr` path と `vrt2` path を排他的に扱う。

## Phase 5: 難所組版

- UAX #50 generated table。
- UAX #29 grapheme cluster。
- UAX #14/JLREQ based line break classes。
- text-combine-upright。
- ruby collision and expansion。
- punctuation compression / hanging punctuation。

## Phase 6: Agent / input / cache

- `TextRunObservation`、`GlyphObservation`、`TextHitRegion`。
- pointer → cluster/caret hit-test。
- selection polygon generation。
- layout cache key / invalidation。
- LayerTree object-id pass との接続。

## Phase 7: conformance fixtures

- W3C/JLREQ に沿った目視 fixtures。
- `吾輩は猫である。ABC 123` mixed orientation。
- `2026` text-combine。
- `「縦書き」` punctuation alternates。
- ruby over/under in vertical-rl。
- typewriter reveal with stable line breaks。
