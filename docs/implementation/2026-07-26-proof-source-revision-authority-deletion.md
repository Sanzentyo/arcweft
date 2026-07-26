# Proof source-revision authority deletion

Date: 2026-07-26

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_PROOF_GATE`

## Boundary

This deletion-driven cut removes three independently computed BLAKE3 source
digests and one retained warning count where an accepted typed owner already
contains the exact value:

- `arcweft_lang_syntax::source::SourceHash`;
- `arcweft_project::sources::ModuleSourceHash`;
- the `CompiledProjectModule` source-hash and source-identity fields; and
- the `CompiledProjectModule` syntax-warning field.

The sole source-content authority is now
`SourceDocumentIdentity::revision(): SourceRevision`. `ProjectSourceFile`
exposes that owned revision without hashing the source again. Build-cache
consumers use the infallible `From<SourceRevision> for BuildDigest`
projection, which preserves the exact 32 bytes rather than hashing a digest.

`CompiledProjectModule::source()` derives the accepted identity from its
source-backed HIR. Cache admission compares that complete identity, including
the logical document ID, revision, and source length. A second content-hash
comparison after that identity comparison was redundant and has been deleted.
The public project warning total is derived from the retained typed lint
inventory.

This HIR-derived accessor is only the current-main construction invariant. It
does not promote HIR to the final source authority. When the Proof public
switch installs bound `ParsedSource` ownership, `CompiledProjectModule` must
derive the identity from that bound snapshot and retain the stronger
`ParsedSource::is_same_snapshot` cache-admission check; the protected Proof WIP
must not be rebased by weakening that check to document identity alone.

## Observable evidence

Direct tests cover:

- project source revision equality with the owning document revision;
- equal UTF-8 bytes producing an equal content revision without conflating
  distinct document identities;
- revision-to-build-digest projection without rehashing;
- compiled module identity equality with the accepted project document;
- rejection of a cached HIR module from another document identity even when
  its source bytes and compile-unit fingerprint are identical; and
- compile-unit fingerprint invalidation and cache miss when the same logical
  document ID receives changed source bytes; and
- project warning totals matching the Warning rows in the retained lint
  inventory.

Existing persistent-object tests continue to prove deterministic source
digests, and the CLI project/persistent-cache tests continue to exercise the
same JSON `source_hash` value and object-key digest. The JSON field name remains
accurate public vocabulary for a content hash; it is not a compatibility
reader or a second authority.

## Explicit non-goals

- `SyntaxParseStats` remains retained by `CompiledProjectModule` because
  current `main` consumes `ParsedSource` into the typed tree. It becomes
  derivable only when the bound `ParsedSource` is the compiled-module owner.
- The public parser facade, Agent REPL synthetic source, and tooling parse
  records are sole owners on current `main`; this cut does not delete or
  duplicate them.
- This cut does not restore `hir::lower_source_document`, add a detached
  syntax reader, or select a final HIR leaf-expression payload.
- Proof `01.1.1.4` remains rejected as documented in
  [`2026-07-26-proof-01.1.1.4-return-intake.md`](2026-07-26-proof-01.1.1.4-return-intake.md).
  The byte-identical archive reattached on 2026-07-26 is not the requested
  corrected `01.1.1.4.1` redelivery. The accepted public HIR switch continues
  to wait for
  [`2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md`](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

## Validation

Passed:

- `cargo fmt --all -- --check`;
- `cargo check -p arcweft-lang-syntax -p arcweft-project -p arcweft-compiler -p arcweft-cli --all-targets`;
- `cargo test -p arcweft-project`: 34 unit and 2 integration tests;
- the focused syntax revision-owner test: 1 test;
- `cargo test -p arcweft-compiler --test project_cache_transaction`: 13 tests;
- the focused retained non-blocking-lint test: 1 test;
- focused compiler persistent tests: 9 tests;
- `cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens`: 2 tests;
- changed-crate all-target/all-feature strict Clippy;
- workspace all-target/all-feature check;
- workspace all-target/all-feature strict Clippy;
- `just test-tier2`: all 46 MCP, Agent/native capture, visual,
  integrity, and exact-golden rows; and
- the canonical structural audit: 3,694 files, 1,939 Rust files,
  906,597 Rust physical LOC, 95 manifests, 0 errors, and 146 warnings.

The final structural reports are retained under
[`structure-audits/proof-source-revision-authority-deletion-2026-07-26/`](structure-audits/proof-source-revision-authority-deletion-2026-07-26/).
Changed production hotspots were reviewed with exact current sizes:

- `project_commands.rs`: 82,227 bytes / 2,276 physical LOC; this cut only
  replaces its two source-digest projections and adds no responsibility;
- `persistent.rs`: 53,756 bytes / 1,426 physical LOC; this cut mechanically
  redirects nine existing persistent facts to the canonical revision and adds
  no responsibility; and
- all other changed production files are below the 1,200-LOC warning
  threshold, including `project.rs` at 36,498 bytes / 1,085 physical LOC.

The two warning-level large-file findings above predate this cut and are not
made worse by it. Splitting unrelated CLI orchestration or persistent-fact
tests is not part of this source-authority deletion.

`just test-workspace` ran for 929.7 seconds. All workspace, CLI lib/bin,
runtime/style/release/profile, persistent-cache golden, and compile-fail stages
preceding the final Arcweft fixture gate passed. The fixture gate passed 3 of 5
rows and retained the two known Proof failures:

- `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
- `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`.

Both still require the final attached extern-capability `FsError` authority.
This cut neither changes that path nor restores a detached fallback.

An additional `just test-cli-check` diagnostic passed 1 of 51 bench rows and
failed 50 pre-existing broad bench fixtures on unresolved `Vec<_>`/array
typing, `Bool`/`Char`/matrix/tensor and `FsError` nominals, stale `Unit` return
expectations, or missing product AWBC entrypoints. Those failures are outside
this source-revision cut and are not repaired by reviving old semantic paths.

Resolved intermediate validation findings were a formatter diff, a missing
`# Panics` section on the source-backed compiled-module invariant, and an
initial warning test that exercised a Hint rather than a nonzero Warning. The
checkout was formatted, the invariant documented, and the test changed to the
typed `RedundantDeclIdentity` Warning before the final focused and broad gates.
