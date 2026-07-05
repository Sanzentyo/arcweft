# Dialogue speaker display labels - 2026-07-06

## Scope

Dialogue display lowering now preserves an authored speaker display label separately
from the runtime callee identifier.

## Contract

- `LineDisplaySpec` and `LineDisplayFrame` include optional `speaker_label`.
- `callee` remains the stable runtime/source identifier.
- Native and wgpu dialogue rendering prefer `speaker_label` and fall back to
  `callee` when no label is available.

## Source rules

- Character body `display = "..."` is the preferred speaker label.
- If `display` is absent, the character header name is used when present.
- Surface aliases and declaration IDs remain matching keys for resolving a
  dialogue callee back to the character declaration.

## Verification

- `cargo test -p arcweft-runtime-plan render_text`
- `cargo test -p arcweft-render-text`
- `cargo check -p arcweft-render-wgpu`
- `just test-fast`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `cargo run -p arcweft-cli --features native-capture --quiet -- agent observe samples\modern-feedback-ui\src\main.arcw --json --image png --out target\modern-feedback-ui-debug\speaker-display-label.png --mode drain --steps 8 --max-ops 128`

The modern feedback capture emitted `speaker_label: "Arcweft Concierge"` while
leaving `callee: "concierge"` unchanged, and the rendered PNG shows
`Arcweft Concierge` in the dialogue speaker label.

## Remaining work

Locale-specific character display selection is not implemented in this cut.
