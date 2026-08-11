# AWBC ABI 2 / codec 8 and canonical wire

This file is the integrated numeric allocation for the correction. It supersedes
the flat external-Stream argument portion of Lang-01.3.1.2.1. All unrelated ABI-2
Stream lifecycle choices remain unchanged.

## 1. Version cut

```rust
pub const AWBC_ABI_VERSION: u32 = 2;
pub const AWBC_CODEC_VERSION: u16 = 8;
```

The codec-8 decoder accepts only codec 8 and ABI 2 for Product AWBC. It rejects
codec 7 before decoding payload tables. There is no v7-to-v8 reader, Source-table
reader, or flat external-argument reader.

The header has no optional compatibility mode for this feature. Group-aware
external Stream signatures and products are mandatory ABI-2 layout.

## 2. Program table order

Codec 8 encodes `AwbcProgram` tables in this exact order:

1. `strings`
2. `runtime_types`
3. `constants`
4. `effect_sets`
5. `signatures` — ordinary frame/call signatures retained in their existing flat
   form
6. `callable_signatures` — public group-aware callable signatures
7. `callable_parameter_groups`
8. `callable_parameters`
9. `frame_layouts`
10. `functions`
11. `blocks`
12. `instructions`
13. `resume_points`
14. `patterns`
15. `match_arms`
16. `intrinsics`
17. `host_calls`
18. `task_plans`
19. `audio_commands`
20. `effect_plans`
21. `choices`
22. `choice_options`
23. `content_units`
24. `line_task_groups`
25. `line_task_nodes`
26. `stream_definitions` — the sole parent-contract Stream metadata table
27. `pure_helpers`
28. `trait_methods`
29. `display_map`
30. `source_map`
31. `resources`
32. `callable_executables`
33. `flow_executables`
34. `entries`

`stream_plans` and `source_plans` are not present. The three callable-signature
tables are inserted immediately after frame signatures so all function/public
signature metadata has one deterministic locality.

New IDs are transparent `u32` table indices:

```rust
AwbcCallableSignatureId
AwbcCallableParameterGroupId
AwbcCallableParameterId
AwbcStreamDefinitionId
```

## 3. Group-aware metadata records

```rust
pub struct AwbcCallableSignature {
    pub definition: AwbcStreamDefinitionId,
    pub declaration: AwbcDigest,
    pub groups: AwbcTableRange,
    pub item_type: AwbcTypeId,
    pub error_type: AwbcTypeId,
    pub effects: AwbcEffectSetId,
    pub provider_abi: AwbcDigest,
    pub fingerprint: AwbcDigest,
}

pub struct AwbcCallableParameterGroup {
    pub owner: AwbcCallableSignatureId,
    pub index: u16,
    pub kind: AwbcCallableGroupKind,
    pub parameters: AwbcTableRange,
}

pub struct AwbcCallableParameter {
    pub owner: AwbcCallableParameterGroupId,
    pub coordinate: AwbcCallableParameterCoordinate,
    pub name: Option<AwbcStringId>,
    pub passing: AwbcCallableParameterPassing,
    pub presence: AwbcCallableParameterPresence,
    pub ty: AwbcTypeId,
}

pub struct AwbcCallableParameterCoordinate {
    pub group: u16,
    pub parameter: u16,
}
```

The global group table is ordered by `(signature_id, group_index)`. The global
parameter table is ordered by `(signature_id, group_index, parameter_index)`.
Every owner range is contiguous, non-overlapping, in bounds, and exactly covers
its rows. Verification compares the stored indices with table position; it never
normalizes malformed metadata.

### Metadata enum tags

| Wire enum | Tag | Variant |
| --- | ---: | --- |
| `AwbcCallableGroupKind` | 0 | `Initial` |
|  | 1 | `Curried` |
| `AwbcCallableParameterPassing` | 0 | `PositionalOnly` |
|  | 1 | `PositionalOrNamed` |
|  | 2 | `NamedOnly` |
|  | 3 | `RestPositional` |
|  | 4 | `RestNamed` |
| `AwbcCallableParameterPresence` | 0 | `Required` |
|  | 1 | `Optional` |
|  | 2 | `Defaulted(AwbcDigest)` |

Unknown tags are codec errors at the tag byte. Reserved values are not skipped.

## 4. Runtime type and constant allocation

Current codec 7 uses runtime-type tags `0..=20` and constant tags `0..=17`.
Codec 8 adds:

| Family | Tag | Variant | Payload |
| --- | ---: | --- | --- |
| runtime type | 21 | `StreamHandle` | `item: AwbcTypeId`, `error: AwbcTypeId` |
| runtime type | 22 | `ExternalStreamCallable` | `definition: AwbcStreamDefinitionId`, `next_group: u16` |
| constant | 18 | `ExternalStreamCallable` | `definition: AwbcStreamDefinitionId` |

Loading constant tag 18 creates the initial
`RuntimeFunctionValue::ExternalStreamPartial` with the executing fiber's generation,
`next_group = 0`, and an empty product. It does not open a Stream.

Runtime type tag 22 is a function value, not a Stream handle. A verifier never
accepts it where type tag 21 is required.

## 5. Instruction allocation

Codec 8 preserves existing non-Source instruction numbers. Removed Source
instruction bytes `0x22` and `0x23` are invalid and are not reused in this cut.
The closure instructions remain `0x25` and `0x26`.

| Opcode | Instruction |
| ---: | --- |
| `0x27` | `ApplyExternalStreamGroup` |
| `0x28` | `OpenStream` |

No provisional opcode exists. Both are part of the single codec-8 reader/writer.

```rust
pub struct AwbcExternalStreamGroupArguments {
    pub coordinates: Vec<AwbcCallableParameterCoordinate>,
    pub values: Vec<AwbcExternalStreamArgumentOperand>,
}

pub enum AwbcExternalStreamArgumentOperand {
    Explicit { value: AwbcRegisterId },
    Defaulted {
        default: AwbcDigest,
        value: AwbcRegisterId,
    },
    OmittedOptional,
    RestPositional { value: AwbcRegisterId },
    RestNamed { value: AwbcRegisterId },
}

pub enum AwbcInstruction {
    // existing variants
    ApplyExternalStreamGroup {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
    OpenStream {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
}
```

### Operand tags

| Tag | Operand |
| ---: | --- |
| 0 | `Explicit { value }` |
| 1 | `Defaulted { default, value }` |
| 2 | `OmittedOptional` |
| 3 | `RestPositional { value }` |
| 4 | `RestNamed { value }` |

Instruction wire field order is exactly the Rust field order above. Vector lengths
use the existing canonical unsigned length encoding. Coordinates encode `group`
then `parameter`, each as a canonical unsigned integer fitting `u16`. Digests are
32 raw bytes. Register and table IDs use their existing canonical ID encoding.

`coordinates` and `values` must have equal lengths and are not zipped until both
vectors have passed allocation and length budgets. The coordinate vector must be
strictly increasing and must equal the complete declared coordinate list for the
instruction's `group`.

For a positional-rest operand, its register type is `Sequence<parameter.ty>`. For
a named-rest operand, its register type is
`Sequence<Tuple<String, parameter.ty>>`; entries are unique and sorted by UTF-8
name bytes. Empty rest values are empty sequences, not absent operands.

## 6. Verifier rules

The structural verifier performs, in order:

1. header version and table-count budgets;
2. every table range and owner relation;
3. group/signature limits and contiguous indices;
4. parameter passing/presence/type/default legality;
5. canonical signature fingerprint equality;
6. function/frame/register structure; and
7. instruction-specific checks.

For both new instructions it proves:

- `definition` exists and refers to an external origin;
- `signature` exists and is exactly the definition's signature;
- `callee` has static type
  `ExternalStreamCallable { definition, next_group: group }`;
- the instruction group is in range and matches every operand coordinate;
- coordinate/value lengths are equal, nonzero only as permitted by the group,
  and at most 128;
- coordinates are complete, unique, and strictly ordered;
- every operand disposition is legal for its parameter;
- every operand register exists and has the exact declared type/rest aggregate
  type;
- every default digest matches the parameter metadata;
- `ApplyExternalStreamGroup` is used only when `group + 1 < group_count` and its
  destination has type
  `ExternalStreamCallable { definition, next_group: group + 1 }`; and
- `OpenStream` is used only when `group + 1 == group_count` and its destination
  has `StreamHandle { item, error }` matching the signature.

A statically known failure is a verifier error, not a runtime branch. Dynamic
foreign-generation, affine, and value-payload checks remain runtime checks and are
atomic.

## 7. Runtime function-value and save tags

The ABI-2 canonical runtime-value encoding keeps the existing outer
`RuntimeValue::Function` tag and adds a closed inner function-value kind:

| Inner tag | Kind |
| ---: | --- |
| 0 | `Closure` |
| 1 | `ExternalStreamPartial` |

`ExternalStreamPartial` wire fields are encoded in this order:

1. definition digest/ID;
2. declaration digest;
3. generation as canonical `u64`;
4. signature digest;
5. next group as `u16`;
6. ownership tag (`0 = Unrestricted`, `1 = Affine`);
7. completed group count as `u16`;
8. coordinate vector; and
9. argument-value vector.

Argument-value tags are the same `0..=4` allocation used by AWBC operands, but
runtime values contain checked payloads instead of register IDs. Reusing the
semantic tag map prevents an adapter-specific disposition mapping.

## 8. Canonical order and bounds

Per callable:

```text
group_count: 1..=16
parameter_count_total: 0..=128
parameter_count_per_group: 0..=128
completed_groups: 0..=group_count
coordinate_count == value_count <= 128
```

Per instruction, coordinates belong to exactly one group and count equals that
group's parameter count. Per complete request, coordinates cover every parameter
in every group.

Global decode defaults add these budgets to the existing codec budget:

```text
callable_signatures <= 262_144
callable_parameter_groups <= 4_194_304
callable_parameters <= 16_777_216
stream_definitions <= 262_144
external_stream_argument_cells <= collection_items
```

Existing limits remain authoritative for encoded bytes (256 MiB by default),
string bytes, runtime types, collection items, tensor elements, and nesting depth
(64 by default). Every count is checked before allocation and every multiplication
or range end uses checked arithmetic.

## 9. Encode/decode/re-encode parity

For every accepted codec-8 program:

```text
encode(decode(encode(program))) == encode(program)
```

Canonical string remapping visits callable parameter names and external Stream
capability/operation strings. Digest fields are not remapped. A decoder rejects
unknown tags, noncanonical integers, nonzero reserved bits, trailing bytes,
noncanonical table order, and every product invariant violation; it never repairs
them before re-encoding.
