# Opaque owner and value carrier semantics

## 1. Why this is not `Dynamic`

An opaque type is statically known but its producer does not publish a
`RuntimeTypeSchema`/`TypeLayoutHash`. The checked representation therefore
retains exactly the evidence the producer truthfully owns: a typed producer ID
and normalized semantic identity. It never claims structural layout.

`Dynamic` remains the explicit unchecked AWBC type. No opaque projection may
fall back to it, and `Dynamic` does not satisfy an opaque checked type.

## 2. Evidence split

A checked owner can be exact or producer-wide. A concrete runtime value is
always exact. This split prevents an abstract top type such as
`CharacterDialogue::Any` from being serialized as though it were a concrete
value identity.

| Expected owner | Actual checked row | Static compatibility |
|---|---|---|
| exact P/A | exact P/A | accept |
| exact P/A | exact P/B | reject |
| exact P/A | producer-wide P/T | reject |
| producer-wide P/T | exact P/A | accept |
| producer-wide P/T | exact Q/A | reject |
| producer-wide P/T | same producer-wide P/T | accept by equality |
| producer-wide P/T | different producer-wide P/U | reject |

At runtime, exact P/A accepts only an opaque value P/A. Producer-wide P/T
accepts any producer-validated exact opaque value whose producer is P.

## 3. Producer validation

Core does not understand the payload. The producer shall:

1. validate source/host/domain input through its existing typed owner;
2. obtain the compiler-projected exact opaque owner;
3. call `RuntimeOpaqueTypeOwner::try_wrap` only after validation;
4. on decode/restore, require the canonical producer ID;
5. decode and domain-validate the payload;
6. recompute the exact semantic identity from typed decoded facts;
7. reject any mismatch before publishing the value.

This is not an optional core predicate and creates no side table. It is the
existing producer's inherent construction/decode responsibility.

## 4. Native checked acceptance

`RuntimeCheckedType::accepts_value` follows the existing depth limit. Its opaque
branch checks only the owner evidence; producer-domain validation is already a
construction/restore prerequisite. It never recursively interprets payload as
the opaque semantic type. The payload still participates in the ordinary
runtime-value nesting and canonical-encodability traversal so an opaque wrapper
cannot bypass depth or unsupported-value rules.

## 5. Canonical runtime value bytes

Canonical value tag 16 is allocated after the inspected tags 1–15:

```text
u8  = 16
canonical UTF-8 producer ID (existing canonical string encoding)
[32] exact RuntimeSemanticTypeId bytes
canonical RuntimeValue payload
```

Admission is absent because a value is always exact. Existing tags retain their
numbers. A runtime-only payload (Function, iterator, matrix/tensor where the
existing encoder rejects it, or another unsupported value) returns the existing
canonical-encoding error; the wrapper does not convert it to bytes.

## 6. Serde

`RuntimeOpaqueTypeProducerId`, admission, owner, and value derive the same Serde
traits as their enclosing checked type/value. Fields are private. No default,
untagged alternate, alias, flattened legacy form, or `skip_serializing_if` is
permitted. Deserialized opaque values are not published to a typed execution
slot until normal slot/type validation and, where a domain object is reified,
producer decode validation succeed.
