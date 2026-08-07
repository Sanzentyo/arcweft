# Proof DesugaredTemporary production-recipe design gap

- Request date: 2026-08-06
- Repository evidence rechecked: 2026-08-06
- Inspected committed revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Inspected worktree: branch `codex/proof-public-switch`, dirty with 1,284
  changed paths (718 modified, 356 deleted, 210 untracked)
- Status: `DESIGN_BLOCKED_ROLE_SPECIFIC_MATRIX_ONLY`

## Finding

The accepted Proof v6.1.1.4.1.1.1.1 contract requires a real production
`DesugaredTemporary` recipe cursor, attached Expr/Stmt plans, source-token plus
immutable recipe-step ordering, typed temporary payloads and references, and
atomic 1,024/1,025 evidence.

Committed `main` has the role, admission row, and fingerprint tag in
`arcweft-lang-hir`, but non-test final lowering has no producer and production
syntax/HIR has no recipe descriptor or cursor authority. Pipe remains a direct
retained `HirPipeExpr { left, right }` payload. The dirty integration worktree
also exposed that a fixture-only direct synthetic reservation could make a
role-presence test pass without satisfying the accepted production contract.
That WIP is not an accepted revision and receives no completion credit.

## Blocker and freeze

The independently throwable correction request is:

- [Proof 01.1.1.4.1.1.1.1.3 DesugaredTemporary production-recipe authority correction](../reviews/requests/2026-08-06-seq-proof-01.1.1.4.1.1.1.1.3-desugared-temporary-production-recipe-authority-correction.md)

It must choose one closed result: define the complete real producer/recipe/
payload/reference/source/consumer/lifetime/limit/rollback authority, or delete
the unreachable role and all affected fingerprint and matrix claims directly.

Until that return is accepted:

- do not infer a Pipe desugaring from the predecessor's example;
- do not add or count a test-only fake cursor, direct synthetic reservation,
  source scan, or hand-built plan as production evidence;
- do not repair a dead role path or introduce a compatibility reader; and
- do not claim the `DesugaredTemporary` generator rows complete.

Unrelated accepted Proof syntax, HIR, project, runtime assertion, codec, and
save/replay work may continue. This blocker is limited to the role-specific
producer and matrix rows; it does not reopen accepted substrate.

## Match is not made ownerless by this blocker

This request does not decide Match lexical ownership. A source-backed Match
`ExprId` or `StmtId` remains the semantic and transaction owner of its
scrutinee and ordered arms. The Match delimiter creates no common `Block`
scope: the scrutinee evaluates once in the inherited outer scope, and each arm
uses its context-specific lexical owner (`MatchArm` for ordinary expression or
statement arms, or the single nested `Block` owner for a braced Thread arm).

An implementation-local retained scrutinee value is not, by itself, a HIR
synthetic temporary. If a later lowering actually materializes such a HIR node,
the dedicated `MatchScrutinee` role—not the general `DesugaredTemporary`
role—must own the exact-zero reservation. Therefore neither retaining nor
deleting `DesugaredTemporary` may add a Match-level `Block`, merge sibling arm
scopes, or change the once-evaluation and binding-cleanup rules.

## Evidence and validation

Performed:

- inspected the accepted correction and intake directly;
- inspected committed `main` for role producers, recipe vocabulary, Pipe
  payload ownership, and final-lowering behavior; and
- recorded the full Git SHA and dirty state without treating WIP as accepted
  evidence.

Passed:

- repository request and blocker note were cross-linked;
- the request is GitHub-only, design-only, independently throwable, and names
  one standalone ZIP containing all sidecars.

Not run:

- Rust tests, workspace checks, Clippy, Tier 2, and structural audit are not
  applicable to this docs-only blocker/request cut.

`git diff --check` is the only required mechanical validation for these two
new Markdown files.
