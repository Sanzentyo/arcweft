# Exact deletion and replacement matrix

The implementation must confirm the repository-wide inventory at its actual
starting SHA.  The paths below are the current known owners/consumers at the
design baseline.

| ID | Current symbol/shape | Owner/consumers | Final action | Replacement/proof |
|---|---|---|---|---|
| DEL-01 | `TypeKind::Named("StageApi")` | sema types/schema/tests | delete | `TypeKind::StageApi(CharacterId)` non-value |
| DEL-02 | `TypeKind::Named("StageActorHandle")` | sema stage signatures/tests | delete | direct `StageActorHandle(Exact/Any)` plus exact opaque projection |
| DEL-03 | `TypeKind::Named("CueHandle")` | sema stage/at signatures/tests | delete | direct `TypeKind::CueHandle` |
| DEL-04 | `TypeKind::Named("VoiceHandle")` | sema line-context signature/tests | delete | direct `TypeKind::VoiceHandle` |
| DEL-05 | `LineContext.voice_handle` branch in `CapacityMethodId` | `callable/identity.rs`, schema/tests | delete | `LineContextMethodId::VoiceHandle` |
| DEL-06 | checked Stage/at/line callable runtime semantic exclusion | compiler/runtime-plan fact projection | delete | exact `runtime_line_operation` mapping on original callable enums |
| DEL-07 | `LineEffectRequest::RegisterHandle { key:String, handle:String }` | core effect, observation, runtime host, CLI, bundle, AWBC mapping/tests | delete enum variant and every arm | opaque value issuance + ledger |
| DEL-08 | `LineEffectRequest::DropHandle { key:String }` | same | delete enum variant and every arm | original affine `Drop` operation + ledger typed command |
| DEL-09 | `LineEffectRequest::Out(LineOutRequest)` | same plus engine suspension | delete | `CommitDialogueResult` and dialogue result target |
| DEL-10 | `LineOutRequest { label, value:String }` | `core/src/line_task.rs` and serde/tests | delete type | exact `RuntimeValue` in hidden result cell |
| DEL-11 | `AwbcEffectKind::RegisterHandle` | schema/codec/verify/VM/product/lowering/tests | delete tag/reader/writer | line operation opcode/table |
| DEL-12 | `AwbcEffectKind::DropHandle` | same | delete | existing typed Drop opcode and ledger |
| DEL-13 | `AwbcEffectKind::Out` | same | delete | opcode `0x20 CommitDialogueResult` |
| DEL-14 | old `AwbcTerminator::Dialogue` payload without result | schema/codec/verify/VM/lowering/tests | replace in place | typed result target in `0x86`; old payload rejected |
| DEL-15 | `AwbcLineTaskTrigger::DelayNanos` authored-at route | schema/codec/verify/VM/lowering/tests | replace | `Scheduled(site)` plus evaluated Schedule args |
| DEL-16 | string effect inventory/descriptors for old variants | `awbc_lower/inventory.rs`, product mapping | delete | typed line-operation table |
| DEL-17 | string effect observation renderers | `core/src/observation.rs`, CLI output | delete | normalized typed line observations |
| DEL-18 | runtime-host/bundle runner old match arms | `runtime-host/src/bundle_runner.rs` | delete | typed command/result execution |
| DEL-19 | CLI bundle/output old match arms | `cli/src/output.rs`, `cli/src/app/bundle.rs` | delete | typed observation rendering only |
| DEL-20 | fixture exception/edge skip for RUN-037 | CLI fixture tests/config | delete | ordinary check/structured/AWBC/CLI pipeline |
| DEL-21 | old string handle/out unit tests and snapshots | core/AWBC/host/CLI tests | delete | positive typed and negative API-absence rows |
| DEL-22 | any source/callee-name recognizer added during implementation | compiler/runtime-plan/runtime | forbidden/delete | accepted callable identity only |
| DEL-23 | any copied line opaque-producer map | line lowering/VM/host | forbidden/delete | existing RuntimePlan/AWBC runtime type owner table |
| DEL-24 | any dynamic/untyped dialogue result register | runtime/AWBC | forbidden/delete | exact `RuntimePlanTypeId`/`AwbcTypeId` target and cell |
| DEL-25 | any compatibility alias, shim, dual reader, version >1 | all crates | forbidden/delete | one in-place version-1 shape |

## Repository search proof

After implementation, the production tree must satisfy equivalent checks:

```text
rg 'RegisterHandle|DropHandle|LineOutRequest|AwbcEffectKind::Out' crates tests
    -> only negative compile fixtures or historical design/review documents

rg 'Named\("(StageApi|StageActorHandle|CueHandle|VoiceHandle)"\)' crates tests
    -> no production/test construction

rg 'voice_handle' crates/arcweft-lang-sema/src/callable
    -> dedicated LineContextMethodId path, no CapacityMethodId branch

rg 'DelayNanos' crates/arcweft-core/src/awbc crates/arcweft-runtime-plan
    -> no authored-at schema/reader

rg 'AWBC_(ABI|CODEC)_VERSION' crates
    -> relevant constants equal 1
```

History/review/source-request text is not rewritten merely to make searches
empty; production selection proof is compile/API based.
