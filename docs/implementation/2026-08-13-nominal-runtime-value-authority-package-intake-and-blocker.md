# Nominal runtime-value authority package intake and blocker

Date: 2026-08-13

Supersedes:
`docs/implementation/2026-08-12-nominal-runtime-value-a4-dialogue-authority-blocker.md`

Inspected Git baseline:
`50771a19f57f86570837f616a66252be24e77e0c` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake and G1 editing. The
independent G1 correction was committed as
`1648894fbfc38ba623d1b01c6001fbd55b67b10b` before this intake record.

## Returned archive intake

Retained archive:
`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2-nominal-runtime-value-external-admission-and-dialogue-layout-authority-correction-final-contract.zip`

SHA-256:
`7a7001cba41f312d428a88589877ce48eb3bb6734aff234b72601d7bfa6a9d70`

The 61,542-byte ZIP contains 23 entries below one redundant root wrapper. It
has no unsafe path, symlink/reparse entry, case-insensitive collision, or
production overlay. The wrapper was stripped in the searchable frozen mirror.
All 23 extracted files match their ZIP member SHA-256 values, all 22 internal
manifest rows pass, `SOURCE_REQUEST.md` is byte-identical to the repository
request, and both parent ZIP hashes match retained repository bytes.

Package metadata reports `READY_FOR_IMPLEMENTATION=1`, `DESIGN_ONLY=1`,
`PRODUCTION_OVERLAY=0`, `OPEN_QUESTIONS=0`, and all Arcweft-owned versions
fixed at `1`.

## Readiness adjudication

The package closes the prior external-constructor and CharacterDialogue
physical-representation questions, including schema-owned digest/canonical
encoding and descriptor-aware normalize/clear/patch semantics. It is not yet
safe to implement from G2 onward because plan/product admission cannot verify
the proposed authority from independent evidence:

- producer declarations carry only producer plus catalog keys, with no
  canonical payload roots against which extra keys can be rejected;
- public arbitrary CharacterDialogue role types are not correlated to
  accepted semantic facts or one admitted generation;
- current named standard role facts do not project to the assumed closed
  runtime checked types;
- the custom catalog digest remains caller-supplied without a canonical
  descriptor digest grammar or generation correlation; and
- independently executable AWBC VM/fiber/product-step paths have no admitted
  product authority corresponding to the proposed `RuntimePlan::try_admit`.

Sol max was used for this result-changing judgment. Production work is stopped
after the independent G1 closed-variant predicate correction. The required
follow-up request is:

`docs/reviews/requests/2026-08-13-lang-01.3.1.2.3.2.1.2.1-generation-bound-producer-root-and-awbc-admission-authority-correction.md`

No catalog, generation, role/custom, AWBC admission, CharacterDialogue
representation, or unchecked-constructor deletion code is claimed by this
intake cut.

## Validation performed

- retained core nominal focused baseline: 11 tests passed;
- G1 exact nominal-variant focused test: passed;
- G1 Result/Option exact builtin-case focused test: passed;
- `cargo clippy -p arcweft-core --all-targets --all-features --jobs 1 -- -D
  warnings`: passed; and
- `cargo fmt --all` and `git diff --check`: passed.

The first G1 compile attempt exposed and corrected a Rust struct-literal
parenthesization error. The second exposed and corrected a test closure type
annotation and a Clippy function-length finding by moving the cohesive nominal
variant predicate into a private inherent method. Neither failed command is
reported as green.

`cargo check --workspace --all-targets --all-features --jobs 1` passed after
the G1 correction. Tier 2 and structural gates were intentionally not run for
this small predicate correction and package-intake blocker. They remain
required for the resumed public catalog/product implementation.
