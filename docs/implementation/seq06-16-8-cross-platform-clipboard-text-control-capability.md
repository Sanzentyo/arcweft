# Seq06.16.8 Cross-platform clipboard text-control capability

Date: 2026-07-08
Status: implemented in this checkout; focused native/web/player-scene checks pass.

## Repository fit

The current Arcweft text-input layering already places platform-specific IME/event translation outside editor semantics. This package extends that same ownership model to host clipboard access.

The package was authored against the app-local clipboard behavior that previously lived behind `TextEditorClipboard` and `TextEditorOutput::ClipboardWrite(String)`. This checkout replaces those with `TextEditorLocalClipboard` and typed `TextClipboardIntent` output; no compatibility shim remains.

## Implemented overlay shape

### `arcweft-presentation`

New module:

```rust
pub mod clipboard;
```

New file:

```text
crates/arcweft-presentation/src/clipboard.rs
```

Key types:

- `ClipboardCapability`
- `TextClipboardOperation`
- `TextClipboardOrigin`
- `ClipboardText`
- `TextClipboardIntent`
- `TextClipboardRequest`
- `TextClipboardOutcome`
- `TextClipboardErrorKind`

Editor changes:

- `TextEditorClipboard` → `TextEditorLocalClipboard`.
- `TextEditorOutput::ClipboardWrite(String)` → `TextEditorOutput::Clipboard(TextClipboardIntent)`.
- Copy/cut emit write intents and update local fallback only after secure policy passes.
- Paste emits a read intent and does not synchronously read fallback.
- `paste_local_clipboard` is explicit and called only by player-scene fallback logic.
- Selection text for copy/cut is first expanded with `TextIndexSnapshot::expand_byte_range_to_grapheme_boundaries`.

Index changes:

- Add `expand_byte_range_to_grapheme_boundaries` as an inherent `TextIndexSnapshot` method.
- Add `U+FF9E..=U+FF9F` to Arcweft's conservative grapheme-extend set.

### `arcweft-player-scene`

`InputOutcome` gains:

```rust
pub clipboard_requests: Vec<TextClipboardRequest>
```

`InputController` gains:

```rust
next_clipboard_request_id: TextClipboardRequestId
```

Behavior:

- Editor intents are stamped with monotonic request ids.
- Host read success is applied through `TextEditorState::paste_text`.
- Host read failure may use `TextEditorLocalClipboard` only when the mapped error permits fallback.
- Host write failure never rolls back a cut; it only emits diagnostics.

### `arcweft-runtime-host`

New file:

```text
crates/arcweft-runtime-host/src/clipboard_host.rs
```

It defines:

- async-capable `TextClipboardHostAdapter`;
- sync-backed `SyncTextClipboardHostAdapter` for native;
- `ClipboardCapabilityPolicy`;
- `ClipboardAccessPolicy`;
- `SecurePastePolicy`.

### `arcweft-player-native`

New file:

```text
crates/arcweft-player-native/src/clipboard.rs
```

Dependency:

```toml
arboard = "3.6.1"
```

Linux native builds enable the `wayland-data-control` feature. The adapter is stored as long-lived native player state to preserve Linux clipboard ownership and avoid repeated construction races.

Error mapping:

| `arboard::Error` | Arcweft kind |
|---|---|
| `ContentNotAvailable` | `UnsupportedFormat` |
| `ClipboardNotSupported` | `Unavailable` |
| `ClipboardOccupied` | `Busy` |
| `ConversionFailure` | `UnsupportedFormat` |
| `Unknown` | `InternalFailure` |

### Web

New file:

```text
web/player-clipboard.js
```

Behavior:

- Uses clipboard event `clipboardData` first when a copy/cut/paste event is present.
- Uses `navigator.clipboard.writeText/readText` when available in a secure context.
- Emits structured request/outcome metadata without payloads.
- Maps `NotAllowedError`/`SecurityError` to `denied`.
- Maps missing secure context/API to `unavailable`.

## Diagnostics added

The overlay uses structured strings in the first pass and is designed for later typed trace-event integration:

- `text_clipboard.request`
- `text_clipboard.host_result`
- `text_clipboard.fallback_used`
- `text_clipboard.fallback_unavailable`
- `text_clipboard.host_failed`
- `text_clipboard.stale_ignored`
- `text_clipboard.secure_blocked`

Payload text is never logged by the proposed code. `ClipboardText::Debug` is redacted.

## Non-goals in this implementation slice

- Rich text / HTML / images / file lists / custom MIME data.
- Linux PRIMARY/SECONDARY selection support.
- Programmatic DSL clipboard API beyond capability policy scaffolding.
- Clipboard polling or clipboard-change watchers.
- Hidden DOM text-area fallback for web text controls.
- Redesigning IME, text layout, or renderer glyph geometry.
- Full iOS/iPadOS UIKit implementation; the policy and platform contract reserve that adapter.

## Manual conflict notes

The patch set deliberately includes large-file overlay pointers for new files to avoid duplicating complete source bodies inside every patch. Copy the matching files from `src-overlays/` when applying. Existing `InputOutcome` construction sites will need mechanical `clipboard_requests: Vec::new()` additions where not covered by the illustrative patch.

The native integration hunk assumes synchronous host handling immediately after `InputOutcome` is applied. If the real branch has an async event queue by the time this is applied, the request stamping/outcome application methods should remain the same and only the host completion scheduling should differ.

## Acceptance criteria

- `Ctrl+C`/`Ctrl+X` writes text to host clipboard on Windows/macOS/Linux when host allows it.
- Host clipboard text pasted from another app enters Arcweft through a user paste command.
- Web secure-context Clipboard API success and denied paths are tested.
- Secure copy/cut create no host request and no local fallback text.
- Host denial/unavailable paths remain deterministic and diagnose fallback/local-only behavior.
- `Дﾟ` cannot be cut as only `U+FF9F`.
- No clipboard payload text appears in debug/trace/default diagnostics.

## Checkout implementation notes

- `arcweft-presentation::clipboard` owns the Sans I/O intent/request/outcome/error contract.
- `TextEditorOutput::Clipboard(TextClipboardIntent)` is stamped into `TextClipboardRequest` values by `arcweft-player-scene::input::InputController`.
- Native player state owns a long-lived `NativeClipboardAdapter` backed by `arboard`.
- Web EditContext glue routes copy/cut/paste through `web/player-clipboard.js` and structured outcomes.
- `TextIndexSnapshot::expand_byte_range_to_grapheme_boundaries` is used before copy/cut payload extraction so halfwidth voiced/semi-voiced marks remain attached to their base cluster.
- Static view text blocks gained an opt-in selection policy in this implementation slice, but OS clipboard export for selected static text is not wired until keyboard modifiers expose Ctrl/Meta to `InputController`.

## Validation in this checkout

```bash
cargo check -p arcweft-presentation -p arcweft-player-scene -p arcweft-player-native --all-features
cargo check -p arcweft-player-web --all-features
cargo check -p arcweft-player-scene --tests --all-features
cargo test -p arcweft-presentation --test text_editor_behavior --all-features
cargo test -p arcweft-player-scene selectable_runtime_text_block_drag_adds_selection_rectangles --test scroll_regions --all-features
```

Manual OS clipboard validation with a real Windows/macOS/Linux desktop clipboard and browser permission prompts is still required before calling the host integration fully platform-verified.

## Structural audit

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq06168
```

Result: 2421 files scanned, 1159 Rust files, 567908 Rust physical LOC, 91 package manifests, 1 error and 148 warnings. The error is `crates/arcweft-cli/src/app/bundle_view.rs` at 2598 physical LOC. This slice only adds a small static-text selection policy hook to that file; the required decomposition is broader CLI view-lowering work and remains outside this clipboard implementation cut.
