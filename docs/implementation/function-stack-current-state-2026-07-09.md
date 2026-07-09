# Function Stack Current State - 2026-07-09

This is the current entry point for the active
function/closure/currying/pipeline language-stack goal. It consolidates the
status rollup, gap map, and request boundaries after the latest pushed
function-stack slice.

Status: **open**. The implemented surface is broad, but the goal is not
complete.

## Executive Summary

- The latest function-stack baseline accepts pure pipe expressions inside
  accepted source-function bodies when they lower through local function
  values, pure helpers, or accepted source-function candidates, while
  continuing to reject unsupported bare source-function value references and
  unsupported data-last source-function partials instead of lowering them as
  ordinary calls/locals.
- The function-stack worktree is clean at that baseline.
- Implemented language/runtime surface now covers formal function types,
  curried call groups, closures, runtime apply, non-suspending AWBC apply,
  fixed-shape `_` partials, fixed-shape pipes, method fallback, the closed
  fixed-literal spread partial/fallback contract, typed runtime IDs, user enum
  shorthand, source identity evidence, the first accepted non-helper
  source-local `fn` subset, and explicit rejection for bare and data-last
  source-function values outside that accepted subset.
- The active goal remains open only for contract-sized items that should not
  be guessed from implementation: suspension-aware AWBC dynamic apply,
  persisted function snapshots, broad non-helper/effectful/suspending callable
  allocation, and the final closure effect-row model.

## Baseline

- Current pushed function-stack baseline:
  the function-stack baseline that accepts top-level pure-helper/source-
  function aliases and pure pipe expressions inside accepted source-function
  bodies and rejects unsupported bare source-function values and data-last
  source-function partials without executable runtime candidates.
- The previous function-stack baseline before the spread rejection hardening
  slice was `486738b31 Handle pipe control-expression RHS placeholders`.
- Earlier status-cleanup and pure-helper source-function commits were
  `7841f2613 Document current function stack gaps` and
  `d8254a253 Allow pure helper calls in source function values`.
- Keep any future unrelated View/Web/text-input changes separate from this
  function-stack state; they should not be staged with language-stack
  documentation or implementation unless deliberately validated as their own
  slice.

## Documentation Map

Read these files in this order:

1. `docs/implementation/function-stack-current-state-2026-07-09.md`
   gives the current answer to "what is done and what remains?"
2. `docs/implementation/function-stack-status-rollup-2026-07-09.md`
   is the one-page implementation rollup with evidence pointers.
3. `docs/implementation/function-stack-current-gap-map-2026-07-09.md`
   is the shorter blocker map for choosing the next slice.
4. `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
   is the long chronological implementation log.
5. `docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`
   is the requirement-by-requirement audit against the active goal text.

Supporting focused notes:

- `docs/implementation/function-stack-pipe-control-expression-rhs-2026-07-09.md`
- `docs/implementation/function-stack-spread-rejection-boundary-2026-07-09.md`
- `docs/implementation/function-stack-function-value-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-signature-fixed-spread-apply-2026-07-09.md`
- `docs/implementation/function-stack-spread-contract-closure-2026-07-09.md`
- `docs/implementation/function-stack-unsupported-bare-source-function-values-2026-07-09.md`
- `docs/implementation/function-stack-data-last-unsupported-source-partial-2026-07-09.md`
- `docs/implementation/function-stack-data-last-callable-kind-partial-rejection-2026-07-09.md`
- `docs/implementation/function-stack-prefix-source-partial-rejection-2026-07-09.md`
- `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`
- `docs/implementation/function-stack-source-function-top-level-aliases-2026-07-09.md`
- `docs/implementation/function-stack-source-function-pipe-bodies-2026-07-09.md`
- `docs/implementation/function-stack-non-helper-callable-kind-rejection-2026-07-09.md`
- `docs/implementation/function-stack-method-value-rejection-2026-07-09.md`
- `docs/implementation/function-stack-awbc-control-expression-parity-2026-07-09.md`
- `docs/implementation/function-stack-awbc-expression-apply-suspension-boundary-2026-07-09.md`
- `docs/implementation/function-stack-closure-effect-row-audit-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-closed-boundary-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-report-boundary-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-artifact-closed-input-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-lsp-hover-2026-07-09.md`
- `docs/implementation/function-stack-effect-trace-report-2026-07-09.md`
- `docs/implementation/function-stack-function-type-effect-rows-2026-07-09.md`
- `docs/implementation/function-stack-closure-effect-callable-evidence-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-partial-closure-timing-2026-07-09.md`
- `docs/implementation/function-stack-effect-row-curried-higher-order-timing-2026-07-09.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`

## Implemented And Pushed

The following are implemented in pushed commits:

- Formal function types `A -> B`, right associativity, tuple call-group
  function types, and multiple curried `ParamGroup`s for function-like
  declarations.
- Rejection of curried `flow` parameter groups.
- Parser-level call/select unification through neutral `Expr::Select` plus
  `Expr::Call`.
- Expression closures, zero-argument closures, typed closure parameters,
  destructuring closure parameters, braced closure return annotations, and
  closure-local `return`.
- Closure capture inventory and borrowed-capture diagnostics at checked
  suspension boundaries.
- Runtime `Function` / `Apply`, exact apply, partial apply, and curried apply
  for accepted non-suspending paths, with low-level spread-argument expansion
  verified for runtime `Apply` and inline fixed-length literal spread accepted
  for source function-value calls, direct fixed-parameter signature calls, and
  data-last method fallback. Variable-length spread in partial-call
  construction and data-last fallback is a structured rejection by the current
  language contract.
- AWBC `MakeFunction` / `ApplyFunction` for non-suspending generated runtime
  functions.
- Lazy AWBC lowering for value-position `if`, `if let`, and `match` in
  generated function bodies.
- AWBC expression-level `ApplyFunction` rejection coverage for applied
  functions that suspend or exhaust the synchronous expression-apply budget,
  so the current runtime cannot accidentally claim resumable dynamic apply.
- Expression `_` placeholder abstraction for the implemented
  expected-function and known-callable shapes, distinct from pattern `_`.
- Pipe `^` substitution and no-`^` data-last application for implemented fixed
  argument paths, including value-position `if`, `if let`, and `match`
  expressions in the pipe RHS.
- Method-chain fallback after inherent/trait/env method lookup, with
  deterministic argument order and ambiguity diagnostics.
- Canonical primitive spellings without compatibility aliases or formatter
  normalization shims.
- User enum shorthand lowering through the expected-type path.
- Typed runtime ID paths instead of raw public-label string newtypes.
- Source identity and LSP inlay evidence for the audited expression and
  statement families.
- LSP effect diagnostics surface current graph traces as related information
  for returned-closure callback edges and directly performed static effects.
- Current closure-effect composition for the broad implemented closure,
  callback, higher-order, returned-closure, and curried-call paths, plus
  captured-function-alias preservation, borrowed-capture row evidence at an
  `await` boundary, partial-closure timing, returned-closure callback timing,
  curried higher-order callback timing, `no_effect` invocation coverage, and a
  closed-row report projection consumed by Agent verified-effects lowering
  through a typed `ClosedEffectRowReport` boundary. `EffectAnalysisReport`
  now owns current row-substitution resolution and returns that closed report
  directly to compiler/LSP consumers. Agent verified-effect artifact lowering
  now consumes `ClosedEffectRowReport` as its direct input instead of accepting
  the full effect-analysis report. LSP callable declaration hover also consumes
  that closed boundary for current inferred/upper-bound/forbidden row display.
  `EffectAnalysisReport` also owns a typed `EffectTraceReport`, so row-origin
  witnesses for inferred effects can be consumed from the normal analysis
  report instead of only from effect diagnostics. Semantic `Function` types now
  carry `EffectRow` values, environment/project function values can expose
  closed registered rows, function-value partials preserve those rows, and
  source labels display closed non-empty rows. Closure expressions also export
  typed lowering evidence that joins their expression judgment to the
  synthetic callable used by closed row reports.
- Product AWBC save/load structured rejection of escaped runtime function
  values.
- Checked runtime-plan lowering rejects bare top-level source-function value
  references when type checking proves a function value but no pure helper or
  accepted source-function candidate exists.
- Checked runtime-plan lowering rejects data-last pipe partials through
  unsupported source functions with the same `signature_partial_without_helper`
  family used by direct partial calls.
- Checked runtime-plan lowering rejects data-last partials through unsupported
  `task fn`, `dialogue fn`, and `stream fn` declarations with
  `signature_partial_without_helper`.
- Checked runtime-plan lowering rejects positional prefix partial calls through
  unsupported source functions with `signature_partial_without_helper`, proving
  the direct partial-call rejection boundary covers both named missing-input and
  prefix partial shapes.
- Checked runtime-plan lowering rejects source-local wrapper functions that
  exact-call unaccepted source-local functions when those wrappers are used as
  missing-input partials, data-last partials, or bare function values.
- Checked runtime-plan lowering rejects bare `task fn`, `dialogue fn`, and
  `stream fn` value references with
  `source_function_value_without_runtime_candidate` instead of treating those
  callable kinds as ordinary locals.
- Sema rejects value-position environment, inherent, and trait/impl method
  references such as `score.above` with structured
  `UnsupportedMethodValueReference` diagnostics instead of letting them fall
  through as ordinary field selection. This preserves the explicit
  receiver-binding boundary until method values have a stable callable
  representation.
- A first non-helper source-local `fn` runtime-function subset:
  ordinary source `fn` declarations with fixed identifier declaration
  parameters and simple expression/final-return bodies, including multiple
  curried groups, returned simple closure literals, direct calls to
  function-typed parameters, function-valued local aliases/partials,
  destructuring closure literals in those local aliases, exact calls to
  already-lowered pure helpers, fixed-point exact calls to already-accepted
  source-local candidates, simple local aliases to those already executable
  top-level function values, pipe expressions that lower only through local
  function values, pure helpers, or accepted source-function candidates, and
  pure value-position `if` / `if let` / `match` expressions.

## Remaining Blocking Work

These items keep the active goal open:

| Area | Current state | Blocking document |
| --- | --- | --- |
| AWBC suspension-aware dynamic apply | Non-suspending dynamic apply works. Applying a function that suspends or budget-yields is explicitly rejected as a runtime trap in the synchronous expression-apply path, but there is still no resumable safe-point contract for accepting that behavior. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`; `docs/implementation/function-stack-awbc-expression-apply-suspension-boundary-2026-07-09.md` |
| Persisted closure/function snapshots | Product AWBC save/load now rejects function values explicitly. Serializable closure state, captured environment versioning, and restore semantics are not designed. | `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md` |
| Broad non-helper callable allocation | The first source-local `fn` subset is implemented, including pure value control expressions, fixed-point exact calls to already-accepted source-local candidates, local aliases to already executable pure-helper/source-function values, and pure pipe expressions through those executable callable paths inside accepted source-function bodies. Bare task/dialogue/stream function values now have focused structured rejection coverage. Data-last task/dialogue/stream partials now have focused structured rejection coverage. Source-local wrappers that exact-call unaccepted source-local functions now have focused structured rejection coverage for missing-input partial, data-last partial, and bare-value surfaces. Value-position environment, inherent, and trait/impl method references now have focused structured rejection coverage. Effectful/suspending bodies, host/adapter call-bearing bodies, accepted task/dialogue/stream values, accepted method values, adapter thunks, and persisted callable values remain outside the accepted contract. | `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`; `docs/implementation/function-stack-source-function-top-level-aliases-2026-07-09.md`; `docs/implementation/function-stack-source-function-pipe-bodies-2026-07-09.md`; `docs/implementation/function-stack-source-function-unaccepted-source-call-rejection-2026-07-09.md`; `docs/implementation/function-stack-non-helper-callable-kind-rejection-2026-07-09.md`; `docs/implementation/function-stack-data-last-callable-kind-partial-rejection-2026-07-09.md`; `docs/implementation/function-stack-method-value-rejection-2026-07-09.md` |
| Final closure effect-row model | Current effect composition, captured-function-alias preservation, partial-closure delayed timing evidence, returned-closure callback timing evidence, curried higher-order delayed timing evidence, LSP trace related-information evidence, typed closed-row boundary projection, report-owned closed-row resolution, direct artifact consumption of `ClosedEffectRowReport`, LSP declaration hover consumption, report-owned `EffectTraceReport` row-origin witnesses, row-bearing semantic `Function` types for known function values, and closure expression callable evidence are useful, but source row syntax, open-row inference/substitution, row variables for closure and higher-order parameters, and final runtime-plan/verifier/LSP consumers are not finalized. | `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`; `docs/implementation/function-stack-effect-row-closed-boundary-2026-07-09.md`; `docs/implementation/function-stack-effect-row-report-boundary-2026-07-09.md`; `docs/implementation/function-stack-effect-row-artifact-closed-input-2026-07-09.md`; `docs/implementation/function-stack-effect-row-lsp-hover-2026-07-09.md`; `docs/implementation/function-stack-effect-trace-report-2026-07-09.md`; `docs/implementation/function-stack-function-type-effect-rows-2026-07-09.md`; `docs/implementation/function-stack-closure-effect-callable-evidence-2026-07-09.md`; `docs/implementation/function-stack-effect-row-partial-closure-timing-2026-07-09.md`; `docs/implementation/function-stack-effect-row-returned-closure-no-effect-2026-07-09.md`; `docs/implementation/function-stack-lsp-performed-effect-trace-2026-07-09.md`; `docs/implementation/function-stack-effect-row-curried-higher-order-timing-2026-07-09.md` |

Runtime ID atom-table storage is deferred until profiling shows ID comparison,
hashing, serialization, or allocation pressure. The typed path API is in
place, so atom-table storage is not a completion blocker by itself.

## What Is Implementation-Ready Next

No remaining broad blocker should be implemented by guessing a contract. At
this audit point, the function-stack items that are obviously
implementation-ready are narrow hardening/documentation tasks around already
accepted behavior, not new language semantics. The next implementation slice
should either:

- answer one of the request boundaries above with a concrete accepted contract,
  then implement the smallest end-to-end behavior covered by that contract; or
- take a narrow already-specified hardening step, such as improving an existing
  rejection diagnostic or validation fixture, without widening accepted
  language behavior.

For the non-helper callable area, the next meaningful implementation after the
current source-local `fn` subset needs an explicit answer about identity,
effects, suspension, AWBC lowering, and persistence. Without that, accepting
more callable families would create hidden adapter or save/load semantics.

## Separate Dirty Track

The current working copy has a separate uncommitted View/Web/text-input track.
It covers rendering, View style/radius/shadow/filter behavior, fonts,
text-control/IME behavior, Web player/EditContext glue, generated `.awfb`
artifacts, and the modern feedback sample.

That track is real work, but it is not evidence for this goal. It should be
inspected, validated, and committed separately from function-stack work.

## Validation For This State

The exact pure-helper source-function slice was validated with:

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
rustfmt --edition 2024 --check crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-runtime-plan\src\flow.rs crates\arcweft-runtime-plan\src\flow\pure_helpers.rs crates\arcweft-compiler\src\tests.rs
git diff --check -- crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-runtime-plan\src\flow.rs crates\arcweft-runtime-plan\src\flow\pure_helpers.rs crates\arcweft-compiler\src\tests.rs docs\implementation\2026-07-07-functions-closures-pipeline-language-stack.md docs\implementation\current-work-status-2026-07-09.md docs\implementation\function-stack-current-gap-map-2026-07-09.md docs\implementation\function-stack-goal-completion-audit-2026-07-08.md docs\implementation\function-stack-non-helper-source-function-values-2026-07-09.md docs\implementation\function-stack-status-rollup-2026-07-09.md docs\reviews\requests\2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
```

All commands passed. Clippy reported only pre-existing warnings, and the
structure audit reported 0 errors / 151 warnings after splitting
`flow/pure_helpers.rs`.

The pipe control-expression RHS hardening slice was validated with:

```bash
cargo test -p arcweft-lang-syntax --all-features parses_pipe_rhs_ -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_substitutes_pipe_left_inside_ -- --nocapture
cargo test -p arcweft-lang-sema --all-features typechecker_lowers_pipe_placeholder_and_data_last_calls -- --nocapture
rustfmt --edition 2024 --check crates\arcweft-lang-syntax\src\expr.rs crates\arcweft-lang-syntax\src\expr\control_parse.rs crates\arcweft-lang-syntax\src\parser\statements.rs crates\arcweft-lang-sema\src\checker\expr\pipe.rs crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\expr\desugar.rs crates\arcweft-runtime-plan\src\expr\tests.rs crates\arcweft-compiler\src\tests.rs
cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-pipe-control-expression-rhs-2026-07-09
git diff --check -- crates\arcweft-lang-syntax\src\expr.rs crates\arcweft-lang-syntax\src\expr\control_parse.rs crates\arcweft-lang-syntax\src\parser\statements.rs crates\arcweft-lang-sema\src\checker\expr\pipe.rs crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\expr\desugar.rs crates\arcweft-runtime-plan\src\expr\tests.rs crates\arcweft-compiler\src\tests.rs docs\implementation\2026-07-07-functions-closures-pipeline-language-stack.md docs\implementation\function-stack-current-state-2026-07-09.md docs\implementation\function-stack-current-gap-map-2026-07-09.md docs\implementation\function-stack-status-rollup-2026-07-09.md docs\implementation\function-stack-pipe-control-expression-rhs-2026-07-09.md
```
