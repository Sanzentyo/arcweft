# Player Scene Input Module Split

Date: 2026-07-09

## Summary

This cut resolves the structural audit error for
`crates/arcweft-player-scene/src/input.rs` by splitting the shared player input
implementation into responsibility modules without changing runtime behavior.

The public module remains `arcweft_player_scene::input`; the split is internal:

- `input.rs`: public input types, `InputController` storage, common focus/text
  selection helpers, activation helpers, and private shared utilities.
- `input/state.rs`: snapshots, basic accessors, live text-control state
  retention, and choice autofocus.
- `input/pointer.rs`: pointer move/down/up/cancel/context-menu routing and
  selectable text-block pointer selection.
- `input/keyboard.rs`: keyboard, IME shortcut gating, controller normalization,
  focus movement, and dialogue reveal/advance gating.
- `input/text_edit.rs`: platform text input application, clipboard outcomes,
  text write-back construction, and clipboard request bookkeeping.
- `input/scroll.rs`: wheel/precision scroll, scroll-by-id, page/edge scrolling,
  and scroll offset storage.
- `input/tests.rs`: the former embedded `#[cfg(test)]` tests.

The child modules use explicit imports rather than wildcard imports, and
cross-module helpers are restricted to `pub(super)` only where sibling modules
need them.

## Structural Audit

Command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-codex-input-split-final
```

Result:

- Files scanned: 2562
- Rust files: 1186
- Rust physical LOC: 588877
- Violations: 1 error, 152 warnings

The previous `input.rs` error is gone. Remaining error:

- `crates/arcweft-cli/src/app/bundle_view.rs`: 2590 physical LOC

Changed `arcweft-player-scene` input metrics:

| Path | Kind | Bytes | Physical LOC | Embedded tests |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-player-scene/src/input.rs` | production | 32339 | 993 | no |
| `crates/arcweft-player-scene/src/input/keyboard.rs` | production | 7744 | 204 | no |
| `crates/arcweft-player-scene/src/input/pointer.rs` | production | 14636 | 380 | no |
| `crates/arcweft-player-scene/src/input/scroll.rs` | production | 4753 | 140 | no |
| `crates/arcweft-player-scene/src/input/state.rs` | production | 7189 | 197 | no |
| `crates/arcweft-player-scene/src/input/text_edit.rs` | production | 8870 | 205 | no |
| `crates/arcweft-player-scene/src/input/tests.rs` | test | 29288 | 887 | no |

## Validation

Commands run:

```bash
cargo fmt
cargo check -p arcweft-player-scene
cargo test -p arcweft-player-scene dialogue -- --nocapture
cargo clippy -p arcweft-player-scene --all-targets --all-features
```

Focused clippy exits successfully. It still reports existing
`arcweft-runtime-driver` `Option<Option<T>>` warnings through the dependency
graph; no new `arcweft-player-scene` warnings remain after replacing wildcard
imports with explicit imports.
