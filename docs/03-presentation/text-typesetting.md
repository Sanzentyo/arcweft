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
        wrap = container
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

Dialogue defaults, authored View styles, character `dialogue_style`, speaker presets,
line options, and inline spans all contribute to the same effective RichText
style. Records deep-merge by field. The nearest explicit field wins, while
unspecified sibling fields continue to inherit from lower-priority defaults.

Ruby-specific fields mean:

| Field | Meaning |
|---|---|
| `position` | Default ruby track: `over`, `under`, `inter_character`, or `auto`. As in CSS Ruby, `inter_character` is a right-side `vertical-rl` inline annotation in horizontal text and has the same layout effect as `over` in vertical text. |
| `size` | Annotation font size |
| `gap` | Extra block-axis separation between the base and annotation containers. The default is `0px`, matching CSS Ruby's no-intervening-space stacking. |
| `overhang` | Maximum inline-axis annotation overhang allowed beyond the base allocation. `0px` disables overhang; the default deterministic `auto` policy is half an annotation cell. |
| `collision_gap` | Extra separation between adjacent ruby annotations or continuation tracks. The default is `0px`. |

Arcweft follows the CSS Ruby box model rather than copying a browser's CSSOM
element rectangles as layout offsets. Interlinear annotation containers stack
outward from the base container without intervening space; an authored `gap`
adds space to that boundary. Browser `ruby` and `rt` rectangles can overlap
because they expose font metrics inside those abstract containers, even when
the rendered glyph ink does not collide. Applying that measured rectangle
overlap directly to Arcweft's glyph cells caused the annotation ink to enter
the base glyph, especially in vertical text, so it is intentionally not part of
the canonical layout formula. `ruby-overhang` remains an inline-axis edge
effect: it can share space with adjacent content but never authorizes annotation
ink to collide with the contents of its own base.

### Horizontal wrapping

`horizontal_tb` RichText wraps inside its View container width by default.
The current deterministic layout model places one visual cluster at a time and
starts a new line before a cluster that would exceed `origin.x + size.width`.
Explicit hard line breaks reset `x` to the content origin and advance `y` by the
line advance. A single cluster wider than the container is placed at the line
start and may overhang; this keeps geometry deterministic until word-aware
UAX14 wrapping and overflow diagnostics are implemented.

### Vertical column break quality policy

Vertical writing uses the closed `balanced_v1` `VerticalBreakPolicy` in the
renderer-independent `arcweft-text-layout` planner. `vertical_rl` and
`vertical_lr` therefore share the same inline break plan.

Priority is normative and lexicographic:

1. remove UAX #14/JLREQ-prohibited boundaries and generated keep-together pairs;
2. minimize non-hanging forced overflow;
3. score allowed hanging, intermediate raggedness, final-column shortness,
   generated pair preference, and intermediate-column creation;
4. minimize column count;
5. prefer lexicographically later break offsets for deterministic fill-forward
   ties.

Physical metrics are normalized to 1/4096 of the lower median positive shaped
cluster advance. Objective comparison is integer-only after normalization.
Uniform scaling preserves decisions except when an input crosses the documented
half-quantum normalization boundary. The final column has no ordinary
raggedness or break cost; only a bounded short-final penalty below
`min(capacity / 3, 2em)` applies after at least one preceding column.

Closing punctuation and middle dots may use at most half of their already
resolved cluster advance as hanging. Hanging is a bounded soft cost; overflow
beyond that allowance is a forced-overflow escape and is considered only for
the shortest legal fragment from a column start. The generated JLREQ tables
remain the sole source of punctuation class, keep-together, and pair-penalty
facts.

The request carries only the closed `balanced_v1` identity. Unknown serialized
policy names are rejected, arbitrary weights are not authored, and the policy
identity participates in `TextLayoutHash`. Policy changes require curated
corpus delta review; regenerating expectations is not approval.

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


