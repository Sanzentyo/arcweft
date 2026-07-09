# Function Stack Effect Row Report Boundary - 2026-07-09

## Status

Implemented as a 07.8 boundary-hardening follow-up.

`EffectAnalysisReport` now owns the row-substitution state used by the current
effect analysis and exposes `closed_effect_rows()` as the crate-boundary method
that returns a `ClosedEffectRowReport`. Compiler and LSP consumers no longer
construct an empty `EffectSubstitution` or call `EffectRowReport::resolve_closed`
directly.

## Contract

This slice does not add source effect-row syntax and does not implement open
row inference. It only tightens the boundary between sema internals and
downstream consumers:

- sema keeps the current substitution state with the effect-analysis report;
- raw `EffectRowReport` projection remains available inside sema as
  `effect_rows()` for row-model tests and future inference work;
- downstream artifact and editor consumers accept only resolved
  `ClosedEffectRowReport` evidence.

## Evidence

Updated consumers:

- Agent verified-effects manifest lowering now calls
  `EffectAnalysisReport::closed_effect_rows()` directly.
- LSP callable declaration hover now calls the same boundary method.
- function-stack row evidence tests now assert against `ClosedEffectRowReport`
  summaries instead of resolving rows locally.

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
cargo test -p arcweft-lang-sema --all-features borrowed_closure_capture_keeps_effect_row_evidence_at_await_boundary -- --nocapture
cargo test -p arcweft-lsp --all-features hover_describes_callable_closed_effect_row -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-report-boundary-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax`, `arcweft-lang-sema`,
`arcweft-runtime-driver`, and `arcweft-runtime-host`; no warning is attributed
to the report-boundary changes. The structure audit reports the existing
`crates/arcweft-lang-sema/src/checker/expr.rs` size error plus 150 warnings.
