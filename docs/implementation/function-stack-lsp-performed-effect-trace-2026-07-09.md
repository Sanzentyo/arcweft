# Function Stack LSP Performed Effect Trace - 2026-07-09

This slice strengthens the current 07.8 LSP diagnostic evidence for effect
traces.

## What Changed

- Added `diagnostics_surface_performed_effect_trace` in `arcweft-lsp`.
- The fixture awaits an extern capability returning `Need<String, AssetError>`
  from a flow with `effects { }`.
- The resulting `control.suspend` upper-bound diagnostic must expose LSP
  `relatedInformation` containing:
  - `effect trace for control.suspend`;
  - a direct performed-effect step for `flow.await_avatar`;
  - the `await` site label.

This complements the existing returned-closure LSP trace test, which covers
function-value calls, returned function values, callback edges, and external
effect calls. Both are current graph-evidence tests; final row-origin rendering
still belongs to the 07.8 effect-row contract.

## Evidence

- `crates/arcweft-lsp/src/diagnostics.rs`
  - `diagnostics_surface_performed_effect_trace`
  - existing `diagnostics_surface_returned_closure_effect_trace`

## Validation

```bash
cargo test -p arcweft-lsp --all-features diagnostics_surface_performed_effect_trace -- --nocapture
```

The command passed.
