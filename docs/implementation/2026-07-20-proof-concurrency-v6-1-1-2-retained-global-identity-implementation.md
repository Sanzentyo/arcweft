# Proof concurrency v6.1.1.2 retained global-identity implementation

## Current reconciled package

The implementation source of truth is
[`docs/reviews/packages/arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip`](../reviews/packages/arcweft-proof-concurrency-v6.1.1.2-retained-global-identity-declaration-grammar-reconciliation-final-contract.zip).
Its SHA-256 is
`0e30a91fa2f7a288e9a12d8afc7356525604cbdc907d659cd97311207d26a68e`.
All 18 archive members match `MANIFEST.txt`, the entries are in lexical order,
and the manifest self-entry uses the required 64-zero rule.

This archive is the latest-`main` reconciliation returned against Git
`3acc9cfec034d00cee173e41cbfb37cd46115c50`. It replaces the earlier accepted
archive with SHA-256
`7be398ebe2cefa2daefa963c7c8c6efb0b2389bb015edf36e585fb8b770242b1`
as the implementation source of truth without adding a compatibility contract.
The earlier digest remains recorded here as historical package provenance.

Intake started from local Git parent
`6b97057a0a430179175682494e07c7529554933b` and Jujutsu working-copy change
`xpzvlyvqvtvowssyxlpswsnpkwnspxqr`. The package itself was designed against
Git `27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9`; implementation reconciles the
contract with newer accepted resource, callable, Character, View, and project
source-index work rather than restoring an older shape.

## Completion contract

The package contains 184 normative direct-test rows. Completion requires all
of the following:

1. retain `asset` as a catalog/reference family with no authored declaration;
2. implement private one-pass typed grammar rows for `character`, `view`,
   `action`, `activity`, `signal`, `metric`, and `layer`;
3. close their success, lossless, malformed, recovery, ambiguity, and inclusive
   budget tests before changing the public AST;
4. atomically replace the generic/stringly public entity declaration path with
   seven attached typed declarations;
5. lower attached declarations into arena-owned HIR and the single project
   symbol authority without cloning or reparsing source strings;
6. migrate Character, View callable, Action, Activity, Signal, Metric, Layer,
   formatter, LSP, CLI, Agent, runtime-plan, bundle, and manifest consumers;
7. delete the generic entity AST/HIR, raw signature/body storage, cloned View
   callable projection, and all removed-family readers; and
8. pass focused tests, stable-feature workspace check and strict Clippy,
   formatter, workspace suite, structural audit, and affected Tier 2 tests.

`res` remains the independent configured-resource declaration. No generic
`entity` declaration, authored `asset`, compatibility reader, removed-spelling
diagnostic, CSS/Takumi route, or source gate is permitted.

## Ordered implementation cuts

| Cut | Scope | Status |
| --- | --- | --- |
| 0 | package integrity and production reconciliation | complete |
| 1 | owned identity vocabulary, shared private header nodes/roles/limits, classification inventory | complete |
| 2 | private Character and Action grammar plus direct tests | complete |
| 3 | private Signal and Metric grammar plus direct tests | complete |
| 4 | private Activity grammar plus direct tests | complete |
| 5 | private Layer grammar plus direct tests | complete |
| 6 | private View grammar integration with typed common expression descendants | complete |
| 7 | complete reduced Stage 1 declaration inventory gate | complete |
| 8 | atomic attached public AST switch and generic entity deletion | pending |
| 9 | typed HIR/project-symbol and downstream migration | pending |
| 10 | docs/examples/fixtures and obsolete-path deletion | pending |
| 11 | full validation, structural audit, Tier 2, commit/push cleanup | pending |

## 2026-07-23 latest-main reconciliation

The returned latest-`main` contract was reconciled at Git
`3acc9cfec034d00cee173e41cbfb37cd46115c50` and Jujutsu working-copy change
`vszsuyoznmpkzsrxotpwulouvowktqro`. The independently safe owner correction is
implemented without starting the atomic public syntax/HIR switch:

- `RetainedIdentityFamily::from_prefix` now belongs to the original owned enum;
- `AssetVirtualPath` validates normalized relative `/`-separated catalog paths;
- `AssetId` owns exact path-to-public-ID derivation and typed failures;
- Layer reference-family recognition uses the owned family API rather than a
  second prefix table; and
- CLI bundle image admission consumes `AssetVirtualPath`/`AssetId`, rejects an
  invalid identity instead of silently omitting the file, and transactionally
  rejects two image paths that normalize to one asset ID.

This does not claim the final project-wide asset catalog from Cut 7. It moves
the existing image-bundle caller to the final identity owner and leaves
filesystem enumeration and bytes in the CLI/build adapter as required.

The public switch remains sequencing-blocked rather than design-blocked. A
one-off planning inventory in this checkout still found `.typed_tree()` in 90
Rust files, `TypedSyntaxTree` in 20, `lower_to_hir` in 59, `SourceItem` in 8,
`EntityDeclItem` in 13, and `HirTopLevelDecl::EntityDecl` in 13. These counts
are review evidence only and are not a source gate. Cut 8 must still replace
the complete-document authority atomically; implementing one retained public
wrapper early would create the prohibited dual reader. Cuts 9–11 therefore
remain pending.

### Focused validation

- `cargo test -p arcweft-id --lib --tests`: 20 unit tests and 1 public-API
  trybuild test passed;
- `cargo test -p arcweft-cli --lib collect_bundle_image_assets --all-features`:
  3 focused tests passed, including invalid identity and dash/underscore
  normalized collision; after adding case/extension collision to the same test,
  its repeated CLI test-harness link was stopped at the coordinating agent's
  request to release the shared Cargo target for the concurrent package;
- `cargo test -p arcweft-lang-syntax --lib --all-features`: all 452 tests
  passed, including the seven retained private grammars and their limits;
- `cargo clippy -p arcweft-id -p arcweft-lang-syntax --all-targets
  --all-features -- -D warnings`: passed;
- `cargo clippy -p arcweft-cli --all-targets --all-features -- -D warnings`:
  blocked by nine warnings in the concurrently changing sema environment
  publication projection, outside this package slice, before Clippy reached
  the CLI crate;
- the same CLI Clippy gate with `--no-deps` was retried without changing the
  feature set, but a concurrent adapter-context registration-source compile
  error and unused import still stopped it before the CLI crate;
- `cargo fmt --all -- --check`: this slice is formatted, but the workspace
  check reported diffs in concurrently changing adapter-context registration
  input files;
- direct stable `rustfmt --check` over this slice's four Rust files: passed;
- `git diff --check` over the five edited text files: passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: scanned 3,595
  files, 1,893 Rust files, 879,825 physical Rust lines, and 94 manifests; it
  reported 0 errors and 138 warnings and wrote no report in dry-run mode.

### Structural measurement

| Path | Crate / role | Bytes | Physical LOC | Responsibility |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-id/src/lib.rs` | `arcweft-id`, production | 18,148 | 597 | owned IDs, retained family vocabulary, asset path/identity |
| `crates/arcweft-lang-syntax/src/parser/layer_grammar.rs` | `arcweft-lang-syntax`, production | 13,997 | 434 | private lossless Layer grammar and recovery |
| `crates/arcweft-cli/src/app/bundle.rs` | `arcweft-cli`, production | 50,134 | 1,396 | bundle command orchestration, virtual files, typed asset admission |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | `arcweft-cli`, unit tests | 89,434 | 2,764 | bundle command and product acceptance tests |

Cargo metadata reports fan-in/fan-out of 24/4 for `arcweft-id`, 13/8 for
`arcweft-lang-syntax`, and 0/69 for the application-level `arcweft-cli` crate.
The CLI production and test modules remain above warning thresholds, but this
slice removes the local identity helpers and adds only the two direct behavior
tests; it does not add a new responsibility or cross an error threshold. Their
broader decomposition remains part of the repository structural-warning
backlog rather than a reason to move asset identity back into CLI.

## Current evidence

The complete seven-row private inventory now emits directly through
`ShadowDocumentParser` and `GrammarBudget`. Character and Action own typed
headers/body or signatures; Signal owns a typed common Type child whose closed
observable head/arity is intentionally deferred to sema; Metric owns typed
kind, value type, unit, labels, and buckets; Activity owns abstract policies,
ports, and contracts; Layer owns typed singleton members, family-checked
references, and closed policy values; and View owns one fixed signature, a
leading export block, and a typed common-expression fragment. The private
classifier maps removed
`asset`, `content`, `extern mod`, `dialogue defaults`, `source`, `state`, and
regular top-level statements to ordinary `ErrorItem` recovery.

Direct tests cover canonical and malformed rows, all seven shared-header
missing/wrong-family/relative-ID/keyword-name cases, sibling preservation,
prefix attachment, LF/CRLF/Unicode losslessness, mixed documents, and every
new narrow inclusive budget. The Stage 1 close-out also directly exercises the
inclusive global limits for 16,384 top-level items, 1,048,576 identity-bearing
nodes, and 1,024 diagnostics, including one-over exhaustion and fresh-budget
recovery. Duplicate declarations and sections now retain exact first and
duplicate ranges; malformed Action and View signatures retain exact recovery
ranges; View values retain typed common-expression descendants or an
`ErrorExpression`; and dotted namespace calls at the top level recover as
ordinary `ErrorItem` nodes rather than being misclassified as declarations.

`cargo test -p arcweft-id` passed 6 tests, and the latest
`cargo test -p arcweft-lang-syntax --lib` passed all 373 tests. The final
Activity header recovery uses the generic current-grammar
`syntax.declaration.unexpected_header` diagnostic. The temporary
concrete-origin spelling recognizer, its dedicated diagnostic code, and its
spelling-specific test have been removed as required by the repository-wide
removed-syntax policy. The Stage 1 close-out reran the all-targets strict
Clippy gate for `arcweft-id` and `arcweft-lang-syntax` with `-D warnings`
successfully. The repository structural audit scanned 3,404 files and reported
0 errors and 131 warnings. No public syntax reader has been switched,
preserving exactly one public reader until the atomic public AST cut.

As a prerequisite to Cut 8, the duplicate public View callable projection has
been removed. A View declaration now contributes its callable symbol directly
from the same `EntityDeclItem` that owns the structured View body; syntax no
longer exposes `CallableItem`/`CallableKind`, HIR no longer clones a second
`HirTopLevelDecl::Callable`, and project indexing registers the View callable
facet from that single declaration. This is not the attached-AST switch:
`EntityDeclItem` and its raw signature tail still remain, so Cut 8 stays
pending. The change closes the package's single-owner invariant without
introducing another parser or compatibility carrier.

The single-owner prerequisite passed the syntax public-API and View-callable
tests, including compile-fail coverage for the removed projection; the HIR
View-callable test; and all 15 project-index tests. Strict all-target,
all-feature Clippy passed for syntax, HIR, sema, LSP, and tooling. Formatting
and `git diff --check` passed. The review-cut structural audit scanned 3,414
files and reported 0 errors and 131 existing warnings.

Further validation results will be recorded here as later cuts close. Passing
this prerequisite does not complete the package.
