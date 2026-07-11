# Arcweft samples

This directory contains structured sample Arcweft projects. Each sample is
kept separate from `docs/examples/`: design examples live in `docs/`, while
these directories are project-shaped fixtures that can be copied, checked, and
run with `arcw`.

## Samples

- [visual-novel-mini](visual-novel-mini/README.md) - a small narrative project
  with game, CLI, server, test, and bench entry examples.
- [rich-text-showcase.arcw](rich-text-showcase.arcw) - rich-text rendering
  showcase with Windows fonts, vertical snippets, canonical nested typography
  defaults, textbox theme defaults, speaker preset overrides, and line ruby
  overrides for Agent observe provenance checks.
- [rich-text-full-grammar.arcw](rich-text-full-grammar.arcw) - broad
  rich-text grammar sample covering ruby forms, interpolation, control tags,
  inferred dot selectors, explicit tag families, family-relative dialogue
  defaults, and line option overrides.
- [rich-text-fx.arcw](rich-text-fx.arcw) - typed reusable presentation Fx with
  required/default named parameters, ordered composition, View `.fx(...)`, and
  dialogue `[fx ...]...[/fx]` application.
- [rich-text-profiled](rich-text-profiled/README.md) - project-shaped rich-text
  sample with launch profiles that select different `dialogue defaults`
  profiles for provenance-aware runtime-plan and LSP cascade checks.
- [css-style-parity](css-style-parity/README.md) - image-free Web/native
  renderer parity sample for CSS-like text and choice styling.
- [css-layout-cascade-coverage](css-layout-cascade-coverage/README.md) -
  retained View CSS layout/cascade coverage sample and fixture entry for seq06.12.
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
