# AW-AH-009.2.1 character definition source-index completion

## Status and basis

The remaining source-index arithmetic and direct negative-evidence cut is
implemented as an isolated change above call-surface commit `e268c3f20b9b`.
Focused validation, the registration regression suite, sema all-target
checking, and warning-denying Clippy are complete. This note does not claim a
separate workspace-wide test result; the cut is validated and committed
independently from the already-pushed direct expression-parser change.

The source package is
`arcweft-aw-ah-009.2.1-character-nominal-definition-source-index-final-contract.zip`
with SHA-256
`89b0ecbab84b9954626f139e320d2dba3f7a273a92ff0d6cbd0dc922c50770d7`.
This note covers only the isolated immutable source-index completion. It does
not claim the launch-overlay, source-adapter diagnostic, or exhaustive external
compile/matrix follow-ups from AW-AH-009.2.1.1 through AW-AH-009.2.1.3.

## Implemented contract

- Production index construction continues to use only
  `CharacterDefinitionLimits::PRODUCTION`. The reduced-limit entry is now the
  contract-specified same-crate, test-only
  `CharacterDefinitionIndex::try_build_with_limits`; no public limit override
  or compatibility overload was added.
- Diagnostic report bounding sorts and deduplicates before applying its
  inclusive maximum. `usize` conversion, omitted-count subtraction, and
  truncation conversion are checked. An unrepresentable operation yields the
  typed fail-closed `ArithmeticOverflow { counter: Diagnostics }` report rather
  than a saturated count.
- Full-value width and quote-excluding selection-range construction use checked
  arithmetic. The index no longer uses saturating subtraction to validate a
  declaration span.
- Source-set revision construction, declaration string-content projection, and
  admitted-document lookup no longer terminate through production `expect`.
  They become typed source-revision conflict, arithmetic-overflow,
  non-string-declaration, or missing-document build errors, and the index
  remains unpublished.
- `CharacterDefinitionIndexBuildError` gains the explicit
  `ConflictingSourceRevision` fail-closed variant for a corrupted retained
  source set. The variant retains the document ID, both revisions, and both
  lengths supplied by `SourceSetRevisionError`. It maps to the existing stable
  `aw.character.definition.index.conflicting_document` code; no second code,
  legacy reader, or compatibility variant was introduced.
  It is not folded into the earlier `ConflictingDocument` variant because that
  variant owns two complete `SourceDocumentIdentity` values from manifest
  admission, while the lower-layer aggregate revision error exposes only
  identity components and no public constructor exists for fabricating those
  provenance-bearing identities.
- Descriptor inventory auditing is a named build phase shared by production
  construction and same-module corruption tests. It still compares the
  complete environment inventory with the complete primary descriptor map and
  publishes no partial index.
- Source-index tests live in the responsibility-specific
  `registration/source_index/tests.rs` module instead of expanding the
  registration-wide test file.

## Direct evidence

The focused module covers the original package rows I004 through I012:

| Rows | Evidence |
| --- | --- |
| I004–I005 | missing exact manifest document and conflicting same-ID revision |
| I006–I008 | foreign span identity, selection outside value, and quote-inclusive selection |
| I009–I010 | exact duplicate and inconsistent same-occurrence source facts |
| I011–I012 | missing and unexpected primary descriptor map corruption |

It also covers:

- the exact reduced manifest, descriptor, document, per-descriptor source,
  source-byte, and build-work limits;
- one-over fail-closed outcomes for each of those counters;
- exact and truncated diagnostic report bounds, omitted count, and ordering
  independent of insertion order, including zero and `u64::MAX` maxima,
  deduplication before bounding, and a bounded conversion-overflow report;
- declaration-source and source-revision determinism when equal
  co-definitions arrive in reverse catalog order; and
- source-set revision failure mapping plus `u64::MAX + 1` counter and build-work
  overflow, including terminal build exhaustion without saturation.

The tests exercise typed builder and validator state directly. They do not scan
source files, assert implementation spellings, introduce removed-syntax
recognizers, or rely on a compatibility path.

The source-revision conflict test corrupts only the private retained-document
map, then calls the same `source_revision` method used by production finalization.
That method invokes `SourceSetRevision::try_for_identities` and the production
mapper, so the revision/length assertions cover the real boundary rather than
a parallel test-only conversion. Missing/unexpected descriptor tests similarly
mutate private builder state before invoking the production inventory-audit
phase. Reduced-limit tests call the contract's test-only entry into the complete
production builder. No production-like helper exists solely for a test.

The package's L011 conversion obligation has two current owners. The
source-index diagnostic count performs a checked `usize -> u64` conversion and
has a typed fail-closed branch; a safely allocated `Vec` cannot force that
branch on supported `usize <= u64` targets. The request-budget owner already
tests its controllable synthetic conversion boundary in
`character_definition/tests/budget.rs`. L012 is directly covered here through
the source-index counter and build-work `checked_add` states. The source index
does not multiply work or preallocate from untrusted aggregate counts, so L013
has no production path in this owner; no artificial multiplication helper was
introduced solely to manufacture a test.

## Structural measurement

Measured at working-copy Jujutsu change `zxovmyvyxsrlvrnzpuszqrowptsyxquz`
after focused validation:

| Path | Bytes | Physical LOC | Class | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-sema/src/registration/source_index.rs` | 39,961 | 1,102 | production | immutable descriptor provenance, checked construction, and typed build failures |
| `crates/arcweft-lang-sema/src/registration/source_index/tests.rs` | 24,700 | 699 | unit test | negative build invariants, bounds, overflow, and deterministic ordering |

The production module remains below the 1,200-LOC structural warning threshold,
and the extracted test module is within the preferred 300–800 LOC
responsibility range. This cut changes no Cargo dependency, feature, crate
boundary, unsafe code, or I/O owner. It deliberately extends the existing
public typed build-error enum as described above while preserving its stable
diagnostic code family.

The canonical repository-wide structure audit was run as a dry run against the
same working copy: 3,302 files, 1,694 Rust files, 781,278 physical Rust LOC,
92 package manifests, 0 errors, and 128 pre-existing repository warnings. It
created no report artifact, so this isolated slice does not add a fourth
unreviewed documentation change. This cut adds no Cargo dependency, feature,
or crate boundary; consequently there is no new dependency fan-in/fan-out edge
to record.

## Validation

The following commands completed against the working copy:

```bash
cargo test -p arcweft-lang-sema --lib --all-features registration::source_index::tests -- --nocapture
cargo test -p arcweft-lang-sema --lib --all-features registration::tests:: -- --nocapture
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
jj diff --git | git apply --numstat --whitespace=error-all
```

The focused source-index module passed 15 tests; the registration suite passed
60 tests. The sema check and warning-denying Clippy completed successfully.
The formatting and whitespace checks passed. Clippy initially identified a
long declaration-admission routine, a long overflow test, and two cloned
single-element test slices. The production routine is now split by the
descriptor-admission, source-projection, and source-storage invariants; the
test is split by its independent overflow assertions; the comparisons use
borrowed singleton slices. No lint allowance or test-only production helper was
introduced.
