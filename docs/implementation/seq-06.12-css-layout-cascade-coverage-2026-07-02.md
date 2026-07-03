# Seq06.12 CSS layout/cascade coverage implementation note — 2026-07-02

This implementation slice adds deterministic CSS coverage classification around
Takumi retained UI lowering.

## Added code

- `arcweft-takumi-adapter::coverage`
- expanded `TakumiDiagnosticCode`
- expanded `DirectCssSupport` to carry `CssCoverageReport`
- focused integration tests for selectors, cascade, custom properties,
  unsupported grid/container/media diagnostics, and invalidation evidence
- fixture/sample data outside `css-style-parity`
- `tools/run-css-layout-cascade-coverage-gates.rs`

## Related docs

- `docs/design/seq-06.12-css-layout-cascade-coverage.md`
- `docs/design/seq-06.12-css-layout-cascade-support-matrix.md`
- `docs/implementation/seq-06.12-css-layout-cascade-future-work-2026-07-02.md`

## Validation

```bash
cargo fmt --all --check
cargo test -p arcweft-takumi-adapter css_layout_cascade --quiet
cargo +nightly -Zscript tools/run-css-layout-cascade-coverage-gates.rs --fixtures fixtures/css-layout-cascade-coverage
cargo test -p arcweft-takumi-adapter --quiet
cargo clippy -p arcweft-takumi-adapter --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq06-12
```

All focused build, test, fixture, and clippy commands above pass in the
repository checkout after applying this package. Structure audit completed and
reported existing repository-wide violations: 2 errors and 126 warnings. The
errors are pre-existing size violations in `crates/arcweft-lang-sema/src/checker/expr.rs`
and `crates/arcweft-runtime-plan/src/flow.rs`; the new seq06.12 files are not
reported as audit violations.

## Structural note

The new production file `crates/arcweft-takumi-adapter/src/coverage.rs` is kept
below the 1,200 physical LOC structural-audit warning threshold in this package
(1,173 physical LOC after repository formatting and lint fixes).
The focused tests live in `crates/arcweft-takumi-adapter/tests/` rather than in an
embedded test module.

## Design deviations

The package patch could not be applied directly because the generated patch was
malformed near the first hunk. The package overlay files were copied and the
existing source connections were applied manually against current `main`.

Clippy-only adjustments were made without changing the public contract:

- derived `Default` for `CssCascadeLayer`;
- rewrote small iterator/`Option` expressions to lint-preferred forms;
- collapsed identical at-rule status branches and removed the now-unused
  environment-media helper.
