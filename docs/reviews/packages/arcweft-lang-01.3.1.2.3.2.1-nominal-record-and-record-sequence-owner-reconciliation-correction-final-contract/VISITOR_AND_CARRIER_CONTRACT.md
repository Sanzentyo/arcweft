# Validated carrier and shared visitor contract

## 1. Persisted shapes

There are exactly three record-bearing value/storage shapes after G1.2-A:

| Family | Persisted child identity | Order |
|---|---|---|
| anonymous `RuntimeValue::Record` | `RuntimeFieldValue.field` | accepted authored order |
| `RuntimeSeq::RecordColumns` | `RecordSeqField.field` | accepted stored column order |
| `RuntimeValue::NominalRecord` | derived by `field_id(ordinal)` | defining nominal layout order |

Names remain diagnostic for anonymous/column carriers. Nominal values carry no
names. No shape has a separate ID vector, side map, or schema pointer.

## 2. Shared path visitor

`arcweft_core::value::ownership` owns one recursive, path-aware traversal. Its
record branches use only the inherent APIs of the owning carriers:

```text
RuntimeValue::Record(fields):
    child path += RuntimeValuePathSegment::RecordField(field.field())

RuntimeSeq::RecordColumns(record):
    child path += RuntimeValuePathSegment::RecordColumn(field.field())

RuntimeValue::NominalRecord(record):
    for (ordinal, child) in record.fields().iter().enumerate():
        id = record.field_id(ordinal) // invariant-backed, no name input
        child path += RuntimeValuePathSegment::NominalRecordField(id)
```

The exact accepted `RuntimeValuePath` segment enum, ordering, maximum length,
Serde representation, and fixed-LE tags are unchanged.

## 3. No fallback rule

The visitor must not:

- hash or sort a field name;
- reconstruct IDs by searching a schema;
- use source order metadata outside the carrier;
- assume `ordinal + 1` in a consumer instead of calling the owner method;
- read an optional old carrier shape;
- consult runtime-plan, HIR, sema, or entry role objects; or
- retain a second name-to-ID table.

Malformed deserialized carriers are rejected by ingress validators before the
visitor runs. The visitor does not repair data.

## 4. Consumer convergence

The following behaviors delegate to the same recursive owner or a single
owner-provided iterator/callback built on it:

- affine ownership classification;
- copy/move/borrow preparation;
- duplicate-owner detection and deterministic path ranking;
- nesting/depth and node-count accounting;
- snapshot owner graph collection;
- root and replay value validation that requires child paths;
- save/restore ownership capture; and
- diagnostics that report structured paths.

A consumer may filter events emitted by the visitor, but may not implement a
second recursive record walk.

## 5. Column reconstruction

`RecordSeq::into_values`, `tail_from`, `value_at`, literal materialization, and
any row reconstruction copy the stored `RuntimeRecordFieldId` together with
name and child value. They do not call anonymous admission again and do not
regenerate an ID from the name.

When record literal rows can be columnarized, every row must have identical
field IDs, names, and order. A mismatch falls back to the existing values
sequence behavior; it never silently normalizes by name.

## 6. Nominal pattern distinction

Nominal pattern name resolution is not part of this visitor. It is performed
through the sole `RuntimeNominalRecordLayout` before indexing a nominal value.
Visitor paths remain purely ID-based and never use that name lookup as a
fallback.
