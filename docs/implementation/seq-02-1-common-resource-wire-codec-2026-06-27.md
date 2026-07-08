# Seq-02.1 Common Resource Wire Codec and Product Resource Codec Plan

Request source: `docs/reviews/requests/2026-06-27-seq-02.1-common-resource-wire-codec-and-resource-codec-plan.md`

This note records the concrete design and implementation cut for the common
compact product resource wire substrate. It does not migrate any individual
section-family product payload away from typed JSON; it freezes the shared
contract that seq-02.2 and later must reuse.

## Current Repository Evidence Inspected

The uploaded overlay package was produced from GitHub connector evidence on
`main` at commit `346f3ac06a75247a870dcf45a478095f754341a0`
(`Add seq-02 follow-up request splits`, `2026-06-27T18:37:02+09:00`). Before
and after applying the overlay, the current local checkout was rechecked and
validated with Cargo.

Inspected files:

- `AGENTS.md`.
- `docs/README.md` and `docs/00-overview/architecture.md`.
- `crates/arcweft-bundle/src/lib.rs`.
- `crates/arcweft-bundle/src/resource_codec.rs`.
- `crates/arcweft-bundle/src/container.rs`.
- `crates/arcweft-bundle/src/container/opaque.rs`.
- `crates/arcweft-bundle/src/product.rs`.
- `crates/arcweft-bundle/src/patch.rs`.
- `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`.
- `docs/implementation/seq-02-product-artifact-patch-signing-first-cut-2026-06-27.md`.

Findings:

- AWFB v1 fixed descriptors, section kind codes, placement, compression,
  decoded/stored sizes, digests, bounded reads, content roots, and artifact
  identity are already implemented and must be preserved.
- Unknown optional section kinds are preserved as raw `SectionKindCode`; unknown
  required kinds are rejected. The common resource codec therefore must not hide
  or reinterpret AWFB section-kind policy.
- Product `ProgramBytecode` is already the canonical AWBC v1 executable payload.
  This slice does not redesign AWBC.
- `product.rs` still emits typed JSON for runtime-types, entrypoints, adapter
  requirements, content catalog, display catalog, and normalized source.
- The existing `resource_codec.rs` contained initial budget/table/header ideas,
  but it did not yet own field envelopes, shared reference newtypes, enum
  registries, inspection export, source gates, or a cross-family migration plan.

## Common Resource Wire Schema

Every migrated product resource section payload is decoded only after AWFB
container validation. The AWFB descriptor remains the section identity source;
the compact resource payload starts with a common 48-byte little-endian header:

```text
offset  size  field
0       8     codec-family magic, e.g. AWRT\r\n\x1a\n
8       4     common resource schema version, currently 1
12      4     ProductSectionCodecKind numeric tag
16      4     string table entry count
20      4     public-id table entry count
24      4     enum registry entry count
28      4     common field count
32      4     section-family record count
36      4     reserved, must be zero
40      8     common body length in bytes
```

The common body has no padding. AWFB still aligns section payload starts. The
body order is fixed:

1. canonical string table;
2. canonical public-id table;
3. canonical enum registry;
4. canonical common field stream.

All integer lengths and tags are little-endian fixed-width values. This first
contract intentionally does not introduce varints or codec probing. The decoder
must be called with the expected `ProductSectionCodecKind`, and the header tag
must match that expectation.

### Table layout

String and public-id table entries are encoded as:

```text
u32 utf8_byte_len
[u8; utf8_byte_len]
```

The string table is sorted ascending by UTF-8 byte string and deduplicated by the
owning typed API. Decoding rejects duplicate or non-canonical string entries.
The public-id table is sorted ascending and rejects duplicates rather than
silently collapsing them, so stable external references remain auditable.

Enum registry entries are encoded as pairs:

```text
u32 enum_code
u32 name_string_id
```

Enum codes are canonical ascending and duplicate codes are rejected. Names must
resolve through the common string table.

### Field envelope

Each common field has a 12-byte header followed by raw section-family payload:

```text
offset  size  field
0       2     FieldId
2       1     ResourceWireType
3       1     flags bit 0 = required, all other bits zero
4       2     declared nesting depth
6       2     declared reference count
8       4     payload byte length
12      N     payload bytes
```

Known field contracts are supplied by the owning section-family codec through a
`FieldRegistry`. Unknown optional fields are skipped and counted. Unknown
required fields are rejected. Known required fields must be present. Known field
wire-type mismatches are rejected before the family-specific decoder interprets
the payload bytes.

### Shared encodings

The common module owns these cross-family primitives:

- `StringId(u32)` — index into the common string table.
- `PublicIdRef(u32)` — index into the common public-id table.
- `StableId([u8; 16])` — deterministic 128-bit identity derived from an
  owner-defined stable key using the current bundle digest primitive.
- `DigestRef { digest: BundleDigest }` — reference to content-addressed bytes.
- `SourceRangeRef { source: PublicIdRef, start_byte: u32, end_byte: u32 }` —
  UTF-8 byte range in a normalized source/public-id resource.
- `CrossSectionRef { section_kind: SectionKindCode, section_id: SectionId,
  content_digest: BundleDigest, public_id: Option<PublicIdRef> }` — raw-kind
  cross-section reference that preserves unknown optional section identity.

The common field `ResourceWireType` registry is shared. Section-family codecs
may define their own field ids and enum domains, but they must encode strings,
public ids, stable ids, digests, source ranges, and cross-section references
through these common types.

## Decoder Budget Model

All compact resource-family decoders must expose and pass a `SectionCodecBudget`:

| Category | Field | Common enforcement |
| --- | --- | --- |
| Byte length | `bytes` | Entire compact resource payload length after AWFB decoding. |
| Item count | `items` | Common fields and enum symbols. |
| Record count | `records` | Section-family record count declared in the header. |
| String count | `strings` | String table entry count. |
| String bytes | `string_bytes` | Aggregate UTF-8 bytes for string and public-id tables. |
| Public-id count | `public_ids` | Public-id table entry count. |
| Nesting depth | `depth` | Maximum declared field nesting depth. |
| Reference count | `references` | Aggregate declared field references. |
| Table fan-out | `table_fan_out` | Aggregate strings + public ids + enum symbols + fields. |

The common module enforces syntactic/resource budgets. Semantic budgets that
require domain knowledge, such as graph reachability, runtime ABI compatibility,
shader backend constraints, audio device policies, or adapter capabilities, stay
with compiler/runtime/player adapters.

## Crate and Module Ownership Map

| Owner | Responsibility |
| --- | --- |
| `arcweft-bundle::container` | AWFB v1 container, descriptors, raw section-kind preservation, content root, artifact identity, external payload digest checks. |
| `arcweft-bundle::resource_codec` | Common compact resource envelope, shared tables, shared references, common budgets, inspection export, source gate fixtures. |
| `arcweft-bundle::product` | Current product AWFB composition and typed JSON temporary sections until the owning seq-02.x family migrates. No probing. |
| `arcweft-bundle::patch` | Current patch diff/materialization and conservative compatibility classification using section descriptors and codec kind behavior. |
| `arcweft-runtime-plan` | Runtime semantic lowering and future runtime-types/entrypoints/adapter section-family typed APIs. |
| `arcweft-project-loader` | Fetch/cache/manifest behavior for product artifacts and releases; no common resource byte interpretation unless mediated by bundle APIs. |
| CLI/player adapters | Filesystem/network/clock/cache/signing and platform-specific semantic validation. |

## Migration Matrix

| Family | Current evidence/status | Seq-02.1 status | Migration owner | Notes |
| --- | --- | --- | --- | --- |
| Runtime types | Existing AWFB `RuntimeTypes`, typed JSON. | Compact-first | seq-02.2 | First family after common wire; validates runtime layout shape but not VM semantics. |
| Entrypoints | Existing AWFB `Entrypoints`, typed JSON. | Compact-first | seq-02.2 | Must use public-id table and no string fallback. |
| Adapter requirements | Existing AWFB `AdapterRequirements`, typed JSON. | Compact-first | seq-02.2 | Bundle validates structure; adapters validate actual host support. |
| Content catalog | Existing AWFB `ContentCatalog`, typed JSON. | JSON-temporary | seq-02.3 | Shares public-id, digest, and source-range references. |
| Presentation/display catalog | Existing AWFB `DisplayCatalog`, typed JSON. | JSON-temporary | seq-02.3 | Presentation schema owns family fields; no private text tables. |
| Asset catalog | Existing AWFB `AssetCatalog` kind, not product-owned yet. | JSON-temporary | seq-02.3 | Asset blobs stay separate AWFB sections; catalog references digest/section refs. |
| Entity resources | No current AWFB kind. | Future | seq-02.3 | Requires new container section kind or explicit carrier decision. |
| Source maps | Existing AWFB `SourceMap` kind. | JSON-temporary | seq-02.3 | Source ranges use common `SourceRangeRef`; normalized source remains inspection/debug oriented. |
| Locale/text | Existing `LocaleCatalog` kind. | JSON-temporary | seq-02.3 | Must reuse common string/public-id tables while allowing family-specific text shaping records. |
| Shader resources | No current AWFB kind. | Future | seq-02.4 | Needs section kind/carrier decision before compact payload is active. |
| View resources | No current AWFB kind. | Future | seq-02.4 | Needs split between native View and HTML/DOM View families. |
| Audio graph | Existing AWFB `AudioGraph`, typed JSON in content catalog today. | JSON-temporary | seq-02.4 | Bundle validates graph bytes; host validates backend capabilities/device policies. |
| Debug symbols | Existing AWFB `DebugSymbols` kind. | JSON-temporary | seq-02.4 | Inspection/export heavy; may remain optional/on-demand. |
| Contracts | No current AWFB kind. | Future | seq-02.4 | Restart-required/code identity interaction must be explicit. |
| Graph indexes | No current AWFB kind. | Future | seq-02.4 | Code-compatible by default but depends on owner schema. |

## Migrated/Unmigrated Coexistence

During migration, each known AWFB section family has exactly one declared payload
contract for a given schema version. Product decoding must choose by descriptor
kind plus schema/manifest contract, not by probing bytes. A migrated family must
reject typed JSON product fallback for that family. An unmigrated family may stay
typed JSON only because the migration matrix still marks it `JSON-temporary`.

Unknown optional AWFB sections are still preserved by the container and remain
opaque to product resource decoding. Unknown required AWFB sections still fail at
container validation.

## Inspection and Export Policy

JSON remains a human-facing export/inspection representation. It is generated
from compact bytes through `ProductResourceEnvelope::inspection()` or
`inspection_json_bytes()`. It is not accepted as an alternate product resource
codec once a family migrates.

Round-trip tests must go:

```text
owning typed API -> compact common envelope bytes -> decoded envelope -> JSON inspection view
                                                   -> owning typed API
```

They must not go through product JSON fallback.

## Golden Fixture Conventions

Later seq-02.x requests must reuse these fixture conventions:

- keep common wire fixtures tiny and schema-neutral;
- name fixtures by codec family, schema version, and expected outcome;
- store expected bytes as deterministic hex or byte arrays, not pretty JSON;
- include duplicate table, unknown optional field, unknown required field, budget
  failure, canonical digest, and inspection-view cases;
- include the expected `ProductSectionCodecKind`, never a probing decoder;
- include source-gate coverage when a new section-family codec is introduced;
- when a family migrates, keep a negative test proving product JSON fallback is
  rejected for that family.

## Implementation Cuts

Implemented by this overlay:

1. Split `arcweft-bundle::resource_codec` into responsibility modules.
2. Added common 48-byte header and expected-codec decode API.
3. Added shared string/public-id/enum tables with canonical ordering.
4. Added shared stable-id, digest, source-range, and cross-section references.
5. Added common field envelope, field registry, unknown optional skip, unknown
   required rejection, required-presence validation, and field wire-type checks.
6. Added shared budget categories and enforcement points.
7. Added inspection/export JSON generated from compact envelope bytes/typed APIs.
8. Added schema-neutral common codec integration tests.
9. Added source gate preventing ad hoc private resource table formats outside the
   common module.
10. Added migration matrix and seq-02.x dependency boundaries in this note.

## Test Matrix

| Requirement | Test coverage |
| --- | --- |
| Deterministic bytes for common table ordering | `common_wire_bytes_are_deterministic_for_canonical_table_ordering` |
| Duplicate string/public-id handling | `duplicate_table_entries_are_rejected_when_decoding_non_canonical_bytes`, `public_id_table_rejects_duplicates_without_deduplicating` |
| Unknown optional skip / unknown required reject | `unknown_optional_fields_skip_and_unknown_required_fields_reject` |
| Budget failures | `budgets_fail_for_byte_count_string_and_item_limits`, `budgets_fail_for_depth_reference_and_fanout_limits` |
| Canonical digest stability | `canonical_digest_is_stable_for_equivalent_logical_resources` |
| Inspection/export round-trip through typed API | `inspection_json_round_trips_through_typed_owner_api_not_product_json_fallback` |
| No ad hoc table formats | `resource_codec_source_gate.rs` |

## Dependencies for seq-02.2 through seq-02.8

- seq-02.2 must reuse `ProductResourceEnvelope`, `FieldRegistry`, `StringTable`,
  `PublicIdTable`, `EnumRegistry`, and `SectionCodecBudget` for runtime-types,
  entrypoints, and adapter requirements. It must update `product.rs` so those
  migrated families reject product JSON fallback.
- seq-02.3 must reuse the same common tables/references for content,
  presentation/display, asset catalog, entity, source maps, and locale/text. It
  must not invent per-family public-id or source-range encodings.
- seq-02.4 must decide any missing AWFB section kinds/carriers before shader,
  UI, audio, debug, contracts, and graph-index payloads become active compact
  product sections.
- seq-02.5 patch v2 must consume `ProductSectionCodecKind::patch_compatibility()`
  and descriptor-level content digests rather than parsing private resource
  payloads in the patch layer.
- seq-02.6 AWFR/external carrier work must reference sections through AWFB
  descriptors, `SectionId`, raw `SectionKindCode`, and `BundleDigest`; it must
  not require embedded payloads.
- seq-02.7 signing policy may consume content roots, artifact identity, and
  compact resource digests, but signing keys and policy checks stay out of
  `arcweft-bundle` data-format code.
- seq-02.8 overlay production application may apply generated code only after it
  is reconciled with this common contract and the source gate.

## Verification

Overlay package checks completed before repository application:

- file inventory checked;
- overlay Rust line counts measured;
- no overlay production Rust file crosses AGENTS structural warning thresholds;
- ZIP archive integrity checked with `zip -T`.

Repository validation run after applying the overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle resource_codec --all-features -- --nocapture
cargo test -p arcweft-bundle --test resource_codec_common --all-features -- --nocapture
cargo test -p arcweft-bundle --test resource_codec_source_gate --all-features -- --nocapture
cargo check -p arcweft-bundle --all-targets --all-features
cargo clippy -p arcweft-bundle --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

`cargo test -p arcweft-bundle resource_codec --all-features -- --nocapture`
matches only tests whose names include `resource_codec`; the full schema-neutral
fixture suite was therefore also run explicitly with
`--test resource_codec_common`.

The structure audit scanned 1545 files and 850 Rust files, with 415277 Rust
physical LOC. It reported 0 errors and 105 existing warning-level hotspots.

## Structural Audit Notes

Repository state measured at Jujutsu change `xrlsypyo`.

| Path | Bytes | LOC | Kind | Embedded test LOC | Responsibilities |
| --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-bundle/src/resource_codec.rs` | 1211 | 30 | production facade module | 0 | public module boundary and deliberate common resource codec re-exports |
| `crates/arcweft-bundle/src/resource_codec/budget.rs` | 1624 | 49 | production responsibility module | 0 | shared byte/item/record/string/reference/depth/fan-out budget categories |
| `crates/arcweft-bundle/src/resource_codec/codec_io.rs` | 2504 | 66 | production private module | 0 | little-endian cursor and checked length conversion helpers |
| `crates/arcweft-bundle/src/resource_codec/error.rs` | 2597 | 59 | production responsibility module | 0 | structured common resource codec diagnostics |
| `crates/arcweft-bundle/src/resource_codec/field.rs` | 8627 | 262 | production responsibility module | 0 | field ids, wire types, requirement flags, registries, field encode/decode, field budgets |
| `crates/arcweft-bundle/src/resource_codec/header.rs` | 5705 | 153 | production responsibility module | 0 | fixed 48-byte resource header encode/decode and validation |
| `crates/arcweft-bundle/src/resource_codec/inspection.rs` | 3198 | 86 | production responsibility module | 0 | human-facing JSON inspection view generated from compact envelopes |
| `crates/arcweft-bundle/src/resource_codec/kind.rs` | 7396 | 187 | production responsibility module | 0 | product section codec enum, migration status, AWFB kind mapping, patch compatibility behavior |
| `crates/arcweft-bundle/src/resource_codec/table.rs` | 10443 | 302 | production responsibility module | 0 | canonical string table, public-id table, enum registry encode/decode |
| `crates/arcweft-bundle/src/resource_codec/types.rs` | 1746 | 54 | production responsibility module | 0 | stable id, digest, source-range, and cross-section reference types |
| `crates/arcweft-bundle/src/resource_codec/wire.rs` | 9986 | 252 | production responsibility module | 0 | canonical envelope encode/decode, expected-codec decode, unknown-field handling, digest |
| `crates/arcweft-bundle/tests/resource_codec_common.rs` | 15440 | 461 | integration test | 0 | schema-neutral common wire, budget, unknown-field, digest, and inspection tests |
| `crates/arcweft-bundle/tests/resource_codec_source_gate.rs` | 2145 | 60 | integration test | 0 | source gate against private resource table/reference formats outside the common module |

No changed production Rust file crosses the 1200 LOC warning threshold. No
changed integration test crosses the 2500 LOC warning threshold.

Largest workspace Rust files at this cut are unchanged seq-independent
hotspots:

| Path | Bytes | LOC | Note |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12394 | generated-like vertical orientation table |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255424 | 7445 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 225209 | 5838 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 5760 | integration fixture suite |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 209852 | 5285 | integration fixture suite |

## Non-Goals Preserved

- No section-family runtime-types/entrypoints/adapter/content/presentation/etc.
  codec is implemented in this slice.
- No AWBC product executable redesign.
- No codec probing.
- No product JSON fallback for migrated future families.
- No filesystem, network, clocks, signing keys, or cache access added to
  `arcweft-bundle`.
