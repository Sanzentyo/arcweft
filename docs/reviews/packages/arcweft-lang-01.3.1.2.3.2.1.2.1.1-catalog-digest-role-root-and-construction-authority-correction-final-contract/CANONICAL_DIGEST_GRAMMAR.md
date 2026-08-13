# Canonical catalog-digest and role-root grammar

## 1. Primitive

Use the repository's existing canonical 32-byte digest implementation. Do not introduce a second hashing crate or algorithm. Every hash starts with a fixed ASCII domain followed by `0x00`; every variable byte sequence is length-prefixed with unsigned big-endian `u32`; counts are unsigned big-endian `u32`; stable role ordinals are unsigned big-endian `u16`. Integer widths are not inferred and JSON/Serde bytes are never hashed.

## 2. Role digest

For role `r` and canonical typed catalog payload `P(r)`:

```text
role_digest(r) = H(
  bytes("arcweft/runtime-catalog-role-digest/v1") || 0x00 ||
  u16be(r.stable_ordinal()) ||
  bytes_len32(r.domain_tag()) ||
  u32be(entry_count(P(r))) ||
  bytes_len32(canonical_encode(P(r)))
)
```

`canonical_encode` is role-owned behavior selected by `RuntimeCatalogDigestRole`. It emits typed fields in the exact order fixed by the role's catalog contract. Set-like catalogs are sorted by their existing stable typed key; sequence-semantic catalogs preserve admitted sequence order. Duplicate keys are rejected, not silently deduplicated. HashMap/BTreeMap implementation iteration is never the normative grammar.

| Role | Stable ordinal | Role domain | Payload owner |
|---|---:|---|---|
| `AcceptedNominalCatalog` | `1` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/1/v1` | role-specific canonical typed catalog |
| `ExternalProducerDeclarationCatalog` | `2` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/2/v1` | role-specific canonical typed catalog |
| `CharacterDialogueLayoutCatalog` | `3` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/3/v1` | role-specific canonical typed catalog |
| `CharacterCatalog` | `4` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/4/v1` | role-specific canonical typed catalog |
| `ViewCatalog` | `5` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/5/v1` | role-specific canonical typed catalog |
| `CustomFieldSchemaCatalog` | `6` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/6/v1` | role-specific canonical typed catalog |
| `AwbcRuntimePlanBinding` | `7` as `u16be` | UTF-8 bytes `arcweft/runtime-catalog-role/7/v1` | role-specific canonical typed catalog |

## 3. Role-root digest

Entries are sorted by stable ordinal, and completeness/uniqueness is validated before hashing:

```text
role_root_digest = H(
  bytes("arcweft/runtime-catalog-role-root/v1") || 0x00 ||
  u16be(1) ||                         // grammar version
  u32be(required_role_count) ||
  repeat in ordinal order {
    u16be(role_ordinal) ||
    digest32(role_digest)
  }
)
```

No role name string, source span, process address, collection iteration order, admitted object pointer, root digest, or generation identity is included. Diagnostic source locations are retained outside canonical bytes.

## 4. Generation derivation

```text
generation_identity = H(
  bytes("arcweft/runtime-generation-identity/v1") || 0x00 ||
  digest32(producer_declaration_root_digest) ||
  digest32(role_root_digest) ||
  digest32(plan_awbc_binding_digest)
)
```

`plan_awbc_binding_digest` is itself derived from canonical typed plan/AWBC binding material, never from executable object addresses. This order is acyclic. Assertions in plan, AWBC, bundle, save, replay, or producer declarations are compared only after all three derived values exist.

## 5. Canonical rejection rules

Reject before hashing or admission:

- duplicate catalog key or role;
- missing required role;
- unknown role ordinal or grammar version;
- non-canonical/disallowed identifier representation;
- over-limit item count, encoded bytes, nesting, or work units;
- unresolved nominal/layout/type/View reference;
- catalog entry outside its producer/role closure;
- disagreement between independently represented plan/AWBC binding material.

## 6. Required vectors

Freeze one positive empty/minimal/full vector per role, one complete root vector, one generation vector, and negative vectors for cross-role substitution, one-bit assertion changes, duplicate/missing role, alternate integer width, alternate ordering, unknown version, over-limit input, and plan/AWBC disagreement. Golden files must record canonical input fields and expected digest bytes, not only a hex output.
