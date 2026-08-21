# Canonical RuntimeValue identity and explicit constant admission

## 1. One grammar, two sinks

The original exhaustive visitor in
`crates/arcweft-core/src/entry/schema.rs` is the sole grammar. It is refactored
from “append bytes to Vec” into:

```rust
fn write_canonical(
    value: &RuntimeValue,
    sink: &mut impl CanonicalRuntimeValueSink,
    work: &mut RuntimeValueCanonicalWork,
) -> Result<(), RuntimeValueCanonicalError>
```

`CanonicalBytesSink` and `CanonicalBlake3Sink` implement only raw byte
consumption and byte accounting. They do not match on `RuntimeValue`. Therefore:

- variant order and tags are identical;
- child order is identical;
- UTF-8, lengths and numeric bit representations are identical;
- recursion/node/byte limits are identical;
- validation order and first errors are identical;
- direct hashing never materializes a second transcript.

`RuntimeValue::try_digest` initializes BLAKE3, runs the same visitor and wraps
the complete output as `RuntimeValueDigest`. It does not hash canonical bytes
through an alternate helper and does not special-case producer arguments.

## 2. Corrected opaque arm

The existing opaque tag and payload order remain unchanged. The corrected
admission before emission is:

```text
producer/type/class/persistence/payload structural validation
< canonical work-limit check
< class:
    Plain          continue
    AffineHandle   RuntimeValueCanonicalError::AffineOpaqueIdentity
< persistence:
    ConstantAndSnapshot  continue
    SnapshotOnly         continue
< emit existing opaque transcript
```

No byte identifies “producer arguments” versus “snapshot diagnostics”. The
value has one identity.

An affine handle may have an adapter/runtime-specific snapshot row only when
that row has a complete typed restore owner. It never receives
`RuntimeValueDigest`, even for non-producer diagnostics, because session-local
handle identity cannot be made stable by changing the caller's purpose.

## 3. Constant fence

Constant publication has stricter semantics than stable value identity.
`RuntimeValue::validate_constant_admission` recursively visits the same value
graph but answers a different question. It rejects:

- `RuntimeOpaquePersistence::SnapshotOnly`;
- `RuntimeOpaqueValueClass::AffineHandle`;
- `RuntimeValue::NeedHandle`;
- runtime/frame-local iterator, range cursor, stream, thread/handle and
  equivalent nonconstant carriers;
- any child containing one of those rows.

The fence is invoked before or together with canonical encoding. It does not
change canonical tags or hashes.

## 4. Current caller migration

The current callers are divided by purpose. The implementation must not
blindly add the constant fence to identity/snapshot callers.

| Current path/owner | Current use | Final call |
|---|---|---|
| `crates/arcweft-core/src/plan/construction/lower.rs` `validate_plan_value` / expression constant | runtime-plan constant publication | `validate_constant_admission` then canonical/type validation |
| `crates/arcweft-dialogue/src/character_dialogue/typed_value.rs` | typed dialogue constant | `try_constant_canonical_bytes` |
| `crates/arcweft-dialogue/src/character_dialogue.rs` | dialogue/config accepted constant | explicit recursive constant fence before catalog publication |
| `crates/arcweft-dialogue/src/character_dialogue/schema.rs` | dialogue schema constant encoding | constant fence then canonical encoding |
| command constant constructors in `arcweft-core` entry/command lowering | accepted command argument/default constant | constant fence then canonical encoding |
| generated constant fixtures for runtime plan/dialogue/command | expected constant bytes | rebuild through the explicit fence |
| `RuntimeValue::try_canonical_bytes` general caller | value identity/debug/snapshot comparison | canonical visitor only; no constant fence |
| `RuntimeValue::try_digest` and `RuntimeValueDigest` | sole identity digest | canonical visitor only |
| `crates/arcweft-core/src/root.rs` semantic/root digest callers | semantic value identity | canonical visitor only |
| Need producer argument construction | producer identity | canonical visitor only after ownership/producer admission |
| `RuntimeValueSnapshotV1` | save/restore | snapshot codec plus structural validation; no constant fence |

During implementation, repository search for
`try_canonical_bytes`, `try_digest`, `contains_nonconstant_opaque`,
`RuntimeExprSeedKind::Value`, constant/default constructors and generated
fixture builders is the admission inventory. Each match must be assigned one
of the rows above; no incidental persistence rejection may remain.

## 5. Paired evidence fixture

The shared fixture is:

```rust
RuntimeValue::Opaque(
    RuntimeOpaqueValue::try_new(
        accepted_producer,
        accepted_semantic_type,
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::SnapshotOnly,
        validated_payload,
    )?
)
```

Required assertions:

```rust
let bytes = value.try_canonical_bytes(canonical_limits)?;
let direct = value.try_digest(canonical_limits)?;
assert_eq!(direct, RuntimeValueDigest::from_blake3(blake3::hash(&bytes)));

let admission = producer_admission.admit_argument(&value)?;
let instance = NeedProducerSpec::try_new(..., direct)?.instance_key()?;

let saved = RuntimeValueSnapshotV1::try_from_value(&value, snapshot_limits)?;
assert_eq!(saved.restore(&catalogs)?, value);

assert_eq!(
    value.validate_constant_admission(constant_limits),
    Err(RuntimeValueConstantAdmissionError::SnapshotOnlyOpaque),
);
```

The same fixture and exact payload are used in all five assertions. Separate
fixtures would not prove the policy split.

## 6. Error precedence

For canonical identity:

```text
recursion cycle/depth
< node count
< variant structural validation
< opaque producer/type/class/persistence/payload validation
< affine identity rejection
< byte limit while emitting
```

For constant publication:

```text
constant-admission recursion/node limit
< first nonconstant child in canonical traversal order
< canonical identity validation/encoding
< publication
```

A constant failure does not alter or cache a different RuntimeValue digest.
