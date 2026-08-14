# Exact version-1 `RuntimeCheckedType` canonical byte grammar

This file closes the retained parent grammar without changing any tag, meaning, or version. The encoder and decoder are inherent/core-owned behavior beside the existing `arcweft_core::pattern::RuntimeCheckedType`; no extension trait, display/debug string, Serde representation, helper-side type table, or new digest is an authority.

## Exact API and owner

```rust
pub const MAX_RUNTIME_CHECKED_TYPE_DEPTH: u32 = 64;
pub const MAX_RUNTIME_CHECKED_TYPE_NODES: u32 = 65_536;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCheckedTypeCanonicalError {
    #[error("checked-type byte input ended at offset {offset}")]
    Truncated { offset: usize },
    #[error("unknown checked-type tag {tag:#04x} at offset {offset}")]
    UnknownTypeTag { tag: u8, offset: usize },
    #[error("unknown {family} integer-width tag {tag:#04x} at offset {offset}")]
    UnknownIntegerWidthTag { family: RuntimeIntegerWidthFamily, tag: u8, offset: usize },
    #[error("unknown opaque admission tag {tag:#04x} at offset {offset}")]
    UnknownOpaqueAdmissionTag { tag: u8, offset: usize },
    #[error("checked-type depth {actual} exceeds {maximum}")]
    Depth { maximum: u32, actual: u32 },
    #[error("checked-type node work {actual} exceeds {maximum}")]
    Work { maximum: u32, actual: u32 },
    #[error("checked-type length cannot be represented as u32")]
    LengthOverflow,
    #[error("checked-type UTF-8 is invalid at offset {offset}")]
    InvalidUtf8 { offset: usize },
    #[error("checked-type nominal identity is invalid")]
    NominalIdentity { source: RuntimeIdentityError },
    #[error("checked-type opaque producer identity is invalid")]
    OpaqueProducerIdentity { source: RuntimeIdentityError },
    #[error("variant case {ordinal} has an empty name")]
    EmptyVariantCaseName { ordinal: u32 },
    #[error("variant case name is duplicated at ordinals {first} and {second}")]
    DuplicateVariantCaseName { first: u32, second: u32 },
    #[error("checked-type canonical body has {count} trailing bytes")]
    TrailingBytes { count: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeIntegerWidthFamily { Signed, Unsigned }

impl RuntimeCheckedType {
    pub(crate) fn encode_canonical_v1(
        &self,
        output: &mut Vec<u8>,
    ) -> Result<(), RuntimeCheckedTypeCanonicalError>;

    pub(crate) fn decode_canonical_v1(
        input: &[u8],
    ) -> Result<Self, RuntimeCheckedTypeCanonicalError>;
}
```

`encode_canonical_v1` and `decode_canonical_v1` use one private walker/reader with `(depth, consumed_nodes)` state. The original `RuntimeCheckedType` inherent implementation owns all variant dispatch. The original `RuntimeSignedIntWidth`, `RuntimeUnsignedIntWidth`, and `RuntimeOpaqueTypeAdmission` inherent implementations own their canonical tag methods. No parallel helper enum is introduced.

## Common scalars

```text
u8                    = one byte
u32_le(n)             = n.to_le_bytes(), exactly four bytes
bytes32               = exactly 32 bytes, no length prefix
utf8(value)           = u32_le(value.as_bytes().len()) || exact UTF-8 bytes
optional(payload)     = 0x00 | 0x01 || payload
```

All lengths and counts are `u32` little-endian. Encoding fails with `LengthOverflow` before truncation. Decoding checks the count against the remaining 65,536-node budget before allocation. Every decoded nominal or producer string passes its existing checked constructor: it is nonempty and contains no control character. There is no Unicode normalization, case folding, source-path recovery, or display-name lookup.

## Type tags

`RUNTIME_CHECKED_TYPE_TAGS.csv` is normative. The exact tag/payload sequence is:

```text
00 Never
01 Unit
02 Bool
03 Signed      || signed_width_tag
04 Unsigned    || unsigned_width_tag
05 F32
06 F64
07 String
08 Char
09 Duration
0a EntityReference
0b Bytes
10 Sequence    || child
11 Tuple       || u32_le(count) || child[0] ... child[count-1]
12 Choice      || u32_le(count) || alternative[0] ... alternative[count-1]
13 Nominal     || utf8(nominal_id) || semantic_identity[32] || layout_hash[32]
14 Opaque      || utf8(producer_id) || semantic_identity[32] || admission_tag
15 Variant     || utf8(nominal_id) || semantic_identity[32]
               || u32_le(case_count) || case[0] ... case[case_count-1]
16 Result      || ok || error
17 Option      || item
```

Tuple ordinal order, Choice source order, and Variant source ordinal order are preserved exactly. Empty Tuple and Choice vectors retain their typed meaning; they are not silently rewritten. A tree may contain at most 65,536 checked-type nodes and depth at most 64, including the root. A vector count larger than remaining node work fails before allocation. Unknown/reserved tags fail; they are not skipped.

## Integer-width tags

`RUNTIME_INTEGER_WIDTH_TAGS.csv` is normative:

```text
signed:   00 I8, 01 I16, 02 I32, 03 I64, 04 I128, 05 ISize
unsigned: 00 U8, 01 U16, 02 U32, 03 U64, 04 U128, 05 USize
```

The tags are behavior added to the original width enums' inherent implementations. Host pointer width does not alter `ISize`/`USize` bytes; they are semantic width variants, not `usize` serialization.

## Nominal, opaque, and Variant payloads

Nominal payload:

```text
utf8(RuntimeNominalTypeId)
RuntimeSemanticTypeId[32]
TypeLayoutHash[32]
```

Opaque payload:

```text
utf8(RuntimeOpaqueTypeProducerId.as_str())
RuntimeSemanticTypeId[32]
u8 admission       # 00 ExactIdentity, 01 ProducerWide
```

`ProducerWide` is legal static root evidence but cannot own a concrete `RuntimeOpaqueValue`. Unknown admission tags fail.

Each Variant case is:

```text
utf8(case_name)
u8 payload_present # 00 absent, 01 present
[payload checked type]
```

Case names are exact nonempty UTF-8 bytes. Duplicate names are rejected in first duplicate ordinal order; no sorting or normalization occurs. Payload flags other than `00` or `01` fail at their byte offset. The complete type decoder rejects trailing bytes.

## Use in equality

The direct plan/AWBC equality row stores `u32_le(encoded_checked_type_len) || encoded_checked_type`. These are the bytes above. They are compared directly and are not hashed. This grammar remains version 1 and is also the retained generation-contract checked-type grammar.
