# AW-AH-009.3.1 call-surface syntax production reconciliation

Status: **READY FOR IMPLEMENTATION**  
Repository: `Sanzentyo/arcweft`  
Inspected `main`: `8984661d5679efccf7a16255f921530cd0b7cacc`  
Jujutsu change: `unavailable`  
Parent contract: AW-AH-009.3, archive SHA-256 `cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`

## Result

This contract replaces the contradictory assumption that every semantic call has a parenthesized argument list. The final source model keeps one semantic `Expr::Call` variant and gives it one exhaustive authored-surface enum:

- `CallSurfaceSyntax::Parenthesized(ParenthesizedCallSyntax)` owns the exact `ArgumentListSyntax` required by AW-AH-009.3;
- `CallSurfaceSyntax::CallbackBlock(CallbackBlockCallSyntax)` owns exact callee, brace, closure-parameter, arrow, body, and closing-brace ranges without claiming that braces are parentheses;
- signature help is applicable only to a parenthesized argument-list carrier; an outer callback-block application is explicitly `NotApplicable`, while nested parenthesized calls inside its body remain eligible;
- source-AST call construction is parser-only; generated applications use the existing source-independent runtime expression model and never fabricate authored ranges.

All fields that establish source invariants are private. All constructors that can create `CallExpr`, `ArgumentListSyntax`, or callback-block syntax are `pub(crate)` to `arcweft-lang-syntax`. Downstream crates receive read-only accessors and clone the immutable types through the current HIR boundary.

## Archive map

Read in this order:

1. `FINAL_CONTRACT.md` — normative Rust model, invariants, recovery, cursor behavior, HIR/sema consumption, and deletion rules.
2. `PRODUCTION_RECONCILIATION.md` — current contradiction, producer inventory, replaced parent clause, and unchanged AW-AH-009.3 clauses.
3. `IMPLEMENTATION_HANDOFF.md` — exact compiling-frontier order, module ownership, direct caller migration, deletion points, and validation commands.
4. `TEST_MATRIX.md` — direct observable tests, including exact UTF-8 byte ranges.
5. `REQUIREMENTS_TRACEABILITY.md` — request-to-contract and request-to-test mapping.
6. `REPOSITORY_EVIDENCE.md` — inspected revision, current owner/consumer evidence, and verification boundaries.
7. `FINAL_STATUS.md` and `OPEN_QUESTIONS.md` — readiness decision and zero open decisions.
8. `MANIFEST.txt` — verified member integrity.

## Delivery boundary

The governing request prohibits production edits. This archive therefore contains a decision-complete production design and implementation handoff, not a patch, checkout, overlay, Cargo output, or fabricated build log. The package does not claim that the future Arcweft production change has already passed Cargo validation. It records the exact commands that the implementation assignee must run.

No parent signature-query, cache, accepted-world, character nominal, checked LSP position, or proof typed-node identity policy is redesigned here. Only the authored call-surface contradiction is reconciled.
