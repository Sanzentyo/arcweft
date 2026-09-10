# Conditional recovery ownership review

Date: 2026-09-10. Inspected Git base:
`a44835906f70d0b12343448e24192e5556cc22d6`, existing `main`, dirty with this
implementation cut. Canonical screening and `--fail-on-blocking` passed:
95 workspace packages, 2,258 Rust files, 310 review triggers, zero blocking
violations. The [generated reports](structure-audits/2026-09-10-conditional-recovery/README.md)
retain exact file classes, bytes, embedded test LOC and Cargo dependency edges.
The final reports were regenerated at
`11bbdff0471881480e731abcf15605234ac3979e` after the independent hover-fixture
commit, with this implementation still dirty; base LOC below remains the
initial inspected base.

No Cargo dependency, feature, package, unsafe boundary or I/O owner changed.
Source parsing remains in syntax; source admission and structural evaluation
remain in HIR; semantic selection remains in sema; runtime projection and
verification consume that selection. The public changes replace raw-HIR
executable admission and expose the typed provenance/capture/source components
needed by existing downstream owners. No facade was widened to support a file
split. Contract versions remain `1`.

## Dispositions

Each group below is a cohesion decision over the named owners' state,
dependencies, API and tests. It is not a numeric exemption. The measured table
maps every touched trigger to one group, including upper-size and embedded-test
triggers. Unchanged triggers outside the cut were screened but not represented
as newly reviewed implementation work.

- **Syntax.** `attachment/expression.rs` owns attached expression relations;
  `expressions.rs` owns their closed payload algebra; `grammar/build.rs` and
  `parser/cursor.rs` own source publication and parser transactions. Recovery
  composition, retained diagnostic work and candidate component attachment stay
  at these existing boundaries. Rebasing is in the new `dialogue/rebase.rs`
  module; diagnostic adversarial tests live under their event owner. No HIR
  selection or runtime policy enters syntax. Existing embedded cursor/build
  tests still test their transaction owner.
- **HIR lowering.** `final_lowering.rs` owns the module transaction and
  `expression_lowering.rs` owns expression allocation/dispatch. Complete capture
  uses replace first-use-only capture evidence in the existing capture lowerer;
  the candidate producers feed the same ledger. Source component completion and
  capture validation live under source freeze, not in a second lowerer. These
  dispatchers retain the existing arena transaction rather than acquiring a new
  unrelated state cluster. Their detailed tests remain in lowering test modules.
- **HIR admission.** `module.rs` owns atomic publication; `source_index.rs`,
  `expression_manifest.rs`, `candidate_projection.rs` and `pattern_projection.rs`
  own exact source-role applicability and payload validation. The new private
  candidate component collector is consumed into the existing index after
  validation. The new provenance builder is the sole final membership authority.
  Type/Pattern source requirements are shared with ordinary attachment. No
  independent source cache, range-derived membership, string reparse or parallel
  resolver survives. Keeping the exhaustive validator together preserves the
  single failure/publication boundary.
- **HIR evaluation.** `semantic_paths.rs` owns the closed structural path,
  capture and control relations; `selected_expressions.rs` owns the selected
  traversal; `runtime_semantic_owners.rs` owns runtime reachability. Provenance
  and capture-use slices are shared, not copied into a competing catalog. The
  capture projection has its own small module and is used by both semantic and
  runtime consumers. Control failures retain their owning typed row. The large
  path walker remains one exhaustive structural algebra; no additional parser
  or semantic resolver was introduced there.
- **Sema preparation.** `checked_catalog.rs` owns callable construction/sealing;
  analyzer `items.rs` and `expressions.rs` own signature/body and expression
  preparation. The new `function_body.rs` closes result/execution constraints
  using the existing transaction in `state.rs`. `calls.rs`, `calls/constraints.rs`
  and `callable/resolver.rs` retain their existing affine call resolver and
  invariants. They were migrated to analysis admission, not given another
  inference model. Capture choice receipts remain in their capture owner.
  Keeping preparation and final execution assignment separate follows their
  evidence phases, without provisional runtime roles or rollback side tables.
- **Sema publication.** `model.rs`, `report.rs`, `validation.rs`, `match_edges.rs`,
  `nominal_schema.rs`, `semantic_transcript.rs` and `semantic_coordinate/catalog.rs`
  retain checked payloads, final atomic publication and accepted coordinates.
  Selection filters the all-arena inventory before final facts escape. The
  duplicate checked-capture carrier was removed. Nominal, transcript and
  coordinate modules retain their existing responsibility; their broad size is
  not used to justify a new shared universal model. `entry/checker.rs`,
  `checked_text_proxy/prepared.rs` and `analyzer/dialogue_line_plan.rs` remain
  consumers of those accepted owners, with no new source/type authority.
- **Compiler/tooling.** Compiler `lower.rs` remains the sema-to-runtime adapter;
  `project.rs` owns compilation/cache transactions and `view.rs` owns View
  lowering. Selected capture and structural postfix facts come from the existing
  semantic report. Their large conversion matrices preserve the single target
  schema and do not re-resolve source names. CLI `app/project.rs` and LSP
  `profiles/accepted_project.rs` only migrate the HIR admission lease. The hover
  fixture correction changes a source lookup in a test, not product behavior.
- **Runtime/verify.** `semantic_facts.rs` owns runtime catalog admission and
  completeness; `semantic_facts/project_function.rs` owns closed instance
  catalogs. Captured source-local lookup is resolved inside the same catalog,
  with declared/captured locals kept disjoint. `final_flow.rs` owns statement
  sequencing and continuations, including blocks nested in values; it does not
  acquire transport or storage state. Its embedded tests remain flow-lowering
  tests. Verifier `lib.rs` retains obligation construction and uses selected
  statement evidence; `insertion.rs` remains its insertion consumer. These
  owners add no I/O, fallback global lookup or independent selection authority.
- **Tests.** HIR control/project tests retain allocation, scope, capture and
  selected-inventory fixtures. Syntax incremental tests retain source-lease and
  fragment transactions. Sema's large root test module remains the existing
  fixture/semantic matrix; the migration is an admission API update, with new
  capture reconciliation tests in the owning edge module. Runtime semantic-fact
  tests retain malformed/missing/extra inventory rejection. No production API
  was widened merely to move test helpers. The compiler execution test remains
  below its integration-test size trigger; its growth adds actual native/AWBC
  acceptance rather than a source-spelling gate.

Normal workspace fan-in/out is syntax 12/2, HIR 10/3, sema 8/14, compiler 3/23,
runtime-plan 5/9, LSP 0/23 and verify 6/4. Development fan-in/out is respectively
1/1, 1/0, 3/0, 1/5, 5/1, 1/3 and 0/0. These edges are unchanged.

## Measured touched triggers

The following rows come from the canonical dirty-checkout report. Base LOC is
the complete file at the inspected Git base; final LOC is the complete current
file. Generated report files are audit evidence, not production source.

| Path | Class | Base → final LOC | Bytes | Embedded test LOC |
| --- | --- | --- | --- | --- |
| crates/arcweft-cli/src/app/project.rs | production | 1486 → 1486 | 53539 | 55 |
| crates/arcweft-compiler/src/lower.rs | production | 7855 → 7861 | 332462 | 0 |
| crates/arcweft-compiler/src/project.rs | production | 1708 → 1708 | 60724 | 0 |
| crates/arcweft-compiler/src/view.rs | production | 1421 → 1415 | 57276 | 0 |
| crates/arcweft-lang-hir/src/final_lowering/expression_lowering/tests/control.rs | test | 2826 → 2885 | 107877 | 0 |
| crates/arcweft-lang-hir/src/final_lowering/expression_lowering.rs | production | 2403 → 2409 | 104188 | 0 |
| crates/arcweft-lang-hir/src/final_lowering.rs | production | 1472 → 1472 | 57346 | 0 |
| crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs | production | 1398 → 1460 | 53213 | 0 |
| crates/arcweft-lang-hir/src/final_project/selected_expressions.rs | production | 1159 → 1257 | 48203 | 0 |
| crates/arcweft-lang-hir/src/final_project/semantic_paths.rs | production | 6396 → 6468 | 240979 | 0 |
| crates/arcweft-lang-hir/src/final_project/tests.rs | test | 4837 → 4837 | 172735 | 0 |
| crates/arcweft-lang-hir/src/module.rs | production | 2063 → 2103 | 84780 | 0 |
| crates/arcweft-lang-hir/src/source_index/expression_manifest/candidate_projection.rs | production | 1413 → 1564 | 63605 | 0 |
| crates/arcweft-lang-hir/src/source_index/expression_manifest.rs | production | 1416 → 1440 | 62482 | 0 |
| crates/arcweft-lang-hir/src/source_index/pattern_projection.rs | production | 1352 → 1352 | 54205 | 0 |
| crates/arcweft-lang-hir/src/source_index.rs | production | 1733 → 1748 | 57553 | 0 |
| crates/arcweft-lang-sema/src/callable/checked_catalog.rs | production | 2657 → 2685 | 98872 | 0 |
| crates/arcweft-lang-sema/src/callable/resolver.rs | production | 1579 → 1580 | 58619 | 0 |
| crates/arcweft-lang-sema/src/checked_text_proxy/prepared.rs | production | 1678 → 1678 | 62492 | 0 |
| crates/arcweft-lang-sema/src/entry/checker.rs | production | 2024 → 2024 | 80346 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | production | 4940 → 4940 | 201826 | 480 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/calls.rs | production | 4304 → 4304 | 186446 | 185 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/dialogue_line_plan.rs | production | 1669 → 1672 | 72857 | 66 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs | production | 4037 → 4084 | 177944 | 88 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/items.rs | production | 1448 → 1471 | 61733 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/analyzer/state.rs | production | 3080 → 3080 | 116762 | 503 |
| crates/arcweft-lang-sema/src/final_analysis/match_edges.rs | production | 1467 → 1476 | 60537 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/model.rs | production | 2807 → 2807 | 91598 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs | production | 2946 → 2946 | 120498 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/report.rs | production | 2234 → 2264 | 90840 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/semantic_transcript.rs | production | 4101 → 4101 | 164200 | 98 |
| crates/arcweft-lang-sema/src/final_analysis/tests.rs | test | 9046 → 9046 | 311935 | 0 |
| crates/arcweft-lang-sema/src/final_analysis/validation.rs | production | 2755 → 2779 | 114803 | 0 |
| crates/arcweft-lang-sema/src/semantic_coordinate/catalog.rs | production | 1206 → 1209 | 50082 | 341 |
| crates/arcweft-lang-syntax/src/attachment/expression.rs | production | 2134 → 2226 | 80997 | 0 |
| crates/arcweft-lang-syntax/src/expressions.rs | production | 1230 → 1314 | 42714 | 0 |
| crates/arcweft-lang-syntax/src/grammar/build.rs | production | 1265 → 1313 | 51558 | 297 |
| crates/arcweft-lang-syntax/src/incremental/database_tests.rs | test | 3794 → 3886 | 135005 | 0 |
| crates/arcweft-lang-syntax/src/parser/cursor.rs | production | 1213 → 1203 | 41456 | 153 |
| crates/arcweft-lsp/src/profiles/accepted_project.rs | production | 1270 → 1270 | 43465 | 0 |
| crates/arcweft-runtime-plan/src/final_flow.rs | production | 6896 → 6898 | 282470 | 374 |
| crates/arcweft-runtime-plan/src/semantic_facts/project_function.rs | production | 2364 → 2378 | 87391 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts/tests.rs | test | 2853 → 2863 | 103711 | 0 |
| crates/arcweft-runtime-plan/src/semantic_facts.rs | production | 10402 → 10456 | 397963 | 0 |
| crates/arcweft-verify/src/lib.rs | facade | 1229 → 1238 | 42802 | 0 |
