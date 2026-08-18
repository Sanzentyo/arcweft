# Checked prefix Try carrier CFG — 2026-08-18

## Inspected state

- Inspected Git revision: `742a743c3dd3081726dbde67e4776691f4802678`.
- Working tree at inspection: clean on `main`, matching `origin/main`.
- Implementation state recorded here: dirty direct-checkout cut, before its
  commit.

## Established boundary

Prefix `try` is represented by one checked expression fact. The fact owns the
exact operand, the closed `Result` or `Option` carrier, and the nearest typed
propagation boundary: a carrier block, a callable, or the infallible
`Result<_, Never>` case. Await does not own propagation and no fused Try/Await
variant or postfix-question form is introduced.

The compiler projects that checked fact one-to-one into runtime semantic
facts. Flow lowering admits deterministic hidden success/residual locals in
the existing semantic batch and lowers Try to ordinary typed Match, Return,
and carrier-construction operations. Carrier blocks wrap their normal tail and
short-circuit residuals; callable boundaries return the corresponding
`Result::Err` or `Option::None`. A control-producing child such as Await is
evaluated once and supplied to the enclosing expression through a typed local
override, so `try await value` remains ordinary composition.

## Performed and passed

- `cargo check -p arcweft-lang-sema -p arcweft-runtime-plan
  -p arcweft-compiler -p arcweft-lsp --all-targets --all-features`.
- `cargo test -p arcweft-lang-sema --lib` — 199 passed.
- `cargo test -p arcweft-runtime-plan --lib` — 47 passed.
- `cargo test -p arcweft-compiler --lib` — 52 passed.
- `cargo test -p arcweft-lsp --lib
  propagation_diagnostics_project_exact_try_operator` — 1 passed.
- `arcw compile --emit check` for
  `current_pass/check/018_result_option_try_boundaries.arcw` — 3 flows,
  0 warnings, verified.
- `arcw compile --emit check` for
  `spec_should_pass/check/010_capability_fs_read.arcw` — 1 flow,
  0 warnings, verified.
- `arcw run --entry entry.main --mode drain --steps 4 --max-ops 32` for
  `spec_should_pass/run/002_file_read_task.arcw` — terminal
  `Return("missing")` after the authored Error handler.
- `just structure-audit` — 0 blocking violations.
- `cargo fmt --all -- --check` and `git diff --check`.

The first post-change check encountered a full `D:` volume in Cargo's
regenerable incremental cache. `cargo clean` removed the build artifacts, the
same check was rerun with incremental compilation disabled, and it passed.

## Failed or blocked validation

- Strict runtime-plan Clippy is blocked before this crate by existing strict
  warnings in `arcweft-lang-syntax` and `arcweft-core`: large enum variants,
  long functions, and the existing task constructor argument count. No
  cut-local Clippy diagnostic remained in the emitted output.
- The positive directory fixture target was also run during this cut and stops
  at the pre-existing
  `current_pass/check/008_let_else_diverge.arcw` HIR transaction failure before
  reaching the new fixture. The exact new and maintained Try fixtures were run
  separately as listed above.

## Structural review

- `CheckedExpressionResolution::Try` is the sole sema propagation judgment;
  diagnostics and validation consume it rather than reconstructing source
  spelling.
- `RuntimeTryFact` is bound to the exact HIR generation and normalized type
  roots. Runtime lowering consumes it without a second carrier resolver.
- `FinalFlowLowerer` owns the control-flow expansion because Try can compose
  with suspension and loop expressions. `FinalExprLowerer` only receives
  already-evaluated typed local substitutions and remains a pure expression
  lowerer.

## Non-goals and remaining work

- Unary `Need<T>` is not implemented in this cut. The current Await result and
  handler surface remains until the separate direct replacement.
- Partial abstraction `_` and pipe-left `^` still need generic implicit
  callable elaboration. This cut does not add Try-specific placeholder paths.
- Try inside pure helper/function evaluation is not yet lowered through this
  Flow CFG path. It must be implemented through the final callable execution
  boundary rather than by teaching the pure expression carrier to suspend.
- The repository-wide strict Clippy backlog and the unrelated positive fixture
  failure remain separate cuts.
