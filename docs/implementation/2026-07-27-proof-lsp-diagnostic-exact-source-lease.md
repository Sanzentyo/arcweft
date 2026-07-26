# Proof LSP diagnostic exact-source lease deletion

Date: 2026-07-27
Status: `IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`
Parent Git revision: `dfc004e55b64e05a1582a3a4d0b9ac7000799558`
Jujutsu change: `pmvqzkulurrn`

## Boundary and package precedence

This is a deletion-driven preparatory cut for Proof-concurrency v6.1.1. It
does not claim the public attached-syntax or final HIR authority switch.

The returned Proof `01.1.1.4` archive remains rejected under
[`2026-07-26-proof-01.1.1.4-return-intake.md`](2026-07-26-proof-01.1.1.4-return-intake.md).
Its final expression, literal, call, Thread, Dialogue, and RichText payloads
are not inferred here. The independently throwable complete-redelivery request
is
[`2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

All 29 ZIP archives retained under `docs/reviews/` were rehashed at this push
cut. Every SHA-256 has a case-insensitive match in a package-specific
implementation intake or completion note. No unclassified inbox archive was
found. TTS production remains skipped under its existing intake record.

## Deleted authority

LSP diagnostic analysis previously discarded the open snapshot's exact
`Arc<SourceDocument>` and created a second generated source identity by calling
`parse_source(text.to_owned())`. Its cache then stored a duplicate
`SourceRevision` and rehashed `snapshot.text()` on every lookup. Equal content
could therefore appear to validate an analysis that did not retain the exact
source lease used by the editor snapshot.

This cut deletes:

- public `DocumentAnalysis::analyze(&str, ...)`;
- the diagnostic path's source-free `parse_source` call and generated document
  identity;
- `DocumentAnalysis::source_revision()` and its duplicate revision field;
- `CachedDocumentAnalysis::revision`; and
- cache-time `SourceRevision::for_utf8(snapshot.text())` recomputation.

`DocumentAnalysis::analyze_snapshot` now passes the snapshot's exact
`Arc<SourceDocument>` and cloned source-aware `LineIndex` into one private
analysis entry. Parsing uses `parse_document_with_source` with that same Arc,
and syntax linting, old HIR lowering, semantic checking, verification, and
diagnostic projection all receive the same document identity. The analysis
retains the Arc for its entire lifetime.

The cache admits a hit only when document version, accepted profile generation,
and `Arc::ptr_eq` source lease all match. A newly allocated document with the
same URI, version, and bytes is a miss. There is no hash fallback, compatibility
reader, wrapper, source gate, or raw-text adapter.

The removed public raw-text API is enforced through an ordinary Rust
compile-fail test. This is typed public-API evidence, not a scan for source
spellings or file locations.

## Intentionally retained boundary

Diagnostics still projects the standalone parser's `typed_tree()` through
`lower_document_to_hir`, semantic analysis, and verification. That reader is
the current production semantic authority until the corrected final HIR leaf
contract and the attached syntax/HIR database switch can land together.

This cut therefore does not:

- publish private attached syntax handles or qualified HIR arenas;
- adapt attached nodes back into detached `TypedSyntaxTree` values;
- delete `lower_document_to_hir` or the old semantic readers prematurely;
- migrate actions, hover, or Character definition through a partial second
  semantic authority;
- add runtime assertion fields, AWBC assertion payloads, or save/replay fields;
  or
- restore linked/cloned HIR, raw syntax readers, compatibility aliases, dual
  readers, removed-syntax diagnostics, CSS/Takumi, or source gates.

Runtime assertion site/inventory/fault and its AWBC/bundle/cache identity remain
one later authority switch after final `StmtId`/`ExprId`, project publication,
and runtime-plan identity exist. Save, checkpoint, and replay currently have no
independent assertion payload owner, so no speculative persistence field is
introduced.

## Validation

Passed on the final checkout:

- `cargo fmt --all -- --check`;
- `git diff --check`;
- exact-source focused LSP tests: 18 passed;
- exact semantic-analysis cache lifecycle test: 1 passed;
- raw-text API compile-fail test: 1 passed;
- `cargo test -p arcweft-lsp --all-features`: 212 library tests and all LSP
  integration, compile-fail, and documentation tests passed;
- `cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings`;
- `cargo check --workspace --all-targets --all-features`; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

`just test-workspace` ran through the workspace and the new LSP compile-fail
case successfully. Its only failures were the two inherited Proof public-switch
fixtures already recorded by earlier cuts:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

Both fixtures require capability-owned `FsError` publication from attached
`ExternCapabilityItem` members. Repairing the detached reader, adding a global
fallback, or preserving a second nominal owner would hide the missing Proof
authority and is not part of this cut.

Tier 2 was not run. This changes one LSP source-identity/cache boundary and no
runtime, render, Agent, MCP, or capture path, so it does not meet the Tier 2
risk condition.

## Structural audit

The canonical audit command was:

```text
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-lsp-diagnostic-exact-source-lease-2026-07-27
```

It scanned 3,723 files, 1,944 Rust files, 904,371 physical Rust LOC, and 95
package manifests, with 0 errors and 146 repository-wide warnings. Reports are
retained under
[`structure-audits/proof-lsp-diagnostic-exact-source-lease-2026-07-27/`](structure-audits/proof-lsp-diagnostic-exact-source-lease-2026-07-27/).

`arcweft-lsp` has 34 outgoing and 1 incoming workspace dependency edges. The
only manifest change is the existing workspace `trybuild` dependency added as
a development dependency for the compile-fail evidence; no production
dependency or crate-layer edge was added.

| Path | Classification | Bytes | Physical LOC | Responsibility |
|---|---|---:|---:|---|
| `crates/arcweft-lsp/src/diagnostics.rs` | production plus embedded unit tests | 47,376 | 1,279 | exact source lease, diagnostic pipeline, and its focused unit evidence |
| `crates/arcweft-lsp/src/session.rs` | production | 33,352 | 797 | document/profile session and exact-lease analysis cache |
| `crates/arcweft-lsp/src/session/tests.rs` | unit tests | 71,587 | 2,131 | LSP session and cache lifecycle evidence |
| `crates/arcweft-lsp/tests/public_api.rs` | integration compile-fail harness | 151 | 4 | removed raw-text API boundary |
| `crates/arcweft-lsp/tests/ui/document_analysis_raw_text.rs` | compile-fail input | 260 | 9 | external construction attempt for the deleted API |
| `crates/arcweft-lsp/tests/ui/document_analysis_raw_text.stderr` | compiler diagnostic fixture | 735 | 11 | expected typed API rejection |

`diagnostics.rs` triggers the size and embedded-test warnings. Its production
portion is 498 physical lines and the pre-existing embedded test module is 781
lines; the production responsibility itself remains within the preferred
300-800 LOC range. Moving the entire established test module would create a
large mechanical diff unrelated to this deletion and is deferred to a
test-structure cut rather than mixed into this authority change. The 2,131-line
session test module remains below the 2,500-line integration-test warning
threshold.

## Next Proof boundary

Until Proof `.1.1.4.1` returns, continue only leaf-independent deletion work.
The public attached syntax/HIR/project switch must still delete the old
`TypedSyntaxTree`, source reparsing, old lowerers, linked/cloned HIR, and all
remaining consumers in one compiling authority migration. Runtime assertion
and codec publication must follow final HIR/project identity rather than
freezing provisional site, ordinal, or persistence payloads.
