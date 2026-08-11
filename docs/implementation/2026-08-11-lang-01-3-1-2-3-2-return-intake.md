# Lang-01.3.1.2.3.2 return intake

Date: 2026-08-11

Inspected clean Git baseline:
`d8fbeaa5757fe5836fba17fca35fa104eeb72a1d` on `main`, equal to
`origin/main`.

This intake supersedes the waiting status recorded in
`2026-08-11-lang-01-3-g1-2-generic-ownership-identity-blocker.md`. The blocker
remains historical implementation evidence. The classifier shipped at
`b76465c128322be2d5e66398bc6c30794ca0276f` remains authoritative and is not
redesigned by the returned correction.

## Returned archive

The attached archive was copied without modification to
`docs/reviews/packages/zips/`; its searchable extracted contents are under
`docs/reviews/packages/arcweft-lang-01.3.1.2.3.2-generic-ownership-identity-and-slot-reconciliation-correction-final-contract/`:

- archive:
  `arcweft-lang-01.3.1.2.3.2-generic-ownership-identity-and-slot-reconciliation-correction-final-contract.zip`;
- bytes: 110,434; and
- SHA-256:
  `e95de2a9958000034a48f8c5228c8a4ff17f62226195cce4c0ef93e398c816e4`.

`SOURCE_REQUEST.md` matches the repository request byte-for-byte at SHA-256
`dc9d39578e4706b7b518bc2cfdd37fda33d6be38352007c957e2360704afcf76`.
`OPEN_QUESTIONS.md` is exactly `none\n`; `FINAL_STATUS.md` reports
`READY_FOR_IMPLEMENTATION`, 0 open result-changing decisions, and no production
overlay.

## Integrity and validation performed

- safe ZIP member enumeration: 26 lexical, non-directory entries; no absolute
  path or `..` member;
- archive CRC, deterministic metadata, member-byte parity, and path safety:
  passed through the package validator;
- `MANIFEST.txt`: 25 payload hashes passed; the documented all-zero self row
  was skipped;
- package reference model via `uv run validation/model_checks.py`: 77 checks
  passed;
- package validator via `uv run validation/validate_package.py`: 26 files,
  108 closed symbols, 438 normative test rows, 23 valid goldens, 18 invalid
  binary vectors, and 15 invalid JSON vectors passed; and
- copied repository archive hash equals the attached archive hash.

The validator and reference model are design-package evidence. They do not
claim Arcweft production compilation or satisfy implementation full-gate rows.

## Accepted correction

The package closes the missing G1.2 identity/slot/transaction boundary with:

- core-owned private-field nonzero scalar identities and strict codecs;
- domain-only monotonic execution identity minting;
- one-based accepted record-field IDs;
- execution-wide, nonreused local-slot and occurrence IDs;
- the exact eight-variant diagnostic `RuntimeOwnedSlotId`;
- a ten-segment canonical value path and one shared visitor;
- integrated slot revisions/reservations and staged Copy/Move/Drop;
- an infallible commit boundary after `RuntimeCommitPermit` construction;
- persisted allocator cursors and a twelve-stage restore validation order; and
- one runtime-host-shared execution domain with linear reservation and active
  owners.

The corrected compile-clean order is G1.2-A through G1.2-F. G1.3/G1.4, View
expansion, AWBC wire publication, affine-token minting, and Stream-handle
publication remain blocked until G1.2-F passes.

## Production validation state

No Rust, Cargo, runtime, codec, or product file changes in this documentation
intake. Cargo format/check/Clippy/tests, Tier 2, metadata, and structure audit
were not rerun. Each production cut records its own selected validation and
real test counts.
