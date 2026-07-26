# Proof v6.1.1 source-free fragment and exact attachment

Date: 2026-07-27

Status: `IMPLEMENTED_PRIVATE_SUBSTRATE`

## Contract and returned-package boundary

This cut implements the standalone-fragment boundary from the accepted
Proof-concurrency v6.1.1 package
[`arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip),
SHA-256
`1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`.

The archive supplied on 2026-07-27 as a possible Proof `01.1.1.4.1`
redelivery has SHA-256
`414F95F8EF4C5F3ABCCE163F0C9B01F124098F0BAC856F174AF09B5C1E7D564B`.
It is byte-identical to the rejected `01.1.1.4` return recorded in
[`2026-07-26-proof-01.1.1.4-return-intake.md`](2026-07-26-proof-01.1.1.4-return-intake.md),
so it does not authorize the final HIR leaf-expression schema. The active
request remains
[`01.1.1.4.1`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).
This fragment slice depends only on the already accepted syntax identity,
event, source-span, and attachment contract and does not freeze any rejected
HIR payload.

## Deletion-driven replacement

The cut deletes the private predecessor rather than wrapping or repairing it:

- `ShadowFragmentKind` and `parse_shadow_fragment`;
- the document-range `DocumentLexer::for_range` entry point;
- `BoundFragment<K>`, its four aliases and marker inventory, and its range
  projection helpers;
- all four `parse_bound_*_fragment` database entry points; and
- attachment-time parsing of the target document span.

The replacement has exactly four crate-private standalone families:
expression, type, pattern, and one ordinary statement. Parsing owns an exact
`Arc<str>`, one validated fragment-relative grammar event tree, structured
diagnostics, and grammar-derived `ParseCompletion`. It owns no source identity.

Explicit attachment then validates, before identity allocation:

1. the snapshot name, exact document identity, UTF-8 span, and range;
2. `ParseCompletion::Complete`; and
3. byte equality between the retained fragment and the target span.

The retained event transaction is checked-rebased into the target document,
with document prefix/suffix represented only as lossless root text. The
fragment EOF is replaced by the document EOF, all token, missing-token,
primary-diagnostic, and related-diagnostic coordinates are rebased with
checked arithmetic, and one fresh syntax lineage is committed only after
grammar construction, identity allocation, typed attachment, and family-root
validation all succeed. Attachment performs no parse.

## Direct evidence

Tests cover:

- complete and incomplete source-free expression parsing and exact/one-over
  prefix-depth limits;
- all four exact entry points and typed family-root predicates;
- grammar-owned incomplete and invalid completion without copying the legacy
  REPL's text heuristics;
- embedded exact-byte attachment for all four families, absolute root ranges,
  full-document losslessness, and fresh lineages;
- incomplete, invalid, byte-mismatched, and foreign-span rejection before
  lineage allocation;
- injected attachment failure rollback followed by control-slot equality;
- exact event-coordinate rebasing and overflow rejection; and
- retention of the exact validated source-free event transaction.

No source gate, compatibility alias, detached projection, item fragment,
statement-list fragment, substring reparse, or removed-syntax diagnostic was
added.

## Deliberate private boundary

The existing public `parser::fragment` API and Agent REPL remain the sole
public fragment authority until the atomic public syntax/HIR/tooling switch.
They are frozen in this cut: no new caller is routed through them and none of
their heuristics are copied into the new parser.

Publishing `ParseFailure::Attachment(AttachmentFailure)` now would also expose
the still-private attached-syntax identity vocabulary. Therefore this cut
keeps the final validation behavior behind a crate-private error projection.
The public error variant, public fragment exports, REPL synthetic-document
migration, and compile-fail evidence that fragments cannot satisfy a whole-file
HIR request remain one coherent part of the later atomic switch. This is an
explicit publication boundary, not a second reader or compatibility shim.

## Verification

Validation results for the final checkout are recorded below:

- `cargo test -p arcweft-lang-syntax parser::unbound_fragment --no-fail-fast`:
  passed, 5 focused tests;
- `cargo test -p arcweft-lang-syntax incremental::database::tests --no-fail-fast`:
  passed, 32 focused transaction tests;
- `cargo test -p arcweft-lang-syntax --all-features --no-fail-fast`:
  passed, including 492 unit tests, all integration and UI compile-fail tests,
  and 3 documentation tests;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features --
  -D warnings`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check` and `git diff --check`: passed;
- `just test-workspace`: did not complete. Its first command,
  `cargo test --workspace --lib --tests --exclude arcweft-cli --quiet`,
  remained active without terminal output for more than 20 minutes and was
  stopped together with only its verified descendant process tree. It did not
  reach the later CLI fixture commands. This broad-gate timeout is not counted
  as a pass; the changed syntax crate's complete suite and every workspace
  compile/Clippy gate above passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-unbound-fragment-exact-attachment-2026-07-27`:
  passed with 3,720 files scanned, 1,942 Rust files, 904,296 Rust physical LOC,
  0 errors, and 146 workspace warnings.

The structural audit was run on Jujutsu change `mnqpsrnp`. The largest changed
test file is `incremental/database_tests.rs` at 54,373 bytes and 1,601 physical
LOC, below the 2,500-LOC integration-test warning threshold. The new production
owner `parser/unbound_fragment.rs` is 14,032 bytes and 444 physical LOC;
`incremental/transaction.rs` is 11,612 bytes and 367 physical LOC. The audit's
changed-file warnings are inherited owners: `attachment.rs` is 52,756 bytes and
1,500 physical LOC, and `grammar/kinds.rs` is 39,580 bytes and 1,202 physical
LOC. This cut adds only the fragment family markers to the former and exact
token display spellings to the latter; it does not add a new subsystem to
either owner. The complete machine-readable measurements and warnings are in
[`proof-unbound-fragment-exact-attachment-2026-07-27`](structure-audits/proof-unbound-fragment-exact-attachment-2026-07-27/).

An independent read-only review found no blocker and confirmed that parsing is
source-free, attachment validates the exact source/span/completion/bytes before
staging, the transaction projects retained events without parsing, lineage is
committed only after successful validation, and the new API remains
crate-private.

Tier 2 is not required for this private syntax-and-attachment-only cut. It
changes no public contract and reaches no runtime, renderer, Agent, MCP, or
capture path. The later public consumer switch remains Tier 2.
