# Lang-01.3 affine runtime-value substrate blocker

Date: 2026-08-08

Inspected clean baseline:
`177ba1e61e43fb2da2149869ce35e165d1e93b66`.

## Outcome

Lang-01.3.1.2.1, Lang-01.3.1.2.2, and Lang-01.3.1.2.2.1 remain returned,
verified, and authoritative. Their group-aware callable/product and reconciled
opcode decisions are implementation-ready, and `0x27`, `0x28`, and `0x29` are
still collision-free on current `main`.

Production implementation cannot cross the P4+C1 core publication boundary yet.
The returned contracts rely on an existing ABI-2 affine runtime-value owner, but
current production exposes unconditional cloning through `RuntimeValue`,
`RuntimePayload`, `RuntimeBinding`, closures, environments, iterators,
aggregates, AWBC fibers, and snapshot candidates.

A diagnostic deletion-driven edit removed `Clone` from the direct value/
closure/aggregate owners. `cargo check -p arcweft-core --lib --jobs 4` then
reported 322 compile errors before downstream crates. The important exposed
semantic gaps were:

- closure construction clones the complete environment through
  `bindings_snapshot()`;
- partial application clones existing captures and arguments;
- iterator-next clones sequence elements;
- sequence repeat/get/slice and generic plan/runtime paths assume duplicability;
- RuntimePlan/AOT/AWBC/fiber/snapshot carriers clone values without an ownership
  classification or transfer graph.

Those errors are useful inventory, but selecting capture, indexing, slicing,
snapshot, and verifier semantics would change language/runtime behavior and is
not closed by the returned Stream packages. The diagnostic production edits
were removed with `apply_patch`; the checkout returned to the pushed baseline,
and `cargo check -p arcweft-core --lib --jobs 4` passed.

## Blocking request

The independently throwable correction is
[Lang-01.3.1.2.3 affine runtime value owner and capture reconciliation](../reviews/requests/2026-08-08-lang-01.3.1.2.3-affine-runtime-value-owner-and-capture-reconciliation.md).

Until that contract returns, the following are explicit non-goals:

- publishing `StreamHandle` inside the cloneable `RuntimeValue` enum;
- adding `ExternalStreamPartial` while closure capture still clones the whole
  environment;
- inventing a Stream-only affine registry or a panic-on-Clone implementation;
- partially switching RuntimePlan, AWBC, host, bundle, or save formats; and
- deleting Source before its sole Stream replacement can satisfy affine
  ownership and snapshot identity end to end.

No compatibility alias, dual reader, source gate, removed-syntax diagnostic,
CSS path, or Takumi path is introduced.

## Validation evidence

- `cargo check -p arcweft-core --lib --jobs 4`: passed after removing the
  diagnostic edits.
- Git status before this documentation cut: clean.
- No Tier 2 or workspace-wide validation is claimed for a documentation-only
  blocker record.
