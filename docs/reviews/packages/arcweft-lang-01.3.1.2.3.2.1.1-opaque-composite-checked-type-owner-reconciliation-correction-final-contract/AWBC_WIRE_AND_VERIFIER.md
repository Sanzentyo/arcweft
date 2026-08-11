# AWBC wire, verifier, lowerer, and VM contract

## 1. Version allocation

The inspected commit defines `AWBC_ABI_VERSION = 1` and
`AWBC_CODEC_VERSION = 10`. This correction keeps ABI 1 and changes codec to 11.
The parent's codec-8 statement is superseded only because it is not true at the
required commit.

## 2. Runtime type row

```rust
pub enum AwbcRuntimeType {
    // existing tags 0..=22 unchanged
    Opaque {
        producer: AwbcStringId,
        semantic_identity: [u8; 32],
        admission: RuntimeOpaqueTypeAdmission,
    }, // tag 23
}
```

Wire bytes:

```text
u8 23
u32-le producer string index
32 semantic identity bytes
u8 admission (0 exact, 1 producer-wide)
```

The canonical string table owns the producer spelling. Structural validation
checks the string index and validates it through
`RuntimeOpaqueTypeProducerId::try_new`; empty/control/invalid identity fails
before instruction verification.

## 3. Constant row

```rust
pub enum AwbcConstant {
    // existing tags 0..=17 unchanged
    Opaque {
        ty: AwbcTypeId,
        payload: AwbcConstantId,
    }, // tag 18
}
```

Wire bytes are `u8(18) + u32-le ty + u32-le payload`. The referenced type row
must be opaque with `ExactIdentity`; a producer-wide constant is structurally
invalid. The payload index must exist and obey existing acyclicity, depth,
constant-count, and allocation limits. Materialization recursively creates the
payload then calls the resolved exact owner's `try_wrap`.

## 4. Inherent row projection

Behavior is added to the original AWBC type/program owners:

```rust
impl AwbcRuntimeType {
    pub fn try_opaque_owner(
        &self,
        strings: &[String],
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError>;
}

impl AwbcProgram {
    pub fn opaque_owner(
        &self,
        ty: AwbcTypeId,
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, AwbcTypeProjectionError>;
}
```

No free name parser, extension trait, or parallel type table is added.

## 5. Type interning and lowering

`RuntimeCheckedType::Opaque` interns directly to tag 23. Complete Result/Option
owners intern both branches in their single Variant row exactly as today.
`RuntimeResolvedVariant::checked_selection` supplies the complete owner and
selected case; `MakeVariant` receives that owner row. Both `Ok` and `Err` of one
semantic Result therefore reference the same AWBC type ID.

## 6. Compatibility

The existing type compatibility algorithm keeps exact type-ID equality,
explicit `Dynamic`, and existing `Choice` behavior. It adds only:

```text
expected opaque, actual opaque
    => expected_owner.accepts_owner(actual_owner)
```

This relation applies at register moves, frame slots, `MakeVariant` payloads,
pattern bind/test, branch merge validation, function arguments, returns,
captures, and resume/snapshot slot validation. It does not apply between opaque
and nominal/record/variant/dynamic rows and does not add covariance for Variant.

A producer-wide expected row accepts an exact row from the same producer. A
producer-wide actual row is accepted only by exact row equality. The semantic
checker, not the verifier, decides when a branch result is producer-wide.

## 7. `MakeVariant`

Verification order:

1. resolve result register and complete owner type row;
2. require the row is Variant;
3. validate ordinal range;
4. validate canonical case name/string index;
5. validate payload presence;
6. validate payload register type against the declared case payload using the
   compatibility relation;
7. record the complete owner type on the destination.

No selected-case type row or implicit `Never` is emitted.

## 8. Pattern verification

The pattern row stores the complete owner type ID, ordinal, canonical name, and
payload subpattern. Scrutinee compatibility is checked against the complete
owner. The selected case descriptor supplies payload type. A pattern cannot
match a different Result/Option branch type or an opaque value directly unless
the pattern language has an explicit typed binding whose expected opaque row
accepts the value.

## 9. VM parity

The VM opaque branch resolves the AWBC owner and calls the same core acceptance
relation used by native `RuntimeCheckedType::accepts_value`. It never examines
producer strings ad hoc and never parses payload. Constant materialization,
slot ingress, calls, returns, captures, snapshots, and resume validation all
produce/consume `RuntimeValue::Opaque`.

## 10. Tamper rejection

Codec/verifier tests pin unknown runtime type tag, unknown admission tag,
unknown constant tag, invalid producer string index, invalid producer spelling,
producer-wide constant, non-opaque type referenced by opaque constant, cyclic
payload, wrong `MakeVariant` payload, exact identity mismatch, producer
mismatch, and codec-10 input rejection. No fallback reader is permitted.
