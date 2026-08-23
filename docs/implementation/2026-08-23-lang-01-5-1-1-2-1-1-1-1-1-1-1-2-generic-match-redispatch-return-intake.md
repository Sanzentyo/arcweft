# Lang-01.5.1.1.2.1.1.1.1.1.1.1.2 generic Match redispatch return intake

Date: 2026-08-23
Inspected Git commit: `9a5d30d25620541c3f2975d31e04e04e3bc9514c`
Working tree before intake: clean; `main` matched `origin/main`

Follow-up to:
[the first rejected return intake](2026-08-23-lang-01-5-1-1-2-1-1-1-1-1-1-1-2-generic-match-complete-return-intake.md).

## Intake result

- Archive safety and byte integrity: `PASS`
- Hardened request mirror: `PASS`
- Mandatory repository preflight: `FAIL`
- Required returned-archive contract: `FAIL`
- Repository/source reconciliation: `FAIL`
- Classification: `RETURN_REJECTED / DESIGN_NOT_READY`
- Further dispatch to the same repository-inaccessible executor: prohibited
- `.1.4` dispatch: `BLOCKED`
- Production implementation: not authorized

This is not the same malformed eight-file delivery as the first return. It is
a larger internally hashed design package, but it knowingly emits the named
final-contract ZIP after repository acquisition failed. That directly violates
the hardened request's withhold-on-failure rule and leaves every current-owner
claim unverified.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction-final-contract(1).zip`
- byte length: 54,076
- SHA-256:
  `314DB38EB48BB44BB24335FFC4DFDF4839E70D9B34BF441BA4BA8357554DB151`

The unchanged ZIP is retained at
[`packages/zips/...final-contract(1).zip`](<../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction-final-contract(1).zip>).
Its 20-file frozen mirror is retained at
[`packages/...final-contract(1)/`](<../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction-final-contract(1)/README.md>).

The `(1)` suffix is the attached download name used to avoid colliding with the
first retained return. The archive's one internal wrapper exactly matches the
required output basename without that local suffix.

## Mechanical checks

- 20 safe file members under one exact wrapper;
- no absolute, drive-qualified, parent-traversal, backslash, duplicate,
  case-fold-colliding, or special Unix member;
- all 17 rows in `verification/checksums.sha256` match; the checksum file,
  `ZIP-MANIFEST.tsv`, and `verification/artifact-validation.json` are not
  covered by that list;
- `ZIP-MANIFEST.tsv` covers 18 of 20 files, excluding itself and
  `verification/artifact-validation.json`;
- the hardened `REQUEST.md` is exactly 9,780 bytes with SHA-256
  `981158FD20AFCC41E737604F7C94EA2D56E455F7DF2026D1A16A8C7994AC9628`;
  it matched the hardened dispatch revision. The maintained request was later
  closed in place by the repository-local accepted design and intentionally no
  longer has those bytes; and
- archive extraction/readability completed without a CRC or member error.

After retaining both redispatch returns, the 65 retained review ZIPs' sorted
lowercase-SHA
`name<TAB>length<TAB>sha256<LF>` transcript hashes to
`9D94E1BB3F13F97E97E290741FFDACF1A6E202A0EAB5DB9238FB992B5242612C`.

## Mandatory delivery-gate failures

The return records all of the following itself:

- local Git worktree: unavailable;
- source tree: unavailable;
- `HEAD` and `origin/main`: not materialized;
- clean baseline: false;
- source inventory: zero rows and zero symbols;
- repository verification tier: `V3_REQUEST_AND_CONTRACT_ONLY`; and
- `source_ok: false`.

Despite that evidence, `verification/artifact-validation.json` says
`"valid": true` and the main design claims `ACCEPTED-DESIGN / IMPLEMENTABLE`.
The JSON is a result sidecar, not an executable repository-aware validator.
There is no negative mutation corpus. `FINAL_STATUS.md` is absent, and
`OPEN_QUESTIONS.md` contains a heading plus ``OPEN_QUESTIONS=0`` rather than
the required exact `none`.

The hardened request requires the responder to stop and return only a blocker
report when the repository is unavailable. It also requires the produced ZIP
to be reopened and validated against the cited Git commit. With no cited commit
or source, the named final-contract ZIP was forbidden.

## Repository reconciliation

The package's owner table contains only `(not materialized)` placeholders. It
therefore does not prove any owner, path, visibility, dependency edge, current
variant inventory, constructor, reader, or deletion target.

Current source has 27 `CheckedExpressionResolution` families and existing
`CheckedMatchRef` plus `final_analysis/semantic_transcript.rs` authority. The
package inventories none of them. It also does not close the current
`ViewItem` `MissingBody` path required by decision 6.

Its proposed model also diverges materially from the current request and
source direction:

- it invents `GenericMatchOwnerId`, `MatchArmId`,
  `CompleteMatchTranscript`, `CheckedCatalog`, and multiple runtime identities
  without joining current accepted owners;
- its stable grammar includes `SourceSpanId`, `CheckedPatternId`, source
  ordinals, and a fixed-width `u16-le` schema version, defeating the requested
  source/HIR perturbation invariance and current version-1 canonical boundary;
- it introduces persisted generic-Match carriers, legacy diagnostics,
  migration shims, restore-time transcript decoding, and task-plan admission
  beyond this request's scope;
- its coverage description is a finite-constructor bitset/recursive atom
  sketch, not the required private bounded Maranget matrix and specialization
  algebra for products, sequences, Or, literals plus Other, open residuals,
  Never, Choice, and every accepted closed variant family; and
- the request-to-decision table mechanically maps long request paragraphs to
  generic decision/test lists without source-backed one-owner closure.

No returned type, wire grammar, persistence model, or owner placement is safe
to reuse independently. Generic principles such as fail-closed publication and
unknown guards not closing coverage were already accepted substrate and do not
constitute new closure credit.

## Next action

Do not modify or redispatch the maintained `.1.2` request again. The request
already required the behavior this return ignored. It is now resolved by the
[repository-local accepted design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/README.md),
which uses the existing sema transcript/coverage implementation and every
current consumer as its evidence basis. `.1.4` remains blocked until that
accepted `.1.2` design is implemented.

No Rust, Cargo, generated production artifact, fixture, or runtime test was
changed or run for this rejected design-only intake.
