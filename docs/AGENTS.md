# Documentation instructions

Read `docs/README.md` before changing documentation. Keep each kind of document
in its authority layer.

## Authority

- `00-overview/` through `05-build-and-security/` are maintained stable design
  and specification chapters.
- `schemas/` owns wire, manifest, bytecode, bundle, save, trace, and other
  serialized contract documentation.
- `examples/` illustrates maintained behavior but does not override a stable
  specification or production typed API.
- `implementation/` records dated implementation state, validation evidence,
  non-goals, blockers, and handoff details.
- `reviews/` owns independently throwable requests, returned packages, intake
  decisions, and retained design material.
- Historical implementation notes and requests are evidence of their time;
  they do not outrank current production, maintained stable documentation, or a
  later accepted contract.

## Editing rules

- Put transient progress only in `implementation/`. Do not add completion
  percentages, current task lists, local change IDs, or temporary blockers to
  stable design chapters.
- Update stable chapters when the selected design itself changes. Record the
  concrete implementation and validation state separately.
- Distinguish performed, passed, failed, blocked, and not-run work. Never write
  a planned command as completed evidence.
- Use full Git commit SHAs for repository revisions. Current documentation must
  not require Jujutsu identities. Leave old Jujutsu references in historical
  notes unchanged unless correcting a factual error in that historical record.
- When renaming or moving a maintained document, update every maintained link
  in the same cut. Do not preserve the old path as a compatibility duplicate.
- Keep normative schemas and examples synchronized when the schema changes.
- Do not use documentation source scans as automated acceptance gates.

## Formatting

Use these code-fence labels consistently:

- Arcweft DSL: `arcw`
- Rust: `rust`
- JSON: `json`
- TOML: `toml`
- shell commands: `bash`
- plain diagrams and inventories: `text`

Prefer links to existing detailed authorities over copying long command,
threshold, crate-map, or package-workflow tables into multiple documents.
