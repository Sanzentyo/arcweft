# Legacy UI/View rename concrete targets

Date: 2026-07-09

## Source

Applied package:

```text
D:/sanze/Downloads/arcweft-legacy-ui-view-rename-concrete-targets-2026-07-07.zip
```

The package target is seq 06.16.6.3 cleanup after the retained View Rust
boundary had already moved to `View*` types. It removes stale active
`component` vocabulary from source gates, Justfile recipes, tests, and stable
design docs without adding compatibility aliases.

## Implemented

- Renamed `Justfile` recipes:
  - `component-text-input-native-smoke-check` -> `view-text-input-native-smoke-check`
  - `component-text-input-native-smoke` -> `view-text-input-native-smoke`
- Updated the native interactive smoke test to assert the new `view-*` recipe
  names.
- Renamed the stable layout-bounds design doc to:

```text
docs/design/view-text-control-layout-bounds-resource-contract-2026-07-04.md
```

- Updated implementation docs that referenced the old active recipe or the old
  stable design-doc path.
- Updated stable scoped-presentation-handle design examples from the removed
  component-scoped constructor vocabulary to `view`-scoped constructor
  vocabulary.
- Confirmed the seq06.4j.1 source gate already expected
  `pub view NativeTextInputPanel()` and `view-authored` controls in this
  checkout.

## Non-goals and remaining terminology

No compatibility recipe aliases were added.

The strict search gate has no remaining hits in `crates/`, `tools/`,
`samples/`, or `docs/design/`. Remaining hits are historical implementation
notes that document prior states or rejected old names:

- `docs/implementation/component-text-input-unification-2026-07-04.md`
- `docs/implementation/2026-07-04-component-scoped-render-capture.md`
- `docs/implementation/scoped-presentation-handles-final-ui-syntax-2026-07-06.md`
- `docs/implementation/seq06-2-takumi-css-scene-to-wgpu-lowering.md`

Those files are not active authoring, source-gate, recipe, parser, runtime, or
serde surfaces in this cut.

## Validation

Executed:

```text
cargo fmt --all
cargo +nightly -Zscript tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs --root .
cargo test -p arcweft-cli --all-features --test native_text_input_native_interactive_smoke --quiet
cargo test -p arcweft-takumi-adapter --all-features --quiet
cargo test -p arcweft-view --all-features --quiet
cargo check -p arcweft-view -p arcweft-takumi-adapter -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-cli --all-features --test native_text_input_sample_sidecars --quiet
cargo clippy -p arcweft-view -p arcweft-takumi-adapter -p arcweft-cli --all-targets --all-features
just view-text-input-native-smoke-check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/legacy-ui-view-rename-concrete-targets-2026-07-09
```

Results:

- All package-targeted compile/test/source-gate checks passed.
- `just view-text-input-native-smoke-check` passed with the existing
  `method-call receiver is lowered as first intrinsic argument` warning during
  the `text-submit-flow` bundle check.
- The package named `arcweft-ui`; the current workspace package is
  `arcweft-view`, so validation used `arcweft-view`.
- Structure audit wrote:

```text
docs/implementation/structure-audits/legacy-ui-view-rename-concrete-targets-2026-07-09/
```

The audit reported existing error-level size violations in:

- `crates/arcweft-cli/src/app/bundle_view.rs`
- `crates/arcweft-player-scene/src/input.rs`

Those files were not expanded by this cleanup package.
