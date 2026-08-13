# Canonical plan, generation-contract, checked-type, custom, and AWBC grammars

All byte grammars in this file are version-`1` direct replacements. No version
number changes and no old reader remains.

## 1. Common scalar grammar

Unless a retained Arcweft codec already fixes a stricter encoding:

- unsigned integers are little-endian;
- sequence counts and byte/string lengths are `u32`;
- booleans are exactly `0x00` or `0x01`;
- optional fields use `0x00` absent and `0x01` present;
- identifiers whose owning type is UTF-8 encode as `u32 byte_len || bytes`;
- 32-byte identities/digests encode as exactly 32 bytes;
- every input count is checked against its owner limit before allocation;
- arrays declared canonical must already be strictly sorted and duplicate-free
  on decode; a decoder does not silently reorder hostile input.

## 2. Closed `RuntimeCheckedType` grammar

The generation-contract encoder is owned beside `RuntimeCheckedType`; it is not
Serde/JSON and does not use debug/display spelling.

| Tag | Variant | Payload |
|---:|---|---|
| `0x00` | Never | none |
| `0x01` | Unit | none |
| `0x02` | Bool | none |
| `0x03` | Signed | one retained signed-width tag |
| `0x04` | Unsigned | one retained unsigned-width tag |
| `0x05` | F32 | none |
| `0x06` | F64 | none |
| `0x07` | String | none |
| `0x08` | Char | none |
| `0x09` | Duration | none |
| `0x0a` | EntityRef | none |
| `0x0b` | Bytes | none |
| `0x10` | Sequence | child checked type |
| `0x11` | Tuple | `u32 count`, children in ordinal order |
| `0x12` | Choice | `u32 count`, alternatives in source order |
| `0x13` | Nominal | nominal ID, semantic identity, layout hash |
| `0x14` | Opaque | producer ID, semantic identity, admission tag |
| `0x15` | Variant | nominal ID, semantic identity, case count/cases |
| `0x16` | Result | ok child, error child |
| `0x17` | Option | item child |

A checked nominal does not embed field descriptors. Its key resolves against
the generation catalog during admission traversal.

Each nominal variant case encodes:

```text
u32 name_len
name_utf8
u8 payload_present
[payload_checked_type]
```

Cases are in exact source ordinal order. Empty/duplicate names and an out-of-
range count are errors.

Opaque admission encodes `0` for exact identity and `1` for producer-wide.
Producer-wide is permitted as a static root but never as a concrete opaque
value owner.

## 3. Catalog key and layout grammar

`RuntimeNominalRecordCatalogKey` encodes:

```text
nominal_id
32-byte RuntimeSemanticTypeId
32-byte TypeLayoutHash
```

A layout declaration encodes:

```text
catalog_key
u32 field_count
for each defining-order field:
    one-based RuntimeRecordFieldId as u32
    field_name
    canonical checked_type
```

Field IDs must be exactly `1..=field_count`. The decoder rejects a stored
descriptor whose derived key, field ID, or defining-order structure differs
from its scalar declarations.

The catalog declaration encodes `u32 layout_count` followed by layouts strictly
ordered by catalog key. Equal keys must have byte-identical descriptors; any
duplicate is rejected rather than collapsed.

## 4. Project root grammar

A project root encodes:

```text
32-byte RuntimeProjectRootId
canonical checked_type
```

The root array is strictly ascending by ID. The ID is accepted semantic
evidence created by the runtime-plan bridge; it is not a string hash recovered
by core.

## 5. Generic producer root grammar

A generic producer root encodes:

```text
32-byte RuntimeProducerRootId
canonical checked_type
```

The generic root array is strictly ascending by ID.

## 6. CharacterDialogue role grammar

The role declaration encodes exactly seven entries in enum order:

```text
u8 role_tag
canonical checked_type
```

Tags are Stage `0`, Portrait `1`, Focus `2`, Cleanup `3`, Hook `4`, Style `5`,
RichText `6`.

Admission recomputes Style as:

```text
Choice([
    EntityRef,
    exact RichText checked type
])
```

and compares canonical bytes. The serialized Style field is diagnostic
redundancy, not independent authority.

## 7. CharacterDialogue custom-field grammar and digest

A descriptor encodes:

```text
field_id
canonical checked_type
u8 clearable
u32 accepted_view_count
accepted RuntimeViewId values in strict ascending order
```

The catalog body is:

```text
u32 field_count
descriptors in strict ascending field_id order
```

The runtime digest is:

```text
BLAKE3(
    "arcweft.character-dialogue-runtime-custom-fields.v1\0"
    || catalog_body
)
```

The serialized catalog encodes:

```text
32-byte claimed_digest
catalog_body
```

Admission recomputes the digest before any nominal traversal.

Changing only checked type, clearability, View membership, View order,
field ID, or descriptor count changes the digest or fails canonical-order
validation. Source spans and source-binding coordinates do not appear in this
runtime digest.

## 8. CharacterDialogue producer payload grammar

The payload kind tag is:

- `0x00`: generic checked roots;
- `0x01`: CharacterDialogue.

Generic payload body:

```text
u32 root_count
generic roots
```

CharacterDialogue payload body:

```text
role declaration
custom-field declaration
32-byte RuntimeCharacterCatalogDigest
32-byte RuntimeViewCatalogDigest
```

The enclosing producer contract encodes:

```text
producer_id
u8 payload_kind
payload_body
u32 claimed_authorization_count
catalog keys in strict ascending order
```

For CharacterDialogue, the producer ID must encode exactly
`std.character_dialogue`.

## 9. Generation-contract canonical body

The generation-contract **body** is:

```text
u32 nominal_catalog_byte_len
nominal_catalog_bytes

u32 project_root_count
project_roots

u32 producer_count
producer contracts in strict ascending producer_id order
```

The claimed generation identity is not part of this body.

The identity is:

```text
BLAKE3(
    "arcweft.runtime-generation-contract.v1\0"
    || generation_contract_body
)
```

The serialized declaration is:

```text
32-byte claimed RuntimeGenerationIdentity
u32 body_len
body
```

Admission recomputes the body from parsed typed data, requires canonical parsed
order, computes the identity, and compares it with the claim.

When two raw artifacts are joined, both the 32-byte identity and the complete
canonical body bytes must match.

## 10. `RuntimePlan` Serde shape

`RuntimePlan` retains every current field and adds one required field named:

```text
generation_contract
```

The `.1.2` proposed standalone `nominal_record_catalog` field is not present;
the catalog is nested in `generation_contract`. Producer-only rows are not
present elsewhere.

Serde construction yields raw quarantine data. Field privacy and Serde success
do not imply admission.

## 11. AWBC codec placement

The existing AWBC header remains first and its ABI/codec values remain `1`.
The exact program order becomes:

```text
existing AWBC header
u32 generation_contract_len
canonical serialized generation contract
existing strings table
existing runtime_types table
existing constants table
... every remaining current table in its current order
```

The decoder checks header/version and length limits before allocating or
decoding the generation contract. The generation-contract bytes participate in
the existing AWBC content/product digest at the same level as the tables.

There is no marker for an absent contract and no fallback to plan-side data.

## 12. Plan-to-AWBC equality

Runtime-plan lowering owns one
`RuntimeGenerationContractDeclaration`. The raw RuntimePlan and every AWBC
product derived from it receive clones of that exact declaration.

Acceptance requires:

```text
plan.identity == awbc.identity
&& plan.canonical_body_bytes == awbc.canonical_body_bytes
```

Comparing only identity, catalog keys, or producer IDs is insufficient.

## 13. Voice canonical bytes

Tuple index 5 uses the retained RuntimeValue variant codec and therefore
encodes the actual nested value:

```text
Option/None
Option/Some -> CharacterDialogueVoice/Auto
Option/Some -> CharacterDialogueVoice/Id -> EntityRef
```

No dedicated flat voice tag is allocated. The outer and inner owner, ordinal,
name, payload-presence, and payload bytes all remain visible in canonical
RuntimeValue bytes.
