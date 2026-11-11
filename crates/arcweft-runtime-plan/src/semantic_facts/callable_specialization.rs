//! Checked source schemes and specialization demands, before state IDs exist.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
};

use arcweft_core::entry::RuntimeCallableId;
use arcweft_core::plan::{
    RuntimeCallableAttachedContract, RuntimeCallableParameterInput, RuntimeCallableRetainedInput,
    RuntimeCallableRetainedRole, RuntimeFunctionSpecializationArguments,
};
use arcweft_id::runtime_program::RuntimeProjectContinuationLineageId;
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_sema::callable::{
    CallableGroupIndex, CallableParameterCoordinate, CheckedProjectFunctionCallableSourceDigest,
};
use thiserror::Error;

use super::{
    RuntimeNormalizedType, RuntimeProjectCallable, RuntimeProjectFunctionInstanceKey,
    RuntimeSemanticTypeId, RuntimeTypeShape,
};

/// The checked code origin of a reusable source value. A source may be reached
/// through any number of expressions; expression IDs do not select its body.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectCallableSourceOrigin {
    Root,
    Continuation(RuntimeProjectContinuationLineageId),
    Retained(CheckedProjectFunctionCallableSourceDigest),
    PositionalPrefix(CheckedProjectFunctionCallableSourceDigest),
}

/// A closed producing context is part of source identity, including retained
/// types that need not occur in the remaining function arrow.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeProjectCallableSourceKey {
    digest: CheckedProjectFunctionCallableSourceDigest,
    callable: RuntimeCallableId,
    origin: RuntimeProjectCallableSourceOrigin,
    position: CallableParameterCoordinate,
    function_type: RuntimeSemanticTypeId,
    retained_types: Box<[RuntimeSemanticTypeId]>,
}

impl RuntimeProjectCallableSourceKey {
    pub const fn new(
        digest: CheckedProjectFunctionCallableSourceDigest,
        callable: RuntimeCallableId,
        origin: RuntimeProjectCallableSourceOrigin,
        position: CallableParameterCoordinate,
        function_type: RuntimeSemanticTypeId,
        retained_types: Box<[RuntimeSemanticTypeId]>,
    ) -> Self {
        Self {
            digest,
            callable,
            origin,
            position,
            function_type,
            retained_types,
        }
    }
    pub const fn callable(&self) -> &RuntimeCallableId {
        &self.callable
    }
    pub const fn digest(&self) -> CheckedProjectFunctionCallableSourceDigest {
        self.digest
    }
    pub const fn origin(&self) -> RuntimeProjectCallableSourceOrigin {
        self.origin
    }
    pub const fn function_type(&self) -> RuntimeSemanticTypeId {
        self.function_type
    }
    pub const fn group(&self) -> CallableGroupIndex {
        self.position.group()
    }
    pub const fn position(&self) -> CallableParameterCoordinate {
        self.position
    }
}

/// Either a complete existing body chain, or a retained source scheme whose
/// body is selected only by checked specialization.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProjectCallableValueTarget {
    Closed {
        instance: RuntimeProjectFunctionInstanceKey,
        position: CallableParameterCoordinate,
    },
    Source(RuntimeProjectCallableSourceKey),
}

impl RuntimeProjectCallableValueTarget {
    pub const fn callable(&self) -> &RuntimeCallableId {
        match self {
            Self::Closed { instance, .. } => instance.callable(),
            Self::Source(source) => source.callable(),
        }
    }
    pub const fn instance(&self) -> Option<&RuntimeProjectFunctionInstanceKey> {
        match self {
            Self::Closed { instance, .. } => Some(instance),
            Self::Source(_) => None,
        }
    }
}

/// A source retains the same formal/attached grammar as executable states.
/// `Infallible` prevents an unclosed source from carrying a fabricated body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallableSourceFact {
    key: RuntimeProjectCallableSourceKey,
    root: RuntimeProjectCallableSourceKey,
    callable: RuntimeProjectCallable,
    function_type: RuntimeNormalizedType,
    retained: Box<[RuntimeCallableRetainedInput<RuntimeNormalizedType>]>,
    parameters: Box<[RuntimeCallableParameterInput<RuntimeNormalizedType>]>,
    attached: RuntimeCallableAttachedContract<RuntimeNormalizedType, Infallible>,
    body: Option<RuntimeProjectFunctionInstanceKey>,
}

impl RuntimeProjectCallableSourceFact {
    pub fn try_new(
        callable: RuntimeProjectCallable,
        digest: CheckedProjectFunctionCallableSourceDigest,
        origin: RuntimeProjectCallableSourceOrigin,
        root: Option<RuntimeProjectCallableSourceKey>,
        position: CallableParameterCoordinate,
        function_type: RuntimeNormalizedType,
        retained: Box<[RuntimeCallableRetainedInput<RuntimeNormalizedType>]>,
        parameters: Box<[RuntimeCallableParameterInput<RuntimeNormalizedType>]>,
        attached: RuntimeCallableAttachedContract<RuntimeNormalizedType, Infallible>,
        body: Option<RuntimeProjectFunctionInstanceKey>,
    ) -> Result<Self, RuntimeCallableSpecializationFactError> {
        let group = u32::try_from(position.group().get())
            .map_err(|_| RuntimeCallableSpecializationFactError::Source)?;
        let first = u32::try_from(position.parameter().get())
            .map_err(|_| RuntimeCallableSpecializationFactError::Source)?;
        let mut roles = BTreeSet::new();
        if !function_type.scope().is_root()
            || !matches!(function_type.shape(), RuntimeTypeShape::Function { .. })
            || (origin == RuntimeProjectCallableSourceOrigin::Root
                && (position.group().get() != 0 || position.parameter().get() != 0))
            || parameters.iter().enumerate().any(|(ordinal, row)| {
                row.coordinate.group != group
                    || u32::try_from(ordinal)
                        .ok()
                        .and_then(|ordinal| first.checked_add(ordinal))
                        != Some(row.coordinate.parameter)
            })
            || retained.iter().any(|row| {
                !roles.insert(row.role)
                    || matches!(row.role,
                        RuntimeCallableRetainedRole::Parameter(coordinate)
                            if coordinate.group > group
                                || (coordinate.group == group && coordinate.parameter >= first))
            })
            || !retained
                .iter()
                .filter_map(|row| match row.role {
                    RuntimeCallableRetainedRole::Parameter(coordinate)
                        if coordinate.group == group =>
                    {
                        Some(coordinate.parameter)
                    }
                    _ => None,
                })
                .eq(0..first)
        {
            return Err(RuntimeCallableSpecializationFactError::Source);
        }
        if body.as_ref().is_some_and(|body| {
            body.callable() != callable.runtime() || body.group().get() < position.group().get()
        }) {
            return Err(RuntimeCallableSpecializationFactError::Source);
        }
        let key = RuntimeProjectCallableSourceKey {
            digest,
            callable: callable.runtime().clone(),
            origin,
            position,
            function_type: function_type.identity(),
            retained_types: retained.iter().map(|row| row.ty.identity()).collect(),
        };
        let root = match (origin, root) {
            (RuntimeProjectCallableSourceOrigin::Root, None) => key.clone(),
            (
                RuntimeProjectCallableSourceOrigin::Continuation(_)
                | RuntimeProjectCallableSourceOrigin::Retained(_)
                | RuntimeProjectCallableSourceOrigin::PositionalPrefix(_),
                Some(root),
            ) if root.origin() == RuntimeProjectCallableSourceOrigin::Root
                && root.callable() == key.callable() =>
            {
                root
            }
            _ => return Err(RuntimeCallableSpecializationFactError::Source),
        };
        Ok(Self {
            key,
            root,
            callable,
            function_type,
            retained,
            parameters,
            attached,
            body,
        })
    }
    pub const fn key(&self) -> &RuntimeProjectCallableSourceKey {
        &self.key
    }
    pub const fn root(&self) -> &RuntimeProjectCallableSourceKey {
        &self.root
    }
    pub const fn callable(&self) -> &RuntimeProjectCallable {
        &self.callable
    }
    pub const fn group(&self) -> CallableGroupIndex {
        self.key.group()
    }
    pub const fn position(&self) -> CallableParameterCoordinate {
        self.key.position()
    }
    pub const fn function_type(&self) -> &RuntimeNormalizedType {
        &self.function_type
    }
    pub const fn body(&self) -> Option<&RuntimeProjectFunctionInstanceKey> {
        self.body.as_ref()
    }
    pub(super) fn admits_body(&self, body: &super::RuntimeProjectFunctionInstanceFact) -> bool {
        let mut ty = body.callable_type();
        for _ in 0..self.group().get() {
            let RuntimeTypeShape::Function { result, .. } = ty.shape() else {
                return false;
            };
            ty = result;
        }
        let coordinate = (
            self.position().group().get(),
            self.position().parameter().get(),
        );
        let prefix = body
            .parameters()
            .iter()
            .filter(|row| (row.group().get(), row.parameter() as usize) < coordinate)
            .collect::<Vec<_>>();
        let parameters = body
            .parameters()
            .iter()
            .filter(|row| row.group() == self.group() && row.parameter() as usize >= coordinate.1)
            .collect::<Vec<_>>();
        body.callable() == self.callable()
            && self.function_type().is_function_suffix_of(ty, coordinate.1)
            && prefix.len() == self.retained.len()
            && prefix.iter().zip(&self.retained).all(|(parameter, input)| {
                input.ty == *parameter.binding_ty()
                    && matches!(input.role, RuntimeCallableRetainedRole::Parameter(coordinate)
                        if coordinate.group as usize == parameter.group().get()
                            && coordinate.parameter == parameter.parameter())
            })
            && parameters.len() == self.parameters.len()
            && parameters
                .iter()
                .zip(&self.parameters)
                .all(|(parameter, input)| {
                    parameter.abi_ty() == &input.abi_ty
                        && parameter.binding_ty() == &input.binding_ty
                        && input.coordinate.group as usize == parameter.group().get()
                        && input.coordinate.parameter == parameter.parameter()
                        && input.kind
                            == match parameter.kind() {
                                arcweft_lang_hir::item::HirParameterKind::Fixed
                                | arcweft_lang_hir::item::HirParameterKind::ExtensionReceiver => {
                                    arcweft_core::plan::RuntimeCallableParameterKind::Fixed
                                }
                                arcweft_lang_hir::item::HirParameterKind::RestPositional => {
                                    arcweft_core::plan::RuntimeCallableParameterKind::Rest
                                }
                            }
                })
            && match (
                body.callable()
                    .attached_content_abi()
                    .filter(|row| row.group() == self.group()),
                &self.attached,
            ) {
                (None, RuntimeCallableAttachedContract::None) => true,
                (Some(expected), RuntimeCallableAttachedContract::Required { ty }) => {
                    expected.presence()
                        == arcweft_lang_sema::callable::CallableParameterPresence::Required
                        && ty == expected.binding_ty()
                }
                (
                    Some(expected),
                    RuntimeCallableAttachedContract::Defaulted {
                        ty,
                        default: arcweft_core::plan::RuntimeCallableDefault::RequiresSpecialization,
                    },
                ) => {
                    expected.presence()
                        == arcweft_lang_sema::callable::CallableParameterPresence::Defaulted
                        && ty == expected.binding_ty()
                }
                (Some(expected), RuntimeCallableAttachedContract::Optional { value, binding }) => {
                    expected.presence()
                        == arcweft_lang_sema::callable::CallableParameterPresence::Optional
                        && binding == expected.binding_ty()
                        && matches!(binding.shape(), RuntimeTypeShape::Option { item, .. } if item.as_ref() == value)
                }
                _ => false,
            }
    }
    pub const fn retained(&self) -> &[RuntimeCallableRetainedInput<RuntimeNormalizedType>] {
        &self.retained
    }
    pub const fn parameters(&self) -> &[RuntimeCallableParameterInput<RuntimeNormalizedType>] {
        &self.parameters
    }
    pub const fn attached(
        &self,
    ) -> &RuntimeCallableAttachedContract<RuntimeNormalizedType, Infallible> {
        &self.attached
    }

    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.push(&self.function_type);
        roots.extend(self.retained.iter().map(|row| &row.ty));
        for row in &self.parameters {
            roots.extend([&row.abi_ty, &row.binding_ty]);
        }
        match &self.attached {
            RuntimeCallableAttachedContract::None => {}
            RuntimeCallableAttachedContract::Required { ty }
            | RuntimeCallableAttachedContract::Defaulted { ty, .. } => roots.push(ty),
            RuntimeCallableAttachedContract::Optional { value, binding } => {
                roots.extend([value, binding])
            }
        }
    }
}

/// Includes every source-binder argument, even arguments erased from the
/// resulting arrow. Equal result types alone do not identify specialization.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RuntimeCallableSpecializationKey {
    source: RuntimeSemanticTypeId,
    target: RuntimeSemanticTypeId,
    arguments: RuntimeFunctionSpecializationArguments<RuntimeSemanticTypeId>,
}

impl RuntimeCallableSpecializationKey {
    pub fn from_types(
        source: &RuntimeNormalizedType,
        target: &RuntimeNormalizedType,
        arguments: &RuntimeFunctionSpecializationArguments<RuntimeNormalizedType>,
    ) -> Self {
        Self {
            source: source.identity(),
            target: target.identity(),
            arguments: RuntimeFunctionSpecializationArguments {
                types: arguments
                    .types
                    .iter()
                    .map(RuntimeNormalizedType::identity)
                    .collect(),
                const_lengths: arguments.const_lengths.clone(),
                effects: arguments.effects.clone(),
            },
        }
    }
    pub const fn source(&self) -> RuntimeSemanticTypeId {
        self.source
    }
    pub const fn target(&self) -> RuntimeSemanticTypeId {
        self.target
    }
}

/// All reachable code origins for one checked type-use demand. State IDs are
/// assigned only after the closed instance worklist reaches its fixed point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableSpecializationFact {
    key: RuntimeCallableSpecializationKey,
    source: RuntimeNormalizedType,
    target: RuntimeNormalizedType,
    arguments: RuntimeFunctionSpecializationArguments<RuntimeNormalizedType>,
    selections: BTreeMap<RuntimeProjectCallableSourceKey, RuntimeProjectCallableValueTarget>,
}

/// An expression evaluates its independently checked source exactly once,
/// then applies this program-owned specialization to obtain its final type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCallableValueSpecialization {
    source: RuntimeNormalizedType,
    key: RuntimeCallableSpecializationKey,
}

impl RuntimeCallableValueSpecialization {
    pub fn try_new(
        source: RuntimeNormalizedType,
        key: RuntimeCallableSpecializationKey,
    ) -> Result<Self, RuntimeCallableSpecializationFactError> {
        if source.identity() != key.source() || !source.scope().is_root() {
            return Err(RuntimeCallableSpecializationFactError::Source);
        }
        Ok(Self { source, key })
    }
    pub const fn source(&self) -> &RuntimeNormalizedType {
        &self.source
    }
    pub const fn key(&self) -> &RuntimeCallableSpecializationKey {
        &self.key
    }
}

impl RuntimeCallableSpecializationFact {
    pub fn try_new(
        source: RuntimeNormalizedType,
        target: RuntimeNormalizedType,
        arguments: RuntimeFunctionSpecializationArguments<RuntimeNormalizedType>,
        selections: impl IntoIterator<
            Item = (
                RuntimeProjectCallableSourceKey,
                RuntimeProjectCallableValueTarget,
            ),
        >,
    ) -> Result<Self, RuntimeCallableSpecializationFactError> {
        if !source.scope().is_root()
            || !target.scope().is_root()
            || !matches!(source.shape(), RuntimeTypeShape::Function { .. })
            || !matches!(target.shape(), RuntimeTypeShape::Function { .. })
        {
            return Err(RuntimeCallableSpecializationFactError::Source);
        }
        let mut rows = BTreeMap::new();
        for (origin, selection) in selections {
            if origin.function_type() != source.identity()
                || origin.callable() != selection.callable()
                || rows.insert(origin, selection).is_some()
            {
                return Err(RuntimeCallableSpecializationFactError::Selection);
            }
        }
        let key = RuntimeCallableSpecializationKey::from_types(&source, &target, &arguments);
        Ok(Self {
            key,
            source,
            target,
            arguments,
            selections: rows,
        })
    }
    pub const fn key(&self) -> &RuntimeCallableSpecializationKey {
        &self.key
    }
    pub const fn source(&self) -> &RuntimeNormalizedType {
        &self.source
    }
    pub const fn target(&self) -> &RuntimeNormalizedType {
        &self.target
    }
    pub const fn arguments(
        &self,
    ) -> &RuntimeFunctionSpecializationArguments<RuntimeNormalizedType> {
        &self.arguments
    }
    pub const fn selections(
        &self,
    ) -> &BTreeMap<RuntimeProjectCallableSourceKey, RuntimeProjectCallableValueTarget> {
        &self.selections
    }
    pub fn with_selection(
        mut self,
        source: RuntimeProjectCallableSourceKey,
        target: RuntimeProjectCallableValueTarget,
    ) -> Result<Self, RuntimeCallableSpecializationFactError> {
        if source.function_type() != self.source.identity()
            || source.callable() != target.callable()
            || self
                .selections
                .get(&source)
                .is_some_and(|selected| selected != &target)
        {
            return Err(RuntimeCallableSpecializationFactError::Selection);
        }
        self.selections.insert(source, target);
        Ok(self)
    }
    pub(super) fn append_normalized_types<'a>(
        &'a self,
        roots: &mut Vec<&'a RuntimeNormalizedType>,
    ) {
        roots.extend([&self.source, &self.target]);
        roots.extend(self.arguments.types.iter());
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCallableSpecializationFactError {
    #[error("callable source has an invalid rooted function or formal layout")]
    Source,
    #[error("callable specialization has a foreign, duplicate, or mistyped code-origin selection")]
    Selection,
    #[error(
        "callable source/specialization inventory repeats a key or references an absent accepted owner"
    )]
    Inventory,
    #[error(
        "callable specialization disagrees with the independently typed expression {expression:?}"
    )]
    Expression { expression: ExprId },
}

pub(super) fn admit_sources(
    input: Vec<RuntimeProjectCallableSourceFact>,
) -> Result<
    BTreeMap<RuntimeProjectCallableSourceKey, RuntimeProjectCallableSourceFact>,
    RuntimeCallableSpecializationFactError,
> {
    let mut result = BTreeMap::new();
    for fact in input {
        if result.insert(fact.key().clone(), fact).is_some() {
            return Err(RuntimeCallableSpecializationFactError::Inventory);
        }
    }
    if result
        .values()
        .any(|fact| !result.contains_key(fact.root()))
    {
        return Err(RuntimeCallableSpecializationFactError::Inventory);
    }
    Ok(result)
}

pub(super) fn admit_specializations(
    input: Vec<RuntimeCallableSpecializationFact>,
    sources: &BTreeMap<RuntimeProjectCallableSourceKey, RuntimeProjectCallableSourceFact>,
    instances: &BTreeMap<
        RuntimeProjectFunctionInstanceKey,
        super::RuntimeProjectFunctionInstanceFact,
    >,
) -> Result<
    BTreeMap<RuntimeCallableSpecializationKey, RuntimeCallableSpecializationFact>,
    RuntimeCallableSpecializationFactError,
> {
    let mut result = BTreeMap::new();
    for fact in input {
        if fact.selections().is_empty() {
            return Err(RuntimeCallableSpecializationFactError::Inventory);
        }
        for (source, target) in fact.selections() {
            let source = sources
                .get(source)
                .ok_or(RuntimeCallableSpecializationFactError::Inventory)?;
            if source.function_type() != fact.source() {
                return Err(RuntimeCallableSpecializationFactError::Inventory);
            }
            let valid = match target {
                RuntimeProjectCallableValueTarget::Closed {
                    instance: key,
                    position,
                } => instances.get(key).is_some_and(|target| {
                    source.position() == *position
                        && source.group().get() <= target.key().group().get()
                        && source.callable().declaration() == target.callable().declaration()
                }),
                RuntimeProjectCallableValueTarget::Source(key) => {
                    sources.get(key).is_some_and(|next| {
                        let RuntimeTypeShape::Function { result, .. } = fact.target().shape()
                        else {
                            return false;
                        };
                        next.root() == source.root()
                            && ((next.key() == source.key()
                                && next.function_type() == fact.target())
                                || (next.key().origin()
                                    == RuntimeProjectCallableSourceOrigin::Retained(
                                        source.key().digest(),
                                    )
                                    && ((next.position() == source.position()
                                        && next.function_type() == fact.target())
                                        || (next.group().get() == source.group().get() + 1
                                            && next.position().parameter().get() == 0
                                            && next.function_type() == result.as_ref()
                                            && matches!(
                                                source.attached(),
                                                RuntimeCallableAttachedContract::None
                                            )))))
                    })
                }
            };
            if !valid {
                return Err(RuntimeCallableSpecializationFactError::Inventory);
            }
        }
        if result.insert(fact.key().clone(), fact).is_some() {
            return Err(RuntimeCallableSpecializationFactError::Inventory);
        }
    }
    Ok(result)
}
