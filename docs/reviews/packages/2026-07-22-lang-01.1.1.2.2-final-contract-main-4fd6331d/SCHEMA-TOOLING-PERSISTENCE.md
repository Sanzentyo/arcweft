# Schema, Digest, Tooling, and Persistent Identity Consequences

## 1. Identity rule

A displayed type label is never an identity; a source label is never an identity. All semantic consumers use the final `TypeKind`, and every Rust/adapter nominal in a final callable schema is `TypeKind::AcceptedNominal` (or an existing exact accepted semantic such as character/exact) produced by `AcceptedNominalRecord::try_instantiate`.

The existing derives on `TypeKind`, `CallableLookupKey`, `ReceiverMethodKey`, and callable IDs already make exact semantic types part of equality/hash identity. The correction ensures the publication side supplies the right semantic value.

## 2. Stable semantic type digest

`TypeKind::semantic_identity_digest()` uses BLAKE3 with this domain prefix:

```text
arcweft.semantic-type.identity.v1\0
```

The encoder is an exhaustive inherent `impl TypeKind` match. Each variant writes:

1. an explicit fixed `u16` tag assigned in code;
2. scalar fields in fixed-width little-endian form;
3. strings as checked `u32` byte length + UTF-8 bytes;
4. sequences as checked `u32` count + elements in semantic order;
5. option fields as one byte (`0`/`1`) + payload;
6. maps only after sorting by their typed key.

The tags are explicit constants. They are not derived from Rust enum declaration order.

### 2.1 Accepted nominal encoding

`TypeKind::AcceptedNominal` writes:

```text
tag(accepted_nominal)
owner discriminator
owner identity bytes
canonical TypePath root discriminator
path segment count
each exact segment
argument count
each argument semantic encoding
```

Owner encoding:

- `Standard` → standard environment discriminant;
- `Environment` → exact `EnvironmentBindingId`;
- `RustPackage` → exact `RustPackageId`;
- `Character` → exact `CharacterId`.

No source label, display name, terminal name, package version, or adapter provider is used in `AcceptedNominalType` identity.

Package version/hash belong to Rust provenance and registered environment identity, not to the language-level nominal owner ID.

### 2.2 Other identity-sensitive variants

The exhaustive encoder also writes typed identity for:

- `ProjectNominal`: full `ProjectNominalDeclarationId` + arguments;
- `GenericParam`: complete `GenericTypeOwnerId` + ordinal;
- `OpenNominal`: exact rule ID + exact path + arguments;
- `CharacterNominal`: structural character owner/family identity;
- `Error`: poison ID, for existing recovery caches only;
- `Named`: internal/host label, while adapter/Rust publication rejects this variant;
- function/effect types: parameter types, result, and complete effect row;
- array lengths: concrete value or complete generic/error identity.

A new `TypeKind` variant causes a compile error in the exhaustive encoder until assigned an explicit tag and tests.

## 3. Callable schema digest

`CallableSignatureSchema::semantic_digest()` uses domain:

```text
arcweft.callable-signature.semantic.v1\0
```

It encodes:

1. parameter group count and group indices;
2. parameter count and parameter indices within each group;
3. optional parameter name;
4. passing mode;
5. presence mode;
6. exact/unchecked type discriminator;
7. exact `TypeKind` semantic identity for every exact parameter;
8. result `TypeKind`;
9. canonical effect row;
10. call policy;
11. validator kind.

Documentation, source ranges, declaration order, provider, and Rust provenance are excluded from the *schema* digest because they do not change semantic call compatibility.

`CallableSignatureSchema::semantic_eq` remains exact structural equality. It may use the digest as a fast negative check only; equality is still authoritative.

## 4. Publication digest

`EnvironmentCallablePublicationDigest` uses domain:

```text
arcweft.environment-publication.v1\0
```

It encodes:

- `AcceptedNominalWorldStamp`;
- environment callable owner;
- canonical manifest digest;
- record count;
- records sorted by declaration order, then lookup key, then overload;
- record kind;
- exact lookup key, including exact method receiver `TypeKind`;
- overload index;
- schema digest;
- Rust provenance, including adapter provider, `RustPackageId`, version, metadata hash, and Rust item path;
- documentation digest;
- source identity/range digest;
- declaration order.

This digest changes when tooling-visible documentation/source evidence changes, even if the schema digest does not.

## 5. Registered callable catalog digest

`RegisteredCallableCatalogDigest` uses domain:

```text
arcweft.registered-callable-catalog.v1\0
```

It encodes canonical catalog entries by final `CallableCandidateId` and includes:

- candidate ID;
- key;
- authority;
- provider;
- schema digest;
- full publication/tooling digest;
- equivalent source IDs and origins in canonical order;
- method receiver index entries in canonical key order.

No `HashMap` iteration order participates. The implementation obtains sorted keys and indexes the immutable catalog.

## 6. Accepted Rust metadata digest

`AcceptedRustTypeMetadataDigest` uses domain:

```text
arcweft.accepted-rust-metadata.v1\0
```

It encodes records ordered by `AcceptedNominalId`:

- accepted ID;
- Rust package ID;
- package version/hash claim from the owning manifest batch;
- Rust item path;
- generic parameter count and exact typed parameter IDs;
- struct/enum/newtype shape;
- template `TypeKind` semantic identities;
- variant/field names in declaration order;
- declaration source identity/range.

Instantiation results are not separately persisted; they are derived from the template and exact nominal arguments.

## 7. Registered environment digest

`RegisteredEnvironmentDigest` uses domain:

```text
arcweft.registered-semantic-environment.v1\0
```

It encodes:

1. project symbol world ID;
2. project symbol revision;
3. accepted nominal catalog digest;
4. accepted nominal visibility-index digest;
5. accepted Rust metadata digest;
6. registered callable catalog digest;
7. selected environment manifest digests sorted by owner;
8. existing character/project environment digest components already owned by registration.

This digest is calculated only after successful registration.

## 8. Callable candidate identity

No new candidate-ID variant is required.

Current identity already includes:

- `EnvironmentCallableOwner`;
- environment callable kind;
- exact `CallableLookupKey`;
- overload.

A method lookup key contains `ReceiverMethodKey { receiver: TypeKind, method }`. Once projection supplies `AcceptedNominal`, the accepted declaration ID and arguments are therefore part of:

- method candidate ID;
- method hash/index lookup;
- overload candidate set;
- deterministic record ordering tie breakers.

A free function’s candidate ID remains owner/path/overload based, while its schema carries accepted nominal identity. Two free function overloads with display-equal but owner-distinct nominal parameters remain distinct by exact schema applicability.

## 9. Overload and authored `extern` matching

The existing checked schema model remains authoritative.

- authored `extern` parameters/results are resolved by the source-backed nominal resolver;
- adapter/Rust parameters/results are projected by `AcceptedNominalWorld`;
- both sides call the same accepted record instantiation behavior;
- `matches_function_signature` and `semantic_eq` compare exact `TypeKind`.

There is no compatibility relation equating an internal `Named` with an accepted nominal.

For generic accepted nominals, argument order and each argument identity are exact. `PackageA::Box<ProjectX>` cannot match `PackageA::Box<ProjectY>`, and `PackageA::Box<T>` cannot match `PackageB::Box<T>`.

## 10. Signature help

The existing semantic signature projection already copies checked parameter/result `TypeKind` values from the selected callable record. It must continue to do so.

Required invariants:

- `SemanticSignature` retains the selected `CallableCandidateId`;
- every parameter/result retains exact `TypeKind`;
- labels call `TypeKind::source_label()` only at the final presentation step;
- signature cache keys include the accepted world stamp and registered environment digest;
- no signature-help path reparses a rendered type;
- stale profile/world generations continue to cancel or reject the request.

Tests must assert both display text and the underlying `AcceptedNominalId`.

## 11. Hover

Callable/type hover may render Markdown or strings, but its semantic input must be one of:

- `CallableRecord` + exact schema;
- a type judgment containing exact `TypeKind`;
- an accepted nominal record/ID;
- accepted Rust metadata keyed by the exact ID.

The LSP feature layer must not branch on `TypeKind::Named` for a Rust accepted export. The current internal dialogue-view `Named` presentation remains outside this correction.

For an accepted Rust nominal hover:

- headline label may be `AcceptedNominalId::source_label()`;
- package, mounted path, arity, and Rust item may be shown from typed records;
- generic arguments come from `AcceptedNominalType.arguments()`;
- navigation source comes from the accepted record/metadata source;
- the hover builder retains the ID until final string serialization.

Tests inspect the typed sema/tooling projection before inspecting LSP JSON.

## 12. Method lookup

Method publication projects the receiver first. `ReceiverMethodKey` is constructed only from the resulting final `TypeKind`.

Consequences:

- the same Rust nominal as receiver and as a later curried parameter has equal accepted ID;
- same terminal names from two packages produce different receiver keys;
- an inaccessible/unknown/mismatched receiver aborts the whole publication;
- no untyped method record is inserted for a projection failure.

## 13. Persistent query keys

The existing `CompilerObjectKey.environment_digest` field is the authoritative persistent slot. No AWBO field or schema version is added.

Every compiler path that has a complete registered semantic world must supply:

```rust
registered.typecheck_env().environment_digest()
```

instead of `BuildDigest::ZERO`.

At the pinned baseline, conservative snapshot paths contain zero placeholders. The implementation must replace those placeholders only where the registered world is actually available; a pre-registration syntax-only query retains its existing non-environment key contract.

Persistent key consequences:

- changing a Rust package mount invalidates semantic queries;
- changing a nominal owner or path invalidates semantic queries;
- changing accepted arity invalidates semantic queries;
- changing a nested callable type argument invalidates semantic queries;
- changing callable docs/source evidence invalidates tooling-facing cached products through the full environment digest;
- changing only presentation formatting code does not alter semantic type digest, but can alter an LSP response cache namespace owned by the LSP layer.

## 14. Persistent carrier version decision

No persistent wire field changes are required:

- `CompilerObjectKey` already has `environment_digest`;
- stage-input objects already carry it;
- AWBO schema shape remains unchanged;
- cache schema/version is not bumped for this unpublished correction.

Round-trip tests cover the existing key/object codec with a non-zero registered environment digest derived from accepted nominals.

## 15. Canonical manifest digest

`EnvironmentManifestDigest` is computed from typed manifest content, not generated source text. Domain:

```text
arcweft.environment-manifest.v1\0
```

Encoding order:

1. manifest owner/id and display name;
2. package mounts sorted by package ID;
3. adapter nominal declarations sorted by full path;
4. Rust manifests sorted by package ID;
5. symbols sorted by path;
6. methods sorted by typed receiver semantic input + method + overload;
7. functions sorted by path + overload;
8. Rust functions sorted by package + Rust item + callable path + overload;
9. effects and host calls sorted by typed ID;
10. tooling docs sorted by documented item ID.

Type inputs are encoded from owner/path/argument structure. Their generated `SourceSpan` values are excluded from this digest; source identity/ranges enter the publication/tooling digest.

## 16. Equality and digest test oracles

Tests must establish:

1. equal authored and projected accepted nominal values have equal semantic digests;
2. same display label under different Rust package owners has different semantic digests;
3. same owner/path with different arguments has different semantic digests;
4. manifest insertion order does not change manifest/publication/catalog/environment digests;
5. changing any identity-bearing field changes the appropriate digest;
6. changing only an excluded field does not change the schema digest but changes the full publication digest when tooling evidence changes;
7. no digest is compared by formatted hexadecimal source text inside semantic code.
