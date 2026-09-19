# Implementation evidence instructions

Keep operational policies distinct from dated implementation state; neither
silently replaces a stable language/runtime contract.

A new state note records its date, inspected full Git SHA, and observed local
working-tree state. For connector-only work, say local state is unobserved and
record the remote base instead. Identify an older replaced state note with
`Supersedes`; retain the original as historical evidence.

Record performed work, passed/failed/blocked/not-run validation, relevant exact
test counts, non-goals, design deviations, and unresolved decisions. Plans,
nearby implementations, private substrate, and unvalidated edits are not
completion evidence. Link any blocker request that exists; do not invent a
request path or redesign accepted substrate without evidence.

For a handoff or compaction checkpoint, keep the current goal, accepted contract
references, base SHA and changed paths, completed evidence, remaining acceptance
criteria, and next action in the current task note. Update that note rather than
creating a new status hierarchy for each small edit. Reconcile it against live
Git state on resume; a summary is a locator, not authority.

Use [test-execution-policy.md](test-execution-policy.md) for validation and
[structural-audit-policy.md](structural-audit-policy.md) for triggered ownership
reviews and generated-report placement. Record the named owner, cohesive
responsibility, reviewed state/dependency/API/test boundaries, and decomposition
or cohesion disposition; LOC reduction alone is not a design result.

Keep progress and package ledgers here, not in instructions or stable chapters.
When maintaining prompts or skills, use [agent-workflow.md](agent-workflow.md).
