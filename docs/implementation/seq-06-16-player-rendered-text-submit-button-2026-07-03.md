# seq-06.16 player-rendered text submit button

## Status

Seq-06.16 is applied as resource/runtime/player substrate, not as a final
authoring syntax cut.

The initial package suggested adding a top-level `ui action_button` item next
to `ui text_input` and `ui style`. That surface is intentionally not kept,
because the final UI language direction is component/View authoring with root
`style` declarations. The follow-up request is:

- `docs/reviews/requests/2026-07-03-seq-06.16.1-component-view-text-submit-button-lowering.md`

## Applied Substrate

- `UiProgramResource` now carries action button records.
- Compact UI resource codec canonicalization, budgets, deterministic public ID
  collection, and record counts include action buttons.
- Runtime presentation snapshots carry resolved action buttons.
- Player-scene lowers runtime action buttons into WGPU render scene button
  nodes.
- Pointer and keyboard activation of a prepared action button emits the same
  text-control submit write-back kind used by Enter/IME send.
- IME composition submit policy is typed as commit, cancel, or reject with a
  structured input diagnostic.

## Intentionally Not Applied

- No top-level `ui action_button` parser branch.
- No `UiActionButtonItem` AST/HIR/sema/project-index item.
- No `ui action_button` sample syntax.
- No DOM/native platform button fallback.

## Follow-Up Boundary

Seq-06.16.1 must design and implement the authored component/View surface that
lowers to this substrate. The expected shape is a `Button` View element with a
typed click action such as text submit, styled through the root `style`
direction rather than another resource-side `ui` declaration.

## Validation

- `cargo fmt --all`
- `cargo test -p arcweft-bundle --test ui_action_button_resources -- --nocapture`
- `cargo test -p arcweft-player-scene --test action_button_submit -- --nocapture`
- `cargo test -p arcweft-render-wgpu --test geometry -- --nocapture`
- `cargo run -p arcweft-cli -- compile --emit check samples/text-submit-flow/src/main.arcw`
- `cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `git diff --check`

Structure audit completed successfully and reported the current workspace
hotspot total as `4 error(s), 125 warning(s)`. The new seq-06.16 Rust files are
below the review thresholds.
