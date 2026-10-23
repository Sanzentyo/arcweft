# Validation report

This is a design-package validation, not a claim that production implementation tests
have already run.

## Source validation

- latest GitHub main was resolved immediately before design as
  `5214a4836d5aa13a934ea8cb7037cc3a2a3c8e31`;
- `REQUEST_COPY.md` reproduces Git blob
  `abcab3da13ddf2241d4a97ea47437de9a1bb7311` exactly;
- `REJECTED_RETURN_INTAKE_COPY.md` reproduces Git blob
  `b95e44abf4fb7f0f7bafd5c0d91d785ecc932a79` exactly;
- all immediate predecessor ZIPs were CRC/manifest checked and opened;
- base Proof and AW-AH-009.4.2 package inventories plus their exact relevant
  normative members were reconstructed and read; and
- the current exact `SyntheticOwner`, `SyntheticRole`, `NotYetLive`, and `Retired`
  schemas were compared with `identity.rs` blob
  `2c5abea32ca7df642522b449af832064bd1dd1ce`.

## Contract checks

The artifact builder verifies:

- 21 unique role rows with contiguous tags `0x01..0x15`;
- tail roles accept exactly `Expr | Scope` and only ordinal zero;
- every tail producer selects an existing eight-variant typed owner, reserves it
  before the child, requests child kind Expr, and names direct tests;
- all six source-ordered roles have direct lowerer, perturbation, and boundary test
  rows distinct from identity tests;
- normative liveness expected payloads contain exact `id`, `snapshot`, `born`, and
  `retired_at` fields; no payload or schema defines `last_live`;
- the retained fingerprint member is byte-identical to the rejected return;
- both 51-byte fixed vectors recompute exactly;
- every test ID and requirement ID is unique;
- every traceability row is `CLOSED`;
- `FINAL_STATUS.md` is exactly `READY_FOR_IMPLEMENTATION` plus newline;
- `OPEN_QUESTIONS.md` is exactly four bytes: `none`;
- every non-manifest member has one exact sorted manifest row; and
- ZIP member names, timestamps, permissions, CRCs, lengths, and SHA-256 values are
  deterministic and valid.

## Implementation evidence required later

`TEST_MATRIX.tsv` is normative implementation work. In particular, each `T-GEN-*`
row must execute the real lowerer/transaction and may not be substituted by a key
constructor unit test or source scan. Normal focused crate tests, workspace
check/Clippy, applicable workspace/Tier-2 tests, and structural audit remain required
when the design is implemented.

## Final artifact table counts

- role/owner/ordinal rows: `21`;
- tail producer rows: `11`;
- affected lowering rows: `9`;
- direct generator fixture rows: `6`;
- focused test rows: `88`;
- requirements traceability rows: `21`; and
- fingerprint fixed vectors: `2`, both exactly 51 bytes and independently recomputed.
