# Algorithm 01: segmentation と vertical orientation

## 1. なぜ難しいか

縦書きの最初の難所は「文字ごとに回すかどうか」ではありません。正しくは、**grapheme cluster ごとに writing mode と text orientation を解決する**ことです。

例:

```text
A        mixed では sideways
Ａ       upright
あ       upright
。       vertical alternate または位置調整
ー       vertical alternate が必要
🧑‍💻     cluster 全体を壊さない
```

## 2. 入力

```rust
struct OrientationInput<'a> {
    text: &'a str,
    writing_mode: WritingMode,
    text_orientation: TextOrientation,
    vertical_latin: VerticalLatinMode,
    text_combine: TextCombinePolicy,
}
```

## 3. Unicode property

production では Unicode UAX #50 の `Vertical_Orientation` data から generated table を作ります。

```rust
pub enum VerticalOrientation {
    Upright,              // U
    Rotated,              // R
    TransformedUpright,   // Tu
    TransformedRotated,   // Tr
}
```

生成方法:

```text
1. Unicode release に対応する VerticalOrientation.txt を取得する。
2. codepoint range → property を parse する。
3. range table を sorted non-overlap に正規化する。
4. Rust const slice と binary search function を生成する。
5. Unicode version を table metadata に入れる。
```

API:

```rust
pub fn vertical_orientation(ch: char) -> VerticalOrientation;
pub fn vertical_orientation_cluster(cluster: &str) -> VerticalOrientation;
```

cluster 内に複数 codepoint がある場合:

```text
- variation selector / combining mark は base に従う。
- emoji ZWJ sequence は cluster 全体を Upright または Rotated へ丸める。
- 不明な結合は base character の property を採用する。
```

## 4. Resolved orientation

```rust
pub enum ResolvedOrientation {
    Upright,
    SidewaysClockwise,
    SidewaysCounterClockwise,
}
```

決定表:

| writing_mode | text_orientation | VO | result |
|---|---|---|---|
| HorizontalTb | any | any | Upright |
| Vertical* | Upright | any | Upright |
| Vertical* | Sideways | any | SidewaysClockwise |
| Vertical* | Mixed | U | Upright |
| Vertical* | Mixed | Tu | Upright + vertical feature required |
| Vertical* | Mixed | R | SidewaysClockwise |
| Vertical* | Mixed | Tr | SidewaysClockwise or font-provided rotated alternate |

`VerticalLr` でも East Asian vertical typesetting の sideways は通常 clockwise です。ただし Mongolian 等を視野に入れる場合は script-specific rule を拡張します。

## 5. text-combine-upright detection

縦中横は orientation resolution の前後で専用 group にします。

```text
candidate:
  - ASCII digits 2〜4 桁
  - optional punctuation: / . - :
  - author style: text_combine = Auto | Digits(n) | All
reject:
  - line break / style break を跨ぐ
  - ruby base range を部分的に跨ぐ
  - bidi embedding boundary を跨ぐ
```

Algorithm:

```text
scan cluster index i:
  1. cluster[i] が ASCII digit なら run start。
  2. 最大 n cluster まで digit/punctuation を読む。
  3. width estimation で 1em 内に圧縮可能か試算。
  4. group を TextCombineGroup に置換。
```

`TextCombineGroup` は中の glyph を横組み shape し、group 全体を vertical line 内の 1em square に収めます。

## 6. 約物と small kana

`Tu` は upright だが vertical alternate または位置調整が必要な cluster です。

```text
- 。、：「」『』（）
- small kana: ゃゅょっァィゥェォッ
- prolonged sound mark: ー
```

処理順:

```text
1. font が vert/vrtr alternate を持つなら shaping feature に任せる。
2. alternate がなければ layout fallback adjustment を使う。
3. fallback adjustment は glyph bbox を 1em box 内で visual center へ補正する。
```

fallback adjustment は font ごとの差が大きいため、最終的には font metrics probe cache を持ちます。

## 7. Data structures

```rust
pub struct OrientedCluster {
    pub range: ByteRange,
    pub cluster_index: u32,
    pub vertical_orientation: VerticalOrientation,
    pub resolved: ResolvedOrientation,
    pub requires_vertical_alternate: bool,
    pub style_run: StyleRunId,
}
```

`requires_vertical_alternate` は shaping plan の feature policy へ渡します。
