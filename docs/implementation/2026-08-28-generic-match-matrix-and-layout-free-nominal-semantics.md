# Generic Match matrix and layout-free nominal semantics

Date: 2026-08-28

Inspected base Git commit:
`d78acbe834dbace0903c18d8b2d12e8d33176f6c`

Initial branch state: `main` equalled `origin/main`. The working tree already
contained protected, unrelated Agent, player, renderer, compiler, sample, and
implementation-note WIP. This cut did not create a branch, worktree, or
workspace and does not claim or discard that WIP.

## Result

Generic Match C4 now uses one private, bounded Maranget-style matrix authority
over exact checked semantic domains. The cut also closes the variant-payload
and runtime-record isomorphism required for that matrix to describe executable
values rather than a sema-only approximation.

- All eleven Match resources use checked `u64` accounting. Limits and observed
  work are enum-indexed fixed tables owned by `CheckedMatchLimitKind`; the old
  eleven-field model, eleven-argument constructor, duplicated kind switches,
  and lint exception were deleted.
- The coverage owner is split by actual responsibility while retaining one
  `MatchCoverageAnalyzer`: transaction, canonical literal grammar, model,
  domain and sequence planning, pattern deconstruction, matrix search, and arm
  orchestration. Production modules are 178--803 physical LOC; tests are in
  separate modules.
- Dynamic guards do not commit coverage, `Or` alternatives retain source order
  and exact coordinates, recursive witnesses are finite, cancellation and all
  limits are transactional, and open domains use a typed `Other(type_digest)`
  witness. There is no semantic-success `Any` fallback.
- Project nominal semantic fields and cases are declaration-ordered,
  layout-free owners. Runtime layout IDs and diagnostic spellings do not enter
  their semantic identity.
- `VariantPayloadShape` distinguishes Unit, empty Tuple, and empty Record.
  Project newtypes, environment/Rust tuple variants, builtin Option/Result,
  structural Record patterns, and record-rest bindings use one exact internal
  payload type.
- Runtime checked types now retain exact ordered Record fields. Record value
  admission and matching use runtime field identity plus recursive checked
  type; field diagnostic names remain non-semantic.
- Option/Result executable payloads are canonically one-field Tuples. Pure,
  flow, Try, Standard Map, AWBC, compiler projection, and Agent controller test
  consumers all use `Variant -> Tuple -> raw value`; flat legacy payloads are
  rejected.

## Design precedence

The accepted package's machine/source inventory was pinned to an older source
baseline and was not used to delete live authority. Current typed source and
producer evidence establish these live inventories:

- 38 `HirExprKind` variants;
- 28 `CheckedExpressionResolution` variants, including `Closure`;
- 5 `CheckedSelectResolution` variants;
- 6 `CheckedPatternResolution` variants, including live `Record` and
  `TypedBinding` and no producer for the package's stale `Nominal` row;
- 62 checked expression-child roles;
- 17 expression-owned body roles; and
- 26 `ViewSpecifiedValue` variants.

No stale `Nominal`, `TupleElement`, or `RecordElement` producer was fabricated,
and `Closure` was not silently dropped. C3 must transcribe the live inventory
compile-exhaustively.

## Deletion evidence

Repository searches returned no production occurrences of the deleted
`basic_coverage`, `CoverageAtom`, `CoverageShape`, `UnsupportedCoverage`,
`max_coverage_states`, `legacy_coverage`, `BudgetedCoverageHasher`,
`coverage_digest_update`, `CheckedVariantPayloadShape`, or BTreeMap-backed
record payload models. Search also found no `CheckedCoverageWitness::Any`,
`CoverageConstructorId::Any`, or `TypeKind::Any`. Remaining `Any` spellings in
the focused search are test-oracle wildcards or explicitly modeled domain tops
outside Match coverage.

## Performed and passed

- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- `cargo check -p arcweft-core --all-targets`: passed.
- `cargo check -p arcweft-lang-sema --all-targets`: passed.
- `cargo check -p arcweft-runtime-plan --all-targets`: passed.
- `cargo check -p arcweft-compiler --all-targets`: passed.
- `cargo check -p arcweft-agent-runner --all-targets`: passed.
- `cargo check --workspace --all-targets --all-features`: passed.
- `cargo test -p arcweft-lang-sema --all-targets`: 535 unit, 12 compile-API,
  and 4 integration tests passed.
- Focused internal coverage tests: 7 passed, including the exact/one-less
  boundary for all eleven limits and the finite oracle.
- Focused final Match tests: 11 passed, including Result, Choice, array,
  sequence, project/environment records, DropPolicy record payload/rest,
  cancellation, and unreachable `Or` coordinates.
- `cargo test -p arcweft-agent-runner --lib`: 62 passed after adding exact
  Result payload type rows and Tuple payload patterns.
- `cargo test -p arcweft-core --all-targets`: 313 core unit tests and all
  crate integration/compile tests passed; focused AWBC Record codec tests were
  3/3 and direct-suspension tests were 8/8.
- `cargo test -p arcweft-runtime-plan --all-targets`: 49 unit tests and all
  crate integration/compile tests passed; AWBC product parity was 5/5 and the
  iterator witness integration test passed.
- `cargo test -p arcweft-compiler --all-targets`: 55 unit tests and all crate
  integration/compile tests passed; project-cache tests were 18/18 and Try/Pipe
  tests were 8/8.
- `just structure-audit` and `just structure-audit-gate`: 2,261 files, 2,133
  Rust files, 1,141,941 Rust physical LOC, 95 packages, 274 review triggers,
  and 0 blocking violations. The gate passed.
- Strict Clippy filtered to every newly introduced C4 owner produced no
  diagnostic after decomposition and cleanup.

## Failed and blocked

Strict Clippy for the four changed primary crates remains failed on existing
repository debt outside the new C4 owners. Representative command totals were
107 core test-target diagnostics, 375 sema test-target diagnostics, 45
runtime-plan diagnostics, and 103 compiler diagnostics. These include existing
large functions/enums, unnested patterns, and documentation/style lints; no
`allow` was added to hide a new C4 diagnostic.

`just test-workspace` was attempted twice. Both attempts were blocked by the
Windows paging file (`os error 1455`) while rustc tried to mmap the existing
`arcweft_bundle` rlib. The second attempt cascaded into delayed rustc import
ICEs after that mmap failure. The same checkout passed the complete workspace
all-target/all-features check, the changed-crate suites, and all focused tests.

## Structural ownership review

The initial 2,674-LOC coverage owner was not accepted as one cohesive file. It
was decomposed without creating a second analyzer, budget, cache, or traversal:

- `match_transaction.rs` owns limits, work, budget, errors, and transcript-byte
  accounting;
- `canonical_literal.rs` is the sole checked literal grammar shared by
  transcript and coverage;
- `match_coverage/model.rs` owns the private matrix/domain algebra;
- `domain.rs` owns exact catalog joins and `SequencePartitionPlan`;
- `deconstruct.rs` owns `PatternSite`, exhaustive HIR routing, payload lifting,
  and `SequencePatternForm`;
- `matrix.rs` owns constructor/wildcard usefulness, specialization/default,
  and witness construction; and
- `match_coverage.rs` owns analyzer state and arm transaction orchestration.

Touched pre-existing triggers were reviewed as follows:

- `arcweft-core`'s `pattern.rs`, `value.rs`, `plan.rs`,
  `plan/construction/lower.rs`, and `pure.rs` remain the respective sole owners
  of checked pattern admission, runtime value identity, plan type resolution,
  seed validation, and pure evaluation. The change extends their existing
  exact payload/Record responsibility and adds no parallel schema or evaluator.
- sema's `model.rs`, `nominal_schema.rs`, `semantic_coordinate.rs`,
  `semantic_transcript.rs`, `validation.rs`, `ownership.rs`, `types.rs`,
  `env/base.rs`, `analyzer/patterns.rs`, and `match_edges.rs` retain their
  existing typed fact, coordinate, validation, and owner boundaries. The new
  layout-free nominal catalog and variant payload type were separated instead
  of further accumulating in those large owners.
- runtime-plan's `semantic_facts.rs`, `final_flow.rs`, `final_expr.rs`, and AWBC
  lowerers remain the single normalization/lowering owners. A shared
  `final_variant.rs` projector was added to prevent duplicate Try payload
  wrapping.
- compiler `lower.rs` remains the single final-sema-to-runtime-plan projection
  owner. This cut adds exact payload child projection only; it does not add a
  parallel lowerer.
- large existing sema and core test owners remain above review thresholds, but
  new Match tests were split into responsibility-specific modules. No new
  integration-test file exceeds the structural threshold.

The audit found no forbidden dependency edge or mixed I/O/state owner caused by
this cut.

## Not completed in this cut

C3 complete semantic transcript closure and C5 exhaustive-only atomic
publication remain required. Current known C3 work includes expression shape
atoms, exact live resolution payloads including `Closure`, statement/body and
rich-text digests, nested Match embedding, removal of `UnsupportedIdentity`,
and accepted-root/path closure. This cut does not claim completion for those
items or for the later positive fixture/phase-inversion work.

Unary Need, Await/View, producer outcome closure, positive fixture gate, Const
phase fence, and Need timeout remain separate convergence cuts after C3/C5.
