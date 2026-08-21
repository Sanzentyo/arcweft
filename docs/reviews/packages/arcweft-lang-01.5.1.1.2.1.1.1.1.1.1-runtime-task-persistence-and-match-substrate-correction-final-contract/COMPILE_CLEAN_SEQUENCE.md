# Corrected five-cut compile-clean sequence

Each cut is a protected commit/merge group. Every published cut must compile
using only current source and types introduced in that or earlier cuts.
Feature-gate names below are implementation staging gates; they are removed or
made unconditional after the protected public switch.

## Cut 1 — Generic Match

**Crates:** `arcweft-lang-sema`  
**Staging gate:** `lang_checked_match_v1`

Publishes:
- `AcceptedDeclarationSemanticId`
- `CheckedExpressionChildRole`
- `CheckedExpressionChildRolePath`
- `StableCheckedValueCoordinate`
- `CheckedLiteralSemanticV1`
- `CheckedExpressionSemanticDigest`
- `CheckedPatternSemanticDigest`
- `CheckedMatchRef`
- `CheckedMatchSemanticDigest`
- `CheckedMatch`
- `CheckedMatchCoverage`
- `CheckedUnreachableArm`

Forbidden in this cut:
- ViewProgramId dependency
- Need/task identity
- RuntimeValue::NeedHandle
- bundle projection
- task snapshot schema

Required gates:
```bash
cargo test -p arcweft-lang-sema --features lang_checked_match_v1
```
```bash
cargo clippy -p arcweft-lang-sema --features lang_checked_match_v1 --all-targets -- -D warnings
```

## Cut 2 — Ownership

**Crates:** `arcweft-lang-sema`, `arcweft-runtime-plan`, `arcweft-core`  
**Staging gate:** `lang_ownership_evidence_v1`

Publishes:
- `AcceptedOpaqueRuntimeEvidence`
- `OwnershipDisposition`
- `OwnershipEvidenceDigest`
- `CheckedNeedProducerAdmissionDigest`
- `RuntimeAgentProtocolRecordProjectionV1`
- `RuntimeAgentClosedVariantProjectionV1`
- `RuntimeDialogueNominalProjectionV1`
- `carrier-backed TypeKind classifier`

Forbidden in this cut:
- View catalog row
- View bundle row
- TaskSpec final schema
- RuntimeValue::NeedHandle
- Shared carrier

Required gates:
```bash
cargo test -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-core --features lang_ownership_evidence_v1
```
```bash
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-core --features lang_ownership_evidence_v1 --all-targets -- -D warnings
```

## Cut 3 — Compiler-local View admission

**Crates:** `arcweft-view`, `arcweft-compiler`, `arcweft-lang-sema`  
**Staging gate:** `view_checked_match_admission_v1`

Publishes:
- `ViewMatchSiteId`
- `CheckedViewMatchAdmissionDigest`
- `CompilerLocalViewMatchCatalogRow`
- `compiler-local exact CheckedMatchRef lookup`

Exact type dependencies:
- `CheckedMatchRef`
- `CheckedMatchSemanticDigest`
- `StableCheckedValueCoordinate`
- `OwnershipEvidenceDigest`
- `CheckedNeedProducerAdmissionDigest`
- `ViewProgramId`
- `AcceptedViewProgramRevision`
- `ViewMatchSiteId`
- `CheckedViewMatchAdmissionDigest`

Forbidden in this cut:
- NeedProducerContractDigest
- TaskPlanSemanticDigest
- RuntimeValueDigest
- TaskSpec
- TaskExecution
- AcceptedViewMatchBundleRowV1
- RuntimeValue::NeedHandle

Required gates:
```bash
cargo test -p arcweft-view -p arcweft-compiler --features view_checked_match_admission_v1
```
```bash
cargo clippy -p arcweft-view -p arcweft-compiler --features view_checked_match_admission_v1 --all-targets -- -D warnings
```

## Cut 4 — Private preparation

**Crates:** `arcweft-core`, `arcweft-runtime-plan`  
**Staging gate:** `runtime_need_v1_prepare`

Publishes:
- `GenerationId`
- `NeedProducerFamily`
- `NeedProducerContractDigest`
- `TaskPlanSemanticDigest`
- `RuntimeTypeSemanticDigest`
- `NeedProducerInstanceKey`
- `NeedId`
- `TaskKey`
- `TaskId`
- `TaskEventDigest`
- `private CanonicalRuntimeValueSink`
- `private BytesSink`
- `private Blake3Sink`
- `explicit RuntimeValue constant-admission validator`

Forbidden in this cut:
- public RuntimeValue::NeedHandle
- public final TaskSpec
- public TaskExecution
- public scheduler/journal schema
- bundle row

Required gates:
```bash
cargo test -p arcweft-core -p arcweft-runtime-plan --features runtime_need_v1_prepare
```
```bash
cargo clippy -p arcweft-core -p arcweft-runtime-plan --features runtime_need_v1_prepare --all-targets -- -D warnings
```

## Cut 5 — Atomic public switch

**Crates:** `arcweft-core`, `arcweft-need`, `arcweft-runtime-plan`, `arcweft-runtime-scheduler`, `arcweft-runtime-driver`, `arcweft-runtime-host`, `arcweft-host-adapter`, `arcweft-adapter-desktop`, `arcweft-player-web`, `arcweft-view`, `arcweft-compiler`, `arcweft-bundle`, `arcweft-agent-runner`, `arcweft-dialogue`  
**Staging gate:** `none after protected merge; all public exhaustive consumers update in one commit`

Publishes:
- `RuntimeValue::NeedHandle`
- `RuntimeNeedHandle manual semantic equality/hash/order`
- `TaskSpec`
- `TaskExecution`
- `RuntimeTaskRequest`
- `TaskCorrelation`
- `TaskEvent`
- `RuntimeTaskScheduler<A>`
- `RuntimeTaskJournal`
- `RuntimeTaskState`
- `RuntimeAwaitManyAggregateTask`
- `RuntimeTimeoutNeed`
- `AcceptedViewMatchBundleRowV1`
- `View runtime generic Match over Need`
- `Await/AwaitMany/timeout`
- `all version-1 snapshot/replay/replacement rows`
- `host adapter prepare/commit/rollback implementations`
- `generated schema/fixtures`

Deletes atomically:
- all String Need/task identities
- TaskSpec.request
- driver RuntimeTaskRegistry
- driver-local GenerationId
- direct-Await surrogates
- old partial events/snapshots
- copied compiler-local View rows in bundles
- old String/hex/legacy readers

Required gates:
```bash
cargo fmt --all -- --check
```
```bash
cargo check --workspace --all-targets --all-features
```
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```bash
cargo test --workspace --all-features
```

## Cut dependency assertion

Cut 3's machine `type_dependencies` list is a strict subset of Cut 1/2/current View owners. The package validator fails if any of `NeedProducerContractDigest`, `TaskPlanSemanticDigest`, `RuntimeValueDigest`, `TaskSpec`, `TaskExecution` or bundle/runtime snapshot types appear there.

Cut 4 may add private sink plumbing and standalone core identity/digest types, but it must not describe a public RuntimeValue variant as private. The public enum change occurs only in Cut 5.

## Cut 5 all-exhaustive-consumer rule

The protected Cut 5 diff must enumerate every exhaustive match on RuntimeValue, TaskExecution, RuntimeTaskRequest, NeedProducerFamily, snapshot rows and generated schemas. A compile failure is preferred to a wildcard arm. No temporary wildcard, String fallback or compatibility route is allowed.

## Implementation completion gate

`READY_FOR_IMPLEMENTATION` remains valid for this design package. Production completion requires all Cut 5 workspace gates plus the focused matrix in `TEST_MATRIX.md`; until then documentation must not claim the production feature is implemented.
