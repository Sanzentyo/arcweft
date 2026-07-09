# Function Stack Effect Row Report Boundary - 2026-07-09

## Status

Implemented as a 07.8 boundary-hardening follow-up.

`EffectAnalysisReport` now owns the row-substitution state used by the current
effect analysis and exposes `closed_effect_rows()` as the crate-boundary method
that returns a `ClosedEffectRowReport`. Compiler and LSP consumers no longer
construct an empty `EffectSubstitution` or call `EffectRowReport::resolve_closed`
directly.

A follow-up hardening slice also moved the current `EffectRowReport` itself
into `EffectAnalysisReport`. `effect_rows()` now returns the owned report
instead of rebuilding a closed-only projection from `EffectSummary` values on
each call. This keeps the current closed-row behavior intact while making open
rows and future substitutions part of the analysis result rather than a
consumer-side reconstruction step.

## Contract

This slice does not add source effect-row syntax and does not implement open
row inference. It only tightens the boundary between sema internals and
downstream consumers:

- sema keeps the current substitution state with the effect-analysis report;
- sema keeps the current `EffectRowReport` with the effect-analysis report;
- raw `EffectRowReport` evidence remains available as `effect_rows()` for
  row-model tests and future inference work;
- downstream artifact and editor consumers accept only resolved
  `ClosedEffectRowReport` evidence.

## Evidence

Updated consumers:

- Agent verified-effects manifest lowering now calls
  `EffectAnalysisReport::closed_effect_rows()` directly.
- LSP callable declaration hover now calls the same boundary method.
- function-stack row evidence tests now assert against `ClosedEffectRowReport`
  summaries instead of resolving rows locally.
- A follow-up regression asserts that `EffectAnalysisReport::effect_rows()`
  exposes the report-owned row summary and that `closed_effect_rows()` resolves
  through the same owned report/substitution boundary.
- The public type-check facade now re-exports `ClosedEffectRowReport`,
  `ClosedEffectRowSummary`, `EffectRowCloseError`, and `EffectRowError` so
  crate-boundary consumers can name the row boundary without importing sema's
  internal module path directly.

A later artifact-consumer follow-up also changed Agent verified-effect summary
building to take `ClosedEffectRowReport` directly instead of the full
`EffectAnalysisReport`.

## Remaining Open Work

The final 07.8 model still needs source row syntax, open-row
inference/substitution, row-bearing callable values, and final row-origin
display. This slice only removes the downstream dependency on row resolution
mechanics.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features closure_effect_rows_project_closed_report_evidence -- --nocapture
cargo test -p arcweft-lang-sema --all-features effect_analysis_report_owns_effect_row_report_boundary -- --nocapture
cargo test -p arcweft-lang-sema --all-features borrowed_closure_capture_keeps_effect_row_evidence_at_await_boundary -- --nocapture
cargo test -p arcweft-lsp --all-features hover_describes_callable_closed_effect_row -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-owned-report-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax`, `arcweft-lang-sema`,
`arcweft-runtime-driver`, and `arcweft-runtime-host`; no warning is attributed
to the report-boundary changes. The structure audit reports 0 errors and 153
warnings.
