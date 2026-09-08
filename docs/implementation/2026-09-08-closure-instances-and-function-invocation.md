# Closed closure instances and same-fiber function-value invocation

- Date: 2026-09-08
- Inspected HEAD and locally recorded `origin/main`:
  `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d`.
- Checkout: existing `main`; inherited changes were preserved. At the start
  of this follow-up: 8 deleted, 642 modified, 76 untracked status entries;
  the index was empty. After implementation and evidence updates: 8 deleted,
  643 modified, 82 untracked; the index remains empty. These counts include
  earlier goal work and grouped untracked directories.
- Supersedes the closure-execution status and test baseline in
  [constructor schema and nominal roots](2026-09-08-constructor-schema-and-nominal-roots.md).
  That note remains historical evidence for its constructor changes.
- The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
  This is an uncommitted part of its connected implementation, not acceptance
  of the complete callable model or a separate main push cut.

## Established behavior and ownership

Ordinary root closures now use the same closed semantic instance and frame
model as closures inside instantiated project functions. `RuntimeClosureInstanceKey`
retains the checked source closure and its optional enclosing project
instantiation. A root uses the global checked context directly; it does not
fabricate an empty generic solution or an ordinary Function declaration.
Nested closures are recursively owned by their exact executable partitions.

Compiler root production lives in `lower/closure_instances.rs`. It consumes
the accepted reachable executable partition and the sealed instance-discovery
graph. Function-owned roots remain the project-instance producer's concern;
Flow, Entry and impl-method roots use the common closure producer. Closure
body type/call/pattern/local/capture/statement facts are removed from global
inventories and published through the closed subcatalog. The outer closure
value remains in its enclosing scope. Global dialogue/content and trigger
publication observes the same ownership exclusion.

RuntimePlan admits root closures atomically with their exact checked context,
revision, lexical source, ABI and semantic partition. It requires closure
facts for executable closure values, excluding separately owned pure View
programs. The common scoped visitors include root closure nominal roots,
dialogue applications, content fragments and function-call edges. Selected
expression-child rows now include every owned expression, including leaf
closure values, without importing a nested closure body into its parent.

The old global closure parameter/capture allocation and unconditional
Expression/empty-effects site reservation were deleted. All explicit
closures use checked effects, suspension/control role, capture and parameter
ABI, and the same Expression/Executable function-site definition path.
Native and AWBC now execute closures which capture an enclosing local and
call an ordinary project function. A root closure can return a nested closure
whose executable body uses the outer parameter; both engines return `42`.

Native function-value applications now lower to typed `ApplyFunction` flow
operations. They share `FunctionCallFrame` and one invocation/return/unwind
implementation with terminal ProjectCall targets and omitted defaults. The
former `ProjectCallReturn` target/default representation was removed. A
default's continuation retains the already-evaluated prefix and logical
arguments; ordinary returns retain their result binding. The actual function
site owns return-type validation and fallthrough reporting, including a
default function's own site. Invocation remains in the current fiber and
does not use reveal activation or recursively run executable bodies through
the pure expression evaluator.

The native regression enters a captured executable function, reaches an
external task await, resumes on a typed Ready event, and returns the capture
through the caller binding. Existing direct/defaulted ProjectCall, goto,
fallthrough and source-ordered rest tests remain passing.

Core plan construction shares function-application type/argument validation
between expression and flow carriers. Free-local collection, local-scope
validation, operation traversal, entry verification and AOT scheduling
classification consume the new operation. The CLI's host-call inventory
already visits each owned function-site body through `visit_flow_ops`; a
dynamic invocation does not introduce another host-call declaration.

AWBC executable function headers now use the same canonical parameter names
as `MakeFunction` and expression function headers. The verifier was not
relaxed. Expression and flow function applications share AWBC lowering which
evaluates source operands once, expands admitted tuple/fixed-array spreads,
then assembles ABI order. This does not establish the complete compiler ANF
ordering requirement discussed below.

## Validation

All commands used Cargo's normal concurrency; no job-count override was set.

| Command | Result |
| --- | --- |
| `cargo check -p arcweft-compiler --tests --message-format=short` | Passed after common invocation and producer/consumer migration. Three existing sema dead-code warnings remain. |
| `cargo test -p arcweft-core -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --lib -- --nocapture` | Passed: core 357, sema 730, RuntimePlan 58, compiler 67; total 1,212. Zero failed/ignored. Log: `target/function-value-invocation-library-tests.log`. |
| `cargo test -p arcweft-compiler --test callable_execution -- --nocapture` | Failed: 36 passed, 14 failed, zero ignored, 50 total. Log: `target/function-value-invocation-matrix.log`. |
| `cargo test -p arcweft-lang-hir --lib final_lowering::tests::closure_calls -- --nocapture` | Failed: 3 passed, 1 failed, zero ignored, 889 filtered. Added after the workspace check/lint to isolate the existing paired HIR failure; no HIR production code was changed. Log: `target/direct-closure-hir-tests.log`. |
| `cargo check --workspace --all-targets --all-features --message-format=short` | Passed. Log: `target/function-value-workspace-check.log`. Sema's three dead-code warnings and Windows linker-output warnings in two macro crates remain. |
| `cargo clippy --workspace --all-targets --all-features --message-format=short` | Passed with warnings, exit 0. Log: `target/function-value-workspace-clippy.log`. This is not a warning-free result. |
| `cargo fmt --all` | Passed. |
| `git diff --check` | Passed after formatting. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-08-closure-instances-and-function-invocation --fail-on-blocking` | Passed after final formatting and the HIR fixtures: 95 workspace packages, 2,224 Rust files, 310 review triggers, zero blocking violations. |

The initial focused check exposed missing exhaustive consumers and seed/API
uses; those were repaired before the passing check. The new await fixture
initially assumed invocation and await occurred in one Engine step, then
treated the normal task-delivery diagnostic as an error. It now steps to the
typed waiting state and verifies the return value and final state after
delivery. Neither earlier fixture failure is counted as a pass. The earlier
46-case execution checkpoint was 34 passed / 12 failed; two new paired cases
added nested-closure success and a HIR failure below.

The workspace check/lint used all targets and features because this change
crosses public executable-operation and cross-crate semantic boundaries.
Clippy's library/test-library summaries were core 110/129 (110 duplicates),
sema 1,207/1,387 (1,206 duplicates), RuntimePlan 148/150 (148 duplicates), and
compiler 222/226 (219 duplicates), plus other workspace/integration warnings.
New invocation code also has by-value argument suggestions and its await
fixture has a length warning; these are not relabeled as inherited warnings.
No lint was suppressed and `-D warnings` was not used. Full workspace tests,
doctests, exhaustive codec/golden and Tier 2 are not run in this follow-up
and remain required before the connected main push cut. The paired compiler
matrix already contains required failing cases; full completion is not claimed.
The final documentation link check covered three maintained documents and
25 relative targets: zero missing files; anchors were not checked.

## Open obligations and concrete failures

The 14 matrix failures are seven cases, each in native and AWBC:

1. `callback_with_inferred_effects`: the dynamic call effect row is not closed.
2. `contextual_project_unit_constructor_closes_with_a_later_argument`:
   the child constructor needs a parameter determined by a later parent operand.
3. `contextual_project_constructor_infers_an_unselected_case_parameter`:
   the same issue includes the nominal owner's unselected case parameter.
4. `curried_prefix_as_callback`: the admitted value is still a
   `ProjectContinuation`, while ordinary application expects a Function.
5. `generic_prefix_as_monomorphic_callback`: checked runtime reachability
   still rejects the structural specialization projection.
6. `shared_prefix_with_distinct_later_types`: a Bound depth-zero reference
   reaches runtime projection outside its lexical scope.
7. `function_callee_is_captured_before_a_later_call_argument`: the parsed
   closure application with an assignment/block argument fails HIR
   source-component index validation. Four isolated HIR fixtures show that
   direct closure calls, calls with a capture, and calls with a simple block
   argument pass. The case with an outer-variable assignment and nested
   project call in the argument block fails during project staging. It has
   not reached runtime; this test does not yet prove an observed
   evaluation-order result. The initially incorrect module path for these
   new fixtures was repaired before running their four tests.

Compiler inspection also leaves a source-order obligation: composing a later
Flow-producing child currently materializes that child without generally
binding preceding pure callee/operand expressions first. The final typed
evaluation/materialization boundary must cover all eager producers and
consumers, preserve lazy branch selection, and avoid re-evaluating captures
or attached defaults. The AWBC argument-order repair alone does not close it.

Assertion metadata is not complete. A checked closure semantic digest now
permits a root closure to name its own guard owner without inventing a parent
Function. However, closure/project/default site definition still discards its
collected assertion-site inventory, and source-only closure/declaration guard
owners do not distinguish all closed instantiations and body/default roles.
Complete the typed instance/site-role identity and retain the metadata;
do not deduplicate real collisions or fabricate declarations. No assertion
inventory completion is claimed here.

Root Content/Fx/trigger and nominal-domain visitors have been migrated, but
their complete behavior/restore matrices have not been run. Function schemes,
finite specialization discovery and limits, callable persistence and the
remaining evaluation/suspension consumers stay in the
[active coupled request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
These are internal implementation/design obligations, not external blockers.
Generic Match, retained View, RuntimePlan/task-plan, nominal restore,
scheduler/restore and full main integration remain goal requirements.

## Structural review

The [findings](structure-audits/2026-09-08-closure-instances-and-function-invocation/findings.md),
[file metrics](structure-audits/2026-09-08-closure-instances-and-function-invocation/file_metrics.csv)
and [dependency metrics](structure-audits/2026-09-08-closure-instances-and-function-invocation/package_metrics.csv)
are generated evidence for the complete current files, including inherited
changes. They are not measurements of only this follow-up's additions.

Whole-file physical LOC at the inspected HEAD versus the current tree:

| Owner | HEAD → current LOC |
| --- | ---: |
| compiler `lower.rs` | 3,981 → 7,821 |
| core `engine.rs` / `engine/flow.rs` | 1,823 → 2,056 / 1,143 → 1,566 |
| core `plan.rs` / `plan/entry_inventory.rs` | 1,338 → 1,394 / 1,244 → 1,492 |
| core construction `seed.rs` / `lower.rs` | 2,392 → 2,524 / 4,358 → 5,361 |
| sema `callable/identity.rs` | 1,799 → 1,893 |
| RuntimePlan `semantic_facts.rs` | 7,328 → 10,516 |
| RuntimePlan `final_expr.rs` / `final_flow.rs` | 1,967 → 2,539 / 4,343 → 6,808 |
| AWBC `expr.rs` / `flow.rs` / `inventory.rs` | 1,865 → 2,076 / 2,803 → 3,247 / 1,969 → 2,170 |

The scoped project-function fact owner is absent from HEAD and has 2,364
current lines, including substantial inherited work. The new core invocation
module has 320 lines. Their responsibilities are reviewed below rather than
treating a missing base file as evidence of zero pre-existing work in the
dirty checkout.

- Compiler `lower.rs` (7,821 LOC / 330,862 bytes) owns conversion from the
  accepted semantic world into the single runtime fact inventory. Root
  discovery was separated into its 59-line closure producer; it borrows the
  sealed graph and owns no alternative inference or publication state.
- RuntimePlan `semantic_facts.rs` (10,516 LOC / 400,146 bytes) and
  `semantic_facts/project_function.rs` (2,364 LOC / 86,815 bytes) retain the
  admission transaction, exact scoped subcatalog and recursive consumer
  traversal. This change removes global/scoped dual readers. Their size
  remains an explicit cohesion concern; schema validation and publication
  must not be split into independent authorities merely to reduce LOC.
- RuntimePlan `final_flow.rs` (6,808 LOC / 278,880 bytes; 361 embedded test
  lines) owns checked control lowering and frame-local admission;
  `final_expr.rs` (2,539 LOC / 102,054 bytes) owns expression projection.
  Call result continuation lowering was unified, and obsolete global closure
  paths were deleted. The embedded tests continue to exercise that owner.
- AWBC `expr.rs` (2,076 LOC / 77,759 bytes), `flow.rs` (3,247 LOC /
  126,727 bytes), and `inventory.rs` (2,170 LOC / 86,984 bytes) retain expression
  code generation, control-block construction and canonical table interning.
  Application emission and parameter naming now have one owning implementation
  each; no verifier exception, runtime type inference or wire version was added.
- Core `engine.rs` (2,056 LOC / 77,835 bytes) retains fiber state ownership;
  `engine/flow.rs` (1,566 LOC / 61,431 bytes) retains operation dispatch and
  ProjectCall materialization. Invocation/return/unwind was decomposed into
  `engine/flow/function_call.rs` (320 LOC / 12,358 bytes), with private frame
  state and visibility limited to the Engine implementation. This boundary
  adds no host, transport or storage dependency.
- Core `plan.rs` (1,394 LOC / 49,255 bytes), construction `seed.rs`
  (2,524 LOC / 78,846 bytes), construction `lower.rs` (5,361 LOC /
  222,114 bytes; 208 embedded test lines), and `entry_inventory.rs`
  (1,492 LOC / 56,217 bytes) retain transient seed, recursive typed admission,
  sole executable representation and entry-catalog validation responsibilities.
  The shared application validator is builder-owned and does not widen an
  internal API just to support file splitting.
- Sema `callable/identity.rs` (1,893 LOC / 58,607 bytes) owns checked callable
  and closure identity encoding. Compiler `project/tests.rs` (1,564 LOC /
  54,262 bytes) only migrated its exact-parent assertion; the execution cases
  live in the separate 402-line paired compiler integration-test file.
  The later HIR fixtures are a private child of the existing final-lowering
  tests and exercise the same accepted-source transaction helper. They add
  no production API, alternate source reader or source-spelling gate.
- Workspace dependency fan-in/out remains core 29/6, sema 8/14,
  RuntimePlan 5/9, compiler 3/23 (development respectively 3/6, 3/0, 5/1,
  1/5). No Cargo manifest/feature, Sans-I/O boundary or layer direction changed.

The retained scope archive was rechecked at 45,039 bytes and SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
The archive and frozen extracted mirror were not edited.
Contract and identity-domain versions remain `1`. No branch/worktree,
destructive Git operation, implementation commit or push was performed.
