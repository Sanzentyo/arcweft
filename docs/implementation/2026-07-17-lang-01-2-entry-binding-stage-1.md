# Lang-01.2 Stage 1 — typed source-entry binding

## Outcome

Stage 1 establishes one checked project catalog for source entry roles before
the old `state`, `reducer`, and `agent` declaration families are removed.
Source `entry` declarations are the only authority for state, initializer,
event, reducer, initial-flow, and Agent-controller bindings. Launch profiles
select a complete `entry.*` ID and expected entry kind; they cannot repeat any
of those roles.

The compiler constructs `CheckedEntryCatalog` immediately after registered
project type checking. Entry checking and launch selection both complete before
Style, line-task, or RuntimePlan lowering. `CompiledProject` retains the checked
catalog, and project semantic indexing consumes `HirProject` plus that catalog
instead of reconstructing roles from strings.

## Checked contracts

Stateful `game`, `editor`, and `test` entries require exactly one of every role:

- `state`: an ordinary registered struct;
- `initializer`: ordinary `fn () -> State` with an explicit closed empty effect
  row and an empty inferred effect row;
- `event`: an ordinary registered enum;
- `reducer`: ordinary
  `fn (&State, Event) -> Result<Reduction<State>, ReducerError>` with an
  immutable elided borrow, owned event, explicit closed empty effect row, and
  empty inferred effect row;
- `goto`: one `flow.*` declaration whose sole fixed parameter is owned `State`.

Agent entries require one ordinary zero-parameter controller returning
`Result<Unit, AgentError>`. Its effect row must be explicit and closed; inferred
effects must be a subset. The checked policy includes the declared effects,
inferred effects, and effective `#[budget(...)]` limits.

All callable roles resolve through the shared registered callable catalog and
retain their canonical `CallableDeclarationId`. Same-named functions in
different modules therefore remain distinct.

## Identity contract

Schema-v1 digests use domain-separated, versioned canonical byte encoders.
They contain canonical nominal/callable/flow identities and typed contracts,
never absolute filesystem paths or authored type aliases. The stateful binding
digest includes entry kind and ID, package identity, state/event schema
digests, initializer/reducer declaration and contract digests, and initial-flow
identity and contract digest. Agent bindings include controller identity and
contract plus the complete Agent policy digest.

All three checked binding representations share the
`arcweft.checked-entry-binding` domain and therefore carry an explicit,
prefix-free representation tag: stateful is `1`, Agent is `2`, and the
non-stateful launch-model binding is `3`. The latter is not a reusable owner
identity: it still binds one package-qualified source entry ID to its checked
launch kind. An exact-byte test fixes that representation boundary and avoids
letting the first package-string length masquerade as a variant discriminator.

The ID-006 matrix needs one explicit interpretation. The reducer contract is
exact, so an authored signature or effect change is invalid and is rejected
before a checked binding exists; it cannot produce a second accepted binding
digest. Canonical encoder unit coverage separately proves that changing the
reducer contract digest changes the stateful binding digest. This tests the
wiring without weakening the exact source contract.

Body-only changes are deliberately excluded from entry binding identity. The
tests prove both sides of that boundary: valid body variants preserve the
binding/contract digest while changing the existing `SourceRevision` and
compiler compile-unit fingerprint.

## Compiler and CLI selection

`ProjectCompilationContext` optionally carries a typed
`ProjectEntrySelection { PublicId, ProjectEntrySelectionKind }`. The CLI maps
`LaunchKind` exhaustively to that enum and passes the profile's typed entry ID
into project compilation. Selection fails at `entry-selection` when the entry
is absent or its checked kind differs. The comparison is enum-based; no kind
string dispatch is used.

The same context retains the already typed
`EnvironmentCallablePublication` values selected by the CLI. Registration
folds them into the existing atomic `CharacterRegistrationRequest`; it does not
reconstruct callable paths or add another resolver. Standard publications use
their fixed standard owners, while selected custom and desktop manifests use
their existing adapter owners. This closes the compiler integration gap that
previously dropped accepted adapter callables before source-entry contract
checking.

The AW-AH-009.3.3 reconciliation in this cut also removes the last separate
Rust-function lookup used by source entry checking. `extern rust mod` members
are collected as typed `ProjectCallablePath` aliases, bound only after the
accepted environment catalog has been built, and must match one exact Rust
export by package, callable name, and signature. The resulting project binding
points to the environment callable ID and all calls continue through the shared
resolver. `RustPackageExports` now retains type exports only; there is no dual
function reader in `TypeCheckEnv`.

Standard manifests augmented with Rust metadata publish only their ordered Rust
delta beside the fixed standard publication. Custom manifests publish their
complete accepted callable set. Free aliases, selected environment methods,
entry-role callables, custom adapters, inference adapters, and Rust metadata
therefore consume the same immutable catalog and checked query-work budget.

`Reduction.unchanged` is represented as a core owned callable identity and a
family schema with dependent validation. It requires exactly one positional
shared borrow and returns `Reduction<State>` for that same state type. The
modern-feedback sample now compiles through the canonical profile/project path,
so this constructor is no longer supplied by a test-only environment string or
an ad hoc source-name branch.

Direct-source CLI commands still require an explicit canonical `--entry
entry.*`. A profile's entry cannot be overridden. Profile fields named
`state`, `initializer`, `event`, `reducer`, or `controller` are rejected by
both ordinary manifest decoding and the source-backed TOML diagnostic path.

A profile-selected source outside the default source root is the crate root for
that compilation; its imports still resolve from the configured source root.
The loader accepts that root source directly and rejects a non-root `mod`
declaration through the ordinary typed module-path mismatch. It no longer first
infers a contradictory module identity from the selected file's location.

## Stage 1 boundaries

This stage does not yet delete the old syntax/HIR declaration variants or add
state commit/save/replay execution. Those are subsequent Lang-01.2 stages. It
does provide the typed binding, identity, compiler gate, and project-index
substrate those removals require, without compatibility aliases or dual role
readers.

The binding-validation follow-up is recorded in
[`2026-07-17-lang-01-2-binding-validation-stage-2.md`](2026-07-17-lang-01-2-binding-validation-stage-2.md).

## Fixture and profile migration

Making launch-profile `entry` required intentionally exposed every provisional
profile and sample that relied on implicit first-flow selection. Checked-in
manifests now select a complete `entry.*` identity, and their sources declare
the matching typed entry. Stateful game/test samples also define the exact
state, event, initializer, reducer, and state-parameterized initial-flow
contract. State-free presentation and tool fixtures use explicit CLI entries
instead of inventing dummy state.

The migration covered project-loader, LSP profile/session/integration fixtures,
bundle and CLI runtime fixtures, checked-in samples, `web`, runnable examples,
native-capture sources, and canonical parser/runtime fixtures. A one-off
manifest and source inspection confirmed that checked-in profile blocks select
complete `entry.*` IDs and that canonical source fixtures no longer depend on a
bare entry form. This was a review step, not a source-gate test.

Runtime fixtures now assert the observable contract they own. In particular,
the AOT profile test uses an actually linear return-only flow and structured
JSON fields; it no longer assumes that a mixed-dispatch logging effect counts
as two fast-path operations. CLI/test/bench fixtures each use a distinct typed
entry and source module.

The direct-run fixture harness now always passes `--entry entry.main`, and each
runnable fixture declares that exact entry. The server-health fixture was moved
from the run inventory to the compile/check inventory because a routed server
entry does not select one runnable flow. This preserves the server surface
coverage without restoring implicit first-flow execution.

## Structural audit

The rebased audit was run from Jujutsu change `mkpplpty` on parent
`9a63ac5512cd`. The canonical audit scanned 3,183 files, including 1,611 Rust
files and 739,930 Rust physical LOC. It reported zero errors and 127 warnings.
The 12,399-LOC Unicode vertical-orientation table is explicitly marked generated
and excluded from the production hotspot ranking below.

The new entry ownership boundary stays below the production warning threshold:

| Path | Owning crate | Bytes | Physical LOC | Role |
|---|---:|---:|---:|---|
| `src/entry/digest.rs` | `arcweft-lang-sema` | 33,566 | 1,008 | canonical typed contract and binding encoders plus exact-byte unit tests |
| `src/entry/checker.rs` | `arcweft-lang-sema` | 27,129 | 755 | project entry orchestration and diagnostics |
| `src/entry/checker/contract.rs` | `arcweft-lang-sema` | 28,469 | 734 | callable/flow contract canonicalization and exact role contracts |
| `src/entry.rs` | `arcweft-lang-sema` | 17,435 | 631 | public checked-entry data model and catalog |
| `src/entry/checker/nominal.rs` | `arcweft-lang-sema` | 16,000 | 436 | nominal resolution and schema ownership |
| `src/project_index/entry_roles.rs` | `arcweft-lang-sema` | 7,409 | 246 | checked catalog projection into semantic-index edges |
| `src/project/entry_tests.rs` | `arcweft-compiler` | 5,589 | 180 | selection and compile-artifact identity tests |
| `src/entry/checker/roles.rs` | `arcweft-lang-sema` | 2,458 | 81 | exact role inventory and duplicate detection |
| `src/callable/identity.rs` | `arcweft-lang-sema` | 37,740 | 1,311 | shared project/environment/core callable identities |
| `src/callable/resolver.rs` | `arcweft-lang-sema` | 37,524 | 1,166 | free and selected environment resolution under one query budget |
| `src/callable/schema/families.rs` | `arcweft-lang-sema` | 31,792 | 896 | closed family schemas, including typed `Reduction` constructors |
| `src/checker/expr/binary.rs` | `arcweft-lang-sema` | 6,504 | 157 | binary-operator type checking split from the expression dispatcher |
| `src/checker/expr/reduction.rs` | `arcweft-lang-sema` | 4,196 | 96 | dependent `Reduction` constructor validation |

`arcweft-compiler/src/project.rs` is 37,404 bytes / 1,138 physical LOC after
moving the new entry tests to its responsibility module, so it remains below
the 1,200-LOC production warning. Existing changed hotspots above the warning
threshold were inspected individually: syntax item/parser ownership remains in
`arcweft-lang-syntax`; type/effect registration remains in
`arcweft-lang-sema`; AWBC/runtime projections remain in `arcweft-core` and
`arcweft-runtime-plan`; and CLI/launch files only forward the typed selection.
This stage did not add a new responsibility to those files or a reversed crate
dependency.

The combined callable work initially pushed
`arcweft-lang-sema/src/checker/expr.rs` over the 2,500-LOC error threshold.
Binary-operator checking and dependent `Reduction` validation were moved into
their named responsibility modules; the dispatcher is now 89,906 bytes / 2,373
physical LOC. This is a direct ownership split, not an audit exemption.

The largest current workspace Rust files are existing integration/unit test
inventories: `cli_runtime_bench.rs` (256,672 bytes / 7,984 LOC),
`native_vertical.rs` (238,805 / 6,620),
`published_jlreq_class_mix.rs` (220,473 / 6,109), and
`native_samples_effects.rs` (214,730 / 5,850). The largest changed test file is
the existing `arcweft-compiler/src/tests.rs` inventory (180,861 / 5,397); this
stage only updates entry fixtures there and adds its new tests to the separate
180-LOC module above. The added CLI/test/bench profile integration is isolated
in `profile_entry_selection.rs` (3,627 bytes / 155 LOC). The first audit caught
the original fixture expansion crossing the 8,000-line error threshold; moving
that coherent profile-entry scenario to its own file restored the final zero-
error result without a lint exemption.

Normal-dependency fan-in/fan-out remains: syntax 10/5, HIR 10/3, sema 8/10,
compiler 4/14, launch 3/5, manifest-model 2/5, core 21/6, runtime-plan 3/8,
bundle 10/23, project 3/6, and the CLI application 0/65. The new direction is
the existing layered flow `syntax -> HIR -> sema -> compiler/tooling`; no lower
layer depends on compiler or CLI entry selection.

## Verification

After the latest-main rebase:

- `cargo test -p arcweft-lang-syntax --test entry_roles`: 7 passed;
- `cargo test -p arcweft-lang-syntax --test parser_flow_statements_and_body
  entry_goto_is_the_structured_flow_dispatch_item`: 1 passed;
- `cargo test -p arcweft-lang-sema entry::`: 26 passed, including ID-001
  through ID-008 and all exact-byte binding encoders;
- `cargo test -p arcweft-lang-sema project_index`: 14 passed;
- `cargo test -p arcweft-compiler project::`: 7 passed, including SEL-005 and
  compile-artifact identity;
- `cargo test -p arcweft-launch`: 19 passed;
- `cargo test -p arcweft-project -p arcweft-project-loader --all-targets`:
  164 passed;
- `cargo test -p arcweft-lsp --all-targets`: 133 passed;
- focused CLI AOT profile, CLI/test/bench profile, profiled native Agent
  observe, canonical modern-feedback project compile, and outside-source-root
  profile tests passed;
- `callable::resolver_tests::extern_rust_alias_resolves_exact_typed_environment_record`
  and `callable::resolver_tests::selected_resolver_returns_adapter_method_candidate`:
  passed;
- `tests::typecheck::reduction_unchanged_is_a_typed_shared_borrow_constructor`:
  passed, including rejection of an owned state argument;
- `callable::tests::builtin_extended_identity_and_schema_table_is_typed` and
  `callable::tests::family_schemas_preserve_validator_result_effect_and_structural_owner`:
  passed;
- `entry::tests::same_named_cross_module_calls_keep_distinct_canonical_effect_identities`
  and
  `effect_model::tests::project_function_identity_is_prefix_free_across_package_and_module_boundaries`:
  passed;
- `cargo check --workspace --all-targets`: passed;
- `cargo test -p arcweft-adapter-context --all-features
  rust_callable_publication_is_a_typed_delta_for_augmented_standard_manifest`:
  1 passed;
- exact CLI rows
  `profile_check_loads_project_adapter_manifest`,
  `bench_json_measures_profile_inference_matmul_bias_adapter_fixture`, and
  `profile_check_loads_rust_metadata_for_extern_module`: 3 passed;
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run`: 5 passed;
- `just test-workspace`: passed with exit code 0 after the explicit-entry
  fixture migration;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed with `CARGO_INCREMENTAL=0` after extracting the standard-float family
  schema from the 101-line builtin dispatcher;
- `cargo fmt --all -- --check`: passed;
- `jj diff --git --color never | git apply --check --reverse
  --whitespace=error-all -`: passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: zero errors,
  127 warnings after rebasing onto the native physical-box geometry cut.

Earlier focused runs in the same cut also passed launch manifest selection and
forbidden-role-key coverage, CLI direct-source selection, the complete syntax
entry-role suite, exact custom-adapter and inference-adapter profile rows, Rust
metadata callable resolution, and the typed adapter publication delta. The
final workspace check and normal workspace test entrypoint are recorded before
handoff.

The first full workspace-test attempts exposed stale implicit-entry fixtures in
compiler, sema, LSP, project-loader, CLI, bundle, sample, and web paths. Those
were migrated directly rather than accepted through a fallback. One attempt
also exhausted the previous shared target volume while writing MSVC PDB files;
the verified worktree target had reached 201.87 GiB and left D: with 0.00 GiB
free. `cargo clean` removed 106,281 files / 201.9 GiB, restoring 192.61 GiB at
the immediate post-clean measurement. Subsequent Clippy used
`CARGO_INCREMENTAL=0`. The hot-swap session regression was also corrected to
assert the canonical selected source display name rather than the superseded
implicit source spelling.
