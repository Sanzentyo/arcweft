# Canonical wire, value identity, and restore

## Existing allocation authority

The sole binary allocation owner is
`crates/arcweft-core/src/awbc/codec/types.rs`. This cut allocates zero new
runtime-type tags and zero new constant tags.

Runtime-type tags reused:

- Tuple 0x0a;
- Record 0x0c;
- Variant 0x0d;
- Nominal 0x16 (decimal 22);
- Opaque 0x17 (decimal 23); and
- NominalRecord 0x18 (decimal 24).

Constant tags reused:

- Unit 0x00;
- Tuple 0x0a;
- Record 0x0c;
- Variant 0x0d; and
- Opaque 0x12 (decimal 18).

The AWBC envelope remains AWBC magic `41 57 42 43 0d 0a 1a 0a`, codec
version `01 00`, reserved `00 00`, and an eight-byte little-endian payload
length. That fixed envelope field is not a per-value version. All row lengths,
table IDs, field IDs, and ordinals are shortest base-128 u32 varints. Digests
remain fixed 32-byte atoms.

## In-place row grammar

    AwbcRecordField :=
        field_id:u32-var
        name:option<AwbcStringId>
        ty:AwbcTypeId

    NominalRecordType :=
        0x18
        public_id:AwbcStringId
        semantic_identity:[u8;32]
        layout:[u8;32]
        shape:u8       // Unit=0 Tuple=1 Record=2 Newtype=3
        fields:vec<AwbcRecordField>

    RecordType :=
        0x0c
        public_id:option<AwbcStringId>
        fields:vec<AwbcRecordField>

    NominalVariantIdentity :=
        0x00
        public_id:AwbcStringId
        semantic_identity:[u8;32]
        layout:[u8;32]

    VariantType :=
        0x0d
        owner:AwbcVariantIdentity
        cases:vec<{ name:AwbcStringId, payload:option<AwbcTypeId> }>

    RecordConstant :=
        0x0c
        ty:AwbcTypeId
        fields:vec<AwbcConstantId>

    VariantConstant :=
        0x0d
        ty:AwbcTypeId
        case:u32-var
        payload:option<AwbcConstantId>

For RecordType every name is Some. For NominalRecordType, Unit has no fields;
Tuple has only None names; Record has only nonempty unique Some names; and
Newtype has exactly one None name. Each field ID equals its one-based vector
position. Each case ordinal equals its zero-based vector position.

The old constant field-name vector and case-name field are deleted because the
referenced type row already owns them.

Internal version-1 transcript allocations are also closed. Existing
RuntimeTypeSchema tags occupy 1 through 25; Tuple, Result, RecordValue,
ExactOpaque, and NominalRef use 26, 27, 28, 29, and 30. Existing checked-type
tags occupy 0 through 21; RuntimeCheckedType::Record uses 22. Record shape
uses Unit=0, Tuple=1, Record=2, Newtype=3. Canonical RuntimeValue tags do not
change: Tuple=11, Record=13, Variant=14, NominalRecord=15.

## Version-1 golden fragments

These are exact row fragments. S means 32 bytes of 11; L means 32 bytes of 22.
IDs use the small numeric values shown.

| Shape | Exact hexadecimal fragment |
|---|---|
| unit struct descriptor, public 0 | `18 00 S L 00 00` |
| one-field tuple struct, field 1/type 2 | `18 00 S L 01 01 01 00 02` |
| one-field record struct, field 1/name 3/type 2 | `18 00 S L 02 01 01 01 03 02` |
| newtype, field 1/type 2 | `18 00 S L 03 01 01 00 02` |
| one-item tuple payload type, item type 2 | `0a 01 02` |
| one-field record payload type, name 3/type 2 | `0c 00 01 01 01 03 02` |
| one unit-case nominal variant type, public 0/name 1 | `0d 00 00 S L 01 01 00` |
| one payload-case nominal variant type, payload type 2 | `0d 00 00 S L 01 01 01 02` |
| unit constant | `00` |
| one-item tuple constant, constant 2 | `0a 01 02` |
| one-field record constant, type 3/constant 2 | `0c 03 01 02` |
| unit-case variant constant, type 4/case 0 | `0d 04 00 00` |
| payload-case variant constant, type 4/case 0/constant 5 | `0d 04 00 01 05` |

Implementation tests expand S and L and compare complete vectors. ID or
ordinal 300 is `ac 02`; encodings `ac 82 00`, `80 00`, and any sixth u32
varint byte reject before allocation.

Tuple-zero and record-zero payload type rows are `0a 00` and `0c 00 00`;
their enclosing case payload option is `01 <id>`. A unit case uses `00`.
This proves byte-for-byte that the three empty forms and a one-field tuple do
not collapse.

## Schema/layout transcript

The bounded core transcript is:

    domain = bytes("arcweft.nominal-schema") + 00
    schema_version = u32-var(1)
    root = semantic_identity[32]
    definition_count = u32-var(n)
    definitions = reachable definitions sorted by semantic_identity

Each definition encodes nominal identity bytes, semantic identity, body tag,
shape, ordered field/case count, explicit IDs, semantic names, child schema
tags, and exact opaque owner atoms. NominalRef encodes only typed nominal and
semantic identity. Source spans, environment/catalog digests, display paths,
input ordering, and derived layout hashes are excluded.

Lengths, field IDs, case ordinals, and collection counts call the same
shortest-varint helper as AWBC. Schema and canonical RuntimeValue encoders do
not retain fixed-little-endian u32 copies. Float bits, exact-width integer
payloads, and 32-byte identities retain their fixed widths.

Canonical RuntimeValue identity additionally:

- encodes nominal ID, semantic identity, layout, count, and source-order values
  for nominal records;
- encodes structural record field ID, name, and value in accepted order, never
  sorted by name; and
- encodes nominal variant ID, semantic identity, and layout before
  ordinal/name/payload.

## Snapshot and candidate restore

The outer session save remains strict typed JSON under the existing version-1
envelope. It does not use AWBC binary row tags and gains no per-value version.
Nominal DTOs carry only the current-program reference/evidence: AWBC type ID,
nominal identity, semantic identity, layout, explicit field IDs, ordinal/name,
and recursive values.

Restore order is:

1. outer byte and JSON depth/node/string limits;
2. unknown/duplicate JSON fields and outer version;
3. every type/string/field ID, ordinal, range, and canonical binary varint;
4. nominal/semantic/layout and descriptor shape;
5. private candidate construction through checked record, variant,
   Result/Option, tuple, structural record, and exact opaque constructors;
6. whole fiber/product/task candidate validation for the same program; and
7. complete driver swap.

Any failure before step 7 drops the candidate and exposes no live value or
mutation. There is no compatibility reader, migration table, opaque forwarding
rule, or post-swap validation.
