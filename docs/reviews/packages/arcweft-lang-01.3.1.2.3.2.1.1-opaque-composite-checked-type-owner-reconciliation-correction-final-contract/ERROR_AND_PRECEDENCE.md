# Typed errors and deterministic precedence

## 1. Checked-type projection error

Owner: `arcweft_runtime_plan::semantic_facts`.

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCheckedTypeProjectionError {
    #[error("runtime type `{type_label}` has no opaque producer evidence")]
    MissingOpaqueProducerEvidence {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        type_label: String,
    },

    #[error("runtime type shape `{shape:?}` is not representable")]
    UnsupportedRuntimeShape {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        shape: RuntimeUnsupportedTypeShape,
    },

    #[error("project nominal runtime identity is invalid")]
    InvalidProjectNominal {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        reason: RuntimeResolvedNominalError,
    },
}
```

No `String` success/failure return remains on `checked_type`.
`RuntimeUnsupportedTypeShape` is a closed diagnostic enum; it is not a copy of
the recursive type tree and contains no source spelling fallback.

## 2. Accepted catalog errors

`AcceptedNominalRecord::try_new_opaque` validates in this order:

1. producer ID was already successfully constructed;
2. arity is within the existing maximum;
3. path is non-reserved and valid;
4. catalog duplicate/collision checks;
5. catalog capacity/work accounting;
6. atomic publication.

The producer field is mandatory, so there is no `MissingProducer` state inside
a published record. Old direct construction of producerless
`AcceptedNominalSemantics::Opaque` no longer compiles.

## 3. Composite projection precedence

Projection is deterministic pre-order:

1. validate the current node's own owner/shape evidence;
2. sequence item;
3. tuple items by increasing index;
4. choice alternatives by increasing index;
5. Result `ok`, then `error`;
6. Option item.

The first error is returned with the complete typed path. No later branch error
replaces it, and a selected constructor does not suppress an error in the
unselected branch because the complete owner is required.

## 4. Opaque value construction/decode errors

Core construction first rejects `ProducerWide`; an exact owner then wraps the
payload without domain parsing. Producer decode errors precede publication:
wrong producer, invalid payload outer shape, existing producer/domain validation
errors, semantic identity mismatch, then enclosing slot type mismatch. A raw
payload supplied where opaque is expected is simply a closed type mismatch; it
is not auto-wrapped.

## 5. AWBC codec/verifier precedence

1. magic/version/declared lengths and budgets;
2. raw tag validity;
3. table index validity;
4. producer string validity and admission tag;
5. type/constant row shape and cycle/depth checks;
6. instruction/pattern structural checks in table order;
7. control-flow/branch/call/return compatibility;
8. VM runtime-value validation.

Codec 10 fails at version validation before its payload is interpreted as codec
11. An unknown admission tag is an AWBC codec error, not `Dynamic`.

## 6. Restore precedence

Envelope/save version failure precedes AWBC/runtime-value interpretation.
Within a valid schema-3 save, artifact/generation mismatch precedes value type
validation; value nesting/decoding precedes slot acceptance; producer-domain
reification precedes atomic publication. No partially restored state survives.
