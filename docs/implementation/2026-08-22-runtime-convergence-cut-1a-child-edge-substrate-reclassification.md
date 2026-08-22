# Runtime convergence Cut 1a — child-edge substrate reclassification

Date: 2026-08-22
Inspected Git commit: `423bc649a1755669c45dedce04cdd9706f710e4f`

This note supersedes the completion classification in
`2026-08-22-runtime-convergence-cut-1-match-child-edges.md`. The older note is
retained as historical validation evidence.

## Reclassified result

- Commit `423bc649a1755669c45dedce04cdd9706f710e4f` is accepted as
  `Cut 1a — HIR expression child-edge and callable-join substrate`.
- It is not complete Cut 1 implementation evidence.
- Statement, pattern, declaration-body, and stable semantic transcript owners
  remained to be implemented in Cut 1b.

## Established by Cut 1a

- one HIR-owned exhaustive expression child-edge inventory and projections;
- sema-owned checked child-role enrichment and current callable-catalog join;
- checked ordinary-Match scrutinee/arm/guard/value facts and Bool guard
  validation; and
- typed Choice/dialogue/line-plan expression paths and first-error publication.

## Missing products found by re-audit

- typed statement/pattern/body edges and declaration-rooted stable paths;
- accepted declaration, checked value, and pattern coordinates;
- literal/expression/pattern/Match semantic transcripts without raw HIR IDs,
  spans, or source spelling;
- bounded coverage, unreachable evidence, and non-exhaustive rejection;
- semantic perturbation and statement/For/thread-body closure tests; and
- one complete stable `CheckedMatchSemanticDigest` authority.

`CheckedMatchRef { HirSnapshotId, ExprId }` remains a Cut 3 compiler-local
lookup product. It is not part of the stable Cut 1 digest.
