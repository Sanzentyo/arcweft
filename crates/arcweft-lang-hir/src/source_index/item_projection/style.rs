//! Native Style source-manifest projection and payload freeze.

mod expression_owners;
mod role_validation;

pub(super) use expression_owners::retained_expression_owners;

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{
    AttachedStyleAssignment, AttachedStyleAssignmentState, AttachedStyleBody,
    AttachedStyleDeclaration, AttachedStyleEnvironment, AttachedStyleEnvironmentComparison,
    AttachedStyleEnvironmentField, AttachedStyleExpression, AttachedStyleId, AttachedStyleMember,
    AttachedStyleName, AttachedStyleProperty, AttachedStyleRule, AttachedStyleSelector,
    AttachedStyleSelectorSequence, AttachedStyleToken, StyleEnvironmentComparisonKind,
    StyleEnvironmentConditionIssue, StyleEnvironmentFieldKind, StyleIdForm, StylePropertyOperation,
    StyleSelectorRelation, StyleSyntaxNameIssue,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::identity::{ExprId, ItemId, SyntheticOwner};
use crate::item::{
    HirItem, HirItemIssue, HirItemKind, HirStyleAssignOperation, HirStyleAssignOperationIssue,
    HirStyleBodyIssue, HirStyleBodyItem, HirStyleCombinator, HirStyleDeclaration,
    HirStyleEnvironment, HirStyleEnvironmentComparison, HirStyleEnvironmentComparisonIssue,
    HirStyleEnvironmentField, HirStyleEnvironmentFieldIssue, HirStyleItem, HirStyleName,
    HirStyleNameIssue, HirStyleRule, HirStyleSelector, HirStyleSelectorIssue,
    HirStyleSelectorSequence, HirStyleToken, HirStyleTokenIssue,
};
use crate::slot::SlotSnapshot;

use super::super::{
    HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery,
    HirSourceRequirement, HirSourceSite, HirStyleBodyPath, HirStyleBodySourcePart,
    HirStyleSourceRole, HirStyleTokenSourcePart, StagedHirSourceIndex,
};
use super::{
    expression_tree_is_unallocated, item_prefix_matches, item_state, prefix_issue,
    slot_is_poisoned, source_matches, type_tree_is_unallocated,
};

#[derive(Default)]
struct StyleManifest {
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

impl StyleManifest {
    fn insert(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirStyleSourceRole,
        requirement: HirSourceRequirement,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        let query = HirSourceQuery::Item {
            owner,
            role: HirItemSourceRole::Style(role),
        };
        if self
            .requirements
            .insert(query.clone(), requirement)
            .is_some()
        {
            return Err(HirSourceCommitInvariantError::ConflictingRequirement { query });
        }
        if let Some(span) = span {
            let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
            if self.components.insert(query.clone(), site).is_some() {
                return Err(HirSourceCommitInvariantError::ConflictingComponent { query });
            }
        } else if requirement == HirSourceRequirement::Required {
            return Err(HirSourceCommitInvariantError::MissingRequiredComponent { query });
        }
        Ok(())
    }

    fn required(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirStyleSourceRole,
        span: SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            owner,
            role,
            HirSourceRequirement::Required,
            Some(span),
        )
    }

    fn optional(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirStyleSourceRole,
        span: Option<SourceSpan>,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.insert(
            parsed,
            owner,
            role,
            if span.is_some() {
                HirSourceRequirement::Required
            } else {
                HirSourceRequirement::Optional
            },
            span,
        )
    }
}

impl StagedHirSourceIndex {
    /// Projects the exact item-owned Style component manifest.
    ///
    /// Expression initializers and token type annotations are deliberately
    /// absent: their existing expression/type manifests remain their sole
    /// source authorities.
    pub(crate) fn stage_attached_style(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &AttachedStyleDeclaration,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.syntax().snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.syntax().snapshot_id().clone(),
            });
        }
        let manifest = match style_manifest(parsed, owner, attached) {
            Ok(manifest) => manifest,
            Err(error) => return self.reject(error),
        };

        for (query, requirement) in manifest.requirements {
            self.require(&query, requirement)?;
        }
        for (query, site) in manifest.components {
            self.stage(&query, site)?;
        }
        Ok(())
    }
}

pub(super) fn payload_matches(
    index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedStyleDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
) -> bool {
    let HirItemKind::Style(style) = item.kind() else {
        return false;
    };
    item.members().is_empty()
        && item_prefix_matches(item, attached.prefix(), slots)
        && id_ref_matches(style.id(), attached.id().reference())
        && style_id_expression_is_unallocated(attached.id(), slots)
        && outer_body_shape_is_consistent(attached.body())
        && style_tokens_match(style.tokens(), attached.body(), slots)
        && style_body_matches(style.body(), attached.body(), true, slots)
        && item.state() == &expected_item_state(attached, style, item, slots)
        && exact_style_manifest(index, parsed, owner, attached)
}

fn style_manifest(
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AttachedStyleDeclaration,
) -> Result<StyleManifest, HirSourceCommitInvariantError> {
    let mut manifest = StyleManifest::default();
    manifest.required(
        parsed,
        owner,
        HirStyleSourceRole::ItemId,
        attached.id().source_span(),
    )?;
    manifest.required(
        parsed,
        owner,
        HirStyleSourceRole::Body {
            path: HirStyleBodyPath::root(),
            part: HirStyleBodySourcePart::BodyWhole,
        },
        attached.body().source_span(),
    )?;
    project_body_manifest(
        &mut manifest,
        parsed,
        owner,
        attached.body(),
        &HirStyleBodyPath::root(),
        true,
    )?;
    Ok(manifest)
}

fn project_body_manifest(
    manifest: &mut StyleManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    body: &AttachedStyleBody,
    path: &HirStyleBodyPath,
    outer: bool,
) -> Result<(), HirSourceCommitInvariantError> {
    let mut token_ordinal = 0_u32;
    let mut body_ordinal = 0_u32;
    for member in body.members() {
        match member {
            AttachedStyleMember::Token(token) if outer => {
                project_token_manifest(manifest, parsed, owner, token_ordinal, token)?;
                token_ordinal = token_ordinal.checked_add(1).ok_or(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: SyntheticOwner::Item(owner),
                    },
                )?;
            }
            AttachedStyleMember::Rule(rule) if rule_whole_issue(rule).is_none() => {
                project_rule_manifest(manifest, parsed, owner, path, body_ordinal, rule)?;
                body_ordinal = next_body_ordinal(owner, body_ordinal)?;
            }
            AttachedStyleMember::Environment(environment)
                if environment_whole_issue(environment).is_none() =>
            {
                project_environment_manifest(
                    manifest,
                    parsed,
                    owner,
                    path,
                    body_ordinal,
                    environment,
                )?;
                body_ordinal = next_body_ordinal(owner, body_ordinal)?;
            }
            AttachedStyleMember::Token(_)
            | AttachedStyleMember::Rule(_)
            | AttachedStyleMember::Environment(_)
            | AttachedStyleMember::Error { .. } => {
                body_ordinal = next_body_ordinal(owner, body_ordinal)?;
            }
        }
    }
    Ok(())
}

fn next_body_ordinal(owner: ItemId, ordinal: u32) -> Result<u32, HirSourceCommitInvariantError> {
    ordinal.checked_add(1).ok_or(
        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
            owner: SyntheticOwner::Item(owner),
        },
    )
}

fn project_token_manifest(
    manifest: &mut StyleManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    ordinal: u32,
    token: &AttachedStyleToken,
) -> Result<(), HirSourceCommitInvariantError> {
    for (part, span) in [
        (HirStyleTokenSourcePart::Whole, token.syntax().source_span()),
        (
            HirStyleTokenSourcePart::Key,
            token.name().syntax().source_span(),
        ),
        (
            HirStyleTokenSourcePart::Assignment,
            token.assignment().source_span(),
        ),
    ] {
        manifest.required(
            parsed,
            owner,
            HirStyleSourceRole::Token { ordinal, part },
            span,
        )?;
    }
    Ok(())
}

fn project_rule_manifest(
    manifest: &mut StyleManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    path: &HirStyleBodyPath,
    rule_ordinal: u32,
    rule: &AttachedStyleRule,
) -> Result<(), HirSourceCommitInvariantError> {
    let body_role = |part| HirStyleSourceRole::Body {
        path: path.clone(),
        part,
    };
    manifest.required(
        parsed,
        owner,
        body_role(HirStyleBodySourcePart::RuleSelector { rule: rule_ordinal }),
        rule.selector().syntax().source_span(),
    )?;
    for (sequence_index, sequence) in rule.selector().sequences().iter().enumerate() {
        let sequence_ordinal = semantic_ordinal(owner, sequence_index)?;
        manifest.required(
            parsed,
            owner,
            body_role(HirStyleBodySourcePart::RuleSequence {
                rule: rule_ordinal,
                sequence: sequence_ordinal,
            }),
            sequence.syntax().source_span(),
        )?;
        manifest.optional(
            parsed,
            owner,
            body_role(HirStyleBodySourcePart::RuleElement {
                rule: rule_ordinal,
                sequence: sequence_ordinal,
            }),
            sequence
                .element()
                .map(|element| element.syntax().source_span()),
        )?;
        manifest.optional(
            parsed,
            owner,
            body_role(HirStyleBodySourcePart::RulePart {
                rule: rule_ordinal,
                sequence: sequence_ordinal,
            }),
            sequence
                .part()
                .map(|part| part.name().syntax().source_span()),
        )?;
        for (predicate_index, predicate) in sequence.predicates().iter().enumerate() {
            manifest.required(
                parsed,
                owner,
                body_role(HirStyleBodySourcePart::RulePredicate {
                    rule: rule_ordinal,
                    sequence: sequence_ordinal,
                    predicate: semantic_ordinal(owner, predicate_index)?,
                }),
                predicate.name().syntax().source_span(),
            )?;
        }
    }
    for (declaration_index, declaration) in rule.body().declarations().iter().enumerate() {
        let declaration_ordinal = semantic_ordinal(owner, declaration_index)?;
        for (part, span) in [
            (
                HirStyleBodySourcePart::DeclarationWhole {
                    rule: rule_ordinal,
                    declaration: declaration_ordinal,
                },
                declaration.syntax().source_span(),
            ),
            (
                HirStyleBodySourcePart::DeclarationProperty {
                    rule: rule_ordinal,
                    declaration: declaration_ordinal,
                },
                declaration.name().syntax().source_span(),
            ),
            (
                HirStyleBodySourcePart::DeclarationAssignment {
                    rule: rule_ordinal,
                    declaration: declaration_ordinal,
                },
                declaration.assignment().source_span(),
            ),
        ] {
            manifest.required(parsed, owner, body_role(part), span)?;
        }
    }
    Ok(())
}

fn project_environment_manifest(
    manifest: &mut StyleManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    path: &HirStyleBodyPath,
    environment_ordinal: u32,
    environment: &AttachedStyleEnvironment,
) -> Result<(), HirSourceCommitInvariantError> {
    let body_role = |part| HirStyleSourceRole::Body {
        path: path.clone(),
        part,
    };
    for (part, span) in [
        (
            HirStyleBodySourcePart::EnvironmentWhole {
                environment: environment_ordinal,
            },
            environment.syntax().source_span(),
        ),
        (
            HirStyleBodySourcePart::EnvironmentCondition {
                environment: environment_ordinal,
            },
            environment.condition().syntax().source_span(),
        ),
        (
            HirStyleBodySourcePart::EnvironmentBody {
                environment: environment_ordinal,
            },
            environment.body().source_span(),
        ),
    ] {
        manifest.required(parsed, owner, body_role(part), span)?;
    }
    for (clause_index, clause) in environment.condition().clauses().iter().enumerate() {
        let clause_ordinal = semantic_ordinal(owner, clause_index)?;
        for (part, span) in [
            (
                HirStyleBodySourcePart::ClauseWhole {
                    environment: environment_ordinal,
                    clause: clause_ordinal,
                },
                clause.syntax().source_span(),
            ),
            (
                HirStyleBodySourcePart::ClauseField {
                    environment: environment_ordinal,
                    clause: clause_ordinal,
                },
                clause.field().name().syntax().source_span(),
            ),
            (
                HirStyleBodySourcePart::ClauseComparison {
                    environment: environment_ordinal,
                    clause: clause_ordinal,
                },
                clause.comparison().source_span(),
            ),
        ] {
            manifest.required(parsed, owner, body_role(part), span)?;
        }
    }

    let mut ordinals = path.ordinals().to_vec();
    ordinals.push(environment_ordinal);
    let nested_path = HirStyleBodyPath::from_ordinals(ordinals.into_boxed_slice());
    manifest.required(
        parsed,
        owner,
        HirStyleSourceRole::Body {
            path: nested_path.clone(),
            part: HirStyleBodySourcePart::BodyWhole,
        },
        environment.body().source_span(),
    )?;
    project_body_manifest(
        manifest,
        parsed,
        owner,
        environment.body(),
        &nested_path,
        false,
    )
}

fn semantic_ordinal(owner: ItemId, index: usize) -> Result<u32, HirSourceCommitInvariantError> {
    u32::try_from(index).map_err(
        |_| HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
            owner: SyntheticOwner::Item(owner),
        },
    )
}

fn exact_style_manifest(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AttachedStyleDeclaration,
) -> bool {
    let Ok(expected) = style_manifest(parsed, owner, attached) else {
        return false;
    };
    let semantic_owner = SyntheticOwner::Item(owner);
    !index.syntax_owners.contains_key(&semantic_owner)
        && index
            .requirements
            .iter()
            .filter(|(query, _)| query.owner() == semantic_owner)
            .eq(expected.requirements.iter())
        && index
            .components
            .iter()
            .filter(|(query, _)| query.owner() == semantic_owner)
            .eq(expected.components.iter())
}

fn style_tokens_match(
    retained: &[HirStyleToken],
    body: &AttachedStyleBody,
    slots: &SlotSnapshot,
) -> bool {
    let attached = body
        .members()
        .iter()
        .filter_map(|member| match member {
            AttachedStyleMember::Token(token) => Some(token),
            _ => None,
        })
        .collect::<Vec<_>>();
    retained.len() == attached.len()
        && retained.iter().zip(attached).all(|(retained, attached)| {
            attached.is_allowed_at_this_depth() && style_token_matches(retained, attached, slots)
        })
}

fn style_token_matches(
    retained: &HirStyleToken,
    attached: &AttachedStyleToken,
    slots: &SlotSnapshot,
) -> bool {
    id_ref_matches(retained.id(), Some(attached.id()))
        && match (retained.value_type(), attached.type_annotation()) {
            (Some(retained), Some(attached)) => {
                !attached.colon().range().is_empty()
                    && source_matches(slots, retained, attached.value().id())
            }
            (None, None) => true,
            _ => false,
        }
        && style_expression_matches(retained.value(), attached.value(), slots)
        && retained.recovery_issue() == token_assignment_issue(attached.assignment())
        && assignment_shape_is_consistent(attached.assignment())
}

fn style_body_matches(
    retained: &[HirStyleBodyItem],
    attached: &AttachedStyleBody,
    outer: bool,
    slots: &SlotSnapshot,
) -> bool {
    let expected_len = attached
        .members()
        .iter()
        .filter(|member| !matches!(member, AttachedStyleMember::Token(_) if outer))
        .count();
    if retained.len() != expected_len {
        return false;
    }
    let mut retained = retained.iter();
    for (source_ordinal, attached) in attached.members().iter().enumerate() {
        if usize::try_from(attached.source_ordinal()).ok() != Some(source_ordinal) {
            return false;
        }
        match attached {
            AttachedStyleMember::Token(token) if outer => {
                if !token.is_allowed_at_this_depth() {
                    return false;
                }
                continue;
            }
            _ => {}
        }
        let Some(retained) = retained.next() else {
            return false;
        };
        let matches = match (retained, attached) {
            (
                HirStyleBodyItem::Recovered(HirStyleBodyIssue::Unexpected),
                AttachedStyleMember::Token(token),
            ) => {
                !token.is_allowed_at_this_depth()
                    && style_token_children_are_unallocated(token, slots)
            }
            (
                HirStyleBodyItem::Recovered(HirStyleBodyIssue::Malformed),
                AttachedStyleMember::Error { .. },
            ) => true,
            (HirStyleBodyItem::Recovered(retained_issue), AttachedStyleMember::Rule(rule)) => {
                rule_whole_issue(rule) == Some(*retained_issue)
                    && rule_children_are_unallocated(rule, slots)
            }
            (HirStyleBodyItem::Rule(retained), AttachedStyleMember::Rule(attached)) => {
                rule_whole_issue(attached).is_none()
                    && style_rule_matches(retained, attached, slots)
            }
            (
                HirStyleBodyItem::Recovered(retained_issue),
                AttachedStyleMember::Environment(environment),
            ) => {
                environment_whole_issue(environment) == Some(*retained_issue)
                    && environment_children_are_unallocated(environment, slots)
            }
            (
                HirStyleBodyItem::Environment(retained),
                AttachedStyleMember::Environment(attached),
            ) => {
                environment_whole_issue(attached).is_none()
                    && style_environment_matches(retained, attached, slots)
            }
            _ => false,
        };
        if !matches {
            return false;
        }
    }
    retained.next().is_none()
}

fn style_rule_matches(
    retained: &HirStyleRule,
    attached: &AttachedStyleRule,
    slots: &SlotSnapshot,
) -> bool {
    !attached.body().open_delimiter().range().is_empty()
        && !attached.body().close_delimiter().range().is_empty()
        && style_selector_matches(retained.selector(), attached.selector())
        && retained.declarations().len() == attached.body().declarations().len()
        && retained
            .declarations()
            .iter()
            .zip(attached.body().declarations())
            .enumerate()
            .all(|(ordinal, (retained, attached))| {
                usize::try_from(attached.source_ordinal()).ok() == Some(ordinal)
                    && style_declaration_matches(retained, attached, slots)
            })
}

fn style_selector_matches(retained: &HirStyleSelector, attached: &AttachedStyleSelector) -> bool {
    if !attached
        .sequences()
        .iter()
        .enumerate()
        .all(|(sequence_ordinal, sequence)| {
            u32::try_from(sequence_ordinal).ok() == Some(sequence.source_ordinal())
                && sequence
                    .relation()
                    .is_none_or(|relation| !relation.source_span().range().is_empty())
                && sequence
                    .part()
                    .is_none_or(|part| !part.separator_span().range().is_empty())
                && sequence
                    .predicates()
                    .iter()
                    .enumerate()
                    .all(|(predicate_ordinal, predicate)| {
                        u16::try_from(predicate_ordinal).ok() == Some(predicate.source_ordinal())
                            && !predicate.colon_span().range().is_empty()
                    })
        })
    {
        return false;
    }
    let expected = attached
        .sequences()
        .iter()
        .map(expected_selector_sequence)
        .collect::<Vec<_>>();
    if retained.sequences() != expected.as_slice() {
        return false;
    }
    retained.recovery_issue() == expected_selector_issue(attached, &expected)
}

fn expected_selector_sequence(
    attached: &AttachedStyleSelectorSequence,
) -> HirStyleSelectorSequence {
    HirStyleSelectorSequence::new(
        attached.relation().map(|relation| match relation.value() {
            StyleSelectorRelation::Descendant => HirStyleCombinator::Descendant,
            StyleSelectorRelation::Child => HirStyleCombinator::Child,
        }),
        attached.element().map(expected_style_name),
        attached.part().map(|part| expected_style_name(part.name())),
        attached
            .predicates()
            .iter()
            .map(|predicate| expected_style_name(predicate.name()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn expected_selector_issue(
    attached: &AttachedStyleSelector,
    sequences: &[HirStyleSelectorSequence],
) -> Option<HirStyleSelectorIssue> {
    let Some((first, rest)) = sequences.split_first() else {
        return Some(HirStyleSelectorIssue::MissingSequence);
    };
    if attached.missing().is_some() {
        return Some(HirStyleSelectorIssue::MissingSequence);
    }
    if !attached.recoveries().is_empty()
        || first.relation_to_previous().is_some()
        || rest
            .iter()
            .any(|sequence| sequence.relation_to_previous().is_none())
    {
        return Some(HirStyleSelectorIssue::InvalidRelation);
    }
    if sequences.iter().any(|sequence| {
        sequence.element().is_none()
            && sequence.part().is_none()
            && sequence.predicates().is_empty()
    }) {
        return Some(HirStyleSelectorIssue::MissingComponent);
    }
    attached
        .sequences()
        .iter()
        .any(AttachedStyleSelectorSequence::has_recovery)
        .then_some(HirStyleSelectorIssue::InvalidComponent)
}

fn style_declaration_matches(
    retained: &HirStyleDeclaration,
    attached: &AttachedStyleProperty,
    slots: &SlotSnapshot,
) -> bool {
    let append_is_consistent = match attached.operation() {
        StylePropertyOperation::Replace => attached.append_keyword().is_none(),
        StylePropertyOperation::Append => attached
            .append_keyword()
            .is_some_and(|keyword| !keyword.range().is_empty()),
    };
    expected_style_name(attached.name()) == *retained.property()
        && style_expression_matches(retained.value(), attached.value(), slots)
        && retained.operation() == expected_assignment_operation(attached)
        && assignment_shape_is_consistent(attached.assignment())
        && append_is_consistent
}

fn style_environment_matches(
    retained: &HirStyleEnvironment,
    attached: &AttachedStyleEnvironment,
    slots: &SlotSnapshot,
) -> bool {
    let condition = attached.condition();
    attached.intrinsic().value().is_ok_and(|name| {
        name.as_str() == "environment" && !attached.intrinsic().syntax().range().is_empty()
    }) && !condition.open_delimiter().range().is_empty()
        && !condition.close_delimiter().range().is_empty()
        && condition.recoveries().is_empty()
        && retained.clauses().len() == condition.clauses().len()
        && retained
            .clauses()
            .iter()
            .zip(condition.clauses())
            .enumerate()
            .all(|(ordinal, (retained, attached))| {
                usize::from(attached.source_ordinal()) == ordinal
                    && retained.field() == expected_environment_field(attached.field())
                    && retained.comparison()
                        == expected_environment_comparison(attached.comparison())
                    && environment_comparison_shape_is_consistent(attached.comparison())
                    && style_expression_matches(retained.value(), attached.value(), slots)
            })
        && style_body_matches(retained.body(), attached.body(), false, slots)
}

fn expected_style_name(attached: &AttachedStyleName) -> HirStyleName {
    match attached.value() {
        Ok(value) => HirStyleName::try_new(value.as_str().into())
            .unwrap_or_else(|_| HirStyleName::recovered(HirStyleNameIssue::Invalid)),
        Err(StyleSyntaxNameIssue::Missing) => HirStyleName::recovered(HirStyleNameIssue::Missing),
        Err(
            StyleSyntaxNameIssue::EmptyComponent { .. }
            | StyleSyntaxNameIssue::InvalidComponent { .. },
        ) => HirStyleName::recovered(HirStyleNameIssue::Invalid),
    }
}

fn expected_assignment_operation(attached: &AttachedStyleProperty) -> HirStyleAssignOperation {
    match attached.assignment().state() {
        AttachedStyleAssignmentState::Missing => {
            HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Missing)
        }
        AttachedStyleAssignmentState::Unsupported => {
            HirStyleAssignOperation::Recovered(HirStyleAssignOperationIssue::Invalid)
        }
        AttachedStyleAssignmentState::Authored => match attached.operation() {
            StylePropertyOperation::Replace => HirStyleAssignOperation::Replace,
            StylePropertyOperation::Append => HirStyleAssignOperation::Append,
        },
    }
}

fn expected_environment_field(
    attached: &AttachedStyleEnvironmentField,
) -> HirStyleEnvironmentField {
    match attached {
        AttachedStyleEnvironmentField::Known { value, .. } => match value {
            StyleEnvironmentFieldKind::ColorScheme => HirStyleEnvironmentField::ColorScheme,
            StyleEnvironmentFieldKind::Contrast => HirStyleEnvironmentField::Contrast,
            StyleEnvironmentFieldKind::ReducedMotion => HirStyleEnvironmentField::ReducedMotion,
            StyleEnvironmentFieldKind::TextScale => HirStyleEnvironmentField::TextScale,
        },
        AttachedStyleEnvironmentField::Unsupported(_) => {
            HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Unknown)
        }
        AttachedStyleEnvironmentField::Missing(_) => {
            HirStyleEnvironmentField::Recovered(HirStyleEnvironmentFieldIssue::Missing)
        }
    }
}

fn expected_environment_comparison(
    attached: &AttachedStyleEnvironmentComparison,
) -> HirStyleEnvironmentComparison {
    match attached {
        AttachedStyleEnvironmentComparison::Known { value, .. } => match value {
            StyleEnvironmentComparisonKind::Equal => HirStyleEnvironmentComparison::Equal,
            StyleEnvironmentComparisonKind::NotEqual => HirStyleEnvironmentComparison::NotEqual,
            StyleEnvironmentComparisonKind::Less => HirStyleEnvironmentComparison::Less,
            StyleEnvironmentComparisonKind::LessOrEqual => {
                HirStyleEnvironmentComparison::LessOrEqual
            }
            StyleEnvironmentComparisonKind::Greater => HirStyleEnvironmentComparison::Greater,
            StyleEnvironmentComparisonKind::GreaterOrEqual => {
                HirStyleEnvironmentComparison::GreaterOrEqual
            }
        },
        AttachedStyleEnvironmentComparison::Unsupported { .. } => {
            HirStyleEnvironmentComparison::Recovered(HirStyleEnvironmentComparisonIssue::Invalid)
        }
        AttachedStyleEnvironmentComparison::Missing { .. } => {
            HirStyleEnvironmentComparison::Recovered(HirStyleEnvironmentComparisonIssue::Missing)
        }
    }
}

fn token_assignment_issue(assignment: &AttachedStyleAssignment) -> Option<HirStyleTokenIssue> {
    match assignment.state() {
        AttachedStyleAssignmentState::Authored => None,
        AttachedStyleAssignmentState::Missing => Some(HirStyleTokenIssue::MissingAssignment),
        AttachedStyleAssignmentState::Unsupported => Some(HirStyleTokenIssue::MalformedAssignment),
    }
}

fn assignment_shape_is_consistent(assignment: &AttachedStyleAssignment) -> bool {
    match assignment.state() {
        AttachedStyleAssignmentState::Authored => {
            !assignment.equals().range().is_empty()
                && assignment.unsupported_syntax().is_none()
                && assignment.equals().source_span() == assignment.source_span()
        }
        AttachedStyleAssignmentState::Missing => {
            assignment.equals().range().is_empty()
                && assignment.unsupported_syntax().is_none()
                && assignment.equals().source_span() == assignment.source_span()
        }
        AttachedStyleAssignmentState::Unsupported => {
            assignment.equals().range().is_empty()
                && assignment.unsupported_syntax().is_some_and(|unsupported| {
                    !unsupported.range().is_empty()
                        && unsupported.source_span() == assignment.source_span()
                })
        }
    }
}

fn environment_comparison_shape_is_consistent(
    comparison: &AttachedStyleEnvironmentComparison,
) -> bool {
    match comparison {
        AttachedStyleEnvironmentComparison::Known { .. }
        | AttachedStyleEnvironmentComparison::Unsupported { .. } => {
            !comparison.source_span().range().is_empty()
        }
        AttachedStyleEnvironmentComparison::Missing { .. } => {
            comparison.source_span().range().is_empty()
        }
    }
}

fn outer_body_shape_is_consistent(body: &AttachedStyleBody) -> bool {
    match body {
        AttachedStyleBody::Missing(syntax) => syntax.range().is_empty(),
        AttachedStyleBody::Braced { open, .. } => !open.range().is_empty(),
    }
}

fn rule_whole_issue(rule: &AttachedStyleRule) -> Option<HirStyleBodyIssue> {
    (rule.body().open_delimiter().range().is_empty()
        || rule.body().close_delimiter().range().is_empty())
    .then_some(HirStyleBodyIssue::Malformed)
}

fn environment_whole_issue(environment: &AttachedStyleEnvironment) -> Option<HirStyleBodyIssue> {
    match environment.intrinsic().value() {
        Err(StyleSyntaxNameIssue::Missing) => return Some(HirStyleBodyIssue::Missing),
        Ok(name) if name.as_str() == "environment" => {}
        Ok(_)
        | Err(
            StyleSyntaxNameIssue::EmptyComponent { .. }
            | StyleSyntaxNameIssue::InvalidComponent { .. },
        ) => return Some(HirStyleBodyIssue::Malformed),
    }
    let condition = environment.condition();
    if condition.open_delimiter().range().is_empty()
        || condition.close_delimiter().range().is_empty()
    {
        return Some(HirStyleBodyIssue::Malformed);
    }
    if let Some(recovery) = condition.recoveries().first() {
        return Some(match recovery.issue() {
            StyleEnvironmentConditionIssue::EmptyCondition => HirStyleBodyIssue::Missing,
            StyleEnvironmentConditionIssue::EmptyClause
            | StyleEnvironmentConditionIssue::TrailingComma => HirStyleBodyIssue::Malformed,
        });
    }
    match environment.body() {
        AttachedStyleBody::Missing(_) => Some(HirStyleBodyIssue::Missing),
        AttachedStyleBody::Braced { open, close, .. }
            if open.range().is_empty() || close.range().is_empty() =>
        {
            Some(HirStyleBodyIssue::Malformed)
        }
        AttachedStyleBody::Braced { .. } => None,
    }
}

fn id_ref_matches(
    retained: &crate::leaf::HirIdRefValue,
    attached: Option<&arcweft_lang_syntax::id_ref::SyntaxIdRefSyntax>,
) -> bool {
    attached.is_some_and(|attached| {
        crate::final_lowering::id_ref_projection::id_ref(attached)
            .is_ok_and(|expected| &expected == retained)
    })
}

fn style_id_expression_is_unallocated(id: &AttachedStyleId, slots: &SlotSnapshot) -> bool {
    match id {
        AttachedStyleId::Authored {
            syntax,
            form: StyleIdForm::Explicit,
            ..
        } => slots.prepared_source_owner::<ExprId>(syntax.id()).is_none(),
        AttachedStyleId::Authored {
            form: StyleIdForm::Bare,
            ..
        }
        | AttachedStyleId::Invalid { .. }
        | AttachedStyleId::Missing { .. } => true,
    }
}

fn style_expression_matches(
    retained: ExprId,
    attached: &AttachedStyleExpression,
    slots: &SlotSnapshot,
) -> bool {
    match attached {
        AttachedStyleExpression::Authored(attached) => {
            source_matches(slots, retained, attached.id())
        }
        AttachedStyleExpression::Missing(attached) => {
            source_matches(slots, retained, attached.id())
        }
    }
}

fn style_expression_is_unallocated(
    attached: &AttachedStyleExpression,
    slots: &SlotSnapshot,
) -> bool {
    match attached {
        AttachedStyleExpression::Authored(attached) => {
            expression_tree_is_unallocated(attached, slots, &mut BTreeSet::new())
        }
        AttachedStyleExpression::Missing(attached) => slots
            .prepared_source_owner::<ExprId>(attached.id())
            .is_none(),
    }
}

fn style_token_children_are_unallocated(token: &AttachedStyleToken, slots: &SlotSnapshot) -> bool {
    token
        .type_annotation()
        .is_none_or(|annotation| type_tree_is_unallocated(annotation.value(), slots))
        && style_expression_is_unallocated(token.value(), slots)
}

fn rule_children_are_unallocated(rule: &AttachedStyleRule, slots: &SlotSnapshot) -> bool {
    rule.body()
        .declarations()
        .iter()
        .all(|declaration| style_expression_is_unallocated(declaration.value(), slots))
}

fn environment_children_are_unallocated(
    environment: &AttachedStyleEnvironment,
    slots: &SlotSnapshot,
) -> bool {
    environment
        .condition()
        .clauses()
        .iter()
        .all(|clause| style_expression_is_unallocated(clause.value(), slots))
        && body_children_are_unallocated(environment.body(), slots)
}

fn body_children_are_unallocated(body: &AttachedStyleBody, slots: &SlotSnapshot) -> bool {
    body.members().iter().all(|member| match member {
        AttachedStyleMember::Token(token) => style_token_children_are_unallocated(token, slots),
        AttachedStyleMember::Rule(rule) => rule_children_are_unallocated(rule, slots),
        AttachedStyleMember::Environment(environment) => {
            environment_children_are_unallocated(environment, slots)
        }
        AttachedStyleMember::Error { .. } => true,
    })
}

fn expected_item_state(
    attached: &AttachedStyleDeclaration,
    style: &HirStyleItem,
    item: &HirItem,
    slots: &SlotSnapshot,
) -> crate::item::HirItemPoisonState {
    item_state(
        prefix_issue(attached.prefix(), item.prefix(), slots)
            .or_else(|| style_id_issue(attached.id()))
            .or_else(|| {
                attached
                    .has_header_trailing_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                matches!(attached.body(), AttachedStyleBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| {
                style_has_member_or_child_recovery(style, slots)
                    .then_some(HirItemIssue::InvalidMember)
            })
            .or_else(|| (!attached.body().is_closed()).then_some(HirItemIssue::Recovery)),
    )
}

fn style_id_issue(id: &AttachedStyleId) -> Option<HirItemIssue> {
    match id {
        AttachedStyleId::Missing { .. } => Some(HirItemIssue::MissingId),
        AttachedStyleId::Invalid { .. } => Some(HirItemIssue::MalformedHeader),
        AttachedStyleId::Authored {
            canonical_style_family,
            ..
        } if !canonical_style_family => Some(HirItemIssue::MalformedHeader),
        AttachedStyleId::Authored { reference, .. } if reference.value().is_err() => {
            Some(HirItemIssue::Recovery)
        }
        AttachedStyleId::Authored { .. } => None,
    }
}

fn style_has_member_or_child_recovery(style: &HirStyleItem, slots: &SlotSnapshot) -> bool {
    style.tokens().iter().any(|token| {
        token.id().is_recovered()
            || token.recovery_issue().is_some()
            || token
                .value_type()
                .is_some_and(|owner| slot_is_poisoned(slots, owner))
            || slot_is_poisoned(slots, token.value())
    }) || body_has_recovery(style.body(), slots)
}

fn body_has_recovery(body: &[HirStyleBodyItem], slots: &SlotSnapshot) -> bool {
    body.iter().any(|item| match item {
        HirStyleBodyItem::Recovered(_) => true,
        HirStyleBodyItem::Rule(rule) => {
            rule.selector().recovery_issue().is_some()
                || rule.declarations().iter().any(|declaration| {
                    declaration.property().has_recovery()
                        || declaration.operation().has_recovery()
                        || slot_is_poisoned(slots, declaration.value())
                })
        }
        HirStyleBodyItem::Environment(environment) => {
            environment.clauses().iter().any(|clause| {
                clause.field().has_recovery()
                    || clause.comparison().has_recovery()
                    || slot_is_poisoned(slots, clause.value())
            }) || body_has_recovery(environment.body(), slots)
        }
    })
}
