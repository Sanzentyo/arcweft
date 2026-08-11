# Lang-01.3 G1.2 generic ownership identity blocker

Date: 2026-08-11

Inspected clean baseline:
`b76465c128322be2d5e66398bc6c30794ca0276f` on `main`, equal to
`origin/main`.

## Outcome

The accepted Lang-01.3.1.2.3 contract and mandatory Lang-01.3.1.2.3.1
correction remain authoritative. Their first safe production subcut is complete:
`RuntimeValueOwnership` and exhaustive inherent runtime-value graph
classification shipped in `b76465c128322be2d5e66398bc6c30794ca0276f`.

Production cannot cross G1.2 without one narrow design correction. The exact
Rust API contract references identity, slot, transaction, and evidence types
whose owners and representations are defined by neither current production nor
any of the three returned packages. Repository and extracted-package searches
found no definitions for `ExecutionInstanceId`, `RuntimeRecordFieldId`, or
`RuntimeLocalSlotId`. G1.2 also uses but does not close the shapes of
`RuntimeOwnedSlotId`, `RuntimeOwnershipTransactionId`, slot revisions,
`RuntimeMovedValueEvidence`, `RuntimeDroppedValueEvidence`, prepared Copy/Move
records, and their commit errors. The mandatory correction additionally
references an undefined `RuntimeFreshExecution` activation owner.

These are not private representation choices:

- `ExecutionInstanceId` is serialized through affine-owner and snapshot
  evidence and keys domain-wide activation. Its allocation and representation
  affect canonical bytes, digests, collision behavior, replay, restore, and
  exclusivity.
- `RuntimeRecordFieldId` selects canonical first-error paths over current
  name-bearing record vectors. Name, layout ordinal, and authored ordinal are
  observably different choices.
- `RuntimeLocalSlotId` selects the identity and lifetime of locals across the
  current name-indexed nested scopes. A global ordinal, `(scope, slot)` pair,
  or HIR-local projection produces different shadowing, capture, revision, and
  plan behavior; `arcweft-core` also cannot depend upward on HIR `LocalId`.
- prepared transaction/evidence identity determines stale-source detection,
  destination validation, error ownership, and whether preflight failure is
  byte-identical.

Choosing any of these locally would freeze result-changing public and
persistent behavior that the returned package says is final.

## Blocking request

The independently throwable correction is
[Lang-01.3.1.2.3.2 generic ownership identity and slot reconciliation](../reviews/requests/2026-08-11-lang-01.3.1.2.3.2-generic-ownership-identity-and-slot-reconciliation-correction.md).

Until it returns, the following are explicit non-goals:

- completing `RuntimeValuePath`, duplication errors, checked duplication,
  value slots, transfer/drop transactions, or affine allocation;
- partially replacing `RuntimePayload` or publishing constant/capture/pattern
  schemas out of the accepted G1 order;
- adding snapshot evidence or activation types with a guessed execution ID;
- assigning record fields or locals ad hoc numeric IDs; and
- touching View, AWBC wire bytes, or Stream handle publication.

There is no safe out-of-order G1.3/G1.4 production cut. Payload eligibility and
errors require canonical value paths and owner IDs; capture and pattern plans
require local-slot identity; constants depend on the closed payload boundary;
snapshot evidence serializes execution/owner identity directly.

No compatibility alias, reduced placeholder enum, dual reader, side table,
source gate, fake token constructor, or renamed owner is introduced.

## Validation evidence

- clean Git identity and `main == origin/main`: passed;
- exhaustive production/document search for the named definitions: none found;
- returned-package search for the same definitions: none found;
- G1.2/G1.3/G1.4 dependency review against `IMPLEMENTATION_ORDER.md` and
  `RUST_OWNERS_AND_APIS.md`: blocked as described above; and
- no production files changed by this blocker analysis.

Cargo, Tier 2, metadata, and structure audit were not rerun for this
documentation-only blocker cut. The immediately preceding production cut
records its own complete validation, including the known final-HIR View test
failures.
