# Function Stack Request Split Audit - 2026-07-08

## Purpose

This note records the request/design split performed after the AWBC typed
flow-ID lookup cleanup. It is part of the active function/closure/currying/
pipeline goal's evidence trail.

The requirement-by-requirement goal audit is recorded separately in
`docs/implementation/function-stack-goal-completion-audit-2026-07-08.md`.

## Current Implementation-Ready State

The previously identified implementation-ready runtime-ID cleanup is complete:
AWBC flow targets now resolve through typed `FlowRuntimeId` keys in the
compiler inventory, and the old general public-label function lookup was
removed.

No additional concrete implementation-ready function-stack item was identified
from the status index without either:

- finding another lookup/index map in code that uses public labels where an
  owned typed key exists; or
- receiving a design contract for one of the remaining broad items.

Follow-up: the later requirement-by-requirement completion audit identified
one evidence-level implementation item rather than a new design request:
expected-type enum shorthand for user-defined tuple and record payload
constructors needed focused sema/runtime-plan coverage. That gap is closed in
the enum-shorthand evidence cut and does not change the request/design split
for the broader items below.

## Request Coverage

Remaining broad items now map to request/design files:

- Spread partial application and spread data-last fallback:
  `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
- Resumable AWBC dynamic apply and persisted closure snapshots:
  `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`
- Non-helper/effectful/suspending callable allocation:
  `docs/reviews/requests/2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md`
- Full closure effect-row finalization:
  `docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`
- Runtime ID atom-table storage:
  `docs/reviews/requests/2026-07-07-seq-07.6-relative-runtime-id-boundaries.md`
  and `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`

## Non-Goals

- This audit does not implement spread partial execution.
- This audit does not implement resumable AWBC dynamic apply.
- This audit does not introduce serializable closure snapshots.
- This audit does not redesign existing helper-backed/local closure behavior.
- This audit does not touch the unrelated dirty View/Web/text-input worktree
  files.

## Validation

This is a documentation/request split. Validation is limited to:

```bash
git diff --check -- docs/implementation docs/reviews/requests
```
