# Function Stack Effect Row Raw LSP Hover - 2026-07-09

## Status

Implemented as a 07.8 consumer-boundary hardening slice.

LSP callable and closure effect-row hover now reads
`EffectAnalysisReport::effect_rows()` instead of requiring
`closed_effect_rows()`. This preserves the existing closed-row hover behavior
while letting future open row evidence display without first resolving into a
`ClosedEffectRowReport`.

## Contract

This slice does not add source-level open-row syntax and does not infer new row
variables. It moves editor display toward the final row model by making hover
consume the owned raw row boundary:

- `EffectRow::display_label()` is the owned formatting API for closed, open,
  and unknown rows;
- semantic function type labels reuse the same row formatting instead of
  carrying a second local open-row formatter;
- LSP hover displays raw row labels for inferred and upper-bound rows;
- empty closed forbidden rows remain hidden, while future open forbidden rows
  can be shown.

## Evidence

- `EffectRow::display_label()` covers closed rows, open rows, and unknown rows.
- `effect_row_hover_text_renders_open_rows_without_closed_projection` proves
  LSP hover can render `{ fs.read | e9 }` and `{ | e9 }` without resolving a
  closed projection.
- Existing callable and closure hover tests continue to prove the current
  closed-row display path.

## Remaining Open Work

The 07.8 final model still needs source syntax for open rows or row variables,
inference/substitution that produces those rows from checked programs, final
diagnostic policy, and verifier/runtime-plan consumers beyond current artifact
and hover evidence.

## Validation

```bash
cargo fmt --all --check
cargo test -p arcweft-lang-sema --all-features effect_row_display_label_covers_closed_open_and_unknown_rows -- --nocapture
cargo test -p arcweft-lsp --all-features effect_row_hover_text_renders_open_rows_without_closed_projection -- --nocapture
cargo test -p arcweft-lsp --all-features hover_describes_callable_closed_effect_row -- --nocapture
cargo test -p arcweft-lsp --all-features hover_describes_closure_expression_expected_effect_row_bound -- --nocapture
cargo check -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-effect-row-raw-lsp-hover-2026-07-09
```

All commands passed for this slice. Clippy still reports pre-existing warnings
in `arcweft-lang-syntax`, `arcweft-lang-sema`,
`arcweft-runtime-driver`, and `arcweft-runtime-host`; no warning is attributed
to the raw-row hover changes. The structure audit reports 0 errors and 153
warnings.
