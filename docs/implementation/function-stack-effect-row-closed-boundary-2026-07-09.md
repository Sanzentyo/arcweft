# Function Stack Effect Row Closed Boundary - 2026-07-09

## Status

Implemented as a 07.8 boundary-hardening slice.

The sema `EffectRowReport` can now be resolved into a
`ClosedEffectRowReport` containing only typed `EffectSet` values for inferred,
upper-bound, and forbidden rows. Consumers no longer need to inspect
`EffectRow` tails or manually resolve row substitutions when they only accept
closed effect evidence.

## Contract

This slice does not add source-level row syntax and does not complete open-row
inference. It makes the existing closed-row projection a stronger crate
boundary:

- `EffectRowSummary::resolve_closed(...)` resolves one callable row summary;
- `EffectRowReport::resolve_closed(...)` resolves a whole report and attaches
  the callable identity to unresolved-row failures;
- `ClosedEffectRowSummary` and `ClosedEffectRowReport` expose only resolved
  `EffectSet` values.

The Agent verified-effects builder now consumes this closed boundary instead
of resolving `EffectRow` internals itself.

## Evidence

Unit coverage verifies that:

- closed and open rows resolve into a `ClosedEffectRowReport` through explicit
  substitutions; and
- unresolved row variables report the callable whose row could not be closed.

Compiler coverage verifies that Agent verified-effects lowering still builds
artifact proofs from the closed row boundary.

## Remaining Open Work

The final 07.8 model still needs:

- source-level row syntax;
- open-row inference/substitution from function signatures;
- row-bearing callable values for closures, returned functions, curried groups,
  and higher-order parameters;
- runtime-plan/verifier/LSP consumers beyond the current Agent artifact proof;
  and
- replacement of path-specific closure/higher-order graph edges with final row
  evidence.

## Validation

```bash
cargo test -p arcweft-lang-sema --all-features effect_row -- --nocapture
cargo test -p arcweft-compiler --all-features compile_agent_bundle_with_project_builds_agent_controller_bundle -- --nocapture
cargo test -p arcweft-compiler --all-features compile_agent_bundle_lowers_inferred_effects_not_unused_source_upper_bound -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-closed-boundary-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing
large-enum warnings in `arcweft-lang-syntax` and pre-existing `too_many_lines`
warnings in `arcweft-lang-sema`; no warning is attributed to the closed
effect-row boundary changes. The structure audit reports the existing
`crates/arcweft-lang-sema/src/checker/expr.rs` size error plus 150 warnings.
