# Lang-01.1.1 ordinary-function role final authority closure

Date: 2026-08-09

Inspected Git commit: `6877c45ce0cf6fa7e620260776face27538df82f`

Working-tree state at inspection: dirty on `main` with the four Rust paths in
this coherent LSP admission cut; no other checkout or subagent is involved.

Supersedes the remaining semantic/tooling rows in:

- [the direct-style suspension and generator record](2026-07-22-lang-01-1-1-direct-style-suspension-generator.md); and
- [the AWBC direct-suspension kernel record](2026-07-24-lang-01-1-1-awbc-direct-suspension-kernel.md).

## Performed

The final source and semantic authority is one ordinary `fn` declaration.
There is no source-visible task, dialogue, or Stream function role and no
syntax/HIR `FunctionKind` compatibility carrier.

`FinalSemanticAnalysis` classifies each ordinary function as
`CheckedFunctionExecution::DirectFrame` or
`CheckedFunctionExecution::StreamFactory`. Own-scope `await`/`yield` evidence
also produces `CheckedSuspensionRole`; nested closures, `Seq` bodies, and
other execution owners do not leak their `yield` into the enclosing function.
The same execution fact is retained by `CheckedCallableFacts` under its exact
`CheckedCallableId`.

The older status rows asked project/LSP to publish callable execution facts.
The returned Lang-01.1.1.3.1 contract explicitly supersedes a separate
execution vector or metadata copy: `TypeCheckReport` and
`ProjectSemanticIndex` retain the same `Arc<CheckedCallableCatalog>`, and LSP
queries reach the checked fact through typed source/call identity.

This cut closes the last admission gap at that boundary. LSP accepted-project
publication now rejects:

- a final semantic report and project index that retain different checked
  catalog allocations; and
- a checked catalog whose accepted registered-catalog allocation, project
  world, symbol revision, or catalog digest does not match the compiled
  semantic world.

The validation is owned by `CheckedCallableCatalog`; LSP adds no copied
catalog, execution table, name lookup, HIR reconstruction, or source fallback.

## Current completion classification

| Boundary | State | Evidence or owner |
|---|---|---|
| ordinary `fn` parser/HIR public switch | `LANDED_VALIDATED` | old authored roles and `FunctionKind` are absent from language layers |
| direct/generator semantic classification | `LANDED_VALIDATED` | exact direct, own-scope generator, and independent-owner tests |
| direct suspension effect and diagnostics | `LANDED_VALIDATED` | final semantic facts plus maintained negative matrix |
| direct typed `Need<T, E>` runtime behavior | `LANDED_VALIDATED` | [direct Need authority switch](2026-08-06-lang-01-1-1-direct-need-authority-switch.md) |
| checked effect traits/catalog | `LANDED_VALIDATED` | one checked catalog authority from the returned Lang-01.1.1.3/.3.1/.3.1.1 contracts |
| project/LSP callable execution authority | `LANDED_VALIDATED` | shared catalog Arc and fail-closed LSP admission from this cut |
| detached external-capability member HIR | `LANDED_VALIDATED` | completed Proof attached syntax/HIR public switch |
| authored ordinary-function AWBC kind/public lowering | `EXCLUDED_EXTERNAL_DESIGN` | one atomic ABI/codec owner must be selected with the final Stream runtime cut; no `Synthetic` surrogate is allowed |
| `StreamFactory` runtime/wire/save projection | `EXCLUDED_EXTERNAL_DESIGN` | explicitly outside the current task until the selected Lang-01.3 correction cohort is admitted |

## Request dispatch guidance

Do not dispatch Lang-01.1.1.2 project nominal resolution again. Its package and
the .2.1/.2.2 corrections are already retained, verified, and implemented.
Likewise, the repository currently retains verified return packages for
Lang-01.3.1.2.1, Lang-01.3.1.2.2, and Lang-01.3.1.2.2.1; they are excluded by
the current task order, not absent from the package ledger.

Before any ordinary-function AWBC/Stream runtime implementation is resumed,
adjudicate those retained packages against current `main` and the user's
explicitly excluded correction list. If a newer correction is genuinely
missing, send the existing narrow request with all parent archives named in
its dispatch contract to one design-only assignee. Require one returned ZIP
with `OPEN_QUESTIONS=0`, exact ABI/codec/save allocations, one owner for each
runtime identity/state, complete consumer and deletion matrices, and no code
overlay. Do not split opcode, RuntimePlan, host, bundle, and save decisions
among independent assignees because they form one atomic serialized boundary.

The current cut does not create a replacement request from implementation
judgment and does not infer any excluded wire shape.

## Passed validation

- `cargo test -p arcweft-lang-sema --lib
  final_analysis::tests::ordinary_function_roles_walk_nested_final_hir_and_publish_suspend_effects
  --all-features`: 1 passed;
- `cargo test -p arcweft-lang-sema --lib
  final_analysis::tests::nested_yield_classifies_stream_factory_but_independent_owners_do_not_leak
  --all-features`: 1 passed;
- `cargo test -p arcweft-lang-sema --lib
  final_analysis::tests::multi_module_report_is_complete_generation_bound_and_exactly_accounted
  --all-features`: 1 passed; and
- `cargo test -p arcweft-lsp --lib
  profiles::accepted_project::tests::compiled_semantic_authority_requires_one_checked_catalog_generation
  --all-features`: 1 passed;
- `cargo test -p arcweft-lang-sema --lib --all-features`: 163 passed;
- `cargo test -p arcweft-lsp --lib profiles::accepted_project::tests::
  --all-features`: 10 passed, including the production 65,536/65,537 accepted
  project boundary pair;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed; and
- `just structure-audit` and `just structure-audit-gate`: passed at 2,113
  files, 1,990 Rust files, 979,597 Rust LOC, 94 workspace packages, 181 review
  triggers, and zero blocking findings.

The touched review-trigger files were inspected directly. `checked_catalog.rs`
remains the single cohesive checked-callable authority at 1,727 lines with no
net line growth. `accepted_project.rs` remains the single accepted-generation
admission boundary at 1,261 lines, 57 lines above the inspected base and below
the 300-line growth threshold.

## Workspace test result

After the requested `cargo clean` removed 230.4 GiB of regenerable build
artifacts from the sole Git worktree, the final ordinary-function focused
suites passed with the normal shared target and eight build jobs.

`just test-workspace` is not green. It passed the stale compiler diagnostic
stage assertion after that test was aligned with the recovered-source
Readiness contract and current `view::export_part_missing_as` typed code, then
stopped in the independent `arcweft-compiler --test view_product` target: one
test passed and six failed. Those failures expose the not-yet-integrated final
View catalog boundary (`Image` has no registered callable, accepted View items
have no checked product projection, and several tests retain pre-switch
stage/code/cardinality expectations). This cut does not add a temporary
builtin, restore the old View lowerer, or weaken those failures.

No Tier 2 runtime/render/Agent/MCP/capture target is selected for this isolated
semantic-to-LSP admission check; it changes no runtime or wire behavior.

## Explicit non-goals

- no ordinary-function AWBC kind, opcode, codec, save, or Stream runtime
  projection;
- no authored role attribute or removed-syntax-specific diagnostic;
- no project/LSP metadata copy, name lookup, raw-HIR fallback, or source
  reconstruction; and
- no compatibility alias, dual reader, shim, CSS path, or Takumi path.
