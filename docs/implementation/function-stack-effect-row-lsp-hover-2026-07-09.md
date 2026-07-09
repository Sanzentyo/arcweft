# Function Stack Effect Row LSP Hover - 2026-07-09

## Status

Implemented as a 07.8 tooling-consumer slice.

LSP hover now consumes the typed closed effect-row boundary for callable
declaration names. Hovering a `flow`, `fn`, or `agent` declaration name after a
successful type check shows the callable's inferred row, source upper bound
when present, and forbidden row when non-empty.

## Contract

This slice does not add source-level effect-row syntax and does not widen the
semantic effect model. It is a display consumer of the existing
`ClosedEffectRowReport` boundary:

- the hover path parses, lowers, resolves, and type checks the document using
  the active LSP profile;
- hover output is emitted only when type checking succeeds and the callable has
  a closed row summary;
- matching is limited to callable declaration headers so body references such
  as `let body = load_story(...)` are not treated as declaration hovers.

## Evidence

LSP hover regressions cover:

- `flow` and `fn` declaration hovers rendering inferred and upper-bound
  `fs.read` rows from the closed effect-row boundary; and
- body call references with the same callable name not producing declaration
  effect-row hover text.

## Remaining Open Work

This is not the final 07.8 row-display policy. Remaining work still includes
source row syntax, open-row inference/substitution, row-bearing callable
values, final row-origin traces, and any richer LSP display for callback row
origins.

## Validation

```bash
cargo test -p arcweft-lsp --all-features hover_describes_callable_closed_effect_row -- --nocapture
cargo test -p arcweft-lsp --all-features callable_effect_row_hover_ignores_body_name_references -- --nocapture
cargo check -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lsp --all-targets --all-features
cargo fmt --all --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-lsp-hover-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax`, `arcweft-lang-sema`,
`arcweft-runtime-driver`, and `arcweft-runtime-host`; no warning is attributed
to the LSP hover changes. The structure audit reports the existing
`crates/arcweft-lang-sema/src/checker/expr.rs` size error plus 150 warnings.
