# seq06.9c Compositing Capture, Hit Metadata, and Goldens Implementation

Date: 2026-06-29

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
- adds CSS and expected JSON fixtures under the adapter crate's test fixtures;
- adds manual-only promotion review notes for exact PNG baselines.

Tests:

- `compositing_capture_schema.rs` checks the public metadata schema surface;
- `compositing_capture_fixtures.rs` verifies all five CSS compositing families
  are represented and stable fixture ids/effect evidence are present;
- `compositing_capture_source_gates.rs` blocks platform identity leakage and
  CPU-raster expected-output hooks;
- `compositing_capture_exact_png.rs` is ignored/manual-only.

## Validation commands

Run from the repository root after applying this package:

```bash
./APPLY_OVERLAY.sh /path/to/arcweft
cargo fmt --all -- --check
cargo test -p arcweft-takumi-adapter --test compositing_capture_schema -- --nocapture
cargo test -p arcweft-takumi-adapter --test compositing_capture_fixtures -- --nocapture
cargo test -p arcweft-takumi-adapter --test compositing_capture_source_gates -- --nocapture
cargo test -p arcweft-takumi-adapter --lib -- --nocapture capture evidence lowering
cargo check -p arcweft-takumi-adapter --all-targets --all-features
cargo clippy -p arcweft-takumi-adapter --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Optional manual exact PNG lane:

```bash
cargo test -p arcweft-takumi-adapter --test compositing_capture_exact_png -- --ignored --nocapture
```

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
