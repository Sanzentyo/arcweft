//! Typed applicability checks for item-owned Style source roles.

use crate::identity::ItemId;
use crate::item::{HirStyleBodyItem, HirStyleEnvironment, HirStyleItem, HirStyleRule};

use super::super::super::{
    HirItemSourceRole, HirSourceQueryError, HirStyleBodyPath, HirStyleBodySourcePart,
    HirStyleSourceRole,
};

impl HirStyleItem {
    /// Validates a Style component role against the final semantic payload
    /// before the immutable source-component table is consulted.
    pub(crate) fn validate_source_role(
        &self,
        owner: ItemId,
        role: &HirStyleSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        let applicable = match role {
            HirStyleSourceRole::ItemId => true,
            HirStyleSourceRole::Token { ordinal, .. } => {
                self.tokens().get(*ordinal as usize).is_some()
            }
            HirStyleSourceRole::Body { path, part } => self
                .body_at(path)
                .is_some_and(|body| style_body_role_applies(body, *part)),
        };
        if applicable {
            Ok(())
        } else {
            Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::Style(role.clone()),
            })
        }
    }

    fn body_at(&self, path: &HirStyleBodyPath) -> Option<&[HirStyleBodyItem]> {
        let mut body = self.body();
        for ordinal in path.ordinals() {
            let HirStyleBodyItem::Environment(environment) = body.get(*ordinal as usize)? else {
                return None;
            };
            body = environment.body();
        }
        Some(body)
    }
}

fn style_body_role_applies(body: &[HirStyleBodyItem], part: HirStyleBodySourcePart) -> bool {
    match part {
        HirStyleBodySourcePart::BodyWhole => true,
        HirStyleBodySourcePart::RuleSelector { rule } => {
            matches!(body.get(rule as usize), Some(HirStyleBodyItem::Rule(_)))
        }
        HirStyleBodySourcePart::RuleSequence { rule, sequence }
        | HirStyleBodySourcePart::RuleElement { rule, sequence }
        | HirStyleBodySourcePart::RulePart { rule, sequence } => body
            .get(rule as usize)
            .and_then(as_rule)
            .and_then(|rule| rule.selector().sequences().get(sequence as usize))
            .is_some(),
        HirStyleBodySourcePart::RulePredicate {
            rule,
            sequence,
            predicate,
        } => body
            .get(rule as usize)
            .and_then(as_rule)
            .and_then(|rule| rule.selector().sequences().get(sequence as usize))
            .and_then(|sequence| sequence.predicates().get(predicate as usize))
            .is_some(),
        HirStyleBodySourcePart::DeclarationWhole { rule, declaration }
        | HirStyleBodySourcePart::DeclarationProperty { rule, declaration }
        | HirStyleBodySourcePart::DeclarationAssignment { rule, declaration } => body
            .get(rule as usize)
            .and_then(as_rule)
            .and_then(|rule| rule.declarations().get(declaration as usize))
            .is_some(),
        HirStyleBodySourcePart::EnvironmentWhole { environment }
        | HirStyleBodySourcePart::EnvironmentCondition { environment }
        | HirStyleBodySourcePart::EnvironmentBody { environment } => matches!(
            body.get(environment as usize),
            Some(HirStyleBodyItem::Environment(_))
        ),
        HirStyleBodySourcePart::ClauseWhole {
            environment,
            clause,
        }
        | HirStyleBodySourcePart::ClauseField {
            environment,
            clause,
        }
        | HirStyleBodySourcePart::ClauseComparison {
            environment,
            clause,
        } => body
            .get(environment as usize)
            .and_then(as_environment)
            .and_then(|environment| environment.clauses().get(clause as usize))
            .is_some(),
    }
}

fn as_rule(item: &HirStyleBodyItem) -> Option<&HirStyleRule> {
    match item {
        HirStyleBodyItem::Rule(rule) => Some(rule),
        HirStyleBodyItem::Environment(_) | HirStyleBodyItem::Recovered(_) => None,
    }
}

fn as_environment(item: &HirStyleBodyItem) -> Option<&HirStyleEnvironment> {
    match item {
        HirStyleBodyItem::Environment(environment) => Some(environment),
        HirStyleBodyItem::Rule(_) | HirStyleBodyItem::Recovered(_) => None,
    }
}
