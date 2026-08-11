# Compile-clean implementation cuts

**BASE_MAIN=c957a61e4a0b9abf094165c41ef4038ce25324c0**  
**OPEN_QUESTIONS=0**

No production implementation was performed while creating this package. These are the required future edit/validation cuts. A “cut” means a coherent commit candidate; intermediate local edits inside a cut may temporarily fail, but no reviewable commit may retain a dual model or fail its stated gate.

## Cut 0 — Contract freeze, no Rust behavior change

Record the final decision and exact type names from `FINAL_CONTRACT.md`. Do not add a migration alias or temporary public API.

Gate:

```bash
cargo fmt --all --check
```

## Cut 1 — Atomic typed Try syntax switch and all consumer migration

Order inside the working cut:

1. Add focused failing tests for the exact AST/ranges, grouping, trivia, UTF-8, base offsets, malformed recovery, and typed bound-expression fragments.
2. Add `TryOperatorSource`, `TryExprSource`, and `TryExpr`; replace `Expr::Try { expr }` with `Expr::Try(TryExpr)`.
3. Update the strict prefix and postfix parsers; leave the existing Await parser/types unchanged.
4. Add prefix Try to the ordinary private expression grammar.
5. Delete the dialogue-only Try prefix flag/wrapper and `strip_prefix("try ")` construction.
6. Delete Try spelling recovery in `expr/source_ranges.rs`; recurse through `TryExprSource::operand()`.
7. Migrate every exhaustive owner in `OWNER_MIGRATION_INVENTORY.csv` before committing.

No reviewable point may contain both old and new Try variants or both dialogue and ordinary Try readers.

Focused gate:

```bash
cargo test -p arcweft-lang-syntax --lib --tests
cargo test -p arcweft-lang-hir --lib --tests
cargo test -p arcweft-lang-sema --lib --tests
cargo test -p arcweft-runtime-plan --lib --tests
cargo test -p arcweft-verify --lib --tests
cargo test -p arcweft-agent-repl --lib --tests
cargo test -p arcweft-cli --lib --bins
cargo check --workspace --all-targets --all-features
```

## Cut 2 — Source-backed lexical return boundary

1. Add `FlowSignatureSource` and retain it through HIR.
2. Add `ClosureExprSource` to the existing closure variant and retain it.
3. Add existing `FunctionSignatureSource` to `ImplMember::Function` and retain it.
4. Add the propagation evidence/frame types.
5. Replace, do not supplement, `expected_returns` with `return_propagation_frames`.
6. Push boundaries/barriers exactly at the contexts defined in the contract.
7. Read existing `CallableSource`/`HirCallableSignatureSource` by the same `CallableDeclarationId`; do not publish a second catalog.
8. Keep existing return checking behavior on the same new stack.

Focused gate:

```bash
cargo test -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --lib --tests
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
```

## Cut 3 — Try/Await propagation checks and structured diagnostics

1. Route `TryExpr` and `AwaitExpr` source facts into the checker.
2. Implement nearest-frame selection; do not skip inner boundaries or barriers.
3. Apply existing type resolution/substitution, then directional `expected.accepts(actual)`.
4. Add the four `TypeCheckErrorKind` variants and inherent constructors.
5. Add exact codes, primary operator labels, related boundary labels, and typed payload access.
6. Suppress cascades after unresolved operand/boundary type errors.
7. Add ordinary function, closure, method, flow, Agent, generic, mismatch, missing, and generator-barrier tests.

Focused gate:

```bash
cargo test -p arcweft-lang-sema --lib --tests
cargo test -p arcweft-lang-hir --lib --tests
cargo test -p arcweft-cli --lib --bins
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
```

## Cut 4 — Formatter and tooling after grouping is green

Prerequisite: all `SYN-*`, `REC-*`, `HIR-*`, `SEM-*`, `BND-*`, and `DIA-*` required rows pass.

- Update any existing expression printer/refactoring/formatting path to obey the fixed precedence and lexical collision rules.
- If main still has no production Arcweft formatter, do not create a broad formatter subsystem; preserve source variants in the typed APIs and execute the tooling rows that are observable today.
- Update Agent/CLI/LSP/diagnostic serialization and snapshots only to the final typed payload.
- Update grammar and language documentation without introducing a preferred rewrite between canonical spellings.

Gate:

```bash
cargo test -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-agent-repl -p arcweft-cli -p arcweft-lsp --lib --tests
cargo test -p arcweft-runtime-plan -p arcweft-verify --lib --tests
cargo check --workspace --all-targets --all-features
```

## Cut 5 — Final reviewable validation and audit

Run and record:

```bash
just fmt-check
cargo check --workspace --all-targets --all-features
just clippy
just test-workspace
just test-doc
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Because this change spans syntax, HIR, sema, runtime-plan, verifier, Agent, and CLI and materially changes a public compiler contract used by Agent paths, run the repository’s Tier 2 gate at the final cut:

```bash
just test-tier2
```

Structural audit evidence must include current physical LOC/bytes and responsibilities for all changed Rust files and current workspace hotspots, as required by the latest `AGENTS.md`. Do not replace behavioral tests with source spelling/path searches.

## Final no-compatibility acceptance

- Both canonical spellings parse directly to `Expr::Try(TryExpr)`.
- `try await`/`await?` remain one existing Await node.
- Dialogue uses the ordinary expression parser.
- The old Rust variant cannot remain because all workspace targets compile against the new exhaustive enum.
- No source-gate test is added.
- No compatibility alias/constructor/reader or spelling-specific diagnostic is retained.
- No CSS/Takumi code or test path changes.
