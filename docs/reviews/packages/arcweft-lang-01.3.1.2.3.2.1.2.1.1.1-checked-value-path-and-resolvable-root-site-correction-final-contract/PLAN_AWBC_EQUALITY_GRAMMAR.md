# Exact version-1 plan-to-AWBC direct equality grammar

This grammar compares canonical typed rows directly. It does not define, compute, persist, or compare another digest or root map.

## Scalars

```text
u8(v)              = one byte
u32_le(v)          = exactly four little-endian bytes
u64_le(v)          = exactly eight little-endian bytes
bytes32            = exactly 32 bytes
utf8(v)            = u32_le(v.as_bytes().len()) || exact UTF-8 bytes
sequence(v)        = u32_le(v.len()) || elements in owner order
optional(v)        = 00 | 01 || v
```

Every numeric conversion is checked before encoding. Unknown enum tags, invalid UTF-8, out-of-bounds dense IDs/indices, noncanonical order, duplicates, and trailing bytes fail rather than normalize.

`RuntimeIndexPath` is exactly:

```text
u32_le(segment_count) || u32_le(segment[0]) ... u32_le(segment[count-1])
```

`segment_count` is 1 through 64 inclusive and `segment[0]` is exactly zero. Manual deserialization invokes the same constructor.

## RuntimePlan coordinates

`RuntimePlanTypedSite` is `site_tag:u8 || payload` using `PLAN_SITE_CANONICAL_TAGS.csv` exactly.

Nested fields/slots are `slot_tag:u8 || payload` using `RUNTIME_PLAN_NESTED_SLOT_TAGS.csv`. Flow and stream descent steps are `step_tag:u8 || payload` using `RUNTIME_PLAN_COORDINATE_STEP_TAGS.csv`.

```text
RuntimeFlowOpCoordinate = u32_le(flow) || u32_le(root)
                        || u32_le(step_count) || step[0] ... step[n-1]
RuntimeStreamOpCoordinate = u32_le(plan) || u32_le(root)
                          || u32_le(step_count) || step[0] ... step[n-1]
RuntimeSourceOpCoordinate = u32_le(plan) || u32_le(handler) || u32_le(op)
```

Every step/index resolves against the actual current owner enum/vector before a site exists. No display name, source path, debug output, or generic slot ordinal participates.

## AWBC coordinates

`AwbcTypedSite` is `site_tag:u8 || payload` using `AWBC_SITE_CANONICAL_TAGS.csv` exactly. The top-level tags follow the final Rust enum and include `FunctionFrame`; there is no bare `Frame` or `Function` tag.

- non-instruction slots: `slot_tag:u8 || payload` from `AWBC_NESTED_SLOT_TAGS.csv`;
- instruction slots: `AwbcOpcode::encoded():u8 || field_tag:u8 || field-specific u32 payload` from `AWBC_INSTRUCTION_TYPED_SLOTS.csv`;
- terminator slots: `AwbcTerminator::opcode().encoded():u8 || field_tag:u8 || field-specific u32 payload` from `AWBC_TERMINATOR_TYPED_SLOTS.csv`;
- audio slots: `command_tag:u8 || field_tag:u8` from `AWBC_AUDIO_TYPED_SLOTS.csv`.

The enclosing payload supplies the exact function/instruction, function/block, command, plan, signature, pattern, or other table ID shown in `AWBC_SITE_CANONICAL_TAGS.csv`. Slot/opcode agreement and actual referenced owner fields are checked before type/root correlation.

## Checked type and authority

Checked types are exactly the bytes in `RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md` and `RUNTIME_CHECKED_TYPE_TAGS.csv`.

One resolved plan row is:

```text
u32_le(plan_site_bytes.len)
|| plan_site_bytes
|| RuntimeSemanticTypeId[32]
|| u32_le(checked_type_bytes.len)
|| checked_type_bytes
|| authority_tag:u8
|| authority_payload
```

Authority tag and payload:

```text
authority_tag = 00; authority_payload = RuntimeProjectRootId[32]
authority_tag = 01; authority_payload = utf8(RuntimeOpaqueTypeProducerId) || RuntimeProducerRootId[32]
```

The root IDs are the accepted lossless 32-byte semantic projections fixed by the retained contract. They are not recomputed from coordinates.

## Pair transcript and equality

```text
ASCII "arcweft.plan_awbc.equality.v1\0"
|| u32_le(row_count)
|| resolved rows in strict RuntimePlanTypedSite canonical byte order
```

The ASCII prefix is direct framing only; the transcript is never hashed. Plan admission independently resolves every real owner into rows. AWBC admission independently resolves every actual table owner and then uses coordinate-only `AwbcTypedOrigin { plan_site, awbc_site }` to collapse each AWBC row to its plan site. Origins contain no semantic ID, checked type, root, generation, or dense type declaration claim.

For one plan site, every collapsed AWBC row must resolve to byte-identical semantic ID, checked-type bytes, and authority. Duplicate origin pairs, conflicting collapsed rows, a missing/extra site, noncanonical ordering, a changed plan site, a changed AWBC site, a changed declaration, or coordinated changes to both raw claims fail. Acceptance is exact transcript byte equality after each side has independently passed generation admission.
