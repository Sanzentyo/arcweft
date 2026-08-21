# Normative owner and API contract

Rust snippets in this package are design surfaces, not production overlays. Names intentionally reuse current domain owners rather than adding extension traits or ad-hoc compiler helpers.

## 1. Crate ownership

| Concern | Sole owner |
|---|---|
| Structural reachability, scope closure, typed HIR owner sets, deterministic first paths | `arcweft-lang-hir::final_project::runtime_semantic_owners` |
| Checked item/callable execution and suspension facts | existing `arcweft-lang-sema::final_analysis` and shared checked callable catalog |
| Mapping accepted semantic targets/Entries to HIR edges; emission preflight | `arcweft-compiler::lower::reachability` |
| Runtime semantic fact admission for one closure | `arcweft-runtime-plan::semantic_facts` |
| Closed project nominal schema projection | `arcweft-lang-sema::final_analysis::nominal_schema` |
| Project nominal layout hash | existing `RuntimeTypeSchema::try_layout_hash` owner |
| Runtime nominal descriptor/value validation | existing `arcweft-core` nominal-record owners |
| AWBC materialization/verification/execution | existing AWBC schema/lower/verifier/VM owners |
| Entry root construction | existing checked Entry catalog plus compiler selection bridge |
| Save/replay validation | existing AWBC snapshot and runtime-driver session-save owners |
| Display-only emission state | semantic tooling index, projected from the compiler-owned result |

## 2. In-place HIR replacement

The old type and method are removed. There is no deprecated alias.

```rust
pub struct HirRuntimeSemanticReachability<'project> {
    project: HirExecutableProjectView<'project>,
    mode: HirRuntimeEmissionMode,
    roots: Box<[HirRuntimeReachabilityRoot]>,
    edges: Box<[HirRuntimeReachabilityEdge]>,
    reachable_executables: BTreeSet<HirRuntimeExecutableOwner>,
    first_paths: BTreeMap<HirRuntimeExecutableOwner, HirRuntimeReachabilityPath>,
    locals: Box<[LocalId]>,
    expressions: BTreeSet<ExprId>,
    statements: BTreeSet<StmtId>,
    types: BTreeSet<TypeId>,
    patterns: BTreeSet<PatternId>,
    captures: BTreeSet<CaptureId>,
    identity: HirRuntimeReachabilityIdentity,
}
```

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeEmissionMode {
    CheckAll,
    SelectedEntry,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeExecutableOwner {
    Item(ItemId),
    ImplMethod(ImplMethodDeclarationId),
    Closure(ExprId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilitySite {
    Item(ItemId),
    Expression(ExprId),
    Statement(StmtId),
}
```

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilityRootKind {
    CheckedFlow,
    CheckedEntry,
    SelectedEntry,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityRoot {
    pub kind: HirRuntimeReachabilityRootKind,
    pub owner: HirRuntimeExecutableOwner,
}
```

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRuntimeReachabilityEdgeKind {
    CheckedProjectCall {
        call: ExprId,
        declaration: CallableDeclarationKey,
    },
    CheckedTraitDispatch {
        call: ExprId,
        implementation: ItemId,
        method: ImplMethodDeclarationId,
    },
    CheckedFlowTransfer {
        source: StmtId,
        declaration: CallableDeclarationKey,
    },
    CheckedEntryBinding {
        entry: ItemId,
        declaration: CallableDeclarationKey,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityEdge {
    pub source: HirRuntimeReachabilitySite,
    pub target: HirRuntimeExecutableOwner,
    pub kind: HirRuntimeReachabilityEdgeKind,
}
```

The public construction surface has private fields and validates atomically:

```rust
pub struct HirRuntimeSemanticReachabilityInput {
    mode: HirRuntimeEmissionMode,
    roots: Vec<HirRuntimeReachabilityRoot>,
    edges: Vec<HirRuntimeReachabilityEdge>,
}

impl HirExecutableProjectView<'_> {
    pub fn runtime_semantic_reachability(
        self,
        input: HirRuntimeSemanticReachabilityInput,
        selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
        call_disposition: impl FnMut(ExprId) -> HirRuntimeExpressionTypeDisposition,
    ) -> Result<HirRuntimeSemanticReachability<'_>, HirRuntimeReachabilityError>;
}
```

`HirRuntimeSemanticReachabilityInput::try_new` sorts and rejects duplicate/conflicting roots and edges. It never receives source strings.

## 3. HIR-owned behavior on domain enums

Structural root behavior belongs on existing HIR enums rather than compiler pattern-matching copies:

```rust
impl HirItemKind {
    pub fn runtime_execution_roots(
        &self,
        selected: HirRuntimeExecutableOwner,
    ) -> Result<HirRuntimeExecutionRoots, HirRuntimeReachabilityError>;
}

impl HirExprKind {
    pub fn runtime_structural_edges(&self) -> HirRuntimeStructuralEdges<'_>;
}

impl HirStmtKind {
    pub fn runtime_structural_edges(&self) -> HirRuntimeStructuralEdges<'_>;
}
```

These are inherent methods. No extension trait is introduced.

## 4. Reachability accessors

```rust
impl HirRuntimeSemanticReachability<'_> {
    pub const fn mode(&self) -> HirRuntimeEmissionMode;
    pub const fn identity(&self) -> &HirRuntimeReachabilityIdentity;
    pub fn roots(&self) -> impl ExactSizeIterator<Item = &HirRuntimeReachabilityRoot>;
    pub fn edges(&self) -> impl ExactSizeIterator<Item = &HirRuntimeReachabilityEdge>;
    pub fn reachable_executables(
        &self,
    ) -> impl ExactSizeIterator<Item = HirRuntimeExecutableOwner> + '_;
    pub fn first_path(
        &self,
        owner: HirRuntimeExecutableOwner,
    ) -> Option<&HirRuntimeReachabilityPath>;
    pub fn edge_from(
        &self,
        source: HirRuntimeReachabilitySite,
    ) -> impl Iterator<Item = &HirRuntimeReachabilityEdge>;

    pub fn locals(&self) -> impl ExactSizeIterator<Item = LocalId> + '_;
    pub fn contains_expression(&self, owner: ExprId) -> bool;
    pub fn contains_statement(&self, owner: StmtId) -> bool;
    pub fn contains_type(&self, owner: TypeId) -> bool;
    pub fn contains_pattern(&self, owner: PatternId) -> bool;
    pub fn contains_capture(&self, owner: CaptureId) -> bool;

    pub fn selected_expression_type_owners(
        &self,
    ) -> Result<BTreeSet<ExprId>, HirSelectedExpressionInventoryError>;
}
```

The selected postfix and call-disposition decisions are captured during atomic construction so downstream consumers cannot supply a different answer.

## 5. Reachability identity

The identity is not a layout hash and is not a wire-version marker.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirRuntimeReachabilityDigest([u8; 32]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirRuntimeReachabilityIdentity {
    pub module_snapshots: Box<[(HirModuleId, HirSnapshotId)]>,
    pub symbol_world: ProjectSymbolWorldId,
    pub symbol_revision: ProjectSymbolRevision,
    pub mode: HirRuntimeEmissionMode,
    pub digest: HirRuntimeReachabilityDigest,
}
```

Digest grammar is defined in `REACHABILITY_ALGORITHM.md`. Display names and source ranges are excluded.

## 6. Sema-owned classification

No new execution variant is added. Behavior is placed on the existing role enum:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedOrdinaryFunctionEmission {
    PureDirectFrame,
    EffectfulDirectFrameUnsupported,
    SuspendingDirectFrameUnsupported,
    StreamFactoryUnsupported,
}

impl CheckedItemRole {
    pub fn ordinary_function_emission(
        &self,
        effects: &EffectSet,
    ) -> Option<CheckedOrdinaryFunctionEmission>;
}
```

Rules are exhaustive over `CheckedFunctionExecution` and `CheckedSuspensionRole`. This method is the only classification table; the compiler does not repeat the match.

The checked callable owner exposes exact graph targets through an inherent method on the catalog/fact type, not an extension trait:

```rust
impl CheckedCallableFacts {
    pub fn runtime_executable_owner(
        &self,
    ) -> Option<CheckedRuntimeExecutableOwner>;
}
```

## 7. Compiler bridge

```rust
pub enum RuntimeEmissionMode<'a> {
    CheckAll,
    SelectedEntry(&'a ProjectEntrySelection),
}

pub fn project_runtime_reachability<'project>(
    project: HirExecutableProjectView<'project>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    entries: &CheckedProjectEntries,
    mode: RuntimeEmissionMode<'_>,
) -> Result<HirRuntimeSemanticReachability<'project>, RuntimeReachabilityProjectionError>;
```

```rust
pub fn validate_reachable_runtime_callables(
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
) -> Result<(), RuntimeReachabilityProjectionError>;
```

`project_runtime_semantic_facts` changes from creating an all-owner inventory to requiring the accepted closure:

```rust
pub fn project_runtime_semantic_facts(
    project: HirExecutableProjectView<'_>,
    symbols: &ProjectSymbolTable,
    analysis: &FinalSemanticAnalysis,
    reachability: &HirRuntimeSemanticReachability<'_>,
    dialogue_profile: Option<(&DialoguePresentationProfile, &DialogueProfileRevision)>,
    character_name_policy: Option<&CharacterNameLocalePolicySpec>,
) -> Result<RuntimePlanSemanticFacts, RuntimeSemanticProjectionError>;
```

## 8. Typed errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeReachabilityProjectionError {
    #[error(transparent)]
    Generation(#[from] FinalSemanticAnalysisError),
    #[error(transparent)]
    Hir(#[from] HirRuntimeReachabilityError),
    #[error("selected Entry has no accepted checked root")]
    MissingSelectedEntry,
    #[error("checked project executable edge is missing")]
    MissingCheckedEdge {
        source: HirRuntimeReachabilitySite,
        expected_target: HirRuntimeExecutableOwner,
    },
    #[error("checked project executable edge targets a different owner")]
    MismatchedCheckedEdge {
        source: HirRuntimeReachabilitySite,
        expected_target: HirRuntimeExecutableOwner,
        actual_target: HirRuntimeExecutableOwner,
    },
    #[error("reachable ordinary function cannot be emitted by the current runtime")]
    UnsupportedOrdinaryFunction {
        owner: ItemId,
        reason: CheckedOrdinaryFunctionEmission,
        path: HirRuntimeReachabilityPath,
        suspension_site: Option<ExprId>,
    },
}
```

## 9. Typed nominal schema path

`NominalSchemaProjectionError::InvalidShape { path: String, reason: String }` is replaced for known checked cases with typed data:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalSchemaPathStep {
    Field { ordinal: u32, name: HirName },
    VariantPayload { ordinal: u32, name: HirName },
    OptionItem,
    SequenceItem,
    MapKey,
    MapValue,
    GenericArgument { ordinal: u32 },
    NestedNominal { declaration: ProjectNominalDeclarationId },
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NominalSchemaPath(Box<[NominalSchemaPathStep]>);
```

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NominalSchemaProjectionError {
    GenerationMismatch,
    MissingDeclaration { nominal: String },
    OwnerMismatch { /* existing typed fields */ },
    WrongArity { /* existing typed fields */ },
    MissingTypeFact { ty: TypeId },
    OpaqueLeaf {
        path: NominalSchemaPath,
        producer: AcceptedNominalProducerId,
        semantic_identity: SemanticTypeDigest,
    },
    UnsupportedLeaf {
        path: NominalSchemaPath,
        ty: Box<TypeKind>,
    },
    CyclicGenericSubstitution {
        path: NominalSchemaPath,
        parameter: GenericTypeParameterId,
    },
}
```

The owning enum's `within`/`prepend` inherent method appends path steps. The compiler maps `OpaqueLeaf` to a dedicated runtime projection error without converting it to a string.

## 10. Runtime-plan admission

```rust
impl RuntimePlanSemanticFactInput {
    pub fn new(reachability: HirRuntimeReachabilityIdentity) -> Self;
}

impl RuntimePlanSemanticFacts {
    pub const fn reachability(&self) -> &HirRuntimeReachabilityIdentity;
    pub fn contains_runtime_owner(&self, owner: HirRuntimeExecutableOwner) -> bool;
}
```

Every pushed fact is rejected if its owner is outside the reachability closure or if its generation differs. `final_flow` consumes only the admitted callable/Flow set; it never rescans final semantic analysis.

## 11. Entry projection

`runtime_entry_lowering_input` accepts the closure and includes only checked Entry roots admitted by the selected mode. A lightweight root projection runs before any Entry schema/layout projection. Full Entry schema projection runs only after reachable-callable preflight.

## 12. No core/AWBC layout API change

The following types retain their existing fields and semantics:

- `RuntimeCheckedType::Nominal { nominal, semantic_identity, layout: TypeLayoutHash }`;
- `RuntimeCheckedType::Opaque { owner }`;
- `RuntimeNominalRecordLayout`;
- `RuntimeNominalRecordValue`;
- `AwbcRuntimeType::Nominal`;
- `AwbcRuntimeType::NominalRecord`;
- `AwbcRuntimeType::Opaque`.

Only their callers receive a filtered reachable domain. No V2 type or compatibility field is added.
