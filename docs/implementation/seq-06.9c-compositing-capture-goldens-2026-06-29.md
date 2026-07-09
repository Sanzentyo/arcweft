# seq06.9c Compositing Capture, Hit Metadata, and Goldens Implementation

Date: 2026-06-29

Maintenance update: 2026-07-10. The original source-spelling gates and the
placeholder "exact PNG" test were removed. They inspected repository text and
did not capture pixels. The maintained CI contract is now the complete typed
evidence packet comparison described below.

## Baseline

This package is designed against the latest inspected main-line state after
seq06.9a and seq06.9b, specifically the commit whose message is `Apply seq06.9b
wgpu View compositor effects` (`21476c5843845eed29e0376efb9aa4930c6abf02`).

## Implemented overlay

Rust implementation:

- extends `arcweft-takumi-adapter::capture` with:
  - `TakumiPaintNodeId`;
  - `TakumiCompositingGroupId`;
  - `TakumiEffectOutsets`;
  - expanded object capture bounds fields;
  - `TakumiCompositingCaptureRecord`;
  - `TakumiCaptureFrame::compositing_records()`;
  - `TakumiCaptureFrame::evidence_json()`;
- adds `arcweft-takumi-adapter::evidence` for deterministic JSON emission;
- wires Takumi lowering through `patches/seq06.9c-lowering-capture-wiring.patch`
  so direct records receive paint/group ids and group records receive layout,
  visual, hit, clip, mask, effect, isolation, blend, and primitive-range evidence;
- exports the new evidence/capture types from `arcweft-takumi-adapter::lib`.

Schemas and fixtures:

- adds `docs/schemas/compositing-capture-evidence.schema.json`;
- adds `docs/schemas/compositing-capture-evidence.md`;
- adds the expected JSON fixture under the adapter crate's test fixtures.

Tests:

- `compositing_capture_schema.rs` checks the public metadata schema surface;
- `compositing_capture_fixtures.rs` compares the complete deterministic JSON
  emitted from a typed compositing record with `expected-evidence.json`;
- renderer and lowering behavior remains covered by their typed unit and
  integration tests. No source-spelling or placeholder PNG gate is retained.

## Validation commands

Run from the repository root after applying this package:

```bash
./APPLY_OVERLAY.sh /path/to/arcweft
cargo fmt --all -- --check
cargo test -p arcweft-takumi-adapter --test compositing_capture_schema -- --nocapture
cargo test -p arcweft-takumi-adapter --test compositing_capture_fixtures -- --nocapture
cargo test -p arcweft-takumi-adapter --lib -- --nocapture capture evidence lowering
cargo check -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Pixel-exact promotion remains out of scope until a real pinned-GPU capture lane
exists. It must compare generated image bytes or metrics, not source text.

## Structural audit notes

This package adds new capture/evidence responsibilities but keeps them in the
Takumi adapter crate because the data is created at the lowering boundary. It
does not add renderer dependencies beyond the existing `arcweft-render-wgpu`
contract already used by the adapter.

The lowerer patch extends existing lowering flow with id allocation and group
capture records. It does not move shader planning into the adapter and does not
read files or touch platform APIs.

## Known validation status for this generated zip

In this artifact-generation environment, the private/full checkout and Rust
components required for Cargo validation were unavailable. The zip package itself
was created locally and must be validated with the commands above after applying
to the repository.
