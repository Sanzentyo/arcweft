# Final design contract

## Authority and precedence

This package is a narrow correction layered over the retained `.1.2.1` archive with SHA-256 `aa43429b6ffe5aac6489c94c7ff7a117ca1bbd43c764fed6ff4a1f3b5d540e06`. It does not add a second role root, catalog graph, semantic generation ID, binding digest, custom digest, operational catalog, or construction handle. The one serialized `RuntimeGenerationContractDeclaration`, its canonical version-1 body, and its `RuntimeGenerationIdentity` remain the sole generation correlation authority.

The current-main production dependency direction is decisive: `arcweft-character` and `arcweft-view` cannot return core admission wrappers, and `arcweft-core` cannot depend on either presentation crate. Therefore each lower catalog owns only its canonical local digest; `arcweft-runtime-driver`, which already depends on character, View, dialogue, and core, performs the lossless digest projection and issues generation-bound admitted borrows. This is the only current-source defect that refines the parent wording without reopening its result.

## Exact outcome summary

- `CharacterCatalog::runtime_digest_v1` hashes validated catalog rows in `CharacterId` order, using the existing `CharacterManifest::semantic_fingerprint_v1` as the exact per-manifest semantic payload.
- `ViewRegistry::runtime_digest_v1` hashes live public identities in `ViewId` order, includes schema and exact Rust/Arcweft implementation identity, excludes anonymous process-local registrations and tombstone slots, and requires `AcceptedViewProgramRevision` on Arcweft implementations.
- `AdmittedGenerationCatalogs<'generation>` is issued only after local validation, digest recomputation, declared-digest comparison, generation comparison, cross-catalog relationship checks, and atomic publication.
- Six authored dialogue roles are exact accepted standard opaque nominal types owned by `std.character_dialogue`; Style is not authored and is exactly the ordered `Choice([EntityRef<Style>, RichText])` projection.
- Project and producer root facts are non-Serde semantic bridge facts. Both root IDs are lossless copies of the existing `RuntimeSemanticTypeId([u8; 32])`; dense table indices, display names, HIR IDs, source text, and iteration order are never identity inputs.
- Every typed `RuntimePlan` publication site and every AWBC typed site retains a typed site coordinate and the same root ID. Plan-paired admission compares normalized rows exactly; standalone AWBC carries the same embedded generation contract and retained plan-site coordinates.
- `RuntimeNominalRecordAdmissionDomain<'generation>` is a borrowed enum over a project-site domain or the retained producer-shape view. Neither variant is Serde, defaultable, field-constructible, dereferenceable, or generation-erasing.
- AWBC adds a canonical nominal-domain table and a `domain` operand to `MakeRecord` and record constants. Tag `0x00` selects a project root; tag `0x01` selects one exact producer. Opcode `0x0f` and all version numbers remain unchanged.
- Authority-bearing checked-value validation is `RuntimeCheckedType::validate_value`, returning structured paths and errors. One 65,536-unit work budget is shared across all recursive descent, Choice branches, and nominal-tree traversal; nesting remains the existing limit 64. Choice success requires exactly one matching branch and all branches are evaluated in source order.
- Typed checked failures remain structured through nominal tree, dialogue, restore, replay, View, plan/AWBC admission, and VM boundaries.
- Migration lands the lower role/ID vocabulary first, then checked scalar substrate, declarations, semantic projections, root correlation, admission, AWBC, execution cut, dialogue, persistence/activation consumers, and finally deletion of unchecked/generation-blind paths.

## Version and compatibility rule

Every Arcweft-owned schema, ABI, codec, digest-domain, protocol, save, snapshot, persistence, and related version is exactly `1`. This is an unreleased in-place replacement. There is no compatibility alias, optional field, defaulted authority coordinate, old reader, dual reader, source-name resolver, or fallback project/producer domain.

## Readiness

All fifteen result-changing decisions have exact owners, fields, derives, constructors, accessors, errors, tables, byte encodings, precedence, migration order, and tests. `OPEN_QUESTIONS.md` is exactly `none`.
