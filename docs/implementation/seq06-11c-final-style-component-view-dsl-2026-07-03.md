# Seq06.11c final style declaration and View DSL

> **Superseded Style-path premise (2026-07-13):** The provisional CSS syntax,
> source inventories, and flattened Style resource recorded below were removed
> by the [native-only typed Style path](native-only-style-path-2026-07-13.md).
> The body remains historical evidence only.

This cut applied the seq06.11c direction in production code: source authors use
root `style` declarations and View bodies. Product resources keep their compact
names such as `ViewStyleResource` and `ViewProgramResource`.

## Implemented

- Replaced the source language item `ViewStyleItem` / `Item::ViewStyle` /
  `HirTopLevelDecl::ViewStyle` with `StyleItem` / `Item::Style` /
  `HirTopLevelDecl::Style`.
- Added parser support for:
  - `style primary_button { ... }`
  - `style @.primary_button { ... }`
  - `style @style:.primary_button { ... }`
  - `style primary_button: .Css { ... }`
- Made bare and relative style declaration IDs module-aware. In `mod hoge`,
  `style primary_button`, `style @.primary_button`, and
  `style @style:.primary_button` normalize to `style.hoge.primary_button`.
- Added View body parsing using the existing
  `arcweft-lang-syntax::ast::view` substrate.
- Added View style modifiers:
  - `.style(@.name)` and `.style(@style:.name)` as module-local references;
  - `.style { ... }` as inline Arcweft style;
  - `.style(.Css) { ... }` as inline CSS.
- Lowered View authoring into deterministic product sidecars:
  `ViewProgramResource`, `ViewTextResource`, `ViewInputResource`, and inline style
  identities in `ViewStyleResource`.
- Changed DSL-authored top-level `style` lowering so inline `.Css` style bodies
  populate product `css_sources`, while Arcweft style bodies populate
  `arcweft_sources` and typed token/rule data.
- Migrated production samples away from the early top-level style spelling:
  `samples/css-style-parity/main.arcw` and
  `samples/native-text-input/src/main.arcw`.

## Design Decisions

- The removed early top-level style spelling falls through the ordinary invalid
  top-level item path. This avoids a parser branch that exists only to preserve
  historical syntax.
- Product resource and codec names remain `ViewStyleResource` /
  `ViewProgramResource`; the rename is source-language-facing only.
- Superseded on 2026-07-04: early top-level text-control declarations are no
  longer the current text-control resource declaration path. View-owned text
  controls are recorded in
  `docs/implementation/component-text-input-unification-2026-07-04.md`.

## Validation

- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets`
- `cargo check -p arcweft-cli --all-targets`
- `cargo test -p arcweft-lang-syntax --test style_component_view -- --nocapture`
- `cargo test -p arcweft-cli view_dsl_lowers_to_view_sidecars -- --nocapture`
- `cargo test -p arcweft-cli --test css_style_parity_sample -- --nocapture`
- `cargo test -p arcweft-cli --test native_text_input_sample_sidecars -- --nocapture`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit result: `files scanned: 2284`, `Rust files: 1103`,
`Rust physical LOC: 516122`, `package manifests: 91`, `4 error(s), 125
warning(s)`. The audit was a dry run and wrote no report files.
