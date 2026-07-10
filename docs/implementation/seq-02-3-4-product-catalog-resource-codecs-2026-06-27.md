# Seq-02.3/02.4 Product Catalog Resource Codecs

Packages:

- `arcweft-seq-02-3-content-presentation-entity-resource-codecs-package.zip`
- `arcweft-seq-02-4-audio-graph-resource-codec-package.zip`

Review input: attached review text for seq-02.2 through seq-02.4 packages.

## Integration Decision

The packages were not applied as blind overlays. The review correctly identified
that both packages assumed a different common resource API
(`ProductResourceSection`, `ResourceRecord`, `ResourceFieldValue`, and related
types) and that seq-02.4 wanted full-file `product.rs` / `Cargo.toml`
replacement. This cut instead rebases the implementation onto the actual
seq-02.1/02.2 API:

- `ProductResourceEnvelope`
- `ResourceField`
- `FieldRegistry`
- `StringTable`
- `PublicIdTable`
- `EnumRegistry`

## Implemented Scope

Implemented migrated product resource families:

- `ContentCatalog`
- `AssetCatalog`
- `DisplayCatalog`
- `SourceMap`
- `AudioGraph`

These families now have Sans I/O owner codecs in
`arcweft-bundle::resource_codec::product_catalog`. Product AWFB encode/decode
uses those compact resource envelopes instead of typed JSON product section
fallbacks.

The current `ArcweftBundle` model does not yet expose lowered dialogue content,
presentation graph, entity graph, shader, UI, debug-symbol, contract, or locale
resource models. The implemented sections therefore preserve current product
truth:

| Family | Current product truth encoded |
| --- | --- |
| `ContentCatalog` | required empty catalog marker; no unsupported dialogue/entity projection invented |
| `AssetCatalog` | `BundleVirtualFile` and `BundleImageAsset` |
| `DisplayCatalog` | `LineDisplayCatalog` and `BundleImageObject` |
| `SourceMap` | `BundleSource` |
| `AudioGraph` | `AudioGraph` |

Each family is wrapped in a compact `ProductResourceEnvelope` with a required
typed transcript field. The transcript is owned by the family codec and decoded
only through the compact section owner; product AWFB no longer has a generic
JSON section fallback for these migrated families.

## Product AWFB Changes

`crates/arcweft-bundle/src/product.rs` now:

- emits compact `ContentCatalog`, `DisplayCatalog`, and `SourceMap` sections;
- emits optional compact `AssetCatalog` when virtual files or image assets are
  present;
- emits optional compact `AudioGraph` when `ArcweftBundle.audio` is present;
- decodes the same migrated families through compact owner codecs;
- no longer emits or decodes legacy `NormalizedSource` product JSON;
- no longer carries `ContentCatalogSection.audio`.

As of the 2026-07-10 inventory cleanup, `ProductSectionCodecKind::ALL` contains
only implemented compact families. `LocaleText`, `DebugSymbols`, Shader,
Contracts, Entity, and GraphIndex are not codec variants until an owning compact
codec is implemented; migration planning belongs in implementation notes, not
the runtime enum.

## Review Issues Addressed

- The invalid seq-02.4 patches were not used.
- `product.rs` and `Cargo.toml` were not replaced wholesale.
- The implementation uses the actual current `ProductResourceEnvelope` API.
- `EntityGraph` was not added as an ad hoc section kind. There is no stable
  `BundleSectionKind`, product data, or placeholder codec variant for it.
- Migrated catalog families are verified through their typed decode behavior;
  the temporary source-spelling gates from this historical cut were removed.

## Tests Added

- `product_catalog_compact_codecs_round_trip_current_bundle_resources`
- `product_awfb_uses_compact_sections_for_migrated_catalog_families`
- `product_catalog_unknown_optional_fields_skip_and_unknown_required_reject`
- `product_catalog_common_budget_failures_are_reported`
- `migrated_product_catalog_families_do_not_use_product_json_fallback`

## Validation

Applied at repository change `mqqumzmmkuywnkvxwsxywqtzzpmnxtqo`
(`87f2a76b6a5b` before description/commit finalization). Final validation run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle --test product_catalog_resource_codecs --all-features -- --nocapture
cargo test -p arcweft-bundle --test product_catalog_source_gate --all-features -- --nocapture
cargo test -p arcweft-bundle --all-features
cargo check -p arcweft-bundle -p arcweft-audio-core -p arcweft-interaction-model --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-audio-core -p arcweft-interaction-model --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands passed. The structural audit reported `0` errors and `106`
workspace warning-level findings.

## Structural Audit Notes

Changed Rust file measurements at the validation point:

| Path | Crate | Kind | Bytes | LOC | Embedded test LOC | Responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-bundle/src/resource_codec/product_catalog.rs` | `arcweft-bundle` | production | 13,892 | 451 | 0 | compact owner codecs for ContentCatalog, AssetCatalog, DisplayCatalog, SourceMap, and AudioGraph |
| `crates/arcweft-bundle/src/resource_codec.rs` | `arcweft-bundle` | facade | 1,755 | 47 | 0 | resource codec module exports |
| `crates/arcweft-bundle/src/resource_codec/kind.rs` | `arcweft-bundle` | production | 7,431 | 199 | 0 | product codec kind section mapping and migration status |
| `crates/arcweft-bundle/src/product.rs` | `arcweft-bundle` | production plus unit tests | 27,975 | 749 | 263 | product AWFB encode/decode wiring and fixture helpers |
| `crates/arcweft-bundle/tests/product_catalog_resource_codecs.rs` | `arcweft-bundle` | integration test | 12,286 | 344 | 0 | compact catalog round-trip, product section, budget, and unknown-field tests |
| `crates/arcweft-bundle/tests/product_catalog_source_gate.rs` | `arcweft-bundle` | integration test | 1,191 | 36 | 0 | product JSON fallback deletion gate |

No changed production file crosses the AGENTS.md warning threshold. The
pre-existing seq-02.2 `resource_codec/runtime.rs` warning-level size remains
unchanged by this cut.

## Remaining Follow-Up

- Split the compact product catalog transcript fields into finer family-specific
  fields once the product model exposes lowered dialogue/content/entity data.
- Design whether entity work should use `Entity`, `GraphIndex`, or a new
  `BundleSectionKind` before implementing the package's `EntityGraph` concept.
- Implement shader, UI, locale/text, debug-symbol, and contract codecs in later
  seq-02.x cuts.
- Patch v2, AWFR, external payload carrier redesign, and signing-policy redesign
  remain separate later goals.
