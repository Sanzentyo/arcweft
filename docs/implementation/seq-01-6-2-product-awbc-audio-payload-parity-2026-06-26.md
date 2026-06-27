# Seq-01.6.2 Product AWBC Audio Payload Parity

## Source package

- Package: `D:/sanze/Downloads/arcweft-seq-01.6.2-awbc-audio-payload-parity.zip`
- Request source: `2026-06-26-seq-01.6.2-product-awbc-audio-payload-parity.md`
- Integration date: 2026-06-27

The package was treated as the source of truth. Its provided patch was generated
against an older tree, so the implementation was manually integrated into the
current seq-01.6.1 split product-step structure rather than applied verbatim.

## Implemented contract

`AwbcProgram` now owns a typed `audio_commands` table. `AwbcEffectPlan` owns
`audio: Option<AwbcAudioCommandId>`, and `AwbcEffectKind::Audio` is valid only
when that field references a typed payload row.

The old product shape where an audio effect carried only
`RuntimeAudioCommand::operation_name()` in `static_args` is rejected by the
verifier. Product execution does not reconstruct audio commands from strings
and does not fall back to structured runtime execution.

AWBC codec version advanced to 5 because the canonical program and effect-plan
wire layouts changed. The `audio_commands` table is encoded after `task_plans`
and before `effect_plans`, with a decode budget entry for the new table.

## Lowering and execution

Flow-level audio effects lower every `RuntimeExpr` field through compact VM
instructions. The evaluated registers are passed as `AwbcInstruction::EmitEffect`
arguments, and the audio payload row stores only field-to-argument references
plus static audio policy values such as loop mode, effect-parameter kind, and
microphone constraints.

Entry-parameter discovery now scans audio effect expressions, so root bindings
used only inside audio commands are still bound into the compact entry frame.

Line-task audio effects have no compact flow frame in the current table shape.
Literal `RuntimeExpr::Value` and `RuntimeExpr::EntityRef` fields are emitted as
constant audio payload refs. Non-literal line-task audio expressions produce a
lowering diagnostic rather than a product fallback.

`AwbcProductStepExecutor` maps typed payload rows into
`AudioCommandEnvelope` requests with deterministic `AudioDispatchId::new(0,
sequence)` dispatch IDs and increments `RuntimeStepStats.audio_commands`.

## Verifier rules

The structural verifier now rejects:

- audio effect plans with no typed payload row;
- non-audio effect plans that carry an audio payload row;
- audio effect plans that keep legacy `static_args`;
- out-of-range `audio_commands` references;
- out-of-range `AwbcAudioValueRef::Arg` indices relative to the effect
  signature arity;
- out-of-range constant references inside audio payload rows.

All of these use `AwbcVerifyError::MalformedAudioPayload { effect, message }`.

## Test coverage added

- codec round-trip for a typed audio payload table row;
- verifier rejection for missing audio payload, non-audio payload, and
  out-of-arity audio arg;
- product-step diagnostics for missing dynamic audio args and invalid audio
  identifiers;
- differential product parity for `audio.stop_all`;
- differential product parity for expression-bearing `SetBusGain` using a root
  binding that is evaluated identically by structured and AWBC product
  execution.

## Validation log

Completed during integration:

```text
cargo check -p arcweft-core -p arcweft-runtime-plan --all-targets
  passed

cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-native --all-targets
  passed

cargo test -p arcweft-core awbc:: --all-targets
  passed: 23 passed, 0 failed

cargo test -p arcweft-runtime-plan awbc -- --nocapture
  passed: 1 awbc_lower unit test and 40 awbc_product_parity tests passed, 0 failed

cargo test -p arcweft-runtime-plan --test awbc_product_parity awbc_product_parity_audio -- --nocapture
  passed: 2 passed, 0 failed

cargo test -p arcweft-bundle product_awbc -- --nocapture
  passed: 4 product_awbc tests and 2 product source-gate tests passed, 0 failed

cargo test -p arcweft-cli awfb -- --nocapture
  passed: 8 passed, 0 failed
  note: the stderr line about a truncated AWFB is the expected rejection path
  from the non-AWFB-input test.

cargo test -p arcweft-runtime-driver awbc_product -- --nocapture
  passed: 1 passed, 0 failed

cargo test -p arcweft-runtime-host awbc_product -- --nocapture
  passed: 1 passed, 0 failed

cargo test -p arcweft-player-native awbc_product -- --nocapture
  passed: 1 passed, 0 failed

cargo fmt --all -- --check
  passed

cargo clippy --workspace --all-targets --all-features -- -D warnings
  passed

cargo +nightly -Zscript tools/structure-audit.rs --root .
  passed with 0 errors, 105 warnings

git diff --check
  passed
```

An earlier combined smoke command timed out at 304 seconds before producing
usable per-command evidence; each smoke command was rerun individually and
passed as recorded above.

## Structural audit measurements

Current working change while measuring: `rrlttntxsqsnlsvzzmotuxotuxotqmqo`
(`aaef4ae0e730`).

Changed Rust files:

| Path | Bytes | Physical LOC | Kind / responsibility |
|---|---:|---:|---|
| `crates/arcweft-core/src/awbc/codec/metadata.rs` | 13136 | 315 | production codec metadata table order |
| `crates/arcweft-core/src/awbc/codec/runtime.rs` | 34988 | 915 | production runtime table wire codecs |
| `crates/arcweft-core/src/awbc/codec/types.rs` | 15600 | 421 | production typed ID codecs |
| `crates/arcweft-core/src/awbc/codec.rs` | 6700 | 186 | production codec facade and budgets |
| `crates/arcweft-core/src/awbc/product_step/audio.rs` | 12162 | 294 | production audio payload adapter |
| `crates/arcweft-core/src/awbc/product_step/mapping.rs` | 12125 | 304 | production effect/task mapping |
| `crates/arcweft-core/src/awbc/product_step/tests.rs` | 17955 | 454 | unit tests |
| `crates/arcweft-core/src/awbc/product_step.rs` | 91377 | 2317 | production product-step orchestrator; warning-level size hotspot, below error threshold |
| `crates/arcweft-core/src/awbc/schema.rs` | 52411 | 1697 | production AWBC schema; warning-level size hotspot, below error threshold |
| `crates/arcweft-core/src/awbc/tests.rs` | 10482 | 290 | unit tests |
| `crates/arcweft-core/src/awbc/verify/structure.rs` | 45009 | 1213 | production structural verifier; warning-level size hotspot, below error threshold |
| `crates/arcweft-core/src/awbc/verify.rs` | 7502 | 185 | production verifier facade/errors |
| `crates/arcweft-runtime-plan/src/awbc_lower/audio.rs` | 11639 | 309 | production audio lowering module |
| `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | 48006 | 1212 | production flow lowering; warning-level size hotspot, below error threshold |
| `crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs` | 47321 | 1142 | production lowerer inventory; below 1200 LOC warning threshold |
| `crates/arcweft-runtime-plan/src/awbc_lower.rs` | 5293 | 151 | production lowerer facade |
| `crates/arcweft-runtime-plan/tests/awbc_product_parity.rs` | 56933 | 1617 | integration differential parity tests; below 2500 LOC warning threshold |

Largest workspace Rust files observed during the audit were existing generated
or broad test/fixture files, led by `arcweft-text-layout/src/vertical_orientation.rs`
at 357456 bytes / 12394 LOC and several CLI exact-check fixtures above 5000 LOC.
The checked-in structure audit reported 0 errors and 105 warnings.

## Design deviations

- The zip patch was not applied verbatim because the current repository already
  had the seq-01.6.1 product-step split. The audio adapter was integrated as
  `product_step/audio.rs` beside the existing private modules.
- No structured product fallback, stringly audio reconstruction, host audio I/O,
  new dependency, unstable Rust feature, macro, `unsafe`, or compatibility shim
  was introduced.
