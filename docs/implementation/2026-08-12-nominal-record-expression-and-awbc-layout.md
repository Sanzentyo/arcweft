# Nominal-record expression and AWBC layout evidence

Date: 2026-08-12

Inspected Git baseline:
`c1b479d4502993a3b622f88321161af35136598e` on `main`, equal to
`origin/main`, with a clean working tree before the A2 implementation began.

## Implemented state

The nominal-record correction A2 gate is implemented as one compile-clean cut:

- final-HIR nominal record expressions now publish
  `RuntimeExpr::NominalRecord` through a checked carrier that retains the shared
  layout and authored initializer order while assigning accepted field IDs;
- native structured and pure evaluation execute initializer children in
  authored order, scatter results by accepted field ID, and construct values
  through `RuntimeNominalRecordValue::try_from_accepted_layout`;
- nominal record patterns retain the shared layout, validate value identity and
  layout before child matching, and resolve authored field names to accepted
  field IDs instead of using positional `zip` behavior;
- `RuntimePlan::verify` recursively revalidates nominal-record expression
  carriers across flow, helper, trait, stream, source, task, effect, and audio
  expression positions after interim Serde decoding; and
- direct construction of `RuntimeNominalRecordFieldExpr` remains outside the
  public API and is covered by a compile-fail test.

AWBC uses one executable nominal-record descriptor:

- `AwbcRuntimeType::NominalRecord` is tag 24 and carries public nominal ID,
  semantic identity, exact layout hash, and defining-order fields;
- tag 22 `Nominal` remains the identity/layout-only checked predicate used by
  nested checked types;
- tag 25 `Bytes` and tag 26 `Never` preserve the only two previously lossy
  reverse projections (`Bytes` versus `Sequence<U8>`, and `Never` versus an
  empty `Choice`);
- verifier, type compatibility, constants, patterns, VM construction, runtime
  matching, bundle type publication, and codec traversal consume these rows;
- the verifier rejects duplicate executable descriptors for one nominal
  identity/layout, invalid defining-order names or field types, non-image field
  types, cycles, and checked-type nesting beyond 64; and
- VM expression and constant construction reconstruct an ephemeral native
  layout and always use the admitted nominal value constructor.

`AWBC_ABI_VERSION` and `AWBC_CODEC_VERSION` remain fixed at `1`. No old reader,
version bump, migration table, anonymous-record fallback, or parallel nominal
schema was introduced.

## Judgment used

Sol max was used only for the result-changing AWBC owner decision. It selected
the dedicated nominal-record row rather than overloading anonymous `Record` or
identity-only `Nominal`, required exact tag-22/tag-24 compatibility on all three
identity scalars, and identified the `Bytes` and `Never` reverse-projection
collisions. The existing closed checked-type model determined the implementation
after those decisions, so no separate correction request was required.

## Validation performed and passed

- `cargo fmt --all` and `git diff --check`.
- `cargo check --workspace --all-targets --all-features --jobs 4`.
- `cargo clippy --workspace --all-targets --all-features --jobs 4 -- -D
  warnings`.
- `cargo test -p arcweft-core -p arcweft-runtime-plan --all-targets
  --all-features --jobs 4`: core 290 library tests, core compile-fail and
  integration suites, runtime-plan 29 library tests, runtime-plan compile-fail
  and integration suites all passed.
- The focused AWBC nominal expression test proves host calls occur in authored
  order `z,a`, stored values are in layout order `a,z`, and nominal ID/layout
  survive execution.
- Focused tests passed for plan tamper rejection, reversed-field nominal
  pattern matching, wrong-layout rejection, Bytes/Never codec projection, and
  duplicate AWBC descriptor rejection.
- `just structure-audit-gate`: 2,162 files, 2,034 Rust files, 1,007,827 Rust
  physical LOC, 95 packages, 184 review triggers, and zero blocking findings.

## Structural review and continuation

The new expression carrier is isolated in `value/nominal_record_expr.rs`. The
existing large `RuntimeExpr`, plan, AWBC schema/verifier/VM, bundle runtime
codec, CLI scanner, and accelerator files retain their established exhaustive
owner responsibilities; their changes are narrow new-family branches. A
second descriptor registry or copied field map would split authority, so the
AWBC verifier reconstructs from the type table and the VM reconstructs through
the same projection.

A3 anonymous and record-column admitted carriers, A4 deletion of the remaining
unchecked nominal value constructor, and later visitor/persistence closure are
explicitly not claimed by this A2 evidence.
