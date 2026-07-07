# Cross-platform clipboard contract for Arcweft runtime text controls

Date: 2026-07-08
Sequence: seq06.16.8

## 1. Problem statement

The current runtime text-control behavior can copy, cut, and paste inside a running Arcweft player, but it does not define a host clipboard bridge. The design goal is to make text-control copy/cut/paste deterministic and platform-safe without allowing OS, browser, or pasteboard APIs to leak into low-level editor or presentation crates.

The contract below keeps the editor responsible for text semantics and the host responsible for clipboard side effects.

## 2. Final ownership boundary

| Owner | Responsibilities | Must not do |
|---|---|---|
| `arcweft-presentation::text_index` | Canonical byte/UTF-16/scalar/grapheme range validation and normalization. | Call host APIs, inspect permissions, log payloads. |
| `arcweft-presentation::text_editor` | Editor state, selection/caret/composition, secure-field blocking, production of typed clipboard intents. | Read/write OS/browser clipboard, retry host failures, treat local fallback as host clipboard. |
| `arcweft-presentation::clipboard` | Sans I/O typed clipboard text, intent, request, outcome, error, origin, and capability names. | Depend on `serde`, `arboard`, `web-sys`, platform APIs, or runtime config. |
| `arcweft-player-scene` | Turns editor intents into request ids, owns the in-app fallback clipboard, applies host paste outcomes to the focused editor, emits diagnostics metadata. | Call native/browser APIs directly. |
| `arcweft-runtime-host` | Capability policy, async-capable host adapter trait, fake-provider test utilities, error policy mapping. | Own editor state or text selection behavior. |
| `arcweft-player-native` | Native host adapter using `arboard` for text clipboard, serialized access, platform error mapping. | Put `arboard` in presentation/core/text layout crates. |
| `web/player-editcontext.js` / web player host | Browser adapter using `navigator.clipboard` where available and clipboard events as user-initiated fallback. | Install hidden textareas or browser DOM editing fallbacks for Arcweft editor semantics. |

## 3. Existing type decision

`TextEditorClipboard` is renamed to `TextEditorLocalClipboard`. This is an intentional breaking rename, not a compatibility shim.

The renamed type remains as the deterministic in-app fallback only:

- it is updated on successful editor copy/cut intent creation, before host write completion;
- it may be used for paste only when host read is denied/unavailable/busy and policy permits local fallback;
- it is never reported as proof that OS clipboard integration worked;
- it is not exposed to DSL/runtime capability code.

`TextEditorOutput::ClipboardWrite(String)` is replaced by `TextEditorOutput::Clipboard(TextClipboardIntent)`. Paste commands no longer synchronously read the local clipboard from the editor. Instead, paste produces a read intent. The host result later enters through `InputController::apply_clipboard_outcome` and calls the existing `TextEditorState::paste_text(...)` path.

## 4. Typed model

The concrete overlay introduces `arcweft-presentation::clipboard` with these core types:

```rust
pub enum ClipboardCapability {
    ReadText,
    WriteText,
    Clear,
}

pub enum TextClipboardOperation {
    Copy,
    Cut,
    Paste,
    Clear,
}

pub enum TextClipboardOrigin {
    UserKeyboardShortcut,
    UserPlatformClipboardEvent,
    RuntimeRequest,
}

pub enum TextClipboardIntent {
    Write(TextClipboardWriteIntent),
    Read(TextClipboardReadIntent),
    Clear(TextClipboardClearIntent),
}

pub enum TextClipboardRequest {
    Write(TextClipboardWriteRequest),
    Read(TextClipboardReadRequest),
    Clear(TextClipboardClearRequest),
}

pub enum TextClipboardOutcome {
    WriteCommitted { request_id: TextClipboardRequestId },
    ReadCommitted { request_id: TextClipboardRequestId, text: ClipboardText },
    Cleared { request_id: TextClipboardRequestId },
    Failed { request_id: TextClipboardRequestId, error: TextClipboardError },
}
```

`ClipboardText` is a newtype. Its `Debug` implementation reports only redacted metadata such as byte/scalar counts; it does not print payload text. `ClipboardText::new` normalizes `\r\n` and bare `\r` to Arcweft's internal `\n` representation.

The editor emits intents; the player-scene host creates requests by adding a monotonic `TextClipboardRequestId`. The host adapter returns outcomes with that id. Stale ids are ignored with a structured diagnostic.

## 5. Host adapter API

The adapter boundary is async-capable without requiring `async_trait`:

```rust
pub type ClipboardHostFuture<'a> = Pin<Box<dyn Future<Output = TextClipboardOutcome> + Send + 'a>>;

pub trait TextClipboardHostAdapter {
    fn apply_clipboard_request<'a>(
        &'a mut self,
        request: TextClipboardRequest,
    ) -> ClipboardHostFuture<'a>;
}

pub trait SyncTextClipboardHostAdapter {
    fn apply_clipboard_request_sync(
        &mut self,
        request: TextClipboardRequest,
    ) -> TextClipboardOutcome;
}
```

A blanket implementation wraps sync adapters in `std::future::ready`. Native uses the sync trait. Web/wasm resolves the same request type through JavaScript promises.

## 6. Error categories and editor behavior

| Error kind | Meaning | Copy/Cut write behavior | Paste read behavior | Retry |
|---|---|---|---|---|
| `Unavailable` | No adapter/API/display/session support. | Keep local fallback; emit diagnostic; do not claim host success. | Use local fallback only for user paste if policy allows; otherwise no edit. | No automatic retry. |
| `Denied` | OS/browser/user denied host access. | Keep local fallback; emit diagnostic. | Use local fallback only if policy permits; otherwise no edit. | No retry. |
| `PolicyDenied` | Arcweft capability policy denied. | No host access. Local fallback only if denial is for host escalation, not secure-field. | No edit unless explicit local-only mode is allowed. | No retry. |
| `UnsupportedFormat` | Clipboard has no UTF-8 text or format unavailable. | Should not occur for write; diagnostic if adapter maps it. | No edit; local fallback may be used if requested. | No retry. |
| `Busy` | Clipboard occupied/global object held. | Native host may do one serialized retry; after failure keep local fallback and diagnose. | Native host may do one serialized retry; after failure local fallback policy. | Host-controlled one retry only. |
| `Stale` | Response no longer matches focused session/revision. | Ignore outcome; do not rollback. | Ignore; no edit. | No retry. |
| `SecureFieldBlocked` | Secure policy blocked payload exposure. | No local/host write. | Secure paste policy decides before payload read; denied path no edit. | No retry. |
| `InternalFailure` | Adapter bug/unclassified failure. | Keep local fallback; diagnose without payload. | Local fallback only if policy allows; otherwise no edit. | No automatic retry. |

Cut is not rolled back if host write later fails. The editor's deterministic text mutation occurs once the user invokes cut. This mirrors normal text-control expectations while still reporting whether host clipboard integration succeeded.

## 7. Platform behavior matrix

| Platform | Adapter | Required behavior |
|---|---|---|
| Windows | `arcweft-player-native::clipboard::NativeClipboardAdapter` using `arboard`. | Process clipboard requests serially on the player event-loop path. Do not open the global clipboard from parallel worker threads. Map `ClipboardOccupied` to `Busy`; map unsupported desktop/session to `Unavailable`. |
| Linux X11 | Same native adapter. | Store a long-lived `arboard::Clipboard` in native player state so X11 selection ownership does not vanish immediately. Use the normal `Clipboard` selection, not PRIMARY/SECONDARY for text controls. |
| Linux Wayland | Same native adapter with `wayland-data-control` feature enabled for Linux. | Treat missing compositor data-control support as `Unavailable`; retain the clipboard object for app lifetime. Do not emulate with polling. |
| macOS | Same native adapter through arboard/AppKit pasteboard backend. | Reads happen only for user paste or explicit future capability. Policy must treat pasteboard reads as privacy-sensitive and map prompts/denials to `Denied`. |
| iOS/iPadOS | Future UIKit `UIPasteboard` adapter, not arboard. | Reads only under user paste intent or explicit future capability. Paste prompts and user-intent requirements map to `Denied` or `Unavailable`; secure payloads are never logged. |
| Web | JS browser adapter. | Prefer Clipboard API in secure contexts. Use `copy`/`cut`/`paste` event `clipboardData` as the user-initiated fallback. Map insecure context/missing `navigator.clipboard` to `Unavailable`, DOM `NotAllowedError`/permission denial to `Denied`. |

## 8. Dependency strategy

Use `arboard 3.6.1` for native desktop text clipboard support, but only behind `arcweft-player-native`. It is suitable for the first cut because it provides an OS-independent `Clipboard` with `get_text`, `set_text`, and `clear`, supports Linux/macOS/Windows, and exposes specific error variants that map cleanly to Arcweft categories.

Arcweft still needs a platform-specific adapter around `arboard` because:

- Arcweft must serialize requests and assign request ids;
- Windows parallel access must be avoided;
- Linux clipboard ownership/lifetime must be handled by keeping the provider alive;
- Arcweft capability policy and secure field behavior must be enforced before `arboard` is called;
- `arboard::Error` categories do not encode Arcweft policy or stale session errors.

Web cannot share `arboard`. It must use the browser Clipboard API and clipboard events from JavaScript/wasm glue.

Native iOS/iPadOS is not covered by arboard. It requires a future UIKit adapter behind the Apple mobile player boundary.

## 9. Permission and capability policy

Capability names:

- `clipboard.read`
- `clipboard.write`
- `clipboard.clear`

Default text-control policy:

```toml
[capabilities.clipboard]
read = "user_initiated_text_control"
write = "user_initiated_text_control"
clear = "deny"
background_read = "deny"
programmatic_read = "deny"
programmatic_write = "deny"

[text_controls.secure]
copy = "deny"
cut = "deny"
paste = "deny" # may be changed to "user_initiated" only by explicit product policy
trace_payload = "deny"
```

User-initiated text-control copy/cut/paste means a keyboard shortcut, platform clipboard event, context-menu paste, or accessibility paste event routed to the focused text control. It does not include runtime scripts, DSL code, timers, render passes, or agent observation.

DSL/runtime requests for clipboard access must declare capabilities in the authored manifest/config and be surfaced during bundle validation. The runtime must not silently escalate from text-control user gesture to programmatic clipboard access.

Polling/background clipboard reads are explicitly forbidden. A future request must design a separate capability and threat model before any clipboard-change tracking is implemented.

## 10. Secure text-field behavior

Secure fields:

- reject copy and cut before selection text leaves the editor;
- do not write to host clipboard;
- do not update local fallback;
- do not include selected text, pasted text, or payload length in traces by default;
- emit only policy/error category metadata;
- may allow paste only if `secure.paste = "user_initiated"` and `clipboard.read` policy allows it.

When secure paste is allowed, diagnostics include request id, target, operation, origin, and policy outcome, but not payload, payload length, or before/after text.

## 11. Selection and grapheme model

Copy, cut, delete, selection expansion, and paste replacement must use `TextIndexSnapshot`.

This package adds:

```rust
pub fn expand_byte_range_to_grapheme_boundaries(
    &self,
    range: TextRange<TextByteOffset>,
) -> Result<TextRange<TextByteOffset>, TextIndexError>
```

The method validates byte boundaries, then expands non-collapsed ranges to enclosing grapheme boundaries. This prevents a platform selection covering only `U+FF9F` in `Дﾟ` from being cut/copied as a standalone mark. The method is added to the owned enum/module rather than implemented as an ad-hoc helper.

`is_grapheme_extend` is extended to include `U+FF9E HALFWIDTH KATAKANA VOICED SOUND MARK` and `U+FF9F HALFWIDTH KATAKANA SEMI-VOICED SOUND MARK`.

## 12. Data formats and extension boundary

Implemented now:

- Plain UTF-8 text only.
- Internal newline state is LF (`\n`).
- Host inbound text normalizes CRLF and bare CR to LF before entering `TextEditorState`.
- Outbound text passes normalized LF to the host adapter. Platform-specific newline conversion is left to the adapter/platform crate; the Arcweft editor does not store CRLF.

Explicit non-goals for this cut:

- HTML/rich text.
- Images.
- File lists.
- Custom MIME data.
- Primary/secondary Linux selection.
- Background clipboard watchers.
- Programmatic runtime clipboard access beyond policy scaffolding.

Future extension points:

- Add `ClipboardFormat::{Html, Image, FileList, CustomMime}` only when payload redaction, capability policy, host format negotiation, and secure-field behavior are designed.
- Add separate `clipboard.observe` or `clipboard.poll` only with a privacy review.

## 13. Diagnostics and tracing

Structured events:

- `text_clipboard.request`
- `text_clipboard.host_result`
- `text_clipboard.fallback_used`
- `text_clipboard.policy_denied`
- `text_clipboard.secure_blocked`
- `text_clipboard.stale_ignored`

Default fields:

- `request_id`
- `operation`
- `origin`
- `capability`
- `target` / stable redacted target id
- `session`
- `secure` boolean
- `host_adapter` name
- `error_kind`
- `retry_count`
- `fallback_kind`

Default fields must not include clipboard payload, selected text, before/after editor text, or pasted text. Payload length is disabled by default and must remain disabled for secure fields.

## 14. Validation plan

Unit tests:

- editor emits write/read intents for copy/cut/paste;
- local fallback is updated only for non-secure copy/cut;
- paste applies only through host/local outcome path;
- secure copy/cut blocked without local/host payload;
- grapheme expansion prevents partial cut of `Дﾟ`, kana combining marks, emoji modifiers, variation selectors, and CRLF.

Host adapter tests:

- fake provider success for write/read;
- denied/unavailable/busy/stale outcomes;
- policy denial;
- local fallback behavior is marked diagnostic-only, not host success.

Native smoke tests:

- Windows round-trip copy into host clipboard and read back from another process where available;
- Linux long-lived provider retains data while player runs;
- unsupported Wayland compositor maps to `Unavailable`.

Web tests:

- `navigator.clipboard.writeText/readText` success in secure context;
- permission denial maps to `Denied`;
- no `navigator.clipboard` falls back to clipboard events;
- insecure context maps to `Unavailable`;
- secure fields never call read/write.

## 15. Implementation order

1. Introduce `arcweft-presentation::clipboard` and `TextEditorLocalClipboard` rename.
2. Add `TextIndexSnapshot::expand_byte_range_to_grapheme_boundaries` and grapheme fixture tests.
3. Route editor copy/cut/paste to clipboard intents.
4. Add `arcweft-player-scene` request id stamping and host outcome application.
5. Add `arcweft-runtime-host` adapter trait and capability policy.
6. Add native `arboard` adapter and serialized event-loop handling.
7. Add web adapter and event/Clipboard API mapping.
8. Add diagnostics and tests.
9. Run native/web smoke validation and update implementation docs.
