//! Direct attached-pattern projection into the final HIR source manifest.

mod payload_validation;

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{AttachedPatternChild, AttachedPatternNode};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::patterns::{
    PatternBindingSiteKind, PatternComponentRole, PatternFieldPart, PatternLiteralPart,
    PatternNodeStep, PatternRecordFieldSyntax, PatternRestPart, PatternSyntaxFamily,
    PatternSyntaxKind, PatternSyntaxNode, PatternTypeChildRelation, VariantPatternHeadPart,
    VariantPatternPayloadPart,
};

use super::{
    HirIdRefSourcePart, HirLiteralSourcePart, HirPatternFieldSourcePart, HirPatternRestSourcePart,
    HirPatternSourceRole, HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery,
    HirSourceQueryError, HirSourceRequirement, HirSourceSite, HirVariantPatternHeadSourcePart,
    HirVariantPatternPayloadSourcePart, StagedHirSourceIndex, validate_component_source,
};
use crate::arena::ArenaSnapshot;
use crate::identity::{LocalGeneration, LocalId, PatternId, ScopeId, SyntheticOwner, TypeId};
use crate::leaf::{HirIdRef, HirIdRefShape, HirIdRefValue, HirName};
use crate::pattern::{
    HirPatternField, HirPatternKind, HirUnqualifiedVariantForm, HirVariantPatternHead,
    HirVariantPatternHeadIssue, HirVariantPatternHeadValue, HirVariantPatternPayload,
};
use crate::scope::{HirLocal, HirPatternBindingPolicy};
use crate::slot::{HirOrigin, SlotSnapshot};

use self::payload_validation::PatternPayloadValidation;
use super::control_projection::ExpectedLocal;

impl From<PatternLiteralPart> for HirLiteralSourcePart {
    fn from(value: PatternLiteralPart) -> Self {
        match value {
            PatternLiteralPart::Body => Self::Body,
            PatternLiteralPart::Prefix => Self::Prefix,
            PatternLiteralPart::Suffix => Self::Suffix,
            PatternLiteralPart::Unit => Self::Unit,
        }
    }
}

impl From<VariantPatternHeadPart> for HirVariantPatternHeadSourcePart {
    fn from(value: VariantPatternHeadPart) -> Self {
        match value {
            VariantPatternHeadPart::QualifiedRoot => Self::QualifiedRoot,
            VariantPatternHeadPart::QualifiedSegment { ordinal } => {
                Self::QualifiedSegment { ordinal }
            }
            VariantPatternHeadPart::DotShorthandMarker => Self::DotShorthandMarker,
        }
    }
}

impl From<VariantPatternPayloadPart> for HirVariantPatternPayloadSourcePart {
    fn from(value: VariantPatternPayloadPart) -> Self {
        match value {
            VariantPatternPayloadPart::Whole => Self::Whole,
            VariantPatternPayloadPart::OpenDelimiter => Self::OpenDelimiter,
            VariantPatternPayloadPart::CloseDelimiter => Self::CloseDelimiter,
        }
    }
}

impl From<PatternFieldPart> for HirPatternFieldSourcePart {
    fn from(value: PatternFieldPart) -> Self {
        match value {
            PatternFieldPart::Whole => Self::Whole,
            PatternFieldPart::Name => Self::Name,
            PatternFieldPart::Colon => Self::Colon,
            PatternFieldPart::Pattern => Self::Pattern,
            PatternFieldPart::RestMarker => Self::RestMarker,
            PatternFieldPart::RestBinding => Self::RestBinding,
        }
    }
}

impl From<PatternRestPart> for HirPatternRestSourcePart {
    fn from(value: PatternRestPart) -> Self {
        match value {
            PatternRestPart::Whole => Self::Whole,
            PatternRestPart::Marker => Self::Marker,
            PatternRestPart::Binding => Self::Binding,
        }
    }
}

impl From<PatternComponentRole> for HirPatternSourceRole {
    fn from(value: PatternComponentRole) -> Self {
        match value {
            PatternComponentRole::Whole => Self::Whole,
            PatternComponentRole::Name => Self::Name,
            PatternComponentRole::MutKeyword => Self::MutKeyword,
            PatternComponentRole::Literal(part) => Self::Literal(part.into()),
            PatternComponentRole::EntityReference(part) => Self::EntityReference(part.into()),
            PatternComponentRole::VariantHead(part) => Self::VariantHead(part.into()),
            PatternComponentRole::VariantName => Self::VariantName,
            PatternComponentRole::VariantPayload(part) => Self::VariantPayload(part.into()),
            PatternComponentRole::Element { ordinal } => Self::Element { ordinal },
            PatternComponentRole::RecordPathRoot => Self::RecordPathRoot,
            PatternComponentRole::RecordPathSegment { ordinal } => {
                Self::RecordPathSegment { ordinal }
            }
            PatternComponentRole::PatternField { field, part } => Self::PatternField {
                field,
                part: part.into(),
            },
            PatternComponentRole::SequenceRest(part) => Self::SequenceRest(part.into()),
            PatternComponentRole::WholeBindingName => Self::WholeBindingName,
            PatternComponentRole::NestedPattern => Self::NestedPattern,
            PatternComponentRole::TypedBindingColon => Self::TypedBindingColon,
            PatternComponentRole::TypedBindingType => Self::TypedBindingType,
            PatternComponentRole::Recovery => Self::Recovery,
        }
    }
}

impl StagedHirSourceIndex {
    /// Projects one final pattern owner's complete role manifest directly from
    /// the exact attached Pattern grammar transaction.
    pub(crate) fn stage_attached_pattern(
        &mut self,
        parsed: &ParsedSource,
        owner: PatternId,
        attached: &AttachedPatternNode,
        payload: &HirPatternKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }
        if !pattern_family_matches(payload, attached.family()) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Pattern(owner),
                },
            );
        }

        let components = attached.components();
        let present = components
            .iter()
            .map(|component| HirPatternSourceRole::from(component.role()))
            .filter(|role| *role != HirPatternSourceRole::Whole)
            .collect::<BTreeSet<_>>();
        let requirements = pattern_requirements(payload);

        if let Some(role) = present
            .iter()
            .find(|role| !requirements.contains_key(role))
            .copied()
        {
            return self.reject(HirSourceCommitInvariantError::UndeclaredComponent {
                query: HirSourceQuery::Pattern { owner, role },
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
                query: HirSourceQuery::Pattern { owner, role },
            });
        }

        self.bind_syntax_owner(SyntheticOwner::Pattern(owner), attached.id())?;
        for (role, requirement) in requirements {
            self.require(&HirSourceQuery::Pattern { owner, role }, requirement)?;
        }
        for component in components {
            let role = HirPatternSourceRole::from(component.role());
            if role == HirPatternSourceRole::Whole {
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
            self.stage(&HirSourceQuery::Pattern { owner, role }, site)?;
        }
        Ok(())
    }
}

impl HirSourceIndex {
    /// Re-derives every source-backed pattern manifest from the exact accepted
    /// syntax snapshot and checks it against the final semantic arena.
    pub(crate) fn validates_attached_patterns(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        patterns: &ArenaSnapshot<crate::pattern::HirPattern, PatternId>,
    ) -> bool {
        let Ok(entries) = patterns.try_iter_prepared(slots) else {
            return false;
        };
        let entries = entries.collect::<Vec<_>>();
        let Some(payload_validation) = PatternPayloadValidation::new(parsed, slots, patterns)
        else {
            return false;
        };
        entries.into_iter().all(|(owner, payload)| {
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            match metadata.origin() {
                HirOrigin::Source(source) => {
                    let Ok(attached) = parsed.attached_pattern(source.syntax()) else {
                        return false;
                    };
                    self.syntax_owners
                        .get(&SyntheticOwner::Pattern(owner))
                        .is_some_and(|syntax| *syntax == attached.id())
                        && metadata.source_site()
                            == &HirSourceSite::Span(attached.whole_source_span())
                        && payload_validation.matches(owner, payload, &attached)
                        && pattern_children_match(payload.kind(), &attached, slots)
                        && pattern_manifest_matches(self, parsed, owner, payload.kind(), &attached)
                }
                HirOrigin::Synthetic(_) => !source_index_has_pattern_owner(self, owner),
            }
        })
    }
}

fn source_index_has_pattern_owner(index: &HirSourceIndex, owner: PatternId) -> bool {
    let owner = SyntheticOwner::Pattern(owner);
    index.syntax_owners.contains_key(&owner)
        || index
            .requirements
            .keys()
            .any(|query| query.owner() == owner)
        || index.components.keys().any(|query| query.owner() == owner)
}

/// Validates the complete Local payload selected by one parser-owned binding
/// inventory. The canonical ID/pattern pairing remains owned by the pattern
/// graph; this adds name, timeline, mutability, annotation, and poison checks
/// without inventing another binding traversal.
pub(super) struct BindingLocalValidation<'state, 'arena> {
    scope: ScopeId,
    policy: HirPatternBindingPolicy,
    poisoned: bool,
    generations: &'state mut BTreeMap<HirName, LocalGeneration>,
    slots: &'arena SlotSnapshot,
    patterns: &'arena ArenaSnapshot<crate::pattern::HirPattern, PatternId>,
    locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
}

impl<'state, 'arena> BindingLocalValidation<'state, 'arena> {
    pub(super) fn new(
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
        generations: &'state mut BTreeMap<HirName, LocalGeneration>,
        slots: &'arena SlotSnapshot,
        patterns: &'arena ArenaSnapshot<crate::pattern::HirPattern, PatternId>,
        locals: &'arena ArenaSnapshot<HirLocal, LocalId>,
    ) -> Self {
        Self {
            scope,
            policy,
            poisoned: false,
            generations,
            slots,
            patterns,
            locals,
        }
    }

    pub(super) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

pub(super) fn binding_locals_match(
    attached: &AttachedPatternNode,
    expected: &[ExpectedLocal],
    validation: &mut BindingLocalValidation<'_, '_>,
) -> bool {
    let context_poisoned = validation.policy.requires_irrefutable()
        && !source_pattern_is_irrefutable(attached.value());
    validation.poisoned |= context_poisoned;
    let mut ordinals = BTreeSet::new();
    let sites = attached
        .binding_sites()
        .iter()
        .filter(|site| ordinals.insert(site.ordinal()))
        .filter(|site| site.binding().name().is_some())
        .collect::<Vec<_>>();
    if sites.len() != expected.len() {
        return false;
    }

    let mut names_in_pattern = BTreeSet::new();
    for (site, expected) in sites.into_iter().zip(expected) {
        let Some(source_name) = site.binding().name() else {
            return false;
        };
        let Ok(local) = validation
            .locals
            .resolve_prepared(validation.slots, expected.local)
        else {
            return false;
        };
        if local.name().as_str() != source_name.as_str() {
            return false;
        }
        let first_in_pattern = names_in_pattern.insert(local.name().clone());
        let expected_generation = if first_in_pattern {
            let next = match validation.generations.get(local.name()).copied() {
                Some(generation) => match generation.checked_next() {
                    Some(next) => next,
                    None => return false,
                },
                None => LocalGeneration::FIRST,
            };
            validation.generations.insert(local.name().clone(), next);
            next
        } else {
            let Some(generation) = validation.generations.get(local.name()).copied() else {
                return false;
            };
            generation
        };
        let expected_annotation = validation
            .patterns
            .resolve_prepared(validation.slots, expected.pattern)
            .ok()
            .and_then(|pattern| match pattern.kind() {
                HirPatternKind::TypedBinding { ty, .. } => Some(*ty),
                _ => None,
            });
        let mutable = matches!(site.kind(), PatternBindingSiteKind::MutableBinding);
        let poisoned = !first_in_pattern
            || context_poisoned
            || validation.policy.forbids_mutable() && mutable
            || validation.policy.reserves_result() && local.name().as_str() == "result";
        validation.poisoned |= poisoned;
        if local.scope() != validation.scope
            || local.kind() != validation.policy.local_kind()
            || local.pattern() != Some(expected.pattern)
            || local.annotation() != expected_annotation
            || local.generation() != expected_generation
            || local.is_mutable_binding() != mutable
            || local.is_poisoned() != poisoned
        {
            return false;
        }
    }
    true
}

fn source_pattern_is_irrefutable(pattern: &PatternSyntaxNode) -> bool {
    match pattern.kind() {
        PatternSyntaxKind::Binding(_)
        | PatternSyntaxKind::MutableBinding(_)
        | PatternSyntaxKind::Discard
        | PatternSyntaxKind::TypedBinding(_) => true,
        PatternSyntaxKind::Tuple(elements) => elements.iter().all(source_pattern_is_irrefutable),
        PatternSyntaxKind::Record(record) => record.fields().iter().all(|field| match field {
            PatternRecordFieldSyntax::Explicit { pattern, .. } => {
                source_pattern_is_irrefutable(pattern)
            }
            PatternRecordFieldSyntax::Shorthand(_) | PatternRecordFieldSyntax::Rest(_) => true,
            PatternRecordFieldSyntax::Invalid(_) => false,
        }),
        PatternSyntaxKind::WholeBinding { pattern, .. } => source_pattern_is_irrefutable(pattern),
        PatternSyntaxKind::Literal(_)
        | PatternSyntaxKind::EntityReference(_)
        | PatternSyntaxKind::Variant(_)
        | PatternSyntaxKind::BracketSequence(_)
        | PatternSyntaxKind::Or(_)
        | PatternSyntaxKind::Error => false,
    }
}

fn pattern_manifest_matches(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: PatternId,
    payload: &HirPatternKind,
    attached: &AttachedPatternNode,
) -> bool {
    let expected_requirements = pattern_requirements(payload);
    let actual_requirements = index
        .requirements
        .iter()
        .filter_map(|(query, requirement)| match *query {
            HirSourceQuery::Pattern {
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
    for component in attached.components() {
        let role = HirPatternSourceRole::from(component.role());
        if role == HirPatternSourceRole::Whole {
            if validate_component_source(&index.source, component.source_span().source()).is_err() {
                return false;
            }
            continue;
        }
        if !expected_requirements.contains_key(&role) {
            return false;
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
            HirSourceQuery::Pattern {
                owner: candidate,
                role,
            } if candidate == owner => Some((role, site.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    actual_components == expected_components
        && expected_requirements.iter().all(|(role, requirement)| {
            *requirement != HirSourceRequirement::Required || expected_components.contains_key(role)
        })
}

fn pattern_children_match(
    payload: &HirPatternKind,
    attached: &AttachedPatternNode,
    slots: &SlotSnapshot,
) -> bool {
    let Ok(mut attached_children) = attached.children() else {
        return false;
    };
    let semantic_children = pattern_children(payload);
    for child in &semantic_children {
        let Some(index) = attached_children
            .iter()
            .position(|attached| match (child, attached) {
                (
                    SemanticPatternChild::Pattern { step, pattern },
                    AttachedPatternChild::Pattern {
                        step: attached_step,
                        node,
                    },
                ) if *step == *attached_step => prepared_source_syntax(slots, *pattern)
                    .is_some_and(|syntax| syntax == node.id()),
                (
                    SemanticPatternChild::Type { relation, ty },
                    AttachedPatternChild::Type {
                        relation: attached_relation,
                        node,
                    },
                ) if *relation == *attached_relation => {
                    prepared_source_syntax(slots, *ty).is_some_and(|syntax| syntax == node.id())
                }
                _ => false,
            })
        else {
            return false;
        };
        attached_children.remove(index);
    }

    // HIR cross-field validation can invalidate an explicit record field
    // (duplicate name or other sibling-dependent conflict) before lowering
    // its attached Pattern child. Such a child is intentionally absent from
    // the semantic arena; every other unmatched attached child is invalid.
    attached_children
        .into_iter()
        .all(|attached| match attached {
            AttachedPatternChild::Pattern {
                step: PatternNodeStep::RecordField(field),
                ..
            } => matches!(
                payload,
                HirPatternKind::Record { fields, .. }
                    if fields
                        .get(usize::try_from(field).expect("u32 field ordinal fits usize"))
                        .is_some_and(|field| matches!(field, HirPatternField::Invalid { .. }))
            ),
            AttachedPatternChild::Pattern { .. } | AttachedPatternChild::Type { .. } => false,
        })
}

#[derive(Clone, Copy)]
enum SemanticPatternChild {
    Pattern {
        step: PatternNodeStep,
        pattern: PatternId,
    },
    Type {
        relation: PatternTypeChildRelation,
        ty: TypeId,
    },
}

fn pattern_children(payload: &HirPatternKind) -> Vec<SemanticPatternChild> {
    match payload {
        HirPatternKind::Variant(pattern) => match pattern.payload() {
            HirVariantPatternPayload::Pattern(pattern)
            | HirVariantPatternPayload::Recovered {
                pattern: Some(pattern),
                ..
            } => vec![SemanticPatternChild::Pattern {
                step: PatternNodeStep::VariantPayload,
                pattern: *pattern,
            }],
            HirVariantPatternPayload::Absent
            | HirVariantPatternPayload::Recovered { pattern: None, .. } => Vec::new(),
        },
        HirPatternKind::Tuple { elements }
        | HirPatternKind::BracketSequence { elements, .. }
        | HirPatternKind::Or {
            alternatives: elements,
        } => elements
            .iter()
            .enumerate()
            .map(|(ordinal, pattern)| SemanticPatternChild::Pattern {
                step: PatternNodeStep::Element(pattern_ordinal(ordinal)),
                pattern: *pattern,
            })
            .collect(),
        HirPatternKind::Record { fields, .. } => fields
            .iter()
            .enumerate()
            .filter_map(|(field, payload)| match payload {
                HirPatternField::Explicit { pattern, .. } => Some(SemanticPatternChild::Pattern {
                    step: PatternNodeStep::RecordField(pattern_ordinal(field)),
                    pattern: *pattern,
                }),
                HirPatternField::Shorthand { .. }
                | HirPatternField::Rest { .. }
                | HirPatternField::Invalid { .. } => None,
            })
            .collect(),
        HirPatternKind::WholeBinding { pattern, .. } => vec![SemanticPatternChild::Pattern {
            step: PatternNodeStep::NestedPattern,
            pattern: *pattern,
        }],
        HirPatternKind::TypedBinding { ty, .. } => vec![SemanticPatternChild::Type {
            relation: PatternTypeChildRelation::TypedBinding,
            ty: *ty,
        }],
        HirPatternKind::Binding(_)
        | HirPatternKind::MutableBinding(_)
        | HirPatternKind::Literal(_)
        | HirPatternKind::EntityReference(_)
        | HirPatternKind::Discard
        | HirPatternKind::Error(_) => Vec::new(),
    }
}

pub(super) fn pattern_child_ids(payload: &HirPatternKind) -> Vec<PatternId> {
    pattern_children(payload)
        .into_iter()
        .filter_map(|child| match child {
            SemanticPatternChild::Pattern { pattern, .. } => Some(pattern),
            SemanticPatternChild::Type { .. } => None,
        })
        .collect()
}

fn prepared_source_syntax<I: crate::identity::HirTypedId>(
    slots: &SlotSnapshot,
    owner: I,
) -> Option<arcweft_lang_syntax::attachment::SyntaxNodeId> {
    slots
        .resolve_prepared(owner)
        .ok()
        .and_then(|metadata| match metadata.origin() {
            HirOrigin::Source(source) => Some(source.syntax()),
            HirOrigin::Synthetic(_) => None,
        })
}

fn pattern_family_matches(payload: &HirPatternKind, family: PatternSyntaxFamily) -> bool {
    matches!(
        (payload, family),
        (HirPatternKind::Binding(_), PatternSyntaxFamily::Binding)
            | (
                HirPatternKind::MutableBinding(_),
                PatternSyntaxFamily::MutableBinding
            )
            | (HirPatternKind::Literal(_), PatternSyntaxFamily::Literal)
            | (
                HirPatternKind::EntityReference(_),
                PatternSyntaxFamily::EntityReference
            )
            | (HirPatternKind::Variant(_), PatternSyntaxFamily::Variant)
            | (HirPatternKind::Discard, PatternSyntaxFamily::Discard)
            | (HirPatternKind::Tuple { .. }, PatternSyntaxFamily::Tuple)
            | (HirPatternKind::Record { .. }, PatternSyntaxFamily::Record)
            | (
                HirPatternKind::BracketSequence { .. },
                PatternSyntaxFamily::BracketSequence
            )
            | (
                HirPatternKind::WholeBinding { .. },
                PatternSyntaxFamily::WholeBinding
            )
            | (HirPatternKind::Or { .. }, PatternSyntaxFamily::Or)
            | (
                HirPatternKind::TypedBinding { .. },
                PatternSyntaxFamily::TypedBinding
            )
            | (HirPatternKind::Error(_), PatternSyntaxFamily::Error)
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed thirteen-family attached-pattern manifest is one exhaustive grammar matrix"
)]
fn pattern_requirements(
    payload: &HirPatternKind,
) -> BTreeMap<HirPatternSourceRole, HirSourceRequirement> {
    use HirPatternSourceRole as Role;
    use HirSourceRequirement::{Optional, Required};

    let mut requirements = BTreeMap::new();
    match payload {
        HirPatternKind::Binding(_) => {
            add_pattern_requirement(&mut requirements, Role::Name, Required);
        }
        HirPatternKind::MutableBinding(_) => {
            add_pattern_requirement(&mut requirements, Role::MutKeyword, Required);
            add_pattern_requirement(&mut requirements, Role::Name, Required);
        }
        HirPatternKind::Literal(_) => {
            add_pattern_requirement(
                &mut requirements,
                Role::Literal(HirLiteralSourcePart::Body),
                Required,
            );
            for part in [
                HirLiteralSourcePart::Prefix,
                HirLiteralSourcePart::Suffix,
                HirLiteralSourcePart::Unit,
            ] {
                add_pattern_requirement(&mut requirements, Role::Literal(part), Optional);
            }
        }
        HirPatternKind::EntityReference(reference) => {
            add_pattern_requirement(
                &mut requirements,
                Role::EntityReference(HirIdRefSourcePart::Whole),
                Required,
            );
            match reference {
                HirIdRefValue::Resolved(HirIdRef::Absolute(reference)) => {
                    add_pattern_requirement(
                        &mut requirements,
                        Role::EntityReference(HirIdRefSourcePart::AbsoluteMarker),
                        Required,
                    );
                    add_pattern_id_segments(&mut requirements, reference.segment_count(), Required);
                }
                HirIdRefValue::Resolved(HirIdRef::Relative(reference)) => {
                    add_pattern_parent_markers(
                        &mut requirements,
                        reference.parent_depth(),
                        Required,
                    );
                    add_pattern_id_segments(
                        &mut requirements,
                        reference.suffix().segment_count(),
                        Required,
                    );
                }
                HirIdRefValue::Resolved(HirIdRef::FamilyRelative(reference)) => {
                    add_pattern_requirement(
                        &mut requirements,
                        Role::EntityReference(HirIdRefSourcePart::Family),
                        Required,
                    );
                    add_pattern_requirement(
                        &mut requirements,
                        Role::EntityReference(HirIdRefSourcePart::FamilySeparator),
                        Required,
                    );
                    add_pattern_parent_markers(
                        &mut requirements,
                        reference.relative().parent_depth(),
                        Required,
                    );
                    add_pattern_id_segments(
                        &mut requirements,
                        reference.relative().suffix().segment_count(),
                        Required,
                    );
                }
                HirIdRefValue::Recovered(recovery) => match recovery.shape() {
                    HirIdRefShape::Missing => {}
                    HirIdRefShape::Absolute { segment_count } => {
                        add_pattern_requirement(
                            &mut requirements,
                            Role::EntityReference(HirIdRefSourcePart::AbsoluteMarker),
                            Required,
                        );
                        add_pattern_id_segments(
                            &mut requirements,
                            usize::try_from(segment_count)
                                .expect("u32 ID segment count fits usize"),
                            Required,
                        );
                    }
                    HirIdRefShape::Relative {
                        parent_depth,
                        suffix_segment_count,
                    } => {
                        add_pattern_parent_markers(&mut requirements, parent_depth, Required);
                        add_pattern_id_segments(
                            &mut requirements,
                            usize::try_from(suffix_segment_count)
                                .expect("u32 ID segment count fits usize"),
                            Required,
                        );
                    }
                    HirIdRefShape::FamilyRelative {
                        parent_depth,
                        suffix_segment_count,
                    } => {
                        add_pattern_requirement(
                            &mut requirements,
                            Role::EntityReference(HirIdRefSourcePart::Family),
                            Required,
                        );
                        add_pattern_requirement(
                            &mut requirements,
                            Role::EntityReference(HirIdRefSourcePart::FamilySeparator),
                            Required,
                        );
                        add_pattern_parent_markers(&mut requirements, parent_depth, Required);
                        add_pattern_id_segments(
                            &mut requirements,
                            usize::try_from(suffix_segment_count)
                                .expect("u32 ID segment count fits usize"),
                            Required,
                        );
                    }
                },
            }
        }
        HirPatternKind::Variant(pattern) => {
            match pattern.head() {
                HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path)) => {
                    add_pattern_requirement(
                        &mut requirements,
                        Role::VariantHead(HirVariantPatternHeadSourcePart::QualifiedRoot),
                        Optional,
                    );
                    add_indexed_pattern_requirements(
                        &mut requirements,
                        path.segments().len(),
                        |ordinal| {
                            Role::VariantHead(HirVariantPatternHeadSourcePart::QualifiedSegment {
                                ordinal,
                            })
                        },
                        Required,
                    );
                }
                HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
                    HirUnqualifiedVariantForm::DotShorthand,
                )) => {
                    add_pattern_requirement(
                        &mut requirements,
                        Role::VariantHead(HirVariantPatternHeadSourcePart::DotShorthandMarker),
                        Required,
                    );
                }
                HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
                    HirUnqualifiedVariantForm::BareExpectedType,
                ))
                | HirVariantPatternHeadValue::Recovered(HirVariantPatternHeadIssue::Missing) => {}
                HirVariantPatternHeadValue::Recovered(
                    HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count },
                ) => {
                    add_pattern_requirement(
                        &mut requirements,
                        Role::VariantHead(HirVariantPatternHeadSourcePart::QualifiedRoot),
                        Optional,
                    );
                    add_indexed_pattern_requirements(
                        &mut requirements,
                        usize::try_from(*segment_count)
                            .expect("u32 Pattern path segment count fits usize"),
                        |ordinal| {
                            Role::VariantHead(HirVariantPatternHeadSourcePart::QualifiedSegment {
                                ordinal,
                            })
                        },
                        Required,
                    );
                }
            }
            add_pattern_requirement(&mut requirements, Role::VariantName, Required);
            let payload_requirement = match pattern.payload() {
                HirVariantPatternPayload::Absent => Optional,
                HirVariantPatternPayload::Pattern(_)
                | HirVariantPatternPayload::Recovered { .. } => Required,
            };
            for part in [
                HirVariantPatternPayloadSourcePart::Whole,
                HirVariantPatternPayloadSourcePart::OpenDelimiter,
                HirVariantPatternPayloadSourcePart::CloseDelimiter,
            ] {
                add_pattern_requirement(
                    &mut requirements,
                    Role::VariantPayload(part),
                    payload_requirement,
                );
            }
        }
        HirPatternKind::Discard => {}
        HirPatternKind::Tuple { elements }
        | HirPatternKind::Or {
            alternatives: elements,
        } => {
            add_indexed_pattern_requirements(
                &mut requirements,
                elements.len(),
                |ordinal| Role::Element { ordinal },
                Required,
            );
        }
        HirPatternKind::Record { path, fields } => {
            add_pattern_requirement(&mut requirements, Role::RecordPathRoot, Optional);
            add_indexed_pattern_requirements(
                &mut requirements,
                path.segment_count(),
                |ordinal| Role::RecordPathSegment { ordinal },
                Required,
            );
            for (field, payload) in fields.iter().enumerate() {
                let field = pattern_ordinal(field);
                match payload {
                    HirPatternField::Explicit { .. } => {
                        for part in [
                            HirPatternFieldSourcePart::Whole,
                            HirPatternFieldSourcePart::Name,
                            HirPatternFieldSourcePart::Colon,
                            HirPatternFieldSourcePart::Pattern,
                        ] {
                            add_pattern_requirement(
                                &mut requirements,
                                Role::PatternField { field, part },
                                Required,
                            );
                        }
                    }
                    HirPatternField::Shorthand { .. } => {
                        for part in [
                            HirPatternFieldSourcePart::Whole,
                            HirPatternFieldSourcePart::Name,
                        ] {
                            add_pattern_requirement(
                                &mut requirements,
                                Role::PatternField { field, part },
                                Required,
                            );
                        }
                    }
                    HirPatternField::Rest { binding } => {
                        for part in [
                            HirPatternFieldSourcePart::Whole,
                            HirPatternFieldSourcePart::RestMarker,
                        ] {
                            add_pattern_requirement(
                                &mut requirements,
                                Role::PatternField { field, part },
                                Required,
                            );
                        }
                        add_pattern_requirement(
                            &mut requirements,
                            Role::PatternField {
                                field,
                                part: HirPatternFieldSourcePart::RestBinding,
                            },
                            if binding.is_some() {
                                Required
                            } else {
                                Optional
                            },
                        );
                    }
                    HirPatternField::Invalid { .. } => {
                        for part in [
                            HirPatternFieldSourcePart::Whole,
                            HirPatternFieldSourcePart::Name,
                            HirPatternFieldSourcePart::Colon,
                            HirPatternFieldSourcePart::Pattern,
                            HirPatternFieldSourcePart::RestMarker,
                            HirPatternFieldSourcePart::RestBinding,
                        ] {
                            add_pattern_requirement(
                                &mut requirements,
                                Role::PatternField { field, part },
                                Optional,
                            );
                        }
                    }
                }
            }
        }
        HirPatternKind::BracketSequence { elements, rest } => {
            add_indexed_pattern_requirements(
                &mut requirements,
                elements.len(),
                |ordinal| Role::Element { ordinal },
                Required,
            );
            let authored_rest = if rest.has_authored_rest() {
                Required
            } else {
                Optional
            };
            for part in [
                HirPatternRestSourcePart::Whole,
                HirPatternRestSourcePart::Marker,
            ] {
                add_pattern_requirement(&mut requirements, Role::SequenceRest(part), authored_rest);
            }
            add_pattern_requirement(
                &mut requirements,
                Role::SequenceRest(HirPatternRestSourcePart::Binding),
                if rest.has_authored_binding() {
                    Required
                } else {
                    Optional
                },
            );
        }
        HirPatternKind::WholeBinding { .. } => {
            add_pattern_requirement(&mut requirements, Role::WholeBindingName, Required);
            add_pattern_requirement(&mut requirements, Role::NestedPattern, Required);
        }
        HirPatternKind::TypedBinding { .. } => {
            add_pattern_requirement(&mut requirements, Role::Name, Required);
            add_pattern_requirement(&mut requirements, Role::TypedBindingColon, Required);
            add_pattern_requirement(&mut requirements, Role::TypedBindingType, Required);
        }
        HirPatternKind::Error(_) => {
            add_pattern_requirement(&mut requirements, Role::Recovery, Required);
        }
    }
    requirements
}

fn add_pattern_parent_markers(
    requirements: &mut BTreeMap<HirPatternSourceRole, HirSourceRequirement>,
    length: usize,
    requirement: HirSourceRequirement,
) {
    add_indexed_pattern_requirements(
        requirements,
        length,
        |ordinal| {
            HirPatternSourceRole::EntityReference(HirIdRefSourcePart::ParentMarker { ordinal })
        },
        requirement,
    );
}

fn add_pattern_id_segments(
    requirements: &mut BTreeMap<HirPatternSourceRole, HirSourceRequirement>,
    length: usize,
    requirement: HirSourceRequirement,
) {
    add_indexed_pattern_requirements(
        requirements,
        length,
        |ordinal| {
            HirPatternSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment { ordinal })
        },
        requirement,
    );
}

fn add_indexed_pattern_requirements(
    requirements: &mut BTreeMap<HirPatternSourceRole, HirSourceRequirement>,
    length: usize,
    role: impl Fn(u32) -> HirPatternSourceRole,
    requirement: HirSourceRequirement,
) {
    for index in 0..length {
        add_pattern_requirement(requirements, role(pattern_ordinal(index)), requirement);
    }
}

fn add_pattern_requirement(
    requirements: &mut BTreeMap<HirPatternSourceRole, HirSourceRequirement>,
    role: HirPatternSourceRole,
    requirement: HirSourceRequirement,
) {
    let previous = requirements.insert(role, requirement);
    debug_assert!(previous.is_none() || previous == Some(requirement));
}

fn pattern_ordinal(index: usize) -> u32 {
    u32::try_from(index).expect("validated attached pattern limits fit HIR source ordinals")
}

impl HirPatternKind {
    /// Rejects an inapplicable role or one-over ordinal from the resolved
    /// semantic family before the source manifest and source identity are read.
    pub(crate) fn validate_source_role(
        &self,
        owner: PatternId,
        role: HirPatternSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        if role == HirPatternSourceRole::Whole {
            return Ok(());
        }
        match self {
            Self::Binding(_) => admit(owner, role, role == HirPatternSourceRole::Name),
            Self::MutableBinding(_) => admit(
                owner,
                role,
                matches!(
                    role,
                    HirPatternSourceRole::Name | HirPatternSourceRole::MutKeyword
                ),
            ),
            Self::Literal(_) => admit(
                owner,
                role,
                matches!(role, HirPatternSourceRole::Literal(_)),
            ),
            Self::EntityReference(reference) => validate_id_ref_role(owner, role, reference),
            Self::Variant(pattern) => match role {
                HirPatternSourceRole::VariantName
                | HirPatternSourceRole::VariantPayload(
                    HirVariantPatternPayloadSourcePart::Whole
                    | HirVariantPatternPayloadSourcePart::OpenDelimiter
                    | HirVariantPatternPayloadSourcePart::CloseDelimiter,
                ) => Ok(()),
                HirPatternSourceRole::VariantHead(head_role) => {
                    validate_variant_head_role(owner, role, head_role, pattern.head())
                }
                _ => not_applicable(owner, role),
            },
            Self::Discard => not_applicable(owner, role),
            Self::Tuple { elements } => match role {
                HirPatternSourceRole::Element { ordinal } => {
                    validate_ordinal(owner, role, ordinal, elements.len())
                }
                _ => not_applicable(owner, role),
            },
            Self::Record { path, fields } => match role {
                // A path root is an optional component of the record family.
                // The attached manifest distinguishes an authored root from
                // the implicit/absent forms.
                HirPatternSourceRole::RecordPathRoot => Ok(()),
                HirPatternSourceRole::RecordPathSegment { ordinal } => {
                    validate_ordinal(owner, role, ordinal, path.segment_count())
                }
                HirPatternSourceRole::PatternField { field, part } => {
                    let Some(field_payload) = fields.get(field as usize) else {
                        return ordinal_out_of_bounds(owner, role, fields.len());
                    };
                    validate_field_part(owner, role, part, field_payload)
                }
                _ => not_applicable(owner, role),
            },
            Self::BracketSequence { elements, .. } => match role {
                HirPatternSourceRole::Element { ordinal } => {
                    validate_ordinal(owner, role, ordinal, elements.len())
                }
                // Family applicability is stable for all explicit rest states;
                // the source manifest distinguishes absent, unbound, bound,
                // and recovered authored components.
                HirPatternSourceRole::SequenceRest(
                    HirPatternRestSourcePart::Whole
                    | HirPatternRestSourcePart::Marker
                    | HirPatternRestSourcePart::Binding,
                ) => Ok(()),
                _ => not_applicable(owner, role),
            },
            Self::WholeBinding { .. } => admit(
                owner,
                role,
                matches!(
                    role,
                    HirPatternSourceRole::WholeBindingName | HirPatternSourceRole::NestedPattern
                ),
            ),
            Self::Or { alternatives } => match role {
                HirPatternSourceRole::Element { ordinal } => {
                    validate_ordinal(owner, role, ordinal, alternatives.len())
                }
                _ => not_applicable(owner, role),
            },
            Self::TypedBinding { .. } => admit(
                owner,
                role,
                matches!(
                    role,
                    HirPatternSourceRole::Name
                        | HirPatternSourceRole::TypedBindingColon
                        | HirPatternSourceRole::TypedBindingType
                ),
            ),
            Self::Error(_) => admit(owner, role, role == HirPatternSourceRole::Recovery),
        }
    }
}

fn validate_variant_head_role(
    owner: PatternId,
    role: HirPatternSourceRole,
    head_role: HirVariantPatternHeadSourcePart,
    head: &HirVariantPatternHeadValue,
) -> Result<(), HirSourceQueryError> {
    match (head, head_role) {
        (
            HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(_))
            | HirVariantPatternHeadValue::Recovered(
                HirVariantPatternHeadIssue::InvalidQualifiedPath { .. },
            ),
            HirVariantPatternHeadSourcePart::QualifiedRoot,
        )
        | (
            HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
                HirUnqualifiedVariantForm::DotShorthand,
            )),
            HirVariantPatternHeadSourcePart::DotShorthandMarker,
        ) => Ok(()),
        (
            HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path)),
            HirVariantPatternHeadSourcePart::QualifiedSegment { ordinal },
        ) => validate_ordinal(owner, role, ordinal, path.segments().len()),
        (
            HirVariantPatternHeadValue::Recovered(
                HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count },
            ),
            HirVariantPatternHeadSourcePart::QualifiedSegment { ordinal },
        ) => validate_ordinal(
            owner,
            role,
            ordinal,
            usize::try_from(*segment_count).expect("u32 Pattern path segment count fits usize"),
        ),
        _ => not_applicable(owner, role),
    }
}

fn validate_id_ref_role(
    owner: PatternId,
    role: HirPatternSourceRole,
    reference: &HirIdRefValue,
) -> Result<(), HirSourceQueryError> {
    let HirPatternSourceRole::EntityReference(part) = role else {
        return not_applicable(owner, role);
    };
    match (reference, part) {
        (_, HirIdRefSourcePart::Whole)
        | (HirIdRefValue::Resolved(HirIdRef::Absolute(_)), HirIdRefSourcePart::AbsoluteMarker)
        | (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(_)),
            HirIdRefSourcePart::Family | HirIdRefSourcePart::FamilySeparator,
        ) => Ok(()),
        (
            HirIdRefValue::Resolved(HirIdRef::Absolute(reference)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(owner, role, ordinal, reference.segment_count()),
        (
            HirIdRefValue::Resolved(HirIdRef::Relative(relative)),
            HirIdRefSourcePart::ParentMarker { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.parent_depth()),
        (
            HirIdRefValue::Resolved(HirIdRef::Relative(relative)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.suffix().segment_count()),
        (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative)),
            HirIdRefSourcePart::ParentMarker { ordinal },
        ) => validate_ordinal(owner, role, ordinal, relative.relative().parent_depth()),
        (
            HirIdRefValue::Resolved(HirIdRef::FamilyRelative(relative)),
            HirIdRefSourcePart::SuffixSegment { ordinal },
        ) => validate_ordinal(
            owner,
            role,
            ordinal,
            relative.relative().suffix().segment_count(),
        ),
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::AbsoluteMarker)
            if matches!(recovery.shape(), HirIdRefShape::Absolute { .. }) =>
        {
            Ok(())
        }
        (
            HirIdRefValue::Recovered(recovery),
            HirIdRefSourcePart::Family | HirIdRefSourcePart::FamilySeparator,
        ) if matches!(recovery.shape(), HirIdRefShape::FamilyRelative { .. }) => Ok(()),
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::ParentMarker { ordinal }) => {
            match recovery.shape() {
                HirIdRefShape::Relative { parent_depth, .. }
                | HirIdRefShape::FamilyRelative { parent_depth, .. } => {
                    validate_ordinal(owner, role, ordinal, parent_depth)
                }
                HirIdRefShape::Missing | HirIdRefShape::Absolute { .. } => {
                    not_applicable(owner, role)
                }
            }
        }
        (HirIdRefValue::Recovered(recovery), HirIdRefSourcePart::SuffixSegment { ordinal }) => {
            match recovery.shape() {
                HirIdRefShape::Absolute { segment_count } => validate_ordinal(
                    owner,
                    role,
                    ordinal,
                    usize::try_from(segment_count).expect("u32 ID segment count fits usize"),
                ),
                HirIdRefShape::Relative {
                    suffix_segment_count,
                    ..
                }
                | HirIdRefShape::FamilyRelative {
                    suffix_segment_count,
                    ..
                } => validate_ordinal(
                    owner,
                    role,
                    ordinal,
                    usize::try_from(suffix_segment_count).expect("u32 ID segment count fits usize"),
                ),
                HirIdRefShape::Missing => not_applicable(owner, role),
            }
        }
        _ => not_applicable(owner, role),
    }
}

const fn validate_field_part(
    owner: PatternId,
    role: HirPatternSourceRole,
    part: HirPatternFieldSourcePart,
    field: &HirPatternField,
) -> Result<(), HirSourceQueryError> {
    let applicable = match field {
        HirPatternField::Explicit { .. } => matches!(
            part,
            HirPatternFieldSourcePart::Whole
                | HirPatternFieldSourcePart::Name
                | HirPatternFieldSourcePart::Colon
                | HirPatternFieldSourcePart::Pattern
        ),
        HirPatternField::Shorthand { .. } => matches!(
            part,
            HirPatternFieldSourcePart::Whole | HirPatternFieldSourcePart::Name
        ),
        HirPatternField::Rest { .. } => matches!(
            part,
            HirPatternFieldSourcePart::Whole
                | HirPatternFieldSourcePart::RestMarker
                | HirPatternFieldSourcePart::RestBinding
        ),
        // Exact present components for a typed invalid field come from its
        // attached manifest; the semantic issue does not preserve spelling.
        HirPatternField::Invalid { .. } => true,
    };
    admit(owner, role, applicable)
}

fn validate_ordinal(
    owner: PatternId,
    role: HirPatternSourceRole,
    ordinal: u32,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    if ordinal as usize >= length {
        ordinal_out_of_bounds(owner, role, length)
    } else {
        Ok(())
    }
}

fn ordinal_out_of_bounds(
    owner: PatternId,
    role: HirPatternSourceRole,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    let length = u32::try_from(length)
        .expect("a failing u32 source ordinal proves the semantic length fits u32");
    Err(HirSourceQueryError::PatternOrdinalOutOfBounds {
        owner,
        role,
        length,
    })
}

const fn admit(
    owner: PatternId,
    role: HirPatternSourceRole,
    applicable: bool,
) -> Result<(), HirSourceQueryError> {
    if applicable {
        Ok(())
    } else {
        not_applicable(owner, role)
    }
}

const fn not_applicable(
    owner: PatternId,
    role: HirPatternSourceRole,
) -> Result<(), HirSourceQueryError> {
    Err(HirSourceQueryError::PatternRoleNotApplicable { owner, role })
}
