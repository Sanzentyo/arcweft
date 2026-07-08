# Current Work Status - 2026-07-09

This note is the current repository map after the latest pushed function-stack
slice. It supersedes the operational pointers in
`docs/implementation/current-work-status-2026-07-08.md` without rewriting that
historical note.

## Repository Baseline

- Baseline before the latest function-stack status refresh:
  `097f694a5 Apply source callback parameters`.
- `main` and `origin/main` are aligned at that head.
- The working copy still has unrelated View/Web/text-input changes. They are
  not part of the function/closure/currying/pipeline goal and should not be
  staged with function-stack documentation or language changes.

The unrelated dirty files at this audit point are:

- `crates/arcweft-cli/src/app/bundle/tests.rs`
- `crates/arcweft-cli/src/app/bundle_view.rs`
- `crates/arcweft-cli/src/app/bundle_view_layout.rs`
- `crates/arcweft-cli/src/app/progress.rs`
- `crates/arcweft-cli/src/app/runtime/run.rs`
- `crates/arcweft-cli/src/app.rs`
- `crates/arcweft-cli/tests/native_text_input_native_interactive_smoke.rs`
- `crates/arcweft-cli/tests/native_text_input_sample_sidecars.rs`
- `crates/arcweft-player-scene/src/fonts.rs`
- `crates/arcweft-player-scene/src/input.rs`
- `crates/arcweft-player-scene/tests/action_button_submit.rs`
- `crates/arcweft-render-wgpu/src/font_system.rs`
- `crates/arcweft-render-wgpu/src/geometry/text_controls.rs`
- `crates/arcweft-render-wgpu/src/renderer.rs`
- `crates/arcweft-render-wgpu/src/view_compositor.rs`
- `crates/arcweft-render-wgpu/src/view_compositor_uniform.rs`
- `crates/arcweft-render-wgpu/src/view_shaders/compositor.wgsl`
- `crates/arcweft-runtime-driver/src/session.rs`
- `crates/arcweft-runtime-driver/tests/session.rs`
- `samples/modern-feedback-view/README.md`
- `samples/modern-feedback-view/src/main.arcw`
- `web/assets/README.md`
- `web/assets/noto-emoji-regular.ttf`
- `web/ime-player-rendered.awfb`
- `web/index.html`
- `web/modern-feedback-view.awfb`
- `web/player-editcontext.js`
- `web/player.js`
- `web/tests/ime-sample-smoke.mjs`
- `web/tests/player-editcontext-glue-unit.mjs`

## Active Goal Status

The active function/closure/currying/pipeline language-stack goal remains
open. The implemented surface is broad, but completion still depends on four
explicit request/design areas:

1. Spread partial application and spread data-last fallback semantics.
2. AWBC suspension-aware dynamic apply plus resume-point behavior.
3. Serializable persisted closure/function snapshots.
4. General non-helper/effectful/suspending callable allocation and the final
   closure effect-row contract.

The current summary is:

- `docs/implementation/function-stack-status-rollup-2026-07-09.md`

The detailed evidence trail remains:

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`
- `docs/implementation/function-stack-current-status-2026-07-08.md`
- `docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`
- `docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
- `docs/implementation/function-stack-request-split-audit-2026-07-08.md`
- `docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`

## Completed And Pushed Function-Stack Slices

- Call/select unification: parser surface uses neutral `Expr::Select` plus
  `Expr::Call`; runtime semantics stay in the lowered executable operation.
- Function types and curried call groups: `A -> B`, right associativity, tuple
  call groups, multiple curried `ParamGroup`s for function-like declarations,
  and rejection of curried `flow` parameters.
- Closure syntax and runtime apply: expression closures, typed/pattern
  parameters, braced closure return annotations, closure-local `return`,
  captured runtime functions, destructured closure parameters lowered through
  runtime pattern matches, exact apply, partial apply, and curried apply.
- First AWBC closure/apply cut: non-suspending generated closures lower through
  `MakeFunction` and `ApplyFunction`; snapshot persistence rejects runtime
  function values explicitly.
- Placeholder and pipe behavior: expression `_` is distinct from pattern `_`;
  `^` is pipe-RHS scoped; no-`^` pipes use data-last application for the
  implemented fixed-argument paths. Named RHS calls in those pipes now preserve
  callable input-name order for pure helpers and accepted source-function
  candidates.
- Method-chain fallback: inherent/trait/env methods win before data-last
  callable fallback; implemented fallback cases carry deterministic argument
  ordering and ambiguity diagnostics.
- Canonical primitive spellings: accepted primitive names are canonical, and
  non-canonical aliases are rejected rather than normalized.
- Expected-type enum shorthand: user-defined unit, tuple-payload, and
  record-payload short constructors are covered in sema and runtime-plan
  lowering.
- Runtime ID boundary cleanup: runtime lookup IDs use typed `RuntimeIdPath`
  values and AWBC flow targets use typed `FlowRuntimeId` keys instead of raw
  public-label maps.
- Source identity and tooling evidence: source ranges and type inlay evidence
  cover the audited expression/statement families, with `TypeCheckStats`
  reporting source-backed and source-missing counts.
- Non-helper callable inventory: accepted, rejected, adapter-facing, and
  design-blocked callable families are classified; unsupported helper-less
  signature partials now report the explicit family marker
  `signature_partial_without_helper`.
- First non-helper source function value cut: source-local `fn` declarations
  with simple identifier parameters and expression bodies that contain no
  host/effect/suspension-capable syntax now materialize as
  `RuntimeExpr::Function` values, including named missing-input wrapper
  partials, multiple curried `ParamGroup`s lowered to nested functions, and
  returned simple closure literals lowered to nested runtime functions. Direct
  calls to function-typed parameters lower as local `RuntimeExpr::Apply`, and
  function-valued `let` aliases/partials are tracked inside that accepted body.

## Remaining Function-Stack Work

The remaining items are not "forgotten implementation tasks"; they are
documented request/design boundaries that must be answered before final
implementation:

- Spread partials and spread data-last fallback:
  `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
- AWBC suspension-aware dynamic apply and persisted closure snapshots:
  `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
- Non-helper/effectful/suspending callable allocation:
  `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
- Closure effect-row final contract:
  `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`

Runtime ID atom-table storage is deliberately deferred until profiling shows
ID comparison, hashing, serialization, or allocation pressure. The typed path
API is in place; the atom table is not a current blocker by itself.

## Separate Open Tracks

The following are real open tracks, but they must be planned and validated
separately from the function-stack goal:

- Native/Web View rendering parity, radius/shadow/filter behavior, and modern
  feedback View visuals.
- Text-control editing, selection, IME handling, and focus-loss behavior.
- Web player/EditContext glue and generated `.awfb` samples.
- Scoped presentation handle save/load and rollback follow-ups.
- Parser file/module naming cleanup.
- Pinned exact visual PNG baseline promotion and Web exact readback.

## Recommended Next Order

1. Keep function-stack commits separate from the current dirty View/Web
   worktree.
2. For the function-stack goal, either receive/author a concrete design answer
   for one of the four request boundaries, or audit code for another narrow
   typed-key/diagnostic gap that is implementation-ready without changing the
   language contract.
3. Treat the dirty View/Web/text-input files as their own validation slice:
   inspect, decide whether to continue or revert, run targeted renderer/player
   checks, then commit separately.

## Validation For This Note

```bash
git status --short --branch
jj status
git log --oneline -8
git diff --check -- docs/implementation
```
