# Text / RichText / Typst

Text は二階層に分ける。

```text
RichText:
  会話、UI、HUD、毎frame利用。

TypesetBlock:
  Typst級の文書、数式、図鑑、クレジット。
  lazy precompile + cache 前提。
```

## RichText

```arcw
alice.say()[
今日は少しだけ、{ruby "変な夢" "へんなゆめ"}を見たんだ。
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

Text run ごとに bbox/source/ruby/style を返せる。

```rust
pub struct TextRunObservation {
    pub text: String,
    pub bbox: BBox,
    pub baseline: f32,
    pub ruby: Option<String>,
    pub style: TextStyleSummary,
    pub source: Option<SourceAnchor>,
}
```


