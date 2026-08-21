# Lang-01.5.1.1.2.1.1 reactive unary Need correction intake

Date: 2026-08-21
Inspected Git commit: `a1a098976b39cbba09c527369193a5c5d4fc816a`
Working tree before intake: clean; `main` matched `origin/main`
Supersedes: `2026-08-21-lang-01-5-1-1-2-1-reactive-unary-need-return-intake.md`

## Intake result

- Classification: `ACCEPTED_DESIGN_CONTRACT`
- Initial design readiness: `READY_FOR_IMPLEMENTATION` (superseded by the
  post-intake implementation reconciliation below)
- Production implementation claim: none
- Open questions: none
- Previous design-validation blocker: cleared; new ABI correction blocker open

Post-intake implementation reconciliation found that product/runtime readiness
was overstated: the returned generic-Match output schema exports a
function-local AWBC register across a destroyed callee frame, and its typed
Need producer claim conflicts with the current string-backed NeedHandle path.
The language/lifecycle contract remains accepted, but product/runtime work is
now `BLOCKED_PENDING_CORRECTION` by
[`Lang-01.5.1.1.2.1.1.1`](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction.md).
Only the generic checked Match semantic-authority cut is independently safe;
see the
[implementation blocker](2026-08-21-lang-01-5-1-1-2-1-1-1-generic-match-and-typed-need-producer-abi-blocker.md).

The corrected return closes the failed package's missing design, evidence,
traceability, matrix, implementation-order, and verification requirements. It
selects one generation-bound checked View catalog, one typed unary Need
subscription, a deterministic generation/Need journal, ordinary generic Match
and AWBC/RuntimeValue execution, strict version-1 replacement, and complete
deletion of the unreleased direct View Await path.

The package is retained unchanged at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract.zip).
Its searchable frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract/README.md).

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1-reactive-unary-need-match-design-validation-correction-final-contract.zip`
- byte length: 98,997
- SHA-256: `A7E146CD8F263127FE36EE29D10B24B118F8717BFC900BB88E957D3D863E30F4`

## Performed and passed

- Verified 41 safe file members below one redundant top-level directory; no
  absolute, parent-traversal, drive-qualified, duplicate, or colliding path was
  accepted.
- Verified the retained ZIP is byte-identical to the attachment.
- Verified all 40 `SHA256SUMS` payload rows against ZIP member bytes; no
  missing, extra, or mismatched member exists.
- Verified all 41 extracted files against their ZIP member bytes.
- Read the complete final contract, owner/API and Rust schema definitions,
  publication and generic-Match semantics, wire/save/replay/replacement,
  failure precedence, work limits, implementation sequence, decisions,
  non-goals, parent supersession, producer lifecycle, static certification,
  version allocation, verification record, and matrices.
- Confirmed `FINAL_STATUS.md` closes every decision,
  `evidence/design-validation.status` is `PASS`, and
  `OPEN_QUESTIONS.md` contains exactly `none`.
- Confirmed the correction-request input is byte-identical to the repository
  request.
- Confirmed 30 producer/consumer rows, 40 deletion rows, 72 traceability rows,
  90 source-evidence rows, 22 bounded-work limits, and 445 unique test rows.
  Test coverage includes 184 positive, 59 negative, 60 tamper, 44 limit, 43
  structural, 35 parity, and 20 gate rows; 102 rows are Tier 2.

## Current-main reconciliation

The returned evidence was collected at
`cec30b57fa734efb059d7b846b397ac7d2b0701a`. Current `main` is later by the two
accepted line-plan cuts `22df44b80` and `a1a098976`. Repeating the package audit
found:

- all 90 cited repository paths still exist;
- 16 evidence rows are on files changed by those cuts;
- those changes add line semantic capability/handle types and affine
  snapshot-only opaque projections;
- no View checked catalog, generic Match, Need journal, old Await consumer, or
  unary Need state authority was changed; and
- the package's fail-closed Cut 1 premise remains accurate: the current tree
  still contains the unreleased View Await surface and lacks the final checked
  View catalog/generic Match substrate.

The ZIP member `inputs/PRIMARY_REQUEST.md` is a faithful scope summary and
repository pointer, not a byte-identical copy of the full primary request. This
does not leave a result-changing decision open: the full repository request was
read directly, all twelve primary requirements map to the final contract and
the 72-row traceability table, and the corrected package is independently
implementable. The discrepancy is retained here rather than silently described
as an exact request copy.

## Selected implementation order

1. Freeze current main and repeat the Await/Need consumer scan.
2. Add the genuinely absent checked View catalog and generic Match/AWBC binding
   substrate while product construction remains fail-closed.
3. Publish complete checked unary Need match/subscription facts and replace the
   stale static reason with `LiveNeedSubscription`.
4. Add the core projection, strict v1 DTO/catalog, journal, observers, start
   intents, save/replay, and replacement owners.
5. Atomically switch compiler, bundle, runtime, native/Web/headless/Agent, and
   generated consumers, then delete every old Await surface in the same v1
   cut.
6. Run focused, tamper, differential, exact/one-over, workspace, Clippy, docs,
   generated, backend, structure, and Tier-2 gates before claiming completion.

No Rust, fixture, manifest, build, test, Clippy, generated, or platform command
was changed or run for this documentation-only intake cut.
