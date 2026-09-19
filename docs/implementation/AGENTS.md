# Implementation evidence instructions

Keep operational policy, dated implementation evidence, and stable design
contracts distinct. Root autonomy and completion rules apply here.

Keep a durable note when acceptance, a nontrivial design decision, package intake,
a genuine blocker, or later continuation needs it. A small completed edit does
not require a new ledger or status document. Reuse the current task note; do not
create a hierarchy for each edit, checkpoint, or commit.

A state note gives its date, inspected full Git SHA, and observed working-tree
state. For connector-only work, record the remote base and say local state is
unobserved. Mark replaced current decisions with `Supersedes` while retaining the
old note as historical evidence.

Record the result and actual passed/failed/blocked/not-run evidence, relevant
counts, unresolved acceptance, and material deviations. Planned work, nearby
implementations, and unvalidated substrate are not completion evidence. Include
a blocker request only when one is genuinely needed; documenting a resolvable
question does not discharge the implementation or design assignment.

On continuation, reconcile the existing goal, contract references, base/changed
paths, evidence, and next action with live Git. Resume remaining work without a
new approval round or a blanket re-audit. Reuse valid evidence and reopen only
what changed or is needed to resolve uncertainty.

Use [test policy](test-execution-policy.md) and
[structural policy](structural-audit-policy.md) for their affected boundaries.
Keep any required ownership disposition concise and tied to the changed owner;
LOC reduction is not a design result. Instruction-maintenance details belong in
[agent workflow](agent-workflow.md), not every task's startup context.
