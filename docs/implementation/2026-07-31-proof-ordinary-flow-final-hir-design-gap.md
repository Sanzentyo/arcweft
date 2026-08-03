# Proof ordinary Flow final-HIR design gap

Date: 2026-07-31

Status: `CORRECTED_RETURN_ADJUDICATED_IMPLEMENTATION_RELEASED`

## Outcome

The Proof final-HIR item audit found one remaining externally designed boundary:
the ordinary source-level Flow item does not yet have a decision-complete
attached-syntax, contract-clause, `HirThreadFlowItem`, scope, source-freeze, and
recovery matrix.

The independently throwable request is:

- [`2026-07-31-seq-proof-01.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation.md`](../reviews/requests/2026-07-31-seq-proof-01.1.1.2.1.1-ordinary-flow-attached-hir-projection-reconciliation.md)

This is a narrower follow-up to the repository-resolved v6.1.1.2.1 item/member
inventory. It does not reopen that inventory or the accepted Proof arena,
source-query, project, Call, Select, Thread, or Dialogue contracts.

## Accepted rows that remain implementation-authorized

- Flow remains one final `HirItemKind` family.
- Its outer payload retains optional typed ID/name, typed signature and
  contracts, a callable scope, and ordered `HirThreadBody`.
- Flow bodies are statement-only and have no value tail. Empty authored bodies
  are semantic Unit; missing bodies are typed recovery.
- The accepted qualified HIR arenas, source map, rollback, limits, and project
  identity remain unchanged.
- Dialogue content in Flow migrates directly to the accepted typed dialogue
  application owner; legacy speaker/content-call carriers are not repaired.

Other final-HIR item families with closed contracts may continue privately.
The Flow item itself must not become public, and the complete project/compiler/
LSP authority switch cannot claim closure, until the returned Flow package is
adjudicated.

## Unresolved result-changing rows

The external return must close:

1. exact attached ownership for ordinary name, typed ID, ID plus name, and
   missing Flow identity;
2. typed projection of the complete admitted contract family, including scope
   and result-local visibility;
3. exhaustive attached-node to `HirThreadFlowItem` mapping;
4. omitted-return semantic Unit storage without a fabricated source/type;
5. statement-only body parsing and source freeze;
6. primary issue precedence, exact/one-over accounting, and rollback; and
7. deletion-driven migration of every old Flow consumer.

## Implementation freeze

Until the return is accepted:

- do not invent provisional Flow syntax kinds, source roles, scopes, HIR
  payloads, sentinels, or fallback readers;
- do not repair or deepen the old Flow identity/header/body path;
- do not preserve a value-tail reader for Flow;
- do not reconstruct typed clauses or Flow items from source strings; and
- do not add compatibility aliases, wrappers, shims, dual readers, source
  gates, removed-syntax diagnostics, CSS, or Takumi paths.

When design returns, implementation starts by making obsolete Flow readers
unavailable and uses compile failures as the migration inventory. The final
public switch must delete the old authority rather than leave a parallel path.

## Return state

The first return was received on 2026-08-02 and rejected as not
implementation-ready. Although mechanically readable, it inspected no
repository revision or predecessor and supplied none of the exact schemas or
exhaustive matrices requested. Its intake is recorded in
[`2026-08-02-proof-ordinary-flow-return-intake.md`](2026-08-02-proof-ordinary-flow-return-intake.md).

The independently throwable full-redelivery correction is
[`2026-08-02-seq-proof-01.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction.md`](../reviews/requests/2026-08-02-seq-proof-01.1.1.2.1.1.1-ordinary-flow-evidence-schema-redelivery-correction.md).

The corrected return arrived on 2026-08-03. Its standalone READY claim was
rejected because it conflicted with accepted limits, shared final-HIR owners,
the maintained `ensures no_effect` grammar, and required Thread recovery
evidence. Those decisions are fully determined by current accepted authority,
so the repository adjudicated them locally instead of requesting another
redelivery. The verified archive, exact adjudication, and released
deletion-driven boundary are recorded in
[`2026-08-03-proof-ordinary-flow-redelivery-intake.md`](2026-08-03-proof-ordinary-flow-redelivery-intake.md).
Ordinary Flow and the shared Thread/Flow-item public projection are no longer
design blocked. TTS remains separately on hold under its existing skip
decision.
