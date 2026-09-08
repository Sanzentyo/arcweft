//! Variant schemas retained during source probing, before stable identities.

use super::{
    CheckedProjectNominal, CheckedVariantCase, CheckedVariantOwner, CheckedVariantOwnerError,
    VariantOwnerKind,
};
use crate::env::{EnumVariantPayload, EnvironmentEnumSchema};
use crate::types::{
    AgentBuiltinType, CharacterNominalType, ProjectNominalType, TypeKind, VariantPayloadShape,
    VariantPayloadType, VariantPayloadTypeShape,
};
use arcweft_core::pattern::RuntimeBuiltinVariantIdentity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantCaseSeed {
    ordinal: u32,
    payload: Option<VariantPayloadTypeShape>,
    diagnostic_name: Option<String>,
}

impl PreparedVariantCaseSeed {
    pub(crate) fn new(
        ordinal: u32,
        payload: Option<TypeKind>,
        diagnostic_name: Option<String>,
    ) -> Self {
        Self {
            ordinal,
            payload: payload.map(|ty| VariantPayloadTypeShape::Tuple(Box::new([ty]))),
            diagnostic_name,
        }
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn payload(&self) -> Option<&VariantPayloadTypeShape> {
        self.payload.as_ref()
    }
    pub(crate) fn diagnostic_name(&self) -> Option<&str> {
        self.diagnostic_name.as_deref()
    }
    pub(crate) fn project_payload_field(&self) -> Option<&TypeKind> {
        self.payload
            .as_ref()
            .and_then(VariantPayloadTypeShape::single_tuple_field)
    }
}

/// Complete source-owned case inventory. Project arguments and intrinsic
/// payloads may contain active types; this carrier issues no stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantOwnerSeed {
    kind: VariantOwnerKind<ProjectNominalType>,
    cases: Box<[PreparedVariantCaseSeed]>,
}

impl PreparedVariantOwnerSeed {
    pub(crate) fn try_project(
        nominal: ProjectNominalType,
        cases: impl Into<Box<[PreparedVariantCaseSeed]>>,
    ) -> Option<Self> {
        let cases = cases.into();
        if !cases.iter().enumerate().all(|(ordinal, case)| {
            u32::try_from(ordinal).is_ok_and(|ordinal| ordinal == case.ordinal)
                && case.payload.as_ref().is_none_or(|payload| {
                    payload
                        .single_tuple_field()
                        .is_some_and(|ty| !ty.contains_nominal_poison())
                })
        }) {
            return None;
        }
        Some(Self {
            kind: VariantOwnerKind::Project { nominal },
            cases,
        })
    }

    pub(crate) fn option(item: TypeKind) -> Self {
        Self {
            kind: VariantOwnerKind::Option { item: item.clone() },
            cases: Box::new([
                PreparedVariantCaseSeed::new(0, Some(item), Some("Some".into())),
                PreparedVariantCaseSeed::new(1, None, Some("None".into())),
            ]),
        }
    }

    pub(crate) fn result(ok: TypeKind, error: TypeKind) -> Self {
        Self {
            kind: VariantOwnerKind::Result {
                ok: ok.clone(),
                error: error.clone(),
            },
            cases: Box::new([
                PreparedVariantCaseSeed::new(0, Some(ok), Some("Ok".into())),
                PreparedVariantCaseSeed::new(1, Some(error), Some("Err".into())),
            ]),
        }
    }

    pub(crate) fn try_character_nominal(
        nominal: CharacterNominalType,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        let cases = names
            .into_iter()
            .enumerate()
            .map(|(ordinal, name)| {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| CheckedVariantOwnerError::CaseOrdinalOverflow)?;
                Ok(PreparedVariantCaseSeed::new(ordinal, None, Some(name)))
            })
            .collect::<Result<_, CheckedVariantOwnerError>>()?;
        Ok(Self {
            kind: VariantOwnerKind::CharacterNominal { nominal },
            cases,
        })
    }

    pub(crate) fn try_environment(
        schema: &EnvironmentEnumSchema,
        ty: &TypeKind,
    ) -> Result<Self, CheckedVariantOwnerError> {
        let kind = match ty {
            TypeKind::AgentResourceBody => VariantOwnerKind::RuntimeBuiltin {
                owner: RuntimeBuiltinVariantIdentity::AgentResourceBody,
                ty: ty.clone(),
            },
            TypeKind::AgentBuiltin(AgentBuiltinType::AgentBinaryEncoding) => {
                VariantOwnerKind::RuntimeBuiltin {
                    owner: RuntimeBuiltinVariantIdentity::AgentBinaryEncoding,
                    ty: ty.clone(),
                }
            }
            _ => VariantOwnerKind::BuiltinClosed {
                nominal: schema.owner().clone(),
                ty: ty.clone(),
            },
        };
        let cases = schema
            .variants()
            .iter()
            .enumerate()
            .map(|(ordinal, variant)| {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| CheckedVariantOwnerError::CaseOrdinalOverflow)?;
                let payload = match variant.payload() {
                    EnumVariantPayload::Unit => None,
                    EnumVariantPayload::Tuple(fields) => Some(VariantPayloadTypeShape::Tuple(
                        fields.clone().into_boxed_slice(),
                    )),
                    EnumVariantPayload::Record(fields) => Some(
                        VariantPayloadTypeShape::try_record(
                            fields
                                .iter()
                                .map(|field| (field.name().to_owned(), field.ty().clone())),
                        )
                        .map_err(|reason| CheckedVariantOwnerError::Payload { ordinal, reason })?,
                    ),
                };
                Ok(PreparedVariantCaseSeed {
                    ordinal,
                    payload,
                    diagnostic_name: Some(variant.name().to_owned()),
                })
            })
            .collect::<Result<_, CheckedVariantOwnerError>>()?;
        Ok(Self { kind, cases })
    }

    pub(crate) fn ty(&self) -> TypeKind {
        self.kind
            .ty_with(|nominal| TypeKind::ProjectNominal(nominal.clone()))
    }

    pub(crate) const fn payload_owner_family(&self) -> crate::types::VariantPayloadOwnerFamily {
        self.kind.payload_owner_family()
    }
    pub(crate) const fn project_nominal(&self) -> Option<&ProjectNominalType> {
        match &self.kind {
            VariantOwnerKind::Project { nominal } => Some(nominal),
            _ => None,
        }
    }
    pub(crate) const fn cases(&self) -> &[PreparedVariantCaseSeed] {
        &self.cases
    }
    pub(crate) fn case(&self, ordinal: u32) -> Option<&PreparedVariantCaseSeed> {
        self.cases.get(usize::try_from(ordinal).ok()?)
    }
    pub(crate) fn case_payload_type(&self, ordinal: u32) -> Option<Option<TypeKind>> {
        let case = self.case(ordinal)?;
        Some(case.payload.clone().map(|shape| {
            TypeKind::VariantPayload(Box::new(VariantPayloadType::from_prepared_case(
                self.kind.payload_owner_family(),
                self.ty(),
                ordinal,
                shape,
            )))
        }))
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.ty())?;
        self.cases.iter().try_for_each(|case| {
            case.payload
                .as_ref()
                .map_or(Ok(()), |payload| payload.visit_types(visitor))
        })
    }

    pub(crate) fn try_map_types<E: From<CheckedVariantOwnerError>>(
        &self,
        map: &mut impl FnMut(&TypeKind) -> Result<TypeKind, E>,
    ) -> Result<Self, E> {
        let kind = match (&self.kind, map(&self.ty())?) {
            (VariantOwnerKind::Project { nominal }, TypeKind::ProjectNominal(mapped))
                if nominal.declaration() == mapped.declaration() =>
            {
                VariantOwnerKind::Project { nominal: mapped }
            }
            (
                VariantOwnerKind::CharacterNominal { nominal },
                TypeKind::CharacterNominal(mapped),
            ) if nominal == &mapped => VariantOwnerKind::CharacterNominal { nominal: mapped },
            (VariantOwnerKind::BuiltinClosed { nominal, .. }, ty) => {
                VariantOwnerKind::BuiltinClosed {
                    nominal: nominal.clone(),
                    ty,
                }
            }
            (VariantOwnerKind::RuntimeBuiltin { owner, .. }, ty) => {
                VariantOwnerKind::RuntimeBuiltin { owner: *owner, ty }
            }
            (VariantOwnerKind::Option { .. }, TypeKind::Option(item)) => {
                VariantOwnerKind::Option { item: *item }
            }
            (VariantOwnerKind::Result { .. }, TypeKind::Result { ok, error }) => {
                VariantOwnerKind::Result {
                    ok: *ok,
                    error: *error,
                }
            }
            _ => return Err(CheckedVariantOwnerError::InvalidOwnerProjection.into()),
        };
        let cases = self
            .cases
            .iter()
            .map(|case| {
                Ok(PreparedVariantCaseSeed {
                    ordinal: case.ordinal,
                    payload: case
                        .payload
                        .as_ref()
                        .map(|payload| payload.try_map(map))
                        .transpose()?,
                    diagnostic_name: case.diagnostic_name.clone(),
                })
            })
            .collect::<Result<_, E>>()?;
        Ok(Self { kind, cases })
    }

    pub(crate) fn seal(
        self,
        project: impl FnOnce(
            ProjectNominalType,
            &[PreparedVariantCaseSeed],
        ) -> Result<CheckedProjectNominal, CheckedVariantOwnerError>,
    ) -> Result<CheckedVariantOwner, CheckedVariantOwnerError> {
        let expected = self.ty();
        let kind = self
            .kind
            .try_map_project(|nominal| project(nominal, &self.cases))?;
        if kind.ty() != expected {
            return Err(CheckedVariantOwnerError::MissingProjectDefinition);
        }
        CheckedVariantOwner::seal(kind, |family, identity| {
            self.cases
                .into_vec()
                .into_iter()
                .map(|case| {
                    let payload =
                        match case.payload {
                            None => VariantPayloadShape::Unit,
                            Some(shape) => shape.try_seal(family, identity, case.ordinal).map_err(
                                |reason| CheckedVariantOwnerError::Payload {
                                    ordinal: case.ordinal,
                                    reason,
                                },
                            )?,
                        };
                    CheckedVariantCase::seal(
                        family,
                        identity,
                        case.ordinal,
                        payload,
                        case.diagnostic_name,
                    )
                })
                .collect()
        })
    }

    pub(super) fn seal_intrinsic(self) -> Result<CheckedVariantOwner, CheckedVariantOwnerError> {
        self.seal(|_, _| Err(CheckedVariantOwnerError::MissingProjectDefinition))
    }
}
