//! Direct attached-type projection into the final HIR source manifest.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::ast::module_path::ModulePathRoot;
use arcweft_lang_syntax::attachment::{AttachedTypeFamily, AttachedTypeRefNode};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::reference::{BorrowKind, RegionSyntax};
use arcweft_lang_syntax::types::{
    TypeRef, TypeRefAssociatedBindingPart, TypeRefComponentRole, TypeRefNodeStep, TypeRefRegionPart,
};

use super::{
    HirAssociatedTypeBindingSourcePart, HirSourceCommitInvariantError, HirSourceIndex,
    HirSourceQuery, HirSourceQueryError, HirSourceRequirement, HirSourceSite,
    HirTypeRegionSourcePart, HirTypeSourceRole, StagedHirSourceIndex, validate_component_source,
};
use crate::arena::ArenaSnapshot;
use crate::expr::{HirPoisonState, HirRecoveryIssue};
use crate::identity::{ItemId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId};
use crate::item::{HirItem, HirItemKind};
use crate::leaf::{
    HirName, HirPath, HirPathRoot, HirPathSegment, HirTypeRegion, HirTypeRegionIssue,
};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::type_ref::{HirGenericTypeIssue, HirType, HirTypeKind};

use super::expression_manifest::candidate_projection::candidate_type_expectations;

impl From<TypeRefAssociatedBindingPart> for HirAssociatedTypeBindingSourcePart {
    fn from(value: TypeRefAssociatedBindingPart) -> Self {
        match value {
            TypeRefAssociatedBindingPart::Whole => Self::Whole,
            TypeRefAssociatedBindingPart::Name => Self::Name,
            TypeRefAssociatedBindingPart::Equals => Self::Equals,
            TypeRefAssociatedBindingPart::Value => Self::Value,
        }
    }
}

impl From<TypeRefRegionPart> for HirTypeRegionSourcePart {
    fn from(value: TypeRefRegionPart) -> Self {
        match value {
            TypeRefRegionPart::Whole => Self::Whole,
            TypeRefRegionPart::NamedApostrophe => Self::NamedApostrophe,
            TypeRefRegionPart::NamedName => Self::NamedName,
            TypeRefRegionPart::ElisionInsertion => Self::ElisionInsertion,
        }
    }
}

impl From<TypeRefComponentRole> for HirTypeSourceRole {
    fn from(value: TypeRefComponentRole) -> Self {
        match value {
            TypeRefComponentRole::Whole => Self::Whole,
            TypeRefComponentRole::NeverMarker => Self::NeverMarker,
            TypeRefComponentRole::ConstInteger => Self::ConstInteger,
            TypeRefComponentRole::PathRoot => Self::PathRoot,
            TypeRefComponentRole::PathSegment { ordinal } => Self::PathSegment { ordinal },
            TypeRefComponentRole::TupleOpen => Self::TupleOpen,
            TypeRefComponentRole::TupleElement { ordinal } => Self::TupleElement { ordinal },
            TypeRefComponentRole::TupleSeparator { ordinal } => Self::TupleSeparator { ordinal },
            TypeRefComponentRole::TupleClose => Self::TupleClose,
            TypeRefComponentRole::FunctionOpen => Self::FunctionOpen,
            TypeRefComponentRole::FunctionParameter { ordinal } => {
                Self::FunctionParameter { ordinal }
            }
            TypeRefComponentRole::FunctionSeparator { ordinal } => {
                Self::FunctionSeparator { ordinal }
            }
            TypeRefComponentRole::FunctionClose => Self::FunctionClose,
            TypeRefComponentRole::FunctionArrow => Self::FunctionArrow,
            TypeRefComponentRole::FunctionReturn => Self::FunctionReturn,
            TypeRefComponentRole::FunctionEffectOpen => Self::FunctionEffectOpen,
            TypeRefComponentRole::FunctionEffect { ordinal } => Self::FunctionEffect { ordinal },
            TypeRefComponentRole::FunctionEffectClose => Self::FunctionEffectClose,
            TypeRefComponentRole::ChoiceAlternative { ordinal } => {
                Self::ChoiceAlternative { ordinal }
            }
            TypeRefComponentRole::ChoiceSeparator { ordinal } => Self::ChoiceSeparator { ordinal },
            TypeRefComponentRole::GenericBase => Self::GenericBase,
            TypeRefComponentRole::GenericOpen => Self::GenericOpen,
            TypeRefComponentRole::GenericArgument { ordinal } => Self::GenericArgument { ordinal },
            TypeRefComponentRole::GenericSeparator { ordinal } => {
                Self::GenericSeparator { ordinal }
            }
            TypeRefComponentRole::GenericClose => Self::GenericClose,
            TypeRefComponentRole::TraitBase => Self::TraitBase,
            TypeRefComponentRole::TraitOpen => Self::TraitOpen,
            TypeRefComponentRole::TraitArgument { ordinal } => Self::TraitArgument { ordinal },
            TypeRefComponentRole::TraitSeparator { ordinal } => Self::TraitSeparator { ordinal },
            TypeRefComponentRole::AssociatedBinding { ordinal, part } => Self::AssociatedBinding {
                ordinal,
                part: part.into(),
            },
            TypeRefComponentRole::TraitClose => Self::TraitClose,
            TypeRefComponentRole::ProjectionSubject => Self::ProjectionSubject,
            TypeRefComponentRole::ProjectionSeparator => Self::ProjectionSeparator,
            TypeRefComponentRole::ProjectionName => Self::ProjectionName,
            TypeRefComponentRole::ReferenceAmpersand => Self::ReferenceAmpersand,
            TypeRefComponentRole::Region(part) => Self::Region(part.into()),
            TypeRefComponentRole::ReferenceMutKeyword => Self::ReferenceMutKeyword,
            TypeRefComponentRole::ReferenceReferent => Self::ReferenceReferent,
            TypeRefComponentRole::SliceOpen => Self::SliceOpen,
            TypeRefComponentRole::SliceElement => Self::SliceElement,
            TypeRefComponentRole::SliceClose => Self::SliceClose,
            TypeRefComponentRole::Recovery => Self::Recovery,
        }
    }
}

impl StagedHirSourceIndex {
    /// Projects one final type owner's complete role manifest directly from
    /// the exact attached type grammar transaction.
    #[allow(
        clippy::result_large_err,
        reason = "type staging failures retain the complete typed owner, component, and syntax evidence"
    )]
    pub(crate) fn stage_attached_type(
        &mut self,
        parsed: &ParsedSource,
        owner: TypeId,
        attached: &AttachedTypeRefNode,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }

        let components = attached
            .components()
            .into_iter()
            .filter(|component| {
                final_type_component_for_family(attached.family(), component.role())
            })
            .collect::<Vec<_>>();
        let present = components
            .iter()
            .map(|component| HirTypeSourceRole::from(component.role()))
            .filter(|role| *role != HirTypeSourceRole::Whole)
            .collect::<BTreeSet<_>>();
        let requirements = type_requirements(attached.value());

        if let Some(role) = present
            .iter()
            .find(|role| !requirements.contains_key(role))
            .copied()
        {
            return self.reject(HirSourceCommitInvariantError::UndeclaredComponent {
                query: HirSourceQuery::Type { owner, role },
            });
        }
        if let Some(role) = requirements
            .iter()
            .find(|(role, requirement)| {
                **requirement == HirSourceRequirement::Required && !present.contains(role)
            })
            .map(|(role, _)| *role)
        {
            return self.reject(HirSourceCommitInvariantError::MissingRequiredComponent {
                query: HirSourceQuery::Type { owner, role },
            });
        }

        self.bind_syntax_owner(SyntheticOwner::Type(owner), attached.id())?;
        for (role, requirement) in requirements {
            self.require(&HirSourceQuery::Type { owner, role }, requirement)?;
        }
        for component in components {
            let role = HirTypeSourceRole::from(component.role());
            if role == HirTypeSourceRole::Whole {
                if let Err(error) =
                    validate_component_source(&self.source, component.source_span().source())
                {
                    return self.reject(error);
                }
                continue;
            }
            let site =
                match HirSourceSite::from_attached_span(parsed.document(), component.source_span())
                {
                    Ok(site) => site,
                    Err(error) => return self.reject(error.into()),
                };
            self.stage(&HirSourceQuery::Type { owner, role }, site)?;
        }
        Ok(())
    }
}

impl HirSourceIndex {
    /// Re-derives every source-backed type manifest from the exact accepted
    /// syntax snapshot and checks it against the final semantic arena.
    #[allow(
        clippy::too_many_lines,
        reason = "one projection validates source-backed and synthetic return types against the complete typed owner matrix"
    )]
    pub(crate) fn validates_attached_types(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        items: &ArenaSnapshot<HirItem, ItemId>,
        types: &ArenaSnapshot<HirType, TypeId>,
    ) -> bool {
        let Ok(item_entries) = items.try_iter_prepared(slots) else {
            return false;
        };
        let mut expected_synthetic_returns = BTreeMap::new();
        for (owner, item) in item_entries {
            let (return_type, role) = match item.kind() {
                HirItemKind::Predicate(predicate) => {
                    (predicate.return_type(), SyntheticRole::PredicateBoolReturn)
                }
                HirItemKind::Proof(proof) => (proof.return_type(), SyntheticRole::ProofUnitReturn),
                _ => continue,
            };
            let Ok(return_metadata) = slots.resolve_prepared(return_type) else {
                return false;
            };
            if !matches!(return_metadata.origin(), HirOrigin::Synthetic(_)) {
                continue;
            }
            let Ok(key) = SyntheticKey::try_new(SyntheticOwner::Item(owner), role, 0) else {
                return false;
            };
            if expected_synthetic_returns
                .insert(return_type, key)
                .is_some()
            {
                return false;
            }
        }
        let Ok(entries) = types.try_iter_prepared(slots) else {
            return false;
        };
        let entries = entries.collect::<Vec<_>>();
        let Some(retained_style_expressions) =
            super::item_projection::retained_style_expression_owners(items, slots)
        else {
            return false;
        };
        let Some(expected_candidate_types) =
            candidate_type_expectations(parsed, slots, &retained_style_expressions)
        else {
            return false;
        };
        if expected_candidate_types
            .keys()
            .any(|owner| expected_synthetic_returns.contains_key(owner))
        {
            return false;
        }
        if !expected_synthetic_returns
            .keys()
            .chain(expected_candidate_types.keys())
            .all(|expected| entries.iter().any(|(owner, _)| owner == expected))
        {
            return false;
        }
        let expected_key_only = entries
            .iter()
            .filter_map(|(_, payload)| match payload.kind() {
                HirTypeKind::Reference(reference) => match reference.region() {
                    Some(HirTypeRegion::Elided(region)) => Some(region.key()),
                    Some(HirTypeRegion::Named(_)) | None => None,
                },
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if expected_key_only != slots.key_only_synthetic_keys().collect::<BTreeSet<_>>() {
            return false;
        }
        entries.into_iter().all(|(owner, payload)| {
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            match metadata.origin() {
                HirOrigin::Source(source) => {
                    if expected_synthetic_returns.contains_key(&owner) {
                        return false;
                    }
                    let Ok(attached) = parsed.attached_type_ref(source.syntax()) else {
                        return false;
                    };
                    self.syntax_owners
                        .get(&SyntheticOwner::Type(owner))
                        .is_some_and(|syntax| *syntax == attached.id())
                        && metadata.source_site()
                            == &HirSourceSite::Span(attached.whole_source_span())
                        && type_payload_matches(payload, attached.value())
                        && match payload.kind() {
                            HirTypeKind::Reference(reference) => match reference.region() {
                                Some(HirTypeRegion::Elided(region)) => {
                                    slots.contains_key_only_synthetic(region.key())
                                }
                                Some(HirTypeRegion::Named(_)) | None => true,
                            },
                            _ => true,
                        }
                        && type_children_match(payload.kind(), &attached, slots)
                        && type_manifest_matches(self, parsed, owner, &attached)
                }
                HirOrigin::Synthetic(key) => {
                    if expected_synthetic_returns.get(&owner) == Some(key) {
                        return !source_index_has_type_owner(self, owner);
                    }
                    expected_candidate_types
                        .get(&owner)
                        .is_some_and(|expected| {
                            expected.key == *key
                                && metadata.source_site() == &expected.source_site
                                && type_payload_matches(payload, &expected.payload)
                                && candidate_type_state_matches(payload)
                                && candidate_type_children_match(payload.kind(), &expected.children)
                                && !source_index_has_type_owner(self, owner)
                        })
                }
            }
        })
    }
}

fn candidate_type_state_matches(payload: &HirType) -> bool {
    match payload.kind() {
        HirTypeKind::Recovery(recovery) => matches!(
            payload.state(),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidType(issue))
                if recovery.issue() == HirGenericTypeIssue::UnclassifiedSyntax
                    && *issue == HirGenericTypeIssue::UnclassifiedSyntax
        ),
        HirTypeKind::Reference(reference) if reference.region().is_none() => matches!(
            payload.state(),
            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidTypeRegion(
                HirTypeRegionIssue::InvalidNamedRegion
            ))
        ),
        _ => matches!(payload.state(), HirPoisonState::Clean),
    }
}

fn candidate_type_children_match(
    kind: &HirTypeKind,
    expected: &BTreeMap<TypeRefNodeStep, TypeId>,
) -> bool {
    let Some(actual) = type_child_ids(kind) else {
        return false;
    };
    actual.len() == expected.len()
        && actual
            .into_iter()
            .all(|(step, child)| expected.get(&step) == Some(&child))
}

fn source_index_has_type_owner(index: &HirSourceIndex, owner: TypeId) -> bool {
    let synthetic = SyntheticOwner::Type(owner);
    index.syntax_owners.contains_key(&synthetic)
        || index
            .requirements
            .keys()
            .any(|query| query.owner() == synthetic)
        || index
            .components
            .keys()
            .any(|query| query.owner() == synthetic)
}

fn type_manifest_matches(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: TypeId,
    attached: &AttachedTypeRefNode,
) -> bool {
    let expected_requirements = type_requirements(attached.value());
    let actual_requirements = index
        .requirements
        .iter()
        .filter_map(|(query, requirement)| match *query {
            HirSourceQuery::Type {
                owner: candidate,
                role,
            } if candidate == owner => Some((role, *requirement)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    if actual_requirements != expected_requirements {
        return false;
    }

    let mut expected_components = BTreeMap::new();
    for component in attached
        .components()
        .into_iter()
        .filter(|component| final_type_component_for_family(attached.family(), component.role()))
    {
        let role = HirTypeSourceRole::from(component.role());
        if role == HirTypeSourceRole::Whole {
            if validate_component_source(&index.source, component.source_span().source()).is_err() {
                return false;
            }
            continue;
        }
        let Ok(site) =
            HirSourceSite::from_attached_span(parsed.document(), component.source_span())
        else {
            return false;
        };
        if expected_components.insert(role, site).is_some() {
            return false;
        }
    }
    let actual_components = index
        .components
        .iter()
        .filter_map(|(query, site)| match *query {
            HirSourceQuery::Type {
                owner: candidate,
                role,
            } if candidate == owner => Some((role, site.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    actual_components == expected_components
}

impl HirTypeKind {
    /// Validates payload-family applicability and every type-role ordinal
    /// before the immutable source manifest is consulted.
    #[allow(
        clippy::too_many_lines,
        reason = "the closed twelve-family type-role matrix is clearer as one exhaustive match"
    )]
    pub(crate) fn validate_source_role(
        &self,
        owner: TypeId,
        role: HirTypeSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        if role == HirTypeSourceRole::Whole {
            return Ok(());
        }
        match self {
            Self::Never if role == HirTypeSourceRole::NeverMarker => Ok(()),
            Self::ConstInt(_) if role == HirTypeSourceRole::ConstInteger => Ok(()),
            Self::Path(path) => match role {
                HirTypeSourceRole::PathRoot => Ok(()),
                HirTypeSourceRole::PathSegment { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, path.segments().len())
                }
                _ => type_role_not_applicable(owner, role),
            },
            Self::Tuple(elements) => match role {
                HirTypeSourceRole::TupleOpen | HirTypeSourceRole::TupleClose => Ok(()),
                HirTypeSourceRole::TupleElement { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, elements.len())
                }
                HirTypeSourceRole::TupleSeparator { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, elements.len().saturating_sub(1))
                }
                _ => type_role_not_applicable(owner, role),
            },
            Self::Function(function) => match role {
                HirTypeSourceRole::FunctionOpen
                | HirTypeSourceRole::FunctionClose
                | HirTypeSourceRole::FunctionArrow
                | HirTypeSourceRole::FunctionReturn
                | HirTypeSourceRole::FunctionEffectOpen
                | HirTypeSourceRole::FunctionEffectClose => Ok(()),
                HirTypeSourceRole::FunctionParameter { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, function.parameters().len())
                }
                HirTypeSourceRole::FunctionSeparator { ordinal } => validate_type_ordinal(
                    owner,
                    role,
                    ordinal,
                    function.parameters().len().saturating_sub(1),
                ),
                HirTypeSourceRole::FunctionEffect { ordinal } => validate_type_ordinal(
                    owner,
                    role,
                    ordinal,
                    function
                        .effects()
                        .map_or(0, |effects| effects.effects().len()),
                ),
                _ => type_role_not_applicable(owner, role),
            },
            Self::Choice(alternatives) => match role {
                HirTypeSourceRole::ChoiceAlternative { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, alternatives.len())
                }
                HirTypeSourceRole::ChoiceSeparator { ordinal } => validate_type_ordinal(
                    owner,
                    role,
                    ordinal,
                    alternatives.len().saturating_sub(1),
                ),
                _ => type_role_not_applicable(owner, role),
            },
            Self::Generic(generic) => match role {
                HirTypeSourceRole::GenericBase
                | HirTypeSourceRole::GenericOpen
                | HirTypeSourceRole::GenericClose => Ok(()),
                HirTypeSourceRole::GenericArgument { ordinal }
                | HirTypeSourceRole::GenericSeparator { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, generic.arguments().len())
                }
                _ => type_role_not_applicable(owner, role),
            },
            Self::TraitBound(bound) => match role {
                HirTypeSourceRole::TraitBase
                | HirTypeSourceRole::TraitOpen
                | HirTypeSourceRole::TraitClose => Ok(()),
                HirTypeSourceRole::TraitArgument { ordinal } => {
                    validate_type_ordinal(owner, role, ordinal, bound.arguments().len())
                }
                HirTypeSourceRole::TraitSeparator { ordinal } => validate_type_ordinal(
                    owner,
                    role,
                    ordinal,
                    bound.arguments().len() + bound.associated().len(),
                ),
                HirTypeSourceRole::AssociatedBinding { ordinal, .. } => {
                    validate_type_ordinal(owner, role, ordinal, bound.associated().len())
                }
                _ => type_role_not_applicable(owner, role),
            },
            Self::Projection(_) => match role {
                HirTypeSourceRole::ProjectionSubject
                | HirTypeSourceRole::ProjectionSeparator
                | HirTypeSourceRole::ProjectionName => Ok(()),
                _ => type_role_not_applicable(owner, role),
            },
            Self::Reference(reference) => match role {
                HirTypeSourceRole::ReferenceAmpersand
                | HirTypeSourceRole::ReferenceMutKeyword
                | HirTypeSourceRole::ReferenceReferent => Ok(()),
                HirTypeSourceRole::Region(
                    HirTypeRegionSourcePart::Whole
                    | HirTypeRegionSourcePart::NamedApostrophe
                    | HirTypeRegionSourcePart::NamedName,
                ) if matches!(reference.region(), Some(HirTypeRegion::Named(_)) | None) => Ok(()),
                HirTypeSourceRole::Region(HirTypeRegionSourcePart::ElisionInsertion)
                    if matches!(reference.region(), Some(HirTypeRegion::Elided(_))) =>
                {
                    Ok(())
                }
                _ => type_role_not_applicable(owner, role),
            },
            Self::Slice(_) => match role {
                HirTypeSourceRole::SliceOpen
                | HirTypeSourceRole::SliceElement
                | HirTypeSourceRole::SliceClose => Ok(()),
                _ => type_role_not_applicable(owner, role),
            },
            Self::Recovery(_) if role == HirTypeSourceRole::Recovery => Ok(()),
            _ => type_role_not_applicable(owner, role),
        }
    }
}

fn validate_type_ordinal(
    owner: TypeId,
    role: HirTypeSourceRole,
    ordinal: u32,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    let length = type_ordinal(length);
    if ordinal < length {
        Ok(())
    } else {
        Err(HirSourceQueryError::TypeOrdinalOutOfBounds {
            owner,
            role,
            length,
        })
    }
}

fn type_role_not_applicable(
    owner: TypeId,
    role: HirTypeSourceRole,
) -> Result<(), HirSourceQueryError> {
    Err(HirSourceQueryError::TypeRoleNotApplicable { owner, role })
}

fn type_payload_matches(payload: &HirType, value: &TypeRef) -> bool {
    match (payload.kind(), value) {
        (HirTypeKind::ConstInt(actual), TypeRef::ConstInt(expected)) => actual == expected,
        (HirTypeKind::Path(actual), TypeRef::Path(expected)) => {
            hir_path_matches_type_path(actual, expected)
        }
        (HirTypeKind::Tuple(actual), TypeRef::Tuple(expected))
        | (HirTypeKind::Choice(actual), TypeRef::Choice(expected)) => {
            actual.len() == expected.len()
        }
        (
            HirTypeKind::Function(actual),
            TypeRef::Function {
                params, effects, ..
            },
        ) => {
            actual.parameters().len() == params.len()
                && match (actual.effects(), effects.as_ref()) {
                    (None, None) => true,
                    (Some(actual), Some(expected)) => {
                        actual.effects().len() == expected.effects().len()
                            && actual
                                .effects()
                                .iter()
                                .zip(expected.effects())
                                .all(|(actual, expected)| actual.as_str() == expected)
                    }
                    _ => false,
                }
        }
        (
            HirTypeKind::Generic(actual),
            TypeRef::Generic {
                base,
                args: expected,
            },
        ) => {
            hir_path_matches_type_path(actual.base(), base)
                && actual.arguments().len() == expected.len()
        }
        (HirTypeKind::TraitBound(actual), TypeRef::TraitBound(expected)) => {
            hir_path_matches_type_path(actual.base(), expected.path())
                && actual.arguments().len() == expected.args().len()
                && actual.associated().len() == expected.associated().len()
                && actual
                    .associated()
                    .iter()
                    .zip(expected.associated())
                    .all(|(actual, expected)| actual.name().as_str() == expected.name().as_str())
        }
        (HirTypeKind::Projection(actual), TypeRef::Projection { assoc, .. }) => {
            actual.associated().as_str() == assoc.as_str()
        }
        (HirTypeKind::Reference(actual), TypeRef::Reference(expected)) => {
            matches!(
                (actual.kind(), expected.kind()),
                (crate::expr::HirBorrowKind::Shared, BorrowKind::Shared)
                    | (crate::expr::HirBorrowKind::Mutable, BorrowKind::Mutable)
            ) && match (actual.region(), expected.region()) {
                (
                    Some(HirTypeRegion::Named(actual)),
                    RegionSyntax::Named { name: expected, .. },
                ) => actual.name().as_str() == expected.name(),
                (Some(HirTypeRegion::Elided(_)), RegionSyntax::Elided { .. }) => true,
                (None, RegionSyntax::Named { name: expected, .. }) => {
                    HirName::try_new(Box::<str>::from(expected.name())).is_err()
                        && matches!(
                            payload.state(),
                            HirPoisonState::Poisoned(HirRecoveryIssue::InvalidTypeRegion(
                                HirTypeRegionIssue::InvalidNamedRegion
                            ))
                        )
                }
                _ => false,
            }
        }
        (HirTypeKind::Never, TypeRef::Never)
        | (HirTypeKind::Slice(_), TypeRef::Slice(_))
        | (HirTypeKind::Recovery(_), TypeRef::Recovery(_)) => true,
        _ => false,
    }
}

fn type_children_match(
    kind: &HirTypeKind,
    attached: &AttachedTypeRefNode,
    slots: &SlotSnapshot,
) -> bool {
    let Some(expected) = type_child_ids(kind) else {
        return false;
    };
    let Ok(actual) = attached.children() else {
        return false;
    };
    expected.len() == actual.len()
        && expected.into_iter().all(|(step, child)| {
            let Some(attached_child) = actual.iter().find(|candidate| candidate.step() == step)
            else {
                return false;
            };
            slots
                .resolve_prepared(child)
                .is_ok_and(|metadata| match metadata.origin() {
                    HirOrigin::Source(source) => source.syntax() == attached_child.node().id(),
                    HirOrigin::Synthetic(_) => false,
                })
        })
}

fn type_child_ids(kind: &HirTypeKind) -> Option<Vec<(TypeRefNodeStep, TypeId)>> {
    let mut children = Vec::new();
    match kind {
        HirTypeKind::Tuple(items) => {
            push_indexed_type_children(&mut children, items, TypeRefNodeStep::TupleItem)?;
        }
        HirTypeKind::Function(function) => {
            push_indexed_type_children(
                &mut children,
                function.parameters(),
                TypeRefNodeStep::FunctionParameter,
            )?;
            children.push((TypeRefNodeStep::FunctionReturn, function.return_type()));
        }
        HirTypeKind::Choice(items) => {
            push_indexed_type_children(&mut children, items, TypeRefNodeStep::ChoiceAlternative)?;
        }
        HirTypeKind::Generic(generic) => {
            push_indexed_type_children(
                &mut children,
                generic.arguments(),
                TypeRefNodeStep::GenericArgument,
            )?;
        }
        HirTypeKind::TraitBound(bound) => {
            push_indexed_type_children(
                &mut children,
                bound.arguments(),
                TypeRefNodeStep::TraitArgument,
            )?;
            for (index, binding) in bound.associated().iter().enumerate() {
                children.push((
                    TypeRefNodeStep::AssociatedBinding(u16::try_from(index).ok()?),
                    binding.value(),
                ));
            }
        }
        HirTypeKind::Projection(projection) => {
            children.push((TypeRefNodeStep::ProjectionSubject, projection.subject()));
        }
        HirTypeKind::Reference(reference) => {
            children.push((TypeRefNodeStep::ReferenceReferent, reference.referent()));
        }
        HirTypeKind::Slice(item) => children.push((TypeRefNodeStep::SliceItem, *item)),
        HirTypeKind::Never
        | HirTypeKind::ConstInt(_)
        | HirTypeKind::Path(_)
        | HirTypeKind::Recovery(_) => {}
    }
    Some(children)
}

fn push_indexed_type_children(
    output: &mut Vec<(TypeRefNodeStep, TypeId)>,
    values: &[TypeId],
    step: fn(u16) -> TypeRefNodeStep,
) -> Option<()> {
    for (index, value) in values.iter().copied().enumerate() {
        output.push((step(u16::try_from(index).ok()?), value));
    }
    Some(())
}

pub(super) fn hir_path_matches_type_path(
    actual: &HirPath,
    expected: &arcweft_lang_syntax::types::TypePath,
) -> bool {
    let root_matches = matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::ImplicitCrate, ModulePathRoot::ImplicitCrate)
            | (HirPathRoot::Crate, ModulePathRoot::Crate)
            | (HirPathRoot::SelfModule, ModulePathRoot::SelfModule)
    ) || matches!(
        (actual.root(), expected.root()),
        (HirPathRoot::Super { depth: actual }, ModulePathRoot::Super(expected)) if actual == expected
    );
    root_matches
        && actual.segments().len() == expected.segments().len()
        && actual.segments().iter().zip(expected.segments()).all(
            |(actual, expected)| match actual {
                HirPathSegment::Identifier(actual) => actual.as_str() == expected.as_str(),
                HirPathSegment::ProjectSymbol(actual) => actual.as_str() == expected.as_str(),
            },
        )
}

fn final_type_component_for_family(family: AttachedTypeFamily, role: TypeRefComponentRole) -> bool {
    !matches!(
        (family, role),
        (
            AttachedTypeFamily::Generic | AttachedTypeFamily::TraitBound,
            TypeRefComponentRole::PathRoot | TypeRefComponentRole::PathSegment { .. }
        )
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed twelve-family attached-type manifest is one exhaustive grammar matrix"
)]
fn type_requirements(value: &TypeRef) -> BTreeMap<HirTypeSourceRole, HirSourceRequirement> {
    use HirSourceRequirement::{Optional, Required};
    use HirTypeSourceRole as Role;

    let mut requirements = BTreeMap::new();

    match value {
        TypeRef::Never => add_requirement(&mut requirements, Role::NeverMarker, Required),
        TypeRef::ConstInt(_) => {
            add_requirement(&mut requirements, Role::ConstInteger, Required);
        }
        TypeRef::Path(path) => {
            add_requirement(&mut requirements, Role::PathRoot, Optional);
            add_indexed_requirements(&mut requirements, path.segments().len(), |ordinal| {
                Role::PathSegment { ordinal }
            });
        }
        TypeRef::Tuple(elements) => {
            add_requirement(&mut requirements, Role::TupleOpen, Required);
            add_indexed_requirements(&mut requirements, elements.len(), |ordinal| {
                Role::TupleElement { ordinal }
            });
            add_separator_requirements(&mut requirements, elements.len(), |ordinal| {
                Role::TupleSeparator { ordinal }
            });
            add_requirement(&mut requirements, Role::TupleClose, Required);
        }
        TypeRef::Function {
            params, effects, ..
        } => {
            add_requirement(&mut requirements, Role::FunctionOpen, Optional);
            add_indexed_requirements(&mut requirements, params.len(), |ordinal| {
                Role::FunctionParameter { ordinal }
            });
            add_separator_requirements(&mut requirements, params.len(), |ordinal| {
                Role::FunctionSeparator { ordinal }
            });
            add_requirement(&mut requirements, Role::FunctionClose, Optional);
            add_requirement(&mut requirements, Role::FunctionArrow, Required);
            add_requirement(&mut requirements, Role::FunctionReturn, Required);
            if let Some(effects) = effects {
                add_requirement(&mut requirements, Role::FunctionEffectOpen, Required);
                add_indexed_requirements(&mut requirements, effects.effects().len(), |ordinal| {
                    Role::FunctionEffect { ordinal }
                });
                add_requirement(&mut requirements, Role::FunctionEffectClose, Required);
            } else {
                add_requirement(&mut requirements, Role::FunctionEffectOpen, Optional);
                add_requirement(&mut requirements, Role::FunctionEffectClose, Optional);
            }
        }
        TypeRef::Choice(alternatives) => {
            add_indexed_requirements(&mut requirements, alternatives.len(), |ordinal| {
                Role::ChoiceAlternative { ordinal }
            });
            add_separator_requirements(&mut requirements, alternatives.len(), |ordinal| {
                Role::ChoiceSeparator { ordinal }
            });
        }
        TypeRef::Generic { args, .. } => {
            add_requirement(&mut requirements, Role::GenericBase, Required);
            add_requirement(&mut requirements, Role::GenericOpen, Required);
            add_indexed_requirements(&mut requirements, args.len(), |ordinal| {
                Role::GenericArgument { ordinal }
            });
            add_separator_requirements(&mut requirements, args.len(), |ordinal| {
                Role::GenericSeparator { ordinal }
            });
            if !args.is_empty() {
                add_requirement(
                    &mut requirements,
                    Role::GenericSeparator {
                        ordinal: type_ordinal(args.len() - 1),
                    },
                    Optional,
                );
            }
            add_requirement(&mut requirements, Role::GenericClose, Required);
        }
        TypeRef::TraitBound(bound) => {
            add_requirement(&mut requirements, Role::TraitBase, Required);
            add_requirement(&mut requirements, Role::TraitOpen, Required);
            add_indexed_requirements(&mut requirements, bound.args().len(), |ordinal| {
                Role::TraitArgument { ordinal }
            });
            for (index, _) in bound.associated().iter().enumerate() {
                let ordinal = type_ordinal(index);
                for part in [
                    HirAssociatedTypeBindingSourcePart::Whole,
                    HirAssociatedTypeBindingSourcePart::Name,
                    HirAssociatedTypeBindingSourcePart::Equals,
                    HirAssociatedTypeBindingSourcePart::Value,
                ] {
                    add_requirement(
                        &mut requirements,
                        Role::AssociatedBinding { ordinal, part },
                        Required,
                    );
                }
            }
            let entry_count = bound.args().len() + bound.associated().len();
            add_separator_requirements(&mut requirements, entry_count, |ordinal| {
                Role::TraitSeparator { ordinal }
            });
            if entry_count != 0 {
                add_requirement(
                    &mut requirements,
                    Role::TraitSeparator {
                        ordinal: type_ordinal(entry_count - 1),
                    },
                    Optional,
                );
            }
            add_requirement(&mut requirements, Role::TraitClose, Required);
        }
        TypeRef::Projection { .. } => {
            add_requirement(&mut requirements, Role::ProjectionSubject, Required);
            add_requirement(&mut requirements, Role::ProjectionSeparator, Required);
            add_requirement(&mut requirements, Role::ProjectionName, Required);
        }
        TypeRef::Reference(reference) => {
            add_requirement(&mut requirements, Role::ReferenceAmpersand, Required);
            add_requirement(&mut requirements, Role::ReferenceMutKeyword, Optional);
            add_requirement(&mut requirements, Role::ReferenceReferent, Required);
            match reference.region() {
                RegionSyntax::Named { .. } => {
                    add_requirement(
                        &mut requirements,
                        Role::Region(HirTypeRegionSourcePart::Whole),
                        Required,
                    );
                    add_requirement(
                        &mut requirements,
                        Role::Region(HirTypeRegionSourcePart::NamedApostrophe),
                        Required,
                    );
                    add_requirement(
                        &mut requirements,
                        Role::Region(HirTypeRegionSourcePart::NamedName),
                        Required,
                    );
                }
                RegionSyntax::Elided { .. } => add_requirement(
                    &mut requirements,
                    Role::Region(HirTypeRegionSourcePart::ElisionInsertion),
                    Required,
                ),
            }
        }
        TypeRef::Slice(_) => {
            add_requirement(&mut requirements, Role::SliceOpen, Required);
            add_requirement(&mut requirements, Role::SliceElement, Required);
            add_requirement(&mut requirements, Role::SliceClose, Required);
        }
        TypeRef::Recovery(_) => add_requirement(&mut requirements, Role::Recovery, Required),
    }
    requirements
}

fn add_requirement(
    requirements: &mut BTreeMap<HirTypeSourceRole, HirSourceRequirement>,
    role: HirTypeSourceRole,
    requirement: HirSourceRequirement,
) {
    let previous = requirements.insert(role, requirement);
    debug_assert!(previous.is_none() || previous == Some(requirement));
}

fn add_indexed_requirements(
    requirements: &mut BTreeMap<HirTypeSourceRole, HirSourceRequirement>,
    len: usize,
    role: impl Fn(u32) -> HirTypeSourceRole,
) {
    for index in 0..len {
        add_requirement(
            requirements,
            role(type_ordinal(index)),
            HirSourceRequirement::Required,
        );
    }
}

fn add_separator_requirements(
    requirements: &mut BTreeMap<HirTypeSourceRole, HirSourceRequirement>,
    len: usize,
    role: impl Fn(u32) -> HirTypeSourceRole,
) {
    add_indexed_requirements(requirements, len.saturating_sub(1), role);
}

fn type_ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("validated attached type limits fit HIR source ordinals")
}
