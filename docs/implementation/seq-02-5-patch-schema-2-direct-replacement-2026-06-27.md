# Seq-02.5 Patch Schema 2 Compatibility and Target Materialization

Request source: `docs/reviews/requests/2026-06-27-seq-02.5-patch-v2-compatibility-and-materialization.md`.

This package is a direct replacement for `arcweft_bundle::patch`. The previous
package shape that added a separate versioned patch submodule has been
discarded. The patch surface is not currently consumed as a stable external API,
so this cut moves the existing module to schema 2 and deletes schema-1 reader
behavior rather than preserving it.

## Current Evidence Rechecked

- `AGENTS.md`: data-format crates remain Sans I/O, lower-level crates must not
  depend on adapters, broad root re-exports are not used for compatibility, and
  internal refactors should replace the old model directly when compatibility
  conflicts with the target architecture.
- `crates/arcweft-bundle/src/patch.rs`: current patch artifacts hold a manifest,
  `BundlePatchPlan`, and changed embedded payloads; materialization keeps base
  manifest bytes and validates the target content root.
- `crates/arcweft-bundle/src/container.rs` and `container/identity.rs`: AWFB
  already exposes content root and `ArtifactIdentity` over kind, content root,
  and manifest digest.
- `crates/arcweft-bundle/src/resource_codec/runtime.rs`: migrated runtime
  sections already expose semantic compatibility classification.
- `crates/arcweft-bundle/src/resource_codec/product_catalog.rs`: migrated catalog
  families are compact-first and content/presentation/resource oriented.
- `crates/arcweft-runtime-driver/src/swap.rs`: runtime generation swap has a
  local classifier; patch application needs a declared-compatibility entrypoint.
- `crates/arcweft-runtime-driver/src/session.rs` and
  `crates/arcweft-player-native/src/patch_endpoint.rs`: patch paths currently
  inspect/apply patch bytes and need to consume declared labels.

## Direct Replacement Policy

`arcweft_bundle::patch` becomes schema 2. There is no alternate module or reader
branch. `PATCH_PLAN_SCHEMA_VERSION` is `2`; decode rejects any manifest or
PatchPlan section whose schema version differs. Existing public names such as
`BundlePatchArtifact`, `BundlePatchPlan`, `PatchCompatibility`,
`encode_patch_bundle`, `decode_patch_bundle`, and `apply_patch_bundle` remain only
because they are now the primary names of the schema-2 API, not compatibility
aliases.

## Patch Schema 2

### Patch AWFB layout

The AWFB bundle kind stays `Patch`; the required section kind stays `PatchPlan`.
The PatchPlan section schema version is now `2`. Changed embedded section
payloads continue to be carried as `AssetBlob` sections because this is already
allowed by the patch container policy and preserves raw section-kind semantics.

### Patch manifest

`BundlePatchManifest` now contains:

- `schema_version = 2` and `min_reader_schema_version = 2`;
- `runtime_abi: RuntimeAbiRange`;
- `base_artifact: ArtifactIdentity` and `target_artifact: ArtifactIdentity`;
- `base_content_root` and `target_content_root` for fast inspection and request
  compatibility;
- aggregate `compatibility: PatchCompatibility`;
- `materialization: PatchMaterializationContract`;
- ordered `compatibility_fingerprints: Vec<SectionCompatibilityFingerprint>`.

The identity fields bind bundle kind, content root, and manifest digest. A patch
that changes only the manifest must still carry a new `target_artifact` even if
section descriptors are unchanged.

### PatchPlan section

The section payload is:

```rust
struct PatchPlanSection {
    schema_version: u32,
    plan: BundlePatchPlan,
    target_manifest_bytes: Option<Vec<u8>>,
}
```

`target_manifest_bytes` is present when the target artifact identity has a
manifest digest different from the base manifest digest. Core materialization
checks the digest before encoding target bytes.

### Materialization contract

```rust
struct PatchMaterializationContract {
    descriptor_merge: PatchDescriptorMergeMode::ReplaceBySectionId,
    manifest_rewrite: PatchManifestRewrite,
    external_descriptor_policy: PatchExternalDescriptorPolicy::MetadataOnlyAllowed,
    target_signature: PatchTargetSignaturePolicy::StripBaseSignature,
}
```

The target emitted by core materialization is unsigned. Adapters may sign the
returned bytes later, but core never preserves the base signature or accesses
signing keys.

## Compatibility Fingerprints

Every operation has one `SectionCompatibilityFingerprint` with:

- `id`, operation kind, raw section kind code, optional known kind, and required
  flag;
- operation compatibility and derivation source;
- descriptor fingerprints for base/target sides when present;
- content digest fingerprints for base/target sides when present.

The aggregate label is the maximum severity across fingerprints.

### Derivation rules

| Source | Rule |
| --- | --- |
| Runtime compact codec | Use `migrated_runtime_section_compatibility` for `RuntimeTypes`, `Entrypoints`, and `AdapterRequirements`. |
| Product catalog compact codec | Decode compact `ContentCatalog`, `AssetCatalog`, `DisplayCatalog`, `SourceMap`, and `AudioGraph`; classify as `content-only` after successful decode. |
| ProgramBytecode / AWBC | Decode canonical AWBC; ABI change or removed existing function is `restart-required`; existing function interface change is `code-generational`; body-only change or added function is `code-compatible`; byte-identical executable is `content-only`. |
| External descriptor change | Preserve metadata-only descriptor mutation; use the owning section kind default compatibility. |
| Unknown optional kind | Preserve raw section kind code; replacement defaults to `content-only`; removal remains `restart-required`. |
| Unknown required kind | Already rejected by AWFB container parsing. |
| Non-migrated known family | Use `BundleSectionKind::patch_default_compatibility`, implemented on the owning enum. |

## Target Materialization State Machine

1. `Planned`: artifact decoded or constructed.
2. `BaseValidated`: active content root and full base `ArtifactIdentity` match.
3. `DescriptorsMerged`: remove/replace/add operations are applied by section id;
   old digests and duplicate ids are checked.
4. `ManifestRewritten`: either base manifest bytes are reused or target manifest
   bytes are validated against `target_artifact.manifest_digest`.
5. `TargetEncoded`: AWFB target bytes are encoded with no signature block.
6. `TargetValidated`: target content root and full `ArtifactIdentity` match the
   patch manifest.
7. `Materialized`: bytes and `PatchMaterializationReport` are returned.

Failures return before replacing adapter-owned active bytes. Native/player code
keeps rollback by assigning `base_awfb_bytes` only after session apply/restart
succeeds.

## Crate Ownership

| Area | Owner | Change |
| --- | --- | --- |
| Patch schema and materialization | `arcweft-bundle::patch` | Direct schema-2 replacement; no alternate module. |
| Section default compatibility | `arcweft-bundle::container::BundleSectionKind` | Adds `patch_default_compatibility` on the owning enum. |
| Product section inverse mapping | `arcweft-bundle::resource_codec::kind::ProductSectionCodecKind` | Adds `from_section_kind` on the owning enum. |
| Product catalog fingerprints | `arcweft-bundle::resource_codec::product_catalog` | Adds decode-backed content compatibility API. |
| Runtime swap declared labels | `arcweft-runtime-driver::swap::SwapCompatibility` and `SwapSession` | Adds conversion from patch label and `prepare_with_compatibility`. |
| Session patch apply | `arcweft-runtime-driver::session` | Materializes schema-2 targets and passes declared compatibility to swap. |
| Native endpoint | `arcweft-player-native::patch_endpoint` | Applies/restarts from materialized target without local compatibility heuristics. |

## Implementation Cuts

1. Replace `crates/arcweft-bundle/src/patch.rs` with the schema-2 implementation.
2. Add owning-type behavior to `BundleSectionKind` and `ProductSectionCodecKind`.
3. Add product catalog compatibility decoding.
4. Add runtime swap declared-compatibility preparation.
5. Update session patch apply to consume `readiness.compatibility` and use
   `PatchMaterializedTarget.bytes`.
6. Update native endpoint to remove local live-apply heuristics and rely on
   runtime-driver patch apply/restart result.
7. Add focused tests and source gates.

## Tests

Added tests:

- `patch_schema_two_is_the_only_decoded_schema`
- `patch_bytes_are_deterministic_and_round_trip_schema_two`
- `materialization_rewrites_manifest_and_reports_unsigned_target_identity`
- `missing_target_manifest_rolls_back_before_target_encoding`
- `external_descriptor_change_is_metadata_only_and_preserved`
- `unknown_optional_section_kind_is_preserved_through_patch`
- `patch_session_path_consumes_declared_patch_compatibility_without_reclassifying`

Additional focused tests to add after applying to a full checkout:

- AWBC body-only change is `code-compatible`.
- AWBC existing function interface change is `code-generational`.
- Runtime type / entrypoint / adapter requirement compact fixtures preserve the
  seq-02.2 compatibility table.
- Product catalog compact decode failure prevents a compatibility label.
- Signature-policy decode verifies the patch artifact before materialization and
  does not imply target signature validity.

## Commands

```bash
./scripts/apply-overlay.sh /path/to/Sanzentyo/arcweft
cd /path/to/Sanzentyo/arcweft
cargo fmt --all -- --check
cargo test -p arcweft-bundle --test patch_schema --all-features -- --nocapture
cargo test -p arcweft-runtime-driver --test patch_source_gate --all-features -- --nocapture
cargo test -p arcweft-player-native native_patch_endpoint --all-features -- --nocapture
cargo check -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Relationship to Seq3

Seq3 windowed live patch should consume `PatchCompatibility` from the decoded
patch manifest and the per-section fingerprints as authoritative. It should not
rederive patch compatibility from runtime/player-local matches. Seq3 may still
validate whether the current runtime window can honor a declared label, but that
is an execution readiness decision, not artifact classification.

## Remaining Boundaries

- AWFR release archive/fetch-cache policy remains seq-02.6.
- Signing key access and target signature generation remain adapter responsibilities.
- Shader/View/entity/contracts/graph-index compact codecs remain future work until
  their section-family designs are concrete.

## Applied Validation

This checkout applied the direct replacement overlay from
`arcweft-seq-02-5-patch-v2-direct-replacement-package.zip` and adapted the
existing runtime-driver/native tests to build schema-2 artifacts with explicit
manifests and compatibility fingerprints.

Validation run:

- `cargo test -p arcweft-bundle --test patch_schema --all-features -- --nocapture`
- `cargo test -p arcweft-runtime-driver --test patch_source_gate --all-features -- --nocapture`
- `cargo test -p arcweft-player-native native_patch_endpoint --all-features -- --nocapture`
- `cargo check -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-native --all-targets --all-features`
- `cargo check -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-native -p arcweft-cli --all-targets --all-features`
- `cargo clippy -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-native -p arcweft-cli --all-targets --all-features -- -D warnings`
- `cargo test -p arcweft-cli --test regression_harness --quiet`
- `cargo test -p arcweft-cli run_bundle_applies_awfb_patch_before_execution --all-features -- --nocapture`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`
- `git diff --check`
- `just test-workspace`

Additional cut-point validation is recorded in the commit response.
