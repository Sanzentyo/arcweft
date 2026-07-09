# Function Stack Closure Expected Effect-Row Bound - 2026-07-09

Status: implemented for closed source function type rows.

## Summary

Closed effect rows written on a function type ascription now participate in
closure checking when that function type is the expected type for a closure
literal.

This closes the current closed-row path:

```arcw
let later: String -> String effects { fs.read } =
    |path: String| -> String {
        adapter.read_text(path = path)
    }
```

The semantic closure judgment preserves the expected closed row on the
resulting `TypeKind::Function`. The same row is also registered as the
synthetic closure callable's upper bound, so an empty expected row rejects body
effects with `UpperBoundExceeded` instead of allowing the effect graph to infer
an unbounded private closure row.

## Parser Boundary

The flow-body statement collector now keeps a value-required `let ... =` head
joined with the following indented expression line. This is a general logical
statement rule, not a special case for `effects { ... }`. It prevents
multiline let values such as return-typed closure literals from being split
into a raw flow-item recovery node when the type annotation itself is already
punctuation-balanced.

## Verification

```bash
cargo test -p arcweft-lang-syntax --all-features flow_let_value_continuation_keeps_effect_row_type_ascription_with_closure -- --nocapture
cargo test -p arcweft-lang-sema --all-features closure_expected_ -- --nocapture
cargo test -p arcweft-lsp --all-features hover_describes_closure_expression_expected_effect_row_bound -- --nocapture
```

## Remaining Work

This slice does not implement open rows, row variables, polymorphic
higher-order row inference, or the final runtime-plan/verifier/LSP row
consumer contract. Those remain under
`docs/reviews/requests/2026-07-08-seq-07.8-function-stack-closure-effect-row-final-contract.md`.
