# Resource extension-manifest V1 wire schema

The normative machine schema is `schema/resource-type-manifest-v1.schema.json`. This document closes semantic rules that JSON Schema cannot express completely.

## Lexical transport

- Encoding: UTF-8 without BOM.
- Top-level JSON value: object.
- Trailing non-whitespace bytes: error.
- JSON number tokens: integers only; fractions/exponents are never accepted.
- JSON `null`: never accepted.
- Nonstandard `NaN`, `Infinity`, comments, and trailing commas: syntax error.
- Duplicate object keys: error before any last-value selection.
- Unpaired Unicode surrogate escape: invalid string.
- Object fields are closed at every level.

## Root record

```rust
pub struct ResourceTypeManifestFileV1 {
    pub format: ResourceTypeManifestFormatV1, // exact marker
    pub schema: ResourceTypeManifestSchemaV1, // exact integer 1
    pub package: PackageCoordinateFile,
    pub schemas: Vec<ResourceValueSchemaFileV1>,
    pub resource_types: Vec<ResourceTypeDescriptorFileV1>,
    pub codecs: Vec<ResourceCodecSupportFileV1>,
}
```

All six fields are required. Unknown root fields are errors. Arrays may be empty. One document may publish multiple types.

## `PackageCoordinateFile`

```json
{ "id": "org.example.package", "version": "1.2.3" }
```

`id` constructs current `PackageId`; `version` constructs current `PackageVersion`. The canonical encoder writes `PackageId::as_str()` and `PackageVersion::to_string()`. No other normalization is introduced.

## Typed identities

| Semantic type | Wire |
| --- | --- |
| `NominalTypeId` / `ResourceTypeId` | `{ "package": PackageId, "module": ResourceModulePath, "name": ResourceTypeName }` |
| `ResourceSchemaId` and other stable text IDs | JSON string using the current constructor |
| field/variant/schema/codec/section IDs and versions | JSON integer `1..=u32::MAX` |
| `EntityId` | JSON string through `EntityId::try_new` |
| `PublicId` | JSON string through `PublicId::try_new` |
| digest | exact `blake3:` plus 64 lowercase hex digits |

Nominal identities are always fully qualified. No `self`, local, package alias, combined `package::module.Type` string, or version-bearing nominal type spelling exists in V1.

## Scalar type tags

The content of a `ResourceValueTypeFileV1 { kind: "scalar" }` is one of:

`unit`, `bool`, `signed_integer`, `unsigned_integer`, `float`, `string`, `char`, `duration`, `ratio`, `length`, `gain`, `pan`, `locale`, `public_id`.

## Scalar constant values

Scalar constants are wrapped as:

```json
{ "kind": "scalar", "value": { "kind": "string", "value": "text" } }
```

Exact inner forms:

| Scalar kind | Content |
| --- | --- |
| `unit` | no `value` member |
| `bool` | JSON boolean |
| `signed_integer` | canonical JSON integer in `i64` range |
| `unsigned_integer` | canonical JSON integer in `u64` range |
| `float` | string `^0x[0-9a-f]{16}$`; finite binary64 bits; negative zero rejected |
| `string` | JSON string |
| `char` | JSON string containing exactly one Unicode scalar |
| `duration` | `u64` nanoseconds |
| `ratio` | integer `0..=1_000_000` millionths |
| `length` | `{ "milli_units": i64, "unit": LayoutUnitToken }` |
| `gain` | integer `-120000..=24000` |
| `pan` | integer `-1000..=1000` |
| `locale` | JSON string through `LocaleId::try_new`; canonical casing on encode |
| `public_id` | JSON string through `PublicId::try_new` |

`LayoutUnitToken` is exactly `px`, `sp`, `percent`, `vw`, `vh`, `cw`, `ch`, `em`, `glyph_ch`, `safe_area_top`, `safe_area_right`, `safe_area_bottom`, or `safe_area_left`.

Integer lexical spelling follows JSON's integer grammar and adds rejection of `-0`. Leading zeroes, `+`, fractions, and exponents are malformed. Overflow is reported separately from malformed syntax.

## `ResourceValueTypeFileV1`

Every variant is an adjacent-tagged closed object with `kind` and, where listed, required `value`:

| Rust variant | `kind` | `value` |
| --- | --- | --- |
| `Scalar` | `scalar` | scalar type token |
| `Option` | `option` | nested value type |
| `Vec` | `list` | nested value type |
| `NonEmptyVec` | `non_empty_list` | nested value type |
| `Map` | `ordered_map` | `{ "key": type, "value": type }` |
| `NominalRecord` | `record` | `ResourceSchemaId` string |
| `NominalEnum` | `enum` | `ResourceSchemaId` string |
| `AssetRef` | `asset_ref` | `{ "payload_kind": ResourceAssetPayloadKindId }` |
| `ResourceRef` | `resource_ref` | `{ "type_id": NominalTypeIdFile }` |
| `RetainedIdentityRef` | `retained_identity_ref` | retained token |
| `ConstrainedScalar` | `constrained_scalar` | constraint record |

Constraint record:

```json
{
  "scalar": "signed_integer",
  "lower": { "kind": "inclusive", "value": { "kind": "signed_integer", "value": 0 } },
  "upper": { "kind": "exclusive", "value": { "kind": "signed_integer", "value": 10 } }
}
```

`lower` and `upper` are independently optional and absent rather than null. Their scalar value kind must equal `scalar`. Existing `ResourceScalarConstraint::try_new` rejects mismatched, inverted, and empty ranges.

## `ResourceConstValueFileV1`

| Rust variant | `kind` | Content |
| --- | --- | --- |
| `Scalar` | `scalar` | required scalar-value object |
| `Option(None)` | `option` | no `value` member |
| `Option(Some)` | `option` | nested constant in `value` |
| `Sequence` | `list` | array of nested constants; authored order is semantic |
| `Map` | `ordered_map` | array of `{key,value}` entries |
| `Record` | `record` | `{schema_id, fields:[{field_id,value}]}` |
| `Enum` | `enum` | `{schema_id, variant_id, payload?}` |
| `AssetRef` | `asset_ref` | `{public_id, payload_kind}` |
| `ResourceRef` | `resource_ref` | `{entity_id, public_id, type_id}` |
| `RetainedIdentityRef` | `retained_identity_ref` | exact resolved retained record |

Map entries are represented as an array because constant keys are typed values, not strings. Duplicate semantic keys are rejected. Input order is ignored; canonical order is ascending canonical wire bytes of the normalized key. Record field input order is ignored and duplicate `field_id` is rejected; canonical order is numeric `field_id`. Enum payload is absent for unit variants and present for payload variants.

## Retained identity constants

Exact adjacent tags:

```json
{ "kind": "character", "value": { "entity_id": "entity.character.alice" } }
{ "kind": "view",      "value": { "entity_id": "entity.view.main" } }
{ "kind": "action",    "value": { "entity_id": "entity.action.open" } }
{ "kind": "layer",     "value": { "entity_id": "entity.layer.front" } }
{ "kind": "signal",    "value": { "entity_id": "entity.signal.ready" } }
```

Presentation target:

```json
{
  "kind": "presentation_target",
  "value": {
    "scope": { "kind": "global" },
    "target_id": "presentation.global"
  }
}
```

or View-scoped:

```json
{
  "kind": "presentation_target",
  "value": {
    "scope": { "kind": "view", "value": { "owner_view_entity_id": "entity.view.main" } },
    "target_id": "presentation.dialogue"
  }
}
```

Scroll region:

```json
{
  "kind": "scroll_region",
  "value": {
    "owner_view_entity_id": "entity.view.main",
    "region_id": "scroll.dialogue"
  }
}
```

No retained form admits `ResourceRefValue` fields or an inferred owner.

## No byte value in V1

The current closed Rust model has no `Bytes` scalar/type/constant variant. Consequently:

- there is no `bytes` kind;
- there is no base64/hex byte content spelling;
- `list<unsigned_integer>` remains an ordinary typed list and is not reinterpreted;
- invalid byte-encoding tests assert rejection of the attempted unknown byte tag;
- asset payload bytes remain outside this document and are referenced by `AssetRef`.

Adding bytes later requires a real resource-model variant and a new schema contract; it must not be smuggled into V1.

## Schemas

Record schema:

```json
{
  "kind": "record",
  "value": {
    "schema_id": "org.example.record",
    "nominal_type": { "package": "org.example", "module": "m", "name": "Record" },
    "version": 1,
    "fields": [
      {
        "field_id": 1,
        "name": "value",
        "value_type": { "kind": "scalar", "value": "string" },
        "presence": "optional",
        "default": { "kind": "scalar", "value": { "kind": "string", "value": "x" } },
        "docs": "optional documentation"
      }
    ]
  }
}
```

Enum schema uses `kind: "enum"` and `variants:[{variant_id,name,payload?,docs?}]`. Empty docs are semantically absent and omitted canonically. Required fields may not carry a default; optional fields may omit a default.

## Descriptors and codecs

Descriptor record:

```json
{
  "type_id": { "package": "org.example", "module": "m", "name": "Record" },
  "public_id_family": "record",
  "family_group": "org.example",
  "body_schema": "org.example.record",
  "capabilities": {
    "runtime_handle_kind": "org.example.handle",
    "agent_exposure": "catalog_and_runtime",
    "save_definition_reference": true,
    "hot_reload": "update_live_handle"
  },
  "lowering": {
    "codec_id": "org.example.codec",
    "codec_version": 1,
    "section_id": "org.example.section",
    "section_version": 1
  },
  "docs": { "summary": "optional" },
  "descriptor_digest": "blake3:...64 lowercase hex..."
}
```

`runtime_handle_kind` and `docs` are absent rather than null. Exact enum tokens are:

- exposure: `hidden`, `catalog`, `catalog_and_runtime`;
- hot reload: `restart_required`, `replace_definition`, `update_live_handle`.

Codec support is `{ "codec_id": string, "versions": [nonzero u32, ...] }`. Versions must be nonempty and unique; canonical order is ascending numeric.

## Package ownership and references

- every descriptor `type_id.package` equals the document package id;
- every locally published schema `nominal_type.package` equals the document package id;
- provenance package is derived from the selected coordinate, never authored;
- provenance source is derived from the owner-qualified logical path and record index;
- same-file and same-package forward references resolve after all selected manifests are lowered;
- cross-package references require the target in the supplied base or selected dependency registry;
- the aggregate admits only one selected version per package id;
- asset/entity target existence remains the later asset/declaration catalog's responsibility, while payload/type/category equality is enforced here.

## Canonical ordering

| Collection | Canonical order |
| --- | --- |
| root object and all objects | UTF-8 bytes of field name |
| `schemas` | `schema_id` UTF-8 bytes |
| `resource_types` | package, module, name UTF-8 bytes |
| `codecs` | `codec_id` UTF-8 bytes |
| record fields | `field_id`, then name |
| enum variants | `variant_id`, then name |
| codec versions | numeric ascending |
| record constant fields | numeric `field_id` |
| ordered-map entries | canonical normalized key bytes |
| lists/sequences | authored order preserved |

The encoder omits absent optionals and empty docs, emits no trailing newline, and always recomputes descriptor digest claims.
