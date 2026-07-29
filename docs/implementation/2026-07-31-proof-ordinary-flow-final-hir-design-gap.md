# Proof ordinary Flow final-HIR design gap

Date: 2026-07-31

Status: `DESIGN_RETURN_REQUIRED_BEFORE_FLOW_FINAL_HIR_PUBLIC_SWITCH`

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

## Dispatch state

The request is ready to send to one design assignee using latest GitHub `main`.
No return has been received or adjudicated yet. TTS remains separately on hold
under its existing skip decision; this Flow request does not resume it.
