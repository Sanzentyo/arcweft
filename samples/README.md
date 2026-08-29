# Arcweft samples

This directory contains structured sample Arcweft projects. Each sample is
kept separate from `docs/examples/`: design examples live in `docs/`, while
these directories are project-shaped fixtures that can be copied, checked, and
run with `arcw`.

## Samples

- [visual-novel-mini](visual-novel-mini/README.md) - a small narrative project
  with game, CLI, server, test, and bench entry examples.
- [rich-text-showcase](rich-text-showcase/README.md) - rich-text rendering
  showcase with Windows fonts, vertical snippets, canonical nested typography
  defaults, authored dialogue-View styles, speaker preset overrides, and line ruby
  overrides for Agent observe provenance checks.
- [rich-text-full-grammar.arcw](rich-text-full-grammar.arcw) - broad
  rich-text grammar sample covering ruby forms, interpolation, control tags,
  recognized dot selectors, explicit tag families, family-relative dialogue
  defaults, and line option overrides.
- [rich-text-fx.arcw](rich-text-fx.arcw) - typed reusable presentation Fx with
  required/default named parameters, ordered composition, View `.fx(...)`, and
  dialogue `[fx ...]...[/fx]` application.
  profiles for provenance-aware runtime-plan and LSP cascade checks.
- [native-style-parity](native-style-parity/README.md) - image-free Web/native
  renderer parity sample for typed native Style and choice rendering.
- [native-style-layout-coverage](native-style-layout-coverage/README.md) -
  retained View layout and native Style application coverage sample.
- [vertical-writing-style](vertical-writing-style/README.md) - an authored
  dialogue View and `pub style` sample with visible vertical-rl/vertical-lr,
  ruby, text-combine-upright, and loose/strict JLREQ output.
- [native-text-input](native-text-input/README.md) - native player IME sample
  with text controls declared in Arcweft DSL and styled by retained View style
  resources.
- [text-submit-flow](text-submit-flow/README.md) - DSL-authored text input
  submit sample that waits for Enter/IME send and branches on submitted text
  length.
- [function-curried-call-groups](function-curried-call-groups/README.md) -
  pure function sample showing `f(a, b)(c)` and `f(a)(b)(c, d)` call groups
  without flattening.
- [rich-text-windows-fonts.arcw](rich-text-windows-fonts.arcw) - Windows
  default font comparison sample using nested character `dialogue_style`
  typography for horizontal, mixed, and vertical text.
