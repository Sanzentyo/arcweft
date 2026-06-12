# Algorithm 03: line breaking と vertical columns

## 1. model

縦書きでも line breaking の本質は inline axis 上の item sequence を分割することです。

```text
HorizontalTb:
  inline axis = left → right
  block axis  = top → bottom

VerticalRl:
  inline axis = top → bottom
  block axis  = right → left

VerticalLr:
  inline axis = top → bottom
  block axis  = left → right
```

内部 layout は logical axes で行い、最後に physical coordinates へ写像します。

## 2. InlineItem

```rust
pub struct InlineItem {
    pub cluster_range: ClusterRange,
    pub advance: f32,
    pub stretch: f32,
    pub shrink: f32,
    pub break_after: BreakOpportunity,
    pub kind: InlineItemKind,
}
```

`advance` は writing mode に関係なく inline 軸長です。

## 3. BreakOpportunity

```rust
pub enum BreakOpportunity {
    Prohibited,
    Allowed { penalty: i32 },
    Mandatory,
}
```

長期では UAX #14 + JLREQ の class pair table を生成します。

```text
break_allowed(left_class, right_class, locale, strictness) -> BreakOpportunity
```

JLREQ 由来の考慮:

```text
- 行頭禁則: closing punctuation, small kana など
- 行末禁則: opening punctuation など
- 分離禁止: em dash, leader, repeat marks など
- ぶら下げ: punctuation を line edge に出す
- 約物詰め: punctuation advance compression
```

## 4. DP line breaking

greedy は typewriter reveal と ruby で揺れやすいので、長期では paragraph 単位 dynamic programming を使います。

Cost:

```text
cost(line) =
    overflow_penalty(line)
  + badness(remaining_space, stretch, shrink)
  + break_penalty
  + consecutive_short_line_penalty
  + ruby_collision_estimate
```

`badness`:

```text
ratio = remaining / available_stretch_or_shrink
badness = 100 * abs(ratio)^3
```

Algorithm:

```text
let n = items.len()
dp[0] = 0
for i in 0..n:
  if dp[i] is INF: continue
  accum = 0
  for j in i+1..=n:
    accum += items[j-1].advance
    if break after j-1 is prohibited: continue
    line_cost = evaluate(i, j, accum, measure)
    dp[j] = min(dp[j], dp[i] + line_cost)
```

Pruning:

```text
- accum > max_inline * 1.5 で break。
- candidate j は allowed/mandatory break のみに限定。
- paragraph が長い場合は window DP にする。
```

## 5. Vertical column placement

LineBox は logical coordinates で持ちます。

```rust
pub struct LineBox {
    pub item_range: Range<usize>,
    pub inline_start: f32,
    pub inline_size: f32,
    pub block_start: f32,
    pub block_size: f32,
}
```

`VerticalRl` の physical mapping:

```text
physical_x = box_right - block_start - block_size + local_block_offset
physical_y = box_top + inline_start + local_inline_offset
```

`VerticalLr`:

```text
physical_x = box_left + block_start + local_block_offset
physical_y = box_top + inline_start + local_inline_offset
```

## 6. Column block size

`block_size` は単純な font size ではありません。

```text
block_size = max(
  body_em_advance,
  body_ink_width,
  ruby_overhang_side,
  emphasis_marks_side,
  inline_object_width,
)
```

隣接 column の collision を防ぐため、layout 後に column block extents を再計算します。

```text
pass 1: provisional columns by em size
pass 2: ruby/emphasis/object extents を測る
pass 3: block_start を再配置
```

## 7. Typewriter reveal stability

ノベルゲームでは reveal 中に line break が動くと読みにくいです。

方針:

```text
- paragraph 全文で line break を先に確定する。
- reveal は glyph visibility mask だけを変える。
- 未表示 glyph も layout advance には含める。
```

これにより、1 文字増えるたびに縦書き列が揺れる問題を避けます。
