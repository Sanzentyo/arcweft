# Proof 01.1.1.4.1.1.1 synthetic-role admission intake

Date: 2026-07-28

Status: `REJECTED_NOT_IMPLEMENTATION_READY`

## Adjudication correction

The archive is mechanically complete, and its fingerprint transcript is
decision-complete, but its overall `READY_FOR_IMPLEMENTATION` claim is not
accepted. A predecessor-body audit after the initial package-self-consistency
check found a result-changing owner contradiction and incomplete production
generator evidence.

The repository correction request is
[Proof 01.1.1.4.1.1.1.1](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction.md).
No `SyntheticKey` admission or fingerprint production API is implemented from
this rejected return while the complete role table remains unresolved.

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

## Result-changing blockers

### Tail owners do not cover accepted HIR body shapes

The returned table makes both `ImplicitUnitTail` and `MissingRequiredTail`
Expr-only. That works for the final ordinary `Block`, `ComputationBlock`,
`NamedBlock`, closure, `If`, and `IfLet` expression families, but it cannot
represent every retained producer:

- base Proof `HirPredicateBody::Block` and `HirProofBody::Block` contain
  `{ scope: ScopeId, statements: Box<[StmtId]>, tail: ExprId }`; the block
  itself has no source-backed `ExprId`;
- base `PROOF_BLOCK.md` requires Unit proof, non-Unit proof, and predicate
  omitted tails to allocate the tail from the block owner; using the tail being
  allocated as its own owner would be circular; and
- final `HirMatchArm` has no independent expression ID, multiple arms can miss
  values under one match expression, and exact-zero `MissingRequiredTail`
  keys owned only by the shared match `ExprId` would collide. Each arm already
  has a distinct typed `ScopeId`.

The correction must therefore give the exact typed owner and allocation order
for ordinary expression tails, predicate/proof block tails, and each match-arm
tail. It must preserve existing final HIR payload shapes and must not restore a
Syntax/raw owner or invent a compatibility carrier. The existing body/arm
scope is the evident typed candidate, but this rejected package does not
authorize that result.

### Generator tests do not exercise production ordering

The archive specifies canonical ordinal generators for `RecoveryOperand`,
`DesugaredTemporary`, `DestructuredBinding`, and `ClosureCapture`, but its
corresponding test rows are identity-table unit tests. Those rows can prove
boolean admission boundaries; they cannot prove semantic child-role order,
source-token plus fixed-recipe order, pattern preorder, first-use capture
order, or independence from map/vector iteration. The corrected matrix must
require direct lowering/transaction tests for those four producer families,
as it already does for postfix candidate preorder.

`TEST_MATRIX.tsv` also describes `T-LIVE-03` using an obsolete "last-live"
field name. The retained and current exact payload is
`Retired { id, snapshot, retired_at }`; the corrected liveness rows must use
the exact `NotYetLive` and `Retired` payload fields.

## Retained non-blocking decisions

The exact-zero/source-ordered ordinal domains, owner-kind-before-ordinal error
precedence, liveness separation, aggregate descendant accounting, and
candidate-only source-backed postfix rules are internally consistent for the
owner rows that are actually representable. They remain useful input to the
correction but are not sufficient to construct the complete final key.

The proposed identity layer emits an opaque, read-only 51-byte fingerprint
transcript:

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
format, raw constructor, or numeric slot accessor. No flaw was found in this
focused transcript design; it may be retained unchanged by the correction.

## Correction and implementation boundary

The package intended to change only role admission, arbitrary-ordinal
predicates, constructor precedence, and fingerprint bytes. The unresolved tail
rows prevent that focused change from becoming authority. The final eight
typed owners, database-qualified IDs, typed source query, Type-owned elision,
AW-AH-009.4.2 source-backed postfix candidate ownership, and deletion-driven
consumer migration remain unchanged.

Implementation remains blocked on Proof 01.1.1.4.1.1.1.1. The deleted
raw-owner `SyntheticKey` is not restored while waiting. No partial admission
table, Syntax/raw owner, alias, wrapper, extension trait, dual reader, source
reparse, source gate, CSS/Takumi path, old Dialogue repair, or
removed-syntax-specific final diagnostic is authorized.

## Intake validation

- every ZIP member was opened and every manifest row was recomputed;
- request, predecessor archives, status, questions, baseline, and archive name
  were checked directly;
- all 21 role rows mechanically partition the eight owner kinds without gaps
  or duplicates, but predecessor consumers disprove two Expr-only rows;
- the role and owner tags are unique, contiguous, and match the complete
  explicit tag tables;
- both 51-byte fixed vectors were independently reconstructed;
- `TEST_MATRIX.tsv` contains `56` unique rows;
- `REQUIREMENTS_TRACEABILITY.tsv` contains `21` rows marked `CLOSED`, but the
  owner and generator findings above invalidate that readiness conclusion; and
- no production source, Cargo manifest, runtime, renderer, Agent, MCP,
  persistence, or codec behavior changed in this docs-only intake cut.
