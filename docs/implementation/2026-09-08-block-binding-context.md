# Block binding inference in the owning expression context

- Date: 2026-09-08.
- Inspected HEAD and freshly fetched `origin/main`:
  `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`; divergence `0 0`.
- Existing `main` checkout and inherited changes preserved. Start: 8 deleted,
  645 modified, 86 untracked status entries; empty index.
  Final checkpoint: 8 deleted, 646 modified, 89 untracked entries; empty index.
- Supersedes the named-argument and block-LHS pipe failure status in
  [call evaluation order and nominal AWBC](2026-09-08-call-evaluation-order-and-nominal-awbc.md).
  Earlier results remain historical evidence.
- The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
  This is part of its connected implementation, not a completed main push cut.

## Established behavior

A block now infers its statement bindings before checking its value, using
the same expression context and fact transaction as its caller. This applies
when the block occurs inside a call operand, a pipe operand, a closure, a
carrier block, a named block or a loop. Declaration and residual statement
roots enter through an explicit published context. Nested evaluation retains
the current candidate authority and call-frame scope.

Previously, `infer_nested_expression_bindings` only inspected the direct
initializer's expression family. A call or pipe initializer did not pre-infer
the bindings in its child blocks. The other statement-use visitor returned
immediately outside an implicit callable. Consequently a block tail could be
checked before its local's type existed. The named call retained a rejected
call row and later failed runtime reachability with a structural projection;
the pipe failed semantic analysis with an unavailable expression type.

The new statement-binding owner centralizes initializer, pattern-scrutinee
and iteration binding inference. The old direct-initializer prepass, separate
control-binding path and line-plan statement-collection helper were removed.
The existing block statement traversal invokes this owner before visiting
nested statements. Its implicit-callable expression-use checks remain in
their existing context. Dialogue line output checking now prepares its
statement bindings in that same context before dependent content expressions
are checked. Existing dialogue handle/output tests remain passing.

This changes when semantic facts are established; it does not change lexical
scope identities or the runtime evaluation order contract. RuntimePlan and
AWBC were not relaxed to accept rejected calls or missing types. No additional
fact store, resolver, compatibility reader, version, dependency, unsafe code
or I/O boundary was introduced.

The named-argument execution fixture now returns `212` in native and AWBC:
argument expressions run in source order while values reach the correct
formal parameters. The block-LHS pipe returns `3` in both engines, proving
that the mutation occurs once and both placeholders consume that result.
The existing stronger fixtures were retained without reducing their checks.

A new closure-in-block fixture checks a captured local and a contextually
typed closure parameter; it returns `42` in native and AWBC. The callback's
effect row is explicitly closed for this binding/capture test. Its initial
unannotated form reached the existing `Effect(UnknownRow)` failure; the
separate inferred-effects execution pair remains required and failing.
Explicitly closing the row here does not establish inferred callback effects.

A candidate-transaction test checks that a block's expression, local and
pattern facts all roll back together. The same block subsequently publishes
successfully. Additional semantic tests inspect selected-call execution
evidence and verify that both pipe occurrences carry one typed binding
identity, with their distinct source ordinals and `i64` value types.

## Validation actually run

Cargo selected its normal concurrency; no explicit job count or parallel test
commands were used. The structural audit was an independent metadata command
during the workspace validation sequence. Rust sources were not edited while
builds, tests or the formatter were running.

| Command / evidence | Result |
| --- | --- |
| `cargo check -p arcweft-lang-sema --all-targets --message-format=short` | Passed; three existing generic-scope dead-code warnings. Log: `target/block-binding-check.log`. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | Final: 737 passed, zero failed/ignored. Log: `target/block-binding-sema-tests-final.log`. Initial expanded suite: 735 passed / 1 failed because the extra closure fixture also requested inferred effects. That failure was not counted as a pass. |
| `cargo test -p arcweft-compiler --lib --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Failed overall. Compiler library 67 passed; callable execution 43 passed / 12 failed; evaluated effects 9 passed; project instances 6 passed; Try/pipe 8 passed. Zero ignored. Log: `target/block-binding-compiler-tests-final.log`. |
| `cargo check --workspace --all-targets --all-features --message-format=short` | Passed. Existing sema dead-code and two Windows macro-linker output warnings remain. Log: `target/block-binding-workspace-check.log`. |
| `cargo clippy --workspace --all-targets --all-features --message-format=short` | Passed with warnings, exit 0. Log: `target/block-binding-workspace-clippy.log`. No suppression or `-D warnings` was used. |
| `cargo fmt --all` | Passed after the final Rust changes. |
| `git diff --check` | Passed. |
| Maintained documentation link check | Passed: 3 documents, 29 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-08-block-binding-context --fail-on-blocking` | Passed: 95 packages, 2,227 Rust files, 310 review triggers, zero blocking violations. |

The final directly exercised library total is 804 passed; the related
integration total is 66 passed / 12 failed. The callable file now has 27
native/AWBC pairs plus one negative nominal codec/verifier test. These are
current results, not a reuse of the previous broader library run.

Clippy reports 1,203 sema library warnings and 1,388 library-test warnings
(1,202 duplicates); compiler library/test summaries remain 222/226 warnings
(219 duplicates). The new publication entrypoint inherits the existing large
`FinalSemanticAnalysisError` boundary and reports `result_large_err`.
Additional workspace and integration warnings remain. Passing exit status
does not mean the tree is warning-free.

Earlier diagnostic runs reproduced the named/pipe failures before the fix.
Temporary logging was removed before final validation. Intermediate logs are
`target/named-block-diagnostic.log`, `target/pipe-block-diagnostic.log`,
`target/block-binding-diagnostic.log`, `target/block-binding-sema-tests.log`
and `target/block-binding-integration-tests.log`.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not
run in this follow-up. They remain required before the connected main push
cut. The earlier HIR/core/RuntimePlan library results are not claimed as new
runs here.

## Remaining requirements

Six callable families still fail in both native and AWBC: inferred callback
effects; a contextual project unit constructor depending on a later argument;
an unselected nominal case parameter depending on a later argument; a curried
prefix as a callback; a generic prefix at a monomorphic callback type; and a
shared prefix used at different later types. The generic-prefix structural
call failure is not fixed merely because named block arguments now work.

The [coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
still owns function schemes, continuation execution, effect/suspension closure,
assertion instance identity/inventory, discovery bounds and the writable-place
contract. Bare-local assignment, complete Content/Fx ordering and program-bound
restore are not established by this change. Generic Match, retained View,
RuntimePlan/task-plan, remaining nominal C1–C6 and scheduler/restore remain
goal requirements. No acceptance condition was removed and no stable language
rule was narrowed.

There is no external blocker. The index remains empty; no implementation
commit or push was made because the connected semantic/runtime cut is still
incomplete. No branch, worktree, checkout switch, reset or unrelated cleanup
was performed. Frozen review archives and mirrors were not edited.

## Structural ownership review

The [generated findings](structure-audits/2026-09-08-block-binding-context/findings.md),
[file measurements](structure-audits/2026-09-08-block-binding-context/file_metrics.csv)
and [dependency measurements](structure-audits/2026-09-08-block-binding-context/package_metrics.csv)
cover the current checkout. HEAD-to-current growth includes inherited changes,
not just this follow-up.

| Owner | HEAD → current LOC | Current bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| sema `analyzer.rs` | 917 → 1,004 | 40,037 | 0 |
| sema `analyzer/expressions.rs` | 3,357 → 3,904 | 170,755 | 88 |
| sema `analyzer/preparation.rs` | 1,050 → 935 | 38,061 | 0 |
| sema `analyzer/statement_bindings.rs` | new → 178 | 7,358 | 0 |
| sema `analyzer/dialogue_line_plan.rs` | 482 → 1,669 | 72,747 | 66 |
| sema `analyzer/executable_ingress.rs` | 616 → 609 | 23,373 | 0 |

- `statement_bindings.rs` owns contextual local/pattern inference. It uses the
  analyzer's existing fact journal; it owns no parallel state. Its publication
  entrypoint selects a real source expression for diagnostics. Nested callers
  pass their existing expression context and retain candidate rejection.
- `preparation.rs` retains declaration/type preparation and inventory roots.
  Removing its direct-expression prepass follows the semantic ownership
  boundary, not a numeric file-size target.
- The upper-size expression dispatcher retains implicit-callable context,
  pipe/closure/control expression dispatch and its cohesive statement-use
  traversal. Local-binding rules moved to their owner without widening a
  public API. Dialogue output typing consumes the same binding owner instead
  of maintaining a separate line-plan prepass. Existing embedded tests remain
  tied to these responsibilities; no additional embedded test body was added.
- Declaration ingress still owns its event/worklist protocol; only the binding
  consumer changed. Analyzer orchestration has one private module declaration
  added and acquires no new state or transport responsibility.
- Analyzer tests are 1,737 LOC / 65,368 bytes; the callable semantic tests are
  154 LOC / 4,646 bytes; callable execution is 465 LOC / 11,721 bytes.
  They inspect typed facts, transaction behavior and production execution.
- Workspace/development dependency fan-in/out remains sema 8/14 and 3/0,
  compiler 3/23 and 1/5. No manifest, facade export or cross-crate owner changed.
