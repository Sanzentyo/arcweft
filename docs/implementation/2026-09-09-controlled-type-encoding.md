# Controlled canonical type encoding — 2026-09-09

Supersedes the encoding and validation state in
[controlled type projection](2026-09-09-controlled-type-projection.md). The
[convergence goal](2026-09-08-convergence-goal-plan.md) remains active; this is
progress within its connected implementation, not a completed main push cut.

Inspected HEAD is `4da7bfd7a9f8324f3763d9e579fce1f0ab38965d` on the existing
`main` checkout. `git fetch origin` confirmed 0/0 divergence. Before this note,
Git reported 8 deleted tracked paths, 648 modified tracked paths and 106
untracked status entries, including the new audit directory and inherited
work. The index is empty. No branch, worktree, reset, commit or push was made.

## Implemented behavior

The semantic type identity encoder now uses an explicit task stack. Ordinary
children append to the parent's canonical stream. Type lists retain a cursor
instead of allocating tasks or copying all unvisited elements. Function
continuations restore their enclosing lexical scope after writing the result
and effect row; array lengths and projection suffixes retain their original
position in the stream.

Variant payload identity requires independent hashes of its owner and fields.
Those computations now use encoding frames on the same stack and retain their
insertion depth and incoming binders. The payload owner joins the resulting
typed digests using its existing case/field identity rules. The old recursive
`semantic_case_in_scope` path and recursive child-encoding helpers were removed.
Default and controlled entry points share this encoder; there is no second
serializer or preflight type walk.

The consumer's `TypeProjectionControl` admits structural type, constant and
effect nodes, including concrete effect labels and tails. Incoming scope
binders and function binders consume binding work. Depth derives from the
finite owned type tree, with checked host-index increments; it is not a wire
integer or a caller-supplied traversal counter. The compiler's existing ledger
enforces its inclusive `u64` depth/node/work bounds before admission.

Checked array-length bytes now use the same controlled/default scalar encoder
and report typed failures. Invalid scope, escaped inference, unresolved terms
and encoding-length overflow no longer collapse into `None`, a mapper-seal
failure or a deferred-row mismatch. All four checked-application consumers
were migrated to preserve the instantiation cause.

An effect row's existing version-1 identity is the nullary `Unit` function
carrying that row. The encoder writes it from the borrowed row using the shared
function header and effect writer; it no longer allocates a synthetic owned
function type and cloned row. This changes neither its identity nor the
effect algebra.

Completed project-instance keys now account for base projection and canonical
type/constant/effect bindings using the same control as substitution. Their
errors preserve the exact consumer abort through the callable and compiler
boundaries. Checked nominal specialization also uses controlled hashing and
avoids one whole-type clone previously made solely to hash the closed type.

## Validation actually run

| Command / scope | Result |
| --- | --- |
| Original `nested_scopes_and_payload_hashes_keep_version_one_identity` fixture | Passed before replacing the encoder. Captured two then-current working-tree digests in `target/type-encoding-v1-baseline.log`. |
| `cargo check -p arcweft-lang-sema --all-targets` | Passed for the iterative encoder and again after typed constant errors. |
| `cargo check -p arcweft-compiler --all-targets` | Passed after connecting callable key encoding. |
| `cargo test -p arcweft-lang-sema --lib types::digest::tests -- --nocapture` | 7/7 passed before the later effect/constant tests; 750 filtered out. |
| Exact compiler `instance_key_encoding_spends_the_same_projection_budget` test | Corrected final run passed, 81 filtered out. |
| `cargo test -p arcweft-lang-sema --lib -- --nocapture` | 752 passed / 7 failed / 759 total, zero ignored. All nine digest tests passed. |
| `cargo test -p arcweft-compiler --lib --test project_function_instances --test project_cache_transaction --test callable_execution --test evaluated_effects --test try_pipe --no-fail-fast -- --nocapture` | Library 82/82, project instances 11/11, cache transactions 19/19, evaluated effects 9/9 and Try/pipe 8/8 passed. Callable execution: 53 passed / 18 failed / 71 total. |
| `cargo check --workspace --all-targets --all-features` | Passed, exit 0. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, exit 0. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/2026-09-09-controlled-type-encoding --fail-on-blocking` | Passed: 95 packages, 2,234 Rust files, 309 review triggers, zero blocking violations. |
| `cargo fmt --all`, `cargo fmt --all -- --check`, `git diff --check` | Passed, exit 0. |
| Documentation links | Three maintained documents, 44 relative targets, zero missing files. Anchors were not checked. |

The fixed expected digest values are captured from the working tree before this
algorithm change, not from an assumed released format or from HEAD's earlier
type model. They verify unchanged version-1 bytes for nested binders, payload
owner/field hashes, array constants, function effects and projection suffixes.
The constant test additionally asserts its exact scoped bytes and typed invalid
scope/unresolved failures. Effect-row identities match the corresponding
function-type identity and produce the same controlled visits.

A 10,000-level owned type encodes without recursive traversal. A depth limit
of 32 rejects its 33rd node; the test drops that deliberately extreme input
iteratively as well. Other tests prove exact node limits, cancellation without
an identity, and depth retention through nested independently hashed payloads.

The compiler key test confirms the actual checked pure invocation still has an
effect binding. Its type/effect closure and function type use six structural
visits; key encoding requires five more. Limits 6, 8 and 10 reject attempts 7,
9 and 11 with the original call origin and prevent sealing. The complete key
uses 11 structural visits and 15 work units. The initial test incorrectly
omitted that existing pure effect binding; it was corrected against the typed
binding inventory, not by changing production accounting.

The existing failures remain three contextual constructor cases and four
inferred higher-order effect cases in sema, plus the same 18 paired native/AWBC
failures recorded in the preceding note. The integration total remains 100
passed / 18 failed. They were neither ignored nor removed from acceptance.

Logs use `target/type-encoding-{sema-final,compiler-final,workspace-check,workspace-clippy,structure-audit}.log`.
Clippy library/test-library counts are sema 1,204/1,396 (1,202 duplicates),
compiler 223/227 (220 duplicates), and runtime-plan 146/148 (146 duplicates),
plus other workspace/integration warnings. The controlled canonical binding
writer has a 115-line cohesion warning; it retains one fixed base-then-bindings
transcript order under one control, rather than splitting that protocol among
independent writers. No blanket suppression was added.

Full workspace tests, doctests, exhaustive codec/golden and Tier 2 were not run
in this follow-up. They remain required for the connected main push cut.

## Structural ownership

Generated [findings](structure-audits/2026-09-09-controlled-type-encoding/findings.md),
[file metrics](structure-audits/2026-09-09-controlled-type-encoding/file_metrics.csv)
and [dependency metrics](structure-audits/2026-09-09-controlled-type-encoding/package_metrics.csv)
describe the final Rust state. Growth from HEAD includes inherited work.

| Owner / path | HEAD → current physical LOC | Bytes | Classification |
| --- | ---: | ---: | --- |
| sema `types/digest.rs` | 1,028 → 1,342 | 48,720 | production |
| sema `types/digest/traversal.rs` | new → 140 | 5,816 | production |
| sema `types/digest/tests.rs` | extracted and extended → 428 | 14,465 | test |
| sema `types/generics.rs` | existing untracked → 611 | 19,476 | production |
| sema `types/constraints/solution/instantiation.rs` | existing untracked → 334 | 13,204 | production |
| sema `types/variant_payload/projection.rs` | existing untracked → 399 | 13,605 | production |
| sema `callable/checked_application.rs` | 3,641 → 4,498 | 166,888 | production |
| sema `callable/join.rs` | 859 → 1,906 | 72,297 | production |
| compiler `lower/project_instances/tests.rs` | existing untracked → 668 | 24,754 | test |

These files have zero embedded test LOC after extracting the identity tests.
The digest owner retains the exhaustive fixed-tag schema and scalar codecs;
traversal scheduling and independent hash state are a separate child owner.
This is a state/algorithm/test decomposition, not an alternative type model.
The payload owner still owns case/field identity construction and ordinal rules.
Scoped views forward their incoming context rather than reconstructing it.

The large checked-application owner retains immutable source/application
evidence and frozen solution construction; it now propagates typed scalar
encoding errors. The callable join owner retains selection and its canonical
instantiation transcript. Neither owns compiler policy, counters or I/O.
These cohesive evidence and protocol boundaries justify keeping their current
owners while the coupled callable migration continues. Normal/development
workspace fan-in/out remains sema 8/14 and 3/0, compiler 3/23 and 1/5. There is
no new Cargo dependency, feature, facade or platform boundary.

## Remaining work

Complete bounded compilation is still unproven. Default semantic/frozen-source
identity entry points and runtime type normalization do not yet all receive
the compilation ledger. Checked nominal `ty()` construction, surrounding
copies/equality checks and other recursive non-encoding traversals remain to
be reconciled with their actual owners. Do not treat the new controlled API or
its completed-instance consumer as proof that every compiler path is bounded.

Function schemes, partial constructor source constraints, declaration-owned
effect inference, callable value specialization/execution and persistence
remain required in
[AW-AH-009.4.2.1.1.1](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Match, View, RuntimePlan/task-plan, nominal C1-C6 and scheduler/restore acceptance
remain in the full active goal. No external blocker, contract-version change,
compatibility exception or frozen-package edit is claimed.

Final Git status is 8 deleted tracked paths, 648 modified tracked paths and
107 untracked status entries, with an empty index. Required work remains, so
the goal is neither complete nor blocked.
