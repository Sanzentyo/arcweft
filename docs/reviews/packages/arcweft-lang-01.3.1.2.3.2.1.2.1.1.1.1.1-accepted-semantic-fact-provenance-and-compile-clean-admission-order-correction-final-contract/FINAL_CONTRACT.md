# Final contract — accepted semantic-fact provenance and compile-clean admission order

Status: `READY_FOR_IMPLEMENTATION`; `OPEN_QUESTIONS=0`. This is design-only. Every Arcweft-owned version remains `1`.

## 1. Exhaustive accepted expression-type fact
The semantic authority stays `FinalSemanticAnalysis`; do not create a parallel semantic owner. Its complete `expressions: BTreeMap<ExprId, CheckedExpression>` is projected once, after `validate_generation`, into the existing generation-bound `RuntimePlanSemanticFacts`:

```rust
pub struct RuntimeAcceptedExpressionTypeFact {
    expression: ExprId,
    normalized: RuntimeNormalizedType,
}
impl RuntimeAcceptedExpressionTypeFact {
    pub fn new(expression: ExprId, normalized: RuntimeNormalizedType) -> Self;
    pub const fn expression(&self) -> ExprId;
    pub const fn normalized(&self) -> &RuntimeNormalizedType;
}
impl RuntimePlanSemanticFacts {
    pub fn expression_type(&self, id: ExprId) -> Option<&RuntimeNormalizedType>;
}
```

The backing map is private `BTreeMap<ExprId, RuntimeNormalizedType>`. Projection validates exact HIR snapshots plus symbol world/revision already owned by `FinalSemanticAnalysis`; duplicate => `DuplicateExpressionTypeFact`, executable HIR expression missing => `MissingExpressionTypeFact`, stale snapshot => `SnapshotMismatch`, wrong world/revision => `SymbolWorldMismatch`. No runtime value, spelling, root-shape tag, plan or AWBC object may repair a missing fact.

## 2. One borrowed fact; exact type mapping
`FinalExprLowerer`, `FinalPatternLowerer`, final-flow lowering and compiler generation assembly borrow the same `&RuntimePlanSemanticFacts`; no second map is copied. `RuntimeNormalizedType::identity()` is the semantic identity. Checked families are `Never, Unit, Bool, Signed, Unsigned, F32, F64, String, Char, Bytes, Duration, EntityReference, Sequence/Array, ProjectNominal, Tuple, Choice, Result, Option, Opaque`; they use `RuntimePlanTypeKind::Checked` and the existing `checked_type()` projection. Operational families map exactly: `Range→Range`, `Iterator→Iterator`, `Map→Map`, `Need→Need`, `Stream→Stream`, `Source→Source`, `ThreadHandle→ThreadHandle`, `Shared→Shared`, `Reference→Reference`, `Function→Function`; non-checked sequence/composite execution shapes use their closed `RuntimeOperationalType` family. Published unknown/unresolved types are forbidden; projection failure is `UnsupportedAcceptedSemanticType` or the existing `RuntimeCheckedTypeProjectionError`.

## 3. Type ID is interned before typed nodes
`RuntimePlanBuilder` is the only issuer; `RuntimePlanTypeId` gets no public constructor or `From<u32>`.

```rust
impl RuntimePlanBuilder {
    pub fn intern_type(&mut self, declaration: RuntimePlanTypeDeclaration)
        -> Result<RuntimePlanTypeId, RuntimePlanBuildError>;
    pub fn finish(self) -> Result<RuntimePlan, RuntimePlanBuildError>;
}
```

The builder owns a private canonical declaration→ID interner and vector. Exact duplicate declaration returns the existing ID; same semantic identity with unequal kind/authority is `ConflictingTypeDeclaration`. Call order is fixed: validate semantic generation → create builder → obtain accepted normalized type → project declaration → `intern_type` → create type fact with returned ID → create typed expr/pattern → push executable → consuming atomic `finish`.

## 4–6. Provenance/trust and all publication APIs
`AdmittedRuntimeGeneration` is explicitly a **trusted-integrator structural admission result**, not a non-forgeable capability. Public projection rows, builder, v1 fact decoder and `try_issue` are retained. They prove internal consistency, not compiler provenance. All contrary “non-forgeable accepted-world” claims are deleted.

Operational publication requires higher-layer evidence:

```rust
// arcweft-compiler
pub struct CompilerAcceptedRuntimeProduct { /* private AdmittedRuntimeProduct + accepted analysis evidence */ }
pub fn assemble_accepted_runtime_product(
    project: HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    /* accepted catalogs/lowering inputs */
) -> Result<CompilerAcceptedRuntimeProduct, CompilerRuntimeProductError>;

// arcweft-bundle
pub struct VerifiedBundleRuntimeProduct { /* private admitted product + verified container evidence */ }
pub fn load_verified_runtime_product(
    bundle: &VerifiedBundle,
    limits: RuntimeLoadLimits,
) -> Result<VerifiedBundleRuntimeProduct, BundleRuntimeProductError>;
```

Core APIs remain structural: `AdmittedRuntimeGeneration::try_issue`, `generation.try_admit_plan`, `generation.try_admit_awbc`, `admitted_plan.try_admit_awbc`/pair creation. They all require the exact same issued parent. `AdmittedRuntimeProduct` must exist before its inherent `checked_value_context` and `nominal_record_domain` methods land.

A decoded generation-fact section is never operational authority by itself. `VerifiedBundleRuntimeProduct` can be issued only after v1 container framing, unique required sections, size/count limits, integrity/digest checks and configured bundle signature/trust policy have succeeded; then facts decode → structural generation issue → plan admission → AWBC admission → same-parent product → verified wrapper. Public fact decode cannot construct that wrapper. Raw plan/AWBC declarations cannot fill, amend or override generation facts.

Runtime-driver publication, hot swap, restore/replay and public VM/AOT execution accept only `CompilerAcceptedRuntimeProduct` or `VerifiedBundleRuntimeProduct` (or an owner-local enum wrapping exactly those two). Bare structural products are not public operational publication inputs; direct core execution remains crate-private/test substrate.

## 7. Compile-clean implementation order
`IMPLEMENTATION_ORDER.csv` is normative. Type interning lands before external typed lowering. `AdmittedRuntimeProduct` lands before product context/domain inherent methods. No phase refers to a final owner introduced later.

## 8. Test/inventory
`TEST_MATRIX.csv` and `PRODUCER_CONSUMER_DELETION_INVENTORY.csv` are normative and cover every affected compiler/runtime-plan/core/bundle/driver/VM/AOT/restore boundary.

## 9. Nominal record field projection
```rust
impl RuntimeNominalRecordFieldProjection {
    pub fn try_new(field: RuntimeRecordFieldId, ty: RuntimeSemanticTypeId)
        -> Result<Self, RuntimeGenerationProjectionError>;
    pub const fn field(&self) -> &RuntimeRecordFieldId;
    pub const fn ty(&self) -> RuntimeSemanticTypeId;
}
```
Fields stay private. The constructor validates both owning newtypes. `RuntimeNominalRecordProjection::try_new` requires canonical declaration-ordinal order and rejects duplicate field IDs/ordinals. Compiler generation assembly uses only `try_new`; no struct literal or unchecked row exists.

## 10. RuntimePatternBindingCoordinate
Owner: `arcweft-core::plan::typed_sites`.

```rust
pub enum RuntimePatternBindingCoordinate {
    Local { local: LocalId, path: RuntimePatternBindingPath },
    Capture { capture: CaptureId, path: RuntimePatternBindingPath },
}
pub struct RuntimePatternBindingPath(Box<[RuntimePatternBindingStep]>);
pub enum RuntimePatternBindingStep {
    Whole, Tuple(u32), Record(u32), Sequence(u32), Rest, VariantPayload(u32),
}
```
Checked APIs: `try_local`, `try_capture`, `RuntimePatternBindingPath::try_from_steps`, plus read-only accessors. Path nonempty; max 64; `Whole` must be sole step; `Rest` terminal and unique. Canonical order `(family tag, identity, lexicographic path)`. v1 tags: coordinate Local=0/Capture=1; path Whole=0, Tuple=1+u32le, Record=2+u32le, Sequence=3+u32le, Rest=4, VariantPayload=5+u32le. Decoder uses checked constructors. Exact coordinate equality resolves the plan binding; never spelling reconstruction. Errors: `EmptyPath`, `TooDeep`, `WholeNotExclusive`, `RestNotTerminal`, `DuplicateRest`, `UnknownTag`, `TruncatedOperand`, `BindingNotInPlan`.

## 11. One AWBC nominal-record-domain table
Owner remains `arcweft-core::awbc`; no second map.

```rust
pub struct AwbcNominalRecordDomainId(u32); // no public ctor
pub struct AwbcNominalRecordDomain {
    semantic_identity: RuntimeSemanticTypeId,
    authority: RuntimeTypeAuthorityDeclaration,
}
impl AwbcNominalRecordDomain {
    pub fn try_new(semantic_identity: RuntimeSemanticTypeId,
                   authority: RuntimeTypeAuthorityDeclaration)
        -> Result<Self, AwbcBuildError>;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn authority(&self) -> &RuntimeTypeAuthorityDeclaration;
}
impl AwbcProgramBuilder {
    pub fn push_nominal_record_domain(&mut self, row: AwbcNominalRecordDomain)
        -> Result<AwbcNominalRecordDomainId, AwbcBuildError>;
}
impl AwbcProgram {
    pub const fn nominal_record_domains(&self) -> &[AwbcNominalRecordDomain];
}
```
Exact duplicate rows intern to one ID; one semantic identity with conflicting authority fails. Limit 65,536. Private v1 wire: `count:u32le`, each row `semantic_identity[32]`, `authority_tag:u8` (Project=0, Producer=1), canonical authority payload. `MakeRecord` and nominal record constants carry `domain:u32le` before record operands/data. Verifier checks bounds and direct project/producer authority/root correlation; VM resolves only through this table. Nominal spelling is never a domain source.

## 12. Source-backed and synthetic expression table
Every source-backed runtime node uses the exact accepted `ExprId` fact of the HIR expression that causes it. Synthetic nodes use this closed coordinate solely for deriving the correct accepted type:

```rust
pub enum RuntimeSyntheticExprCoordinate {
    BlockEmptyTail { owner: ExprId },
    AssignmentUnit { statement: StmtId },
    ReductionAccumulator { owner: ExprId },
    ReductionStep { owner: ExprId },
    AgentScaffold { owner: ExprId, slot: u32 },
    CompositeEmpty { owner: ExprId, family: RuntimeSyntheticCompositeFamily },
}
```
`BlockEmptyTail` and `AssignmentUnit` use the canonical accepted Unit type from semantic analysis. Reduction accumulator/step use the accepted normalized accumulator/result facts already owned by the reduction semantic fact. Agent scaffolding uses the accepted registered intrinsic signature fact for the exact slot. Composite empty uses the owning source expression's accepted normalized composite type and validates that empty construction is legal for that shape. The final runtime expression node table remains path→`RuntimePlanTypeId`; the synthetic coordinate is lowering evidence, not a second runtime identity. Missing evidence => `MissingSyntheticTypeEvidence`; derived/expected mismatch => `SyntheticTypeMismatch`; unsupported synthetic family => `UnsupportedSyntheticType`. Runtime-value inspection is prohibited.
