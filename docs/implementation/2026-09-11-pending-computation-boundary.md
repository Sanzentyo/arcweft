# Pending computation boundary — 2026-09-11

Inspected `main` and `origin/main` at
`b25f2590c8436a15647463eb5ccc974352f4a5e0`, with an empty index, 73 dirty/untracked
status entries and 76 Rust input paths. The coupled callable migration is still
uncommitted. This note records a required design reconciliation, not a completed
implementation or an external blocker. It supplements the
[component design](2026-09-11-callable-component-design.md) and is assigned to the
existing [coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
It does not create an independent implementation-ready subcut.

## Current evidence

The working copy now completes every admitted application's solutions and
projections before source materialization. Completed components retain one
source trace; a materialization request borrows that trace and identifies its
owning application. The driver also retains the initialization's verified graph
issuer and checks it against the analyzer's graph during source callbacks.
These changes preserve the accepted borrowed driver, and do not implement
child admission or pending source computations by themselves.

The remaining boundary is concrete:

- [Application scopes](../../crates/arcweft-lang-sema/src/types/constraints/application.rs)
  currently authenticate inference references through a callable application's
  formal parameter opening. There is no admitted existential result-port schema
  for a computation that has no declaration parameter. Inventing a declaration
  owner/ordinal, or growing a sibling's scope implicitly, would break this rule.
- [Source probing](../../crates/arcweft-lang-sema/src/types/constraints/transaction.rs)
  requires an observation or rejection before advancing the active source.
  An observation already carries an actual `TypeKind`. There is no suspended
  computation result, dependency activation or consuming resume operation.
- [Expression preparation](../../crates/arcweft-lang-sema/src/final_analysis/prepared.rs)
  still stores `CheckedExpression` in `Complete`. The checked expression owns
  an `EffectSet`, while [call publication](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs)
  asks for `constant_effects` even when the declaration body is pending.
- [Source callable effects](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/semantics.rs)
  return an unknown row for a pending body with no known exposed row.
  [Body effect closure](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/callable_effect_graph.rs)
  currently folds concrete sets through target identities. It has no
  per-invocation substitution for symbolic body equations.

These are connected limitations. Merely admitting a child parameter scope
would still require the child to finish before the parent can consume it.
Merely adding a pending effect flag would still leave type-directed member,
constructor and overload choices without their required input. Neither is the
selected final model.

## Decisions that must close together

1. Specify computation identities, typed result ports, declaration-parameter
   and existential-result scope ownership, branch admission and foreign-port
   rejection. Slots must not become invented declaration generics or escaping
   universal quantifiers. State how every admitted result reference is erased
   or legitimately quantified before a checked value can be published.
2. Specify the source lifecycle and evaluator continuation. Show how an exact
   physical source can wait for a type premise or body result, retain its
   prepared fact delta, and resume without repeating the whole argument or
   manufacturing an accepted observation. Identify which existing HIR topology
   and fact owner supplies each dependency; do not copy the expression tree.
3. Specify how the retained graph authority combines with the exact parent
   source ticket, child site, prepared inputs and selected schema. Graph identity
   alone is not child authorization. Admission and result contribution must use
   the same live context and accumulated accounting as the containing path.
4. Close the interaction between declaration-body recursion, application
   substitution, effect predicates and overload alternatives. The current
   post-body concrete-set fold cannot supply this contract. Show the exact
   ownership and scheduling of a cycle in which a body row is needed by a
   candidate while the body's checked operations depend on candidate selection.
   A guessed empty row, source-order-dependent preliminary result or a second
   callable resolver is not an admissible cycle breaker.
5. Give the complete error, cancellation, work-limit and publication behavior.
   A pending result is neither a mismatch nor a checked value. Retained work and
   discarded alternatives must remain charged; a failed or unresolved required
   dependency must expose no partial facts, source receipt or executable instance.

The accepted finite effect algebra and component ownership remain foundations.
This reconciliation must supply exact Rust state transitions and their producer/
consumer migration, rather than restart that algebra or introduce a second
inference-variable authority. It must also show where per-candidate limits and
program/body dependency work are charged, including recursive components.

## Required evidence and current status

Retain the full acceptance scope of the coupled request. In particular, exercise
the existing [correlated calls](../../crates/arcweft-lang-sema/src/final_analysis/tests/generic_calls.rs)
and [higher-order effects](../../crates/arcweft-lang-sema/src/final_analysis/tests/higher_order_effects.rs),
then cover pending member/constructor premises, aliases and aggregates, implicit
and explicit callback rows, mutually recursive bodies, incompatible and
equivalent overload alternatives, reversed declaration/source order, and exact
limit/cancellation boundaries. A lower port test is not end-to-end evidence.

The preceding working-copy run was 757 semantic tests passed and 79 failed;
its failure inventory was unchanged by the graph-authority handoff. Workspace
all-target/all-feature checking, sema Clippy and the structural gate passed with
warnings/review triggers. No Rust or acceptance test is changed by this note.
Those results establish the present gap, not acceptance of the whole migration.

The [full convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
Callable/source/effect convergence retains priority. The already-authorized
independent nominal C1-C6 work can proceed in the existing checkout while this
connected boundary is resolved; none of these obligations is waived or marked
externally blocked.
