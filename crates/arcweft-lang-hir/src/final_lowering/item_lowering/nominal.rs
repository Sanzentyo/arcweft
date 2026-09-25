//! Final nominal item lowering.

use arcweft_lang_syntax::attachment::{
    AstNode, AttachedEnumBody, AttachedEnumVariantPayload, AttachedResourceBody,
    AttachedResourceInitializer, AttachedResourcePublicId, AttachedStructBody,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirEnumItem, HirEnumVariant, HirEnumVariantField, HirEnumVariantPayload, HirItem, HirItemIssue,
    HirItemKind, HirResourceDeclaration, HirResourceField, HirStructField, HirStructItem,
    HirTypeAliasItem,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::{StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state, project_documentation, project_required_name};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_resource_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::ResourceDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_resource_fields(attached.body().fields().len())?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let public_id = attached.public_id().value().cloned();
        let resource_type = self.lower_attached_type(attached.resource_type(), scope)?;
        let type_poisoned = self.staged_type_is_poisoned(resource_type)?;
        let mut field_recovery = false;
        let mut fields = Vec::with_capacity(attached.body().fields().len());
        for field in attached.body().fields() {
            let value = match field.initializer() {
                AttachedResourceInitializer::Authored(initializer) => {
                    self.lower_attached_expression(initializer, scope)?
                }
                AttachedResourceInitializer::Absent => {
                    field_recovery = true;
                    continue;
                }
            };
            let name = project_required_name(field.name())?;
            field_recovery |= field.has_recovery()
                || name.issue.is_some()
                || self.staged_expression_is_poisoned(value)?;
            fields.push(HirResourceField::new(name.value, value));
        }
        let fields = fields.into_boxed_slice();
        let type_issue = if type_poisoned {
            Some(
                if attached.resource_type().syntax().kind() == SyntaxKind::MissingType {
                    HirItemIssue::MissingType
                } else {
                    HirItemIssue::Recovery
                },
            )
        } else if attached.has_nominal_type_head() {
            None
        } else {
            Some(HirItemIssue::MalformedHeader)
        };
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| {
                matches!(
                    attached.public_id(),
                    AttachedResourcePublicId::Recovered { .. }
                )
                .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or(type_issue)
            .or_else(|| {
                matches!(attached.body(), AttachedResourceBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| field_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| {
                attached
                    .body()
                    .is_unclosed()
                    .then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Resource(HirResourceDeclaration::new(
                public_id,
                name.value,
                resource_type,
                fields,
            )),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_type_alias(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::TypeAliasItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let target = self.lower_attached_type(attached.target(), scope)?;
        let target_recovery = self.staged_type_is_poisoned(target)?;
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), scope)?;
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                attached
                    .assignment()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or_else(|| target_recovery.then_some(HirItemIssue::MissingType))
            .or_else(|| where_recovery.then_some(HirItemIssue::Recovery));
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::TypeAlias(HirTypeAliasItem::new(
                name.value,
                generic_parameters,
                where_predicates,
                target,
            )),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_struct(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::StructItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_nominal_members(attached.body().fields().len())?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), scope)?;
        let mut field_recovery = false;
        let fields = attached
            .body()
            .fields()
            .iter()
            .map(|field| {
                let name = project_required_name(field.name())?;
                let ty = self.lower_attached_type(field.ty(), scope)?;
                field_recovery |= name.issue.is_some()
                    || field.has_recovery()
                    || self.staged_type_is_poisoned(ty)?;
                Ok(HirStructField::new(
                    field.prefix().documentation().map(project_documentation),
                    name.value,
                    ty,
                ))
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        let body_recovery = attached.body().is_missing_or_unclosed();
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| where_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                matches!(attached.body(), AttachedStructBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| field_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| body_recovery.then_some(HirItemIssue::Recovery));
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Struct(HirStructItem::new(
                name.value,
                generic_parameters,
                where_predicates,
                fields,
            )),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_enum(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::EnumItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let member_count = attached.body().variants().iter().fold(
            attached.body().variants().len(),
            |count, variant| {
                count.saturating_add(match variant.payload() {
                    AttachedEnumVariantPayload::Record(record) => record.fields().len(),
                    AttachedEnumVariantPayload::Unit | AttachedEnumVariantPayload::Tuple(_) => 0,
                })
            },
        );
        preflight_nominal_members(member_count)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), scope)?;
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), scope)?;
        let mut variant_recovery = false;
        let variants = attached
            .body()
            .variants()
            .iter()
            .map(|variant| {
                let name = project_required_name(variant.name())?;
                let payload = match variant.payload() {
                    AttachedEnumVariantPayload::Unit => HirEnumVariantPayload::Unit,
                    AttachedEnumVariantPayload::Tuple(payload) => {
                        let payload = self.lower_attached_type(payload, scope)?;
                        variant_recovery |= self.staged_type_is_poisoned(payload)?;
                        HirEnumVariantPayload::Tuple(payload)
                    }
                    AttachedEnumVariantPayload::Record(record) => {
                        let mut fields = Vec::with_capacity(record.fields().len());
                        for field in record.fields() {
                            let name = project_required_name(field.name())?;
                            let ty = self.lower_attached_type(field.ty(), scope)?;
                            variant_recovery |= field.has_recovery()
                                || name.issue.is_some()
                                || self.staged_type_is_poisoned(ty)?;
                            fields.push(HirEnumVariantField::new(
                                field.prefix().documentation().map(project_documentation),
                                name.value,
                                ty,
                            ));
                        }
                        HirEnumVariantPayload::Record(fields.into_boxed_slice())
                    }
                };
                variant_recovery |= name.issue.is_some() || variant.has_recovery();
                Ok(HirEnumVariant::new(
                    variant.prefix().documentation().map(project_documentation),
                    name.value,
                    payload,
                ))
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        let body_recovery = attached.body().is_missing_or_unclosed();
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| where_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                matches!(attached.body(), AttachedEnumBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| variant_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| body_recovery.then_some(HirItemIssue::Recovery));
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Enum(HirEnumItem::new(
                name.value,
                generic_parameters,
                where_predicates,
                variants,
            )),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }
}

pub(super) fn preflight_nominal_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}

pub(super) fn preflight_resource_fields(field_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, field_count)
}
