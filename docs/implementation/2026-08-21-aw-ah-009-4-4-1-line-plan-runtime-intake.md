# AW-AH-009.4.4.1 line-plan runtime package intake

## Intake state

- Date: 2026-08-21
- Inspected Git commit: `d266c6cddc5f7e3ece428666f5397756748134b9`
- Working tree before intake: clean; `main` matched `origin/main`
- Classification: `READY_FOR_IMPLEMENTATION`
- Open implementation-changing questions: 0

The attached archive is retained unchanged at
[`docs/reviews/packages/zips/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract.zip).
Its searchable, byte-identical members are retained under
[`docs/reviews/packages/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract/`](../reviews/packages/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract/README.md).

External source archive:

- path: `D:/sanze/Downloads/arcweft-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation-final-contract.zip`
- byte length: 69,180
- SHA-256: `089B3F610E39BC898DD7096C491DEEB3FB02204EAEF17232BF2763D7DCAFCFF2`

## Performed and passed

- Enumerated 20 file members below one redundant wrapper directory. No unsafe,
  duplicate, or colliding member path was accepted.
- Verified the retained ZIP is byte-identical to the attachment and verified
  every extracted file against its ZIP member.
- Verified all 19 payload rows in `MANIFEST.sha256`; no missing or mismatched
  payload was found.
- Confirmed `SOURCE_REQUEST.md` is byte-identical to
  [`docs/reviews/requests/2026-08-21-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation.md`](../reviews/requests/2026-08-21-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation.md)
  with SHA-256
  `D84FA7828C8CFAD6750B3C7C13DEE5D74E0201337D43D96081DFBA17D5D4B43A`.
- Read the complete baseline, source request, final contract, owner/API shapes,
  runtime-plan and admission contract, structured/AWBC parity contract,
  identity and failure rules, bounded-work rules, save/replay/hot-replacement
  contract, deletion and consumer inventories, implementation interleave,
  requirements traceability, test matrix, and verification record.
- Confirmed the package's inspected baseline is
  `9138efeeabdfca56809e8ad9c16fc85380ae18c5`, `OPEN_QUESTIONS.md` is exactly
  `none`, and every result-changing decision required by the request is closed.

## Authority and precedence

The returned documents are accepted design evidence, not independent user
instructions. The active user request, repository instructions, maintained
documentation, current production, and current `main` take precedence over the
older inspected baseline recorded by the package.

The accepted implementation direction is deletion-driven:

- use direct semantic `TypeKind` authority for stage APIs, line context, and
  scoped handles;
- keep the existing affine `RuntimeValue::Opaque` family as the sole physical
  handle representation and make handles snapshot-only;
- make `LineTaskGroup` the sole plan owner for activation operations, result
  authority, and handle issuance sites;
- replace stringly registration, drop, output, and line-result requests with
  typed schedule, actor, voice, dialogue-result, and commit operations;
- evolve structured, AWBC, save, replay, and hot-replacement boundaries in
  place with every Arcweft-owned version marker fixed at 1; and
- enforce the package's explicit bounded-work limits and structured/AWBC/native
  parity matrix without compatibility readers or source-string recovery.

## Validation not run at intake

No production build, test, Clippy, fixture, codec, VM, save/replay, or parity
command was run for this documentation-only intake cut. Those gates belong to
the implementation cuts and must be recorded from actual command output.

## Next action

Implement the accepted interleave as reviewable deletion-driven cuts, starting
with the semantic and runtime-plan authorities needed to remove the old
stringly line-plan operations before modifying codecs and persistence.
