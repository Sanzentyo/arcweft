# Proof 01.1.1.4.1.1.1.1 tail-owner and generator-evidence intake

Date: 2026-07-28

Status: `ACCEPTED_READY_FOR_IMPLEMENTATION`

## Adjudication

The returned standalone correction is accepted as the final synthetic-role
admission authority. It closes the three blockers recorded against the
rejected Proof 01.1.1.4.1.1.1 return:

1. `ImplicitUnitTail` and `MissingRequiredTail` accept exactly `Expr | Scope`
   owners at ordinal zero. Ordinary expression producers use their already
   reserved source-backed root `ExprId`; predicate and proof block bodies use
   their already reserved body `ScopeId`; each match arm uses its distinct
   already reserved arm `ScopeId`.
2. All six source-ordered role families now require direct production
   lowerer/transaction evidence, ordering perturbation evidence, exact 1,024
   admission, and one-over atomic rollback. Identity truth tables are no longer
   offered as a substitute for generator evidence.
3. Liveness failures use the retained exact payloads
   `NotYetLive { id, snapshot, born }` and
   `Retired { id, snapshot, retired_at }`.

The correction retains the rejected package's non-tail admission and exact
51-byte fingerprint transcript unchanged. It does not restore a raw or Syntax
owner, add a second source reader, or authorize a compatibility path.

## Archive integrity

- repository path:
  `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`
- external intake path:
  `D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction-final-contract.zip`
- ZIP bytes: `50,036`
- ZIP SHA-256:
  `69dc42fc7c985fed638d08d694ed301291a50af3cefa7117321d4219be7e6471`
- members: `23` unique entries
- manifest: `22` intentional non-self rows; every declared byte length and
  SHA-256 matches, with zero missing, extra, duplicate, or mismatched entries
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION` plus newline
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`
- request copy: `8,339` bytes, SHA-256
  `32d566f02c3eaa208edaa92337d1f3b423123821ee437f93806706ed445fb9bd`,
  byte-identical to the repository
  [01.1.1.4.1.1.1.1 request](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction.md)
- rejected-return intake copy: `6,986` bytes, SHA-256
  `3a67412b62105467262b4e28723405354ec144bcf090a7c72d73f1c6ac5e388f`,
  byte-identical to the repository
  [rejected intake](2026-07-28-proof-01-1-1-4-1-1-1-synthetic-role-admission-intake.md)
- audited `main`: `5214a4836d5aa13a934ea8cb7037cc3a2a3c8e31`, exactly the
  intake baseline

All sidecars are inside the ZIP. There is no adjacent summary, status, or hash
file and no production overlay.

## Final tail-owner mapping

All eleven producers allocate an `ExprId` child with ordinal zero. Owners are:

| Producer | Role | Owner reserved before the child |
|---|---|---|
| ordinary block with omitted tail | `ImplicitUnitTail` | block root `ExprId` |
| computation block with allowed omission | `ImplicitUnitTail` | computation root `ExprId` |
| computation block with required result missing | `MissingRequiredTail` | computation root `ExprId` |
| named block with omitted tail | `ImplicitUnitTail` | named-block root `ExprId` |
| closure with required body missing | `MissingRequiredTail` | closure root `ExprId` |
| `if` with omitted Unit-producing else | `ImplicitUnitTail` | `if` root `ExprId` |
| `if let` with one required branch/body component missing | `MissingRequiredTail` | `if let` root `ExprId` |
| predicate block with omitted required Bool tail | `MissingRequiredTail` | body `ScopeId` |
| Unit proof block with omitted tail | `ImplicitUnitTail` | body `ScopeId` |
| non-Unit proof block with omitted tail | `MissingRequiredTail` | body `ScopeId` |
| match arm with missing required value | `MissingRequiredTail` | that arm's distinct `ScopeId` |

For `if let`, the retained lowering classification emits at most one
`MissingRequiredTail` key for the required branch/body component. Other
missing operands remain `RecoveryOperand`; optional absence consumes no key.
This preserves the accepted expression payload while preventing an
exact-zero collision.

The Scope owner changes allocation identity only. The synthetic child remains
an Expr arena slot and its insertion remains in the Expr source index. A tail
never owns itself. Repeating the same `(SyntheticKey, HirIdKind::Expr)` reuses
the same child; any failure rolls the owner, child, source insertion,
diagnostic, and accounting state back together.

## Matrix and retained authority

- `21` unique role rows with contiguous fingerprint tags `0x01..=0x15`;
- `11` tail-producer rows;
- `9` affected-lowering rows;
- `6` source-ordered generator families;
- `88` unique test rows; and
- `21` traceability rows, all `CLOSED`.

The fingerprint member is byte-identical to the rejected return: `4,463`
bytes, SHA-256
`8b8598bf219803819fb0c1077219d0c34274aa90ae19d0412d339fb58d6edb8e`.
Both fixed 51-byte vectors were independently reconstructed. The transcript
is session-qualified identity input only; it is not a decoder, persisted wire
format, portable digest, or authorization to add a hashing dependency.

## Deletion-driven implementation boundary

The package releases two dependency-ordered implementation cuts:

1. The current identity layer can immediately add the complete owner/ordinal
   admission table, private-constructor/read-only `SyntheticKey`, typed
   constructor errors with owner-kind-before-ordinal precedence, and the
   opaque 51-byte transcript. This slice has no reason to restore a deleted
   raw-owner key or repair an old HIR consumer.
2. Tail allocation, source-ordered generators, liveness, descendant
   accounting, and full rollback evidence attach to the final HIR arena and
   transaction owner. They are fully designed but implementation-ordered
   after that owner exists. They must not be simulated by extending the
   provisional `model.rs`, old `HirMatchArm`, or legacy `HirLowerError`.

The eventual public switch remains deletion-driven: migrate every consumer to
the accepted typed arena/source authority and delete the old readers and
provisional payloads in the same compiling cut. No alias, wrapper, extension
trait, dual reader, source-string reparse, source gate, CSS/Takumi path,
removed-syntax-only final diagnostic, or repair of the old
`SpeakerLine`/`ContentCall`/stringly `HirDialogue` path is authorized.

## Intake validation

- every ZIP member was opened and every manifest row was recomputed;
- request and rejected-intake copies, predecessor hashes, status, questions,
  archive name, baseline, and member inventory were checked directly;
- three independent read-only audits compared the role table, all eleven tail
  producers, all six production generators, liveness payloads, and fingerprint
  vectors with the retained predecessors and current `main`;
- all three audits reached `ACCEPT / READY_FOR_IMPLEMENTATION` with no new
  result-changing issue;
- this docs-only intake changes no Rust, Cargo, runtime, render, Agent, MCP,
  persistence, or codec behavior, so Rust tests and Tier 2 are not applicable.
