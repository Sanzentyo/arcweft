# Const phase fence and Need timeout contract adoption — 2026-08-18

Inspected revision: `45703aa30980305f9df0bb4a5e74238c9dec2fe4`

Working tree at inspection: clean.

## Performed

- Adopted `const { ... }` as a compile-time phase fence with no `Const<T>`
  wrapper, runtime captures, runtime effects, or product-resident evaluator.
- Specified closed type/value admission, inferred exact const-callables,
  deterministic fuel/value budgets, restricted AWBC evaluation, typed constant
  interning, cache identity, and phase-aware diagnostics.
- Adopted `timeout(Need<T>, Duration) -> Need<Result<T, Timeout>>` as an ordinary
  resolved standard combinator rather than an Await syntax/branch.
- Specified logical-time start and countdown, deterministic same-step race
  ordering, wait-local cancellation, source Progress forwarding, typed
  RuntimePlan/AWBC ownership, and save/replay/hot-reload behavior.
- Updated maintained language/runtime indexes, grammar, prelude, Await/Need,
  block-scope, scheduler, and executable AWBC chapters.

## Precedence corrections

The referenced design conversations were treated as input, not repository
authority. Two proposed version changes conflict with the repository-wide
invariant that every Arcweft-owned version marker remains `1`. The maintained
contract therefore evolves ConstEval, typed constants, NeedTimeout, AWBC,
codec, cache, and snapshot schemas in place at version `1`; it introduces no
v2 reader, writer, or compatibility path.

The timeout design also does not extend the current binary-Need/Await-specific
implementation. Timeout implementation begins only after Checked Try/carrier
boundaries, unary Need, and generic Await are authoritative.

## Current implementation evidence

- Parser/HIR currently own `ComputationBlock(Result | Option | Seq | Stream)`;
  `Const` is not implemented.
- Result/Option blocks currently wrap their tails but do not yet intercept a
  checked Try residual.
- Sema/runtime still retain binary Need, physical Await Result, Error/Denied
  handler logic, and Await-specific Try lowering.
- No standard checked `std.need.timeout` callable, Runtime Need producer,
  NeedTimeout plan operation, AWBC instruction, or timeout snapshot is present.
- Existing AWBC constants and `LoadConst` provide substrate, but the typed
  constant interner and ConstEval profile described by the stable contract are
  not implemented.

## Passed

- `git diff --cached --check`.
- Relative Markdown link targets in every changed chapter, excluding fenced
  Arcweft examples.
- Balanced Markdown code fences in every changed chapter.

## Not run

- Cargo checks, tests, Clippy, or structural audit; this is a documentation-only
  adoption cut.
- Documentation rendering.

## Required implementation order

1. Complete the Checked Try and Result/Option propagation-boundary cut.
2. Replace binary Need with unary `Need<T>` across all layers and delete Await
   physical-Result/Error/Denied special paths.
3. Establish canonical runtime Need identity and producer ownership.
4. Implement Const syntax/HIR, checked phase facts, typed constant interning,
   ConstEval verification/VM, cache, and diagnostics atomically.
5. Implement Timeout type/intrinsic/producer, logical reducer, RuntimePlan,
   AWBC, persistence, and parity tests atomically after the temporal cut.

## Explicit non-goals

- No production Rust changes in this documentation cut.
- No AwaitTimeout/TryAwaitTimeout syntax or opcode.
- No producer cancellation authority granted to generic timeout.
- No HIR interpreter or arbitrary native callback for const evaluation.
- No version bump, legacy reader, dual Need model, or compatibility alias.
