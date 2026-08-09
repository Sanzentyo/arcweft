//! External-capability lowering into one source-frozen final HIR owner.

use arcweft_lang_syntax::attachment::node::ExternCapabilityItemKind;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCapabilityAssociatedType, AttachedCapabilityFunction, AttachedCapabilityMember,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirCapabilityAssociatedType, HirCapabilityFunction, HirCapabilityMember,
    HirExternCapabilityItem, HirItem, HirItemIssue, HirItemKind,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::{StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state, project_required_name};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_extern_capability_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<ExternCapabilityItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_extern_capability_members(attached.body().members().len())?;

        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let mut issue = prefix.issue.or(name.issue);
        if !attached.header_recovery().is_empty() {
            issue.get_or_insert(HirItemIssue::MalformedHeader);
        }
        if attached.body().is_missing() {
            issue.get_or_insert(HirItemIssue::MissingBody);
        }

        let mut members = Vec::with_capacity(attached.body().members().len());
        for (position, member) in attached.body().members().iter().enumerate() {
            if usize::from(member.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let (lowered, member_issue) = match member {
                AttachedCapabilityMember::AssociatedType(associated) => {
                    self.lower_capability_associated_type(scope, associated)?
                }
                AttachedCapabilityMember::Function(function) => {
                    self.lower_capability_function(owner, scope, function)?
                }
                AttachedCapabilityMember::Error { .. } => (
                    HirCapabilityMember::Error,
                    Some(HirItemIssue::InvalidMember),
                ),
            };
            issue = issue.or(member_issue);
            members.push(lowered);
        }
        if attached.body().is_unclosed() {
            issue.get_or_insert(HirItemIssue::Recovery);
        }

        let declaration = HirExternCapabilityItem::try_new(
            owner.module(),
            name.value,
            members.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::ExternCapability(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    fn lower_capability_associated_type(
        &mut self,
        scope: ScopeId,
        attached: &AttachedCapabilityAssociatedType,
    ) -> Result<(HirCapabilityMember, Option<HirItemIssue>), HirLowerFailure> {
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let (value, missing_type, type_recovery) = match attached.value() {
            Some(value) => {
                let ty = self.lower_attached_type(value, scope)?;
                let poisoned = self.staged_type_is_poisoned(ty)?;
                (
                    Some(ty),
                    poisoned && value.syntax().kind() == SyntaxKind::MissingType,
                    poisoned,
                )
            }
            None => (None, false, false),
        };
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| type_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let associated = HirCapabilityAssociatedType::try_new(
            scope.module(),
            prefix.value,
            name.value,
            generic_parameters,
            value,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((HirCapabilityMember::AssociatedType(associated), issue))
    }

    fn lower_capability_function(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        attached: &AttachedCapabilityFunction,
    ) -> Result<(HirCapabilityMember, Option<HirItemIssue>), HirLowerFailure> {
        let callable_scope = self.allocate_item_callable_scope(attached.syntax(), owner, scope)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), callable_scope)?;
        let parameter_groups = self.lower_function_parameter_groups(
            attached.parameter_groups(),
            callable_scope,
            attached.has_parameter_shape_recovery(),
        )?;
        let (return_type, return_missing_type, return_recovery) = match attached.authored_return() {
            Some(authored) => {
                let ty = self.lower_attached_type(authored.ty(), callable_scope)?;
                let poisoned = self.staged_type_is_poisoned(ty)?;
                (
                    Some(ty),
                    poisoned && authored.ty().syntax().kind() == SyntaxKind::MissingType,
                    poisoned,
                )
            }
            None => (None, false, false),
        };

        let mut effects = Vec::new();
        let mut effects_recovery = false;
        if let Some(attached_effects) = attached.effects() {
            effects_recovery = attached_effects.has_recovery();
            effects.reserve(attached_effects.expressions().len());
            for expression in attached_effects.expressions() {
                let expression = self.lower_attached_expression(expression, callable_scope)?;
                effects_recovery |= self.staged_expression_is_poisoned(expression)?;
                effects.push(expression);
            }
        }

        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| {
                parameter_groups
                    .missing_type
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| {
                parameter_groups
                    .recovery
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| return_missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| return_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| effects_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let function = HirCapabilityFunction::try_new(
            prefix.value,
            name.value,
            generic_parameters,
            parameter_groups.groups,
            return_type,
            callable_scope,
            effects.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((HirCapabilityMember::Function(function), issue))
    }
}

pub(super) fn preflight_extern_capability_members(
    member_count: usize,
) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}
