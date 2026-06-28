# Seq-02.8 Overlay Production Application

Request source: `docs/reviews/requests/2026-06-27-seq-02.8-overlay-production-application.md`.

This note is the final seq-02 cleanup gate. It audits the generated seq-02 overlay/package material against the production contracts that were actually implemented by seq-02.1 through seq-02.7, records which hunk families were accepted or discarded, and closes the gate without applying any new generated code blindly.

## Audit Baseline

Audited repository snapshot: GitHub connector evidence for `main` at commit `7ce664c5fcaeced5c4424d26f31625815a8784b3` before this docs-only closure note.

Repository-local commands such as `jj status`, `jj diff`, and Cargo validation were not executable in this connector-only audit environment. Instead, this note records:

- repository-visible implementation notes and source files inspected through the GitHub connector;
- source-gate tests already checked into the repository;
- validation results already recorded at each seq-02.x production cut;
- the exact validation commands that must remain green at the next full local checkout cut point.

No Rust source hunk is applied by seq-02.8. The only production change for this cleanup gate is this implementation-status note.

## Overlay Location and Scratch Status

The original broad seq-02 package and later package ZIPs are referenced by the seq-02 implementation notes, but the generated overlay directories, patch scripts, and package ZIP contents are not present as active repository files in this snapshot. Connector source searches found `apply_seq_02` only in the seq-02.8 request and the first-cut implementation note. Searches for the obsolete generated API names `ProductResourceSection`, `ResourceRecord`, and `ResourceFieldValue` found only implementation-note discussion of why those overlay hunks were rejected.

Decision:

- absent original overlay files are not a source of production hunks;
- no scratch overlay directory is promoted, archived as source, or left looking like active `crates/`, `tests/`, or fixtures;
- production acceptance is limited to hunk groups that were already translated into normal source edits and validated by the seq-02.x cuts below;
- any unavailable or unreferenced generated hunk is classified as `obsolete/unverifiable` and discarded for production.

## Overlay Classification Table

| Overlay / package source | File or hunk family | Status | Production decision | Rationale |
| --- | --- | --- | --- | --- |
| Broad seq-02 package, `apply_seq_02.py` generated overlay | Generated overlay as a whole | Obsolete / unverifiable | Discard | The first-cut note explicitly says no generated `apply_seq_02.py` overlay was applied as a production patch. Later seq-02.x cuts redesigned the contracts before production edits. |
| Broad seq-02 first cut | AWFB unknown optional section preservation in `container.rs` / `container/opaque.rs` | Already implemented | Keep | Unknown optional section-kind codes, raw descriptor metadata, and unknown required rejection are production behavior from the first cut. |
| Broad seq-02 first cut | `ArtifactIdentity`, content root, manifest digest identity | Already implemented | Keep | `BundleView::artifact_identity()` and manifest-sensitive artifact identity are production substrate for patch/signing. |
| Broad seq-02 first cut | Patch descriptor raw section-kind preservation | Already implemented, later superseded by schema 2 | Keep through schema-2 path | Raw `SectionKindCode` preservation remains required; schema-2 patch replaced earlier patch behavior without dropping this invariant. |
| Broad seq-02 first cut | Native/web/runtime-host/product AWBC fixture regeneration and AWBC lowering fixes | Already implemented | Keep, do not reopen | These were fixture/runtime parity closures validated before seq-02.1; seq-02.8 found no overlay flaw requiring reopening them. |
| seq-02.1 common resource wire package | `resource_codec` split into budget/header/table/field/wire/types/kind/inspection modules | Already implemented | Keep | This is the accepted shared compact envelope substrate for every migrated product resource family. |
| seq-02.1 common resource wire package | 48-byte header, expected-codec decode, canonical tables, field registry, unknown optional/required policy, budgets, inspection export | Already implemented | Keep | The implementation note records this as the common contract; later packages must reuse it and must not probe codecs. |
| seq-02.1 common resource wire package | Schema-neutral common fixtures and source gate | Already implemented | Keep as golden/focused tests | Fixtures remain deterministic common-wire tests; source gate prevents private table/reference formats outside the common module. |
| seq-02.2 runtime resource package | `resource_codec::runtime` compact schemas for `RuntimeTypes`, `Entrypoints`, `AdapterRequirements` | Already implemented | Keep | Product runtime families are compact-first and no longer decode product JSON fallback. |
| seq-02.2 runtime resource package | `product.rs` compact runtime encode/decode wiring | Already implemented | Keep | Source gate rejects `RuntimeTypesSection`, `EntrypointsSection`, `AdapterRequirementsSection`, old `encode_json` payloads, and old `required_payload::<...>` patterns. |
| seq-02.2 runtime resource package | Runtime compatibility fingerprints and runtime-driver adapter digest | Already implemented | Keep | Patch compatibility and generation identity now use compact owner data instead of serde JSON fingerprints. |
| seq-02.3 content/presentation/entity package | Generated API based on `ProductResourceSection`, `ResourceRecord`, `ResourceFieldValue` | Superseded | Reject | The package assumed a different common resource API. Production was rebased to `ProductResourceEnvelope`, `ResourceField`, `FieldRegistry`, `StringTable`, `PublicIdTable`, and `EnumRegistry`. |
| seq-02.3 content/presentation/entity package | Entity graph / ad hoc entity section ideas | Speculative / future | Reject for seq-02.8 | The product model and AWFB carrier decisions were not concrete enough; entity/graph-index work remains a future design slice. |
| seq-02.3/02.4 package application | Compact `ContentCatalog`, `AssetCatalog`, `DisplayCatalog`, `SourceMap`, `AudioGraph` owner codecs | Already implemented | Keep | These migrated current product truth without inventing unsupported dialogue/entity projections. |
| seq-02.4 audio package | Full-file `product.rs` / `Cargo.toml` replacement | Unsafe / superseded | Reject | The implementation note records that full-file replacement was not used; only narrow production edits against current APIs were accepted. |
| seq-02.4.1 UI/style/text/input/theme package | New section kinds and compact codecs for `UiProgram`, `UiStyle`, `UiText`, `UiInput`, `UiTheme` | Already implemented | Keep | UI families are explicit compact-first sections; the legacy umbrella `Ui` codec family remains reserved/future. |
| seq-02.4.1 UI package | UI tests for deterministic bytes, budgets, CSS descriptor identity, palette/input compatibility, source gate | Already implemented | Keep as focused tests | These are production tests, not scratch overlay fixtures. |
| seq-02.5 patch package, earlier shape | Separate versioned patch submodule / schema-1 compatibility reader | Obsolete | Reject | Production directly replaced `arcweft_bundle::patch` with schema 2 because the old patch surface is not a stable external API. |
| seq-02.5 direct replacement package | `patch.rs` schema 2 manifest, materialization contract, per-section fingerprints, target identity checks | Already implemented | Keep | Schema 2 is the only decoded patch schema; materialized targets are unsigned from core and validated against target identity. |
| seq-02.5 direct replacement package | Section default compatibility on `BundleSectionKind` and inverse product mapping on `ProductSectionCodecKind` | Already implemented | Keep | Behavior lives on Arcweft-owned enums instead of scattered helper matches or extension traits. |
| seq-02.5 direct replacement package | Runtime-driver/session/native endpoint local reclassification removal | Already implemented | Keep | Runtime/player paths consume declared patch compatibility instead of rederiving local heuristics. |
| seq-02.6 AWFR package | `AwfrArchiveManifest`, `ExternalPayloadCarrier`, release-manifest rewrite plan | Already implemented | Keep | Sans I/O release/archive models bind descriptors, digests, carriers, and metadata-only rewrite policy. |
| seq-02.6/02.7 signing package | `SigningPolicy`, `SigningSubjectKind`, digest transcripts, `SignatureDisposition` | Already implemented | Keep | Signing decisions stay in `arcweft-bundle` data models; keys, clocks, network credentials, and signature generation stay in adapters. |
| seq-02.6/02.7 separated draft patch | CLI external payload command draft patch | Superseded | Reject direct patch; keep equivalent production edit | The draft patch was not a valid unified patch for the checkout. Equivalent `arcw cache fetch-external` wiring was implemented against the current CLI file. |
| seq-02.6/02.7 remaining items | HTTP(S) external payload fetch, publish adapter, target signature regeneration adapter, full release trust verifier, external payload materialization mode wiring | Still valid but deferred | Do not apply through seq-02.8 | These are follow-up implementation slices, not generated overlay cleanup. They remain explicit TODOs below. |
| Any leftover generated fixture/scratch material | Workspace-external `crates/`, `tests/`, fixtures, overlay outputs | Obsolete / not present | Delete or keep absent | No such active scratch path is promoted. Only checked-in focused tests and implementation notes remain. |

## Accepted Hunk Rationale

The accepted hunk families share these properties:

1. They were applied through normal production edits after the relevant seq-02.x design contract existed.
2. They compile against the current Arcweft APIs rather than package-local generated shims.
3. They preserve the AGENTS.md layering rules: data-format code remains Sans I/O, adapter code owns filesystem/network/key/clock/platform effects, and behavior for Arcweft-owned enums lives on the enum or owning module.
4. They have focused tests or source gates recorded in the relevant implementation note.
5. They do not reopen older seq-02 substrate unless they are the designated owner slice for that substrate.

## Rejected Hunk Rationale

Rejected or discarded hunk families were rejected for one or more of these reasons:

- blind generated overlay application from `apply_seq_02.py` would bypass the seq-02.1 through seq-02.7 contracts;
- the package assumed non-existent common API names such as `ProductResourceSection`, `ResourceRecord`, or `ResourceFieldValue`;
- a full-file replacement would erase current production changes and make review unsafe;
- a patch tried to preserve compatibility with schema-1/internal parser/compiler shapes that the current architecture intentionally removes;
- a hunk would add JSON product fallback for a migrated family;
- a hunk would move filesystem, network, clocks, signing keys, or platform handles into Sans I/O crates;
- a hunk described a still-valid future feature but not an implementation-ready seq-02.8 production cleanup.

## Fixture Decisions

| Fixture/source material | Decision | Notes |
| --- | --- | --- |
| seq-02.1 common wire fixtures | Golden/focused tests | Keep deterministic byte, table, budget, unknown-field, digest, and inspection tests. |
| seq-02.2 runtime resource fixtures | Golden/focused tests | Keep deterministic runtime section round trips, compatibility fingerprints, and migrated runtime product gate. |
| seq-02.3/02.4 catalog/audio fixtures | Focused tests | Keep current-product-truth round trips and source gate against JSON fallback. |
| seq-02.4.1 UI fixtures | Focused tests | Keep UI family codec tests and fallback rejection/source-gate coverage. |
| seq-02.5 patch schema fixtures | Focused tests | Keep schema-2 round trip/materialization/unknown optional/external descriptor/source-gate tests. |
| seq-02.6/02.7 AWFR/signing/cache fixtures | Focused unit/adapter tests | Keep archive/signing-policy/external-payload cache tests. |
| Original generated overlay scratch fixtures | Deleted / not preserved | Not present in the repository snapshot and not promoted as active fixtures. |
| Review-only package prose | Implementation notes | Keep under `docs/implementation/` only where it records actual production status, deviations, and TODOs. |

## Safety Gate Review

| Gate | Evidence / decision |
| --- | --- |
| No migrated product JSON fallback | Runtime and catalog source gates are checked in. `product.rs` emits/decodes migrated runtime, catalog/audio, and UI families through compact owner codecs. JSON remains only manifest/inspection/export where still allowed. |
| No codec probing | Product decode calls expected `decode_canonical_section` functions by AWFB section kind. The seq-02.1 contract requires callers to provide the expected `ProductSectionCodecKind`; migrated families do not choose by probing bytes. |
| No legacy compatibility layers | Patch schema 2 directly replaces schema 1; no alternate schema-1 reader or compatibility module is preserved. Current public names remain only as primary schema-2 names. |
| No raw signature passthrough | Core patch materialization emits unsigned changed targets; signing policy distinguishes unchanged preservation from changed-target invalidation and adapter-required signatures. |
| No ad hoc section-family fingerprinting | `BundleSectionKind::patch_default_compatibility`, `ProductSectionCodecKind::from_section_kind`, migrated runtime/catalog/UI compatibility APIs, and patch schema-2 fingerprints keep behavior on owning boundaries. |
| Sans I/O preserved | `arcweft-bundle` owns deterministic codecs, manifests, carriers, digests, policies, and rewrite plans; `arcweft-project-loader` and CLI own filesystem/cache/command adapters. |
| No active scratch workspace paths | No overlay directory or package scratch path is promoted as source. Seq-02.8 adds only this documentation note. |

## Production Change Summary

Seq-02.8 accepts no additional Rust production code. All accepted production hunks were already applied in earlier seq-02.x cut points:

- first cut: AWFB unknown optional section preservation, artifact identity, raw section-kind preservation, fixture/product AWBC closures;
- seq-02.1: common compact resource envelope and source gate;
- seq-02.2: runtime compact resource sections and migrated runtime product gate;
- seq-02.3/02.4: content/catalog/source/audio compact resource sections and gate;
- seq-02.4.1: UI/style/text/input/theme compact resource sections and tests;
- seq-02.5: direct patch schema-2 replacement and declared compatibility materialization;
- seq-02.6/02.7: AWFR/external carrier/signing policy Sans I/O models, local/cache external payload adapter, and CLI fetch-external wiring.

The seq-02.8 production application package consists of this final audit note and the decision to discard all remaining generated overlay material that has not already been translated through those contracts.

## Validation Record

The following validation was recorded by the earlier production cut notes:

| Cut | Recorded validation result |
| --- | --- |
| First cut | `just test-workspace`, clippy workspace, structure audit, web/player/runtime/project-loader focused tests, CLI regression gates, and `git diff --check` passed; structure audit reported 0 errors and 105 warnings. |
| seq-02.1 | `cargo fmt --all -- --check`, common codec tests, source gate, bundle check/clippy, structure audit, and `git diff --check` passed; structure audit reported 0 errors and 105 warnings. |
| seq-02.2 | Runtime resource tests, migrated runtime product gate, runtime codec integration tests, bundle/runtime-driver check/clippy, structure audit, `git diff --check`, and broader bundle tests passed; structure audit reported 0 errors and 106 warnings. |
| seq-02.3/02.4 | Product catalog/audio codec tests, source gate, bundle full tests, focused check/clippy, structure audit, and `git diff --check` passed; structure audit reported 0 errors and 106 warnings. |
| seq-02.4.1 | UI resource codec tests, bundle all-target tests, clippy, structure audit, `git diff --check`, and `just test-workspace` passed; structure audit reported 0 errors and 107 warnings. |
| seq-02.5 | Patch schema, runtime-driver patch source gate, native endpoint tests, focused check/clippy, CLI regression/run-bundle patch tests, structure audit, `git diff --check`, and `just test-workspace` passed. |
| seq-02.6/02.7 | Release archive/signing-policy tests, project-loader external payload cache tests, CLI cache tests, focused clippy, bundle/project-loader tests, structure audit, `git diff --check`, and `just test-workspace` passed; structure audit reported 0 errors and 107 warnings. |
| seq-02.8 | Connector audit completed; no Rust/source hunks were changed; no local Cargo/Jujutsu commands were run in this environment. |

Commands that must remain the final local checkout gate before claiming a post-seq-02 code cut:

```bash
rg -n "apply_seq_02|seq-02 overlay|overlay|patch v2|AWFR|external payload|signing policy|resource codec" . -g "*.md" -g "*.rs" -g "*.py" -g "*.json"
jj status
jj diff
cargo fmt --all -- --check
just test-workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Focused seq-02 source gates that must remain present and passing:

```bash
cargo test -p arcweft-bundle --test resource_codec_source_gate --all-features -- --nocapture
cargo test -p arcweft-bundle migrated_runtime_sections_do_not_use_product_json_fallback --all-features -- --nocapture
cargo test -p arcweft-bundle --test runtime_resource_product_gate --all-features -- --nocapture
cargo test -p arcweft-bundle --test product_catalog_source_gate --all-features -- --nocapture
cargo test -p arcweft-bundle --test ui_resource_codecs --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test patch_source_gate --all-features -- --nocapture
```

## Structural Audit Summary

Seq-02.8 itself changes no Rust file and introduces no new module, dependency, feature, public Rust contract, or crate boundary. Therefore it adds no new structural hotspot.

The latest recorded seq-02 structural-audit envelope is from the seq-02.6/02.7 application:

```text
files scanned: 1607
Rust files: 887
Rust physical LOC: 431785
package manifests: 90
violations: 0 error(s), 107 warning(s)
```

Known warning-band files are already documented in the relevant cut notes. Newly introduced responsibility modules for common codecs, UI codecs, AWFR archive, signing policy, and external-payload cache adapters were kept below the 1200 LOC production warning threshold in their cut notes. Existing broad modules such as `container.rs`, `lib.rs`, `patch.rs`, and `release.rs` remain warning-band ownership-review items, not new seq-02.8 errors.

## Commit and Push Cut Points

Historical production cut points are already separated by implementation note:

1. First cut: unknown optional AWFB preservation, artifact identity, and AWBC/product fixture closures.
2. seq-02.1: common resource wire substrate.
3. seq-02.2: runtime compact resource sections.
4. seq-02.3/02.4: catalog/audio compact resource sections.
5. seq-02.4.1: UI/style/text/input/theme resource sections.
6. seq-02.5: patch schema 2 direct replacement.
7. seq-02.6/02.7: AWFR/external payload/signing boundary.
8. seq-02.8: this docs-only overlay production-application closure.

No seq-02.8 Rust change should be batched with seq3 generation/windowed live patch behavior. Future implementation slices for the remaining TODOs below should each be committed/pushed as independent, validated cut points.

## Final Completion Criteria

Seq-02.8 is complete when:

- every repo-visible generated overlay/package hunk family is classified in this note;
- no unseen or scratch overlay material is applied as production code;
- accepted production code is traceable to seq-02.x implementation notes and tests;
- rejected/superseded/unsafe/speculative hunk families are not preserved as legacy compatibility layers;
- source gates still reject migrated product JSON fallback and removed compatibility shapes;
- structural audit status is recorded;
- remaining TODOs are explicitly separated from the overlay cleanup gate.

This note satisfies the seq-02.8 cleanup gate for the connector-audited snapshot. Full local checkout validation should still run before the next code-bearing push cut point.

## Remaining TODOs / Non-Goals

These are not seq-02.8 overlay-application work and must not be smuggled in by generated overlay code:

1. Implement HTTP(S) external payload fetching with auth/proxy/client/retry/cancellation policy consistent with the existing release bundle fetch adapter.
2. Implement a release-publish adapter that stages target AWFB and payload bytes, uploads mirrors, generates signatures, writes the final AWFR archive atomically, and records recoverable rollback state on failure.
3. Implement target signature regeneration adapter flow by reusing the existing signing adapter boundary rather than moving key access into `arcweft-bundle`.
4. Implement a full release trust verifier that combines AWFR archive signatures, release manifests, signed base bundles, signed patches, materialized targets, and external payload digest validation.
5. Thread `ExternalPayloadMaterializationMode` through `apply_patch_bundle` or a higher-level adapter materialization workflow.
6. Split `resource_codec/runtime.rs` after later migrated section-family work proves stable split points.
7. Design and implement entity, shader, contracts, graph-index, locale/text, and debug-symbol product resource families only when their owning design requests provide concrete carriers and schemas.
8. Implement seq3 generation/windowed live patch behavior separately; seq3 must consume patch schema-2 declared compatibility instead of reclassifying artifact changes locally.

## Design Deviations

- The original package ZIPs and extracted overlay directories were not present in this connector-audited repository snapshot, so file-level hunks not preserved in repo-visible notes are classified as discarded/unverifiable rather than reconstructed.
- `jj status`, `jj diff`, Cargo, clippy, and the structural audit script were not run in this connector-only environment for seq-02.8. Prior cut-point validation is recorded above; the next local checkout code cut must rerun the commands listed in the validation gate.
- Seq-02.8 intentionally applies no new code. It is a production-application audit, not a new resource codec, patch, AWFR, signing, or seq3 design slice.
