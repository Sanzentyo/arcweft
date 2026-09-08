# Convergence implementation checkpoint — 2026-09-09

Status: IN_PROGRESS. The user explicitly requested autonomous commit/push of
the accumulated implementation while the full convergence work continues.
This checkpoint records the connected working state; it does not mark the
[convergence goal](2026-09-08-convergence-goal-plan.md) complete or change its
acceptance criteria. Subsequent validation results belong in this note.

Inspected base: `f2ef610df535326bdcf35c17366e1d8a7a419e36` on existing `main`,
pushed to `origin/main`. That commit replaces root-only target exclusions with
`**/target/`; the generated native patch-test bundle remains on disk and is
not staged. No branch, worktree, reset or user-source deletion was used.
The preceding implementation base is
`4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`.

## Scope of the implementation snapshot

The accumulated changes connect the same Content/Fx/callable contracts across
syntax, HIR, sema, compiler, runtime-plan, native/AWBC execution, presentation,
player/Agent consumers, fixtures and maintained documentation. Their dependent
producer and consumer changes are recorded together. The staged snapshot had
869 changed files before adding this checkpoint note. Renames account for the
difference from the 871 explicit paths in the staging manifest.

The major implemented responsibilities are:

- typed attached-content declarations, physical source operands, Content/Fx
  construction and their semantic evidence;
- Free/Bound/Inference type and constant references, scoped substitutions,
  continuation reapplication, nominal owner/payload projection and source
  completion checks;
- deterministic closed-instance discovery and metered type projection and
  encoding, with preservation of an accepted cache generation on failure;
- project/closure calls, captures, omitted defaults and same-fiber native
  call/return/suspension, plus the corresponding AWBC and owned type inventory;
- immutable dialogue content templates, evaluated effects and the dependent
  bundle, render, player, Agent and sample migrations.

The prior implementation records remain evidence of their own checkpoints.
The latest completed broad comparison before this follow-up is
[iterative constraint projection](2026-09-09-constraint-projection.md): sema
758 passed / 7 failed; compiler library 82/82; related integrations 100 passed /
18 failed. Its workspace check, Clippy and structural audit passed, with the
recorded warnings. Those commands preceded the small comparison fix below;
they are not relabeled as reruns of this snapshot.

## Comparison failure now preserved

Candidate closure previously used `bindings_equal(...).unwrap_or(false)` while
grouping normalized paths. A node-limit or cancellation error was converted
into an ordinary unequal-path result, allowing source materialization tickets
to be issued after comparison had aborted.

The grouping loop now propagates the exact error before ticket issuance. Two
tests in `types/constraints/transaction/tests.rs` exercise the actual closure
phase with two correlated source rows. Four node visits normalize their
bindings and source types; the fifth compares the binding values. A four-node
limit rejects visit five, while five nodes admit both materialization tickets.
Cancellation after the fourth visit likewise prevents either ticket. Both
tests fail against the old grouping and pass with the propagated error.

The initial test fixture incorrectly assumed six visits before comparison.
That attempt did not establish the regression; it was corrected after checking
the shape-level occurs accounting. The valid failing baseline is
`target/constraint-family-comparison-baseline.log`, and the passing run is
`target/convergence-checkpoint-focused.log` (2/2). No production limit or
existing test expectation was relaxed.

## Validation at snapshot creation

| Command / check | Actual state |
| --- | --- |
| `cargo test -p arcweft-lang-sema --lib types::constraints::transaction::tests -- --nocapture` | Passed: 2/2. |
| `cargo fmt --all` | Passed before staging. |
| `git diff --cached --check` | Passed for the staged implementation. |
| `just test-workspace` | First attempt failed during compilation after 260.14 seconds because D: ran out of space; this was not a test-assertion result. |
| Workspace retry, doctests, Tier 2, workspace check/Clippy and structural audit | Validation is in progress at snapshot creation; no success is claimed here. |

The first validation supervisor was interrupted after the verified disk-space
failure and confirmed stopped. A broad filesystem cache-deletion command was
rejected by automatic approval review with a policy-block response and did
not execute. The narrower Cargo command was then used:

```bash
cargo clean -p arcweft-lang-sema --dry-run --target-dir D:/git/arcweft/target
cargo clean -p arcweft-lang-sema --target-dir D:/git/arcweft/target
```

Cargo removed 2,907 generated files totaling 28.5 GiB. Source and staged files
were preserved. The workspace test retry began only after the previous
supervisor and Rust processes had stopped. Commands use normal Cargo
concurrency; no explicit job count was supplied. Stage results are retained
in `target/convergence-checkpoint-validation-results.json` and the corresponding
`target/convergence-checkpoint-*.log` files.

## Remaining work and evidence

The known semantic failures still require correlated parent/child source
constraints and inferred callback effects. The execution matrix additionally
requires unified callable-value/scheme execution and CharacterDialogue value
construction. This follow-up did not add a scope-family graph, new effect
algebra, constructor-specific fallback, recovery value or runtime type solver.
Those coupled requirements remain in the
[active callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Match, View, RuntimePlan/task-plan, nominal and scheduler/restore requirements
also remain part of the full goal.

Review inventory was re-enumerated: 71 ZIPs, zero ZIPs in the review inbox.
The retained scope archive is still 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Frozen mirrors were staged as provided, not rewritten by this follow-up.
The snapshot introduces no new contract-version marker; the version-1 rule
and layer/Sans-I/O requirements remain in force.
