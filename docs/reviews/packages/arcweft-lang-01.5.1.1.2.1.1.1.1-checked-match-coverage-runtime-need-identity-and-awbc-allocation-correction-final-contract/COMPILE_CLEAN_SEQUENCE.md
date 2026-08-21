# Deletion-driven compile-clean implementation sequence

Each step is reviewable and compile-clean. A commit contains only final owners;
no empty catalog, dummy enum variant, compatibility branch, duplicate identity,
or conflicting byte meaning is permitted.

## Cut 1 — checked semantic authorities

1. Extend `FinalSemanticCatalogs::production` with the verified
   `ResourceTypeRegistry` and existing digest.
2. Add `CheckedOwnershipContext`, complete TypeKind mapping, limits, cycle paths,
   and producer argument/capture admission.
3. Extend the original `AcceptedNominalSemantics::Opaque` variant with existing
   value-class/persistence facts and migrate all constructors/matches in the
   same commit.
4. Add `MatchCoverageAnalyzer`, its bounded usefulness matrix, diagnostics, and
   property oracle tests.
5. Add generic `CheckedMatch`, private coverage construction, stable semantic
   encoders, and bidirectional final-HIR completeness. Replace Match
   `Structural` rows atomically.

## Cut 2 — checked View catalog and stable coordinates

1. Complete the checked View Need Match catalog with only a `CheckedMatchRef`.
2. Publish View-owned site/arm/output/local/body coordinates.
3. Add compiler one-way projection and `RuntimePlanSemanticFactInput` seed.
4. Reject copied coverage/types/effects/source coordinates and HIR IDs in
   product rows.

## Cut 3 — existing AWBC primitive migration and identity substrate

This cut changes no feature meaning and adds no pending variants.

1. Convert only currently executable `AwbcOpcode` and `AwbcFunctionKind` rows to
   repr discriminants, ALL-derived decode tables, direct numeric Serde/Wire.
2. Introduce typed `AwbcFunctionFlag`/private `AwbcFunctionFlags` for current
   bits 0..3 and migrate every caller.
3. Make all ordinary u32 wire values canonical varints, delete `usize::Wire`,
   repair tensor shapes, and install the one-buffer encoder with rollback.
4. Replace `NeedId`, `TaskKey`, and `TaskId` String owners across ordinary task,
   direct Await, AwaitMany, structured parity, snapshots, restore, scheduler,
   events, and runtime-plan interning.
5. Replace `AwbcTaskPlan.need_id` with mandatory `AwbcTaskProducer`, and migrate
   codec/verifier/bundle/product-step/fixtures together.

## Cut 4 — CopyValue

Publish `CopyValue=0x2a` only with total checked ownership evidence, AWBC
verifier, VM `step/step_with_host`, structured path, AOT implementation, exact
wire golden, and parity tests. Delete its previous feature-local allocation
claim in the same cut.

## Cut 5 — typed Need producer

Publish `MakeNeedHandle=0x29`, `AwbcRuntimeType::NeedHandle { payload }`,
`RuntimeValue::NeedHandle`, Synthetic Need-producer verification, and flag bit 5
together. The runtime handle constructor consumes only checked producer
admission. Delete payloadless/String/Dynamic Need routes and old View Await
producer binding in this cut.

## Cut 6 — timeout

Publish `NeedTimeout=0x1e` only after the identity substrate is complete. Add
source/output Need identities, typed limit verification, VM/structured/AOT,
snapshot/replay and maintained timeout lifecycle parity. Delete any local
0x1e table.

## Cut 7 — line-plan

Publish `CommitDialogueResult=0x20`, `ExecuteLineOperation=0x2b`, and
`LineActivation=10` only with the complete accepted line-operation table,
verifier, functional VM/structured/AOT, result authority, snapshots and tests.
This supersedes the stale line allocation without a reader for 0x1e.

## Cut 8 — protected Stream

Publish `OpenStream=0x2c`, `FinishStream=0x2d`,
`ApplyExternalStreamGroup=0x2e`, `NextStream=0x8f`, `YieldStream=0x90`, kinds
`Ordinary=8`/`GeneratorProducer=9`, and flag bit 4 only in the complete protected
Stream cut. Preserve the accepted grouped operand owners and delete the old
0x27/0x28/0x29 feature table and goldens atomically.

## Cut 9 — selector/producer core and runtime-plan projection

Lower explicit pattern/bind/guard/Branch selector control flow, produce the
owning synthetic Variant/Tuple result, construct the verified typed Need
producer, and bind its final AWBC/task-plan coordinates. No register escapes and
View retains no core values.

## Cut 10 — staged persistent consumers

Migrate bundle content roots, runtime journal, observers, task events, AwaitMany
snapshots, timeout snapshots, save/replay, and explicit replacement mapping to
typed identities/digests. Native/Web/Agent adapters serialize shared owners and
add no DTO.

## Cut 11 — final atomic publication switch

Enable the complete checked View Need Match path, install private runtime-driver
decode/install, run all Tier-1/Tier-2 tests, then delete every superseded View
Await, payloadless/String NeedHandle, string task identity, suffix parser,
fallback resolver, stale request fixture, duplicate numeric map, and old bundle
row. Publication is blocked unless the deletion matrix is empty.
