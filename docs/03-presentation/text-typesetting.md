# Text / RichText / Typst

Text は二階層に分ける。

```text
RichText:
  会話、View、HUD、毎frame利用。

TypesetBlock:
  Typst級の文書、数式、図鑑、クレジット。
  lazy precompile + cache 前提。
```

## RichText

Rich-text layout, transform, effect, shader, and capture semantics are split
across this document,
[Rich Text Effects and Transforms](rich-text-effects-transforms.md), and
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md).

```arcw
alice.say()[
今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。
]
```

サポート:

- font fallback
- shaping
- bidi
- line breaking
- ruby
- emphasis marks
- inline image/icon
- inline animation marker
- color / gradient
- outline / shadow
- typewriter reveal
- hit testing
- selection
- vertical writing roadmap

### RichText style defaults

RichText typography is configured as structured style data, not as a flat list
of dialogue-only knobs. Dialogue, choices, View labels, logs, and HUD text can all
consume the same model.

```arcw
rich_text {
    text {
        font = "Yu Gothic"
        size = 30px
        color = rgb("#f5f5f5")
    }

    layout {
        writing_mode = horizontal_tb
        jlreq = normal
        vertical_latin = mixed
        wrap = textbox
        overflow = page
    }

    ruby {
        position = over
        size = 14px
        gap = 2px
        overhang = 7px
        collision_gap = 2px
    }
}
```

Dialogue defaults, textbox themes, character `dialogue_style`, speaker presets,
line options, and inline spans all contribute to the same effective RichText
style. Records deep-merge by field. The nearest explicit field wins, while
unspecified sibling fields continue to inherit from lower-priority defaults.

Ruby-specific fields mean:

| Field | Meaning |
|---|---|
| `position` | Default ruby track: `over`, `under`, `inter_character`, or `auto` |
| `size` | Annotation font size |
| `gap` | Extra separation from the default ruby track; for horizontal over-ruby, `0px` keeps the CSS/HTML-like natural overlap between the annotation bbox and base bbox instead of forcing the boxes to merely touch |
| `overhang` | Maximum annotation overhang allowed beyond the base allocation |
| `collision_gap` | Separation between adjacent ruby annotations or continuation tracks |

Horizontal over-ruby is compared against browser `<ruby><rb><rt>` behavior.
With a 30px base font, 13px ruby font, and `line-height: 1`, Chromium places the
`rt` bbox about 4.67px into the `rb` bbox. Arcweft models this as a
`0.36em` annotation overlap before applying the explicit ruby `gap`, so
authoring `gap = 0px` remains close to standard HTML ruby placement.

### Horizontal wrapping

`horizontal_tb` RichText wraps inside the textbox layout width by default.
The current deterministic layout model places one visual cluster at a time and
starts a new line before a cluster that would exceed `origin.x + size.width`.
Explicit hard line breaks reset `x` to the textbox origin and advance `y` by the
line advance. A single cluster wider than the textbox is placed at the line
start and may overhang; this keeps geometry deterministic until word-aware
UAX14 wrapping and overflow diagnostics are implemented.

## Content functions

```arcw
fn ruby(base: Content, ruby: Content) -> Content
fn emph(content: Content) -> Content
fn strong(content: Content) -> Content
fn color(color: Color)(content: Content) -> Content
fn font(spec: FontSpec)(content: Content) -> Content
fn inline_icon(icon: Ref<Vector>) -> Content
fn math(src: String) -> Content
```

## TypesetBlock

```arcw
pub typeset @typeset.credits typst {
    engine = typst
    source = """
    @set text(font: "Noto Serif CJK JP", size: 18pt)
    #align(center)[
      = Staff

      Scenario: Alice \\
      Engine: Arcweft
    ]
    """
    page.width = 720pt
    page.height = auto
}
```

使用:

```arcw
let doc = try await typeset(@typeset.credits) with {
    pending p => { scene.show(@scene.loading_typeset); text.show("組版中"); progress.set(p.ratio) }
}

TypesetView(doc).scroll()
```

## Typeset cache

```rust
pub struct TypesetCacheKey {
    pub source_hash: Hash,
    pub font_set_hash: Hash,
    pub page_size: SizeSpec,
    pub locale: Locale,
    pub theme_hash: Hash,
}
```

## Agent observation

Text run ごとに bbox/source/ruby/style/presentation を返せる。画像取得、
layer/object/rich-text child capture、raw RGBA/PNG、座標変換の契約は
[Agent Observe and Capture Contract](../04-tooling/agent-observe-capture-contract.md)
に従う。

```rust
pub struct TextRunObservation {
    pub text: String,
    pub bbox: BBox,
    pub baseline: f32,
    pub ruby: Option<String>,
    pub style: TextStyleSummary,
    pub presentation: RichTextPresentationSummary,
    pub source: Option<SourceAnchor>,
}
```


