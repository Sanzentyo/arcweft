# Validation report

Baseline: Git `main` and `origin/main` at
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`.

Scope: repository-local design artifacts only. Production Rust, Cargo files,
tests, fixtures, branches, worktrees, maintained request status, returned
packages, and ZIPs were intentionally not changed. Pre-existing review intake
and sibling-design dirt was preserved. The validator separately proves the
production Rust/Cargo scope is clean.

## Commands

Run from `D:\git\arcweft`:

```text
rustfmt +nightly --check <schemas/final_contract.rs and three validator Rust files>
cargo +nightly -Zscript docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/tools/validate_design.rs
cargo +nightly -Zscript docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/tools/negative_self_tests.rs
```

The repository-aware validator checks terminal files, request byte/hash
identity, all required members, manifest hashes, structured decisions 1–7,
forbidden scope, exact Git HEAD/origin/branch, production-scope cleanliness,
21 pinned source blobs, nine live Rust enums through `syn` AST parsing, and
Cargo metadata/dependency direction. The negative runner mutates every listed
mandatory gate in `machine/negative_corpus.json` and requires the intended
gate-specific rejection.

## Results

Validated on 2026-08-23 (Asia/Tokyo):

| Gate | Result |
|---|---|
| `rustfmt +nightly --check` for `schemas/final_contract.rs` and all validator Rust | PASS, no output |
| repository-aware validator | PASS: `files=21`, pinned head, inventories `27/8/7/5/38/13/35/5/13`, decisions `1-7` |
| negative self-tests | PASS: `negative_cases=37` |

Final outputs:

```text
PASS design=\\?\D:\git\arcweft\docs\reviews\designs\lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure files=21 head=9a5d30d25620541c3f2975d31e04e04e3bc9514c inventories=27/8/7/5/38/13/35/5/13 decisions=1-7
PASS design=\\?\D:\git\arcweft\docs\reviews\designs\lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure negative_cases=37
```

Three preliminary validator runs correctly failed while the local validator
contract was being completed: one decision-register spelling mismatch and two
over-specific documentation anchors. Those validator defects were corrected;
the manifest was refreshed after each edit, and the full repository-aware gate
then passed. The first negative run passed all 35 cases but emitted dead-code
warnings because that entry point had not invoked the repository checks. The
entry point was tightened to invoke them and use its root in the result; the
warning-free rerun passed. A final inventory audit then added explicit current
`HirBodyChildRole` and `HirStatementBodyRole` AST gates and two corresponding
negative mutations; the final 37-case rerun above passed.

An independent root recheck initially invoked the validator from the design
folder rather than its documented repository-root working directory, so the
required-file lookup failed without changing files. The documented root command
then passed. That recheck also found import-order drift in the two entry-point
scripts; the imports and manifest rows were corrected, after which the positive
validator, all 37 negative mutations, `rustfmt +nightly --check`, and design
diff check were rerun and passed.

No workspace compile/test tier was run because this task is design-only and
changes no production Rust/Cargo/tests/fixtures; those tiers are mandatory for
C1–C5 implementation and are enumerated in `CUTS_TESTS_AND_DELETION.md`.

No archive or ZIP validation was run or claimed: this repository-local accepted
design was explicitly scoped not to create or modify packages/ZIPs.

## C1 schema erratum validation addendum

Added on 2026-08-23 (Asia/Tokyo). This addendum corrects only the Rust-shaped
C1 declaration/expression-owned path schema. It does not replace or restate the
historical validation above, change the accepted request mirror, or claim that
the in-progress C1 production implementation has passed review.

The correction makes `HirNestedExpressionPath` the sole nested Choice and
line-plan coordinate authority, defines all six `HirLinePlanStatementRole`
cases, mirrors the `role` field on dialogue statement roots, and records how 14
Rust role variants represent the 19 logical root families. Validator gates and
negative mutations now reject the former raw `group_path` parallel authority,
a missing line-plan role, and drift between `SCHEMAS.md` and the Rust-shaped
mirror.

Repository inspection during the erratum also established that the baseline
does not publish a View row from `ProjectSymbolTable::callable_symbols()`.
Although the existing callable key/owner, `ViewItem` source roles, sema
validation, and downstream project-callable family already represent View,
the linker and registered/checked callable builders omit it. The corrected C1
contract therefore requires same-cut completion of that existing nonbinding
callable pipeline. It does not authorize a retained-symbol fallback, synthetic
site/key, second name binding, or parallel catalog.

| Erratum gate | Result |
|---|---|
| JSON parse for `machine/final_contract.json` and `machine/negative_corpus.json` | PASS |
| `rustfmt +nightly --check` for the Rust-shaped schema and validator files | PASS, no output |
| manifest member/hash verification | PASS, 20 covered members plus the self-excluded manifest |
| semantic validator over the amended design bundle | PASS: `files=21 repository=NOT_RUN` |
| negative schema self-tests | PASS: `negative_cases=40 repository=NOT_RUN` |
| historical repository/source gate | NOT RUN — the shared production scope contains the in-progress C1 implementation diff |
| workspace compile/test | NOT RUN — outside this documentation-only erratum task |

The historical baseline/source evidence remains pinned to
`9a5d30d25620541c3f2975d31e04e04e3bc9514c`. No current dirty production byte
is represented as clean baseline evidence.

Erratum-only commands, run from `D:\git\arcweft`:

```text
rustfmt +nightly --check <schemas/final_contract.rs and three validator Rust files>
cargo +nightly -Zscript <design>/tools/validate_design.rs --design-only
cargo +nightly -Zscript <design>/tools/negative_self_tests.rs --design-only
```

Final erratum-only outputs:

```text
PASS design=\\?\D:\git\arcweft\docs\reviews\designs\lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure files=21 repository=NOT_RUN inventories=27/8/7/5/38/13/35/5/13 decisions=1-7
PASS design=\\?\D:\git\arcweft\docs\reviews\designs\lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure negative_cases=40 repository=NOT_RUN
```

Two preliminary invocations placed `--` before `--design-only`; the scripts
therefore treated `--` as a design path and failed `required_files`. After the
documented argument form was used, the first semantic run correctly rejected
the old raw-group spelling still quoted in erratum prose. That obsolete quote
was removed, the manifest was refreshed, and both erratum-only gates passed.
