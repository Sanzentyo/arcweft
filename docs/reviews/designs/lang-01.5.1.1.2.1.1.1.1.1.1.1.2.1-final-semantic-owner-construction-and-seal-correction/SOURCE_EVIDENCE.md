# Source evidence

## Repository state

- inspected date: 2026-08-23
- full Git commit: `300e824eea6740eab0ae708508cce00a1bd49435`
- branch: `main`
- initial state: clean and equal to `origin/main`
- design authoring changes: documentation only

## Phase and Entry evidence

- `crates/arcweft-compiler/src/project.rs` calls
  `analyze_final_project`, then `verify_project`, then
  `check_project_entries`.
- `crates/arcweft-lang-sema/src/final_analysis/analyzer.rs` currently finishes
  checked callables and calls the final-analysis constructor before Entry
  checking.
- `crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs`
  constructs Entry references from `PublicId + ItemId` by scanning the
  executable HIR.
- `crates/arcweft-lang-sema/src/entry.rs` shows that
  `CheckedEntryBindingDigest` exists only on checked Entry bindings/catalog.
- `crates/arcweft-lang-sema/src/entry/checker.rs` and
  `entry/checker/contract.rs` use generation validation, checked callables,
  `ty`, `item`, `calls`, and project nominal projection; they do not require a
  fully published analysis object.
- `crates/arcweft-lang-sema/src/final_analysis/report.rs` is the sole current
  complete semantic publication owner.

## Nominal and environment evidence

- `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs` owns
  `RuntimeProjectNominalProjection` and the only
  `TypeShape -> RuntimeTypeSchema` mapping, but its expander currently borrows
  `FinalSemanticAnalysis`.
- `crates/arcweft-lang-sema/src/env/base.rs` stores nominal record fields in
  nested `HashMap`s, losing declaration order after its ordered constructor
  input.
- project record-pattern seeding in
  `final_analysis/analyzer/patterns.rs` uses source declarations and names;
  environment record patterns are not admitted.
- `RuntimeRecordFieldId` is the core nonzero ordinal owner; project runtime
  projection supplies the canonical semantic type and layout.

## Select, View, Style, and RichText evidence

- `final_analysis/model.rs` retains name-only Method/Field, open StageLook,
  and dead Tuple/Record element variants.
- repository-wide typed search finds no final-sema producer for
  `TupleElement` or `RecordElement`.
- `final_analysis/analyzer/expressions.rs` accepts any ViewValue unresolved-dot
  name as `CheckedViewCall::Modifier`.
- `crates/arcweft-compiler/src/view.rs` rejects every Modifier during lowering.
- `crates/arcweft-view/src/style/value.rs` has 26 current
  `ViewSpecifiedValue` variants; final sema currently produces Color through
  the checked `rgba` constructor.
- `crates/arcweft-lang-sema/src/checked_rich_text/model.rs` already retains
  typed ordered tokens/actions plus lookup/source IDs.
- `crates/arcweft-compiler/src/lower.rs` and
  `lower/reachability.rs` consume the selected Postfix `ExprId`.

## Accepted predecessor

The accepted parent design under
`docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/`
defines the C2 rows and C3 transcript grammar. Its implemented C1 topology is
present at the inspected SHA and is consumed without alteration.
