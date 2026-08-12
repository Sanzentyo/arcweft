# Anonymous-record and record-column admission evidence

Date: 2026-08-12

Inspected Git baseline:
`23c1d2a8d4f1b73bb6e5f2386d95ae160389e1ae` on `main`, equal to
`origin/main`, with a clean working tree before the A3 implementation began.

## Implemented state

The nominal-record correction A3 gate is implemented as one compile-clean cut:

- `RuntimeFieldValue` and `RecordSeqField` now carry private accepted
  `RuntimeRecordFieldId` values and expose read-only inherent accessors;
- public `RuntimeValue::try_record` preflights the one-based identity space,
  rejects the first duplicate authored name, assigns contiguous IDs, and
  publishes no partial record on failure;
- `RuntimeSeq::record_columns` is the sole public column admission boundary and
  accepts `(String, RuntimeSeq)` pairs rather than preconstructed carriers;
- the existing `RuntimeSeqError` remains the only column error owner and now
  reports count and identity failures with the fixed identity-before-length-
  before-duplicate precedence at each stored ordinal;
- row reconstruction, `value_at`, `tail_from`, literal columnarization, native
  evaluation, AWBC execution, adapters, dialogue values, and runtime drivers
  preserve accepted field identities instead of recreating fields by name; and
- `RecordSeq::new`, raw public field construction, and the old public
  `record_columns` signature are closed by compile-fail tests.

Interim `Clone` and Serde traits remain only because enclosing live values still
require them under the parent affine migration schedule. No schema, ABI, or
codec version changed; all Arcweft-owned version constants remain fixed at `1`.

## Visibility reconciliation

The returned package wrote `RuntimeValue::try_record` as `pub(crate)`, while it
also required deleting all external raw field construction. Current legitimate
producers exist in agent-runner, CLI, dialogue, the runtime accelerator, and the
runtime driver, and no other admitted anonymous-record constructor exists.

Sol max was used for this result-affecting public-boundary judgment. The minimal
reconciliation is a public `RuntimeValue::try_record` and crate-private
`RuntimeFieldValue::new_accepted`. This gives external producers one checked
admission path without exposing field-ID forgery. The analogous record-column
boundary remains public `RuntimeSeq::record_columns` over pair input, with its
carrier constructor crate-private. The evidence determined one narrow API
correction, so no separate request was warranted.

## Validation performed and passed

- `cargo fmt --all` and `git diff --check`.
- `cargo check --workspace --all-targets --all-features --jobs 4`.
- `cargo clippy --workspace --all-targets --all-features --jobs 4 -- -D
  warnings`.
- `cargo test -p arcweft-core --all-targets --all-features --jobs 4`: 295
  library tests, 5 record-admission integration tests, all existing integration
  suites, and all seven compile-fail cases passed.
- `cargo test -p arcweft-agent-runner -p arcweft-cli -p arcweft-dialogue -p
  arcweft-runtime-accelerator -p arcweft-runtime-driver -p
  arcweft-runtime-plan --all-targets --all-features --jobs 4`: agent-runner 45
  tests passed before the command reached CLI; CLI then reported 166 passed and
  36 failed in pre-existing parser/View fixture and duplicate-public-ID paths.
  The failures did not execute the record boundary and are recorded below.
- `cargo test -p arcweft-dialogue -p arcweft-runtime-accelerator -p
  arcweft-runtime-driver -p arcweft-runtime-plan --all-targets --all-features
  --jobs 4`: dialogue 35 tests, accelerator 98 tests, runtime-driver 209 tests,
  and runtime-plan 102 tests passed.
- `just structure-audit` and `just structure-audit-gate`: 2,166 files, 2,038
  Rust files, 1,008,273 Rust physical LOC, 95 packages, 185 review triggers,
  and zero blocking findings.

## Failed validation outside this cut

`just test-workspace` did not reach a test failure. Its compilation stopped
after Windows reported OS error 1455 (the paging file was too small), followed
by invalid dependent rlib metadata and rustc ICE cascades. The same changed
workspace had already passed the all-target/all-feature workspace check and
Clippy run, and the affected crate suites above passed, so this environmental
failure is recorded without treating the aggregate command as green.

The broad CLI all-feature library suite failed 36 of 202 tests. Representative
failures reject old View fixtures with current syntax diagnostics such as
`syntax.view.invalid_parameter`, `flow.identity.missing`, and
`syntax.flow.missing_body`; three cache tests also report the existing duplicate
`view_text_sources:std.dialogue.text.character_display_name`. No failure
mentions `RuntimeRecordAdmissionError`, `RuntimeFieldValue`, `RecordSeqField`,
or `RuntimeSeqError`. These unrelated baseline failures were not repaired or
hidden in A3.

`CARGO_BUILD_JOBS=1 just test-tier2` completed its single-job build, then the
first `test-slow-mcp` target reported 5 passed and 17 failed and stopped the
recipe before later Tier 2 targets. The failures are existing MCP observe and
fixture mismatches: player-backed observe initialization failures, null values
where old fixtures expect booleans or arrays, and one stale rich-text fixture
rejected with `syntax.choice.missing_body`. None reports or traverses the new
record admission errors or field carriers. Tier 2 is therefore explicitly not
claimed as passing.

## Structural review and continuation

The touched large owners retain cohesive existing responsibilities: core value
and sequence storage own admission and identity-preserving materialization;
AWBC VM/fiber and native evaluators only consume the new accessors; runtime-plan
only projects admitted values; adapter/driver files only translate their local
payloads through the public core admission boundary. No copied field-ID table,
name-derived identity resolver, second record error enum, or compatibility
constructor was introduced. Splitting these narrow branches solely for their
existing file sizes would separate them from the state they validate.

A4 nominal runtime-value admission/deletion and the later ownership visitor and
persistence closure are explicitly not claimed by this A3 evidence.
