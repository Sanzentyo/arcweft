# Design readiness and dispatch order

> Historical readiness snapshot. Do not dispatch from the table below. The
> current authority is
> [the 2026-07-16 dispatch order](design-readiness-and-dispatch-2026-07-16.md).

- Date: 2026-07-15
- Readiness baseline: `11f5acecf1ad`
- Status: remaining package-driven work classified by decision completeness

## Outcome

The remaining deliveries were judged by whether they contain a final,
implementation-ready contract, not by whether they contain Rust changes. No
item in this readiness cut is safe to implement yet: each current document is a
request that asks another assignee to choose public grammar, ownership,
lifecycle, identity, diagnostic, tooling, or runtime policy. Treating those
questions as answers would make the implementation agent the language and API
designer by accident.

The requests now state their task mode, checkout evidence, dependency gate,
required decision closure, archive name, and implementation handoff. The next
step is to obtain final design packages, accept them, and only then start a
separate implementation task for each accepted contract.

## Ready to dispatch now

These five final-design tasks are independent and may run in parallel. Give one
request to one assignee; do not combine them into one archive or give one
archive to multiple workers.

| Task | Request | Reason implementation is gated |
| --- | --- | --- |
| AW-AH-007/008 | [typed RichText attribute validation](../reviews/requests/2026-07-14-aw-ah-007-008-typed-rich-text-attribute-validation.md) | Grammar, schema ownership, values, duplicate/unknown/default/recovery/codec policy are unanswered. |
| AW-AH-009.1 | [Character registration, alias, provenance, diagnostics](../reviews/requests/2026-07-14-aw-ah-009.1-character-nominal-registration-alias-diagnostics-contract.md) | Registration transaction, scope, aliases, provenance, errors, and budgets are unanswered. |
| d.2.1.1 | [View exported-part authoring](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.1-view-exported-part-authoring-production-reconciliation.md) | No source grammar exists; ownership, occurrence, re-export, rename, and provenance are unanswered. |
| d.4.1.1 | [logical-axis host seed/provider invalidation](../reviews/requests/2026-07-14-seq-06.11d.4.1.1-native-logical-axis-host-seed-provider-invalidation-contract.md) | The landed core has no final host seed API, provider lifecycle, propagation, or bounded invalidation contract. |
| d.4.2 | [environment Style conditions](../reviews/requests/2026-07-15-seq-06.11d.4.2.1-native-environment-style-condition-production-reconciliation.md) | Source grammar, text-scale type, enum operators, revision authority, invalidation, and tooling are unanswered. |

The failed RichText, exported-part, and environment implementation ZIPs are not
inputs to these design tasks. Their relevant facts are already repeated in the
standalone requests, and attaching them would reintroduce empty or misleading
delivery evidence.

## Dependent dispatch order

### Character nominal follow-ups

```text
AW-AH-009.1 final design
  -> accept its final-contract ZIP
     +-> AW-AH-009.2 definition/rename final design
     `-> AW-AH-009.3 signature-help final design or explicit non-goal
```

AW-AH-009.2 and AW-AH-009.3 may run in parallel after AW-AH-009.1 is accepted.
Attach the accepted AW-AH-009.1 archive to each and use separate workers. Land
the AW-AH-009.1 implementation before implementing either dependent contract.

### Native Style follow-ups

```text
landed d.4.1 core
  +-> d.4.1.1 final design -> implementation
  |      `-> d.4.1.2 final design -> implementation
  |             +-> d.4.1.3 final design -> implementation
  |             `-> d.4.1.4 final design -> implementation
  `-> d.4.2 final design -> implementation

d.4.1.1 implementation + d.4.1.2 implementation + d.4.2 implementation
  -> d.4.3 final design -> implementation
     -> d.5.1.1 trace reconciliation final design -> implementation
        +-> d.5.2 Agent Style observation
        `-> d.5.3 Style LSP/formatter
```

The d.4.1.1 and d.4.2 design tasks may proceed in parallel. Their implementations
are independent contracts, but they overlap resolver context, cache, and runtime
adapter files; integrate them as separate validated cuts, d.4.1.1 before d.4.2
when they share one worktree. After d.4.1.2 is implemented, d.4.1.3 and d.4.1.4
design tasks may also proceed in parallel with separate archives and workers.
d.4.3 must wait for d.4.1.1, d.4.1.2, and d.4.2 implementations because it
consumes the mounted provider revision, final measured geometry, and the
finalized zero-specificity environment activation model. d.5.1.1 must inspect
those landed types rather than invent trace-only provider, geometry,
environment, or container identities.

View exported-part and RichText work remain independent of this Style chain.

## How to dispatch a final-design task

Create a new task with access to the latest `main`, attach only the linked
request, and send this instruction:

> Treat the attached request as the sole requirements brief. Do not modify
> production code. Inspect the current repository, close every required
> decision with no TBD, surviving alternative, or implementation-selected
> policy, and return the exact final-contract archive and external status/hash
> artifacts required by the request. If the checkout or a named predecessor is
> unavailable, return a blocked report and do not fabricate a final ZIP.

When a request names an accepted predecessor archive, attach that archive too.
Do not attach unrelated historical ZIPs. One request/archive gets at most one
worker.

## How to dispatch the later implementation task

After checking that the final contract satisfies its completion gate, create a
new implementation task, attach the accepted final-contract ZIP, and send:

> Treat this final-contract ZIP as the source of truth and implement it end to
> end on the latest `main`. Preserve the request's fixed substrate and crate
> direction. Do not add CSS/Takumi paths, compatibility shims, dual readers,
> deprecated aliases, or source gates. Use focused tests during implementation,
> then the required workspace validation and structural audit at the reviewable
> cut. If the contract still leaves a result-changing choice open, stop and
> report that exact gap instead of guessing.

Do not ask one task to design and implement a still-open contract. The design
archive is an explicit acceptance boundary between those two stages.

## Intake checks for returned final contracts

Before implementation, reject a delivery if any of the following is true:

- a required decision is absent, marked TBD, or delegated to implementation;
- the repository revision or current owning API was not inspected;
- prose, Rust type sketches, codec/wire shapes, diagnostics, examples, and tests
  select different models;
- a predecessor contract is restated or redesigned instead of consumed;
- a speculative patch or repository snapshot is presented as design evidence;
- required traceability, final status, manifest hashes, or negative/limit tests
  are missing; or
- the package claims validation that its contents cannot reproduce.

An accepted design is then implemented as its own coherent, validated, pushed
cut before the next dependent request is dispatched.
