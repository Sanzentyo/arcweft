# 05. Persistence byte grammar and two-phase restore

## Canonical record

The carrier record is embedded through the repository's existing snapshot framing. The following is the normative semantic grammar; existing canonical integer and digest primitives should be reused verbatim.

```text
accepted_runtime_carrier :=
    format_version:u16le
    variant:u8
    flags:u8
    body_len:canonical_uvarint
    body:bytes[body_len]

variant 0x00 (structural) body :=
    stable_shape_key:stable_key
    stable_payload_key:stable_key

variant 0x01 (nominal) body :=
    stable_catalog_domain:stable_key
    stable_nominal_def:stable_key
    generic_arg_count:canonical_uvarint
    generic_args:stable_type_key[generic_arg_count]
    stable_representation_shape:stable_key
    stable_payload_key:stable_key

stable_key := key_kind:u8 || key_len:canonical_uvarint || key_bytes[key_len]
```

Constraints:

- `format_version = 1` for the first admitted format.
- Reserved flags must be zero; nonzero unknown identity-bearing flags are rejected.
- `body_len` must be minimal/canonical and exactly consumed.
- Generic arguments appear in declaration order, not hash-map order.
- Stable key bytes use the repository's catalog/type digest representation; never debug strings.
- Payload bytes live in the snapshot's payload/value table and are referenced once by stable payload key.
- The carrier record has no raw `Runtime*Id`, arena slot, pointer, `usize`, or platform-endian integer.

## Phase A — decode and local validation

Decode into wire-only records:

```rust
pub enum UnresolvedAcceptedCarrier {
    Structural { shape: StableShapeKey, payload: StablePayloadKey },
    Nominal {
        catalog: StableCatalogKey,
        definition: StableNominalKey,
        generic_args: Box<[StableTypeKey]>,
        representation: StableShapeKey,
        payload: StablePayloadKey,
    },
}
```

Phase A checks version/tag/flags, canonical integers, lengths, duplicates, key syntax, allocation bounds, trailing bytes, and aggregate resource limits. It does not publish runtime handles.

## Phase B — resolve, validate, seal

Resolution order is deterministic:

1. Resolve catalog domain and verify catalog digest/version contract.
2. Resolve every generic type key.
3. Intern/resolve the nominal instance key.
4. Resolve the structural representation shape.
5. Verify the catalog-declared representation of the nominal instance equals the encoded shape.
6. Resolve payload and verify its actual shape/representation.
7. Re-resolve projection witness references used by restored match plans and verify semantic digests.
8. Construct the carrier through the same checked constructor used for live values.
9. Add the sealed carrier to the staged batch.
10. Publish all carrier/value/task handles atomically only after the entire batch succeeds.

## Failure and rollback

A failure drops the unresolved/staged batch. It does not mutate the live interner in an externally visible way, bind any task handle, wake a Need waiter, or emit a successful transcript. Interning implementations that cannot roll back may retain unreachable canonical metadata internally, but publication roots and observable handle tables remain unchanged.

## Isomorphism requirements

For valid `x`:

```text
semantic_eq(resolve(decode(encode(x))), x) == true
encode(resolve(decode(encode(x)))) == encode(x)
match(resolve(decode(encode(x))), plan) == match(x, plan)
```

For invalid/noncanonical bytes, decode or resolve returns a typed error before publication.
