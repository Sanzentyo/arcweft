# Decision 12 — AWBC `MakeRecord` domain coordinate, wire, lowering, verifier, and VM

## Schema owner

Owner: `arcweft_core::awbc::schema`.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct AwbcNominalRecordDomainId(u32);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AwbcNominalRecordDomainDeclaration {
    Project { root: RuntimeProjectRootId },
    Producer { producer: RuntimeOpaqueTypeProducerId },
}

pub enum AwbcInstruction {
    MakeRecord {
        dst: AwbcRegisterId,
        domain: AwbcNominalRecordDomainId,
        ty: AwbcTypeId,
        field_names: Box<[AwbcStringId]>,
        fields: Box<[AwbcRegisterId]>,
    },
}

pub enum AwbcConstant {
    Record {
        domain: AwbcNominalRecordDomainId,
        ty: AwbcTypeId,
        field_names: Box<[AwbcStringId]>,
        fields: Box<[AwbcConstantId]>,
    },
}
```

`AwbcProgram` gains mandatory `nominal_record_domains: Vec<AwbcNominalRecordDomainDeclaration>` immediately after `runtime_types` and before `constants` in the version-1 canonical body. No field default is accepted. The table is canonical by the declaration byte encoding below; duplicates fail before reference validation.

## Canonical table bytes

All integers are little-endian. `str32` is `u32_le(UTF8 byte length) || UTF8 bytes`.

```text
u32_le(domain_count)
repeat declarations in canonical byte order:
    Project:
        u8(0x00)
        RuntimeProjectRootId[32]
    Producer:
        u8(0x01)
        str32(RuntimeOpaqueTypeProducerId canonical public string)
```

Unknown tags fail codec decoding. Empty producer strings, invalid producer IDs, noncanonical ordering, duplicate declarations, count over 65,536, and strings over `u32::MAX` fail before instruction references.

## Opcode payload

The existing `MakeRecord` opcode remains `0x0f`. Its version-1 payload becomes:

```text
u32_le(dst register)
u32_le(domain table index)
u32_le(runtime type index)
u32_le(field_name_count)
repeat field_name_count: u32_le(string table index)
u32_le(field_register_count)
repeat field_register_count: u32_le(register index)
```

The record constant payload uses the same first three coordinates and field-name array, followed by `u32_le(field_constant_count)` and constant IDs. Field-name and field-value counts must match. No optional domain or inference sentinel exists.

## Lowering

- A project nominal expression obtains its exact `RuntimePlanTypedSite`, reads that site's `RuntimeProjectRootId`, interns `Project { root }`, and writes the resulting domain ID.
- An external producer expression obtains its accepted `RuntimeProducerFact`, interns `Producer { producer }`, and writes that ID.
- Lowering never selects a domain from nominal name, semantic ID alone, layout hash, producer string recognition, or whichever catalog contains the key.
- Record constants obey the same rule; a constant without an exact retained site/producer fact is not lowerable.

## Verifier, admission, and VM precedence

1. Decode/header/version/table structure.
2. Verify opcode, register, type, string, field-count, and domain-index references.
3. Require admitted generation and exact `AwbcInstructionSite` root coordinate.
4. Resolve domain declaration: project root or exact producer.
5. Resolve nominal/semantic/layout from `ty` and the admitted catalog.
6. Require domain authorization membership.
7. Validate field count, one-based field-ID derivation, and every checked value in defining layout order.
8. Call crate-private checked construction.
9. Publish `dst` only after success.

The VM borrows the domain from `AdmittedAwbcProduct::nominal_record_domain`; raw `AwbcProgram` never reaches this path. A project row cannot authorize a producer-only key, a producer row cannot authorize another producer, and there is no fallback between variants.
