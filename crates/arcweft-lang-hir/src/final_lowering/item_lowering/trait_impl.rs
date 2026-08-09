//! Direct attached Trait/Impl lowering into inline final-HIR members.

use arcweft_lang_syntax::attachment::node::{
    ErrorNodeKind, FunctionItemKind, ImplItemKind, TraitItemKind,
};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCallableParameterKind, AttachedCallableReturn, AttachedFunctionBody,
    AttachedGenericParameterGroup, AttachedImplAssociatedType, AttachedImplFunction,
    AttachedImplMember, AttachedItemPrefix, AttachedMethodParameter, AttachedMethodParameterGroup,
    AttachedMethodReceiverKind, AttachedRequiredName, AttachedTraitAssociatedType,
    AttachedTraitFunction, AttachedTraitMember, AttachedWhereClause,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::{HirLimit, ItemId, LocalId, ScopeId};
use crate::item::{
    HirFunctionBody, HirImplAssociatedType, HirImplFunction, HirImplItem, HirImplMember, HirItem,
    HirItemIssue, HirItemKind, HirMethodParameter, HirMethodParameterGroup, HirMethodReceiver,
    HirMethodReceiverKind, HirParameter, HirParameterKind, HirTraitAssociatedType,
    HirTraitFunction, HirTraitItem, HirTraitMember,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::HirPatternBindingPolicy;

use super::super::{StagedHirModuleTransaction, require_limit};
use super::{ItemProjection, LoweredItemProjection, item_state, project_required_name};

struct LoweredMethodParameterGroups {
    groups: Box<[HirMethodParameterGroup]>,
    locals: Box<[LocalId]>,
    missing_type: bool,
    recovery: bool,
}

struct LoweredMethodParts {
    prefix: crate::item::HirItemPrefix,
    name: crate::item::HirRequiredName,
    generic_parameters: Box<[crate::item::HirGenericParameter]>,
    parameter_groups: Box<[HirMethodParameterGroup]>,
    where_predicates: Box<[crate::item::HirWherePredicate]>,
    return_type: Option<crate::identity::TypeId>,
    callable_scope: ScopeId,
    body: Option<HirFunctionBody>,
    issue: Option<HirItemIssue>,
}

/// Borrowed attachment fields shared by Trait and Impl method lowering.
///
/// This is an input record for the lowering context, not a second syntax API:
/// both conversions preserve the original attached owners and their order.
struct AttachedMethodInput<'a> {
    syntax: &'a AstNode<FunctionItemKind>,
    prefix: &'a AttachedItemPrefix,
    name: &'a AttachedRequiredName,
    generics: Option<&'a AttachedGenericParameterGroup>,
    parameter_groups: &'a [AttachedMethodParameterGroup],
    has_parameter_shape_recovery: bool,
    where_clauses: &'a [AttachedWhereClause],
    authored_return: Option<&'a AttachedCallableReturn>,
    body: Option<&'a AttachedFunctionBody>,
    trailing_recovery: &'a [AstNode<ErrorNodeKind>],
}

impl<'a> From<&'a AttachedTraitFunction> for AttachedMethodInput<'a> {
    fn from(attached: &'a AttachedTraitFunction) -> Self {
        Self {
            syntax: attached.syntax(),
            prefix: attached.prefix(),
            name: attached.name(),
            generics: attached.generics(),
            parameter_groups: attached.parameter_groups(),
            has_parameter_shape_recovery: attached.has_parameter_shape_recovery(),
            where_clauses: attached.where_clauses(),
            authored_return: attached.authored_return(),
            body: attached.body(),
            trailing_recovery: attached.trailing_recovery(),
        }
    }
}

impl<'a> From<&'a AttachedImplFunction> for AttachedMethodInput<'a> {
    fn from(attached: &'a AttachedImplFunction) -> Self {
        Self {
            syntax: attached.syntax(),
            prefix: attached.prefix(),
            name: attached.name(),
            generics: attached.generics(),
            parameter_groups: attached.parameter_groups(),
            has_parameter_shape_recovery: attached.has_parameter_shape_recovery(),
            where_clauses: attached.where_clauses(),
            authored_return: attached.authored_return(),
            body: attached.body(),
            trailing_recovery: attached.trailing_recovery(),
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_trait_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<TraitItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_trait_members(attached.body().members().len())?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;

        let mut supertrait_recovery = false;
        let mut supertraits = Vec::with_capacity(attached.supertraits().len());
        for supertrait in attached.supertraits() {
            let ty = self.lower_attached_type(supertrait, scope)?;
            supertrait_recovery |= self.staged_type_is_poisoned(ty)?;
            supertraits.push(ty);
        }
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), scope)?;

        let mut member_issue = None;
        let mut members = Vec::with_capacity(attached.body().members().len());
        for (position, member) in attached.body().members().iter().enumerate() {
            if usize::from(member.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let lowered = self.lower_trait_member(owner, scope, member)?;
            member_issue = member_issue.or(lowered.issue);
            members.push(lowered.value);
        }

        let declaration = HirTraitItem::try_new(
            owner.module(),
            name.value,
            generic_parameters,
            supertraits.into_boxed_slice(),
            where_predicates,
            members.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| supertrait_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Trait(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_impl_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<ImplItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_impl_members(attached.body().members().len())?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let trait_ref = attached
            .trait_ref()
            .map(|ty| self.lower_attached_type(ty, scope))
            .transpose()?;
        let target = self.lower_attached_type(attached.target(), scope)?;
        let trait_recovery = trait_ref
            .map(|ty| self.staged_type_is_poisoned(ty))
            .transpose()?
            .unwrap_or(false);
        let target_recovery = self.staged_type_is_poisoned(target)?;
        let target_missing =
            target_recovery && attached.target().syntax().kind() == SyntaxKind::MissingType;
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), scope)?;

        let mut member_issue = None;
        let mut members = Vec::with_capacity(attached.body().members().len());
        for (position, member) in attached.body().members().iter().enumerate() {
            if usize::from(member.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let lowered = self.lower_impl_member(owner, scope, member)?;
            member_issue = member_issue.or(lowered.issue);
            members.push(lowered.value);
        }

        let declaration = HirImplItem::try_new(
            owner.module(),
            generic_parameters,
            trait_ref,
            target,
            where_predicates,
            members.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let issue = prefix
            .issue
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| target_missing.then_some(HirItemIssue::MissingType))
            .or_else(|| {
                (trait_recovery || target_recovery).then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| {
                attached
                    .body()
                    .is_missing()
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(member_issue)
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Impl(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    fn lower_trait_member(
        &mut self,
        owner: ItemId,
        item_scope: ScopeId,
        member: &AttachedTraitMember,
    ) -> Result<ItemProjection<HirTraitMember>, HirLowerFailure> {
        match member {
            AttachedTraitMember::AssociatedType(member) => self
                .lower_trait_associated_type(item_scope, member)
                .map(|lowered| ItemProjection {
                    value: HirTraitMember::AssociatedType(lowered.value),
                    issue: lowered.issue,
                }),
            AttachedTraitMember::Function(member) => self
                .lower_trait_function(owner, item_scope, member)
                .map(|lowered| ItemProjection {
                    value: HirTraitMember::Function(lowered.value),
                    issue: lowered.issue,
                }),
            AttachedTraitMember::Error { .. } => Ok(ItemProjection::recovered(
                HirTraitMember::Error,
                HirItemIssue::InvalidMember,
            )),
        }
    }

    fn lower_impl_member(
        &mut self,
        owner: ItemId,
        item_scope: ScopeId,
        member: &AttachedImplMember,
    ) -> Result<ItemProjection<HirImplMember>, HirLowerFailure> {
        match member {
            AttachedImplMember::AssociatedType(member) => self
                .lower_impl_associated_type(item_scope, member)
                .map(|lowered| ItemProjection {
                    value: HirImplMember::AssociatedType(lowered.value),
                    issue: lowered.issue,
                }),
            AttachedImplMember::Function(member) => self
                .lower_impl_function(owner, item_scope, member)
                .map(|lowered| ItemProjection {
                    value: HirImplMember::Function(lowered.value),
                    issue: lowered.issue,
                }),
            AttachedImplMember::Error { .. } => Ok(ItemProjection::recovered(
                HirImplMember::Error,
                HirItemIssue::InvalidMember,
            )),
        }
    }

    fn lower_trait_associated_type(
        &mut self,
        scope: ScopeId,
        attached: &AttachedTraitAssociatedType,
    ) -> Result<ItemProjection<HirTraitAssociatedType>, HirLowerFailure> {
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let default = attached
            .default()
            .map(|ty| self.lower_attached_type(ty, scope))
            .transpose()?;
        let default_recovery = default
            .map(|ty| self.staged_type_is_poisoned(ty))
            .transpose()?
            .unwrap_or(false);
        let value = HirTraitAssociatedType::try_new(
            scope.module(),
            prefix.value,
            name.value,
            generic_parameters,
            default,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(ItemProjection {
            value,
            issue: prefix
                .issue
                .or(name.issue)
                .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| default_recovery.then_some(HirItemIssue::Recovery))
                .or_else(|| {
                    (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
                }),
        })
    }

    fn lower_impl_associated_type(
        &mut self,
        scope: ScopeId,
        attached: &AttachedImplAssociatedType,
    ) -> Result<ItemProjection<HirImplAssociatedType>, HirLowerFailure> {
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let target = self.lower_attached_type(attached.target(), scope)?;
        let target_recovery = self.staged_type_is_poisoned(target)?;
        let target_missing =
            target_recovery && attached.target().syntax().kind() == SyntaxKind::MissingType;
        let value = HirImplAssociatedType::try_new(
            scope.module(),
            prefix.value,
            name.value,
            generic_parameters,
            target,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(ItemProjection {
            value,
            issue: prefix
                .issue
                .or(name.issue)
                .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| target_missing.then_some(HirItemIssue::MissingType))
                .or_else(|| target_recovery.then_some(HirItemIssue::Recovery))
                .or_else(|| {
                    (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
                }),
        })
    }

    fn lower_trait_function(
        &mut self,
        owner: ItemId,
        item_scope: ScopeId,
        attached: &AttachedTraitFunction,
    ) -> Result<ItemProjection<HirTraitFunction>, HirLowerFailure> {
        let input = AttachedMethodInput::from(attached);
        let parts = self.lower_method_parts(owner, item_scope, &input)?;
        let value = HirTraitFunction::try_new(
            owner.module(),
            parts.prefix,
            parts.name,
            parts.generic_parameters,
            parts.parameter_groups,
            parts.where_predicates,
            parts.return_type,
            parts.callable_scope,
            parts.body,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(ItemProjection {
            value,
            issue: parts.issue,
        })
    }

    fn lower_impl_function(
        &mut self,
        owner: ItemId,
        item_scope: ScopeId,
        attached: &AttachedImplFunction,
    ) -> Result<ItemProjection<HirImplFunction>, HirLowerFailure> {
        let input = AttachedMethodInput::from(attached);
        let parts = self.lower_method_parts(owner, item_scope, &input)?;
        let value = HirImplFunction::try_new(
            owner.module(),
            parts.prefix,
            parts.name,
            parts.generic_parameters,
            parts.parameter_groups,
            parts.where_predicates,
            parts.return_type,
            parts.callable_scope,
            parts.body,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(ItemProjection {
            value,
            issue: parts.issue,
        })
    }

    fn lower_method_parts(
        &mut self,
        owner: ItemId,
        item_scope: ScopeId,
        attached: &AttachedMethodInput<'_>,
    ) -> Result<LoweredMethodParts, HirLowerFailure> {
        let callable_scope =
            self.allocate_item_callable_scope(attached.syntax, owner, item_scope)?;
        let prefix = self.lower_item_prefix(attached.prefix, item_scope)?;
        let name = project_required_name(attached.name)?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics, callable_scope)?;
        let LoweredMethodParameterGroups {
            groups: parameter_groups,
            locals: parameter_locals,
            missing_type: parameter_missing_type,
            recovery: parameter_recovery,
        } = self.lower_method_parameter_groups(
            attached.parameter_groups,
            callable_scope,
            attached.has_parameter_shape_recovery,
        )?;
        let (return_type, return_missing, return_recovery) = match attached.authored_return {
            Some(authored) => {
                let ty = self.lower_attached_type(authored.ty(), callable_scope)?;
                let poisoned = self.staged_type_is_poisoned(ty)?;
                (
                    Some(ty),
                    poisoned && authored.ty().syntax().kind() == SyntaxKind::MissingType,
                    poisoned || authored.has_recovery(),
                )
            }
            None => (None, false, false),
        };
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses, callable_scope)?;

        let (body, body_recovery) = match attached.body {
            Some(attached_body @ AttachedFunctionBody::Block { block, .. }) => {
                let lowered = self.lower_attached_method_block(
                    block,
                    owner,
                    callable_scope,
                    parameter_locals,
                )?;
                (
                    Some(HirFunctionBody::Block {
                        scope: lowered.scope,
                        statements: lowered.statements,
                        tail: lowered.tail,
                    }),
                    attached_body.has_recovery() || lowered.recovery.is_some(),
                )
            }
            Some(AttachedFunctionBody::Missing { .. }) => {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            None => {
                self.close_scope_members(callable_scope, parameter_locals)?;
                (None, false)
            }
        };

        Ok(LoweredMethodParts {
            prefix: prefix.value,
            name: name.value,
            generic_parameters,
            parameter_groups,
            where_predicates,
            return_type,
            callable_scope,
            body,
            issue: prefix
                .issue
                .or(name.issue)
                .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| parameter_missing_type.then_some(HirItemIssue::MissingType))
                .or_else(|| parameter_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| return_missing.then_some(HirItemIssue::MissingType))
                .or_else(|| return_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
                .or_else(|| body_recovery.then_some(HirItemIssue::Recovery))
                .or_else(|| {
                    (!attached.trailing_recovery.is_empty()).then_some(HirItemIssue::Recovery)
                }),
        })
    }

    fn lower_method_parameter_groups(
        &mut self,
        groups: &[AttachedMethodParameterGroup],
        callable_scope: ScopeId,
        has_shape_recovery: bool,
    ) -> Result<LoweredMethodParameterGroups, HirLowerFailure> {
        let mut recovery = groups
            .iter()
            .any(AttachedMethodParameterGroup::has_recovery)
            || has_shape_recovery;
        let mut missing_type = false;
        let mut lowered_groups = Vec::with_capacity(groups.len());
        let mut callable_locals = Vec::new();
        let mut source_position = 0_usize;
        for (group_position, group) in groups.iter().enumerate() {
            if usize::from(group.source_ordinal()) != group_position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let mut parameters = Vec::with_capacity(group.parameters().len());
            for (parameter_position, parameter) in group.parameters().iter().enumerate() {
                if usize::from(parameter.source_ordinal()) != source_position
                    || usize::from(parameter.group_ordinal()) != group_position
                    || usize::from(parameter.parameter_ordinal()) != parameter_position
                {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                source_position += 1;
                match parameter {
                    AttachedMethodParameter::Receiver(receiver) => {
                        let pattern = self.lower_attached_pattern_binding(
                            receiver.pattern(),
                            callable_scope,
                            HirPatternBindingPolicy::CallableParameter,
                        )?;
                        recovery |= pattern.poisoned;
                        callable_locals.extend_from_slice(&pattern.locals);
                        parameters.push(HirMethodParameter::Receiver(
                            HirMethodReceiver::try_new(
                                method_receiver_kind(receiver.kind()),
                                pattern.owner,
                                pattern.locals,
                            )
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        ));
                    }
                    AttachedMethodParameter::Typed(parameter) => {
                        let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
                        let pattern = self.lower_attached_pattern_binding(
                            parameter.pattern(),
                            callable_scope,
                            HirPatternBindingPolicy::CallableParameter,
                        )?;
                        let type_poisoned = self.staged_type_is_poisoned(ty)?;
                        let kind = match parameter.kind() {
                            AttachedCallableParameterKind::Fixed => HirParameterKind::Fixed,
                            AttachedCallableParameterKind::Rest { .. } => {
                                HirParameterKind::RestPositional
                            }
                        };
                        let default = parameter
                            .default()
                            .map(|default| {
                                self.lower_attached_expression(default.value(), callable_scope)
                            })
                            .transpose()?;
                        let default_poisoned = default
                            .map(|value| self.staged_expression_is_poisoned(value))
                            .transpose()?
                            .unwrap_or(false);
                        missing_type |= type_poisoned
                            && parameter.ty().syntax().kind() == SyntaxKind::MissingType;
                        recovery |= pattern.poisoned
                            || type_poisoned
                            || default_poisoned
                            || parameter.has_recovery();
                        callable_locals.extend_from_slice(&pattern.locals);
                        parameters.push(HirMethodParameter::Typed(
                            HirParameter::try_new(pattern.owner, ty, kind, default, pattern.locals)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        ));
                    }
                }
            }
            lowered_groups.push(
                HirMethodParameterGroup::try_new(
                    callable_scope.module(),
                    parameters.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        require_limit(HirLimit::LocalsPerScope, callable_locals.len())?;
        Ok(LoweredMethodParameterGroups {
            groups: lowered_groups.into_boxed_slice(),
            locals: callable_locals.into_boxed_slice(),
            missing_type,
            recovery,
        })
    }
}

const fn method_receiver_kind(kind: AttachedMethodReceiverKind) -> HirMethodReceiverKind {
    match kind {
        AttachedMethodReceiverKind::Owned => HirMethodReceiverKind::Owned,
        AttachedMethodReceiverKind::SharedReference => HirMethodReceiverKind::SharedReference,
        AttachedMethodReceiverKind::MutableReference => HirMethodReceiverKind::MutableReference,
    }
}

pub(super) fn preflight_trait_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}

pub(super) fn preflight_impl_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}
