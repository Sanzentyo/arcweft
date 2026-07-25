# Proof concurrency v6.1.1.2 authored Asset declaration deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_WORKSPACE_FIXTURE_FAILURE`

This is a deletion-only prerequisite to the retained global-identity public
switch. It removes the obsolete source producer for Asset declarations without
claiming completion of the project asset catalog cohort.

## Contract and intake

The governing correction package is
[`arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip),
SHA-256
`0E30A91FA2F7A288E9A12D8AFC7356525604CBDC907D659CD97311207D26A68E`.
Its retained-declaration inventory makes Asset catalog-only: source may refer to
`asset.*`, but it cannot author an Asset declaration AST or HIR item.

The implementation started from Git `67b918736d8f` and Jujutsu change
`snzzpzox`. The package was already verified as `READY_FOR_IMPLEMENTATION`,
with every non-self manifest payload matching its recorded digest and no open
questions.

## Deleted authority

- `EntityDeclKind::Asset` and its public keyword projection;
- old CST classification of `asset` as a generic entity declaration;
- old parser header and declaration-family recognition;
- checker, resolver, and project-index mappings that registered an authored
  Asset declaration as `EntityKind::Asset`;
- source Asset declaration blocks in production-facing tests, fixtures,
  samples, and the web project; and
- lint and design documentation that advertised either compact or fully
  qualified Asset declaration headers.

Removing the CST classifier is part of the authority deletion. Leaving it in
place would consume an Asset-shaped block as an entity declaration and then
silently return no AST item when the deleted header parser rejected it. The
current parser instead uses its ordinary unrecognized-top-level recovery,
publishes a non-executable `Item::Raw`, and preserves the following sibling.
No Asset-specific removed-syntax diagnostic was added.

## Retained catalog and reference boundary

The cut intentionally retains:

- `RetainedIdentityFamily::Asset`, `AssetVirtualPath`, and `AssetId`;
- `EntityKind::Asset` and typed `@asset...` reference expressions;
- presentation callable schemas and checks that require Asset references;
- bundle asset-catalog/image-asset records and runtime/player lookup;
- compiler and CLI validation of catalog-derived Asset IDs; and
- typed resource Asset-reference values.

Tests that previously authored a source Asset shell now either rely on the
existing catalog-backed reference family or inject an external Asset symbol
into `NameRegistry` when the test directly exercises reference validation.
They do not synthesize another source declaration or compatibility carrier.

## Explicit non-completion boundary

This cut does **not** complete the final project asset catalog. In particular,
it does not yet add `ProjectAssetSymbol`, inclusion provenance, LSP/Agent
definition lookup, or structured absent/excluded-asset diagnostics. The
current manifest-backed-family resolution behavior is existing substrate, not
evidence that the package's later catalog rows such as ASSET-011/ASSET-012 are
complete. Those rows remain part of the later project/public authority switch.

## Validation

Focused suites passed:

- `cargo test -p arcweft-lang-syntax --all-features`;
- `cargo test -p arcweft-lang-sema --all-features`;
- `cargo test -p arcweft-runtime-plan --all-features`;
- the `arcweft-compiler` project-cache transaction integration test;
- the exact `arcweft-player-web` released/scoped image-handle parity test;
- `cargo test -p arcweft-cli --test responsive_stage_placement --all-features`;
- the exact CLI modern-feedback wrapped-subtitle unit test; and
- the exact CLI image-animation bundle/catalog/run test. The test now supplies
  its already required exact `entry.image_static_png` selection instead of
  relying on ambiguous multi-entry execution.

Broad gates:

- `cargo fmt --all -- --check`: pass;
- `cargo check --workspace --all-targets --all-features`: pass;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: pass;
- `just test-tier2` with `CARGO_BUILD_JOBS=2`: pass, including 22 MCP/native
  cases, image/capture cases, and exact visual goldens;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 3,664 files,
  1,936 Rust files, 907,808 Rust physical LOC, 0 errors and 146 warnings; and
- `git diff --check`: pass.

The first `just test-workspace` attempt exhausted the Windows paging file while
mapping an `arcweft_bundle` rlib (`os error 1455`). Re-running the same recipe
with `CARGO_BUILD_JOBS=2` compiled and passed the workspace suites until the
pre-existing `arcw_fixtures_check_run` gate. Its current-fixture rows pass; its
two `spec_should_pass_*_after_refactor` rows still fail on:

```text
error[sema.nominal.unknown_type]: unknown type `FsError`
```

The exact inherited fixtures are
`spec_should_pass/check/010_capability_fs_read.arcw` and
`spec_should_pass/run/002_file_read_task.arcw`. No changed Asset source,
parser, catalog, bundle, runtime, or test path participates in that nominal
capability-type failure. The failure is recorded rather than hidden or repaired
by restoring the deleted Asset producer.

## Structural measurement

The measurement is from Jujutsu change `wslmrxqz` on parent Git
`b4d086e7bb60`. No Cargo manifest, feature, dependency edge, public crate
boundary, or fan-in/fan-out direction changed.

| Path | Owner/kind | Bytes | LOC | Embedded test LOC | Responsibility |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | `arcweft-cli` integration test | 228,252 | 7,021 | 0 | CLI runtime/bundle regression matrix; this cut adds exact entry selection only. |
| `crates/arcweft-compiler/tests/project_cache_transaction.rs` | `arcweft-compiler` integration test | 18,668 | 565 | 0 | project cache transaction behavior. |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | `arcweft-lang-sema` production | 32,820 | 937 | 0 | checker declaration/reference helper policy. |
| `crates/arcweft-lang-sema/src/project_index/entities.rs` | `arcweft-lang-sema` production | 37,115 | 1,048 | 0 | typed project entity indexing. |
| `crates/arcweft-lang-sema/src/project_index/tests.rs` | `arcweft-lang-sema` unit test | 26,848 | 808 | 0 | project-index behavior matrix. |
| `crates/arcweft-lang-sema/src/resolve.rs` | `arcweft-lang-sema` production | 11,180 | 326 | 0 | typed entity/reference resolution. |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | `arcweft-lang-sema` unit test | 42,911 | 1,392 | 0 | declaration lowering/typecheck matrix. |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | `arcweft-lang-sema` unit test | 137,992 | 4,471 | 0 | semantic typecheck/reference matrix. |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | `arcweft-lang-syntax` production | 46,260 | 1,832 | 0 | public typed source item/declaration AST. |
| `crates/arcweft-lang-syntax/src/cst/classify.rs` | `arcweft-lang-syntax` production | 14,938 | 449 | 0 | CST/current-grammar item classification. |
| `crates/arcweft-lang-syntax/src/lint.rs` | `arcweft-lang-syntax` production | 29,871 | 993 | 386 | syntax lint policy plus colocated focused tests. |
| `crates/arcweft-lang-syntax/src/parser/headers.rs` | `arcweft-lang-syntax` production | 33,620 | 998 | 0 | current declaration-header grammar. |
| `crates/arcweft-lang-syntax/tests/parser_declarations_recovery_comments.rs` | `arcweft-lang-syntax` integration test | 15,320 | 505 | 0 | generic recovery and sibling-preservation behavior. |
| `crates/arcweft-lang-syntax/tests/public_api.rs` | `arcweft-lang-syntax` integration test | 530 | 11 | 0 | compile-fail public API harness. |
| `crates/arcweft-lang-syntax/tests/ui/removed_asset_declaration_kind.rs` | `arcweft-lang-syntax` compile-fail test | 103 | 5 | 0 | absence of the obsolete public AST variant. |
| `crates/arcweft-player-web/tests/parity.rs` | `arcweft-player-web` integration test | 38,562 | 1,131 | 0 | web/runtime image-handle parity. |
| `crates/arcweft-runtime-plan/src/flow/tests.rs` | `arcweft-runtime-plan` unit test | 19,361 | 575 | 0 | flow/dialogue/presentation plan lowering. |

Largest workspace Rust files at the same checkout:

| Path | Bytes | LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated Unicode 17.0 lookup table, explicitly marked generated/do-not-edit. |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 228,252 | 7,021 | integration test, below the 8,000-LOC error threshold. |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 241,029 | 6,712 | integration test. |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | integration test. |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 215,661 | 5,901 | integration test. |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 196,292 | 5,257 | integration test. |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 137,992 | 4,471 | unit-test responsibility module. |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 144,581 | 4,218 | integration test. |
| `crates/arcweft-lang-sema/src/callable/resolver_tests.rs` | 128,792 | 3,663 | unit-test responsibility module. |
| `crates/arcweft-compiler/src/tests.rs` | 131,196 | 3,621 | unit-test responsibility module. |

`ast/items.rs` remains a warning-level cohesive typed-AST owner and shrank in
this cut; it did not acquire another responsibility. The warning-level large
integration tests likewise did not gain another test subsystem. The canonical
audit reports no error-level decomposition or dependency violation.
