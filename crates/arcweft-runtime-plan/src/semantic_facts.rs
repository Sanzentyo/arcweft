//! Accepted-generation semantic facts consumed by final-HIR runtime lowering.
//!
//! Runtime lowering is intentionally below semantic analysis in the crate
//! graph. The compiler therefore projects checked decisions into this closed
//! vocabulary and binds them to the exact executable HIR generation. Facts are
//! keyed by qualified final-HIR IDs; source-order counters, byte ranges,
//! display labels, and reconstructed paths are not accepted as identities.
//!
//! The closed fact vocabulary, staging input, immutable inventory, and atomic
//! admission remain one cohesive boundary so their families cannot drift into
//! partial schemas. HIR-owned graph traversal is delegated to HIR rather than
//! duplicated here; only runtime-plan validation and storage stay in this
//! module.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_character::presentation_name::CharacterPresentationCatalogData;
use arcweft_core::entry::{
    RuntimeCallableId, RuntimeIdentityError, RuntimeNominalTypeId, TypeLayoutHash,
};
pub use arcweft_core::pattern::RuntimeSemanticTypeId;
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimeOpaqueTypeAdmission,
    RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeIteratorEvidence, RuntimeLineId, RuntimeLocalDeclarationTable,
    RuntimeLocalDeclarationTableBuilder, RuntimeLocalDeclarationTableError, RuntimeOperationalType,
    RuntimePlanTypeKind, RuntimeReceiverMode, RuntimeTraitMethodId,
};
use arcweft_core::runtime_id::RuntimeLocalDeclarationId;
use arcweft_core::step::RuntimeHostCallMode;
use arcweft_core::value::{
    RuntimeIntrinsic, RuntimeNominalRecordLayout, RuntimeSignedIntWidth, RuntimeUnsignedIntWidth,
    RuntimeValue,
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
use arcweft_lang_hir::project::{HirExecutableProjectView, HirSelectedExpressionInventoryError};
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_hir::symbol::{
    CallableDeclarationKey, CallableDeclarationOwner, nominal::ProjectNominalDeclarationId,
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
    Tuple(Box<[RuntimeNormalizedType]>),
    Choice(Box<[RuntimeNormalizedType]>),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        admission: RuntimeOpaqueTypeAdmission,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSequenceKind {
    Vec,
    Array,
    Slice,
    Seq,
}

/// One deterministic descent from a normalized runtime type to a checked leaf.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTypeProjectionStep {
    SequenceItem,
    TupleItem(u32),
    ChoiceAlternative(u32),
    ResultOk,
    ResultError,
    OptionItem,
}

/// Typed location of the first checked-type projection failure.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeProjectionPath(Box<[RuntimeTypeProjectionStep]>);

impl RuntimeTypeProjectionPath {
    #[must_use]
    pub fn root() -> Self {
        Self(Box::new([]))
    }

    #[must_use]
    pub fn pushed(&self, step: RuntimeTypeProjectionStep) -> Self {
        let mut steps = self.0.to_vec();
        steps.push(step);
        Self(steps.into_boxed_slice())
    }

    #[must_use]
    pub const fn steps(&self) -> &[RuntimeTypeProjectionStep] {
        &self.0
    }
}

/// Closed diagnostic category for shapes outside the checked value algebra.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeUnsupportedTypeShape {
    Range,
    Iterator,
    Map,
    Need,
    Stream,
    Source,
    ThreadHandle,
    Shared,
    Reference,
    Function,
}

/// Invalid retained identity on a checked project nominal fact.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedNominalError {
    #[error(transparent)]
    InvalidIdentity(#[from] RuntimeIdentityError),
}

/// Failure to project a normalized semantic type into the closed runtime algebra.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCheckedTypeProjectionError {
    #[error("runtime type `{type_label}` has no opaque producer evidence")]
    MissingOpaqueProducerEvidence {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        type_label: String,
    },
    #[error("runtime type shape `{shape:?}` is not representable")]
    UnsupportedRuntimeShape {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        shape: RuntimeUnsupportedTypeShape,
    },
    #[error("project nominal runtime identity is invalid")]
    InvalidProjectNominal {
        semantic_identity: RuntimeSemanticTypeId,
        path: RuntimeTypeProjectionPath,
        reason: RuntimeResolvedNominalError,
    },
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

    pub fn checked_type(&self) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        self.checked_type_at(&RuntimeTypeProjectionPath::root())
    }

    /// Selects the final runtime representation for this normalized type.
    ///
    /// Complete checked shapes retain their exact checked predicate. When a
    /// checked projection reaches an operational descendant, the normalized
    /// root selects its closed execution family. Nominal and opaque validation
    /// failures remain errors rather than being reclassified.
    pub fn runtime_plan_type_kind(
        &self,
    ) -> Result<RuntimePlanTypeKind, RuntimeCheckedTypeProjectionError> {
        self.classify_runtime_plan_type_projection(self.checked_type())
    }

    fn classify_runtime_plan_type_projection(
        &self,
        projection: Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError>,
    ) -> Result<RuntimePlanTypeKind, RuntimeCheckedTypeProjectionError> {
        match projection {
            Ok(checked) => Ok(RuntimePlanTypeKind::Checked(checked)),
            Err(error @ RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape { .. }) => self
                .shape
                .operational_type()
                .map(RuntimePlanTypeKind::Operational)
                .ok_or(error),
            Err(error) => Err(error),
        }
    }

    fn checked_type_at(
        &self,
        path: &RuntimeTypeProjectionPath,
    ) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        if let Some(checked) = self.checked_leaf_type() {
            return Ok(checked);
        }
        if let Some(shape) = unsupported_runtime_shape(self.shape()) {
            return Err(self.unsupported(path, shape));
        }
        Ok(match self.shape() {
            RuntimeTypeShape::Sequence { item, .. } | RuntimeTypeShape::Array { item, .. } => {
                RuntimeCheckedType::Sequence(Box::new(
                    item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::SequenceItem))?,
                ))
            }
            RuntimeTypeShape::ProjectNominal { nominal, .. } => RuntimeCheckedType::Nominal {
                nominal: RuntimeNominalTypeId::try_new(nominal.declaration().qualified_name())
                    .map_err(
                        |error| RuntimeCheckedTypeProjectionError::InvalidProjectNominal {
                            semantic_identity: self.identity(),
                            path: path.clone(),
                            reason: error.into(),
                        },
                    )?,
                semantic_identity: self.identity(),
                layout: nominal.layout(),
            },
            RuntimeTypeShape::Choice(alternatives) => RuntimeCheckedType::Choice(
                alternatives
                    .iter()
                    .enumerate()
                    .map(|(index, alternative)| {
                        alternative.checked_type_at(&path.pushed(
                            RuntimeTypeProjectionStep::ChoiceAlternative(projection_index(index)),
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Tuple(items) => RuntimeCheckedType::Tuple(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::TupleItem(
                            projection_index(index),
                        )))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RuntimeTypeShape::Result { value, error } => RuntimeCheckedType::Result {
                ok: Box::new(
                    value.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::ResultOk))?,
                ),
                error: Box::new(
                    error.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::ResultError))?,
                ),
            },
            RuntimeTypeShape::Option(item) => RuntimeCheckedType::Option(Box::new(
                item.checked_type_at(&path.pushed(RuntimeTypeProjectionStep::OptionItem))?,
            )),
            RuntimeTypeShape::Opaque {
                producer,
                admission,
            } => RuntimeCheckedType::Opaque {
                owner: match admission {
                    RuntimeOpaqueTypeAdmission::ExactIdentity => {
                        RuntimeOpaqueTypeOwner::exact(producer.clone(), self.identity())
                    }
                    RuntimeOpaqueTypeAdmission::ProducerWide => {
                        RuntimeOpaqueTypeOwner::producer_wide(producer.clone(), self.identity())
                    }
                },
            },
            RuntimeTypeShape::Never
            | RuntimeTypeShape::Unit
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
            | RuntimeTypeShape::Range(_)
            | RuntimeTypeShape::Iterator(_)
            | RuntimeTypeShape::Map { .. }
            | RuntimeTypeShape::Need { .. }
            | RuntimeTypeShape::Stream { .. }
            | RuntimeTypeShape::Source { .. }
            | RuntimeTypeShape::ThreadHandle(_)
            | RuntimeTypeShape::Shared(_)
            | RuntimeTypeShape::Reference(_)
            | RuntimeTypeShape::Function { .. } => {
                unreachable!("leaf and unsupported shapes returned before recursive projection")
            }
        })
    }

    fn checked_leaf_type(&self) -> Option<RuntimeCheckedType> {
        match self.shape() {
            RuntimeTypeShape::Never => Some(RuntimeCheckedType::Never),
            RuntimeTypeShape::Unit => Some(RuntimeCheckedType::Unit),
            RuntimeTypeShape::Bool => Some(RuntimeCheckedType::Bool),
            RuntimeTypeShape::Signed(width) => Some(RuntimeCheckedType::Signed(*width)),
            RuntimeTypeShape::Unsigned(width) => Some(RuntimeCheckedType::Unsigned(*width)),
            RuntimeTypeShape::F32 => Some(RuntimeCheckedType::F32),
            RuntimeTypeShape::F64 => Some(RuntimeCheckedType::F64),
            RuntimeTypeShape::String => Some(RuntimeCheckedType::String),
            RuntimeTypeShape::Char => Some(RuntimeCheckedType::Char),
            RuntimeTypeShape::Bytes => Some(RuntimeCheckedType::Bytes),
            RuntimeTypeShape::Duration => Some(RuntimeCheckedType::Duration),
            RuntimeTypeShape::EntityReference => Some(RuntimeCheckedType::EntityReference),
            _ => None,
        }
    }

    fn unsupported(
        &self,
        path: &RuntimeTypeProjectionPath,
        shape: RuntimeUnsupportedTypeShape,
    ) -> RuntimeCheckedTypeProjectionError {
        RuntimeCheckedTypeProjectionError::UnsupportedRuntimeShape {
            semantic_identity: self.identity(),
            path: path.clone(),
            shape,
        }
    }
}

impl RuntimeTypeShape {
    fn operational_type(&self) -> Option<RuntimeOperationalType> {
        match self {
            Self::Range(_) => Some(RuntimeOperationalType::Range),
            Self::Iterator(_) => Some(RuntimeOperationalType::Iterator),
            Self::Sequence { .. } | Self::Array { .. } => Some(RuntimeOperationalType::Sequence),
            Self::Tuple(_) => Some(RuntimeOperationalType::Tuple),
            Self::Choice(_) => Some(RuntimeOperationalType::Choice),
            Self::Result { .. } => Some(RuntimeOperationalType::Result),
            Self::Option(_) => Some(RuntimeOperationalType::Option),
            Self::Map { .. } => Some(RuntimeOperationalType::Map),
            Self::Need { .. } => Some(RuntimeOperationalType::Need),
            Self::Stream { .. } => Some(RuntimeOperationalType::Stream),
            Self::Source { .. } => Some(RuntimeOperationalType::Source),
            Self::ThreadHandle(_) => Some(RuntimeOperationalType::ThreadHandle),
            Self::Shared(_) => Some(RuntimeOperationalType::Shared),
            Self::Reference(_) => Some(RuntimeOperationalType::Reference),
            Self::Function { .. } => Some(RuntimeOperationalType::Function),
            Self::Never
            | Self::Unit
            | Self::Bool
            | Self::Signed(_)
            | Self::Unsigned(_)
            | Self::F32
            | Self::F64
            | Self::String
            | Self::Char
            | Self::Bytes
            | Self::Duration
            | Self::EntityReference
            | Self::ProjectNominal { .. }
            | Self::Opaque { .. } => None,
        }
    }
}

fn projection_index(index: usize) -> u32 {
    u32::try_from(index).expect("normalized type collections fit the u32 contract")
}

fn unsupported_runtime_shape(shape: &RuntimeTypeShape) -> Option<RuntimeUnsupportedTypeShape> {
    match shape {
        RuntimeTypeShape::Range(_) => Some(RuntimeUnsupportedTypeShape::Range),
        RuntimeTypeShape::Iterator(_) => Some(RuntimeUnsupportedTypeShape::Iterator),
        RuntimeTypeShape::Map { .. } => Some(RuntimeUnsupportedTypeShape::Map),
        RuntimeTypeShape::Need { .. } => Some(RuntimeUnsupportedTypeShape::Need),
        RuntimeTypeShape::Stream { .. } => Some(RuntimeUnsupportedTypeShape::Stream),
        RuntimeTypeShape::Source { .. } => Some(RuntimeUnsupportedTypeShape::Source),
        RuntimeTypeShape::ThreadHandle(_) => Some(RuntimeUnsupportedTypeShape::ThreadHandle),
        RuntimeTypeShape::Shared(_) => Some(RuntimeUnsupportedTypeShape::Shared),
        RuntimeTypeShape::Reference(_) => Some(RuntimeUnsupportedTypeShape::Reference),
        RuntimeTypeShape::Function { .. } => Some(RuntimeUnsupportedTypeShape::Function),
        _ => None,
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
    layout: TypeLayoutHash,
}

impl RuntimeResolvedNominal {
    pub const fn new(
        declaration: ProjectNominalDeclarationId,
        owner: ItemId,
        identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Self {
        Self {
            declaration,
            owner,
            identity,
            layout,
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

    pub const fn layout(&self) -> TypeLayoutHash {
        self.layout
    }

    #[must_use]
    /// Projects the checked nominal owner retained by this accepted fact.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted project declaration violates the invariant
    /// that its qualified name is a valid runtime nominal identity.
    pub fn checked_type(&self) -> RuntimeCheckedType {
        RuntimeCheckedType::Nominal {
            nominal: RuntimeNominalTypeId::try_new(self.declaration.qualified_name())
                .expect("an accepted project nominal has a valid runtime identity"),
            semantic_identity: self.identity,
            layout: self.layout,
        }
    }
}

/// One checked nominal-record fact paired with its executable field layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominalRecord {
    nominal: RuntimeResolvedNominal,
    layout: Arc<RuntimeNominalRecordLayout>,
}

/// Failure to pair a nominal fact with an executable record layout.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordFactError {
    #[error("nominal record fact has runtime identity {actual:?}, expected {expected:?}")]
    NominalIdentity {
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },
    #[error("nominal record fact has a different semantic identity")]
    SemanticIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },
    #[error("nominal record fact has a different layout identity")]
    LayoutIdentity {
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
}

impl RuntimeResolvedNominalRecord {
    /// Pairs one accepted nominal fact with its executable record layout.
    ///
    /// # Panics
    ///
    /// Panics only if an accepted project declaration violates the invariant
    /// that its qualified name is a valid runtime nominal identity.
    pub fn try_new(
        nominal: RuntimeResolvedNominal,
        layout: Arc<RuntimeNominalRecordLayout>,
    ) -> Result<Self, RuntimeNominalRecordFactError> {
        let expected = RuntimeNominalTypeId::try_new(nominal.declaration().qualified_name())
            .expect("an accepted project nominal has a valid runtime identity");
        if layout.nominal() != &expected {
            return Err(RuntimeNominalRecordFactError::NominalIdentity {
                expected,
                actual: layout.nominal().clone(),
            });
        }
        if layout.semantic_identity() != nominal.identity() {
            return Err(RuntimeNominalRecordFactError::SemanticIdentity {
                expected: nominal.identity(),
                actual: layout.semantic_identity(),
            });
        }
        if layout.layout() != nominal.layout() {
            return Err(RuntimeNominalRecordFactError::LayoutIdentity {
                expected: nominal.layout(),
                actual: layout.layout(),
            });
        }
        Ok(Self { nominal, layout })
    }

    pub const fn nominal(&self) -> &RuntimeResolvedNominal {
        &self.nominal
    }

    pub const fn layout(&self) -> &Arc<RuntimeNominalRecordLayout> {
        &self.layout
    }

    #[must_use]
    pub fn checked_type(&self) -> RuntimeCheckedType {
        self.layout.checked_type()
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

/// One source-ordered case in a complete normalized runtime variant schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNormalizedVariantCase {
    name: String,
    payload: Option<Box<RuntimeNormalizedType>>,
}

impl RuntimeNormalizedVariantCase {
    #[must_use]
    pub fn new(name: impl Into<String>, payload: Option<RuntimeNormalizedType>) -> Self {
        Self {
            name: name.into(),
            payload: payload.map(Box::new),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn payload(&self) -> Option<&RuntimeNormalizedType> {
        self.payload.as_deref()
    }

    fn checked_case(&self) -> Result<RuntimeCheckedVariantCase, RuntimeCheckedTypeProjectionError> {
        Ok(RuntimeCheckedVariantCase {
            name: self.name.clone(),
            payload: self
                .payload()
                .map(RuntimeNormalizedType::checked_type)
                .transpose()?
                .map(Box::new),
        })
    }
}

#[derive(Clone, Copy)]
struct RuntimeNormalizedVariantCaseRef<'a> {
    name: &'a str,
    payload: Option<&'a RuntimeNormalizedType>,
}

impl<'a> RuntimeNormalizedVariantCaseRef<'a> {
    fn from_case(case: &'a RuntimeNormalizedVariantCase) -> Self {
        Self {
            name: case.name(),
            payload: case.payload(),
        }
    }
}

/// Exact semantic owner of one runtime enum case.
///
/// Project, Character, and base-environment variants retain one complete
/// normalized case table. Checked cases are derived views rather than a
/// parallel payload authority. Option and Result retain their normalized type
/// arguments and expose the same internal source-ordered selection algebra.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "variant owners retain their complete normalized semantic arguments as immutable checker evidence without adding a second indirection contract"
)]
pub enum RuntimeVariantOwner {
    Project {
        nominal: RuntimeResolvedNominal,
        cases: Box<[RuntimeNormalizedVariantCase]>,
    },
    CharacterNominal {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
    },
    BuiltinClosed {
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
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
    fn selected_case(
        &self,
        ordinal: u32,
    ) -> Result<RuntimeNormalizedVariantCaseRef<'_>, RuntimeResolvedVariantError> {
        let ordinal_index = usize::try_from(ordinal).ok();
        let selected = match self {
            Self::Project { cases, .. }
            | Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. } => ordinal_index
                .and_then(|ordinal| cases.get(ordinal))
                .map(RuntimeNormalizedVariantCaseRef::from_case),
            Self::Option { item } => match ordinal {
                0 => Some(RuntimeNormalizedVariantCaseRef {
                    name: "Some",
                    payload: Some(item),
                }),
                1 => Some(RuntimeNormalizedVariantCaseRef {
                    name: "None",
                    payload: None,
                }),
                _ => None,
            },
            Self::Result { ok, error } => match ordinal {
                0 => Some(RuntimeNormalizedVariantCaseRef {
                    name: "Ok",
                    payload: Some(ok),
                }),
                1 => Some(RuntimeNormalizedVariantCaseRef {
                    name: "Err",
                    payload: Some(error),
                }),
                _ => None,
            },
        };
        selected.ok_or(RuntimeResolvedVariantError::CaseOrdinal {
            ordinal,
            case_count: self.case_count(),
        })
    }

    fn case_count(&self) -> u32 {
        match self {
            Self::Project { cases, .. }
            | Self::CharacterNominal { cases, .. }
            | Self::BuiltinClosed { cases, .. } => u32::try_from(cases.len()).unwrap_or(u32::MAX),
            Self::Option { .. } | Self::Result { .. } => 2,
        }
    }

    fn project_checked_type(
        &self,
    ) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError> {
        Ok(match self {
            Self::Project { nominal, cases } => RuntimeCheckedType::Variant {
                nominal: RuntimeNominalTypeId::try_new(nominal.declaration().qualified_name())
                    .map_err(
                        |error| RuntimeCheckedTypeProjectionError::InvalidProjectNominal {
                            semantic_identity: nominal.identity(),
                            path: RuntimeTypeProjectionPath::root(),
                            reason: error.into(),
                        },
                    )?,
                semantic_identity: nominal.identity(),
                cases: cases
                    .iter()
                    .map(RuntimeNormalizedVariantCase::checked_case)
                    .collect::<Result<Vec<_>, _>>()?,
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
                cases: cases
                    .iter()
                    .map(RuntimeNormalizedVariantCase::checked_case)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            Self::Option { item } => RuntimeCheckedType::Option(Box::new(item.checked_type()?)),
            Self::Result { ok, error } => RuntimeCheckedType::Result {
                ok: Box::new(ok.checked_type()?),
                error: Box::new(error.checked_type()?),
            },
        })
    }
}

/// Complete checked variant owner and its canonical selected case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedVariantSelection {
    owner: RuntimeCheckedType,
    ordinal: u32,
    case: RuntimeCheckedVariantCase,
}

impl RuntimeCheckedVariantSelection {
    #[must_use]
    pub const fn owner(&self) -> &RuntimeCheckedType {
        &self.owner
    }

    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    #[must_use]
    pub const fn case(&self) -> &RuntimeCheckedVariantCase {
        &self.case
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.case.name
    }

    #[must_use]
    pub fn payload(&self) -> Option<&RuntimeCheckedType> {
        match &self.case.payload {
            Some(payload) => Some(payload.as_ref()),
            None => None,
        }
    }
}

/// Failure to reconcile one semantically selected case with its complete owner.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeResolvedVariantError {
    #[error("variant owner checked-type projection failed")]
    CheckedTypeProjection(#[from] RuntimeCheckedTypeProjectionError),
    #[error("variant case ordinal {ordinal} is outside {case_count} cases")]
    CaseOrdinal { ordinal: u32, case_count: u32 },
    #[error("variant case {ordinal} resolved as `{actual}`, expected `{expected}`")]
    CaseName {
        ordinal: u32,
        expected: String,
        actual: String,
    },
}

/// Checked enum case selected for a variant expression, constructor call, or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedVariant {
    owner: RuntimeVariantOwner,
    ordinal: u32,
}

impl RuntimeResolvedVariant {
    fn try_new(
        owner: RuntimeVariantOwner,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        let selected = owner.selected_case(ordinal)?;
        if selected.name != selected_name {
            return Err(RuntimeResolvedVariantError::CaseName {
                ordinal,
                expected: selected.name.to_owned(),
                actual: selected_name.to_owned(),
            });
        }
        Ok(Self { owner, ordinal })
    }

    /// Retains a case directly from its accepted project enum declaration.
    pub fn project(
        owner: RuntimeResolvedNominal,
        ordinal: u32,
        selected_name: &str,
        cases: Box<[RuntimeNormalizedVariantCase]>,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        Self::try_new(
            RuntimeVariantOwner::Project {
                nominal: owner,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains a Character nominal case already admitted by checked final HIR.
    pub fn character(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        Self::try_new(
            RuntimeVariantOwner::CharacterNominal {
                identity,
                nominal,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains a case from one source-ordered base-environment enum schema.
    pub fn builtin_closed(
        identity: RuntimeSemanticTypeId,
        nominal: RuntimeNominalTypeId,
        cases: Box<[RuntimeNormalizedVariantCase]>,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        Self::try_new(
            RuntimeVariantOwner::BuiltinClosed {
                identity,
                nominal,
                cases,
            },
            ordinal,
            selected_name,
        )
    }

    /// Retains one accepted Option case after reconciling its closed name.
    pub fn option(
        item: RuntimeNormalizedType,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        Self::try_new(RuntimeVariantOwner::Option { item }, ordinal, selected_name)
    }

    /// Retains one accepted Result case after reconciling its closed name.
    pub fn result(
        ok: RuntimeNormalizedType,
        error: RuntimeNormalizedType,
        ordinal: u32,
        selected_name: &str,
    ) -> Result<Self, RuntimeResolvedVariantError> {
        Self::try_new(
            RuntimeVariantOwner::Result { ok, error },
            ordinal,
            selected_name,
        )
    }

    pub const fn owner(&self) -> &RuntimeVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the selected name borrowed from the complete owner table.
    pub fn selected_name(&self) -> Result<&str, RuntimeResolvedVariantError> {
        self.owner
            .selected_case(self.ordinal)
            .map(|selected| selected.name)
    }

    /// Returns the selected normalized payload borrowed from its sole owner.
    pub fn selected_payload_type(
        &self,
    ) -> Result<Option<&RuntimeNormalizedType>, RuntimeResolvedVariantError> {
        self.owner
            .selected_case(self.ordinal)
            .map(|selected| selected.payload)
    }

    /// Reconciles the selected semantic case with the complete checked owner.
    pub fn checked_selection(
        &self,
    ) -> Result<RuntimeCheckedVariantSelection, RuntimeResolvedVariantError> {
        self.owner.selected_case(self.ordinal)?;
        let owner = self.owner.project_checked_type()?;
        let case =
            owner
                .variant_case(self.ordinal)
                .ok_or(RuntimeResolvedVariantError::CaseOrdinal {
                    ordinal: self.ordinal,
                    case_count: self.owner.case_count(),
                })?;
        Ok(RuntimeCheckedVariantSelection {
            owner,
            ordinal: self.ordinal,
            case,
        })
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
#[derive(Debug)]
pub struct RuntimePlanSemanticFactInput {
    local_declaration_builder: RuntimeLocalDeclarationTableBuilder,
    local_declarations: Vec<(LocalId, RuntimeLocalDeclarationFact)>,
    flows: Vec<(ItemId, FlowRuntimeId)>,
    expression_types: Vec<(ExprId, RuntimeNormalizedType)>,
    pattern_types: Vec<(PatternId, RuntimeNormalizedType)>,
    expression_literals: Vec<(ExprId, RuntimeValue)>,
    pattern_literals: Vec<(PatternId, RuntimeValue)>,
    pattern_items: Vec<(PatternId, RuntimeProjectItem)>,
    values: Vec<(ExprId, RuntimeResolvedValue)>,
    selects: Vec<(ExprId, RuntimeResolvedSelect)>,
    nominal_records: Vec<(ExprId, RuntimeResolvedNominalRecord)>,
    pattern_nominal_records: Vec<(PatternId, RuntimeResolvedNominalRecord)>,
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
        Self {
            local_declaration_builder: RuntimeLocalDeclarationTableBuilder::new(),
            local_declarations: Vec::new(),
            flows: Vec::new(),
            expression_types: Vec::new(),
            pattern_types: Vec::new(),
            expression_literals: Vec::new(),
            pattern_literals: Vec::new(),
            pattern_items: Vec::new(),
            values: Vec::new(),
            selects: Vec::new(),
            nominal_records: Vec::new(),
            pattern_nominal_records: Vec::new(),
            expression_variants: Vec::new(),
            pattern_variants: Vec::new(),
            types: Vec::new(),
            calls: Vec::new(),
            postfix_candidates: Vec::new(),
            trait_methods: Vec::new(),
            iterations: Vec::new(),
            assertions: Vec::new(),
            captures: Vec::new(),
        }
    }

    /// Appends one accepted HIR local and its exact normalized type in
    /// canonical project order, then returns its final plan-local identity.
    pub fn push_local_declaration(
        &mut self,
        owner: LocalId,
        ty: RuntimeNormalizedType,
    ) -> Result<RuntimeLocalDeclarationId, RuntimeLocalDeclarationTableError> {
        let local = self.local_declaration_builder.push()?;
        self.local_declarations
            .push((owner, RuntimeLocalDeclarationFact { local, ty }));
        Ok(local)
    }

    pub fn push_flow(&mut self, owner: ItemId, identity: FlowRuntimeId) {
        self.flows.push((owner, identity));
    }

    /// Stages the accepted normalized type of one live final-HIR expression.
    pub fn push_expression_type(&mut self, owner: ExprId, ty: RuntimeNormalizedType) {
        self.expression_types.push((owner, ty));
    }

    /// Stages the accepted normalized type of one final-HIR pattern.
    pub fn push_pattern_type(&mut self, owner: PatternId, ty: RuntimeNormalizedType) {
        self.pattern_types.push((owner, ty));
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

    pub fn push_nominal_record(&mut self, owner: ExprId, nominal: RuntimeResolvedNominalRecord) {
        self.nominal_records.push((owner, nominal));
    }

    pub fn push_pattern_nominal_record(
        &mut self,
        owner: PatternId,
        nominal: RuntimeResolvedNominalRecord,
    ) {
        self.pattern_nominal_records.push((owner, nominal));
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

impl Default for RuntimePlanSemanticFactInput {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable semantic fact set bound to one exact executable project generation.
#[derive(Clone, Debug)]
pub struct RuntimePlanSemanticFacts {
    snapshots: BTreeMap<HirModuleId, HirSnapshotId>,
    local_declaration_table: RuntimeLocalDeclarationTable,
    local_declarations: BTreeMap<LocalId, RuntimeLocalDeclarationFact>,
    flows: BTreeMap<ItemId, FlowRuntimeId>,
    expression_types: BTreeMap<ExprId, RuntimeNormalizedType>,
    pattern_types: BTreeMap<PatternId, RuntimeNormalizedType>,
    expression_literals: BTreeMap<ExprId, RuntimeValue>,
    pattern_literals: BTreeMap<PatternId, RuntimeValue>,
    pattern_items: BTreeMap<PatternId, RuntimeProjectItem>,
    values: BTreeMap<ExprId, RuntimeResolvedValue>,
    selects: BTreeMap<ExprId, RuntimeResolvedSelect>,
    nominal_records: BTreeMap<ExprId, RuntimeResolvedNominalRecord>,
    pattern_nominal_records: BTreeMap<PatternId, RuntimeResolvedNominalRecord>,
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

/// One atomic accepted local projection. The plan-local identity and semantic
/// type cannot be published or reordered independently.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeLocalDeclarationFact {
    local: RuntimeLocalDeclarationId,
    ty: RuntimeNormalizedType,
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
        let expected_local_declarations = project
            .modules()
            .flat_map(|(_, module)| module.locals().map(|(owner, _)| owner))
            .collect::<Vec<_>>();
        let modules = project
            .modules()
            .map(|(_, module)| (module.module_id(), module.as_ref()))
            .collect::<BTreeMap<_, _>>();
        let snapshots = modules
            .iter()
            .map(|(id, module)| (*id, module.snapshot_id()))
            .collect();

        let expression_types = collect_unique(
            input.expression_types,
            RuntimeSemanticFactFamily::ExpressionType,
        )?;
        for (owner, ty) in &expression_types {
            resolve_expr(&modules, *owner)?;
            validate_normalized_type(&modules, ty)?;
        }

        let pattern_types =
            collect_unique(input.pattern_types, RuntimeSemanticFactFamily::PatternType)?;
        for (owner, ty) in &pattern_types {
            resolve_pattern(&modules, *owner)?;
            validate_normalized_type(&modules, ty)?;
        }

        let local_declaration_table = input.local_declaration_builder.finish();
        let local_declarations = collect_unique(
            input.local_declarations.iter().cloned(),
            RuntimeSemanticFactFamily::LocalDeclaration,
        )?;
        let expected_local_set = expected_local_declarations
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if let Some((local, _)) = input
            .local_declarations
            .iter()
            .find(|(local, _)| !expected_local_set.contains(local))
        {
            return Err(RuntimeSemanticFactsError::ExtraLocalDeclaration { local: *local });
        }
        if let Some(local) = expected_local_declarations
            .iter()
            .find(|local| !local_declarations.contains_key(local))
        {
            return Err(RuntimeSemanticFactsError::MissingLocalDeclaration { local: *local });
        }
        for (position, ((owner, fact), expected_owner)) in input
            .local_declarations
            .iter()
            .zip(&expected_local_declarations)
            .enumerate()
        {
            if owner != expected_owner {
                return Err(
                    RuntimeSemanticFactsError::NonCanonicalLocalDeclarationOrder {
                        expected: *expected_owner,
                        actual: *owner,
                    },
                );
            }
            let expected = u32::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1));
            if expected != Some(fact.local.get().get()) {
                return Err(
                    RuntimeSemanticFactsError::NonCanonicalLocalDeclarationIdentity {
                        owner: *owner,
                        actual: fact.local,
                    },
                );
            }
            validate_normalized_type(&modules, &fact.ty)?;
        }
        if usize::try_from(local_declaration_table.len()).ok() != Some(local_declarations.len()) {
            return Err(RuntimeSemanticFactsError::LocalDeclarationTableMismatch);
        }

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

        let mut nominal_records = collect_unique(
            input.nominal_records,
            RuntimeSemanticFactFamily::NominalRecord,
        )?;
        for (expression, nominal) in &nominal_records {
            require_expr_family(
                &modules,
                *expression,
                RuntimeSemanticFactFamily::NominalRecord,
                |kind| matches!(kind, HirExprKind::Record(_)),
            )?;
            validate_nominal_record(&modules, nominal)?;
        }

        let mut pattern_nominal_records = collect_unique(
            input.pattern_nominal_records,
            RuntimeSemanticFactFamily::PatternNominalRecord,
        )?;
        for (pattern, nominal) in &pattern_nominal_records {
            require_pattern_family(
                &modules,
                *pattern,
                RuntimeSemanticFactFamily::PatternNominalRecord,
                |kind| matches!(kind, HirPatternKind::Record { .. }),
            )?;
            validate_nominal_record(&modules, nominal)?;
        }

        let mut nominal_layouts = BTreeMap::new();
        for record in nominal_records
            .values_mut()
            .chain(pattern_nominal_records.values_mut())
        {
            intern_nominal_record_layout(record, &mut nominal_layouts)?;
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

        validate_complete_expression_types(project, &postfix_candidates, &expression_types)?;
        validate_complete_pattern_types(&modules, &pattern_types)?;

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
            local_declaration_table,
            local_declarations,
            flows,
            expression_types,
            pattern_types,
            expression_literals,
            pattern_literals,
            pattern_items,
            values,
            selects,
            nominal_records,
            pattern_nominal_records,
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

    /// Returns the sole accepted normalized type of one live final-HIR expression.
    pub fn expression_type(&self, expression: ExprId) -> Option<&RuntimeNormalizedType> {
        self.expression_types.get(&expression)
    }

    /// Returns the sole accepted normalized type of one final-HIR pattern.
    pub fn pattern_type(&self, pattern: PatternId) -> Option<&RuntimeNormalizedType> {
        self.pattern_types.get(&pattern)
    }

    /// Final plan-local identity for one accepted final-HIR local.
    pub fn local_declaration(&self, local: LocalId) -> Option<RuntimeLocalDeclarationId> {
        self.local_declarations.get(&local).map(|fact| fact.local)
    }

    /// Sole accepted normalized semantic type of one final-HIR local.
    pub fn local_type(&self, local: LocalId) -> Option<&RuntimeNormalizedType> {
        self.local_declarations.get(&local).map(|fact| &fact.ty)
    }

    /// Complete contiguous local domain shared by patterns, expressions, and
    /// later capture projection.
    pub const fn local_declaration_table(&self) -> &RuntimeLocalDeclarationTable {
        &self.local_declaration_table
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

    pub fn nominal_record(&self, expression: ExprId) -> Option<&RuntimeResolvedNominalRecord> {
        self.nominal_records.get(&expression)
    }

    pub fn pattern_nominal_record(
        &self,
        pattern: PatternId,
    ) -> Option<&RuntimeResolvedNominalRecord> {
        self.pattern_nominal_records.get(&pattern)
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

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSemanticFactsError {
    #[error("runtime semantic facts are bound to a different accepted HIR generation")]
    WrongProjectGeneration,
    #[error("runtime semantic facts contain more than one {family:?} fact for the same HIR ID")]
    DuplicateFact { family: RuntimeSemanticFactFamily },
    #[error("accepted runtime semantic facts omit expression type {expression:?}")]
    MissingExpressionType { expression: ExprId },
    #[error(
        "accepted runtime semantic facts contain a type for inactive expression {expression:?}"
    )]
    InactiveExpressionType { expression: ExprId },
    #[error("accepted runtime semantic facts omit pattern type {pattern:?}")]
    MissingPatternType { pattern: PatternId },
    #[error("postfix expression {expression:?} has no accepted candidate fact")]
    MissingPostfixCandidate { expression: ExprId },
    #[error("runtime semantic fact references unknown HIR module {module:?}")]
    UnknownModule { module: HirModuleId },
    #[error("runtime semantic fact references unresolved item {item:?}")]
    UnresolvedItem { item: ItemId },
    #[error("runtime semantic fact references unresolved local {local:?}")]
    UnresolvedLocal { local: LocalId },
    #[error("accepted runtime semantic facts omit executable local declaration {local:?}")]
    MissingLocalDeclaration { local: LocalId },
    #[error("accepted runtime semantic facts contain extra local declaration {local:?}")]
    ExtraLocalDeclaration { local: LocalId },
    #[error(
        "runtime local declarations are not in canonical project order: expected {expected:?}, observed {actual:?}"
    )]
    NonCanonicalLocalDeclarationOrder { expected: LocalId, actual: LocalId },
    #[error("runtime local declaration {owner:?} has non-canonical identity {actual}")]
    NonCanonicalLocalDeclarationIdentity {
        owner: LocalId,
        actual: RuntimeLocalDeclarationId,
    },
    #[error("runtime local-declaration table does not match its HIR owner projection")]
    LocalDeclarationTableMismatch,
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
    #[error("runtime nominal-record fact item {item:?} is not a struct")]
    WrongNominalRecordItemFamily { item: ItemId },
    #[error("runtime nominal-record fact item {item:?} has {actual} fields, expected {expected}")]
    NominalRecordFieldCount {
        item: ItemId,
        expected: usize,
        actual: usize,
    },
    #[error(
        "runtime nominal-record fact item {item:?} field {ordinal} is `{actual}`, expected `{expected}`"
    )]
    NominalRecordFieldName {
        item: ItemId,
        ordinal: usize,
        expected: String,
        actual: String,
    },
    #[error("runtime nominal-record layout catalog contains conflicting descriptors")]
    ConflictingNominalRecordLayout {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    #[error("runtime nominal-record field {ordinal} (`{name}`) on {item:?} is not representable")]
    UnrepresentableNominalRecordField {
        item: ItemId,
        ordinal: usize,
        name: String,
    },
    #[error("runtime nominal layout for `{nominal:?}` on {item:?} is unresolved")]
    UnresolvedNominalLayout {
        item: ItemId,
        nominal: RuntimeNominalTypeId,
    },
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
    LocalDeclaration,
    FlowIdentity,
    ExpressionType,
    PatternType,
    ExpressionLiteral,
    PatternLiteral,
    PatternItem,
    Value,
    Select,
    NominalRecord,
    PatternNominalRecord,
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

fn validate_complete_expression_types(
    project: HirExecutableProjectView<'_>,
    postfix_candidates: &BTreeMap<ExprId, ExprId>,
    expression_types: &BTreeMap<ExprId, RuntimeNormalizedType>,
) -> Result<(), RuntimeSemanticFactsError> {
    let accepted = project
        .selected_expression_owners(|owner| postfix_candidates.get(&owner).copied())
        .map_err(|error| match error {
            HirSelectedExpressionInventoryError::UnknownModule { module } => {
                RuntimeSemanticFactsError::UnknownModule { module }
            }
            HirSelectedExpressionInventoryError::UnresolvedExpression { expression } => {
                RuntimeSemanticFactsError::UnresolvedExpression { expression }
            }
            HirSelectedExpressionInventoryError::MissingPostfixSelection { expression } => {
                RuntimeSemanticFactsError::MissingPostfixCandidate { expression }
            }
            HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                expression,
                candidate,
            } => RuntimeSemanticFactsError::WrongPostfixCandidate {
                expression,
                candidate,
            },
        })?;

    if let Some(expression) = accepted
        .iter()
        .find(|owner| !expression_types.contains_key(owner))
    {
        return Err(RuntimeSemanticFactsError::MissingExpressionType {
            expression: *expression,
        });
    }
    if let Some(expression) = expression_types
        .keys()
        .find(|owner| !accepted.contains(owner))
    {
        return Err(RuntimeSemanticFactsError::InactiveExpressionType {
            expression: *expression,
        });
    }
    Ok(())
}

/// Final semantic publication owns a fact for every final-HIR pattern,
/// including patterns retained inside bounded candidate HIR. If candidate
/// rollback leaves one without a type, semantic analysis fails before this
/// projection can be constructed.
fn validate_complete_pattern_types(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    pattern_types: &BTreeMap<PatternId, RuntimeNormalizedType>,
) -> Result<(), RuntimeSemanticFactsError> {
    for pattern in modules
        .values()
        .flat_map(|module| module.patterns().map(|(owner, _)| owner))
    {
        if !pattern_types.contains_key(&pattern) {
            return Err(RuntimeSemanticFactsError::MissingPatternType { pattern });
        }
    }
    Ok(())
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
    variant
        .checked_selection()
        .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)?;
    let selected_name = variant
        .selected_name()
        .map_err(|_| RuntimeSemanticFactsError::WrongVariantIdentity)?;
    match variant.owner() {
        RuntimeVariantOwner::Project { nominal, cases } => {
            validate_nominal(modules, nominal)?;
            validate_normalized_variant_payloads(modules, cases)?;
            let HirItemKind::Enum(declaration) = resolve_item(modules, nominal.owner())?.kind()
            else {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            };
            if declaration.variants().len() != cases.len()
                || declaration.variants().iter().zip(cases.iter()).any(
                    |(declaration, normalized)| {
                        declaration.name().resolved().map(HirName::as_str)
                            != Some(normalized.name())
                            || declaration.payload().is_some() != normalized.payload().is_some()
                    },
                )
            {
                return Err(RuntimeSemanticFactsError::WrongVariantIdentity);
            }
            let selected = usize::try_from(variant.ordinal())
                .ok()
                .and_then(|ordinal| declaration.variants().get(ordinal))
                .and_then(|selected| selected.name().resolved())
                .ok_or(RuntimeSemanticFactsError::WrongVariantIdentity)?;
            if selected.as_str() == selected_name {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
        RuntimeVariantOwner::CharacterNominal { nominal, cases, .. } => {
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name().is_empty()
                        || case.payload().is_some()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name() == case.name())
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name() != selected_name)
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::BuiltinClosed { nominal, cases, .. } => {
            validate_normalized_variant_payloads(modules, cases)?;
            if nominal.as_str().is_empty()
                || cases.is_empty()
                || cases.iter().enumerate().any(|(ordinal, case)| {
                    case.name().is_empty()
                        || cases[..ordinal]
                            .iter()
                            .any(|previous| previous.name() == case.name())
                })
                || usize::try_from(variant.ordinal())
                    .ok()
                    .and_then(|ordinal| cases.get(ordinal))
                    .is_none_or(|case| case.name() != selected_name)
            {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            } else {
                Ok(())
            }
        }
        RuntimeVariantOwner::Option { item } => {
            validate_normalized_type(modules, item)?;
            if matches!(
                (variant.ordinal(), selected_name),
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
            if matches!((variant.ordinal(), selected_name), (0, "Ok") | (1, "Err")) {
                Ok(())
            } else {
                Err(RuntimeSemanticFactsError::WrongVariantIdentity)
            }
        }
    }
}

fn validate_normalized_variant_payloads(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    cases: &[RuntimeNormalizedVariantCase],
) -> Result<(), RuntimeSemanticFactsError> {
    for case in cases {
        if let Some(payload) = case.payload() {
            validate_normalized_type(modules, payload)?;
        }
    }
    Ok(())
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
        | RuntimeTypeShape::Opaque { .. } => Ok(()),
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

fn validate_nominal_record(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    record: &RuntimeResolvedNominalRecord,
) -> Result<(), RuntimeSemanticFactsError> {
    let nominal = record.nominal();
    let item = resolve_item(modules, nominal.owner())?;
    let arcweft_lang_hir::symbol::nominal::ProjectNominalDeclarationKind::Struct =
        nominal.declaration().kind()
    else {
        return Err(RuntimeSemanticFactsError::WrongNominalRecordItemFamily {
            item: nominal.owner(),
        });
    };
    let HirItemKind::Struct(declaration) = item.kind() else {
        return Err(RuntimeSemanticFactsError::WrongNominalRecordItemFamily {
            item: nominal.owner(),
        });
    };

    let layout = Arc::clone(record.layout());
    if layout.len() != declaration.fields().len() {
        return Err(RuntimeSemanticFactsError::NominalRecordFieldCount {
            item: nominal.owner(),
            expected: declaration.fields().len(),
            actual: layout.len(),
        });
    }
    for (ordinal, (expected, actual)) in
        declaration.fields().iter().zip(layout.fields()).enumerate()
    {
        let Some(expected) = expected.name().resolved() else {
            return Err(
                RuntimeSemanticFactsError::UnrepresentableNominalRecordField {
                    item: nominal.owner(),
                    ordinal,
                    name: actual.name().to_owned(),
                },
            );
        };
        if expected.as_str() != actual.name() {
            return Err(RuntimeSemanticFactsError::NominalRecordFieldName {
                item: nominal.owner(),
                ordinal,
                expected: expected.as_str().to_owned(),
                actual: actual.name().to_owned(),
            });
        }
    }
    Ok(())
}

fn intern_nominal_record_layout(
    record: &mut RuntimeResolvedNominalRecord,
    layouts: &mut BTreeMap<
        (RuntimeNominalTypeId, RuntimeSemanticTypeId, TypeLayoutHash),
        Arc<RuntimeNominalRecordLayout>,
    >,
) -> Result<(), RuntimeSemanticFactsError> {
    let layout = Arc::clone(record.layout());
    let key = (
        layout.nominal().clone(),
        layout.semantic_identity(),
        layout.layout(),
    );
    match layouts.get(&key) {
        Some(previous) if previous.as_ref() != layout.as_ref() => {
            return Err(RuntimeSemanticFactsError::ConflictingNominalRecordLayout {
                nominal: key.0,
                semantic_identity: key.1,
                layout: key.2,
            });
        }
        Some(previous) => record.layout = Arc::clone(previous),
        None => {
            layouts.insert(key, layout);
        }
    }
    Ok(())
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
