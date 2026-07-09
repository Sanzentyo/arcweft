# Function Stack Effect Row Artifact Closed Input - 2026-07-09

## Status

Implemented as a 07.8 artifact-consumer boundary slice.

Agent verified-effect manifest lowering now takes a `ClosedEffectRowReport`
instead of an `EffectAnalysisReport`. The Agent bundle compiler closes the
effect rows after successful type checking and passes only boundary-safe row
evidence into the artifact builder.

## Contract

This slice does not add new source syntax, open-row inference, or row-bearing
callable values. It tightens the artifact boundary:

- `effect_manifest::build_verified_effect_summary` consumes
  `ClosedEffectRowReport`;
- missing callable rows remain a structured artifact-build error;
- unresolved rows are converted at the Agent bundle boundary before artifact
  construction;
- the serialized `declared` slot still mirrors the closed inferred row, not the
  source upper bound.

## Evidence

Unit coverage verifies that the verified-effect builder:

- uses the closed inferred row even when the source upper bound contains extra
  capabilities; and
- rejects a missing callable row.

The existing Agent bundle regression still verifies that unused source upper
bound effects are not serialized into `declared` or `inferred`.

## Remaining Open Work

This is still not the final 07.8 model. Source row syntax, open-row
inference/substitution, row-bearing callable values, and richer row-origin
display remain open.

## Validation

```bash
cargo test -p arcweft-compiler --all-features verified_effect_summary -- --nocapture
cargo test -p arcweft-compiler --all-features compile_agent_bundle_lowers_inferred_effects_not_unused_source_upper_bound -- --nocapture
cargo check -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-artifact-closed-input-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax` and `arcweft-lang-sema`; no warning is attributed to
the artifact closed-input changes. The structure audit reports the existing
`crates/arcweft-lang-sema/src/checker/expr.rs` size error plus 150 warnings.
