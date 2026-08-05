//! Final View lowering into one retained item, callable scope, and member arena.

use arcweft_id::DeclarationIdentityFamily;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCallableParameterKind, AttachedViewFragmentEntry, AttachedViewPartPath,
};
use arcweft_lang_syntax::patterns::PatternSyntaxFamily;

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirParameter, HirParameterKind, HirViewDeclaration,
    HirViewExportMember,
};
use crate::leaf::{HirPathIssue, HirPathRecovery, HirPathRoot, HirPathValue};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::HirPatternBindingPolicy;

use super::super::super::path_projection::{TypedPathProjection, project_attached_path};
use super::super::super::{StagedHirModuleTransaction, require_limit};
use super::super::{LoweredItemProjection, item_state};
use super::{project_retained_header, retained_header_issue};

impl StagedHirModuleTransaction<'_> {
    pub(in crate::final_lowering::item_lowering) fn lower_view_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::ViewDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let export_count = attached.exports().count();
        preflight_view_exports(export_count)?;

        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let prefix_issue = prefix.issue;
        let header = project_retained_header(attached.header(), DeclarationIdentityFamily::View)?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;

        let mut parameters = Vec::with_capacity(attached.parameter_group().parameters().len());
        let mut callable_locals = Vec::new();
        let mut parameter_issue = None;
        for (position, parameter) in attached.parameter_group().parameters().iter().enumerate() {
            let expected =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if parameter.source_ordinal() != expected
                || parameter.group_ordinal() != 0
                || parameter.parameter_ordinal() != expected
            {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }

            let pattern = self.lower_attached_pattern_binding(
                parameter.pattern(),
                callable_scope,
                HirPatternBindingPolicy::CallableParameter,
            )?;
            let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
            let type_poisoned = self.staged_type_is_poisoned(ty)?;
            let default = parameter
                .default()
                .map(|default| self.lower_attached_expression(default.value(), callable_scope))
                .transpose()?;
            let default_poisoned = match default {
                Some(default) => self.staged_expression_is_poisoned(default)?,
                None => false,
            };
            let invalid_shape = !matches!(parameter.kind(), AttachedCallableParameterKind::Fixed)
                || parameter.pattern().family() != PatternSyntaxFamily::Binding;
            parameter_issue = parameter_issue
                .or_else(|| invalid_shape.then_some(HirItemIssue::InvalidMember))
                .or_else(|| {
                    (type_poisoned
                        && parameter.ty().syntax().kind()
                            == arcweft_lang_syntax::grammar::SyntaxKind::MissingType)
                        .then_some(HirItemIssue::MissingType)
                })
                .or_else(|| {
                    (parameter.has_recovery()
                        || pattern.poisoned
                        || type_poisoned
                        || default_poisoned)
                        .then_some(HirItemIssue::InvalidMember)
                });
            callable_locals.extend_from_slice(&pattern.locals);
            parameters.push(
                HirParameter::try_new(
                    pattern.owner,
                    ty,
                    HirParameterKind::Fixed,
                    default,
                    pattern.locals,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        require_limit(HirLimit::LocalsPerScope, callable_locals.len())?;
        self.close_scope_members(callable_scope, callable_locals.into_boxed_slice())?;

        let mut retained_members = Vec::with_capacity(export_count);
        let mut member_ids = Vec::with_capacity(export_count);
        let mut member_issue = None;
        for (position, export) in attached.exports().enumerate() {
            let ordinal =
                u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if u32::from(export.source_ordinal()) != ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let id = HirDeclarationMemberId::new(owner, ordinal);
            let local_part = project_view_part(export.local_part())?;
            let public_part = project_view_part(export.public_part())?;
            let payload = HirViewExportMember::new(local_part, public_part);
            let state = if export.has_recovery() || payload.has_recovery() {
                member_issue.get_or_insert(HirItemIssue::InvalidMember);
                HirDeclarationMemberPoisonState::Poisoned(HirDeclarationMemberIssue::RecoveredChild)
            } else {
                HirDeclarationMemberPoisonState::Clean
            };
            retained_members.push(
                HirDeclarationMember::try_new(
                    id,
                    HirDeclarationMemberKind::ViewExport(payload),
                    state,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
            member_ids.push(id);
        }

        let members = if retained_members.is_empty() {
            None
        } else {
            Some(
                HirDeclarationMemberArena::try_new(
                    owner,
                    HirItemFamily::View,
                    retained_members.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            )
        };

        let mut values = Vec::new();
        let mut value_issue = None;
        if let Some(fragment) = attached.body().fragment() {
            for entry in fragment.entries() {
                let AttachedViewFragmentEntry::Value(value) = entry else {
                    continue;
                };
                let value = self.lower_attached_expression(value, callable_scope)?;
                if self.staged_expression_is_poisoned(value)? {
                    value_issue.get_or_insert(HirItemIssue::InvalidMember);
                }
                values.push(value);
            }
        }

        let issue = prefix_issue
            .or_else(|| retained_header_issue(attached.header()))
            .or_else(|| {
                attached
                    .header_recovery()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                (attached.parameter_group().open_state().is_missing()
                    || attached.parameter_group().close_state().is_missing())
                .then_some(HirItemIssue::Recovery)
            })
            .or(parameter_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or(value_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            });
        let declaration = HirViewDeclaration::try_new(
            owner,
            header,
            callable_scope,
            parameters.into_boxed_slice(),
            member_ids.clone().into_boxed_slice(),
            values.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::View(declaration),
            member_ids.into_boxed_slice(),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection { item, members })
    }
}

pub(in crate::final_lowering::item_lowering) fn preflight_view_exports(
    export_count: usize,
) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, export_count)
}

fn project_view_part(path: &AttachedViewPartPath) -> Result<HirPathValue, HirLowerFailure> {
    match path {
        AttachedViewPartPath::Path(path) => Ok(match project_attached_path(path)? {
            TypedPathProjection::Resolved(path) => HirPathValue::Resolved(path),
            TypedPathProjection::Recovered(recovery) => HirPathValue::Recovered(recovery),
        }),
        AttachedViewPartPath::Missing(_) => Ok(HirPathValue::Recovered(HirPathRecovery::new(
            HirPathRoot::ImplicitCrate,
            0,
            HirPathIssue::Empty,
        ))),
    }
}
