//! Variant schemas retained during source probing, before stable identities.

use super::{
    CheckedProjectNominal, CheckedVariantCase, CheckedVariantOwner, CheckedVariantOwnerError,
    VariantOwnerKind,
};
use crate::env::{EnumVariantPayload, EnvironmentEnumSchema};
use crate::types::{
    CanonicalEffects, CharacterNominalType, ProjectNominalType, TypeEffectDomain, TypeKind,
    VariantPayloadSealError, VariantPayloadShape, VariantPayloadType, VariantPayloadTypeShape,
};
use arcweft_core::pattern::RuntimeBuiltinVariantIdentity;
use arcweft_lang_hir::{
    identity::TypeId,
    symbol::nominal::{ProjectNominalVariant, ProjectNominalVariantPayload},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantCaseSeed<D: TypeEffectDomain = CanonicalEffects> {
    ordinal: u32,
    payload: Option<VariantPayloadTypeShape<D>>,
    diagnostic_name: Option<String>,
}

impl<D: TypeEffectDomain> PreparedVariantCaseSeed<D> {
    pub(crate) fn new(
        ordinal: u32,
        payload: Option<TypeKind<D>>,
        diagnostic_name: Option<String>,
    ) -> Self {
        Self {
            ordinal,
            payload: payload.map(|ty| VariantPayloadTypeShape::Tuple(Box::new([ty]))),
            diagnostic_name,
        }
    }

    pub(crate) fn from_project<E>(
        ordinal: u32,
        variant: &ProjectNominalVariant,
        mut resolve_type: impl FnMut(TypeId) -> Result<TypeKind<D>, E>,
        invalid_shape: impl FnOnce(VariantPayloadSealError) -> E,
    ) -> Result<Self, E> {
        let payload = match variant.payload() {
            ProjectNominalVariantPayload::Unit => None,
            ProjectNominalVariantPayload::Tuple(owner) => Some(VariantPayloadTypeShape::Tuple(
                Box::new([resolve_type(*owner)?]),
            )),
            ProjectNominalVariantPayload::Record(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| Ok((field.name().as_str().to_owned(), resolve_type(field.ty())?)))
                    .collect::<Result<Vec<_>, E>>()?;
                Some(VariantPayloadTypeShape::try_record(fields).map_err(invalid_shape)?)
            }
        };
        Ok(Self {
            ordinal,
            payload,
            diagnostic_name: Some(variant.name().as_str().to_owned()),
        })
    }

    pub(crate) const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) const fn payload(&self) -> Option<&VariantPayloadTypeShape<D>> {
        self.payload.as_ref()
    }
    pub(crate) fn diagnostic_name(&self) -> Option<&str> {
        self.diagnostic_name.as_deref()
    }
}

/// Complete source-owned case inventory. Project arguments and intrinsic
/// payloads may contain active types; this carrier issues no stable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedVariantOwnerSeed<D: TypeEffectDomain = CanonicalEffects> {
    kind: VariantOwnerKind<ProjectNominalType<D>, D>,
    cases: Box<[PreparedVariantCaseSeed<D>]>,
}

impl<D: TypeEffectDomain> PreparedVariantOwnerSeed<D> {
    /// Build the original environment case inventory in the requested type
    /// domain. The same control pays for metadata and each projected field
    /// before that field is copied into the new owner.
    pub(crate) fn try_environment_with<C, E>(
        schema: &EnvironmentEnumSchema,
        ty: TypeKind<D>,
        control: &mut C,
        mut project: impl FnMut(&TypeKind, &mut C) -> Result<TypeKind<D>, E>,
    ) -> Result<Self, E>
    where
        C: crate::types::TypeProjectionControl,
        E: From<CheckedVariantOwnerError> + From<C::Error>,
    {
        control.check().map_err(E::from)?;
        control.visit_binding().map_err(E::from)?;
        let runtime_owner = match &ty {
            TypeKind::AgentResourceBody => Some(RuntimeBuiltinVariantIdentity::AgentResourceBody),
            TypeKind::AgentBuiltin(builtin) => builtin.runtime_variant(),
            _ => None,
        };
        let kind = match runtime_owner {
            Some(owner) => VariantOwnerKind::RuntimeBuiltin { owner, ty },
            None => VariantOwnerKind::BuiltinClosed {
                nominal: schema.owner().clone(),
                ty,
            },
        };
        let mut cases = Vec::new();
        for (ordinal, variant) in schema.variants().iter().enumerate() {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| E::from(CheckedVariantOwnerError::CaseOrdinalOverflow))?;
            control.visit_binding().map_err(E::from)?;
            let payload = match variant.payload() {
                EnumVariantPayload::Unit => None,
                EnumVariantPayload::Tuple(fields) => {
                    for _ in fields {
                        control.visit_binding().map_err(E::from)?;
                    }
                    Some(VariantPayloadTypeShape::Tuple(
                        fields
                            .iter()
                            .map(|field| project(field, control))
                            .collect::<Result<_, E>>()?,
                    ))
                }
                EnumVariantPayload::Record(fields) => {
                    for _ in fields {
                        control.visit_binding().map_err(E::from)?;
                    }
                    let projected = fields
                        .iter()
                        .map(|field| Ok((field.name().to_owned(), project(field.ty(), control)?)))
                        .collect::<Result<Vec<_>, E>>()?;
                    Some(
                        VariantPayloadTypeShape::try_record(projected).map_err(|reason| {
                            E::from(CheckedVariantOwnerError::Payload { ordinal, reason })
                        })?,
                    )
                }
            };
            cases.push(PreparedVariantCaseSeed {
                ordinal,
                payload,
                diagnostic_name: Some(variant.name().to_owned()),
            });
        }
        Ok(Self {
            kind,
            cases: cases.into_boxed_slice(),
        })
    }

    pub(crate) fn try_project(
        nominal: ProjectNominalType<D>,
        cases: impl Into<Box<[PreparedVariantCaseSeed<D>]>>,
    ) -> Option<Self> {
        let cases = cases.into();
        if !cases.iter().enumerate().all(|(ordinal, case)| {
            u32::try_from(ordinal).is_ok_and(|ordinal| ordinal == case.ordinal)
                && case.payload.as_ref().is_none_or(|payload| {
                    payload
                        .visit_types(&mut |ty| {
                            (!ty.contains_nominal_poison()).then_some(()).ok_or(())
                        })
                        .is_ok()
                })
        }) {
            return None;
        }
        Some(Self {
            kind: VariantOwnerKind::Project { nominal },
            cases,
        })
    }

    pub(crate) fn option(item: TypeKind<D>) -> Self {
        Self {
            kind: VariantOwnerKind::Option { item: item.clone() },
            cases: Box::new([
                PreparedVariantCaseSeed::new(0, Some(item), Some("Some".into())),
                PreparedVariantCaseSeed::new(1, None, Some("None".into())),
            ]),
        }
    }

    pub(crate) fn result(ok: TypeKind<D>, error: TypeKind<D>) -> Self {
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

    pub(crate) fn ty(&self) -> TypeKind<D> {
        self.kind
            .ty_with(|nominal| TypeKind::ProjectNominal(nominal.clone()))
    }

    pub(crate) const fn payload_owner_family(&self) -> crate::types::VariantPayloadOwnerFamily {
        self.kind.payload_owner_family()
    }
    pub(crate) const fn project_nominal(&self) -> Option<&ProjectNominalType<D>> {
        match &self.kind {
            VariantOwnerKind::Project { nominal } => Some(nominal),
            _ => None,
        }
    }
    pub(crate) const fn cases(&self) -> &[PreparedVariantCaseSeed<D>] {
        &self.cases
    }
    pub(crate) fn case(&self, ordinal: u32) -> Option<&PreparedVariantCaseSeed<D>> {
        self.cases.get(usize::try_from(ordinal).ok()?)
    }
    pub(crate) fn case_payload_type(&self, ordinal: u32) -> Option<Option<TypeKind<D>>> {
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

    /// The enclosing expression or pattern already owns the variant type.
    pub(crate) fn visit_payload_types<'a, E>(
        &'a self,
        visitor: &mut impl FnMut(&'a TypeKind<D>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.cases.iter().try_for_each(|case| {
            case.payload
                .as_ref()
                .map_or(Ok(()), |payload| payload.visit_types(visitor))
        })
    }

    pub(crate) fn try_map_types<T: TypeEffectDomain, E: From<CheckedVariantOwnerError>>(
        &self,
        map: &mut impl FnMut(&TypeKind<D>) -> Result<TypeKind<T>, E>,
    ) -> Result<PreparedVariantOwnerSeed<T>, E> {
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
        Ok(PreparedVariantOwnerSeed { kind, cases })
    }
}

// Stable case and field identities are issued only from canonical types.
impl PreparedVariantOwnerSeed {
    pub(crate) fn try_environment(
        schema: &EnvironmentEnumSchema,
        ty: &TypeKind,
    ) -> Result<Self, CheckedVariantOwnerError> {
        Self::try_environment_with(
            schema,
            ty.clone(),
            &mut crate::types::UnmeteredTypeProjection,
            |ty, _| Ok::<_, CheckedVariantOwnerError>(ty.clone()),
        )
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
                    let payload = match case.payload {
                        None => VariantPayloadShape::Unit,
                        Some(shape) => shape
                            .try_seal(
                                family,
                                identity,
                                case.ordinal,
                                &crate::types::GenericScope::default(),
                            )
                            .map_err(|reason| CheckedVariantOwnerError::Payload {
                                ordinal: case.ordinal,
                                reason,
                            })?,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::{
        PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow,
    };
    use crate::effect_row::{EffectPredicate, EffectRow};
    use crate::effects::EffectSet;
    use crate::types::constraints::context::{
        LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits,
    };
    use crate::types::constraints::{NoConstraintClient, TypeConstraintError};
    use crate::types::{
        GenericBinder, GenericScope, PreparedEffects, PreparedTypeEffectProjectionFailure,
        TypeProjectionError,
    };
    use std::sync::atomic::AtomicBool;

    #[derive(Debug, thiserror::Error)]
    enum MappingFailure {
        #[error(transparent)]
        Owner(#[from] CheckedVariantOwnerError),
        #[error(transparent)]
        Projection(
            #[from] TypeProjectionError<PreparedTypeEffectProjectionFailure<TypeConstraintError>>,
        ),
    }

    #[test]
    fn nested_variant_payload_retains_a_live_arrow_until_original_body_closure() {
        let cancellation = AtomicBool::new(false);
        let mut work = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(100_000, 100_000, 128, 128),
                &cancellation,
            ),
        );
        let mut graph = PreparedEffectGraph::default();
        let scheme = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let row = graph.application(&scheme, vec![], &mut work).unwrap();
        let callback = TypeKind::<PreparedEffects>::Function {
            binder: GenericBinder::EMPTY,
            predicate: PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            params: vec![TypeKind::I64],
            return_type: Box::new(TypeKind::I64),
            effects: row.clone(),
        };
        let source = PreparedVariantOwnerSeed::result(
            TypeKind::Option(Box::new(callback)),
            TypeKind::String,
        );
        let TypeKind::VariantPayload(payload) = source.case_payload_type(0).unwrap().unwrap()
        else {
            panic!("the actual Result case owns its payload");
        };
        assert_eq!(payload.owner_type(), &source.ty());
        let TypeKind::Option(callback) = payload.stored_fields().next().unwrap() else {
            panic!("nested Option payload");
        };
        let TypeKind::Function { effects, .. } = callback.as_ref() else {
            panic!("nested callback");
        };
        // Equality here checks retention of the same operational handle. It
        // does not decide source type equivalence or candidate ranking.
        assert_eq!(effects, &row);
        assert!(matches!(effects, PreparedEffectRow::Term(_)));

        let body = EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap());
        graph
            .prepare_body(
                &scheme,
                PreparedEffectRow::Canonical(body.clone()),
                vec![],
                &mut work,
            )
            .unwrap();
        let principal = graph.solve_principal(&mut work).unwrap();
        let canonical = source
            .try_map_types::<CanonicalEffects, MappingFailure>(&mut |ty| {
                ty.project_canonical_effects(&GenericScope::default(), &principal, &mut work)
                    .map_err(MappingFailure::from)
            })
            .unwrap();
        let checked = canonical.seal_intrinsic().unwrap();
        assert_eq!(checked.cases().len(), 2);
        let TypeKind::Result { ok, error } = checked.ty() else {
            panic!("closed Result owner");
        };
        assert_eq!(*error, TypeKind::String);
        let TypeKind::Option(callback) = *ok else {
            panic!("closed nested Option");
        };
        let TypeKind::Function { effects, .. } = *callback else {
            panic!("closed callback");
        };
        assert_eq!(effects, body);
    }
}
