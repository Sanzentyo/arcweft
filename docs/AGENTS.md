# Documentation instructions

Use [README.md](README.md) to locate an unfamiliar authority; a local edit does
not require reading the whole index or unrelated chapters.

## Authority and placement

- `00-overview/` through `05-build-and-security/`: maintained design/specification.
- `schemas/`: serialized contracts; `examples/`: illustrations, not overriding
  specifications or typed production APIs.
- `implementation/`: operational policies, dated state, validation, and handoffs.
- `reviews/`: requests, returned packages, intake decisions, and retained design.

Historical notes and requests do not outrank current production, maintained
specifications, or later accepted contracts. Preserve frozen package mirrors and
historical evidence, including old paths and VCS identities. Correct actual
historical errors explicitly rather than silently modernizing the record.

Update the stable chapter when design changes and record implementation evidence
separately. Keep progress, task ordering, percentages, and temporary blockers out
of stable chapters and `AGENTS.md`. Synchronize affected normative schemas and
examples. On a move/rename, update maintained links in the same cut, without a
compatibility duplicate.

Prefer links to an existing authority over copied rules and command tables.
Use `arcw`, `rust`, `json`, `toml`, `bash`, and `text` fence labels for the
respective languages, commands, and plain inventories/diagrams.

For docs-only validation use the corresponding section of
[test-execution-policy.md](implementation/test-execution-policy.md), not the
Rust workspace gate. Do not turn documentation/source scans into behavioral
acceptance tests. Instruction edits must preserve domain constraints and exact
permission boundaries; shortening is not itself evidence of better behavior.
