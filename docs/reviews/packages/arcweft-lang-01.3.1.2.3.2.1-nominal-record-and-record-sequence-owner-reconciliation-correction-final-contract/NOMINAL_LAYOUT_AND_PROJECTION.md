# Nominal layout and projection contract

## 1. Authority matrix

| Concern | Sole owner after this correction | Not the owner |
|---|---|---|
| Source declaration/order/generic substitution | sema `CheckedProjectNominal` plus accepted HIR declaration | core value, entry schema |
| Runtime nominal text identity | `RuntimeNominalTypeId` | source span, display label, Arc address |
| Checked instantiated type identity | `RuntimeSemanticTypeId` | value bytes, layout hash alone |
| Exact runtime representation identity | existing `RuntimeTypeSchema::try_layout_hash` result stored as `TypeLayoutHash` | semantic digest alias, direct `from_bytes(schema_digest)`, local BLAKE3 |
| Executable record field layout | `RuntimeNominalRecordLayout` | `RuntimeTypeSchema`, `RuntimeNominalRole`, copied name map |
| Entry persistence/wire schema | `RuntimeTypeSchema` owned by `RuntimeNominalRole` | executable layout descriptor |
| Runtime nominal value | existing `RuntimeNominalRecordValue` | layout descriptor or schema copy |
| Shared allocation | runtime-plan `Arc<RuntimeNominalRecordLayout>` | global mutable registry, value-owned Arc |

## 2. Projection pipeline

```text
accepted HIR struct declaration
        +
sema accepted nominal-record fact / CheckedProjectNominal
        |
        v
compiler RuntimeSchemaProjection
  - apply accepted generic substitution in defining order
  - project one transient RuntimeTypeSchema::Record
  - RuntimeTypeSchema::try_layout_hash()  [sole hash algorithm]
  - for CheckedNominalRole: compare with NominalSchemaDigest bytes
  - project each field to closed RuntimeCheckedType
  - discard transient schema unless it is an explicit entry role
  - build RuntimeResolvedNominal(..., layout)
  - RuntimeNominalRecordLayout::try_from_checked_projection(...)
  - Arc allocation/interning
        |
        v
RuntimeResolvedNominalRecord fact
        |                         |
        v                         v
RuntimeExpr::NominalRecord       RuntimePattern::Record { nominal_layout: Some(...) }
        |
        v
runtime evaluation: authored order -> field-ID scatter -> layout order
        |
        v
RuntimeNominalRecordValue { type_id, layout, fields }
```

No stage serializes HIR/sema owners into core. The bridge performs a one-way
projection. Core has no reverse dependency.

## 3. Layout construction validation

`try_from_checked_projection` applies this fixed order:

1. if `fields.len() > u32::MAX`, `TooManyFields`;
2. scan defining order and return the first repeated name as
   `DuplicateFieldName`;
3. derive every one-based `RuntimeRecordFieldId`, returning the first defensive
   `InvalidFieldIdentity`; and
4. publish the structurally immutable descriptor.

Field-name lexing is not repeated: the input is a checked HIR identifier
projection, not raw source. The constructor does not parse strings, derive
identity from names, or accept a name hash.

The constructor does not recompute `TypeLayoutHash`. This is deliberate: a
second hash grammar inside the record carrier would create a competing layout
authority. The compiler supplies only the output of
`RuntimeTypeSchema::try_layout_hash`; the runtime-plan fact validator requires
that scalar to match `RuntimeResolvedNominal.layout`. Role-backed projections
also require `checked.schema_digest`, projected schema hash, role `.layout`, and
descriptor `.layout` to be bytewise identical.

## 4. Runtime-plan publication checks

For each record expression or record pattern fact:

1. owner ID resolves under the exact HIR snapshot lease;
2. declaration kind and HIR item kind are both struct;
3. `RuntimeResolvedNominalRecord::try_new` proves nominal, semantic, and layout
   scalar equality;
4. descriptor field count equals defining declaration field count;
5. every descriptor field name equals the same defining ordinal after generic
   substitution;
6. every projected checked type is closed and runtime-representable;
7. duplicate fact owners are rejected by existing unique-collection logic; and
8. duplicate interning keys with non-equal descriptors return
   `ConflictingNominalRecordLayout`.

A schema that cannot be canonically encoded fails in compiler projection with
`EntryRuntimeProjectionError::NominalLayoutHash`. A role digest mismatch fails with
`NominalSchemaDigestMismatch`. A nested nominal absent from the already projected
catalog fails fact generation with `UnresolvedNominalLayout`. None may fall
back to nominal ID only, source names, semantic digest bytes, or an ad-hoc hash.

## 5. Initializer mapping

For layout `[a, z]` and authored initializers `[z = e1, a = e2]`, the checked
expression stores:

```text
initializers (authored execution order):
  { field = 2, name = "z", value = e1 }
  { field = 1, name = "a", value = e2 }
```

Evaluation runs `e1` before `e2`, then builds layout-order values `[value(e2),
value(e1)]`. Visitor paths are `NominalRecordField(1)` then
`NominalRecordField(2)` regardless of authored order.

Initializer admission order is fixed:

1. count overflow;
2. per authored entry: duplicate name, unknown name, field-ID mapping;
3. first missing defining-layout field.

For deserialized plan validation, the same scan also compares each stored ID to
the authoritative ID resolved by its name. A mismatch returns
`FieldIdentityMismatch`; there is no name-based repair.

## 6. Runtime value admission

`try_from_accepted_layout` does not repeat name admission. It receives no names.
Its values vector is layout-order by contract. It checks count and each field's
closed value predicate in layout order. On success it copies only
`layout.nominal()` and `layout.layout()` into the existing value carrier.

`validate_against_layout` is the ingress/restore check. Its precedence is:
nominal ID, layout hash, count, field-ID derivation, then first field type.

A nested nominal checked type carries its required layout hash. This prevents
the target repository's current acceptance of a same-name nominal value with a
different layout.

## 7. Pattern mapping

Anonymous patterns keep name-based matching against anonymous diagnostic names.
Nominal patterns use `nominal_layout` as their sole mapping authority:

1. validate value against the descriptor;
2. resolve each pattern field name to a descriptor field ID;
3. index the nominal value by that ID; and
4. recurse into the child pattern.

The current positional `zip(pattern_fields, value.fields)` is deleted. A
pattern field name is never used to fabricate an ordinal; it resolves only
through the accepted descriptor.

## 8. Entry role relationship

`RuntimeNominalRole { identity, layout, schema }` remains intact.
`RuntimeTypeSchema::try_layout_hash` is already the production canonical
schema-to-layout algorithm; this correction makes every compiler projection use
it explicitly. `RuntimeTypeSchema::Record` may contain wire names, defaults,
skip policy, byte format, and persistence details that do not belong to the
executable descriptor. Retaining/copying it there would create equality and
snapshot ambiguity. Therefore:

- descriptor fields contain only semantic diagnostic name and checked runtime
  predicate;
- role schema remains independently validated by entry registration;
- explicit role binding cross-checks checked digest, projected schema hash,
  role identity/layout scalars, and descriptor layout;
- ordinary nominal records use the same transient canonical schema projection
  without creating an entry role; and
- no implicit binding is inferred from matching strings.

## 9. Canonical identity and bytes

The descriptor is plan metadata, not part of value canonical identity. Existing
nominal canonical bytes remain:

```text
nominal tag + RuntimeNominalTypeId + TypeLayoutHash + field count
            + field values in defining layout order
```

No descriptor field names, semantic identity, Arc address, or derived field IDs
are inserted. Anonymous record canonical bytes remain on their existing
anonymous path, preserving anonymous/nominal distinction.
