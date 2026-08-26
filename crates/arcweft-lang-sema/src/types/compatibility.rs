use core::convert::Infallible;

use crate::{
    effect_row::{EffectRow, EffectRowTail},
    types::{ArrayLength, TypeKind},
};

// Generic binding/Choice discovery is a compatibility-owned planning hook.
// It may fork provisional rows, but all terminal compatibility verdicts still
// go through this module's `SelectedCall` engine.
pub(super) mod binding_plan;

/// Selects the semantic contract used by the recursive type relation.
///
/// Recovery is the diagnostic-preserving relation used by the ordinary
/// checker. Selected calls and published invariants are fail-closed: a
/// recovered or unresolved node is not evidence of a successful relation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeCompatibilityPolicy {
    Recovery,
    SelectedCall,
    Invariant,
}

impl TypeCompatibilityPolicy {
    const fn rejects_recovery(self) -> bool {
        matches!(self, Self::SelectedCall | Self::Invariant)
    }
}

/// The unresolved structures that cannot be used as selected-call or
/// invariant evidence. The side is retained in the outer failure so callers
/// can map an invalid expected or actual row without string inspection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeCompatibilityForbidden {
    Error,
    Projection,
    Placeholder,
    ArrayLengthError,
    ArrayLengthInferred,
    UnknownEffectTail,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TypeCompatibilitySide {
    Expected,
    Actual,
}

/// Failure returned by the typed relation. A `false` result is an ordinary
/// structural incompatibility; `Forbidden` is reserved for a recovered or
/// unresolved node in a strict policy. The generic control error is kept
/// separate so a caller can charge the same bounded context as its enclosing
/// transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeCompatibilityFailure<E> {
    Forbidden {
        side: TypeCompatibilitySide,
        kind: TypeCompatibilityForbidden,
    },
    Control(E),
}

/// Hook used by bounded callers to charge one recursive compatibility node.
///
/// The type relation does not own a second budget. A constraint transaction
/// can implement this hook with its existing checked node/cancellation
/// context, while ordinary callers use [`NoopTypeCompatibilityControl`].
/// Strict policies deliberately charge their complete-tree prevalidation when
/// no forbidden node is found, and their subsequent compatibility traversal
/// through the same hook. A forbidden node is terminal after its own control
/// event, so the two-pass accounting remains deterministic on valid trees.
pub(crate) trait TypeCompatibilityControl {
    type Error;

    fn enter(&mut self, expected: &TypeKind, actual: &TypeKind) -> Result<(), Self::Error>;
}

/// Control used by the recovery wrapper and unbudgeted semantic checks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct NoopTypeCompatibilityControl;

impl TypeCompatibilityControl for NoopTypeCompatibilityControl {
    type Error = Infallible;

    fn enter(&mut self, _expected: &TypeKind, _actual: &TypeKind) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl TypeKind {
    /// Returns whether an earlier authoritative resolution failure prevents a
    /// second compatibility diagnostic from adding useful information.
    pub(crate) fn is_unresolved_for_compatibility(&self) -> bool {
        matches!(self, Self::Named(name) if name == "_") || matches!(self, Self::Error(_))
    }

    /// Recovery compatibility retained for existing semantic checking.
    ///
    /// Strict callers must use [`Self::accepts_with`] and name their policy;
    /// this method deliberately cannot silently publish strict success.
    pub(crate) fn accepts(&self, actual: &Self) -> bool {
        let mut control = NoopTypeCompatibilityControl;
        match self.accepts_with(actual, TypeCompatibilityPolicy::Recovery, &mut control) {
            Ok(accepted) => accepted,
            Err(TypeCompatibilityFailure::Forbidden { .. }) => false,
            Err(TypeCompatibilityFailure::Control(error)) => match error {},
        }
    }

    /// Runs the one recursive compatibility engine under a typed policy.
    pub(crate) fn accepts_with<C>(
        &self,
        actual: &Self,
        policy: TypeCompatibilityPolicy,
        control: &mut C,
    ) -> Result<bool, TypeCompatibilityFailure<C::Error>>
    where
        C: TypeCompatibilityControl,
    {
        if policy.rejects_recovery() {
            let expected_forbidden =
                validate_strict_tree(self, TypeCompatibilitySide::Expected, control)?;
            if let Some(kind) = expected_forbidden {
                return Err(TypeCompatibilityFailure::Forbidden {
                    side: TypeCompatibilitySide::Expected,
                    kind,
                });
            }
            let actual_forbidden =
                validate_strict_tree(actual, TypeCompatibilitySide::Actual, control)?;
            if let Some(kind) = actual_forbidden {
                return Err(TypeCompatibilityFailure::Forbidden {
                    side: TypeCompatibilitySide::Actual,
                    kind,
                });
            }
            accepts_node(self, actual, policy, control, true, false)
        } else {
            accepts_node(self, actual, policy, control, true, false)
        }
    }
}

fn accepts_node<C>(
    expected: &TypeKind,
    actual: &TypeKind,
    policy: TypeCompatibilityPolicy,
    control: &mut C,
    meter: bool,
    structural: bool,
) -> Result<bool, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    // Projection subjects use the same engine in a structural subrelation:
    // local headers still charge here, but Recovery widening is disabled.
    if meter {
        control
            .enter(expected, actual)
            .map_err(TypeCompatibilityFailure::Control)?;
    }

    if policy.rejects_recovery() {
        if let Some(kind) = forbidden_here(expected) {
            return Err(TypeCompatibilityFailure::Forbidden {
                side: TypeCompatibilitySide::Expected,
                kind,
            });
        }
        if let Some(kind) = forbidden_here(actual) {
            return Err(TypeCompatibilityFailure::Forbidden {
                side: TypeCompatibilitySide::Actual,
                kind,
            });
        }
    }

    if matches!(policy, TypeCompatibilityPolicy::Recovery) && !structural {
        if matches!(expected, TypeKind::Error(_)) || matches!(actual, TypeKind::Error(_)) {
            return Ok(true);
        }
        if matches!(expected, TypeKind::Named(name) if name == "_") {
            return Ok(true);
        }
    }

    // Never is the bottom type in every directional policy. A structural
    // subrelation only accepts its local Never/ Never header below.
    if !structural && matches!(actual, TypeKind::Never) {
        return Ok(true);
    }
    if !structural && matches!(expected, TypeKind::Never) {
        return Ok(false);
    }

    if let Some(compatible) =
        nominal_types_compatible(expected, actual, policy, control, meter, structural)?
    {
        return Ok(compatible);
    }

    match (expected, actual) {
        (TypeKind::Bool, TypeKind::Bool)
        | (TypeKind::I8, TypeKind::I8)
        | (TypeKind::I16, TypeKind::I16)
        | (TypeKind::I32, TypeKind::I32)
        | (TypeKind::I64, TypeKind::I64)
        | (TypeKind::I128, TypeKind::I128)
        | (TypeKind::ISize, TypeKind::ISize)
        | (TypeKind::U8, TypeKind::U8)
        | (TypeKind::U16, TypeKind::U16)
        | (TypeKind::U32, TypeKind::U32)
        | (TypeKind::U64, TypeKind::U64)
        | (TypeKind::U128, TypeKind::U128)
        | (TypeKind::USize, TypeKind::USize)
        | (TypeKind::F32, TypeKind::F32)
        | (TypeKind::F64, TypeKind::F64)
        | (TypeKind::String, TypeKind::String)
        | (TypeKind::Char, TypeKind::Char)
        | (TypeKind::Bytes, TypeKind::Bytes)
        | (TypeKind::TextCluster, TypeKind::TextCluster)
        | (TypeKind::Duration, TypeKind::Duration)
        | (TypeKind::Progress, TypeKind::Progress)
        | (TypeKind::LineContext, TypeKind::LineContext)
        | (TypeKind::CueHandle, TypeKind::CueHandle)
        | (TypeKind::VoiceHandle, TypeKind::VoiceHandle)
        | (TypeKind::DisplayText, TypeKind::DisplayText)
        | (TypeKind::DebugStatePath, TypeKind::DebugStatePath)
        | (TypeKind::ObservationFieldPath, TypeKind::ObservationFieldPath)
        | (TypeKind::Predicate, TypeKind::Predicate)
        | (TypeKind::Observation, TypeKind::Observation)
        | (TypeKind::ObservedObject, TypeKind::ObservedObject)
        | (TypeKind::AgentBBox, TypeKind::AgentBBox)
        | (TypeKind::ActionTarget, TypeKind::ActionTarget)
        | (TypeKind::ActionResult, TypeKind::ActionResult)
        | (TypeKind::DataFormat, TypeKind::DataFormat)
        | (TypeKind::DataShape, TypeKind::DataShape)
        | (TypeKind::AgentEntityMetadata, TypeKind::AgentEntityMetadata)
        | (TypeKind::AgentSourceAnchor, TypeKind::AgentSourceAnchor)
        | (TypeKind::AgentProjectGraphNeighborhood, TypeKind::AgentProjectGraphNeighborhood)
        | (TypeKind::AgentProjectGraphSymbol, TypeKind::AgentProjectGraphSymbol)
        | (TypeKind::AgentProjectGraphEdge, TypeKind::AgentProjectGraphEdge)
        | (TypeKind::CaptureTarget, TypeKind::CaptureTarget)
        | (TypeKind::CaptureRef, TypeKind::CaptureRef)
        | (TypeKind::AgentResource, TypeKind::AgentResource)
        | (TypeKind::AgentResourceBody, TypeKind::AgentResourceBody)
        | (TypeKind::RagContextPack, TypeKind::RagContextPack)
        | (TypeKind::FocusPatch, TypeKind::FocusPatch)
        | (TypeKind::ViewValue, TypeKind::ViewValue)
        | (TypeKind::Unit, TypeKind::Unit)
        | (TypeKind::Never, TypeKind::Never) => Ok(true),
        (TypeKind::StageApi(expected), TypeKind::StageApi(actual)) => Ok(expected == actual),
        (
            TypeKind::StageActorHandle(super::StageActorHandleType::Any),
            TypeKind::StageActorHandle(super::StageActorHandleType::Exact(_)),
        ) if !structural => Ok(true),
        (TypeKind::StageActorHandle(expected), TypeKind::StageActorHandle(actual)) => {
            Ok(expected == actual)
        }
        (TypeKind::CharacterDialogue(expected), TypeKind::CharacterDialogue(actual)) => {
            Ok(if structural {
                expected.character() == actual.character()
            } else {
                expected.accepts(actual)
            })
        }
        (TypeKind::CharacterNominal(expected), TypeKind::CharacterNominal(actual)) => {
            Ok(expected.family() == actual.family()
                && expected.character() == actual.character()
                && match (expected.part(), actual.part()) {
                    (Some(expected), Some(actual)) => expected == actual,
                    _ => true,
                })
        }
        (TypeKind::Bytes, TypeKind::Vec(inner) | TypeKind::Slice(inner) | TypeKind::Seq(inner))
            if !structural =>
        {
            if !accepts_node(inner, inner, policy, control, meter, structural)? {
                return Ok(false);
            }
            Ok(matches!(inner.as_ref(), TypeKind::U8))
        }
        (TypeKind::ActionName, TypeKind::ActionName) => Ok(true),
        (TypeKind::ActionName, TypeKind::String | TypeKind::Named(_)) if !structural => Ok(true),
        (TypeKind::AgentValue, TypeKind::AgentValue) => Ok(true),
        (TypeKind::AgentValue, actual) if !structural => {
            is_agent_value_type(actual, policy, control, true, meter)
        }
        (TypeKind::AgentBuiltin(expected), TypeKind::AgentBuiltin(actual)) => {
            Ok(expected == actual)
        }
        (TypeKind::Error(expected), TypeKind::Error(actual)) => Ok(expected == actual),
        (TypeKind::CharacterPatch(expected), TypeKind::CharacterPatch(actual)) => {
            Ok(expected == actual)
        }
        (TypeKind::GenericParam(expected), TypeKind::GenericParam(actual)) => {
            Ok(expected == actual)
        }
        (
            TypeKind::Handle {
                name: expected_name,
                lifetime: expected_lifetime,
                state: expected_state,
                must_drop: expected_must_drop,
            },
            TypeKind::Handle {
                name: actual_name,
                lifetime: actual_lifetime,
                state: actual_state,
                must_drop: actual_must_drop,
            },
        ) => Ok(expected_name == actual_name
            && expected_lifetime == actual_lifetime
            && expected_state == actual_state
            && expected_must_drop == actual_must_drop),
        (TypeKind::Named(expected), TypeKind::Named(actual)) => Ok(expected == actual),
        (TypeKind::Ref(expected), TypeKind::Ref(actual)) if expected.kind() == actual.kind() => {
            // A payload-free family constraint accepts a retained
            // specialization. Structural projection subjects require exact
            // payload presence and recurse through the same relation engine.
            match (expected.value(), actual.value()) {
                (Some(expected), Some(actual)) => {
                    accepts_node(expected, actual, policy, control, meter, structural)
                }
                (None, Some(_)) if !structural => Ok(true),
                (None, None) => Ok(true),
                (Some(_), None) | (None, Some(_)) => Ok(false),
            }
        }
        (
            TypeKind::IteratorState {
                family: expected_family,
                item: expected_item,
            },
            TypeKind::IteratorState {
                family: actual_family,
                item: actual_item,
            },
        ) if expected_family == actual_family => accepts_node(
            expected_item,
            actual_item,
            policy,
            control,
            meter,
            structural,
        ),
        (
            TypeKind::Map {
                kind: expected_kind,
                key: expected_key,
                value: expected_value,
            },
            TypeKind::Map {
                kind: actual_kind,
                key: actual_key,
                value: actual_value,
            },
        ) if expected_kind == actual_kind => {
            if !accepts_node(expected_key, actual_key, policy, control, meter, structural)? {
                return Ok(false);
            }
            accepts_node(
                expected_value,
                actual_value,
                policy,
                control,
                meter,
                structural,
            )
        }
        (
            TypeKind::BorrowRef {
                kind: expected_kind,
                lifetime: expected_lifetime,
                inner: expected_inner,
            },
            TypeKind::BorrowRef {
                kind: actual_kind,
                lifetime: actual_lifetime,
                inner: actual_inner,
            },
        ) if expected_kind == actual_kind && expected_lifetime == actual_lifetime => accepts_node(
            expected_inner,
            actual_inner,
            policy,
            control,
            meter,
            structural,
        ),
        (TypeKind::Need(expected), TypeKind::Need(actual))
        | (TypeKind::ThreadHandle(expected), TypeKind::ThreadHandle(actual))
        | (TypeKind::Shared(expected), TypeKind::Shared(actual)) => {
            accepts_node(expected, actual, policy, control, meter, structural)
        }
        (
            TypeKind::Stream {
                item: expected_item,
                error: expected_error,
            },
            TypeKind::Stream {
                item: actual_item,
                error: actual_error,
            },
        ) => {
            if !accepts_node(
                expected_item,
                actual_item,
                policy,
                control,
                meter,
                structural,
            )? {
                return Ok(false);
            }
            accepts_node(
                expected_error,
                actual_error,
                policy,
                control,
                meter,
                structural,
            )
        }
        (TypeKind::Choice(expected_alternatives), TypeKind::Choice(actual_alternatives))
            if structural =>
        {
            if expected_alternatives.len() != actual_alternatives.len() {
                return Ok(false);
            }
            for (expected, actual) in expected_alternatives.iter().zip(actual_alternatives) {
                if !accepts_node(expected, actual, policy, control, meter, true)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (TypeKind::Choice(alternatives), TypeKind::Choice(actual_alternatives)) if !structural => {
            if alternatives.len() == actual_alternatives.len() {
                let mut exact = true;
                for (expected, actual) in alternatives.iter().zip(actual_alternatives) {
                    if !accepts_node(expected, actual, policy, control, meter, true)? {
                        exact = false;
                        break;
                    }
                }
                if exact {
                    return Ok(true);
                }
            }
            let mut accepted = true;
            for actual in actual_alternatives {
                if !choice_has_unique_injection(alternatives, actual, policy, control, meter)? {
                    accepted = false;
                    break;
                }
            }
            Ok(accepted)
        }
        (TypeKind::Choice(alternatives), actual) if !structural => {
            choice_has_unique_injection(alternatives, actual, policy, control, meter)
        }
        (expected, TypeKind::Choice(alternatives)) if !structural => {
            for actual in alternatives {
                if !accepts_node(expected, actual, policy, control, meter, structural)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            TypeKind::Result {
                ok: expected_ok,
                error: expected_error,
            },
            TypeKind::Result {
                ok: actual_ok,
                error: actual_error,
            },
        ) => {
            if !accepts_node(expected_ok, actual_ok, policy, control, meter, structural)? {
                return Ok(false);
            }
            if accepts_node(
                expected_error,
                actual_error,
                policy,
                control,
                meter,
                structural,
            )? {
                Ok(true)
            } else {
                Ok(matches!(
                    (policy, actual_error.as_ref()),
                    (TypeCompatibilityPolicy::Recovery, TypeKind::Named(name))
                        if !structural && name == "_"
                ))
            }
        }
        (TypeKind::Option(expected), TypeKind::Option(actual)) => {
            if accepts_node(expected, actual, policy, control, meter, structural)? {
                Ok(true)
            } else {
                Ok(matches!(
                    (policy, actual.as_ref()),
                    (TypeCompatibilityPolicy::Recovery, TypeKind::Named(name))
                        if !structural && name == "_"
                ))
            }
        }
        (TypeKind::DialogueLine(expected), TypeKind::DialogueLine(actual))
        | (TypeKind::Probe(expected), TypeKind::Probe(actual))
        | (TypeKind::Vec(expected), TypeKind::Vec(actual))
        | (TypeKind::Seq(expected), TypeKind::Seq(actual))
        | (TypeKind::Slice(expected), TypeKind::Slice(actual))
        | (TypeKind::Range(expected), TypeKind::Range(actual)) => {
            accepts_node(expected, actual, policy, control, meter, structural)
        }
        (
            TypeKind::Array {
                item: expected_item,
                len: expected_len,
            },
            TypeKind::Array {
                item: actual_item,
                len: actual_len,
            },
        ) => {
            let lengths_compatible = if structural {
                expected_len == actual_len
            } else {
                match policy {
                    TypeCompatibilityPolicy::Recovery => match expected_len {
                        ArrayLength::Const(expected) => {
                            matches!(actual_len, ArrayLength::Const(actual) if expected == actual)
                                || matches!(actual_len, ArrayLength::Error(_))
                        }
                        ArrayLength::Generic(_) | ArrayLength::Error(_) | ArrayLength::Inferred => {
                            true
                        }
                    },
                    TypeCompatibilityPolicy::SelectedCall | TypeCompatibilityPolicy::Invariant => {
                        matches!(
                            (expected_len, actual_len),
                            (ArrayLength::Const(expected), ArrayLength::Const(actual))
                                if expected == actual
                        ) || matches!(
                            (expected_len, actual_len),
                            (ArrayLength::Generic(expected), ArrayLength::Generic(actual))
                                if expected == actual
                        )
                    }
                }
            };
            if !lengths_compatible {
                return Ok(false);
            }
            accepts_node(
                expected_item,
                actual_item,
                policy,
                control,
                meter,
                structural,
            )
        }
        (TypeKind::Tuple(expected), TypeKind::Tuple(actual)) => {
            if expected.len() != actual.len() {
                return Ok(false);
            }
            for (expected, actual) in expected.iter().zip(actual) {
                if !accepts_node(expected, actual, policy, control, meter, structural)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (
            TypeKind::Function {
                params: expected_params,
                return_type: expected_return,
                effects: expected_effects,
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
                effects: actual_effects,
            },
        ) => {
            if expected_params.len() != actual_params.len()
                || if structural {
                    expected_effects != actual_effects
                } else {
                    !effect_rows_compatible(expected_effects, actual_effects)
                }
            {
                return Ok(false);
            }
            // Function inputs are contravariant: the supplied function must
            // accept every value admitted by the expected parameter type.
            for (expected, actual) in expected_params.iter().zip(actual_params) {
                if !accepts_node(actual, expected, policy, control, meter, structural)? {
                    return Ok(false);
                }
            }
            accepts_node(
                expected_return,
                actual_return,
                policy,
                control,
                meter,
                structural,
            )
        }
        (
            TypeKind::Projection {
                subject: expected_subject,
                trait_name: expected_trait,
                assoc: expected_assoc,
            },
            TypeKind::Projection {
                subject: actual_subject,
                trait_name: actual_trait,
                assoc: actual_assoc,
            },
        ) if expected_trait == actual_trait && expected_assoc == actual_assoc => accepts_node(
            expected_subject,
            actual_subject,
            policy,
            control,
            meter,
            true,
        ),
        _ => Ok(false),
    }
}

fn nominal_types_compatible<C>(
    expected: &TypeKind,
    actual: &TypeKind,
    policy: TypeCompatibilityPolicy,
    control: &mut C,
    meter: bool,
    structural: bool,
) -> Result<Option<bool>, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    let result = match (expected, actual) {
        (TypeKind::ProjectNominal(expected), TypeKind::ProjectNominal(actual)) => Some(
            expected.declaration() == actual.declaration()
                && nominal_arguments_compatible(
                    expected.arguments(),
                    actual.arguments(),
                    policy,
                    control,
                    meter,
                    structural,
                )?,
        ),
        (TypeKind::AcceptedNominal(expected), TypeKind::AcceptedNominal(actual)) => Some(
            expected.declaration() == actual.declaration()
                && nominal_arguments_compatible(
                    expected.arguments(),
                    actual.arguments(),
                    policy,
                    control,
                    meter,
                    structural,
                )?,
        ),
        (TypeKind::OpenNominal(expected), TypeKind::OpenNominal(actual)) => Some(
            expected.rule() == actual.rule()
                && expected.path() == actual.path()
                && nominal_arguments_compatible(
                    expected.arguments(),
                    actual.arguments(),
                    policy,
                    control,
                    meter,
                    structural,
                )?,
        ),
        _ => None,
    };
    Ok(result)
}

fn nominal_arguments_compatible<C>(
    expected: &[TypeKind],
    actual: &[TypeKind],
    policy: TypeCompatibilityPolicy,
    control: &mut C,
    meter: bool,
    structural: bool,
) -> Result<bool, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        if !accepts_node(expected, actual, policy, control, meter, structural)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn effect_rows_compatible(expected: &EffectRow, actual: &EffectRow) -> bool {
    match (expected.tail(), actual.tail()) {
        (EffectRowTail::Unknown, _) | (_, EffectRowTail::Unknown) => true,
        (EffectRowTail::Closed, EffectRowTail::Closed)
        | (EffectRowTail::Variable(_), EffectRowTail::Closed | EffectRowTail::Variable(_)) => {
            actual
                .concrete()
                .effects_not_covered_by(expected.concrete())
                .is_empty()
        }
        (EffectRowTail::Closed, EffectRowTail::Variable(_)) => false,
    }
}

fn is_agent_value_type<C>(
    ty: &TypeKind,
    policy: TypeCompatibilityPolicy,
    control: &mut C,
    already_entered: bool,
    meter: bool,
) -> Result<bool, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    if !already_entered && meter {
        control
            .enter(&TypeKind::AgentValue, ty)
            .map_err(TypeCompatibilityFailure::Control)?;
    }
    if !already_entered && policy.rejects_recovery() {
        if let Some(kind) = forbidden_here(ty) {
            return Err(TypeCompatibilityFailure::Forbidden {
                side: TypeCompatibilitySide::Actual,
                kind,
            });
        }
    }
    match ty {
        TypeKind::Bool
        | TypeKind::I8
        | TypeKind::I16
        | TypeKind::I32
        | TypeKind::I64
        | TypeKind::I128
        | TypeKind::ISize
        | TypeKind::U8
        | TypeKind::U16
        | TypeKind::U32
        | TypeKind::U64
        | TypeKind::U128
        | TypeKind::USize
        | TypeKind::F32
        | TypeKind::F64
        | TypeKind::String
        | TypeKind::Char
        | TypeKind::Bytes
        | TypeKind::Duration
        | TypeKind::Progress
        | TypeKind::DisplayText
        | TypeKind::ActionName
        | TypeKind::AgentValue
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody
        | TypeKind::AgentBuiltin(_) => Ok(true),
        TypeKind::Ref(_) => Ok(true),
        TypeKind::Error(_) => Ok(matches!(policy, TypeCompatibilityPolicy::Recovery)),
        TypeKind::Vec(inner)
        | TypeKind::Array { item: inner, .. }
        | TypeKind::Slice(inner)
        | TypeKind::Range(inner)
        | TypeKind::Option(inner) => is_agent_value_type(inner, policy, control, false, meter),
        TypeKind::Map { key, value, .. } => {
            if !accepts_node(&TypeKind::String, key, policy, control, meter, false)? {
                return Ok(false);
            }
            is_agent_value_type(value, policy, control, false, meter)
        }
        TypeKind::Choice(alternatives) => {
            for alternative in alternatives {
                if !is_agent_value_type(alternative, policy, control, false, meter)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn choice_has_unique_injection<C>(
    alternatives: &[TypeKind],
    actual: &TypeKind,
    policy: TypeCompatibilityPolicy,
    control: &mut C,
    meter: bool,
) -> Result<bool, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    let mut matches = 0_u8;
    for alternative in alternatives {
        if matches!(policy, TypeCompatibilityPolicy::Recovery)
            && matches!(alternative, TypeKind::Error(_))
        {
            continue;
        }
        if accepts_node(alternative, actual, policy, control, meter, false)? {
            matches = matches.saturating_add(1);
            if matches > 1 {
                return Ok(false);
            }
        }
    }
    if matches == 1 {
        Ok(true)
    } else {
        Ok(matches == 0
            && matches!(policy, TypeCompatibilityPolicy::Recovery)
            && alternatives
                .iter()
                .any(|alternative| matches!(alternative, TypeKind::Error(_))))
    }
}

fn forbidden_here(ty: &TypeKind) -> Option<TypeCompatibilityForbidden> {
    match ty {
        TypeKind::Error(_) => Some(TypeCompatibilityForbidden::Error),
        TypeKind::Projection { .. } => Some(TypeCompatibilityForbidden::Projection),
        TypeKind::Named(name) if name == "_" => Some(TypeCompatibilityForbidden::Placeholder),
        TypeKind::Array { len, .. } => match len {
            ArrayLength::Error(_) => Some(TypeCompatibilityForbidden::ArrayLengthError),
            ArrayLength::Inferred => Some(TypeCompatibilityForbidden::ArrayLengthInferred),
            ArrayLength::Const(_) | ArrayLength::Generic(_) => None,
        },
        TypeKind::Function { effects, .. } if matches!(effects.tail(), EffectRowTail::Unknown) => {
            Some(TypeCompatibilityForbidden::UnknownEffectTail)
        }
        _ => None,
    }
}

fn validate_strict_tree<C>(
    ty: &TypeKind,
    side: TypeCompatibilitySide,
    control: &mut C,
) -> Result<Option<TypeCompatibilityForbidden>, TypeCompatibilityFailure<C::Error>>
where
    C: TypeCompatibilityControl,
{
    control
        .enter(ty, ty)
        .map_err(TypeCompatibilityFailure::Control)?;
    if let Some(kind) = forbidden_here(ty) {
        return Ok(Some(kind));
    }

    match ty {
        TypeKind::Ref(entity) => {
            if let Some(value) = entity.value() {
                return validate_strict_tree(value, side, control);
            }
        }
        TypeKind::Range(value)
        | TypeKind::Probe(value)
        | TypeKind::Vec(value)
        | TypeKind::Slice(value)
        | TypeKind::Seq(value)
        | TypeKind::Need(value)
        | TypeKind::Option(value)
        | TypeKind::DialogueLine(value)
        | TypeKind::ThreadHandle(value)
        | TypeKind::Shared(value) => {
            return validate_strict_tree(value, side, control);
        }
        TypeKind::IteratorState { item, .. } => {
            return validate_strict_tree(item, side, control);
        }
        TypeKind::Array { item, .. } => {
            return validate_strict_tree(item, side, control);
        }
        TypeKind::Map { key, value, .. } => {
            if let Some(kind) = validate_strict_tree(key, side, control)? {
                return Ok(Some(kind));
            }
            return validate_strict_tree(value, side, control);
        }
        TypeKind::BorrowRef { inner, .. } => {
            return validate_strict_tree(inner, side, control);
        }
        TypeKind::Stream { item, error } | TypeKind::Result { ok: item, error } => {
            if let Some(kind) = validate_strict_tree(item, side, control)? {
                return Ok(Some(kind));
            }
            return validate_strict_tree(error, side, control);
        }
        TypeKind::Function {
            params,
            return_type,
            ..
        } => {
            for parameter in params {
                if let Some(kind) = validate_strict_tree(parameter, side, control)? {
                    return Ok(Some(kind));
                }
            }
            return validate_strict_tree(return_type, side, control);
        }
        TypeKind::ProjectNominal(nominal) => {
            for argument in nominal.arguments() {
                if let Some(kind) = validate_strict_tree(argument, side, control)? {
                    return Ok(Some(kind));
                }
            }
        }
        TypeKind::AcceptedNominal(nominal) => {
            for argument in nominal.arguments() {
                if let Some(kind) = validate_strict_tree(argument, side, control)? {
                    return Ok(Some(kind));
                }
            }
        }
        TypeKind::OpenNominal(nominal) => {
            for argument in nominal.arguments() {
                if let Some(kind) = validate_strict_tree(argument, side, control)? {
                    return Ok(Some(kind));
                }
            }
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            for item in items {
                if let Some(kind) = validate_strict_tree(item, side, control)? {
                    return Ok(Some(kind));
                }
            }
        }
        TypeKind::Projection { subject, .. } => {
            return validate_strict_tree(subject, side, control);
        }
        _ => {}
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use arcweft_character::id::CharacterId;

    use super::{
        NoopTypeCompatibilityControl, TypeCompatibilityControl, TypeCompatibilityFailure,
        TypeCompatibilityPolicy, TypeKind,
    };
    use crate::types::{
        ArrayLength, DetachedGenericOwnerId, EntityKind, GenericConstParameterId,
        GenericParameterOwnerId, StageActorHandleType, TypePoisonId,
    };

    fn accepts_with(
        expected: &TypeKind,
        actual: &TypeKind,
        policy: TypeCompatibilityPolicy,
    ) -> Result<bool, TypeCompatibilityFailure<core::convert::Infallible>> {
        let mut control = NoopTypeCompatibilityControl;
        expected.accepts_with(actual, policy, &mut control)
    }

    #[derive(Default)]
    struct CountingControl {
        entries: usize,
    }

    impl TypeCompatibilityControl for CountingControl {
        type Error = core::convert::Infallible;

        fn enter(&mut self, _expected: &TypeKind, _actual: &TypeKind) -> Result<(), Self::Error> {
            self.entries += 1;
            Ok(())
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ControlFailure;

    struct FailingControl {
        entries: usize,
        fail_at: usize,
    }

    impl TypeCompatibilityControl for FailingControl {
        type Error = ControlFailure;

        fn enter(&mut self, _expected: &TypeKind, _actual: &TypeKind) -> Result<(), Self::Error> {
            self.entries += 1;
            (self.entries != self.fail_at)
                .then_some(())
                .ok_or(ControlFailure)
        }
    }

    #[derive(Default)]
    struct RecordingControl {
        entries: usize,
        events: Vec<(TypeKind, TypeKind)>,
        limit: Option<usize>,
    }

    impl RecordingControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                limit: Some(limit),
                ..Self::default()
            }
        }
    }

    impl TypeCompatibilityControl for RecordingControl {
        type Error = ControlFailure;

        fn enter(&mut self, expected: &TypeKind, actual: &TypeKind) -> Result<(), Self::Error> {
            self.entries += 1;
            self.events.push((expected.clone(), actual.clone()));
            if self.limit.is_some_and(|limit| self.entries > limit) {
                Err(ControlFailure)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn erased_stage_actor_handle_accepts_exact_without_erasing_exact_expectations() {
        let alice = TypeKind::StageActorHandle(StageActorHandleType::Exact(
            CharacterId::try_new("character.alice").expect("Character identity"),
        ));
        let bob = TypeKind::StageActorHandle(StageActorHandleType::Exact(
            CharacterId::try_new("character.bob").expect("Character identity"),
        ));
        let any = TypeKind::StageActorHandle(StageActorHandleType::Any);

        assert!(any.accepts(&alice));
        assert!(!alice.accepts(&any));
        assert!(!alice.accepts(&bob));
        assert!(alice.first_mismatch(&any).is_some());
    }

    #[test]
    fn family_entity_reference_accepts_payload_specialization() {
        let family = TypeKind::entity_ref(EntityKind::Signal);
        let typed = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);

        assert!(family.accepts(&typed));
        assert!(!typed.accepts(&family));
    }

    #[test]
    fn typed_entity_reference_requires_compatible_family_and_payload() {
        let expected = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);
        let matching = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Bool);
        let wrong_payload = TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::String);
        let wrong_family = TypeKind::entity_ref_with_value(EntityKind::Metric, TypeKind::Bool);

        assert!(expected.accepts(&matching));
        assert!(!expected.accepts(&wrong_payload));
        assert!(!expected.accepts(&wrong_family));
    }

    #[test]
    fn recovery_recurses_through_all_same_constructor_children() {
        use arcweft_lang_syntax::reference::BorrowKind;

        let iterator = |item| TypeKind::IteratorState {
            family: crate::types::IteratorStateKind::Seq,
            item: Box::new(item),
        };
        let map = |value| TypeKind::Map {
            kind: crate::types::MapKind::Ordered,
            key: Box::new(TypeKind::String),
            value: Box::new(value),
        };
        let borrow = |inner| TypeKind::BorrowRef {
            kind: BorrowKind::Shared,
            lifetime: Some(crate::types::LifetimeScopeKind::Flow),
            inner: Box::new(inner),
        };
        let cases = [
            (iterator(TypeKind::I32), iterator(TypeKind::Never)),
            (map(TypeKind::I32), map(TypeKind::Never)),
            (borrow(TypeKind::I32), borrow(TypeKind::Never)),
            (
                TypeKind::Need(Box::new(TypeKind::I32)),
                TypeKind::Need(Box::new(TypeKind::Never)),
            ),
            (
                TypeKind::Stream {
                    item: Box::new(TypeKind::I32),
                    error: Box::new(TypeKind::String),
                },
                TypeKind::Stream {
                    item: Box::new(TypeKind::Never),
                    error: Box::new(TypeKind::String),
                },
            ),
            (
                TypeKind::ThreadHandle(Box::new(TypeKind::I32)),
                TypeKind::ThreadHandle(Box::new(TypeKind::Never)),
            ),
            (
                TypeKind::Shared(Box::new(TypeKind::I32)),
                TypeKind::Shared(Box::new(TypeKind::Never)),
            ),
            (
                TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::I32),
                TypeKind::entity_ref_with_value(EntityKind::Signal, TypeKind::Never),
            ),
        ];
        for (expected, actual) in cases {
            assert!(expected.accepts(&actual));
        }
        assert!(!iterator(TypeKind::I32).accepts(&iterator(TypeKind::String)));
        assert!(!map(TypeKind::I32).accepts(&map(TypeKind::String)));
        assert!(!borrow(TypeKind::I32).accepts(&borrow(TypeKind::String)));
    }

    #[test]
    fn poison_inside_recovered_shapes_does_not_create_follow_on_mismatches() {
        let poison = TypeKind::Error(TypePoisonId::from_index(3));

        assert!(TypeKind::Vec(Box::new(TypeKind::I32)).accepts(&TypeKind::Vec(Box::new(poison))));
        assert!(
            TypeKind::AgentValue.accepts(&TypeKind::Vec(Box::new(TypeKind::Error(
                TypePoisonId::from_index(4)
            ))))
        );
        assert!(
            TypeKind::Choice(vec![
                TypeKind::Error(TypePoisonId::from_index(5)),
                TypeKind::I32,
            ])
            .accepts(&TypeKind::I32)
        );
    }

    #[test]
    fn strict_policies_reject_nested_recovery_nodes() {
        let poison = TypeKind::Error(TypePoisonId::from_index(6));
        let nested = TypeKind::Vec(Box::new(poison));
        assert!(matches!(
            accepts_with(
                &TypeKind::Vec(Box::new(TypeKind::I32)),
                &nested,
                TypeCompatibilityPolicy::SelectedCall
            ),
            Err(TypeCompatibilityFailure::Forbidden { .. })
        ));
        assert!(matches!(
            accepts_with(
                &TypeKind::Vec(Box::new(TypeKind::I32)),
                &nested,
                TypeCompatibilityPolicy::Invariant
            ),
            Err(TypeCompatibilityFailure::Forbidden { .. })
        ));
        assert!(TypeKind::Vec(Box::new(TypeKind::I32)).accepts(&nested));
    }

    #[test]
    fn strict_prevalidation_visits_both_complete_trees_once() {
        let expected = TypeKind::Vec(Box::new(TypeKind::I32));
        let actual = TypeKind::Vec(Box::new(TypeKind::String));
        let mut control = CountingControl::default();
        assert!(
            !expected
                .accepts_with(&actual, TypeCompatibilityPolicy::SelectedCall, &mut control)
                .expect("strict structural mismatch")
        );
        // Four prevalidation visits (two nodes per tree) plus the two
        // compatibility-pair visits are charged deterministically.
        assert_eq!(control.entries, 6);

        let mut recovery_control = CountingControl::default();
        assert!(
            expected
                .accepts_with(
                    &expected,
                    TypeCompatibilityPolicy::Recovery,
                    &mut recovery_control
                )
                .expect("recovery exact relation")
        );
        assert_eq!(recovery_control.entries, 2);
    }

    #[test]
    fn equal_composites_charge_every_relation_child_for_recovery_and_strict() {
        let ty = TypeKind::Tuple(vec![
            TypeKind::Vec(Box::new(TypeKind::I32)),
            TypeKind::Map {
                kind: crate::types::MapKind::Ordered,
                key: Box::new(TypeKind::String),
                value: Box::new(TypeKind::Option(Box::new(TypeKind::Bool))),
            },
        ]);
        let relation_nodes = 7;

        let mut recovery = RecordingControl::default();
        assert!(
            ty.accepts_with(&ty, TypeCompatibilityPolicy::Recovery, &mut recovery)
                .expect("metered recovery exact relation")
        );
        assert_eq!(recovery.entries, relation_nodes);
        assert_eq!(recovery.events.len(), relation_nodes);
        assert!(
            recovery
                .events
                .iter()
                .all(|(expected, actual)| expected == actual)
        );

        for policy in [
            TypeCompatibilityPolicy::SelectedCall,
            TypeCompatibilityPolicy::Invariant,
        ] {
            let strict_nodes = relation_nodes * 3;
            let mut strict = RecordingControl::default();
            assert!(
                ty.accepts_with(&ty, policy, &mut strict)
                    .expect("strict exact relation")
            );
            assert_eq!(strict.entries, strict_nodes);
            assert_eq!(strict.events.len(), strict_nodes);

            let mut exact_limit = RecordingControl::with_limit(strict_nodes);
            assert!(
                ty.accepts_with(&ty, policy, &mut exact_limit)
                    .expect("exact strict meter limit")
            );
            assert_eq!(exact_limit.entries, strict_nodes);

            let mut one_over = RecordingControl::with_limit(strict_nodes - 1);
            assert!(matches!(
                ty.accepts_with(&ty, policy, &mut one_over),
                Err(TypeCompatibilityFailure::Control(ControlFailure))
            ));
            assert_eq!(one_over.entries, strict_nodes);
        }

        let mut exact_limit = RecordingControl::with_limit(relation_nodes);
        assert!(
            ty.accepts_with(&ty, TypeCompatibilityPolicy::Recovery, &mut exact_limit)
                .expect("exact recovery meter limit")
        );
        assert_eq!(exact_limit.entries, relation_nodes);

        let mut one_over = RecordingControl::with_limit(relation_nodes - 1);
        assert!(matches!(
            ty.accepts_with(&ty, TypeCompatibilityPolicy::Recovery, &mut one_over),
            Err(TypeCompatibilityFailure::Control(ControlFailure))
        ));
        assert_eq!(one_over.entries, relation_nodes);
    }

    #[test]
    fn bytes_widening_charges_inner_relation_for_recovery_and_strict() {
        let expected = TypeKind::Bytes;
        let actual = TypeKind::Vec(Box::new(TypeKind::U8));

        let mut recovery = RecordingControl::default();
        assert!(
            expected
                .accepts_with(&actual, TypeCompatibilityPolicy::Recovery, &mut recovery)
                .expect("metered recovery bytes widening")
        );
        assert_eq!(recovery.entries, 2);
        assert_eq!(recovery.events[1], (TypeKind::U8, TypeKind::U8));

        for policy in [
            TypeCompatibilityPolicy::SelectedCall,
            TypeCompatibilityPolicy::Invariant,
        ] {
            let strict_nodes = 5;
            let mut strict = RecordingControl::default();
            assert!(
                expected
                    .accepts_with(&actual, policy, &mut strict)
                    .expect("strict bytes widening")
            );
            assert_eq!(strict.entries, strict_nodes);
            assert_eq!(strict.events.last(), Some(&(TypeKind::U8, TypeKind::U8)));

            let mut exact_limit = RecordingControl::with_limit(strict_nodes);
            assert!(
                expected
                    .accepts_with(&actual, policy, &mut exact_limit)
                    .expect("exact bytes meter limit")
            );
            assert_eq!(exact_limit.entries, strict_nodes);

            let mut one_over = RecordingControl::with_limit(strict_nodes - 1);
            assert!(matches!(
                expected.accepts_with(&actual, policy, &mut one_over),
                Err(TypeCompatibilityFailure::Control(ControlFailure))
            ));
            assert_eq!(one_over.entries, strict_nodes);
        }

        let mut exact_limit = RecordingControl::with_limit(2);
        assert!(
            expected
                .accepts_with(&actual, TypeCompatibilityPolicy::Recovery, &mut exact_limit)
                .expect("exact recovery bytes meter limit")
        );
        assert_eq!(exact_limit.entries, 2);

        let mut one_over = RecordingControl::with_limit(1);
        assert!(matches!(
            expected.accepts_with(&actual, TypeCompatibilityPolicy::Recovery, &mut one_over),
            Err(TypeCompatibilityFailure::Control(ControlFailure))
        ));
        assert_eq!(one_over.entries, 2);
    }

    #[test]
    fn recovery_projection_requires_exact_subject_structure() {
        let expected = TypeKind::Projection {
            subject: Box::new(TypeKind::Vec(Box::new(TypeKind::I32))),
            trait_name: Some("Trait".to_owned()),
            assoc: "Item".to_owned(),
        };
        let matching = expected.clone();
        let widened_subject = TypeKind::Projection {
            subject: Box::new(TypeKind::Vec(Box::new(TypeKind::Never))),
            trait_name: Some("Trait".to_owned()),
            assoc: "Item".to_owned(),
        };

        assert!(expected.accepts(&matching));
        assert!(!expected.accepts(&widened_subject));

        let mut control = RecordingControl::default();
        assert!(
            expected
                .accepts_with(&matching, TypeCompatibilityPolicy::Recovery, &mut control)
                .expect("metered exact projection")
        );
        assert_eq!(control.entries, 3);
    }

    #[test]
    fn failing_relation_enter_stops_deep_equal_child_traversal() {
        let ty = TypeKind::Tuple(vec![TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(
            TypeKind::I32,
        ))))]);

        let mut recovery = RecordingControl::with_limit(2);
        assert!(matches!(
            ty.accepts_with(&ty, TypeCompatibilityPolicy::Recovery, &mut recovery),
            Err(TypeCompatibilityFailure::Control(ControlFailure))
        ));
        assert_eq!(recovery.entries, 3);
        assert_eq!(
            recovery.events,
            vec![
                (ty.clone(), ty.clone()),
                (
                    TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(TypeKind::I32)))),
                    TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(TypeKind::I32)))),
                ),
                (
                    TypeKind::Vec(Box::new(TypeKind::I32)),
                    TypeKind::Vec(Box::new(TypeKind::I32)),
                ),
            ]
        );

        let mut strict = RecordingControl::with_limit(10);
        assert!(matches!(
            ty.accepts_with(&ty, TypeCompatibilityPolicy::Invariant, &mut strict),
            Err(TypeCompatibilityFailure::Control(ControlFailure))
        ));
        assert_eq!(strict.entries, 11);
        assert_eq!(
            &strict.events[8..],
            &[
                (ty.clone(), ty.clone()),
                (
                    TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(TypeKind::I32)))),
                    TypeKind::Vec(Box::new(TypeKind::Vec(Box::new(TypeKind::I32)))),
                ),
                (
                    TypeKind::Vec(Box::new(TypeKind::I32)),
                    TypeKind::Vec(Box::new(TypeKind::I32)),
                ),
            ]
        );
    }

    #[test]
    fn strict_prevalidation_rejects_nested_nodes_before_outer_shortcuts() {
        let poison = TypeKind::Error(TypePoisonId::from_index(11));
        let invalid = TypeKind::Vec(Box::new(poison));
        let cases = [
            (invalid.clone(), TypeKind::String),
            (TypeKind::Bytes, invalid.clone()),
            (
                TypeKind::Choice(vec![invalid.clone(), TypeKind::I32]),
                TypeKind::I32,
            ),
            (invalid.clone(), TypeKind::Never),
        ];
        for (expected, actual) in cases {
            assert!(matches!(
                accepts_with(&expected, &actual, TypeCompatibilityPolicy::Invariant),
                Err(TypeCompatibilityFailure::Forbidden { .. })
            ));
        }
    }

    #[test]
    fn strict_prevalidation_stops_at_first_forbidden_after_control_event() {
        let expected = TypeKind::Tuple(vec![
            TypeKind::Error(TypePoisonId::from_index(12)),
            TypeKind::Vec(Box::new(TypeKind::I32)),
        ]);
        let mut control = CountingControl::default();
        assert!(matches!(
            expected.accepts_with(
                &TypeKind::String,
                TypeCompatibilityPolicy::SelectedCall,
                &mut control
            ),
            Err(TypeCompatibilityFailure::Forbidden {
                side: super::TypeCompatibilitySide::Expected,
                kind: super::TypeCompatibilityForbidden::Error,
            })
        ));
        assert_eq!(control.entries, 2);

        let mut failing = FailingControl {
            entries: 0,
            fail_at: 1,
        };
        assert!(matches!(
            expected.accepts_with(
                &TypeKind::String,
                TypeCompatibilityPolicy::Invariant,
                &mut failing
            ),
            Err(TypeCompatibilityFailure::Control(ControlFailure))
        ));
        assert_eq!(failing.entries, 1);
    }

    #[test]
    fn strict_policies_reject_projection_placeholder_and_unresolved_lengths() {
        let projection = TypeKind::Projection {
            subject: Box::new(TypeKind::I32),
            trait_name: None,
            assoc: "Item".to_owned(),
        };
        let placeholder = TypeKind::Named("_".to_owned());
        for ty in [projection, placeholder] {
            assert!(matches!(
                accepts_with(&ty, &ty, TypeCompatibilityPolicy::SelectedCall),
                Err(TypeCompatibilityFailure::Forbidden { .. })
            ));
        }

        for len in [
            ArrayLength::Error(TypePoisonId::from_index(7)),
            ArrayLength::Inferred,
        ] {
            let ty = TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len,
            };
            assert!(matches!(
                accepts_with(&ty, &ty, TypeCompatibilityPolicy::Invariant),
                Err(TypeCompatibilityFailure::Forbidden { .. })
            ));
        }

        let unknown_effects = TypeKind::function([TypeKind::I32], TypeKind::I32);
        assert!(matches!(
            accepts_with(
                &unknown_effects,
                &unknown_effects,
                TypeCompatibilityPolicy::Invariant
            ),
            Err(TypeCompatibilityFailure::Forbidden {
                kind: super::TypeCompatibilityForbidden::UnknownEffectTail,
                ..
            })
        ));
    }

    #[test]
    fn strict_array_lengths_are_exact_and_generic_identity_is_not_open() {
        let generic = GenericConstParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(41)),
            0,
        );
        let same = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Generic(generic.clone()),
        };
        let other = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Generic(GenericConstParameterId::new(
                GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(41)),
                1,
            )),
        };
        let const_three = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Const(3),
        };
        assert!(accepts_with(&same, &same, TypeCompatibilityPolicy::SelectedCall).unwrap());
        assert!(!accepts_with(&same, &other, TypeCompatibilityPolicy::SelectedCall).unwrap());
        assert!(!accepts_with(&same, &const_three, TypeCompatibilityPolicy::Invariant).unwrap());
        assert!(
            TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::Const(3),
            }
            .accepts(&TypeKind::Array {
                item: Box::new(TypeKind::I32),
                len: ArrayLength::Const(3),
            })
        );
    }

    #[test]
    fn recovery_array_lengths_retain_open_recovery_behavior() {
        let generic = GenericConstParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(42)),
            0,
        );
        let expected_generic = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Generic(generic),
        };
        let actual_const = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Const(7),
        };
        let expected_const = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Const(7),
        };
        let actual_error = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Error(TypePoisonId::from_index(9)),
        };
        let actual_inferred = TypeKind::Array {
            item: Box::new(TypeKind::I32),
            len: ArrayLength::Inferred,
        };
        assert!(expected_generic.accepts(&actual_const));
        assert!(expected_const.accepts(&actual_error));
        assert!(!expected_const.accepts(&actual_inferred));
    }

    #[test]
    fn recovery_domain_widening_and_bottom_match_existing_rules() {
        assert!(TypeKind::Bytes.accepts(&TypeKind::Vec(Box::new(TypeKind::U8))));
        assert!(TypeKind::ActionName.accepts(&TypeKind::String));
        assert!(TypeKind::I32.accepts(&TypeKind::Never));
        assert!(!TypeKind::Never.accepts(&TypeKind::I32));
        assert!(
            TypeKind::AgentValue.accepts(&TypeKind::Option(Box::new(TypeKind::Vec(Box::new(
                TypeKind::U8,
            )))))
        );
    }

    #[test]
    fn recovery_character_nominals_retain_structural_family_identity() {
        let owner = CharacterId::try_new("character.a").expect("character id");
        let other_owner = CharacterId::try_new("character.b").expect("character id");
        let expected = TypeKind::character_look(owner.clone());
        let matching = TypeKind::character_look(owner);
        let different_owner = TypeKind::character_look(other_owner);
        let different_family =
            TypeKind::character_part(CharacterId::try_new("character.a").expect("character id"));

        assert!(expected.accepts(&matching));
        assert!(!expected.accepts(&different_owner));
        assert!(!expected.accepts(&different_family));
    }

    #[test]
    fn strict_choice_has_unique_injection_without_poison_fallback() {
        let poison = TypeKind::Error(TypePoisonId::from_index(8));
        let expected = TypeKind::Choice(vec![poison, TypeKind::I32]);
        assert!(expected.accepts(&TypeKind::I32));
        assert!(matches!(
            accepts_with(
                &expected,
                &TypeKind::I32,
                TypeCompatibilityPolicy::SelectedCall
            ),
            Err(TypeCompatibilityFailure::Forbidden { .. })
        ));
        assert!(
            !accepts_with(
                &TypeKind::Choice(vec![TypeKind::ActionName, TypeKind::String]),
                &TypeKind::String,
                TypeCompatibilityPolicy::Invariant
            )
            .unwrap()
        );
    }

    #[test]
    fn strict_family_reference_checks_specialized_payloads() {
        let expected = TypeKind::entity_ref(EntityKind::Signal);
        let actual = TypeKind::entity_ref_with_value(
            EntityKind::Signal,
            TypeKind::Error(TypePoisonId::from_index(10)),
        );
        assert!(expected.accepts(&actual));
        assert!(matches!(
            accepts_with(&expected, &actual, TypeCompatibilityPolicy::SelectedCall),
            Err(TypeCompatibilityFailure::Forbidden { .. })
        ));
    }
}
