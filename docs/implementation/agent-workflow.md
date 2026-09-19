# Agent workflow and task prompts

Maintained guidance for authoring repository instructions, task prompts, and
handoffs. This is an on-demand reference, not an additional startup checklist.
Root/scoped `AGENTS.md` and the linked operational policies own their rules;
this document does not replace domain contracts or expand permissions.

## Choose the smallest relevant context, not the smallest implementation

Begin with the requested outcome, applicable instructions, and the current
owner/contract. Expand the inspected surface when a dependency, consumer,
acceptance criterion, or conflict requires it. Completing one typed authority
can require a broad migration; selective reading is not permission to omit
mutually constraining requirements.

For package work, read the active request and applicable contract completely;
follow inherited requirements and precedence until the acceptance scope is
closed. Use [review intake](../reviews/README.md) for archive identity and
readiness rather than reprocessing the historical archive tree on every task.

For a continuation, compare the task note's base with current Git state. Reopen
changed instructions/contracts and relevant implementation/evidence, not the
whole document stack by default. Never reinstate an obsolete model merely
because a compressed conversation summary described it.

## Make completion and permission distinct

A useful task states its outcome, authoritative input, non-goals, observable
acceptance, and delivery boundary. Routine investigation, implementation choices,
local validation, and change-caused repairs stay within that scope. Finishing a
first patch is not completion; passing an unrelated test is not acceptance.

An analysis-only request ends with findings; a design-only request ends with its
complete design/artifact, not a production patch. User-data loss, unresolved
external authority, conflicting accepted contracts, new external side effects,
and forbidden Git operations remain real boundaries. Isolate a blocked decision
and finish independent work; do not mask a blocker with a speculative fallback.

Select checks through [test-execution-policy.md](test-execution-policy.md).
Evidence can be reused only with the relevant inputs unchanged. Report passed,
failed, blocked, and not-run checks separately; do not manufacture a green
milestone from an unavailable environment.

## Reusable prompt shapes

Use only the relevant shape; replace bracketed fields with task facts. These
are examples, not mandatory forms or extra steps for a small edit.

### Implementation

```text
Implement [outcome] against [maintained contract or accepted request].
Done means [observable behavior and affected consumer migration], selected
validation, and fixes for failures caused by this change. Non-goals: [scope].
Continue through that result, not just the first patch. Follow repository Git
and permission rules; report exact evidence and unresolved external blockers.
```

### Continue accepted work

```text
Continue [goal] from [current task note]. Reconcile its base and remaining
acceptance criteria with live Git state. Preserve accepted, validated substrate
unless a concrete defect requires correction. Finish [remaining outcome];
reuse unchanged evidence and stop at that outcome, not at a compaction boundary.
```

### Design only

```text
Design [boundary] for [active request and inherited acceptance criteria].
Deliver [artifact] with all mutually constraining decisions closed, current
consumer traceability, and validation requirements. Do not edit production.
Identify any genuinely external unresolved authority precisely; do not split
one semantic authority merely to return an easier partial package.
```

## Skill and instruction maintenance

Keep `AGENTS.md` for durable repository constraints and short task-specific
routing. Keep dated progress in implementation notes, command matrices in their
existing policy, and archive details in the intake guide. Do not duplicate an
existing authority in a new skill merely to make it discoverable.

Add a skill only for a recurring workflow with repository-specific value.
Its description should name the operation and its narrow trigger, not everything
related to a language or subsystem. For example: "Verify a returned Arcweft
contract ZIP for intake or readiness adjudication" is narrower than "Use for
all Arcweft design, Rust, and documentation work."

A skill root routes to only the references/scripts needed for the selected
workflow. Keep integrity requirements and completion criteria, but omit generic
coding tutorials, repeated exhortations, and prescribed step-by-step itineraries
that do not protect an actual boundary. Repository instructions are shared by
multiple models: do not hard-code a model identity or assume a particular model
makes approval or verification unnecessary.

Check a proposed instruction change with representative tasks: a documentation
typo, focused Rust fix, cross-crate contract change, ZIP intake, resumed work,
and design-only assignment. Review whether each selects relevant context,
preserves all applicable acceptance gates, and stops only at the correct
boundary. These are review scenarios, not a keyword-based CI gate or proof of
agent performance. Add a rule for an observed failure, not speculative coverage.

Background: OpenAI's
[Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)
(2026-09-11). The repository-specific boundaries above remain authoritative.
