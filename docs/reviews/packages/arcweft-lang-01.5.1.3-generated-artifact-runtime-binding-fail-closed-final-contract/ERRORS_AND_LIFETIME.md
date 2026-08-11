# Error taxonomy, mismatch order, and lifetime rules

## 1. Exact machine codes

| Code | Meaning | Raised by |
|---|---|---|
| `runtime-binding-missing` | selected, correctly typed requirement has no registered slot | catalog resolve at runtime attempt |
| `runtime-binding-stale` | profile/source revision or process-local LSP generation does not own the catalog/claim | registration, resolution, or LSP lease gate |
| `runtime-binding-mismatch` | selected ID and kind are valid, but the complete claimed key differs | registration before mutation |
| `runtime-binding-unselected` | ID is outside the selected product / no requirement was projected | registration or resolution |
| `runtime-binding-kind-mismatch` | function API used for Activity slot or vice versa | registration or resolution |
| `runtime-binding-duplicate` | a valid slot is already registered | registration before replacement |
| `runtime-binding-product-invalid` | key/product/schema/canonical order/plan-product invariant fails | construction, decode, or compiler verification |

Every public error family exposes `code()`. Behavior tests assert the typed variant and exact code, not Display prose.

## 2. Precedence

### Registration

1. product validity (constructor guarantee; defensive check at external decode boundary);
2. claimed topology profile;
3. claimed topology source-set revision;
4. selected ID;
5. expected/actual kind;
6. structural key fields in the fixed order below;
7. duplicate slot;
8. mutate slot.

Topology differences return stale, not mismatch. A stale claim cannot reveal a misleading missing/duplicate result from a newer product.

### Resolution

1. active LSP generation lease, when applicable;
2. active topology profile;
3. active topology source-set revision;
4. selected ID;
5. requested kind;
6. slot presence;
7. return borrowed binding.

No key comparison occurs during ordinary resolution because registration fixed the exact key to the immutable product slot. No fallback search occurs after missing.

## 3. Fixed structural mismatch order

`GeneratedArtifactBindingKey::correlate(expected, claimed)` returns the first differing typed field in this order:

1. import ID;
2. module mount;
3. metadata path;
4. metadata raw digest;
5. import visibility;
6. import demand;
7. metadata source document ID;
8. metadata source revision;
9. metadata source length;
10. package ID;
11. package version;
12. module ID;
13. target family;
14. target ABI;
15. Rust target triple, WASM world, or process transport;
16. metadata ABI hash;
17. metadata payload hash;
18. artifact path;
19. artifact raw digest;
20. artifact size;
21. export kind;
22. function name;
23. function visibility;
24. function ordered parameters;
25. function return type;
26. function purity;
27. function ordered effects;
28. Activity export ID;
29. Activity visibility;
30. Activity identity (the invariant pair of selected abstract and metadata Activity IDs);
31. selected `ActivityImplementationId`;
32. Activity interface hash;
33. Activity state hash.

Topology fields are checked before this method and reported as stale. Function-only and Activity-only fields are compared only after the export kind agrees. Because every constructible Activity key enforces `abstract_activity == export.activity_id`, correlation reports one typed `ActivityIdentity` mismatch for that pair; an internally inconsistent pair is product/claim invalid and never a second unreachable mismatch. A family/detail inconsistency is likewise an invalid claim/product construction or decode error. A coherent alternate family reaches `TargetFamily`; a syntactically valid wrong ABI/transport in the same family reaches the corresponding typed mismatch.

## 4. Typed mismatch enum

The mismatch enum has one variant per row or one family-specific nested enum preserving the same distinctions. It stores typed expected and actual values. Collections such as parameters/effects are retained as their typed vectors/slices in the error payload or a typed owned summary; they are not flattened into a debug string.

At minimum the public API can distinguish:

- `ImportId`, `Mount`, `MetadataPath`, `MetadataRawHash`, `ImportVisibility`, `ImportDemand`;
- `MetadataDocumentId`, `MetadataDocumentRevision`, `MetadataSourceLength`;
- `PackageId`, `PackageVersion`, `ModuleId`;
- `TargetFamily`, `TargetAbi`, `RustTargetTriple`, `WasmWorld`, `ProcessTransport`;
- `MetadataAbiHash`, `MetadataPayloadHash`;
- `ArtifactPath`, `ArtifactHash`, `ArtifactSize`;
- `ExportKind`;
- every function and Activity field listed above, including the invariant `ActivityIdentity` pair and `ActivityImplementationId`.

This supports exact field-level test assertions and host diagnostics without string matching.

## 5. Product-invalid reasons

`GeneratedArtifactBindingProductError` includes typed cases for:

- invalid format/schema;
- invalid ABI/transport spelling;
- target family/detail mismatch;
- ABI/transport marker not equal to the current accepted Arcweft owner marker;
- Activity abstract/metadata identity mismatch;
- missing/duplicate/non-canonical Activity selection or selection/requirement implementation mismatch;
- envelope/key topology mismatch;
- non-canonical requirement order;
- duplicate/conflicting canonical anchor;
- non-contiguous/duplicate requirement ID;
- requirement count/ID overflow;
- nested typed codec failure;
- runtime plan missing requirement;
- runtime plan wrong requirement kind;
- runtime plan/product topology mismatch.

Serde may surface nested parse errors through its normal error channel, while programmatic construction returns the typed product errors.

## 6. Lifetime rules

### Product/catalog lifetime

- A catalog owns an `Arc` to exactly one selected-profile launch product; a no-profile `None` context has no catalog.
- IDs are valid only through that product.
- A catalog is immutable after `freeze()`.
- There is no slot replacement, hot patch, merge, parent catalog, fallback catalog, or last-known-good chain.
- Rebinding requires constructing a new builder/catalog from a product.

### Topology revision

- An old product/catalog used with a new `SourceSetRevision` is stale even if all IDs and visible names match.
- A metadata overlay changes `SourceDocumentIdentity`, metadata raw digest, and source-set revision; any one of the product checks is sufficient to prevent reuse, and all remain in evidence.
- The runtime checks topology before missing, so an old empty slot cannot mask stale ownership.

### LSP generation

- `AcceptedEnvironmentGeneration` is a process-local lease only.
- It is not serialized or used to make IDs globally stable.
- `AcceptedProfileEnvironment` owns generation + compiled project + product + catalog lease atomically.
- Replacing the environment always creates a new lease and catalog registration opportunity; no old binding is copied even for equal source bytes.
- A request carrying the old generation fails stale before catalog access.

### Non-LSP hosts

A native/web/bundle host that does not use LSP still supplies the active topology identity from the compiled launch context to every catalog resolve. It must replace the launch context/catalog together when loading a new compiled project.

## 7. No partial work

An error is observable only as the structured failure and diagnostics. On all resolution failures:

- no function callback/provider method is called;
- no host task/request is enqueued;
- no scheduler state is advanced for that external attempt;
- no Activity instance/state/registry entry is committed;
- no start/action/host event is emitted;
- no catalog slot is mutated.

Tests snapshot or count each relevant state owner before and after the failure.
