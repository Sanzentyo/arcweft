//! Native Style lowering into the final item, expression, and type owners.

use arcweft_lang_syntax::attachment::{
    AstNode, AttachedStyleAssignmentState, AttachedStyleBody, AttachedStyleDeclaration,
    AttachedStyleEnvironment, AttachedStyleEnvironmentComparison, AttachedStyleEnvironmentField,
    AttachedStyleExpression, AttachedStyleId, AttachedStyleMember, AttachedStyleName,
    AttachedStyleProperty, AttachedStyleRule, AttachedStyleSelector, AttachedStyleSelectorSequence,
    AttachedStyleToken, StyleEnvironmentComparisonKind, StyleEnvironmentConditionIssue,
    StyleEnvironmentFieldKind, StylePropertyOperation, StyleSelectorRelation, StyleSyntaxNameIssue,
};

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirItem, HirItemIssue, HirItemKind, HirStyleAssignOperation, HirStyleAssignOperationIssue,
    HirStyleBodyIssue, HirStyleBodyItem, HirStyleCombinator, HirStyleDeclaration,
    HirStyleEnvironment, HirStyleEnvironmentClause, HirStyleEnvironmentComparison,
    HirStyleEnvironmentComparisonIssue, HirStyleEnvironmentField, HirStyleEnvironmentFieldIssue,
    HirStyleItem, HirStyleName, HirStyleNameIssue, HirStyleRule, HirStyleSelector,
    HirStyleSelectorIssue, HirStyleSelectorSequence, HirStyleToken, HirStyleTokenIssue,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::id_ref_projection::id_ref;
use super::super::{StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state};

struct LoweredStyleBody {
    items: Box<[HirStyleBodyItem]>,
    has_recovery: bool,
}

impl StagedHirModuleTransaction<'_> {
    pub(in crate::final_lowering::item_lowering) fn lower_style_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::StyleItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        preflight_style(&attached)?;

        let retained_id = id_ref(
            attached
                .id()
                .reference()
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?,
        )?;
        let mut tokens = Vec::new();
        let body = self.lower_style_body(attached.body(), scope, true, &mut tokens)?;
        let issue = prefix
            .issue
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
            .or_else(|| body.has_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| {
                matches!(
                    attached.body(),
                    AttachedStyleBody::Braced { close, .. } if close.range().is_empty()
                )
                .then_some(HirItemIssue::Recovery)
            });
        let module = self.snapshot_id().module();
        let style =
            HirStyleItem::try_new(module, retained_id, tokens.into_boxed_slice(), body.items)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components
            .stage_attached_style(self.request.source(), owner, &attached)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Style(style),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    fn lower_style_body(
        &mut self,
        attached: &AttachedStyleBody,
        scope: ScopeId,
        allow_tokens: bool,
        tokens: &mut Vec<HirStyleToken>,
    ) -> Result<LoweredStyleBody, HirLowerFailure> {
        let mut items = Vec::new();
        let mut has_recovery = false;
        for member in attached.members() {
            match member {
                AttachedStyleMember::Token(token) if allow_tokens => {
                    let (token, recovered) = self.lower_style_token(token, scope)?;
                    tokens.push(token);
                    has_recovery |= recovered;
                }
                AttachedStyleMember::Token(_) => {
                    items.push(HirStyleBodyItem::Recovered(HirStyleBodyIssue::Unexpected));
                    has_recovery = true;
                }
                AttachedStyleMember::Rule(rule) => {
                    if style_rule_whole_recovery(rule) {
                        items.push(HirStyleBodyItem::Recovered(HirStyleBodyIssue::Malformed));
                        has_recovery = true;
                    } else {
                        let (rule, recovered) = self.lower_style_rule(rule, scope)?;
                        items.push(HirStyleBodyItem::Rule(rule));
                        has_recovery |= recovered;
                    }
                }
                AttachedStyleMember::Environment(environment) => {
                    if let Some(issue) = style_environment_whole_recovery(environment) {
                        items.push(HirStyleBodyItem::Recovered(issue));
                        has_recovery = true;
                    } else {
                        let (environment, recovered) =
                            self.lower_style_environment(environment, scope, tokens)?;
                        items.push(HirStyleBodyItem::Environment(environment));
                        has_recovery |= recovered;
                    }
                }
                AttachedStyleMember::Error { .. } => {
                    items.push(HirStyleBodyItem::Recovered(HirStyleBodyIssue::Malformed));
                    has_recovery = true;
                }
            }
        }
        Ok(LoweredStyleBody {
            items: items.into_boxed_slice(),
            has_recovery,
        })
    }

    fn lower_style_token(
        &mut self,
        attached: &AttachedStyleToken,
        scope: ScopeId,
    ) -> Result<(HirStyleToken, bool), HirLowerFailure> {
        let value_type = attached
            .type_annotation()
            .map(|annotation| self.lower_attached_type(annotation.value(), scope))
            .transpose()?;
        let value = self.lower_style_expression(attached.value(), scope)?;
        let child_recovery = value_type
            .map(|ty| self.staged_type_is_poisoned(ty))
            .transpose()?
            .unwrap_or(false)
            || self.staged_expression_is_poisoned(value)?;
        let recovery = match attached.assignment().state() {
            AttachedStyleAssignmentState::Authored => None,
            AttachedStyleAssignmentState::Missing => Some(HirStyleTokenIssue::MissingAssignment),
            AttachedStyleAssignmentState::Unsupported => {
                Some(HirStyleTokenIssue::MalformedAssignment)
            }
        };
        let id = id_ref(attached.id())?;
        let retained =
            HirStyleToken::try_new(self.snapshot_id().module(), id, value_type, value, recovery)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((retained, attached.has_recovery() || child_recovery))
    }

    fn lower_style_rule(
        &mut self,
        attached: &AttachedStyleRule,
        scope: ScopeId,
    ) -> Result<(HirStyleRule, bool), HirLowerFailure> {
        let selector = project_style_selector(attached.selector());
        let mut has_recovery = selector.has_recovery();
        let mut declarations = Vec::with_capacity(attached.body().declarations().len());
        for declaration in attached.body().declarations() {
            let value = self.lower_style_expression(declaration.value(), scope)?;
            let child_recovery = self.staged_expression_is_poisoned(value)?;
            let property = project_style_name(declaration.name());
            let operation = project_style_operation(declaration);
            has_recovery |= declaration.has_recovery()
                || child_recovery
                || property.has_recovery()
                || operation.has_recovery();
            declarations.push(
                HirStyleDeclaration::try_new(
                    self.snapshot_id().module(),
                    property,
                    value,
                    operation,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        let retained = HirStyleRule::try_new(
            self.snapshot_id().module(),
            selector,
            declarations.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((retained, attached.has_recovery() || has_recovery))
    }

    fn lower_style_environment(
        &mut self,
        attached: &AttachedStyleEnvironment,
        scope: ScopeId,
        tokens: &mut Vec<HirStyleToken>,
    ) -> Result<(HirStyleEnvironment, bool), HirLowerFailure> {
        let mut has_recovery = false;
        let mut clauses = Vec::with_capacity(attached.condition().clauses().len());
        for clause in attached.condition().clauses() {
            let value = self.lower_style_expression(clause.value(), scope)?;
            let child_recovery = self.staged_expression_is_poisoned(value)?;
            let field = project_style_environment_field(clause.field());
            let comparison = project_style_environment_comparison(clause.comparison());
            has_recovery |= clause.has_recovery()
                || child_recovery
                || field.has_recovery()
                || comparison.has_recovery();
            clauses.push(
                HirStyleEnvironmentClause::try_new(
                    self.snapshot_id().module(),
                    field,
                    comparison,
                    value,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        let body = self.lower_style_body(attached.body(), scope, false, tokens)?;
        has_recovery |= body.has_recovery;
        let retained = HirStyleEnvironment::try_new(
            self.snapshot_id().module(),
            clauses.into_boxed_slice(),
            body.items,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((retained, has_recovery))
    }

    fn lower_style_expression(
        &mut self,
        attached: &AttachedStyleExpression,
        scope: ScopeId,
    ) -> Result<crate::identity::ExprId, HirLowerFailure> {
        match attached {
            AttachedStyleExpression::Authored(attached) => {
                self.lower_attached_expression(attached, scope)
            }
            AttachedStyleExpression::Missing(attached) => {
                self.lower_source_missing_expression(attached, scope)
            }
        }
    }
}

fn preflight_style(attached: &AttachedStyleDeclaration) -> Result<(), HirLowerFailure> {
    let mut member_count = 0_usize;
    let mut bodies = vec![(attached.body(), 0_usize)];
    while let Some((body, environment_depth)) = bodies.pop() {
        for (position, member) in body.members().iter().enumerate() {
            let expected =
                u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if member.source_ordinal() != expected {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            charge_style_members(&mut member_count, 1)?;
            match member {
                AttachedStyleMember::Token(token) => {
                    require_style_name(token.name())?;
                }
                AttachedStyleMember::Rule(rule) => {
                    let selector = rule.selector();
                    charge_style_members(&mut member_count, selector.sequences().len())?;
                    for (sequence_position, sequence) in selector.sequences().iter().enumerate() {
                        let expected = u32::try_from(sequence_position)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        if sequence.source_ordinal() != expected {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                        if let Some(element) = sequence.element() {
                            require_style_name(element)?;
                        }
                        if let Some(part) = sequence.part() {
                            require_style_name(part.name())?;
                        }
                        charge_style_members(&mut member_count, sequence.predicates().len())?;
                        for (predicate_position, predicate) in
                            sequence.predicates().iter().enumerate()
                        {
                            let expected = u16::try_from(predicate_position)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                            if predicate.source_ordinal() != expected {
                                return Err(HirInvariantFailure::InvalidArenaCommit.into());
                            }
                            require_style_name(predicate.name())?;
                        }
                    }
                    charge_style_members(&mut member_count, rule.body().declarations().len())?;
                    for (declaration_position, declaration) in
                        rule.body().declarations().iter().enumerate()
                    {
                        let expected = u32::try_from(declaration_position)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        if declaration.source_ordinal() != expected {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                        require_style_name(declaration.name())?;
                    }
                }
                AttachedStyleMember::Environment(environment) => {
                    let nested_depth = environment_depth
                        .checked_add(1)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    preflight_style_nesting_depth(nested_depth)?;
                    require_style_name(environment.intrinsic())?;
                    charge_style_members(
                        &mut member_count,
                        environment.condition().clauses().len(),
                    )?;
                    for (clause_position, clause) in
                        environment.condition().clauses().iter().enumerate()
                    {
                        let expected = u16::try_from(clause_position)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        if clause.source_ordinal() != expected {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                        require_style_name(clause.field().name())?;
                    }
                    for (recovery_position, recovery) in
                        environment.condition().recoveries().iter().enumerate()
                    {
                        let expected = u32::try_from(recovery_position)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        if recovery.source_ordinal() != expected {
                            return Err(HirInvariantFailure::InvalidArenaCommit.into());
                        }
                    }
                    bodies.push((environment.body(), nested_depth));
                }
                AttachedStyleMember::Error { .. } => {}
            }
        }
    }
    Ok(())
}

fn charge_style_members(observed: &mut usize, charge: usize) -> Result<(), HirLowerFailure> {
    *observed = observed
        .checked_add(charge)
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    preflight_style_member_count(*observed)
}

pub(in crate::final_lowering::item_lowering) fn preflight_style_member_count(
    observed: usize,
) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, observed)
}

pub(in crate::final_lowering::item_lowering) fn preflight_style_nesting_depth(
    observed: usize,
) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::StyleNestingDepth, observed)
}

fn require_style_name(name: &AttachedStyleName) -> Result<(), HirLowerFailure> {
    let range = name.syntax().source_span().range();
    let observed = range
        .end()
        .checked_sub(range.start())
        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
    require_limit(HirLimit::NameBytes, observed)
}

fn style_id_issue(id: &AttachedStyleId) -> Option<HirItemIssue> {
    match id {
        AttachedStyleId::Missing { .. } => Some(HirItemIssue::MissingId),
        AttachedStyleId::Invalid { .. } => Some(HirItemIssue::MalformedHeader),
        AttachedStyleId::Authored {
            reference,
            canonical_style_family: false,
            ..
        } => {
            let _ = reference;
            Some(HirItemIssue::MalformedHeader)
        }
        AttachedStyleId::Authored { reference, .. } if reference.value().is_err() => {
            Some(HirItemIssue::Recovery)
        }
        AttachedStyleId::Authored { .. } => None,
    }
}

fn style_rule_whole_recovery(rule: &AttachedStyleRule) -> bool {
    rule.body().open_delimiter().range().is_empty()
        || rule.body().close_delimiter().range().is_empty()
}

fn style_environment_whole_recovery(
    environment: &AttachedStyleEnvironment,
) -> Option<HirStyleBodyIssue> {
    match environment.intrinsic().value() {
        Ok(value) if value.as_str() == "environment" => {}
        Err(StyleSyntaxNameIssue::Missing) => return Some(HirStyleBodyIssue::Missing),
        Ok(_) | Err(_) => return Some(HirStyleBodyIssue::Malformed),
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

fn project_style_selector(attached: &AttachedStyleSelector) -> HirStyleSelector {
    let sequences = attached
        .sequences()
        .iter()
        .map(|sequence| {
            HirStyleSelectorSequence::new(
                sequence.relation().map(|relation| match relation.value() {
                    StyleSelectorRelation::Descendant => HirStyleCombinator::Descendant,
                    StyleSelectorRelation::Child => HirStyleCombinator::Child,
                }),
                sequence.element().map(project_style_name),
                sequence.part().map(|part| project_style_name(part.name())),
                sequence
                    .predicates()
                    .iter()
                    .map(|predicate| project_style_name(predicate.name()))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let issue = style_selector_issue(attached, &sequences);
    match issue {
        Some(issue) => HirStyleSelector::recovered(sequences, issue),
        None => HirStyleSelector::try_new(sequences)
            .expect("preflighted clean Style selector satisfies final HIR invariants"),
    }
}

fn style_selector_issue(
    attached: &AttachedStyleSelector,
    sequences: &[HirStyleSelectorSequence],
) -> Option<HirStyleSelectorIssue> {
    if attached.missing().is_some() || sequences.is_empty() {
        return Some(HirStyleSelectorIssue::MissingSequence);
    }
    if !attached.recoveries().is_empty()
        || sequences
            .first()
            .is_some_and(|first| first.relation_to_previous().is_some())
        || sequences
            .iter()
            .skip(1)
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
    if attached
        .sequences()
        .iter()
        .any(AttachedStyleSelectorSequence::has_recovery)
        || sequences.iter().any(|sequence| {
            sequence
                .element()
                .into_iter()
                .chain(sequence.part())
                .chain(sequence.predicates())
                .any(HirStyleName::has_recovery)
        })
    {
        return Some(HirStyleSelectorIssue::InvalidComponent);
    }
    None
}

fn project_style_name(attached: &AttachedStyleName) -> HirStyleName {
    match attached.value() {
        Ok(value) => HirStyleName::try_new(value.as_str().into())
            .unwrap_or_else(|_| HirStyleName::recovered(HirStyleNameIssue::Invalid)),
        Err(StyleSyntaxNameIssue::Missing) => HirStyleName::recovered(HirStyleNameIssue::Missing),
        Err(_) => HirStyleName::recovered(HirStyleNameIssue::Invalid),
    }
}

fn project_style_operation(attached: &AttachedStyleProperty) -> HirStyleAssignOperation {
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

fn project_style_environment_field(
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

fn project_style_environment_comparison(
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
