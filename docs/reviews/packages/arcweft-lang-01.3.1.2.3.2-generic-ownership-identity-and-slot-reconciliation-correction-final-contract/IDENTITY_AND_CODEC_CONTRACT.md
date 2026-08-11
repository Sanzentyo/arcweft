# Identity, allocation, ordering, and codec contract

## 1. Identity namespaces

The following namespaces are distinct even when raw integers happen to match:

| Namespace | Width | Zero valid | Allocation scope | Reuse |
|---|---:|---:|---|---|
| execution instance | u64 | no | shared runtime domain | never |
| execution reservation (ephemeral) | u64 | no | shared runtime domain/process | never; not persisted |
| dynamic occurrence | u64 | no | one execution | never |
| runtime local slot | u64 | no | one execution | never |
| ownership transaction | u64 | no | one execution | never |
| affine owner | u64 | no | one execution | never |
| record field | u32 | no | one accepted record layout | never within layout |
| local declaration | u32 | no | one executable plan | stable in plan |
| capture slot | u32 | no | one capture plan | stable in plan |
| frame local/lane/packet/cleanup slot | u32 | no | owning occurrence | stable while owner lives |
| slot revision | u64 | no | one slot | monotonic |
| activation epoch | u64 | no | one execution in one domain | monotonic |

A raw value is never sufficient to compare IDs from different wrapper types.

## 2. Allocation algorithms

### 2.1 Generic `RuntimeIdCursor`

```text
initial = Next(1)

allocate(Next(n)):
    result = n
    if n == u64::MAX:
        cursor = Exhausted
    else:
        cursor = Next(n + 1)
    return result

allocate(Exhausted):
    return IdentityExhausted
```

No allocation scans storage for a free number. Cancellation and failure do not
rewind a successful reservation/allocation.

### 2.2 New execution

Under the domain lock:

1. reject an existing reservation;
2. reject a current active execution for `Empty` mode;
3. allocate one reservation ID;
4. allocate one execution ID;
5. create fresh identity cursors at 1;
6. store the reservation record; and
7. return the linear reservation plus dormant candidate.

Steps 3–6 commit together. An error before step 6 consumes neither execution
nor reservation ID. Once step 6 succeeds, dropping the candidate clears the
reservation but does not rewind the execution ID.

### 2.3 Empty restore/replay

The complete image is decoded and validated before the domain is locked.
Suppose the image preserves execution `e`, image next-execution cursor `s`, and
the current empty domain cursor is `d`.

Admission requires:

- no reservation and no active record;
- `s` is `Next(n)` with `n > e`, or `Exhausted` with `e == u64::MAX`;
  unlike execution-local slots, the sole active execution is necessarily the
  domain's last issued execution, because the domain cannot mint another
  execution while it is active;
- `d` has not passed `e`; specifically:
  - `d = Next(m)` requires `m <= e`;
  - `d = Exhausted` rejects; and
- every identity-state cursor passes its own authoritative high-water check;
  `Next(n)` must exceed every currently represented ordinal, while persisted
  `Exhausted` remains valid independently of the live maximum.

The reservation is then made for `e`, and the domain next-execution cursor
becomes `max_cursor(d, s)`. IDs skipped between `d` and `e` are permanently
retired; they are never filled later.

### 2.4 Replacement restore/replay

Replacement requires active `(execution=e, epoch=a)`, candidate execution `e`,
and requested expected epoch `a`. The image next-execution cursor and all
execution-local cursors must be equal to or ahead of the active cursors. The
new active epoch is `a + 1`, checked before swap.

### 2.5 Local slots and shadowing

The runtime plan supplies a `RuntimeLocalDeclarationId`; the executor allocates
a fresh `RuntimeLocalSlotId` every time that declaration is dynamically bound.
The slot cursor is execution-wide, not scope-local and not declaration-local.

```text
outer `x` declaration 3 -> dynamic slot 17
inner shadowing `x` declaration 8 -> dynamic slot 18
inner scope exits -> slot 18 becomes Dropped
later declaration uses spare Vec capacity -> dynamic slot 19
```

Name equality never causes slot reuse. Mutable assignment retains the same slot
and increments its revision.

### 2.6 HIR/sema/runtime-plan projection

At accepted-HIR freeze time, sema/runtime-plan creates a transient sorted map
keyed by `(LocalGeneration, LocalId)`. Values are allocated
`RuntimeLocalDeclarationId`s in deterministic declaration-publication order.

For each accepted closure, a second transient map projects `CaptureId` to
`RuntimeCaptureSlotId` in the parent capture plan's canonical order. The
projection validates:

- every referenced HIR local resolves;
- no HIR local maps to two declaration IDs;
- no declaration ID maps to two distinct HIR locals in one executable plan;
- every capture has one local source;
- capture IDs are unique;
- capture slots are contiguous from 1; and
- the accepted HIR generation matches the plan input generation.

Only the projected IDs and accepted plans are serialized. The maps and HIR IDs
are discarded.

## 3. Record-field allocation

### 3.1 Anonymous records

1. Validate the authored vector in authored order.
2. Reject the first duplicate field name by the second field's authored
   position.
3. Check field count against `u32::MAX` and configured value limits.
4. Assign field ID `index + 1`.
5. Store the vector unchanged.

Thus authored order is accepted runtime order.

### 3.2 Nominal records

1. Resolve every authored initializer to the accepted nominal schema.
2. Reject unknown, missing, and duplicate initializers.
3. Build the existing schema-ordered value vector.
4. Derive field ID `layout_index + 1` when traversing or diagnosing.

No field-ID side vector exists.

### 3.3 Columnar records

Column fields are admitted once, duplicate names are rejected, and IDs are
assigned in the accepted column vector order. All rows share that field
identity.

## 4. Canonical ordering

### 4.1 Scalar/composite IDs

Scalar wrappers compare unsigned numeric values. Composite affine-owner and
transaction IDs compare `(execution, ordinal)`.

### 4.2 `RuntimeOwnedSlotId`

Compare the following tuples:

```text
EnvironmentLocal:
  (0, execution, local)

ClosureCapture:
  (1, execution, closure, capture)

AwbcRegister:
  (2, execution, fiber, frame, register.raw_u32)

AwbcFrameLocal:
  (3, execution, fiber, frame, local)

MailboxLane:
  (4, execution, mailbox, lane)

ChildPacket:
  (5, execution, child, packet)

TransferPacket:
  (6, execution, transfer, packet)

CleanupSlot:
  (7, execution, cleanup_scope, slot)
```

No `Debug`, display string, source order, map iteration, address, or enum
discriminant cast participates.

### 4.3 Canonical rendering

Exact diagnostic rendering is lowercase ASCII:

```text
exec/<execution>/env/<local>
exec/<execution>/closure/<closure>/capture/<capture>
exec/<execution>/awbc/fiber/<fiber>/frame/<frame>/register/<register>
exec/<execution>/awbc/fiber/<fiber>/frame/<frame>/local/<local>
exec/<execution>/mailbox/<mailbox>/lane/<lane>
exec/<execution>/child/<child>/packet/<packet>
exec/<execution>/transfer/<transfer>/packet/<packet>
exec/<execution>/cleanup/<scope>/slot/<slot>
```

Rendering is diagnostic only and is never parsed.

## 5. Canonical human-readable Serde

The manual Serde implementations branch on `Serializer::is_human_readable()`.

### 5.1 Scalar forms

- u64-backed IDs/revisions/epochs: strict decimal JSON strings.
- u32-backed IDs: JSON integers.
- `RuntimeIdCursor`:
  - `{"state":"next","value":"<u64>"}`
  - `{"state":"exhausted"}`

A decimal string:

- is nonempty ASCII digits only;
- has no sign or whitespace;
- has no leading zero;
- is bounded by its integer width; and
- is nonzero for nonzero wrappers.

### 5.2 Composite forms

`RuntimeAffineOwnerId`:

```json
{"execution":"1","ordinal":"7"}
```

`RuntimeOwnershipTransactionId`:

```json
{"execution":"1","ordinal":"9"}
```

`RuntimeOwnedSlotId`:

```json
{"kind":"environment_local","execution":"1","local":"2"}
```

and analogous objects using the exact snake-case kind and declaration field
order. Unknown or duplicate fields are rejected.

`RuntimeValuePath` is an array. Examples:

```json
[
  {"kind":"record_field","field":2},
  {"kind":"sequence_element","index":"4"},
  {"kind":"variant_payload"}
]
```

u64 path indexes are decimal strings; u32 tuple/column indexes are JSON
integers. Index payloads are zero-based. Record/capture payloads are their
one-based typed IDs.

## 6. Canonical binary codec

The canonical binary codec is independent of a general-purpose Serde format.

### 6.1 Primitives

- u32: exactly 4 bytes little-endian.
- u64: exactly 8 bytes little-endian.
- nonzero wrappers: same bytes, with zero rejected on decode.
- enum tag: exactly 1 byte.
- vector length: checked u32 little-endian followed by elements.
- no alignment, padding, varint, platform `usize`, or trailing bytes.

### 6.2 Cursor

```text
tag 0: Next      + u64
tag 1: Exhausted + no payload
all other tags invalid
```

### 6.3 Owned slot

A one-byte variant tag 0–7 is followed by fields in the tuple order in §4.2.
Every execution/occurrence ID is u64 LE. Every AWBC register and owner-local ID
is u32 LE.

### 6.4 Value path

```text
u32 segment_count
for each segment:
    u8 segment_tag
    fixed payload, if any
```

Segment payloads:

| Tag | Payload |
|---:|---|
| 0 tuple element | u32 |
| 1 sequence element | u64 |
| 2 tuple column | u32 |
| 3 record field | u32 nonzero |
| 4 record column | u32 nonzero |
| 5 nominal field | u32 nonzero |
| 6 capture | u32 nonzero |
| 7 variant payload | none |
| 8 iterator remainder | u64 |
| 9 iterator witness state | none |

### 6.5 Identity snapshot

`RuntimeExecutionIdentitySnapshotV2` encodes:

1. execution u64;
2. next-occurrence cursor;
3. next-local-slot cursor;
4. next-transaction cursor; and
5. next-affine-owner cursor.

`RuntimeExecutionDomainSnapshotV2` encodes:

1. next-execution cursor;
2. activation epoch u64; and
3. the active core identity snapshot.

## 7. Strict decoder rejection

All identity/snapshot decoders reject, before allocation or activation:

- zero for nonzero IDs;
- unknown enum tags/states/kinds;
- duplicate/unknown object fields;
- missing required fields;
- a numeric JSON token where u64 decimal string is required;
- sign, leading zero, whitespace, decimal point, exponent, or overflow;
- count multiplication/addition overflow;
- count above the applicable hard/configured limit;
- invalid UTF-8 or BOM;
- trailing bytes;
- non-contiguous record/capture IDs;
- wrong-execution nested IDs; and
- cursor regression against maximum persisted use.

## 8. Digest integration

The existing canonical execution digest gains one section. The preimage is:

```text
u32_le(label_byte_length)
UTF-8 bytes "arcweft.runtime-ownership-identity.v1"
u64_le(section_body_byte_length)
canonical binary RuntimeExecutionDomainSnapshotV2 bytes
canonical binary ordered slot-identity/revision/evidence bytes
```

The section is inserted after immutable executable/program identity and before
mutable runtime payload. The existing digest owner receives this behavior
through its inherent section method. No extension trait or second digest is
introduced.

Source names, source spans, diagnostic names, vector capacity, cache entries,
mutex state, and HIR projection maps are excluded. Record field IDs, slot IDs,
revisions, owner IDs, and allocator cursors are included.

## 9. Golden vectors

`CODEC_GOLDENS.md` and `CODEC_GOLDENS.json` are normative. Implementations must
decode each vector, compare the typed value, re-encode byte-identically, and
reject all listed single-field corruptions.
