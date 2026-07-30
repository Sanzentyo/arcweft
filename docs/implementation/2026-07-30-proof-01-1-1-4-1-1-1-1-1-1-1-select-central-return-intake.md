# Proof 01.1.1.4.1.1.1.1.1.1.1 Select central return intake

Date: 2026-07-30

Status: `RETURNED_ACCEPTED_READY_FOR_IMPLEMENTATION_WITH_EVIDENCE_NORMALIZATION`

## Archive identity and mechanical validation

The externally returned archive was inspected at:

```text
D:/sanze/Downloads/arcweft-proof-concurrency-v6.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction-final-contract.zip
```

- byte length: `61,791`;
- SHA-256:
  `E20B646F8914E39E50456C164F2F6BF967376620571335297D3EFF42824213F4`;
- audited repository baseline:
  `004ff3d69f241954eb808985878c348b165a815c`;
- `28` unique, flat, safe ZIP members;
- all `27` intentional non-manifest rows match their recorded byte lengths and
  SHA-256 values;
- `FINAL_STATUS.md` is exactly `READY_FOR_IMPLEMENTATION` plus a newline;
- `OPEN_QUESTIONS.md` is exactly the four bytes `none`;
- the current request, primary request, previous continuation request, and both
  rejected-return intake copies are byte-identical to their repository files;
  and
- all `85` rows in `PREDECESSOR_MEMBER_AUDIT.tsv` were independently checked
  against the four predecessor archives, with zero missing or mismatched
  members.

The archive is mechanically valid. It is not copied into Git. This external
path and digest are the retained archive identity.

## Accepted final E13 contract

The package closes the result-changing Select decisions left open by the two
rejected returns:

- the final payload is exactly
  `HirSelectedMember::Name(HirName) | HirSelectedMember::Missing`;
- the original central `ExpressionProjection` and
  `AttachedExpressionNode` are the sole typed syntax owner, with fixed
  `Target` and `SelectedMember` components;
- `target?.member` and `target?.` retain an inner postfix-Try identity followed
  by an ordinary dot-Select identity; the combined `?.`/`OptionalDot` path is
  deleted;
- a missing Select target is unreachable because leading `.member` remains a
  `ShortVariant`, so E13 allocates no synthetic target child;
- a non-name token after a dot is not an `Invalid` selected member;
  `target.42` consists of an inner missing-member Select followed by ordinary
  outer error recovery;
- a missing member contributes no syntax diagnostic and exactly one
  Select-root HIR recovery diagnostic keyed by the qualified owner;
- a poisoned authored target produces roleful `RecoveredChild(Target)` parent
  poison without copying the child's terminal diagnostic;
- `Whole` remains slot metadata, while `Target` and `SelectedMember` are the
  only Select source components; and
- syntax identities, source bytes, name bytes, HIR expressions, total slots,
  final diagnostics, checked component arithmetic, rollback, retry, and
  deduplication retain their existing independent owners.

This contract supersedes the rejected E13 rows recorded in
[the recovered-member intake](2026-07-29-proof-01-1-1-4-1-1-1-1-1-select-return-intake.md)
and
[the source/producer intake](2026-07-30-proof-01-1-1-4-1-1-1-1-1-1-select-authority-return-intake.md).
The independently throwable
[central projection and accounting request](../reviews/requests/2026-07-30-seq-proof-01.1.1.4.1.1.1.1.1.1.1-select-central-projection-and-accounting-correction.md)
is therefore returned and accepted.

## Repository adjudications

Three package evidence areas require normalization, but none changes the
accepted schema, producer, semantic result, or migration order.

### Fixed `SelectedMember` has no ordinal

`SOURCE_ROLE_AND_QUERY_MATRIX.tsv` row `Q-E13-019` and
`T_Q_13_MATRIX.tsv` row `T-Q-13-016` describe
`SelectedMember[1]`. The same package correctly defines
`HirExprSourceRole::SelectedMember` as a fixed non-ordinal enum variant, which
matches the repository owner. An ordinal-bearing Select wrapper or role would
contradict the accepted source schema and must not be introduced merely to
make this row constructible.

Those two spellings are classified as
`NOT_APPLICABLE_WITH_EVIDENCE`. The general validation-order assertion remains
testable through a real ordinal-bearing role: query the one-segment Path child
of `target.member` with `PathSegment { ordinal: 1 }` and a wrong expected
document, then require `ExprOrdinalOutOfBounds` to precede the document error.
Select-specific tests retain fixed-role applicability, wrong-document, stale
revision, retained-length, rollback, and retry evidence.

### A failed transaction publishes no queryable owner

`SOURCE_ROLE_AND_QUERY_MATRIX.tsv` row `Q-E13-023` and
`T_Q_13_MATRIX.tsv` row `T-Q-13-020` allow either `NotYetLive` or an absent
reservation after rollback. A failed transaction does not return a public
`ExprId`, so a public query outcome for its internal reservation is not an
acceptance surface.

These rows are normalized to `NOT_PUBLISHED_NO_PUBLIC_QUERY`: the failed
transaction returns its typed error, the committed source index contains no
owner or component key from the attempt, and a retry publishes the same
deterministic identities and sites exactly once. No internal reservation ID is
exported merely to select one branch of the package's alternative expectation.

### Consumer inventory paths are descriptive, not source gates

`CONSUMER_DELETION_INVENTORY.tsv` row `M-E13-019` names an obsolete CLI path.
The current relevant consumers are
`crates/arcweft-agent-repl/src/binding.rs` and
`crates/arcweft-cli/src/app/runtime/expectations.rs`. Implementation uses the
current checkout and compile fallout as the consumer inventory. It does not
turn package paths or symbol spellings into a source gate.

These are local, non-result-changing adjudications. They do not justify
another design request or package redelivery.

## Deletion-driven implementation boundary

E13 is no longer design-blocked. Its private implementation may proceed after
the current compiling expression slice is restored to green. The E13 switch
must directly:

1. remove combined `?.` token/Select handling and any `OptionalDot` state;
2. add Select to the original central projection and attachment owner;
3. replace the original final-HIR member field with `Name | Missing`;
4. migrate consumers to the final typed owner; and
5. delete detached HIR-facing Select readers, source fallbacks, invalid-member
   branches, and obsolete constructors while fixing the resulting compile
   failures.

The unrelated flow-statement Select family remains intact. No alias, wrapper,
compatibility reader, dual map, source reparse, source gate, CSS/Takumi path,
or removed-syntax-specific diagnostic is authorized.

The complete Proof public authority switch still waits for the independently
pending E12 Call correction and for all production consumers to be ready for
the same deletion-driven compiling cut. That dependency does not re-block
private E13 implementation or other decision-complete attached/final-HIR
slices.
