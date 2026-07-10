# Current work status — 2026-07-10

This is the current repository map for the cleanup, dialogue paging, revised
function-stack, and seq-06.16.6.2 work. It supersedes the operational pointers
in `current-work-status-2026-07-09.md`; dated implementation notes remain as
evidence rather than being rewritten into one changelog.

## Completed and pushed baseline

- Source-spelling gates were removed and `AGENTS.md` now prohibits adding a
  test, CI step, or audit that passes by searching repository implementation
  text. Behavioral, codec, compile, lint, dependency, and generated-artifact
  evidence replace those gates.
- The `justfile` is organized by workflow responsibility and exposes the
  documented fast/workspace/doc/Tier-2 validation routes without duplicate
  pass-through recipes.
- `DataFormat`, dialogue identity, runtime collection conversions, compact
  resource codecs, and unpublished session-save identity were moved to their
  owning typed boundaries. The remaining independent candidates are ranked in
  `independent-cleanup-inventory-2026-07-10.md`.
- Dialogue `[p]` and `[l]` now have stateful reveal/page/line advancement.
  Native and Web loops retain a page cursor within one display frame instead
  of treating a planner split as a complete paging implementation.
- Function types, curried declaration/call groups, closures, `_`, `^`, pipes,
  data-last method fallback, numeric inference/LSP policy, canonical primitive
  spelling, typed relative IDs, and enum shorthand are implemented across the
  language stack. AWBC dynamic apply is suspension-aware and AWBC-backed
  function snapshots are serialized and validated.
- Scroll policy rendering, indicator fade, elastic overscroll, nested scroll
  chaining, focus auto-scroll, keyboard/precision/gamepad input, and Agent
  region-addressed scroll actions are implemented. The previous error-level
  `bundle_view.rs` and player `input.rs` size findings were removed by
  responsibility splits.

## Final implementation cuts in this checkout

- Ordinary source `fn` values and analyzable closures use open inferred effect
  rows. Function-body and higher-order callback effects occur only when the
  applicable final call group is reached; aliases, partials, returned closures,
  data-last fallback, closed report projection, LSP hover, and effect traces
  preserve that timing. Inferred-let inlays resolve those open rows through the
  owning sema report, so internal effect variables do not leak into source type
  labels.
- Source, stream, ordinary flow return, and host-request executable lowering
  is fail-closed. Unsupported or recovered syntax produces structured owner,
  statement path, role, and authored-range diagnostics. Source/stream `Noop`
  variants and source-policy defaults are removed. A shared single-pass source
  header inventory rejects duplicate singular headers at the second authored
  range; sema and runtime-plan both reject recovered policies, invalid bounded
  capacities, missing/unknown overflow, and private/full replay.
- Runtime-plan flow optimization/count/local-use analysis is separated from
  flow lowering, removing the error-level `flow.rs` size hotspot without
  introducing a wrapper facade or compatibility module.
- `arcweft-view` owns an exact Sans I/O finite-list range planner with monotonic
  per-mount IDs, stable keys, half-open windows, paged/full range records,
  key-relative source replacement, and atomic tamper-rejecting snapshots.
  Runtime-driver save/load and compatible hot swap validate that state; Agent
  observation/capture reports include materialized and off-window ranges and
  link them to the authored Scroll action target.

## Larger or genuinely undecided follow-ups

The following are explicit future contracts, not hidden completion claims:

- `docs/reviews/requests/2026-07-08-seq-07.8.1-task-dialogue-stream-callable-effect-abi.md`
  decides creation/start/resume/yield/cancellation effect timing for callable
  kinds that cannot be treated as ordinary `fn`.
- `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
  covers escaped host/adapter thunks and bound method values outside the
  revised language surface.
- `docs/reviews/requests/2026-07-10-seq-06.16.6.2.1-view-runtime-evaluator-and-lazy-source.md`
  defines typed View evaluation, multiple program/mount action identities,
  deterministic off-window extent measurement, child-state ownership, and
  off-window focus/capture materialization before enabling `LazyRow` /
  `LazyColumn` source syntax. The source remains rejected rather than being an
  eager Row/Column alias.

No compatibility promise is attached to these unreleased provisional
boundaries. Once their contracts are decided, the old representation should be
replaced directly.

## Validation

Focused command evidence is recorded in:

- `function-stack-effect-row-curried-higher-order-timing-2026-07-09.md`
- `function-stack-checked-executable-lowering-2026-07-10.md`
- `seq-06.16.6.2-scroll-axis-virtualization-retained-content-2026-07-09.md`

The final checkout is also formatted, run through the normal workspace
test/check/clippy routes, checked with `-D warnings` for the changed owners,
checked for diff whitespace, and measured by the canonical structural audit.
The audit report is stored at
`docs/implementation/structure-audits/function-stack-checked-executable-lowering-2026-07-10/`.
It scanned 2,520 files, including 1,175 Rust files / 597,937 physical LOC, and
reported 0 errors / 149 warnings.
