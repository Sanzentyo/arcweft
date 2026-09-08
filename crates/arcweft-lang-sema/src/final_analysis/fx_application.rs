//! Checked Fx definition and application authority.
//!
//! A Content or View owner retains the complete checked application.  The
//! final report owns only the deduplicated definition catalog; it does not
//! introduce an application side table or require a compiler consumer to
//! reconstruct an application from HIR and call facts.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::{HirCallArgumentOrdinal, HirCallInvocation, HirExprKind},
    identity::ExprId,
    module::HirModule,
    symbol::CallableDeclarationKey,
};
use arcweft_presentation::fx::{
    BuiltinFxAbiProjection, BuiltinFxApplicationBindingPlan, BuiltinFxCallableRowId,
    BuiltinFxCallableSchemaDigest, BuiltinFxParameterId, BuiltinFxSpecialization,
    FxDefinitionArgumentValue, FxDefinitionParameterIndex, FxDefinitionParameterLayoutDigest,
    FxDefinitionParameterRef, FxDefinitionParameterSchema, FxDefinitionParameterType,
    FxFontFamilyName, FxId, FxPhase, FxRuntimeType, FxRuntimeValue, FxSamplerProgram, FxSelectorId,
    FxShaderStage, FxSourceConstructor, FxStaticType, FxTarget, MotionFunction,
    ValidatedValueProgram, ValueInstruction, ValueProgramLimits, ValueProgramSchema,
};
use thiserror::Error;

use crate::callable::{
    CallableCandidateId, CallableGroupIndex, CallableParameter, CallableParameterPresence,
    CallableSignatureSchemaDigest, CallableValidator, CheckedCallApplication,
    CheckedCallApplicationDigest, CheckedCallApplicationSite, CheckedCallArgumentSlotSource,
    CheckedCallExecutionSource, CheckedCallOperandDestination,
};
use arcweft_view::ViewParameterCoordinate;

use super::{CheckedExpression, CheckedExpressionResolution};
use crate::checked_rich_text::CheckedDialogueToken;
use crate::semantic_coordinate::{SemanticCoordinateIndex, StableCheckedValueCoordinate};
use crate::types::{CompileTimeFxType, TypeKind};

const CHECKED_FX_APPLICATION_SEMANTIC_DOMAIN: &[u8] =
    b"arcweft.lang.checked-fx-application-semantic.v1\0";
const CHECKED_FX_APPLICATION_CONTENT_CONTEXT_TAG: u8 = 0;
const CHECKED_FX_APPLICATION_VIEW_CONTEXT_TAG: u8 = 1;

/// Stable authored order of one Fx application within its checked Content or
/// View owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFxApplicationOrdinal(u32);

/// Opaque semantic identity of one checked Content or View Fx application.
///
/// This value is issued by the shared Fx application sealer from the complete
/// typed application record.  It intentionally has no Serde implementation:
/// transcript and downstream consumers may borrow the issued bytes, but may
/// not deserialize or mint a parallel application identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFxApplicationSemanticDigest([u8; 32]);

impl CheckedFxApplicationSemanticDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact definition selected by one checked Fx-producing call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxDefinitionRef {
    Builtin {
        row: BuiltinFxCallableRowId,
        specialization: BuiltinFxSpecialization,
        schema: BuiltinFxCallableSchemaDigest,
        definition: FxId,
        layout: FxDefinitionParameterLayoutDigest,
    },
    Project {
        declaration: CallableDeclarationKey,
        definition: FxId,
        schema: CallableSignatureSchemaDigest,
        layout: FxDefinitionParameterLayoutDigest,
        body: CheckedFxBodyDigest,
    },
}

/// Source-schema parameter identity retained before the direct one-to-one
/// definition ABI binding is materialized.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedFxSourceParameter {
    Builtin(BuiltinFxParameterId),
    Project(FxDefinitionParameterIndex),
}

/// Final presence/default decision for one source-schema parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxBindingDecision<S> {
    Explicit(S),
    Defaulted,
    Omitted,
}

/// One ordered checked source-parameter decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFxArgument<S> {
    parameter: CheckedFxSourceParameter,
    decision: CheckedFxBindingDecision<S>,
}

/// Closed semantic value admitted for a Content-side Fx application.
///
/// This is deliberately a Content binding, rather than a generic "closed"
/// value that View may reuse as an independent authority.  View may wrap the
/// same closed value in `CheckedViewFxBinding::Closed`, but the producer
/// definition and ABI admission remain shared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedContentFxBinding {
    Abi(FxDefinitionArgumentValue),
    Phase(FxPhase),
    Target(FxTarget),
    MotionFunction(MotionFunction),
}

/// View-side Fx argument source after final sealing. Closed values are stored
/// directly in the application; reactive values carry a complete self-checked,
/// HIR-free View value program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedViewFxBinding {
    Closed(CheckedContentFxBinding),
    Reactive(CheckedViewValueProgram),
}

/// The shared producer resolver exposes only this closed structural domain to
/// its context seals.  ABI values are carried by the binding itself; these
/// selectors are the only values allowed to participate in a definition
/// specialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckedFxStructuralBinding {
    Phase(FxPhase),
    Target(FxTarget),
    MotionFunction(MotionFunction),
}

/// Binding operations required by the shared producer seal.  Content and
/// View implement this once each; definition identity, default policy, and
/// ABI mapping are consequently owned by one resolver.
pub(crate) trait CheckedFxProducerBinding: Clone + Eq {
    fn context_tag() -> u8
    where
        Self: Sized;
    fn encode_semantic_binding(
        &self,
        encoder: &mut CheckedFxApplicationSemanticEncoder,
    ) -> Result<(), CheckedFxApplicationSemanticDigestError>;
    fn fx_abi_value(&self) -> Option<&FxDefinitionArgumentValue>;
    fn fx_structural(&self) -> Option<CheckedFxStructuralBinding>;
    fn fx_bool(&self) -> Option<bool>;
    fn fx_is_reactive(&self) -> bool;
}

/// One definition-qualified input consumed by a checked reactive View value
/// program. The coordinate is the accepted View parameter ABI, never a local
/// name or HIR arena identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedViewValueInput {
    parameter: ViewParameterCoordinate,
    value_type: FxRuntimeType,
}

/// Self-validated HIR-free value program retained by a View Fx binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewValueProgram {
    inputs: Box<[CheckedViewValueInput]>,
    program: ValidatedValueProgram,
    semantic_digest: [u8; 32],
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CheckedViewValueProgramSealError {
    #[error("reactive View value program binds parameter {parameter:?} more than once")]
    DuplicateInput { parameter: ViewParameterCoordinate },
    #[error(transparent)]
    Program(#[from] arcweft_presentation::fx::ValueProgramValidationError),
    #[error("reactive View value program cannot be encoded in its semantic domain")]
    Encoding,
}

/// Semantic digest of one HIR-free checked project Fx body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedFxBodyDigest([u8; 32]);

/// One HIR-free graph expression in a checked project Fx definition body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxGraphExpression {
    Constructor(CheckedFxConstructorCall),
    Builtin(CheckedFxBodyCall<CheckedSymbolicFxBinding>),
    Project(CheckedFxBodyCall<CheckedSymbolicFxBinding>),
}

/// One checked call to an Arcweft-owned graph constructor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFxConstructorCall {
    constructor: FxSourceConstructor,
    arguments: Box<[CheckedFxConstructorArgument]>,
}

/// One source-schema argument retained by a checked constructor call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFxConstructorArgument {
    parameter: u16,
    value: CheckedFxConstructorArgumentValue,
}

/// Closed value family accepted by a graph-constructor parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxConstructorArgumentValue {
    Value(CheckedFxSymbolicValue),
    Graph(Box<CheckedFxGraphExpression>),
    Graphs(Box<[CheckedFxGraphExpression]>),
}

/// One checked builtin or project Fx call inside a project definition body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFxBodyCall<S> {
    definition: CheckedFxDefinitionRef,
    arguments: Box<[CheckedFxArgument<S>]>,
}

/// A definition-body binding is either symbolic ABI data or a closed
/// structural selector. Structural selectors may never depend on a project
/// parameter because they choose the definition graph itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedSymbolicFxBinding {
    Value(CheckedFxSymbolicValue),
    Phase(FxPhase),
    Target(FxTarget),
    MotionFunction(MotionFunction),
}

/// HIR-free symbolic property/ABI value retained by a project Fx body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxSymbolicValue {
    Parameter(FxDefinitionParameterRef),
    Constant(CheckedFxConstant),
    Program(FxSamplerProgram),
}

/// Closed compile-time value embedded in a checked Fx body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxConstant {
    Abi(FxDefinitionArgumentValue),
    Selector(FxSelectorId),
    ShaderStage(FxShaderStage),
    FontFamily(FxFontFamilyName),
    Target(FxTarget),
    Phase(FxPhase),
}

/// Complete HIR-free body authority for one project Fx declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedFxBody {
    root: CheckedFxGraphExpression,
    digest: CheckedFxBodyDigest,
    expanded_nodes: u32,
    expanded_visits: u32,
    expanded_depth: u16,
}

/// Complete checked application retained by its Content or View owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedFxApplication<S> {
    definition: CheckedFxDefinitionRef,
    call_schema: CallableSignatureSchemaDigest,
    call_application: CheckedCallApplicationDigest,
    site: CheckedCallApplicationSite,
    ordinal: CheckedFxApplicationOrdinal,
    arguments: Box<[CheckedFxArgument<S>]>,
    semantic_digest: CheckedFxApplicationSemanticDigest,
}

/// One generation-bound call identity retained by an Fx payload plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedApplicationRef {
    site: CheckedCallApplicationSite,
    schema: CallableSignatureSchemaDigest,
    application: CheckedCallApplicationDigest,
}

impl SealedApplicationRef {
    fn from_call(application: &CheckedCallApplication) -> Self {
        Self {
            site: application.core().application_site().clone(),
            schema: application
                .core()
                .candidates()
                .selected()
                .schema()
                .semantic_digest(),
            application: application.digest(),
        }
    }

    fn from_fx<S>(application: &CheckedFxApplication<S>) -> Self {
        Self {
            site: application.site.clone(),
            schema: application.call_schema,
            application: application.call_application,
        }
    }

    pub(crate) const fn site(&self) -> &CheckedCallApplicationSite {
        &self.site
    }

    pub(crate) const fn schema(&self) -> CallableSignatureSchemaDigest {
        self.schema
    }

    pub(crate) const fn application(&self) -> CheckedCallApplicationDigest {
        self.application
    }
}

/// Stable source identity for a retained final-expression edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedExpressionEdgeSource {
    raw: ExprId,
    coordinate: StableCheckedValueCoordinate,
}

impl SealedExpressionEdgeSource {
    fn issue(
        raw: ExprId,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<Self, SealedFxEdgePlanError> {
        let coordinate = coordinates
            .expression(raw)
            .map(StableCheckedValueCoordinate::Expression)
            .map_err(|_| SealedFxEdgePlanError::InvalidCallee)?;
        Ok(Self { raw, coordinate })
    }
}

/// Exact outer argument row and inner Fx application join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedFxProducerEdge {
    argument: HirCallArgumentOrdinal,
    source: CheckedCallExecutionSource,
    inner: SealedApplicationRef,
    digest: CheckedFxApplicationSemanticDigest,
}

impl SealedFxProducerEdge {
    pub(crate) const fn source(&self) -> &CheckedCallExecutionSource {
        &self.source
    }

    pub(crate) const fn inner(&self) -> &SealedApplicationRef {
        &self.inner
    }

    pub(crate) const fn digest(&self) -> CheckedFxApplicationSemanticDigest {
        self.digest
    }
}

/// Content-owned outer/inner Fx edge authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedContentFxEdgePlan {
    outer: SealedApplicationRef,
    producer: SealedFxProducerEdge,
}

impl SealedContentFxEdgePlan {
    pub(crate) const fn outer(&self) -> &SealedApplicationRef {
        &self.outer
    }

    pub(crate) const fn producer(&self) -> &SealedFxProducerEdge {
        &self.producer
    }
}

/// Owner-validated outer View application and exact receiver projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewFxExecutionProjection {
    outer: SealedApplicationRef,
    receiver: CheckedCallExecutionSource,
}

impl CheckedViewFxExecutionProjection {
    pub const fn outer_site(&self) -> &CheckedCallApplicationSite {
        self.outer.site()
    }

    pub const fn outer_schema(&self) -> CallableSignatureSchemaDigest {
        self.outer.schema()
    }

    pub const fn outer_application(&self) -> CheckedCallApplicationDigest {
        self.outer.application()
    }

    pub const fn receiver_source(&self) -> &CheckedCallExecutionSource {
        &self.receiver
    }

    pub const fn receiver_expression(&self) -> Option<ExprId> {
        match self.receiver.raw() {
            CheckedCallArgumentSlotSource::Expression(expression) => Some(expression),
            CheckedCallArgumentSlotSource::CompactNumericElement { .. } => None,
        }
    }
}

/// View-owned outer/inner Fx edge authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SealedViewFxEdgePlan {
    execution: CheckedViewFxExecutionProjection,
    callee: SealedExpressionEdgeSource,
    producer: SealedFxProducerEdge,
}

/// Borrowed view of one owner-specific sealed Fx edge plan.
///
/// The plan remains owned by its Content or View application.  This enum is
/// only a temporary typed projection for consumers that need to inspect the
/// common producer edge without cloning or reconstructing a plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SealedFxEdgePlanRef<'a> {
    View(&'a SealedViewFxEdgePlan),
}

impl SealedFxEdgePlanRef<'_> {
    pub(crate) const fn producer(&self) -> &SealedFxProducerEdge {
        match self {
            Self::View(plan) => plan.producer(),
        }
    }
}

impl SealedViewFxEdgePlan {
    pub(crate) const fn execution(&self) -> &CheckedViewFxExecutionProjection {
        &self.execution
    }

    pub(crate) const fn producer(&self) -> &SealedFxProducerEdge {
        &self.producer
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SealedFxEdgePlanError {
    #[error("Fx edge plan outer application is not the owning call")]
    InvalidOuterApplication,
    #[error("Fx edge plan callee or receiver source is invalid")]
    InvalidCallee,
    #[error("Fx edge plan outer argument is not one exact expression slot")]
    InvalidArgument,
    #[error("Fx edge plan inner application is invalid")]
    InvalidInnerApplication,
}

/// One checked definition record retained once in the final semantic report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedFxDefinition {
    Builtin {
        row: BuiltinFxCallableRowId,
        specialization: BuiltinFxSpecialization,
        schema: BuiltinFxCallableSchemaDigest,
        definition: FxId,
        layout: FxDefinitionParameterLayoutDigest,
        binding_plan: BuiltinFxApplicationBindingPlan,
    },
    Project(CheckedProjectFxDefinition),
}

/// One checked project Fx definition retained exactly once in the catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProjectFxDefinition {
    declaration: CallableDeclarationKey,
    definition: FxId,
    schema: CallableSignatureSchemaDigest,
    parameter_schema: FxDefinitionParameterSchema,
    body: CheckedFxBody,
}

/// Deduplicated definition authority for all checked Fx applications in one
/// final semantic generation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CheckedFxDefinitionCatalog {
    definitions: BTreeMap<FxId, CheckedFxDefinition>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CheckedFxDefinitionCatalogError {
    #[error("Fx identity `{definition}` maps to conflicting checked definitions")]
    ConflictingDefinition { definition: FxId },
    #[error("checked Fx body cannot be encoded in its canonical semantic domain")]
    InvalidBodyEncoding,
    #[error("checked Fx application references a missing or mismatched definition `{definition}`")]
    MismatchedReference { definition: FxId },
}

/// Typed reason a project Fx declaration cannot be sealed into the checked
/// symbolic graph domain.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CheckedFxDefinitionSealError {
    #[error("Fx declaration is absent from the accepted project authority")]
    MissingDeclaration,
    #[error("Fx declaration has an invalid callable or parameter schema")]
    InvalidSchema,
    #[error("Fx declaration body is recovered or has no checked root")]
    InvalidBody,
    #[error("Fx expression {owner:?} is outside the checked symbolic graph domain")]
    UnsupportedExpression {
        owner: arcweft_lang_hir::identity::ExprId,
    },
    #[error("Fx call expression {owner:?} has an invalid argument shape")]
    InvalidCallShape {
        owner: arcweft_lang_hir::identity::ExprId,
    },
    #[error("Fx value expression {owner:?} cannot satisfy {expected:?}")]
    InvalidValue {
        owner: arcweft_lang_hir::identity::ExprId,
        expected: FxStaticType,
    },
    #[error("Fx builtin parameter {parameter:?} has a symbolic constraint that cannot be proven")]
    UnprovableBuiltinConstraint { parameter: BuiltinFxParameterId },
    #[error("Fx project dependency cycle reaches `{definition}`")]
    DependencyCycle { definition: FxId },
    #[error("Fx graph expansion depth {actual} exceeds the limit of {limit}")]
    DepthLimit { actual: usize, limit: usize },
    #[error("Fx graph expansion visits {actual} nodes, exceeding the limit of {limit}")]
    VisitLimit { actual: usize, limit: usize },
    #[error("Fx graph expands to {actual} nodes, exceeding the limit of {limit}")]
    NodeLimit { actual: usize, limit: usize },
    #[error("Fx graph accounting overflowed")]
    AccountingOverflow,
    #[error("Fx graph owner validation failed")]
    OwnerInvariant,
    #[error(transparent)]
    Catalog(#[from] CheckedFxDefinitionCatalogError),
}

impl CheckedFxApplicationOrdinal {
    pub(crate) const fn from_checked_order(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl CheckedFxDefinitionRef {
    pub const fn definition(&self) -> &FxId {
        match self {
            Self::Builtin { definition, .. } | Self::Project { definition, .. } => definition,
        }
    }

    pub const fn layout(&self) -> FxDefinitionParameterLayoutDigest {
        match self {
            Self::Builtin { layout, .. } | Self::Project { layout, .. } => *layout,
        }
    }

    pub const fn body_digest(&self) -> Option<CheckedFxBodyDigest> {
        match self {
            Self::Builtin { .. } => None,
            Self::Project { body, .. } => Some(*body),
        }
    }

    pub(crate) fn builtin_definition(&self) -> Option<CheckedFxDefinition> {
        match self {
            Self::Builtin {
                row,
                specialization,
                schema,
                definition,
                layout,
            } => Some(CheckedFxDefinition::Builtin {
                row: *row,
                specialization: *specialization,
                schema: *schema,
                definition: definition.clone(),
                layout: *layout,
                binding_plan: arcweft_presentation::fx::build_builtin_fx_definition(
                    *specialization,
                )
                .ok()?
                .binding_plan()
                .clone(),
            }),
            Self::Project { .. } => None,
        }
    }
}

impl<S> CheckedFxApplication<S> {
    pub(crate) fn new(
        definition: CheckedFxDefinitionRef,
        call_schema: CallableSignatureSchemaDigest,
        call_application: CheckedCallApplicationDigest,
        site: CheckedCallApplicationSite,
        ordinal: CheckedFxApplicationOrdinal,
        arguments: Vec<CheckedFxArgument<S>>,
    ) -> Result<Self, CheckedFxApplicationSemanticDigestError>
    where
        S: CheckedFxProducerBinding,
    {
        let arguments = arguments.into_boxed_slice();
        let semantic_digest = issue_checked_fx_application_semantic_digest(
            &definition,
            call_schema,
            call_application,
            &site,
            ordinal,
            &arguments,
        )?;
        Ok(Self {
            definition,
            call_schema,
            call_application,
            site,
            ordinal,
            arguments,
            semantic_digest,
        })
    }

    pub(crate) const fn definition(&self) -> &CheckedFxDefinitionRef {
        &self.definition
    }

    pub(crate) const fn call_schema(&self) -> CallableSignatureSchemaDigest {
        self.call_schema
    }

    pub(crate) const fn call_application(&self) -> CheckedCallApplicationDigest {
        self.call_application
    }

    pub(crate) const fn site(&self) -> &CheckedCallApplicationSite {
        &self.site
    }

    pub(crate) const fn ordinal(&self) -> CheckedFxApplicationOrdinal {
        self.ordinal
    }

    pub(crate) const fn arguments(&self) -> &[CheckedFxArgument<S>] {
        &self.arguments
    }

    pub(crate) const fn semantic_digest(&self) -> CheckedFxApplicationSemanticDigest {
        self.semantic_digest
    }

    fn producer_edge(
        &self,
        outer: &CheckedCallApplication,
    ) -> Result<SealedFxProducerEdge, SealedFxEdgePlanError> {
        let [argument] = outer.core().execution().arguments() else {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        };
        let zero = HirCallArgumentOrdinal::try_from_usize(0)
            .map_err(|_| SealedFxEdgePlanError::InvalidArgument)?;
        if argument.argument() != zero {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        }
        let [slot] = argument.slots() else {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        };
        let CheckedCallArgumentSlotSource::Expression(inner_owner) = slot.source().raw() else {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        };
        if self.site.raw() != crate::callable::CheckedCallSite::HirCall(inner_owner)
            || self.site.raw().expression() == outer.core().site().expression()
            || slot.source().owner() != inner_owner
        {
            return Err(SealedFxEdgePlanError::InvalidInnerApplication);
        }
        Ok(SealedFxProducerEdge {
            argument: zero,
            source: slot.source().clone(),
            inner: SealedApplicationRef::from_fx(self),
            digest: self.semantic_digest,
        })
    }
}

impl<S> CheckedFxArgument<S> {
    pub(crate) const fn new(
        parameter: CheckedFxSourceParameter,
        decision: CheckedFxBindingDecision<S>,
    ) -> Self {
        Self {
            parameter,
            decision,
        }
    }

    pub const fn parameter(&self) -> CheckedFxSourceParameter {
        self.parameter
    }

    pub const fn decision(&self) -> &CheckedFxBindingDecision<S> {
        &self.decision
    }
}

impl CheckedContentFxBinding {
    pub(crate) const fn abi(value: FxDefinitionArgumentValue) -> Self {
        Self::Abi(value)
    }

    pub const fn abi_value(&self) -> Option<&FxDefinitionArgumentValue> {
        match self {
            Self::Abi(value) => Some(value),
            Self::Phase(_) | Self::Target(_) | Self::MotionFunction(_) => None,
        }
    }
}

impl CheckedFxProducerBinding for CheckedContentFxBinding {
    fn context_tag() -> u8 {
        CHECKED_FX_APPLICATION_CONTENT_CONTEXT_TAG
    }

    fn encode_semantic_binding(
        &self,
        encoder: &mut CheckedFxApplicationSemanticEncoder,
    ) -> Result<(), CheckedFxApplicationSemanticDigestError> {
        match self {
            Self::Abi(value) => {
                encoder.tag(0);
                let bytes = value.canonical_v1_bytes()?;
                encoder.bytes(&bytes)?;
            }
            Self::Phase(value) => {
                encoder.tag(1);
                encoder.u16(value.tag());
            }
            Self::Target(value) => {
                encoder.tag(2);
                encoder.u16(value.tag());
            }
            Self::MotionFunction(value) => {
                encoder.tag(3);
                encoder.u16(value.tag());
            }
        }
        Ok(())
    }

    fn fx_abi_value(&self) -> Option<&FxDefinitionArgumentValue> {
        match self {
            Self::Abi(value) => Some(value),
            Self::Phase(_) | Self::Target(_) | Self::MotionFunction(_) => None,
        }
    }

    fn fx_structural(&self) -> Option<CheckedFxStructuralBinding> {
        match self {
            Self::Abi(_) => None,
            Self::Phase(value) => Some(CheckedFxStructuralBinding::Phase(*value)),
            Self::Target(value) => Some(CheckedFxStructuralBinding::Target(*value)),
            Self::MotionFunction(value) => Some(CheckedFxStructuralBinding::MotionFunction(*value)),
        }
    }

    fn fx_bool(&self) -> Option<bool> {
        match self.fx_abi_value() {
            Some(FxDefinitionArgumentValue::Runtime(FxRuntimeValue::Bool(value))) => Some(*value),
            _ => None,
        }
    }

    fn fx_is_reactive(&self) -> bool {
        false
    }
}

impl CheckedViewFxBinding {
    pub(crate) const fn closed(value: CheckedContentFxBinding) -> Self {
        Self::Closed(value)
    }

    pub(crate) const fn reactive(program: CheckedViewValueProgram) -> Self {
        Self::Reactive(program)
    }
}

impl CheckedFxProducerBinding for CheckedViewFxBinding {
    fn context_tag() -> u8 {
        CHECKED_FX_APPLICATION_VIEW_CONTEXT_TAG
    }

    fn encode_semantic_binding(
        &self,
        encoder: &mut CheckedFxApplicationSemanticEncoder,
    ) -> Result<(), CheckedFxApplicationSemanticDigestError> {
        match self {
            Self::Closed(value) => {
                encoder.tag(0);
                value.encode_semantic_binding(encoder)?;
            }
            Self::Reactive(program) => {
                encoder.tag(1);
                encoder.digest(program.semantic_digest());
            }
        }
        Ok(())
    }

    fn fx_abi_value(&self) -> Option<&FxDefinitionArgumentValue> {
        match self {
            Self::Closed(value) => value.fx_abi_value(),
            Self::Reactive(_) => None,
        }
    }

    fn fx_structural(&self) -> Option<CheckedFxStructuralBinding> {
        match self {
            Self::Closed(value) => value.fx_structural(),
            Self::Reactive(_) => None,
        }
    }

    fn fx_bool(&self) -> Option<bool> {
        match self {
            Self::Closed(value) => value.fx_bool(),
            Self::Reactive(_) => None,
        }
    }

    fn fx_is_reactive(&self) -> bool {
        matches!(self, Self::Reactive(_))
    }
}

/// Content-owned context seal around the shared checked Fx application.
///
/// Consumers cannot construct or name the raw generic application; they only
/// receive the binding domain admitted for attached Content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedContentFxApplication(CheckedFxApplication<CheckedContentFxBinding>);

impl CheckedContentFxApplication {
    pub(crate) const fn from_inner(inner: CheckedFxApplication<CheckedContentFxBinding>) -> Self {
        Self(inner)
    }

    pub const fn definition(&self) -> &CheckedFxDefinitionRef {
        self.0.definition()
    }

    pub const fn call_schema(&self) -> CallableSignatureSchemaDigest {
        self.0.call_schema()
    }

    pub const fn call_application(&self) -> CheckedCallApplicationDigest {
        self.0.call_application()
    }

    pub const fn site(&self) -> &CheckedCallApplicationSite {
        self.0.site()
    }

    pub const fn ordinal(&self) -> CheckedFxApplicationOrdinal {
        self.0.ordinal()
    }

    pub const fn arguments(&self) -> &[CheckedFxArgument<CheckedContentFxBinding>] {
        self.0.arguments()
    }

    pub const fn semantic_digest(&self) -> CheckedFxApplicationSemanticDigest {
        self.0.semantic_digest()
    }

    pub(crate) fn seal_content_fx(
        &self,
        outer: &CheckedCallApplication,
        hir_application: &arcweft_lang_hir::dialogue_application::HirAttachedContentApplication,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<SealedContentFxEdgePlan, SealedFxEdgePlanError> {
        let owner = outer.core().application_site().raw().expression();
        let expected_site = crate::callable::CheckedCallSite::AttachedContentApplication {
            expression: owner,
            family: crate::callable::CheckedAttachedContentApplicationFamily::ContentCall,
        };
        if outer.core().site() != expected_site
            || outer.core().application_site().raw() != expected_site
            || outer.core().application_site().coordinate()
                != &StableCheckedValueCoordinate::Expression(
                    coordinates
                        .expression(owner)
                        .map_err(|_| SealedFxEdgePlanError::InvalidOuterApplication)?,
                )
            || !matches!(
                outer.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Direct
            )
            || !matches!(
                outer.core().execution().receiver(),
                crate::callable::CheckedCallReceiverProjection::None
            )
            || !matches!(
                outer.result().content_emission(),
                Some(crate::callable::ContentCallableIdentity::Language {
                    definition:
                        arcweft_presentation::rich_text::PresentationContentCallableDefinitionId::Fx,
                    ..
                })
            )
        {
            return Err(SealedFxEdgePlanError::InvalidOuterApplication);
        }
        let arcweft_lang_hir::dialogue_application::HirAttachedContentApplicationFamily::ContentCall {
            invocation,
            ..
        } = hir_application.family()
        else {
            return Err(SealedFxEdgePlanError::InvalidOuterApplication);
        };
        if invocation.form() != arcweft_lang_hir::expr::HirCallInvocationForm::Parenthesized {
            return Err(SealedFxEdgePlanError::InvalidOuterApplication);
        }
        let producer = self.0.producer_edge(outer)?;
        let [argument] = invocation.arguments() else {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        };
        if argument.value() != producer.source().owner() {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        }
        Ok(SealedContentFxEdgePlan {
            outer: SealedApplicationRef::from_call(outer),
            producer,
        })
    }
}

/// View-owned context seal around the shared checked Fx application.
///
/// The only additional binding admitted here is a self-validated reactive
/// value program. Definition identity, producer schema, defaults, and ABI
/// mapping still come from the shared producer authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedViewFxApplication {
    inner: CheckedFxApplication<CheckedViewFxBinding>,
    plan: SealedViewFxEdgePlan,
}

impl CheckedViewFxApplication {
    pub(crate) fn seal_view_fx(
        inner: CheckedFxApplication<CheckedViewFxBinding>,
        outer: &CheckedCallApplication,
        module: &HirModule,
        call: &HirCallInvocation,
        coordinates: &SemanticCoordinateIndex<'_, '_>,
    ) -> Result<Self, SealedFxEdgePlanError> {
        let owner = outer.core().application_site().raw().expression();
        let expected_site = crate::callable::CheckedCallSite::HirCall(owner);
        if outer.core().site() != expected_site
            || outer.core().application_site().raw() != expected_site
            || outer.core().application_site().coordinate()
                != &StableCheckedValueCoordinate::Expression(
                    coordinates
                        .expression(owner)
                        .map_err(|_| SealedFxEdgePlanError::InvalidOuterApplication)?,
                )
            || !matches!(
                outer.core().candidates().selected().schema().validator(),
                CallableValidator::ViewModifier(crate::callable::ViewModifierId::Fx)
            )
            || outer.result().value_type() != Some(&TypeKind::ViewValue)
            || !matches!(
                outer.core().callee(),
                crate::callable::CheckedCallCalleeExecution::Direct
            )
        {
            return Err(SealedFxEdgePlanError::InvalidOuterApplication);
        }
        let receiver = match outer.core().execution().receiver() {
            crate::callable::CheckedCallReceiverProjection::Operand { source, .. } => source,
            crate::callable::CheckedCallReceiverProjection::None
            | crate::callable::CheckedCallReceiverProjection::SemanticOnly { .. } => {
                return Err(SealedFxEdgePlanError::InvalidCallee);
            }
        };
        let CheckedCallArgumentSlotSource::Expression(receiver_expression) = receiver.raw() else {
            return Err(SealedFxEdgePlanError::InvalidCallee);
        };
        let callee = call
            .callee()
            .value_expression()
            .ok_or(SealedFxEdgePlanError::InvalidCallee)?;
        let HirExprKind::Select(select) = module
            .resolve_expr(callee)
            .map_err(|_| SealedFxEdgePlanError::InvalidCallee)?
            .kind()
        else {
            return Err(SealedFxEdgePlanError::InvalidCallee);
        };
        if select.target() != receiver_expression {
            return Err(SealedFxEdgePlanError::InvalidCallee);
        }
        let callee = SealedExpressionEdgeSource::issue(callee, coordinates)?;
        let producer = inner.producer_edge(outer)?;
        let [argument] = call.arguments() else {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        };
        if argument.value() != producer.source().owner() {
            return Err(SealedFxEdgePlanError::InvalidArgument);
        }
        let execution = CheckedViewFxExecutionProjection {
            outer: SealedApplicationRef::from_call(outer),
            receiver: receiver.clone(),
        };
        let plan = SealedViewFxEdgePlan {
            execution,
            callee,
            producer,
        };
        Ok(Self { inner, plan })
    }

    pub const fn definition(&self) -> &CheckedFxDefinitionRef {
        self.inner.definition()
    }

    pub const fn call_schema(&self) -> CallableSignatureSchemaDigest {
        self.inner.call_schema()
    }

    pub const fn call_application(&self) -> CheckedCallApplicationDigest {
        self.inner.call_application()
    }

    pub const fn site(&self) -> &CheckedCallApplicationSite {
        self.inner.site()
    }

    pub const fn ordinal(&self) -> CheckedFxApplicationOrdinal {
        self.inner.ordinal()
    }

    pub const fn arguments(&self) -> &[CheckedFxArgument<CheckedViewFxBinding>] {
        self.inner.arguments()
    }

    pub const fn semantic_digest(&self) -> CheckedFxApplicationSemanticDigest {
        self.inner.semantic_digest()
    }

    pub const fn execution(&self) -> &CheckedViewFxExecutionProjection {
        self.plan.execution()
    }

    pub(crate) const fn plan_ref(&self) -> SealedFxEdgePlanRef<'_> {
        SealedFxEdgePlanRef::View(&self.plan)
    }

    pub const fn outer_application(&self) -> CheckedCallApplicationDigest {
        self.plan.execution().outer_application()
    }

    pub const fn receiver_source(&self) -> &CheckedCallExecutionSource {
        self.plan.execution().receiver_source()
    }

    pub const fn receiver_expression(&self) -> Option<ExprId> {
        self.plan.execution().receiver_expression()
    }
}

/// Shared failure boundary for the ordinary Fx producer seal.  Context
/// adapters intentionally map this to their owning expression diagnostic;
/// they do not get separate resolver or ABI fallback paths.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CheckedFxApplicationSealError {
    #[error("checked Fx producer application has an invalid call shape")]
    InvalidShape,
    #[error("checked Fx producer application has an invalid binding")]
    InvalidBinding,
    #[error("checked Fx producer application has an invalid definition")]
    InvalidDefinition,
    #[error(transparent)]
    SemanticDigest(#[from] CheckedFxApplicationSemanticDigestError),
    #[error(transparent)]
    Catalog(#[from] CheckedFxDefinitionCatalogError),
}

/// Structured failure while issuing the one checked Fx application semantic
/// digest.  Every fallible canonical boundary is retained as an error so a
/// failed digest can abort the surrounding seal transaction; no zero or
/// provisional digest is ever published.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(crate) enum CheckedFxApplicationSemanticDigestError {
    #[error("checked Fx application semantic digest length does not fit u64")]
    LengthOverflow,
    #[error(transparent)]
    Coordinate(#[from] crate::semantic_coordinate::SemanticCoordinateEncodingError),
    #[error(transparent)]
    Builtin(#[from] arcweft_presentation::fx::BuiltinFxBuildError),
    #[error(transparent)]
    Binding(#[from] arcweft_presentation::fx::FxDefinitionError),
}

/// Canonical encoder owned by this module for the application semantic
/// domain.  It writes length-delimited typed fields directly into the hash
/// stream, so no consumer can look up or re-encode an application later.
pub(crate) struct CheckedFxApplicationSemanticEncoder {
    hasher: blake3::Hasher,
}

impl CheckedFxApplicationSemanticEncoder {
    fn new() -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHECKED_FX_APPLICATION_SEMANTIC_DOMAIN);
        Self { hasher }
    }

    fn tag(&mut self, value: u8) {
        self.hasher.update(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.hasher.update(&value.to_le_bytes());
    }

    fn count(&mut self, value: usize) -> Result<(), CheckedFxApplicationSemanticDigestError> {
        let value = u64::try_from(value)
            .map_err(|_| CheckedFxApplicationSemanticDigestError::LengthOverflow)?;
        self.hasher.update(&value.to_le_bytes());
        Ok(())
    }

    fn digest(&mut self, value: &[u8; 32]) {
        self.hasher.update(value);
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), CheckedFxApplicationSemanticDigestError> {
        self.count(value.len())?;
        self.hasher.update(value);
        Ok(())
    }

    fn finish(self) -> CheckedFxApplicationSemanticDigest {
        CheckedFxApplicationSemanticDigest(*self.hasher.finalize().as_bytes())
    }
}

fn issue_checked_fx_application_semantic_digest<S: CheckedFxProducerBinding>(
    definition: &CheckedFxDefinitionRef,
    call_schema: CallableSignatureSchemaDigest,
    call_application: CheckedCallApplicationDigest,
    site: &CheckedCallApplicationSite,
    ordinal: CheckedFxApplicationOrdinal,
    arguments: &[CheckedFxArgument<S>],
) -> Result<CheckedFxApplicationSemanticDigest, CheckedFxApplicationSemanticDigestError> {
    let mut encoder = CheckedFxApplicationSemanticEncoder::new();
    encoder.tag(S::context_tag());
    encode_fx_definition_ref(&mut encoder, definition)?;
    encoder.digest(call_schema.as_bytes());
    encoder.digest(call_application.as_bytes());
    encoder.bytes(&site.coordinate().canonical_bytes()?)?;
    encoder.u32(ordinal.get());
    encoder.count(arguments.len())?;
    for argument in arguments {
        match argument.parameter() {
            CheckedFxSourceParameter::Builtin(parameter) => {
                encoder.tag(0);
                encoder.tag(parameter.semantic_tag());
            }
            CheckedFxSourceParameter::Project(parameter) => {
                encoder.tag(1);
                encoder.u16(parameter.get());
            }
        }
        match argument.decision() {
            CheckedFxBindingDecision::Explicit(binding) => {
                encoder.tag(0);
                binding.encode_semantic_binding(&mut encoder)?;
            }
            CheckedFxBindingDecision::Defaulted => encoder.tag(1),
            CheckedFxBindingDecision::Omitted => encoder.tag(2),
        }
    }
    Ok(encoder.finish())
}

fn encode_fx_definition_ref(
    encoder: &mut CheckedFxApplicationSemanticEncoder,
    definition: &CheckedFxDefinitionRef,
) -> Result<(), CheckedFxApplicationSemanticDigestError> {
    match definition {
        CheckedFxDefinitionRef::Builtin {
            row,
            specialization,
            schema,
            definition,
            layout,
        } => {
            encoder.tag(0);
            encoder.tag(row.semantic_tag());
            encoder.digest(&specialization.semantic_digest_v1()?);
            encoder.digest(schema.as_bytes());
            encoder.digest(definition.semantic_digest().as_bytes());
            encoder.digest(layout.as_bytes());
        }
        CheckedFxDefinitionRef::Project {
            declaration,
            definition,
            schema,
            layout,
            body,
        } => {
            encoder.tag(1);
            encoder.digest(declaration.semantic_digest().as_bytes());
            encoder.digest(definition.semantic_digest().as_bytes());
            encoder.digest(schema.as_bytes());
            encoder.digest(layout.as_bytes());
            encoder.digest(body.as_bytes());
        }
    }
    Ok(())
}

/// Source-schema request handed to the one context binding callback.  Keeping
/// this as one callback is important: Content and View cannot accidentally
/// carry two independently borrowed producer resolvers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckedFxBindingRequest {
    Builtin(arcweft_presentation::fx::BuiltinFxCallableParameter),
    Project(FxDefinitionParameterType),
}

/// Materializes one ordinary Fx producer application for either Content or
/// View.  Definition identity, source-schema defaulting, structural
/// specialization, and ABI parameter ordering are all sealed here.  The two
/// context callbacks only decide whether an admitted source value becomes a
/// closed Content binding or a View reactive binding.
pub(crate) fn seal_shared_fx_application<S, Bind>(
    expression: ExprId,
    application: &CheckedCallApplication,
    actual: &CompileTimeFxType,
    ordinal: CheckedFxApplicationOrdinal,
    definitions: &mut CheckedFxDefinitionCatalog,
    mut bind: Bind,
) -> Result<CheckedFxApplication<S>, CheckedFxApplicationSealError>
where
    S: CheckedFxProducerBinding,
    Bind: FnMut(CheckedFxBindingRequest, &CallableParameter, ExprId) -> Result<S, ()>,
{
    let mut staged_definitions = definitions.clone();
    let selected = application.core().candidates().selected();
    let call_schema = selected.schema().semantic_digest();
    let (definition, arguments) = match actual {
        CompileTimeFxType::Builtin(row) => seal_builtin_fx_producer(
            expression,
            application,
            *row,
            ordinal,
            &mut staged_definitions,
            &mut bind,
        )?,
        CompileTimeFxType::Registered(definition) => seal_project_fx_producer(
            expression,
            application,
            definition,
            &staged_definitions,
            &mut bind,
        )?,
        CompileTimeFxType::Abstract | CompileTimeFxType::Constructor(_) => {
            return Err(CheckedFxApplicationSealError::InvalidDefinition);
        }
    };
    let result = CheckedFxApplication::new(
        definition,
        call_schema,
        application.digest(),
        application.core().application_site().clone(),
        ordinal,
        arguments,
    )
    .map_err(CheckedFxApplicationSealError::from);
    if result.is_ok() {
        *definitions = staged_definitions;
    }
    result
}

fn seal_builtin_fx_producer<S>(
    expression: ExprId,
    application: &CheckedCallApplication,
    row_id: arcweft_presentation::fx::BuiltinFxCallableRowId,
    ordinal: CheckedFxApplicationOrdinal,
    definitions: &mut CheckedFxDefinitionCatalog,
    bind: &mut impl FnMut(CheckedFxBindingRequest, &CallableParameter, ExprId) -> Result<S, ()>,
) -> Result<(CheckedFxDefinitionRef, Vec<CheckedFxArgument<S>>), CheckedFxApplicationSealError>
where
    S: CheckedFxProducerBinding,
{
    use arcweft_presentation::fx::{
        BUILTIN_FX_CALLABLE_CATALOG, BuiltinFxActiveAbiParameterSet, BuiltinFxArgument,
        BuiltinFxParameterBinding, BuiltinFxParameterId, BuiltinFxParameterPresence,
        BuiltinFxSpecialization, build_builtin_fx_definition,
    };

    if application
        .core()
        .candidates()
        .selected()
        .schema()
        .validator()
        != &CallableValidator::BuiltinFx(row_id)
    {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }
    let row = BUILTIN_FX_CALLABLE_CATALOG
        .get(row_id)
        .ok_or(CheckedFxApplicationSealError::InvalidDefinition)?;
    let group = application
        .core()
        .candidates()
        .selected()
        .schema()
        .group(CallableGroupIndex::ZERO)
        .ok_or(CheckedFxApplicationSealError::InvalidShape)?;
    if row.parameters().len() != group.parameters().len() {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }
    let explicit = collect_fx_expression_arguments(expression, application, group)?;
    let mut explicit_bindings = BTreeMap::new();
    for (index, source) in row.parameters().iter().copied().enumerate() {
        let selected = group
            .parameters()
            .get(index)
            .ok_or(CheckedFxApplicationSealError::InvalidShape)?;
        if selected
            .name()
            .is_none_or(|name| name.as_str() != source.source_name())
            || selected.index().get() != index
        {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        }
        if let Some(source_expression) = explicit.get(&index).copied() {
            let binding = bind(
                CheckedFxBindingRequest::Builtin(source),
                selected,
                source_expression,
            )
            .map_err(|_| CheckedFxApplicationSealError::InvalidBinding)?;
            if explicit_bindings.insert(index, binding).is_some() {
                return Err(CheckedFxApplicationSealError::InvalidShape);
            }
        }
    }
    if explicit
        .keys()
        .any(|index| *index >= row.parameters().len())
    {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }

    let mut decisions = Vec::with_capacity(row.parameters().len());
    let mut active_abi = Vec::new();
    let mut target = None;
    let mut motion_function = None;
    for (index, source) in row.parameters().iter().copied().enumerate() {
        let predicate_holds = match source.presence() {
            BuiltinFxParameterPresence::Conditional { predicate, .. } => Some(
                builtin_fx_predicate_holds(row, predicate, &explicit_bindings, &decisions)
                    .ok_or(CheckedFxApplicationSealError::InvalidShape)?,
            ),
            _ => None,
        };
        let decision = match (source.presence(), explicit_bindings.get(&index)) {
            (_, Some(_)) if predicate_holds == Some(false) => {
                return Err(CheckedFxApplicationSealError::InvalidShape);
            }
            (_, Some(binding)) => CheckedFxBindingDecision::Explicit(binding.clone()),
            (BuiltinFxParameterPresence::Required, None) => {
                return Err(CheckedFxApplicationSealError::InvalidShape);
            }
            (BuiltinFxParameterPresence::Defaulted(_), None) => CheckedFxBindingDecision::Defaulted,
            (BuiltinFxParameterPresence::Optional, None) => CheckedFxBindingDecision::Omitted,
            (BuiltinFxParameterPresence::Conditional { default, .. }, None) => {
                match predicate_holds {
                    Some(false) => CheckedFxBindingDecision::Omitted,
                    Some(true) if default.is_some() => CheckedFxBindingDecision::Defaulted,
                    Some(true) | None => {
                        return Err(CheckedFxApplicationSealError::InvalidShape);
                    }
                }
            }
        };
        let structural = builtin_fx_structural_value(&decision, source.presence());
        match source.id() {
            BuiltinFxParameterId::Phase => {
                if structural != Some(CheckedFxStructuralBinding::Phase(row_id.phase())) {
                    return Err(CheckedFxApplicationSealError::InvalidShape);
                }
            }
            BuiltinFxParameterId::Target => {
                target = structural.and_then(|value| match value {
                    CheckedFxStructuralBinding::Target(value) => Some(value),
                    _ => None,
                });
            }
            BuiltinFxParameterId::MotionFunction => {
                motion_function = structural.and_then(|value| match value {
                    CheckedFxStructuralBinding::MotionFunction(value) => Some(value),
                    _ => None,
                });
            }
            _ => {}
        }
        if source.binding() == BuiltinFxParameterBinding::Abi
            && match source.presence() {
                BuiltinFxParameterPresence::Required | BuiltinFxParameterPresence::Defaulted(_) => {
                    true
                }
                BuiltinFxParameterPresence::Optional => {
                    matches!(decision, CheckedFxBindingDecision::Explicit(_))
                }
                BuiltinFxParameterPresence::Conditional { .. } => predicate_holds == Some(true),
            }
        {
            active_abi.push(source.id());
        }
        decisions.push(CheckedFxArgument::new(
            CheckedFxSourceParameter::Builtin(source.id()),
            decision,
        ));
    }

    let specialization = BuiltinFxSpecialization::try_new(
        row_id,
        target.ok_or(CheckedFxApplicationSealError::InvalidShape)?,
        motion_function,
        BuiltinFxActiveAbiParameterSet::from_parameters(active_abi),
    )
    .map_err(|_| CheckedFxApplicationSealError::InvalidDefinition)?;
    let template = build_builtin_fx_definition(specialization)
        .map_err(|_| CheckedFxApplicationSealError::InvalidDefinition)?;
    if !decisions.iter().any(|argument| {
        matches!(
            argument.decision(),
            CheckedFxBindingDecision::Explicit(binding) if binding.fx_is_reactive()
        )
    }) {
        let arguments = decisions
            .iter()
            .filter_map(|argument| {
                let CheckedFxSourceParameter::Builtin(parameter) = argument.parameter() else {
                    return Some(Err(CheckedFxApplicationSealError::InvalidShape));
                };
                let CheckedFxBindingDecision::Explicit(binding) = argument.decision() else {
                    return None;
                };
                let source = row.parameter(parameter)?;
                (source.binding() == BuiltinFxParameterBinding::Abi
                    && specialization.active_abi_parameters().contains(parameter))
                .then(|| {
                    binding
                        .fx_abi_value()
                        .cloned()
                        .map(|value| BuiltinFxArgument::new(parameter, value))
                        .ok_or(CheckedFxApplicationSealError::InvalidBinding)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        template
            .bind_application(arguments, ordinal.get(), None)
            .map_err(|_| CheckedFxApplicationSealError::InvalidDefinition)?;
    }
    let definition = CheckedFxDefinition::Builtin {
        row: row_id,
        specialization,
        schema: row.schema_digest(),
        definition: template.definition().id().clone(),
        layout: template.definition().parameter_layout().digest(),
        binding_plan: template.binding_plan().clone(),
    };
    let reference = definition.reference();
    definitions.insert(definition)?;
    Ok((reference, decisions))
}

fn seal_project_fx_producer<S>(
    expression: ExprId,
    application: &CheckedCallApplication,
    definition: &FxId,
    definitions: &CheckedFxDefinitionCatalog,
    bind: &mut impl FnMut(CheckedFxBindingRequest, &CallableParameter, ExprId) -> Result<S, ()>,
) -> Result<(CheckedFxDefinitionRef, Vec<CheckedFxArgument<S>>), CheckedFxApplicationSealError>
where
    S: CheckedFxProducerBinding,
{
    let CallableCandidateId::Project(declaration) = application.core().candidates().selected().id()
    else {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    };
    let selected_schema = application.core().candidates().selected().schema();
    let checked_definition = definitions
        .get(definition)
        .and_then(CheckedFxDefinition::project)
        .filter(|checked| {
            checked.declaration() == declaration
                && checked.schema() == selected_schema.semantic_digest()
        })
        .ok_or(CheckedFxApplicationSealError::InvalidDefinition)?;
    let [group] = selected_schema.groups() else {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    };
    let parameter_schema = checked_definition.parameter_schema();
    if parameter_schema.parameters().len() != group.parameters().len() {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }
    let explicit = collect_fx_expression_arguments(expression, application, group)?;
    if explicit
        .keys()
        .any(|index| *index >= group.parameters().len())
    {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }
    let mut arguments = Vec::with_capacity(group.parameters().len());
    for (index, (parameter, definition_parameter)) in group
        .parameters()
        .iter()
        .zip(parameter_schema.parameters())
        .enumerate()
    {
        if parameter.passing() != crate::callable::CallableParameterPassing::NamedOnly
            || parameter.index().get() != index
        {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        }
        let decision = match (parameter.presence(), explicit.get(&index).copied()) {
            (_, Some(source)) => CheckedFxBindingDecision::Explicit(
                bind(
                    CheckedFxBindingRequest::Project(definition_parameter.parameter_type()),
                    parameter,
                    source,
                )
                .map_err(|_| CheckedFxApplicationSealError::InvalidBinding)?,
            ),
            (CallableParameterPresence::Defaulted, None) => CheckedFxBindingDecision::Defaulted,
            (CallableParameterPresence::Optional, None)
            | (CallableParameterPresence::Required, None) => {
                return Err(CheckedFxApplicationSealError::InvalidShape);
            }
        };
        let parameter = FxDefinitionParameterIndex::from_index(index)
            .ok_or(CheckedFxApplicationSealError::InvalidShape)?;
        arguments.push(CheckedFxArgument::new(
            CheckedFxSourceParameter::Project(parameter),
            decision,
        ));
    }
    if explicit.len() != arguments.len()
        && explicit
            .keys()
            .any(|index| group.parameters().get(*index).is_none())
    {
        return Err(CheckedFxApplicationSealError::InvalidShape);
    }
    Ok((checked_definition.reference(), arguments))
}

fn collect_fx_expression_arguments(
    owner: ExprId,
    application: &CheckedCallApplication,
    group: &crate::callable::CallableParameterGroup,
) -> Result<BTreeMap<usize, ExprId>, CheckedFxApplicationSealError> {
    let mut values = BTreeMap::new();
    for slot in application
        .core()
        .execution()
        .arguments()
        .iter()
        .flat_map(|argument| argument.slots())
    {
        let CheckedCallOperandDestination::Parameter(coordinate) = slot.destination() else {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        };
        if coordinate.group() != group.index() {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        }
        let CheckedCallArgumentSlotSource::Expression(source) = slot.source().raw() else {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        };
        if values
            .insert(coordinate.parameter().get(), source)
            .is_some()
        {
            return Err(CheckedFxApplicationSealError::InvalidShape);
        }
    }
    let _ = owner;
    Ok(values)
}

pub(crate) fn builtin_fx_presence_default(
    presence: arcweft_presentation::fx::BuiltinFxParameterPresence,
) -> Option<arcweft_presentation::fx::BuiltinFxDefaultValue> {
    use arcweft_presentation::fx::BuiltinFxParameterPresence;
    match presence {
        BuiltinFxParameterPresence::Defaulted(value)
        | BuiltinFxParameterPresence::Conditional {
            default: Some(value),
            ..
        } => Some(value),
        BuiltinFxParameterPresence::Required
        | BuiltinFxParameterPresence::Optional
        | BuiltinFxParameterPresence::Conditional { default: None, .. } => None,
    }
}

fn builtin_fx_predicate_holds<S: CheckedFxProducerBinding>(
    row: arcweft_presentation::fx::BuiltinFxCallableRow,
    predicate: arcweft_presentation::fx::BuiltinFxParameterPredicate,
    explicit: &BTreeMap<usize, S>,
    decisions: &[CheckedFxArgument<S>],
) -> Option<bool> {
    use arcweft_presentation::fx::{BuiltinFxDefaultValue, BuiltinFxParameterPredicate};
    let BuiltinFxParameterPredicate::BoolEquals {
        parameter,
        value: expected,
    } = predicate;
    let index = row
        .parameters()
        .iter()
        .position(|candidate| candidate.id() == parameter)?;
    let actual = match decisions.get(index).map(CheckedFxArgument::decision) {
        Some(CheckedFxBindingDecision::Explicit(value)) => value.fx_bool(),
        Some(CheckedFxBindingDecision::Defaulted) => {
            match builtin_fx_presence_default(row.parameters()[index].presence())? {
                BuiltinFxDefaultValue::Bool(value) => Some(value),
                _ => None,
            }
        }
        Some(CheckedFxBindingDecision::Omitted) => None,
        None => explicit
            .get(&index)
            .and_then(CheckedFxProducerBinding::fx_bool)
            .or_else(
                || match builtin_fx_presence_default(row.parameters()[index].presence())? {
                    BuiltinFxDefaultValue::Bool(value) => Some(value),
                    _ => None,
                },
            ),
    }?;
    Some(actual == expected)
}

fn builtin_fx_structural_value<S: CheckedFxProducerBinding>(
    decision: &CheckedFxBindingDecision<S>,
    presence: arcweft_presentation::fx::BuiltinFxParameterPresence,
) -> Option<CheckedFxStructuralBinding> {
    match decision {
        CheckedFxBindingDecision::Explicit(binding) => binding.fx_structural(),
        CheckedFxBindingDecision::Defaulted => {
            builtin_fx_presence_default(presence).and_then(|value| {
                Some(match value {
                    arcweft_presentation::fx::BuiltinFxDefaultValue::Phase(value) => {
                        CheckedFxStructuralBinding::Phase(value)
                    }
                    arcweft_presentation::fx::BuiltinFxDefaultValue::Target(value) => {
                        CheckedFxStructuralBinding::Target(value)
                    }
                    arcweft_presentation::fx::BuiltinFxDefaultValue::MotionFunction(value) => {
                        CheckedFxStructuralBinding::MotionFunction(value)
                    }
                    arcweft_presentation::fx::BuiltinFxDefaultValue::Bool(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::Milli(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::RatioMilli(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::LengthMilliPx(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::AngleMilliDegrees(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::DurationMillis(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::Seed32(_)
                    | arcweft_presentation::fx::BuiltinFxDefaultValue::Vec2Milli(_) => return None,
                })
            })
        }
        CheckedFxBindingDecision::Omitted => None,
    }
}

impl CheckedViewValueInput {
    pub(crate) const fn new(parameter: ViewParameterCoordinate, value_type: FxRuntimeType) -> Self {
        Self {
            parameter,
            value_type,
        }
    }

    pub const fn parameter(self) -> ViewParameterCoordinate {
        self.parameter
    }

    pub const fn value_type(self) -> FxRuntimeType {
        self.value_type
    }
}

impl CheckedViewValueProgram {
    pub(crate) fn seal(
        inputs: Vec<CheckedViewValueInput>,
        instructions: Vec<ValueInstruction>,
        return_type: FxRuntimeType,
    ) -> Result<Self, CheckedViewValueProgramSealError> {
        let mut seen = std::collections::BTreeSet::new();
        for input in &inputs {
            if !seen.insert(input.parameter()) {
                return Err(CheckedViewValueProgramSealError::DuplicateInput {
                    parameter: input.parameter(),
                });
            }
        }
        let schema = ValueProgramSchema::new(
            inputs.iter().map(|input| input.value_type()).collect(),
            Vec::new(),
            return_type,
        );
        let program =
            ValidatedValueProgram::validate(schema, instructions, ValueProgramLimits::VIEW)?;
        let semantic_digest = digest_checked_view_value_program(&inputs, &program)?;
        Ok(Self {
            inputs: inputs.into_boxed_slice(),
            program,
            semantic_digest,
        })
    }

    pub const fn inputs(&self) -> &[CheckedViewValueInput] {
        &self.inputs
    }

    pub const fn program(&self) -> &ValidatedValueProgram {
        &self.program
    }

    pub const fn return_type(&self) -> FxRuntimeType {
        self.program.schema().return_type()
    }

    pub const fn semantic_digest(&self) -> &[u8; 32] {
        &self.semantic_digest
    }
}

fn digest_checked_view_value_program(
    inputs: &[CheckedViewValueInput],
    program: &ValidatedValueProgram,
) -> Result<[u8; 32], CheckedViewValueProgramSealError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.checked-view-value-program.v1\0");
    hash_view_len(&mut hasher, inputs.len())?;
    for input in inputs {
        hasher.update(&input.parameter().value().to_le_bytes());
        hasher.update(&[input.value_type() as u8]);
    }
    hasher.update(
        &program
            .semantic_digest_v1()
            .map_err(|_| CheckedViewValueProgramSealError::Encoding)?,
    );
    Ok(*hasher.finalize().as_bytes())
}

fn hash_view_len(
    hasher: &mut blake3::Hasher,
    value: usize,
) -> Result<(), CheckedViewValueProgramSealError> {
    let value = u64::try_from(value).map_err(|_| CheckedViewValueProgramSealError::Encoding)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}

impl CheckedFxBodyDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl CheckedFxConstructorCall {
    pub(crate) fn new(
        constructor: FxSourceConstructor,
        arguments: Vec<CheckedFxConstructorArgument>,
    ) -> Self {
        Self {
            constructor,
            arguments: arguments.into_boxed_slice(),
        }
    }

    pub const fn constructor(&self) -> FxSourceConstructor {
        self.constructor
    }

    pub const fn arguments(&self) -> &[CheckedFxConstructorArgument] {
        &self.arguments
    }
}

impl CheckedFxConstructorArgument {
    pub(crate) const fn new(parameter: u16, value: CheckedFxConstructorArgumentValue) -> Self {
        Self { parameter, value }
    }

    pub const fn parameter(&self) -> u16 {
        self.parameter
    }

    pub const fn value(&self) -> &CheckedFxConstructorArgumentValue {
        &self.value
    }
}

impl<S> CheckedFxBodyCall<S> {
    pub(crate) fn new(
        definition: CheckedFxDefinitionRef,
        arguments: Vec<CheckedFxArgument<S>>,
    ) -> Self {
        Self {
            definition,
            arguments: arguments.into_boxed_slice(),
        }
    }

    pub const fn definition(&self) -> &CheckedFxDefinitionRef {
        &self.definition
    }

    pub const fn arguments(&self) -> &[CheckedFxArgument<S>] {
        &self.arguments
    }
}

impl CheckedFxSymbolicValue {
    pub const fn static_type(&self) -> FxStaticType {
        match self {
            Self::Parameter(value) => value.parameter_type().static_type(),
            Self::Constant(value) => value.static_type(),
            Self::Program(value) => FxStaticType::Runtime(value.return_type()),
        }
    }
}

impl CheckedFxConstant {
    pub const fn static_type(&self) -> FxStaticType {
        match self {
            Self::Abi(value) => value.parameter_type().static_type(),
            Self::Selector(value) => FxStaticType::Selector(value.domain()),
            Self::ShaderStage(_) => FxStaticType::ShaderStage,
            Self::FontFamily(_) => FxStaticType::FontFamily,
            Self::Target(_) => FxStaticType::Target,
            Self::Phase(_) => FxStaticType::Phase,
        }
    }

    pub const fn abi_value(&self) -> Option<&FxDefinitionArgumentValue> {
        match self {
            Self::Abi(value) => Some(value),
            Self::Selector(_)
            | Self::ShaderStage(_)
            | Self::FontFamily(_)
            | Self::Target(_)
            | Self::Phase(_) => None,
        }
    }
}

impl CheckedFxBody {
    pub(crate) fn seal(
        root: CheckedFxGraphExpression,
        expanded_nodes: u32,
        expanded_visits: u32,
        expanded_depth: u16,
    ) -> Result<Self, CheckedFxDefinitionCatalogError> {
        let digest = digest_checked_fx_body(&root)?;
        Ok(Self {
            root,
            digest,
            expanded_nodes,
            expanded_visits,
            expanded_depth,
        })
    }

    pub const fn root(&self) -> &CheckedFxGraphExpression {
        &self.root
    }

    pub const fn digest(&self) -> CheckedFxBodyDigest {
        self.digest
    }

    pub(crate) const fn expanded_nodes(&self) -> u32 {
        self.expanded_nodes
    }

    pub(crate) const fn expanded_visits(&self) -> u32 {
        self.expanded_visits
    }

    pub(crate) const fn expanded_depth(&self) -> u16 {
        self.expanded_depth
    }
}

impl CheckedProjectFxDefinition {
    pub(crate) fn new(
        declaration: CallableDeclarationKey,
        definition: FxId,
        schema: CallableSignatureSchemaDigest,
        parameter_schema: FxDefinitionParameterSchema,
        body: CheckedFxBody,
    ) -> Option<Self> {
        (parameter_schema.id() == &definition).then_some(Self {
            declaration,
            definition,
            schema,
            parameter_schema,
            body,
        })
    }

    pub const fn declaration(&self) -> &CallableDeclarationKey {
        &self.declaration
    }

    pub const fn definition(&self) -> &FxId {
        &self.definition
    }

    pub const fn schema(&self) -> CallableSignatureSchemaDigest {
        self.schema
    }

    pub const fn parameter_schema(&self) -> &FxDefinitionParameterSchema {
        &self.parameter_schema
    }

    pub const fn body(&self) -> &CheckedFxBody {
        &self.body
    }

    pub fn reference(&self) -> CheckedFxDefinitionRef {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.checked-project-fx-definition.v1\0");
        hasher.update(self.schema.as_bytes());
        hasher.update(self.parameter_schema.digest().as_bytes());
        hasher.update(self.parameter_schema.parameter_layout().digest().as_bytes());
        hasher.update(self.body.digest().as_bytes());
        let body = CheckedFxBodyDigest(*hasher.finalize().as_bytes());
        CheckedFxDefinitionRef::Project {
            declaration: self.declaration.clone(),
            definition: self.definition.clone(),
            schema: self.schema,
            layout: self.parameter_schema.parameter_layout().digest(),
            body,
        }
    }
}

impl CheckedFxDefinitionCatalog {
    pub(crate) fn validate_applications(
        &self,
        expressions: &BTreeMap<arcweft_lang_hir::identity::ExprId, CheckedExpression>,
    ) -> Result<(), CheckedFxDefinitionCatalogError> {
        for expression in expressions.values() {
            match expression.resolution() {
                CheckedExpressionResolution::DialogueApplication { rich_text, .. } => {
                    self.visit_tokens(rich_text.content().tokens())?;
                }
                CheckedExpressionResolution::ViewFxApplication(application) => {
                    self.validate_reference(application.definition())?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn visit_tokens(
        &self,
        tokens: &[CheckedDialogueToken],
    ) -> Result<(), CheckedFxDefinitionCatalogError> {
        for token in tokens {
            let CheckedDialogueToken::ContentInsert(insertion) = token else {
                continue;
            };
            if let crate::checked_rich_text::CheckedContentEmission::Fx(application) =
                insertion.emission()
            {
                self.validate_reference(application.definition())?;
            }
            if let Some(content) = insertion.argument().checked_content() {
                self.visit_tokens(content.content().tokens())?;
            }
        }
        Ok(())
    }

    pub(crate) fn insert(
        &mut self,
        definition: CheckedFxDefinition,
    ) -> Result<(), CheckedFxDefinitionCatalogError> {
        let id = definition.definition().clone();
        if let CheckedFxDefinition::Builtin {
            row,
            specialization,
            schema,
            definition,
            layout,
            binding_plan,
        } = &definition
        {
            let template = arcweft_presentation::fx::build_builtin_fx_definition(*specialization)
                .map_err(|_| CheckedFxDefinitionCatalogError::MismatchedReference {
                definition: definition.clone(),
            })?;
            if specialization.row() != *row
                || template.definition().id() != definition
                || template.definition().parameter_layout().digest() != *layout
                || template.binding_plan().schema_digest() != *schema
                || template.binding_plan() != binding_plan
            {
                return Err(CheckedFxDefinitionCatalogError::MismatchedReference {
                    definition: definition.clone(),
                });
            }
        }
        match self.definitions.entry(id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(definition);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &definition => {
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(CheckedFxDefinitionCatalogError::ConflictingDefinition { definition: id })
            }
        }
    }

    pub fn get(&self, definition: &FxId) -> Option<&CheckedFxDefinition> {
        self.definitions.get(definition)
    }

    /// Resolves one checked source-schema parameter to its direct definition
    /// ABI coordinate. Builtin structural parameters deliberately return
    /// `None`; a mismatched definition reference or source family is rejected.
    pub fn abi_parameter(
        &self,
        reference: &CheckedFxDefinitionRef,
        source: CheckedFxSourceParameter,
    ) -> Result<Option<FxDefinitionParameterIndex>, CheckedFxDefinitionCatalogError> {
        self.validate_reference(reference)?;
        let definition = self.get(reference.definition()).ok_or_else(|| {
            CheckedFxDefinitionCatalogError::MismatchedReference {
                definition: reference.definition().clone(),
            }
        })?;
        match (definition, source) {
            (
                CheckedFxDefinition::Builtin { binding_plan, .. },
                CheckedFxSourceParameter::Builtin(parameter),
            ) => Ok(binding_plan
                .parameters()
                .iter()
                .find(|row| row.id() == parameter)
                .map(|row| match row.projection() {
                    BuiltinFxAbiProjection::Direct(index) => *index,
                })),
            (CheckedFxDefinition::Project(project), CheckedFxSourceParameter::Project(index)) => {
                Ok(project
                    .parameter_schema()
                    .parameters()
                    .get(usize::from(index.get()))
                    .is_some_and(|parameter| parameter.index() == index)
                    .then_some(index))
            }
            (CheckedFxDefinition::Builtin { .. }, CheckedFxSourceParameter::Project(_))
            | (CheckedFxDefinition::Project(_), CheckedFxSourceParameter::Builtin(_)) => {
                Err(CheckedFxDefinitionCatalogError::MismatchedReference {
                    definition: reference.definition().clone(),
                })
            }
        }
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = (&FxId, &CheckedFxDefinition)> {
        self.definitions.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    fn validate_reference(
        &self,
        reference: &CheckedFxDefinitionRef,
    ) -> Result<(), CheckedFxDefinitionCatalogError> {
        let Some(definition) = self.get(reference.definition()) else {
            return Err(CheckedFxDefinitionCatalogError::MismatchedReference {
                definition: reference.definition().clone(),
            });
        };
        if &definition.reference() != reference {
            return Err(CheckedFxDefinitionCatalogError::MismatchedReference {
                definition: reference.definition().clone(),
            });
        }
        Ok(())
    }
}

impl CheckedFxDefinition {
    pub const fn definition(&self) -> &FxId {
        match self {
            Self::Builtin { definition, .. } => definition,
            Self::Project(definition) => definition.definition(),
        }
    }

    pub fn reference(&self) -> CheckedFxDefinitionRef {
        match self {
            Self::Builtin {
                row,
                specialization,
                schema,
                definition,
                layout,
                ..
            } => CheckedFxDefinitionRef::Builtin {
                row: *row,
                specialization: *specialization,
                schema: *schema,
                definition: definition.clone(),
                layout: *layout,
            },
            Self::Project(definition) => definition.reference(),
        }
    }

    pub const fn project(&self) -> Option<&CheckedProjectFxDefinition> {
        match self {
            Self::Project(definition) => Some(definition),
            Self::Builtin { .. } => None,
        }
    }
}

fn digest_checked_fx_body(
    root: &CheckedFxGraphExpression,
) -> Result<CheckedFxBodyDigest, CheckedFxDefinitionCatalogError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"arcweft.checked-fx-body.v1\0");
    hash_graph_expression(&mut hasher, root)?;
    Ok(CheckedFxBodyDigest(*hasher.finalize().as_bytes()))
}

fn hash_graph_expression(
    hasher: &mut blake3::Hasher,
    expression: &CheckedFxGraphExpression,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    match expression {
        CheckedFxGraphExpression::Constructor(call) => {
            hasher.update(&[0, call.constructor().semantic_tag()]);
            hash_len(hasher, call.arguments().len())?;
            for argument in call.arguments() {
                hasher.update(&argument.parameter().to_le_bytes());
                match argument.value() {
                    CheckedFxConstructorArgumentValue::Value(value) => {
                        hasher.update(&[0]);
                        hash_symbolic_value(hasher, value)?;
                    }
                    CheckedFxConstructorArgumentValue::Graph(value) => {
                        hasher.update(&[1]);
                        hash_graph_expression(hasher, value)?;
                    }
                    CheckedFxConstructorArgumentValue::Graphs(values) => {
                        hasher.update(&[2]);
                        hash_len(hasher, values.len())?;
                        for value in values {
                            hash_graph_expression(hasher, value)?;
                        }
                    }
                }
            }
        }
        CheckedFxGraphExpression::Builtin(call) => {
            hasher.update(&[1]);
            hash_body_call(hasher, call)?;
        }
        CheckedFxGraphExpression::Project(call) => {
            hasher.update(&[2]);
            hash_body_call(hasher, call)?;
        }
    }
    Ok(())
}

fn hash_body_call(
    hasher: &mut blake3::Hasher,
    call: &CheckedFxBodyCall<CheckedSymbolicFxBinding>,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    hash_definition_ref(hasher, call.definition())?;
    hash_len(hasher, call.arguments().len())?;
    for argument in call.arguments() {
        match argument.parameter() {
            CheckedFxSourceParameter::Builtin(parameter) => {
                hasher.update(&[0, parameter.semantic_tag()]);
            }
            CheckedFxSourceParameter::Project(parameter) => {
                hasher.update(&[1]);
                hasher.update(&parameter.get().to_le_bytes());
            }
        }
        match argument.decision() {
            CheckedFxBindingDecision::Explicit(binding) => {
                hasher.update(&[0]);
                match binding {
                    CheckedSymbolicFxBinding::Value(value) => {
                        hasher.update(&[0]);
                        hash_symbolic_value(hasher, value)?;
                    }
                    CheckedSymbolicFxBinding::Phase(value) => {
                        hasher.update(&[1]);
                        hasher.update(&value.tag().to_le_bytes());
                    }
                    CheckedSymbolicFxBinding::Target(value) => {
                        hasher.update(&[2]);
                        hasher.update(&value.tag().to_le_bytes());
                    }
                    CheckedSymbolicFxBinding::MotionFunction(value) => {
                        hasher.update(&[3]);
                        hasher.update(&value.tag().to_le_bytes());
                    }
                }
            }
            CheckedFxBindingDecision::Defaulted => {
                hasher.update(&[1]);
            }
            CheckedFxBindingDecision::Omitted => {
                hasher.update(&[2]);
            }
        }
    }
    Ok(())
}

fn hash_definition_ref(
    hasher: &mut blake3::Hasher,
    reference: &CheckedFxDefinitionRef,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    match reference {
        CheckedFxDefinitionRef::Builtin {
            row,
            specialization,
            schema,
            definition,
            layout,
        } => {
            hasher.update(&[0, row.semantic_tag()]);
            hasher.update(
                &specialization
                    .semantic_digest_v1()
                    .map_err(|_| CheckedFxDefinitionCatalogError::InvalidBodyEncoding)?,
            );
            hasher.update(schema.as_bytes());
            hash_fx_id(hasher, definition)?;
            hasher.update(layout.as_bytes());
        }
        CheckedFxDefinitionRef::Project {
            definition,
            schema,
            layout,
            body,
            ..
        } => {
            hasher.update(&[1]);
            hash_fx_id(hasher, definition)?;
            hasher.update(schema.as_bytes());
            hasher.update(layout.as_bytes());
            hasher.update(body.as_bytes());
        }
    }
    Ok(())
}

fn hash_symbolic_value(
    hasher: &mut blake3::Hasher,
    value: &CheckedFxSymbolicValue,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    match value {
        CheckedFxSymbolicValue::Parameter(value) => {
            hasher.update(&[0]);
            hasher.update(&value.index().get().to_le_bytes());
            hash_parameter_type(hasher, value.parameter_type());
        }
        CheckedFxSymbolicValue::Constant(value) => {
            hasher.update(&[1]);
            match value {
                CheckedFxConstant::Abi(value) => {
                    hasher.update(&[0]);
                    let bytes = value
                        .canonical_v1_bytes()
                        .map_err(|_| CheckedFxDefinitionCatalogError::InvalidBodyEncoding)?;
                    hash_bytes(hasher, &bytes)?;
                }
                CheckedFxConstant::Selector(value) => {
                    hasher.update(&[1, value.domain() as u8]);
                    hash_bytes(hasher, value.name().as_str().as_bytes())?;
                }
                CheckedFxConstant::ShaderStage(value) => {
                    hasher.update(&[2]);
                    hasher.update(&value.tag().to_le_bytes());
                }
                CheckedFxConstant::FontFamily(value) => {
                    hasher.update(&[3]);
                    hash_bytes(hasher, value.as_str().as_bytes())?;
                }
                CheckedFxConstant::Target(value) => {
                    hasher.update(&[4]);
                    hasher.update(&value.tag().to_le_bytes());
                }
                CheckedFxConstant::Phase(value) => {
                    hasher.update(&[5]);
                    hasher.update(&value.tag().to_le_bytes());
                }
            }
        }
        CheckedFxSymbolicValue::Program(value) => {
            hasher.update(&[2]);
            let bytes = value
                .canonical_v1_bytes()
                .map_err(|_| CheckedFxDefinitionCatalogError::InvalidBodyEncoding)?;
            hash_bytes(hasher, &bytes)?;
        }
    }
    Ok(())
}

fn hash_parameter_type(hasher: &mut blake3::Hasher, value: FxDefinitionParameterType) {
    match value {
        FxDefinitionParameterType::Runtime(value) => {
            hasher.update(&[0, value as u8]);
        }
        FxDefinitionParameterType::Resource => {
            hasher.update(&[1]);
        }
        FxDefinitionParameterType::UniformRecord => {
            hasher.update(&[2]);
        }
    }
}

fn hash_fx_id(
    hasher: &mut blake3::Hasher,
    value: &FxId,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    hash_bytes(hasher, value.package().as_bytes())?;
    hash_bytes(hasher, value.function().as_bytes())
}

fn hash_bytes(
    hasher: &mut blake3::Hasher,
    value: &[u8],
) -> Result<(), CheckedFxDefinitionCatalogError> {
    hash_len(hasher, value.len())?;
    hasher.update(value);
    Ok(())
}

fn hash_len(
    hasher: &mut blake3::Hasher,
    value: usize,
) -> Result<(), CheckedFxDefinitionCatalogError> {
    let value =
        u64::try_from(value).map_err(|_| CheckedFxDefinitionCatalogError::InvalidBodyEncoding)?;
    hasher.update(&value.to_le_bytes());
    Ok(())
}
pub(crate) fn checked_builtin_fx_binding(
    parameter_type: arcweft_presentation::fx::BuiltinFxParameterType,
    value: &crate::final_analysis::CheckedCompileTimeValue,
) -> Result<crate::final_analysis::CheckedContentFxBinding, ()> {
    use crate::checked_compile_time::CheckedCompileTimeScalar;
    use crate::checked_rich_text::{CheckedColor, LengthUnit};
    use crate::final_analysis::CheckedCompileTimeValue;
    use arcweft_presentation::fx::{
        Angle, BuiltinFxParameterType, FiniteF32, FxColor, FxDefinitionArgumentValue, FxResourceId,
        FxRuntimeValue, FxVec2, Length, Seconds,
    };

    let runtime = match (parameter_type, value) {
        (
            BuiltinFxParameterType::Bool,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Bool(value)),
        ) => FxRuntimeValue::Bool(*value),
        (BuiltinFxParameterType::Seed32, CheckedCompileTimeValue::Seed32(value)) => {
            FxRuntimeValue::U32(*value)
        }
        (
            BuiltinFxParameterType::FixedMilli,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Milli(value)),
        ) => FxRuntimeValue::F32(
            FiniteF32::try_from_f64(f64::from(value.0) / 1_000.0).map_err(|_| ())?,
        ),
        (
            BuiltinFxParameterType::Ratio,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Ratio(value)),
        ) => FxRuntimeValue::F32(
            FiniteF32::try_from_f64(f64::from(value.0) / 1_000.0).map_err(|_| ())?,
        ),
        (
            BuiltinFxParameterType::Length,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Length(value)),
        ) if value.unit == LengthUnit::Px => FxRuntimeValue::Length(
            Length::try_pixels_f64(f64::from(value.milli) / 1_000.0).map_err(|_| ())?,
        ),
        (
            BuiltinFxParameterType::Angle,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Angle(value)),
        ) => FxRuntimeValue::Angle(
            Angle::try_degrees(f64::from(value.milli_degrees) / 1_000.0).map_err(|_| ())?,
        ),
        (
            BuiltinFxParameterType::Duration,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Duration(value)),
        ) => FxRuntimeValue::Seconds(
            Seconds::try_milliseconds(value.millis.to_string().parse().map_err(|_| ())?)
                .map_err(|_| ())?,
        ),
        (
            BuiltinFxParameterType::Color,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::Color(CheckedColor::Rgba8(
                value,
            ))),
        ) => FxRuntimeValue::Color(FxColor::from_rgba8(*value)),
        (BuiltinFxParameterType::Vec2, CheckedCompileTimeValue::Vector(value))
            if value.dimensions() == 2 =>
        {
            let [x, y] = value.components() else {
                return Err(());
            };
            FxRuntimeValue::Vec2(FxVec2 {
                x: FiniteF32::try_from_f64(f64::from(x.0) / 1_000.0).map_err(|_| ())?,
                y: FiniteF32::try_from_f64(f64::from(y.0) / 1_000.0).map_err(|_| ())?,
            })
        }
        (
            BuiltinFxParameterType::Resource,
            CheckedCompileTimeValue::Scalar(CheckedCompileTimeScalar::PublicId(value)),
        ) => {
            return FxResourceId::try_new(value.as_str().to_owned())
                .map(FxDefinitionArgumentValue::Resource)
                .map(crate::final_analysis::CheckedContentFxBinding::abi)
                .map_err(|_| ());
        }
        (BuiltinFxParameterType::Phase, CheckedCompileTimeValue::Enum(value)) => {
            return arcweft_presentation::fx::FxPhase::from_value_id(*value)
                .map(crate::final_analysis::CheckedContentFxBinding::Phase)
                .ok_or(());
        }
        (BuiltinFxParameterType::Target, CheckedCompileTimeValue::Enum(value)) => {
            return arcweft_presentation::fx::FxTarget::from_value_id(*value)
                .map(crate::final_analysis::CheckedContentFxBinding::Target)
                .ok_or(());
        }
        (BuiltinFxParameterType::MotionFunction, CheckedCompileTimeValue::Enum(value)) => {
            return arcweft_presentation::fx::MotionFunction::from_value_id(*value)
                .map(crate::final_analysis::CheckedContentFxBinding::MotionFunction)
                .ok_or(());
        }
        _ => return Err(()),
    };
    Ok(crate::final_analysis::CheckedContentFxBinding::abi(
        FxDefinitionArgumentValue::Runtime(runtime),
    ))
}

pub(crate) fn checked_project_fx_parameter_type(
    ty: &TypeKind,
) -> Option<arcweft_presentation::fx::FxDefinitionParameterType> {
    use crate::types::CompileTimeScalarKind;
    use arcweft_presentation::fx::{FxDefinitionParameterType, FxRuntimeType};

    Some(FxDefinitionParameterType::Runtime(match ty {
        TypeKind::Bool => FxRuntimeType::Bool,
        TypeKind::I32 => FxRuntimeType::I32,
        TypeKind::U32 => FxRuntimeType::U32,
        TypeKind::F32 => FxRuntimeType::F32,
        TypeKind::Duration => FxRuntimeType::Seconds,
        TypeKind::CompileTimeScalar(value) => match value.kind() {
            CompileTimeScalarKind::Milli | CompileTimeScalarKind::Ratio => FxRuntimeType::F32,
            CompileTimeScalarKind::Length => FxRuntimeType::Length,
            CompileTimeScalarKind::Angle => FxRuntimeType::Angle,
            CompileTimeScalarKind::Color => FxRuntimeType::Color,
            CompileTimeScalarKind::PublicId => return None,
        },
        TypeKind::FixedVector(vector)
            if vector.dimensions() == crate::callable::VectorDimensions::Two
                && matches!(vector.component(), TypeKind::F32) =>
        {
            FxRuntimeType::Vec2
        }
        _ => return None,
    }))
}

pub(crate) fn checked_project_fx_argument(
    expected: arcweft_presentation::fx::FxDefinitionParameterType,
    value: &crate::final_analysis::CheckedCompileTimeValue,
) -> Result<arcweft_presentation::fx::FxDefinitionArgumentValue, ()> {
    use arcweft_presentation::fx::FxRuntimeType;
    use arcweft_presentation::fx::{FxDefinitionArgumentValue, FxDefinitionParameterType};

    let source_type = match expected {
        FxDefinitionParameterType::Runtime(FxRuntimeType::Bool) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Bool
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::U32) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Seed32
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::F32) => match value {
            crate::final_analysis::CheckedCompileTimeValue::Scalar(
                crate::checked_compile_time::CheckedCompileTimeScalar::Ratio(_),
            ) => arcweft_presentation::fx::BuiltinFxParameterType::Ratio,
            _ => arcweft_presentation::fx::BuiltinFxParameterType::FixedMilli,
        },
        FxDefinitionParameterType::Runtime(FxRuntimeType::Length) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Length
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::Angle) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Angle
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::Seconds) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Duration
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::Color) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Color
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::Vec2) => {
            arcweft_presentation::fx::BuiltinFxParameterType::Vec2
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::I32) => {
            let crate::final_analysis::CheckedCompileTimeValue::Scalar(
                crate::checked_compile_time::CheckedCompileTimeScalar::Int(value),
            ) = value
            else {
                return Err(());
            };
            return i32::try_from(*value)
                .map(arcweft_presentation::fx::FxRuntimeValue::I32)
                .map(FxDefinitionArgumentValue::Runtime)
                .map_err(|_| ());
        }
        FxDefinitionParameterType::Runtime(FxRuntimeType::Transform2D)
        | FxDefinitionParameterType::Resource
        | FxDefinitionParameterType::UniformRecord => return Err(()),
    };
    let crate::final_analysis::CheckedContentFxBinding::Abi(value) =
        checked_builtin_fx_binding(source_type, value)?
    else {
        return Err(());
    };
    Ok(value)
}
