# arcweft-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction-final-contract

This is the standalone, design-only correction for Lang-01.3.1.2.3.2.1.
It is pinned to repository commit `2585f527b02808305b3a8cab0442eb522e8d0352` and narrowly supersedes the
nominal-record and record-sequence owner portions of the returned
Lang-01.3.1.2.3.2 generic ownership contract. It contains no production patch,
overlay, compatibility constructor, dual reader, or alternate record model.

## Final result

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
PRODUCTION_CHANGES=0
```

The two absent names from the parent are not revived:

- `RuntimeNominalRecordSchema` is replaced normatively by the new core owner
  `RuntimeNominalRecordLayout`.
- `RecordSeqError` is not introduced; record-column admission continues to use
  and extends the existing `RuntimeSeqError`.

`RuntimeNominalRecordLayout` is an immutable executable layout descriptor. It
aggregates the already canonical `RuntimeNominalTypeId`,
`RuntimeSemanticTypeId`, and `TypeLayoutHash` with one defining-order field
projection. It does not copy `RuntimeTypeSchema`, does not replace
`RuntimeNominalRole`, and is never retained by `RuntimeNominalRecordValue`.
Runtime-plan owns `Arc` allocation and sharing; pointer identity is never
semantic identity.

## Read first

1. `FINAL_CONTRACT.md`
2. `RUST_OWNERS_AND_APIS.md`
3. `NOMINAL_LAYOUT_AND_PROJECTION.md`
4. `ERROR_AND_PRECEDENCE.md`
5. `IMPLEMENTATION_ORDER.md`
6. `PRODUCER_CONSUMER_DELETION_INVENTORY.md`
7. `TEST_MATRIX.md`
8. `FINAL_STATUS.md`

Machine-readable closure is in `contract.json`, `SYMBOL_CLOSURE.json`, and
`TEST_MATRIX.csv`. Package checks and an executable behavioral reference model
are under `validation/`.

## Baseline and verification boundary

The requested Git object and exact-commit source pages were inspected, and the
request/evidence files plus targeted source owners were retained as a local
read-only evidence snapshot. A literal local Git checkout could not be
materialized because the available Git transport failed DNS resolution; the
captured failure is hashed in `SOURCE_INPUTS.sha256`. This limitation was not
used to leave a design decision open. The package distinguishes exact inspected
source facts from compile-closure inventory that must be confirmed by the
implementation build.

The foundation at `08bc30c0c8eac77152a42e92a5ca2f83280b94bc` remains accepted. This package does not
redesign runtime IDs, `RuntimeRecordFieldId`, `RuntimeOwnedSlotId`,
`RuntimeValuePath`, their ordering, Serde, or fixed-LE codecs.
