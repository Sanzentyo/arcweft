# Arcweft affine runtime / final-HIR View ABI-1 correction contract

This design-only archive is the mandatory correction and reconciliation layer for:

- `Lang-01.3.1.2.3` affine runtime value owner and capture reconciliation; and
- `Lang-01.5.1.1.2` final-HIR View execution catalog and static certification reconciliation.

It closes the concrete defects found by the 2026-08-10 cross-package audit and applies the direct user decision that the unreleased AWBC ABI number remains **1**. ABI numbering is not used as a compatibility fiction: the ownership-complete instruction and verifier semantics replace the unreleased ABI-1 contract in place. There is no ABI-2 reader, writer, alias, migration layer, or dual validation path.

The package is a correction overlay, not a production patch. The two parent archives remain normative except where `SUPERSESSION_MATRIX.md` says otherwise. An implementer must use this archive together with the exact parent archive identities recorded in `INPUT_IDENTITIES.md`.

## Closed decisions

1. AWBC remains ABI 1 and codec 8; `CopyValue = 0x2a` and corrected existing `Move`/`Drop` semantics are part of the sole ABI-1 contract.
2. Snapshot activation uniqueness is owned by one `RuntimeExecutionActivationAuthority` per runtime execution domain and spans every driver in that domain.
3. The affine owner allocator cursor is persisted exactly and restored without recomputation or reuse.
4. Prepared drop owns the exact value removed from the exact source slot; commit accepts no independent value argument.
5. `RuntimeValueSnapshotV2` is `PartialEq`, not `Eq`; canonical bytes/digests own identity.
6. Current View render/default/state/repeat/nested-call values are unrestricted-only. Handler input is moved once; other View program inputs are checked copies.
7. View product and save use ownership-aware ABI-1 functions and dormant `RuntimeValueSnapshotV2`, never live `RuntimeBinding` Serde.
8. `#[static]` is wire-enforced through serialized static-requirement rows; omission cannot downgrade a required subject to dynamic execution.
9. Overlapping static subjects use deterministic outermost-fragment dispatch; partial overlap is invalid.

`OPEN_QUESTIONS.md` is exactly `none\n`.
