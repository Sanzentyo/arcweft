# Implementation evidence instructions

This directory records current and historical implementation evidence, not the
stable language or runtime design.

- Give every new state note a date, the inspected full Git commit SHA, and the
  relevant dirty/clean working-tree state. Do not record a Jujutsu change ID.
- Separate `performed`, `passed`, `failed`, `blocked`, and `not run`. Include
  exact focused-test counts when they materially support acceptance.
- When a new status note replaces an older one, identify it with `Supersedes`.
  Preserve the older note as historical evidence; do not rewrite its past-tense
  Git/Jujutsu details merely to match the current Git-only workflow.
- Record explicit non-goals, underdesigned boundaries, external blockers, and
  the repository request that can close each blocker.
- Do not award completion credit for a nearby implementation, private substrate,
  or unvalidated working-copy change.
- Record design deviations and precedence decisions. If a durable design rule
  changes, update the maintained stable chapter as a separate documented part
  of the same coherent cut.
- Use `test-execution-policy.md` to select validation and
  `structural-audit-policy.md` for structure evidence. Planned validation is
  never a substitute for command output.
- Keep generated structural reports in a task-named directory under
  `structure-audits/`; mark generated data and do not treat it as production
  source.
- Keep temporary progress, package ledgers, and current goal ordering here, not
  in root or scoped `AGENTS.md` files.
