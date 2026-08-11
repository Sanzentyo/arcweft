# Exact Rust-shaped schemas

The following shapes are normative. Module placement and visibility are part of
the contract; derives may be narrowed only when compilation proves a derive is
not needed. Public wire DTOs use `Serialize`, `Deserialize`,
`#[serde(deny_unknown_fields)]`, `snake_case` tags, and private validated-domain
fields exposed through accessors.

## Identity strata

Accepted persistent owners are reused:

```rust
// Existing accepted arcweft-view owners; shown for relationship, not redefined.
pub struct ViewId(PublicId);
pub struct ViewProgramId(PublicId);
pub struct AcceptedViewProgramRevision([u8; 32]);

pub struct ViewProgramIdentity {
    view: ViewId,
    program: ViewProgramId,
}

pub struct AcceptedViewProgramIdentity {
    identity: ViewProgramIdentity,
    revision: AcceptedViewProgramRevision,
}

#[repr(transparent)]
pub struct ViewParameterContractDigest([u8; 32]);
#[repr(transparent)]
pub struct ViewParameterTableContractDigest([u8; 32]);
#[repr(transparent)]
pub struct ViewExportContractDigest([u8; 32]);
```

`ViewRegistryId(u32)` and the runtime definition index remain process-local and
never serialize. The following catalog keys are session-only and deliberately have
no Serde implementation:

```rust
pub(crate) struct CheckedViewNodeKey {
    owner: ItemId,
    expression: ExprId,
}

pub(crate) struct CheckedViewParameterKey {
    owner: ItemId,
    local: LocalId,
}

pub(crate) struct CheckedViewExportKey {
    owner: ItemId,
    target: ExprId,
    source: CheckedViewSourceRole,
}
```

The product uses dense typed coordinates with private nonzero fields:

```rust
#[repr(transparent)]
pub struct ViewProgramNodeId(NonZeroU32);
#[repr(transparent)]
pub struct ViewInstructionId(NonZeroU32);
#[repr(transparent)]
pub struct ViewEvaluationSiteId(NonZeroU32);
#[repr(transparent)]
pub struct ViewParameterRef(NonZeroU32);
#[repr(transparent)]
pub struct ViewLocalRef(NonZeroU32);
```

They are allocated deterministically from canonical typed order inside one
`ViewProgramId`, and they are valid only under the exact
`AcceptedViewProgramRevision` that contains them. Raw equality across revisions is
meaningless and rejected by validation.

## Semantic catalog

```rust
pub struct CheckedViewDefinition {
    owner: ItemId,
    module: HirModuleId,
    view: ViewId,
    program: ViewProgramId,
    callable_scope: ScopeId,
    source: CheckedViewSourceRole,
    parameters: Box<[CheckedViewParameter]>,
    roots: Box<[ExprId]>,
    exports: Box<[CheckedViewExport]>,
    dependencies: CheckedViewDependencySet,
    static_disposition: CheckedViewStaticDisposition,
}

pub struct CheckedViewParameter {
    key: CheckedViewParameterKey,
    parameter: ViewParameterRef,
    ordinal: u16,
    role: CheckedViewParameterRole,
    ty: TypeKind,
    default: Option<CheckedViewValue>,
    source: CheckedViewSourceRole,
}

pub enum CheckedViewParameterRole {
    Input,
    DialogueProfile,
    StateSeed,
}

pub struct CheckedViewExport {
    key: CheckedViewExportKey,
    name: ViewPartName,
    target: ExprId,
    node: ViewProgramNodeId,
    part: ViewPartId,
    site: ViewEvaluationSiteId,
    source: CheckedViewSourceRole,
}

pub struct CheckedViewNode {
    key: CheckedViewNodeKey,
    node: ViewProgramNodeId,
    scope: ScopeId,
    ty: TypeKind,
    effects: EffectSet,
    source: CheckedViewSourceRole,
    dependencies: CheckedViewDependencySet,
    execution: CheckedViewExecution,
    attachments: Box<[CheckedViewAttachment]>,
    static_disposition: CheckedViewStaticDisposition,
}

pub enum CheckedViewExecution {
    Element(CheckedViewElement),
    Text(CheckedViewText),
    NestedView(CheckedNestedViewCall),
    ResourceImage(CheckedViewResourceImage),
    Sequence(CheckedViewSequence),
    BindLocal(CheckedViewLocalBinding),
    Branch(CheckedViewBranch),
    Match(CheckedViewMatch),
    RepeatKeyed(CheckedViewRepeat),
    Await(CheckedViewAwait),
    Value(CheckedViewValue),
}

pub enum CheckedViewAttachment {
    Modifier(CheckedViewModifier),
    Handler(CheckedViewHandler),
    Fx(CheckedViewFx),
    Part(CheckedViewPart),
    Input(CheckedViewInput),
    Layout(CheckedViewLayout),
    Scroll(CheckedViewScroll),
    Navigation(CheckedViewNavigation),
    Semantic(CheckedViewSemantic),
}
```

`CheckedViewElement` carries exact `ViewElementKind`, ordered child `ExprId`s, and
typed attached members. `CheckedNestedViewCall` carries resolved `ItemId`,
`ViewId`, `ViewProgramId`, exact `ViewParameterRef` for every argument,
caller-evaluated `CheckedViewValue`, key value, and call source. The parameter
coordinate is interpreted only in the callee program record. `CheckedViewMatch`
carries the checked HIR match plus an ordinary generated selector value returning
the exact arm coordinate; runtime does not implement a second pattern matcher.

```rust
pub struct CheckedViewValue {
    expression: ExprId,
    ty: TypeKind,
    effects: EffectSet,
    suspension: CheckedSuspensionRole,
    role: CheckedViewValueRole,
    dependencies: CheckedViewDependencySet,
    source: CheckedViewSourceRole,
}

pub enum CheckedViewValueRole {
    Pure,
    DirectAwait,
    Handler,
}

pub enum CheckedViewDependency {
    Parameter(ViewParameterRef),
    Local(ViewLocalRef),
    Callable(CallableDeclarationKey),
    View(ItemId),
    Resource(ResourceRefValue),
    ImmutableResource(ResourceRefValue, ResourceDescriptorDigest),
    RichText(RegisteredSemanticValueId),
    Environment(EnvironmentBindingId),
    HandlerInput(CheckedViewHandlerInputId),
}

pub struct CheckedViewDependencySet {
    ordered: Box<[CheckedViewDependency]>,
    digest: CheckedViewDependencyDigest,
}
```

The dependency digest canonicalizes resolved semantic identities. It excludes
`ItemId`, `ExprId`, `LocalId`, every syntax/HIR database or snapshot identity,
source role, span, and source text.

## Static result

```rust
pub struct CheckedViewStaticDisposition {
    result: CheckedViewStaticResult,
    requirement: Option<CheckedViewStaticRequirement>,
}

pub enum CheckedViewStaticResult {
    Certified(CheckedViewStaticEvidence),
    Dynamic(CheckedViewDynamicEvidence),
}

pub enum CheckedViewStaticSubject {
    Definition(ItemId),
    Subtree(ExprId),
}

pub struct CheckedViewStaticRequirement {
    subject: CheckedViewStaticSubject,
    attribute_source: CheckedViewSourceRole,
}

pub struct CheckedViewStaticEvidence {
    subject: CheckedViewStaticSubject,
    node: ViewProgramNodeId,
    semantic_digest: ViewSemanticDigest,
    dependency_closure: CheckedViewDependencySet,
    folded_modifiers: Box<[CheckedViewFoldedModifier]>,
    immutable_resources: Box<[CheckedImmutableViewResource]>,
    retained_lifecycle: CheckedViewLifecycleSet,
}

pub struct CheckedViewDynamicEvidence {
    subject: CheckedViewStaticSubject,
    first_reason: CheckedViewDynamicReason,
    contaminating_nodes: Box<[ExprId]>,
    dependencies: CheckedViewDependencySet,
}

pub enum CheckedViewDynamicReason {
    EffectfulRenderValue,
    DirectAwait,
    DynamicParameter,
    DynamicDefault,
    DynamicLocal,
    DynamicBranch,
    DynamicMatch,
    DynamicRepeatSource,
    DynamicRepeatKey,
    DynamicNestedArgument,
    DynamicResourceSelection,
    MutableResource,
    UnfoldableModifier,
    RecursiveViewCall,
    EnvironmentDependency,
}
```

## Product value program

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValueProgramResource {
    pub id: ViewValueProgramId,
    pub semantic_id: ViewValueSemanticId,
    pub function: ViewValueFunctionRef,
    pub result_type: RuntimeCheckedType,
    pub role: ViewValueProgramRole,
    pub inputs: Vec<ViewValueInputBinding>,
    pub dependencies: ViewRuntimeDependencySet,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValueFunctionRef {
    pub function: AwbcFunctionId,
    pub function_binding: CrossSectionRef,
    pub awbc_abi: u32,
    pub program_digest: BundleDigest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewValueProgramRole {
    Pure,
    DirectAwait,
    Handler,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewValueInputBinding {
    pub register: u16,
    pub source: ViewValueInputSource,
    pub value_type: RuntimeCheckedType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewValueInputSource {
    Parameter { parameter: ViewParameterRef },
    Local { local: ViewLocalRef },
    RepeatItem { repeat: ViewProgramNodeId },
    RepeatIndex { repeat: ViewProgramNodeId },
    HandlerInput { input: CheckedViewHandlerInputId },
    Environment { binding: EnvironmentBindingId },
}
```

`function_binding` is required; a numeric AWBC index without a cross-section
binding is invalid. Program inputs are canonical by register and unique. Every
parameter/local/repeat coordinate is validated against the containing accepted
program revision before AWBC invocation.

## Exact projection and binding

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewValueProjection {
    Identity { expected: RuntimeCheckedType },
    Bool,
    StableKey,
    PlainText,
    RichText,
    Fx { expected: FxRuntimeType },
    ResourceRef { expected: ResourceTypeId },
    SignedInteger { bits: u16 },
    UnsignedInteger { bits: u16 },
    Length,
    LogicalRect,
    EnumPolicy { contract: ViewPropertyContractDigest },
    HandlerInput { expected: RuntimeCheckedType },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewProjectedProgramRef {
    pub program: ViewValueProgramId,
    pub projection: ViewValueProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewBinding<T> {
    Constant { value: T },
    Program { value: ViewProjectedProgramRef },
}
```

`ViewBinding<T>` is a product binding carrier, not a runtime value model. Its
constant arm uses the field's existing native validated type. Its program arm
always returns `RuntimeValue` and applies one exact projection.

## Instructions added/replaced

```rust
pub struct ViewMatch {
    pub selector: ViewValueProgramId,
    pub arms: Vec<ViewMatchArm>,
}

pub struct ViewMatchArm {
    pub ordinal: u16,
    pub body: ViewInstructionRange,
}

pub struct ViewCallTargetResource {
    pub view: ViewId,
    pub program: ViewProgramId,
    pub parameter_table: ViewParameterTableContractDigest,
}

pub struct ViewCall {
    pub target: ViewCallTargetResource,
    pub arguments: Vec<ViewCallArgument>,
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewBinding<ViewStableKey>>,
}

pub struct ViewCallArgument {
    pub parameter: ViewParameterRef,
    pub parameter_contract: ViewParameterContractDigest,
    pub value: ViewValueProgramId,
    pub source: Option<SourceRangeRef>,
}

pub struct ViewElementSpec {
    pub node: ViewProgramNodeId,
    pub kind: ViewElementKind,
    pub properties: Vec<ViewPropertyBindingResource>,
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub part: Option<ViewPartId>,
    pub key: Option<ViewBinding<ViewStableKey>>,
}

pub struct ViewPropertyBindingResource {
    pub member: ViewResolvedMemberRef,
    pub value: ViewProjectedProgramRef,
}

pub struct ViewResolvedMemberRef {
    pub owner: ViewResolvedOwnerId,
    pub ordinal: u16,
    pub contract: ViewPropertyContractDigest,
}
```

For a static field the transcript stores the native constant in the owning field
rather than a `ViewPropertyBindingResource`; dynamic property records exist only
when the accepted member contract is program-bindable. Member name is optional
diagnostic text and never lookup authority.

## Text and image

```rust
pub enum ViewTextSourceKind {
    Literal { value: String },
    Localized { key: LocalizedTextKey },
    RichTextDocument { document: RichTextDocument },
    DisplayFrame { frame: LineDisplayFrameRef },
    Dialogue {
        parameter: ViewParameterRef,
        projection: DialogueTextProjection,
    },
    RuntimeProgram {
        value: ViewProjectedProgramRef,
        surface: ViewTextSurface,
    },
}

pub enum ViewImageBindingResource {
    Constant { resource: ViewResolvedResourceRef },
    Program { value: ViewProjectedProgramRef },
}

pub struct ViewResolvedResourceRef {
    pub entity: EntityId,
    pub public: PublicId,
    pub resource_type: ResourceTypeId,
    pub declaration_digest: ResourceDeclarationDigest,
    pub descriptor_digest: ResourceDescriptorDigest,
    pub registry_digest: ResourceTypeRegistryDigest,
}
```

`ViewTextSourceKind::Projection { path: String }` and `Local { name: String }`
are removed. Dynamic local/projection text is an ordinary program. Dialogue keeps
its accepted typed optimized projection and exact parameter coordinate.

## Certificate wire

```rust
pub struct ViewStaticFragmentResource {
    pub id: ViewStaticFragmentId,
    pub program: ViewProgramIdentity,
    pub subject: ViewStaticSubjectResource,
    pub instructions: ViewInstructionSpan,
    pub dependencies: ViewRuntimeDependencySet,
    pub fragment_digest: ViewStaticFragmentDigest,
}

pub struct ViewStaticCertificateResource {
    pub id: ViewStaticCertificateId,
    pub program: ViewProgramIdentity,
    pub subject: ViewStaticSubjectResource,
    pub proof_origin: ViewStaticProofOrigin,
    pub semantic_digest: ViewSemanticDigest,
    pub dependency_digest: ViewDependencyClosureDigest,
    pub immutable_resource_digest: ViewImmutableResourceClosureDigest,
    pub retained_lifecycle_digest: ViewRetainedLifecycleDigest,
    pub fragment: ViewStaticFragmentId,
    pub fragment_digest: ViewStaticFragmentDigest,
    pub program_semantic_digest: ViewProgramSemanticDigest,
    pub certificate_digest: ViewStaticCertificateDigest,
}

pub enum ViewStaticSubjectResource {
    Definition,
    Subtree { node: ViewProgramNodeId },
}

pub enum ViewStaticProofOrigin {
    Automatic,
    AuthoredRequired,
}
```

The containing `ViewProgramTranscriptV1` supplies the exact
`AcceptedViewProgramRevision`; certificate and fragment records do not duplicate it
or create a circular digest. A missing certificate record means dynamic execution.
A certificate ID referenced by the program but absent, or a present malformed/
stale record, is a hard validation failure.

## Canonical digests and revisions

`ExprId`, `LocalId`, `ItemId`, `SyntaxNodeId`, every syntax/HIR database, lineage,
snapshot, module, revision, scope, source role, `ProductSourceId`, span, and source
text are excluded from every persisted identity and semantic digest.

Canonical derivation is:

```text
ViewParameterContractDigest = H("arcweft.view.parameter-contract.v1\0" ||
  ViewProgramIdentity || ViewParameterRef || ordinal || role || exact checked
  value type || value slot || default ViewValueSemanticId-or-absence)

ViewParameterTableContractDigest = H(
  "arcweft.view.parameter-table-contract.v1\0" || ViewProgramIdentity ||
  ordered ViewParameterContractDigest rows || required/default presence bitmap)

ViewExportContractDigest = H("arcweft.view.export-contract.v1\0" ||
  ViewProgramIdentity || public part name || node/instruction/site/part
  coordinates || state/input schema digest || visibility contract)

ViewValueSemanticId = H("arcweft.view.value-semantic.v1\0" ||
  ViewProgramIdentity || canonical typed expression semantics ||
  result type/effect/suspension/role || resolved dependency semantics)

ViewProgramSemanticDigest = H("arcweft.view.program-semantic.v1\0" ||
  ViewProgramIdentity || ViewParameterTableContractDigest || ordered export
  contract digests || executable instruction algebra || value semantic IDs and exact AWBC function digests || resolved member
  contracts || stable nested-call targets and parameter-table contract digests ||
  resource closure || handler/input/state contracts)

ViewStaticFragmentDigest = H("arcweft.view.static-fragment.v1\0" ||
  ViewProgramIdentity || subject coordinate || canonical fragment instructions ||
  dependency rows)

ViewStaticCertificateDigest = H("arcweft.view.static-certificate.v1\0" ||
  ViewProgramIdentity || subject coordinate || ViewProgramSemanticDigest ||
  semantic/dependency/resource/lifecycle digests || fragment ID/digest ||
  proof origin)

AcceptedViewProgramRevision = H("arcweft.view.program-revision.v1\0" ||
  ViewProgramIdentity || ViewProgramSemanticDigest || ordered static fragment
  IDs/digests || ordered static certificate IDs/digests)
```

`ViewParameterRef`, `ViewLocalRef`, `ViewProgramNodeId`, `ViewInstructionId`, and
`ViewEvaluationSiteId` are not hash outputs. They are one-based dense coordinates
allocated from canonical typed inventories, and their meaning is scoped by
`AcceptedViewProgramRevision`. Reordered arena allocation or a fresh compiler
session cannot affect canonical product bytes. Source-map bytes have their own
`SourceMapSection` digest and contribute only through the existing full bundle
artifact identity, not the View semantic/program/certificate digests.
