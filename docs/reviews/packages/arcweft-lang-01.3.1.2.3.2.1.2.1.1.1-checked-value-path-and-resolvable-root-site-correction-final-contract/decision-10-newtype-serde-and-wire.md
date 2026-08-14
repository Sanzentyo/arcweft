# Decision 10 — bounded newtype Serde and private wire DTOs

## RuntimeIndexPath

`RuntimeIndexPath` has one canonical wire grammar in both human and non-human Serde: a sequence of unsigned `u32` values. The first value is the fixed root marker `0`; total length is 1 through 64 inclusive.

Manual deserialization is mandatory:

```rust
impl<'de> Deserialize<'de> for RuntimeIndexPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>
    {
        let wire = Vec::<u32>::deserialize(deserializer)?;
        Self::try_from_indices(wire).map_err(serde::de::Error::custom)
    }
}
```

`try_new`, `try_from_indices`, `child`, and deserialization therefore share exactly the same `Empty`, `InvalidRoot`, and `TooDeep` checks. There is no derived `Deserialize`, `Default`, transparent public field, or unchecked `From<Vec<u32>>`.

## Root, site, and domain evidence

Operational root/site/domain types do not derive `Deserialize` directly:

- `RuntimeProjectRootId` and `RuntimeProducerRootId` are constructed by crate-private lossless projection from `RuntimeSemanticTypeId`;
- raw declarations deserialize `[u8;32]` into private `*Wire` DTO fields and call a constructor that compares those bytes with the declaration semantic ID;
- `RuntimePlanTypedSite`, all nested coordinates/slot enums, `AwbcTypedSite`, `AwbcTypedOrigin`, `AwbcNominalRecordDomainDeclaration`, and `AwbcNominalRecordDomainId` use private wire DTOs and checked `try_from_wire` constructors;
- admitted generation/plan/AWBC/domain/context/catalog wrappers are never Serde.

A dense index newtype whose only invariant is `u32` may serialize as `u32`, but its field is private and table bounds are checked by the owning constructor/admission before publication. It is not a capability.

## Human-readable tagged maps

`RuntimePlanTypedSite` and `AwbcTypedSite` serialize as internally tagged maps with `table` in snake case and exact named fields shown in their API documents. Unknown/missing fields and unknown enum tags fail. Nested slot enums use `kind` in snake case. Repeated indices are native `u32`; no decimal-string fallback exists for these bounded indices.

## Canonical byte encoding

Serde is storage/query representation, not the equality authority. Plan/AWBC pair equality uses the explicit little-endian grammar in `PLAN_AWBC_EQUALITY_GRAMMAR.md`. A Serde format cannot define another canonical site order or tag assignment.

`NEWTYPE_SERDE_GRAMMARS.md` lists every affected type, its wire owner, constructor, bound, and bypass-prevention rule.
