# AWBC nominal-record domain and version-1 wire grammar

## Fixed envelope

The existing envelope remains:

```
magic[8] = 41 57 42 43 0d 0a 1a 0a    # "AWBC\r\n\x1a\n"
codec_version:u16_le = 1
reserved:u16_le = 0
payload_length:u64_le
payload[payload_length]
```

No V2 type, compatibility alias, old reader, or version increment exists.

## Payload table order

The version-1 payload evolves in place to this sole order:

1. `header`
2. `strings`
3. `runtime_types`
4. `nominal_record_domains` — inserted here
5. `constants`
6. `effect_sets`
7. `signatures`
8. `frame_layouts`
9. `functions`
10. `blocks`
11. `instructions`
12. `resume_points`
13. `patterns`
14. `match_arms`
15. `intrinsics`
16. `host_calls`
17. `task_plans`
18. `audio_commands`
19. `effect_plans`
20. `choices`
21. `choice_options`
22. `content_units`
23. `line_task_groups`
24. `line_task_nodes`
25. `stream_plans`
26. `source_plans`
27. `pure_helpers`
28. `trait_methods`
29. `display_map`
30. `source_map`
31. `resources`
32. `callable_executables`
33. `flow_bindings`
34. `flow_executables`
35. `entries`

All existing tables retain their current encodings and relative order. The
added table has a maximum of 262,144 rows. `AwbcDecodeBudget` gains exactly
`nominal_record_domains: usize`, whose v1 default is 262,144. Its IDs are
implicit zero-based table ordinals and have no public raw constructor.

```rust
pub struct AwbcDecodeBudget {
    pub nominal_record_domains: usize,
}
```

This excerpt shows the only added field; the existing v1 fields and defaults are
unchanged. `Default::default().nominal_record_domains == 262_144`.

## Domain row grammar

```
nominal_record_domain_count:var_u32
repeat count:
    origin_tag:u8
    origin_payload
    type_id:var_u32
```

Origin tags:

- `0x00 Plan`: retained canonical `RuntimePlanTypedSite` v1 bytes;
- `0x01 Project`: `RuntimeProjectRootId[32]`;
- `0x02 Producer`: `producer_id_len:var_u32 || producer_id:utf8 || RuntimeProducerRootId[32]`.

Producer IDs must pass the existing `RuntimeOpaqueTypeProducerId`/`PublicId`
validation and 128-byte maximum. UTF-8 spelling is an identifier payload, not a
source/type-name authority. The root is derived independently from the accepted
semantic identity and must correlate exactly.

Canonical row key is the complete bytes `origin_tag || origin_payload ||
type_id`. The builder preflights same-origin/different-type conflicts. Exact
duplicates share one staging handle. Unique rows are sorted lexicographically by
that encoded key; final IDs are issued `0..count-1`; all drafts are rewritten
atomically. Decoder rejects non-canonical order and adjacent duplicate rows.

## Record construction operand

```
record_construction_tag:u8
```

- `0x00 Structural`:
  `type_id:var_u32 || field_name_count:var_u32 || field_name_id[var_u32]*`
- `0x01 Nominal`:
  `nominal_record_domain_id:var_u32`

Nominal values are supplied in the admitted layout's defining order; names are
not repeated and cannot override the domain.

## Constant tag 12

```
0x0c
record_construction
field_count:var_u32
field_constant_id:var_u32 * field_count
```

## MakeRecord opcode 0x0f

```
0x0f
dst_register:var_u32
record_construction
field_count:var_u32
field_register:var_u32 * field_count
```

## Decode/verify precedence

1. envelope magic/version/reserved/length;
2. global decode byte/table/nesting budgets (existing v1 budgets; nesting 64);
3. nominal-domain count limit;
4. row tag/UTF-8/identifier/root/type-ID decode;
5. canonical row order and exact duplicate rejection;
6. origin resolution against admitted generation/plan;
7. project/producer root correlation;
8. type resolution and exact checked nominal-record classification;
9. record operand domain-ID bounds;
10. field count/layout equality;
11. defining-order field semantic types;
12. remaining existing AWBC structural verification.

The VM is given only `AdmittedAwbcProgram`/`AdmittedRuntimeProduct`. It resolves
the already-admitted domain table and never derives a domain from a nominal
spelling, source path, crate name, or raw value.
