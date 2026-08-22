# Lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1 design

Date: 2026-08-22
Repository revision: `f43ca943d84f9a6a6da17605947a3d30c518a5a8`
Inspected branch: `main`
Working tree at design start: dirty only with untracked review intake, package,
and request files; no production file was changed by this design.

## Result

This is the accepted **blocked correction direction**, not an
implementation-ready contract. It replaces the returned `.1.3` package's
claim that all semantic row owners and the Cut 3 View product already exist.

The only lawful dispatch and intake order is:

```text
.1.2 design return and intake
  -> .1.2 implementation and validation
  -> .1.4 dispatch
  -> .1.4 design return and intake
  -> .1.4 implementation and validation
  -> .1.3.1 finalization against those exact accepted APIs
  -> task-plan semantic integration
  -> Cut 5 atomic publication
```

`.1.4` must not be dispatched as if `.1.2` were already accepted. This design
does not guess either predecessor's output.

## What is closed now

- `RuntimePlanBuilder` remains the sole mutable core plan owner.
- Task-plan construction uses a seed accepted by the builder; callers never
  construct a final `RuntimeTaskPlan` field literal.
- Task-plan build coordinates reuse the builder's current `Arc` issuer
  identity and a source-order ordinal. There is no global numeric token.
- Runtime-plan lowering returns a private/unpublished draft, not a partial
  `RuntimePlan`, when View sealing is still required.
- The compiler, which already depends on runtime-plan, bundle, and View, owns
  orchestration. Neither bundle nor core depends on compiler.
- Core owns and writes the complete seven-role prefix. The View authority can
  finish only through a one-use core request whose single completion operation
  appends the closed View payload in the only legal order. No hasher or sink is
  exposed.
- The existing live `ViewTaskPlanAuthority::validate_view_task_plan` behavior
  is preserved; semantic sealing is an additional method on the same trusted
  protocol.
- Candidate task references are builder coordinates; final executable rows
  contain a sealed table index. A public `RuntimePlan` never retains a
  construction issuer.
- Expected decoded bytes are assertions checked after recomputation, and one
  global duplicate check precedes publication.

## What remains blocked

The exact `.1.2` accepted declaration/body paths and field/case identities,
and the exact `.1.4` retained View site/admission/product APIs, are not yet
available. Those outputs change transcript bytes, bridge types, validation
joins, and failure variants. They cannot be represented by placeholders while
claiming readiness.

See:

- `FINAL_DESIGN.md` for the selected correction and predecessor gates;
- `SCHEMAS.md` for the frozen core-independent Rust boundary;
- `TRANSCRIPTS.md` for the retained bytes and the atoms that remain gated;
- `DEPENDENCIES_AND_STATE_MACHINE.md` for legal crate flow and publication;
- `CUTS_TESTS_AND_DELETION.md` for implementation order and tests;
- `OPEN_QUESTIONS.md` for questions that only accepted predecessor returns can
  close; and
- `FINAL_STATUS.md` for the current status.

## Superseded claim

The returned archive
`arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip`
is retained as review evidence. Its archive structure validates, but its
`READY_FOR_IMPLEMENTATION` and `OPEN_QUESTIONS=none` claims are not accepted.
