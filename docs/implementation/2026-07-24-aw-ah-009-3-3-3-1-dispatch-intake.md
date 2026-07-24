# AW-AH-009.3.3.3.1 correction dispatch intake

Date: 2026-07-24

## Status

`READY_TO_DISPATCH`. This is the only design request that should be sent now:

- [`AW-AH-009.3.3.3.1 Capacity, Dialogue, and overload accounting
  reconciliation`](../reviews/requests/2026-07-24-aw-ah-009.3.3.3.1-capacity-dialogue-and-overload-accounting-reconciliation.md),
  SHA-256
  `BE1D54C1763EEB5F2C76B91ED45970E5C6EA958EC2C3A308190E20F983251EF2`.

The original AW-AH-009.3.3.3 request and its returned archive remain immutable
audit inputs. Do not resend the original request or silently replace its
return. AW-AH-009.3.3.4 has already returned and is implementation-ready; it is
an input to this correction and production work, not another request to send.
The TTS correction remains held under the explicit TTS skip decision.

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

No production Rust behavior is changed by this dispatch cut.
