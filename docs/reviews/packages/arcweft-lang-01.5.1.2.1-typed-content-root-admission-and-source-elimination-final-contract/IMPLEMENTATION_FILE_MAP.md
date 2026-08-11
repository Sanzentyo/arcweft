# Implementation file and symbol map

Paths below are the concrete current owners inspected for this design. If main
moves a module without changing authority before implementation starts, update
the mechanical path only; the owner and API decisions remain unchanged.

| Cut | Current owner/path | Change |
| --- | --- | --- |
| 1 | `crates/arcweft-project/src/content.rs` | add final family/target/fact/reference/content types and inherent validation/accessors; retain existing binary/revision types |
| 1 | `crates/arcweft-project/Cargo.toml` | add only the lower-layer `arcweft-source`/`arcweft-id` normal dependencies required by the selected fields if not already present |
| 2 | `crates/arcweft-resource-model` identity owner | add `Ord`/`PartialOrd` derives to `ResourceDeclarationIdentity` at its original definition only if absent |
| 3 | current `arcweft-launch` `ManifestTokenPath` / `ManifestSourceMap` owner | add content-unit/profile-content token variants and inherent mapping to existing internal path segments |
| 4 | `crates/arcweft-lang-sema/src/project_index.rs` | embed `Arc<AcceptedProjectContent>`, make construction crate-private and mandatory, add inherent queries |
| 4 | `crates/arcweft-lang-sema/src/project_index/relations.rs` (current relation module) | remove `index_content_root_relations`; retain ordinary dependency relations |
| 4 | current sema type owner (`EntityKind`, `TypeKind`) | delete `EntityKind::Source`, `TypeKind::Source`, authored family table rows, displays, codecs/tests |
| 5 | new `crates/arcweft-lang-sema/src/project_index/content.rs` | own typed root resolver bridge, reference-candidate collection, errors, and deterministic ordering; no I/O |
| 6 | current project-loader profile-topology model (`model.rs`) | add `ProfileTopologyOverlaySet`, logical package paths, topology revision, watch target, `AcceptedProfileProject` |
| 6 | current project-loader topology constructor | replace `strip_prefix("@character.")` preloading with typed resolution/acquisition plan; add optional presence classification |
| 6 | current project-loader overlay resolver | consume separate text/binary sets before decode/read and reject unconsumed/cross-kind entries |
| 7 | parser/HIR declaration owners | delete source `content` syntax/HIR and all Source declaration/type ownership atomically with final manifest fact publication |
| 7 | project-index entity/formatter/Agent adapters | remove Source and old `ContentRoot` relation branches; query accepted content facts |
| 8 | compiler candidate assembly | pass accepted semantic world to content admission, then build final index; no partial index publication |
| 8 | `arcweft-bundle` Character package builder caller | consume `AcceptedProfileProject`; call existing inherent `BundleCharacterPackage::from_character_package` |
| 8 | project-loader watcher adapter | consume `ProfileTopologyWatchTarget`; add exact optional manifest appearance entry |
| 8 | LSP accepted environment/profile rebuild owner | replace loose candidate/characters/topology tuple with one `Arc<AcceptedProfileProject>`; add binary overlay capture |
| 8 | accepted cache key owners | replace accepted-project use of `SourceSetRevision` with `ProjectTopologyRevision` |
| 9 | maintained fixtures/docs/examples | migrate source `content`/Source cases directly to manifest roots and Stream/function forms; no compatibility fixture |

## Required search inventory during implementation

Use typed compiler errors and targeted symbol navigation to migrate all
consumers of:

- `ProjectGraphRelationKind::ContentRoot`;
- `index_content_root_relations`;
- `EntityKind::Source`;
- `TypeKind::Source`;
- Source AST/HIR/runtime/wire/tooling variants;
- source `content` AST/HIR variants;
- `SourceSetRevision` in accepted topology/cache/LSP publication;
- `LoadedProfileTopology` constructors;
- `ProfileTopologyWatchEntry.id`;
- text-only topology overlay entry points;
- Character root `strip_prefix` discovery.

Acceptance is compile/test/behavior evidence, not a checked-in source grep.
