# Algorithm 04: ruby と text-combine-upright

## 1. Ruby の model

Ruby は base text に付属する inline annotation ですが、縦書きでは placement side が物理座標と直感的にずれます。

```text
VerticalRl:
  ruby over = base column の右側
  ruby under = base column の左側

VerticalLr:
  ruby over = base column の左側
  ruby under = base column の右側
```

内部では logical side として持ちます。

```rust
pub enum RubySide {
    Over,
    Under,
    InterCharacter,
}
```

## 2. RubyGroup

```rust
pub struct RubyGroup {
    pub base_cluster_range: ClusterRange,
    pub ruby_text: String,
    pub side: RubySide,
    pub base_advance: f32,
    pub ruby_advance: f32,
    pub overhang_before: f32,
    pub overhang_after: f32,
}
```

## 3. Ruby shaping

Ruby text は本文より小さい font size で独立 shape します。

```text
1. ruby text を style cascade する。
2. writing_mode は親に従う。
3. vertical ruby も orientation resolution を行う。
4. ruby run を shape する。
5. ruby_advance を計算する。
```

## 4. Ruby alignment

基本 alignment:

```text
if ruby_advance <= base_advance:
    ruby_start = base_start + (base_advance - ruby_advance) / 2
else:
    overhang = (ruby_advance - base_advance) / 2
    ruby_start = base_start - allowed_overhang_before
```

allowed overhang:

```text
- 隣接 ruby がなければ半分まで許容。
- 隣接 ruby と衝突するなら base expansion を試す。
- line edge では text bounds を超えないよう clamp。
```

## 5. Ruby collision resolution

line 内の ruby group を interval として扱います。

```text
interval = [ruby_start, ruby_end]
```

Algorithm:

```text
1. provisional interval を作る。
2. inline axis で sort。
3. 左から右/上から下に sweep し、重なり量を計算。
4. 重なりが小さい場合は adjacent ruby を少しずつ押す。
5. 押し出しが閾値を超える場合は base group の advance を拡張する。
6. paragraph line break DP に ruby_collision_estimate を戻す。
```

実装上は一発で完璧にしません。

```text
phase A: no overlap allowed, base expansion only
phase B: limited overhang
phase C: line-break feedback
```

## 6. Inter-character ruby

`RubyPosition::InterCharacter` は本文 cluster 間に ruby glyph を挿入します。これは通常 ruby より layout 影響が大きいです。

```text
base: 漢字
ruby: かんじ

vertical inter-character:
漢
か
ん
じ
字
```

方針:

```text
- RubyGroup ではなく InlineItem sequence の rewrite として扱う。
- base cluster の間に ruby cluster item を挿入する。
- selection/source map は ruby item を annotation として扱い、本文 caret と分離する。
```

## 7. text-combine-upright

Text-combine は「複数 glyph を 1em box に収める inline object」です。

```rust
pub struct TextCombineGroup {
    pub cluster_range: ClusterRange,
    pub text: String,
    pub shaped_horizontal_glyphs: Vec<ShapedGlyph>,
    pub inline_advance: f32, // usually 1em
    pub scale: f32,
    pub offset: Vector,
}
```

Algorithm:

```text
1. candidate digits run を検出。
2. horizontal shaping する。
3. natural_width を測る。
4. scale = min(1.0, em_size / natural_width)
5. baseline alignment を調整する。
6. group を縦書き inline axis 上の 1em item として置く。
7. group 内 glyph は horizontal coordinate のまま group-local transform で置く。
```

## 8. text-combine と ruby の相互作用

```text
- text-combine group 全体が ruby base になることは許可。
- ruby base range が text-combine group の一部だけを覆うのは禁止。
- ruby annotation 側に text-combine を適用するかは style で決める。
```

## 9. caret and selection

text-combine は内部文字に caret を置けるかが問題です。

方針:

```text
- navigation granularity = cluster なら内部 caret を許可。
- visual caret は group 内 horizontal coordinate に出す。
- simple dialogue mode では group 前後だけに caret を丸めてもよい。
```

`HitMap` は group-level cell と internal cell の二層にします。
