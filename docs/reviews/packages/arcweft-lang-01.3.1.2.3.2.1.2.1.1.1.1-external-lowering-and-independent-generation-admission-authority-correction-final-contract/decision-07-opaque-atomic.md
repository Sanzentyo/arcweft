# Decision 07 — opaque payloads are atomic for generic checked validation

The selected semantics are **atomic owner-only**. For
`RuntimeCheckedType::Opaque { owner }` and `RuntimeValue::Opaque(actual)` the
generic checked validator:

1. charges one work unit for the opaque wrapper;
2. checks the outer value variant;
3. applies `RuntimeOpaqueTypeOwner::accepts_opaque_value`;
4. succeeds without inspecting `actual.payload()`.

`ExactIdentity` requires equal producer and semantic identity.
`ProducerWide` accepts any exact value from the same producer. Concrete opaque
values remain exact; `RuntimeOpaqueTypeOwner::try_wrap` continues to reject a
ProducerWide owner.

The validator does not look up a payload type, push a checked or value path,
increment payload depth, or charge payload nodes. Therefore there is no payload
lookup error or role-specific branch. Generic and CharacterDialogue/custom role
values use the same atomic rule; a producer may validate its private payload
before calling the exact wrapper constructor, but that producer-local check is
not part of generic `RuntimeCheckedType` admission.

`RuntimeValuePathSegment::OpaquePayload` tag `10` remains normative for physical
ownership/save/diagnostic traversal. This distinction is explicit in
`CHECKED_PATH_PUSH_RULES.csv`.
