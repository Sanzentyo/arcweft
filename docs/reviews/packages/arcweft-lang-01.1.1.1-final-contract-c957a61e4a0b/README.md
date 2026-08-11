# Lang-01.1.1.1 implementation-ready final contract

- **Status:** FINAL
- **OPEN_QUESTIONS:** 0
- **Closure marker:** `OPEN_QUESTIONS=0`
- **Repository:** `Sanzentyo/arcweft`
- **Inspected branch:** `main`
- **Pinned main commit:** `c957a61e4a0b9abf094165c41ef4038ce25324c0`
- **Request authority:** `source/REQUEST.md`
- **Request SHA-256:** `105ea99a9425b367824985f4495b86304a16bc13bba1d0db869d0409f399d837`
- **Implementation performed:** no
- **Repository mutation performed:** no
- **Normative test rows:** 132
- **Owner migration/deletion rows:** 42
- **Cargo/check/test execution:** no; this package records a static implementation contract and the commands required at each future cut

## Final decision in one paragraph

Retain both general prefix `try expr` and postfix `expr?` as canonical, semantically identical Result/Option propagation forms. Replace the current source-less `Expr::Try { expr }` directly with one `Expr::Try(TryExpr)` carrying a closed `TryOperatorSource` (`PrefixTry` or `PostfixQuestion`) and exact `whole`, `operand`, and operator ranges. Do not alter the already implemented typed Await node: `try await need` and `await? need` remain single propagating `AwaitExpr` nodes. Replace the checker’s type-only expected-return stack atomically with one source-backed lexical propagation-frame stack; reuse the existing `CallableDeclarationId` and AW-AH-009.3 callable catalog, and never route propagation through generator/Stream terminals. Add four structured diagnostics with the stable codes fixed in `FINAL_CONTRACT.md`. Do not add aliases, shims, dual readers, source-text recovery, source gates, CSS/Takumi paths, or a parallel callable catalog.

## Package map

- `FINAL_CONTRACT.md` — normative language, AST, source, semantic, diagnostic, and formatter contract.
- `IMPLEMENTATION_CUTS.md` — compile-clean edit order and validation gates.
- `TEST_MATRIX.csv` — complete positive, negative, malformed, generic, boundary, tooling, and no-compatibility matrix.
- `OWNER_MIGRATION_INVENTORY.csv` — owner-by-owner migration/deletion inventory.
- `EVIDENCE_AND_VERIFICATION.md` — inspected evidence and explicit limits of verification.
- `OPEN_QUESTIONS.md` — closure record; exactly zero open design questions.
- `source/REQUEST.md` — verbatim copy of the sole request specification.
- `MANIFEST.json` and `SHA256SUMS` — artifact integrity metadata.

Normative precedence is: `source/REQUEST.md`, then `FINAL_CONTRACT.md`, then the implementation cuts and matrices. The evidence document is descriptive, not a competing specification.
