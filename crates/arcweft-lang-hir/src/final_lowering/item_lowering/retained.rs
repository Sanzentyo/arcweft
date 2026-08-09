//! Final retained-identity item lowering.

use arcweft_id::{CharacterSurfaceAlias, DeclarationIdentityFamily, DeclarationName};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCharacterBody, AttachedCharacterDeclaration, AttachedCharacterInitializer,
    AttachedCharacterMember, AttachedCharacterSurfaceAlias, AttachedDeclarationPublicId,
    AttachedDeclarationPublicIdIssue, AttachedRetainedName,
};
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirActionDeclaration, HirCharacterAssignmentState, HirCharacterDeclaration,
    HirCharacterDisplayNameMember, HirCharacterMemberRecovery, HirCharacterSurfaceAlias,
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind, HirParameter, HirParameterKind, HirPublicIdOrigin,
    HirRetainedHeader, HirRetainedName, HirRetainedPublicId, HirRetainedPublicIdIssue,
    HirSignalDeclaration,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::HirPatternBindingPolicy;

use super::super::{StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state};

mod activity;
mod layer;
mod metric;
mod view;

#[cfg(test)]
pub(super) use self::view::preflight_view_exports;

impl StagedHirModuleTransaction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "Character lowering atomically projects its closed header, member, and initializer inventory"
    )]
    pub(super) fn lower_character_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::CharacterDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        preflight_character_members(attached.body().members().len())?;
        let header =
            project_retained_header(attached.header(), DeclarationIdentityFamily::Character)?;
        let alias = project_character_alias(attached.surface_alias())?;
        let mut retained_members = Vec::with_capacity(attached.body().members().len());
        let mut member_ids = Vec::with_capacity(attached.body().members().len());
        let mut display_name = None;

        for (position, member) in attached.body().members().iter().enumerate() {
            let ordinal =
                u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if u32::from(member.source_ordinal()) != ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let id = HirDeclarationMemberId::new(owner, ordinal);
            let (kind, state) = match member {
                AttachedCharacterMember::DisplayName(member) => {
                    display_name.get_or_insert(id);
                    let assignment = if member.assignment().is_missing() {
                        HirCharacterAssignmentState::Missing
                    } else {
                        HirCharacterAssignmentState::Present
                    };
                    let initializer = match member.initializer() {
                        AttachedCharacterInitializer::Authored(expression) => {
                            Some(self.lower_attached_expression(expression, scope)?)
                        }
                        AttachedCharacterInitializer::Missing(_) => None,
                    };
                    let state = if member.is_duplicate() {
                        HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::Duplicate,
                        )
                    } else if assignment == HirCharacterAssignmentState::Missing {
                        HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::MissingAssignment,
                        )
                    } else if initializer.is_none() {
                        HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::MissingInitializer,
                        )
                    } else if initializer.is_some_and(|initializer| {
                        let (slots, arenas) = self.storage_mut();
                        arenas
                            .expressions()
                            .resolve_staged(slots, initializer)
                            .is_ok_and(crate::expr::HirExpr::is_poisoned)
                    }) {
                        HirDeclarationMemberPoisonState::Poisoned(
                            HirDeclarationMemberIssue::RecoveredChild,
                        )
                    } else {
                        HirDeclarationMemberPoisonState::Clean
                    };
                    (
                        HirDeclarationMemberKind::CharacterDisplayName(
                            HirCharacterDisplayNameMember::new(
                                assignment,
                                initializer,
                                member.is_duplicate(),
                            ),
                        ),
                        state,
                    )
                }
                AttachedCharacterMember::Recovery { .. } => (
                    HirDeclarationMemberKind::CharacterRecovery(
                        HirCharacterMemberRecovery::Unknown,
                    ),
                    HirDeclarationMemberPoisonState::Poisoned(
                        HirDeclarationMemberIssue::UnclassifiedSyntax,
                    ),
                ),
            };
            retained_members.push(
                HirDeclarationMember::try_new(id, kind, state)
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
                    HirItemFamily::Character,
                    retained_members.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            )
        };
        let state = item_state(
            prefix
                .issue
                .or_else(|| character_issue(&attached, members.as_ref())),
        );
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Character(HirCharacterDeclaration::new(header, alias, display_name)),
            member_ids.into_boxed_slice(),
            state,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection { item, members })
    }

    pub(super) fn lower_signal_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::SignalDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let prefix_issue = prefix.issue;
        let header = project_retained_header(attached.header(), DeclarationIdentityFamily::Signal)?;
        let observable_type = self.lower_attached_type(attached.observable_type(), scope)?;
        let type_recovery = self.staged_type_is_poisoned(observable_type)?;
        let type_issue = type_recovery.then_some(
            if attached.observable_type().syntax().kind() == SyntaxKind::MissingType {
                HirItemIssue::MissingType
            } else {
                HirItemIssue::Recovery
            },
        );
        let issue = prefix_issue
            .or_else(|| retained_header_issue(attached.header()))
            .or_else(|| {
                attached
                    .colon()
                    .is_missing()
                    .then_some(HirItemIssue::Recovery)
            })
            .or(type_issue)
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            });
        let declaration = HirSignalDeclaration::try_new(header, observable_type)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Signal(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_action_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::ActionDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let prefix_issue = prefix.issue;
        let header = project_retained_header(attached.header(), DeclarationIdentityFamily::Action)?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;
        let mut parameters = Vec::with_capacity(attached.signature().parameters().len());
        let mut scope_locals = Vec::new();
        let mut parameter_issue = None;

        for parameter in attached.signature().parameters() {
            let pattern = self.lower_attached_pattern_binding(
                parameter.pattern(),
                callable_scope,
                HirPatternBindingPolicy::CallableParameter,
            )?;
            let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
            let type_poisoned = self.staged_type_is_poisoned(ty)?;

            parameter_issue = parameter_issue
                .or_else(|| {
                    parameter
                        .has_invalid_binding()
                        .then_some(HirItemIssue::InvalidMember)
                })
                .or_else(|| {
                    (parameter.colon().is_missing()
                        || (type_poisoned
                            && parameter.ty().syntax().kind() == SyntaxKind::MissingType))
                        .then_some(HirItemIssue::MissingType)
                })
                .or_else(|| type_poisoned.then_some(HirItemIssue::InvalidMember))
                .or_else(|| {
                    parameter
                        .forbidden_default()
                        .is_some()
                        .then_some(HirItemIssue::InvalidMember)
                });

            scope_locals.extend_from_slice(&pattern.locals);
            parameters.push(
                HirParameter::try_new(
                    pattern.owner,
                    ty,
                    HirParameterKind::Fixed,
                    None,
                    pattern.locals,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        self.close_scope_members(callable_scope, scope_locals.into_boxed_slice())?;

        let issue = prefix_issue
            .or_else(|| retained_header_issue(attached.header()))
            .or_else(|| {
                (attached.signature().open_state().is_missing()
                    || attached.signature().close_state().is_missing())
                .then_some(HirItemIssue::Recovery)
            })
            .or(parameter_issue)
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            });
        let declaration =
            HirActionDeclaration::try_new(header, callable_scope, parameters.into_boxed_slice())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Action(declaration),
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

fn project_retained_header(
    attached: &arcweft_lang_syntax::attachment::AttachedRetainedHeader,
    family: DeclarationIdentityFamily,
) -> Result<HirRetainedHeader, HirLowerFailure> {
    let name = match attached.name() {
        AttachedRetainedName::Resolved { value, .. } => HirRetainedName::Resolved(
            DeclarationName::try_new(value.as_str())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
        ),
        AttachedRetainedName::Missing { .. } => HirRetainedName::Missing,
        AttachedRetainedName::Invalid { .. } => HirRetainedName::Invalid,
    };
    let public_id = match attached.public_id() {
        AttachedDeclarationPublicId::Derived => match &name {
            HirRetainedName::Resolved(name) => HirRetainedPublicId::Resolved {
                value: family
                    .derive_public_id(name)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                origin: HirPublicIdOrigin::DerivedFromName,
            },
            HirRetainedName::Missing | HirRetainedName::Invalid => {
                HirRetainedPublicId::Recovered(HirRetainedPublicIdIssue::DerivedFromRecoveredName)
            }
        },
        AttachedDeclarationPublicId::Explicit { value, .. } => HirRetainedPublicId::Resolved {
            value: value.clone(),
            origin: HirPublicIdOrigin::Explicit,
        },
        AttachedDeclarationPublicId::Recovered { issue, .. } => {
            HirRetainedPublicId::Recovered(match issue {
                AttachedDeclarationPublicIdIssue::WrongFamily(value) => {
                    HirRetainedPublicIdIssue::WrongFamily(value.clone())
                }
                AttachedDeclarationPublicIdIssue::Malformed => HirRetainedPublicIdIssue::Malformed,
                AttachedDeclarationPublicIdIssue::Missing => HirRetainedPublicIdIssue::Missing,
            })
        }
    };
    HirRetainedHeader::try_new(family, public_id, name)
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

fn retained_header_issue(
    attached: &arcweft_lang_syntax::attachment::AttachedRetainedHeader,
) -> Option<HirItemIssue> {
    match attached.name() {
        AttachedRetainedName::Resolved { .. } => None,
        AttachedRetainedName::Missing { .. } => Some(HirItemIssue::MissingName),
        AttachedRetainedName::Invalid { .. } => Some(HirItemIssue::MalformedHeader),
    }
    .or_else(|| {
        matches!(
            attached.public_id(),
            AttachedDeclarationPublicId::Recovered { .. }
        )
        .then_some(HirItemIssue::MalformedHeader)
    })
}

fn project_character_alias(
    attached: &AttachedCharacterSurfaceAlias,
) -> Result<HirCharacterSurfaceAlias, HirLowerFailure> {
    match attached {
        AttachedCharacterSurfaceAlias::Absent => Ok(HirCharacterSurfaceAlias::Absent),
        AttachedCharacterSurfaceAlias::Resolved { value, .. } => {
            Ok(HirCharacterSurfaceAlias::Resolved(
                CharacterSurfaceAlias::try_new(value.as_str())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            ))
        }
        AttachedCharacterSurfaceAlias::Missing { .. } => Ok(HirCharacterSurfaceAlias::Missing),
    }
}

fn character_issue(
    attached: &AttachedCharacterDeclaration,
    members: Option<&HirDeclarationMemberArena>,
) -> Option<HirItemIssue> {
    retained_header_issue(attached.header())
        .or_else(|| {
            attached
                .has_unexpected_header()
                .then_some(HirItemIssue::MalformedHeader)
        })
        .or_else(|| {
            matches!(
                attached.surface_alias(),
                AttachedCharacterSurfaceAlias::Missing { .. }
            )
            .then_some(HirItemIssue::MissingName)
        })
        .or_else(|| {
            matches!(attached.body(), AttachedCharacterBody::Missing(_))
                .then_some(HirItemIssue::MissingBody)
        })
        .or_else(|| {
            members
                .is_some_and(|members| {
                    members
                        .members()
                        .iter()
                        .any(HirDeclarationMember::is_poisoned)
                })
                .then_some(HirItemIssue::InvalidMember)
        })
        .or_else(|| {
            (attached.body().is_missing_or_unclosed() || attached.has_trailing_syntax())
                .then_some(HirItemIssue::Recovery)
        })
}

pub(super) fn preflight_character_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}
