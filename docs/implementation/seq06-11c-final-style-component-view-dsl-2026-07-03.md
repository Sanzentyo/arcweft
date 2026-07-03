# Seq06.11c final style declaration and component/View DSL

This cut applies the seq06.11c direction in production code: source authors use
root `style` declarations and `component ... -> View` bodies. Product resources
keep their existing compact names such as `UiStyleResource` and
`UiProgramResource`.

## Implemented

- Replaced the source language item `UiStyleItem` / `Item::UiStyle` /
  `HirTopLevelDecl::UiStyle` with `StyleItem` / `Item::Style` /
  `HirTopLevelDecl::Style`.
- Added parser support for:
  - `style primary_button { ... }`
  - `style @.primary_button { ... }`
  - `style @style:.primary_button { ... }`
  - `style primary_button: .Css { ... }`
- Made bare and relative style declaration IDs module-aware. In `mod hoge`,
  `style primary_button`, `style @.primary_button`, and
  `style @style:.primary_button` normalize to `style.hoge.primary_button`.
- Added component View body parsing for `component ... -> View { ... }` using
  the existing `arcweft-lang-syntax::ast::view` substrate.
- Added View style modifiers:
  - `.style(@.name)` and `.style(@style:.name)` as module-local references;
  - `.style { ... }` as inline Arcweft style;
  - `.style(.Css) { ... }` as inline CSS.
- Lowered component/View authoring into deterministic product sidecars:
  `UiProgramResource`, `UiTextResource`, `UiInputResource`, and inline style
  identities in `UiStyleResource`.
- Changed DSL-authored top-level `style` lowering so inline `.Css` style bodies
  populate product `css_sources`, while Arcweft style bodies populate
  `arcweft_sources` and typed token/rule data.
- Migrated production samples away from `ui style`:
  `samples/css-style-parity/main.arcw` and
  `samples/native-text-input/src/main.arcw`.

## Design Decisions

- `ui` is not treated as a reserved top-level keyword for the removed
  `ui style` spelling. `ui style ...` now falls through the ordinary invalid
  top-level item path, the same way `hoge style ...` would. This avoids a
  parser branch that exists only to preserve historical syntax.
- Product resource and codec names remain `UiStyleResource` /
  `UiProgramResource`; the rename is source-language-facing only.
- Superseded on 2026-07-04: top-level `ui text_input` / `ui text_area` /
  `ui secure_field` are no longer the current text-control resource
  declaration path. Component/View-owned text controls are recorded in
  `docs/implementation/component-text-input-unification-2026-07-04.md`.

## Validation

- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets`
- `cargo check -p arcweft-cli --all-targets`
- `cargo test -p arcweft-lang-syntax --test style_component_view -- --nocapture`
- `cargo test -p arcweft-cli component_view_dsl_lowers_to_ui_sidecars -- --nocapture`
- `cargo test -p arcweft-cli --test css_style_parity_sample -- --nocapture`
- `cargo test -p arcweft-cli --test native_text_input_sample_sidecars -- --nocapture`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit result: `files scanned: 2284`, `Rust files: 1103`,
`Rust physical LOC: 516122`, `package manifests: 91`, `4 error(s), 125
warning(s)`. The audit was a dry run and wrote no report files.
