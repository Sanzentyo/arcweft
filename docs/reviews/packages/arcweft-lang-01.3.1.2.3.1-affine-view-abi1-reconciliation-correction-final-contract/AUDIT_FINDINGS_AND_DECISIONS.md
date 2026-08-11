# Audit findings and selected corrections

| Audit finding | Selected result |
|---|---|
| View parent uses ordinary cloneable `RuntimeValue` while affine parent removes unconditional clone/Serde | View current-language persistent/render values become unrestricted-only; every cross-section input has explicit Copy/Move intent; save uses dormant snapshots |
| affine parent labels ownership wire ABI 2 while View parent retains ABI 1 | ABI number fixed at 1 by direct user authority; ownership semantics replace ABI 1 in place |
| snapshot exclusivity only per driver | one activation authority spans all drivers in one runtime execution domain |
| allocator continuation after restore undefined | exact allocator cursor is serialized, validated, and installed |
| prepared drop can validate A then commit B | prepared drop owns the removed value and exact source reservation; commit accepts no value argument |
| `RuntimeValueSnapshotV2` `Eq` mismatch | derive `PartialEq` only; floats remain ordinary snapshot values and canonical bytes own equality-sensitive validation |
| `#[static]` not represented on wire | serialized `ViewStaticRequirementResource` rows are included in program semantic identity and require matching authored certificates |
| ancestor/descendant fragment selection undefined | strict containment is valid; partial overlap invalid; runtime selects the outermost valid fragment and suppresses descendant dispatch inside it |

## User ABI decision

The ABI number is not released, has no external consumer, and was not a formally frozen compatibility boundary. Therefore:

```text
AWBC_ABI_VERSION = 1
AWBC_CODEC_VERSION = 8
```

The former parent label “ABI 2” is superseded everywhere. This is not compatibility support for an old ABI-1 meaning. There is only one post-cut ABI-1 meaning. All checked-in products, caches, fixtures, and generated artifacts are rebuilt; no reader decides between old and new semantics.
