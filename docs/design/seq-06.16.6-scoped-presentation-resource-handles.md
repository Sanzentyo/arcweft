# Seq 06.16.6 scoped presentation resource handles

Date: 2026-07-06

## Scope

This document designs the scoped presentation resource handle model requested by
seq 06.16.6. The design builds on the verified explicit image/view mount
surface instead of replacing it. An expression statement such as
`image(@image.menu_background)` continues to explicitly emit or mount a
presentation object. A binding such as
`let background = image(@image.menu_background)` instead creates an owned
presentation handle whose lifetime is tracked by the language and serialized by
the runtime presentation snapshot.

The goals are:

- deterministic lexical cleanup without host garbage collection;
- stable runtime identities that survive waits, choices, save/load, and rollback;
- identical native, web, and Agent observe behavior through the existing
  `BundlePresentationSnapshot` / player-frame path;
- no lower-level renderer awareness of language scopes;
- no redesign of already implemented explicit image/view declaration or
  mount behavior.

## Decision summary

Scoped presentation handles are both lexical resources and runtime-owned
entities.

The language owns the lexical value. The value is move-only and typed, for
example `ImageHandle`, `ComponentHandle`, `MenuHandle`, `OverlayHandle`,
`TextBoxHandle`, or `RuntimeControlHandle`. Leaving the owning scope emits a
runtime lifecycle operation in reverse creation order unless ownership was moved,
detached, or explicitly disposed.

The runtime owns the stable identity. A `PresentationHandleRecord` is serialized
into the portable presentation snapshot with:

```rust
struct PresentationHandleRecord {
    id: PresentationHandleId,
    kind: PresentationHandleKind,
    resource_id: String,
    owner: Option<String>,
    state: PresentationResourceState,
    layer: Option<String>,
    depth_milli: i32,
}
```

The renderer never sees lexical scope. It receives only prepared presentation
state after the runtime has applied lifecycle operations and filtered hidden,
unmounted, released, or destroyed resources.

## Source grammar

The canonical source grammar is intentionally narrow. Existing explicit mount
syntax remains valid in flow-item position. Handle creation is recognized when a
presentation constructor is used as a value in a binding, `out`, argument, or
return expression.

```text
PresentationHandleCreateExpr :=
    image(EntityRef, MountArgs?)
  | view(EntityRef, MountArgs?)
  | menu(EntityRef, MountArgs?)
  | overlay(EntityRef, MountArgs?)
  | textbox(EntityRef, MountArgs?)
  | runtime_control(EntityRef, MountArgs?)

MountArgs := named arguments including:
  layer, depth, visible, focus, input_capture, owner, drop

PresentationHandleLifecycleStmt :=
    handle.show()
  | handle.hide()
  | handle.unmount()
  | handle.release()
  | handle.destroy()
  | dispose(handle)
```

Examples:

```arcw
flow @flow.menu {
    let background = image(@image.menu_background, layer=@layer.background)
    let menu = view(@view.MainMenu, focus=.Trap, input_capture=.Modal)

    wait(choice_ready())

    menu.hide()
    wait(250ms)
    menu.show()
}
// background and menu are released here in reverse declaration order.
```

```arcw
let overlay_result = scope pause_menu {
    let overlay = overlay(@view.PauseMenu, focus=.Modal)
    out await overlay.closed()
}
// overlay is popped/released here, while the underlying flow state remains live.
```

Binding to `_` means immediate drop, matching the existing scoped-handle rule:

```arcw
let _ = image(@image.flash)
// Mount then immediately release. LSP should warn unless the drop policy is explicit.
```

## Typed HIR and lowering representation

HIR gains typed representation for the semantic shape, not a compatibility shim
for old syntax:

```rust
enum HirPresentationHandleKind {
    Image,
    View,
    Menu,
    Overlay,
    TextBox,
    RuntimeControl,
}

struct HirPresentationHandleCreate {
    binding: HirPattern,
    kind: HirPresentationHandleKind,
    resource: EntityRef,
    args: HirMountArgs,
    owner_scope: HirScopeId,
    range: TextRange,
}

enum HirPresentationHandleLifecycleOp {
    Show,
    Hide,
    Unmount,
    Release,
    Destroy,
    Dispose,
}

struct HirPresentationHandleLifecycle {
    handle: HirExpr,
    op: HirPresentationHandleLifecycleOp,
    range: TextRange,
}
```

Lowering emits canonical runtime calls. This keeps `arcweft-core` Sans I/O and
lets `arcweft-runtime-driver` own presentation-specific lifecycle semantics.

```text
presentation.handle.create(
  handle = @handle.flow.menu.background,
  kind = "image",
  resource = @image.menu_background,
  owner = @scope.flow.menu.block.0,
  visible = true,
  layer = @layer.background,
  depth = -1000,
)

presentation.handle.hide(handle = @handle.flow.menu.background)
presentation.handle.unmount(handle = @handle.flow.menu.background)
presentation.handle.release(handle = @handle.flow.menu.background)
presentation.handle.dispose(handle = @handle.flow.menu.background)
presentation.handle.destroy(handle = @handle.flow.menu.background)
```

Scope cleanup is lowered by the control-flow lowering pass, not by renderer
adapters. Each lexical scope has a cleanup stack of live owned handles. Normal
block exit, flow return, branch exit, cancellation, overlay pop, and scene
transition all drain the relevant stack deterministically.

## Runtime lifecycle states

`PresentationResourceState` has five states:

| State | Meaning | Rendered? | Input/focus? | Can be shown again? |
|---|---|---:|---:|---:|
| `mounted` | resource is live and contributes to prepared presentation state | yes | yes, subject to policy | already visible |
| `hidden` | resource remains live but is excluded from render, hit-test, focus navigation, and capture | no | no | yes |
| `unmounted` | resource identity remains, but host renderer resources are detached | no | no | yes, by remount/show |
| `released` | lexical owner ended; runtime caches may remain, but handle cannot be used | no | no | no |
| `destroyed` | resource instance and runtime state are tombstoned | no | no | no |

`hide` is a visibility operation. `unmount` is a renderer/host detachment
operation. `release` is ownership cleanup. `destroy` is terminal teardown.
`dispose(handle)` lowers to `release` unless the handle type has a stricter
policy, such as `destroy_on_dispose`.

## Snapshot operations

`BundlePresentationSnapshot` stores `presentation_handles: Vec<PresentationHandleRecord>`.
On each VM step, `arcweft-runtime-driver`:

1. collects canonical `presentation.handle.*` calls from line effects;
2. applies operations to the stable handle table;
3. resolves existing explicit image effects exactly as before;
4. overlays scoped image handles onto the active image object list;
5. filters text controls, action buttons, focus groups, and focus navigation for
   hidden/unmounted/released/destroyed view/menu/overlay/textbox/control
   handles;
6. increments snapshot revision when either the filtered presentation data or the
   handle table changes;
7. returns structured diagnostics to the session step diagnostics.

This makes native, web, and Agent observe converge because they already consume
the same snapshot/frame-planner path.

## Deterministic cleanup rules

Cleanup never relies on host GC.

- Block exit releases live handles owned by the block in reverse creation order.
- Flow return releases live handles owned by the returning flow before the final
  status is reported.
- Cancellation drains only handles owned by the cancelled scope and then reports
  cancellation effects.
- Overlay pop releases overlay/menu handles owned by that overlay scope while the
  underlying flow fiber remains paused or waiting.
- Scene transition releases all scene-owned handles after transition-out hooks and
  before transition-in hooks for the next scene.
- Explicit `dispose(handle)` releases immediately and removes the handle from the
  lexical cleanup stack so scope exit does not double-release.
- `detach()` transfers the runtime owner and removes lexical cleanup responsibility.

Scope cleanup order is part of deterministic replay and must be serialized in the
save snapshot through handle records plus owner/scope cleanup metadata.

## Waits, choices, async effects, save/load, and rollback

Handles survive waits and choices because a paused flow still owns its lexical
scope. A wait or choice does not release handles unless it exits the owning
scope. Async effects inherit the owner scope that created them. If an async task
is cancelled, its owned handles are released with the task's cancellation scope.
Detached tasks must either detach their handles or own a separate runtime scope.

Save/load stores the handle table, owner scope stack, lifecycle state, and a
monotonic operation epoch. Loading resumes from that exact table. Rollback
restores the previous table and tombstones, so a released handle cannot be
revived by a stale `show` operation emitted after rollback.

## Typed handle families

All families share the runtime lifecycle contract, but the language exposes typed
families. This avoids invalid operations such as asking an image handle for
view focus traversal while still allowing one serializer and one runtime
operation table.

```text
ImageHandle          -> show/hide/unmount/release/destroy, image playback query
ViewHandle           -> show/hide/unmount/release/destroy, view capture id
MenuHandle           -> overlay stack and modal focus APIs
OverlayHandle        -> pop/close result APIs
TextBoxHandle        -> text reveal/cursor APIs
RuntimeControlHandle -> focus/input/writeback APIs
```

A generic `ScopedPresentationHandle` trait is available for drop policy and
owner transfer, but public APIs should prefer the typed families.

## Ownership and storage

Presentation handles are move-only. Passing a handle to another view moves
ownership unless the signature borrows it for a bounded scope:

```arcw
fn show_menu(menu: ViewHandle) { ... }       // moves ownership
fn inspect_menu(menu: &ViewHandle) { ... }   // temporary borrow only
```

Storing a live handle in persistent game state is rejected because it would tie
save data to renderer lifetime. A handle may be stored only after an explicit
`detach(scope=scene|global)` operation that produces a serializable detached
handle token with a runtime owner.

## Layers, depth, focus, input capture, and view capture

Mount arguments carry layer, depth, focus, and input-capture policy. The runtime
resolves these before frame planning. Hidden, unmounted, released, and destroyed
resources are absent from:

- rendered frames;
- hit testing;
- focus navigation;
- view/layer/object capture descriptors;
- Agent observe resource lists.

View-scoped capture preserves the existing URI/resource identifier grammar.
A visible scoped view uses the same view id as the mounted view.
A hidden or disposed handle returns a missing-scope diagnostic instead of a
synthetic visual substitute.

## Diagnostics

Diagnostics are stable and structured:

| Code | Meaning |
|---|---|
| `PH001_INVALID_CALL` | malformed runtime lifecycle operation |
| `PH002_DUPLICATE_HANDLE` | handle id reused within one save lineage |
| `PH003_RESOURCE_ALREADY_OWNED` | live resource already has an owner handle |
| `PH004_UNKNOWN_HANDLE` | lifecycle operation targets an unknown handle |
| `PH005_DOUBLE_DISPOSE` | release/dispose/destroy repeated on a terminal handle |
| `PH006_TERMINAL_HANDLE` | show/hide/unmount attempted after release/destroy |
| `PH007_HIDDEN_BUT_FOCUSABLE` | hidden resource appeared in focus navigation before filtering |
| `PH008_OWNER_ESCAPED` | handle would outlive its owner flow/scope |
| `PH009_CAPTURE_HIDDEN_HANDLE` | capture requested for a hidden/unmounted/disposed handle |

Lowering diagnostics include source ranges. Runtime diagnostics include handle id
and operation. Agent observe repeats relevant diagnostics in frame/capture
metadata.

## Migration note

No migration is required for existing explicit mount code. These remain explicit
one-shot presentation emissions:

```arcw
image(@image.menu_background)
view(@view.MainMenu)
```

The new behavior is selected only when the presentation constructor appears in a
value-producing position:

```arcw
let background = image(@image.menu_background)
let menu = view(@view.MainMenu)
```

This distinction keeps existing authored scenario files and generated bundles
valid while allowing new handle-aware code to be written intentionally. The old
fluent sketch syntax and any rejected parser forms remain rejected; no
compatibility shim is introduced.

## Acceptance criteria and test names

Parser/lowering:

- `presentation_handle_image_binding_lowers_to_create_operation`
- `presentation_handle_view_binding_lowers_to_create_operation`
- `presentation_handle_hide_show_unmount_dispose_lower_to_lifecycle_operations`
- `presentation_handle_invalid_kind_reports_structured_diagnostic`
- `presentation_handle_escape_to_state_requires_detach`
- `presentation_handle_discard_binding_warns_immediate_drop`

Runtime lifecycle:

- `presentation_handle_block_exit_releases_in_reverse_creation_order`
- `presentation_handle_flow_return_releases_live_handles`
- `presentation_handle_cancellation_releases_cancelled_scope_handles`
- `presentation_handle_overlay_pop_releases_overlay_owned_handles`
- `presentation_handle_wait_preserves_live_handles`
- `presentation_handle_choice_preserves_live_handles_until_branch_exit`
- `presentation_handle_save_load_restores_lifecycle_table`
- `presentation_handle_rollback_restores_handle_tombstones`
- `presentation_handle_double_dispose_is_diagnostic`

Native/web/observe parity:

- `hidden_presentation_handle_absent_from_native_frame`
- `hidden_presentation_handle_absent_from_web_frame`
- `hidden_presentation_handle_absent_from_agent_observe_objects`
- `disposed_presentation_handle_absent_from_hit_test`
- `disposed_presentation_handle_absent_from_focus_navigation`
- `hidden_runtime_control_writeback_is_rejected`

Capture:

- `view_capture_selected_handle_uses_existing_view_uri`
- `view_capture_hidden_handle_reports_missing_scope`
- `view_capture_disposed_handle_reports_missing_scope`
- `image_handle_object_capture_preserves_existing_object_uri`

Regression:

- `explicit_image_call_still_emits_declared_presentation_object`
- `explicit_inline_image_call_still_emits_bounded_object`
- `explicit_view_mount_still_works_without_scoped_handle`
- `runtime_control_style_snapshot_path_is_unchanged`
