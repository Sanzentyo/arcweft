# Current Work Status - 2026-07-08

This note is the short operational map for the current Arcweft worktree. It
does not replace the detailed implementation logs; it records what is actually
complete, what is still implementation-ready, and what should stay as request
or design work.

## Repository Baseline

- Current checked-in head before this request split:
  `97f34793 Use typed flow ids for AWBC flow lookup`.
- `main` and `origin/main` are aligned at that head.
- The dirty files at this audit point are unrelated View rendering, font,
  sample, Web IME/player, and runtime-driver text-input files:
  - `crates/arcweft-cli/tests/native_text_input_native_interactive_smoke.rs`
  - `crates/arcweft-cli/tests/native_text_input_sample_sidecars.rs`
  - `crates/arcweft-player-scene/src/fonts.rs`
  - `crates/arcweft-player-scene/src/input.rs`
  - `crates/arcweft-player-scene/tests/action_button_submit.rs`
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
  - `web/modern-feedback-view.awfb`
  - `web/player-editcontext.js`
  - `web/player.js`
  - `web/tests/ime-sample-smoke.mjs`
  - `web/tests/player-editcontext-glue-unit.mjs`
- These files are not part of this function-stack/runtime-ID cleanup. They
  look like a mixed View-rendering/Web-player/text-input slice: display-output
  encoding, emoji font registration, EditContext printable-key handling,
  runtime-driver text-input/session handling, and modern-feedback sample and
  sidecar-test changes around removing the dead name submit route. They should
  be validated and committed, reverted, or continued only as their own
  UI/Web/text-input slice.

## Active Goal

The active goal is still the function/closure/currying/pipeline language stack.
The current status index for that goal is:

- `docs/implementation/function-stack-current-status-2026-07-08.md`

The detailed running log is:

- `docs/implementation/2026-07-07-functions-closures-pipeline-language-stack.md`

## Completed Function-Stack Surface

The following is implemented and evidenced by tests or implementation logs:

- Function types `A -> B`, right associativity, tuple call-group function
  types, and curried `ParamGroup` preservation for function-like declarations.
- Curried `flow` parameter groups are rejected.
- `f(a)(b)` and `f(a, b)` are kept distinct through parser, sema, and runtime
  lowering.
- `Expr::Select` plus `Expr::Call` replaces parser-level `MethodCall` /
  `Field` variants. Runtime IR still keeps method/field semantics where those
  are the lowered executable operation.
- Closure expressions, typed/pattern parameters, block return annotations, and
  closure-local `return` checking are implemented.
- Runtime function values, captured runtime functions, partial application,
  curried application, and the first non-suspending AWBC closure/apply cut are
  implemented.
- `_` expression placeholder abstraction works in expected-function contexts
  and in the implemented inferred binary / known-callable cases.
- `^` is pipe-RHS scoped, and `|>` supports both explicit `^` substitution and
  no-`^` data-last application.
- Data-last method fallback exists after inherent/trait/env method resolution,
  with deterministic runtime argument order and ambiguity diagnostics for the
  implemented fixed-argument cases.
- Closure capture inventory, borrowed-capture suspension diagnostics, closure
  effect composition on invocation, numeric fallback lints, function-valued let
  inlays, and opt-in expression type inlays are implemented.
- Canonical primitive spellings are enforced without compatibility aliases.
- Runtime lookup IDs now use typed `RuntimeIdPath` wrappers instead of raw
  `FlowRuntimeId("flow.main")`-style string newtypes.
- AWBC flow target lookup now uses typed `FlowRuntimeId` keys in the compiler
  inventory. Static `goto`, choice targets, entries, and route targets no
  longer resolve through a public-label function map.
- `Stmt::Signal` and `Stmt::LifetimeSet` now use source-backed
  `AuthoredExpr` payloads. `LifetimeSet` parser output carries authored ranges
  through sema so lifetime write values can produce source-backed type
  judgments.
- Typed statement branch expressions now also carry authored source identity:
  `Stmt::LetElse.expr`, statement `while let` guards, and statement `match`
  arm guards/bodies are covered by focused source-range tests.
- `TypeCheckStats` reports source-backed and source-missing expression
  judgment counts, so expression inlay/source-range coverage can be audited
  from the type-check report.

## Completed Adjacent Cleanup

These were completed as supporting slices and should not be reopened unless a
new concrete defect appears:

- Call/select unification:
  `docs/implementation/2026-07-07-call-select-unification-refactor.md`
- Runtime ID boundary cleanup:
  `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`
- View resource/name cleanup:
  `docs/implementation/view-resource-rename-2026-07-08.md`

## Implementation-Ready Remaining Work

The previously identified AWBC flow-target runtime-ID cleanup has been
implemented. At this point there is no additional concrete implementation-ready
function-stack item identified in the status index without either finding
another typed-key cleanup site in code or receiving more design for the items
below.

Continue runtime-ID cleanup only for concrete lookup/index maps that still use
public strings where an owned typed key exists. Do not redesign AWBC/schema
public strings or add an atom table without profiling evidence.

## Request/Design Remaining Work

These are not implementation-ready enough to fold into the current code without
another request/design cut:

1. Spread partial application and spread data-last fallback semantics:
   `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
2. Resumable AWBC dynamic function apply and suspension resume points:
   `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
3. Serializable persisted closure snapshots:
   `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
   covers the versioned representation and restore contract. The current
   behavior deliberately rejects persisted runtime function values with
   structured save/load errors.
4. General non-helper/effectful/suspending top-level callable allocation as
   first-class runtime function values:
   `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
5. Full closure effect-row contract:
   `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`
   The current implementation composes broad effect cases, but the stable
   effect-row model remains larger design work.
6. Runtime ID atom-table storage. The typed path API is in place; interning
   should wait for profiling evidence and remains documented by
   `docs/reviews/requests/2026-07-07-seq-07.6-relative-runtime-id-boundaries.md`
   plus `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`.

## Separate Open Tracks

These are real open topics, but they are separate from the active
function-stack goal:

- Native/Web View rendering parity, CSS radius/shadow/filter behavior, modern
  feedback View visuals, text-control editing/IME behavior, and the dirty
  View/Web files listed above.
- Pinned exact visual PNG baseline promotion and Web exact readback. These
  should not be mixed into function-stack commits.
- View scroll/save/load/scoped-handle follow-ups from seq-06.16.6.x.
- Parser file/module naming cleanup. Treat this as a structural refactor slice,
  not as part of the expression source-range closure.

## Recommended Next Order

1. Keep this request split as a docs-only function-stack cut.
2. Decide whether to validate and commit the dirty View/Web slice separately or
   leave it for the rendering/IME track.
3. Audit for another concrete typed-key cleanup site, or wait for design
   output from the seq-07 request files listed above.

## Current Structural Risk

The latest structure audits still report the existing
`crates/arcweft-cli/src/app/bundle_view.rs` size error and many warnings. That
is known structural debt, not a blocker for the source-range closure, but any
new large cross-boundary implementation slice should rerun the audit and avoid
growing that module further.
