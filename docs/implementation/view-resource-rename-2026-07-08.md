# View Resource Rename - 2026-07-08

## Summary

This cut completes the public/internal rename from the retained presentation
substrate to the View substrate for the active runtime/resource path.

The rename is direct. It does not add compatibility aliases, deprecated
re-exports, parser branches, serde aliases, or duplicate modules for the removed
legacy UI-prefixed names.

## Implemented

- The legacy UI-named bundle resource-codec module became
  `arcweft-bundle::resource_codec::view`.
- The active resource/table names are `ViewStyleResource`,
  `ViewProgramResource`, `ViewTextResource`, `ViewInputResource`,
  `ViewThemeResource`, and `ViewStyleTable`.
- Compact product section families now use `View`, `ViewProgram`,
  `ViewStyle`, `ViewText`, `ViewInput`, and `ViewTheme`.
- View resource magic bytes use View-oriented tags:
  `AWVW`, `AWVP`, `AWVS`, `AWVT`, `AWVI`, and `AWVH`.
- Bundle sidecars use `view.program.json`, `view.style.json`,
  `view.text.json`, `view.input.json`, and `view.theme.json`.
- Runtime/presentation naming uses `GameView`, `HtmlView`, `NativeView`,
  `LocalView`, `AgentViewTree`, and `ViewEntity`.
- Scheduler/native bench counters use `local_view` for `TaskClass::LocalView`.
- The web IME player-rendered fixture builder now imports
  `arcweft_bundle::resource_codec::view`, calls `with_view_*`, and constructs
  only `View*` resource types.
- `choice option` metadata uses `view { ... }` in parser docs and fixtures.
- `receive action` lowers to `view.action.await` / `view.action`.
- The character retained-image adapter crate is now `arcweft-character-view`.
- The reactive style sample moved from `samples/reactive-ui-style` to
  `samples/reactive-view-style`, with `reactive-view.*` style sidecars and
  `.reactive-view` CSS selectors. Active follow-up request prose now targets
  the renamed sample and View authoring terminology.
- The modern feedback sample now lives at `samples/modern-feedback-view`, and
  its package name and fixture references use `modern-feedback-view`.
- Web IME rendered-fixture evidence now lives at
  `fixtures/web-ime-player-rendered/view-runtime-text-controls.json`; the
  fixture-owned sans stack id is now `view-sans`.
- The retained View fixture now lives under `docs/fixtures/retained-view/`.
- Focus-navigation fixture naming now uses
  `focus-navigation-expected-view-program.json` and
  `view.program.focus_navigation_controller_sample`.
- Stable presentation/schema/tooling docs now use `view_tree`, `view_layout`,
  `view_tree_order`, `@layer.view.*`, `layer.view.*`, and `choice_view`
  spellings for View-owned layers and input routing.
- The stable reactive presentation docs now use `view-reactive.md` and
  `reactive-view.md`.
- The follow-up cleanup also replaced the remaining View-owned schema/example
  fields that still used the old node/capture spelling with
  `target_view_node`, `focused_view_node`, and `include_view`.
- The Web IME sample diagnostic flag now uses `visibleDomTextView`.
- Active design, implementation, and request-file slugs that named the old
  View-owned UI concepts now use `view-style`, `view-scene`,
  `view-interaction`, `reactive-view`, `modern-feedback-view`, and
  `retained-view` spellings.

## Deliberate Remaining `ui` Spellings

- External API names such as `Uint8Array`, `UITextInput`, and CSS generic font
  families such as `ui-sans` remain unchanged.
- Platform-owned IME/composition/candidate UI references remain unchanged when
  the text is specifically about native OS UI rather than Arcweft View content.
- Font family names such as `Yu Gothic UI` and `Segoe UI` remain unchanged.
- Rust trybuild's conventional `tests/ui` directory naming remains unchanged.
- Historical review documents are not mechanically rewritten unless they are
  active request/spec handoff material.

## Validation

- `cargo fmt --all`
- A source search for legacy UI-prefixed Rust identifiers and the removed
  UI-named resource-codec/helper paths across crates/tools/build metadata
  returned no active implementation matches.
- Follow-up source-gate and parser fixture cleanup removed the remaining
  active old top-level UI declaration strings from tests/tools; external names,
  sample slugs, and historical design prose remain outside this direct API
  rename.
- The legacy style table/resource/rule identifiers are now `View*` identifiers;
  a repository search for active Arcweft-owned legacy UI-prefixed Rust
  identifiers returns no matches.
- Legacy UI-prefixed style table/resource/rule and runtime Rust identifiers have
  no active implementation matches; the retained style table is
  `arcweft_view::ViewStyleTable`.
- Stable docs and active request text now use `@view.*`, `view.open`, and
  View-owned text-control language instead of the old View-resource predecessor
  examples.
- Stable docs and schema examples no longer contain the old View-tree,
  View-layout, View-layer, or choice-View predecessor spellings.
- Stable docs under `docs/00-overview` through `docs/05-build-and-security`,
  `docs/schemas`, and `docs/examples` no longer use standalone `UI` / `ui`
  for Arcweft-owned View concepts.
- Stable docs/schema now also avoid the old View-predecessor node/capture field
  names; active Web sample diagnostics use `visibleDomTextView`.
- Follow-up cleanup also removed the active sample/fixture exceptions for the
  previous modern-feedback sample slug, underscore-form fixture names, Web IME
  runtime text-control fixture filename, and retained fixture directory;
  remaining hits are historical structural-audit snapshots.
- `cargo fmt --all --check`
- `cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-view -p arcweft-presentation -p arcweft-bundle -p arcweft-character-view -p arcweft-player-scene -p arcweft-render-native -p arcweft-runtime-host -p arcweft-cli --all-features`
- `cargo test -p arcweft-lang-syntax --all-features`
- `cargo test -p arcweft-lang-sema --all-features choice`
- `cargo test -p arcweft-runtime-plan --all-features receive_action_lowers_to_view_action_host_call -- --exact`
- `cargo test -p arcweft-bundle --all-features --test view_resource_codecs`
- `cargo test -p arcweft-bundle --all-features --test view_action_button_resources --test view_focus_navigation_resources --test view_runtime_text_controls`
- `cargo test -p arcweft-cli --all-features --test reactive_view_style_sample_sidecars`
- `cargo test -p arcweft-cli --all-features app::bundle::tests::view_dsl_lowers_to_view_sidecars -- --exact`
- `cargo test -p arcweft-cli --all-features app::bundle::tests::view_box_and_scroll_lower_to_typed_view_resources -- --exact`
- `cargo test -p arcweft-view -p arcweft-character-view --all-features`
- `cargo run -p arcweft-cli --all-features -- check samples/reactive-view-style/src/main.arcw`
- `cargo run -p arcweft-cli --all-features -- check --manifest-path samples/modern-feedback-view/arcw.toml`
- `cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --help`
- `cargo check -p arcweft-runtime-scheduler -p arcweft-runtime-host -p arcweft-cli -p arcweft-takumi-adapter -p arcweft-lang-syntax --all-targets --all-features`
- `cargo test -p arcweft-lang-syntax --all-features --test style_view`
- `cargo +nightly -Zscript tools/source-gates/seq06_4j1_native_ime_player_rendered_gates.rs --root .`
- `cargo test -p arcweft-cli --all-features --test reactive_view_style_sample_sidecars -- --nocapture`
- `cargo test -p arcweft-runtime-host --all-features execute_plan_reports_handler_failure_index_without_running_later_actions -- --nocapture`
- `cargo clippy -p arcweft-runtime-scheduler -p arcweft-runtime-host -p arcweft-cli -p arcweft-takumi-adapter -p arcweft-lang-syntax --all-targets --all-features` exited 0. It still reports pre-existing warnings in crates outside this follow-up rename patch.
- `cargo clippy --workspace --all-targets --all-features` exited 0. It still
  reports pre-existing warnings outside this rename slice in
  `arcweft-lang-syntax`, `arcweft-render-wgpu`, `arcweft-runtime-host`,
  `arcweft-player-web`, and `arcweft-player-native`.
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/view-resource-rename-2026-07-08`
- `cargo fmt --all --check`
- `cargo check -p arcweft-view -p arcweft-bundle -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features`
- Search gates for removed View-predecessor Rust identifiers, resource-codec
  spellings, schema fields, and Web sample diagnostic flags returned no active
  matches outside historical audit outputs.
- Search gates for stable docs under `docs/README.md`,
  `docs/00-overview` through `docs/05-build-and-security`, `docs/schemas`, and
  `docs/examples` returned no standalone View-predecessor `ui` spellings.
- Search gates for active code/docs/request file paths and content returned no
  old View-owned sample/resource/layer slugs or legacy UI-prefixed Rust
  identifier matches outside historical structure-audit snapshots.
- Follow-up gates on 2026-07-08 specifically searched for
  `UiStyleTable`, `UiStyleRule`, `UiStyleResource`, `UiRuntime*`, active
  Arcweft-owned `Ui*` Rust identifiers, and old View-owned `ui-*` slugs across
  crates, samples, tests, fixtures, tools, web assets, and docs. They returned
  no active matches outside excluded historical audit/pro-review snapshots.

## Structural Audit

The structural audit report is checked in under
`docs/implementation/structure-audits/view-resource-rename-2026-07-08/`.

Current audit summary:

- 2445 files scanned.
- 1170 Rust files.
- 573743 Rust physical LOC.
- 91 package manifests.
- 1 error and 148 warnings.

The error is the existing large
`crates/arcweft-cli/src/app/bundle_view.rs` module at 2621 physical LOC. This
rename cut touches that file but does not resolve its larger decomposition
need; the audit preserves the follow-up boundary.
