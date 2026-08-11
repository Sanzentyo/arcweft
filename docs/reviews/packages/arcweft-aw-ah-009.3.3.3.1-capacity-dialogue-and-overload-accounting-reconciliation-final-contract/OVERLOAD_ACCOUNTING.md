# Overload accounting

## 1. Problem statement

The shared resolver intentionally evaluates candidate-mapped arguments under
candidate-specific expected types in rollback transactions. A later selected or
recovery replay may physically evaluate them again. Therefore a retained fact
count cannot prove one physical traversal, and a physical traversal count
cannot describe the semantic state retained after rollback.

This contract defines two non-interchangeable evidence products without
changing candidate ordering, selection, rollback, replay, work charging, or
checker-owned call-target facts.

## 2. Retained semantic evidence

The normative name is:

```text
retained_argument_inference_facts
```

This is not a new parallel inference store. It is the multiset projection of
existing `CheckedCallArgumentSlotFact` values in the final published
`CheckedCallTarget` for one call expression.

Each element is identified by the existing typed coordinates retained by the
checked facts, including the call, authored argument index, logical slot index,
expression identity, source identity/range, mapped parameter coordinate when
present, inferred type, expected type when present, and poison state.

### Dispositions

- **Committed** — facts produced by the selected-candidate replay and published
  by `CheckedCallTarget::selected`.
- **Ambiguous recovery projection** — facts from the deterministic primary tied
  probe already selected by the accepted stable resolver order and published by
  `CheckedCallTarget::ambiguous`; no candidate is semantically selected.
- **Multi-rejection recovery projection** — facts from the stable primary probe
  used by the existing rejected-target path.
- **Singleton rejected replay** — facts from the explicit rejected-recovery
  replay used to retain precise diagnostics.
- **Terminal failure** — no retained candidate inference facts are published.

The measure is computed after final publication. It is not incremented during a
probe and is not reconstructed from `TypeCheckStats`.

## 3. Physical operational evidence

A separate counter is required. The exact evidence name is:

```text
physical_candidate_argument_evaluations
```

The exact crate-owned shape is:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateEvaluationPass {
    Probe,
    SelectedReplay,
    RejectedRecoveryReplay,
    DirectCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CandidateExpectedType {
    Exact(TypeKind),
    Unchecked,
    Unmapped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalArgumentEvaluationKind {
    Authored,
    Recovered,
    FixedLiteralSpread,
    TypedRestSpread,
    Unmapped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PhysicalCandidateArgumentEvaluation {
    pub(crate) call_expression: TypeExpressionId,
    pub(crate) candidate: CallableCandidateId,
    pub(crate) pass: CandidateEvaluationPass,
    pub(crate) argument: CallableArgumentIndex,
    pub(crate) slot: CallableArgumentSlotIndex,
    pub(crate) kind: PhysicalArgumentEvaluationKind,
    pub(crate) expected: CandidateExpectedType,
}
```

`DirectCommitted` is reserved for an already-singular registered-candidate path
that does not enter the ordinary probe/select/replay loop. It cannot be used to
collapse an overload probe or selected replay.

The recorder is owned by `arcweft-lang-sema`'s checker as operational evidence.
It is crate-owned/read-only and does not become a language or LSP semantic API.
The public semantic product remains the existing checked call-target facts.

## 4. Exact observation points

A physical event is emitted only after cancellation/deadline/work admission for
that evaluation succeeds and immediately before the checker actually evaluates
the expression or logical spread slot.

### Ordinary, recovered, and fixed-literal slots

In `check_registered_argument_slot_with_inferred`, emit after the candidate's
substituted expected type has been computed and immediately before
`check_fixed_literal_spread_slot`. A fixed-literal spread emits one event per
logical slot, sharing the authored `CallableArgumentIndex` and using distinct
`CallableArgumentSlotIndex` values.

### Typed-rest spread

In `check_registered_typed_rest_spread`, emit immediately before the container
`check_expr(value)` call. Its expected type is `Unchecked`; the later rest-item
compatibility comparison is not a second expression traversal.

### Unmapped arguments

In `check_unmapped_registered_arguments`, emit immediately before
`check_expr(argument.value())` with `CandidateExpectedType::Unmapped`.

### Candidate/pass context

The checker pushes a bounded candidate evaluation context around the existing
`evaluate_registered_candidate` call:

- the probe loop supplies `Probe`;
- unique selected replay supplies `SelectedReplay`;
- singleton rejected diagnostic replay supplies `RejectedRecoveryReplay`;
- a direct committed path supplies `DirectCommitted`.

Nested calls push their own context. Candidate identity and call expression ID
make interleaving unambiguous. No source string or display label participates.

## 5. Rollback ownership

`RegisteredCandidateCheckpoint` continues to snapshot and restore semantic
state, including `TypeCheckStats`, judgments, typed lowering evidence, captures,
numeric fallback, effects, diagnostics, scopes, borrow/lifetime state, curried
facts, project references, Speaker observations, and call-target facts.

The physical recorder is deliberately not part of that semantic checkpoint.
Completed physical events survive candidate rollback because they describe work
that actually occurred. This exception is operational evidence only; it cannot
be read by semantic selection, lowering, cache identity, or signature ranking.

Retained inference remains entirely inside the committed/recovery
`CheckedCallTarget` projection. No unselected candidate facts survive there.

## 6. Resolver and work-meter relation

The following are separate quantities:

- one shared `resolve_call_target` invocation;
- candidate materialization operations;
- candidate probe operations;
- `SignatureQueryStep::CandidateArgumentProbe` admission steps;
- `SignatureWorkKind::SpecificityChecks` and `ArgumentBindings` charges;
- physical candidate argument evaluations;
- retained argument inference facts.

The existing work contract remains authoritative. In particular, selected or
rejected replay may physically re-evaluate expressions while intentionally not
charging speculative candidate work a second time. No test may assert equality
between work charges and physical events unless the exact fixture proves that
shape independently.

The physical recorder itself consumes no callable/signature work budget beyond
the bounded append needed to report an event. Its maximum length is derived
from existing production candidate, argument, fixed-spread-slot, nesting, and
query limits; this contract adds no client-configurable limit and no unbounded
trace.

## 7. Exact transaction formulas

Let `slots(pass, candidate)` be the number of logical expression/slot checks that
actually reach the observation point.

### Unique viable winner

```text
physical = sum(slots(Probe, each candidate))
         + slots(SelectedReplay, winner)
retained = slots represented by the selected replay's final CheckedCallTarget)
```

For two one-argument candidates: physical `3`, retained `1`.

### Ambiguous tie

```text
physical = sum(slots(Probe, each candidate))
retained = deterministic primary tied probe projection
```

There is no selected replay and no selected semantic candidate.

### Multiple rejected candidates

```text
physical = sum(slots(Probe, each candidate))
retained = stable primary rejected probe projection
```

### Singleton rejected candidate

```text
physical = slots(Probe, candidate)
         + slots(RejectedRecoveryReplay, candidate)
retained = rejected recovery replay projection
```

### Terminal cancellation/deadline/work failure

```text
physical = completed prefix before the terminal admission failure
retained = 0 candidate inference facts
```

The typed terminal error survives rollback through the existing terminal-error
channel.

### Fixed-literal spread

If a spread expands to `k` logical slots and two candidates are probed with one
winner replayed:

```text
physical = 3k
retained = k
```

The authored argument count remains one; slot count is `k`.

## 8. Contextual semantics that must be preserved

The physical event's expected-type field proves that each candidate can receive
its own context. No context-free result cache is permitted. The following must
remain candidate-contextual and transactionally isolated:

- enum shorthand such as `.Variant`;
- unsuffixed numeric fallback and typed lowering evidence;
- closures, captures, effects, and partial placeholders;
- generic substitutions;
- nested calls whose result is checked against a candidate-specific expectation;
- fixed-literal spread slots;
- borrow/lifetime state, diagnostics, and call-target facts.

Only the final committed or deterministic recovery projection appears in
`retained_argument_inference_facts`.

## 9. Clean recovery

`CleanRecovery` means the family schema intentionally accepts the shape and the
ordinary checker evaluates every present or recovered expression. It does not
mean zero diagnostics globally. Parser recovery or a nested expression may have
its own typed diagnostic, while the family itself remains clean and does not
manufacture an arity, name, or spread rejection.

This distinction is required for CapacityMethod, Drop, Promotion, and the
transitional Speaker observation.

## 10. Evidence API constraints

- Do not add a second call-target fact store.
- Do not expose the physical trace as stable language semantics.
- Do not cache candidate-independent expression results.
- Do not erase candidate-specific expected types.
- Do not use source text, symbol spellings, or implementation-path scans.
- Do not alter candidate rank or replay behavior to make counts smaller.
- Do not count resolver/materialization operations as expression evaluations.
- Do not retain rolled-back semantic facts merely to explain physical work.
