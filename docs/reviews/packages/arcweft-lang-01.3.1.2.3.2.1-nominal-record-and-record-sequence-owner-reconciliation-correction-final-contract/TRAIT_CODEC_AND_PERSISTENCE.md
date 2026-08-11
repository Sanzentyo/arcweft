# Trait, codec, and persistence schedule

## 1. Interim traits

At `2585f527b02808305b3a8cab0442eb522e8d0352`, enclosing live values/plans derive `Clone`, `Serialize`,
and `Deserialize`. To keep each gate compile-clean, these target types retain
matching interim derives:

- `RuntimeNominalRecordLayout` and field;
- `RuntimeNominalRecordExpr` and field expression;
- `RuntimeFieldValue`;
- `RecordSeqField`; and
- existing `RuntimeNominalRecordValue`.

`Eq` is present only where every field supports it. Expression/value carriers
that can contain floating values remain `PartialEq`, not `Eq`. No `Copy`,
`Default`, `Hash`, `Ord`, or pointer-identity behavior is added to the layout or
expression carrier.

## 2. Final removal

The parent affine contract already schedules removal of live-value `Clone` and
Serde after immutable snapshot projection/codec owners replace live carrier
serialization. This correction neither accelerates nor postpones that cut. At
that parent stage, the new types lose traits in the same atomic migration as
the enclosing `RuntimeValue`, `RuntimeSeq`, `RuntimeExpr`, and `RuntimePattern`
requirements. No compatibility boundary preserves them afterward.

## 3. Arc and Serde

`Arc<RuntimeNominalRecordLayout>` is a sharing optimization inside one plan
generation. Serialization is structural. Decoding does not promise pointer
coalescing; plan validation/reinterning may restore sharing, but semantic
behavior cannot depend on allocation identity.

## 4. Runtime canonical bytes

No runtime-value semantic codec changes are authorized:

- anonymous record encoding remains existing anonymous encoding;
- nominal record encoding remains nominal ID + layout + count + layout-order
  values;
- `RuntimeSemanticTypeId`, descriptor names/types, Arc identity, and derived
  field IDs are omitted; and
- anonymous/nominal values remain distinct.

## 5. Accepted identity/path codecs

This correction makes no change to:

- one-based `RuntimeRecordFieldId` strict human-readable Serde;
- `RuntimeRecordFieldId` fixed little-endian bytes;
- `RuntimeOwnedSlotId` variants, ordering, diagnostics, Serde, or fixed-LE;
- ten `RuntimeValuePath` segments, manual ordering, 64-segment limit, Serde, or
  fixed-LE tags; or
- runtime ID/cursor codecs implemented at `08bc30c0c8eac77152a42e92a5ca2f83280b94bc`.

## 6. Plan/bundle/save

The interim plan representation changes structurally because nominal expressions
retain the descriptor and accepted field IDs. This is an internal replacement
on the pinned unreleased surface. Bundle/plan readers migrate atomically; there
is no old/new dual reader.

Saved runtime values do not embed the descriptor. Restore obtains the active
layout from the admitted plan/registration context and calls
`validate_against_layout` before owner traversal or activation.

## 7. AWBC

The parent correction fixes AWBC at ABI 1 and codec 8. This contract adds no
version allocation. AWBC verifier/lowering/VM must carry or reference the same
accepted nominal layout through existing typed plan projection, and exhaustive
matches must fail at compile time until migrated. No ABI-2 compatibility shim
is allowed.
