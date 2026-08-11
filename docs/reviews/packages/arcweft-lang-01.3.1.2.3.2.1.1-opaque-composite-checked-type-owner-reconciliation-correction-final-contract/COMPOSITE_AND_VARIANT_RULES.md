# Composite and variant rules

## 1. Recursive checked-type construction

The projection function is total only for representable shapes. Opaque leaves
are representable when producer evidence is present. The following rules are
normative:

```text
opaque(P, I, A)        => Opaque(owner(P, I, A))
sequence(T)            => Sequence(project(T, SequenceItem))
tuple(T0..Tn)          => Tuple(project(T0)..project(Tn))
choice(T0..Tn)         => Choice(project(T0)..project(Tn))
result(T, E)           => Result { ok: project(T), error: project(E) }
option(T)              => Option(project(T))
project nominal        => parent nominal projection with exact layout
named reference        => MissingOpaqueProducerEvidence
unsupported shape      => UnsupportedRuntimeShape
```

No leaf is changed because it occurs in a selected constructor.

## 2. Empty and uninhabited cases

- `Tuple([])` accepts only `RuntimeValue::Tuple([])`.
- `Choice([])` accepts no value.
- `Never` accepts no value.
- `Sequence(Opaque(...))` accepts an empty sequence; non-empty elements are
  checked individually.
- `Result<Never, E>` and `Result<T, Never>` are legitimate semantic types only
  when they are the actual complete source type. They are never fabricated to
  encode selected `Err` or `Ok`.
- `Option<Never>` admits only `None` through the complete Option owner.

## 3. Recursive generics

A producer-owned opaque nominal such as `Reduction<GameState>` is an atomic
leaf. Its normalized semantic identity includes the accepted nominal ID and all
normalized generic arguments. Projection does not descend into the payload's
structural representation and does not require a schema for `GameState` merely
because it appears under an opaque producer.

A structural recursion that does not cross an opaque producer remains subject
to the existing semantic recursion rejection and runtime nesting limit. There
is no recursive type ID placeholder, `Never` placeholder, mutable fix-up cell,
or post-intern patch.

## 4. Complete variant owners

For one semantic `Result<T,E>`, both `.Ok(value)` and `.Err(error)` lower with
exactly the same checked owner:

```rust
RuntimeCheckedType::Result {
    ok: Box::new(project(T)?),
    error: Box::new(project(E)?),
}
```

The selected case is a validated `(ordinal, RuntimeCheckedVariantCase)`. The
same rule applies to Option and nominal variants. Pattern owners are complete
and identical to scrutinee owners. `RuntimePattern::Variant` never stores a
selected-case refinement.

## 5. Branch merge and subtyping

The semantic checker remains the sole join owner. Exact opaque types join only
according to the producer's existing semantic type relation:

- equal exact identities remain exact;
- distinct exact `CharacterDialogue` identities join to its producer-wide
  `Any` type;
- no generic same-producer widening is invented for accepted opaque nominals;
- otherwise the existing choice/error behavior applies.

AWBC validates the resulting row. It may use
`RuntimeOpaqueTypeOwner::accepts_owner` for assignment/call compatibility, but
it does not compute semantic joins and never widens a variant owner.

## 6. Pattern compatibility

`RuntimeCheckedType::variant_case(ordinal)` validates the case and returns the
canonical name/payload type. Lowering compares authored/resolved case name to
that descriptor before emitting. At execution, owner identity, ordinal, name,
payload presence, and payload checked type must all agree. A payloadless pattern
may ignore an existing payload only where the existing pattern language already
permits it; this does not change owner typing.
