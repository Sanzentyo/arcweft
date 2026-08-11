# AW-AH-009.3.3.3.1 correction return intake

Date: 2026-07-24

## Status

`RETURNED_ACCEPTED_WITH_PRECEDENCE_ADJUDICATION`.

The returned archive is retained at:

- [`arcweft-aw-ah-009.3.3.3.1 Capacity, Dialogue, and overload accounting
  reconciliation`](../reviews/packages/zips/arcweft-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation-final-contract.zip),
  SHA-256
  `060332BC62273C34F267F0F15767FE6BBD328BE177CB8035E83F210267AB0D41`,
  30,748 bytes.

The ZIP has 10 members. All 9 non-manifest payload hashes and lengths match
`MANIFEST.txt`, `OPEN_QUESTIONS.md` is exactly `none`, and
`FINAL_STATUS.md` reports `READY_FOR_IMPLEMENTATION`. The externally supplied
summary and status files match the archive identity/status and were used only
for intake comparison. Under the current package policy, sidecars are canonical
inside the ZIP and the adjacent copies are not retained in the repository.

The return closes the requested staged family classification, Capacity and
Dialogue phase transition, Speaker deletion, and physical-versus-retained
overload-accounting decisions. It does not authorize final Dialogue credit
before the Proof-backed AW-AH-009.4.2/.4.3 public switch.

One internal row is a package-local matrix error: `TEST_MATRIX.md` `CAP-005` requires bare
`Vec.with_capacity(8)` to be accepted, while the same package says
AW-AH-009.3.3.4 remains fully authoritative and that authoritative package
requires a typed generic-arity failure with no placeholder or candidate. The
implementation follows AW-AH-009.3.3.4 T08/C17. This is not an open design
choice: the returned package's own precedence section makes AW-AH-009.3.3.4
fully authoritative for Capacity and says this package adds only phase and
evidence interpretation. `CAP-005` is therefore recorded as superseded without
creating or dispatching another request.

The original request remains the immutable dispatch record:

- [`AW-AH-009.3.3.3.1 Capacity, Dialogue, and overload accounting
  reconciliation`](../reviews/requests/2026-07-24-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation.md),
  SHA-256
  `BE1D54C1763EEB5F2C76B91ED45970E5C6EA958EC2C3A308190E20F983251EF2`.

The original AW-AH-009.3.3.3 request and its returned archive remain immutable
audit inputs. Do not resend the original request or silently replace its
return. AW-AH-009.3.3.4 is implementation-ready production authority and must
not be resent. The TTS correction remains held under the explicit TTS skip
decision.

## Why the correction is required

The AW-AH-009.3.3.3 return is mechanically valid but cannot be accepted as a
whole. Its CapacityMethod row contradicts the accepted typed
`variadic_unchecked` contract, its Dialogue row uses the frozen carrier instead
of the final typed authority, and its overload wording conflates physical
candidate evaluation with retained inference. Speaker is truthful only as a
current-phase observation and earns no final-completion credit; the final
Dialogue authority switch deletes that family and ID.

The correction request fixes separate current-observation and final-model
evidence axes:

```text
pre-capacity:                 current 21/42; final-model 20/40
post-capacity/pre-Dialogue:   current 22/44; final-model 21/42
post-Dialogue/Speaker delete: current 22/44; final-model 22/44
```

## Package verification

All dispatch inputs exist at the paths listed in the request and match their
recorded outer SHA-256 values. The five newly retained returned/final packages
were also read to completion and checked against their internal manifests:

- AW-AH-009.3.3.3: 9 members, 8 non-manifest payloads verified;
- AW-AH-009.3.3.4: 10 members, 9 non-manifest payloads verified;
- AW-AH-009.4: all 19 members verified;
- AW-AH-009.4.2: all 16 members verified; and
- AW-AH-009.4.3: all 17 members verified.

The new AW-AH-009.3.3.3.1 return was independently rehashed and verified as 10
members with all 9 non-manifest payloads valid.

No production Rust behavior is changed by this dispatch cut.
