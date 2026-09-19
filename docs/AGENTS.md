# Documentation instructions

Use [README.md](README.md) to locate an unfamiliar authority, not as a mandatory
reading list. Root autonomy and task-priority rules apply to documentation too.

## Authority and placement

- `00-overview/` through `05-build-and-security/`: maintained design/specification.
- `schemas/`: serialized contracts; `examples/`: illustrations, not specifications.
- `implementation/`: operational policies, dated state, validation, and handoffs.
- `reviews/`: requests, returned packages, intake decisions, and retained design.

Historical notes do not override current user direction or later accepted
contracts. Production records what exists, not what must remain forever. Resolve
stale instructions and technical inconsistencies within the requested scope;
update the maintained target and affected consumers together. A missing paragraph
is not, by itself, a reason to request another design assignment.

Preserve frozen package mirrors and historical evidence, including old paths and
VCS identities. Correct actual historical errors explicitly, not by silently
modernizing the record. Document a changed current decision outside the mirror.

Synchronize affected stable chapters, normative schemas, and examples when the
design changes. Keep progress, task ordering, percentages, and temporary blockers
out of stable chapters and `AGENTS.md`. On a move/rename, update affected
maintained links in the same cut, without a compatibility duplicate.

Prefer links over copied authorities and command tables. Use `arcw`, `rust`,
`json`, `toml`, `bash`, and `text` fence labels for the corresponding content.
Validate the changed surface using the documentation section of the
[test policy](implementation/test-execution-policy.md), not a Rust workspace run.
Do not turn source/documentation scans into behavioral acceptance gates.

For instruction maintenance, separate product invariants and explicit permission
limits from adjustable process defaults. Remove obsolete reading, asking,
stopping, testing, and reporting rituals rather than preserving them merely
because an older instruction required them. Review the resulting behavior;
shorter text alone is not evidence of improvement.
