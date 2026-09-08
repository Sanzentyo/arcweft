# Call evaluation order and nominal AWBC field operations

- Date: 2026-09-08
- Inspected HEAD: `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`.
- `git fetch origin main` completed; `origin/main` has the same full SHA and
  `git rev-list --left-right --count HEAD...origin/main` returned `0 0`.
- Existing `main` checkout; inherited changes preserved. Start: 8 deleted,
  643 modified, 82 untracked status entries, empty index.
  Final checkpoint: 8 deleted, 645 modified, 86 untracked entries; index empty.
- Supersedes the HIR-reproducer status and evaluation-order checkpoint in
  [closure instances and function invocation](2026-09-08-closure-instances-and-function-invocation.md).
  Earlier results there remain historical evidence.
- The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
  This is part of the connected uncommitted implementation, not a main push
  cut or completion of all callable/nominal requirements.

## Implemented behavior

Flow expression composition now stores an evaluated child's value before
continuing with later children. One closed executable context admits the
expression-value locals; the old call-result-only local inventory was replaced.
A call result which already occupies its own admitted local is reused. Pure
children preceding an executable child are also materialized in order.
Lazy branch selection still uses its explicit control lowering.

The selected expression ownership graph does not itself specify callee-first
execution. RuntimePlan uses the typed HIR callee and checked call disposition
to evaluate a function-value callee or retained receiver before arguments.
Static name/type selectors are excluded. ProjectCall and host-call operand
materialization use the same composition path. No source-offset sorting,
source-string reconstruction or additional name resolver was introduced.

Two native counterexamples returned `43` before this repair: a closure
capturing a nominal record before a later argument changed its field, and a
binary expression reading the field before its right operand changed it.
Both now return `42`, in native and AWBC. Captures in these fixtures retain
the value read when the callee is evaluated; nominal records use their
existing value representation and clone behavior.

AWBC nominal field projection now consumes the typed field ordinal. It no
longer uses the diagnostic label `field#0` as a field name. `ProjectRecord`
and `AssignRecordField` verification consume both complete structural-record
and nominal-record descriptors. Field bounds and result/replacement types
remain checked. The VM uses the same core-owned record-field read/write
behavior for both physical record representations; native environment field
writes use that same mutation owner. The obsolete native environment helper
and VM-only structural-record mutation interpretation were deleted.

The new compiler negative test encodes/decodes a canonical nominal-field
program, verifies it, and independently rejects a missing write ordinal, a
record value written to an `i64` field, and a missing read ordinal. Opcode,
codec and identity-domain versions remain `1`; no alternate reader or
compatibility carrier was added.

HIR source-component admission now handles invalid shorthand record fields
consistently with invalid explicit fields. Required components are derived
from the attached typed field form: an invalid shorthand does not acquire
an explicit field's separator/value requirements. The recovery fixture now
publishes a recovered HIR module with its source evidence rather than failing
the transaction with `InvalidSourceIndex`. It does not become executable.

## Probe corrections and acceptance scope

The earlier evaluation-order probe began an argument with `{ offset = ... }`.
The current parser selects a record literal for that form. A leading `let`
produces the intended block. This investigation exposed the invalid-shorthand
recovery defect above; parser grammar was not changed.

The corrected probe then used a bare mutable-local assignment, which current
sema does not admit through its checked assignment-place model. The existing
assignment owner only represents direct-local nominal fields. The final
execution-order tests use that already-admitted mutation surface. No broader
assignment feature or new rejection policy was implemented, and passing the
nominal fixtures does not establish mutable-local assignment support.

The independently reproducible interaction still requiring adjudication is:

```arcw
fn identity(value: i64) -> i64 { value }
flow main() -> i64 {
    let mut offset = 1i64
    return (|value: i64| value + offset)({
        let input = 41i64
        offset = 2i64
        identity(input)
    })
}
```

HIR accepts the unambiguous block; sema reports `WrongPayloadFamily` at the
bare assignment. Reconcile the required writable-place/capture contract in
the [coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Do not infer a new language prohibition from this implementation limit or
expand the place model with a special case merely to make this probe pass.

## Test-consumer migration

Agent controller tests now run the generated controller to its first actual
host request and inspect the typed observe/wait/predicate values. They no
longer require a host call to be the first operation or reconstruct values
with a test-only interpreter for nested Let nodes. All three controller
scenarios and the complete 67-test compiler library suite pass.

The Try/pipe tests no longer count or directly invoke removed pure helpers.
They execute current function frames in native and AWBC, covering success,
first/second residual propagation, ordinary nested blocks, Result/Option
carrier boundaries, and unselected if/match branches containing errors.
The pipe-before-Try case mutates a nominal field on each side and checks the
returned value. A shared test execution harness serves the paired callable,
Try/pipe and named-argument tests; it is not a production evaluator.

The named-argument test retains typed ABI/source-index checks and now also
uses field updates in its authored argument order. Its expected `212` checks
formal argument placement and the final mutation count. That stronger case
currently fails before execution, as recorded below.

## Validation actually performed

Cargo used its normal concurrency; no explicit job count was set. The
structural audit ran as independent metadata screening while the library
command was active. No independent test commands were run in parallel.

| Command / evidence | Result |
| --- | --- |
| `cargo test -p arcweft-lang-hir --lib final_lowering::tests::closure_calls -- --nocapture` | Passed: 5; 889 filtered; zero ignored. Includes recovered invalid shorthand and the unambiguous closure/block fixtures. |
| `cargo test -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler --lib --no-fail-fast -- --nocapture` | HIR 886 passed / 8 ignored; sema 733 passed; core 357 passed; RuntimePlan 58 passed. Compiler initially 65 passed / 2 failed due to obsolete host-operation-position assumptions. Log: `target/evaluation-order-library-tests.log`. |
| `cargo test -p arcweft-compiler --lib -- --nocapture` | After migrating the tests: 67 passed, zero failed/ignored. Combined final library evidence is 2,101 passed and 8 ignored. Log: `target/evaluation-order-compiler-library-tests.log`. |
| `cargo test -p arcweft-compiler --test callable_execution --test evaluated_effects --test project_function_instances --test try_pipe --no-fail-fast -- --nocapture` | Failed overall: callable execution 41 passed / 12 failed; evaluated effects 9 passed; project function instances 5 passed / 1 failed; Try/pipe 7 passed / 1 failed. Total 62 passed / 14 failed / zero ignored. Log: `target/evaluation-order-integration-tests.log`. |
| `cargo check --workspace --all-targets --all-features --message-format=short` | Passed. Three existing sema dead-code warnings and Windows linker-output warnings in two macro crates remain. Log: `target/evaluation-order-workspace-check.log`. |
| `cargo clippy --workspace --all-targets --all-features --message-format=short` | Passed with warnings, exit 0; not a warning-free result. Log: `target/evaluation-order-workspace-clippy.log`. |
| `cargo fmt --all` | Passed after the final test migration. |
| `git diff --check` | Passed. |
| Maintained documentation link check | Passed: 3 documents, 27 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-08-call-evaluation-order-and-nominal-awbc --fail-on-blocking` | Passed: 95 packages, 2,226 Rust files, 310 review triggers, zero blocking violations. |

Workspace all-target/all-feature checking and linting cover the cross-crate
executable projection and public value/schema consumers. Clippy's library/
test-library summaries were HIR 59/76 (59 duplicates), core 110/129 (110
duplicates), sema 1,207/1,389 (1,206 duplicates), RuntimePlan 147/149 (147
duplicates), and compiler 222/226 (219 duplicates), plus other workspace and
integration warnings. No lint was suppressed and `-D warnings` was not used.

Intermediate failures were not counted as passes. Native order probes first
returned `43`; AWBC then exposed unsupported nominal assignment/projection
consumers before the final four-case success. A temporary diagnostic was used
to locate the HIR mismatch and removed. An intermediate controller-test edit
misplaced an observe assertion and omitted the comparison case; the case was
restored before the final 67-test run. One migrated carrier fixture had a
newline before a leading `+`; its corrected expression now passes in both
engines. The earlier local-assignment probes remain limitations, not successful
runtime validation.

The eight ignored HIR tests are the existing Tier 2 select/source/diagnostic/
total-slot and Flow/Thread item-limit cases. Their names and reasons are in
the library log. Full workspace tests, doctests, exhaustive codec/golden and
Tier 2 have not run in this follow-up and remain required for the connected
main push cut.

## Remaining executable failures

The main 53-test file contains 26 native/AWBC pairs and one negative codec/
verifier test. Its six remaining paired failures are unchanged requirements:
inferred callback effects; a project unit constructor depending on a later
argument; an unselected nominal case parameter depending on a later argument;
curried prefix values used as callbacks; monomorphic uses of generic prefixes;
and a shared prefix instantiated at distinct later types.

Two stronger migrated tests expose additional boundaries:

- `named_project_call_lowers_source_ordered_operands_into_typed_anf`:
  a named call with field-mutating block operands reaches runtime
  reachability with a structural Call projection and is rejected.
- `pipe_lowers_the_left_value_once_through_the_admitted_local`: the original
  ordinary-function pipe executes; its additional field-mutating block LHS
  with two placeholders fails sema with an unavailable final expression type.

These frontend failures are not repaired by sorting runtime operations. They
remain part of the coupled call/value/constraint work. Complete Content/Fx
ordering, assertion instance identity/inventory, first-class schemes and
continuations, bounded discovery, and program-bound restore are not established
by the successful cases here. Generic Match, retained View, RuntimePlan/task-plan,
remaining nominal C1–C6 and scheduler/restore remain goal requirements. There
is no external blocker and no implementation commit or push in this follow-up.

## Structural ownership review

The [findings](structure-audits/2026-09-08-call-evaluation-order-and-nominal-awbc/findings.md),
[file measurements](structure-audits/2026-09-08-call-evaluation-order-and-nominal-awbc/file_metrics.csv)
and [dependency measurements](structure-audits/2026-09-08-call-evaluation-order-and-nominal-awbc/package_metrics.csv)
are generated evidence. Whole-file measurements include inherited changes;
HEAD-to-current growth is not this turn's insertion count.

| Owner | HEAD → current LOC | Current bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| HIR expression-manifest `projection.rs` | 1,192 → 1,264 | 48,817 | 0 |
| RuntimePlan `semantic_facts.rs` | 7,328 → 10,526 | 400,720 | 0 |
| RuntimePlan `final_flow.rs` | 4,343 → 6,875 | 281,707 | 361 |
| RuntimePlan AWBC `expr.rs` | 1,865 → 2,081 | 78,032 | 0 |
| Core `value.rs` | 3,627 → 3,800 | 135,344 | 0 |
| Core AWBC verifier `code.rs` | 2,907 → 3,738 | 149,019 | 0 |
| Core AWBC `vm.rs` | 2,108 → 2,855 | 112,854 | 0 |

- HIR projection and its 990-line requirements sibling own the source-freeze
  contract. Field-form requirements now borrow the existing attached grammar
  projection rather than inventing an explicit-field form for every recovery
  payload. No alternate parse tree or name table was added.
- RuntimePlan semantic facts own checked call disposition. Flow lowering owns
  evaluation sequencing and the 125-line control-local admission context owns
  the once-materialized expression values. This replaces a narrower inventory;
  it does not add a second call-result authority. Lazy control paths remain
  explicit and their behavior is tested. The upper-size owners remain cohesive
  admission/control dispatchers; further decomposition must follow a real
  semantic boundary rather than separate publication from validation.
- Core values own physical record read/write behavior. AWBC verifier consumes
  the already-sealed type/field descriptor, and VM operations consume typed
  ordinals. RuntimePlan AWBC expression lowering is a projection consumer;
  diagnostic field labels no longer participate in execution. No transport,
  persistence adapter, I/O, unsafe code or Cargo feature was introduced.
- The compiler execution harness is 58 LOC / 2,446 bytes. Callable execution
  is 451 LOC / 11,422 bytes; project-instance integration 351 LOC / 11,104 bytes;
  Try/pipe 199 LOC / 5,797 bytes; controller tests 1,112 LOC / 34,922 bytes.
  The migrated tests drive the production engines and inspect typed evidence;
  the obsolete test-only ANF reader was deleted.
- Dependency fan-in/out remains HIR 10/3, sema 8/14, core 29/6,
  RuntimePlan 5/9, compiler 3/23. Development values are respectively 1/0,
  3/0, 3/6, 5/1 and 1/5. The Sans-I/O and layer boundaries are unchanged.

The scope archive was rechecked at 45,039 bytes and SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
Frozen archives and extracted mirrors were not edited. Versions remain `1`.
No branch/worktree, reset, checkout switch or unrelated cleanup was performed.
