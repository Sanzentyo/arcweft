//! Final Metric schema lowering into typed retained members and expression owners.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedMetricBucketsValue, AttachedMetricDeclaration, AttachedMetricEntry,
    AttachedMetricKind, AttachedMetricLabel, AttachedMetricLabelsBody, AttachedMetricUnitValue,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::expr::HirExprKind;
use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirMetricAssignmentState, HirMetricBucketsMember,
    HirMetricBucketsValue, HirMetricDeclaration, HirMetricKind, HirMetricKindIssue,
    HirMetricLabelMember, HirMetricUnitMember, HirMetricUnitValue,
};
use crate::leaf::{HirLiteral, HirStringLiteral};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::super::{StagedHirModuleTransaction, require_limit};
use super::super::{LoweredItemProjection, item_state, project_required_name};
use super::{project_retained_header, retained_header_issue};

impl StagedHirModuleTransaction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "Metric lowering atomically projects its closed member and expression inventory"
    )]
    pub(in crate::final_lowering::item_lowering) fn lower_metric_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::MetricDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_metric_inventory(&attached)?;

        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let prefix_issue = prefix.issue;
        let header = project_retained_header(attached.header(), DeclarationIdentityFamily::Metric)?;
        let kind = project_metric_kind(attached.kind());
        let value_type = self.lower_attached_type(attached.value_type(), scope)?;
        let value_type_poisoned = self.staged_type_is_poisoned(value_type)?;

        let mut retained_members = Vec::new();
        let mut member_ids = Vec::new();
        let mut unit = None;
        let mut labels = Vec::new();
        let mut buckets = None;
        let mut first_body_issue = None;

        for (entry_position, entry) in attached.body().entries().iter().enumerate() {
            let expected_ordinal = u16::try_from(entry_position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if entry.source_ordinal() != expected_ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let entry_has_issue = match entry {
                AttachedMetricEntry::Unit(member) => {
                    let id = next_member_id(owner, retained_members.len())?;
                    let (value, child_recovery) =
                        self.lower_metric_unit_value(member.value(), scope)?;
                    let issue = if member.state().is_duplicate() {
                        Some(HirDeclarationMemberIssue::Duplicate)
                    } else if member.assignment().is_missing() {
                        Some(HirDeclarationMemberIssue::MissingAssignment)
                    } else if matches!(value, HirMetricUnitValue::Missing) {
                        Some(HirDeclarationMemberIssue::MissingInitializer)
                    } else if child_recovery {
                        Some(HirDeclarationMemberIssue::RecoveredChild)
                    } else {
                        None
                    };
                    let state = member_state(issue);
                    retained_members.push(
                        HirDeclarationMember::try_new(
                            id,
                            HirDeclarationMemberKind::MetricUnit(HirMetricUnitMember::new(
                                assignment_state(member.assignment().is_missing()),
                                value,
                                member.state().is_duplicate(),
                            )),
                            state,
                        )
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    );
                    member_ids.push(id);
                    unit.get_or_insert(id);
                    member.has_recovery() || state.is_poisoned()
                }
                AttachedMetricEntry::Labels(member) => {
                    let mut issue = member.state().has_recovery() || member.body().has_recovery();
                    if let AttachedMetricLabelsBody::Braced {
                        labels: attached_labels,
                        ..
                    } = member.body()
                    {
                        for (label_position, label) in attached_labels.iter().enumerate() {
                            let expected_label_ordinal = u16::try_from(label_position)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                            if label.source_ordinal() != expected_label_ordinal {
                                return Err(HirInvariantFailure::InvalidArenaCommit.into());
                            }
                            let id = next_member_id(owner, retained_members.len())?;
                            let retained = self.lower_metric_label(id, scope, label)?;
                            issue |= retained.is_poisoned();
                            retained_members.push(retained);
                            member_ids.push(id);
                            labels.push(id);
                        }
                    }
                    issue
                }
                AttachedMetricEntry::Buckets(member) => {
                    let id = next_member_id(owner, retained_members.len())?;
                    let (value, child_recovery) =
                        self.lower_metric_buckets_value(member.value(), scope)?;
                    let issue = if member.state().is_duplicate() {
                        Some(HirDeclarationMemberIssue::Duplicate)
                    } else if member.assignment().is_missing() {
                        Some(HirDeclarationMemberIssue::MissingAssignment)
                    } else if matches!(value, HirMetricBucketsValue::Missing) {
                        Some(HirDeclarationMemberIssue::MissingInitializer)
                    } else if child_recovery {
                        Some(HirDeclarationMemberIssue::RecoveredChild)
                    } else {
                        None
                    };
                    let state = member_state(issue);
                    retained_members.push(
                        HirDeclarationMember::try_new(
                            id,
                            HirDeclarationMemberKind::MetricBuckets(HirMetricBucketsMember::new(
                                assignment_state(member.assignment().is_missing()),
                                value,
                                member.state().is_duplicate(),
                            )),
                            state,
                        )
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    );
                    member_ids.push(id);
                    buckets.get_or_insert(id);
                    member.has_recovery() || state.is_poisoned()
                }
                AttachedMetricEntry::Recovery { .. } => true,
            };
            if entry_has_issue {
                first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
            }
        }

        let members = if retained_members.is_empty() {
            None
        } else {
            Some(
                HirDeclarationMemberArena::try_new(
                    owner,
                    HirItemFamily::Metric,
                    retained_members.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            )
        };
        let type_issue = value_type_poisoned.then_some(
            if attached.value_type().syntax().kind() == SyntaxKind::MissingType {
                HirItemIssue::MissingType
            } else {
                HirItemIssue::Recovery
            },
        );
        let issue = prefix_issue
            .or_else(|| retained_header_issue(attached.header()))
            .or_else(|| {
                attached
                    .kind()
                    .has_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or(type_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(first_body_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                (!attached.declaration_recoveries().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let declaration = HirMetricDeclaration::try_new(
            owner,
            header,
            kind,
            value_type,
            unit,
            labels.into_boxed_slice(),
            buckets,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Metric(declaration),
            member_ids.into_boxed_slice(),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection { item, members })
    }

    fn lower_metric_label(
        &mut self,
        id: HirDeclarationMemberId,
        scope: ScopeId,
        label: &AttachedMetricLabel,
    ) -> Result<HirDeclarationMember, HirLowerFailure> {
        let name = project_required_name(label.name())?;
        let ty = self.lower_attached_type(label.ty(), scope)?;
        let type_poisoned = self.staged_type_is_poisoned(ty)?;
        let issue = if label.is_duplicate() {
            Some(HirDeclarationMemberIssue::Duplicate)
        } else if name.issue.is_some() || label.colon().is_missing() || type_poisoned {
            Some(HirDeclarationMemberIssue::RecoveredChild)
        } else {
            None
        };
        HirDeclarationMember::try_new(
            id,
            HirDeclarationMemberKind::MetricLabel(HirMetricLabelMember::new(
                name.value,
                ty,
                label.is_duplicate(),
            )),
            member_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
    }

    fn lower_metric_unit_value(
        &mut self,
        attached: &AttachedMetricUnitValue,
        scope: ScopeId,
    ) -> Result<(HirMetricUnitValue, bool), HirLowerFailure> {
        let (expression, expected_value, force_recovery) = match attached {
            AttachedMetricUnitValue::Decoded { expression, value } => {
                (expression, Some(value.as_ref()), false)
            }
            AttachedMetricUnitValue::RecoveredString(expression) => (expression, None, true),
            AttachedMetricUnitValue::NonString(expression) => {
                let owner = self.lower_attached_expression(expression, scope)?;
                return Ok((HirMetricUnitValue::NonString(owner), true));
            }
            AttachedMetricUnitValue::Missing(_) => {
                return Ok((HirMetricUnitValue::Missing, true));
            }
        };
        let owner = self.lower_attached_expression(expression, scope)?;
        let poisoned = self.staged_expression_is_poisoned(owner)?;
        let literal = {
            let (slots, arenas) = self.storage_mut();
            let retained = arenas.expressions().resolve_staged(slots, owner)?;
            match retained.kind() {
                HirExprKind::Literal(HirLiteral::String(literal)) => literal.clone(),
                _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
            }
        };
        match (&literal, expected_value) {
            (HirStringLiteral::Value(actual), Some(expected)) if actual.as_ref() == expected => {}
            (HirStringLiteral::Invalid(_), None) => {}
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        }
        Ok((
            HirMetricUnitValue::String(literal),
            force_recovery || poisoned,
        ))
    }

    fn lower_metric_buckets_value(
        &mut self,
        attached: &AttachedMetricBucketsValue,
        scope: ScopeId,
    ) -> Result<(HirMetricBucketsValue, bool), HirLowerFailure> {
        match attached {
            AttachedMetricBucketsValue::Missing(_) => Ok((HirMetricBucketsValue::Missing, true)),
            AttachedMetricBucketsValue::NonSequence(expression) => {
                let owner = self.lower_attached_expression(expression, scope)?;
                Ok((HirMetricBucketsValue::NonSequence(owner), true))
            }
            AttachedMetricBucketsValue::Sequence(expression) => {
                let owner = self.lower_attached_expression(expression, scope)?;
                let poisoned = self.staged_expression_is_poisoned(owner)?;
                let elements = {
                    let (slots, arenas) = self.storage_mut();
                    let retained = arenas.expressions().resolve_staged(slots, owner)?;
                    match retained.kind() {
                        HirExprKind::BracketSequence(sequence) => {
                            sequence.elements().to_vec().into_boxed_slice()
                        }
                        _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
                    }
                };
                let recovered = elements.is_empty() || poisoned;
                Ok((HirMetricBucketsValue::Sequence(elements), recovered))
            }
        }
    }
}

const fn project_metric_kind(kind: &AttachedMetricKind) -> HirMetricKind {
    match kind {
        AttachedMetricKind::Counter(_) => HirMetricKind::Counter,
        AttachedMetricKind::Gauge(_) => HirMetricKind::Gauge,
        AttachedMetricKind::Histogram(_) => HirMetricKind::Histogram,
        AttachedMetricKind::Missing(_) => HirMetricKind::Recovered(HirMetricKindIssue::Missing),
        AttachedMetricKind::Unknown(_) => HirMetricKind::Recovered(HirMetricKindIssue::Invalid),
    }
}

const fn assignment_state(missing: bool) -> HirMetricAssignmentState {
    if missing {
        HirMetricAssignmentState::Missing
    } else {
        HirMetricAssignmentState::Present
    }
}

const fn member_state(issue: Option<HirDeclarationMemberIssue>) -> HirDeclarationMemberPoisonState {
    match issue {
        Some(issue) => HirDeclarationMemberPoisonState::Poisoned(issue),
        None => HirDeclarationMemberPoisonState::Clean,
    }
}

fn next_member_id(
    owner: ItemId,
    position: usize,
) -> Result<HirDeclarationMemberId, HirLowerFailure> {
    let ordinal = u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
    Ok(HirDeclarationMemberId::new(owner, ordinal))
}

fn preflight_metric_inventory(attached: &AttachedMetricDeclaration) -> Result<(), HirLowerFailure> {
    let mut declaration_members = attached.body().entries().len();
    for entry in attached.body().entries() {
        if let AttachedMetricEntry::Labels(labels) = entry {
            declaration_members = declaration_members
                .checked_add(labels.body().labels().len())
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        }
    }
    require_limit(HirLimit::DeclarationMembers, declaration_members)
}
