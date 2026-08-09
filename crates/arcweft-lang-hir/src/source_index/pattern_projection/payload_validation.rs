//! Exact source-to-semantic Pattern payload validation at module freeze.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::AttachedPatternNode;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::patterns::{
    PatternBindingSite, PatternBindingSiteKind, PatternBindingSyntax, PatternNameSyntax,
    PatternPathIssue, PatternRecordFieldSyntax, PatternSequenceRestIssue,
    PatternSequenceRestSyntax, PatternSyntaxKind, PatternSyntaxState,
    PatternUnqualifiedVariantForm, PatternVariantHead, PatternVariantHeadSyntax,
    PatternVariantPayloadIssue, PatternVariantPayloadSyntax,
};

use crate::arena::ArenaSnapshot;
use crate::expr::{HirPoisonState, HirRecoveryIssue};
use crate::final_lowering::name_projection::{name, name_issue};
use crate::final_lowering::pattern_lowering::binding_plan::{
    RecordFieldDisposition, binding_issue, classify_record_fields,
};
use crate::final_lowering::pattern_lowering::leaf::{path_value, record_path};
use crate::final_lowering::pattern_lowering::projected_pattern_state;
use crate::identity::{LocalId, PatternId, ScopeId, SyntheticOwner, SyntheticRole, TypeId};
use crate::pattern::{
    HirGenericPatternIssue, HirPattern, HirPatternBinding, HirPatternField, HirPatternKind,
    HirPatternRecoveryIssue, HirPatternResolver, HirPatternSequenceRest,
    HirPatternSequenceRestIssue, HirUnqualifiedVariantForm, HirVariantPatternHead,
    HirVariantPatternHeadIssue, HirVariantPatternHeadValue, HirVariantPatternName,
    HirVariantPatternNameIssue, HirVariantPatternPayload, HirVariantPatternPayloadIssue,
};
use crate::slot::{HirOrigin, SlotSnapshot};

struct PatternRoot {
    owner: PatternId,
    attached: AttachedPatternNode,
}

pub(super) struct PatternPayloadValidation<'a> {
    parsed: &'a ParsedSource,
    slots: &'a SlotSnapshot,
    patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
    roots: Box<[PatternRoot]>,
}

impl<'a> PatternPayloadValidation<'a> {
    pub(super) fn new(
        parsed: &'a ParsedSource,
        slots: &'a SlotSnapshot,
        patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
    ) -> Option<Self> {
        let mut roots = Vec::new();
        for (owner, _) in patterns.try_iter_prepared(slots).ok()? {
            let metadata = slots.resolve_prepared(owner).ok()?;
            let HirOrigin::Source(source) = metadata.origin() else {
                continue;
            };
            let attached = parsed.attached_pattern(source.syntax()).ok()?;
            let root = attached.root().ok()?;
            if root != attached {
                continue;
            }
            if roots
                .iter()
                .any(|candidate: &PatternRoot| candidate.attached == root)
            {
                return None;
            }
            roots.push(PatternRoot {
                owner,
                attached: root,
            });
        }
        Some(Self {
            parsed,
            slots,
            patterns,
            roots: roots.into_boxed_slice(),
        })
    }

    pub(super) fn matches(
        &self,
        _owner: PatternId,
        payload: &HirPattern,
        attached: &AttachedPatternNode,
    ) -> bool {
        pattern_kind_matches(self, payload.kind(), attached)
            && pattern_state_matches(self, payload, attached)
    }

    fn root_owner(&self, attached: &AttachedPatternNode) -> Option<PatternId> {
        if attached.snapshot_id() != self.parsed.snapshot_id() {
            return None;
        }
        let attached_root = attached.root().ok()?;
        self.roots
            .iter()
            .find(|root| root.attached == attached_root)
            .map(|root| root.owner)
    }

    fn source_pattern(&self, owner: PatternId) -> Option<AttachedPatternNode> {
        let metadata = self.slots.resolve_prepared(owner).ok()?;
        let HirOrigin::Source(source) = metadata.origin() else {
            return None;
        };
        self.parsed.attached_pattern(source.syntax()).ok()
    }

    fn binding_matches(
        &self,
        attached: &AttachedPatternNode,
        source: &PatternBindingSyntax,
        actual: &HirPatternBinding,
        site_kind: PatternBindingSiteKind,
    ) -> bool {
        match (source, actual) {
            (
                PatternBindingSyntax::Resolved(source_name),
                HirPatternBinding::Bound {
                    name: actual,
                    local,
                },
            ) => {
                name(source_name).is_ok_and(|expected| &expected == actual)
                    && self.local_matches(attached, site_kind, *local)
            }
            (
                PatternBindingSyntax::Recovered(source),
                HirPatternBinding::Recovered { issue: actual },
            ) => binding_issue(source) == *actual,
            (PatternBindingSyntax::Resolved(_), HirPatternBinding::Recovered { .. })
            | (PatternBindingSyntax::Recovered(_), HirPatternBinding::Bound { .. }) => false,
        }
    }

    fn local_matches(
        &self,
        attached: &AttachedPatternNode,
        site_kind: PatternBindingSiteKind,
        local: LocalId,
    ) -> bool {
        let Some(site) = binding_site(attached, site_kind) else {
            return false;
        };
        let Ok(metadata) = self.slots.resolve_prepared(local) else {
            return false;
        };
        let HirOrigin::Synthetic(key) = metadata.origin() else {
            return false;
        };
        let SyntheticOwner::Pattern(key_owner) = key.owner() else {
            return false;
        };
        let Some(key_attached) = self.source_pattern(key_owner) else {
            return false;
        };
        if self.root_owner(attached) != self.root_owner(&key_attached) {
            return false;
        }

        if is_rest_site(site.kind()) {
            let Some(canonical) = attached
                .binding_sites()
                .iter()
                .find(|candidate| candidate.ordinal() == site.ordinal())
            else {
                return false;
            };
            key.role() == SyntheticRole::PatternRest
                && key.ordinal() == 0
                && key_attached.path() == canonical.owner()
        } else {
            let Some(ordinal) = ordinary_binding_ordinal(attached.binding_sites(), site.ordinal())
            else {
                return false;
            };
            key.role() == SyntheticRole::DestructuredBinding
                && key.ordinal() == ordinal
                && key_attached.path().steps().is_empty()
        }
    }
}

fn binding_site(
    attached: &AttachedPatternNode,
    kind: PatternBindingSiteKind,
) -> Option<&PatternBindingSite> {
    attached
        .binding_sites()
        .iter()
        .find(|site| site.owner() == attached.path() && site.kind() == kind)
}

const fn is_rest_site(kind: PatternBindingSiteKind) -> bool {
    matches!(
        kind,
        PatternBindingSiteKind::RecordRest { .. } | PatternBindingSiteKind::SequenceRest
    )
}

fn ordinary_binding_ordinal(sites: &[PatternBindingSite], target: u32) -> Option<u32> {
    let mut seen = BTreeSet::new();
    sites
        .iter()
        .filter(|site| seen.insert(site.ordinal()))
        .filter(|site| site.binding().name().is_some() && !is_rest_site(site.kind()))
        .enumerate()
        .find_map(|(ordinal, site)| {
            (site.ordinal() == target)
                .then(|| u32::try_from(ordinal).ok())
                .flatten()
        })
}

fn pattern_kind_matches(
    validation: &PatternPayloadValidation<'_>,
    actual: &HirPatternKind,
    attached: &AttachedPatternNode,
) -> bool {
    match (attached.value().kind(), actual) {
        (PatternSyntaxKind::Binding(source), HirPatternKind::Binding(actual)) => {
            validation.binding_matches(attached, source, actual, PatternBindingSiteKind::Binding)
        }
        (PatternSyntaxKind::MutableBinding(source), HirPatternKind::MutableBinding(actual)) => {
            validation.binding_matches(
                attached,
                source,
                actual,
                PatternBindingSiteKind::MutableBinding,
            )
        }
        (PatternSyntaxKind::Literal(source), HirPatternKind::Literal(actual)) => {
            crate::final_lowering::literal_projection::literal(source)
                .is_ok_and(|expected| &expected == actual)
        }
        (PatternSyntaxKind::EntityReference(source), HirPatternKind::EntityReference(actual)) => {
            crate::final_lowering::id_ref_projection::id_ref(source)
                .is_ok_and(|expected| &expected == actual)
        }
        (PatternSyntaxKind::Variant(source), HirPatternKind::Variant(actual)) => {
            variant_head_matches(source.head(), actual.head())
                && variant_name_matches(source.name(), actual.name())
                && variant_payload_matches(source.payload(), actual.payload())
        }
        (PatternSyntaxKind::Discard, HirPatternKind::Discard) => true,
        (PatternSyntaxKind::Tuple(source), HirPatternKind::Tuple { elements })
        | (
            PatternSyntaxKind::Or(source),
            HirPatternKind::Or {
                alternatives: elements,
            },
        ) => source.len() == elements.len(),
        (PatternSyntaxKind::Record(source), HirPatternKind::Record { path, fields }) => {
            record_path(source.path()).is_ok_and(|expected| &expected == path)
                && record_fields_match(validation, attached, source.fields(), fields)
        }
        (
            PatternSyntaxKind::BracketSequence(source),
            HirPatternKind::BracketSequence { elements, rest },
        ) => {
            source.elements().len() == elements.len()
                && sequence_rest_matches(validation, attached, source.rest(), *rest)
        }
        (
            PatternSyntaxKind::WholeBinding {
                binding: source, ..
            },
            HirPatternKind::WholeBinding {
                binding: actual, ..
            },
        ) => validation.binding_matches(
            attached,
            source,
            actual,
            PatternBindingSiteKind::WholeBinding,
        ),
        (
            PatternSyntaxKind::TypedBinding(source),
            HirPatternKind::TypedBinding {
                binding: actual, ..
            },
        ) => validation.binding_matches(
            attached,
            source,
            actual,
            PatternBindingSiteKind::TypedBinding,
        ),
        (PatternSyntaxKind::Error, HirPatternKind::Error(actual)) => {
            actual.issue() == HirGenericPatternIssue::UnclassifiedSyntax
        }
        _ => false,
    }
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

fn variant_payload_matches(
    source: &PatternVariantPayloadSyntax,
    actual: &HirVariantPatternPayload,
) -> bool {
    match (source, actual) {
        (PatternVariantPayloadSyntax::Absent, HirVariantPatternPayload::Absent)
        | (PatternVariantPayloadSyntax::Resolved(_), HirVariantPatternPayload::Pattern(_)) => true,
        (
            PatternVariantPayloadSyntax::Recovered {
                value: source_value,
                issue: source_issue,
            },
            HirVariantPatternPayload::Recovered {
                pattern: actual_value,
                issue: actual_issue,
            },
        ) => {
            source_value.is_some() == actual_value.is_some()
                && variant_payload_issue(source_issue) == *actual_issue
        }
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

fn record_fields_match(
    validation: &PatternPayloadValidation<'_>,
    attached: &AttachedPatternNode,
    source: &[PatternRecordFieldSyntax],
    actual: &[HirPatternField],
) -> bool {
    let Ok(dispositions) = classify_record_fields(source) else {
        return false;
    };
    source.len() == actual.len()
        && source.iter().zip(dispositions).zip(actual).enumerate().all(
            |(field, ((source, disposition), actual))| {
                let Ok(field) = u32::try_from(field) else {
                    return false;
                };
                match (source, disposition, actual) {
                    (
                        PatternRecordFieldSyntax::Explicit { .. },
                        RecordFieldDisposition::Explicit { name: expected },
                        HirPatternField::Explicit { name: actual, .. },
                    ) => expected == *actual,
                    (
                        PatternRecordFieldSyntax::Shorthand(source),
                        RecordFieldDisposition::Shorthand { name: expected },
                        HirPatternField::Shorthand {
                            name: actual,
                            local,
                        },
                    ) => {
                        expected == *actual
                            && validation.local_matches(
                                attached,
                                PatternBindingSiteKind::RecordShorthand { field },
                                *local,
                            )
                            && source.name().is_some()
                    }
                    (
                        PatternRecordFieldSyntax::Rest(source),
                        RecordFieldDisposition::Rest,
                        HirPatternField::Rest { binding: actual },
                    ) => match (source, actual) {
                        (None, None) => true,
                        (Some(PatternBindingSyntax::Resolved(_)), Some(local)) => validation
                            .local_matches(
                                attached,
                                PatternBindingSiteKind::RecordRest { field },
                                *local,
                            ),
                        _ => false,
                    },
                    (
                        _,
                        RecordFieldDisposition::Invalid(expected),
                        HirPatternField::Invalid { issue: actual },
                    ) => expected == *actual,
                    _ => false,
                }
            },
        )
}

fn sequence_rest_matches(
    validation: &PatternPayloadValidation<'_>,
    attached: &AttachedPatternNode,
    source: &PatternSequenceRestSyntax,
    actual: HirPatternSequenceRest,
) -> bool {
    match (source, actual) {
        (PatternSequenceRestSyntax::Absent, HirPatternSequenceRest::Absent)
        | (PatternSequenceRestSyntax::Unbound, HirPatternSequenceRest::Unbound) => true,
        (
            PatternSequenceRestSyntax::Binding(PatternBindingSyntax::Resolved(_))
            | PatternSequenceRestSyntax::Recovered {
                binding: Some(PatternBindingSyntax::Resolved(_)),
                ..
            },
            HirPatternSequenceRest::Bound(local),
        ) => validation.local_matches(attached, PatternBindingSiteKind::SequenceRest, local),
        (
            PatternSequenceRestSyntax::Recovered {
                binding: Some(PatternBindingSyntax::Recovered(source)),
                ..
            },
            HirPatternSequenceRest::Recovered(HirPatternSequenceRestIssue::InvalidBinding(actual)),
        ) => binding_issue(source) == actual,
        (
            PatternSequenceRestSyntax::Recovered {
                binding: None,
                issues,
            },
            actual,
        ) => recovered_sequence_rest(issues) == actual,
        _ => false,
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

fn pattern_state_matches(
    validation: &PatternPayloadValidation<'_>,
    payload: &HirPattern,
    attached: &AttachedPatternNode,
) -> bool {
    if !has_container_recovery(attached.state()) && !is_container_recovery(payload.state()) {
        return true;
    }
    let resolver = PatternStateResolver {
        slots: validation.slots,
        patterns: validation.patterns,
    };
    projected_pattern_state(payload.kind(), attached.state(), payload.scope(), &resolver)
        == *payload.state()
}

fn has_container_recovery(state: &PatternSyntaxState) -> bool {
    state.issues().iter().any(|issue| {
        matches!(
            issue,
            arcweft_lang_syntax::patterns::PatternRecoveryIssue::MissingCloseDelimiter
                | arcweft_lang_syntax::patterns::PatternRecoveryIssue::MissingOrAlternative { .. }
                | arcweft_lang_syntax::patterns::PatternRecoveryIssue::SequenceRest(
                    PatternSequenceRestIssue::MultipleRest { .. }
                )
        )
    })
}

fn is_container_recovery(state: &HirPoisonState) -> bool {
    matches!(
        state,
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
            HirPatternRecoveryIssue::MissingCloseDelimiter
                | HirPatternRecoveryIssue::MissingOrAlternative { .. }
                | HirPatternRecoveryIssue::SequenceRest(
                    HirPatternSequenceRestIssue::MultipleRest { .. }
                )
        ))
    )
}

struct PatternStateResolver<'a> {
    slots: &'a SlotSnapshot,
    patterns: &'a ArenaSnapshot<HirPattern, PatternId>,
}

impl HirPatternResolver for PatternStateResolver<'_> {
    fn scope_is_live(&self, _: ScopeId) -> bool {
        true
    }

    fn local_is_visible(&self, _: ScopeId, _: LocalId) -> bool {
        true
    }

    fn resolve_type_state(&self, _: ScopeId, _: TypeId) -> Option<&HirPoisonState> {
        None
    }

    fn resolve_pattern(&self, scope: ScopeId, pattern: PatternId) -> Option<&HirPattern> {
        self.patterns
            .resolve_prepared(self.slots, pattern)
            .ok()
            .filter(|pattern| pattern.scope() == scope)
    }
}
