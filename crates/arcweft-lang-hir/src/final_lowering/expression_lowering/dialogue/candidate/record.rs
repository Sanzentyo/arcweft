//! Candidate-only E20/E21 record lowering for an E34 interpretation.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::{AttachedCandidateExpressionChild, AttachedCandidateNode};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionRecordFieldPart, SyntaxRecordField,
};

use crate::expr::{
    HirExpressionRecoveryIssue, HirRecordExpr, HirRecordField, HirRecordFieldIssue,
    HirRecordLiteralExpr, HirRecoveryIssue,
};
use crate::identity::{ExprId, ScopeId};
use crate::leaf::HirNameInvariantError;
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::CaptureAccess;
use crate::source_index::{HirExprSourceRole, HirRecordFieldSourcePart};

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;
use crate::final_lowering::name_projection::{name, name_issue, require_attempted_name_limit};
use crate::final_lowering::path_projection::{TypedPathProjection, project_candidate_path};

#[derive(Clone, Copy)]
struct LoweredCandidateRecordValue {
    expression: ExprId,
    missing: bool,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_candidate_record(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        fields: &[SyntaxRecordField],
    ) -> Result<(HirRecordExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let mut paths = node.children().filter_map(|child| child.path_projection());
        let path = paths
            .next()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if paths.next().is_some() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let TypedPathProjection::Resolved(path) = project_candidate_path(path)? else {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        let (fields, recovery) = self.lower_candidate_record_fields(node, scope, cursor, fields)?;
        Ok((HirRecordExpr::new(path, fields), recovery))
    }

    pub(super) fn lower_candidate_record_literal(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        fields: &[SyntaxRecordField],
    ) -> Result<(HirRecordLiteralExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let (fields, recovery) = self.lower_candidate_record_fields(node, scope, cursor, fields)?;
        Ok((HirRecordLiteralExpr::new(fields), recovery))
    }

    fn lower_candidate_record_fields(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
        source_fields: &[SyntaxRecordField],
    ) -> Result<(Box<[HirRecordField]>, Option<HirRecoveryIssue>), HirLowerFailure> {
        let mut values = BTreeMap::new();
        let mut recovery = None;
        for child in node.semantic_expression_children() {
            let ExpressionComponentRole::RecordField {
                field,
                part: ExpressionRecordFieldPart::Value,
            } = child.component_role()
            else {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            };
            if child.ordinal() != field {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let role = HirExprSourceRole::RecordField {
                field,
                part: HirRecordFieldSourcePart::Value,
            };
            let (expression, missing) = match child {
                AttachedCandidateExpressionChild::Authored { node, .. }
                | AttachedCandidateExpressionChild::Recovered { node, .. } => {
                    (self.lower_candidate_expression(node, scope, cursor)?, false)
                }
                AttachedCandidateExpressionChild::Missing { source, .. } => (
                    self.lower_missing_candidate_expression(scope, cursor, role, &source)?,
                    true,
                ),
            };
            if missing {
                recovery.get_or_insert(HirRecoveryIssue::MissingOperand { role });
            } else if self.staged_expression_is_poisoned(expression)? {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild { role },
                ));
            }
            if values
                .insert(
                    field,
                    LoweredCandidateRecordValue {
                        expression,
                        missing,
                    },
                )
                .is_some()
            {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }

        let mut fields = Vec::with_capacity(source_fields.len());
        let mut names = BTreeSet::new();
        for (field, source) in source_fields.iter().enumerate() {
            let field =
                u32::try_from(field).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            match source {
                SyntaxRecordField::Explicit {
                    name: source_name, ..
                } => {
                    let value = values
                        .remove(&field)
                        .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                    let field_name = match source_name {
                        Ok(source_name) => name(source_name)?,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            recovery
                                .get_or_insert(HirRecoveryIssue::InvalidName(name_issue(issue)));
                            fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingName));
                            continue;
                        }
                    };
                    if !names.insert(field_name.clone()) {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                            HirNameInvariantError::InvalidIdentifier,
                        ));
                        fields.push(HirRecordField::invalid(HirRecordFieldIssue::DuplicateName));
                    } else if value.missing {
                        fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingValue));
                    } else {
                        fields.push(HirRecordField::explicit(field_name, value.expression));
                    }
                }
                SyntaxRecordField::Shorthand { name: source_name } => {
                    let field_name = match source_name {
                        Ok(source_name) => name(source_name)?,
                        Err(issue) => {
                            require_attempted_name_limit(issue)?;
                            recovery
                                .get_or_insert(HirRecoveryIssue::InvalidName(name_issue(issue)));
                            fields.push(HirRecordField::invalid(HirRecordFieldIssue::MissingName));
                            continue;
                        }
                    };
                    if !names.insert(field_name.clone()) {
                        recovery.get_or_insert(HirRecoveryIssue::InvalidName(
                            HirNameInvariantError::InvalidIdentifier,
                        ));
                        fields.push(HirRecordField::invalid(HirRecordFieldIssue::DuplicateName));
                        continue;
                    }
                    let first_use = node
                        .expression_components()
                        .ok_or(HirInvariantFailure::InvalidSourceSpan)?
                        .find(|component| {
                            component.role()
                                == ExpressionComponentRole::RecordField {
                                    field,
                                    part: ExpressionRecordFieldPart::Name,
                                }
                        })
                        .ok_or(HirInvariantFailure::InvalidSourceSpan)?
                        .source_span()
                        .clone();
                    let local = self
                        .visible_local(scope, &field_name, first_use.range().start())?
                        .ok_or(HirInvariantFailure::InvalidLocalTimeline)?;
                    self.record_local_capture(scope, local, first_use, CaptureAccess::Read)?;
                    fields.push(HirRecordField::shorthand(field_name, local));
                }
            }
        }
        if !values.is_empty() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok((fields.into_boxed_slice(), recovery))
    }
}
