# Proof convergence: runtime expected-function evidence payload deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `qvmpowvryrnnpkqowrtpzpwqskmrqlys`

## Boundary

This deletion-driven cut removes the zero-consumer `arity` field from
`RuntimeTypedLoweringEvidenceKind::ExpectedFunctionValue`.

Semantic analysis still owns the complete fact:

- expected function type;
- actual function type; and
- arity.

Compiler projection now exports a presence-only runtime-plan fact. Runtime
lowering has always used that fact only to decide whether a partial placeholder
expression was checked in a function-valued expected context. It never read or
validated the duplicated runtime `arity` payload.

The runtime evidence record itself, expression ID, optional project-function
owner, record order, and required/actual evidence-count gate are unchanged. The
compiler still maps every semantic `ExpectedFunctionValue` record to exactly
one runtime record. No `filter_map`, sentinel arity, renamed field, wrapper,
compatibility variant, or dual reader was introduced.

## Preserved contract

`TypedLoweringEvidenceKind::FunctionEffectCallable` and its compiler to
runtime-plan projection remain intact. A zero-consumer scan alone was not
sufficient evidence for deleting that variant: the resolved 07.8 repository
contract and
[`function-stack-closure-effect-callable-evidence-2026-07-09.md`](function-stack-closure-effect-callable-evidence-2026-07-09.md)
explicitly preserve the callable identity for downstream runtime-plan
consumers. This cut therefore rejects that broader deletion candidate.

The corrected Proof 01.1.1.4.1 archive remains only partially
implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).
This payload deletion does not guess any blocked leaf-expression, PatternId,
TypeId, region, overflow, or path-root decision.

## Direct evidence

The existing compiler integration
`runtime_plan_uses_expected_function_evidence_for_placeholder_args` passes
without an authored expected type at runtime-plan lowering. It proves that the
presence-only fact still lowers `_ > 80i64` to the expected one-parameter
runtime function before it is supplied to `accept`.

The runtime-plan public API trybuild row now also attempts to construct
`ExpectedFunctionValue { arity: 1 }` and is rejected by Rust type checking.
This is public type evidence, not a source-text gate.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-compiler --all-targets --all-features`: passed;
- the updated runtime-plan public API trybuild row: passed;
- exact compiler
  `runtime_plan_uses_expected_function_evidence_for_placeholder_args`: passed;
- `cargo test -p arcweft-runtime-plan --all-targets --all-features`: passed,
  including 114 unit tests, the API compile-fail row, assertion identity, 58
  AWBC parity tests, three iterator-witness tests, and 51 runtime-plan
  integration tests;
- `cargo test -p arcweft-compiler --all-targets --all-features`: passed,
  including all 92 unit tests and every compiler integration/compile-fail
  suite;
- strict changed-crate Clippy for `arcweft-runtime-plan` and
  `arcweft-compiler`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-tier2`: passed in 254 seconds, including MCP stdio, native
  capture, animated image, object-ID/mask, typewriter/ruby, visual smoke, and
  IMQ golden rows; and
- `git diff --check`: passed.

`just test-workspace` ran for 1,280.8 seconds. It passed the changed compiler
and runtime-plan suites, the updated compile-fail row, typed-evidence alignment
tests, and every preceding workspace suite. It stopped only at the established
`arcweft-cli --test arcw_fixtures_check_run` baseline. The exact suite was
rerun and reported three passes plus the same two failures present at the
parent revision:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows require final attached-HIR publication of the capability-owned
`FsError`. This cut does not touch that owner and adds no fallback nominal,
fixture bypass, compatibility reader, or source gate.

The final ZIP ledger compared all 30 retained `docs/reviews/**/*.zip`
archives against implementation records: zero unrecorded hashes and zero
root-inbox ZIPs.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-runtime-expected-function-evidence-payload-deletion-2026-07-27/`](structure-audits/proof-runtime-expected-function-evidence-payload-deletion-2026-07-27/).
Its final pass scanned 3,791 files, including 1,959 Rust files and 905,889
physical Rust LOC across 95 manifests. It reported zero errors and 146 existing
warnings.

Current changed-file metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-compiler/src/lower.rs` | 10,453 | 251 | production compiler projection |
| `arcweft-runtime-plan/src/typed_evidence.rs` | 9,787 | 294 | production runtime evidence owner |
| `arcweft-runtime-plan/tests/ui/removed_zero_consumer_runtime_plan_facades.rs` | 635 | 22 | compile-fail test |

All production owners remain below structural warning thresholds. No manifest,
dependency edge, feature, serialized format, runtime opcode, or crate boundary
changed.

## Next boundary

The Agent REPL source-module visibility remains an independently audited small
deletion candidate. Active Proof leaf readers, numeric/Duration/compact
sequence readers, Dialogue carriers, and 07.8 function-effect callable
identity remain frozen or retained until their typed replacement contract is
ready.
