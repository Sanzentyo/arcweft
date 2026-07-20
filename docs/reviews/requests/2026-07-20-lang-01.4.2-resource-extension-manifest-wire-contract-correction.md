# Lang-01.4.2 — Resource extension-manifest wire contract correction

## Sequence position

This is a contract-correction branch from Lang-01.4. It follows the accepted
`res` surface and generic typed resource registry substrate. It may be designed
in parallel with Lang-01.4.1, but both corrections must be incorporated before
the public extension-manifest decoder, canonical encoder, bundle publication,
or third-party resource-type loading is implemented.

## Why this correction is required

The Lang-01.4 package names `ResourceTypeManifestFileV1`, lists the semantic
information that it must carry, and requires tagged structural records
corresponding to the resource-model enums. It does not define a complete,
independently implementable wire shape.

The package and current repository documentation do not settle:

- the exact top-level document shape and format;
- the exact `PackageCoordinateFile` representation;
- enum tag and content field names;
- canonical spellings for every `ResourceValueType`,
  `ResourceConstValue`, field requirement, constraint, capability, codec, and
  compatibility variant;
- integer, finite-float, identifier, byte-string, map, and nested-value
  encodings;
- duplicate-key and duplicate-record handling;
- unknown-field policy at each record boundary;
- canonical ordering and serialization;
- source-range ownership for nested decoding failures; or
- version selection and rejection behavior.

Implementing a decoder now would therefore freeze guessed public spellings and
canonical bytes. That would conflict with the repository rule that unpublished
formats move directly to one final model without dual readers, aliases, or
migration shims.

## Required decisions

1. Select the manifest transport format and define the exact
   `ResourceTypeManifestFileV1` top-level record.
2. Define the exact neutral wire record for `PackageCoordinateFile`, including
   normalization and equality rules.
3. Define the tag/content representation and canonical spelling for every
   closed resource-model enum and nested variant.
4. Define exact encodings for:
   - signed and unsigned integers;
   - finite `f64`, including `-0.0`, NaN, and infinity rejection;
   - `PublicId`, `EntityId`, `ResourceTypeId`, nominal type IDs, schema IDs,
     field IDs, variant IDs, codec IDs, capability IDs, and family IDs;
   - UTF-8 strings and byte strings;
   - optional, list, ordered map, record, and enum constants;
   - `AssetRef` and exact `ResourceRef`; and
   - the retained-identity category selected by Lang-01.4.1.
5. Define object-key duplication, duplicate registry-record, unknown-field,
   missing-field, and wrong-shape behavior. Do not rely on a generic
   deserializer's accidental last-write-wins policy.
6. Define deterministic canonical ordering and canonical serialization for
   signing, digesting, cache keys, and byte-for-byte regeneration.
7. Define exact version dispatch. An unsupported version, malformed version,
   and missing version must be distinct typed failures without a legacy
   fallback reader.
8. Define nested source ranges, related ranges, and stable diagnostic
   categories for all decode and registry-validation failures.
9. State whether one file may publish more than one resource type and how
   cross-type references within a file or package are resolved.
10. Specify how manifest-declared semantic digests interact with the
    registry's independently recomputed canonical descriptor digest. A claimed
    digest must never replace validation or recomputation.

## Implementation order to specify

1. Freeze the complete neutral wire schema and canonical examples.
2. Reconcile the retained-reference wire branch with Lang-01.4.1.
3. Add typed wire DTOs in the data-format owner without exposing decoder
   implementation details as semantic model.
4. Implement one strict decoder and one canonical encoder.
5. Convert accepted wire DTOs into the existing immutable
   `arcweft-resource-model` registry through typed validation.
6. Connect package/build loading and deterministic artifact publication.
7. Add negative, round-trip, tamper, and source-range tests.
8. Delete any provisional reader or serializer in the same change; do not
   publish two accepted formats.

## Tests to specify

- canonical minimal and full manifests decode to the exact typed descriptor
  inventory;
- decode, encode, and decode again preserve semantic equality and canonical
  bytes;
- canonical output is independent of input record order where the schema
  declares a set, and preserves order only where order is semantic;
- every enum variant has a positive fixture and wrong-tag/wrong-content
  negatives;
- missing, null, malformed, unsupported-version, unknown-field, and duplicate
  inputs produce distinct typed failures with exact nested source ranges;
- NaN, infinity, non-canonical `-0.0`, out-of-range integers, invalid UTF-8 or
  byte encodings, and malformed IDs are rejected as specified;
- raw string type labels cannot substitute for typed IDs or enum tags;
- `AssetRef`, exact `ResourceRef`, and the Lang-01.4.1 retained-reference type
  reject cross-category targets;
- a forged declared digest cannot make a semantically different descriptor
  acceptable;
- regenerated canonical bytes and semantic digests are stable across process
  runs and insertion orders; and
- decoder resource budgets bound nesting, collection sizes, string/byte
  lengths, and total records without partial registry publication.

## Constraints

- Do not redesign the implemented private `res` grammar, identity types,
  constant-value model, descriptor model, immutable registry validation, or
  canonical semantic digest unless a concrete contradiction is demonstrated.
- Do not encode Character, View, Action, Layer, Signal, presentation target, or
  scroll region as `ResourceRef`; consume Lang-01.4.1.
- Do not introduce a permissive generic `serde_json::Value`/TOML value boundary,
  raw type labels, aliases, dual readers, legacy fallbacks, compatibility
  versions, source gates, or spelling-specific removed-syntax diagnostics.
- Keep the decoder and codec Sans I/O. Filesystem discovery and package loading
  remain in build/project adapters.
- Do not revive CSS or Takumi routes.

## Expected output

Return one independently implementable final contract correction containing:

- the complete `ResourceTypeManifestFileV1` neutral wire schema;
- exact tagged-record and scalar spellings for every variant;
- canonical serialization and digest rules;
- strict version, duplicate, unknown-field, and resource-budget policy;
- typed diagnostic categories and nested source-range ownership;
- exact Rust DTO and conversion-boundary recommendations;
- positive, negative, round-trip, tamper, determinism, and budget test matrices;
  and
- explicit coordination points with Lang-01.4.1 and the already implemented
  generic resource registry substrate.
