# Instance graph admission and executable type dependencies — 2026-09-09

Supersedes the compiler/runtime validation state in
[recovery value evidence](2026-09-09-recovery-value-evidence.md). That record's
semantic-analysis results remain historical; this follow-up did not change or
rerun the sema library.

Inspected HEAD is `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d` on the existing
`main` checkout. The final `git fetch origin` confirms the same remote SHA and
0/0 divergence. Before adding this note, the index was empty and Git reported
8 deleted tracked paths, 647 modified tracked paths and 100 untracked status
entries. These totals include inherited connected work. No branch, worktree,
reset, implementation commit or push was made.

## Instance discovery

`ProjectInstantiationSession` now retains exact roots and dependency edges
alongside its existing canonical-key work queue. Admission checks that a key
matches its callable, declaration, group and closed substitution. Repeated
keys require equal callable, selection and solution evidence; a different
root origin may reference the same instance without replacing its evidence.
Materialization checks the previously admitted root or caller-to-callee edge,
not only whether the target happens to exist elsewhere in the graph.

The compiler configuration now supplies `ProjectInstantiationControl` to the
same discovery session through project and source entry points. Ordinary entry
points use production limits. Embedders and tests can provide limits and a
shared atomic cancellation flag. There is no new CLI option or second compiler
pipeline. The implemented inclusive graph bounds are:

| Resource | Production limit |
| --- | ---: |
| Distinct instances | 4,096 |
| Distinct dependency edges | 65,536 |
| Graph visits and state transitions | 4,194,304 |

Each root/dependency visit consumes one work unit, including duplicate and
rejected visits. A new queued node, beginning its discovery and completing its
discovery each consume one further unit. Duplicate edges consume visit work
without consuming another distinct-edge slot. Root evidence is retained by
origin separately from edges between instances. Checked arithmetic and all
admission budgets are evaluated before graph mutation. A performed visit
remains charged when later admission fails.

Budget exhaustion and cancellation latch an abort that prevents session
publication. Cancellation is checked at admission, queue transitions, sealing
and materialization. Pending, dropped or foreign work remains governed by the
existing affine completion permission. Large dependency-error keys are boxed
at the error boundary while retaining their exact typed contents.

The integration regression `growing_instance_graph_stops_at_the_configured_inclusive_bound`
compiles `grow<T>(value: T) -> i64 { grow([value]) }` with an instance limit of
two and observes the compiler's dedicated instantiation-limit diagnostic.
`same_type_recursion_reuses_the_single_allowed_instance` compiles at exactly
one instance and one self edge. A cache-transaction regression verifies that a
later over-limit compile writes no cache artifacts, preserves the previous
accepted semantic generation and HIR lease, and permits a subsequent normal
compile with the same program hash.

This is not complete bounded specialization. The required cumulative
type/constant/effect-node count and structural-depth limit are not yet wired
into this session. Closed substitution, canonical encoding and repeated
materialization projection still need bounded iterative traversal and shared
work accounting before their allocations. The complete graph transcript and
all finite-discovery obligations also remain. The graph counters above must
not be treated as proving a bound on the cost or stack depth of semantic type
projection.

## Executable type dependencies

Broader compiler validation exposed two additional failures. One test still
looked for every runtime type in the global fact map, although closed function
instances now own their local/expression/pattern/source-type projections. The
test now checks the relevant global and instance scopes and requires exactly
one owner in its monomorphic fixture. Presentation types must be absent from
all those scopes. No production fallback reader was added.

The other failure was a missing type in the runtime transaction graph. Variant
domains inside function instances retained every case, but type-root
collection visited only the instance's ordinary result/type projections. For
`enum Event { Empty, Text String }`, constructing `.Empty` therefore omitted
the unselected case's tuple payload type. Constructor calls had the same
omission, including at global scope.

`semantic_facts/type_dependencies.rs` now owns inherent dependency traversal
for the existing typed facts. Global, function-instance and closure catalogs
compose the same owner APIs. The traversal covers call targets and operands,
attached and continuation ABIs, record schemas, all variant case payloads,
Try/iterator carriers, captures, content/effect operands, defaults and nested
closure catalogs. Expression, pattern, statement and static-target families
are matched exhaustively. The old copied global-only loops are removed.
Type roots still enter the existing aggregate type graph and verifier; there
is no extra type registry, layout reconstruction or source/HIR re-walk.

Five new native/AWBC pairs pass: variant values in a function, in a nested
closure and in a root closure; and constructor calls in a function and at
global scope. Each constructs the empty case while retaining the other case's
unused `String` payload schema. Both broader cache/projection tests now pass.
These results establish this dependency boundary, not all nominal C1-C6 or
program-bound restore acceptance.

## Validation actually run

| Command / scope | Result |
| --- | --- |
| `cargo test -p arcweft-runtime-plan --lib -- --nocapture` | Passed: 58/58, zero ignored. |
| `cargo test -p arcweft-compiler --lib --test project_function_instances --test project_cache_transaction --test callable_execution --test evaluated_effects --test try_pipe --no-fail-fast -- --nocapture` | Library passed 76/76. `callable_execution`: 53 passed / 18 failed / 71 total. Cache transaction 19/19, project instances 8/8, evaluated effects 9/9 and Try/pipe 8/8 passed. |
| `cargo test -p arcweft-compiler --lib lower::project_instances -- --nocapture` | Final focused run passed 16/16, with 62 filtered out. This includes the two tests added after the broad run, plus post-lint error-storage cleanup. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0, after production cleanup. The two later additions were tests only and were compiled by the final Clippy run. |
| `cargo clippy --workspace --all-targets --all-features` | Final run passed with warnings, exit 0. Newly introduced owner warnings were repaired without blanket lint suppression. |
| `cargo fmt --all` | Passed after the final Rust edits. |
| `cargo fmt --all -- --check`, `git diff --check` | Passed, exit 0. |
| Maintained documentation link check | Passed: 3 documents, 40 relative targets, zero missing files. Anchors were not checked. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-instance-graph-and-type-dependencies --fail-on-blocking` | Passed: 95 packages, 2,229 Rust files, 310 review triggers, zero blocking violations. |

The integration total in the broad run is 97 passed / 18 failed. The previous
18 callable-execution failures remain; the ten new executable tests pass.
An earlier expanded cache run had 17 passed / 2 failed, identifying the scope
lookup and missing type-root defects repaired above. Initial compile attempts
also exposed a missed closure projection consumer and test calls to private
APIs; those consumers were migrated without widening private APIs for tests.
The full compiler library was not rerun after the last two graph tests were
added; the final focused run explicitly covers both new tests.

Logs are `target/project-instance-graph-{compiler,focused,workspace-check,workspace-clippy,structure-audit}.log`
and `target/project-instance-type-roots-runtime-plan.log`. Final Clippy reports
compiler library/test-library 222/226 warnings (219 duplicates), runtime-plan
146/148 (146 duplicates), plus other workspace and integration warnings.

Full workspace tests, sema library tests, doctests, exhaustive codec/golden and
Tier 2 were not run in this follow-up. Required final tiers remain attached to
the connected main push cut. The native/AWBC cases use the production execution
helper and do not replace those tiers.

The review inventory contains 71 ZIPs and no root-inbox ZIP. Their hashes were
recorded in `target/instance-graph-review-archives.csv`. The retained scope
archive remains 45,039 bytes with SHA-256
`6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185`.
No frozen archive or extracted mirror was edited.

## Structural review and remaining work

The generated [findings](structure-audits/2026-09-09-instance-graph-and-type-dependencies/findings.md),
[file metrics](structure-audits/2026-09-09-instance-graph-and-type-dependencies/file_metrics.csv)
and [dependency graph](structure-audits/2026-09-09-instance-graph-and-type-dependencies/package_metrics.csv)
describe the final Rust state. HEAD-to-current growth includes inherited work.

| Owner / path | HEAD → current physical LOC | Bytes | Embedded test LOC |
| --- | ---: | ---: | ---: |
| compiler `src/lower/project_instances.rs` | existing untracked → 548 | 18,450 | 0 |
| compiler `src/lower/project_instances/tests.rs` (test) | existing untracked → 472 | 17,250 | 0 |
| compiler `src/lower/closure_instances.rs` | existing untracked → 63 | 2,568 | 0 |
| compiler `src/lower.rs` | 3,981 → 7,844 | 331,932 | 0 |
| compiler `src/project/registration.rs` | 312 → 329 | 10,924 | 62 |
| compiler `src/project.rs` | 1,689 → 1,708 | 60,727 | 0 |
| compiler `src/source.rs` | 173 → 234 | 7,850 | 87 |
| runtime-plan `src/semantic_facts/type_dependencies.rs` | new → 346 | 12,277 | 0 |
| runtime-plan `src/semantic_facts.rs` | 7,328 → 10,378 | 394,895 | 0 |
| compiler `tests/callable_execution.rs` (test) | existing untracked → 563 | 14,030 | 0 |
| compiler `tests/project_function_instances.rs` (test) | existing untracked → 389 | 12,802 | 0 |
| compiler `tests/project_cache_transaction.rs` (test) | 1,453 → 1,615 | 57,025 | 0 |
| compiler `tests/evaluated_effects.rs` (test) | 439 → 730 | 24,991 | 0 |

Discovery state, counters, root/edge evidence and affine work remain in the
compiler's existing instance owner, with dedicated tests. Compilation context
owns configuration only. The large compiler lowerer forwards the same control
and uses the sealed graph; it gained no second cache or discovery state.
Its existing semantic-to-runtime orchestration remains coupled to accepted
generations and closed instance environments. Physical splitting without
separating that authority would not improve this migration.

The runtime-plan decomposition follows a real responsibility: dependency
traversal on typed facts is now separate from aggregate admission/storage in
the large semantic-facts owner. Global and nested scopes use it together.
The parent retains the existing atomic fact-family/coverage validation and
generation-bound inventory rather than exposing mutable fragments or a second
reader. Test changes follow graph admission, scope ownership and backend
execution. No dependency, facade, transport, persistence or platform-I/O
boundary was added. Compiler fan-in/out remains 3/23 (development 1/5),
runtime-plan 5/9 (development 5/1).

The [active convergence goal](2026-09-08-convergence-goal-plan.md) remains
unfinished. Partial source constraints, function-scheme specialization,
inferred effect rows, a complete callable-value execution contract and bounded
type traversal remain coupled in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Match, View, RuntimePlan/task-plan, remaining nominal and scheduler/restore
work is not waived. No external blocker or completed-goal claim is present.
All contract versions remain `1`; no compatibility or layer-direction
exception was introduced.
