# Lang 01.5.1 authored external-module deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_WORKSPACE_FIXTURE_FAILURE`

This is a deletion-only prerequisite to the Proof attached-syntax authority
switch. It removes the obsolete source producer for external Rust modules after
the single-manifest topology became the accepted owner. It does not claim
completion of Proof Stage 3 or all Lang 01.5.1 consumers.

## Contract and ZIP intake

The Lang 01.5.1 package identity recorded by the accepted decoder cut is
`lang-01.5.1-single-manifest-decoder-implementation-ready-final-contract-9a63ac55.zip`,
SHA-256
`1A3432EB09994AC4E75209CAE2392ED62DEA2F89B26077B244A57440CD01E647`.
Its final schema owns external-module import identity, mount, artifact path and
raw digest, expected package/version/module/family/ABI identity, visibility,
demand, and per-profile selection.

The retained Proof base package was reverified before this cut at
[`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip),
SHA-256
`1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`.
All 19 non-self payload digests match and the manifest self row uses the
specified zero placeholder. The package remains `READY_FOR_IMPLEMENTATION`
with no open question. Its obsolete-reader deletion rule agrees with this cut.

## Deleted authority

The cut deletes the source-authored external-module path rather than repairing
or translating it:

- `Item::ExternMod` and the complete `ExternModItem`/source/member/type/function/
  activity AST family;
- `CstTopLevelItemKind::ExternMod`, its classifier, parser head/member readers,
  and top-level dispatch;
- `HirTopLevelDecl::ExternMod`, lowering, cache facts, symbol publication, and
  project-index/signature readers;
- the semantic checker for authored Rust packages, exports, signatures, and
  activities;
- the source declaration to generated-environment callable alias bridge,
  including the dead catalog lookup and signature-comparison helpers;
- `MissingRustPackageMetadata`, `MissingRustExport`, and
  `RustExportSignatureMismatch` diagnostics and codes; and
- tooling/LSP matches and positive tests whose only subject was the removed
  declaration or its source alias.

The removal intentionally starts at the producer. Resulting compile errors are
the migration inventory and are fixed only by deleting obsolete readers or by
using the already accepted manifest/generated-metadata owner. No renamed
string helper, compatibility node, dual resolver, deprecated alias, or source
shim is introduced.

## Retained final owner

The cut retains the typed generated-metadata path:

- schema-1 `[external-modules.<id>]` and profile `external-modules` selection;
- exact `ExternalModuleImportSpec` and resolved project topology;
- strict generated metadata decode, source map, raw digest and semantic
  identity validation;
- generated nominal, callable, Activity, visibility, and provenance facts;
- `AdapterManifest` semantic projection and environment callable catalog; and
- Rust package/item provenance used by bundle, sema, LSP, and runtime planning.

Direct source parsing never invents these bindings. A profile must select an
accepted metadata import, and the metadata itself is the signature authority.

## Observable removal evidence

The former authored spelling reaches ordinary current-grammar recovery. It has
no dedicated recognizer or diagnostic, produces no executable typed
external-module item, and preserves following declarations. The public API
compile-fail matrix also proves that `Item::ExternMod` is absent. These are
behavioral and type-system checks, not source-text gates.

## Explicit non-goals

This cut does not:

- publish attached `ParsedSource`, qualified public `SyntaxNodeId`, or arena
  HIR/project authority;
- delete detached `TypedSyntaxTree` or all raw parse/reparse callers;
- implement generated-artifact runtime binding beyond the already accepted
  topology;
- delete source `content`, which still waits for typed content-root admission;
  or
- change Source/Stream, configured resource, Dialogue application, RichText,
  assertion codec, bundle, save, or replay contracts.

`AGENTS.md` already requires obsolete producer deletion, compile-error-driven
caller migration, and removal of compatibility/source-gate machinery. No
repository policy edit is needed for this cut.

## Validation

Focused suites passed:

- `cargo test -p arcweft-lang-syntax -p arcweft-lang-hir
  -p arcweft-lang-sema --all-features`, including 470 syntax unit tests, 90 HIR
  unit tests, 1,117 sema unit tests, all integration tests, doc tests, and the
  removed-API compile-fail matrices;
- `cargo test -p arcweft-project-loader -p arcweft-lsp
  -p arcweft-tooling --all-features`, including 136 project-loader, 225 LSP,
  and 61 tooling unit tests plus their integration/doc/compile-fail tests; and
- generated metadata hash/decode/expectation/fact publication, LSP metadata
  watch refresh, and exact profile-relative diagnostic tests all passed without
  a source-authored alias.

Broad gates:

- `cargo fmt --all -- --check`: pass;
- `cargo check --workspace --all-targets --all-features`: pass with
  `CARGO_BUILD_JOBS=2`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  pass with `CARGO_BUILD_JOBS=2`;
- the non-CLI `test-workspace` recipe
  (`cargo test --workspace --lib --tests --exclude arcweft-cli`): pass;
- the CLI lib/bin recipe: 197 passed;
- `runtime_native_options`, `check_core_cli`, `native_style_parity_sample`,
  `release_trust_json`, `responsive_stage_placement`, and
  `seq04_8_4_persistent_cache_build_cli_goldens`: pass;
- `just test-tier2`: pass, including 22 MCP/native stdio tests, capture tests,
  visual smoke, and four exact IMQ golden rows;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 3,666 files,
  1,937 Rust files, 906,854 Rust physical LOC, 94 manifests, 0 errors and 146
  existing warnings; and
- `git diff --check`: pass.

The first quiet `just test-workspace` wrapper exceeded its 15-minute command
wrapper while the first Cargo recipe was still linking. Its surviving child
process was identified and stopped without deleting source or target data; the
same recipe was then run directly with a 30-minute bound and passed in 7m24s.
Every remaining recipe was run directly so each exit status was retained.

The inherited `arcw_fixtures_check_run` gate remains exactly two failures:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Directly running both fixtures reports
`error[sema.nominal.unknown_type]: unknown type FsError`. Current-pass check/run
fixtures and the spec-should-fail corpus pass. Neither failing source contains
an external-module declaration or enters the deleted parser, HIR, alias,
generated-metadata, LSP, or tooling path. The failure was already recorded by
the parent authored-Asset deletion cut and is preserved rather than hidden or
worked around by restoring an obsolete reader.

## Structural measurement

The measurement is from Jujutsu change `orwxrsnztzvs` on parent Git
`5b0721a51558`. No Cargo manifest, feature, dependency edge, crate boundary, or
fan-in/fan-out direction changed. Every changed production file shrank; the two
new Rust files are one compile-fail case and its harness row.

| Path | Kind | Bytes | LOC | Embedded test LOC |
| --- | --- | ---: | ---: | ---: |
| `crates/arcweft-lang-hir/src/cache_facts.rs` | production | 1,825 | 50 | 0 |
| `crates/arcweft-lang-hir/src/lower.rs` | production + unit tests | 21,236 | 616 | 308 |
| `crates/arcweft-lang-hir/src/model.rs` | production | 30,220 | 1,128 | 0 |
| `crates/arcweft-lang-hir/src/symbol/table/publication.rs` | production | 14,241 | 352 | 0 |
| `crates/arcweft-lang-sema/src/callable/builder.rs` | production | 41,119 | 1,052 | 0 |
| `crates/arcweft-lang-sema/src/callable/catalog.rs` | production + unit tests | 31,915 | 877 | 275 |
| `crates/arcweft-lang-sema/src/callable/nominal_signature.rs` | production | 8,528 | 235 | 0 |
| `crates/arcweft-lang-sema/src/callable/resolver_tests.rs` | unit tests | 125,064 | 3,563 | 0 |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | production | 33,497 | 1,040 | 0 |
| `crates/arcweft-lang-sema/src/checker.rs` | production + unit tests | 85,978 | 2,347 | 2,263 |
| `crates/arcweft-lang-sema/src/checker/module.rs` | production + unit tests | 84,815 | 2,134 | 2,125 |
| `crates/arcweft-lang-sema/src/diagnostics/error.rs` | production | 29,804 | 779 | 0 |
| `crates/arcweft-lang-sema/src/project_index.rs` | production + unit tests | 37,155 | 1,277 | 1,223 |
| `crates/arcweft-lang-sema/src/signature/surface.rs` | production | 36,740 | 1,007 | 0 |
| `crates/arcweft-lang-sema/src/symbols.rs` | production | 36,415 | 1,067 | 0 |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | unit tests | 41,303 | 1,345 | 0 |
| `crates/arcweft-lang-sema/src/tests/support.rs` | unit-test support | 7,673 | 223 | 0 |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | production | 42,295 | 1,651 | 0 |
| `crates/arcweft-lang-syntax/src/cst.rs` | production + unit tests | 12,691 | 436 | 18 |
| `crates/arcweft-lang-syntax/src/cst/classify.rs` | production | 14,740 | 443 | 0 |
| `crates/arcweft-lang-syntax/src/parser/headers.rs` | production | 32,714 | 978 | 0 |
| `crates/arcweft-lang-syntax/src/parser/item_tests.rs` | unit tests | 4,791 | 149 | 0 |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | production | 74,934 | 2,021 | 0 |
| `crates/arcweft-lang-syntax/src/parser/retained_grammar_tests.rs` | unit tests | 10,722 | 300 | 0 |
| `crates/arcweft-lang-syntax/src/parser/top_level.rs` | production | 11,186 | 275 | 0 |
| `crates/arcweft-lang-syntax/tests/parser_function_signatures_and_types.rs` | integration test | 19,061 | 535 | 0 |
| `crates/arcweft-lang-syntax/tests/public_api.rs` | integration-test harness | 593 | 12 | 0 |
| `crates/arcweft-lang-syntax/tests/ui/removed_extern_mod_item.rs` | compile-fail test | 87 | 5 | 0 |
| `crates/arcweft-lsp/src/features/actions.rs` | production + unit tests | 42,228 | 1,308 | 176 |
| `crates/arcweft-tooling/src/dialogue_content.rs` | production | 17,571 | 497 | 0 |

Largest workspace Rust files at the same checkout:

| Path | Bytes | LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357,456 | 12,399 | generated Unicode 17.0 lookup table |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 228,252 | 7,021 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 241,029 | 6,712 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 215,661 | 5,901 | integration test |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 196,292 | 5,257 | integration test |
| `crates/arcweft-lang-sema/src/tests/typecheck.rs` | 137,992 | 4,471 | unit-test responsibility module |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 144,581 | 4,218 | integration test |
| `crates/arcweft-compiler/src/tests.rs` | 131,196 | 3,621 | unit-test responsibility module |
| `crates/arcweft-lang-sema/src/callable/resolver_tests.rs` | 125,064 | 3,563 | changed unit-test responsibility module |

The error-level generated and integration-test exceptions predate this cut and
did not grow. Warning-level production files either shrank or lost a match arm;
none acquired a new responsibility. The canonical audit reports no
error-level decomposition or dependency violation.
