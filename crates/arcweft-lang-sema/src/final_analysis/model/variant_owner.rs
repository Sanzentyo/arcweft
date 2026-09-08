//! Complete checked variant owners and the case selections bound to them.

use super::CheckedProjectNominal;
use crate::env::EnvironmentEnumSchema;
use crate::env::identity::EnvironmentBindingId;
use crate::types::{
    AcceptedVariantCaseSemanticId, CharacterNominalType, GenericScopeError, SemanticTypeDigest,
    TypeKind, VariantPayloadOwnerFamily, VariantPayloadSealError, VariantPayloadShape,
    VariantPayloadType,
};
use arcweft_core::pattern::RuntimeBuiltinVariantIdentity;
use thiserror::Error;

#[path = "variant_owner/prepared.rs"]
mod prepared;
pub(crate) use prepared::{PreparedVariantCaseSeed, PreparedVariantOwnerSeed};

/// One declaration-ordered case retained by its complete checked owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantCase {
    ordinal: u32,
    semantic_id: AcceptedVariantCaseSemanticId,
    payload: VariantPayloadShape,
    diagnostic_name: Option<String>,
}

impl CheckedVariantCase {
    fn seal(
        family: VariantPayloadOwnerFamily,
        semantic_type: SemanticTypeDigest,
        ordinal: u32,
        payload: VariantPayloadShape,
        diagnostic_name: Option<String>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        if !payload.has_valid_rows(family, semantic_type, ordinal) {
            return Err(CheckedVariantOwnerError::Payload {
                ordinal,
                reason: VariantPayloadSealError::InvalidFieldRows,
            });
        }
        Ok(Self {
            ordinal,
            semantic_id: AcceptedVariantCaseSemanticId::issue(
                family,
                semantic_type,
                ordinal,
                &payload,
            ),
            payload,
            diagnostic_name,
        })
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub(crate) const fn semantic_id(&self) -> AcceptedVariantCaseSemanticId {
        self.semantic_id
    }

    pub const fn payload(&self) -> &VariantPayloadShape {
        &self.payload
    }

    pub fn diagnostic_name(&self) -> Option<&str> {
        self.diagnostic_name.as_deref()
    }

    fn payload_type(
        &self,
        owner_family: VariantPayloadOwnerFamily,
        owner_type: TypeKind,
    ) -> Option<Option<TypeKind>> {
        if self.payload.is_unit() {
            return Some(None);
        }
        VariantPayloadType::try_new(
            owner_family,
            owner_type,
            self.ordinal,
            self.semantic_id,
            self.payload.clone(),
        )
        .ok()
        .map(|payload| Some(TypeKind::VariantPayload(Box::new(payload))))
    }
}

/// Typed origin of a checked variant owner. This descriptor does not mint an owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantOwnerKind<N> {
    Project {
        nominal: N,
    },
    CharacterNominal {
        nominal: CharacterNominalType,
    },
    BuiltinClosed {
        nominal: EnvironmentBindingId,
        ty: TypeKind,
    },
    RuntimeBuiltin {
        owner: RuntimeBuiltinVariantIdentity,
        ty: TypeKind,
    },
    Option {
        item: TypeKind,
    },
    Result {
        ok: TypeKind,
        error: TypeKind,
    },
}

/// Descriptor whose project nominal has completed semantic sealing.
pub type CheckedVariantOwnerKind = VariantOwnerKind<CheckedProjectNominal>;

impl<N> VariantOwnerKind<N> {
    pub(crate) const fn payload_owner_family(&self) -> VariantPayloadOwnerFamily {
        match self {
            Self::Project { .. } => VariantPayloadOwnerFamily::Project,
            Self::CharacterNominal { .. } => VariantPayloadOwnerFamily::CharacterNominal,
            Self::BuiltinClosed { .. } => VariantPayloadOwnerFamily::BuiltinClosed,
            Self::RuntimeBuiltin { .. } => VariantPayloadOwnerFamily::RuntimeBuiltin,
            Self::Option { .. } => VariantPayloadOwnerFamily::Option,
            Self::Result { .. } => VariantPayloadOwnerFamily::Result,
        }
    }

    fn ty_with(&self, project: impl FnOnce(&N) -> TypeKind) -> TypeKind {
        match self {
            Self::Project { nominal } => project(nominal),
            Self::CharacterNominal { nominal } => TypeKind::CharacterNominal(nominal.clone()),
            Self::BuiltinClosed { ty, .. } | Self::RuntimeBuiltin { ty, .. } => ty.clone(),
            Self::Option { item } => TypeKind::Option(Box::new(item.clone())),
            Self::Result { ok, error } => TypeKind::Result {
                ok: Box::new(ok.clone()),
                error: Box::new(error.clone()),
            },
        }
    }

    fn try_map_project<M, E>(
        self,
        project: impl FnOnce(N) -> Result<M, E>,
    ) -> Result<VariantOwnerKind<M>, E> {
        Ok(match self {
            Self::Project { nominal } => VariantOwnerKind::Project {
                nominal: project(nominal)?,
            },
            Self::CharacterNominal { nominal } => VariantOwnerKind::CharacterNominal { nominal },
            Self::BuiltinClosed { nominal, ty } => VariantOwnerKind::BuiltinClosed { nominal, ty },
            Self::RuntimeBuiltin { owner, ty } => VariantOwnerKind::RuntimeBuiltin { owner, ty },
            Self::Option { item } => VariantOwnerKind::Option { item },
            Self::Result { ok, error } => VariantOwnerKind::Result { ok, error },
        })
    }
}

impl CheckedVariantOwnerKind {
    fn ty(&self) -> TypeKind {
        self.ty_with(CheckedProjectNominal::ty)
    }
}

/// Failure to seal a complete owner and its declaration-ordered payload rows.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum CheckedVariantOwnerError {
    #[error(transparent)]
    GenericScope(#[from] GenericScopeError),
    #[error("checked variant owner type contains nominal poison")]
    PoisonedOwnerType,
    #[error("checked variant owner identity {expected:?} differs from its type {actual:?}")]
    IdentityMismatch {
        expected: SemanticTypeDigest,
        actual: SemanticTypeDigest,
    },
    #[error("checked variant case ordinal overflow")]
    CaseOrdinalOverflow,
    #[error("variant owner requires its exact accepted project nominal definition")]
    MissingProjectDefinition,
    #[error("type projection changed the variant owner's declaration or family")]
    InvalidOwnerProjection,
    #[error("checked variant case {ordinal} has an invalid payload: {reason}")]
    Payload {
        ordinal: u32,
        #[source]
        reason: VariantPayloadSealError,
    },
}

/// Exact semantic owner with one sealed, immutable case inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantOwner {
    kind: CheckedVariantOwnerKind,
    semantic_type: SemanticTypeDigest,
    cases: Box<[CheckedVariantCase]>,
}

impl CheckedVariantOwner {
    fn seal(
        kind: CheckedVariantOwnerKind,
        build_cases: impl FnOnce(
            VariantPayloadOwnerFamily,
            SemanticTypeDigest,
        ) -> Result<Box<[CheckedVariantCase]>, CheckedVariantOwnerError>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        let ty = kind.ty();
        if ty.contains_nominal_poison() {
            return Err(CheckedVariantOwnerError::PoisonedOwnerType);
        }
        let semantic_type = ty.semantic_identity_digest()?;
        if let CheckedVariantOwnerKind::Project { nominal } = &kind
            && nominal.identity() != semantic_type
        {
            return Err(CheckedVariantOwnerError::IdentityMismatch {
                expected: nominal.identity(),
                actual: semantic_type,
            });
        }
        let cases = build_cases(kind.payload_owner_family(), semantic_type)?;
        Ok(Self {
            kind,
            semantic_type,
            cases,
        })
    }

    #[cfg(test)]
    fn from_payloads(
        kind: CheckedVariantOwnerKind,
        cases: impl IntoIterator<Item = (Option<TypeKind>, Option<String>)>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        Self::seal(kind, |family, semantic_type| {
            cases
                .into_iter()
                .enumerate()
                .map(|(ordinal, (payload, name))| {
                    let ordinal = u32::try_from(ordinal)
                        .map_err(|_| CheckedVariantOwnerError::CaseOrdinalOverflow)?;
                    let payload = match payload {
                        None => VariantPayloadShape::Unit,
                        Some(ty) => {
                            VariantPayloadShape::try_tuple(family, semantic_type, ordinal, [ty])
                                .map_err(|reason| CheckedVariantOwnerError::Payload {
                                    ordinal,
                                    reason,
                                })?
                        }
                    };
                    CheckedVariantCase::seal(family, semantic_type, ordinal, payload, name)
                })
                .collect()
        })
    }

    pub(crate) fn try_project_shapes(
        nominal: CheckedProjectNominal,
        cases: impl IntoIterator<Item = (VariantPayloadShape, Option<String>)>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        Self::seal(
            CheckedVariantOwnerKind::Project { nominal },
            |family, semantic_type| {
                cases
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, (payload, name))| {
                        let ordinal = u32::try_from(ordinal)
                            .map_err(|_| CheckedVariantOwnerError::CaseOrdinalOverflow)?;
                        CheckedVariantCase::seal(family, semantic_type, ordinal, payload, name)
                    })
                    .collect()
            },
        )
    }

    pub(crate) fn try_character_nominal(
        nominal: CharacterNominalType,
        names: impl IntoIterator<Item = String>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        PreparedVariantOwnerSeed::try_character_nominal(nominal, names)?.seal_intrinsic()
    }

    #[cfg(test)]
    pub(crate) fn try_builtin_closed(
        nominal: EnvironmentBindingId,
        ty: TypeKind,
        cases: impl IntoIterator<Item = (Option<TypeKind>, Option<String>)>,
    ) -> Result<Self, CheckedVariantOwnerError> {
        Self::from_payloads(
            CheckedVariantOwnerKind::BuiltinClosed { nominal, ty },
            cases,
        )
    }

    /// Seals the exact environment case order, payloads, and runtime owner.
    pub(crate) fn try_environment(
        schema: &EnvironmentEnumSchema,
        ty: &TypeKind,
    ) -> Result<Self, CheckedVariantOwnerError> {
        PreparedVariantOwnerSeed::try_environment(schema, ty)?.seal_intrinsic()
    }

    /// Seals both `Option` cases for an item type with stable generic references.
    pub fn try_option(item: TypeKind) -> Result<Self, CheckedVariantOwnerError> {
        PreparedVariantOwnerSeed::option(item).seal_intrinsic()
    }

    /// Seals both `Result` payload cases with stable generic references.
    pub fn try_result(ok: TypeKind, error: TypeKind) -> Result<Self, CheckedVariantOwnerError> {
        PreparedVariantOwnerSeed::result(ok, error).seal_intrinsic()
    }
    pub const fn kind(&self) -> &CheckedVariantOwnerKind {
        &self.kind
    }

    pub const fn project(&self) -> Option<&CheckedProjectNominal> {
        match &self.kind {
            CheckedVariantOwnerKind::Project { nominal } => Some(nominal),
            _ => None,
        }
    }

    pub fn cases(&self) -> &[CheckedVariantCase] {
        &self.cases
    }

    pub const fn semantic_type(&self) -> SemanticTypeDigest {
        self.semantic_type
    }

    /// Projects the type whose identity was validated when this owner was sealed.
    pub fn ty(&self) -> TypeKind {
        self.kind.ty()
    }

    pub(crate) const fn payload_owner_family(&self) -> VariantPayloadOwnerFamily {
        self.kind.payload_owner_family()
    }

    pub fn case(&self, ordinal: u32) -> Option<&CheckedVariantCase> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|index| self.cases.get(index))
    }

    pub fn case_payload_type(&self, ordinal: u32) -> Option<Option<TypeKind>> {
        self.case(ordinal)?
            .payload_type(self.payload_owner_family(), self.ty())
    }

    pub(crate) fn has_same_diagnostic_schema(&self, other: &Self) -> bool {
        self == other
            && self
                .cases
                .iter()
                .zip(&other.cases)
                .all(|(left, right)| left.payload.has_same_diagnostic_schema(&right.payload))
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(&self.ty())?;
        self.cases
            .iter()
            .try_for_each(|case| case.payload.visit_types(visitor))
    }
}

/// Checked enum case selected for an expression or pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedVariantResolution {
    owner: CheckedVariantOwner,
    selected_ordinal: u32,
}

impl CheckedVariantResolution {
    pub(crate) fn try_new(owner: CheckedVariantOwner, selected_ordinal: u32) -> Option<Self> {
        owner.case(selected_ordinal)?;
        Some(Self {
            owner,
            selected_ordinal,
        })
    }

    pub const fn owner(&self) -> &CheckedVariantOwner {
        &self.owner
    }

    pub const fn ordinal(&self) -> u32 {
        self.selected_ordinal
    }

    /// Returns the exact row retained by the private selection constructor.
    pub fn selected(&self) -> &CheckedVariantCase {
        self.owner
            .case(self.selected_ordinal)
            .expect("checked variant resolution retains one exact owner case")
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.owner.visit_types(visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_rows_cannot_move_between_owners_or_case_ordinals() {
        let source = CheckedVariantOwner::try_option(TypeKind::I64).expect("source owner");
        let other = CheckedVariantOwner::try_option(TypeKind::String).expect("other owner");
        for (owner_type, ordinal) in [(other.semantic_type(), 0), (source.semantic_type(), 1)] {
            assert_eq!(
                CheckedVariantCase::seal(
                    VariantPayloadOwnerFamily::Option,
                    owner_type,
                    ordinal,
                    source.cases()[0].payload().clone(),
                    Some("Some".into()),
                ),
                Err(CheckedVariantOwnerError::Payload {
                    ordinal,
                    reason: VariantPayloadSealError::InvalidFieldRows,
                }),
            );
        }
    }
}
