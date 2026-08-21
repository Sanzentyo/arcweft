# Exact Rust-shaped schemas

These declarations are normative design shapes. Existing Arcweft-owned enums
are changed in place and receive inherent methods. No extension trait, ad hoc
parallel enum, raw DTO, compatibility wrapper, or source/string resolver is
authorized.

## 1. Global numeric AWBC owners

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AwbcOpcode {
    Nop = 0x00,
    LoadConst = 0x01,
    Move = 0x02,
    Clear = 0x03,
    EnterScope = 0x04,
    ExitScope = 0x05,
    BindPattern = 0x06,
    TestPattern = 0x07,
    MakeTuple = 0x08,
    MakeSequence = 0x09,
    RepeatSequence = 0x0a,
    SequenceLen = 0x0b,
    SequenceGet = 0x0c,
    SequenceSlice = 0x0d,
    SequencePush = 0x0e,
    MakeRecord = 0x0f,
    MakeVariant = 0x10,
    ProjectTuple = 0x11,
    ProjectRecord = 0x12,
    ProjectField = 0x13,
    Unary = 0x14,
    Binary = 0x15,
    CallPureHelper = 0x16,
    CallIntrinsic = 0x17,
    EnsureContent = 0x18,
    EmitEffect = 0x19,
    StartTask = 0x1a,
    SpawnFiber = 0x1b,
    StreamYield = 0x1c,
    StreamClose = 0x1d,
    NeedTimeout = 0x1e,
    Drop = 0x1f,
    CommitDialogueResult = 0x20,
    AssignRecordField = 0x21,
    CallTraitMethod = 0x22,
    RegisterCleanup = 0x23,
    CancelCleanup = 0x24,
    MakeFunction = 0x25,
    ApplyFunction = 0x26,
    MakeAgent = 0x27,
    MakeReductionUnchanged = 0x28,
    MakeNeedHandle = 0x29,
    CopyValue = 0x2a,
    ExecuteLineOperation = 0x2b,
    OpenStream = 0x2c,
    FinishStream = 0x2d,
    ApplyExternalStreamGroup = 0x2e,
    Jump = 0x80,
    Branch = 0x81,
    Match = 0x82,
    CallFunction = 0x83,
    GotoStatic = 0x84,
    GotoDynamic = 0x85,
    Dialogue = 0x86,
    Choice = 0x87,
    Await = 0x88,
    AwaitMany = 0x89,
    HostCall = 0x8a,
    Return = 0x8b,
    Trap = 0x8c,
    BudgetYield = 0x8d,
    Unreachable = 0x8e,
    NextStream = 0x8f,
    YieldStream = 0x90,
}

impl AwbcOpcode {
    pub const ALL: &'static [Self] = &[
        Self::Nop,
        Self::LoadConst,
        Self::Move,
        Self::Clear,
        Self::EnterScope,
        Self::ExitScope,
        Self::BindPattern,
        Self::TestPattern,
        Self::MakeTuple,
        Self::MakeSequence,
        Self::RepeatSequence,
        Self::SequenceLen,
        Self::SequenceGet,
        Self::SequenceSlice,
        Self::SequencePush,
        Self::MakeRecord,
        Self::MakeVariant,
        Self::ProjectTuple,
        Self::ProjectRecord,
        Self::ProjectField,
        Self::Unary,
        Self::Binary,
        Self::CallPureHelper,
        Self::CallIntrinsic,
        Self::EnsureContent,
        Self::EmitEffect,
        Self::StartTask,
        Self::SpawnFiber,
        Self::StreamYield,
        Self::StreamClose,
        Self::NeedTimeout,
        Self::Drop,
        Self::CommitDialogueResult,
        Self::AssignRecordField,
        Self::CallTraitMethod,
        Self::RegisterCleanup,
        Self::CancelCleanup,
        Self::MakeFunction,
        Self::ApplyFunction,
        Self::MakeAgent,
        Self::MakeReductionUnchanged,
        Self::MakeNeedHandle,
        Self::CopyValue,
        Self::ExecuteLineOperation,
        Self::OpenStream,
        Self::FinishStream,
        Self::ApplyExternalStreamGroup,
        Self::Jump,
        Self::Branch,
        Self::Match,
        Self::CallFunction,
        Self::GotoStatic,
        Self::GotoDynamic,
        Self::Dialogue,
        Self::Choice,
        Self::Await,
        Self::AwaitMany,
        Self::HostCall,
        Self::Return,
        Self::Trap,
        Self::BudgetYield,
        Self::Unreachable,
        Self::NextStream,
        Self::YieldStream,
    ];

    const DECODE: [Option<Self>; 256] = Self::build_decode();

    const fn build_decode() -> [Option<Self>; 256] {
        let mut table = [None; 256];
        let mut index = 0;
        while index < Self::ALL.len() {
            let opcode = Self::ALL[index];
            let byte = opcode as u8 as usize;
            assert!(table[byte].is_none());
            table[byte] = Some(opcode);
            index += 1;
        }
        table
    }

    pub const fn encoded(self) -> u8 { self as u8 }
    pub const fn from_encoded(byte: u8) -> Option<Self> { Self::DECODE[byte as usize] }

    pub const fn class(self) -> AwbcOpcodeClass {
        if self.encoded() >= 0x80 { AwbcOpcodeClass::Terminator }
        else { AwbcOpcodeClass::Instruction }
    }
}

impl serde::Serialize for AwbcOpcode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_u8(self.encoded())
    }
}

impl<'de> serde::Deserialize<'de> for AwbcOpcode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let byte = u8::deserialize(deserializer)?;
        Self::from_encoded(byte)
            .ok_or_else(|| serde::de::Error::custom("unknown AWBC opcode"))
    }
}
```

Private `Wire` writes `encoded()` and decodes through `from_encoded()`. The
instruction and terminator `opcode()` methods return enum variants only. The
compile-time assertion makes duplicate discriminants impossible to publish
through `ALL`.

```rust
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AwbcFunctionKind {
    Flow = 0,
    PureHelper = 1,
    TraitMethod = 2,
    StreamTransform = 3,
    LineTask = 6,
    Synthetic = 7,
    Ordinary = 8,
    GeneratorProducer = 9,
    LineActivation = 10,
}

impl AwbcFunctionKind {
    pub const ALL: &'static [Self] = &[
        Self::Flow,
        Self::PureHelper,
        Self::TraitMethod,
        Self::StreamTransform,
        Self::LineTask,
        Self::Synthetic,
        Self::Ordinary,
        Self::GeneratorProducer,
        Self::LineActivation,
    ];
    // Same ALL-derived [Option<Self>; 256], encoded, from_encoded,
    // numeric Serde and Wire implementation as AwbcOpcode.
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AwbcFunctionFlag {
    MaySuspend = 0,
    MayAllocate = 1,
    Deterministic = 2,
    HasDynamicTarget = 3,
    OwnsStreamProducer = 4,
    NeedProducer = 5,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AwbcFunctionFlags(u32);

impl AwbcFunctionFlags {
    pub const KNOWN_MASK: u32 = 0x3f;

    pub const fn empty() -> Self { Self(0) }

    pub const fn with(self, flag: AwbcFunctionFlag) -> Self {
        Self(self.0 | (1_u32 << flag as u8))
    }

    pub const fn contains(self, flag: AwbcFunctionFlag) -> bool {
        self.0 & (1_u32 << flag as u8) != 0
    }

    pub const fn bits(self) -> u32 { self.0 }

    pub const fn try_from_bits(bits: u32) -> Result<Self, AwbcFunctionFlagsError> {
        if bits & !Self::KNOWN_MASK == 0 { Ok(Self(bits)) }
        else { Err(AwbcFunctionFlagsError::UnknownBits { bits }) }
    }
}
```

The set serializes as one numeric u32 and private Wire emits the ordinary u32
varint. The flag enum itself serializes as numeric u8. Verification is an
inherent method on the existing `AwbcFunction`/kind owner, not a detached
feature table.

## 2. Fixed task/Need identities

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedId([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskKey([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NeedProducerContractDigest([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskPlanSemanticDigest([u8; 32]);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeValueDigest([u8; 32]);

impl NeedId {
    pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
    pub fn to_lower_hex(self) -> String { /* presentation only */ }

    pub(crate) fn try_from_verified_bytes(
        bytes: [u8; 32],
    ) -> Result<Self, RuntimeIdentityError> {
        if bytes == [0; 32] { Err(RuntimeIdentityError::Zero) }
        else { Ok(Self(bytes)) }
    }

    pub(crate) fn derive_host_task(input: HostTaskNeedIdentity<'_>) -> Self;
    pub(crate) fn derive_view(input: ViewNeedIdentity<'_>) -> Self;
    pub(crate) fn derive_line(input: LineTaskNeedIdentity<'_>) -> Self;
    pub(crate) fn derive_await_many_base(input: AwaitManyBaseIdentity<'_>) -> Self;
    pub(crate) fn derive_await_many_child(
        base: Self,
        source_index: u32,
        item: RuntimeValueDigest,
    ) -> Self;
    pub(crate) fn derive_timeout(input: TimeoutNeedIdentity<'_>) -> Self;
}
```

There is deliberately no `From<String>`, `From<&str>`, `FromStr`, suffix helper,
or arbitrary public `from_bytes` constructor. Serde and Wire use fixed 32-byte
rows and the verified constructor.

## 3. Typed task-plan producer row

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcTaskPlan {
    pub public_id: AwbcStringId,
    pub producer: AwbcTaskProducer,
    pub capability: AwbcStringId,
    pub operation: AwbcStringId,
    pub signature: AwbcSignatureId,
    pub class: AwbcTaskClass,
    pub priority: i32,
    pub cancel_scope: AwbcStringId,
    pub policy: AwbcTaskPolicy,
    pub payload_type: AwbcTypeId,
    pub arguments: Vec<AwbcHostArgument>,
    pub many: Option<AwbcAwaitManyPolicy>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AwbcTaskProducer {
    pub family: AwbcTaskProducerFamily,
    pub contract: NeedProducerContractDigest,
    pub site: u32,
    pub plan_digest: TaskPlanSemanticDigest,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcTaskProducerFamily {
    HostTask = 0,
    ViewNeed = 1,
    LineTask = 2,
}
```

`AwbcTaskPlan::verify_semantic_digest(&AwbcProgram)` recomputes the digest and
returns a typed verifier error. Codec, bundle, product-step, fixtures, and
snapshots use this final shape only.

## 4. Typed Need carrier and instructions

```rust
pub enum AwbcRuntimeType {
    // existing variants ...
    NeedHandle { payload: AwbcTypeId },
}

pub struct RuntimeNeedHandle {
    need: NeedId,
    producer_contract: NeedProducerContractDigest,
    payload_type: AwbcRuntimeTypeDigest,
    arguments_digest: RuntimeValueDigest,
    arguments: Box<[RuntimeValue]>,
}

impl RuntimeNeedHandle {
    pub const fn need(&self) -> NeedId { self.need }
    pub const fn producer_contract(&self) -> NeedProducerContractDigest { self.producer_contract }
    pub const fn payload_type(&self) -> AwbcRuntimeTypeDigest { self.payload_type }
    pub const fn arguments_digest(&self) -> RuntimeValueDigest { self.arguments_digest }
    pub const fn arguments(&self) -> &[RuntimeValue] { &self.arguments }

    pub(crate) fn from_verified_producer(
        admission: CheckedNeedProducerAdmission,
        arguments: Box<[RuntimeValue]>,
    ) -> Result<Self, RuntimeNeedHandleError>;
}

pub enum RuntimeValue {
    // existing variants ...
    NeedHandle(RuntimeNeedHandle),
}

pub enum AwbcInstruction {
    // existing variants ...
    NeedTimeout {
        dst: AwbcRegisterId,
        source: AwbcRegisterId,
        limit: AwbcRegisterId,
        producer_site: u32,
    },
    CommitDialogueResult { source: AwbcRegisterId },
    MakeNeedHandle {
        dst: AwbcRegisterId,
        plan: AwbcTaskPlanId,
        site: u32,
        args: Vec<AwbcRegisterId>,
    },
    CopyValue { dst: AwbcRegisterId, source: AwbcRegisterId },
    ExecuteLineOperation {
        dst: AwbcRegisterId,
        operation: AwbcLineOperationId,
        args: Vec<AwbcRegisterId>,
    },
    OpenStream {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
    FinishStream {
        stream: AwbcRegisterId,
        outcome: AwbcStreamProducerOutcome,
    },
    ApplyExternalStreamGroup {
        dst: AwbcRegisterId,
        callee: AwbcRegisterId,
        definition: AwbcStreamDefinitionId,
        signature: AwbcCallableSignatureId,
        group: u16,
        arguments: AwbcExternalStreamGroupArguments,
    },
}
```

These variants land only with their complete feature cuts. The protected Stream
field shapes are unchanged apart from the global opcode allocation.

## 5. Checked Match and private coverage

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatch {
    scrutinee: ExprId,
    arms: Box<[CheckedMatchArm]>,
    coverage: CheckedMatchCoverage,
    semantic_digest: CheckedMatchSemanticDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedMatchCoverage {
    exhaustive: bool,
    unreachable: Box<[CheckedMatchUnreachable]>,
    accounting: MatchCoverageAccounting,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchArmOrdinal(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedMatchUnreachableReason {
    Covered,
    FalseGuard,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedMatchUnreachable {
    arm: CheckedMatchArmOrdinal,
    reason: CheckedMatchUnreachableReason,
}

pub(crate) struct MatchCoverageAnalyzer<'a> {
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
    patterns: &'a BTreeMap<PatternId, CheckedPattern>,
    expressions: &'a BTreeMap<ExprId, CheckedExpression>,
    limits: MatchCoverageLimits,
}

impl<'a> MatchCoverageAnalyzer<'a> {
    fn analyze(
        &self,
        module: &HirModule,
        owner: ExprId,
        scrutinee_type: &TypeKind,
        arms: &[HirMatchArm],
    ) -> Result<CheckedMatchCoverage, MatchCoverageError>;
}

impl CheckedMatch {
    pub(crate) fn try_from_hir(
        module: &HirModule,
        owner: ExprId,
        expressions: &BTreeMap<ExprId, CheckedExpression>,
        patterns: &BTreeMap<PatternId, CheckedPattern>,
        bindings: &BTreeMap<LocalId, CheckedBinding>,
        symbols: &ProjectSymbolTable,
        catalogs: FinalSemanticCatalogs<'_>,
        limits: CheckedMatchLimits,
    ) -> Result<Self, CheckedMatchConstructionError>;
}
```

No constructor accepts a `CheckedMatchCoverage`, `bool`, or unreachable list.
`CheckedExpressionResolution` gains `Match(Box<CheckedMatch>)` directly.

## 6. Ownership context and producer admission

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedOwnershipDisposition { Copy, SnapshotClone }

pub(crate) struct CheckedOwnershipContext<'a> {
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
    resources: &'a ResourceTypeRegistry,
    limits: CheckedOwnershipLimits,
}

impl CheckedOwnershipContext<'_> {
    pub(crate) fn classify_type(
        &self,
        ty: &TypeKind,
    ) -> Result<CheckedOwnershipDisposition, CheckedOwnershipError>;

    pub(crate) fn certify_need_producer(
        &self,
        payload: &TypeKind,
        arguments: &[CheckedExpression],
        captures: &[CheckedBinding],
    ) -> Result<CheckedNeedProducerAdmission, CheckedOwnershipError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedNeedProducerAdmission {
    contract: NeedProducerContractDigest,
    payload_type: SemanticTypeDigest,
    argument_dispositions: Box<[CheckedOwnershipDisposition]>,
    capture_dispositions: Box<[CheckedOwnershipDisposition]>,
    argument_digest: RuntimeValueDigestSchema,
}
```

`AcceptedNominalSemantics::Opaque` gains `value_class` and `persistence` fields
on the original enum variant. No side registry or trait wrapper is introduced.

## 7. Final semantic catalog input

```rust
#[derive(Clone, Copy)]
pub struct FinalSemanticCatalogs<'a> {
    world: &'a RegisteredSemanticWorld,
    resource_types: &'a ResourceTypeRegistry,
    resource_type_digest: ResourceTypeRegistryDigest,
    callable_limits: CallableLimits,
    coverage_limits: MatchCoverageLimits,
    ownership_limits: CheckedOwnershipLimits,
}

impl<'a> FinalSemanticCatalogs<'a> {
    pub fn production(
        world: &'a RegisteredSemanticWorld,
        resource_types: &'a ResourceTypeRegistry,
    ) -> Result<Self, FinalSemanticCatalogInputError>;
}
```

The existing `analyze_final_project(project, symbols, catalogs, control)` remains
the sole final-analysis entry and therefore owns the exact symbols/world/resource
intersection.

## 8. Runtime-plan projection using the existing owner

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeViewMatchSelectorSeed {
    pub coordinate: ProductCheckedMatchCoordinate,
    pub checked_match_digest: CheckedMatchSemanticDigest,
    pub input_state_type: RuntimeSemanticTypeId,
    pub result_type: RuntimeSemanticTypeId,
    pub arms: Box<[RuntimeViewMatchSelectorArmSeed]>,
}

impl RuntimePlanSemanticFactInput {
    pub fn try_insert_view_match_selector(
        &mut self,
        selector: RuntimeViewMatchSelectorSeed,
    ) -> Result<(), RuntimeViewMatchSelectorSeedError>;
}
```

Session HIR IDs may be used inside the compiler while resolving the same
accepted generation, but the final RuntimePlan/AWBC/bundle row stores the stable
coordinate and semantic digests only.

## 9. Functional VM entry points

The existing functions remain the execution boundary:

```rust
pub fn step(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
) -> Result<VmStepOutput, VmError>;

pub fn step_with_host(
    program: &AwbcProgram,
    fiber: &mut FiberState,
    options: VmStepOptions,
    host: &mut impl VmHost,
) -> Result<VmStepOutput, VmError>;
```

Their internal instruction match gains final variants only in each feature cut.
No nominal VM struct is introduced. VM, structured product-step, and AOT paths
consume the same verified semantic owners and must pass differential parity.

## 10. Typed snapshots

```rust
pub struct FiberAwaitManyInFlight {
    pub index: u32,
    pub base_need_id: NeedId,
    pub task_id: TaskId,
    pub task_key: TaskKey,
    pub need_id: NeedId,
    pub producer: NeedProducerContractDigest,
    pub generation: GenerationId,
}
```

All fixed identities encode as 32 raw bytes. Restore uses verified constructors
and transcript recomputation before installing any row.
