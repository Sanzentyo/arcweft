# Owners and exact APIs

## 1. Semantic owner

Module: `arcweft_lang_sema::final_analysis::view`.

```rust
#[derive(Clone, Debug)]
pub struct CheckedViewCatalog {
    generation: CheckedViewCatalogGeneration,
    definitions: BTreeMap<ItemId, CheckedViewDefinition>,
    nodes: BTreeMap<ExprId, CheckedViewNode>,
    reverse_dependencies: BTreeMap<CheckedViewDependency, Box<[ExprId]>>,
    work: CheckedViewCatalogWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewCatalogGeneration {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    symbol_world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    resource_registry: ResourceTypeRegistryDigest,
}

impl CheckedViewCatalog {
    pub fn generation(&self) -> &CheckedViewCatalogGeneration;
    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &CheckedViewDefinition>;
    pub fn definition(&self, owner: ItemId) -> Option<&CheckedViewDefinition>;
    pub fn node(&self, expression: ExprId) -> Option<&CheckedViewNode>;
    pub fn dependents(
        &self,
        dependency: &CheckedViewDependency,
    ) -> impl Iterator<Item = ExprId> + '_;
    pub const fn work(&self) -> &CheckedViewCatalogWork;
    pub fn validate_generation(
        &self,
        hir: HirExecutableProjectView<'_>,
        symbols: &ProjectSymbolTable,
        resources: ResourceTypeRegistryDigest,
    ) -> Result<(), CheckedViewCatalogGenerationError>;
}

impl FinalSemanticAnalysis {
    pub fn checked_views(&self) -> &Arc<CheckedViewCatalog>;
    pub fn checked_view(&self, owner: ItemId) -> Option<&CheckedViewDefinition>;
    pub fn checked_view_node(&self, expression: ExprId) -> Option<&CheckedViewNode>;
}
```

All fields stay private. Construction, interning, dependency graph assembly, SCC
analysis, and publication are `pub(crate)` inside sema. Downstream crates receive
read-only references. There is no mutable incremental side table.

## 2. Compiler owner

Module: `arcweft_compiler::view`.

```rust
pub(crate) struct CheckedViewProductLowerer<'a> {
    hir: HirExecutableProjectView<'a>,
    analysis: &'a FinalSemanticAnalysis,
    resources: &'a AcceptedResourceRegistry,
    image_catalog: &'a CheckedImageCatalog,
    style_catalog: &'a CheckedStyleCatalog,
    fx_catalog: &'a CheckedFxCatalog,
    source_map: &'a mut ProductSourceMapBuilder,
    budget: ViewProductLoweringBudget,
}

impl CheckedViewProductLowerer<'_> {
    pub(crate) fn lower(
        self,
        transaction: &mut CompiledProjectTransaction,
    ) -> Result<CompiledViewProductCandidate, ViewProductLowerError>;
}
```

The lowerer accepts only a generation-matching `CheckedViewCatalog`. It invokes the
ordinary function lowerer for each `CheckedViewValue` and emits exact
`AwbcFunctionId`/`CrossSectionRef` records. It has no source text, parser, CST, old
AST, flattened HIR, or endpoint catalog parameter.

`CompiledProjectTransaction::commit` remains the only public publication point.
ViewProgram, ViewText/Input/Style records, image/resource rows, AWBC functions,
source map, generated artifacts, and certificates are committed together.

## 3. Instruction and identity owner

Module: `arcweft_view::program` owns the renderer-neutral instruction algebra,
the already accepted View/program identity owners, and the program-local typed
coordinates. Missing behavior is added to the original types and inherent impls.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewInstruction {
    OpenElement(ViewElementSpec),
    CloseElement,
    EmitText(ViewTextSpec),
    EmitImage(ViewImageSpec),
    EmitCustom(ViewCustomSpec),
    CallView(ViewCall),
    Branch(ViewBranch),
    Match(ViewMatch),
    RepeatKeyed(ViewRepeat),
    Await(ViewAwait),
    BindLocal(ViewLocalBinding),
    ApplyModifier(ViewModifierApplication),
    ApplyFx(ViewFxApplicationInstruction),
    BindEvent(ViewEventBindingSpec),
    AttachPart(ViewPartBindingSpec),
    AttachSemantic(ViewSemanticSpec),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewProgramIdentity {
    view: ViewId,
    program: ViewProgramId,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedViewProgramIdentity {
    identity: ViewProgramIdentity,
    revision: AcceptedViewProgramRevision,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewProgramNodeId(NonZeroU32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewInstructionId(NonZeroU32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewEvaluationSiteId(NonZeroU32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewParameterRef(NonZeroU32);
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ViewLocalRef(NonZeroU32);

pub enum ViewProgramCoordinateKind {
    Node,
    Instruction,
    EvaluationSite,
    Parameter,
    Local,
}

pub enum ViewProgramCoordinateError {
    Zero { kind: ViewProgramCoordinateKind },
    OutOfRange {
        kind: ViewProgramCoordinateKind,
        value: u32,
        maximum: u32,
    },
}
```

Each coordinate owner has only inherent `try_new(raw: u32)`, `get()`, and
validated-program membership behavior. Allocation is deterministic from canonical
final-HIR typed order inside one program. Coordinates have no meaning by themselves:
all product/runtime/public evidence joins them to `ViewProgramId` and the exact
`AcceptedViewProgramRevision`. A decoder rejects a coordinate under another
program/revision even when the raw integer happens to exist there.

`ViewId(PublicId)`, `ViewProgramId(PublicId)`, and
`AcceptedViewProgramRevision([u8; 32])` are the accepted persistent owners and are
reused rather than redefined. `ViewRegistryId(u32)` and the runtime definition
index remain process-local and never serialize. `SyntaxNodeId`, every HIR ID,
`CheckedViewCatalogGeneration`, source role, `ProductSourceId`, and source range are
session/diagnostic facts and never enter a persistent identity seed.

`ViewElementKind` remains the one ten-member canonical element enum; `Image` is not
added. `ViewInstruction::Match` is added to the owner rather than implemented by a
runtime helper. `ViewProgram` no longer owns an executable
`ViewValueProgramInventory`; it retains only referenced program IDs and
instructions.

## 4. Generic value execution owner

- Program execution: `arcweft_core` product AWBC executor.
- Runtime values: `arcweft_core::value::RuntimeValue`.
- Product description: `arcweft_bundle::resource_codec::view::ViewValueProgramResource`.
- Runtime invocation and projection: `arcweft_runtime_driver::view_runtime::value`.

```rust
impl BundleViewRuntime {
    fn evaluate_program(
        &mut self,
        mount: ViewMountId,
        program: ViewValueProgramId,
        context: ViewValueInvocationContext<'_>,
        budget: &mut ViewFrameBudget,
    ) -> Result<RuntimeValue, ViewRuntimeError>;

    fn project_value(
        &self,
        program: ViewValueProgramId,
        projection: &ViewValueProjection,
        value: RuntimeValue,
    ) -> Result<ProjectedViewValue, ViewRuntimeError>;
}
```

The runtime methods are private because callers consume typed instruction results,
not arbitrary View expression evaluation.

## 5. Resource conversion owner

Module: `arcweft_resource_model::value`.

```rust
impl ResourceRefValue {
    pub fn to_runtime_value(
        &self,
        layout: &ResourceRefRuntimeLayout,
    ) -> Result<RuntimeValue, ResourceRefRuntimeError>;

    pub fn try_from_runtime_value(
        expected_type: &ResourceTypeId,
        layout: &ResourceRefRuntimeLayout,
        value: &RuntimeValue,
    ) -> Result<Self, ResourceRefRuntimeError>;
}
```

`ResourceRefRuntimeLayout` is produced by the ordinary nominal type lowering and
contains exact nominal owner identity, layout digest, and field ordinals/types. It
does not contain source spellings. These are contextual inherent methods on the
accepted owner, not an extension trait or a free conversion helper.

## 6. Bundle and runtime catalog owners

- `arcweft_bundle::resource_codec::view::ValidatedViewProduct` owns strict decode,
  cross-section validation, semantic digest, certificate verification, and merge.
- `arcweft_runtime_driver::view_runtime::ViewProgramCatalog` owns the accepted
  executable lookup indexes.
- `BundleViewRuntime` owns mount state, evaluation, resource lifetime, output,
  save/replay, and replacement.
- Native/Web/headless/Agent/MCP adapters observe `BundleViewFrame`; they never
  accept authoring DTOs or resolve identities.

## 7. Static proof owner

Static analysis is private to sema and publishes `CheckedViewStaticDisposition`.
Compiler only canonicalizes the accepted proof into product evidence and verifies
that every certified binding is constant. Bundle/runtime validate the serialized
certificate and fragment; neither reconstructs source semantics.
