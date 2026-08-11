# Persistence, Serde, and migration

## 1. Changed serialized shapes

The following public Rust Serde shapes change:

- `RuntimeCheckedType` gains `Opaque { owner }`;
- `RuntimeValue` gains `Opaque(RuntimeOpaqueValue)`;
- `RuntimeOpaqueTypeProducerId`, `RuntimeOpaqueTypeAdmission`,
  `RuntimeOpaqueTypeOwner`, and `RuntimeOpaqueValue` are serializable;
- AWBC runtime type and constant enums gain their new variants.

There are no aliases, defaults, untagged alternates, optional evidence fields,
or legacy flattening.

## 2. Unchanged serialized authorities

- `RuntimeTypeSchema` gains no variant and its canonical grammar/version is
  unchanged.
- `TypeLayoutHash` construction and bytes are unchanged.
- Parent nominal-record bytes retain nominal ID, layout hash, field count, and
  layout-order values.
- AWBC ABI remains 1.
- The outer bundle product discriminator remains `awbc_v1` and the outer bundle
  schema is unchanged by this narrow gap.

## 3. Version cut

| Boundary | Inspected | Final | Reader policy |
|---|---:|---:|---|
| AWBC ABI | 1 | 1 | unchanged |
| AWBC codec | 10 | 11 | codec 10 rejected |
| session save schema | 2 | 3 | schema 2 rejected |
| canonical runtime value opaque tag | absent | 16 | old tags retained |
| AWBC runtime type opaque tag | absent | 23 | old tags retained |
| AWBC constant opaque tag | absent | 18 | old tags retained |

The session save version changes because fibers, registers, captures, and
snapshots serialize `RuntimeValue`. Save schema 3 is one hard replacement. No
migration registry or dual decoder is added.

## 4. Save/restore validation

Restore remains atomic. Required order for each typed slot/value is:

1. envelope magic/version/length/checksum;
2. AWBC artifact and generation identity;
3. table/index/shape validation;
4. runtime-value nesting/canonical decode;
5. expected checked/AWBC type compatibility;
6. producer decode validation when a domain object is materialized;
7. publish the restored session only after all values pass.

An opaque wrapper cannot hide a too-deep or canonically unsupported payload;
normal traversal enters `RuntimeOpaqueValue::payload`.

## 5. Bundle and cache behavior

The bundle continues to embed canonical AWBC bytes under its existing ABI-1
product key; its digest naturally changes because codec-11 bytes differ. Any
cache keyed by exact AWBC/bundle bytes invalidates. No cache translates codec
10 to 11. Debug/Agent JSON that merely observes a validated runtime value is an
output projection, not an accepted authoring decoder; any public JSON snapshot
that serializes `RuntimeValue` must be versioned with save schema 3 or removed
from accepted ingress.

## 6. Deletion cut

At A1.4, delete codec-10 golden writers/readers, save-schema-2 readers and test
fixtures, old exhaustive enum branches, and any fallback that strips an opaque
wrapper to its payload. Old bytes are rejected deterministically; they are not
silently accepted as `Dynamic`, nominal record, or ordinary record.
