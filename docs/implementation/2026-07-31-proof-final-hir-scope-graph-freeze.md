# Proof final-HIR scope-graph freeze

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This note restores the graph-freeze contract. The protected checkout's test and
structure-audit output is not current validation.

## Single freeze boundary

`HirModuleArenas::try_new` is the single complete scope-graph freeze boundary.
It first validates exact arena/slot coverage, then requires:

- exactly one Module root owned by the current `HirModuleId`;
- every scope reachable from that root exactly once;
- source-ordered parent children with exact child-to-parent backlinks;
- every non-root owner resolving through its typed arena;
- expression/statement-owned scopes as direct lexical children of the scope
  recorded on their owner payload;
- nested item-owned scopes staying inside the same item subtree; and
- every Local appearing exactly once in one scope's source-ordered inventory,
  with an exact Local-to-scope backlink.

The closed kind/owner admission is:

| Scope kind | Admitted owner |
| --- | --- |
| Module | module |
| Callable, Flow, Predicate, Proof | item |
| ContractRequires, ContractEnsures | item |
| Block | item, expression, or statement |
| MatchArm, Conditional | expression or statement |
| Loop | statement |
| Closure | expression |

Freeze reads the immutable child/local order and never sorts or rebuilds it.
Family-specific source/schema validation remains in the typed source-index and
payload owners.

## Atomic failure and evidence boundary

Graph validation precedes immutable module construction and publication. Root,
reachability, backlink, owner, subtree, or Local-membership failure is an
invalid arena snapshot and leaves no current candidate or prepared slot
revision. Empty test fixtures cannot weaken the production one-root rule.

Current acceptance must cover valid order, missing/duplicate roots, owner-kind
mismatch, cross-item splicing, Local membership/backlinks, lexical-parent
substitution, and non-publication on failure. No old focused result or generated
audit is carried into that matrix, and no compatibility reader, reparse,
source gate, alias, or removed-syntax diagnostic is permitted.
