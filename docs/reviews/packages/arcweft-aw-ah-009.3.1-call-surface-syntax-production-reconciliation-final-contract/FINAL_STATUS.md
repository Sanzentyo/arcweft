# Final status

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
REPOSITORY_GIT_COMMIT=8984661d5679efccf7a16255f921530cd0b7cacc
REPOSITORY_JJ_CHANGE=unavailable
```

## Decision-complete result

The selected final model is:

```text
Expr::Call(CallExpr)
  -> CallSurfaceSyntax::Parenthesized(ParenthesizedCallSyntax)
       -> exact non-optional ArgumentListSyntax
  -> CallSurfaceSyntax::CallbackBlock(CallbackBlockCallSyntax)
       -> exact CallbackBlockSyntax
```

All invariant-bearing fields are private. Source-AST constructors are parser-only. Parenthesized missing-close recovery records an exact insertion and owner boundary without a fake `)`. Callback blocks record authored braces/header/body and are explicitly inapplicable to outer signature help. Generated executable applications use the existing source-independent runtime expression representation.

HIR clones the immutable source model. Ordinary sema/runtime/tooling consumers use semantic call accessors. The one AW-AH-009.3 resolver consumes only a selected parenthesized argument-list carrier. Dialogue and speaker special forms own the same exact carrier when parentheses are authored and own no fake list when they are not.

## Readiness checks

- One final call-surface model: **pass**.
- Zero result-changing open decisions: **pass**.
- Exact Rust ownership, visibility, constructors, accessors, and invariants: **pass**.
- Exact parser recovery and owning-boundary behavior: **pass**.
- Explicit callback signature-help behavior: **pass**.
- Parser-only source construction and selected generated representation: **pass**.
- HIR/sema one-resolver consumption: **pass**.
- Complete direct migration/deletion order with no compatibility interval: **pass**.
- Mandatory direct tests, including exact UTF-8 ranges: **pass**.
- No fabricated delimiter or source position: **pass**.
- No source gate, post-parse call search, dual AST, or proof-concurrency prerequisite: **pass**.
- Required archive members and integrity contract: **pass**.

## Production verification boundary

This is the implementation-ready design archive required by the governing production-edits-prohibited request. It does not claim that the future Rust implementation has already been compiled. The exact focused, workspace, Clippy, test, format, diff, and structural-audit gates are frozen in `IMPLEMENTATION_HANDOFF.md`.
