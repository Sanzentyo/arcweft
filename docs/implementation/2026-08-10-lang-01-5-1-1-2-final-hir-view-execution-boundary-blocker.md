# Lang-01.5.1.1.2 final-HIR View execution boundary blocker

Date: 2026-08-10

Inspected clean baseline:
`a6805f7375499e5cce70f84f1531832583474527`.

## Outcome

The returned Lang-01.5.1.1.1 dialogue-profile reconciliation is satisfied by
current production. Its focused compiler admission suite passes 5/5 and keeps
the launch-owned manifest, dialogue-owned resolved profile/revision, exact
resource registry Arc, compiler-owned admission, and retained
`Arc<ValidatedViewProduct>` owner chain.

The independent compiler `view_product` target remains non-green at one pass
and six failures. Its body completes in 0.05 seconds; the longer wall time is
ordinary four-job compile/link and build-lock work. The failures do not expose
a dialogue admission defect. They expose a final-HIR-to-executable-View gap and
stale pre-switch expectations.

## Repository-exposed missing boundary

Final HIR already retains View parameter/default/export/value identities,
scopes, ordering, generation, and typed source roles. Final semantic analysis
currently publishes only element/Text/RichText/modifier-name classifications.
The compiler consequently accepts only argument-free elements, literal text,
and typed dialogue projections. Other dynamic shapes collapse to the generic
`MissingCheckedViewProjection` error.

The existing runtime product cannot be filled in safely by a local helper.
Its instruction model has dynamic branch/repeat/await/local/call machinery,
but its value programs intentionally exclude strings and resource identities,
and several authored properties remain static-only wire fields. Selecting
coercion, evaluation, codec, invalidation, and save behavior would change
observable language/runtime semantics.

Restoring the deleted pre-Proof lowerer is not an option. It read a different
flattened-HIR/AST model, rejected valid dynamic Views, and would create the
parallel authority prohibited by the selected public switch.

## Blocking request

The independently throwable correction is
[Lang-01.5.1.1.2 final-HIR View execution catalog and static certification reconciliation](../reviews/requests/2026-08-10-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation.md).

It fixes the semantic direction that dynamic View remains valid, while static
View is an optional typed optimization certificate. Automatic proof and an
authored `#[static]` assertion must use the same analysis; the assertion cannot
bypass checking. The request also requires exact still/animated image resource
binding rather than guessing whether `Image` is an unconditional builtin.

Until that contract returns, the following are explicit non-goals:

- restoring an `Image` builtin only to satisfy the stale compiler fixture;
- changing stale failure codes/stages so the old static-rejection tests pass;
- accepting dynamic arguments while silently dropping them or substituting
  defaults;
- extending `FxRuntimeValue` with String/resource values without reconciling
  its codec/evaluator/runtime consumers;
- adding a second View catalog, AST reader, source reconstruction, source gate,
  compatibility alias, dual reader, shim, CSS, or Takumi path; and
- implementing broader `mount`, Action emit/receive, persistent-reference,
  Dialogue `#call()[content]`/Ruby, try/pipe, Choice, or Style naming changes.

## Dispatch guidance

Send the request file as the primary prompt with the clean repository snapshot
or exact commit above. Attach the accepted Lang-01.5.1.1.1 archive, the final
Proof-concurrency v6.1.1 typed-HIR/public-switch package chain, the returned
Lang-01.1.1 direct-suspension/ordinary-function contracts, and the current
Lang-01.4.2.1 resource-manifest contract. Attach Lang-01.5.1.2.1 and
Lang-01.5.1.3 as downstream consumer constraints, not as authority to redesign
their content-root or generated-binding decisions.

Use one design-only assignee because semantic catalog, wire allocation,
runtime evaluation, static proof, hot reload, and save identity form one atomic
boundary. Require the exact ZIP name from the request, `OPEN_QUESTIONS=0`, and
no production overlay. Do not split static proof, dynamic value ABI, image
binding, or save/replay among separate assignees.

## Validation evidence

- `cargo test -p arcweft-compiler --test dialogue_profile_admission --jobs 4`:
  5 passed; test body 0.14 seconds.
- `cargo test -p arcweft-compiler --test view_product --jobs 4 -- --nocapture`:
  1 passed, 6 failed; test body 0.05 seconds. Exact failures are the unknown
  `Image` callable, missing checked View-product projection, and obsolete
  stage/code/cardinality/source-label expectations.
- `just structure-audit-gate`: 2,148 files, 2,020 Rust files, 999,125
  physical Rust LOC, 95 workspace packages, 182 review triggers, and zero
  blocking violations.
- The checkout was clean before this documentation-only blocker cut.
- No production, codec, runtime, workspace, Tier 2, or structural completion is
  claimed by this record.
