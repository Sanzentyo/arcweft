//! Validated callable scalar, path, provider, and candidate identities.
//!
//! This module intentionally keeps the closed callable identity hierarchy in
//! one place: family IDs feed `CallableCandidateId`, and their constructors
//! share the same scalar/path invariants. It contains no resolver execution or
//! embedded tests. The file is slightly above the structural-audit warning
//! threshold, but splitting one hierarchy across responsibility modules would
//! make the exact identity contract harder to audit without reducing a mixed
//! responsibility.

use std::sync::Arc;

use arcweft_character::id::CharacterId;
use arcweft_lang_hir::symbol::{CallableDeclarationId, CallablePackageId};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use crate::{
    canonicalization::SemanticScopeId,
    checker::TypeExpressionId,
    types::{EntityKind, TypeKind},
};

use super::{
    BuiltinIdentityError, CallableIdentityError, CallableIndexKind, CallableLimits,
    CallablePathError, CallableScalarError, CallableScalarKind, CallableSignatureSchema,
    PRODUCTION_CALLABLE_LIMITS,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableName(Arc<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterPackageId(Arc<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustItemPath(Arc<str>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableGroupIndex(u16);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterIndex(u16);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableOverloadIndex(u16);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableArgumentIndex(u16);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableArgumentSlotIndex(u16);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LexicalBindingIndex(u32);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionValueOrdinal(u32);

impl CallableName {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError> {
        let value = value.into();
        validate_scalar(&value, CallableScalarKind::CallableName, |character| {
            matches!(
                character,
                '.' | ':' | '/' | '\\' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterPackageId {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CallableScalarError::Empty {
                kind: CallableScalarKind::AdapterPackageId,
            });
        }
        for (byte, character) in value.char_indices() {
            if character.is_control() {
                return Err(CallableScalarError::Control {
                    kind: CallableScalarKind::AdapterPackageId,
                    byte,
                });
            }
            if character.is_whitespace() || matches!(character, '/' | '\\' | ':' | '@') {
                return Err(CallableScalarError::ContainsSeparator {
                    kind: CallableScalarKind::AdapterPackageId,
                    byte,
                    separator: character,
                });
            }
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RustItemPath {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError> {
        let value = value.into();
        validate_scalar(&value, CallableScalarKind::RustItemPath, |_| false)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_scalar(
    value: &str,
    kind: CallableScalarKind,
    separator: impl Fn(char) -> bool,
) -> Result<(), CallableScalarError> {
    if value.is_empty() {
        return Err(CallableScalarError::Empty { kind });
    }
    for (byte, character) in value.char_indices() {
        if character.is_control() {
            return Err(CallableScalarError::Control { kind, byte });
        }
        if separator(character) {
            return Err(CallableScalarError::ContainsSeparator {
                kind,
                byte,
                separator: character,
            });
        }
    }
    Ok(())
}

impl CallableGroupIndex {
    pub(crate) const ZERO: Self = Self(0);

    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::Group,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl CallableParameterIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::Parameter,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl CallableOverloadIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::Overload,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl CallableArgumentIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::Argument,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl CallableArgumentSlotIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u16::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::ArgumentSlot,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl LexicalBindingIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::LexicalBinding,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
impl FunctionValueOrdinal {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::FunctionValue,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallablePath {
    segments: Arc<[CallableName]>,
}

impl CallablePath {
    pub fn try_new(
        segments: impl IntoIterator<Item = CallableName>,
    ) -> Result<Self, CallablePathError> {
        Self::try_new_with_limits(segments, &PRODUCTION_CALLABLE_LIMITS)
    }

    pub(crate) fn try_new_with_limits(
        segments: impl IntoIterator<Item = CallableName>,
        limits: &CallableLimits,
    ) -> Result<Self, CallablePathError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(CallablePathError::Empty);
        }
        if segments.len() > limits.max_path_segments() {
            return Err(CallablePathError::TooManySegments {
                actual: segments.len(),
                limit: limits.max_path_segments(),
            });
        }
        Ok(Self {
            segments: segments.into(),
        })
    }

    pub fn segments(&self) -> &[CallableName] {
        &self.segments
    }
    /// Returns the final segment of this validated non-empty path.
    ///
    /// # Panics
    ///
    /// Panics only if an internal invariant is violated; public constructors reject empty paths.
    pub fn leaf(&self) -> &CallableName {
        self.segments
            .last()
            .expect("validated non-empty callable path")
    }
    #[allow(
        clippy::len_without_is_empty,
        reason = "CallablePath is non-empty by construction"
    )]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns the canonical dotted source spelling of this validated path.
    pub fn dotted_name(&self) -> String {
        self.segments
            .iter()
            .map(CallableName::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }

    pub(crate) fn matches(&self, segments: &[&str]) -> bool {
        self.segments.len() == segments.len()
            && self
                .segments
                .iter()
                .zip(segments)
                .all(|(actual, expected)| actual.as_str() == *expected)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ReceiverMethodKey {
    receiver: TypeKind,
    method: CallableName,
}

impl ReceiverMethodKey {
    pub fn new(receiver: TypeKind, method: CallableName) -> Self {
        Self { receiver, method }
    }
    pub const fn receiver(&self) -> &TypeKind {
        &self.receiver
    }
    pub const fn method(&self) -> &CallableName {
        &self.method
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableLookupKey {
    Free(CallablePath),
    Method(ReceiverMethodKey),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectCallablePath {
    package: CallablePackageId,
    module: CanonicalModulePath,
    path: CallablePath,
}

impl ProjectCallablePath {
    pub fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        path: CallablePath,
    ) -> Self {
        Self {
            package,
            module,
            path,
        }
    }
    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }
    pub const fn path(&self) -> &CallablePath {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ProjectNameBinding {
    Callable(CallableDeclarationId),
    Environment(EnvironmentCallableId),
    NonCallable {
        path: ProjectCallablePath,
        ty: TypeKind,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StandardEnvironmentId {
    Core,
    SansIo,
    NativeHttp,
    InferenceTensor,
    SystemInfo,
    NativeFile,
    Math,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableOwner {
    Standard(StandardEnvironmentId),
    Adapter(AdapterPackageId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableKind {
    Function,
    Method,
    UntypedMethodFallback,
    RustFunction,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnvironmentCallableId {
    owner: EnvironmentCallableOwner,
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: CallableOverloadIndex,
}

impl EnvironmentCallableId {
    pub fn new(
        owner: EnvironmentCallableOwner,
        kind: EnvironmentCallableKind,
        key: CallableLookupKey,
        overload: CallableOverloadIndex,
    ) -> Self {
        Self {
            owner,
            kind,
            key,
            overload,
        }
    }
    pub const fn owner(&self) -> &EnvironmentCallableOwner {
        &self.owner
    }
    pub const fn kind(&self) -> EnvironmentCallableKind {
        self.kind
    }
    pub const fn key(&self) -> &CallableLookupKey {
        &self.key
    }
    pub const fn overload(&self) -> CallableOverloadIndex {
        self.overload
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAuthorityRank {
    Project,
    Standard,
    Adapter,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableProviderId {
    Project(CallablePackageId),
    Standard(StandardEnvironmentId),
    Adapter(AdapterPackageId),
}

impl EnvironmentCallableOwner {
    pub const fn authority(&self) -> CallableAuthorityRank {
        match self {
            Self::Standard(_) => CallableAuthorityRank::Standard,
            Self::Adapter(_) => CallableAuthorityRank::Adapter,
        }
    }
    pub fn provider(&self) -> CallableProviderId {
        match self {
            Self::Standard(id) => CallableProviderId::Standard(*id),
            Self::Adapter(id) => CallableProviderId::Adapter(id.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageDocumentationFamily {
    Builtin,
    Fx,
    Agent,
    Presentation,
    Dialogue,
    Collection,
    Domain,
    Integer,
    Capacity,
    Trait,
    Constructor,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorDimensions {
    Two,
    Three,
    Four,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MathCallableId {
    MatMulF32,
    MatrixAddF32,
    MatMulF64,
    MatrixAddF64,
    TensorAddF32,
    TensorAddF64,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatWidth {
    F32,
    F64,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StdFloatOperation {
    Abs,
    Floor,
    Ceil,
    Round,
    Trunc,
    Fract,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Exp,
    Exp2,
    Ln,
    Log2,
    Log10,
    Powf,
    Atan2,
    MulAdd,
    IsNan,
    IsInfinite,
    IsFinite,
    IsSignPositive,
    IsSignNegative,
    ToBits,
    FromBits,
    ToF32,
    ToF64,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StdFloatCallableId {
    width: FloatWidth,
    operation: StdFloatOperation,
}

impl StdFloatCallableId {
    pub fn try_new(
        width: FloatWidth,
        operation: StdFloatOperation,
    ) -> Result<Self, BuiltinIdentityError> {
        if matches!(
            (width, operation),
            (FloatWidth::F32, StdFloatOperation::ToF32)
                | (FloatWidth::F64, StdFloatOperation::ToF64)
        ) {
            return Err(BuiltinIdentityError::UnsupportedConversion { width, operation });
        }
        Ok(Self { width, operation })
    }
    pub const fn width(self) -> FloatWidth {
        self.width
    }
    pub const fn operation(self) -> StdFloatOperation {
        self.operation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CapabilityCallableId {
    EventEmit,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinCallableId {
    InlineFailureFallback,
    Panic,
    Fail,
    Bail,
    Ensure,
    Assert,
    DebugAssert,
    Rgb,
    Sin,
    Cos,
    Vector { dimensions: VectorDimensions },
    Math(MathCallableId),
    StdFloat(StdFloatCallableId),
    Capability(CapabilityCallableId),
    Reduction(ReductionConstructorKind),
}

impl BuiltinCallableId {
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        let direct = [
            (&["fallback"][..], Self::InlineFailureFallback),
            (
                &["InlineFailure", "fallback"][..],
                Self::InlineFailureFallback,
            ),
            (&["panic"][..], Self::Panic),
            (&["fail"][..], Self::Fail),
            (&["bail"][..], Self::Bail),
            (&["ensure"][..], Self::Ensure),
            (&["assert"][..], Self::Assert),
            (&["debug_assert"][..], Self::DebugAssert),
            (&["rgb"][..], Self::Rgb),
            (&["sin"][..], Self::Sin),
            (&["cos"][..], Self::Cos),
            (
                &["vec2"][..],
                Self::Vector {
                    dimensions: VectorDimensions::Two,
                },
            ),
            (
                &["vec3"][..],
                Self::Vector {
                    dimensions: VectorDimensions::Three,
                },
            ),
            (
                &["vec4"][..],
                Self::Vector {
                    dimensions: VectorDimensions::Four,
                },
            ),
            (
                &["math", "matmul_f32"][..],
                Self::Math(MathCallableId::MatMulF32),
            ),
            (
                &["math", "matrix_add_f32"][..],
                Self::Math(MathCallableId::MatrixAddF32),
            ),
            (
                &["math", "matmul_f64"][..],
                Self::Math(MathCallableId::MatMulF64),
            ),
            (
                &["math", "matrix_add_f64"][..],
                Self::Math(MathCallableId::MatrixAddF64),
            ),
            (
                &["math", "tensor_add_f32"][..],
                Self::Math(MathCallableId::TensorAddF32),
            ),
            (
                &["math", "tensor_add_f64"][..],
                Self::Math(MathCallableId::TensorAddF64),
            ),
            (
                &["event", "emit"][..],
                Self::Capability(CapabilityCallableId::EventEmit),
            ),
        ];
        if let Some((_, id)) = direct
            .into_iter()
            .find(|(segments, _)| path.matches(segments))
        {
            return Some(id);
        }
        resolve_std_float(path)
            .or_else(|| ReductionConstructorKind::resolve(path).map(BuiltinCallableId::Reduction))
    }
}

fn resolve_std_float(path: &CallablePath) -> Option<BuiltinCallableId> {
    let [std, width, operation] = path.segments() else {
        return None;
    };
    if std.as_str() != "std" {
        return None;
    }
    let width = match width.as_str() {
        "f32" => FloatWidth::F32,
        "f64" => FloatWidth::F64,
        _ => return None,
    };
    let operation = match operation.as_str() {
        "abs" => StdFloatOperation::Abs,
        "floor" => StdFloatOperation::Floor,
        "ceil" => StdFloatOperation::Ceil,
        "round" => StdFloatOperation::Round,
        "trunc" => StdFloatOperation::Trunc,
        "fract" => StdFloatOperation::Fract,
        "sqrt" => StdFloatOperation::Sqrt,
        "sin" => StdFloatOperation::Sin,
        "cos" => StdFloatOperation::Cos,
        "tan" => StdFloatOperation::Tan,
        "exp" => StdFloatOperation::Exp,
        "exp2" => StdFloatOperation::Exp2,
        "ln" => StdFloatOperation::Ln,
        "log2" => StdFloatOperation::Log2,
        "log10" => StdFloatOperation::Log10,
        "powf" => StdFloatOperation::Powf,
        "atan2" => StdFloatOperation::Atan2,
        "mul_add" => StdFloatOperation::MulAdd,
        "is_nan" => StdFloatOperation::IsNan,
        "is_infinite" => StdFloatOperation::IsInfinite,
        "is_finite" => StdFloatOperation::IsFinite,
        "is_sign_positive" => StdFloatOperation::IsSignPositive,
        "is_sign_negative" => StdFloatOperation::IsSignNegative,
        "to_bits" => StdFloatOperation::ToBits,
        "from_bits" => StdFloatOperation::FromBits,
        "to_f32" => StdFloatOperation::ToF32,
        "to_f64" => StdFloatOperation::ToF64,
        _ => return None,
    };
    StdFloatCallableId::try_new(width, operation)
        .ok()
        .map(BuiltinCallableId::StdFloat)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectNominalTypeId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: CallableName,
}
impl ProjectNominalTypeId {
    pub fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        name: CallableName,
    ) -> Self {
        Self {
            package,
            module,
            name,
        }
    }
    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }
    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }
    pub const fn name(&self) -> &CallableName {
        &self.name
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnumVariantSignatureId {
    owner: ProjectNominalTypeId,
    variant: CallableName,
}
impl EnumVariantSignatureId {
    pub fn new(owner: ProjectNominalTypeId, variant: CallableName) -> Self {
        Self { owner, variant }
    }
    pub const fn owner(&self) -> &ProjectNominalTypeId {
        &self.owner
    }
    pub const fn variant(&self) -> &CallableName {
        &self.variant
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResultConstructorKind {
    Ok,
    Err,
}

impl ResultConstructorKind {
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        if path.matches(&["Ok"]) {
            Some(Self::Ok)
        } else if path.matches(&["Err"]) {
            Some(Self::Err)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionConstructorKind {
    Some,
}

impl OptionConstructorKind {
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        path.matches(&["Some"]).then_some(Self::Some)
    }
}

/// Core `Reduction` constructor selected by a source callable path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReductionConstructorKind {
    Unchanged,
}

impl ReductionConstructorKind {
    /// Resolves one canonical `Reduction` constructor path.
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        path.matches(&["Reduction", "unchanged"])
            .then_some(Self::Unchanged)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FxCallableSignatureId {
    Style,
    Text,
    Color,
    Transform,
    Mask,
    Filter,
    Shader,
    Transition,
    Conditional,
    Stack,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FxResolution {
    NotFx,
    Known(FxCallableSignatureId),
    UnknownMember { member: CallableName },
    InvalidNestedPath { path: CallablePath },
}

impl FxCallableSignatureId {
    pub fn resolve(path: &CallablePath) -> FxResolution {
        let [namespace, member] = path.segments() else {
            return if path
                .segments()
                .first()
                .is_some_and(|segment| segment.as_str() == "Fx")
            {
                FxResolution::InvalidNestedPath { path: path.clone() }
            } else {
                FxResolution::NotFx
            };
        };
        if namespace.as_str() != "Fx" {
            return FxResolution::NotFx;
        }
        let id = match member.as_str() {
            "style" => Self::Style,
            "text" => Self::Text,
            "color" => Self::Color,
            "transform" => Self::Transform,
            "mask" => Self::Mask,
            "filter" => Self::Filter,
            "shader" => Self::Shader,
            "transition" => Self::Transition,
            "conditional" => Self::Conditional,
            "stack" => Self::Stack,
            _ => {
                return FxResolution::UnknownMember {
                    member: member.clone(),
                };
            }
        };
        FxResolution::Known(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AgentIntrinsicSignatureId {
    Expect,
    Deny,
    Checkpoint,
    Note,
    Attach,
    ChoiceAction,
    Viewport,
    Layer,
    Object,
    Capture,
    ReadResource,
    EntityMeta,
    ProjectNeighbors,
    Signal,
    Metric,
    StatePath,
    ObservationPath,
    State,
    Observation,
    Diagnostics,
    Exists,
    ActionEnabled,
    All,
    Any,
    Not,
    Wait,
    AdvanceText,
    ViewportPoint,
    PointerClick,
    Invoke,
    RagQuery,
}

impl AgentIntrinsicSignatureId {
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        let entries = [
            (&["expect"][..], Self::Expect),
            (&["deny"][..], Self::Deny),
            (&["checkpoint"][..], Self::Checkpoint),
            (&["note"][..], Self::Note),
            (&["attach"][..], Self::Attach),
            (&["choice_action"][..], Self::ChoiceAction),
            (&["viewport"][..], Self::Viewport),
            (&["layer"][..], Self::Layer),
            (&["object"][..], Self::Object),
            (&["capture"][..], Self::Capture),
            (&["read_resource"][..], Self::ReadResource),
            (&["entity_meta"][..], Self::EntityMeta),
            (&["project_neighbors"][..], Self::ProjectNeighbors),
            (&["signal"][..], Self::Signal),
            (&["metric"][..], Self::Metric),
            (&["state_path"][..], Self::StatePath),
            (&["observation_path"][..], Self::ObservationPath),
            (&["state"][..], Self::State),
            (&["observation"][..], Self::Observation),
            (&["diagnostics"][..], Self::Diagnostics),
            (&["exists"][..], Self::Exists),
            (&["action_enabled"][..], Self::ActionEnabled),
            (&["all"][..], Self::All),
            (&["any"][..], Self::Any),
            (&["not"][..], Self::Not),
            (&["wait"][..], Self::Wait),
            (&["advance_text"][..], Self::AdvanceText),
            (&["viewport_point"][..], Self::ViewportPoint),
            (&["pointer", "click"][..], Self::PointerClick),
            (&["invoke"][..], Self::Invoke),
            (&["rag", "query"][..], Self::RagQuery),
        ];
        entries
            .into_iter()
            .find_map(|(segments, id)| path.matches(segments).then_some(id))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocalCallableId {
    scope: SemanticScopeId,
    binding: LexicalBindingIndex,
}
impl LocalCallableId {
    #[allow(dead_code, reason = "allocated by the shared resolver migration cut")]
    pub(crate) fn new(scope: SemanticScopeId, binding: LexicalBindingIndex) -> Self {
        Self { scope, binding }
    }
    pub const fn scope(&self) -> &SemanticScopeId {
        &self.scope
    }
    pub const fn binding(&self) -> LexicalBindingIndex {
        self.binding
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FunctionValueSignatureId {
    expression: TypeExpressionId,
    ordinal: FunctionValueOrdinal,
}
impl FunctionValueSignatureId {
    #[allow(dead_code, reason = "allocated by the shared resolver migration cut")]
    pub(crate) fn new(expression: TypeExpressionId, ordinal: FunctionValueOrdinal) -> Self {
        Self {
            expression,
            ordinal,
        }
    }
    pub const fn expression(&self) -> TypeExpressionId {
        self.expression
    }
    pub const fn ordinal(&self) -> FunctionValueOrdinal {
        self.ordinal
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CurriedCallableId {
    base: Box<CallableCandidateId>,
    next_group: CallableGroupIndex,
}
impl CurriedCallableId {
    pub fn try_new(
        base: CallableCandidateId,
        next_group: CallableGroupIndex,
    ) -> Result<Self, CallableIdentityError> {
        if matches!(
            base,
            CallableCandidateId::Curried(_) | CallableCandidateId::DataLast(_)
        ) {
            return Err(CallableIdentityError::InvalidCurriedBase {
                base: Box::new(base),
            });
        }
        if next_group.get() == 0 {
            return Err(CallableIdentityError::InvalidCurriedGroup {
                base: Box::new(base),
                group: next_group,
            });
        }
        Ok(Self {
            base: Box::new(base),
            next_group,
        })
    }
    pub const fn base(&self) -> &CallableCandidateId {
        &self.base
    }
    pub const fn next_group(&self) -> CallableGroupIndex {
        self.next_group
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollectionMethodId {
    Len,
    Map,
    Filter,
    Sum,
    Contains,
}

impl CollectionMethodId {
    pub fn resolve(method: &CallableName) -> Option<Self> {
        match method.as_str() {
            "len" => Some(Self::Len),
            "map" => Some(Self::Map),
            "filter" => Some(Self::Filter),
            "sum" => Some(Self::Sum),
            "contains" => Some(Self::Contains),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationHandleMethodId {
    Show,
    Hide,
    Unmount,
    Release,
    Destroy,
    OverlayPop,
}

impl PresentationHandleMethodId {
    pub fn resolve(receiver: &TypeKind, method: &CallableName) -> Option<Self> {
        let TypeKind::Handle { name, .. } = receiver else {
            return None;
        };
        match method.as_str() {
            "show" => Some(Self::Show),
            "hide" => Some(Self::Hide),
            "unmount" => Some(Self::Unmount),
            "release" => Some(Self::Release),
            "destroy" => Some(Self::Destroy),
            "pop" if name == "Overlay" => Some(Self::OverlayPop),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntegerMethodId {
    Clamp,
    Min,
    Max,
}

impl IntegerMethodId {
    pub fn resolve(receiver: &TypeKind, method: &CallableName) -> Option<Self> {
        if !receiver.is_integer() {
            return None;
        }
        match method.as_str() {
            "clamp" => Some(Self::Clamp),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProbeComparisonId {
    Eq,
    Ne,
    NotEq,
    Gt,
    Greater,
    Ge,
    GreaterOrEqual,
    Lt,
    Less,
    Le,
    LessOrEqual,
}

impl ProbeComparisonId {
    pub fn resolve(method: &CallableName) -> Option<Self> {
        match method.as_str() {
            "eq" => Some(Self::Eq),
            "ne" => Some(Self::Ne),
            "not_eq" => Some(Self::NotEq),
            "gt" => Some(Self::Gt),
            "greater" => Some(Self::Greater),
            "ge" => Some(Self::Ge),
            "greater_or_equal" => Some(Self::GreaterOrEqual),
            "lt" => Some(Self::Lt),
            "less" => Some(Self::Less),
            "le" => Some(Self::Le),
            "less_or_equal" => Some(Self::LessOrEqual),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DomainMethodId {
    Traverse,
    Parallel,
    FxSampleOrdinalPhase,
    ObservedObjectRequireRole,
    MapGet {
        key: TypeKind,
        value: TypeKind,
    },
    ProbeCompare {
        value: TypeKind,
        operation: ProbeComparisonId,
    },
    DiagnosticsHasError,
    RagContextPackSummary,
    Context,
    WithContext,
    CharacterFace {
        character: Option<CharacterId>,
    },
    CharacterSay {
        character: Option<CharacterId>,
    },
}

impl DomainMethodId {
    pub fn resolve(receiver: &TypeKind, method: &CallableName) -> Option<Self> {
        let name = method.as_str();
        if name == "traverse" {
            return Some(Self::Traverse);
        }
        if name == "parallel" {
            return Some(Self::Parallel);
        }
        if matches!(receiver, TypeKind::Named(name) if name == "FxSampleContext")
            && name == "ordinal_phase"
        {
            return Some(Self::FxSampleOrdinalPhase);
        }
        if matches!(receiver, TypeKind::Vec(item) if item.as_ref() == &TypeKind::ObservedObject)
            && name == "require_role"
        {
            return Some(Self::ObservedObjectRequireRole);
        }
        if let TypeKind::Map { key, value, .. } = receiver
            && name == "get"
        {
            return Some(Self::MapGet {
                key: key.as_ref().clone(),
                value: value.as_ref().clone(),
            });
        }
        if let TypeKind::Probe(value) = receiver
            && let Some(operation) = ProbeComparisonId::resolve(method)
        {
            return Some(Self::ProbeCompare {
                value: value.as_ref().clone(),
                operation,
            });
        }
        if receiver == &TypeKind::Named("Diagnostics".to_owned()) && name == "has_error" {
            return Some(Self::DiagnosticsHasError);
        }
        if receiver == &TypeKind::RagContextPack && name == "summary" {
            return Some(Self::RagContextPackSummary);
        }
        if matches!(
            receiver,
            TypeKind::Need { .. } | TypeKind::Option(_) | TypeKind::Result { .. }
        ) {
            match name {
                "context" => return Some(Self::Context),
                "with_context" => return Some(Self::WithContext),
                _ => {}
            }
        }
        if is_character_speaker(receiver) {
            match name {
                "face" => {
                    return Some(Self::CharacterFace { character: None });
                }
                "say" => {
                    return Some(Self::CharacterSay { character: None });
                }
                _ => {}
            }
        }
        None
    }
}

fn is_character_speaker(receiver: &TypeKind) -> bool {
    matches!(
        receiver,
        TypeKind::Speaker(EntityKind::Character) | TypeKind::SpeakerPreset(EntityKind::Character)
    ) || matches!(
        receiver,
        TypeKind::Ref(entity) if entity.kind() == &EntityKind::Character
    )
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CapacityMethodId {
    receiver: TypeKind,
    method: CallableName,
    arity: u16,
}
impl CapacityMethodId {
    pub fn try_new(
        receiver: TypeKind,
        method: CallableName,
        arity: usize,
    ) -> Result<Self, CallableIdentityError> {
        let arity = u16::try_from(arity).map_err(|_| CallableScalarError::IndexOverflow {
            kind: CallableIndexKind::Parameter,
            value: arity,
        })?;
        Ok(Self {
            receiver,
            method,
            arity,
        })
    }
    pub const fn receiver(&self) -> &TypeKind {
        &self.receiver
    }
    pub const fn method(&self) -> &CallableName {
        &self.method
    }
    pub const fn arity(&self) -> usize {
        self.arity as usize
    }

    pub fn resolve(
        receiver: &TypeKind,
        method: &CallableName,
        arity: usize,
    ) -> Option<(Self, TypeKind)> {
        let result = if receiver == &TypeKind::String
            && matches!((method.as_str(), arity), ("trim" | "to_string", 0))
        {
            TypeKind::String
        } else if matches!(receiver, TypeKind::Named(name) if name == "LineContext")
            && matches!((method.as_str(), arity), ("voice_handle", 0))
        {
            TypeKind::Named("VoiceHandle".to_owned())
        } else if matches!(receiver, TypeKind::Named(name) if name == "StageApi")
            && matches!((method.as_str(), arity), ("acquire", 1))
        {
            TypeKind::Named("StageActorHandle".to_owned())
        } else if matches!(receiver, TypeKind::Named(name) if name == "StageActorHandle")
            && matches!((method.as_str(), arity), ("look", 1 | 2))
        {
            TypeKind::Named("CueHandle".to_owned())
        } else if let TypeKind::Vec(item) = receiver
            && matches!((method.as_str(), arity), ("pop" | "pop_front", 0))
        {
            TypeKind::Option(item.clone())
        } else if let TypeKind::Vec(item) = receiver
            && matches!((method.as_str(), arity), ("collect", 0))
        {
            TypeKind::Vec(item.clone())
        } else if matches!(
            receiver,
            TypeKind::Vec(_) | TypeKind::String | TypeKind::Bytes
        ) {
            match (method.as_str(), arity) {
                ("push" | "reserve" | "shrink_to", 1) | ("shrink", 0) => TypeKind::Unit,
                _ => return None,
            }
        } else {
            return None;
        };
        Self::try_new(receiver.clone(), method.clone(), arity)
            .ok()
            .map(|id| (id, result))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitImplementationIndex(u32);
impl TraitImplementationIndex {
    #[allow(dead_code, reason = "allocated by the trait resolver migration cut")]
    pub(crate) fn try_from_usize(value: usize) -> Result<Self, CallableScalarError> {
        u32::try_from(value)
            .map(Self)
            .map_err(|_| CallableScalarError::IndexOverflow {
                kind: CallableIndexKind::FunctionValue,
                value,
            })
    }
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TraitCallableSource {
    Inherent,
    Predicate,
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitCallableId {
    trait_name: CallablePath,
    method: CallableName,
    implementation: TraitImplementationIndex,
    source: TraitCallableSource,
}
impl TraitCallableId {
    pub fn new(
        trait_name: CallablePath,
        method: CallableName,
        implementation: TraitImplementationIndex,
        source: TraitCallableSource,
    ) -> Self {
        Self {
            trait_name,
            method,
            implementation,
            source,
        }
    }
    pub const fn trait_name(&self) -> &CallablePath {
        &self.trait_name
    }
    pub const fn method(&self) -> &CallableName {
        &self.method
    }
    pub const fn implementation(&self) -> TraitImplementationIndex {
        self.implementation
    }
    pub const fn source(&self) -> TraitCallableSource {
        self.source
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DataLastCallableId {
    callable: Box<CallableCandidateId>,
    receiver_parameter: CallableParameterIndex,
    receiver_group: CallableGroupIndex,
}
impl DataLastCallableId {
    pub fn try_new(
        callable: CallableCandidateId,
        receiver_group: CallableGroupIndex,
        receiver_parameter: CallableParameterIndex,
        schema: &CallableSignatureSchema,
    ) -> Result<Self, CallableIdentityError> {
        if !matches!(
            callable,
            CallableCandidateId::Project(_)
                | CallableCandidateId::Environment(_)
                | CallableCandidateId::Local(_)
        ) {
            return Err(CallableIdentityError::InvalidDataLastBase {
                base: Box::new(callable),
            });
        }
        let Some(group) = schema.group(receiver_group) else {
            return Err(CallableIdentityError::InvalidDataLastCoordinate {
                group: receiver_group,
                parameter: receiver_parameter,
            });
        };
        let Some(parameter) = group.parameter(receiver_parameter) else {
            return Err(CallableIdentityError::InvalidDataLastCoordinate {
                group: receiver_group,
                parameter: receiver_parameter,
            });
        };
        if matches!(
            parameter.passing(),
            super::CallableParameterPassing::RestPositional
                | super::CallableParameterPassing::RestNamed
        ) {
            return Err(CallableIdentityError::DataLastReceiverIsRest {
                group: receiver_group,
                parameter: receiver_parameter,
            });
        }
        let is_final_current = receiver_parameter.get() + 1 == group.parameters().len();
        let is_sole_next = receiver_group.get() > 0 && group.parameters().len() == 1;
        if !is_final_current && !is_sole_next {
            return Err(CallableIdentityError::DataLastReceiverNotFinal {
                group: receiver_group,
                parameter: receiver_parameter,
            });
        }
        Ok(Self {
            callable: Box::new(callable),
            receiver_parameter,
            receiver_group,
        })
    }
    pub const fn callable(&self) -> &CallableCandidateId {
        &self.callable
    }
    pub const fn receiver_group(&self) -> CallableGroupIndex {
        self.receiver_group
    }
    pub const fn receiver_parameter(&self) -> CallableParameterIndex {
        self.receiver_parameter
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DropCallableId {
    Drop,
}

impl DropCallableId {
    pub fn resolve(method: &CallableName) -> Option<Self> {
        matches!(method.as_str(), "drop" | "drop_optional" | "on_drop").then_some(Self::Drop)
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PromotionCallableId {
    Promote,
    PromoteUnchecked,
    Assume,
}
impl PromotionCallableId {
    pub fn resolve(path: &CallablePath) -> Option<Self> {
        if path.matches(&["promote"]) {
            Some(Self::Promote)
        } else if path.matches(&["promote_unchecked"]) {
            Some(Self::PromoteUnchecked)
        } else if path.matches(&["assume"]) {
            Some(Self::Assume)
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SpeakerCallableId {
    character: Option<CharacterId>,
    preset: bool,
}
impl SpeakerCallableId {
    pub fn new(character: Option<CharacterId>, preset: bool) -> Self {
        Self { character, preset }
    }
    pub const fn character(&self) -> Option<&CharacterId> {
        self.character.as_ref()
    }
    pub const fn is_preset(&self) -> bool {
        self.preset
    }

    pub fn resolve_value(ty: &TypeKind, character: Option<CharacterId>) -> Option<Self> {
        match ty {
            TypeKind::Ref(entity) if entity.kind() == &EntityKind::Character => {
                Some(Self::new(character, false))
            }
            TypeKind::Speaker(EntityKind::Character) => Some(Self::new(character, false)),
            TypeKind::SpeakerPreset(EntityKind::Character) => Some(Self::new(character, true)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableCandidateId {
    Fx(FxCallableSignatureId),
    EnumVariant(EnumVariantSignatureId),
    Result(ResultConstructorKind),
    Option(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(super::PresentationCallableId),
    Dialogue(super::DialogueCallableId),
    Project(CallableDeclarationId),
    Environment(EnvironmentCallableId),
    Local(LocalCallableId),
    FunctionValue(FunctionValueSignatureId),
    Curried(CurriedCallableId),
    CollectionMethod(CollectionMethodId),
    PresentationHandleMethod(PresentationHandleMethodId),
    IntegerMethod(IntegerMethodId),
    DomainMethod(DomainMethodId),
    TraitMethod(TraitCallableId),
    DataLast(DataLastCallableId),
    CapacityMethod(CapacityMethodId),
    Drop(DropCallableId),
    Promotion(PromotionCallableId),
    Speaker(SpeakerCallableId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableFamily {
    Fx,
    EnumConstructor,
    ResultConstructor,
    OptionConstructor,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    Project,
    Environment,
    Lexical,
    FunctionValue,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    TraitMethod,
    DataLast,
    CapacityMethod,
    Drop,
    Promotion,
    Speaker,
}

impl CallableFamily {
    /// Every production callable family in stable semantic-audit order.
    pub const ALL: [Self; 22] = [
        Self::Fx,
        Self::EnumConstructor,
        Self::ResultConstructor,
        Self::OptionConstructor,
        Self::Builtin,
        Self::Agent,
        Self::Presentation,
        Self::Dialogue,
        Self::Project,
        Self::Environment,
        Self::Lexical,
        Self::FunctionValue,
        Self::CollectionMethod,
        Self::PresentationHandleMethod,
        Self::IntegerMethod,
        Self::DomainMethod,
        Self::TraitMethod,
        Self::DataLast,
        Self::CapacityMethod,
        Self::Drop,
        Self::Promotion,
        Self::Speaker,
    ];
}

impl CallableCandidateId {
    pub const fn family(&self) -> CallableFamily {
        match self {
            Self::Fx(_) => CallableFamily::Fx,
            Self::EnumVariant(_) => CallableFamily::EnumConstructor,
            Self::Result(_) => CallableFamily::ResultConstructor,
            Self::Option(_) => CallableFamily::OptionConstructor,
            Self::Builtin(_) => CallableFamily::Builtin,
            Self::Agent(_) => CallableFamily::Agent,
            Self::Presentation(_) => CallableFamily::Presentation,
            Self::Dialogue(_) => CallableFamily::Dialogue,
            Self::Project(_) => CallableFamily::Project,
            Self::Environment(_) => CallableFamily::Environment,
            Self::Local(_) => CallableFamily::Lexical,
            Self::FunctionValue(_) => CallableFamily::FunctionValue,
            Self::Curried(id) => id.base().family(),
            Self::CollectionMethod(_) => CallableFamily::CollectionMethod,
            Self::PresentationHandleMethod(_) => CallableFamily::PresentationHandleMethod,
            Self::IntegerMethod(_) => CallableFamily::IntegerMethod,
            Self::DomainMethod(_) => CallableFamily::DomainMethod,
            Self::TraitMethod(_) => CallableFamily::TraitMethod,
            Self::DataLast(_) => CallableFamily::DataLast,
            Self::CapacityMethod(_) => CallableFamily::CapacityMethod,
            Self::Drop(_) => CallableFamily::Drop,
            Self::Promotion(_) => CallableFamily::Promotion,
            Self::Speaker(_) => CallableFamily::Speaker,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LanguageCallableFamily {
    Fx,
    EnumConstructor,
    ResultConstructor,
    OptionConstructor,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    CapacityMethod,
    DataLast,
    Drop,
    Promote,
    Assume,
    Speaker,
}
