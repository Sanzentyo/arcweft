# Repository reconciliation and adjudication matrix

| Requirement/decision | Final adjudication | Pinned repository fact / deviation reason |
| --- | --- | --- |
| Latest main | pin `5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`; connector-visible main equals origin/main | main advanced during work; newer AGENTS/request/intake commit was re-read and adopted |
| Old returned ZIP | not authority; this package is direct final | intake audit classifies old archive received/not implementation-ready |
| Uploaded vs repository old-ZIP SHA | repository SHA governs intake identity | uploaded request says `01F308C9...`; pinned request/audit say `01F308C08...`; substantive request is otherwise the same |
| Rust/AGENTS policy | both read completely before decisions | root AGENTS applies repository-wide; Rust Skill hash/inventory recorded |
| Public `res` path | do not claim public AST/HIR/sema implementation | pinned syntax is private attached grammar; readiness note says public switch blocked |
| Transport | strict UTF-8 JSON | current strict spanned JSON and canonical JSON infrastructure exists |
| Root marker/version | exact format string plus integer schema 1 | current registry accepts schema 1 only; no released extension format requires compatibility |
| Top-level multiplicity | one document may publish many schemas/types/codecs | `ResourceRegistryPublication` already accepts vectors and publishes atomically |
| Empty arrays | admitted | current registry has a canonical empty publication and independent candidate vectors |
| Package coordinate | `{id,version}` using current typed owners | `PackageId`/`PackageVersion` already define exact validation/canonical display |
| Nominal type spelling | object `{package,module,name}` | exact current `NominalTypeId` fields; avoids string parsing and version smuggling |
| Scalar tags | map all 14 current variants exactly | closed `ResourceScalarType` inventory inspected |
| Finite float | 16-byte-bit hex text, not JSON float | current canonical JSON rejects floats; `ResourceFloat` stores canonical finite bits |
| Negative zero | reject wire bits | current constructor normalizes it; rejection preserves one canonical input spelling |
| Bytes | no V1 encoding; attempted byte tag rejects | concrete deviation required by absence of any current byte scalar/type/constant variant |
| Option | absent `value` = None; present typed value = Some; null never used | current `ResourceConstValue::Option` is exact; canonical JSON rejects null |
| List/nonempty list | `list`/`non_empty_list`; sequence order retained | current `Vec`/`NonEmptyVec` and `Sequence` semantics |
| Ordered map | entry array; semantic duplicate rejection; canonical key-byte sort | map keys are typed constants, current digest also uses canonical key bytes |
| Record/enum constants | stable numeric IDs and optional enum payload | current BTreeMap record and exact enum value structures |
| AssetRef | exact public ID + payload kind | current `ResourceAssetRefValue` |
| ResourceRef | exact entity/public/type triple | current `ResourceRefValue`; no family inference |
| Retained refs | seven exact final categories | current Lang-01.4.1 production types and inherent tokens inspected |
| Presentation target | preserve global/view scope and target `PublicId` | current `PresentationTargetScope` and resolved enum |
| Scroll region | preserve owner View `EntityId` and region `PublicId` | current resolved enum |
| Unknown fields/duplicates/null/wrong shape | strict, distinct diagnostics at every object/tag | current strict decoder patterns preserve keys/ranges and avoid permissive serde-only decode |
| Version dispatch | one direct-final reader; unsupported stops | AGENTS direct replacement/no unneeded compatibility policy |
| Canonical bytes | current canonical JSON plus semantic array ordering | `canonical_json_bytes` already freezes object/string/integer behavior |
| Descriptor digest | required claim, independently recomputed | current registry exposes no per-descriptor digest although private exact transcript exists; add inherent API, not helper/trait |
| Registry digest | retain current schema/whole-registry contexts | no contradiction in existing verified substrate |
| Source ranges | `SourceDocument` + parallel lexical/semantic source map | existing launch/adapter patterns and source owner |
| Limits | 8MiB/64/65536 base plus exact string/collection/record/work ceilings | aligns existing source and strict JSON envelopes; adds required missing budgets |
| Atomicity | decode all, aggregate all, publish once | existing registry publisher is immutable/all-or-nothing |
| Wire owner | new `arcweft-resource-manifest` Sans-I/O crate | resource model explicitly excludes parsers/filesystem; adapter metadata shows one-crate codec pattern |
| Filesystem/package owner | project loader only; explicit paths/seeds | current topology model is resolver-seeded and source-backed |
| Project manifest field | singular top-level `resource-type-manifest` path | current strict root manifest is the actual package/build entry owner |
| Bundle publication | required section kind code 22 containing canonical manifests | current bundle owns typed code table/required unknown behavior; no section exists yet |
| Runtime reconstruction | same reader/encoder, base registry, final digest check | avoids second format and silent empty-registry fallback |
| Production changes in this task | none | request is contract-only; package validator confirms artifact-only contents |
