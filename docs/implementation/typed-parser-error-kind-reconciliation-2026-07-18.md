# Typed parser error-kind reconciliation

## Scope and source

This implementation follows
`seq-06.11d.4.2.2-implementation-ready-final-contract.zip`, SHA-256
`83FA3B33783BF849E1628CA07E4F20F97AB2D97E26BAEBD0F66BC7C2177DFEDB`,
originally prepared against source revision `9a63ac55`. The implementation was
then reconciled onto main parent `1737b13c`, after Lang-01.2 ordinary-entry
unification.

The completed implementation scope is:

- one repository-owned, closed `ParseErrorKind` registry with exactly 45
  variants: the package's 30 kinds plus 14 typed entry-parser kinds and one
  nominal-generic kind required by the Lang-01.2 main integration;
- allocation-free, exhaustive `code()` and `label()` projections;
- `ParseError` storing only the typed kind, with crate-private constructors;
- atomic removal of raw code builders and the Style-environment fallback
  spelling;
- typed assertion, entry, nominal generic, Style, environment, and View
  part/export producers;
- kind-preserving normalization and incremental snapshots;
- typed forwarding through compiler and Agent REPL failures;
- explicit parser-origin LSP diagnostics;
- structured Agent terminal and JSON projections with an explicit synthetic
  source coordinate space;
- exact mapping, payload, range, recovery, adapter, and constructor-visibility
  tests.

No compatibility alias, reverse code reader, dual representation, CSS/Takumi
path, or removed-syntax recognizer is part of this change.

## Ownership result

`arcweft-lang-syntax` owns `ParseErrorKind` and is the only crate allowed to
construct `ParseError`. Consumers may read the typed kind, stable code and
label, ranges, expected/found payload, message, and recovery suggestions.

`arcweft-source::DiagnosticCode` remains the open transport representation.
The parser projects its typed kind into that representation in one direction;
no consumer reconstructs a parser kind from code text.

LSP parser diagnostics retain the existing code-family source classifier:
`syntax.*` / `AWF0*` use `arcweft-syntax`, while View diagnostic codes use
`arcweft`. Other shared diagnostics retain their existing source selection.

Agent-owned JSON projection now lives beside the typed REPL command
projection. The CLI consumes that one-way projection instead of maintaining a
second parser-payload serializer. Revision-bound direct projections use
source-local UTF-8 byte coordinates; transaction errors that have not yet been
bound to an authored source mapping remain explicitly marked
`synthetic_source`. `DiagnosticApplicability::as_str()` remains owned by
`arcweft-source`, so protocol spellings are not repeated in adapter crates.

## Changed areas

- `arcweft-lang-syntax`: typed owner, producers, normalization, incremental
  retention, exact registry/range/recovery tests, and external construction
  compile-fail fixture;
- `arcweft-compiler`: project diagnostics retain the original parser payload
  alongside its one-way shared-diagnostic projection;
- `arcweft-agent-repl`: typed transaction failure and structured JSON owner;
- `arcweft-cli`: terminal projection characterization and Agent report
  consumption;
- `arcweft-lsp`: explicit parser origin and payload/suggestion projection;
- `arcweft-source`: owned applicability spelling used by protocol adapters.

## Contract correction resolution

The package's cross-adapter fixture requires a source-derived non-`Generic`
diagnostic with nonempty `found` and at least one concrete recovery edit.
No such production producer exists at the pinned revision:

- specialized producers currently preserve `found = None`;
- production parser code creates recovery suggestions without concrete edits;
- `RecoveryEdit` construction exists only in tests and shared infrastructure.

Adding those payloads would change behavior that the same package requires this
cut to preserve. The owner-level payload test therefore proves concrete edit
projection without falsely claiming a production parser producer.

The independent correction request is:

- [seq-06.11d.4.2.2.1 parser recovery payload producer contract correction](../reviews/requests/2026-07-18-seq-06.11d.4.2.2.1-parser-recovery-payload-producer-contract-correction.md)

The completed correction selects the existing
`ViewExportPartMissingAs` producer, preserves `found=None` and its editless
recovery suggestion, and covers concrete edits through a separate typed
test-only fixture:

- [parser recovery payload producer final contract](parser-recovery-payload-producer-final-contract-2026-07-19.md)

No ad hoc parser behavior change is required.

The package's literal-spelling audit also crosses independent typed owners:
`ExprParseError` owns expression diagnostics including
`syntax.assert.unknown_mode`, and `SyntaxLintCode` owns `style::*` lint
spellings. They are not raw `ParseError` producers. The required ownership and
audit correction is tracked separately:

- [seq-06.11d.4.2.2.2 expression parser diagnostic ownership and audit correction](../reviews/requests/2026-07-18-seq-06.11d.4.2.2.2-expression-parser-diagnostic-ownership-and-audit-correction.md)

## Structural self-review

Manual measurements were taken at Jujutsu change `uulyxqzqqmnr` (parent
`1737b13c`) before the executable structure-audit gate.

| Path | Classification | Bytes | Physical LOC | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | production | 64,661 | 1,741 | 0 | top-level item parsing, including typed entry and nominal-generic producers |
| `crates/arcweft-lang-syntax/src/parser/view.rs` | production | 58,903 | 1,653 | 0 | View body, expression, modifier, exported-part orchestration and recovery |
| `crates/arcweft-lang-syntax/src/parser/style.rs` | production | 50,548 | 1,334 | 0 | native Style sheet, selector, token, declaration, and environment grammar |
| `crates/arcweft-lang-syntax/src/incremental.rs` | production + existing unit tests | 44,315 | 1,167 | existing | immutable syntax snapshots, transaction limits, identity reconciliation, edit validation |
| `crates/arcweft-lsp/src/diagnostics.rs` | production + unit tests | 36,394 | 928 | existing | parser/lint/sema/verifier diagnostic adaptation |
| `crates/arcweft-compiler/src/project.rs` | production | 29,820 | 846 | 0 | project compilation and typed diagnostic ownership |
| `crates/arcweft-agent-repl/src/command/json.rs` | production + unit tests | 25,860 | 624 | existing | typed command and transaction JSON projection |
| `crates/arcweft-lang-syntax/src/parser/recovery.rs` | production + unit tests | 24,233 | 563 | existing | parser error owner, 45-kind registry, recovery payload, shared diagnostic projection |
| `crates/arcweft-source/src/diagnostic.rs` | production + unit tests | 9,655 | 319 | existing | shared diagnostic, applicability, edit, and suggestion model |

`items.rs`, `view.rs`, and `style.rs` remain warning-level existing hotspots.
The entry and nominal integration changes existing item-parser diagnostic
construction rather than adding a second parser subsystem. The previously
unreachable environment EOF fixture was moved from the public integration
surface to owner-local `parser/style/tests.rs`; production behavior was not
changed to manufacture that branch. No file crosses the 2,500-LOC production
error threshold because of this change.

The largest current workspace Rust sources remain the generated Unicode
vertical-orientation table (357,456 bytes/12,399 LOC) and existing CLI
integration-test modules (`cli_runtime_bench.rs`, 256,505 bytes/7,970 LOC;
`native_vertical.rs`, 238,805 bytes/6,620 LOC;
`published_jlreq_class_mix.rs`, 220,473 bytes/6,109 LOC; and
`native_samples_effects.rs`, 214,731 bytes/5,850 LOC). This change does not
touch them.

No Cargo dependency edge changed. The existing lower-to-higher direction
remains `arcweft-source <- arcweft-lang-syntax <- compiler/Agent/tooling`;
`arcweft-source` currently has 24 direct workspace dependents. The added
applicability spelling is inherent on its owning boundary type instead of
being repeated through adapter helpers.

## Main integration result

The package change was duplicated and rebased onto `1737b13c` so the original
base and its two child correction workspaces could finish independently. Two
textual conflicts referred to the compiler and Agent paths removed by
Lang-01.2. The main-side files were retained and the intended behavior was
ported onto the replacement ownership boundary:

- `ProjectCompileDiagnostic` stores `Option<ParseError>` only for syntax
  failures and exposes it read-only;
- the shared `Diagnostic` remains a one-way projection;
- Agent REPL compilation forwards that typed payload from the project compiler
  and never reconstructs it from a code string;
- ordinary compiler diagnostics carry no parser payload.

Compilation then exposed 16 Lang-01.2 call sites still using the removed raw
code builder. They are now represented by 14 entry kinds and one
`NominalInvalidGenericParameters` kind. All entry producers are covered by
actual parser inputs; duplicate-role and duplicate-goto related ranges remain
intact. The nominal producer now highlights the actual `<...>` group rather
than the declaration name.

Seq-06.11d.4.2.2.1 and Seq-06.11d.4.2.2.2 remain separate follow-on cuts. They
were validated against the exact original package parent and must be
transplanted after this main-integrated base. Seq-06.11d.4.2.3 still overlaps
the native Style parser and must retain both this typed diagnostic ownership
and its predicate/body source-range model.

## Verification

Static implementation checks cover:

- no `ParseError::coded` or `ParseError::with_code`;
- no `code: String`, `from_code`, `TryFrom<&str>`, `Other(String)`, or
  string-to-kind fallback;
- no production Style-environment base fallback spelling;
- no parser consumer infers kind from message or code;
- no CSS, Takumi, media-query, or general-expression route was introduced.

Executed:

```text
cargo fmt --all -- --check
PASS

cargo test -p arcweft-lang-syntax --all-features
PASS

cargo test -p arcweft-source --all-features
PASS

cargo test -p arcweft-compiler --all-features
PASS

cargo test -p arcweft-agent-repl --all-features
PASS

cargo test -p arcweft-lsp --all-features
PASS (140 unit tests; the produced test binaries were executed directly after
the surrounding Cargo invocation timed out while entering the CLI build)

cargo test -p arcweft-cli --lib diagnostics
PASS (9 selected tests)

cargo test -p arcweft-cli --lib repl_command_bridge
PASS (3 selected tests)
```

The first check reported only formatting differences; `cargo fmt --all` was
applied and the check then passed.

A changed-file scan also passed with no trailing whitespace, conflict markers,
or missing final line feed. `git diff --check` is not applicable in this
Jujutsu-only workspace because it has no colocated Git working-tree metadata;
the changed-file scan supplies the corresponding whitespace evidence.

The disk embargo was lifted after `cargo clean`. Before the correction child
was integrated, the root checkout passed
`cargo check --workspace --all-targets --all-features`. The integrated cut
repeats that check together with the remaining review gates:

```bash
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The correction child later ran focused source, syntax, compiler, Agent, LSP,
and CLI routes plus structured metadata and the canonical audit. Its exact
results, inherited `.part()` span assertion mismatch, and default-feature
asset blocker are recorded in the
[parser recovery payload producer final contract](parser-recovery-payload-producer-final-contract-2026-07-19.md).
It did not retroactively run every broad predecessor gate above.

The new trybuild fixture's compiler-produced visibility errors were reviewed
and accepted in `tests/ui/parse_error_construction.stderr`.

## Non-goals

- changing parser recovery payload semantics merely to manufacture a fixture;
- globally closing `arcweft-source::DiagnosticCode`;
- stabilizing parser codes as an external wire compatibility promise;
- changing native environment grammar, typed AST/HIR, or checked presentation
  behavior;
- introducing removed-syntax diagnostics or compatibility migration paths.
