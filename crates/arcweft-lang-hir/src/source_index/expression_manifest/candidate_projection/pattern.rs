//! Exact candidate Pattern and Local graph validation.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedCandidatePatternChild, AttachedCandidatePatternProjection,
};
use arcweft_lang_syntax::patterns::{
    PatternBindingSite, PatternBindingSiteKind, PatternBindingSyntax, PatternComponentRole,
    PatternFieldPart, PatternNameSyntax, PatternNodeStep, PatternPathIssue,
    PatternRecordFieldSyntax, PatternRestPart, PatternSequenceRestIssue, PatternSequenceRestSyntax,
    PatternSyntaxKind, PatternTypeChildRelation, PatternUnqualifiedVariantForm, PatternVariantHead,
    PatternVariantHeadSyntax, PatternVariantPayloadIssue, PatternVariantPayloadSyntax,
};

use super::CandidateValidationCursor;
use crate::expr::HirPoisonState;
use crate::final_lowering::name_projection::{name, name_issue};
use crate::final_lowering::pattern_lowering::binding_plan::{
    RecordFieldDisposition, binding_issue, classify_record_fields,
};
use crate::final_lowering::pattern_lowering::leaf::{path_value, record_path};
use crate::final_lowering::pattern_lowering::projected_pattern_state;
use crate::identity::{LocalGeneration, LocalId, PatternId, ScopeId, SyntheticKey, SyntheticOwner};
use crate::leaf::HirName;
use crate::pattern::{
    HirGenericPatternIssue, HirPattern, HirPatternBinding, HirPatternField, HirPatternKind,
    HirPatternResolver, HirPatternSequenceRest, HirPatternSequenceRestIssue,
    HirUnqualifiedVariantForm, HirVariantPatternHead, HirVariantPatternHeadIssue,
    HirVariantPatternHeadValue, HirVariantPatternName, HirVariantPatternNameIssue,
    HirVariantPatternPayload, HirVariantPatternPayloadIssue,
};
use crate::scope::HirPatternBindingPolicy;
use crate::slot::HirOrigin;
use crate::source_index::{HirInsertionPoint, HirSourceSite};

pub(super) struct CandidatePatternBinding {
    pub(super) owner: PatternId,
    pub(super) locals: Box<[LocalId]>,
    pub(super) state: HirPoisonState,
}

struct PatternBindingValidation<'a> {
    root: PatternId,
    scope: ScopeId,
    policy: HirPatternBindingPolicy,
    allocations: BTreeMap<u32, LocalId>,
    ordered: Vec<LocalId>,
    names: BTreeSet<HirName>,
    generations: &'a mut BTreeMap<HirName, LocalGeneration>,
}

impl CandidateValidationCursor<'_> {
    pub(super) fn validate_pattern_binding(
        &mut self,
        source: AttachedCandidatePatternProjection<'_>,
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
        generations: &mut BTreeMap<HirName, LocalGeneration>,
    ) -> Option<CandidatePatternBinding> {
        let owner = self.take_pattern(source, scope)?;
        let mut validation = PatternBindingValidation {
            root: owner,
            scope,
            policy,
            allocations: BTreeMap::new(),
            ordered: Vec::new(),
            names: BTreeSet::new(),
            generations,
        };
        self.validate_pattern_payload(owner, source, &mut validation)?;
        let payload = self.patterns.resolve_prepared(self.slots, owner).ok()?;
        Some(CandidatePatternBinding {
            owner,
            locals: validation.ordered.into_boxed_slice(),
            state: payload.state().clone(),
        })
    }

    fn take_pattern(
        &mut self,
        source: AttachedCandidatePatternProjection<'_>,
        scope: ScopeId,
    ) -> Option<PatternId> {
        let ordinal = self.next_pattern;
        self.next_pattern = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let owner = self
            .slots
            .resolve_prepared_synthetic::<PatternId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(owner).ok()?;
        let payload = self.patterns.resolve_prepared(self.slots, owner).ok()?;
        let site =
            HirSourceSite::from_attached_span(self.parsed.document(), &source.whole_source_span())
                .ok()?;
        if metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != &site
            || payload.scope() != scope
            || self.source_index_has_typed_owner(SyntheticOwner::Pattern(owner))
            || !self.expected.patterns.insert(owner)
        {
            return None;
        }
        Some(owner)
    }

    // The exhaustive pattern-family switch mirrors the final lowering preorder.
    #[allow(clippy::too_many_lines)]
    fn validate_pattern_payload(
        &mut self,
        owner: PatternId,
        source: AttachedCandidatePatternProjection<'_>,
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<()> {
        let mut patterns = BTreeMap::new();
        let mut typed_binding = None;
        for child in source.children()? {
            match child {
                AttachedCandidatePatternChild::Pattern { step, projection } => {
                    if patterns.insert(step, projection).is_some() {
                        return None;
                    }
                }
                AttachedCandidatePatternChild::Type { relation, node } => {
                    if relation != PatternTypeChildRelation::TypedBinding
                        || typed_binding.replace(node).is_some()
                    {
                        return None;
                    }
                }
            }
        }

        let payload = self.patterns.resolve_prepared(self.slots, owner).ok()?;
        let matches = match (source.value().kind(), payload.kind()) {
            (PatternSyntaxKind::Binding(source_binding), HirPatternKind::Binding(actual)) => self
                .binding_matches(
                owner,
                source,
                source_binding,
                actual,
                PatternBindingSiteKind::Binding,
                None,
                false,
                validation,
            )?,
            (
                PatternSyntaxKind::MutableBinding(source_binding),
                HirPatternKind::MutableBinding(actual),
            ) => self.binding_matches(
                owner,
                source,
                source_binding,
                actual,
                PatternBindingSiteKind::MutableBinding,
                None,
                true,
                validation,
            )?,
            (PatternSyntaxKind::Literal(source), HirPatternKind::Literal(actual)) => {
                crate::final_lowering::literal_projection::literal(source)
                    .is_ok_and(|expected| &expected == actual)
            }
            (
                PatternSyntaxKind::EntityReference(source),
                HirPatternKind::EntityReference(actual),
            ) => crate::final_lowering::id_ref_projection::id_ref(source)
                .is_ok_and(|expected| &expected == actual),
            (PatternSyntaxKind::Variant(source_variant), HirPatternKind::Variant(actual)) => {
                let payload_matches = match (source_variant.payload(), actual.payload()) {
                    (PatternVariantPayloadSyntax::Absent, HirVariantPatternPayload::Absent) => true,
                    (
                        PatternVariantPayloadSyntax::Resolved(_),
                        HirVariantPatternPayload::Pattern(actual_child),
                    ) => self.validate_pattern_child(
                        &mut patterns,
                        PatternNodeStep::VariantPayload,
                        *actual_child,
                        validation,
                    )?,
                    (
                        PatternVariantPayloadSyntax::Recovered {
                            value: source_child,
                            issue: source_issue,
                        },
                        HirVariantPatternPayload::Recovered {
                            pattern: actual_child,
                            issue: actual_issue,
                        },
                    ) => {
                        let child_matches = match (source_child, actual_child) {
                            (Some(_), Some(actual_child)) => self.validate_pattern_child(
                                &mut patterns,
                                PatternNodeStep::VariantPayload,
                                *actual_child,
                                validation,
                            )?,
                            (None, None) => true,
                            _ => false,
                        };
                        child_matches && variant_payload_issue(source_issue) == *actual_issue
                    }
                    _ => false,
                };
                variant_head_matches(source_variant.head(), actual.head())
                    && variant_name_matches(source_variant.name(), actual.name())
                    && payload_matches
            }
            (PatternSyntaxKind::Discard, HirPatternKind::Discard) => true,
            (PatternSyntaxKind::Tuple(source_elements), HirPatternKind::Tuple { elements }) => self
                .indexed_patterns_match(
                    &mut patterns,
                    source_elements.len(),
                    elements,
                    validation,
                )?,
            (
                PatternSyntaxKind::Record(source_record),
                HirPatternKind::Record {
                    path: actual_path,
                    fields: actual_fields,
                },
            ) => {
                if !record_path(source_record.path()).is_ok_and(|expected| &expected == actual_path)
                    || source_record.fields().len() != actual_fields.len()
                {
                    false
                } else {
                    let dispositions = classify_record_fields(source_record.fields()).ok()?;
                    let mut exact = true;
                    for (field, ((source_field, disposition), actual_field)) in source_record
                        .fields()
                        .iter()
                        .zip(dispositions)
                        .zip(actual_fields)
                        .enumerate()
                    {
                        let field = u32::try_from(field).ok()?;
                        let step = PatternNodeStep::RecordField(field);
                        exact &= match (source_field, disposition, actual_field) {
                            (
                                PatternRecordFieldSyntax::Explicit { .. },
                                RecordFieldDisposition::Explicit { name: expected },
                                HirPatternField::Explicit {
                                    name: actual,
                                    pattern,
                                },
                            ) => {
                                expected == *actual
                                    && self.validate_pattern_child(
                                        &mut patterns,
                                        step,
                                        *pattern,
                                        validation,
                                    )?
                            }
                            (
                                PatternRecordFieldSyntax::Shorthand(binding),
                                RecordFieldDisposition::Shorthand { name: expected },
                                HirPatternField::Shorthand {
                                    name: actual,
                                    local,
                                },
                            ) => {
                                expected == *actual
                                    && self.binding_local_matches(
                                        owner,
                                        source,
                                        binding,
                                        *local,
                                        PatternBindingSiteKind::RecordShorthand { field },
                                        None,
                                        false,
                                        validation,
                                    )?
                            }
                            (
                                PatternRecordFieldSyntax::Rest(source_binding),
                                RecordFieldDisposition::Rest,
                                HirPatternField::Rest {
                                    binding: actual_binding,
                                },
                            ) => match (source_binding, actual_binding) {
                                (None, None) => true,
                                (Some(binding), Some(local)) => self.binding_local_matches(
                                    owner,
                                    source,
                                    binding,
                                    *local,
                                    PatternBindingSiteKind::RecordRest { field },
                                    None,
                                    false,
                                    validation,
                                )?,
                                _ => false,
                            },
                            (
                                PatternRecordFieldSyntax::Explicit { .. },
                                RecordFieldDisposition::Invalid(expected),
                                HirPatternField::Invalid { issue: actual },
                            ) => patterns.remove(&step).is_some() && expected == *actual,
                            (
                                _,
                                RecordFieldDisposition::Invalid(expected),
                                HirPatternField::Invalid { issue: actual },
                            ) => expected == *actual,
                            _ => false,
                        };
                    }
                    exact
                }
            }
            (
                PatternSyntaxKind::BracketSequence(source_sequence),
                HirPatternKind::BracketSequence {
                    elements,
                    rest: actual_rest,
                },
            ) => {
                self.indexed_patterns_match(
                    &mut patterns,
                    source_sequence.elements().len(),
                    elements,
                    validation,
                )? && self.sequence_rest_matches(
                    owner,
                    source,
                    source_sequence.rest(),
                    *actual_rest,
                    validation,
                )?
            }
            (
                PatternSyntaxKind::WholeBinding {
                    binding: source_binding,
                    ..
                },
                HirPatternKind::WholeBinding {
                    binding: actual_binding,
                    pattern: actual_pattern,
                },
            ) => {
                self.binding_matches(
                    owner,
                    source,
                    source_binding,
                    actual_binding,
                    PatternBindingSiteKind::WholeBinding,
                    None,
                    false,
                    validation,
                )? && self.validate_pattern_child(
                    &mut patterns,
                    PatternNodeStep::NestedPattern,
                    *actual_pattern,
                    validation,
                )?
            }
            (PatternSyntaxKind::Or(source_alternatives), HirPatternKind::Or { alternatives }) => {
                self.indexed_patterns_match(
                    &mut patterns,
                    source_alternatives.len(),
                    alternatives,
                    validation,
                )?
            }
            (
                PatternSyntaxKind::TypedBinding(source_binding),
                HirPatternKind::TypedBinding {
                    binding: actual_binding,
                    ty: actual_type,
                },
            ) => {
                let type_node = typed_binding.take()?;
                let ty = self.validate_type(type_node, validation.scope)?;
                ty.id == *actual_type
                    && self.binding_matches(
                        owner,
                        source,
                        source_binding,
                        actual_binding,
                        PatternBindingSiteKind::TypedBinding,
                        Some(*actual_type),
                        false,
                        validation,
                    )?
            }
            (PatternSyntaxKind::Error, HirPatternKind::Error(actual)) => {
                actual.issue() == HirGenericPatternIssue::UnclassifiedSyntax
            }
            _ => false,
        };
        if !matches || !patterns.is_empty() || typed_binding.is_some() {
            return None;
        }
        let resolver = CandidatePatternResolver {
            slots: self.slots,
            patterns: self.patterns,
            types: self.types,
            locals: self.locals,
        };
        (projected_pattern_state(payload.kind(), source.state(), validation.scope, &resolver)
            == *payload.state())
        .then_some(())
    }

    fn validate_pattern_child(
        &mut self,
        children: &mut BTreeMap<PatternNodeStep, AttachedCandidatePatternProjection<'_>>,
        step: PatternNodeStep,
        actual: PatternId,
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<bool> {
        let source = children.remove(&step)?;
        let owner = self.take_pattern(source, validation.scope)?;
        if owner != actual {
            return Some(false);
        }
        self.validate_pattern_payload(owner, source, validation)?;
        Some(true)
    }

    fn indexed_patterns_match(
        &mut self,
        children: &mut BTreeMap<PatternNodeStep, AttachedCandidatePatternProjection<'_>>,
        source_count: usize,
        actual: &[PatternId],
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<bool> {
        if source_count != actual.len() {
            return Some(false);
        }
        for (ordinal, actual) in actual.iter().enumerate() {
            if !self.validate_pattern_child(
                children,
                PatternNodeStep::Element(u32::try_from(ordinal).ok()?),
                *actual,
                validation,
            )? {
                return Some(false);
            }
        }
        Some(true)
    }

    #[allow(clippy::too_many_arguments)]
    fn binding_matches(
        &mut self,
        owner: PatternId,
        source: AttachedCandidatePatternProjection<'_>,
        source_binding: &PatternBindingSyntax,
        actual: &HirPatternBinding,
        kind: PatternBindingSiteKind,
        annotation: Option<crate::identity::TypeId>,
        mutable: bool,
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<bool> {
        match (source_binding, actual) {
            (
                PatternBindingSyntax::Resolved(_),
                HirPatternBinding::Bound {
                    name: actual_name,
                    local,
                },
            ) => {
                let site = binding_site(source, kind)?;
                let source_name = site.binding().name()?;
                Some(
                    name(source_name).is_ok_and(|expected| &expected == actual_name)
                        && self.binding_local_matches(
                            owner,
                            source,
                            source_binding,
                            *local,
                            kind,
                            annotation,
                            mutable,
                            validation,
                        )?,
                )
            }
            (
                PatternBindingSyntax::Recovered(source_issue),
                HirPatternBinding::Recovered {
                    issue: actual_issue,
                },
            ) => Some(binding_issue(source_issue) == *actual_issue),
            _ => Some(false),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn binding_local_matches(
        &mut self,
        immediate_owner: PatternId,
        source: AttachedCandidatePatternProjection<'_>,
        source_binding: &PatternBindingSyntax,
        actual: LocalId,
        kind: PatternBindingSiteKind,
        annotation: Option<crate::identity::TypeId>,
        mutable: bool,
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<bool> {
        let site = binding_site(source, kind)?;
        let source_name = source_binding.name()?;
        let expected_name = name(source_name).ok()?;
        if let Some(retained) = validation.allocations.get(&site.ordinal()) {
            return Some(*retained == actual);
        }

        let ordinal = self.next_local;
        self.next_local = ordinal.checked_add(1)?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(self.outer), self.role, ordinal).ok()?;
        let expected = self
            .slots
            .resolve_prepared_synthetic::<LocalId>(key)
            .ok()??;
        let metadata = self.slots.resolve_prepared(expected).ok()?;
        let payload = self.locals.resolve_prepared(self.slots, expected).ok()?;
        let source_site = local_source_site(self.parsed.document(), source, kind)?;
        let duplicate = !validation.names.insert(expected_name.clone());
        let generation = if duplicate {
            validation.generations.get(&expected_name).copied()?
        } else {
            let generation = match validation.generations.get(&expected_name).copied() {
                Some(previous) => previous.checked_next()?,
                None => LocalGeneration::FIRST,
            };
            validation
                .generations
                .insert(expected_name.clone(), generation);
            generation
        };
        let pattern_owner = if matches!(
            kind,
            PatternBindingSiteKind::RecordRest { .. } | PatternBindingSiteKind::SequenceRest
        ) {
            immediate_owner
        } else {
            validation.root
        };
        let poisoned = duplicate
            || validation.policy.forbids_mutable() && mutable
            || validation.policy.reserves_result() && expected_name.as_str() == "result";
        if actual != expected
            || metadata.origin() != &HirOrigin::Synthetic(key)
            || metadata.source_site() != &source_site
            || payload.scope() != validation.scope
            || payload.kind() != validation.policy.local_kind()
            || payload.name() != &expected_name
            || payload.generation() != generation
            || payload.pattern() != Some(pattern_owner)
            || payload.annotation() != annotation
            || payload.is_mutable_binding() != mutable
            || payload.is_poisoned() != poisoned
            || self.source_index_has_typed_owner(SyntheticOwner::Local(expected))
            || !self.expected.locals.insert(expected)
        {
            return Some(false);
        }
        validation.allocations.insert(site.ordinal(), expected);
        validation.ordered.push(expected);
        Some(true)
    }

    fn sequence_rest_matches(
        &mut self,
        owner: PatternId,
        source: AttachedCandidatePatternProjection<'_>,
        source_rest: &PatternSequenceRestSyntax,
        actual: HirPatternSequenceRest,
        validation: &mut PatternBindingValidation<'_>,
    ) -> Option<bool> {
        match (source_rest, actual) {
            (PatternSequenceRestSyntax::Absent, HirPatternSequenceRest::Absent)
            | (PatternSequenceRestSyntax::Unbound, HirPatternSequenceRest::Unbound) => Some(true),
            (
                PatternSequenceRestSyntax::Binding(binding @ PatternBindingSyntax::Resolved(_))
                | PatternSequenceRestSyntax::Recovered {
                    binding: Some(binding @ PatternBindingSyntax::Resolved(_)),
                    ..
                },
                HirPatternSequenceRest::Bound(local),
            ) => self.binding_local_matches(
                owner,
                source,
                binding,
                local,
                PatternBindingSiteKind::SequenceRest,
                None,
                false,
                validation,
            ),
            (
                PatternSequenceRestSyntax::Recovered {
                    binding: Some(PatternBindingSyntax::Recovered(source_issue)),
                    ..
                },
                HirPatternSequenceRest::Recovered(HirPatternSequenceRestIssue::InvalidBinding(
                    actual_issue,
                )),
            ) => Some(binding_issue(source_issue) == actual_issue),
            (
                PatternSequenceRestSyntax::Recovered {
                    binding: None,
                    issues,
                },
                actual,
            ) => Some(recovered_sequence_rest(issues) == actual),
            _ => Some(false),
        }
    }
}

fn binding_site(
    source: AttachedCandidatePatternProjection<'_>,
    kind: PatternBindingSiteKind,
) -> Option<&PatternBindingSite> {
    source
        .binding_sites()
        .iter()
        .find(|site| site.owner() == source.path() && site.kind() == kind)
}

fn local_source_site(
    document: &arcweft_source::SourceDocument,
    source: AttachedCandidatePatternProjection<'_>,
    kind: PatternBindingSiteKind,
) -> Option<HirSourceSite> {
    let component = match kind {
        PatternBindingSiteKind::Binding
        | PatternBindingSiteKind::MutableBinding
        | PatternBindingSiteKind::TypedBinding => PatternComponentRole::Name,
        PatternBindingSiteKind::WholeBinding => PatternComponentRole::WholeBindingName,
        PatternBindingSiteKind::RecordShorthand { field } => PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Name,
        },
        PatternBindingSiteKind::RecordRest { field } => PatternComponentRole::PatternField {
            field,
            part: PatternFieldPart::Whole,
        },
        PatternBindingSiteKind::SequenceRest => {
            PatternComponentRole::SequenceRest(PatternRestPart::Whole)
        }
    };
    let span = source.component(component)?;
    let offset = if matches!(
        kind,
        PatternBindingSiteKind::RecordRest { .. } | PatternBindingSiteKind::SequenceRest
    ) {
        span.range().end()
    } else {
        span.range().start()
    };
    HirInsertionPoint::try_new(document, offset)
        .ok()
        .map(HirSourceSite::Insertion)
}

fn variant_head_matches(
    source: &PatternVariantHeadSyntax,
    actual: &HirVariantPatternHeadValue,
) -> bool {
    match (source, actual) {
        (
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Qualified(source)),
            HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(actual)),
        ) => path_value(source).is_ok_and(|expected| &expected == actual),
        (
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(source)),
            HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(actual)),
        ) => matches!(
            (source, actual),
            (
                PatternUnqualifiedVariantForm::DotShorthand,
                HirUnqualifiedVariantForm::DotShorthand
            ) | (
                PatternUnqualifiedVariantForm::BareExpectedType,
                HirUnqualifiedVariantForm::BareExpectedType
            )
        ),
        (
            PatternVariantHeadSyntax::Recovered(source),
            HirVariantPatternHeadValue::Recovered(
                HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count },
            ),
        ) => {
            !matches!(source.issue(), PatternPathIssue::InvalidRootDepth)
                && usize::try_from(*segment_count).ok() == Some(source.segments().len())
        }
        (
            PatternVariantHeadSyntax::Absent,
            HirVariantPatternHeadValue::Recovered(HirVariantPatternHeadIssue::Missing),
        ) => true,
        _ => false,
    }
}

fn variant_name_matches(source: &PatternNameSyntax, actual: &HirVariantPatternName) -> bool {
    match (source, actual) {
        (PatternNameSyntax::Resolved(source), HirVariantPatternName::Resolved(actual)) => {
            name(source).is_ok_and(|expected| &expected == actual)
        }
        (
            PatternNameSyntax::Recovered(source),
            HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Invalid(actual)),
        ) => name_issue(source) == *actual,
        (
            PatternNameSyntax::Absent,
            HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Missing),
        ) => true,
        _ => false,
    }
}

const fn variant_payload_issue(
    source: &PatternVariantPayloadIssue,
) -> HirVariantPatternPayloadIssue {
    match source {
        PatternVariantPayloadIssue::MissingPattern => HirVariantPatternPayloadIssue::MissingPattern,
        PatternVariantPayloadIssue::MissingCloseDelimiter => {
            HirVariantPatternPayloadIssue::MissingCloseDelimiter
        }
        PatternVariantPayloadIssue::InvalidPattern => HirVariantPatternPayloadIssue::InvalidPattern,
    }
}

fn recovered_sequence_rest(issues: &[PatternSequenceRestIssue]) -> HirPatternSequenceRest {
    issues
        .iter()
        .find_map(|issue| match issue {
            PatternSequenceRestIssue::InvalidBinding(issue) => {
                Some(HirPatternSequenceRest::Recovered(
                    HirPatternSequenceRestIssue::InvalidBinding(binding_issue(issue)),
                ))
            }
            PatternSequenceRestIssue::MultipleRest { .. } => None,
        })
        .unwrap_or(HirPatternSequenceRest::Unbound)
}

struct CandidatePatternResolver<'a> {
    slots: &'a crate::slot::SlotSnapshot,
    patterns: &'a crate::arena::ArenaSnapshot<HirPattern, PatternId>,
    types: &'a crate::arena::ArenaSnapshot<crate::type_ref::HirType, crate::identity::TypeId>,
    locals: &'a crate::arena::ArenaSnapshot<crate::scope::HirLocal, LocalId>,
}

impl HirPatternResolver for CandidatePatternResolver<'_> {
    fn scope_is_live(&self, scope: ScopeId) -> bool {
        self.slots.resolve_prepared(scope).is_ok()
    }

    fn local_is_visible(&self, scope: ScopeId, local: LocalId) -> bool {
        self.locals
            .resolve_prepared(self.slots, local)
            .is_ok_and(|local| local.scope() == scope)
    }

    fn resolve_type_state(
        &self,
        scope: ScopeId,
        ty: crate::identity::TypeId,
    ) -> Option<&HirPoisonState> {
        self.types
            .resolve_prepared(self.slots, ty)
            .ok()
            .filter(|ty| ty.scope() == scope)
            .map(crate::type_ref::HirType::state)
    }

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern> {
        self.patterns
            .resolve_prepared(self.slots, pattern)
            .ok()
            .filter(|pattern| pattern.scope() == scope)
    }
}
