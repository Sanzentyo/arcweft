# Arcweft Lang-01.5.1.1.1 final dialogue-profile contract

This archive is a standalone, decision-complete reconciliation of the
`dialogue profile owner and admission` boundary against Arcweft `main` at:

```text
0c8cb74dd96116a8b987cc419c9a280b6cabe4a4
```

The originating request is retained in the repository as **resolved** and says
not to dispatch it again. Current `main` has also implemented the selected
contract. This package therefore has two simultaneous dispositions:

```text
DESIGN_STATUS=READY_FOR_IMPLEMENTATION
CURRENT_MAIN_STATE=SATISFIED_BY_CURRENT_IMPLEMENTATION
DISPATCH=DO_NOT_REDISPATCH_THE_RESOLVED_REQUEST
```

It is useful as:

1. the exact final design contract for future review or regression work;
2. an as-built conformance description of the current implementation; and
3. a complete test and verification specification if the boundary is changed.

It is not a production patch, overlay, compatibility layer, alternate reader,
or source gate. No repository files were modified to prepare it.

## Final decisions in one page

- `arcweft-manifest-model` remains a neutral owner of IDs, hashes, and wire
  primitives. It does not depend on View, dialogue, launch, compiler, or
  runtime-driver presentation types.
- `arcweft-launch` owns the sole schema-1 manifest decoder,
  `ArcweftManifestDocument`, `ProfileSpec`, `DialogueProfileSpec`,
  `SourceBackedManifest`, and the one revision-bound generic
  `ManifestSourceMap`.
- `arcweft-dialogue` owns `DialoguePresentationProfile`,
  `InlineFailurePolicy`, and the lower reusable six-field
  `DialogueProfileRevision`.
- `arcweft-compiler` owns `CheckedDialogueProfile` and the only cross-product
  admission operation. Admission runs after one compiler-owned immutable
  `ValidatedViewProduct` exists.
- Project-loader does not compile View programs, depend on runtime-driver, or
  construct a second catalog.
- The manifest wire is `inline-failure`, not `inline_failure`; the policy uses
  the dialogue-owned strict tagged representation unchanged.
- `SourceBackedManifest::manifest_token_span` projects dialogue ranges through
  the existing generic source map. There is no dialogue-only map and no
  second parse.
- Publication is atomic over a complete checked candidate. A rejected candidate
  leaves the previous complete `ProgramGeneration` and revision tuple intact.
- Source `dialogue defaults`, `DialogueDefaultsItem`, and `@dialogue.*` are
  deleted directly and receive only ordinary parser recovery, not a
  spelling-specific compatibility diagnostic.

## Reading order

1. `SOURCE_REQUEST_STATUS.md`
2. `FINAL_CONTRACT.md`
3. `AS_BUILT_API.md`
4. `OWNER_AND_DEPENDENCY_GRAPH.md`
5. `FINAL_MANIFEST_SCHEMA.md`
6. `ACCEPTED_CANDIDATE_FLOW.md`
7. `SOURCE_MAP_AND_DIAGNOSTICS.md`
8. `MIGRATION_AND_DELETION.md`
9. `TEST_MATRIX.md`
10. `VERIFICATION_PLAN.md`
11. `REPOSITORY_EVIDENCE.md`
12. `REQUIREMENTS_TRACEABILITY.md`
13. `FINAL_STATUS.md`

## Verification boundary

This return directly performed deterministic archive construction, member
enumeration, per-file SHA-256/size verification, exact `OPEN_QUESTIONS.md`
verification, ZIP CRC/integrity testing, and extraction/reverification.

The Arcweft workspace was not cloned into this execution environment, so Cargo,
Clippy, workspace tests, Tier 2, and parity suites were not rerun by this
return. The package separately identifies the checks recorded as passing by the
repository's implementation note and the commands that must be rerun in a real
checkout. It does not convert recorded historical validation into a new test
claim.
