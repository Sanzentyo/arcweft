# Evidence and verification status

**VERIFICATION_STATUS=STATIC_REPOSITORY_INSPECTION_PLUS_ARTIFACT_INTEGRITY**  
**OPEN_QUESTIONS=0**

## Repository basis

- Repository: `Sanzentyo/arcweft` (private; inspected through the configured GitHub connector).
- Branch requested: `main`.
- Latest commit observed and pinned for this package: `c957a61e4a0b9abf094165c41ef4038ce25324c0` (`Document Proof public authority switch`).
- `main` advanced during inspection. Earlier provisional SHAs were discarded; all final decisions were rechecked against the pinned SHA.
- Latest root `AGENTS.md` was read in full at the pinned SHA. No nested exact `AGENTS.md` owner was found by repository search.
- The attached Rust skill was read in full. No Rust code was written.

## Sole request authority

`source/REQUEST.md` is a verbatim copy of the attached request. Its SHA-256 is:

```text
105ea99a9425b367824985f4495b86304a16bc13bba1d0db869d0409f399d837
```

The premise and Rust-skill attachments informed repository handling and Rust design discipline but do not add product requirements beyond the request.

## Current implementation facts actually inspected

| Evidence | Current path / owner | Observed fact at pinned main |
|---|---|---|
| E-001 | `AGENTS.md` (blob `ea4a46132ff8cd004f860c89c854e4cbfe807d86`) | syntax-only crate boundary; inherent behavior on owned enums; direct replacement of unpublished compiler shapes; no shims/aliases/dual readers; no source gates; structural audit and validation policy |
| E-002 | `crates/arcweft-lang-syntax/src/expr.rs` (blob `9ffce152e086fa3974f2d06b34173632b2818c43`) | typed Await node/source is implemented; general Try remains source-less `Expr::Try { expr }` |
| E-003 | `crates/arcweft-lang-syntax/src/expr/prefix.rs` and `expr/pratt.rs` | strict parser recognizes direct `try await`, attached `await?`, general prefix Try, and postfix Try; only Await receives typed source |
| E-004 | `crates/arcweft-lang-syntax/src/parser/expression.rs` (blob `8fcd3cf4ddfeca98e4937973d645bb77f2ebdfe5`) | private ordinary prefix grammar includes Await but not general Try; dialogue has a separate `has_try_prefix` wrapper; ordinary missing-expression recovery is zero-width |
| E-005 | `crates/arcweft-lang-syntax/src/parser/helpers.rs` (blob `d033da4b954a8d2379d7d506b05009b605912130`) | dialogue rescue strips `try ` and constructs the old Try node separately |
| E-006 | `crates/arcweft-lang-syntax/src/expr/source_ranges.rs` (blob `2bf15af934570a3c415cd58b23de48b8e725b3c6`) | Try spelling is recovered with `strip_prefix("try")` / `strip_suffix('?')`; Await recurses using typed operand range |
| E-007 | `crates/arcweft-lang-syntax/src/grammar/kinds.rs` (blob `1cf81a399a93c740e8c0f27be1217121d2efee6a`) | existing `TryExpression`, `MissingExpression`, `MissingTokenNode`, and `MissingToken` kinds are available; no new compatibility kind is needed |
| E-008 | `crates/arcweft-lang-syntax/src/expr/tests.rs` | exact Await source and the four required Await/Try grouping distinctions already have focused substrate tests; Await is not redesigned |
| E-009 | `crates/arcweft-lang-sema/src/checker.rs` / `checker/module.rs` | checker currently stores `expected_returns: Vec<Option<TypeKind>>`; functions, flows, methods, and closures push type-only values |
| E-010 | `crates/arcweft-lang-sema/src/checker/expr.rs` | current Try checks Result/Option but accepts a missing boundary and has no typed operator evidence |
| E-011 | `crates/arcweft-lang-sema/src/checker/suspension.rs` (blob `4d33b3a597161118d197ebd688465b303ea15341`) | propagating Await currently returns the ready type without target selection or error compatibility |
| E-012 | `crates/arcweft-lang-sema/src/types.rs` / `types/compatibility.rs` | typed Result/Option/Need/GenericParam/Projection exist; directional `TypeKind::accepts` is the current compatibility authority; no production propagation conversion resolver was found |
| E-013 | `crates/arcweft-lang-sema/src/diagnostics/error.rs` (blob `e7d129380d16f516e5d42f68da2de0dff3191ca9`) | structured `TypeCheckErrorKind` and stable code mapping exist; the four propagation variants do not |
| E-014 | `crates/arcweft-lang-hir/src/callable_source.rs` and `crates/arcweft-lang-sema/src/callable/*` | existing callable source records carry declaration/signature/result spans and are keyed by one `CallableDeclarationId` |
| E-015 | `crates/arcweft-lang-syntax/src/ast/items.rs`, `ast/flow.rs`, `expr/closure_parse.rs`, and HIR model | function source exists; method, flow, and closure source evidence required for exact related labels is currently incomplete |
| E-016 | `crates/arcweft-lang-sema/src/entry/checker.rs` | Agent controller selection resolves an ordinary callable through the shared catalog; no controller-specific return boundary is needed |
| E-017 | repository-wide `Expr::Try` search | syntax, HIR-adjacent traversal, sema, runtime-plan, verifier, Agent REPL, CLI, docs, and tests all have direct old-shape consumers; inventory is recorded in the CSV |
| E-018 | `just/verify.just` | canonical fmt, clippy, workspace, doc, Tier 2, and full verification entrypoints were inspected and copied into the implementation cuts |
| E-019 | current main includes typed-fragment commit `163281b6089c269d7132941a3ee8fd52710b9b2f`, followed by documentation-only commits | private typed bound fragment families are present; final head `c957a61e4a0b9abf094165c41ef4038ce25324c0` adds only `docs/implementation/2026-07-21-proof-public-switch-readiness.md` after the prior Lang-01.4.1 note; inspected Try/Await/parser/sema/LSP blobs are unchanged, and the documented future function-role/public-authority switch remains outside this request |
| E-020 | `crates/arcweft-lsp/src/diagnostics.rs` (blob `8311e44e8764d871c19aaf2f26af1777fafb5501`) | type-check errors already project through the shared `ArcDiagnostic` adapter into LSP diagnostics; the final cut needs exact propagation span/code projection tests, not a second diagnostic reader |

## What this package verifies

- Every required design decision is fixed, including prefix retention, exact final Rust shapes, grouping, source invariants, boundary selection, generic comparison, conversion policy, diagnostic payloads/codes, owner migration, implementation order, and tests.
- The contract is internally checked for `OPEN_QUESTIONS=0`, missing required package files, duplicate test IDs, and unresolved placeholder markers.
- The source request copy is byte-for-byte identical to the attachment.
- Every payload and manifest file is covered by `SHA256SUMS` (excluding the checksum file itself), and the ZIP is tested for structural integrity after creation.

## What was not executed or claimed

- No repository checkout was available in the artifact runtime, so no Cargo, Just, Clippy, rustfmt, unit, integration, Tier 2, formatter, or structural-audit command was executed against arcweft.
- No production file was edited, committed, or pushed.
- No runtime/bytecode/wire behavior was executed.
- Future implementation compile/test success is therefore a required gate, not evidence already obtained.

These limits are verification status, not open design questions. The implementation contract itself is closed.
