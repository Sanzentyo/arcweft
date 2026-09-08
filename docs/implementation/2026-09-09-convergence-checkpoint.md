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

## Pushed commits and requested full clean

The connected implementation was committed and pushed as
`8dfe60d18640bb2c5785b112bad6dcd874d216d4`:
`feat: checkpoint typed content and callable execution integration`.
It contains 870 changed files, including this checkpoint note. The earlier
generated-output exclusion is
`f2ef610df535326bdcf35c17366e1d8a7a419e36`.
`git ls-remote origin refs/heads/main` confirmed the implementation SHA on the
remote, and the worktree and index were clean after that push.

The pre-clean workspace retry also failed from exhausted disk space after
297.41 seconds. The following doctest attempt failed in the command runner
with a null-reference error after 1.77 seconds; it did not establish a Rust
doctest result. The supervisor was stopped before starting a replacement run.

The user then explicitly requested one full `cargo clean`. Cargo metadata
identified `D:/git/arcweft/target` as the target directory, and no Cargo,
rustc or just process was active before cleaning. Before deletion, 197 direct
text/log/JSON evidence files and the three retained investigation directories
were copied to:

```text
.arcweft-local/validation/2026-09-09-before-cargo-clean/
```

`cargo clean` succeeded: 178,296 generated files, 282.4 GiB removed. D: then
had 273,781,993,472 free bytes (about 255 GiB). The Git worktree remained clean.
The earlier `target/...` log references in this note now refer to the preserved
copies in the directory above. The clean command's output is also retained
there as `cargo-clean.log`.

A fresh validation run is pinned to source commit
`8dfe60d18640bb2c5785b112bad6dcd874d216d4`. Its logs and command/result ledger
are outside `target`, at:

```text
.arcweft-local/validation/2026-09-09-clean-build/
```

The toolchain is `rustc 1.98.1 (48a229cea 2026-09-01)` / Cargo 1.98.1 on
`x86_64-pc-windows-msvc`. The completed cold stages are:

| Stage | Result | Elapsed seconds |
| --- | --- | ---: |
| Workspace all-target/all-feature check | Passed. | 151.08 |
| Workspace all-target/all-feature Clippy | Passed with warnings. | 98.50 |
| `just test-workspace` | Failed during compilation; no workspace test pass is claimed. | 729.05 |
| Sema library tests | 760 passed, the same 7 known inference/effect failures. Both comparison-abort tests passed. | 145.49 |
| Compiler library and five selected integrations | Library 82/82; integrations 100 passed / the same 18 known callable execution failures. | 274.41 |
| `just test-doc` | Passed: 95 crate reports, 8 doctests, no failed or ignored tests. | 360.21 |
| `just test-tier2` | Failed in its first MCP target: 3 passed / 1 failed; later recipe dependencies not run. | 91.30 |
| Structural audit with `--fail-on-blocking` | Passed: 95 packages, 2,236 Rust files, 309 review triggers, zero blocking violations. | 9.96 |
| `cargo fmt --all -- --check` | Passed. | 10.52 |

The workspace test build reported rustc internal errors beginning with
`no resolution for an import` in `arcweft-player-web`, followed by similar
errors in the compiler `assertions` integration and missing-rlib-form errors
for native-player/Agent REPL tests. This is distinct from the pre-clean disk
failures. The named sema and bundle rlibs existed when inspected afterward;
their existence then does not prove when they became available to the failed
compiler processes. The initial log alone did not establish the cause.

One warm retry of `just test-workspace` on the same Rust source failed in
59.66 seconds. Its first errors were `E0786`: mapping the existing bundle and
runtime-driver rlibs failed with Windows error 1455, explicitly reporting an
insufficient paging file. Missing-rlib-form and delayed rustc internal errors
followed. This establishes a memory-commit failure for the retry, rather than
missing source dependencies or a test-assertion failure. The earlier internal
errors are consistent with this failure, but their log did not independently
report error 1455. No OS paging configuration or Cargo job count was changed.
The separate retry log and result JSON are retained beside the cold logs.

The Tier 2 failure was
`agent_mcp_stdio_reads_agent_trace_resource`: compiling the existing
`samples/agent-script/cli-run-smoke.awfagent` rejected the selected Agent
Product because AWBC function 0 did not declare the effects required by its
project-call target. The run never produced a trace to read. This is a
separate execution defect in the connected callable migration, not a native
capture failure or a successful MCP trace test.

The generated
[structural audit](structure-audits/2026-09-09-convergence-checkpoint/findings.md)
records the unchanged Rust source. The comparison grouping remains owned by
candidate transaction closure; its two new tests live in the transaction test
module. The changed projection owners and their cohesion decisions remain in
the [constraint projection record](2026-09-09-constraint-projection.md). No new
production owner or dependency was added by this validation follow-up.

The first command in the workspace recipe failed, so its later CLI library,
binary, and selected integration commands were not run by either attempt.
The Tier 2 recipe stopped before broad Agent observe, native auxiliary capture,
visual goldens, and the two proof-boundary targets. These are not counted as
passed. The full convergence goal remains active; this record establishes the
requested clean and checkpoint validation, not final convergence.
