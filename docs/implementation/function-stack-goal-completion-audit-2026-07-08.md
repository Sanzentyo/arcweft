# Function Stack Goal Completion Audit - 2026-07-08

Current pointer: see
`docs/implementation/function-stack-status-rollup-2026-07-09.md` for the latest
one-page status. This file remains the 2026-07-08 requirement-by-requirement
audit.

## Purpose

This audit maps the active function/closure/currying/pipeline goal to current
implementation evidence. It exists to prevent declaring the goal complete from
memory or from a narrow passing test.

Status: **not complete**. The implemented surface is broad, but several
explicit requirements remain in request/design space.

## Evidence Sources

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
- `docs/implementation/function-stack-current-status-2026-07-08.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-source-function-values-2026-07-09.md`
- `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`
- `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
- `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
- `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
- `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`

## Requirement Matrix

| Goal requirement | Current evidence | Status |
| --- | --- | --- |
| Formal function types `A -> B` | Function type syntax, right associativity, tuple call-group types, and parser/sema coverage are recorded in the function-stack implementation log and status index. | Implemented |
| Multiple curried `ParamGroup`s for `fn`, task/dialogue/stream fn, trait members, and impl members | Status index records top-level/task/dialogue/stream plus trait/impl call-group preservation. `samples/function-curried-call-groups` covers tuple-tail and chained groups. | Implemented |
| Reject curried `flow` parameters | Status index records direct rejection. | Implemented |
| Preserve `f(a)(b)` vs `f(a, b)` semantics | Status index records parser/sema/runtime distinction and runtime-plan preservation. | Implemented |
| Closure expressions `|x| expr` / `|| expr` with typed/pattern parameters | Closure typing, typed parameters, parameter patterns, braced return annotations, and closure-local `return` are recorded in the status index. | Implemented |
| Capture analysis hooks and lifetime diagnostics at suspension boundaries | Capture inventory, checked runtime-plan capture metadata, and borrowed-capture suspension diagnostics are recorded in 07.4 and the status index. | Implemented for current policy |
| Expression `_` placeholder abstraction distinct from pattern wildcard | Expected-function `_`, inferred binary `_`, known-callable partial-call abstraction, and pattern `_` distinction are recorded in the status index. | Implemented for fixed accepted shapes |
| Partial-application desugaring | Helper-backed prefix partials, named/fixed missing-input partials, local aliases, runtime apply, and the first simple non-helper source-local `fn` materialization are implemented, including curried groups in that accepted family. Helper-less signature partials still fail as unsupported callable family `signature_partial_without_helper` when no pure helper or accepted source-function candidate exists; spread partials remain request/design work. | Partially implemented; spread split to 07.2.1 and broader non-helper expansion remains in 07.7 |
| `^` pipe-left placeholder scoped only inside pipe RHS | Status index records scoped RHS behavior and substitution. | Implemented |
| Left-associative `|>` with `^` substitution or data-last application when no `^` appears | Status index records explicit `^` substitution, no-`^` data-last application, helper-aware pipes, and local function-valued aliases. | Implemented for fixed accepted shapes |
| Method-chain sugar with inherent/trait first and data-last fallback with ambiguity diagnostics | Status index records resolution order, deterministic runtime argument order, ambiguity diagnostics, real-method priority, and shadowed fallback warnings. Spread fallback remains request/design work. | Partially implemented; spread fallback split to 07.2.1 |
| Let type ascription and numeric literal inference/fallback representation with LSP/lint hooks | Status index records inferred function-valued `let` inlays, numeric fallback lints in inferred closure bodies, and source-backed expression inlays. | Implemented for current policy |
| Canonical primitive spellings without compatibility aliases or formatter shims | Status index records accepted canonical primitive labels and rejected non-canonical spellings. | Implemented |
| Keep `Unit` / `Never` consistent | Status index records canonical `Unit` / `Never` behavior as part of primitive spelling. | Implemented |
| Keep relative IDs consistent | Runtime ID path/public-label split and AWBC typed flow lookup are implemented; atom-table storage is deferred until profiling evidence. | Implemented except atom-table non-goal |
| Keep enum shorthand behavior consistent | Sema coverage now verifies user-defined unit, tuple-payload, and record-payload enum short constructors through expected-type resolution. Runtime-plan coverage verifies the same unit, tuple-payload, and record-payload short constructors lower to `RuntimeExpr::Variant` instead of a `DataFormat.Json`-only path or plain record payload. | Implemented |
| Update docs, samples, diagnostics, and tests across parser/HIR/sema/runtime-plan/LSP | Implementation logs and status index list parser, sema, runtime-plan, AWBC, LSP, sample, diagnostic, and source-range evidence. | Broadly implemented; final claim requires full test audit |
| Split underspecified or larger future work into requests | Spread partials, AWBC resumable apply/persisted closures, non-helper callable allocation, closure effect-row finalization, and runtime-ID atom-table storage all have request/design boundaries. | Implemented |

## Remaining Blocking Work

The goal is not complete because these explicit areas remain unresolved:

1. Spread partial application and spread data-last fallback semantics:
   `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
2. AWBC suspension-aware dynamic function apply and persisted closure
   snapshots:
   `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
3. General non-helper/effectful/suspending callable allocation:
   `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
   The callable-family inventory step is complete, but the first accepted
   expansion beyond helper-backed callables is still not designed.
4. Full closure effect-row final contract:
   `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`

Runtime ID atom-table storage remains deliberately deferred until profiling
evidence justifies carrying table context through runtime-plan/data-format
boundaries. It is not an implementation-ready blocker by itself.

## Current Non-Goal Worktree Changes

The dirty View/Web/text-input files listed in
`docs/implementation/current-work-status-2026-07-08.md` are separate from this
goal audit. They must not be folded into function-stack completion evidence.

## Validation For This Audit

```bash
git diff --check -- docs/implementation docs/reviews/requests
cargo test -p arcweft-lang-sema --all-features expected_type_resolves_user_enum_short_variant -- --nocapture
cargo test -p arcweft-runtime-plan --all-features runtime_plan_lowers_user_enum_shorthand_payloads_to_variants -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-enum-shorthand-2026-07-08
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_curried_source_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
```
