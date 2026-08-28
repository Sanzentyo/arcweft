//! Declaration-owned generic substitution for semantic types.

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
};

use crate::effect_row::{
    EffectIssuerRebindError, EffectRow, EffectRowError, EffectRowTail, EffectSubstitution,
    EffectVar, EffectVarIssuer,
};
use crate::effects::EffectSet;

use super::{
    AcceptedNominalType, AcceptedVariantCaseSemanticId, ArrayLength, EntityType,
    GenericConstParameterId, GenericTypeParameterId, OpenNominalType, ProjectNominalType, TypeKind,
    VariantPayloadShape, VariantPayloadType,
};

/// One call-site instantiation of declaration-owned generic type parameters.
///
/// Inference observes already checked argument types and only commits a set of
/// bindings when every repeated generic identity remains consistent. Source
/// spellings never participate in the lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TypeParameterSubstitutions {
    bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
}

impl TypeParameterSubstitutions {
    /// Infers bindings from one declared parameter shape and actual argument.
    pub(crate) fn observe(&mut self, declared: &TypeKind, actual: &TypeKind) -> bool {
        let mut bindings = self.bindings.clone();
        if !observe_type_parameters(declared, actual, &mut bindings) {
            return false;
        }
        self.bindings = bindings;
        true
    }

    /// Applies all call-site bindings to a semantic type.
    pub(crate) fn apply(&self, ty: &TypeKind) -> TypeKind {
        ty.substitute_type_parameters(&self.bindings)
    }

    /// Applies the known bindings only when the resulting type is concrete.
    ///
    /// Contextual expression checking must not treat an unbound declaration
    /// parameter as a value type. Once every parameter in the shape is bound,
    /// however, the specialized declaration type is the authoritative expected
    /// type for literals and other context-sensitive expressions.
    pub(crate) fn apply_resolved(&self, ty: &TypeKind) -> Option<TypeKind> {
        let applied = self.apply(ty);
        (!contains_generic_parameter(&applied)).then_some(applied)
    }
}

fn contains_generic_parameter(ty: &TypeKind) -> bool {
    contains_generic_parameter_where(ty, &|_| true)
}

fn contains_generic_parameter_where(
    ty: &TypeKind,
    predicate: &impl Fn(&GenericTypeParameterId) -> bool,
) -> bool {
    match ty {
        TypeKind::GenericParam(parameter) => predicate(parameter),
        TypeKind::Range(inner)
        | TypeKind::Probe(inner)
        | TypeKind::Vec(inner)
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Need(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner)
        | TypeKind::DialogueLine(inner)
        | TypeKind::BorrowRef { inner, .. } => contains_generic_parameter_where(inner, predicate),
        TypeKind::IteratorState { item, .. } | TypeKind::Array { item, .. } => {
            contains_generic_parameter_where(item, predicate)
        }
        TypeKind::Ref(entity) => entity
            .value()
            .is_some_and(|value| contains_generic_parameter_where(value, predicate)),
        TypeKind::Map { key, value, .. } => {
            contains_generic_parameter_where(key, predicate)
                || contains_generic_parameter_where(value, predicate)
        }
        TypeKind::Result {
            ok: left,
            error: right,
        } => {
            contains_generic_parameter_where(left, predicate)
                || contains_generic_parameter_where(right, predicate)
        }
        TypeKind::Stream { item, error } | TypeKind::Parser { item, error } => {
            contains_generic_parameter_where(item, predicate)
                || contains_generic_parameter_where(error, predicate)
        }
        TypeKind::Function {
            params,
            return_type,
            ..
        } => {
            params
                .iter()
                .any(|parameter| contains_generic_parameter_where(parameter, predicate))
                || contains_generic_parameter_where(return_type, predicate)
        }
        TypeKind::ProjectNominal(nominal) => {
            contains_any_generic_where(nominal.arguments(), predicate)
        }
        TypeKind::AcceptedNominal(nominal) => {
            contains_any_generic_where(nominal.arguments(), predicate)
        }
        TypeKind::OpenNominal(nominal) => {
            contains_any_generic_where(nominal.arguments(), predicate)
        }
        TypeKind::Projection { subject, .. } => {
            contains_generic_parameter_where(subject, predicate)
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => {
            contains_any_generic_where(items, predicate)
        }
        TypeKind::VariantPayload(payload) => variant_payload_contains_generic(payload, predicate),
        atomic => atomic_contains_generic_parameter(atomic, predicate),
    }
}

fn atomic_contains_generic_parameter(
    ty: &TypeKind,
    _predicate: &impl Fn(&GenericTypeParameterId) -> bool,
) -> bool {
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
        | TypeKind::TextCluster
        | TypeKind::Duration
        | TypeKind::Progress
        | TypeKind::StageApi(_)
        | TypeKind::LineContext
        | TypeKind::StageActorHandle(_)
        | TypeKind::CueHandle
        | TypeKind::VoiceHandle
        | TypeKind::DisplayText
        | TypeKind::DebugStatePath
        | TypeKind::ObservationFieldPath
        | TypeKind::Predicate
        | TypeKind::Observation
        | TypeKind::ObservedObject
        | TypeKind::AgentBBox
        | TypeKind::ActionName
        | TypeKind::ActionTarget
        | TypeKind::ActionResult
        | TypeKind::AgentValue
        | TypeKind::DataFormat
        | TypeKind::DataShape
        | TypeKind::AgentEntityMetadata
        | TypeKind::AgentSourceAnchor
        | TypeKind::AgentProjectGraphNeighborhood
        | TypeKind::AgentProjectGraphSymbol
        | TypeKind::AgentProjectGraphEdge
        | TypeKind::CaptureTarget
        | TypeKind::CaptureRef
        | TypeKind::AgentResource
        | TypeKind::AgentResourceBody
        | TypeKind::RagContextPack
        | TypeKind::AgentBuiltin(_)
        | TypeKind::Handle { .. }
        | TypeKind::Error(_)
        | TypeKind::CharacterPatch(_)
        | TypeKind::FocusPatch
        | TypeKind::CharacterDialogue(_)
        | TypeKind::ViewValue
        | TypeKind::CharacterNominal(_)
        | TypeKind::Named(_)
        | TypeKind::Unit
        | TypeKind::Never => false,
        TypeKind::GenericParam(_)
        | TypeKind::Range(_)
        | TypeKind::Probe(_)
        | TypeKind::Vec(_)
        | TypeKind::Slice(_)
        | TypeKind::Seq(_)
        | TypeKind::Option(_)
        | TypeKind::ThreadHandle(_)
        | TypeKind::Shared(_)
        | TypeKind::DialogueLine(_)
        | TypeKind::BorrowRef { .. }
        | TypeKind::IteratorState { .. }
        | TypeKind::Array { .. }
        | TypeKind::Ref(_)
        | TypeKind::Map { .. }
        | TypeKind::Need(_)
        | TypeKind::Parser { .. }
        | TypeKind::Result { .. }
        | TypeKind::Stream { .. }
        | TypeKind::Function { .. }
        | TypeKind::ProjectNominal(_)
        | TypeKind::AcceptedNominal(_)
        | TypeKind::OpenNominal(_)
        | TypeKind::Projection { .. }
        | TypeKind::Tuple(_)
        | TypeKind::Choice(_)
        | TypeKind::VariantPayload(_) => unreachable!("composite type reached atomic generic scan"),
    }
}

fn contains_any_generic_where(
    types: &[TypeKind],
    predicate: &impl Fn(&GenericTypeParameterId) -> bool,
) -> bool {
    types
        .iter()
        .any(|ty| contains_generic_parameter_where(ty, predicate))
}

impl TypeKind {
    /// Replaces declaration-owned generic parameters throughout this type.
    ///
    /// Callers provide typed parameter identities, so equal source spellings
    /// from different declarations can never alias one another.
    pub(crate) fn substitute_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Self {
        if let Some(substituted) = self.substitute_nominal_type_parameters(substitutions) {
            return substituted;
        }
        if let Some(substituted) = self.substitute_unary_type_parameters(substitutions) {
            return substituted;
        }
        match self {
            Self::GenericParam(parameter) => substitutions
                .get(parameter)
                .cloned()
                .unwrap_or_else(|| self.clone()),
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Self::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(inner.substitute_type_parameters(substitutions)),
            },
            Self::Ref(entity) => Self::Ref(EntityType::new(
                entity.kind().clone(),
                entity
                    .value()
                    .map(|value| value.substitute_type_parameters(substitutions)),
            )),
            Self::IteratorState { family, item } => Self::IteratorState {
                family: *family,
                item: Box::new(item.substitute_type_parameters(substitutions)),
            },
            Self::Need(value) => {
                Self::Need(Box::new(value.substitute_type_parameters(substitutions)))
            }
            Self::Stream { item, error } => Self::Stream {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Parser { item, error } => Self::Parser {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Result { ok, error } => Self::Result {
                ok: Box::new(ok.substitute_type_parameters(substitutions)),
                error: Box::new(error.substitute_type_parameters(substitutions)),
            },
            Self::Map { kind, key, value } => Self::Map {
                kind: *kind,
                key: Box::new(key.substitute_type_parameters(substitutions)),
                value: Box::new(value.substitute_type_parameters(substitutions)),
            },
            Self::Array { item, len } => Self::Array {
                item: Box::new(item.substitute_type_parameters(substitutions)),
                len: len.clone(),
            },
            Self::Function {
                params,
                return_type,
                effects,
            } => Self::function_with_effects(
                params
                    .iter()
                    .map(|param| param.substitute_type_parameters(substitutions)),
                return_type.substitute_type_parameters(substitutions),
                effects.clone(),
            ),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => Self::Projection {
                subject: Box::new(subject.substitute_type_parameters(substitutions)),
                trait_name: trait_name.clone(),
                assoc: assoc.clone(),
            },
            Self::Tuple(items) => Self::Tuple(
                items
                    .iter()
                    .map(|item| item.substitute_type_parameters(substitutions))
                    .collect(),
            ),
            Self::Choice(items) => Self::Choice(
                items
                    .iter()
                    .map(|item| item.substitute_type_parameters(substitutions))
                    .collect(),
            ),
            Self::DialogueLine(result) => {
                Self::DialogueLine(Box::new(result.substitute_type_parameters(substitutions)))
            }
            Self::VariantPayload(payload) => {
                map_variant_payload_type(payload, |ty| ty.substitute_type_parameters(substitutions))
            }
            other => other.clone(),
        }
    }

    /// Replaces declaration-owned equality-only constant parameters throughout
    /// this type. The completed constraint solution is the sole producer of
    /// these rows; values are canonical `Const` or retained rigid/future
    /// `Generic` identities, never inferred/error sentinels or expression ASTs.
    pub(crate) fn substitute_const_parameters(
        &self,
        substitutions: &BTreeMap<GenericConstParameterId, ArrayLength>,
    ) -> Self {
        let recurse = |ty: &Self| ty.substitute_const_parameters(substitutions);
        match self {
            Self::ProjectNominal(nominal) => Self::ProjectNominal(ProjectNominalType::new(
                nominal.declaration().clone(),
                nominal.arguments().iter().map(recurse).collect::<Vec<_>>(),
            )),
            Self::AcceptedNominal(nominal) => Self::AcceptedNominal(AcceptedNominalType::new(
                nominal.declaration().clone(),
                nominal.arguments().iter().map(recurse).collect::<Vec<_>>(),
            )),
            Self::OpenNominal(nominal) => Self::OpenNominal(OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                nominal.arguments().iter().map(recurse).collect::<Vec<_>>(),
            )),
            Self::Range(inner) => Self::Range(Box::new(recurse(inner))),
            Self::Probe(inner) => Self::Probe(Box::new(recurse(inner))),
            Self::Vec(inner) => Self::Vec(Box::new(recurse(inner))),
            Self::Slice(inner) => Self::Slice(Box::new(recurse(inner))),
            Self::Seq(inner) => Self::Seq(Box::new(recurse(inner))),
            Self::Need(inner) => Self::Need(Box::new(recurse(inner))),
            Self::Option(inner) => Self::Option(Box::new(recurse(inner))),
            Self::ThreadHandle(inner) => Self::ThreadHandle(Box::new(recurse(inner))),
            Self::Shared(inner) => Self::Shared(Box::new(recurse(inner))),
            Self::DialogueLine(inner) => Self::DialogueLine(Box::new(recurse(inner))),
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Self::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(recurse(inner)),
            },
            Self::IteratorState { family, item } => Self::IteratorState {
                family: *family,
                item: Box::new(recurse(item)),
            },
            Self::Array { item, len } => Self::Array {
                item: Box::new(recurse(item)),
                len: match len {
                    ArrayLength::Generic(parameter) => substitutions
                        .get(parameter)
                        .cloned()
                        .unwrap_or_else(|| len.clone()),
                    ArrayLength::Const(_) | ArrayLength::Error(_) | ArrayLength::Inferred => {
                        len.clone()
                    }
                },
            },
            Self::Ref(entity) => Self::Ref(EntityType::new(
                entity.kind().clone(),
                entity.value().map(recurse),
            )),
            Self::Map { kind, key, value } => Self::Map {
                kind: *kind,
                key: Box::new(recurse(key)),
                value: Box::new(recurse(value)),
            },
            Self::Stream { item, error } => Self::Stream {
                item: Box::new(recurse(item)),
                error: Box::new(recurse(error)),
            },
            Self::Parser { item, error } => Self::Parser {
                item: Box::new(recurse(item)),
                error: Box::new(recurse(error)),
            },
            Self::Result { ok, error } => Self::Result {
                ok: Box::new(recurse(ok)),
                error: Box::new(recurse(error)),
            },
            Self::Function {
                params,
                return_type,
                effects,
            } => Self::function_with_effects(
                params.iter().map(recurse),
                recurse(return_type),
                effects.clone(),
            ),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => Self::Projection {
                subject: Box::new(recurse(subject)),
                trait_name: trait_name.clone(),
                assoc: assoc.clone(),
            },
            Self::Tuple(items) => Self::Tuple(items.iter().map(recurse).collect()),
            Self::Choice(items) => Self::Choice(items.iter().map(recurse).collect()),
            Self::VariantPayload(payload) => map_variant_payload_type(payload, recurse),
            other => other.clone(),
        }
    }

    /// Applies issuer-backed effect bindings to every nested function row.
    /// Type and effect substitution are separate authorities; the checked
    /// lower solution composes them in that order.
    pub(crate) fn substitute_effect_rows(
        &self,
        substitutions: &EffectSubstitution,
    ) -> Result<Self, EffectRowError> {
        let recurse = |ty: &Self| ty.substitute_effect_rows(substitutions);
        Ok(match self {
            Self::ProjectNominal(nominal) => Self::ProjectNominal(ProjectNominalType::new(
                nominal.declaration().clone(),
                nominal
                    .arguments()
                    .iter()
                    .map(recurse)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Self::AcceptedNominal(nominal) => Self::AcceptedNominal(AcceptedNominalType::new(
                nominal.declaration().clone(),
                nominal
                    .arguments()
                    .iter()
                    .map(recurse)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Self::OpenNominal(nominal) => Self::OpenNominal(OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                nominal
                    .arguments()
                    .iter()
                    .map(recurse)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Self::Range(inner) => Self::Range(Box::new(recurse(inner)?)),
            Self::Probe(inner) => Self::Probe(Box::new(recurse(inner)?)),
            Self::Vec(inner) => Self::Vec(Box::new(recurse(inner)?)),
            Self::Slice(inner) => Self::Slice(Box::new(recurse(inner)?)),
            Self::Seq(inner) => Self::Seq(Box::new(recurse(inner)?)),
            Self::Need(inner) => Self::Need(Box::new(recurse(inner)?)),
            Self::Option(inner) => Self::Option(Box::new(recurse(inner)?)),
            Self::ThreadHandle(inner) => Self::ThreadHandle(Box::new(recurse(inner)?)),
            Self::Shared(inner) => Self::Shared(Box::new(recurse(inner)?)),
            Self::DialogueLine(inner) => Self::DialogueLine(Box::new(recurse(inner)?)),
            Self::BorrowRef {
                kind,
                lifetime,
                inner,
            } => Self::BorrowRef {
                kind: *kind,
                lifetime: lifetime.clone(),
                inner: Box::new(recurse(inner)?),
            },
            Self::IteratorState { family, item } => Self::IteratorState {
                family: *family,
                item: Box::new(recurse(item)?),
            },
            Self::Array { item, len } => Self::Array {
                item: Box::new(recurse(item)?),
                len: len.clone(),
            },
            Self::Ref(entity) => Self::Ref(EntityType::new(
                entity.kind().clone(),
                entity.value().map(recurse).transpose()?,
            )),
            Self::Map { kind, key, value } => Self::Map {
                kind: *kind,
                key: Box::new(recurse(key)?),
                value: Box::new(recurse(value)?),
            },
            Self::Stream { item, error } => Self::Stream {
                item: Box::new(recurse(item)?),
                error: Box::new(recurse(error)?),
            },
            Self::Parser { item, error } => Self::Parser {
                item: Box::new(recurse(item)?),
                error: Box::new(recurse(error)?),
            },
            Self::Result { ok, error } => Self::Result {
                ok: Box::new(recurse(ok)?),
                error: Box::new(recurse(error)?),
            },
            Self::Function {
                params,
                return_type,
                effects,
            } => Self::function_with_effects(
                params.iter().map(recurse).collect::<Result<Vec<_>, _>>()?,
                recurse(return_type)?,
                effects.resolve_partial(substitutions)?,
            ),
            Self::Projection {
                subject,
                trait_name,
                assoc,
            } => Self::Projection {
                subject: Box::new(recurse(subject)?),
                trait_name: trait_name.clone(),
                assoc: assoc.clone(),
            },
            Self::Tuple(items) => {
                Self::Tuple(items.iter().map(recurse).collect::<Result<Vec<_>, _>>()?)
            }
            Self::Choice(items) => {
                Self::Choice(items.iter().map(recurse).collect::<Result<Vec<_>, _>>()?)
            }
            Self::VariantPayload(payload) => try_map_variant_payload_type(payload, recurse)?,
            other => other.clone(),
        })
    }

    pub(crate) fn checked_rebind_effect_rows(
        &self,
        prepared: EffectVarIssuer,
        checked: EffectVarIssuer,
        authorized_ordinals: &BTreeSet<u32>,
    ) -> Result<Self, EffectIssuerRebindError> {
        let substitutions =
            EffectSubstitution::from_rows(authorized_ordinals.iter().map(|ordinal| {
                (
                    EffectVar::issued(prepared, *ordinal),
                    EffectRow::open(EffectSet::new(), EffectVar::issued(checked, *ordinal)),
                )
            }));
        let rebound = self
            .substitute_effect_rows(&substitutions)
            .map_err(|error| match error {
                EffectRowError::UnknownRow => EffectIssuerRebindError::UnknownRow,
                EffectRowError::UnboundVariable { .. }
                | EffectRowError::ConflictingBinding { .. }
                | EffectRowError::CyclicBinding { .. } => {
                    unreachable!("fresh one-step issuer rebind cannot conflict or cycle")
                }
            })?;
        validate_rebound_effect_rows(&rebound, prepared, checked, authorized_ordinals)?;
        Ok(rebound)
    }

    fn substitute_nominal_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Option<Self> {
        let substitute_arguments = |arguments: &[Self]| {
            arguments
                .iter()
                .map(|argument| argument.substitute_type_parameters(substitutions))
                .collect::<Vec<_>>()
        };
        Some(match self {
            Self::ProjectNominal(nominal) => Self::ProjectNominal(ProjectNominalType::new(
                nominal.declaration().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            Self::AcceptedNominal(nominal) => Self::AcceptedNominal(AcceptedNominalType::new(
                nominal.declaration().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            Self::OpenNominal(nominal) => Self::OpenNominal(OpenNominalType::new(
                nominal.rule().clone(),
                nominal.path().clone(),
                substitute_arguments(nominal.arguments()),
            )),
            _ => return None,
        })
    }

    fn substitute_unary_type_parameters(
        &self,
        substitutions: &BTreeMap<GenericTypeParameterId, TypeKind>,
    ) -> Option<Self> {
        let substitute = |inner: &Self| Box::new(inner.substitute_type_parameters(substitutions));
        Some(match self {
            Self::Vec(inner) => Self::Vec(substitute(inner)),
            Self::Seq(inner) => Self::Seq(substitute(inner)),
            Self::Slice(inner) => Self::Slice(substitute(inner)),
            Self::Range(inner) => Self::Range(substitute(inner)),
            Self::Option(inner) => Self::Option(substitute(inner)),
            Self::ThreadHandle(inner) => Self::ThreadHandle(substitute(inner)),
            Self::Shared(inner) => Self::Shared(substitute(inner)),
            Self::Probe(inner) => Self::Probe(substitute(inner)),
            Self::DialogueLine(inner) => Self::DialogueLine(substitute(inner)),
            _ => return None,
        })
    }
}

fn variant_payload_contains_generic(
    payload: &VariantPayloadType,
    predicate: &impl Fn(&GenericTypeParameterId) -> bool,
) -> bool {
    payload.shape().tuple_fields().is_some_and(|fields| {
        fields
            .iter()
            .any(|field| contains_generic_parameter_where(field.ty(), predicate))
    }) || payload.shape().record_fields().is_some_and(|fields| {
        fields
            .iter()
            .any(|field| contains_generic_parameter_where(field.ty(), predicate))
    })
}

fn map_variant_payload_type(
    payload: &VariantPayloadType,
    mut map: impl FnMut(&TypeKind) -> TypeKind,
) -> TypeKind {
    try_map_variant_payload_type(payload, |ty| Ok::<_, Infallible>(map(ty)))
        .unwrap_or_else(|_| unreachable!("infallible variant payload mapping failed"))
}

fn try_map_variant_payload_type<E>(
    payload: &VariantPayloadType,
    mut map: impl FnMut(&TypeKind) -> Result<TypeKind, E>,
) -> Result<TypeKind, E> {
    let shape = match payload.shape() {
        VariantPayloadShape::Unit => VariantPayloadShape::Unit,
        VariantPayloadShape::Tuple(fields) => {
            let mapped = fields
                .iter()
                .map(|field| map(field.ty()))
                .collect::<Result<Vec<_>, _>>()?;
            VariantPayloadShape::try_tuple(
                payload.owner_family(),
                payload.owner_type(),
                payload.case_ordinal(),
                mapped,
            )
            .unwrap_or_else(|_| unreachable!("valid variant tuple shape became invalid"))
        }
        VariantPayloadShape::Record(fields) => {
            let mapped = fields
                .iter()
                .map(|field| Ok((field.diagnostic_name().to_owned(), map(field.ty())?)))
                .collect::<Result<Vec<_>, E>>()?;
            VariantPayloadShape::try_record(
                payload.owner_family(),
                payload.owner_type(),
                payload.case_ordinal(),
                mapped,
            )
            .unwrap_or_else(|_| unreachable!("valid variant record shape became invalid"))
        }
    };
    let case = AcceptedVariantCaseSemanticId::issue(
        payload.owner_family(),
        payload.owner_type(),
        payload.case_ordinal(),
        &shape,
    );
    let rebuilt = VariantPayloadType::try_new(
        payload.owner_family(),
        payload.owner_type(),
        payload.case_ordinal(),
        case,
        shape,
    )
    .unwrap_or_else(|_| unreachable!("mapped variant payload lost its owner invariant"));
    Ok(TypeKind::VariantPayload(Box::new(rebuilt)))
}

fn validate_rebound_effect_rows(
    ty: &TypeKind,
    prepared: EffectVarIssuer,
    checked: EffectVarIssuer,
    authorized_ordinals: &BTreeSet<u32>,
) -> Result<(), EffectIssuerRebindError> {
    let validate_children = |children: &[TypeKind]| {
        children.iter().try_for_each(|child| {
            validate_rebound_effect_rows(child, prepared, checked, authorized_ordinals)
        })
    };
    match ty {
        TypeKind::Function {
            params,
            return_type,
            effects,
        } => {
            match effects.tail() {
                EffectRowTail::Closed => {}
                EffectRowTail::Unknown => return Err(EffectIssuerRebindError::UnknownRow),
                EffectRowTail::Variable(variable)
                    if variable.issuer() == checked
                        && authorized_ordinals.contains(&variable.index()) => {}
                EffectRowTail::Variable(variable) if variable.issuer() == prepared => {
                    return Err(EffectIssuerRebindError::UnauthorizedVariable { variable });
                }
                EffectRowTail::Variable(variable) => {
                    return Err(EffectIssuerRebindError::ForeignVariable { variable });
                }
            }
            validate_children(params)?;
            validate_rebound_effect_rows(return_type, prepared, checked, authorized_ordinals)
        }
        TypeKind::Range(inner)
        | TypeKind::Probe(inner)
        | TypeKind::Vec(inner)
        | TypeKind::Slice(inner)
        | TypeKind::Seq(inner)
        | TypeKind::Need(inner)
        | TypeKind::Option(inner)
        | TypeKind::ThreadHandle(inner)
        | TypeKind::Shared(inner)
        | TypeKind::DialogueLine(inner)
        | TypeKind::BorrowRef { inner, .. }
        | TypeKind::IteratorState { item: inner, .. }
        | TypeKind::Array { item: inner, .. } => {
            validate_rebound_effect_rows(inner, prepared, checked, authorized_ordinals)
        }
        TypeKind::Ref(entity) => entity.value().map_or(Ok(()), |value| {
            validate_rebound_effect_rows(value, prepared, checked, authorized_ordinals)
        }),
        TypeKind::Map { key, value, .. }
        | TypeKind::Stream {
            item: key,
            error: value,
        }
        | TypeKind::Parser {
            item: key,
            error: value,
        }
        | TypeKind::Result {
            ok: key,
            error: value,
        } => {
            validate_rebound_effect_rows(key, prepared, checked, authorized_ordinals)?;
            validate_rebound_effect_rows(value, prepared, checked, authorized_ordinals)
        }
        TypeKind::ProjectNominal(nominal) => validate_children(nominal.arguments()),
        TypeKind::AcceptedNominal(nominal) => validate_children(nominal.arguments()),
        TypeKind::OpenNominal(nominal) => validate_children(nominal.arguments()),
        TypeKind::Projection { subject, .. } => {
            validate_rebound_effect_rows(subject, prepared, checked, authorized_ordinals)
        }
        TypeKind::Tuple(items) | TypeKind::Choice(items) => validate_children(items),
        TypeKind::VariantPayload(payload) => payload.shape().visit_types(&mut |field| {
            validate_rebound_effect_rows(field, prepared, checked, authorized_ordinals)
        }),
        _ => Ok(()),
    }
}

fn observe_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> bool {
    if declared.is_unresolved_for_compatibility() || actual.is_unresolved_for_compatibility() {
        return true;
    }
    match declared {
        TypeKind::GenericParam(parameter) => {
            if let Some(bound) = bindings.get(parameter) {
                bound == actual
            } else {
                bindings.insert(parameter.clone(), actual.clone());
                true
            }
        }
        TypeKind::ProjectNominal(declared) => match actual {
            TypeKind::ProjectNominal(actual) if declared.declaration() == actual.declaration() => {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::AcceptedNominal(declared) => match actual {
            TypeKind::AcceptedNominal(actual) if declared.declaration() == actual.declaration() => {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::OpenNominal(declared) => match actual {
            TypeKind::OpenNominal(actual)
                if declared.rule() == actual.rule() && declared.path() == actual.path() =>
            {
                observe_type_slices(declared.arguments(), actual.arguments(), bindings)
            }
            _ => true,
        },
        TypeKind::Ref(declared) => match actual {
            TypeKind::Ref(actual) if declared.kind() == actual.kind() => {
                match (declared.value(), actual.value()) {
                    (Some(declared), Some(actual)) => {
                        observe_type_parameters(declared, actual, bindings)
                    }
                    _ => true,
                }
            }
            _ => true,
        },
        _ => observe_unary_type_parameters(declared, actual, bindings)
            .or_else(|| observe_composite_type_parameters(declared, actual, bindings))
            .unwrap_or(true),
    }
}

fn observe_unary_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    let pair = match (declared, actual) {
        (TypeKind::Range(declared), TypeKind::Range(actual))
        | (TypeKind::Probe(declared), TypeKind::Probe(actual))
        | (TypeKind::Vec(declared), TypeKind::Vec(actual))
        | (TypeKind::Slice(declared), TypeKind::Slice(actual))
        | (TypeKind::Seq(declared), TypeKind::Seq(actual))
        | (TypeKind::Option(declared), TypeKind::Option(actual))
        | (TypeKind::ThreadHandle(declared), TypeKind::ThreadHandle(actual))
        | (TypeKind::Shared(declared), TypeKind::Shared(actual))
        | (TypeKind::DialogueLine(declared), TypeKind::DialogueLine(actual)) => (declared, actual),
        (TypeKind::Array { item: declared, .. }, TypeKind::Array { item: actual, .. }) => {
            (declared, actual)
        }
        (
            TypeKind::BorrowRef {
                kind: declared_kind,
                lifetime: declared_lifetime,
                inner: declared,
            },
            TypeKind::BorrowRef {
                kind: actual_kind,
                lifetime: actual_lifetime,
                inner: actual,
            },
        ) if declared_kind == actual_kind && declared_lifetime == actual_lifetime => {
            (declared, actual)
        }
        (
            TypeKind::IteratorState {
                family: declared_family,
                item: declared,
            },
            TypeKind::IteratorState {
                family: actual_family,
                item: actual,
            },
        ) if declared_family == actual_family => (declared, actual),
        (
            TypeKind::Range(_)
            | TypeKind::Probe(_)
            | TypeKind::Vec(_)
            | TypeKind::Slice(_)
            | TypeKind::Seq(_)
            | TypeKind::Option(_)
            | TypeKind::ThreadHandle(_)
            | TypeKind::Shared(_)
            | TypeKind::DialogueLine(_)
            | TypeKind::Array { .. }
            | TypeKind::BorrowRef { .. }
            | TypeKind::IteratorState { .. },
            _,
        ) => return Some(true),
        _ => return None,
    };
    Some(observe_type_parameters(pair.0, pair.1, bindings))
}

fn observe_composite_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    let observed = match (declared, actual) {
        (TypeKind::Need(declared), TypeKind::Need(actual)) => {
            observe_type_parameters(declared, actual, bindings)
        }
        (TypeKind::VariantPayload(declared), TypeKind::VariantPayload(actual)) => {
            observe_variant_payload_types(declared, actual, bindings)
        }
        (
            TypeKind::Stream {
                item: declared_first,
                error: declared_second,
            }
            | TypeKind::Parser {
                item: declared_first,
                error: declared_second,
            }
            | TypeKind::Result {
                ok: declared_first,
                error: declared_second,
            },
            TypeKind::Stream {
                item: actual_first,
                error: actual_second,
            }
            | TypeKind::Parser {
                item: actual_first,
                error: actual_second,
            }
            | TypeKind::Result {
                ok: actual_first,
                error: actual_second,
            },
        ) if core::mem::discriminant(declared) == core::mem::discriminant(actual) => {
            observe_type_parameters(declared_first, actual_first, bindings)
                && observe_type_parameters(declared_second, actual_second, bindings)
        }
        (
            TypeKind::Map {
                kind: declared_kind,
                key: declared_key,
                value: declared_value,
            },
            TypeKind::Map {
                kind: actual_kind,
                key: actual_key,
                value: actual_value,
            },
        ) if declared_kind == actual_kind => {
            observe_type_parameters(declared_key, actual_key, bindings)
                && observe_type_parameters(declared_value, actual_value, bindings)
        }
        (
            TypeKind::Function {
                params: declared_params,
                return_type: declared_return,
                ..
            },
            TypeKind::Function {
                params: actual_params,
                return_type: actual_return,
                ..
            },
        ) => {
            observe_type_slices(declared_params, actual_params, bindings)
                && observe_type_parameters(declared_return, actual_return, bindings)
        }
        (
            TypeKind::Projection {
                subject: declared_subject,
                trait_name: declared_trait,
                assoc: declared_assoc,
            },
            TypeKind::Projection {
                subject: actual_subject,
                trait_name: actual_trait,
                assoc: actual_assoc,
            },
        ) if declared_trait == actual_trait && declared_assoc == actual_assoc => {
            observe_type_parameters(declared_subject, actual_subject, bindings)
        }
        (
            TypeKind::Need(_)
            | TypeKind::Stream { .. }
            | TypeKind::Parser { .. }
            | TypeKind::Result { .. }
            | TypeKind::Map { .. }
            | TypeKind::Function { .. }
            | TypeKind::Projection { .. }
            | TypeKind::VariantPayload(_),
            _,
        ) => true,
        _ => return observe_sequence_type_parameters(declared, actual, bindings),
    };
    Some(observed)
}

fn observe_variant_payload_types(
    declared: &VariantPayloadType,
    actual: &VariantPayloadType,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> bool {
    if declared.owner_family() != actual.owner_family()
        || declared.owner_type() != actual.owner_type()
        || declared.case_ordinal() != actual.case_ordinal()
    {
        return false;
    }
    match (declared.shape(), actual.shape()) {
        (VariantPayloadShape::Unit, VariantPayloadShape::Unit) => true,
        (VariantPayloadShape::Tuple(declared), VariantPayloadShape::Tuple(actual)) => {
            declared.len() == actual.len()
                && declared.iter().zip(actual).all(|(declared, actual)| {
                    declared.ordinal() == actual.ordinal()
                        && observe_type_parameters(declared.ty(), actual.ty(), bindings)
                })
        }
        (VariantPayloadShape::Record(declared), VariantPayloadShape::Record(actual)) => {
            declared.len() == actual.len()
                && declared.iter().zip(actual).all(|(declared, actual)| {
                    declared.ordinal() == actual.ordinal()
                        && observe_type_parameters(declared.ty(), actual.ty(), bindings)
                })
        }
        (VariantPayloadShape::Unit, _)
        | (
            VariantPayloadShape::Tuple(_) | VariantPayloadShape::Record(_),
            VariantPayloadShape::Unit,
        )
        | (VariantPayloadShape::Tuple(_), VariantPayloadShape::Record(_))
        | (VariantPayloadShape::Record(_), VariantPayloadShape::Tuple(_)) => false,
    }
}

fn observe_sequence_type_parameters(
    declared: &TypeKind,
    actual: &TypeKind,
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> Option<bool> {
    Some(match (declared, actual) {
        (TypeKind::Tuple(declared), TypeKind::Tuple(actual))
        | (TypeKind::Choice(declared), TypeKind::Choice(actual)) => {
            observe_type_slices(declared, actual, bindings)
        }
        (TypeKind::Tuple(_) | TypeKind::Choice(_), _) => true,
        _ => return None,
    })
}

fn observe_type_slices(
    declared: &[TypeKind],
    actual: &[TypeKind],
    bindings: &mut BTreeMap<GenericTypeParameterId, TypeKind>,
) -> bool {
    declared.len() == actual.len()
        && declared
            .iter()
            .zip(actual)
            .all(|(declared, actual)| observe_type_parameters(declared, actual, bindings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId};

    #[test]
    fn typed_identity_controls_recursive_substitution() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(7));
        let selected = GenericTypeParameterId::new(owner.clone(), 0);
        let untouched = GenericTypeParameterId::new(owner, 1);
        let ty = TypeKind::Result {
            ok: Box::new(TypeKind::Vec(Box::new(TypeKind::GenericParam(
                selected.clone(),
            )))),
            error: Box::new(TypeKind::GenericParam(untouched.clone())),
        };
        let substitutions = BTreeMap::from([(selected, TypeKind::String)]);

        assert_eq!(
            ty.substitute_type_parameters(&substitutions),
            TypeKind::Result {
                ok: Box::new(TypeKind::Vec(Box::new(TypeKind::String))),
                error: Box::new(TypeKind::GenericParam(untouched)),
            }
        );
    }

    #[test]
    fn observation_specializes_a_nested_result_error() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(11));
        let error = GenericTypeParameterId::new(owner, 0);
        let declared = TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::GenericParam(error.clone())),
        };
        let actual = TypeKind::Result {
            ok: Box::new(TypeKind::I64),
            error: Box::new(TypeKind::String),
        };
        let mut substitutions = TypeParameterSubstitutions::default();

        assert!(substitutions.observe(&declared, &actual));
        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(error)),
            TypeKind::String
        );
    }

    #[test]
    fn conflicting_observation_does_not_commit_partial_bindings() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(12));
        let item = GenericTypeParameterId::new(owner.clone(), 0);
        let error = GenericTypeParameterId::new(owner, 1);
        let mut substitutions = TypeParameterSubstitutions::default();
        assert!(substitutions.observe(&TypeKind::GenericParam(item.clone()), &TypeKind::I64));

        let declared = TypeKind::Tuple(vec![
            TypeKind::GenericParam(error.clone()),
            TypeKind::GenericParam(item.clone()),
        ]);
        let actual = TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::String]);
        assert!(!substitutions.observe(&declared, &actual));

        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(item)),
            TypeKind::I64
        );
        assert_eq!(
            substitutions.apply(&TypeKind::GenericParam(error.clone())),
            TypeKind::GenericParam(error)
        );
    }

    #[test]
    fn resolved_application_only_exposes_concrete_expected_types() {
        let owner = GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(13));
        let item = GenericTypeParameterId::new(owner, 0);
        let declared = TypeKind::Option(Box::new(TypeKind::GenericParam(item.clone())));
        let mut substitutions = TypeParameterSubstitutions::default();

        assert_eq!(
            substitutions.apply_resolved(&TypeKind::I64),
            Some(TypeKind::I64)
        );
        assert_eq!(substitutions.apply_resolved(&declared), None);
        assert!(substitutions.observe(&TypeKind::GenericParam(item), &TypeKind::I64));
        assert_eq!(
            substitutions.apply_resolved(&declared),
            Some(TypeKind::Option(Box::new(TypeKind::I64)))
        );
    }
}
