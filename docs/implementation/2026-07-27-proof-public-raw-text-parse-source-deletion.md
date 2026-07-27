# Proof convergence: public raw-text `parse_source` deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED`

## Context

Proof-concurrency Stage 3 requires a full-document parse to retain the exact
`SourceDocument` lineage and revision selected by the caller. The corrected
final HIR leaf package requested by
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md)
has not returned implementation-ready: the latest same-name archive is
integrity-valid but explicitly `NOT_READY` and contains no contract. This cut
therefore removes only the leaf-independent raw-text parse escape hatch. It
does not infer a final expression payload or alter the production fragment
grammar.

At parent Git commit
`61c7fdf444068e75a32eeeaab9721c1d8a4f5eb5`, public
`arcweft_lang_syntax::parser::parse_source` had no production caller. There
were one definition and 379 direct test/fixture invocation lines:

- 137 syntax;
- 63 compiler;
- 50 sema;
- 46 runtime-plan;
- 33 CLI;
- 21 HIR;
- 11 tooling;
- seven LSP;
- five Agent REPL;
- four verifier; and
- one each in project-loader and `arcweft-test`.

The deleted facade accepted only raw text and constructed a content-addressed
`memory:<source-revision>` document ID internally. That made source lineage an
implementation guess and allowed tests to lower a tree against a separately
constructed same-text document.

## Deleted authority and direct migration

- deleted public `parse_source` rather than renaming or wrapping it;
- migrated every test and fixture to construct an explicit stable
  `SourceDocumentId`, `SourceName`, and `Arc<SourceDocument>` before parsing;
- routed each full document directly through
  `parse_document_with_source(Arc<SourceDocument>, ParseOptions)`;
- reused that same `Arc` for HIR lowering, project modules, source maps, and
  diagnostic projection wherever a consumer already owned the document;
- changed multi-module fixtures to parse each module against its own exact
  document instead of reusing one detached same-text typed tree;
- removed derived content-hash and label-derived fixture identities in favor
  of repository-visible logical fixture IDs;
- added compile-fail evidence that downstream code cannot import the deleted
  function; and
- added the `arcweft-source` workspace dev-dependency to `arcweft-test`, whose
  direct test fixture now owns the source boundary instead of relying on an
  identity-fabricating syntax facade.

No compatibility alias, old-name helper, extension trait, dual reader,
source-string reparse, source gate, or removed-syntax-specific diagnostic was
introduced. Private test helpers that remain name and enforce a concrete
fixture domain or expected recovery invariant; none reproduce the deleted
public raw-text API.

## Deliberately retained boundary

`parse_source_with_options` remains private. `parse_fragment` uses it for the
current production item-fragment route, including Agent REPL consumers. The
private fragment path still creates an internal content-addressed document and
must be replaced together with the attached fragment/public typed HIR switch,
not hidden behind another facade in this cut.

`ParsedSource::into_typed_tree` also remains and still has test consumers plus
one compiler project consumer. It is the next independent deletion target:
callers must retain `ParsedSource` and borrow `typed_tree()` so the document
lease cannot be discarded. Final expression-arena allocation and the
production fragment authority remain gated on the corrected `01.1.1.4.1`
contract.

## Validation

The implementation is Jujutsu change
`tttykontnyonnnwqqorrpqpxpkvpqywv` over parent Git commit
`61c7fdf444068e75a32eeeaab9721c1d8a4f5eb5`.

The final checkout passed:

- `cargo fmt --all -- --check`;
- `git diff --check`;
- the syntax public-API trybuild matrix, including
  `tests/ui/removed_parse_source.rs`;
- the syntax lint exact-document regression;
- Agent REPL lib tests, 11 passed;
- tooling style-environment tests, 15 passed;
- LSP lib tests, 212 passed;
- CLI lib tests with all features, 198 passed;
- CLI native-style parity integration, one passed;
- `cargo check --workspace --all-targets --all-features`, successful in
  6 minutes 4 seconds; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  successful after moving repeated exact-document setup into the domain-owned
  compiler runtime-plan and CLI bundle fixture constructors. No lint allow was
  added.

The normal `just test-workspace` command exceeded the 30-minute command-capture
limit while its child process continued. Its exact component commands were
therefore rerun individually with the same checkout and Cargo profiles:

- workspace lib/integration tests excluding CLI: passed in 334.8 seconds;
- CLI lib/bins: 196 passed;
- CLI runtime-native options: three passed;
- CLI core checks: four passed;
- CLI native-style parity: one passed;
- CLI release-trust JSON: five passed;
- CLI responsive-stage placement: four passed;
- CLI persistent-cache goldens: two passed; and
- CLI Arcweft fixtures: three passed and the same two pre-existing capability
  fixtures failed.

The two failures are:

```text
tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

Direct execution of both fixtures reports
`sema.nominal.unknown_type: unknown type FsError` and says that source text is
unavailable. This is the already recorded Proof attached-HIR/external
capability gap; neither fixture used the deleted test-only parser facade, and
the failure is unchanged by this cut. It is not repaired through a raw source
fallback.

The canonical structural audit command scanned 3,738 files, including 1,946
Rust files and 906,385 Rust physical LOC. It reported zero errors and 146
warnings. Exact metrics for all 97 changed Rust files, dependency edges,
duplicate public types, and the warning inventory are retained under
[`structure-audits/proof-public-raw-text-parse-source-deletion-2026-07-27/`](structure-audits/proof-public-raw-text-parse-source-deletion-2026-07-27/).
The largest changed files are existing test owners; the production files above
the warning threshold were changed only in embedded test regions. The cut adds
no production responsibility or crate-layer edge.

The review-package ledger contains 30 ZIPs. Every exact SHA-256 occurs in a
package-specific implementation intake/completion note; the unrecorded count
is zero.

Before the clean rebuild, `cargo clean` safely removed 235,777 files and
352.8 GiB from the resolved workspace-local `D:/git/arcweft/target` after the
path was verified to remain inside the workspace. The target was recreated by
the validation commands and remains useful for the immediately following
Proof deletion cut.

Tier 2 does not apply: the removed public function had no production caller;
all 379 migrated call sites were tests or fixtures; changed `src` call sites
are inside test modules; and no runtime, render, Agent, MCP, capture, or
serialized production contract changed. The production fragment route is
intentionally unchanged.

## Next boundary

Delete `ParsedSource::into_typed_tree` and migrate the remaining consumers to
retain the complete bound parse product. Do not delete or publicize the private
fragment parser until the attached-fragment authority switch can remove its
old reader and all consumers in the same compiling cut.
