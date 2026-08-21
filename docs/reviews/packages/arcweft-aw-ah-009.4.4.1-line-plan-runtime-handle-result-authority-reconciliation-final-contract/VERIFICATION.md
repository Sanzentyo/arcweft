# Verification record and evidence boundary

## 1. Fixed inputs

| Input | Verification |
|---|---|
| user request | read in full from `/mnt/data/2026-08-21-aw-ah-009.4.4.1-line-plan-runtime-handle-result-authority-reconciliation.md`; SHA-256 `d84fa7828c8cfad6750b3c7c13dee5d74e0201337d43d96081dfba17d5d4b43a` |
| Arcweft premise | read in full from `/mnt/data/前提(Sanzentyo-arcweft).txt`; SHA-256 `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` |
| Rust skill | read in full through final line from `/mnt/data/Rust Skill.txt`; SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| repository | private `Sanzentyo/arcweft` accessed through the GitHub connector |
| fixed current main | `9138efeeabdfca56809e8ad9c16fc85380ae18c5` |
| request commit | repository commit `701a125539dd2116c1d3cc6b283f548a96e7f2ee` |
| resolved preceding production commit | `15ad861a9b89e8b4b69f40381d00e74ab7392961` |

The request text itself records inspected baseline
`15ad861a954249a9430b32d53ae0fc79c019a4f0`.  That exact string was preserved
in `SOURCE_REQUEST.md`.  It does not equal the repository-resolved preceding
production commit above; this contract records both and uses current main for
source truth.

## 2. Policy read

The latest fixed-SHA versions of these instructions were read before design:

```text
AGENTS.md
crates/AGENTS.md
docs/implementation/AGENTS.md
```

The design applies their spec-first, one-way-layering, Sans-I/O, typed-error,
deterministic/bounded-work, AWBC-parity, and compile-clean requirements.

## 3. Source evidence directly inspected

Representative current source and maintained contracts inspected through the
connector include:

```text
README.md
docs/00-overview/crate-map.md
docs/implementation/2026-08-21-dialogue-line-plan-typed-ownership.md
docs/01-language/dialogue-line-handles-and-returns.md
docs/01-language/dialogue-calls-scopes-cancellation.md
docs/reviews/packages/arcweft-aw-ah-009.4-character-dialogue-first-class-runtime-final-contract/FINAL_CONTRACT.md
.../TEST_MATRIX.md
crates/arcweft-lang-hir/src/dialogue_application.rs
crates/arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs
crates/arcweft-lang-sema/src/callable/identity.rs
crates/arcweft-lang-sema/src/callable/schema/families.rs
crates/arcweft-dialogue/src/character_dialogue/runtime_type.rs
crates/arcweft-core/src/value.rs
crates/arcweft-core/src/value/opaque.rs
crates/arcweft-core/src/value/ownership.rs
crates/arcweft-core/src/pattern.rs
crates/arcweft-core/src/plan.rs
crates/arcweft-core/src/plan/dialogue_content.rs
crates/arcweft-core/src/line_task.rs
crates/arcweft-core/src/effect.rs
crates/arcweft-core/src/engine.rs
crates/arcweft-runtime-plan/src/awbc_lower/flow.rs
crates/arcweft-runtime-plan/src/awbc_lower/line.rs
crates/arcweft-core/src/awbc/schema.rs
crates/arcweft-core/src/awbc/vm.rs
crates/arcweft-core/src/awbc/verify/code.rs
crates/arcweft-core/src/awbc/codec/code.rs
crates/arcweft-core/src/awbc/fiber.rs
crates/arcweft-core/src/awbc/product_step/mapping.rs
crates/arcweft-bundle/src/product.rs
crates/arcweft-bundle/src/patch.rs
crates/arcweft-runtime-host/src/bundle_runner.rs
crates/arcweft-cli/src/output.rs
crates/arcweft-cli/src/app/bundle.rs
tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw
tests/fixtures/arcw/current_pass/check/011_dialogue_with_plan.arcw
```

The consumer inventory distinguishes directly observed files from required
implementation consumers; it does not claim that every listed production file
was downloaded locally.

## 4. Current-source facts used

The fixed baseline established all of the following before this design:

- HIR/sema preserve source-ordered line statements, exact handle return types,
  scheduled callback, and `out (voice, cue)`;
- `FlowOp::Dialogue` currently has no result target;
- `DialogueState` currently has no exact result cell/target and no generation
  pinned handle ledger;
- existing line child fibers receive cloned captures and have no typed result
  authority channel;
- `RuntimeValue::Opaque`, `RuntimeOpaqueTypeOwner`, and generic producer
  validation already exist and are the correct value owner to extend;
- `RuntimeValue::ownership` currently treats opaque ownership as payload
  ownership and has no constructible affine opaque leaf;
- `LineEffectRequest` still contains string RegisterHandle/DropHandle/Out and
  `LineOutRequest` still stores a string value;
- `AwbcEffectKind` still contains corresponding old kinds;
- AWBC ABI and codec versions are `1`;
- AWBC currently reserves opcode holes `0x1e` and `0x20`, while Dialogue is
  `0x86` without a result destination;
- `LineTaskGroup` and the common reducer topology already exist and are
  preserved rather than replaced.

## 5. What was and was not executed

This archive is design-only.  No production source was modified and no
production overlay is included.  The private repository was inspected at a
fixed SHA through the GitHub connector; it was not materialized as a local
build tree in this run.  Therefore no claim is made that `cargo check`, Clippy,
workspace tests, native rendering, Web runtime, or CLI execution were run for
this proposed design.

The archive itself was locally validated for:

- exact requested archive filename;
- `OPEN_QUESTIONS.md` exact byte content `none`;
- Markdown-only design payload plus checksum manifest;
- presence of all required output categories;
- requirement rows 1–15;
- exact fixed SHA and source hashes;
- no production code overlay;
- ZIP integrity and per-file SHA-256 manifest.

The production verification commands and exact required test rows are in
`IMPLEMENTATION_INTERLEAVE.md` and `TEST_MATRIX.md`.
