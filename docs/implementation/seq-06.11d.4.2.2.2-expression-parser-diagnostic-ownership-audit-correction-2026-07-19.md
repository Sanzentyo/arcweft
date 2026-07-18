# Seq-06.11d.4.2.2.2 expression parser diagnostic ownership correction

`OPEN_QUESTIONS=0`

## Scope, package, and base

This implementation follows the single package
`D:\sanze\Downloads\arcweft-seq-06.11d.4.2.2.2-expression-parser-diagnostic-ownership-audit-correction-final-contract.zip`,
whose SHA-256 is
`90128E3876A43FA75C7FC21BDEC24292149C815357AE8674DEBFDB005E132AE6`.
Every file listed by the package's `SHA256SUMS` matched its recorded digest.
The packaged `SOURCE_REQUEST.md` digest is
`535CB0F23F62C407ABA0FEA252B8C9E919169ED99344D14333EE90DCBD48A925`.

The implementation was reconciled against the actual predecessor Jujutsu
change `kmlsstwutntn` / commit `56712801`, not reconstructed from the design
package's older inspected revision. The current child working-copy change is
`ltzmpmrstyyw`. It was created directly from that predecessor in
`D:\git\arcweft-ws-seq-06-11d-4-2-2-2`.
The predecessor implementation records
`seq-06.11d.4.2.2-implementation-ready-final-contract.zip`, SHA-256
`83FA3B33783BF849E1628CA07E4F20F97AB2D97E26BAEBD0F66BC7C2177DFEDB`,
as its source artifact in
[typed-parser-error-kind-reconciliation-2026-07-18.md](typed-parser-error-kind-reconciliation-2026-07-18.md).

The independent Seq-06.11d.4.2.2.1 recovery-payload decision is not selected
or changed here.

## Final ownership graph

```text
statement/document parser producers
    -> ParseErrorKind (closed: exactly 30)
    -> ParseError
    -> arcweft_source::Diagnostic
    -> compiler / CLI / LSP projections

strict expression parser producers
    -> ExprParseError { code: &'static str, range, message }
    -> direct strict caller

lossy expression consumers
    -> consume ExprParseError as control flow
    -> Expr::Raw plus existing statistics/recovery behavior
    -> no ParseError conversion

syntax lint producers
    -> SyntaxLintCode (closed: exactly 6)
    -> SyntaxLint
    -> arcweft_source::Diagnostic
    -> compiler / CLI / LSP projections
```

`arcweft_source::Diagnostic` remains a one-way projection envelope. It does
not reconstruct any owner from a code string. The private
`AssertionParseError` is a typed statement-parser producer payload rather than
an additional diagnostic-code owner.

## Locked decisions

| Decision | Implemented contract |
| --- | --- |
| `ParseErrorKind` scope | Discriminator for `ParseError`; not a universal syntax registry. |
| `ExprParseError` | Retains its static code field, exact range, and owned message. |
| `SyntaxLintCode` | Remains a six-variant independent lint owner. |
| Shared unknown-mode code | `syntax.assert.unknown_mode` is intentionally equal across statement and expression owners. |
| Unknown-mode payload | Statement range remains the `assume` token; expression range remains the full reserved call. |
| Uniqueness | Enforced independently within each closed owner; never globally across owners. |
| Inventories | Inherent `pub(crate) const ALL: [Self; N]` on each original enum. |
| Assertion conversion | Direct `From<AssertionParseError> for ParseError`; no string or message inspection. |
| Audit evidence | Typed construction, exact mapping, public behavior, projection, and compile-fail doctests. |
| Compatibility | No aliases, dual readers, fallback variants, or reverse code readers. |

## Actual predecessor reconciliation

| Contract area | Actual `kmlsstwutntn` state | This correction |
| --- | --- | --- |
| `ParseErrorKind` table | Already typed and exactly 30 variants. | Mapping remains byte-for-byte equivalent; zero rows added, removed, or renamed. |
| `ParseError` storage | Already stored `ParseErrorKind`. | Preserved. |
| `ParseErrorKind::ALL` | Public borrowed slice, used by an external integration test. | Reconciled to the required inherent `pub(crate) [Self; 30]`; exhaustive coverage moved beside the owner. |
| Assertion private payload | Already stored `ParseErrorKind`. | Preserved; no redundant migration. |
| Assertion-to-parser conversion | Flow manually projected kind, range, and message through `new_with_kind`. | Replaced with the required owned `From` conversion and `error.into()`. |
| `ExprParseError` | Already had the required static code/range/message representation. | Production representation and grammar unchanged; behavior and negative API coverage strengthened. |
| `SyntaxLintCode` | Already independent with six variants. | Mapping unchanged; required inherent inventory and owner-local tests added. |
| Broad-prefix audit | No executable source audit or source gate was present in the predecessor working tree. | No deletion target existed. No scanner, compatibility mode, or structure-audit rule was added. |
| Compiler/LSP/CLI | Separate parser and lint projection paths already existed. | Production paths unchanged; regressions characterize the separation. |

## Immutable `ParseErrorKind` table

The correction's mapping delta is empty. The accepted predecessor order,
codes, and labels remain:

| # | Variant | Code | Label |
| ---: | --- | --- | --- |
| 1 | `Generic` | `syntax.parse` | Parse error |
| 2 | `AssertionUnknownMode` | `syntax.assert.unknown_mode` | Unknown assertion mode |
| 3 | `AssertionInvalidArgument` | `syntax.assert.invalid_argument` | Invalid assertion argument |
| 4 | `AssertionUnclosedArguments` | `syntax.assert.unclosed_arguments` | Unclosed assertion argument list |
| 5 | `AssertionEmptyConditions` | `syntax.assert.empty_conditions` | Empty assertion condition list |
| 6 | `AssertionTooManyConditions` | `syntax.assert.too_many_conditions` | Too many assertion conditions |
| 7 | `StyleInlineSelectorNotSupported` | `style::inline_selector_not_supported` | Selector rule in inline Style |
| 8 | `StyleMalformedSelector` | `style::malformed_selector` | Malformed Style selector |
| 9 | `StyleEnvironmentExpectedOpenParen` | `syntax.parse.style_environment.expected_open_paren` | Expected environment opening parenthesis |
| 10 | `StyleEnvironmentExpectedField` | `syntax.parse.style_environment.expected_field` | Expected environment field |
| 11 | `StyleEnvironmentExpectedComparison` | `syntax.parse.style_environment.expected_comparison` | Expected environment comparison |
| 12 | `StyleEnvironmentExpectedValue` | `syntax.parse.style_environment.expected_value` | Expected environment value |
| 13 | `StyleEnvironmentExpectedCommaOrCloseParen` | `syntax.parse.style_environment.expected_comma_or_close_paren` | Expected environment clause separator |
| 14 | `StyleEnvironmentExpectedOpenBrace` | `syntax.parse.style_environment.expected_open_brace` | Expected environment body opening brace |
| 15 | `StyleEnvironmentUnterminatedCondition` | `syntax.parse.style_environment.unterminated_condition` | Unterminated environment condition |
| 16 | `StyleEnvironmentUnsupportedValue` | `syntax.parse.style_environment.unsupported_value` | Unsupported environment value |
| 17 | `StyleEnvironmentTokenNotAllowed` | `syntax.parse.style_environment.token_not_allowed` | Style token in environment body |
| 18 | `ViewExportPartMisplaced` | `view::export_part_misplaced` | Misplaced View part export |
| 19 | `ViewDuplicatePartModifier` | `view::duplicate_part_modifier` | Duplicate View part modifier |
| 20 | `ViewExportPartMissingPart` | `view::export_part_missing_part` | Missing `part` keyword in View export |
| 21 | `ViewExportPartDuplicateAs` | `view::export_part_duplicate_as` | Duplicate `as` keyword in View part export |
| 22 | `ViewExportPartTrailingSyntax` | `view::export_part_trailing_syntax` | Trailing syntax in View part export |
| 23 | `ViewExportPartMissingLocal` | `view::export_part_missing_local` | Missing local View part name |
| 24 | `ViewExportPartInvalidLocalName` | `view::export_part_invalid_local_name` | Invalid local View part name |
| 25 | `ViewExportPartMissingAs` | `view::export_part_missing_as` | Missing `as` keyword in View part export |
| 26 | `ViewExportPartMissingPublic` | `view::export_part_missing_public` | Missing public View part name |
| 27 | `ViewExportPartInvalidPublicName` | `view::export_part_invalid_public_name` | Invalid public View part name |
| 28 | `ViewPartMissingName` | `view::part_missing_name` | Missing View part modifier name |
| 29 | `ViewPartTrailingSyntax` | `view::part_trailing_syntax` | Trailing syntax in View part modifier |
| 30 | `ViewPartInvalidLocalName` | `view::part_invalid_local_name` | Invalid View part modifier name |

The owner-local unit test compares the inherent inventory with this exact
mapping, verifies all 30 variants and codes are unique within the owner, and
constructs a typed `ParseError` for every row.

## Expression producer and sink inventory

This inventory is a one-time review of typed boundaries, not an executable
source-spelling gate.

| Owner path | Producer classes reviewed | Observable disposition |
| --- | --- | --- |
| `src/expr.rs` | Empty input and type-reference adaptation use `ExprParseError::new`; post-parse assertion classification uses `ExprParseError::at`. | Generic `syntax.expr.parse`; reserved assertion `syntax.assert.statement_only` and `syntax.assert.unknown_mode`. |
| `src/expr/pratt.rs` | Trailing-token `at`; tuple, call, thread, record, and expected-token `new` sites. | `syntax.expr.unexpected_token` or generic `syntax.expr.parse`. |
| `src/expr/prefix.rs` | Invalid lexer token and ordinary unexpected prefix `new`; depth and missing operand `at`. | Generic, `syntax.expr.prefix_depth_limit`, or `syntax.expr.missing_prefix_operand`. |
| `src/expr/closure_parse.rs` | Parameter, annotation, delimiter, and callback-body failures. | Generic `syntax.expr.parse`. |
| `src/expr/closure_source.rs` | Closure source split, annotation/body, and delimiter failures. | Generic `syntax.expr.parse`. |
| `src/expr/control_parse.rs` | Unclosed expression-block failure. | Generic `syntax.expr.parse`. |

Reviewed non-return sinks:

- strict `parse_expr`, `parse_expr_at`, and stats variants return
  `ExprParseError` unchanged;
- `parser/helpers.rs` deliberately falls back to `Expr::Raw` and reads only
  `syntax.expr.prefix_depth_limit` for its existing statistic;
- `parser/assertion.rs` maps any strict condition failure to
  `ParseErrorKind::AssertionInvalidArgument` without reading the expression
  code or message;
- bracket-index fallback in `expr.rs` and guard/body recovery in
  `control_parse.rs` preserve their existing `Expr::Raw` behavior;
- no `ExprParseError -> ParseError`,
  `ExprParseError -> arcweft_source::Diagnostic`, compiler, LSP, or CLI
  conversion exists or is introduced.

The stable public expression fixtures cover all six observable codes with
exact type/code/range/message evidence. Known and unknown reserved assertion
calls remain rejected, while `assert(true)` and
`object.assert.check(true)` remain ordinary valid expression calls.

## Independent syntax-lint owner

The inherent six-row inventory remains:

| Variant | Stable code | Domain name | Default severity |
| --- | --- | --- | --- |
| `DeepDotRunRelativeId` | `AWF0001` | `id::deep_dot_run` | Warning |
| `FlowIdModuleMismatch` | `AWF0002` | `id::flow_module_mismatch` | Warning |
| `RedundantDeclIdentity` | `AWF0101` | `style::redundant_decl_identity` | Warning |
| `DeclBindingMismatch` | `AWF0102` | `identity::decl_binding_mismatch` | Error |
| `ExplicitDeclId` | `AWF0103` | `style::explicit_decl_id` | Hint |
| `GeneratedSurfaceForm` | `AWF0104` | `style::generated_surface_form` | Information |

Tests compare every `SyntaxLintCode::ALL` row exactly and enforce stable-code
and domain-name uniqueness independently. The `ExplicitDeclId` behavior test
also checks its typed owner, `AWF0103` diagnostic code,
`style::explicit_decl_id` label, Hint severity, and existing replacement
suggestion. It neither imports nor converts through `ParseErrorKind`.

## Implementation delta

Production behavior changes are limited to:

- an owned typed `From<AssertionParseError> for ParseError` conversion;
- the flow assertion call site using `self.errors.push(error.into())`;
- required inherent closed-owner inventory shapes;
- public documentation compile-fail examples proving the two prohibited
  conversion directions are absent;
- deriving the operand range for `.part(...)` / `.part(part)` from the actual
  argument shape, which corrects a source-range bug without changing the View
  grammar; and
- extracting Style environment delimiter recovery into the named
  `take_environment_delimiters` grammar helper while resolving the predecessor
  Clippy complexity hotspot, with no syntax or recovery-output change.

Test-only changes add exact expression, assertion conversion, public recovery,
cross-owner collision, lint, compiler, LSP, and CLI projection evidence. The
former external `ParseErrorKind::ALL` integration test was removed because the
contract requires crate-private inventory visibility; its stronger exhaustive
mapping and payload coverage now lives in the owner module.
Style end-of-input recovery coverage likewise moved beside its private owner,
and the public-construction compile-fail case has a reviewed trybuild stderr
fixture.

No Cargo manifest, feature, dependency edge, syntax grammar, source format,
runtime behavior, CSS path, Takumi path, native Style grammar, View grammar,
or structure-audit rule changed.

## Acceptance ledger

`PASS` means the typed or behavioral evidence was freshly executed in this
working copy. The structural-audit row records the canonical command's exact
strict-mode outcome rather than disguising its warning-driven nonzero exit as
a pass.

| ID | Implementation evidence | Fresh execution |
| --- | --- | --- |
| PK-01 | Owner-local exact 30-row `ParseErrorKind::ALL` mapping and count. | PASS |
| PK-02 | Owner-local variant and code uniqueness sets. | PASS |
| PK-03 | Every kind constructs `ParseError`; generic payload, diagnostic, recovery, and rebasing checks retained/strengthened. | PASS |
| PK-04 | Separate `ParseErrorKind` compile-fail doctests reject `FromStr` parsing and `TryFrom<&str>` conversion. | PASS |
| EX-01 | Explicit `ExprParseError` empty-input generic fixture with exact code/range/message. | PASS |
| EX-02 | `&value as Type` exact trailing-token fixture. | PASS |
| EX-03 | Inclusive 64/65 prefix-depth boundary with exact failure payload. | PASS |
| EX-04 | `&mut` and all existing sync-token missing-operand fixtures with exact payload. | PASS |
| EX-05 | Nested known assertion call exact owner/code/range/message. | PASS |
| EX-06 | Prove/check/debug known-mode matrix. | PASS |
| EX-07 | Unknown expression mode exact owner/code/full-call range/message. | PASS |
| EX-08 | Both non-reserved expression controls remain accepted. | PASS |
| EX-09 | `ExprParseError` compile-fail doctest rejects conversion into `ParseError`. | PASS |
| ST-01 | Private unknown-mode fixture selects `AssertionUnknownMode`, `7..13`, exact message. | PASS |
| ST-02 | Public flow fixture checks one typed error and complete empty recovery payload. | PASS |
| ST-03 | Exact `Stmt::Raw` source/range plus following typed return recovery. | PASS |
| ST-04 | Empty, invalid, unclosed, over-limit, and unknown statement matrix with exact kinds/ranges/messages. | PASS |
| CV-01 | Misleading code-like message proves typed conversion copies message and derives code only from kind. | PASS |
| CV-02 | Public flow conversion and common diagnostic projection use exact kind/code/range/message. | PASS |
| COL-01 | Real statement and expression producers assert required code/message equality and distinct owner ranges. | PASS |
| COL-02 | Parse and lint uniqueness checks remain owner-local; no combined set exists. | PASS |
| LN-01 | Exact inherent six-row lint inventory. | PASS |
| LN-02 | Independent stable-code and domain-name uniqueness sets. | PASS |
| LN-03 | Typed `ExplicitDeclId` common diagnostic and suggestion projection. | PASS |
| LN-04 | Compiler valid-parse lint facade remains separate from parser errors. | PASS |
| CP-01 | Compiler parser facade regression preserves statement owner and payload. | PASS |
| CP-02 | Compiler lint facade regression preserves typed lint owner and AWF mapping. | PASS |
| LSP-01 | Exact parser code/message/source/severity/UTF-16 range regression. | PASS |
| LSP-02 | Existing independent AWF severity/source/suggestion regressions retained. | PASS |
| CLI-01 | Focused check test requires one code/message occurrence and failure. | PASS |
| CLI-02 | Focused valid lint fixture requires success plus AWF/domain output and no assertion code. | PASS |
| AUD-01 | Acceptance depends on Rust types and executable behavior only. | PASS |
| AUD-02 | Canonical structure audit remains unchanged and was run after behavior gates. | 0 errors; 129 repository warnings; strict exit 1 |
| REG-01 | Full required syntax lib/integration/doc suite. | PASS |
| REG-02 | Compiler, LSP, CLI, workspace, and Clippy gates. | PASS |

## Manual structural audit

Measurements were taken from change `ltzmpmrstyyw` after the implementation
and validation repairs. Bytes are exact current file lengths and LOC is
physical line count. The deleted external
`tests/parse_error_kind.rs` has no current-file metric because its coverage
moved owner-local.

| Path | Owning crate | Classification | Bytes | LOC | Embedded test LOC | Responsibilities |
| --- | --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-cli/tests/check_core_cli.rs` | `arcweft-cli` | integration test | 2,033 | 62 | 0 | focused binary help, invalid-option, parser, and lint projection checks |
| `crates/arcweft-compiler/tests/diagnostic_ownership.rs` | `arcweft-compiler` | integration test | 1,321 | 35 | 0 | parser/lint facade ownership regressions |
| `crates/arcweft-lang-syntax/src/expr/tests.rs` | `arcweft-lang-syntax` | unit-test module | 10,699 | 305 | 0 | strict expression grammar and exact diagnostic fixtures |
| `crates/arcweft-lang-syntax/src/expr.rs` | `arcweft-lang-syntax` | production plus external test module declaration | 32,789 | 1,236 | 2 | expression model, parser facade, lexer tokens/helpers, error owner |
| `crates/arcweft-lang-syntax/src/lint.rs` | `arcweft-lang-syntax` | production plus unit tests | 30,230 | 1,006 | 398 | syntax lint owner, ID lint policy, common projection, exact mapping tests |
| `crates/arcweft-lang-syntax/src/parser/assertion.rs` | `arcweft-lang-syntax` | production plus unit tests | 12,674 | 356 | 103 | reserved assertion parsing, private typed producer, owned conversion |
| `crates/arcweft-lang-syntax/src/parser/flow.rs` | `arcweft-lang-syntax` | production | 31,928 | 766 | 0 | flow parsing and typed statement recovery orchestration |
| `crates/arcweft-lang-syntax/src/parser/recovery.rs` | `arcweft-lang-syntax` | production plus unit tests | 26,749 | 704 | 276 | parser error owner, recovery/edit payload, diagnostic projection, exact inventory |
| `crates/arcweft-lang-syntax/src/parser/style/tests.rs` | `arcweft-lang-syntax` | unit-test module | 1,781 | 53 | 0 | owner-local private Style end-of-input recovery branches |
| `crates/arcweft-lang-syntax/src/parser/style.rs` | `arcweft-lang-syntax` | production plus external test module declaration | 50,901 | 1,400 | 2 | Style statement parsing and environment delimiter recovery |
| `crates/arcweft-lang-syntax/src/parser/view/part.rs` | `arcweft-lang-syntax` | production | 11,151 | 316 | 0 | View part/export parsing and operand source ranges |
| `crates/arcweft-lang-syntax/tests/diagnostic_ownership.rs` | `arcweft-lang-syntax` | integration test | 3,619 | 94 | 0 | public statement recovery and cross-owner collision |
| `crates/arcweft-lang-syntax/tests/style_environment.rs` | `arcweft-lang-syntax` | integration test | 14,640 | 401 | 0 | Style environment syntax and recovery behavior |
| `crates/arcweft-lang-syntax/tests/style_view.rs` | `arcweft-lang-syntax` | integration test | 32,451 | 1,189 | 0 | Style/View integration grammar behavior |
| `crates/arcweft-lang-syntax/tests/view_export_part.rs` | `arcweft-lang-syntax` | integration test | 8,456 | 249 | 0 | View part/export diagnostics and source-range regression |
| `crates/arcweft-lsp/src/diagnostics.rs` | `arcweft-lsp` | production plus unit tests | 37,464 | 1,025 | 483 | parser/lint/HIR/sema/verifier LSP adaptation |

`expr.rs` and `parser/style.rs` are the two changed ordinary production files
above the 1,200-LOC warning threshold. Both were already warning-level
hotspots in the predecessor (1,225 and 1,387 physical LOC respectively).
This correction adds owner-level negative API documentation in `expr.rs`; the
Style validation repair extracts a named grammar helper and private test
module without adding a parsing responsibility. Splitting either existing
owner is a broader architecture refactor and is not mixed into this ownership
correction. No changed production file reaches the 2,500-LOC error threshold,
and no changed file grew by 300 production LOC.

The largest workspace Rust files remain outside this change:

| Path | Classification | Bytes | LOC |
| --- | --- | ---: | ---: |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | generated Unicode lookup data, explicitly marked generated | 357,456 | 12,399 |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | integration test | 256,505 | 7,970 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | ignored Tier 2 integration test | 238,805 | 6,620 |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | integration test | 220,473 | 6,109 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | integration test | 214,731 | 5,850 |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | integration test | 195,821 | 5,249 |
| `crates/arcweft-compiler/src/tests.rs` | unit-test module | 181,160 | 5,388 |

No Cargo edge changed. `arcweft-lang-syntax` retains two Arcweft dependency
fan-out edges (`arcweft-dialogue` and `arcweft-source`) and 14 direct workspace
dependents. The existing direction remains
`arcweft-source/dialogue -> arcweft-lang-syntax -> HIR/compiler/tooling`.

The canonical audit scanned 3,175 files, including 1,602 Rust files and
733,677 physical Rust LOC. It reported zero errors and 129 warning-level
repository hotspots. With `--fail-on-violations`, warnings intentionally
produce exit status 1. Only two warnings name changed files: the pre-existing
`SIZE001` findings for `expr.rs` and `parser/style.rs` described above. No
dependency-edge, facade, embedded-test, or public-type-duplication finding was
introduced by this change.

## Validation record

```text
cargo fmt --all --check
PASS

cargo test -p arcweft-lang-syntax --lib --tests --quiet
PASS (221 unit tests plus all integration and trybuild suites)

cargo test -p arcweft-lang-syntax --doc --quiet
PASS (3 doctests)

cargo test -p arcweft-compiler --lib --tests --quiet
PASS (126 unit tests plus all integration and compile-fail suites)

cargo test -p arcweft-lsp --lib --tests --quiet
PASS (129 unit tests plus integration suites)

cargo test -p arcweft-cli --test check_core_cli --quiet
PASS (4 tests)

cargo check --workspace --all-targets --all-features
PASS

cargo clippy --workspace --all-targets --all-features -- -D warnings
PASS

just test-workspace
PASS

cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit/seq-06.11d.4.2.2.2 --fail-on-violations
COMPLETED: 0 errors, 129 warnings; exit 1 because strict mode includes warnings
```

The first CLI attempt stopped before test execution because this isolated
Jujutsu workspace lacked the repository's intentionally ignored
`web/assets/noto-sans-jp-vf.ttf` build fixture. The fixture was copied from the
root checkout after confirming source and destination SHA-256
`5113756F8A3B5D01B2211025E267C50121E3B36F465B7BBAF3CDAF4C3430BFD0`
and length 9,590,844 bytes; it remains ignored and is not part of the change.
The fresh CLI rerun and every downstream gate then passed.

During Clippy validation, the predecessor Style environment parser exceeded
the configured cognitive-complexity limit and two tests triggered lint
findings. The grammar helper extraction and test-only cleanup described above
resolved those findings. The final all-target/all-feature Clippy run is clean.

## Root integration reconciliation

This correction was integrated after Lang-01.2 replaced the former
direct-source compiler path with the shared project compiler. The first
root-wide test run exposed one real integration regression: successful
project modules retained only a warning count, so a typed non-blocking lint
such as `ExplicitDeclId` / `AWF0103` never reached the CLI.

The final ownership path is:

```text
SyntaxLintCode
    -> SyntaxLint
    -> CompiledProjectModule::syntax_lints()
    -> exact ProjectSourceFile::document()
    -> DiagnosticEmitter
```

The compiler retains the typed lint value in its in-process compiled-module
artifact. The CLI does not reconstruct a lint from `AWF0103`, does not
reparse the source, and does not revive the removed direct compiler path.
Compiler and CLI behavior tests cover the retained typed owner and emitted
AWF/domain projection.

Fresh root gates after reconciliation:

```text
cargo fmt --all --check
PASS

cargo test -p arcweft-compiler --all-features
PASS

cargo test -p arcweft-cli --test check_core_cli
PASS (4 tests)

cargo check --workspace --all-targets --all-features
PASS

cargo clippy --workspace --all-targets --all-features -- -D warnings
PASS

cargo +nightly -Zscript tools/structure-audit.rs --root .
PASS (3,242 files; 1,655 Rust files; 758,568 Rust LOC;
      0 errors; 131 repository warnings)
```

The normal `just test-workspace` route was rerun from the final root source
state and passed with exit code zero. The earlier child result remains useful
package evidence but is not substituted for that integrated run.

## Final exclusion review

The working copy contains none of the following:

- a 31st `ParseErrorKind` variant;
- `ExprParseErrorKind`;
- `From<ExprParseError> for ParseError` or any expression-error lowering;
- `FromStr`, `TryFrom<&str>`, `from_code`, or equivalent
  `ParseErrorKind` reverse reader;
- a compatibility alias, shim, dual reader, fallback enum variant, or
  deserializer;
- a source scanner, source gate, global code-prefix rule, or combined
  cross-owner uniqueness set;
- a diagnostic ownership rule in `tools/structure-audit.rs`;
- CSS, Takumi, assertion grammar, native Style grammar, View grammar, or
  environment-condition redesign.

## Remaining work

There is no known production implementation TODO or design deviation within
this package. The `.part(...)` source-range correction and Style helper
extraction are validation repairs, not grammar redesigns.

Integration note: the root transplant reconciles the predecessor's 30-row
inventory with the 45-variant mainline table. The owner-local exact mapping
test covers that complete table while preserving the ownership model
established here.
