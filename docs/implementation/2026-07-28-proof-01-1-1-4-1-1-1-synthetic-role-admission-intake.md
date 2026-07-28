# Proof 01.1.1.4.1.1.1 synthetic-role admission intake

Date: 2026-07-28

Status: `ACCEPTED_READY_FOR_IMPLEMENTATION`

## Archive integrity

- Repository path:
  `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1-synthetic-role-owner-admission-correction-final-contract.zip`
- ZIP bytes: `33,968`
- ZIP SHA-256:
  `a9603b3cc758d95dada69310f87a2dc26b7a2ce0ea8b6e0de39de4aa51e75024`
- members: `18` unique entries
- manifest: `17` intentional non-self rows; every declared byte length and
  SHA-256 matches, with zero missing, extra, duplicate, or mismatched entries
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION` plus newline
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`
- request copy: `8,552` bytes, SHA-256
  `c4f7d650f2e0674b81ff19d85216868363be47982fa9cf72fa43996d8f16cf53`,
  byte-identical to the repository
  [01.1.1.4.1.1.1 request](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1-synthetic-role-owner-admission-correction.md)
- audited `main`: `66f9bffa0ec3422c14627fcacd0457b28c28e146`,
  exactly the intake baseline

The retained predecessor hashes were recomputed from repository archives and
match the package: Proof v6.1.1 is `1b7de5f2...`, AW-AH-009.4.2 is
`05e825dd...`, and Proof 01.1.1.4.1.1 is `2bcd3f78...`.

## Accepted correction

The package closes the complete 21-role structural admission table. Exact-zero
roles accept only ordinal `0`. The six source-ordered roles
`RecoveryOperand`, `DesugaredTemporary`, `DestructuredBinding`,
`ClosureCapture`, `PostfixIndexCandidateExpression`, and
`DialogueContentCandidateExpression` accept `0..=1_023`; `1_024` and
`u32::MAX` are rejected. This structural bound is separate from the aggregate
transaction limit of 1,024 live or staged descendants for one exact owner.

The four former syntax owners are replaced directly by final typed HIR owners:

- `ImplicitUnitTail` and `MissingRequiredTail` use `ExprId`;
- `PredicateBoolReturn` and `ProofUnitReturn` use `ItemId`.

Only `RecoveryOperand`, `DesugaredTemporary`, `IfLetScrutinee`, and
`MatchScrutinee` accept both expression and statement owners. No current role
accepts `LocalId` or `CaptureId`; those variants remain part of the final typed
owner vocabulary without forming a valid current key.

`SyntheticKey::try_new` checks owner kind before ordinal, so a doubly invalid
input returns `WrongOwnerKind`; a valid kind with an invalid ordinal returns
`InvalidOrdinal`. Structural construction performs no liveness lookup. The
owning transaction separately preserves
`WrongModule -> NotYetLive -> Retired -> KindMismatch`, staged-owner admission,
checked descendant accounting, exact reuse by `(SyntheticKey, child
HirIdKind)`, and atomic rollback.

The identity layer emits an opaque, read-only 51-byte fingerprint transcript:

```text
"arcweft-hir-synthetic-key-v1\0"
owner tag
database ID as u64 little-endian
module slot as u32 little-endian
HIR slot as u32 little-endian
role tag
ordinal as u32 little-endian
```

Owner tags are exactly `0x01..=0x08`; role tags are exactly
`0x01..=0x15`. Both supplied fixed vectors independently recompute to the
documented bytes. This layer owns no digest algorithm, decoder, persisted wire
format, raw constructor, or numeric slot accessor.

## Precedence and implementation boundary

The package changes only role admission, arbitrary-ordinal predicates,
constructor precedence, and fingerprint bytes. It retains the final eight
typed owners, database-qualified IDs, typed source query, Type-owned elision,
AW-AH-009.4.2 source-backed postfix candidate ownership, and deletion-driven
consumer migration.

Implementation proceeds directly from the already-landed typed owner. The
deleted raw-owner `SyntheticKey` is not restored. No Syntax/raw owner, alias,
wrapper, extension trait, dual reader, source reparse, source gate,
CSS/Takumi path, old Dialogue repair, or removed-syntax-specific final
diagnostic is authorized.

The first coherent production cut adds the final structural key and transcript
with direct typed and compile-fail evidence. Transaction liveness/allocation,
role producers, postfix candidates, source index, and the public HIR authority
switch follow in dependency order; old consumers are deleted in the same
public switch that activates their final replacement.

## Intake validation

- every ZIP member was opened and every manifest row was recomputed;
- request, predecessor archives, status, questions, baseline, and archive name
  were checked directly;
- all 21 role rows partition the eight owner kinds without gaps or duplicates;
- the role and owner tags are unique, contiguous, and match the complete
  explicit tag tables;
- both 51-byte fixed vectors were independently reconstructed;
- `TEST_MATRIX.tsv` contains `56` unique rows;
- `REQUIREMENTS_TRACEABILITY.tsv` contains `21` rows, all `CLOSED`; and
- no production source, Cargo manifest, runtime, renderer, Agent, MCP,
  persistence, or codec behavior changed in this docs-only intake cut.
