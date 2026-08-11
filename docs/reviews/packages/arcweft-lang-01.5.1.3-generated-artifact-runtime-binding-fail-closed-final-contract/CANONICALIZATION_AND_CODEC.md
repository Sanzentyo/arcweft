# Exact-key canonicalization and codec contract

## 1. Participating facts

| Group | Exact typed fields | Why present |
|---|---|---|
| Topology | selected `ProfileId`, `SourceSetRevision` | rejects prior profile/revision catalogs before lookup |
| Import | import ID, mount, metadata path, metadata raw digest, visibility, demand | binds manifest selection and mount without name/path fallback |
| Metadata source | `SourceDocumentIdentity`, metadata ABI hash, payload hash | binds exact accepted source revision and validated semantic payload |
| Target | family, typed ABI, triple/world/transport | distinguishes Rust/WASM/process and exact ABI/detail |
| Package/module | package ID, version, module ID | prevents package/module/version substitution |
| Artifact | normalized path, raw digest, size | prevents basename/path-only or rebuilt artifact substitution |
| Function export | complete `AdapterFunctionExport` | prevents same-name signature/purity/effect replacement |
| Activity export | abstract Activity ID, selected `ActivityImplementationId`, and complete `AdapterActivityExport` | prevents implementation/export/Activity/interface/state replacement |

The key deliberately does not copy the whole metadata envelope. Exact metadata raw bytes, source identity, payload hash, and topology revision pin generator provenance, requirements, format/schema, and all other exports. This avoids per-export duplication without permitting reuse after any document change.

## 2. No aggregate-key digest

Do not introduce `GeneratedArtifactBindingKeyHash` as an authority in this split. Existing typed digests remain fields, but the key itself is compared structurally. A derived digest may be used only as an internal performance accelerator after an equality check remains authoritative; no such accelerator is required for the bounded fixed-slot design.

## 3. Canonical anchor

For each valid key, derive an internal typed anchor:

```text
(import_id, mount, kind_rank, export_identity, optional_implementation_id, optional_abstract_activity)
```

- Function `export_identity` is `FunctionName`.
- Activity `export_identity` is `AdapterExportId`; `optional_implementation_id` is the selected `ActivityImplementationId`; and `optional_abstract_activity` is the selected `ActivityId`.
- Kind ranks are fixed: Function 0, Activity 1.
- Ordering uses the types' canonical `Ord`; it never uses debug formatting or locale-sensitive comparison.

The product builder sorts by the anchor and rejects duplicate anchors. It assigns ID `u32(index)` after sorting. Input iteration order, map implementation, filesystem order, and metadata export iteration order cannot affect IDs.

A changed key with the same anchor may receive the same ordinal in a new product. The topology identity is therefore required for every registration and resolution.

## 4. Product wire schema 1

Illustrative canonical JSON shape (values are examples, not fixtures):

```json
{
  "format": "arcweft.generated-artifact-bindings",
  "schema": 1,
  "topology": {
    "profile": "game",
    "source_set_revision": "<64 lowercase hex>"
  },
  "requirements": [
    {
      "id": 0,
      "key": {
        "topology": { "profile": "game", "source_set_revision": "<same>" },
        "import": {
          "id": "dialogue-gen",
          "mount": "generated.dialogue",
          "metadata_path": "generated/dialogue.adapter.json",
          "metadata_raw_hash": "<typed raw digest wire>",
          "visibility": "private-or-current-enum-wire",
          "demand": "current-demand-wire"
        },
        "metadata": {
          "document": {
            "id": "<logical source id>",
            "revision": "<64 lowercase hex>",
            "source_len": 1234
          },
          "abi_hash": "<typed semantic digest wire>",
          "payload_hash": "<typed semantic digest wire>"
        },
        "target": {
          "family": "rust",
          "abi": "arcweft-rust-v1",
          "detail": { "kind": "rust", "target_triple": "<typed triple>" }
        },
        "package": { "id": "example.generated", "version": "1.2.3" },
        "module": { "id": "dialogue" },
        "artifact": {
          "path": "target/generated/dialogue.bin",
          "size": 4096,
          "hash": "<typed raw digest wire>"
        },
        "export": {
          "kind": "function",
          "export": { "name": "speak", "visibility": "public", "params": [], "return": "()", "purity": "effectful", "effects": ["dialogue"] }
        }
      }
    }
  ],
  "activity_selections": []
}
```

Exact nested field spellings follow the final serde annotations in `RUST_API_SHAPES.md` and existing Arcweft wire types. Implementation tests must snapshot the finalized schema. The illustrative visibility/demand/digest spellings above do not override their current owning codecs.

## 5. Strict decode invariants

Deserialization must reject:

1. wrong format;
2. schema other than 1;
3. unknown fields at every new product/key struct or enum level;
4. invalid ABI/transport newtypes;
5. target family/detail mismatch;
6. an ABI marker that is syntactically valid but not the exact current `RustAbi`/`WasmAbi`/`ProcessAbi` marker for the family;
7. a process transport that is not the exact current `ProcessTransport` marker;
8. Activity abstract/metadata identity mismatch;
9. missing, duplicate, non-canonical, or cross-inconsistent Activity selection;
10. Activity selection implementation ID differing from its requirement;
11. requirement key topology differing from envelope topology;
12. non-contiguous or duplicate IDs;
13. requirement count beyond `u32` representation;
14. duplicate/conflicting anchors;
15. non-canonical requirement order;
16. any existing nested typed identity/digest/path parse failure.

Decode does not sort, fill defaults, accept aliases, translate legacy fields, or reconstruct identities from strings.

## 6. Runtime-plan codec

`RuntimeCallTarget::GeneratedArtifact(id)` and `RuntimeFunctionBody::GeneratedArtifact(id)` use the existing runtime-plan/value codec framework. Public tests round-trip both variants and all ID boundaries. The compiler's cross-product verifier accepts `None` only when the plan/launch contains no generated ID or Activity selection. With `Some(product)`, it rejects a decoded ID that is absent from the product or points to an Activity requirement and verifies every Activity selection against its exact requirement.

The plan does not contain the full key. The launch product does not contain runtime expressions. Their correlation is the typed ID plus common topology checked by compiler/runtime assembly. A selected profile with zero requirements serializes a real empty product; a no-profile compile carries no product and never serializes a fabricated topology.

## 7. Catalog non-serialization

`GeneratedArtifactBindingCatalogBuilder`, `GeneratedArtifactBindingCatalog`, slots, and host bindings implement neither `Serialize` nor `Deserialize`. Bundles/save data contain requirements, not live host pointers, callbacks, handles, libraries, WASM instances, process objects, or provider state.
