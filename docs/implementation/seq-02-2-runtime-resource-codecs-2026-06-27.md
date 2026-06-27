# Seq-02.2 Runtime Types / Entrypoints / Adapter Requirements Compact Codecs

Package: `arcweft-seq-02-2-runtime-resource-codecs-package.zip`

This implementation assumes seq-02.1 is already applied: `arcweft-bundle::resource_codec`
exposes the common compact resource wire envelope, string/public-id/enum tables,
field registry, unknown optional/required field policy, inspection view, and the
migration matrix. This cut does not redesign AWFB v1, unknown optional section
preservation, artifact identity, or the canonical AWBC executable payload.

## Current Evidence Inspected

- `AGENTS.md`: data-format crates stay Sans I/O, owned enum/boundary behavior
  should live on the owning type, implementation state belongs under
  `docs/implementation/`, and each task ends with changed files, tests, TODOs,
  and deviations.
- `docs/README.md` and `docs/00-overview/architecture.md`: Arcweft keeps the
  bundle/bytecode/data-format layer Sans I/O and leaves filesystem/network/host
  execution to adapters.
- `crates/arcweft-bundle/src/resource_codec.rs`: seq-02.1 common codec contract
  is the substrate for this cut.
- `crates/arcweft-bundle/src/product.rs`: current product AWFB writes
  `RuntimeTypes`, `Entrypoints`, and `AdapterRequirements` as typed JSON and
  reads them through `required_payload::<T>`.
- `crates/arcweft-bundle/src/patch.rs`: patch compatibility currently classifies
  runtime sections conservatively from section kind only.
- `crates/arcweft-runtime-driver/src/swap.rs`: generation identity currently
  computes adapter requirements through a serde JSON fingerprint of bundle
  manifest/adapter fields.
- `crates/arcweft-runtime-driver/src/session.rs`: session construction consumes
  `ArcweftBundle` fields after product AWFB decoding and maps generation
  fingerprint errors into session errors.
- `crates/arcweft-core/src/bytecode.rs` and `crates/arcweft-core/src/awbc/schema.rs`:
  the runtime layout and AWBC type/function/entrypoint tables are the runtime
  facts available to compact resource sections in this cut.

## Scope

Implemented section families:

1. `RuntimeTypes`
2. `Entrypoints`
3. `AdapterRequirements`

Non-goals preserved from the request:

- content, presentation, shader, UI, entity, audio, debug, and contract codecs;
- patch v2 file-format redesign beyond compatibility fingerprints for these
  migrated sections;
- AWFR archive or signing-policy redesign;
- generic resource JSON escape hatches.

## Ownership and Dependency Map

| Area | Owner | Responsibility |
| --- | --- | --- |
| Common wire envelope | `arcweft-bundle::resource_codec` seq-02.1 | fixed header, common tables, field registry, budgets, unknown field policy |
| Runtime section schemas | `arcweft-bundle::resource_codec::runtime` | typed Sans I/O data, canonical bytes, decode budgets, inspection/export typed values, compatibility fingerprints |
| Product AWFB encode/decode | `crates/arcweft-bundle/src/product.rs` | emit and require compact runtime sections; preserve AWBC executable; materialize `ArcweftBundle` fields from compact decode output |
| Patch section compatibility | `arcweft-bundle::patch` plus `resource_codec::runtime` | decode old/new compact migrated runtime section payloads during patch artifact construction and classify replacements semantically without patch v2 redesign |
| Runtime-driver generation identity | `arcweft-runtime-driver::swap` | use compact adapter requirement canonical digest instead of serde JSON adapter fingerprint |
| Player/session loading | `arcweft-runtime-driver::session` | continues to consume `ArcweftBundle`; product decode has already required compact runtime sections |

## Canonical Schemas

All three sections are encoded as a seq-02.1 `ProductResourceEnvelope` with:

- codec-specific magic from `ProductSectionCodecKind`;
- schema version `PRODUCT_SECTION_SCHEMA_VERSION`;
- sorted `StringTable` and duplicate-rejecting `PublicIdTable`;
- shared enum registry for human inspection;
- required fields only for v1 schemas;
- unknown optional field skip and unknown required field rejection inherited from
  `ProductResourceEnvelope::decode_with_registry`.

### RuntimeTypes v1

Required fields:

| Field ID | Wire type | Payload |
| ---: | --- | --- |
| `1` | `U32` | bytecode/AWBC runtime ABI version |
| `2` | `StringRef` | runtime layout signature string |
| `3` | `Bytes` | runtime type declaration table |
| `4` | `Bytes` | function interface fingerprint table |

Runtime type declaration table:

```text
u32 count
repeat count:
  u32 public_id_ref_or_u32_max
  u32 RuntimeValueKind
  u32 TypeCompatibilityLabel
  u8[32] layout_digest
```

Function interface table:

```text
u32 count
repeat count:
  u32 public_id_ref_or_u32_max
  u32 awbc_function_index
  u32 RuntimeFunctionKind
  u32 awbc_function_flags
  u32 TypeCompatibilityLabel
  u8[32] signature_digest
  u8[32] frame_layout_digest
```

The section is cross-checked against the decoded AWBC program in product decode:
function indices must fit the AWBC function table. Full semantic checking of
source types and compiler diagnostics stays outside `arcweft-bundle`.

### Entrypoints v1

Required fields:

| Field ID | Wire type | Payload |
| ---: | --- | --- |
| `1` | `Bytes` | entrypoint declaration table |

Entry table:

```text
u32 count
repeat count:
  u32 public_id_ref
  u32 exported_name_string_ref_or_u32_max
  u32 awbc_function_index_or_u32_max
  u32 InitialStateRequirement
  u32 ProductVisibility
  u32 source_public_id_string_ref_or_u32_max
  u32 source_start_byte
  u32 source_end_byte
```

Product decode validates that the manifest-selected entry is present, and that
AWBC function bindings are in range when a product AWBC payload exists.

### AdapterRequirements v1

Required fields:

| Field ID | Wire type | Payload |
| ---: | --- | --- |
| `1` | `Bytes` | adapter requirement records |

Adapter records preserve the current product contract exactly while adding typed
capability shapes for release policy and later platform adapters:

```text
u32 default_adapter_string_ref_or_u32_max
public_id_list adapter_manifest_ids
public_id_list required_host_calls
u32 adapter_manifest_count
repeat adapter_manifest_count:
  u32 adapter_manifest_id_public_ref
  u32 display_name_string_ref
  public_id_list effects
  u32 host_call_count
  repeat host_call_count:
    u32 host_call_id_public_ref
    public_id_list effects
capability_list required_capabilities
capability_list optional_capabilities
string_ref_list feature_flags
u32 launch_constraint_count
repeat launch_constraint_count:
  u32 launch_constraint_public_ref
  u32 required_bool
u32 platform_requirement_count
repeat platform_requirement_count:
  u32 platform_string_ref
  u32 requirement_public_ref
```

A `capability_list` is:

```text
u32 count
repeat count:
  u32 capability_public_ref
  u32 min_version_string_ref_or_u32_max
  u32 max_version_string_ref_or_u32_max
  string_ref_list feature_flags
```

`required_capabilities` are currently derived from `required_host_calls`,
`optional_capabilities` are derived from adapter manifest effects, and
`feature_flags`, `launch_constraints`, and `platform_refs` are empty when
produced from the current bundle model. They are still part of the v1 bytes so
release policy and platform-specific requirements do not need a private future
wire table.

## Decoder Budgets

`RuntimeResourceBudget` wraps the seq-02.1 `SectionCodecBudget` and adds family
limits:

| Budget | Default |
| --- | ---: |
| common bytes | seq-02.1 default |
| common records/items | `262_144` for this runtime cut |
| common string bytes | `16 MiB` |
| runtime types | `262_144` |
| function interfaces | `262_144` |
| entrypoints | `65_536` |
| adapter requirement ids | `262_144` |
| adapter manifests | `65_536` |
| host calls | `262_144` |

Budget failures surface as structured `SectionCodecError::BudgetExceeded(<name>)`.

## Patch Compatibility Fingerprints

The new `resource_codec::runtime::migrated_runtime_section_compatibility(kind,
old_bytes, new_bytes)` decodes compact bytes and returns a semantic
`RuntimeResourceCompatibility`. It maps to `PatchCompatibility` through
`RuntimeResourceCompatibility::patch_compatibility()`.

Rules:

| Section | Change | Classification |
| --- | --- | --- |
| RuntimeTypes | identical | `content-only` |
| RuntimeTypes | ABI/layout signature change | `restart-required` |
| RuntimeTypes | removal of an existing type/function interface | `restart-required` |
| RuntimeTypes | added declaration/function with compatible label | `code-compatible` |
| RuntimeTypes | changed declaration/function | max of changed item labels (`code-compatible`, `code-generational`, or `restart-required`) |
| Entrypoints | identical | `content-only` |
| Entrypoints | added entrypoint | `code-compatible` |
| Entrypoints | removed entrypoint, changed function binding, visibility, or initial-state requirement | `restart-required` |
| AdapterRequirements | identical | `content-only` |
| AdapterRequirements | optional capability, feature flag, or adapter manifest metadata-only change | `code-compatible` |
| AdapterRequirements | default adapter, required host call, required capability, launch constraint, or platform requirement change | `restart-required` |

Patch v1 file format remains unchanged. `BundlePatchArtifact::from_views` now
uses the runtime compatibility API when both old and new decoded payload bytes
are available for migrated runtime sections. Patch v2 can later carry these
fingerprints explicitly without redesigning the section codecs.

## Product AWFB Migration

The patch changes product AWFB behavior as follows:

- `to_awfb_bytes` emits compact `RuntimeTypes`, `Entrypoints`, and
  `AdapterRequirements` sections using `resource_codec::runtime`.
- `from_awfb_slice_with_external_sections` requires compact runtime sections and
  no longer calls `required_payload::<RuntimeTypesSection>`,
  `required_payload::<EntrypointsSection>`, or
  `required_payload::<AdapterRequirementsSection>`.
- The decoded compact adapter requirements rehydrate
  `manifest.adapter`, `manifest.adapter_manifest_ids`,
  `manifest.required_host_calls`, and `adapter_manifests`.
- Content/display/source sections remain typed JSON in this slice because their
  seq-02.3+ schemas are non-goals.
- Product JSON/TOML/YAML/MessagePack/CBOR/Avro exports remain human-facing
  bundle export paths, not product AWFB resource fallbacks.

## Deletion Gates for Old Typed JSON Product Payloads

The old private product structs are removed from `product.rs` after parity tests
pass:

- `RuntimeTypesSection`
- `EntrypointsSection`
- `AdapterRequirementsSection`

A source gate test rejects their reintroduction and rejects the old
`encode_json(&RuntimeTypesSection...)` / `required_payload::<RuntimeTypesSection>`
patterns for all three migrated families. Fixture source values may still be
constructed as `ArcweftBundle` data and then encoded through the compact owner
APIs; no product AWFB fallback may decode runtime resource JSON.

## Implementation Cuts

1. Add `resource_codec::runtime` typed schemas and codecs.
2. Add deterministic tiny fixture tests for runtime types, entrypoints, and
   adapter requirements.
3. Add decode budget tests and unknown optional/required field tests.
4. Add compatibility fingerprint tests for compatible, generational, and restart
   changes.
5. Wire product AWFB encode/decode to compact runtime sections.
6. Wire runtime-driver adapter generation identity to compact adapter digest.
7. Add source gate proving migrated runtime JSON fallback is deleted.
8. Keep content/display/source resource JSON unchanged for seq-02.3+.

## Focused Tests Added

- `runtime_types_compact_bytes_are_deterministic_and_round_trip`
- `entrypoints_compact_bytes_are_deterministic_and_round_trip`
- `adapter_requirements_compact_bytes_round_trip_without_json_fallback`
- `runtime_resource_budget_failures_are_reported_by_family`
- `unknown_optional_fields_are_skipped_and_unknown_required_fields_reject`
- `patch_compatibility_fingerprints_classify_runtime_resource_changes`
- `patch_artifact_from_views_uses_compact_runtime_compatibility`
- `migrated_runtime_section_compatibility_decodes_compact_bytes`
- `migrated_runtime_sections_do_not_use_product_json_fallback`

Suggested focused commands after applying the package:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle runtime_resource --all-features -- --nocapture
cargo test -p arcweft-bundle migrated_runtime_sections_do_not_use_product_json_fallback --all-features -- --nocapture
cargo check -p arcweft-bundle -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

## Validation State in This Repository

Applied to repository change `unqrlruouxntmxtpnmlmrupkssqqnlot`
(`a80490f7d4f3` before description/commit finalization). Validation run after
applying and adapting the package:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle runtime_resource --all-features -- --nocapture
cargo test -p arcweft-bundle migrated_runtime_sections_do_not_use_product_json_fallback --all-features -- --nocapture
cargo test -p arcweft-bundle --test runtime_resource_codecs --all-features -- --nocapture
cargo check -p arcweft-bundle -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-runtime-driver --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands passed. The structural audit reported `0` errors and `106`
pre-existing or warning-level findings across the workspace. A broader
`cargo test -p arcweft-bundle --all-features` pass also passed after updating
legacy bundle/patch fixtures to encode migrated runtime sections through the
compact owners.

## Structural Audit Notes

Changed Rust file measurements at the validation point:

| Path | Crate | Kind | Bytes | LOC | Embedded test LOC | Responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-bundle/src/resource_codec/runtime.rs` | `arcweft-bundle` | production | 70,781 | 2,018 | 0 | runtime resource section schemas, canonical encode/decode, budgets, and semantic compatibility |
| `crates/arcweft-bundle/src/resource_codec.rs` | `arcweft-bundle` | facade | 1,427 | 39 | 0 | common resource codec module exports and compact runtime owner re-exports |
| `crates/arcweft-bundle/src/product.rs` | `arcweft-bundle` | production plus unit tests | 25,410 | 680 | 255 | product AWFB compact runtime section encode/decode and product gates |
| `crates/arcweft-bundle/src/patch.rs` | `arcweft-bundle` | production plus unit tests | 52,611 | 1,386 | 514 | patch materialization and migrated runtime semantic compatibility classification |
| `crates/arcweft-bundle/src/lib.rs` | `arcweft-bundle` | facade plus unit tests | 66,142 | 1,867 | 620 | bundle facade, typed bundle export/import tests, compact runtime fixture helpers |
| `crates/arcweft-bundle/tests/runtime_resource_codecs.rs` | `arcweft-bundle` | integration test | 13,859 | 383 | 0 | deterministic bytes, round-trip, budget, unknown field, and patch compatibility tests |
| `crates/arcweft-bundle/tests/runtime_resource_product_gate.rs` | `arcweft-bundle` | integration test | 904 | 26 | 0 | migrated runtime product JSON fallback source gate |
| `crates/arcweft-runtime-driver/src/swap.rs` | `arcweft-runtime-driver` | production plus unit tests | 29,278 | 886 | 310 | runtime hot-swap generation identity and compact adapter requirement digesting |
| `crates/arcweft-runtime-driver/src/session.rs` | `arcweft-runtime-driver` | production | 27,987 | 773 | 0 | session loading and generation fingerprint diagnostics |

The main new warning-level hotspot is
`crates/arcweft-bundle/src/resource_codec/runtime.rs` at 2,018 LOC. It remains
below the production error threshold but above the preferred responsibility
module range. It is kept cohesive in this cut because the three migrated
families share the seq-02.1 common envelope glue, budget model, digest helpers,
and semantic compatibility helpers; splitting during initial application would
have made the first production migration harder to review. The expected split
points remain `runtime/types.rs`, `runtime/type_section.rs`,
`runtime/entrypoints.rs`, and `runtime/adapter_requirements.rs` once the next
section-family migrations prove which helpers should stay common.

Largest workspace Rust files at audit time were unchanged by this cut and are
existing hotspots headed by
`crates/arcweft-text-layout/src/vertical_orientation.rs` (12,400 LOC),
`crates/arcweft-cli/tests/check/cli_runtime_bench.rs` (7,946 LOC), and
native observe integration fixtures between 4,000 and 6,300 LOC.

## Remaining TODOs

- Split `resource_codec/runtime.rs` after the next migrated section-family cut
  if common helpers and family-specific code paths are stable enough to make the
  split lower-risk.
- Content, presentation, shader, UI, entity, audio, debug, and contract codecs
  remain seq-02.3+ work.
- Patch v2, AWFR, external payload carrier redesign, and signing policy still
  belong to later seq-02.x goals.
