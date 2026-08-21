# Canonical checked Match product identity

## Session facts versus product coordinates

`CheckedMatch` may retain `ExprId`, `ScopeId`, `PatternId`, and `LocalId` solely
to resolve same-generation semantic facts. These IDs, `HirSnapshotId`, and all
arena ordinals are prohibited from product wire, bundle roots, saves, replay,
and replacement keys.

The compiler performs this one-way projection:

```rust
pub struct ProductCheckedMatchCoordinate {
    pub program: ViewProgramSemanticDigest,
    pub revision: ViewProgramRevision,       // canonical u32
    pub site: ViewMatchSiteId,               // canonical u32
}

pub struct ProductCheckedMatchArmCoordinate {
    pub owner: ProductCheckedMatchCoordinate,
    pub arm: ViewMatchArmOrdinal,             // source ordinal u32
}

pub struct ProductCheckedMatchOutputCoordinate {
    pub arm: ProductCheckedMatchArmCoordinate,
    pub output: ViewMatchBindingOutputOrdinal,// source binding ordinal u32
}
```

`arcweft-view` owns only lightweight site/arm/output/local/body coordinates.
The bundle owns the static View/AWBC join. Runtime-driver privately decodes and
installs it. No core register or RuntimeValue enters a View-owned row.

## Digest transcript

Hash: BLAKE3-256. Domain: `arcweft.checked-match.semantic.v1\0`.
This digest grammar is separate from the AWBC wire:

```text
u8 tag
u8 bool (0/1)
u32-le count/ordinal/revision/site
u32-le byte_length + bytes
bytes32 digest
```

Ordered transcript after domain:

```text
u8  schema = 1
bytes32 view_program_semantic_digest
u32-le view_program_revision
u32-le match_site
bytes32 scrutinee_type_semantic_digest
bytes32 match_result_type_semantic_digest
bytes32 aggregate_effect_digest
bytes32 resource_type_registry_digest
u8 exhaustive = 1
u32-le unreachable_count
repeat unreachable_count:
    u32-le arm_ordinal
    u8 reason_tag
u32-le arm_count
repeat source-order arm:
    u32-le arm_ordinal
    bytes32 pattern_semantic_digest
    u8 guard_class                 # 0 absent, 1 true, 2 false, 3 dynamic
    if dynamic: bytes32 guard_expression_semantic_digest
    bytes32 arm_value_expression_semantic_digest
    bytes32 arm_value_type_semantic_digest
    bytes32 arm_effect_digest
    u32-le binding_count
    repeat source-order binding:
        u32-le output_ordinal
        bytes32 binding_type_semantic_digest
        u8 ownership                # 0 Copy, 1 SnapshotClone
u32-le result_case_count
repeat source-order result case:
    u32-le arm_ordinal
    bytes32 synthetic_case_semantic_digest
    u32-le tuple_item_count
    repeat tuple item:
        u32-le output_ordinal
        bytes32 type_semantic_digest
        u8 ownership
```

The pattern semantic encoder is a private inherent final-analysis owner. It uses
closed constructor tags, stable semantic constructor digests, case/field source
ordinals, canonical literal bits/bytes, record declaration order, sequence rest
tags, and Or source order. Local names and HIR identities are excluded.

The checked expression semantic encoder similarly reads accepted checked
resolution rows and stable declaration/registered identities. It has depth/node
limits and rejects cycles or missing child facts; it is not an unspecified
`canonical_bytes` helper.

## Determinism and replacement

Equal accepted semantic worlds, resource registry, View program coordinate,
patterns, guard/body semantics, binding types, ownership, and coverage produce
the same digest even when HIR arena allocation changes. Arm reorder, semantic
constructor change, guard/value change, resource digest change, ownership
change, or coverage reachability change changes the digest.

The bundle content root commits to the checked Match digest, View program and
revision, selector AWBC function digest, producer contract, payload type digest,
and resource digest. Save/replay require exact equality. Hot replacement
requires an explicit old-revision to new-revision mapping and equality of all
semantic rows; site number alone is never sufficient.
