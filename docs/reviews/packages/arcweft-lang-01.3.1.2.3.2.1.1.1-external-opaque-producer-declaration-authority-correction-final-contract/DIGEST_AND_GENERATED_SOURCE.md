# Canonical digest and generated-source contract

## 1. Shared scalar grammar

Manual digests use existing Arcweft canonical primitives:

```text
u8/u16/u32/u64  fixed little-endian
string          u32 little-endian UTF-8 byte length + exact bytes
list            u32 little-endian count + items
optional        u8 0, or u8 1 + payload
ordered rows    existing semantic sort key, never map iteration accident
```

No `Debug`, serde map order, source span, display label (except an already
canonical descriptor field), process pointer, path on disk, or hash-map
iteration participates.

## 2. Adapter environment manifest digest v2

Domain changes exactly:

```text
old: arcweft.environment-manifest.v1\0
new: arcweft.environment-manifest.v2\0
```

Existing section marker grammar remains `0xff` followed by the existing u8
section tag. In nominal section 2, rows retain path sort order and encode:

```text
accepted path segments
u16 arity
string opaque_producer       # inserted
u8 visibility                # public=0, private=1
string source_label
```

In Rust type section 3, rows retain package/accepted-path sort order and encode:

```text
existing Rust package identity/version/metadata tuple
accepted nominal path
string opaque_producer       # inserted
Rust declaration path
string rust_path
parameters in declaration order
structural Rust kind
```

Producer is placed before presentation/structural details so a producer-only
change changes the digest without affecting sort order. Duplicate producer
values are encoded independently and are valid.

## 3. External type-input digest

Domain remains exactly:

```text
arcweft.environment-type-input.v1\0
```

This digest identifies recursive type-reference input sites. Producer belongs
to the declaration row, not each recursive use, so no field/domain/version
changes.

## 4. Accepted nominal catalog digest v2

Domain changes exactly:

```text
old: arcweft.accepted-nominal-catalog.v1\0
new: arcweft.accepted-nominal-catalog.v2\0
```

The existing catalog hasher and canonical accepted-ID order remain. An opaque
semantic row now hashes its explicit enum tag followed by `string producer`.
All retained non-opaque rows keep their existing grammar. Do not add a parallel
producer digest or side table.

## 5. Rust ABI manifest artifact hash

`arcweft-rust-abi-build` continues to serialize the validated manifest as its
existing deterministic pretty JSON and computes BLAKE3-256 over those exact
bytes. It does not add a second domain/hash. Schema version 2 and every
`opaque_producer` field are in the bytes, so producer-only changes alter the
hash and generated artifact.

## 6. Semantic identities unchanged

The accepted-nominal semantic type identity remains the existing domain/version
and canonical bytes for declaration identity plus normalized arguments. The
implementation must explicitly visit:

```text
accepted nominal declaration ID
argument count
canonical argument identities in order
```

It must not call `Hash` on the entire new `AcceptedNominalType`, because that
would accidentally make producer part of semantic type identity. Two worlds
that differ only in producer have equal semantic nominal identity but unequal
manifest/catalog/artifact digests and unequal runtime exact owners.

Rust structural metadata hashes, callable identities, AWBC executable/type
identity, ABI 1, codec 11, tags 16/23/18, bundle outer schema, and session-save
schema 3 are unchanged by this correction.

## 7. Generated registration source v2

Header:

```text
adapter-manifest-v2
```

Nominal row:

```text
nominal path=<path> arity=<u16> opaque-producer=<n>:<bytes> visibility=<public|private> label=<n>:<bytes>
```

Rust row:

```text
rust-type package=<n>:<bytes> accepted=<path> opaque-producer=<n>:<bytes> rust-item=<n>:<bytes> shape=<existing-shape>
```

`n` is the decimal UTF-8 byte length. Bytes are copied exactly; no quotes,
backslash escapes, JSON escaping, normalization, trimming, or percent encoding
is used. Control characters are already impossible. The existing deterministic
row ordering is retained. Source-map ownership records the producer payload
range only.

Generated source has no independent digest owner. It participates through the
environment-manifest digest and existing source-document revision/evidence.
