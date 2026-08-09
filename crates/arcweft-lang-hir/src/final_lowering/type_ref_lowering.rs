//! Direct attached `TypeRef` lowering into the final qualified type arena.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::AttachedTypeRefNode;
use arcweft_lang_syntax::reference::{BorrowKind, ReferenceType, RegionSyntax};
use arcweft_lang_syntax::types::{
    TraitBound, TypeEffectRow, TypeRef, TypeRefComponentRole, TypeRefNodeStep, TypeRefRegionPart,
};

use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{HirBorrowKind, HirPoisonState, HirRecoveryIssue};
use crate::identity::{HirLimit, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId};
use crate::leaf::{HirElidedRegion, HirName, HirRegionName, HirTypeRegion, HirTypeRegionIssue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::{
    HirSourceQuery, HirSourceSite, HirTypeRegionSourcePart, HirTypeSourceRole,
};
use crate::type_ref::{
    HirAssociatedTypeBinding, HirEffectName, HirFunctionType, HirGenericType, HirGenericTypeIssue,
    HirProjectionType, HirReferenceType, HirTraitBoundType, HirType, HirTypeEffectRow,
    HirTypeError, HirTypeKind,
};

use super::path_projection::project_type_path;
use super::{StagedHirModuleTransaction, require_limit};

impl StagedHirModuleTransaction<'_> {
    /// Lowers one exact attached semantic `TypeRef` into this transaction.
    ///
    /// The source-backed owner is reserved before any child. Every nested
    /// `TypeId`, source-role entry, and key-only elided region is staged through
    /// this same transaction; no detached tree or source spelling is read.
    pub(crate) fn lower_attached_type(
        &mut self,
        attached: &AttachedTypeRefNode,
        scope: ScopeId,
    ) -> Result<TypeId, HirLowerFailure> {
        let result = self.lower_attached_type_inner(attached, scope);
        if result.is_err() {
            self.slots.poison();
        }
        result
    }

    fn lower_attached_type_inner(
        &mut self,
        attached: &AttachedTypeRefNode,
        scope: ScopeId,
    ) -> Result<TypeId, HirLowerFailure> {
        if attached.snapshot_id() != self.request.source().snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: self.request.source().snapshot_id().clone(),
                supplied: attached.snapshot_id().clone(),
            });
        }
        let source_span = attached.whole_source_span();
        if source_span.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: source_span.source().clone(),
            });
        }
        let reservation = self.arenas.types().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(source_span),
        )?;
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            let retained = self
                .arenas
                .types()
                .resolve_staged(&self.slots, owner)
                .map_err(HirLowerFailure::from)?;
            if retained.scope() != scope {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            return Ok(owner);
        }

        let mut children = BTreeMap::new();
        for child in attached
            .children()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            let id = self.lower_attached_type_inner(child.node(), scope)?;
            if children.insert(child.step(), id).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }

        let (kind, state) = self.project_type(owner, attached.value(), &children)?;
        let recovery = match &state {
            HirPoisonState::Poisoned(issue) => Some(issue.clone()),
            HirPoisonState::Clean => None,
        };
        let payload = HirType::try_new(owner, kind, scope, state, self)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        self.source_components
            .stage_attached_type(self.request.source(), owner, attached)?;
        if let Some(recovery) = recovery {
            let (component, role) = match recovery {
                HirRecoveryIssue::InvalidType(_) => {
                    (TypeRefComponentRole::Recovery, HirTypeSourceRole::Recovery)
                }
                HirRecoveryIssue::InvalidTypeRegion(HirTypeRegionIssue::InvalidNamedRegion) => (
                    TypeRefComponentRole::Region(TypeRefRegionPart::NamedName),
                    HirTypeSourceRole::Region(HirTypeRegionSourcePart::NamedName),
                ),
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            };
            let primary_span = attached
                .component(component)
                .ok_or(HirInvariantFailure::InvalidSourceIndex)?;
            let primary_site =
                HirSourceSite::from_attached_span(self.request.source().document(), &primary_span)
                    .map_err(|_| HirInvariantFailure::InvalidSourceIndex)?;
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                SyntheticOwner::Type(owner),
                HirRecoveryPrimary::query(HirSourceQuery::Type { owner, role }),
                primary_site,
            ));
        }

        self.arenas
            .types()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    pub(super) fn project_type(
        &mut self,
        owner: TypeId,
        value: &TypeRef,
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
    ) -> Result<(HirTypeKind, HirPoisonState), HirLowerFailure> {
        let clean = HirPoisonState::Clean;
        let kind = match value {
            TypeRef::Never => HirTypeKind::Never,
            TypeRef::ConstInt(value) => HirTypeKind::ConstInt(*value),
            TypeRef::Path(path) => HirTypeKind::Path(project_type_path(path)?),
            TypeRef::Tuple(elements) => HirTypeKind::Tuple(Self::indexed_children(
                children,
                elements.len(),
                TypeRefNodeStep::TupleItem,
            )?),
            TypeRef::Function {
                params, effects, ..
            } => HirTypeKind::Function(Self::project_function_type(
                params.len(),
                effects.as_ref(),
                children,
            )?),
            TypeRef::Choice(alternatives) => HirTypeKind::Choice(Self::indexed_children(
                children,
                alternatives.len(),
                TypeRefNodeStep::ChoiceAlternative,
            )?),
            TypeRef::Generic { base, args } => HirTypeKind::Generic(HirGenericType::new(
                project_type_path(base)?,
                Self::indexed_children(children, args.len(), TypeRefNodeStep::GenericArgument)?,
            )),
            TypeRef::TraitBound(bound) => {
                HirTypeKind::TraitBound(Self::project_trait_bound(bound, children)?)
            }
            TypeRef::Projection { assoc, .. } => HirTypeKind::Projection(HirProjectionType::new(
                Self::required_child(children, TypeRefNodeStep::ProjectionSubject)?,
                Self::project_name(assoc.as_str())?,
            )),
            TypeRef::Reference(reference) => {
                let (reference, state) = self.project_reference_type(owner, reference, children)?;
                return Ok((HirTypeKind::Reference(reference), state));
            }
            TypeRef::Slice(_) => {
                HirTypeKind::Slice(Self::required_child(children, TypeRefNodeStep::SliceItem)?)
            }
            TypeRef::Recovery(_) => {
                let issue = HirGenericTypeIssue::UnclassifiedSyntax;
                return Ok((
                    HirTypeKind::Recovery(HirTypeError::new(issue)),
                    HirPoisonState::Poisoned(HirRecoveryIssue::InvalidType(issue)),
                ));
            }
        };
        Ok((kind, clean))
    }

    fn project_function_type(
        parameter_count: usize,
        effects: Option<&TypeEffectRow>,
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
    ) -> Result<HirFunctionType, HirLowerFailure> {
        let parameters = Self::indexed_children(
            children,
            parameter_count,
            TypeRefNodeStep::FunctionParameter,
        )?;
        let return_type = Self::required_child(children, TypeRefNodeStep::FunctionReturn)?;
        let effects = effects
            .map(|row| {
                row.effects()
                    .iter()
                    .map(|effect| Self::project_effect_name(effect.as_str()))
                    .collect::<Result<Vec<_>, HirLowerFailure>>()
                    .map(HirTypeEffectRow::new)
            })
            .transpose()?;
        Ok(HirFunctionType::new(parameters, return_type, effects))
    }

    fn project_trait_bound(
        bound: &TraitBound,
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
    ) -> Result<HirTraitBoundType, HirLowerFailure> {
        let arguments =
            Self::indexed_children(children, bound.args().len(), TypeRefNodeStep::TraitArgument)?;
        let associated = bound
            .associated()
            .iter()
            .enumerate()
            .map(|(index, binding)| {
                let ordinal =
                    u16::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok(HirAssociatedTypeBinding::new(
                    Self::project_name(binding.name().as_str())?,
                    Self::required_child(children, TypeRefNodeStep::AssociatedBinding(ordinal))?,
                ))
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        Ok(HirTraitBoundType::new(
            project_type_path(bound.path())?,
            arguments,
            associated,
        ))
    }

    fn project_reference_type(
        &mut self,
        owner: TypeId,
        reference: &ReferenceType,
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
    ) -> Result<(HirReferenceType, HirPoisonState), HirLowerFailure> {
        let referent = Self::required_child(children, TypeRefNodeStep::ReferenceReferent)?;
        let (region, state) = match reference.region() {
            RegionSyntax::Named { name, .. } => {
                require_limit(HirLimit::NameBytes, name.name().len())?;
                match HirName::try_new(name.name().into()) {
                    Ok(name) => (
                        Some(HirTypeRegion::named(HirRegionName::new(name))),
                        HirPoisonState::Clean,
                    ),
                    Err(_) => (
                        None,
                        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidTypeRegion(
                            HirTypeRegionIssue::InvalidNamedRegion,
                        )),
                    ),
                }
            }
            RegionSyntax::Elided { .. } => {
                let key = SyntheticKey::try_new(
                    SyntheticOwner::Type(owner),
                    SyntheticRole::ElidedRegion,
                    0,
                )
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
                self.slots.stage_elided_region_key(key)?;
                (
                    Some(HirTypeRegion::elided(
                        HirElidedRegion::try_new(owner, key)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    )),
                    HirPoisonState::Clean,
                )
            }
        };
        let kind = match reference.kind() {
            BorrowKind::Shared => HirBorrowKind::Shared,
            BorrowKind::Mutable => HirBorrowKind::Mutable,
        };
        Ok((HirReferenceType::new(kind, region, referent), state))
    }

    fn project_name(value: &str) -> Result<HirName, HirLowerFailure> {
        require_limit(HirLimit::NameBytes, value.len())?;
        HirName::try_new(value.into()).map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn project_effect_name(value: &str) -> Result<HirEffectName, HirLowerFailure> {
        require_limit(HirLimit::NameBytes, value.len())?;
        HirEffectName::try_new(value).map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn indexed_children(
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
        len: usize,
        step: fn(u16) -> TypeRefNodeStep,
    ) -> Result<Box<[TypeId]>, HirLowerFailure> {
        (0..len)
            .map(|index| {
                let index =
                    u16::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Self::required_child(children, step(index))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn required_child(
        children: &BTreeMap<TypeRefNodeStep, TypeId>,
        step: TypeRefNodeStep,
    ) -> Result<TypeId, HirLowerFailure> {
        children
            .get(&step)
            .copied()
            .ok_or_else(|| HirInvariantFailure::InvalidArenaCommit.into())
    }
}

#[cfg(test)]
#[path = "type_ref_lowering/tests.rs"]
mod tests;
