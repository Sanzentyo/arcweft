# View exported-part production contract correction

- Date: 2026-07-16
- Package: `arcweft-seq-06.11d.2.1.1.1.1-view-exported-part-production-contract-correction-final-contract.zip`
- Package SHA-256: `b4662f3ecd79c157ee93656a173e9809fff31696aaded1fedb9411cdb1e9732e`
- Package basis: Git `8984661d5679efccf7a16255f921530cd0b7cacc`
- Production base before the final rebase: Git `14e9a0440229`
- Working change: Jujutsu change `vmvymkzz`
- Status: Complete; all eight increments are implemented and focused final-cut
  validation has passed

## Package intake

All twelve archive members were read before production changes. The archive
digest above was recomputed from the provided ZIP. The package has no open
result-changing decisions and requires eight small compiling increments. This
note records the complete correction and does not redefine completion around an
easier subset.

The source-bound exported-part parser, HIR, semantic checking, program builder,
and ordinary removed-syntax recovery from the preceding reconciliation remain
intact. No compatibility alias, dual reader, old-spelling recognizer, or source
gate was introduced.

## Increment 1 implementation

The numeric identity that previously served incompatible semantic and registry
roles is split into:

- `ViewId(PublicId)` for stable authored/public View ownership;
- `ViewProgramId(PublicId)` for stable accepted-program identity;
- `AcceptedViewProgramRevision([u8; 32])` with nonzero, canonical lowercase-hex
  serialization; and
- opaque, non-serializable `ViewRegistryId(u32)` for process-local dense slots.

`arcweft-view::view` is now a small facade over dedicated `identity` and
`registry` responsibility modules. Entity ownership and private fragment/host
references use `ViewRegistryId`; authored `ViewCall` and `ViewProgram` values use
`ViewId`; and value-program snapshots use `ViewProgramId` instead of a numeric
definition position. Manual Serde exists only for stable identities and the
accepted revision. A compile-fail rustdoc proves that `ViewRegistryId` does not
implement `Serialize`.

Registry descriptors have private fields and explicit anonymous-Rust,
public-Rust, and Arcweft construction paths. Registration validates duplicate
public identity and capacity before mutation. Retired Arcweft slots become
permanent tombstones and are never reused. Focused tests cover anonymous/public
capabilities, candidate-first duplicate rejection, stable string codecs,
revision codecs, and stale-slot non-aliasing.

Runtime mount creation and restore now retain the accepted stable
`ViewProgramId`; they no longer synthesize program identity from a definition's
dense vector index. The temporary product resource still carries a string until
the package's product/catalog increments replace that boundary.

## Minimal API deviation

The package lists `ViewDescriptor::arcweft` and `ViewRegistry::retire_arcweft`
as crate-private inside `arcweft-view`, while replacement orchestration is owned
by the separate `arcweft-runtime-driver` crate. A crate-private constructor
cannot be called across that boundary.

The descriptor constructor and dense slot/index access remain crate-private.
`ViewRegistry` instead exposes registry-owned typed `register_arcweft` and
`retire_arcweft` operations that accept stable `ViewId`/`ViewProgramId` values
and neither require nor return a dense slot. This is the smallest dependency-
correct adaptation and does not create a compatibility surface.

## Verification evidence

All Cargo commands use `CARGO_INCREMENTAL=0`.

- `cargo fmt --all` — passed;
- `cargo fmt --all -- --check` — passed;
- `cargo test -p arcweft-view --lib` — 56 passed;
- `cargo test -p arcweft-view --doc` — two compile-fail rustdocs passed,
  including the non-serializable dense registry slot contract;
- `cargo check -p arcweft-runtime-driver --lib` — passed;
- `cargo clippy -p arcweft-view --all-targets --all-features -- -D warnings`
  — passed;
- `cargo clippy -p arcweft-runtime-driver --lib --all-features -- -D warnings`
  — passed;
- the focused `ViewMountState` snapshot/restore unit test — passed;
- the exact runtime nested-mount round-trip integration test — passed. The
  command wrapper reached its timeout immediately after Cargo printed the
  successful one-test result; no Cargo or rustc process remained;
- `cargo test -p arcweft-runtime-driver --test view_runtime` — 11 passed and
  four pre-existing fixture-validation failures. Three fixtures omit required
  `ViewTextBlockResource` owners and the standard dialogue fixture uses a
  resource identity rejected by the existing canonical encoder. These failures
  occur in the pre-existing `encode_canonical_section` call before the new typed
  program-ID adaptation. The changed nested-mount snapshot/restore case passed.

## Structural audit

The canonical audit ran after rebasing the increment onto Git
`8a6d4a62a138`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

It scanned 3,031 files, 1,506 Rust files, 694,449 Rust physical LOC, and 90
package manifests. The result was 0 errors and 128 repository-wide warnings.
Exact reports are checked in under
`docs/implementation/structure-audits/view-exported-part-correction-increment1-2026-07-17/`.

Exact current-checkout metrics for every changed Rust file are:

| Path | Bytes | Physical LOC | Role | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 81,579 | 2,545 | integration fixture adaptation | n/a |
| `crates/arcweft-runtime-driver/src/view_runtime/evaluator.rs` | 63,540 | 1,553 | View evaluation/mount construction | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/part.rs` | 2,289 | 74 | accepted program/part boundary | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 41,154 | 1,068 | runtime orchestration/snapshot restore | 0 |
| `crates/arcweft-view/src/entity.rs` | 6,861 | 242 | entity ownership slots | 0 |
| `crates/arcweft-view/src/lib.rs` | 7,669 | 166 | crate facade | 2 |
| `crates/arcweft-view/src/program.rs` | 33,131 | 979 | authored program/builder | 244 |
| `crates/arcweft-view/src/tests/display_frame.rs` | 7,150 | 238 | display tests | n/a |
| `crates/arcweft-view/src/tests/entity_reactive_registry.rs` | 7,462 | 240 | entity/registry tests | n/a |
| `crates/arcweft-view/src/tests/fragment_layout.rs` | 5,970 | 212 | fragment/layout tests | n/a |
| `crates/arcweft-view/src/value_program.rs` | 26,986 | 849 | value program/mount snapshots | 280 |
| `crates/arcweft-view/src/view/identity.rs` | 6,551 | 229 | stable identities/codecs | 54 |
| `crates/arcweft-view/src/view/registry.rs` | 8,456 | 271 | dense registry/descriptors | 87 |
| `crates/arcweft-view/src/view.rs` | 3,092 | 96 | responsibility facade/host IDs | 0 |

The largest current workspace Rust files are test files: CLI runtime bench
(256,474 bytes/7,970 LOC), native vertical observe (238,805/6,620), published
JLREQ class mix (220,473/6,109), native samples/effects (214,731/5,850), and
compiler tests (180,052/5,363). They are unchanged by this increment.

`arcweft-view` has 12 dependency fan-in edges and eight fan-out edges. The one
new edge is the package-authorized normal dependency on `blake3`; it will own
the canonical accepted-revision transcript in Increment 6. The new identity
and registry modules remain below the ordinary 300-800 LOC responsibility
target. The adapted runtime evaluator and CLI test file remain warning-level
legacy hotspots, but this increment changes only their ID-boundary call sites
and adds no new responsibility to them.

## Post-rebase verification

After rebasing onto Git `8a6d4a62a138`, the increment was verified again with
`CARGO_INCREMENTAL=0`:

- `cargo fmt --all -- --check` — passed;
- `cargo test -p arcweft-view --lib` — 56 passed;
- `cargo test -p arcweft-view --doc` — two compile-fail rustdocs passed;
- the exact runtime nested-mount round-trip integration test — passed; and
- `cargo clippy -p arcweft-view -p arcweft-runtime-driver --all-targets
  --all-features -- -D warnings` — passed.

## Increment 3 implementation

The bundle product boundary now owns one complete multi-source product rather
than a selected `BundleSource`. Every `ViewProgramResource` and
`ViewStyleResource` carries a canonical table of `ProductSourceRef` values;
all stored ranges point through an opaque product-local source index and are
rebased whenever resources are merged. The old `BundleSource`,
`SourceMapIndex`, `SourceMapSourceId`, manifest `source_label`, and
single-document decode projection have been deleted rather than retained as
compatibility readers.

`ValidatedViewProduct` is the only public proof that a View program and its
`SourceMapSection` agree. Candidate-first validation checks every referenced
source identity, revision, extent, range order, UTF-8 boundary, containment,
cross-source relation, and exact shared limit before exposing the validated
program. AWFB decode and bundle validation construct that typestate instead of
accepting a source-bearing product piecemeal. Compiler and CLI lowering now
carry `SourceDocument`/`SourceSpan` ownership from the loaded project source
map, including style environment guards and exported-part ranges; no path or
source string is reparsed to reconstruct provenance.

The standard dialogue program uses the valid public identity
`view.standard.dialogue.program`. This is a correction of an unpublished
reserved-prefix value, not an alias or dual spelling.

Increment 3 verification on Jujutsu change `lktlozot`:

- `cargo fmt --all -- --check` — passed;
- `cargo check --workspace --all-targets` — passed;
- `cargo test -p arcweft-bundle --tests` — every unit and integration target
  passed, including five complete-product acceptance/negative/limit tests;
- `cargo test -p arcweft-compiler --test style` — four passed;
- the focused CLI authored exported-part lowering test — passed; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — scanned 3,083
  files, 1,545 Rust files, and 708,347 Rust physical LOC with 0 errors and 128
  repository-wide warnings.

## Increment 4 implementation

The accepted runtime boundary now preserves program and definition ownership
as `ViewProgramId`, `ViewDefinitionRef`, and `ViewId`; definition and call
targets are no longer reparsed from strings. `ViewProgramCatalog` is built
fallibly from an owned `ValidatedViewProduct`, sorts definitions by semantic
`ViewId`, allocates only crate-private `ViewDefinitionIndex` values, and owns
the single typed semantic-program and exported-part catalog used by execution.
The former raw `ViewProgramResource` runtime constructor and the duplicate
string-indexed definition/part maps are deleted.

Runtime construction first clones or consumes a candidate `ViewRegistry`,
preserves anonymous and public Rust descriptors, then registers every Arcweft
definition with its exact schema and program identity. A public-owner
collision rejects the candidate before any runtime is published. Engine-owned
reserved View identities have an explicit checked constructor; authored
`PublicId::try_new` remains unable to create the reserved namespace, while
standard product decode and save-oriented semantic identity can represent it.

Increment 4 verification after rebasing onto Git `06e502403861`:

- `cargo fmt --all -- --check` — passed;
- focused `arcweft-id`, `arcweft-view`, bundle product/codec/standard-dialogue,
  and runtime View suites — all passed, including 17 runtime integration tests;
- `cargo clippy -p arcweft-id -p arcweft-view -p arcweft-bundle --all-targets
  --all-features -- -D warnings` — passed, including the six reported
  Increment 3 bundle findings;
- runtime-driver all-target/all-feature clippy passed for this slice while
  exempting the pre-existing `session/hot_swap.rs` `assigning_clones` finding;
  no View warning was exempted; and
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — scanned 3,138
  files, 1,575 Rust files, and 721,479 Rust physical LOC with 0 errors and 128
  repository-wide warnings.

## Increment 5 implementation

Live mounts now retain a private `ResolvedMountedViewOwner` that separates
anonymous Rust, public Rust, and accepted Arcweft authority. Process-local
`ViewRegistryId` and crate-private `ViewDefinitionIndex` values remain only in
that live owner. Public mount output and call-node output use typed `ViewId`;
the public owner projection is the closed `ViewOwnerEvidence::{Public,
AnonymousHost}` form.

Save snapshots now serialize `SavedViewOwner`. Public Rust owners persist only
their stable `ViewId` and `ViewSchemaId`; Arcweft owners persist `ViewId`,
`ViewProgramId`, and the accepted program revision. Anonymous Rust projection
fails with `AnonymousRustViewNotPersistable`. Restore resolves the stable owner
through the candidate registry/catalog and verifies implementation kind,
schema, program, and revision before allocating or publishing any mount state.
Forged owner-kind, program, revision, and schema tests prove that failure leaves
the previously accepted snapshot unchanged. A fresh-registry test proves that
public Rust restoration does not depend on the original dense slot.

Exported-part lookup now requires the private Arcweft owner proof; a public
Rust owner with the same stable spelling cannot mint an Arcweft exported-part
capability. The one accepted part catalog remains immutable and authoritative.
Compiler-produced checked owner mapping continues by exact public identity;
the changed compiler call site removes the remaining option-flattening lint
without introducing a string or path reconstruction.

The unpublished save payload was corrected in place and remains schema 1. No
dual reader, compatibility alias, migration shim, numeric owner serialization,
or anonymous persistence path was added.

### Increment 5 verification

All Cargo verification used `CARGO_INCREMENTAL=0` after the final rebase onto
Git `b7be621bab0f`:

- `cargo fmt --all -- --check` — passed;
- the exact runtime `view_identity`, `exported_part`, and `view_save` filters —
  one passed in each filter;
- private owner tests — two passed, covering anonymous/public evidence,
  anonymous save rejection, fresh dense-slot restoration, and forged schema;
- private exported-part capability test — one passed;
- `cargo check --workspace --all-targets` — passed;
- `cargo clippy -p arcweft-runtime-driver -p arcweft-compiler
  -p arcweft-player-scene --all-targets --all-features -- -D warnings` —
  passed; and
- focused runtime integration before and after rebase — all 17 tests passed.

The canonical post-change structural audit on Jujutsu change `osskwmts` scanned
3,150 files, 1,581 Rust files, 723,297 Rust physical LOC, and 92 package
manifests. It reported 0 errors and 128 repository-wide warnings. No duplicate
baseline report was written. Relative to `b7be621bab0f`, this increment changes
21 Rust files by 831 insertions and 183 deletions. The new owner responsibility
module is 9,700 bytes/278 physical LOC and its sibling tests are 3,922 bytes/135
physical LOC. Existing warning-level touched hotspots remain the runtime View
evaluator (65,899 bytes/1,613 LOC) and runtime facade (44,549 bytes/1,161 LOC);
the evaluator's mount-construction policy was extracted into a named
`create_occurrence` responsibility so the changed method passes the active
complexity lint. Other large touched files are test owners or mechanical typed
`ViewId` call-site adaptations. No crate dependency or new broad facade export
was added.

## Completion accounting

The final ordinary-parser rejection accounting is implemented without a
historical recognizer, and the repository completion evidence is recorded in
Increments 7 and 8 below. No package implementation item remains deferred.

## Increment 6 implementation

`AcceptedViewProgramRevision` is no longer the digest of the complete bundle
resource. `arcweft-view` owns the domain-separated BLAKE3 operation, while
`arcweft-bundle` supplies a canonical typed semantic transcript. The transcript
includes program identity, definitions, state/schema facts, typed value and
instruction programs, handlers, local parts, exports, direct call targets, and
runtime metadata. Product source identities, display names, source bytes,
source-table order, and every source range are explicitly excluded. Definition
table reordering is canonicalized before transcript construction. Tests prove
that source-only products have equal accepted revisions and distinct
`SourceSetRevision` values, while typed semantic edits change the accepted
revision.

The immutable runtime catalog now records separate per-definition local and
export fingerprints plus typed direct-call edges. Local fingerprints include
the exact definition/body, referenced value programs and typed input slots,
referenced handlers, state schema, and local parts. Export fingerprints contain
only typed owner/local/public identities and exclude provenance. Catalog diff
therefore distinguishes an owner-local implementation edit from a public export
add/remove/rename and computes direct callers without string reconstruction.

`BundleViewRuntime` exposes a prepared replacement transaction. Preparation
validates program identity, dialogue contract, candidate catalog, registry
collisions, value inventory, checked generation/frame counters, mount graph,
and invalidation facts entirely in scratch state. Commit first compares the
captured program/revision/source/generation/frame plus logical time, allocator,
root bindings, mounts, and axis seeds; a stale prepared value is consumed with
no mutation. The commit path itself performs only infallible field publication.

Equal semantic/source candidates are true no-ops. Source-only candidates replace
only product/catalog provenance and preserve generation, frame, mounts, caches,
registry, and the last semantic invalidation. Semantic candidates increment a
nonzero checked generation and checked frame revision, retire/re-register
Arcweft registry entries in a clone, reconcile all occurrences, prune axis-seed
facts, and publish one targeted invalidation. Stable paths retain mount IDs and
typed state when schema-compatible; incompatible private state is reset from
the candidate inventory. Removed definitions and invalid call/repeat paths are
retired, so a later definition reintroduction keeps its `ViewId` but receives a
fresh private mount identity.

The replacement tests cover unchanged and source-only candidates, stale commit,
catalog and program-identity rollback, exact export owner/direct caller
invalidation, isolation of unexported local edits, two simultaneous root mounts,
schema reset, definition removal/reintroduction, nested call removal, and two
repeat-nested child mounts retired atomically. Exact `u64::MAX` generation/frame
acceptance and one-over exhaustion are tested without a public test hook.

The new responsibilities are split across `view/semantic.rs`,
`view_runtime/catalog/fingerprint.rs`, `view_runtime/replacement.rs`, and
`view_runtime/replacement/reconcile.rs`; replacement logic was not added to the
existing evaluator hotspot. No compatibility reader, dual revision, alias,
removed-syntax recognizer, source gate, `unsafe`, or unchecked counter was
introduced.

### Increment 6 focused verification

All Cargo commands used `CARGO_INCREMENTAL=0`:

- `cargo test -p arcweft-bundle --test view_product_validation` — 8 passed;
- `cargo test -p arcweft-runtime-driver --test view_runtime` — 25 passed;
- the exact replacement counter unit filter — one passed;
- `cargo check -p arcweft-runtime-driver --all-targets` — passed; and
- `cargo clippy -p arcweft-view -p arcweft-bundle
  -p arcweft-runtime-driver --all-targets --all-features -- -D warnings` —
  passed.

The final Increment 6 cut also passed `cargo fmt --all -- --check` and
`cargo check --workspace --all-targets` with the same incremental setting.

### Increment 6 structural audit

The canonical dry-run audit on Jujutsu change `rtlwtrqq`, based on Git
`2966a182a369`, scanned 3,154 files, 1,585 Rust files, 724,936 Rust physical
LOC, and 92 package manifests. It reported 0 errors and 128 repository-wide
warnings. No duplicate report directory was written.

Exact current-checkout metrics for every changed Rust file are:

| Path | Bytes | Physical LOC | Role | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 85,527 | 2,368 | integration tests | n/a |
| `crates/arcweft-runtime-driver/src/view_runtime.rs` | 44,891 | 1,170 | runtime facade/orchestration | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/catalog.rs` | 21,376 | 567 | immutable catalog/diff | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/axis_seed.rs` | 17,463 | 468 | axis-seed lifecycle | 0 |
| `crates/arcweft-bundle/tests/view_product_validation.rs` | 14,128 | 398 | integration tests | n/a |
| `crates/arcweft-runtime-driver/src/view_runtime/replacement.rs` | 13,125 | 368 | prepared transaction | 24 |
| `crates/arcweft-bundle/src/resource_codec/view/validated.rs` | 12,285 | 353 | complete-product typestate | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/owner.rs` | 9,711 | 278 | mount-owner/generation identities | 0 |
| `crates/arcweft-view/src/view/identity.rs` | 8,241 | 272 | stable identities/revision hash | 66 |
| `crates/arcweft-runtime-driver/src/view_runtime/catalog/fingerprint.rs` | 9,008 | 250 | local/export fingerprints | 0 |
| `crates/arcweft-runtime-driver/src/view_runtime/replacement/reconcile.rs` | 6,623 | 188 | scratch mount reconciliation | 0 |
| `crates/arcweft-bundle/src/resource_codec/view/semantic.rs` | 4,833 | 115 | source-free semantic transcript | 0 |
| `crates/arcweft-bundle/src/resource_codec/view.rs` | 1,230 | 33 | resource-codec facade | 0 |

The runtime facade remains below the 1,200-LOC production warning threshold,
and the expanded integration test remains below the 2,500-LOC test warning
threshold. Replacement was split before review: its 368-LOC transaction module
delegates the 188-LOC mount/path responsibility to `reconcile.rs`; fingerprint
construction is a separate 250-LOC module. No production file crossed a review
threshold.

`arcweft-runtime-driver` has six workspace fan-in edges, 12 normal fan-out
edges, and three dev-only fan-out edges. The one new normal edge is `blake3`,
used only for private domain-separated per-definition fingerprints. Accepted
program revision hashing remains owned by the pre-existing
`arcweft-view -> blake3` edge; no upward or reverse architecture dependency was
introduced.

## Increment 2 implementation

`arcweft-bundle` now directly depends on `arcweft-source` and owns the one
canonical multi-source `SourceMapSection`. `ProductSourceId` is derived only
from `SourceDocumentId`; the section retains exact UTF-8, typed display name,
source revision and extent, sorts by product source identity, and computes the
order-independent `SourceSetRevision`.

The compact SourceMap payload is schema 2. Its fixed transcript references the
common public-ID and string tables, and the decoder independently verifies the
derived product identity, exact source revision, exact extent, UTF-8,
source-set revision, record count, limits, and canonical byte-for-byte
re-encoding. The old serde single-source transcript and
`SourceMapSection::from_bundle` are deleted; schema 1 and unknown optional
fields are rejected rather than creating a second accepted byte spelling.

Construction preflights document count, duplicate logical documents,
cryptographic identity collisions, ID/display/source/total byte budgets, and
checked arithmetic before copying source text into the candidate. The exact
limit and one-over tests exercise 65,536 documents, 8 MiB per document, and 64
MiB aggregate source text.

The existing `BundleSource` and `SourceMapIndex::from_source` remain only at
the explicit Increment 3 migration boundary selected by the package. Product
AWFB encoding constructs the new section through `try_from_documents`;
decoding can temporarily project exactly one accepted document back into the
current in-memory bundle model. A multi-document section is never truncated or
silently selected: that projection rejects until Increment 3 removes the
single-source bundle boundary and introduces complete-product typestate.

Focused verification with `CARGO_INCREMENTAL=0`:

- `cargo test -p arcweft-bundle --test source_map -- --nocapture` — 7 passed;
- `cargo test -p arcweft-bundle --test product_catalog_resource_codecs` — 4
  passed;
- `cargo test -p arcweft-bundle` — all unit, integration, and doc tests passed;
- the exact Cargo metadata dependency-contract test — passed; and
- `cargo clippy -p arcweft-bundle -p arcweft-project-loader --all-targets
  --all-features -- -D warnings` — passed after splitting transcript and
  per-document decode responsibilities.

The final Increment 2 checkout also passed:

- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test -p arcweft-bundle`, including 90 unit tests and every bundle
  integration/doc-test target; and
- `cargo fmt --all -- --check`.

The first workspace check attempt stopped because the linked worktree did not
contain the gitignored `web/assets/noto-sans-jp-vf.ttf` test fixture. The exact
fixture already present in the primary checkout was copied into this worktree
for validation only; it is ignored and is not part of the change. The rerun
passed.

## Increment 2 structural audit

The canonical audit ran on working change `yozwpppm` after rebasing onto Git
`bacf6c5a71c0`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/view-exported-part-correction-increment2-2026-07-17
```

It scanned 3,081 files, 1,543 Rust files, 707,496 Rust physical LOC, and 90
package manifests. The result was 0 errors and 128 repository-wide warnings.
Exact reports are checked in under
`docs/implementation/structure-audits/view-exported-part-correction-increment2-2026-07-17/`.

The new SourceMap responsibility modules are 14-270 physical LOC and the new
boundary test is 299 LOC. `product.rs` initially crossed the 1,200 LOC review
threshold, so SourceMap validation and temporary single-source projection were
moved into `product/source_projection.rs`; the production orchestrator is now
1,174 LOC and no longer triggers the size or embedded-test warning. The direct
`arcweft-bundle -> arcweft-source` edge is intentional, while metadata tests
prove there is no reverse `arcweft-source -> arcweft-bundle` path and no
`arcweft-lang-sema -> arcweft-bundle` path.

After that rebase, the focused SourceMap tests, product-catalog codec tests,
exact dependency-direction test, bundle/project-loader check and clippy, and
workspace check and clippy all passed again with the same feature sets listed
above.

## Increment 7 parser accounting correction

No parser production path, compatibility recognizer, historical CST/AST kind,
or spelling-specific diagnostic was added. The existing canonical export
declaration and malformed-current-syntax tests remain the diagnostic authority.
A repository search found no GRA-017 through GRA-021 fixture, diagnostic-code
assertion, or parser recognizer to delete; the checked-in correction request is
the document that retires those former acceptance categories.

The CLI bundle integration owner now contains one spelling-agnostic recovery
test. An arbitrary unsupported View container produces ordinary parser errors
and a typed `ViewExpr::Raw` recovery value. The test then proves directly that
the recovered source creates no `ViewPartExportDecl`, HIR export owner, checked
export, product export record, or accepted runtime export fact. Assertions use
typed public or crate-owned APIs and do not scan source text, match a historical
spelling, or require a dedicated diagnostic code.

Focused verification used `CARGO_INCREMENTAL=0`:

- `cargo test -p arcweft-cli --lib
  ordinary_view_recovery_cannot_create_exported_part_facts -- --nocapture` —
  one passed;
- `cargo test -p arcweft-lang-syntax --test view_export_part` — six passed;
  and
- `cargo test -p arcweft-lang-sema --test view_part` — four passed.

The first CLI invocation used `--exact` without the module-qualified unit-test
name and therefore selected zero tests after a successful build. It was
immediately rerun with the unqualified filter above and executed the intended
test successfully. The earlier initial build attempt also reached its 120-second
wrapper timeout before emitting a test result; the completed rerun is the
recorded evidence.

Additional final-cut verification with `CARGO_INCREMENTAL=0` passed:

- the module-qualified authored exported-part CLI integration test — one
  passed;
- `cargo test -p arcweft-bundle --test source_map` — seven passed;
- `cargo test -p arcweft-bundle --test view_product_validation` — eight
  passed;
- the exported-part resource-codec target — six passed;
- `cargo test -p arcweft-runtime-driver --test view_runtime` — 25 passed before
  the final activation regression was added;
- focused tooling and LSP exported-part tests — two passed in each crate;
- the exact project-loader dependency-direction test — one passed;
- `cargo check --workspace --all-targets --all-features` — passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.

Some package command filters used unqualified names that selected zero tests;
these were not counted as evidence. The actual module-qualified or owning test
targets listed above were run instead.

## Increment 8 completion validation and fixture reconciliation

The final repository validation exposed three stale cross-cutting fixtures and
one missing current-surface normalization; all were corrected at their owning
boundary rather than hidden by compatibility logic:

- a manifest-only native patch test now expects the unchanged SourceMap-owned
  source identity instead of treating manifest metadata as source authority;
- `web/demo.awfb` was regenerated by the checked-in
  `just fixture-refresh-web-demo-awfb` recipe so it carries the canonical
  schema-2 multi-source SourceMap;
- the AWBC session same-content/different-manifest test now changes only
  `profile_id`, preserving its asserted common content root; and
- the canonical `.on_click` View surface maps to the input-agnostic runtime
  `EventKind::Activate`. A direct catalog test proves the typed mapping while
  unsupported event names remain rejected.

Focused verification after these corrections used `CARGO_INCREMENTAL=0` and
passed:

- `cargo test -p arcweft-player-native --lib` — 43 passed;
- `cargo test -p arcweft-player-web --test parity` — seven passed;
- the exact authored-click catalog regression — one passed;
- `cargo test -p arcweft-runtime-driver --test awbc_product_session` — 16
  passed; and
- the exact same-content/different-manifest regression — one passed.

After fetching the remote, `main` remained the current parent
`14e9a0440229`, so no rebase rewrite was required. The final changed-crate
verification then passed:

- `cargo fmt --all -- --check`;
- `cargo check -p arcweft-cli -p arcweft-player-native
  -p arcweft-runtime-driver -p arcweft-player-web --all-targets
  --all-features`; and
- the same four-package `cargo clippy` command with `-D warnings`.

The first final `just test-workspace` attempt exposed the stale native fixture.
After that correction, the next attempt exposed the old checked-in Web fixture
and then the missing click-to-activation normalization. Both affected Web
parity tests passed after the fixes. A low-memory rerun with
`CARGO_BUILD_JOBS=1` avoided the Windows paging-file failure seen under the
default parallelism, passed the workspace sequence through the runtime-driver
suite, and exposed only the stale AWBC manifest fixture described above. Its
exact test and complete 16-test target passed after correction. A last
workspace-wide rerun was intentionally stopped to avoid competing with another
already-running full workspace validation; no known code failure remains, and
the previously completed workspace check, workspace clippy, and all affected
focused targets remain the completion evidence.

Tier-2 MCP stdio, exact visual-golden, and doc-test routes are not applicable:
this correction changes no MCP transport, rendered pixels, public Rust
documentation, or doc-test surface. Web/native parity was nevertheless run
because the checked-in product fixture and accepted View event catalog were in
the affected path.

### Increment 8 structural audit

The canonical final dry-run audit on Jujutsu change `vmvymkzz` scanned 3,155
files, 1,586 Rust files, 725,070 Rust physical LOC, and 92 package manifests. It
reported 0 errors and 128 pre-existing repository-wide warnings.

Exact current-checkout metrics for the Increment 7/8 Rust files are:

| Path | Bytes | Physical LOC | Role | Embedded test LOC |
| --- | ---: | ---: | --- | ---: |
| `crates/arcweft-cli/src/app/bundle/tests/view_part_recovery.rs` | 2,895 | 87 | integration recovery proof | n/a |
| `crates/arcweft-cli/src/app/bundle/tests.rs` | 82,514 | 2,564 | integration-test owner/facade | n/a |
| `crates/arcweft-player-native/src/patch_endpoint.rs` | 39,537 | 1,027 | native patch orchestration and unit tests | 530 |
| `crates/arcweft-runtime-driver/src/view_runtime/catalog.rs` | 21,635 | 570 | immutable accepted View catalog | 0 |
| `crates/arcweft-runtime-driver/tests/awbc_product_session.rs` | 28,448 | 801 | session integration tests | n/a |
| `crates/arcweft-runtime-driver/tests/view_runtime.rs` | 86,723 | 2,399 | View runtime integration tests | n/a |

The CLI test owner and runtime View integration target remain warning-level
test hotspots but stay below the 8,000-LOC error threshold. The new recovery
proof is an 87-LOC responsibility module rather than further expanding the
owner file. No production file crosses a structural warning threshold in this
cut. Workspace normal dependency fan-in/fan-out is 0/50 for `arcweft-cli`,
1/20 for `arcweft-player-native`, and 6/8 for `arcweft-runtime-driver`; the
latter two have two and three dev-only workspace fan-out edges respectively.
No Cargo dependency, crate boundary, facade export, `unsafe`, source gate, or
compatibility path was added in Increments 7 or 8.
