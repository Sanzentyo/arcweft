# Proof ClosureEnvironment payload and consumer design gap

- Request date: 2026-08-06
- Repository evidence rechecked: 2026-08-06
- Inspected committed revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Inspected integration worktree: branch `codex/proof-public-switch`, dirty
- Status: `DESIGN_BLOCKED_ROLE_SPECIFIC_MATRIX_ONLY`

## Finding

The accepted Proof synthetic-role contract names one exact-zero
`ClosureEnvironment` Expr child per source-backed closure and assigns it role
tag `0x0a`. The accepted final `HirClosureExpr` payload has no environment
field or other typed reference: it owns a scope, parameters, optional result
type, body Expr, and ordered CaptureIds. The later generator correction says
capture-limit rollback includes the environment but still defines no payload,
source role, consumer, or persistence boundary.

Current final-HIR integration follows the accepted closure payload: the capture
lowerer allocates typed `HirCapture` records in first-use order, and sema and
compiler consumers read those captures directly. It has no production
`ClosureEnvironment` producer. A test-only synthetic Expr or invented Unit/
tuple/record payload would not close the contract.

## Blocker and freeze

The independently throwable correction request is:

- [Proof 01.1.1.4.1.1.1.1.4 ClosureEnvironment payload and consumer authority correction](../reviews/requests/2026-08-06-seq-proof-01.1.1.4.1.1.1.1.4-closure-environment-payload-consumer-authority-correction.md)

It must select one closed result: define the complete real environment
payload/reference/source/consumer/lifetime/limit/rollback authority, or delete
the dead role and compact every unreleased tag/fingerprint/matrix claim.

Until that return is accepted:

- do not fabricate an unreferenced synthetic environment Expr;
- do not count role-admission or fixture-only key tests as producer evidence;
- do not add a parallel capture reader, side table, codec, or persisted shadow;
- do not claim `T-ROLE-10` or the environment portion of
  `T-GEN-CAPTURE-03` complete; and
- do not treat the current `0x0a` fingerprint gap as an accepted compatibility
  decision.

Closure capture ordering, the typed capture arena, and unrelated Proof syntax,
HIR, project, runtime assertion, codec, and save/replay work may continue. This
blocker is limited to the distinct environment-child claim.

## Evidence and validation

Performed:

- opened the accepted synthetic-role and generator correction members;
- compared their role row with the accepted `HirClosureExpr` schema;
- inspected final closure/capture lowering and typed consumers; and
- separated committed repository evidence from the dirty integration WIP.

Not run for this docs-only blocker:

- Rust tests, workspace checks, Clippy, Tier 2, and structural audit.

`git diff --check` is the required mechanical validation for these two new
Markdown files. The rest of Proof remains under executable validation.
