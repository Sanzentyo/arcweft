# Vertical Writing Style

This sample is a compact, visible reference for styling an authored dialogue
View that presents vertical rich text. It uses only the current language
surface:

- `pub style vertical_writing_showcase` styles the panel, speaker, content, and
  transparent primary-action hit target;
- `pub view VerticalWritingShowcase(dialogue: DialogueView)` owns the complete
  presentation geometry; and
- `pub dialogue defaults` selects that View and supplies ruby defaults.

The responsibilities are deliberately separate. `pub style` and the single
View-root `.style(...)` application control the authored View's visual
presentation through typed descendant `.part(...)` identities;
the typed rich-text layout selectors `[.vertical_rl]` and `[.vertical_lr]`
select writing direction and JLREQ policy for their content runs. View style
does not currently define a second `writing-mode` property, so this sample does
not invent a sample-only alias for one. The capture check observes the
speaker's authored 18 px size together with its font family and color.

The four dialogue pages demonstrate:

1. `vertical-rl`, upright Japanese punctuation, sideways Latin, ruby, and the
   automatically combined `2026` digit cluster;
2. `vertical-lr`, ruby-under, CSS-equivalent vertical `inter-character` (the
   same physical track as over-ruby), sideways Latin, and the same
   text-combine-upright behavior; and
3. the same constrained punctuation pair with `jlreq=loose` and
   `jlreq=strict`, making the authored line-breaking policy visible in separate
   frames.

Run:

```bash
just vertical-writing-style-sample
```

The command checks and bundles `main.arcw`, then uses the shared headless WGPU
text capture path to write four PNGs and their typed frame-observation reports
under `target/vertical-writing-style/`. It also repeats the vertical-RL capture
at the same logical time and requires an identical SHA-256 hash, providing a
small deterministic-rendering smoke check without maintaining a platform-font
golden in the repository. The recipe reads the structured reports to require
the styled speaker/content colors and the expected `vertical_rl`/`vertical_lr`
writing-mode, ruby, and text-combine-upright values. It samples the
deterministic RGBA8 capture to require the authored panel background, and also
requires loose and strict JLREQ frames to differ.

The representative outputs are:

- `target/vertical-writing-style/native-vertical-rl.png`
- `target/vertical-writing-style/native-vertical-lr.png`
- `target/vertical-writing-style/native-jlreq-loose.png`
- `target/vertical-writing-style/native-jlreq-strict.png`

`reference.html` is the browser-side visual reference for the vertical-RL ruby
case. It uses the same bundled Noto Sans JP face, 42 px base text, 14 px ruby,
1 px authored separation, writing mode, viewport, and panel geometry as the
sample. It also exposes `window.rubyGeometry()` so a browser inspection can
record the base, annotation, and ruby CSSOM rectangles. Those rectangles are
useful implementation evidence, but visible glyph collision and the CSS Ruby
container-stacking rules remain the acceptance criteria.
