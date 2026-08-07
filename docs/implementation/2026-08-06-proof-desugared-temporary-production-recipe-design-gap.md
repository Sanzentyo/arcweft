# Proof DesugaredTemporary production-recipe design gap

- Request date: 2026-08-06
- Repository evidence rechecked: 2026-08-08
- Inspected committed revision:
  `52b8c917632358d2360e0bb2ea5c32ecc7ca562b`
- Inspected worktree: branch `codex/proof-public-switch`, dirty with 1,391
  changed paths
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
scope: scrutinee name lookup uses the inherited outer lexical scope, while the
Match ID owns its once-evaluation extent through arm selection until that Match
evaluation exits. Each arm uses its context-specific lexical owner (`MatchArm`
for ordinary expression or statement arms, or the single nested `Block` owner
for a braced Thread arm).

An implementation-local retained scrutinee value is not, by itself, a HIR
synthetic temporary. Current AWBC lowering nevertheless does materialize a
codegen-local register. Flow `awbc_lower/flow.rs::lower_match` lowers the
scrutinee once into a `FrameBuilder::temp` at the containing frame depth and
does not emit `AwbcInstruction::Drop` or `Clear` for that register at the Flow
Match join. Structured Flow selection instead keeps the evaluated value local
to its selection helper, so it is released when that helper returns. Existing
Flow parity evidence proves once-evaluation and arm-binding isolation, but does
not prove equal join-time release.

Expression `awbc_lower/expr.rs::lower_match_value_expr` also uses a temporary,
but inside a synthetic control function whose selected paths `Return` and whose
exhaustion path `Trap`s. It therefore has no equivalent enclosing-frame Match
join. Its missing evidence is instead that return/trap frame exit releases the
temporary and matches structured expression evaluation on every exit.

This is a runtime-lifetime gap, not evidence for a lexical Match Block. If the
final HIR materializes a synthetic child, the dedicated `MatchScrutinee`
role—not the general `DesugaredTemporary` role—must own the exact-zero
reservation. If the final HIR remains source-only, runtime/AWBC must still
define and test a Match-owned codegen-local extent. Pattern rejection and a
guard-false transition to the next arm retain that extent while discarding only
the rejected arm's bindings. Flow releases the scrutinee exactly once when the
Match evaluation exits through the selected arm's successful join, final
no-match/mismatch, error, or terminating/frame-exit edge; expression Match must
prove release at every control-function return/trap. Therefore neither
retaining nor deleting `DesugaredTemporary` may add a Match-level `Block`,
merge sibling arm scopes, assign the retained scrutinee lifetime to the
inherited outer lexical scope, or weaken the once-evaluation and
binding-cleanup rules.

At this audit boundary, non-test final lowering has zero
`SyntheticRole::MatchScrutinee` producers. Role-table presence is not production
evidence. The returned producer inventory must therefore either provide a
complete materialized MatchScrutinee producer/payload/reference/consumer recipe
including its Flow join/drop and expression frame-exit extents, or classify
Match as a no-final-HIR-producer construct and specify the runtime/codegen-local
register release that closes the currently observed structured-runtime/AWBC
difference before
selecting direct deletion of the unreachable role claim. Until that decision is
accepted, lowering retains the source-backed scrutinee expression, evaluates it
once, and allocates no synthetic Match scrutinee child; the current AWBC
register lifetime is explicitly not accepted as exact join/drop evidence.

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
