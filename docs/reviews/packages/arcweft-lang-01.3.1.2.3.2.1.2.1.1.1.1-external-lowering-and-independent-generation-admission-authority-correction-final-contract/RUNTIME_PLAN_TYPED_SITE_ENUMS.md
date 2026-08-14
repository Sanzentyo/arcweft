# RuntimePlan typed-site and external-construction API

Owner: `crates/arcweft-core/src/plan/typed_sites.rs`, re-exported narrowly from
`arcweft_core::plan`. `arcweft-runtime-plan` is a separate downstream crate, so
every constructor it legitimately calls is part of the public checked
construction surface. Public construction creates
only structurally checked raw data; it cannot issue a generation, admit a plan,
produce a checked-value context, or publish executable state.

## Type table and complete node facts

```rust
pub const MAX_RUNTIME_INDEX_PATH_SEGMENTS: u32 = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIndexPath(Box<[u32]>);

impl RuntimeIndexPath {
    pub fn root() -> Self;
    pub fn try_from_indices(
        indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, RuntimeIndexPathError>;
    pub fn child(&self, ordinal: u32) -> Result<Self, RuntimeIndexPathError>;
    pub fn segments(&self) -> &[u32];
    pub const fn first(&self) -> u32;
    pub const fn last(&self) -> u32;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimePlanTypeId(u32);

impl RuntimePlanTypeId {
    pub const fn index(self) -> u32;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum RuntimeTypeAuthorityDeclaration {
    Project { root: RuntimeProjectRootId },
    Producer {
        producer: RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
pub enum RuntimeOperationalType {
    MatrixF32 = 0,
    MatrixF64 = 1,
    TensorF32 = 2,
    TensorF64 = 3,
    Range = 4,
    Iterator = 5,
    StructuralRecord = 6,
    Map = 7,
    Need = 8,
    Stream = 9,
    Source = 10,
    ThreadHandle = 11,
    Shared = 12,
    Reference = 13,
    Function = 14,
    Sequence = 15,
    Tuple = 16,
    Choice = 17,
    Result = 18,
    Option = 19,
    Variant = 20,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum RuntimePlanTypeKind {
    Checked {
        checked_type: RuntimeCheckedType,
        authority: RuntimeTypeAuthorityDeclaration,
    },
    Operational(RuntimeOperationalType),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePlanTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimePlanTypeKind,
}

impl RuntimePlanTypeDeclaration {
    pub fn try_checked(
        semantic_identity: RuntimeSemanticTypeId,
        checked_type: RuntimeCheckedType,
        authority: RuntimeTypeAuthorityDeclaration,
    ) -> Result<Self, RuntimePlanTypeDeclarationError>;

    pub fn try_operational(
        semantic_identity: RuntimeSemanticTypeId,
        shape: RuntimeOperationalType,
    ) -> Result<Self, RuntimePlanTypeDeclarationError>;

    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn kind(&self) -> &RuntimePlanTypeKind;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeExprTypeFact {
    path: RuntimeIndexPath,
    ty: RuntimePlanTypeId,
}

impl RuntimeExprTypeFact {
    pub fn new(path: RuntimeIndexPath, ty: RuntimePlanTypeId) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeTypedExpr {
    expr: RuntimeExpr,
    nodes: Box<[RuntimeExprTypeFact]>,
}

impl RuntimeTypedExpr {
    pub fn try_new(
        expr: RuntimeExpr,
        nodes: impl IntoIterator<Item = RuntimeExprTypeFact>,
    ) -> Result<Self, RuntimeTypedExprConstructionError>;
    pub const fn expr(&self) -> &RuntimeExpr;
    pub const fn nodes(&self) -> &[RuntimeExprTypeFact];
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePatternTypeFact {
    path: RuntimeIndexPath,
    expected: RuntimePlanTypeId,
}

impl RuntimePatternTypeFact {
    pub fn new(path: RuntimeIndexPath, expected: RuntimePlanTypeId) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn expected(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimePatternBindingFact {
    path: RuntimeIndexPath,
    binding: RuntimePatternBindingCoordinate,
    ty: RuntimePlanTypeId,
}

impl RuntimePatternBindingFact {
    pub fn new(
        path: RuntimeIndexPath,
        binding: RuntimePatternBindingCoordinate,
        ty: RuntimePlanTypeId,
    ) -> Self;
    pub const fn path(&self) -> &RuntimeIndexPath;
    pub const fn binding(&self) -> RuntimePatternBindingCoordinate;
    pub const fn ty(&self) -> RuntimePlanTypeId;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeTypedPattern {
    pattern: RuntimePattern,
    nodes: Box<[RuntimePatternTypeFact]>,
    bindings: Box<[RuntimePatternBindingFact]>,
}

impl RuntimeTypedPattern {
    pub fn try_new(
        pattern: RuntimePattern,
        nodes: impl IntoIterator<Item = RuntimePatternTypeFact>,
        bindings: impl IntoIterator<Item = RuntimePatternBindingFact>,
    ) -> Result<Self, RuntimeTypedPatternConstructionError>;
    pub const fn pattern(&self) -> &RuntimePattern;
    pub const fn nodes(&self) -> &[RuntimePatternTypeFact];
    pub const fn bindings(&self) -> &[RuntimePatternBindingFact];
}
```

`RuntimePlanTypeId` has no public constructor, `From<u32>`, `Default`, or
mutable accessor; only `RuntimePlanBuilder::push_type` returns it. Every other
field above is private. `RuntimeTypedExpr::try_new` and
`RuntimeTypedPattern::try_new` collect into temporary storage, invoke the
owning `RuntimeExpr`/`RuntimePattern` exhaustive traversal, require root `[0]`,
exact complete path and binding sets, strict path order, and uniqueness, and
publish only after the whole candidate passes. All invariant-bearing structs
use manual `Deserialize` through private `*WireV1` DTOs and the same public
checked constructors.

## Public coordinates used by the external lowerer

```rust
pub const MAX_RUNTIME_PLAN_COORDINATE_STEPS: u32 = 64;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeFlowOpCoordinate {
    flow: u32,
    root: u32,
    descent: Box<[RuntimeFlowOpStep]>,
}

impl RuntimeFlowOpCoordinate {
    pub fn try_new(
        flow: u32,
        root: u32,
        descent: impl IntoIterator<Item = RuntimeFlowOpStep>,
    ) -> Result<Self, RuntimePlanCoordinateError>;
    pub const fn flow(&self) -> u32;
    pub const fn root(&self) -> u32;
    pub fn descent(&self) -> &[RuntimeFlowOpStep];
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeFlowOpStep {
    LetElseElse(u32),
    IfThen(u32),
    IfElse(u32),
    IfLetThen(u32),
    IfLetElse(u32),
    MatchArm { arm: u32, op: u32 },
    LoopBody(u32),
    LetLoopBody(u32),
    LoopNextBody(u32),
    WhileBody(u32),
    WhileNextBody(u32),
    WhileLetBody(u32),
    WhileLetNextBody(u32),
    ForBody(u32),
    ForNextBody(u32),
    ThreadBody(u32),
    ScopeBody(u32),
    LetScopeBody(u32),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeStreamOpCoordinate {
    plan: u32,
    root: u32,
    descent: Box<[RuntimeStreamOpStep]>,
}

impl RuntimeStreamOpCoordinate {
    pub fn try_new(
        plan: u32,
        root: u32,
        descent: impl IntoIterator<Item = RuntimeStreamOpStep>,
    ) -> Result<Self, RuntimePlanCoordinateError>;
    pub const fn plan(&self) -> u32;
    pub const fn root(&self) -> u32;
    pub fn descent(&self) -> &[RuntimeStreamOpStep];
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum RuntimeStreamOpStep {
    ForNextBody(u32),
    IfThen(u32),
    IfElse(u32),
    MatchArm { arm: u32, op: u32 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeSourceOpCoordinate {
    plan: u32,
    handler: u32,
    op: u32,
}

impl RuntimeSourceOpCoordinate {
    pub const fn new(plan: u32, handler: u32, op: u32) -> Self;
    pub const fn plan(self) -> u32;
    pub const fn handler(self) -> u32;
    pub const fn op(self) -> u32;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "table", rename_all = "snake_case")]
pub enum RuntimePlanTypedSite {
    Entry { entry: u32, slot: RuntimeEntryTypedSlot },
    FlowExecutable { executable: u32, slot: RuntimeFlowExecutableTypedSlot },
    FlowConstant { op: RuntimeFlowOpCoordinate, binding: u32 },
    FlowExpression { op: RuntimeFlowOpCoordinate, field: RuntimeFlowExpressionField, node: RuntimeIndexPath },
    FlowPattern { op: RuntimeFlowOpCoordinate, field: RuntimeFlowPatternField, node: RuntimeIndexPath },
    PureHelper { helper: u32, slot: RuntimePureHelperTypedSlot },
    PureHelperExpression { helper: u32, node: RuntimeIndexPath },
    TraitMethod { method: u32, slot: RuntimeTraitMethodTypedSlot },
    TraitMethodExpression { method: u32, node: RuntimeIndexPath },
    Stream { plan: u32, slot: RuntimeStreamTypedSlot },
    StreamExpression { op: RuntimeStreamOpCoordinate, field: RuntimeStreamExpressionField, node: RuntimeIndexPath },
    StreamPattern { op: RuntimeStreamOpCoordinate, field: RuntimeStreamPatternField, node: RuntimeIndexPath },
    Source { plan: u32, slot: RuntimeSourceTypedSlot },
    SourceExpression { op: RuntimeSourceOpCoordinate, field: RuntimeSourceExpressionField, node: RuntimeIndexPath },
    SourcePattern { plan: u32, handler: u32, node: RuntimeIndexPath },
}
```

The coordinate constructors check the maximum descent count before allocation;
actual flow/plan/root/handler/op bounds and exact step/owner compatibility are
checked by `RuntimePlanBuilder::finish` and again against the real admitted
owner during site resolution. Public enum variants are closed coordinate data,
not mutable owner fields. Unknown tags, noncanonical encodings, overlong paths,
and wrong step payloads fail private version-1 wire decoding; no numeric or
string fallback exists.

```rust
pub enum RuntimePlanCoordinateError {
    TooDeep { maximum: u32, actual: usize },
    LengthOverflow { actual: usize },
}

pub enum RuntimeTypedExprConstructionError {
    Path(RuntimeIndexPathError),
    MissingRoot,
    MissingNode { path: RuntimeIndexPath },
    ExtraNode { path: RuntimeIndexPath },
    DuplicateNode { path: RuntimeIndexPath },
    NonCanonicalNodeOrder { previous: RuntimeIndexPath, actual: RuntimeIndexPath },
    Traversal(RuntimeExprNodeTraversalError),
}

pub enum RuntimeTypedPatternConstructionError {
    Path(RuntimeIndexPathError),
    MissingRoot,
    MissingNode { path: RuntimeIndexPath },
    ExtraNode { path: RuntimeIndexPath },
    DuplicateNode { path: RuntimeIndexPath },
    MissingBinding { binding: RuntimePatternBindingCoordinate },
    ExtraBinding { binding: RuntimePatternBindingCoordinate },
    DuplicateBinding { binding: RuntimePatternBindingCoordinate },
    NonCanonicalOrder,
    Traversal(RuntimePatternNodeTraversalError),
}
```

The exact nested enum tags and current owner paths remain normative in
`RUNTIME_PLAN_SLOT_ENUMS_AND_TAGS.md`,
`RUNTIME_PLAN_NESTED_SLOT_TAGS.csv`,
`RUNTIME_PLAN_COORDINATE_STEP_TAGS.csv`, and
`RUNTIME_PLAN_SITE_RESOLUTION.csv`. Canonical tag behavior is added to the
original Arcweft-owned enums' inherent `impl`; no extension trait, duplicate
enum, or string resolver is permitted.
