# Validation scope and evidence boundary

## What this archive actually verifies

The archive was produced against `Sanzentyo/arcweft` main commit
`3670625a02b9e7e8578b57fc7b148a1758a17dba`. The request-stated
`17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc` is its production parent; the one
later commit is the repository intake/audit for the preceding return. Source
evidence therefore uses the later complete tree while retaining the request's
stated baseline.

The following work is directly performed for this package:

- the complete attached request, project premise, and complete Rust skill are
  retained under `inputs/` and were read;
- the current branch head, current `AGENTS.md` hierarchy, maintained runtime
  contracts, preceding frozen mirror/intake, and concrete source owners in
  `SOURCE_EVIDENCE.md` were inspected through the authenticated GitHub
  connector;
- every required design crossing is mapped to one final owner, schema,
  transcript, state transition, source path, deletion cut, and test row;
- all 72 version-1 persistence rows form a closed reference graph;
- all nine `NeedProducerFamily` rows have an explicit execution route and
  policy set;
- the Match transcript inventory covers all 27 current
  `CheckedExpressionResolution`, eight `CheckedValueResolution`, seven
  `CheckedSelectResolution`, five `CheckedPatternResolution`, thirteen
  `HirPatternKind`, and seven `HirLiteral` variants;
- the ownership classifier has exactly one row for each of the 85 current
  `TypeKind` variants;
- prose, JSON, CSV, manifest, and structural-absence rules are checked by the
  package validator;
- negative self-tests demonstrate rejection of each blocker class mandated by
  the request.

## What is specified rather than executed

This is a design-only archive. It does not modify or build production Rust.
Accordingly:

- no production checkout or patch is included;
- no production `cargo fmt`, `cargo check`, `cargo clippy`, or `cargo test` was
  run in this return;
- the exact commands to run after each implementation cut are normative in
  `COMPILE_CLEAN_SEQUENCE.md`;
- the parent archive's SHA-256 is retained from the request. Its frozen mirror
  and repository intake were inspected, but the parent binary was not
  independently streamed and rehashed here.

`READY_FOR_IMPLEMENTATION` means the result-changing design choices are closed
and every named final type is current-owned or same-cut constructible. It does
not mean the production implementation or its future tests already exist.

## Source-trust order used

1. current production source at the inspected main SHA;
2. maintained stable documentation;
3. the attached correction request;
4. later accepted contracts, when present;
5. preceding package text only where not superseded by the current intake.

No stale package observation overrides a current source owner. No absent source
path is treated as a placeholder implementation target.

## Archive verification layers

1. **Schema validation** checks closed fields, variants, references, versions,
   bounds and first-error declarations.
2. **Cross-artifact validation** checks exact family, type, transcript, cut,
   deletion and test inventories.
3. **Structural-absence validation** rejects the forbidden crossings listed in
   `STRUCTURAL_ABSENCE.md`.
4. **Manifest validation** recomputes SHA-256 for every content file listed in
   `MANIFEST.json`; `MANIFEST.sha256` protects the manifest itself.
5. **Archive validation** extracts the final ZIP to a clean temporary
   directory and runs the same read-only validator there.
