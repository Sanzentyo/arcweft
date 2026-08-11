# Runtime assertion-fault identity and serialization boundary

## 1. Boundary decision

Runtime assertion dispatch and persisted data use a typed artifact-stable guard key. Exact HIR identity remains in a separate session-only runtime-plan inventory.

`arcweft-core` continues to own serialized runtime data. It does not depend on syntax or HIR. `arcweft-runtime-plan` owns the mapping from executable guards to `StmtId`, zero-based condition index, runtime-capable mode, and exact revision-bound `SourceSpan`. CLI/LSP/Agent/debug presentation projects core failure data through that inventory when an exact fresh-session association exists.

## 2. Core persisted/runtime-data types

The following live in `arcweft-core::effect` or the existing core artifact identity module:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RuntimeAssertionGuardId([u8; 16]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RuntimeArtifactFingerprint([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeAssertion {
    guard: RuntimeAssertionGuardId,
    condition: String,
    message: String,
    profile: RuntimeAssertionProfile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeAssertionFailure {
    assertion: RuntimeAssertion,
}

impl RuntimeAssertion {
    pub fn new(
        guard: RuntimeAssertionGuardId,
        condition: String,
        message: String,
        profile: RuntimeAssertionProfile,
    ) -> Self;
    pub fn guard(&self) -> RuntimeAssertionGuardId;
    pub fn condition(&self) -> &str;
    pub fn message(&self) -> &str;
    pub fn profile(&self) -> RuntimeAssertionProfile;
}

impl RuntimeAssertionFailure {
    pub fn new(assertion: RuntimeAssertion) -> Self;
    pub fn assertion(&self) -> &RuntimeAssertion;
    pub fn into_assertion(self) -> RuntimeAssertion;
}
```

`RuntimeAssertionProfile::{Always, DebugOnly}` remains the runtime-data profile authority. Existing condition/message fields retain their materialized runtime-data meaning. The only added dispatch field is the typed guard ID. Because the payload fields remain private, runtime-plan constructs the core payload through `RuntimeAssertion::new`; no downstream crate performs a field-by-field private projection or local helper conversion.

Guard/fingerprint constructors are checked data constructors, not session-ID constructors:

```rust
impl RuntimeAssertionGuardId {
    pub fn try_from_bytes(bytes: [u8; 16]) -> Result<Self, RuntimeIdentityDecodeError>;
    pub const fn as_bytes(&self) -> &[u8; 16];
}

impl RuntimeArtifactFingerprint {
    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, RuntimeIdentityDecodeError>;
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeIdentityDecodeError {
    #[error("runtime assertion guard must not be all zero")]
    ZeroAssertionGuard,
    #[error("runtime artifact fingerprint must not be all zero")]
    ZeroArtifactFingerprint,
}
```

All-zero values are reserved and rejected. Serde encodes fixed byte arrays through the workspace's canonical binary/structured representation; identity is never a free string. No textual parser or string codec is implemented. Generic debug/data presentation can render the bytes, but construction always uses the checked byte newtype.

`RuntimeArtifactFingerprint` is exactly the 32 digest bytes from the existing `arcweft-project::ArtifactKey::digest()` for `QueryKind::RuntimePlan` / `ArtifactKind::RuntimePlan`. The compiler integration copies those bytes into the core transport newtype through `try_from_bytes`; core does not import `arcweft-project`. No second runtime artifact digest is calculated.

## 3. Session-only runtime-plan types

These types live in `arcweft-runtime-plan::assertion_identity` and implement no Serde:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAssertionMode {
    Check,
    Debug,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssertionConditionIndex(u8);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssertionPresentation {
    statement_span: SourceSpan,
    condition_label: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionSite {
    guard: RuntimeAssertionGuardId,
    statement: StmtId,
    condition: AssertionConditionIndex,
    mode: RuntimeAssertionMode,
    condition_span: SourceSpan,
    presentation: AssertionPresentation,
}

#[derive(Clone)]
pub struct RuntimeAssertionInventory {
    artifact: RuntimeArtifactFingerprint,
    sites: BTreeMap<RuntimeAssertionGuardId, RuntimeAssertionSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionFaultIdentity {
    statement: StmtId,
    condition: AssertionConditionIndex,
    mode: RuntimeAssertionMode,
    span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAssertionFault {
    identity: RuntimeAssertionFaultIdentity,
    guard: RuntimeAssertionGuardId,
    presentation: AssertionPresentation,
    observed: RuntimeAssertion,
}
```

`RuntimeAssertionFaultIdentity::span` is the exact authored condition expression span, not merely the whole statement. `AssertionPresentation::statement_span` provides the statement label separately. The observed runtime message remains in `RuntimeAssertion` and is never parsed to recover identity.

All fields above are private. `assertion_lower.rs` alone constructs sites and inventories; `assertion_projection.rs` alone constructs faults. Public immutable access is exactly:

```rust
impl AssertionPresentation {
    pub fn statement_span(&self) -> &SourceSpan;
    pub fn condition_label(&self) -> &str;
}

impl RuntimeAssertionSite {
    pub fn guard(&self) -> RuntimeAssertionGuardId;
    pub fn statement(&self) -> StmtId;
    pub fn condition(&self) -> AssertionConditionIndex;
    pub fn mode(&self) -> RuntimeAssertionMode;
    pub fn condition_span(&self) -> &SourceSpan;
    pub fn presentation(&self) -> &AssertionPresentation;
}

impl RuntimeAssertionFaultIdentity {
    pub fn statement(&self) -> StmtId;
    pub fn condition(&self) -> AssertionConditionIndex;
    pub fn mode(&self) -> RuntimeAssertionMode;
    pub fn span(&self) -> &SourceSpan;
}

impl RuntimeAssertionFault {
    pub fn identity(&self) -> &RuntimeAssertionFaultIdentity;
    pub fn guard(&self) -> RuntimeAssertionGuardId;
    pub fn presentation(&self) -> &AssertionPresentation;
    pub fn observed(&self) -> &RuntimeAssertion;
}
```

## 4. Runtime-capable mode conversion

The existing syntax/HIR `AssertionMode` remains the source authority. The runtime-plan-owned enum performs conversion through an inherent constructor:

```rust
impl RuntimeAssertionMode {
    pub fn try_from_assertion_mode(
        mode: AssertionMode,
    ) -> Result<Self, RuntimeAssertionModeError>;
}

pub enum RuntimeAssertionModeError {
    ProveHasNoRuntimeRepresentation,
}
```

Exact mapping:

- `AssertionMode::Check -> RuntimeAssertionMode::Check`;
- `AssertionMode::Debug -> RuntimeAssertionMode::Debug`;
- `AssertionMode::Prove -> Err(ProveHasNoRuntimeRepresentation)`.

There is no `Prove` variant in `RuntimeAssertionMode`, no public unchecked constructor for a fault/site, and no runtime guard arm for prove assertions. The repository-owned `AssertionMode` inherent implementation gains exactly `pub const fn is_runtime_capable(self) -> bool`, returning false only for `Prove`; no extension trait is used.

## 5. Condition index

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AssertionConditionIndexError {
    #[error("assertion condition count must be in 1..=64")]
    InvalidConditionCount { count: usize },
    #[error("assertion condition index is outside the authored condition list")]
    OutOfBounds { index: usize, count: usize },
}

impl AssertionConditionIndex {
    pub fn try_new(
        index: usize,
        condition_count: usize,
    ) -> Result<Self, AssertionConditionIndexError>;
    pub const fn get(self) -> u8;
}
```

Indices are zero-based authored order. Valid count is 1 through 64. `try_new(0, 1)` succeeds, `try_new(63, 64)` succeeds, and `try_new(64, 64)` fails. The index is validated while lowering each typed condition and again when projecting a failure.

## 6. Guard derivation

A guard is deterministic for one persisted runtime artifact but contains no session ID. Runtime-plan derives it from the canonical typed seed:

```rust
struct RuntimeAssertionGuardSeed<'a> {
    schema: u16,
    package: &'a CallablePackageId,
    module: &'a CanonicalModulePath,
    callable: &'a CallableDeclarationId,
    assertion_ordinal: u32,
    condition: AssertionConditionIndex,
    profile: RuntimeAssertionProfile,
}
```

`assertion_ordinal` is zero-based authored assertion-statement order within the callable after profile-independent HIR lowering. It is not a raw `StmtId` slot and remains stable in the persisted plan when unrelated module arenas change.

Derivation is:

```text
BLAKE3 derive-key context = "arcweft.runtime.assertion-guard.v1"
input = canonical length-prefixed binary encoding of RuntimeAssertionGuardSeed
output = first 16 bytes
all-zero output = same bytes with final byte set to 1
```

The guard key is generated from typed canonical IDs/enums and ordinals. It never parses source or message strings. The exact schema value for this cut is `1`.

The runtime artifact fingerprint is copied from the canonical existing runtime-plan `ArtifactKey` digest. The key is derived from the canonical query inputs for the completed runtime plan, including build/profile/source/dependency identities; the session-only inventory is not an artifact-key input and is never persisted.

## 7. Runtime-plan lowering flow

For each HIR assertion statement in executable non-proof context:

1. resolve the exact `StmtId` and `HirStmtKind::Assertion`;
2. reject `Prove` from runtime conversion;
3. visit typed condition `ExprId`s in authored order;
4. construct validated zero-based `AssertionConditionIndex`;
5. obtain the condition's exact `SourceSpan` and statement span from HIR metadata;
6. derive a source label from the exact document slice of the condition span;
7. derive `RuntimeAssertionGuardId` from typed canonical seed;
8. emit one runtime guard/core assertion payload per condition;
9. stage one `RuntimeAssertionSite` in the session inventory;
10. after persisted plan construction, bind the complete inventory to the exact `RuntimeArtifactFingerprint`.

No line-plan assertion clone or raw assertion source string is consulted.

## 8. Check, Debug, and Prove behavior

### 8.1 Check

Every `Check` condition emits a runtime guard with `RuntimeAssertionProfile::Always` and one inventory site. On a failed condition, the runtime produces `RuntimeAssertionFailure` retaining the guard and materialized condition/message/profile.

### 8.2 Debug

In Debug build profile, every enabled `Debug` condition emits a guard with `RuntimeAssertionProfile::DebugOnly` and one inventory site.

In Release build profile, the condition evaluation, runtime guard, core assertion payload, guard seed, inventory site, and any generated instruction/effect are all omitted. Authored HIR remains available to source tooling but no runtime identity entry exists.

### 8.3 Prove

`Prove` is consumed by semantic/verification paths only. Runtime-plan conversion returns the typed error if invoked, and normal proof/runtime lowering never calls it. No core `RuntimeAssertion`, guard, site, failure, or fault can be constructed for `Prove`.

## 9. Inventory API and failure projection

```rust
impl RuntimeAssertionInventory {
    pub fn artifact(&self) -> RuntimeArtifactFingerprint;
    pub fn site(
        &self,
        guard: RuntimeAssertionGuardId,
    ) -> Option<&RuntimeAssertionSite>;

    pub fn project_failure(
        &self,
        artifact: RuntimeArtifactFingerprint,
        failure: RuntimeAssertionFailure,
    ) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError>;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeAssertionProjectionError {
    ArtifactMismatch {
        expected: RuntimeArtifactFingerprint,
        actual: RuntimeArtifactFingerprint,
    },
    UnknownGuard { guard: RuntimeAssertionGuardId },
    ProfileModeMismatch {
        guard: RuntimeAssertionGuardId,
        profile: RuntimeAssertionProfile,
        mode: RuntimeAssertionMode,
    },
    InvalidConditionIndex(AssertionConditionIndexError),
}
```

Projection order:

1. require exact artifact fingerprint equality;
2. look up the guard;
3. validate profile/mode (`Always` with Check, `DebugOnly` with Debug);
4. copy session-only identity fields into `RuntimeAssertionFaultIdentity`;
5. retain presentation and observed core data separately.

No message or condition text is parsed.

## 10. Execution-session capability

The freshly compiled execution path retains this non-Serde capability in `arcweft-compiler::runtime_diagnostics`:

```rust
pub struct ExecutionDiagnosticContext {
    artifact: RuntimeArtifactFingerprint,
    assertions: Arc<RuntimeAssertionInventory>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExecutionDiagnosticContextError {
    #[error("runtime assertion inventory belongs to another artifact")]
    ArtifactMismatch {
        expected: RuntimeArtifactFingerprint,
        actual: RuntimeArtifactFingerprint,
    },
}

impl ExecutionDiagnosticContext {
    pub fn try_new(
        artifact: RuntimeArtifactFingerprint,
        assertions: Arc<RuntimeAssertionInventory>,
    ) -> Result<Self, ExecutionDiagnosticContextError>;

    pub fn artifact(&self) -> RuntimeArtifactFingerprint;
    pub fn assertions(&self) -> &RuntimeAssertionInventory;
    pub fn project_assertion_failure(
        &self,
        failure: RuntimeAssertionFailure,
    ) -> Result<RuntimeAssertionFault, RuntimeAssertionProjectionError>;
}
```

`try_new` requires `artifact == assertions.artifact()` and otherwise returns `ArtifactMismatch` without constructing a context. This session object belongs in compiler/runtime integration, which already sees runtime results and compile metadata. It is not added as a normal dependency of `arcweft-runtime-host` on runtime-plan/HIR and it does not create a second source catalog.

The runtime host emits/returns `RuntimeAssertionFailure` as core data. A caller with `ExecutionDiagnosticContext` projects it into the exact HIR fault. A caller without the context can still show persisted source-map/core evidence but must state that fresh HIR identity is unavailable.

## 11. Persisted boundary

### 11.1 Serialized

The following are the complete allowed serialized assertion-identity set:

- `RuntimeAssertionGuardId`;
- `RuntimeArtifactFingerprint`, whose bytes are exactly the existing runtime-plan `ArtifactKey` digest;
- `RuntimeAssertion`, including materialized condition/message/profile;
- `RuntimeAssertionFailure` when a debug trace records an assertion-failure event; the trace also records the owning `RuntimeArtifactFingerprint`;
- existing core/AWBC source-map records;
- bundle, AWBC, save, checkpoint, replay, and cache data that reference only the guard/fingerprint/core payload.

### 11.2 Never serialized

The following never implement Serde and never enter a persisted cache key or codec:

- `SyntaxDatabaseId`, `SyntaxLineageId`, `SyntaxSnapshotId`, `SyntaxNodeId`;
- `HirDatabaseId`, `HirModuleId`, `HirRevision`, `HirSnapshotId`;
- `ItemId`, `ScopeId`, `LocalId`, `ExprId`, `StmtId`, `TypeId`, `PatternId`, `CaptureId`;
- `RuntimeAssertionMode`;
- `AssertionConditionIndex` as a session identity object;
- `RuntimeAssertionSite`, inventory, fault identity, fault, presentation, and execution diagnostic context;
- `ProofArtifactId`.

The zero-based condition index exists in the persisted guard seed/result but is not persisted as a HIR identity record. A decoder cannot reconstruct a `StmtId` from the guard.

### 11.3 Codecs and caches

Core/AWBC/bundle/save/checkpoint/replay/cache codecs are updated in one format-version cut to include typed guard bytes where assertion payloads are encoded. No dual reader or compatibility alias survives the cut. Tests inspect decoded typed data and dependency metadata, not repository source text.

## 12. Reload association

Loading a persisted artifact creates no HIR identity. A fresh session associates it only through the following checked operation; without all prerequisites the operation returns a typed error and installs nothing:

1. compile or otherwise obtain a fresh `RuntimeAssertionInventory` from current source/HIR;
2. compare its `RuntimeArtifactFingerprint` with the loaded artifact's exact fingerprint;
3. only on exact equality, install the inventory in a new `ExecutionDiagnosticContext`;
4. project guard failures to the fresh session's `StmtId` and `SourceSpan`;
5. never compare or claim equality with any old session's unavailable `StmtId`.

A source recompile that produces a different artifact fingerprint cannot attach its inventory to the loaded artifact, even when display names and source bytes appear similar.

Without an exact inventory, presentation uses only persisted core message/condition/profile and AWBC/bundle source map. It emits the same stable diagnostic code but does not fabricate HIR labels.

## 13. Presentation

`arcweft-tooling::runtime_diagnostic` owns the shared presentation model used by CLI, LSP, Agent, and debug adapters:

```rust
pub enum RuntimeAssertionDiagnosticIdentity {
    Session {
        mode: RuntimeAssertionMode,
        condition_index: u8,
    },
    PersistedOnly,
}

pub struct RuntimeAssertionDiagnostic {
    code: &'static str, // always "runtime.assertion_failed"
    message: String,
    primary: Option<RuntimeDiagnosticLabel>,
    secondary: Box<[RuntimeDiagnosticLabel]>,
    identity: RuntimeAssertionDiagnosticIdentity,
}

impl RuntimeAssertionDiagnostic {
    pub const fn code(&self) -> &'static str;
    pub fn message(&self) -> &str;
    pub fn primary(&self) -> Option<&RuntimeDiagnosticLabel>;
    pub fn secondary(&self) -> &[RuntimeDiagnosticLabel];
    pub fn identity(&self) -> &RuntimeAssertionDiagnosticIdentity;
}
```

Constructors are crate-private to `arcweft-tooling::runtime_diagnostic`; adapters call the public projection functions for a session fault or a persisted-only failure. With exact session inventory:

- primary label: condition `SourceSpan`;
- secondary label: assertion statement span;
- message: materialized runtime message when nonempty, otherwise `assertion condition {index} failed`;
- condition label: separately derived authored source label;
- identity: `Session { mode, condition_index }`;
- code: `runtime.assertion_failed`.

Without inventory:

- primary label: persisted AWBC/core source map when present;
- no `StmtId`, mode, condition index, or revision-bound HIR span is claimed;
- identity: `PersistedOnly`;
- code remains `runtime.assertion_failed`;
- message comes from core payload, never identity parsing.

No HIR type moves into `arcweft-core`.

## 14. Dependency invariants

The final normal dependency graph must satisfy:

```text
arcweft-lang-hir -> arcweft-lang-syntax -> arcweft-source
arcweft-runtime-plan -> arcweft-lang-hir + arcweft-core + arcweft-source
arcweft-core -X-> arcweft-lang-hir
arcweft-core -X-> arcweft-lang-syntax
arcweft-runtime-host -X-> arcweft-lang-hir
arcweft-runtime-host -X-> arcweft-lang-syntax
arcweft-runtime-host -X-> arcweft-runtime-plan (normal edge)
```

A development-only runtime-host dependency on runtime-plan for tests remains acceptable only when production code does not import it. The dependency graph test in `TEST_MATRIX.md` checks Cargo metadata structurally.

## 15. Non-goal boundary

This contract defines identity, side-table construction, data flow, omission, serialization, re-association, and presentation. It does not redesign assertion condition evaluation, runtime scheduling, checkpoint semantics, or AWBC instruction architecture. Existing failure emission is adapted to carry the typed guard; later execution work consumes the contract without changing identity ownership.
