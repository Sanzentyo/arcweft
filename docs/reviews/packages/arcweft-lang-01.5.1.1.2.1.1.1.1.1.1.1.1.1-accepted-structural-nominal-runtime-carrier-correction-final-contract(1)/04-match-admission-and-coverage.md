# 04. Match admission and coverage closure

## Complete admission matrix

| Subject carrier | Constraint | Additional checked witness | Result |
|---|---|---|---|
| Structural `S` | Structural `S` | `Direct` | accept, identity projection |
| Structural `S1` | Structural `S2` | any | reject unless checked shape-compatibility canonicalizes both to the same accepted shape |
| Structural `S` | Nominal `N` | none possible | reject; structural data cannot synthesize nominal identity |
| Nominal `N` repr `S` | Nominal same `N` | none | accept |
| Nominal `N<A>` repr `S` | Nominal `N<B>` | none | reject when canonical generic args differ |
| Nominal `N1` repr `S` | Nominal `N2` repr `S` | none | reject even though representation is equal |
| Nominal `N` repr `S` | Structural `S` | valid witness `(N → S)` | accept and execute precompiled projection |
| Nominal `N` repr `S` | Structural `S` | absent, stale, or names another source | reject (stale invariant is an error during plan/restore validation) |
| Nominal `N` repr `S1` | Structural `S2` | witness for `S2` | accept only when the witness was validated against current catalog digest and steps |

## Runtime algorithm

1. Read the sealed carrier variant and checked constraint variant.
2. Compare interned stable semantic IDs; never enumerate fields to decide nominal identity.
3. For direct structural admission, verify the canonical accepted shape ID.
4. For nominal structural projection, resolve the witness ID, verify source/target/digest, and return its projection steps.
5. Return `Rejected` for ordinary domain mismatch; return a typed invariant error only when sealed data is internally inconsistent.
6. Execute arm tests against the admitted projection.
7. Append the deterministic transcript row from stable diagnostic keys and outcome.

## Coverage and transcript closure

The checked compiler creates a single normalized `AcceptedCarrierConstraint` table. Both static coverage and runtime arm selection refer to table indices/digests. The coverage certificate contains:

- subject accepted-domain key,
- ordered arm constraint keys,
- projection-witness keys,
- uncovered-domain proof or exhaustiveness marker,
- semantic digest over the normalized table.

At load/restore, runtime validates that the match plan and coverage/transcript table share the same semantic digest. This prevents a complete static transcript from being paired with a different runtime admission domain.

## Alias/newtype rules

- A transparent type alias is normalized by checked typing before carrier construction; it does not create a fresh nominal identity.
- A nominal/newtype declaration creates a distinct nominal instance even when its representation is identical.
- Representation transparency controls whether a structural projection witness can be emitted; it never controls nominal equality.
- Opaque/external nominal values may carry a nominal identity while refusing structural projection. Their representation shape is the opaque runtime carrier shape required for storage, not permission to destructure.
