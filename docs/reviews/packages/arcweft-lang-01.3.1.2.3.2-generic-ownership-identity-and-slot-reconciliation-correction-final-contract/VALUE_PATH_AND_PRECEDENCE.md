# Canonical runtime value path and first-error precedence

## 1. One traversal authority

The existing exhaustive ownership classifier remains the authoritative list of
runtime graph shapes. G1.2 moves its recursive walk into one internal visitor
owned by `arcweft_core::value::ownership`; `RuntimeValue::ownership`,
checked duplication, affine-owner collection, snapshot traversal, and
diagnostics call that visitor.

This is a refactor of traversal mechanics, not a classifier redesign. The
ownership result at the preserved classifier commit remains unchanged.

The visitor is internal and generic over typed callbacks. There is no second
public value enum, path side table, or Stream-specific walker.

## 2. Path root and segments

The root path is the empty segment vector.

| Tag | Rust segment | Payload semantics |
|---:|---|---|
| 0 | `TupleElement(u32)` | zero-based tuple vector index |
| 1 | `SequenceElement(u64)` | zero-based logical element index |
| 2 | `TupleColumn(u32)` | zero-based stored column index |
| 3 | `RecordField(RuntimeRecordFieldId)` | accepted anonymous-record field ID |
| 4 | `RecordColumn(RuntimeRecordFieldId)` | accepted record-column field ID |
| 5 | `NominalRecordField(RuntimeRecordFieldId)` | accepted nominal layout ID |
| 6 | `FunctionCapture(RuntimeCaptureSlotId)` | accepted capture-plan slot |
| 7 | `VariantPayload` | sole variant payload edge |
| 8 | `IteratorRemainder(u64)` | absolute original item index |
| 9 | `IteratorWitnessState` | witness state edge |

Tuple/column/sequence payload zero is valid. Typed record/capture IDs are
nonzero.

## 3. Manual comparison

`RuntimeValuePathSegment` compares:

1. canonical tag;
2. unsigned payload for equal tags.

Payloadless tags compare equal to themselves.

`RuntimeValuePath` compares lexicographically by segments. If all shared
segments compare equal, the shorter path sorts first.

Examples, ascending:

```text
[]
[TupleElement(0)]
[TupleElement(0), VariantPayload]
[TupleElement(1)]
[SequenceElement(0)]
[RecordField(1)]
[RecordField(2)]
[NominalRecordField(1)]
[FunctionCapture(1)]
[VariantPayload]
[IteratorRemainder(4)]
[IteratorWitnessState]
```

Declaration-order discriminants, `Debug`, rendered strings, pointer order, map
iteration, and field names are not used.

## 4. Exact graph traversal

### 4.1 Scalar leaves

These current variants emit only the root node and no child:

```text
Unit
Bool
Int
UInt
F32
F64
MatrixF32
MatrixF64
TensorF32
TensorF64
String
Char
Duration
Range
EntityRef
```

A later affine leaf is also one node at its current path. Adding such a variant
must update the exhaustive visitor and classifier in the same compile-clean
cut.

### 4.2 Tuple

For `RuntimeValue::Tuple(values)`, visit the tuple root, then each value in
stored vector order:

```text
TupleElement(0)
TupleElement(1)
...
```

### 4.3 Ordinary sequence

For `RuntimeSeq::Values(values)`, visit sequence root, then logical elements in
stored vector order using `SequenceElement(index)`.

Indices are encoded as u64. Conversion from `usize` is checked before traversal;
overflow is a budget/identity error before any mutation.

### 4.4 Dense sequence

A dense homogeneous sequence is one unrestricted leaf for ownership traversal.
It emits no per-element child paths. This preserves the shipped classifier and
avoids platform/storage-layout dependence.

Snapshot codecs retain their existing dense payload representation; ownership
paths do not expose internal SIMD/chunk storage.

### 4.5 Tuple columns

For `RuntimeSeq::TupleColumns(columns)`:

1. visit the tuple-column aggregate root;
2. iterate `columns.columns()` in stored column order;
3. enter `TupleColumn(zero_based_index)`; and
4. recursively traverse the child `RuntimeSeq`.

Row-major reconstruction order is not used.

### 4.6 Record columns

For `RuntimeSeq::RecordColumns(record)`:

1. validate field IDs are contiguous and unique;
2. visit aggregate root;
3. iterate fields in accepted stored order;
4. enter `RecordColumn(field_id)`; and
5. recursively traverse the child sequence.

Names decorate diagnostics only.

### 4.7 Anonymous record

For `RuntimeValue::Record(fields)`:

1. validate field IDs are contiguous from 1 and names are unique;
2. visit record root;
3. iterate stored accepted authored order;
4. enter `RecordField(field_id)`; and
5. traverse the field value.

A malformed duplicate-name or ID vector fails before any child is treated as an
owner occurrence.

### 4.8 Nominal record

For `RuntimeValue::NominalRecord(record)`:

1. validate schema/value arity;
2. visit record root;
3. iterate existing `record.fields()` in accepted nominal layout order;
4. derive `RuntimeRecordFieldId(layout_index + 1)`;
5. enter `NominalRecordField(field_id)`; and
6. traverse the field value.

Authored initializer order and field spelling are not visible at runtime.

### 4.9 Function captures

For `RuntimeValue::Function(function)`:

1. visit function root;
2. validate capture slots are contiguous and unique;
3. iterate captures in `RuntimeCaptureSlotId` order;
4. enter `FunctionCapture(capture_id)`; and
5. traverse the live captured value.

A moved/dropped capture is storage evidence and is not a live value child.

### 4.10 Variant payload

Visit the variant root. If payload exists, enter `VariantPayload` and traverse
it. Owner name, variant name, and ordinal do not add path segments.

### 4.11 Iterator values

For `RuntimeIterator::Values { items, index }`:

1. visit iterator root;
2. validate `index <= items.len()`;
3. visit only `items[index..]`;
4. each path uses `IteratorRemainder(absolute_original_index)`; and
5. recursively traverse the item.

The path payload is not `0..remaining_len`. Advancing from index 4 to 5 removes
path `IteratorRemainder(4)` and preserves later absolute identities.

Consumed prefix values do not participate in ownership, duplicate-owner checks,
snapshot candidate traversal, or first error.

### 4.12 Range iterator

A range iterator is one unrestricted leaf and has no child.

### 4.13 Witness iterator

For `RuntimeIterator::Witness { state, .. }`, visit iterator root, enter
`IteratorWitnessState`, and traverse `state`. Witness metadata outside the
runtime value state does not add path segments.

## 5. Node and path accounting

Each entered `RuntimeValue` or `RuntimeSeq` semantic node counts once toward
`value_nodes`. A segment counts when pushed. The root has zero segments but
counts as one visited node.

Path depth is checked before push. At exactly 64 segments the node is accepted;
attempting a 65th segment reports `TooDeep` at the parent path and does not
descend.

Record identity validation work counts toward value-node traversal work but
does not emit synthetic nodes.

## 6. Affine-owner occurrence order

For each affine leaf, emit:

```text
RuntimeAffineOwnerOccurrence {
    owner,
    path: current_path,
}
```

Source slots are traversed in canonical `RuntimeOwnedSlotId` order. Within one
source, paths are emitted in the traversal order above, which is equal to
ascending canonical path order.

The global duplicate-owner detector stores the first occurrence by
`RuntimeAffineOwnerId` and reports the smallest duplicated owner; for that
owner, it reports the two smallest `(slot, path)` occurrences.

## 7. Deterministic graph errors

Within one source slot, malformed graph errors compare:

1. error family:
   - invalid iterator index;
   - invalid record ID continuity;
   - duplicate record name;
   - invalid nominal arity;
   - invalid capture ID continuity;
   - path-depth limit;
2. current canonical path; then
3. offending ordinal/ID.

Across slots, compare slot first.

Graph errors join the transaction precedence rank “type/layout/path/record” and
therefore precede duplicate owner, affine Copy, exhaustion, budget, and
allocation.

## 8. First-error examples

### 8.1 Anonymous versus nominal order

Anonymous source:

```text
{ z: affine_owner(8), a: affine_owner(7) }
```

Accepted authored IDs are `z=1`, `a=2`; first path is
`RecordField(1)` regardless of lexical names.

Nominal layout `[a, z]` with authored initializer order `[z, a]` produces first
path `NominalRecordField(1)` for `a`.

### 8.2 Iterator remainder

At iterator index 3, duplicate owner at original items 2 and 4 ignores consumed
item 2. Only absolute path `IteratorRemainder(4)` is live. If another live slot
contains the same owner, comparison uses that path.

### 8.3 Prefix rule

An affine leaf at tuple element 0 and another nested below tuple element 0 is
not normally possible in one well-formed value, but malformed evidence orders:

```text
[TupleElement(0)]
before
[TupleElement(0), VariantPayload]
```

### 8.4 Error rank beats path

A stale revision in a lexicographically later slot is reported before an affine
Copy in an earlier slot because stale revision has lower precedence rank.
Within stale revisions, the earlier slot wins.

## 9. Snapshot and diagnostic reuse

The snapshot validator uses the same visitor to:

- enumerate live affine owners;
- validate record/capture identities;
- calculate maximum owner ordinal;
- verify cursor continuation; and
- select duplicate/tamper error.

Diagnostics store typed `(slot, path)` and render only afterward. Path rendering
is never parsed to reconstruct evidence.

## 10. Required implementation shape

The owning module adds behavior to existing owners through inherent methods:

```text
RuntimeValue::visit_ownership_graph(...)
RuntimeSeq::visit_ownership_graph(...)
RuntimeIterator::visit_ownership_graph(...)
RuntimeValuePathSegment::canonical_tag()
RuntimeValuePath::cmp(...)
```

The exact internal visitor trait/closure is crate-private. No extension trait is
exported, no helper crate owns traversal, and no enum mirror is generated.
