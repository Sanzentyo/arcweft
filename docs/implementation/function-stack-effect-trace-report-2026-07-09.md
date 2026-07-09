# Function Stack Effect Trace Report - 2026-07-09

## Status

Implemented as a 07.8 report-boundary slice.

`EffectAnalysisReport` now owns a typed `EffectTraceReport` alongside the
existing effect summaries, closed row projection, and diagnostics. The report
contains deterministic origin traces for inferred effects, so downstream
consumers can ask "why does callable X infer effect Y?" without depending on
diagnostic emission or on sema's temporary graph internals.

## Contract

This slice does not add source-level effect-row syntax, open row variables, or
row-bearing callable values. It preserves the current effect graph semantics
and moves the existing shortest deterministic trace calculation into a stable
analysis report boundary:

- `EffectAnalysisReport::effect_traces()` returns the report for all inferred
  effects that have a witness path in the current analysis graph.
- `EffectTraceReport::trace(callable, effect)` returns the same
  `EffectTrace` shape used by diagnostics.
- `EffectTraceReport::traces_for(callable)` and `summaries()` expose typed
  summaries that retain the owning `CallableId`.
- The report is generated for ordinary successful analysis as well as
  diagnostic-producing analysis; consumers no longer need to trigger or parse
  an error diagnostic to inspect row origins.

## Evidence

Sema now has a focused regression for returned closure callback origin
reporting. The fixture calls a returned function value whose body invokes a
callback that performs `fs.read`; the analysis report exposes the caller's
`fs.read` trace and the trace includes both the local callable edge and the
external `adapter.read_text` edge.

This complements the existing LSP related-information trace tests. LSP can
continue rendering diagnostic traces from diagnostics, while future row-origin
display can read the same witness data directly from `EffectTraceReport`.

## Remaining Open Work

This is still not the final 07.8 effect-row contract. Remaining work includes
source row syntax, open-row inference/substitution, row-bearing callable
values, and deciding which runtime-plan, verifier, and LSP surfaces consume
the trace report as final row-origin evidence.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features effect_trace_report_records_returned_closure_callback_origin -- --nocapture
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-trace-report-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax` and `arcweft-lang-sema`; no warning is attributed to
the effect trace report changes. The structure audit reports the existing 1
error / 150 warning baseline.
