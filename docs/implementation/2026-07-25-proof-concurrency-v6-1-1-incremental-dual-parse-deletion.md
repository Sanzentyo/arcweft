# Proof-concurrency v6.1.1 incremental dual-parse deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`

This is a deletion-driven preparatory cut for the Proof-concurrency v6.1.1
public syntax authority switch. It removes the obsolete detached parser from
the incremental transaction itself. It does not publish attached typed handles
or claim completion of Proof Stage 3.

## Package intake and precedence

The repository ZIP inbox was rechecked before selecting this slice. Every
retained `docs/reviews/**/*.zip` SHA-256 has a case-insensitive reference in an
implementation intake/completion note, and the relevant package manifests were
verified again against their payloads.

- Proof-concurrency v6.1.1 base package
  `1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`
  remains `READY_FOR_IMPLEMENTATION`; all 19 non-self manifest rows match and
  its self row is the specified zero placeholder.
- The retained-global reconciliation package
  `0E30A91FA2F7A288E9A12D8AFC7356525604CBDC907D659CD97311207D26A68E`
  remains `READY_FOR_IMPLEMENTATION`; all 17 non-self rows match.
- The AW-AH-009.3.3.3.1 package
  `060332B0B3B3842089D05F36B6ACFF46711A9D706C328B51776CFF3EC74E0D41`
  remains usable. Its `CAP-005` bare-`Vec.with_capacity` row is subordinate to
  the package's own `.3.3.4` authority and `.3.3.4` T08/C17, so bare `Vec`
  remains a type-argument arity failure. No implementation-blocking request or
  speculative correction contract is required.

TTS production remains skipped under its existing intake record. This slice
does not change that boundary.

## Deleted authority

`incremental::SyntaxDatabase` previously ran both parsers for every initial
parse and reparse:

```text
old parser::parse_source -> source::ParsedSource -> detached TypedSyntaxTree
new grammar transaction  -> attached BoundParsedSource
```

The public `incremental::ParsedSource` then stored both results plus duplicate
source, status, and snapshot fields. This cut deletes:

- `parse_checked` and `parse_status`;
- the `parsed: source::ParsedSource` field and its old parser invocation;
- duplicate `document`, `snapshot`, and `status` storage;
- `incremental::ParsedSource::root` and `typed_tree` detached readers;
- the old `ParseError` diagnostic projection and the integration test that
  required it;
- test-only top-level/diagnostic limit fields that existed only to drive the
  deleted parser; and
- unused `HirIdentityKind`, which has no accepted final-contract role or live
  consumer.

The incremental result now owns one `Rc<BoundParsedSource>`. Its source
snapshot, exact `Arc<SourceDocument>`, recovery status, attached syntax, and
private revision-bound diagnostics all derive from that same accepted grammar
transaction. No attached-to-detached adapter, compatibility alias, or second
diagnostic conversion was added.

The standalone old `source::ParsedSource`, detached `TypedSyntaxTree`, and
workspace consumers remain the current production authority outside the
incremental transaction. They must be deleted when attached handles, HIR
arenas, project/compiler, LSP, formatter, CLI, and Agent consumers switch in
one compiling public authority migration. This cut deliberately does not
publish a partial attached tree reader.

## Prefix-depth authority transfer

Deleting `parse_checked` would previously have deleted the fatal prefix-depth
limit because only the old parser's statistic enforced it. The shared grammar
budget now owns that invariant directly:

- depth is the number of active prefix-expression ancestors on one typed
  expression path;
- enter occurs before the prefix node event, so the 65th ancestor never enters
  the staged event vector;
- leave is stack-like and occurs even when a deeper fatal limit has already
  doomed the transaction;
- the 65th entry stops recursive descent and consumes the doomed expression,
  preventing unbounded prefix input from overflowing the Rust call stack;
- parentheses, calls, binary recursion, and sibling expressions consume no
  level but retain any active prefix ancestors around them; and
- `try await expr` and `await? expr` each emit one propagating
  `AwaitExpression` and consume one prefix level. Explicit `try (await expr)`
  retains separate Try and Await nodes.

The provisional shadow-only unary `+` and brace-less `thread` prefix branches
were removed instead of being admitted into the final limit definition.

The old recovered `ExpressionPrefixDepthLimit` diagnostic remains only in the
standalone production parser until that parser is deleted by the public
authority switch. Incremental fatal limits no longer depend on or project that
diagnostic.

## Validation

Completed after the deletion:

```text
cargo test -p arcweft-lang-syntax --all-targets
  PASS: 473 unit tests and all integration/compile-fail tests
cargo test -p arcweft-lang-syntax --lib incremental::database::tests -- --nocapture
  PASS: 36 passed, 0 failed
cargo test -p arcweft-lang-hir --test public_api
  PASS: all 5 compile-fail cases
cargo check -p arcweft-lang-syntax --all-targets
  PASS
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets --all-features -- -D warnings
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
cargo fmt --all -- --check
  PASS
git diff --check
  PASS
```

The first attempt to replace the deleted test-only top-level limit with a full
16,384-item reparse was manually terminated after more than one minute. That
test was not retained: it duplicated the direct `GrammarBudget` exact/one-over
evidence and made the fast suite depend on a large reconciliation workload.
The terminated child briefly held the Windows test executable, causing one
`LNK1104`; the exact residual test process was stopped and the complete 36-test
database suite then passed.

`just test-workspace` reached the inherited Proof migration gate after 488.7
seconds. All preceding suites, including the syntax/HIR tests and compile-fail
matrices, passed. The only failures were the two already-recorded CLI fixture
failures:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

An exact focused rerun of
`cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
confirmed 3 passed and only those 2 failed. They are unchanged from the
pre-existing Proof gate recorded by the preceding obsolete-identity deletion
cut. This slice does not repair or preserve the detached source authority to
make the fixtures pass; the public `ParsedSource` consumer switch remains their
owning migration.

Tier 2 is not applicable to this cut. It changes no runtime, render, Agent,
MCP, capture, or corresponding public transport path.

## Structural audit

The audit used parent Git revision `64f9649acfe6d2fff77278bde6b50d7e546eb816`
and working change `olmrntxy`.

```text
cargo +nightly -Zscript tools/structure-audit.rs --root .
files scanned: 3667
Rust files: 1936
Rust physical LOC: 906931
package manifests: 94
violations: 0 error(s), 146 warning(s)
```

Reports are retained under
`docs/implementation/structure-audits/proof-stage3-incremental-dual-parser-deletion-2026-07-25/`.
The dependency graph has 13 incoming and 8 outgoing edges for
`arcweft-lang-syntax`, and 11 incoming and 5 outgoing edges for
`arcweft-lang-hir`.

| Path | Bytes | Physical LOC | Classification | Responsibility |
|---|---:|---:|---|---|
| `crates/arcweft-lang-hir/src/identity.rs` | 10,085 | 347 | production | retained HIR identity vocabulary after obsolete enum deletion |
| `crates/arcweft-lang-syntax/src/grammar/budget.rs` | 14,995 | 432 | production | deterministic grammar allocation and active prefix-depth budget |
| `crates/arcweft-lang-syntax/src/incremental/bound.rs` | 8,814 | 298 | production | sole incremental source/snapshot/diagnostic product |
| `crates/arcweft-lang-syntax/src/incremental/database.rs` | 15,528 | 485 | production | atomic incremental transaction without detached parse |
| `crates/arcweft-lang-syntax/src/incremental/database_tests.rs` | 61,435 | 1,694 | unit test | rollback, reconciliation, diagnostic, fragment, and limit evidence |
| `crates/arcweft-lang-syntax/src/incremental.rs` | 295 | 13 | production facade | incremental public surface after detached-reader deletion |
| `crates/arcweft-lang-syntax/src/parser/document.rs` | 32,542 | 1,007 | production | shared lexer/event cursor and grammar transaction |
| `crates/arcweft-lang-syntax/src/parser/expression.rs` | 22,916 | 702 | production | typed Pratt grammar and final prefix/await accounting |

No changed production file exceeds the 1,200-LOC warning threshold, and the
unit-test module remains below the 2,500-LOC test warning threshold. No Cargo
dependency, feature, crate boundary, or compatibility surface was added.

## Remaining Proof Stage 3 boundary

The next deletion-driven authority switch must migrate consumers rather than
repair the old reader:

1. finish the final attached `ParsedSource`/handle/accessor API and fragment
   completion/attachment contract;
2. delete old detached source/HIR entry points first in the migration working
   change and use the resulting compile errors as the consumer inventory;
3. move compiler/project-loader, LSP, formatter/tooling, CLI/REPL/Agent, and
   tests to the same bound source and qualified `SyntaxNodeId` identities; and
4. publish the new reader only when the old `TypedSyntaxTree`, source reparse,
   detached fragments, and linked/cloned HIR owners are absent from that same
   compiling cut.

Final Proof substrate such as `SyntheticKey`, assertion/reference carriers,
`CheckedAssertion`, typed IDs, and runtime assertion identity is retained for
its specified Stage 5-7 migration and is not deleted merely because a current
consumer has not yet switched.
