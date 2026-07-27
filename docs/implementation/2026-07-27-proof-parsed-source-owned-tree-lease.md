# Proof convergence: consuming typed-tree escape deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED`

## Context

Proof-concurrency Stage 3 requires the typed syntax view and its exact
`SourceDocument` revision to remain under one parse-product lease until the
final typed HIR owner consumes them together. After the public raw-text parser
facade was deleted in parent Git commit
`bfe4bb4ee7363e0e81d106efbd728f6c635c8612`,
`ParsedSource::into_typed_tree` remained as the last public operation that
could discard the document, lossless syntax, diagnostics, statistics, and line
index while retaining only the provisional typed tree.

At that parent there was one method definition, one production caller in the
compiler project pipeline, and eleven integration-test calls across seven
syntax test files. No caller required ownership of `TypedSyntaxTree`; every
caller could retain `ParsedSource` and borrow `typed_tree()`.

The corrected Proof `01.1.1.4.1` final leaf/expression contract remains
unavailable. Its latest same-name archive is integrity-valid but explicitly
`NOT_READY`, as recorded in
[`2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md`](2026-07-27-proof-01-1-1-4-1-not-ready-redelivery-intake.md).
This cut therefore changes only lease ownership and does not infer the final
expression arena or leaf schema.

## Deleted authority

- delete `ParsedSource::into_typed_tree` rather than rename or wrap it;
- keep the compiler's `ParsedSource` alive through lint and document-bound HIR
  lowering, borrowing `parsed.document()` and `parsed.typed_tree()` together;
- make syntax test helpers return `ParsedSource`, with each test borrowing its
  tree while the owner remains in scope;
- stop cloning an `EntryDeclItem` out of a discarded parse product;
- add compile-fail evidence that downstream code cannot consume the document
  owner into a detached typed tree; and
- retain no compatibility alias, consuming replacement, tree clone, source
  reparse, source gate, or removed-syntax diagnostic.

## Deliberately retained boundary

`ParsedSource::typed_tree()` remains a borrowed provisional reader. Deleting
that reader requires the final typed HIR expression authority and the same-cut
compiler, project, and LSP consumer switch selected by Proof `01.1.1.4.1`.

The private `parse_source_with_options` item-fragment route also remains. Its
Agent REPL production consumers must move to an attached fragment product in
one compiling authority switch; this cut does not hide the old route behind a
new facade.

## Validation

The implementation is Jujutsu change
`svoxttkukkuuuszvorsvloluzvqnlxxq` over parent Git commit
`bfe4bb4ee7363e0e81d106efbd728f6c635c8612`.

The final checkout passed:

- `cargo fmt --all -- --check` and `git diff --check`;
- all 85 tests in the seven migrated syntax integration suites;
- the syntax public-API trybuild matrix, including the new
  `removed_into_typed_tree.rs` row;
- all syntax targets and features, including 492 library tests and every
  integration/compile-fail suite;
- all compiler targets and features, including 92 library tests and every
  integration/compile-fail suite;
- strict changed-crate Clippy for syntax and compiler;
- `cargo check --workspace --all-targets --all-features`; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

`just test-workspace` ran for 17 minutes 2 seconds. Every workspace, CLI,
integration, and compile-fail component preceding the Arcweft fixture gate
passed. That gate retained its exact existing three-pass/two-fail baseline:

```text
tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

The failing checks are the already recorded external-capability `FsError`
publication gap and do not call the deleted method. The only component skipped
after that expected nonzero exit, the persistent-cache build CLI golden suite,
was run separately and passed both tests.

The canonical structural audit scanned 3,741 files, including 1,947 Rust files
and 906,430 Rust physical LOC. It reported zero errors and 146 existing
warnings. Exact results are retained under
[`structure-audits/proof-parsed-source-owned-tree-lease-2026-07-27/`](structure-audits/proof-parsed-source-owned-tree-lease-2026-07-27/).
The changed production owners remain below audit thresholds:
`arcweft-compiler/src/project.rs` is 36,626 bytes / 1,091 physical LOC and
`arcweft-lang-syntax/src/source.rs` is 7,768 bytes / 242 physical LOC. The cut
adds no dependency edge or production responsibility.

The review-package ledger contains 30 ZIPs; every exact SHA-256 is recorded in
a package-specific implementation note and the unrecorded count is zero.

Tier 2 does not apply. The production compiler performs the same lint and HIR
lowering against the same `Arc<SourceDocument>` and typed tree, but now borrows
both from the retained parse product. No runtime, renderer, Agent, MCP,
capture, persistence, or serialized contract changed.

## Next boundary

Delete the unused owned `Expr` and `Item` payloads from
`ParsedFragmentKind::Expression` and `ParsedFragmentKind::Items`, retaining
only their family/completion evidence. Keep the semantically consumed
`Statements(Vec<Stmt>)`, `parse_fragment`, and the private item parser frozen
until the accepted Proof `01.1.1.4.1` schema enables their atomic attached
fragment/Agent switch. Do not publish the final expression arena or guess
missing semantic leaf payloads before that return is accepted.
