//! Source-component validation for record expression projections.

use std::collections::HashSet;

use super::super::{ExpressionComponentRole, ExpressionRecordFieldPart, SyntaxRecordField};
use super::PendingExpressionComponent;

pub(super) fn components_validate(
    fields: &[SyntaxRecordField],
    has_path: bool,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    if roles.contains(&ExpressionComponentRole::RecordPath) != has_path {
        return false;
    }
    let expected_fields =
        fields
            .iter()
            .enumerate()
            .try_fold(usize::from(has_path), |expected, (field, value)| {
                let field = u32::try_from(field).ok()?;
                let (required, required_len) = match value {
                    SyntaxRecordField::Explicit { .. } => (
                        [
                            Some(ExpressionRecordFieldPart::Whole),
                            Some(ExpressionRecordFieldPart::Name),
                            Some(ExpressionRecordFieldPart::Colon),
                            Some(ExpressionRecordFieldPart::Value),
                        ],
                        4,
                    ),
                    SyntaxRecordField::Shorthand { .. } => (
                        [
                            Some(ExpressionRecordFieldPart::Whole),
                            Some(ExpressionRecordFieldPart::Name),
                            None,
                            None,
                        ],
                        2,
                    ),
                };
                required
                    .iter()
                    .flatten()
                    .all(|part| {
                        roles.contains(&ExpressionComponentRole::RecordField { field, part: *part })
                    })
                    .then(|| expected + required_len)
            });
    expected_fields == Some(components.len())
        && components.iter().all(|component| match component.role() {
            ExpressionComponentRole::RecordPath => has_path,
            ExpressionComponentRole::RecordField { field, part } => fields
                .get(usize::try_from(field).unwrap_or(usize::MAX))
                .is_some_and(|value| match value {
                    SyntaxRecordField::Explicit { .. } => matches!(
                        part,
                        ExpressionRecordFieldPart::Whole
                            | ExpressionRecordFieldPart::Name
                            | ExpressionRecordFieldPart::Colon
                            | ExpressionRecordFieldPart::Value
                    ),
                    SyntaxRecordField::Shorthand { .. } => matches!(
                        part,
                        ExpressionRecordFieldPart::Whole | ExpressionRecordFieldPart::Name
                    ),
                }),
            _ => false,
        })
}
