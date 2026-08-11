# Arcweft Lang-01.5.1.2.1 final contract

**Status:** `READY_FOR_IMPLEMENTATION`  
**Open result-changing decisions:** `0`  
**Open questions:** `0`  
**Repository:** `Sanzentyo/arcweft`  
**Inspected main:** `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`  
**Design date:** `2026-08-08`  
**Production changes included:** none

This archive is a standalone replacement contract for Lang-01.5.1.2 after
applying Lang-01.5.1.2.1's Source-elimination correction. It preserves the
already-landed binary topology, binary overlay, `CharacterPackage`, and
`ProjectTopologyRevision` substrate, then closes the remaining family,
presence, `ProjectSemanticIndex`, bundle, watch, and LSP boundaries.

## Final decisions at a glance

1. The closed content-root family is exactly `Character`, `Resource`, and
   `Activity`.
2. `Source`, `Source<T, E>`, `EntityKind::Source`, and source `content`
   ownership are deleted directly. No alias, compatibility reader, or
   old-spelling diagnostic survives.
3. Ordinary, authored-generator, and external-capability functions returning
   `Stream<T, E>` are not content roots.
4. `Character` is the only schema-1 file-backed content-root family. A present
   Character root retains one complete `Arc<CharacterPackage>` plus exact
   logical and host acquisition paths.
5. Text overlays retain `Arc<str>` and binary overlays retain `Arc<[u8]>`.
   Cross-kind coercion is forbidden.
6. `ProjectTopologyRevision` remains the sole accepted topology identity. It
   covers effective bytes, the existing resource-registry digest, and explicit
   optional-absence records; no parallel hash is introduced.
7. Optional absence is accepted only for an exact missing Character manifest
   and only when the selected-profile typed reference inventory contains no
   reference to that Character. Present-but-invalid optional content fails.
8. Manifest content facts are stored in one typed `AcceptedProjectContent`
   authority embedded in the final `ProjectSemanticIndex`; the old source-HIR
   `ContentRoot` graph relation is deleted.
9. `AcceptedProfileProject` atomically couples topology and semantic index by
   the same `ProjectTopologyRevision`. Bundle, watch, cache, and LSP consume
   that one carrier.

## Archive map

- `FINAL_CONTRACT.md` — normative decisions and acceptance predicates.
- `RUST_SHAPES.md` — exact final Rust-shaped public and crate-private owners.
- `OWNERSHIP_AND_DEPENDENCY.md` — crate direction and Sans-I/O boundaries.
- `ADMISSION_TRANSACTION.md` — deterministic candidate construction order.
- `RESOURCE_REVISION_AND_OVERLAYS.md` — exact byte/revision/overlay rules.
- `PRESENCE_AND_FAMILY_RULES.md` — family table and required/optional semantics.
- `MANIFEST_SOURCE_EVIDENCE.md` — source-map extension and range ownership.
- `PROJECT_INDEX_LOWERING.md` — manifest-to-index lowering and deletion of the
  old graph owner.
- `CONSUMER_PROJECTIONS.md` — bundle/watch/LSP/cache projections.
- `DIAGNOSTICS_AND_FAILURE_PRECEDENCE.md` — typed failures and exact ordering.
- `FAILURE_ATOMICITY.md` — no-partial-publication invariants.
- `IMPLEMENTATION_FILE_MAP.md` — concrete production cut map.
- `MIGRATION_AND_DELETION.md` — compiling cuts and deletion inventory.
- `TEST_MATRIX.md` — row-by-row positive, negative, revision, and rollback
  coverage.
- `VALIDATION_PLAN.md` — focused, workspace, Tier 2, and structural gates.
- `REPOSITORY_EVIDENCE.md` and `VERIFICATION_SCOPE.md` — what was and was not
  directly verified while preparing this package.
- `NORMATIVE_DELTA_LANG-01.5.1.2.1.md` — every Source-elimination correction.
- `REQUIREMENTS_TRACEABILITY.md` — request-to-contract/test mapping.
- `OPEN_QUESTIONS.md` — exactly `none`.
- `MANIFEST.txt` — deterministic member hashes; its self-entry is zeroed.

The adjacent `.sha256`, status, summary, and validation files describe the ZIP
itself. Internal member hashes are authoritative for archive contents.
