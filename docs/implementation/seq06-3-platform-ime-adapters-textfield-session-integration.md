# seq06.3 platform IME adapters and TextField session integration

## Scope

This implementation slice builds on the seq06.1 session-scoped text-input
substrate. It adds concrete runtime-host dispatch and UI editor contracts for
Japanese-capable IME composition without moving OS handles into Sans I/O crates.

The implemented path covers `TextField`, `TextArea`, and `SecureField` at the
shared protocol level. Native TSF/AppKit/Wayland/Android/iOS/Web code can now
emit typed platform events with a `TextInputSessionId`, `TextInputSerial`, and
`TextInputFocusGeneration`; runtime-host dispatch validates and routes those
events as `RawInputKind::Text(TextInput)` batches.

## Design decisions

- Platform adapters map TSF, `NSTextInputClient`, Wayland text-input-v3,
  Android `InputConnection`, iOS `UITextInput`, and Web `EditContext` into
  `PlatformTextInputEvent` values. Each event carries adapter kind, session,
  target, serial, and focus generation.
- Runtime-host rejects stale serials, session mismatches, target mismatches, and
  focus-generation mismatches before creating a routed `RawInputEvent`.
- Composition-on-blur is explicit through `TextInputBlurPolicy`:
  `CommitComposition`, `CancelComposition`, or `PlatformDefault`. The runtime
  host emits only data commands: `CommitComposition`, `CancelComposition`, and
  `Deactivate`.
- Keyboard shortcuts are gated by `TextInputKeyDisposition` and the active
  composition flag. If the IME consumed the key, normal shortcut routing is
  suppressed.
- Secure fields use `TextInputSecurityPolicy::SecureRedacted`. Activation and
  update snapshots redact surrounding text, selection payload, composition, and
  character bounds. Incoming batches are marked `TextInputPrivacy::Sensitive`,
  and clipboard commands are rejected.
- Web support is explicit. `WebTextInputApiSupport::UnsupportedNoFallback`
  returns an unsupported capability error; this slice does not introduce hidden
  textarea fallback behavior.
- Text-local geometry is converted through `TextGeometryTransform` from
  text-local to viewport and screen coordinates. `TextWritingMode` distinguishes
  horizontal and vertical writing for candidate anchor and character bounds.
- Mobile delete-surrounding semantics are preserved by applying UTF-16 code
  unit, Unicode scalar, or grapheme-cluster deletion without creating invalid
  UTF-8 ranges.

## Crate changes

### `arcweft-presentation`

- Adds platform text-input adapter identifiers and event/context types:
  `TextInputAdapterKind`, `PlatformTextInputContext`, and
  `PlatformTextInputEvent`.
- Adds `TextInputFocusGeneration` to reject delayed callbacks from previous
  focus transactions.
- Adds `TextInputKeyDisposition`, `TextInputBlurPolicy`,
  `TextInputSecurityPolicy`, `WebTextInputApiSupport`, `TextWritingMode`,
  `TextGeometryTransform`, and `TextInputGeometrySnapshot`.
- Adds secure snapshot redaction on `TextInputClientSnapshot` and adapter
  capability constructors on `TextInputCapabilities`.

### `arcweft-runtime-host`

- Adds `text_input_dispatch.rs` with `TextInputDispatchState`.
- Validates session, serial, target, and focus generation before routing.
- Emits host commands for activation, snapshot update, commit/cancel/deactivate
  blur transactions, and Web unsupported capability reporting.
- Adds protocol tests for Japanese composition, stale serial/generation
  rejection, secure redaction, clipboard blocking, blur policy, Web EditContext
  gating, and platform fixture traces.

### `arcweft-ui`

- Adds `unicode-segmentation` to implement grapheme-aware delete-surrounding.
- Adds `TextFieldEditPolicy`, `TextFieldBindingCommitPolicy`,
  `TextFieldGeometryPolicy`, and `TextFieldPolicyEditError`.
- Adds `TextEditState::apply_text_input_with_policy` for secure fields and
  Unicode-correct delete-surrounding.
- Adds TextField snapshot and geometry export methods for candidate anchors and
  character bounds.
- Adds visual/protocol tests for preedit-not-binding, commit binding, emoji and
  combining mark deletion, secure redaction, secure clipboard rejection, and
  transformed vertical candidate anchors.

### `arcweft-player-scene`

- Adds `InputController::text_input` for routing session-scoped text batches to
  the focused target.
- Adds `InputController::keyboard_with_ime` to suppress shortcuts while IME
  composition consumes keys.

## Validation status

The original patch package was built in a sandbox without a full
`Sanzentyo/arcweft` checkout. This checkout applied the package after seq06.1
and seq06.2, resolved current-context patch drift in `text_field.rs` and
`player-scene/src/input.rs`, and validated the resulting implementation.

Validation run:

- `cargo test -p arcweft-presentation -p arcweft-ui -p arcweft-runtime-host -p arcweft-player-scene --all-features -- --nocapture`
- `cargo check -p arcweft-presentation -p arcweft-ui -p arcweft-runtime-host -p arcweft-player-scene --all-targets --all-features`
- `cargo fmt --all -- --check`
- `cargo clippy -p arcweft-presentation -p arcweft-ui -p arcweft-runtime-host -p arcweft-player-scene --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `git diff --check`
- `just test-workspace`

## Structural audit note

The change adds one runtime-host responsibility module and extends existing
presentation/UI text-input modules. No OS handles are introduced to Sans I/O
crates. The largest new production file is `text_input_dispatch.rs`; it owns one
cohesive validation/dispatch responsibility and is below the AGENTS.md 1,200 LOC
warning threshold for production Rust files.

## Remaining adapter work

The platform-specific FFI/object layers are intentionally represented as hook
contracts in this slice. Real Windows TSF, AppKit, Wayland, Android, iOS, and
browser bindings should be implemented in the concrete adapter/player crates by
creating `PlatformTextInputEvent` values and feeding them through
`TextInputDispatchState`. Those bindings must not store native handles in
presentation/UI/runtime-host state.

## Request coverage

- Session-scoped batches: implemented through seq06.1 presentation routing plus
  seq06.3 runtime-host dispatch.
- Editor core integration: policy-aware TextField editor method added.
- Candidate/character geometry: TextField snapshot and geometry snapshot added.
- Native/mobile/Web hooks: platform event/context contracts added with fixture
  tests.
- Secure redaction: activation/update snapshots and input privacy implemented;
  secure clipboard rejection added.
- Web fallback constraint: unsupported EditContext path reports an error.
- Unicode deletion: UTF-16, scalar, and grapheme delete-surrounding implemented.
