# Ownership and dependency map

## Responsibility map

| Responsibility | Owner | Reason |
| --- | --- | --- |
| package IDs/versions, RawDigest/SemanticDigest, canonical JSON | `arcweft-manifest-model` | current typed owners already exist |
| resource identities, schemas, descriptors, constants, registry validation/digests | `arcweft-resource-model` | accepted immutable semantic substrate |
| source documents/ranges/spans/revisions | `arcweft-source` | current source authority |
| strict resource-manifest DTO/decode/encode/source map/conversion/publication orchestration | new `arcweft-resource-manifest` | Sans-I/O data-format boundary; avoids parser/filesystem pollution in model |
| root project-manifest path declaration | `arcweft-launch` | current strict project manifest owner |
| explicit path/package discovery and UTF-8 byte admission | `arcweft-project-loader` | filesystem/resolver adapter owner |
| immutable registry handoff to compilation | existing compiler project registration | already carries `Arc<ResourceTypeRegistry>` |
| AWFB section kind/framing/read budgets | `arcweft-bundle` | current typed container authority |
| runtime reconstruction with engine base registry | runtime bundle-loading adapter | owns runtime I/O/object graph, calls Sans-I/O codecs |

`arcweft-resource-model` does not gain JSON, filesystem, package discovery, or bundle framing. `arcweft-resource-manifest` does not enumerate directories or read paths.

## New crate dependencies

Add `arcweft-resource-manifest` as a workspace member and workspace dependency. Its direct dependencies are exact current workspace entries:

```toml
[dependencies]
arcweft-core.workspace = true
arcweft-id.workspace = true
arcweft-interaction-model.workspace = true
arcweft-layout.workspace = true
arcweft-manifest-model.workspace = true
arcweft-resource-model.workspace = true
arcweft-source.workspace = true
json-spanned-value.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

Direct domain dependencies are necessary because typed conversion constructs current `LocaleId`, `LogicalDuration`, `GainDbMilli`, `PanMilli`, `LayoutUnit`, `EntityId`, and `PublicId`. `serde_json` remains an implementation detail for canonical projection/number support and never crosses the semantic API. No new third-party dependency is required; pinned workspace main already provides every version.

## Existing crate edge changes

| Crate | Added edge/change |
| --- | --- |
| workspace root | member and `arcweft-resource-manifest` path dependency |
| `arcweft-resource-model` | inherent descriptor digest API; inherent `codecs()` iterator only |
| `arcweft-launch` | top-level optional `resource_type_manifest: Option<NormalizedProjectPath>`; serialized TOML key `resource-type-manifest` |
| `arcweft-project-loader` | normal dependencies on `arcweft-resource-manifest` and `arcweft-resource-model`; explicit resource-manifest topology kind/seed; base registry input and accepted registry output |
| `arcweft-compiler` | consume loader-published registry rather than separately supplied empty/default registry at affected package path; no semantic dependency inversion |
| `arcweft-bundle` | dependencies on resource-manifest/model for section framing and verification; new section enum variant/code 22 |
| runtime bundle loader | supply engine base registry, decode required section through bundle + resource-manifest APIs, verify final registry digest |

## Forbidden dependency directions

- `arcweft-resource-model -> arcweft-resource-manifest`: forbidden.
- `arcweft-resource-manifest -> arcweft-project-loader` or filesystem crates: forbidden.
- `arcweft-resource-manifest -> arcweft-bundle`: forbidden; bundle may depend downward on manifest codec, not vice versa.
- source syntax/HIR/sema -> JSON DTO labels: forbidden; both consume typed registry/model APIs.
- package loader -> private lexical JSON node types: forbidden; loader sees public source-backed typed results only.

## Architecture validation

Validate edges through `cargo metadata --format-version 1 --no-deps` parsed as structured JSON and through compile-time public API tests. Do not grep `Cargo.toml`, Rust source, symbols, or file paths. The structural audit is required because a new crate, workspace edges, and a public wire contract are introduced.
