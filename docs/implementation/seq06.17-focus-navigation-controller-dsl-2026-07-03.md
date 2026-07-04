# seq06.17 Focus Navigation and Controller DSL

Date: 2026-07-03
Target: `Sanzentyo/arcweft` main, after seq06.16.1 component/View submit buttons and automatic prepared-frame spatial focus navigation.

## Intent

This package makes focus navigation an Arcweft-owned product UI contract instead of a DOM/native widget fallback. It keeps the existing player-rendered `TextField`, `TextArea`, `SecureField`, and `Button` activation substrate intact, and layers typed navigation metadata through the already shared runtime-driver → render-wgpu prepared frame → player-scene input path.

## Chosen DSL syntax

The final syntax stays inside the existing `component` / `View` DSL:

```arcw
pub component SettingsPanel() -> View {
  VStack(nav: .vertical, group: @group:.settings, wrap: false, initial: @button:.name, trap: .modal) {
    TextField(@input:.name, value: "Aster", label: "Name")
      .nav(right: @button:.apply, down: @input:.notes)

    TextArea(@input:.notes, value: "Notes", label: "Notes")
      .nav(up: @input:.name, right: @button:.apply)

    Button("Apply", id: @button:.apply)
      .nav(left: @input:.name, down: @button:.danger)

    Button("Disabled", id: @button:.danger, enabled: false)
      .nav(up: @button:.apply, left: auto)
  }
}
```

Container `nav:` declares a group axis and group defaults. `.nav(...)` declares per-target directional overrides. `auto`, `none`, and `boundary` are keywords. Entity references such as `@button:.apply` are lowered with the same normalization rules as the existing component/View lowering.

## Data model

The compact UI program resource gains two typed tables:

- `UiFocusGroupResource`: group id, parent group, axis, wrap policy, initial focus policy, disabled/hidden skip policy, and trap policy.
- `UiFocusNavigationResource`: one focusable target id, optional group id, and typed edges.

Edges are `UiFocusNavigationEdge { direction, target }` where direction is `up`, `down`, `left`, `right`, `next`, or `previous`, and target resolution is explicit target, `auto`, `none`, or `group_boundary`.

The runtime snapshot carries normalized runtime forms, and the renderer converts them to `PreparedFocusGraph`. `PreparedFocusGraph` is intentionally platform-free and uses `InteractionTarget`/`HitRect` evidence already prepared for semantic focus.

## Runtime behavior

Resolution order is deterministic:

1. If a focused target has an explicit edge for the requested direction, use it.
2. If that edge is `none`, stop.
3. If it is `boundary`, stop inside modal/trap groups and otherwise fall back to the parent/default graph boundary.
4. If it is an explicit target that is currently enabled and visible, move to it.
5. If the explicit target is disabled or invisible, use the group skip policy: `skip` falls back to auto; `stop` stops.
6. `auto` uses existing geometric scoring for spatial directions and deterministic linear order for `next`/`previous`.
7. If no DSL edge exists, preserve existing automatic spatial navigation as the default fallback.

Missing explicit target references are diagnosed during bundle lowering, before compact UI resources are emitted.

## Controller normalization

The shared `arcweft-player-scene::controller` module normalizes platform input to `NormalizedControllerAction`:

- keyboard arrows → spatial movement;
- Tab / Shift+Tab → `next` / `previous`;
- D-pad → spatial movement;
- left stick uses a dead zone and repeat gate (`dead_zone = 0.35`, `repeat_delay_ms = 320`, `repeat_interval_ms = 80`);
- confirm/action → activate focused button/choice/text submit;
- cancel/back → return a typed cancel outcome to the player host.

Native and Web adapters call the same `InputController` route. This keeps native/Web parity anchored to `PreparedFrame`, not to per-platform widget behavior.

## Accessibility and inspection

The prepared frame exposes `focus_graph`, `focus_debug()`, and navigation candidates. Browser frame observation reports include a focus section with the focused target, group count, target count, and current navigation candidates. This also gives agent observe/debug users the same graph that the player input controller uses.

## Intentional non-goals

- No DOM/native widget fallback.
- No compatibility shims or removed syntax aliases.
- No root-level broad re-export beyond deliberate facade exports already used by UI resources.
- No redesign of text editor, IME, or action button activation behavior.

## Applied Checkout Notes - 2026-07-04

Applied package:

- `D:/sanze/Downloads/seq06.17-focus-navigation-controller-dsl-package.zip`

Checkout evidence:

- Jujutsu working copy before final describe: `vqqwrqln` / `f48adcc6`, parent `85378107 main`.
- The additive patch applied cleanly; package docs, sample, and JSON fixture were copied into the repository.
- Existing-file wiring was adapted to the current checkout APIs where needed.

Implementation adjustments made during apply:

- `arcw check [PATH]` is now accepted as a direct source check route so the package validation command works against the current CLI.
- The sample gained `arcw.toml` plus a minimal public entrypoint/flow because current AWFB bundle validation requires a public entrypoint.
- `arcweft-render-wgpu` now depends on workspace `serde` so `FocusNavigationDebug` can be serialized into Web observation reports.
- The package's filename-style cargo test commands for `ui_focus_navigation_resources`, `controller_navigation`, and `focus_navigation_report` exit successfully but select zero tests in this checkout because Cargo treats those as test-name filters. The corresponding integration tests were also run explicitly with `--test`.

Validation run in this checkout:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle ui_focus_navigation_resources -- --nocapture
cargo test -p arcweft-bundle --test ui_focus_navigation_resources -- --nocapture
cargo test -p arcweft-lang-syntax focus_navigation_view -- --nocapture
cargo test -p arcweft-render-wgpu focus_navigation -- --nocapture
cargo test -p arcweft-player-scene controller_navigation -- --nocapture
cargo test -p arcweft-player-scene --test controller_navigation -- --nocapture
cargo test -p arcweft-player-web focus_navigation_report -- --nocapture
cargo test -p arcweft-player-web --test focus_navigation_report -- --nocapture
cargo clippy --all-targets --workspace -- -D warnings
cargo run -p arcweft-cli -- check samples/focus-navigation-controller-dsl/src/main.arcw
cargo run -p arcweft-cli -- bundle samples/focus-navigation-controller-dsl/src/main.arcw --output /tmp/focus-navigation-controller-dsl.awfb
```

Validation results:

- All commands above completed successfully.
- The three filename-style cargo filters noted above selected zero tests; explicit `--test` commands ran the intended integration tests.
- The sample `check` and `bundle` commands emitted existing style hints for compact declaration spelling but completed with `ok`.

Structural audit:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq06.17
```

Result:

- Scanned 2308 files, including 1111 Rust files and 519875 Rust physical LOC.
- Reported 4 error-level and 127 warning-level structural findings.
- Reports were written to `target/structure-audit-seq06.17`.
- Error-level findings are existing large-file thresholds in `crates/arcweft-cli/src/app/bundle.rs`, `crates/arcweft-core/src/value.rs`, `crates/arcweft-lang-sema/src/checker/expr.rs`, and `crates/arcweft-runtime-plan/src/flow.rs`.
- Changed files above the warning threshold remain review targets: `crates/arcweft-bundle/src/resource_codec/ui/model.rs`, `crates/arcweft-render-wgpu/src/geometry.rs`, and `crates/arcweft-runtime-driver/src/session.rs`.

Remaining validation gaps:

- Native window and browser/WebGPU interactive traversal were not manually exercised in this apply pass.
- The shared controller normalizer and `InputController::controller` route are implemented and tested, but platform gamepad event polling/bridging was not newly added or manually validated beyond the package's controller-normalizer acceptance scope.
