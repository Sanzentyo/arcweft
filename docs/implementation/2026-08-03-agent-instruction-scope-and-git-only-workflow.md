# Agent instruction scope and Git-only workflow

Date: 2026-08-03

Status: `COMPLETE`

Inspected base Git commit:
`70e24164373e7898ff9ef83f56f4c48523ce108e`

Working-tree state at start: dirty protected Proof integration WIP, 505 paths.
This policy cut uses only explicitly staged documentation paths and does not
reset, stash, or include that WIP.

## Decision

The 481-line root `AGENTS.md` mixed repository invariants, Rust details,
structure thresholds, test commands, documentation authority, package intake,
and an obsolete initial implementation order. It is replaced by a short root
router plus scoped instructions:

```text
AGENTS.md
crates/AGENTS.md
docs/AGENTS.md
docs/implementation/AGENTS.md
docs/reviews/AGENTS.md
```

The root retains only repository-wide invariants, deletion-driven migration,
Git workflow, task boundaries, and completion reporting. Rust ownership and
validation rules live beside `crates/`; documentation authority lives under
`docs/`; current-state evidence and review-package rules have deeper owners.

Git is now the sole VCS authority. Current work records full Git commit SHAs
only. Small isolated cuts may use explicit partial staging on `main`;
independent, parallel, protected-WIP, risky, or long-running work uses a
separate Git worktree and short-lived `codex/<topic>` branch. Validated task
branches are integrated into `main` and removed rather than retained as remote
WIP.

## Related policy consolidation

- Structural audit triggers and thresholds moved from root instructions to
  `structural-audit-policy.md`.
- The current test-selection authority is an 83-line
  `test-execution-policy.md`. The former 465-line profiling and detailed command
  record is retained under `test-profiling/` instead of being loaded for every
  Rust task.
- Stable Graph/RAG tooling now uses a full Git commit OID. The maintained
  Jujutsu tooling chapter and `jj_change_id` GraphPatch field were directly
  replaced; no compatibility document or dual schema was retained.
- Review intake now explicitly keeps sidecars inside returned ZIPs and treats
  older Git/Jujutsu evidence requirements as superseded by Git-only evidence.

## Historical evidence and non-goals

- Existing implementation notes and returned/requested design packages retain
  their historical Jujutsu references. Rewriting past evidence would make it
  less accurate.
- Local `.jj` metadata is not deleted in this cut because it may contain
  recovery history for the protected WIP. It is no longer an operational or
  documentation authority.
- No production Rust API is changed. Generic debug-history `change_id` and
  GraphPatch-operation storage are not treated as Jujutsu identities by this
  workflow cut; a future production Git-history consumer must use the stable
  Git contract rather than add a second VCS reader.
- No per-crate `AGENTS.md` files are added. Add one only after a harmful,
  non-obvious, crate-local mistake demonstrably recurs.

## Validation

Performed and passed:

- local Markdown link resolution across all changed instruction, policy, and
  stable-design files;
- `git diff --check` for the isolated policy/design path set;
- maintained-scope search confirming no Jujutsu authority remains outside the
  explicit Git-only prohibition and historical-evidence explanation; and
- explicit index inspection confirming no pre-existing staged files before the
  policy cut.

Rust tests, workspace checks, Clippy, structural audit, and Tier 2 were not run:
the cut changes only Markdown instructions, documentation routing, and an
unimplemented schema sketch, and the current test policy classifies direct
link/format/schema consistency as sufficient evidence.
