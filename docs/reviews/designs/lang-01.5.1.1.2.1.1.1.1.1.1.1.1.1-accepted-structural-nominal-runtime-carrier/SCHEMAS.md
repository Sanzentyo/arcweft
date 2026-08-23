# Rust-shaped schemas

These signatures are normative. All shown fields are private unless explicitly
marked public; unrelated existing variants remain unchanged.

## 1. Accepted-world roles

Owner: `crates/arcweft-lang-sema/src/env/nominal.rs`

    pub enum AcceptedNominalSemantics {
        Exact(TypeKind),
        Opaque(AcceptedOpaqueRuntimeCarrier),
        RustAdt,
        Character(CharacterNominalType),
    }

    impl AcceptedNominalRecord {
        pub fn try_new_rust_adt(
            id: AcceptedNominalId,
            arity: u16,
            source: SourceSpan,
        ) -> Result<Self, AcceptedNominalCatalogError>;
    }

`RustAdt` is a data-free semantic role, not carrier evidence. It says neither
record nor variant and is executable only after the exact metadata join.

Owner: `crates/arcweft-lang-sema/src/registration/environment_input.rs`

    pub struct AcceptedNominalInventoryInput {
        id: AcceptedNominalId,
        arity: u16,
        semantics: AcceptedNominalSemantics,
        visibility: AcceptedNominalInputVisibility,
        origin: AcceptedNominalOrigin,
        source: SourceSpan,
        item: EnvironmentPublicationItemId,
    }

    impl AcceptedNominalInventoryInput {
        pub fn new_opaque(
            id: AcceptedNominalId,
            arity: u16,
            carrier: AcceptedOpaqueRuntimeCarrier,
            visibility: AcceptedNominalInputVisibility,
            origin: AcceptedNominalOrigin,
            source: SourceSpan,
            item: EnvironmentPublicationItemId,
        ) -> Self;

        pub fn new_rust_adt(
            id: AcceptedNominalId,
            arity: u16,
            visibility: AcceptedNominalInputVisibility,
            source: SourceSpan,
            item: EnvironmentPublicationItemId,
        ) -> Self;

        pub const fn semantics(&self) -> &AcceptedNominalSemantics;
    }

The old raw constructor that requires `AcceptedOpaqueRuntimeCarrier` is
deleted. `new_rust_adt` fixes `RustExport` and requires a `RustPackage` owner.
`new_opaque` cannot publish `RustExport` structural metadata.

Owners: `crates/arcweft-lang-sema/src/env/enums.rs` for
`EnumVariantPayload`, and
`crates/arcweft-lang-sema/src/env/rust_metadata.rs` for the metadata rows.

    pub enum EnumVariantPayload {
        Unit,
        Tuple(Box<[TypeKind]>),
        Record(Box<[(String, TypeKind)]>),
    }

    pub struct AcceptedRustTypeMetadata {
        item: EnvironmentPublicationItemId,
        id: AcceptedNominalId,
        package: RustPackageId,
        package_provenance: RustPackageProvenance,
        rust_item: RustItemPath,
        parameters: Box<[GenericTypeParameterId]>,
        kind: AcceptedRustTypeMetadataKind,
        source: SourceSpan,
    }

    pub struct InstantiatedRustTypeMetadata {
        item: EnvironmentPublicationItemId,
        id: AcceptedNominalId,
        package: RustPackageId,
        package_provenance: RustPackageProvenance,
        rust_item: RustItemPath,
        kind: AcceptedRustTypeMetadataKind,
        source: SourceSpan,
    }

    impl AcceptedRustTypeMetadata {
        pub const fn item(&self) -> &EnvironmentPublicationItemId;
    }

    impl InstantiatedRustTypeMetadata {
        pub const fn item(&self) -> &EnvironmentPublicationItemId;
    }

Record fields and cases are ordered boxed slices. A `BTreeSet` may detect
duplicates; no `BTreeMap` may own their order or silently replace a field.

Owner: `crates/arcweft-lang-sema/src/registration/model.rs`

    pub struct AcceptedRustProjectionStamp {
        environment: RegisteredEnvironmentDigest,
        rust_metadata: AcceptedRustTypeMetadataDigest,
    }

    impl RegisteredTypeCheckEnv {
        pub fn accepted_rust_projection_stamp(&self)
            -> AcceptedRustProjectionStamp;
    }

    impl AcceptedRustProjectionStamp {
        pub const fn environment(&self) -> RegisteredEnvironmentDigest;
        pub const fn rust_metadata(&self) -> AcceptedRustTypeMetadataDigest;
    }

Only successful registration constructs the stamp. It proves the joined world
but is not an atom in a type layout.

## 2. Core schema graph

Owner: `crates/arcweft-core/src/entry/schema.rs`

    pub enum RuntimeNominalRecordShape {
        Unit,
        Tuple,
        Record,
        Newtype,
    }

    pub struct RuntimeNominalSchemaIdentity {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
    }

    impl RuntimeNominalSchemaIdentity {
        pub fn new(
            nominal: RuntimeNominalTypeId,
            semantic_identity: RuntimeSemanticTypeId,
        ) -> Self;
        pub const fn nominal(&self) -> &RuntimeNominalTypeId;
        pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    }

    pub struct RuntimeSchemaValueField {
        field: RuntimeRecordFieldId,
        name: String,
        schema: RuntimeTypeSchema,
    }

    pub struct RuntimeNominalSchemaField {
        field: RuntimeRecordFieldId,
        name: Option<String>,
        schema: RuntimeTypeSchema,
    }

    pub struct RuntimeNominalSchemaCase {
        ordinal: u32,
        name: String,
        payload: Option<RuntimeTypeSchema>,
    }

    pub enum RuntimeNominalSchemaBody {
        Record {
            shape: RuntimeNominalRecordShape,
            fields: Box<[RuntimeNominalSchemaField]>,
        },
        Variant {
            cases: Box<[RuntimeNominalSchemaCase]>,
        },
    }

    pub struct RuntimeNominalSchemaDefinition {
        identity: RuntimeNominalSchemaIdentity,
        body: RuntimeNominalSchemaBody,
    }

    pub struct RuntimeNominalSchemaGraph {
        definitions: Box<[RuntimeNominalSchemaDefinition]>,
    }

The existing `RuntimeTypeSchema` gains these variants in place:

    Tuple(Box<[RuntimeTypeSchema]>),
    Result {
        ok: Box<RuntimeTypeSchema>,
        error: Box<RuntimeTypeSchema>,
    },
    RecordValue {
        fields: Box<[RuntimeSchemaValueField]>,
    },
    ExactOpaque {
        owner: RuntimeOpaqueTypeOwner,
        arguments: Box<[RuntimeTypeSchema]>,
    },
    NominalRef(RuntimeNominalSchemaIdentity),

The graph API is:

    impl RuntimeNominalSchemaGraph {
        pub fn try_new(
            definitions: impl Into<Box<[RuntimeNominalSchemaDefinition]>>,
            limits: RuntimeSchemaLimits,
        ) -> Result<Self, RuntimeNominalSchemaGraphError>;

        pub fn definition(
            &self,
            semantic_identity: RuntimeSemanticTypeId,
        ) -> Option<&RuntimeNominalSchemaDefinition>;

        pub fn definitions(
            &self,
        ) -> impl ExactSizeIterator<Item = &RuntimeNominalSchemaDefinition>;

        pub fn try_layout_hash(
            &self,
            root: RuntimeSemanticTypeId,
        ) -> Result<TypeLayoutHash, RuntimeNominalSchemaGraphError>;

        pub fn try_layouts(&self)
            -> Result<Box<[(RuntimeSemanticTypeId, TypeLayoutHash)]>,
                      RuntimeNominalSchemaGraphError>;

        pub fn accepts_value(
            &self,
            root: RuntimeSemanticTypeId,
            value: &RuntimeValue,
            limits: RuntimeSchemaLimits,
        ) -> Result<RuntimeValueDigest, RuntimeSchemaError>;
    }

`try_new` orders definitions by semantic identity and validates unique
nominal/semantic pairs, references, contiguous IDs, shape/name rules, and
limits. Cycles are permitted only through `NominalRef`. Accepted Rust
projection never constructs the existing stringly `RuntimeTypeSchema::Named`.

## 3. Final-analysis product

Owner: `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs`

    pub enum RuntimeAcceptedRustNominalKind {
        Record(RuntimeNominalRecordShape),
        Variant,
    }

    pub struct RuntimeAcceptedRustNominalProjection {
        stamp: AcceptedRustProjectionStamp,
        nominal_world: AcceptedNominalWorldStamp,
        root: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        kind: RuntimeAcceptedRustNominalKind,
        graph: Arc<RuntimeNominalSchemaGraph>,
    }

    impl FinalSemanticAnalysis {
        pub fn project_accepted_rust_nominal(
            &self,
            world: &RegisteredSemanticWorld,
            nominal: &AcceptedNominalType,
            limits: RuntimeAcceptedRustProjectionLimits,
        ) -> Result<RuntimeAcceptedRustNominalProjection,
                    RuntimeAcceptedRustProjectionError>;
    }

    impl RuntimeAcceptedRustNominalProjection {
        pub const fn stamp(&self) -> AcceptedRustProjectionStamp;
        pub const fn nominal_world(&self) -> &AcceptedNominalWorldStamp;
        pub const fn root(&self) -> RuntimeSemanticTypeId;
        pub const fn nominal(&self) -> &RuntimeNominalTypeId;
        pub const fn layout(&self) -> TypeLayoutHash;
        pub const fn kind(&self) -> RuntimeAcceptedRustNominalKind;
        pub const fn graph(&self) -> &Arc<RuntimeNominalSchemaGraph>;
        pub fn validate_for(
            &self,
            analysis: &FinalSemanticAnalysis,
            world: &RegisteredSemanticWorld,
        ) -> Result<(), RuntimeAcceptedRustProjectionError>;
    }

    pub struct RuntimeAcceptedRustProjectionLimits {
        pub max_type_nodes: u64,
        pub max_nominal_edges: u64,
        pub max_definitions: u64,
        pub max_fields_and_cases: u64,
        pub max_active_nominal_depth: u64,
    }

All projection fields are private and there is no raw-parts constructor.

## 4. Checked and live values

Owner: `crates/arcweft-core/src/pattern.rs`

    pub struct RuntimeCheckedRecordField {
        field: RuntimeRecordFieldId,
        name: String,
        checked_type: RuntimeCheckedType,
    }

`RuntimeCheckedType` gains:

    Record(Box<[RuntimeCheckedRecordField]>),
    Variant {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        cases: Vec<RuntimeCheckedVariantCase>,
    },

The nominal case of `RuntimeVariantIdentity` becomes:

    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    }

`RuntimeCheckedType::Record` requires exact field count, contiguous field IDs,
names, order, and recursively accepted values. `variant_case` remains the sole
case selector.

Owner: `crates/arcweft-core/src/value/nominal_record.rs`

    pub struct RuntimeNominalRecordLayout {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        shape: RuntimeNominalRecordShape,
        fields: Box<[RuntimeNominalRecordLayoutField]>,
    }

    pub struct RuntimeNominalRecordLayoutField {
        field: RuntimeRecordFieldId,
        name: Option<String>,
        checked_type: RuntimeCheckedType,
    }

    pub struct RuntimeNominalRecordValue {
        type_id: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        fields: Vec<RuntimeValue>,
    }

    impl RuntimeNominalRecordLayout {
        pub fn try_from_schema_graph(
            graph: &RuntimeNominalSchemaGraph,
            root: RuntimeSemanticTypeId,
        ) -> Result<Self, RuntimeNominalRecordLayoutError>;
        pub const fn shape(&self) -> RuntimeNominalRecordShape;
    }

    impl RuntimeNominalRecordValue {
        pub(crate) fn try_from_accepted_layout(
            layout: &RuntimeNominalRecordLayout,
            fields: Vec<RuntimeValue>,
        ) -> Result<Self, RuntimeNominalRecordError>;
        pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    }

The unchecked public `RuntimeNominalRecordValue::new` is deleted.

Owner: `crates/arcweft-core/src/value.rs`

    pub struct RuntimeVariantValue {
        owner: RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        payload: Option<Box<RuntimeValue>>,
    }

    pub enum RuntimeValue {
        // existing variants
        Variant(RuntimeVariantValue),
    }

    impl RuntimeVariantValue {
        pub(crate) fn try_from_checked_case(
            owner: RuntimeVariantIdentity,
            ordinal: u32,
            case: &RuntimeCheckedVariantCase,
            payload: Option<RuntimeValue>,
        ) -> Result<Self, RuntimeVariantValueError>;
        pub const fn owner(&self) -> &RuntimeVariantIdentity;
        pub const fn ordinal(&self) -> u32;
        pub fn name(&self) -> &str;
        pub fn payload(&self) -> Option<&RuntimeValue>;
    }

Direct struct-style variant construction is deleted. Option/Result helpers,
the evaluator, and restore call the checked constructor.

## 5. Existing runtime-plan authorities

Owners: `crates/arcweft-core/src/plan/type_kind.rs`,
`nominal_record_domains.rs`, `variant_domains.rs`, and `plan.rs`

The generic type case is renamed in place; the old spelling is deleted:

    RuntimePlanTypeProjection::Nominal {
        nominal: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        arguments: Box<[R]>,
    }

Domain seeds become:

    pub struct RuntimeNominalRecordDomainFieldSeed {
        field: RuntimeRecordFieldId,
        name: Option<String>,
        ty: RuntimeSemanticTypeId,
    }

    pub struct RuntimeNominalRecordDomainSeed {
        owner: RuntimeSemanticTypeId,
        shape: RuntimeNominalRecordShape,
        fields: Box<[RuntimeNominalRecordDomainFieldSeed]>,
    }

    pub struct RuntimeVariantDomainSeed {
        owner: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        layout: TypeLayoutHash,
        cases: Box<[RuntimeVariantCaseSeed]>,
    }

    impl RuntimePlanBuilder {
        pub fn try_admit_nominal_schema_graph(
            &mut self,
            graph: &RuntimeNominalSchemaGraph,
        ) -> Result<RuntimePlanNominalAdmission, RuntimePlanBuildError>;
    }

    impl RuntimePlan {
        pub fn accepts_value(
            &self,
            ty: RuntimePlanTypeId,
            value: &RuntimeValue,
            limits: RuntimeSchemaLimits,
        ) -> Result<RuntimeValueDigest, RuntimePlanValueAdmissionError>;
    }

The builder prepares type and both domain tables, validates graph isomorphism
and layouts, then commits all three or none. The result contains only issued
root IDs; the schema graph is not stored or serialized in `RuntimePlan`.

Owner: `crates/arcweft-runtime-plan/src/semantic_facts.rs`

    pub enum RuntimeResolvedNominalSource {
        Project {
            declaration: ProjectNominalDeclarationId,
            owner: ItemId,
        },
        AcceptedRust,
    }

    pub struct RuntimeResolvedNominal {
        source: RuntimeResolvedNominalSource,
        runtime_nominal: RuntimeNominalTypeId,
        identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    }

    pub enum RuntimeVariantOwner {
        Nominal {
            nominal: RuntimeResolvedNominal,
            cases: Box<[RuntimeNormalizedVariantCase]>,
        },
        // existing CharacterNominal, BuiltinClosed, Option, Result
    }

The compiler can construct `AcceptedRust` only after validating the private
sema projection. Runtime-plan never imports sema or stores `AcceptedNominalId`.

## 6. AWBC rows and snapshot API

Owner: `crates/arcweft-core/src/awbc/schema.rs`

    pub enum AwbcNominalRecordShape { Unit, Tuple, Record, Newtype }

    pub struct AwbcRecordField {
        pub field: RuntimeRecordFieldId,
        pub name: Option<AwbcStringId>,
        pub ty: AwbcTypeId,
    }

The nominal AWBC variant identity becomes:

    Nominal {
        public_id: AwbcStringId,
        semantic_identity: [u8; 32],
        layout: [u8; 32],
    }

The existing AWBC type rows evolve in place:

    Record {
        public_id: Option<AwbcStringId>,
        fields: Vec<AwbcRecordField>,
    },
    Variant {
        owner: AwbcVariantIdentity,
        cases: Vec<AwbcVariantCase>,
    },
    NominalRecord {
        public_id: AwbcStringId,
        semantic_identity: [u8; 32],
        layout: [u8; 32],
        shape: AwbcNominalRecordShape,
        fields: Vec<AwbcRecordField>,
    },

Constant rows become:

    Record { ty: AwbcTypeId, fields: Vec<AwbcConstantId> },
    Variant {
        ty: AwbcTypeId,
        case: u32,
        payload: Option<AwbcConstantId>,
    },

Names have one owner in the type table. The duplicated
`AwbcConstant::Record.field_names` and `Variant.case_name` fields are deleted.

Owner: `crates/arcweft-core/src/value/awbc_save.rs`

    pub struct AwbcRuntimeNominalRecordFieldSnapshot {
        pub field: RuntimeRecordFieldId,
        pub value: AwbcRuntimeValueSnapshot,
    }

    pub struct AwbcRuntimeNominalRecordSnapshot {
        pub ty: AwbcTypeId,
        pub type_id: RuntimeNominalTypeId,
        pub semantic_identity: RuntimeSemanticTypeId,
        pub layout: TypeLayoutHash,
        pub fields: Vec<AwbcRuntimeNominalRecordFieldSnapshot>,
    }

The snapshot variant row gains `ty: AwbcTypeId` and otherwise retains owner,
ordinal, name, and payload.

    impl AwbcProgram {
        pub(crate) fn snapshot_runtime_value(
            &self,
            expected: AwbcTypeId,
            value: &RuntimeValue,
            limits: RuntimeSchemaLimits,
        ) -> Result<AwbcRuntimeValueSnapshot,
                    AwbcRuntimeValueSnapshotError>;

        pub(crate) fn restore_runtime_value(
            &self,
            expected: AwbcTypeId,
            snapshot: AwbcRuntimeValueSnapshot,
            limits: RuntimeSchemaLimits,
        ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError>;
    }

The context-free public snapshot conversion methods are deleted. Fiber,
product-step, and task-publication snapshots supply their exact frame slot,
capture, argument, or task-plan type.

## 7. Required typed failures

`RuntimeAcceptedRustProjectionError` has distinct cases for work limit, stale
authority, unknown/inaccessible nominal, owner/origin mismatch, wrong arity,
missing metadata, metadata item mismatch, unresolved type, duplicate/empty
field, duplicate/empty case, unsupported child, missing exact opaque evidence,
and core schema-graph failure. Paths identify fields, tuple items, case
payloads, Option/Result arms, sequences, generic arguments, and nested
nominals.

`RuntimeNominalSchemaGraphError` distinguishes work limit, duplicate/conflicting
identity, dangling reference, illegal non-nominal cycle, invalid field ID,
invalid case ordinal, invalid shape/name rules, and encoding overflow. No
error branch retries with source spelling, an alternate catalog, or an old
reader.
