//! Accepted-generation semantic facts consumed by final-HIR runtime lowering.
//!
//! Runtime lowering is intentionally below semantic analysis in the crate
//! graph. The compiler therefore projects checked decisions into this closed
//! vocabulary and binds them to the exact executable HIR generation. Facts are
//! keyed by qualified final-HIR IDs; source-order counters, byte ranges,
//! display labels, and reconstructed paths are not accepted as identities.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::entry::{RuntimeCallableId, RuntimeNominalTypeId};
pub use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::pattern::{RuntimeCheckedType, RuntimeCheckedVariantCase};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeIteratorEvidence, RuntimeLineId, RuntimeReceiverMode,
    RuntimeTraitMethodId,
};
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::value::{
    RuntimeIntrinsic, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth, RuntimeValue,
};
use arcweft_id::{DeclarationIdentityFamily, PublicId};
use arcweft_lang_hir::expr::{HirCallExpr, HirExprKind};
use arcweft_lang_hir::identity::{
    CaptureId, ExprId, HirModuleId, HirSnapshotId, ItemId, LocalId, PatternId, StmtId, TypeId,
};
use arcweft_lang_hir::item::{HirImplMember, HirItemFamily, HirItemKind};
use arcweft_lang_hir::leaf::{HirName, HirPathSegment};
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_hir::pattern::HirPatternKind;
use arcweft_lang_hir::project::HirExecutableProjectView;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_hir::symbol::{
    CallableDeclarationKey, CallableDeclarationOwner,
    nominal::{ProjectNominalDeclarationId, ProjectNominalVariant},
};
use arcweft_lang_hir::type_ref::HirTypeKind;
use arcweft_text_model::DialogueContentSpec;
use thiserror::Error;

use crate::assertion_identity::RuntimeAssertionMode;

/// Stable semantic identity for a registered callable or value that is not
/// owned by one project HIR item.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeRegisteredValueId([u8; 32]);

impl RuntimeRegisteredValueId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Runtime-relevant shape paired with an exact semantic type identity.
///
/// `Opaque` is not an unresolved type. It is a fully resolved semantic type
/// whose runtime operations are owned by a registered producer rather than the
/// Arcweft core value algebra. Its exact identity remains in
/// [`RuntimeNormalizedType::identity`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "checked type facts preserve a direct exhaustive semantic shape; they are immutable generation-bound inputs rather than a hot runtime value representation"
)]
pub enum RuntimeTypeShape {
    Unit,
    Never,
    Bool,
    Signed(RuntimeSignedIntWidth),
    Unsigned(RuntimeUnsignedIntWidth),
    F32,
    F64,
    String,
    Char,
    Bytes,
    Duration,
    EntityReference,
    Range(Box<RuntimeNormalizedType>),
    Iterator(Box<RuntimeNormalizedType>),
    Sequence {
        kind: RuntimeSequenceKind,
        item: Box<RuntimeNormalizedType>,
    },
    Array {
        item: Box<RuntimeNormalizedType>,
        length: usize,
    },
    Map {
        key: Box<RuntimeNormalizedType>,
        value: Box<RuntimeNormalizedType>,
    },
    Need {
        ready: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Stream {
        item: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Source {
        item: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Result {
        value: Box<RuntimeNormalizedType>,
        error: Box<RuntimeNormalizedType>,
    },
    Option(Box<RuntimeNormalizedType>),
    ThreadHandle(Box<RuntimeNormalizedType>),
    Shared(Box<RuntimeNormalizedType>),
    Reference(Box<RuntimeNormalizedType>),
    Function {
        parameters: Box<[RuntimeNormalizedType]>,
        result: Box<RuntimeNormalizedType>,
    },
    ProjectNominal {
        nominal: RuntimeResolvedNominal,
        arguments: Box<[RuntimeNormalizedType]>,
    },
    Named {
        nominal: RuntimeNominalTypeId,
    },
    Tuple(Box<[RuntimeNormalizedType]>),
    Choice(Box<[RuntimeNormalizedType]>),
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSequenceKind {
    Vec,
    Array,
    Slice,
    Seq,
}

/// One normalized semantic type that can be compared without source spelling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNormalizedType {
    identity: RuntimeSemanticTypeId,
    shape: RuntimeTypeShape,
}

impl RuntimeNormalizedType {
    pub const fn new(identity: RuntimeSemanticTypeId, shape: RuntimeTypeShape) -> Self {
        Self { identity, shape }
    }

    pub const fn identity(&self) -> RuntimeSemanticTypeId {
        self.identity
    }

    pub const fn shape(&self) -> &RuntimeTypeShape {
        &self.shape
    }

    pub fn checked_type(&self) -> Result<RuntimeCheckedType, String> {
        Ok(match self.shape() {
            RuntimeTypeShape::Never => RuntimeCheckedType::Never,
            RuntimeTypeShape::Unit => RuntimeCheckedType::Unit,
            RuntimeTypeShape::Bool => RuntimeCheckedType::Bool,
            RuntimeTypeShape::Signed(width) => RuntimeCheckedType::Signed(*width),
            RuntimeTypeShape::Unsigned(width) => RuntimeCheckedType::Unsigned(*width),
            RuntimeTypeShape::F32 => RuntimeCheckedType::F32,
            RuntimeTypeShape::F64 => RuntimeCheckedType::F64,
            RuntimeTypeShape::String => RuntimeCheckedType::String,
            RuntimeTypeShape::Char => RuntimeCheckedType::Char,
            RuntimeTypeShape::Bytes => RuntimeCheckedType::Bytes,
            RuntimeTypeShape::Duration => RuntimeCheckedType::Duration,
            RuntimeTypeShape::EntityReference => RuntimeCheckedType::EntityReference,
            RuntimeTypeShape::Sequence { item, .. } | RuntimeTypeShape::Array { item, .. } => {
                RuntimeCheckedType::Sequence(Box::new(item.checked_type()?))
            }
            RuntimeTypeShape::ProjectNominal { nominal, .. } => RuntimeCheckedType::Nominal {
                nominal: RuntimeNominalTypeId::try_new(nominal.declaration().qualified_name())
                    .map_err(|error| {
                        format!("checked project nominal identity is invalid: {error}")
                    })?,
                semantic_identity: self.identity(),
            },
            RuntimeTypeShape::Named { nominal } => RuntimeCheckedType::Nominal {
                nominal: nominal.clone(),
                semantic_identity: self.identity(),
            },
            RuntimeTypeShape::Choice(alternatives) => RuntimeCheckedType::Choice(
                alternatives
                    .iter()
                    .map(RuntimeNormalizedType::checked_type)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Tuple(items) => RuntimeCheckedType::Tuple(
                items
                    .iter()
                    .map(RuntimeNormalizedType::checked_type)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Result { value, error } => RuntimeCheckedType::Result {
                ok: Box::new(value.checked_type()?),
                error: Box::new(error.checked_type()?),
            },
            RuntimeTypeShape::Option(item) => {
                RuntimeCheckedType::Option(Box::new(item.checked_type()?))
            }
            RuntimeTypeShape::Range(_)
            | RuntimeTypeShape::Iterator(_)
            | RuntimeTypeShape::Map { .. }
            | RuntimeTypeShape::Need { .. }
            | RuntimeTypeShape::Stream { .. }
            | RuntimeTypeShape::Source { .. }
            | RuntimeTypeShape::ThreadHandle(_)
            | RuntimeTypeShape::Shared(_)
            | RuntimeTypeShape::Reference(_)
            | RuntimeTypeShape::Function { .. }
            | RuntimeTypeShape::Opaque => {
                return Err(format!(
                    "runtime checked type has no closed representation for semantic type {:?}",
                    self.identity()
                ));
            }
        })
    }
}

/// Exact project callable and its final-HIR owner item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallable {
    declaration: CallableDeclarationKey,
    owner: ItemId,
    runtime: RuntimeCallableId,
}

impl RuntimeProjectCallable {
    pub const fn new(
        declaration: CallableDeclarationKey,
        owner: ItemId,
        runtime: RuntimeCallableId,
    ) -> Self {
        Self {
            declaration,
            owner,
            runtime,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn runtime(&self) -> &RuntimeCallableId {
        &self.runtime
    }
}

/// Exact project nominal and its final-HIR owner item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominal {
    declaration: ProjectNominalDeclarationId,
    owner: ItemId,
    identity: RuntimeSemanticTypeId,
}

impl RuntimeResolvedNominal {
    pub const fn new(
        declaration: ProjectNominalDeclarationId,
        owner: ItemId,
        identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self {
            declaration,
            owner,
            identity,
        }
    }

    pub const fn declaration(&self) -> &ProjectNominalDeclarationId {
        &self.declaration
    }

    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn identity(&self) -> RuntimeSemanticTypeId {
        self.identity
    }

    pub fn checked_type(&self) -> Result<RuntimeCheckedType, String> {
        Ok(RuntimeCheckedType::Nominal {
            nominal: RuntimeNominalTypeId::try_new(self.declaration.qualified_name())
                .map_err(|error| format!("checked project nominal identity is invalid: {error}"))?,
            semantic_identity: self.identity,
        })
    }
}

/// Stable entity identity paired with its closed accepted owner kind.
///
/// Retained declarations keep their exact HIR owner. Registered Characters
/// keep the externally validated Character public identity without fabricating
/// an [`ItemId`]. Runtime lowering never reconstructs either identity from
/// source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectItemOwner {
    Retained(ItemId),
    StructuralFlow {
        owner: ItemId,
        runtime: FlowRuntimeId,
    },
    ExternalCharacter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectItem {
    public_id: PublicId,
    family: DeclarationIdentityFamily,
    owner: RuntimeProjectItemOwner,
}

impl RuntimeProjectItem {
    pub fn new_retained(
        public_id: PublicId,
        family: DeclarationIdentityFamily,
        owner: ItemId,
    ) -> Self {
        Self {
            public_id,
            family,
            owner: RuntimeProjectItemOwner::Retained(owner),
        }
    }

    pub fn new_external_character(public_id: PublicId) -> Self {
        Self {
            public_id,
            family: DeclarationIdentityFamily::Character,
            owner: RuntimeProjectItemOwner::ExternalCharacter,
        }
    }

    pub fn new_structural_flow(public_id: PublicId, owner: ItemId, runtime: FlowRuntimeId) -> Self {
        Self {
            public_id,
            family: DeclarationIdentityFamily::Flow,
            owner: RuntimeProjectItemOwner::StructuralFlow { owner, runtime },
        }
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }

    pub const fn family(&self) -> DeclarationIdentityFamily {
        self.family
    }

    pub const fn owner(&self) -> &RuntimeProjectItemOwner {
        &self.owner
    }

    pub const fn retained_owner(&self) -> Option<ItemId> {
        match &self.owner {
            RuntimeProjectItemOwner::Retained(owner) => Some(*owner),
            RuntimeProjectItemOwner::StructuralFlow { .. }
            | RuntimeProjectItemOwner::ExternalCharacter => None,
        }
    }

    pub const fn flow_runtime_id(&self) -> Option<&FlowRuntimeId> {
        match &self.owner {
            RuntimeProjectItemOwner::StructuralFlow { runtime, .. } => Some(runtime),
            RuntimeProjectItemOwner::Retained(_) | RuntimeProjectItemOwner::ExternalCharacter => {
                None
            }
        }
    }
}

/// Checked meaning of one final-HIR path expression.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeResolvedValue {
    Local(LocalId),
    ProjectCallable(RuntimeProjectCallable),
    ProjectItem(RuntimeProjectItem),
    /// Checked one-way lowering of a durable `say.*` identity into the
    /// path-only runtime line domain.
    DialogueLine(RuntimeLineId),
    Intrinsic(RuntimeIntrinsic),
    Registered(RuntimeRegisteredValueId),
    Constant(RuntimeValue),
}

/// Checked projection selected for one final-HIR member expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeResolvedSelect {
    Method {
        name: HirName,
    },
    Field {
        nominal: Option<RuntimeResolvedNominal>,
        name: HirName,
    },
    TupleElement {
        ordinal: u32,
    },
    RecordElement {
        nominal: Option<RuntimeResolvedNominal>,
        ordinal: u32,
        name: HirName,
    },
}

/// Exact semantic owner of one runtime enum case.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "variant owners retain their complete normalized semantic arguments as immutable checker evidence without adding a second indirection contract"
)]
pub enum RuntimeVariantOwner {
    Project {
        nominal: RuntimeResolvedNominal,
        cases: Box<[RuntimeCheckedVariantCase]>,
    },
    CharacterNominal {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeCheckedVariantCase]>,
    },
    BuiltinClosed {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeCheckedVariantCase]>,
    },
    Option {
        item: RuntimeNormalizedType,
    },
    Result {
        ok: RuntimeNormalizedType,
        error: RuntimeNormalizedType,
    },
}

impl RuntimeVariantOwner {
    pub fn checked_type(&self) -> Result<RuntimeCheckedType, String> {
        Ok(match self {
            Self::Project { nominal, cases } => RuntimeCheckedType::Variant {
                nominal: RuntimeNominalTypeId::try_new(nominal.declaration().qualified_name())
                    .map_err(|error| {
                        format!("checked project variant owner is invalid: {error}")
                    })?,
                semantic_identity: nominal.identity(),
                cases: cases.to_vec(),
            },
            Self::CharacterNominal {
                identity,
                nominal,
                cases,
            }
            | Self::BuiltinClosed {
                identity,
                nominal,
                cases,
            } => RuntimeCheckedType::Variant {
                nominal: nominal.clone(),
                semantic_identity: *identity,
                cases: cases.to_vec(),
            },
            Self::Option { item } => RuntimeCheckedType::Option(Box::new(item.checked_type()?)),
            Self::Result { ok, error } => RuntimeCheckedType::Result {
                ok: Box::new(ok.checked_type()?),
                error: Box::new(error.checked_type()?),
            },
        })
    }
}

/// Checked enum case selected for a variant expression, constructor call, or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedVariant {
    owner: RuntimeVariantOwner,
    ordinal: u32,
    case: RuntimeVariantCase,
}

/// Closed case vocabulary retained below semantic analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeVariantCase {
    Project(Box<str>),
    Character(HirName),
    BuiltinClosed(HirName),
    OptionSome,
    OptionNone,
    ResultOk,
    ResultErr,
}

impl RuntimeResolvedVariant {
    /// Retains a case directly from its accepted project enum declaration.
    pub fn project(
        owner: RuntimeResolvedNominal,
        ordinal: u32,
        variant: &ProjectNominalVariant,
        cases: Box<[RuntimeCheckedVariantCase]>,
    ) -> Self {
        Self {
            owner: RuntimeVariantOwner::Project {
                nominal: owner,
                cases,
            },
            ordinal,
            case: RuntimeVariantCase::Project(variant.name().as_str().into()),
        }
    }

    /// Retains a Character nominal case already admitted by checked final HIR.
    pub fn character(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeCheckedVariantCase]>,
        ordinal: u32,
        name: &HirName,
    ) -> Self {
        Self {
            owner: RuntimeVariantOwner::CharacterNominal {
                identity,
                nominal,
                cases,
            },
            ordinal,
            case: RuntimeVariantCase::Character(name.clone()),
        }
    }

    /// Retains a case from one source-ordered base-environment enum schema.
    pub fn builtin_closed(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeCheckedVariantCase]>,
        ordinal: u32,
        name: &HirName,
    ) -> Self {
        Self {
            owner: RuntimeVariantOwner::BuiltinClosed {
                identity,
                nominal,
                cases,
            },
            ordinal,
            case: RuntimeVariantCase::BuiltinClosed(name.clone()),
        }
    }

    /// Selects the payload-bearing Option case.
    pub fn option_some(item: RuntimeNormalizedType) -> Self {
        Self {
            owner: RuntimeVariantOwner::Option { item },
            ordinal: 0,
            case: RuntimeVariantCase::OptionSome,
        }
    }

    /// Selects the payload-free Option case.
    pub fn option_none(item: RuntimeNormalizedType) -> Self {
        Self {
            owner: RuntimeVariantOwner::Option { item },
            ordinal: 1,
            case: RuntimeVariantCase::OptionNone,
        }
    }

    /// Selects the successful Result constructor.
    pub fn result_ok(ok: RuntimeNormalizedType, error: RuntimeNormalizedType) -> Self {
        Self {
            owner: RuntimeVariantOwner::Result { ok, error },
            ordinal: 0,
            case: RuntimeVariantCase::ResultOk,
        }
    }

    /// Selects the error Result constructor.
    pub fn result_err(ok: RuntimeNormalizedType, error: RuntimeNormalizedType) -> Self {
        Self {
            owner: RuntimeVariantOwner::Result { ok, error },
            ordinal: 1,
            case: RuntimeVariantCase::ResultErr,
        }
    }

    pub const fn owner(&self) -> &RuntimeVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        match &self.case {
            RuntimeVariantCase::Project(name) => name,
            RuntimeVariantCase::Character(name) | RuntimeVariantCase::BuiltinClosed(name) => {
                name.as_str()
            }
            RuntimeVariantCase::OptionSome => "Some",
            RuntimeVariantCase::OptionNone => "None",
            RuntimeVariantCase::ResultOk => "Ok",
            RuntimeVariantCase::ResultErr => "Err",
        }
    }
}

/// One compiler-selected call dispatch for an exact final-HIR call expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedCall {
    target: RuntimeResolvedCallTarget,
    arguments: Box<[RuntimeResolvedCallArgument]>,
    result: RuntimeCallResultShape,
}

impl RuntimeResolvedCall {
    pub fn new(
        target: RuntimeResolvedCallTarget,
        arguments: impl Into<Box<[RuntimeResolvedCallArgument]>>,
        result: RuntimeCallResultShape,
    ) -> Self {
        Self {
            target,
            arguments: arguments.into(),
            result,
        }
    }

    pub const fn target(&self) -> &RuntimeResolvedCallTarget {
        &self.target
    }

    pub const fn arguments(&self) -> &[RuntimeResolvedCallArgument] {
        &self.arguments
    }

    pub const fn result(&self) -> RuntimeCallResultShape {
        self.result
    }
}

/// Closed runtime dispatch selected by the shared semantic resolver.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "the closed selected-target vocabulary deliberately retains exact typed owners without target-specific indirection"
)]
pub enum RuntimeResolvedCallTarget {
    Intrinsic(RuntimeIntrinsic),
    Agent(crate::agent::RuntimeAgentIntrinsic),
    AgentProbeComparison(crate::agent::RuntimeAgentProbeComparison),
    Declaration(RuntimeProjectCallable),
    /// Typed enum case selected by the shared callable resolver.
    ///
    /// Constructor calls are values in the runtime expression algebra, not
    /// registered callables or string-selected intrinsics.
    Variant(RuntimeResolvedVariant),
    /// Core-owned `Reduction` value construction selected by semantic identity.
    Reduction(RuntimeReductionConstructor),
    FunctionValue,
    TraitMethod {
        method: RuntimeTraitMethodId,
        receiver: RuntimeReceiverMode,
    },
    Registered(RuntimeCallableId),
    Host {
        declaration: RuntimeProjectCallable,
        mode: RuntimeHostCallMode,
    },
}

/// Closed core `Reduction` constructor vocabulary below semantic analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeReductionConstructor {
    Unchanged,
}

/// Runtime argument order after overload and data-last resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeResolvedCallArgument {
    Authored { ordinal: u32 },
    Receiver,
}

/// Whether a checked call produces its declared value or a partial function.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeCallResultShape {
    Value,
    PartialFunction,
}

/// Checked assertion disposition for an exact final-HIR assertion statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAssertionAdmission {
    Discharged,
    Runtime(RuntimeAssertionMode),
    OmittedDebug,
}

/// Checked capture metadata that is not derivable from lexical HIR alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedCapture {
    capture: CaptureId,
    ty: RuntimeNormalizedType,
}

/// One executable dialogue application projected from checked semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeDialogueApplication {
    content: DialogueContentSpec,
}

impl RuntimeDialogueApplication {
    pub const fn new(content: DialogueContentSpec) -> Self {
        Self { content }
    }

    pub const fn content(&self) -> &DialogueContentSpec {
        &self.content
    }
}

/// Trait authority selected by final semantic analysis for one executable
/// implementation method.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTraitIdentity {
    Project(ItemId),
    StandardIterator,
    StandardIntoIterator,
}

/// Generation-bound method identity consumed by final-HIR runtime lowering.
///
/// The runtime method ID is assigned deterministically by the compiler from
/// the ordered set of checked conformances. The implementation/member pair is
/// the sole body owner; no detached method catalog or source lookup is
/// retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTraitMethodFact {
    id: RuntimeTraitMethodId,
    implementation: ItemId,
    member: u16,
    trait_identity: RuntimeTraitIdentity,
    self_type: String,
}

impl RuntimeTraitMethodFact {
    pub fn new(
        id: RuntimeTraitMethodId,
        implementation: ItemId,
        member: u16,
        trait_identity: RuntimeTraitIdentity,
        self_type: String,
    ) -> Self {
        Self {
            id,
            implementation,
            member,
            trait_identity,
            self_type,
        }
    }

    pub const fn id(&self) -> RuntimeTraitMethodId {
        self.id
    }

    pub const fn implementation(&self) -> ItemId {
        self.implementation
    }

    pub const fn member(&self) -> u16 {
        self.member
    }

    pub const fn trait_identity(&self) -> &RuntimeTraitIdentity {
        &self.trait_identity
    }

    pub fn self_type(&self) -> &str {
        &self.self_type
    }
}

impl RuntimeCheckedCapture {
    pub const fn new(capture: CaptureId, ty: RuntimeNormalizedType) -> Self {
        Self { capture, ty }
    }

    pub const fn capture(&self) -> CaptureId {
        self.capture
    }

    pub const fn ty(&self) -> &RuntimeNormalizedType {
        &self.ty
    }
}

/// Mutable staging owner used by semantic analysis before generation binding.
///
/// Staged facts are not executable. [`RuntimePlanSemanticFacts::try_new`]
/// validates every owner and nested project identity before publication.
#[derive(Clone, Debug, Default)]
pub struct RuntimePlanSemanticFactInput {
    flows: Vec<(ItemId, FlowRuntimeId)>,
    expression_literals: Vec<(ExprId, RuntimeValue)>,
    pattern_literals: Vec<(PatternId, RuntimeValue)>,
    pattern_items: Vec<(PatternId, RuntimeProjectItem)>,
    values: Vec<(ExprId, RuntimeResolvedValue)>,
    selects: Vec<(ExprId, RuntimeResolvedSelect)>,
    nominals: Vec<(ExprId, RuntimeResolvedNominal)>,
    pattern_nominals: Vec<(PatternId, RuntimeResolvedNominal)>,
    expression_variants: Vec<(ExprId, RuntimeResolvedVariant)>,
    pattern_variants: Vec<(PatternId, RuntimeResolvedVariant)>,
    types: Vec<(TypeId, RuntimeNormalizedType)>,
    calls: Vec<(ExprId, RuntimeResolvedCall)>,
    postfix_candidates: Vec<(ExprId, ExprId)>,
    trait_methods: Vec<RuntimeTraitMethodFact>,
    iterations: Vec<(StmtId, RuntimeIteratorEvidence)>,
    assertions: Vec<(StmtId, RuntimeAssertionAdmission)>,
    captures: Vec<RuntimeCheckedCapture>,
}

impl RuntimePlanSemanticFactInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_flow(&mut self, owner: ItemId, identity: FlowRuntimeId) {
        self.flows.push((owner, identity));
    }

    pub fn push_expression_literal(&mut self, owner: ExprId, value: RuntimeValue) {
        self.expression_literals.push((owner, value));
    }

    pub fn push_pattern_literal(&mut self, owner: PatternId, value: RuntimeValue) {
        self.pattern_literals.push((owner, value));
    }

    pub fn push_pattern_item(&mut self, owner: PatternId, item: RuntimeProjectItem) {
        self.pattern_items.push((owner, item));
    }

    pub fn push_value(&mut self, owner: ExprId, value: RuntimeResolvedValue) {
        self.values.push((owner, value));
    }

    pub fn push_select(&mut self, owner: ExprId, select: RuntimeResolvedSelect) {
        self.selects.push((owner, select));
    }

    pub fn push_nominal(&mut self, owner: ExprId, nominal: RuntimeResolvedNominal) {
        self.nominals.push((owner, nominal));
    }

    pub fn push_pattern_nominal(&mut self, owner: PatternId, nominal: RuntimeResolvedNominal) {
        self.pattern_nominals.push((owner, nominal));
    }

    pub fn push_expression_variant(&mut self, owner: ExprId, variant: RuntimeResolvedVariant) {
        self.expression_variants.push((owner, variant));
    }

    pub fn push_pattern_variant(&mut self, owner: PatternId, variant: RuntimeResolvedVariant) {
        self.pattern_variants.push((owner, variant));
    }

    pub fn push_type(&mut self, owner: TypeId, ty: RuntimeNormalizedType) {
        self.types.push((owner, ty));
    }

    pub fn push_call(&mut self, owner: ExprId, call: RuntimeResolvedCall) {
        self.calls.push((owner, call));
    }

    /// Stages the exact semantic winner for one immutable postfix ambiguity.
    pub fn push_postfix_candidate(&mut self, owner: ExprId, candidate: ExprId) {
        self.postfix_candidates.push((owner, candidate));
    }

    pub fn push_iteration(&mut self, owner: StmtId, evidence: RuntimeIteratorEvidence) {
        self.iterations.push((owner, evidence));
    }

    pub fn push_trait_method(&mut self, method: RuntimeTraitMethodFact) {
        self.trait_methods.push(method);
    }

    pub fn push_assertion(&mut self, owner: StmtId, admission: RuntimeAssertionAdmission) {
        self.assertions.push((owner, admission));
    }

    pub fn push_capture(&mut self, capture: RuntimeCheckedCapture) {
        self.captures.push(capture);
    }
}

/// Immutable semantic fact set bound to one exact executable project generation.
#[derive(Clone, Debug)]
pub struct RuntimePlanSemanticFacts {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    flows: BTreeMap<ItemId, FlowRuntimeId>,
    expression_literals: BTreeMap<ExprId, RuntimeValue>,
    pattern_literals: BTreeMap<PatternId, RuntimeValue>,
    pattern_items: BTreeMap<PatternId, RuntimeProjectItem>,
    values: BTreeMap<ExprId, RuntimeResolvedValue>,
    selects: BTreeMap<ExprId, RuntimeResolvedSelect>,
    nominals: BTreeMap<ExprId, RuntimeResolvedNominal>,
    pattern_nominals: BTreeMap<PatternId, RuntimeResolvedNominal>,
    expression_variants: BTreeMap<ExprId, RuntimeResolvedVariant>,
    pattern_variants: BTreeMap<PatternId, RuntimeResolvedVariant>,
    types: BTreeMap<TypeId, RuntimeNormalizedType>,
    calls: BTreeMap<ExprId, RuntimeResolvedCall>,
    postfix_candidates: BTreeMap<ExprId, ExprId>,
    trait_methods: BTreeMap<RuntimeTraitMethodId, RuntimeTraitMethodFact>,
    iterations: BTreeMap<StmtId, RuntimeIteratorEvidence>,
    assertions: BTreeMap<StmtId, RuntimeAssertionAdmission>,
    captures: BTreeMap<CaptureId, RuntimeCheckedCapture>,
    dialogue_applications: BTreeMap<ExprId, RuntimeDialogueApplication>,
    character_presentation_catalog: Option<Arc<CharacterPresentationCatalogData>>,
}

impl RuntimePlanSemanticFacts {
    /// Validates every staged fact against the exact accepted executable module leases.
    #[allow(
        clippy::too_many_lines,
        reason = "fact publication validates every family and cross-owner identity in one all-or-nothing accepted-generation transaction"
    )]
    pub fn try_new(
        project: HirExecutableProjectView<'_>,
        input: RuntimePlanSemanticFactInput,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        let modules = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let snapshots = modules
            .iter()
            .map(|(id, module)| (*id, module.snapshot_id()))
            .collect();

        let flows = collect_unique(input.flows, RuntimeSemanticFactFamily::FlowIdentity)?;
        for item in flows.keys() {
            let resolved = resolve_item(&modules, *item)?;
            if !matches!(resolved.kind(), HirItemKind::Flow(_)) {
                return Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: *item,
                    actual: resolved.kind().family(),
                });
            }
        }

        let expression_literals = collect_unique(
            input.expression_literals,
            RuntimeSemanticFactFamily::ExpressionLiteral,
        )?;
        for expression in expression_literals.keys() {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::ExpressionLiteral,
                |kind| {
                    matches!(
                        kind,
                        HirExprKind::Literal(_) | HirExprKind::NumericBracketSequence(_)
                    )
                },
            )?;
        }

        let pattern_literals = collect_unique(
            input.pattern_literals,
            RuntimeSemanticFactFamily::PatternLiteral,
        )?;
        for pattern in pattern_literals.keys() {
            require_pattern_family(
                &modules,
                *pattern,
                RuntimeSemanticFactFamily::PatternLiteral,
                |kind| matches!(kind, HirPatternKind::Literal(_)),
            )?;
        }

        let pattern_items =
            collect_unique(input.pattern_items, RuntimeSemanticFactFamily::PatternItem)?;
        for (pattern, item) in &pattern_items {
            require_pattern_family(
                &modules,
                *pattern,
                RuntimeSemanticFactFamily::PatternItem,
                |kind| matches!(kind, HirPatternKind::EntityReference(_)),
            )?;
            validate_project_item(&modules, item)?;
        }

        let values = collect_unique(input.values, RuntimeSemanticFactFamily::Value)?;
        for (expression, value) in &values {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::Value,
                |kind| matches!(kind, HirExprKind::Path(_) | HirExprKind::EntityReference(_)),
            )?;
            validate_resolved_value(&modules, value)?;
            match (resolve_expr(&modules, *expression)?, value) {
                (
                    HirExprKind::Path(_),
                    RuntimeResolvedValue::ProjectItem(_) | RuntimeResolvedValue::DialogueLine(_),
                )
                | (
                    HirExprKind::EntityReference(_),
                    RuntimeResolvedValue::Local(_)
                    | RuntimeResolvedValue::ProjectCallable(_)
                    | RuntimeResolvedValue::Intrinsic(_)
                    | RuntimeResolvedValue::Registered(_)
                    | RuntimeResolvedValue::Constant(_),
                ) => {
                    return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                        expression: *expression,
                        expected: RuntimeSemanticFactFamily::Value,
                    });
                }
                (
                    HirExprKind::EntityReference(_),
                    RuntimeResolvedValue::ProjectItem(_) | RuntimeResolvedValue::DialogueLine(_),
                )
                | (HirExprKind::Path(_), _) => {}
                _ => unreachable!("value fact family was checked immediately above"),
            }
        }

        let selects = collect_unique(input.selects, RuntimeSemanticFactFamily::Select)?;
        for (expression, select) in &selects {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::Select,
                |kind| matches!(kind, HirExprKind::Select(_)),
            )?;
            validate_select(&modules, select)?;
        }

        let nominals = collect_unique(input.nominals, RuntimeSemanticFactFamily::Nominal)?;
        for (expression, nominal) in &nominals {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::Nominal,
                |kind| matches!(kind, HirExprKind::Record(_)),
            )?;
            validate_nominal(&modules, nominal)?;
        }

        let pattern_nominals = collect_unique(
            input.pattern_nominals,
            RuntimeSemanticFactFamily::PatternNominal,
        )?;
        for (pattern, nominal) in &pattern_nominals {
            require_pattern_family(
                &modules,
                *pattern,
                RuntimeSemanticFactFamily::PatternNominal,
                |kind| matches!(kind, HirPatternKind::Record { .. }),
            )?;
            validate_nominal(&modules, nominal)?;
        }

        let expression_variants = collect_unique(
            input.expression_variants,
            RuntimeSemanticFactFamily::ExpressionVariant,
        )?;
        for (expression, variant) in &expression_variants {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::ExpressionVariant,
                |kind| matches!(kind, HirExprKind::ShortVariant(_) | HirExprKind::Path(_)),
            )?;
            validate_variant(&modules, variant)?;
        }

        let pattern_variants = collect_unique(
            input.pattern_variants,
            RuntimeSemanticFactFamily::PatternVariant,
        )?;
        for (pattern, variant) in &pattern_variants {
            require_pattern_family(
                &modules,
                *pattern,
                RuntimeSemanticFactFamily::PatternVariant,
                |kind| matches!(kind, HirPatternKind::Variant(_)),
            )?;
            validate_variant(&modules, variant)?;
        }

        let types = collect_unique(input.types, RuntimeSemanticFactFamily::Type)?;
        for (owner, ty) in &types {
            let hir_type = module_for(&modules, owner.module())?
                .resolve_type(*owner)
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedType { ty: *owner })?;
            if hir_type.is_poisoned() {
                return Err(RuntimeSemanticFactsError::PoisonedType { ty: *owner });
            }
            validate_normalized_type(&modules, ty)?;
        }

        let calls = collect_unique(input.calls, RuntimeSemanticFactFamily::Call)?;
        for (expression, call) in &calls {
            let kind = resolve_expr(&modules, *expression)?;
            let HirExprKind::Call(hir_call) = kind else {
                return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                    expression: *expression,
                    expected: RuntimeSemanticFactFamily::Call,
                });
            };
            validate_call(&modules, hir_call, call)?;
        }

        let postfix_candidates = collect_unique(
            input.postfix_candidates,
            RuntimeSemanticFactFamily::PostfixCandidate,
        )?;
        for (expression, candidate) in &postfix_candidates {
            let HirExprKind::PostfixBracket(postfix) = resolve_expr(&modules, *expression)? else {
                return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                    expression: *expression,
                    expected: RuntimeSemanticFactFamily::PostfixCandidate,
                });
            };
            let arcweft_lang_hir::dialogue_application::HirPostfixBracketCandidates::Ambiguous {
                index,
                dialogue,
            } = postfix.candidates()
            else {
                return Err(RuntimeSemanticFactsError::WrongExpressionFamily {
                    expression: *expression,
                    expected: RuntimeSemanticFactFamily::PostfixCandidate,
                });
            };
            if candidate != index && candidate != dialogue {
                return Err(RuntimeSemanticFactsError::WrongPostfixCandidate {
                    expression: *expression,
                    candidate: *candidate,
                });
            }
            resolve_expr(&modules, *candidate)?;
        }

        let trait_methods = collect_unique(
            input
                .trait_methods
                .into_iter()
                .map(|method| (method.id(), method)),
            RuntimeSemanticFactFamily::TraitMethod,
        )?;
        for (position, (id, method)) in trait_methods.iter().enumerate() {
            if id.0 != position {
                return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
            }
            validate_trait_method(&modules, method)?;
        }

        let iterations = collect_unique(input.iterations, RuntimeSemanticFactFamily::Iteration)?;
        for (statement, evidence) in &iterations {
            require_stmt_family(
                &modules,
                *statement,
                RuntimeSemanticFactFamily::Iteration,
                |kind| matches!(kind, HirStmtKind::For(_)),
            )?;
            let methods_exist = match evidence {
                RuntimeIteratorEvidence::Builtin(_) => true,
                RuntimeIteratorEvidence::Witness(witness) => match witness.executable {
                    arcweft_core::plan::RuntimeIteratorWitnessExecutable::TraitCalls(calls) => {
                        trait_methods.contains_key(&calls.into_iter)
                            && trait_methods.contains_key(&calls.next)
                    }
                    arcweft_core::plan::RuntimeIteratorWitnessExecutable::IdentityIntoIterator(
                        calls,
                    ) => trait_methods.contains_key(&calls.next),
                    arcweft_core::plan::RuntimeIteratorWitnessExecutable::UnsupportedMethodBodyLowering => false,
                },
            };
            if !methods_exist {
                return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
            }
        }

        let assertions = collect_unique(input.assertions, RuntimeSemanticFactFamily::Assertion)?;
        for statement in assertions.keys() {
            require_stmt_family(
                &modules,
                *statement,
                RuntimeSemanticFactFamily::Assertion,
                |kind| matches!(kind, HirStmtKind::Assertion { .. }),
            )?;
        }

        let mut captures = BTreeMap::new();
        for checked in input.captures {
            let id = checked.capture();
            module_for(&modules, id.module())?
                .resolve_capture(id)
                .map_err(|_| RuntimeSemanticFactsError::UnresolvedCapture { capture: id })?;
            validate_normalized_type(&modules, checked.ty())?;
            if captures.insert(id, checked).is_some() {
                return Err(RuntimeSemanticFactsError::DuplicateFact {
                    family: RuntimeSemanticFactFamily::Capture,
                });
            }
        }

        Ok(Self {
            snapshots,
            flows,
            expression_literals,
            pattern_literals,
            pattern_items,
            values,
            selects,
            nominals,
            pattern_nominals,
            expression_variants,
            pattern_variants,
            types,
            calls,
            postfix_candidates,
            trait_methods,
            iterations,
            assertions,
            captures,
            dialogue_applications: BTreeMap::new(),
            character_presentation_catalog: None,
        })
    }

    /// Binds the complete dialogue projection to this exact accepted HIR generation.
    pub fn with_dialogue_projection(
        mut self,
        project: HirExecutableProjectView<'_>,
        catalog: Option<Arc<CharacterPresentationCatalogData>>,
        applications: impl IntoIterator<Item = (ExprId, RuntimeDialogueApplication)>,
    ) -> Result<Self, RuntimeSemanticFactsError> {
        self.validate_generation(project)?;
        let applications =
            collect_unique(applications, RuntimeSemanticFactFamily::DialogueApplication)?;
        if applications.is_empty() != catalog.is_none() {
            return Err(RuntimeSemanticFactsError::DialogueCatalogPresenceMismatch);
        }
        let modules = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        for (owner, application) in &applications {
            require_expr_family(
                &modules,
                *owner,
                RuntimeSemanticFactFamily::DialogueApplication,
                |kind| matches!(kind, HirExprKind::DialogueContentApplication(_)),
            )?;
            let accepted = project
                .dialogue_lines()
                .for_expr(*owner)
                .ok_or(RuntimeSemanticFactsError::DialogueLineMismatch { expression: *owner })?;
            let accepted_runtime_line =
                RuntimeLineId::from_source_entity_body(accepted.id().as_str()).map_err(|_| {
                    RuntimeSemanticFactsError::DialogueLineMismatch { expression: *owner }
                })?;
            if &accepted_runtime_line != application.content().line()
                || accepted.text_key().as_str() != application.content().text_key().as_str()
            {
                return Err(RuntimeSemanticFactsError::DialogueLineMismatch { expression: *owner });
            }
            let catalog = catalog
                .as_ref()
                .ok_or(RuntimeSemanticFactsError::DialogueCatalogPresenceMismatch)?;
            if application.content().character().semantic_digest() != catalog.semantic_digest()
                || application.content().character().locale_policy_digest()
                    != catalog.locale_policy_digest()
            {
                return Err(RuntimeSemanticFactsError::DialogueCharacterPlanMismatch {
                    expression: *owner,
                });
            }
            if let arcweft_dialogue::character_presentation::CharacterPresentationTargetEvidence::Exact(character) =
                application.content().character().target()
                && catalog.record(character).is_err()
            {
                return Err(RuntimeSemanticFactsError::DialogueCharacterPlanMismatch {
                    expression: *owner,
                });
            }
        }
        self.dialogue_applications = applications;
        self.character_presentation_catalog = catalog;
        Ok(self)
    }

    /// Revalidates that the facts are consumed by the exact generation that
    /// admitted them. Stable IDs surviving a reload do not make stale facts valid.
    pub fn validate_generation(
        &self,
        project: HirExecutableProjectView<'_>,
    ) -> Result<(), RuntimeSemanticFactsError> {
        let actual = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.snapshot_id()))
            .collect::<BTreeMap<_, _>>();
        if actual == self.snapshots {
            Ok(())
        } else {
            Err(RuntimeSemanticFactsError::WrongProjectGeneration)
        }
    }

    pub fn expression_literal(&self, expression: ExprId) -> Option<&RuntimeValue> {
        self.expression_literals.get(&expression)
    }

    /// Compiler-admitted core identity for one exact final-HIR Flow item.
    pub fn flow(&self, item: ItemId) -> Option<&FlowRuntimeId> {
        self.flows.get(&item)
    }

    pub fn pattern_literal(&self, pattern: PatternId) -> Option<&RuntimeValue> {
        self.pattern_literals.get(&pattern)
    }

    pub fn pattern_item(&self, pattern: PatternId) -> Option<&RuntimeProjectItem> {
        self.pattern_items.get(&pattern)
    }

    pub fn value(&self, expression: ExprId) -> Option<&RuntimeResolvedValue> {
        self.values.get(&expression)
    }

    pub fn select(&self, expression: ExprId) -> Option<&RuntimeResolvedSelect> {
        self.selects.get(&expression)
    }

    pub fn nominal(&self, expression: ExprId) -> Option<&RuntimeResolvedNominal> {
        self.nominals.get(&expression)
    }

    pub fn pattern_nominal(&self, pattern: PatternId) -> Option<&RuntimeResolvedNominal> {
        self.pattern_nominals.get(&pattern)
    }

    pub fn expression_variant(&self, expression: ExprId) -> Option<&RuntimeResolvedVariant> {
        self.expression_variants.get(&expression)
    }

    pub fn pattern_variant(&self, pattern: PatternId) -> Option<&RuntimeResolvedVariant> {
        self.pattern_variants.get(&pattern)
    }

    pub fn ty(&self, ty: TypeId) -> Option<&RuntimeNormalizedType> {
        self.types.get(&ty)
    }

    pub fn call(&self, expression: ExprId) -> Option<&RuntimeResolvedCall> {
        self.calls.get(&expression)
    }

    /// Returns the sole checked candidate selected for one postfix root.
    pub fn postfix_candidate(&self, expression: ExprId) -> Option<ExprId> {
        self.postfix_candidates.get(&expression).copied()
    }

    pub fn iteration(&self, statement: StmtId) -> Option<&RuntimeIteratorEvidence> {
        self.iterations.get(&statement)
    }

    pub fn trait_methods(&self) -> impl ExactSizeIterator<Item = &RuntimeTraitMethodFact> {
        self.trait_methods.values()
    }

    pub fn assertion(&self, statement: StmtId) -> Option<RuntimeAssertionAdmission> {
        self.assertions.get(&statement).copied()
    }

    pub fn capture(&self, capture: CaptureId) -> Option<&RuntimeCheckedCapture> {
        self.captures.get(&capture)
    }

    pub fn dialogue_application(&self, expression: ExprId) -> Option<&RuntimeDialogueApplication> {
        self.dialogue_applications.get(&expression)
    }

    pub fn dialogue_applications(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ExprId, &RuntimeDialogueApplication)> {
        self.dialogue_applications.iter()
    }

    pub const fn character_presentation_catalog(
        &self,
    ) -> Option<&Arc<CharacterPresentationCatalogData>> {
        self.character_presentation_catalog.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSemanticFactsError {
    #[error("runtime semantic facts are bound to a different accepted HIR generation")]
    WrongProjectGeneration,
    #[error("runtime semantic facts contain more than one {family:?} fact for the same HIR ID")]
    DuplicateFact { family: RuntimeSemanticFactFamily },
    #[error("runtime semantic fact references unknown HIR module {module:?}")]
    UnknownModule { module: HirModuleId },
    #[error("runtime semantic fact references unresolved item {item:?}")]
    UnresolvedItem { item: ItemId },
    #[error("runtime semantic fact references unresolved local {local:?}")]
    UnresolvedLocal { local: LocalId },
    #[error("runtime semantic fact references unresolved expression {expression:?}")]
    UnresolvedExpression { expression: ExprId },
    #[error("runtime semantic fact references unresolved statement {statement:?}")]
    UnresolvedStatement { statement: StmtId },
    #[error("runtime semantic fact references unresolved pattern {pattern:?}")]
    UnresolvedPattern { pattern: PatternId },
    #[error("runtime semantic fact references unresolved type {ty:?}")]
    UnresolvedType { ty: TypeId },
    #[error("runtime semantic fact references poisoned type {ty:?}")]
    PoisonedType { ty: TypeId },
    #[error("runtime semantic fact references unresolved capture {capture:?}")]
    UnresolvedCapture { capture: CaptureId },
    #[error("expression {expression:?} cannot own a {expected:?} runtime semantic fact")]
    WrongExpressionFamily {
        expression: ExprId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("statement {statement:?} cannot own a {expected:?} runtime semantic fact")]
    WrongStatementFamily {
        statement: StmtId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("pattern {pattern:?} cannot own a {expected:?} runtime semantic fact")]
    WrongPatternFamily {
        pattern: PatternId,
        expected: RuntimeSemanticFactFamily,
    },
    #[error("runtime semantic fact item {item:?} has incompatible HIR family {actual:?}")]
    WrongItemFamily { item: ItemId, actual: HirItemFamily },
    #[error("runtime variant fact does not match its typed owner and source-ordered ordinal")]
    WrongVariantIdentity,
    #[error("runtime project item does not match its typed owner and public ID")]
    WrongProjectItemIdentity,
    #[error(
        "runtime call fact references authored argument {ordinal}, but the call has {count} arguments"
    )]
    InvalidCallArgumentOrdinal { ordinal: u32, count: usize },
    #[error("runtime call fact repeats one argument projection")]
    DuplicateCallArgument,
    #[error("runtime function-value call fact is attached to a call without a value callee")]
    MissingFunctionValueCallee,
    #[error(
        "Reduction.unchanged runtime call requires exactly one authored argument and a value result"
    )]
    InvalidReductionConstructorCall,
    #[error("postfix expression {expression:?} does not own selected candidate {candidate:?}")]
    WrongPostfixCandidate {
        expression: ExprId,
        candidate: ExprId,
    },
    #[error("runtime trait method fact does not match its final-HIR implementation member")]
    InvalidTraitMethodIdentity,
    #[error("dialogue projection and Character presentation catalog presence disagree")]
    DialogueCatalogPresenceMismatch,
    #[error("dialogue application {expression:?} does not match its accepted line identity")]
    DialogueLineMismatch { expression: ExprId },
    #[error("dialogue application {expression:?} carries stale or unknown Character evidence")]
    DialogueCharacterPlanMismatch { expression: ExprId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSemanticFactFamily {
    FlowIdentity,
    ExpressionLiteral,
    PatternLiteral,
    PatternItem,
    Value,
    Select,
    Nominal,
    PatternNominal,
    ExpressionVariant,
    PatternVariant,
    Type,
    Call,
    PostfixCandidate,
    TraitMethod,
    Iteration,
    Assertion,
    Capture,
    DialogueApplication,
}

fn collect_unique<K: Ord + Copy, V>(
    values: impl IntoIterator<Item = (K, V)>,
    family: RuntimeSemanticFactFamily,
) -> Result<BTreeMap<K, V>, RuntimeSemanticFactsError> {
    let mut result = BTreeMap::new();
    for (key, value) in values {
        if result.insert(key, value).is_some() {
            return Err(RuntimeSemanticFactsError::DuplicateFact { family });
        }
    }
    Ok(result)
}

fn module_for<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: HirModuleId,
) -> Result<&'project HirModule, RuntimeSemanticFactsError> {
    modules
        .get(&id)
        .copied()
        .ok_or(RuntimeSemanticFactsError::UnknownModule { module: id })
}

fn resolve_expr<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: ExprId,
) -> Result<&'project HirExprKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_expr(id)
        .map(arcweft_lang_hir::expr::HirExpr::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedExpression { expression: id })
}

fn resolve_stmt<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: StmtId,
) -> Result<&'project HirStmtKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_stmt(id)
        .map(arcweft_lang_hir::stmt::HirStmt::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedStatement { statement: id })
}

fn resolve_pattern<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: PatternId,
) -> Result<&'project HirPatternKind, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_pattern(id)
        .map(arcweft_lang_hir::pattern::HirPattern::kind)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedPattern { pattern: id })
}

fn require_expr_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    expression: ExprId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirExprKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    if predicate(resolve_expr(modules, expression)?) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongExpressionFamily {
            expression,
            expected,
        })
    }
}

fn require_stmt_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    statement: StmtId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirStmtKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    if predicate(resolve_stmt(modules, statement)?) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongStatementFamily {
            statement,
            expected,
        })
    }
}

fn require_pattern_family(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    pattern: PatternId,
    expected: RuntimeSemanticFactFamily,
    predicate: impl FnOnce(&HirPatternKind) -> bool,
) -> Result<(), RuntimeSemanticFactsError> {
    if predicate(resolve_pattern(modules, pattern)?) {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongPatternFamily { pattern, expected })
    }
}

fn validate_resolved_value(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    value: &RuntimeResolvedValue,
) -> Result<(), RuntimeSemanticFactsError> {
    match value {
        RuntimeResolvedValue::Local(local) => module_for(modules, local.module())?
            .resolve_local(*local)
            .map(|_| ())
            .map_err(|_| RuntimeSemanticFactsError::UnresolvedLocal { local: *local }),
        RuntimeResolvedValue::ProjectCallable(callable) => validate_callable(modules, callable),
        RuntimeResolvedValue::ProjectItem(item) => validate_project_item(modules, item),
        RuntimeResolvedValue::DialogueLine(_)
        | RuntimeResolvedValue::Intrinsic(_)
        | RuntimeResolvedValue::Registered(_)
        | RuntimeResolvedValue::Constant(_) => Ok(()),
    }
}

fn validate_project_item(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    item: &RuntimeProjectItem,
) -> Result<(), RuntimeSemanticFactsError> {
    item.family()
        .validate_public_id(item.public_id())
        .map_err(|_| RuntimeSemanticFactsError::WrongProjectItemIdentity)?;
    match item.owner() {
        RuntimeProjectItemOwner::ExternalCharacter => (item.family()
            == DeclarationIdentityFamily::Character)
            .then_some(())
            .ok_or(RuntimeSemanticFactsError::WrongProjectItemIdentity),
        RuntimeProjectItemOwner::StructuralFlow { owner, .. } => (item.family()
            == DeclarationIdentityFamily::Flow
            && matches!(resolve_item(modules, *owner)?.kind(), HirItemKind::Flow(_)))
        .then_some(())
        .ok_or(RuntimeSemanticFactsError::WrongProjectItemIdentity),
        RuntimeProjectItemOwner::Retained(owner) => {
            let actual = resolve_item(modules, *owner)?.kind().family();
            let expected = match item.family() {
                DeclarationIdentityFamily::Character => HirItemFamily::Character,
                DeclarationIdentityFamily::View => HirItemFamily::View,
                DeclarationIdentityFamily::Action => HirItemFamily::Action,
                DeclarationIdentityFamily::Activity => HirItemFamily::Activity,
                DeclarationIdentityFamily::Signal => HirItemFamily::Signal,
                DeclarationIdentityFamily::Metric => HirItemFamily::Metric,
                DeclarationIdentityFamily::Layer => HirItemFamily::Layer,
                DeclarationIdentityFamily::Asset
                | DeclarationIdentityFamily::Flow
                | DeclarationIdentityFamily::Proof
                | DeclarationIdentityFamily::Source
                | DeclarationIdentityFamily::Style => {
                    return Err(RuntimeSemanticFactsError::WrongProjectItemIdentity);
                }
            };
            if actual == expected {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: *owner,
                    actual,
                })
            }
        }
    }
}

fn validate_select(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    select: &RuntimeResolvedSelect,
) -> Result<(), RuntimeSemanticFactsError> {
    match select {
        RuntimeResolvedSelect::Method { .. } | RuntimeResolvedSelect::TupleElement { .. } => Ok(()),
        RuntimeResolvedSelect::Field { nominal, .. }
        | RuntimeResolvedSelect::RecordElement { nominal, .. } => nominal
            .as_ref()
            .map_or(Ok(()), |nominal| validate_nominal(modules, nominal)),
    }
}

fn validate_variant(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    variant: &RuntimeResolvedVariant,
) -> Result<(), RuntimeSemanticFactsError> {
    match variant.owner() {
        RuntimeVariantOwner::Project { nominal, cases } => {
            validate_nominal(modules, nominal)?;
            let HirItemKind::Enum(declaration) = resolve_item(modules, nominal.owner())?.kind()
            else {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            };
            if declaration.variants().len() != cases.len()
                || declaration
                    .variants()
                    .iter()
                    .zip(cases)
                    .any(|(declaration, checked)| {
                        declaration.name().resolved().map(HirName::as_str)
                            != Some(checked.name.as_str())
                            || declaration.payload().is_some() != checked.payload.is_some()
                    })
            {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            }
            let selected = usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| declaration.variants().get(ordinal))
                .and_then(|selected| selected.name().resolved())
                .ok_or(RuntimeSemanticFactsError::WrongVariantIdentity)?;
            if selected.as_str() == variant.name() {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeVariantOwner::CharacterNominal { nominal, cases, .. } => {
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name.is_empty()
                        || case.payload.is_some()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name == case.name)
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name != variant.name())
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::BuiltinClosed { nominal, cases, .. } => {
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name.is_empty()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name == case.name)
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name != variant.name())
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::Option { item } => {
            validate_normalized_type(modules, item)?;
            if matches!(
                (variant.ordinal(), variant.name()),
                (0, "Some") | (1, "None")
            ) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeVariantOwner::Result { ok, error } => {
            validate_normalized_type(modules, ok)?;
            validate_normalized_type(modules, error)?;
            if matches!((variant.ordinal(), variant.name()), (0, "Ok") | (1, "Err")) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
    }
}

fn validate_normalized_type(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    ty: &RuntimeNormalizedType,
) -> Result<(), RuntimeSemanticFactsError> {
    match ty.shape() {
        RuntimeTypeShape::Range(item)
        | RuntimeTypeShape::Iterator(item)
        | RuntimeTypeShape::Sequence { item, .. }
        | RuntimeTypeShape::Array { item, .. }
        | RuntimeTypeShape::Option(item)
        | RuntimeTypeShape::ThreadHandle(item)
        | RuntimeTypeShape::Shared(item)
        | RuntimeTypeShape::Reference(item) => validate_normalized_type(modules, item),
        RuntimeTypeShape::Map { key, value }
        | RuntimeTypeShape::Need {
            ready: key,
            error: value,
        }
        | RuntimeTypeShape::Stream {
            item: key,
            error: value,
        }
        | RuntimeTypeShape::Source {
            item: key,
            error: value,
        }
        | RuntimeTypeShape::Result {
            value: key,
            error: value,
        } => {
            validate_normalized_type(modules, key)?;
            validate_normalized_type(modules, value)
        }
        RuntimeTypeShape::Function { parameters, result } => {
            for parameter in parameters {
                validate_normalized_type(modules, parameter)?;
            }
            validate_normalized_type(modules, result)
        }
        RuntimeTypeShape::ProjectNominal { nominal, arguments } => {
            validate_nominal(modules, nominal)?;
            for argument in arguments {
                validate_normalized_type(modules, argument)?;
            }
            Ok(())
        }
        RuntimeTypeShape::Tuple(items) | RuntimeTypeShape::Choice(items) => {
            for item in items {
                validate_normalized_type(modules, item)?;
            }
            Ok(())
        }
        RuntimeTypeShape::Unit
        | RuntimeTypeShape::Never
        | RuntimeTypeShape::Bool
        | RuntimeTypeShape::Signed(_)
        | RuntimeTypeShape::Unsigned(_)
        | RuntimeTypeShape::F32
        | RuntimeTypeShape::F64
        | RuntimeTypeShape::String
        | RuntimeTypeShape::Char
        | RuntimeTypeShape::Bytes
        | RuntimeTypeShape::Duration
        | RuntimeTypeShape::EntityReference
        | RuntimeTypeShape::Named { .. }
        | RuntimeTypeShape::Opaque => Ok(()),
    }
}

fn validate_call(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    hir_call: &HirCallExpr,
    call: &RuntimeResolvedCall,
) -> Result<(), RuntimeSemanticFactsError> {
    match call.target() {
        RuntimeResolvedCallTarget::Declaration(callable)
        | RuntimeResolvedCallTarget::Host {
            declaration: callable,
            ..
        } => validate_callable(modules, callable)?,
        RuntimeResolvedCallTarget::FunctionValue => {
            if hir_call.callee().value_expression().is_none() {
                return Err(RuntimeSemanticFactsError::MissingFunctionValueCallee);
            }
        }
        RuntimeResolvedCallTarget::Variant(variant) => validate_variant(modules, variant)?,
        RuntimeResolvedCallTarget::Agent(_)
        | RuntimeResolvedCallTarget::AgentProbeComparison(_)
        | RuntimeResolvedCallTarget::Reduction(_)
        | RuntimeResolvedCallTarget::Intrinsic(_)
        | RuntimeResolvedCallTarget::TraitMethod { .. }
        | RuntimeResolvedCallTarget::Registered(_) => {}
    }

    let count = hir_call.arguments().len();
    let mut seen = BTreeSet::new();
    for argument in call.arguments() {
        if let RuntimeResolvedCallArgument::Authored { ordinal } = argument
            && usize::try_from(*ordinal).map_or(true, |ordinal| ordinal >= count)
        {
            return Err(RuntimeSemanticFactsError::InvalidCallArgumentOrdinal {
                ordinal: *ordinal,
                count,
            });
        }
        if !seen.insert(*argument) {
            return Err(RuntimeSemanticFactsError::DuplicateCallArgument);
        }
    }
    if matches!(call.target(), RuntimeResolvedCallTarget::Reduction(_))
        && (!matches!(
            call.arguments(),
            [RuntimeResolvedCallArgument::Authored { ordinal: 0 }]
        ) || call.result() != RuntimeCallResultShape::Value)
    {
        return Err(RuntimeSemanticFactsError::InvalidReductionConstructorCall);
    }
    Ok(())
}

fn validate_callable(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    callable: &RuntimeProjectCallable,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, callable.owner())?;
    let valid = matches!(
        (callable.declaration().owner(), item.kind()),
        (CallableDeclarationOwner::Function, HirItemKind::Function(_))
            | (
                CallableDeclarationOwner::ExternCapability,
                HirItemKind::ExternCapability(_)
            )
            | (CallableDeclarationOwner::View, HirItemKind::View(_))
            | (
                CallableDeclarationOwner::Predicate,
                HirItemKind::Predicate(_)
            )
            | (CallableDeclarationOwner::Proof, HirItemKind::Proof(_))
            | (
                CallableDeclarationOwner::TraitRequirement,
                HirItemKind::Trait(_)
            )
            | (
                CallableDeclarationOwner::TraitImplementation
                    | CallableDeclarationOwner::InherentMethod,
                HirItemKind::Impl(_)
            )
    );
    if valid {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: callable.owner(),
            actual: item.kind().family(),
        })
    }
}

fn validate_nominal(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    nominal: &RuntimeResolvedNominal,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, nominal.owner())?;
    let valid = matches!(
        (nominal.declaration().kind(), item.kind()),
        (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Struct,
            HirItemKind::Struct(_)
        ) | (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Enum,
            HirItemKind::Enum(_)
        ) | (
            arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::TypeAlias,
            HirItemKind::TypeAlias(_)
        )
    );
    if valid {
        Ok(())
    } else {
        Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: nominal.owner(),
            actual: item.kind().family(),
        })
    }
}

fn resolve_item<'project>(
    modules: &BTreeMap<HirModuleId, &'project HirModule>,
    id: ItemId,
) -> Result<&'project arcweft_lang_hir::item::HirItem, RuntimeSemanticFactsError> {
    module_for(modules, id.module())?
        .resolve_item(id)
        .map_err(|_| RuntimeSemanticFactsError::UnresolvedItem { item: id })
}

fn validate_trait_method(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    method: &RuntimeTraitMethodFact,
) -> Result<(), RuntimeSemanticFactsError> {
    let item = resolve_item(modules, method.implementation())?;
    let HirItemKind::Impl(implementation) = item.kind() else {
        return Err(RuntimeSemanticFactsError::WrongItemFamily {
            item: method.implementation(),
            actual: item.kind().family(),
        });
    };
    let Some(HirImplMember::Function(function)) =
        implementation.members().get(usize::from(method.member()))
    else {
        return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
    };
    let name = function
        .name()
        .resolved()
        .ok_or(RuntimeSemanticFactsError::InvalidTraitMethodIdentity)?;
    let expected = match method.trait_identity() {
        RuntimeTraitIdentity::Project(trait_item) => {
            let trait_owner = *trait_item;
            let trait_item = resolve_item(modules, trait_owner)?;
            if !matches!(trait_item.kind(), HirItemKind::Trait(_)) {
                return Err(RuntimeSemanticFactsError::WrongItemFamily {
                    item: trait_owner,
                    actual: trait_item.kind().family(),
                });
            }
            None
        }
        RuntimeTraitIdentity::StandardIterator => Some(("Iterator", "next")),
        RuntimeTraitIdentity::StandardIntoIterator => Some(("IntoIterator", "into_iter")),
    };
    if method.self_type().is_empty() {
        return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
    }
    if let Some((trait_name, method_name)) = expected {
        let trait_ref = implementation
            .trait_ref()
            .ok_or(RuntimeSemanticFactsError::InvalidTraitMethodIdentity)?;
        let trait_ref = module_for(modules, trait_ref.module())?
            .resolve_type(trait_ref)
            .map_err(|_| RuntimeSemanticFactsError::UnresolvedType { ty: trait_ref })?;
        let HirTypeKind::Path(path) = trait_ref.kind() else {
            return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
        };
        let terminal = path.segments().last().map(|segment| match segment {
            HirPathSegment::Identifier(name) => name.as_str(),
            HirPathSegment::ProjectSymbol(name) => name.as_str(),
        });
        if terminal != Some(trait_name) || name.as_str() != method_name {
            return Err(RuntimeSemanticFactsError::InvalidTraitMethodIdentity);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "semantic_facts/tests.rs"]
mod tests;
