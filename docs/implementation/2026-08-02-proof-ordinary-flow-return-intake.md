# Proof 01.1.1.2.1.1 ordinary Flow returned-package intake

Date: 2026-08-02

Status: `RETURNED_REJECTED_NOT_READY_FOR_IMPLEMENTATION`

## Archive identity and integrity

The externally returned archive was inspected at:

```text
D:\sanze\Downloads\arcweft-proof-concurrency-v6.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation-final-contract.zip
```

- byte length: `21,466`;
- SHA-256:
  `F4F18E08B7D5A561B352D8B344734F7E892B290EC6D276DECF2A90F4F4D4FF3E`;
- 15 file members, with no unsafe path, duplicate member, case-folded path
  collision, or directory member;
- the archive fully decompresses;
- all 14 rows in `MANIFEST.sha256` match their member hashes; and
- `AUTHORITATIVE_REQUEST.md` exactly matches the repository request with
  SHA-256
  `05930333688E38C28397F73B952CE7D5C2798AA23282B5F6870AD733ACB05F2C`.

The archive declares `READY_FOR_IMPLEMENTATION`, and `OPEN_QUESTIONS.md`
contains exactly `none` plus its final newline. The archive is not copied into
Git. Its external path and digest above are the retained identity of this
rejected return.

## Adjudication

The repository rejects the package's self-status. It is mechanically readable
but does not contain the decision-complete contract requested by
Proof-concurrency v6.1.1.2.1.1.

1. `SUMMARY.md` and `SOURCE_LEDGER.md` say the repository, remote, baseline
   commit, worktree, `AGENTS.md`, every required predecessor, and every current
   consumer were unavailable. The inspected revision is recorded as
   `UNAVAILABLE`. The package therefore did not perform the mandatory
   latest-`main` reconciliation.
2. `REPOSITORY_EVIDENCE.md` contains no repository evidence. Every named
   repository reference in `SOURCE_LEDGER.md` is marked `present False`.
3. The archive omits required standalone members, including `README.md`, the
   parent request copy, `PRECEDENCE_AND_NON_GOALS.md`, exact Rust-facing
   attached-syntax and final-HIR schemas, the complete contract-clause table,
   the exhaustive Flow-item lowering/source/scope/recovery/limit matrix, poison
   precedence, and a current-consumer deletion inventory.
4. `FINAL_CONTRACT.md` only states generic single-owner and rollback rules. It
   does not select exact records, enums, fields, visibility, constructors,
   source roles, scope owners, or allocation order for ordinary Flow.
5. None of the four Flow identity states, omitted-return storage, semantic
   `Unit`, `ensures` result local, second-parameter recovery, or malformed
   `where` rows receives an exact schema or source projection.
6. None of `requires`, `ensures`, `invariant`, `assume`, `reads`, `effects`,
   `no_effect`, `modifies`, or `decreases` receives an admission, payload,
   scope, recovery, diagnostic, or accounting row.
7. None of the required `HirThreadFlowItem` variants receives a normative
   attached kind, child-ID, scope, source, recovery, contextual-admission, or
   freeze row. The package consequently does not close the shared ordinary
   Flow/Thread body boundary.
8. `ACCEPTANCE_MATRIX.md` has only 12 generic cases and does not provide the
   request-mandated identity, signature, clause, body, Flow-item, project,
   stale/foreign, exact-limit, or first-one-over matrix.
9. `IMPLEMENTATION_SEQUENCE.md` does not name current consumers and tells the
   implementer to discover the missing authority. That leaves the requested
   deletion order result-changing and open.
10. `MANIFEST.sha256` records hashes but omits the required byte length for
    every non-self member.

Incorporating the unfulfilled request by reference does not resolve its
questions. `READY_FOR_IMPLEMENTATION` and `OPEN_QUESTIONS=none` are therefore
unsupported.

## Implementation effect

- ordinary Flow attached syntax/final HIR: `DESIGN_BLOCKED`;
- `ThreadExpression` header and shared statement-only `HirThreadBody` public
  projection: `DESIGN_BLOCKED`;
- Proof public HIR/project/compiler/LSP authority switch: remains open until a
  corrected Flow package is accepted;
- already accepted Proof arenas, typed IDs, source queries, Call, Select,
  Dialogue application, and repository-local E34 candidate work remain valid;
- `Block`, `ComputationBlock`, and `NamedBlock` candidate lowering may proceed
  because they do not select a Flow/Thread schema; and
- no old Flow reader may be repaired or wrapped while waiting.

The independently throwable full-redelivery correction is:

- [`2026-08-02-seq-proof-01.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction.md`](../reviews/requests/2026-08-02-seq-proof-01.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction.md)

The corrected assignment must continue from current GitHub `main` and close
the concrete schema and matrix rows; it must not return this request again as a
generic adopted contract.
