# Seq06.11d.1 typed Style syntax, HIR, and semantic catalog

This cut implements the language and semantic ownership slice from the
`arcweft-seq06.11d-typed-style-computed-style-2026-07-13` design package. It
replaces the provisional line-oriented Style path with one typed pipeline from
source syntax through HIR and semantic checking.

## Implemented

- Made `arcweft-lang-syntax::ast::style` the sole owner of top-level and inline
  Style syntax nodes.
- Added the dedicated Style parser with explicit selector sequences,
  combinators, declarations, typed token annotations, native expressions, CSS
  source preservation, source ranges, and structured recovery.
- Lowered named Style declarations and inline patches into HIR-owned types;
  HIR no longer stores syntax Style declarations.
- Added the `arcweft-view` property, value, selector, sheet, application, and
  invalidation metadata boundaries used by semantic checking and later runtime
  cuts.
- Added canonical owner lookups for View properties, elements, states, and
  presentation system colors. Removed source-name special cases from semantic
  checking.
- Added `CheckedViewStyleCatalog` with checked sheet, token, rule,
  declaration, selector, inline-patch, value, and source-range records.
- Added deterministic token dependency checking, including duplicate,
  unresolved, cyclic, annotation, and expected-kind diagnostics.
- Applied the same expression and value checks to top-level native Style and
  inline native patches. CSS bodies remain raw source with exact ranges.
- Moved interactive-overflow applicability into semantic Style diagnostics.
- Removed non-visual `SemanticLabel`, `StructuralCondition`, and `Custom` slots
  from the authored property inventory and deleted the unused duplicate
  `style_authoring` model.
- Split normalized `Ratio` from non-negative, unbounded `Scalar`, and added
  typed shadow, filter, clip, mask, transition, corner-frame, and alignment
  checks needed by current samples.
- Migrated `css-style-parity`, `modern-feedback-view`, and `native-text-input`
  away from provisional text/milli/environment spellings. Typed environment
  conditions return in d.4 through the closed grammar rather than an
  old-syntax parser branch.

## Direct replacement decisions

- Removed the old duplicate Style AST records instead of keeping aliases or
  dual parsers.
- Removed the old inline Arcweft/CSS modifier variants in favor of one typed
  `StylePatch` boundary.
- Unknown source spellings are rejected by owner lookups. Runtime-only legacy
  property variants were removed from the authored owner enum.
- This unpublished compiler surface is replaced directly; no migration shim or
  compatibility reader was added.
- The old runtime-only `Milli`/`Rgba8`/`ViewPropertyValue` cascade data remains
  only until d.3 moves motion and retained runtime consumers to computed style.
  It is not accepted by the new source/sema pipeline.

## Design clarification

The package overlay defined `ViewRatioMilli` as normalized to `0..=1000` while
also assigning `scale`, flex factors, brightness, and contrast to the ratio
family. Those properties validly exceed one. This cut adds a distinct
`ViewScalarMilli` / `ViewStyleValueKind::Scalar` contract for them and keeps
`Ratio` bounded for opacity/progress. This is a correction of an internally
inconsistent proposed API, not a compatibility layer.

## Validation

Passed at the cut point:

- `cargo fmt --all -- --check`
- `cargo test -p arcweft-lang-syntax --all-features --test style_view -- --nocapture`
  — 26 passed
- `cargo test -p arcweft-lang-hir --all-features --test style -- --nocapture`
  — 2 passed
- `cargo test -p arcweft-lang-sema --all-features --test style -- --nocapture`
  — 11 passed
- `cargo test -p arcweft-view --all-features --test style_metadata -- --nocapture`
  — 8 passed
- focused CLI sidecar/overflow/CSS parity tests
- `cargo test -p arcweft-cli --all-features --lib --quiet` — 184 passed
- `target/debug/arcw.exe check --manifest-path samples/native-text-input/arcw.toml`
  — 1 module, 1 compile unit, 0 warnings, 0 obligations
- changed-crate all-target/all-feature check and Clippy with `-D warnings`
- workspace all-target/all-feature check and Clippy with `-D warnings`
- `git diff --check`

The first `just test-workspace` attempt exposed a full `D:` drive and Windows
PDB linker failures. Only the regenerable, verified
`target/debug/incremental` directory was removed, recovering 44.13 GiB. Later
runs exposed two stale CLI/sample expectations, both fixed; the affected CLI
suite then passed all 184 tests. A final full recipe rerun remained blocked by
dependent test-binary relinking and hit the 15-minute command limit; the
resulting BrokenPipe occurred when the runner was terminated, not from a test
assertion. This blocked validation is carried explicitly rather than reported
as passed. Tier 2 MCP/native capture/exact visual suites and doc-tests were not
run because this cut does not touch those risk areas.

## Structural audit

The canonical report is under
`docs/implementation/structure-audits/seq06-11d-1-typed-style-2026-07-13/`.
It records the current revision, file bytes/physical LOC, classification,
dependency edges, duplicate public type names, and all workspace warnings.
The audit has no error-level violations. New Style responsibility modules stay
below the 1,200 LOC warning threshold. Existing `arcweft-cli/src/app/bundle.rs`
remains a warning-level hotspot; d.2 removes its provisional `dsl_view_style_*`
conversion block when compiler-owned resource lowering lands.

## Following cuts

This cut intentionally does not change the product bundle schema or runtime
cascade. The package requires these separately reviewable follow-ups:

1. d.2 sheet-owned resources, compiler lowering, codec validation, and scoped
   applications;
2. d.3 one `arcweft-view` computed-style resolver and runtime projection;
3. d.4 Takumi common-value convergence, logical properties, and adaptive
   conditions;
4. d.5 resolver-native Agent/LSP/formatter explainability.

Those are implementation order boundaries, not optional omissions from the
overall seq06.11d goal.
