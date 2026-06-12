# Algorithm 05: hit-testing と selection

## 1. 基本原則

描画 glyph の ink bbox と、text interaction の advance zone は別物です。

```text
ink bbox:
  見えているピクセル周辺。クリックしづらい。

advance zone:
  文字が占有する logical cell。caret/hit-test 向き。
```

## 2. HitMap

```rust
pub struct HitMap {
    pub cells: Vec<HitCell>,
    pub line_boxes: Vec<LineBox>,
    pub column_boxes: Vec<ColumnBox>,
}

pub struct HitCell {
    pub cluster_range: ClusterRange,
    pub logical_index: u32,
    pub advance_zone: Quad,
    pub ink_bounds: Rect,
    pub caret_before: CaretShape,
    pub caret_after: CaretShape,
    pub bidi_level: u8,
}
```

縦書き caret は横線になります。

```text
HorizontalTb: caret は縦線
VerticalRl/Lr: caret は横線
```

## 3. pointer hit-test

Algorithm:

```text
1. point を layer transform の inverse で text-local へ戻す。
2. column_boxes の AABB で候補 column を絞る。
3. line/column の inline range で候補 cell を絞る。
4. advance_zone contains(point) を優先。
5. 見つからなければ ink_bounds の最近傍。
6. さらに見つからなければ同じ column の nearest inline position。
7. caret before/after は cell center との距離で決める。
```

縦書きでは距離比較の主軸は y です。

```text
if point.inline < cell.inline_center:
    caret_before
else:
    caret_after
```

## 4. selection polygon

選択範囲は glyph bbox の union ではなく line fragment の union です。

```text
logical range [a, b)
  ↓
split by line/column
  ↓
for each fragment:
    make fragment rectangle from caret(a) to caret(b)
  ↓
transform to physical polygon
```

VerticalRl fragment:

```text
x range = column block span
y range = selected inline span
```

## 5. ruby selection

ruby は本文 range に付属する annotation です。

方針:

```text
- 本文を選択した場合、ruby highlight も表示する。
- ruby だけを選択する mode は advanced editor 用として別 action。
- Agent observation には base bbox と ruby bbox を別々に出す。
```

## 6. text-combine hit-test

二段階にします。

```text
1. group advance zone に hit。
2. editor mode なら group-local inverse transform で内部 glyph/caret を hit。
3. dialogue mode なら group 前後 caret に丸める。
```

## 7. bidi in vertical text

日本語 dialogue の主 path は bidi の影響が限定的ですが、長期では bidi level を HitCell に保持します。

```text
- logical order selection は source order。
- visual navigation は visual order。
- Ctrl/Option navigation は script/word segmentation。
```

## 8. Agent observation

Agent 向けには、hit-test map と同じ geometry を利用します。

```rust
pub struct GlyphObservation {
    pub text: String,
    pub source_range: SourceRange,
    pub bbox: Rect,
    pub polygon: [Point; 4],
    pub writing_mode: WritingMode,
    pub role: TextRole,
}
```

object-id pass で glyph/range id を描く場合、`GlyphInstance.metadata` に observation id を pack します。
