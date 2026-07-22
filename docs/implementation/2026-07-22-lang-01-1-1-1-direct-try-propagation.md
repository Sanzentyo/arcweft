# Lang-01.1.1.1 direct Try propagation completion record

## Acceptance source and baseline

- package:
  `docs/reviews/packages/arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`
- package SHA-256:
  `024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C`
- package repository pin:
  `c957a61e4a0b9abf094165c41ef4038ce25324c0`
- implementation parent:
  `4fd6331dc342d30a7f4ac7774852b60801866ef7`
- normative matrix: 132 rows in the package `TEST_MATRIX.csv`

The package is the acceptance authority for this slice. Both `try value` and
`value?` remain authored Arcweft. This implementation does not add a removed
spelling, alias, compatibility wrapper, source scan, or second Await model.

## Implemented boundary

### Syntax and recovery

- General Try is one `Expr::Try(TryExpr)` node. `TryExprSource` owns the exact
  whole expression, operand, and operator token ranges.
- `TryOperatorSource::PrefixTry` and `PostfixQuestion` retain source spelling
  without changing semantic behavior. Constructors stay syntax-owned.
- Prefix binding power remains 90 and postfix binding power remains 100.
- `try await need` and `await? need` continue to construct one existing
  propagating Await node. `(await need)?` and `await need?` retain their
  distinct nesting.
- The lossless parser uses current-grammar recovery for missing operands and a
  typed missing-token slot for a selected postfix Try production. No malformed
  expression becomes executable syntax.
- Dialogue application, closures, calls, incremental parsing, public AST tests,
  and compile-fail construction tests consume the same typed node rather than
  reconstructing spelling.

### HIR and lexical propagation ownership

- HIR carries `TryExpr` and its source record unchanged.
- `FunctionSignatureSource`, `FlowSignatureSource`, method signature source,
  and closure source expose exact header/result ranges, including nonzero
  document offsets.
- The old type-only expected-return stack is replaced by one lexical
  `ReturnPropagationFrame` stack. Function, flow, method, and closure boundaries
  carry checked return type plus exact accepted `SourceSpan`; generator
  terminals stop propagation instead of leaking to an outer function.
- Compiler-internal HIR that has no accepted source document still carries its
  checked return type in a source-less lexical boundary. It can type-check an
  ordinary `return`, but cannot fabricate diagnostic source evidence or become
  a related-location target for propagation diagnostics.
- Callable declarations reuse the accepted `CallableDeclarationId` catalog.
  Agent entry selection does not create a controller-specific boundary or a
  second callable catalog.

### Type checking and diagnostics

- Result and Option propagation select only the nearest lexical boundary.
- Compatibility is directional after nominal and generic resolution. No
  implicit `From`, `Into`, `ArcError`, Option/Result, function-name, or spelling
  conversion participates.
- Generic substitution compares resolved binder identity and concrete
  instantiations rather than textual generic names.
- Existing nominal poison suppresses propagation cascades; an unconstrained
  closure remains a target-missing boundary rather than inferring its result
  from Try.
- The four structured diagnostic codes are:
  - `sema.await.propagation_target_missing`
  - `sema.await.error_mismatch`
  - `sema.try.propagation_target_missing`
  - `sema.try.error_mismatch`
- Primary labels use the exact `try`, `?`, `await?`, or propagating-Await token.
  Related labels use the exact authored result type when present and the
  callable/flow header otherwise. UTF-8 source spans project through UTF-8 and
  UTF-16 LSP position encodings without rescanning source text.

### Runtime, verifier, Agent, CLI, and LSP

- Runtime-plan and verifier projections treat prefix and postfix general Try as
  one semantic operation.
- Agent REPL and CLI snapshots retain both authored forms with one expression
  kind.
- LSP diagnostics preserve stable codes, exact operator ranges, related return
  boundaries, and source identity.
- Runtime type evidence now recognizes a typed `Expr` judgment independently
  of whether its rule is inferred or expected. This fixes the exposed case
  where a minimal explicitly typed flow had only expected-type expression
  judgments.

### Documentation and formatter ownership

- Grammar, Result/Option, Await, error-trace, CLI, and LSP documentation now
  state that both retained Try/Await spellings are preserved and that
  Option/Result envelope changes require explicit typed conversion.
- The inspected workspace still has no production Arcweft expression
  formatter owner. Matrix rows FMT-001 through FMT-008 are therefore
  `conditional-current-owner` and not applicable to this cut. No broad
  formatter subsystem or canonicalizing rewrite was introduced. The specified
  grouping and spelling rules remain binding for the first such owner.

## Fixture corrections exposed by the final rule

The final contract defines an omitted flow result as `Unit`. Existing positive
fixtures that returned `String` or `i64` without declaring a result were updated
to explicit `-> String` / `-> i64` signatures. The type checker was not relaxed.

## Validation evidence

Focused commands completed in the current checkout:

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-lang-syntax --lib --tests` | pass |
| `cargo test -p arcweft-lang-hir -p arcweft-lang-sema --lib --tests` | pass |
| `cargo test -p arcweft-runtime-plan -p arcweft-verify --lib --tests` | pass |
| `cargo test -p arcweft-agent-repl -p arcweft-cli -p arcweft-lsp --lib --tests` | Agent passed; the first combined run exposed stale CLI/LSP fixtures, whose corrected reruns are split below |
| `cargo test -p arcweft-cli --lib` | pass, 195 tests |
| `cargo test -p arcweft-lsp --lib --tests` | pass; one Lang-01.1.1.2.2-gated test remains explicitly ignored |

Reviewable-cut validation is recorded as follows:

- `just fmt-check`, workspace all-target/all-feature check and Clippy with
  `-D warnings`, and `just test-doc` pass;
- the canonical structural audit reports zero errors;
- the ordinary workspace route passes the direct-propagation and Ref suites,
  including 946/946 sema tests, and remains blocked only by two CLI fixtures
  waiting for `extern capability` associated-type AST/HIR publication; and
- Tier 2 passes 18/22 tests. Its four individually reproducible failures are
  stale MCP/Agent-observe response and fixture assumptions, not propagation
  failures. They remain explicitly open rather than being counted as passing.

A 2026-07-23 follow-up during the accepted adapter-nominal integration closed
that stale Tier 2 evidence. The four failures shared one root cause: three
sample sources returned `String` from flows whose result type was omitted and
therefore authoritatively `Unit`. Four checked-in native golden fixtures had
the same drift. After adding explicit `-> String` annotations,
`CARGO_BUILD_JOBS=1 just test-tier2` passed all 46 selected MCP, Agent-observe,
native-capture, visual-smoke, and exact PNG/imq cases. The historical 18/22
result above remains the exact evidence from the original cut, not the current
repository status.

## Deviations and remaining boundaries

- There is no semantic deviation from the package.
- Formatter rows are conditionally inapplicable exactly as the final contract
  permits; they are not silently counted as implemented.
- Lang-01.1.1.2.1 entity-family projection is implemented in this combined cut.
  Lang-01.1.1.2.2 adapter callable nominal publication is already returned and
  remains the next implementation slice. This slice does not add an opaque
  `Ref` fallback or equate a string `Named` type with an accepted nominal
  identity.
- CSS/Takumi, removed-syntax diagnostics, source gates, dual readers, aliases,
  and migration shims remain absent.
