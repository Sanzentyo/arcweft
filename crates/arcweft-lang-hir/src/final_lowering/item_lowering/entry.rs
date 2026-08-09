//! Attached Entry lowering into the final typed item owner.

use arcweft_lang_syntax::attachment::node::EntryDeclarationItemKind;
use arcweft_lang_syntax::attachment::source_file::AttachedPath;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedEntryBody, AttachedEntryHttpMethod, AttachedEntryId, AttachedEntryKind,
    AttachedEntryMember, AttachedEntryName, AttachedEntryPunctuation, AttachedEntryRoleBinding,
    AttachedEntryRouteBinding, AttachedEntryRouteBindings, AttachedEntryValue,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;

use crate::identity::{HirLimit, ItemId, ScopeId};
use crate::item::{
    HirEntryBody, HirEntryDeclaration, HirEntryGoto, HirEntryId, HirEntryKind, HirEntryKindIssue,
    HirEntryMember, HirEntryOption, HirEntryOptionValue, HirEntryPathBinding, HirEntryPathValue,
    HirEntryPunctuationState, HirEntryRoute, HirEntryRouteBinding, HirEntryRouteBindings,
    HirEntryTarget, HirEntryTypeBinding, HirHttpMethod, HirHttpMethodIssue, HirHttpMethodValue,
    HirItem, HirItemIssue, HirItemKind, HirRequiredName, HirRoutePath, HirRoutePathIssue,
    HirRoutePathValue,
};
use crate::leaf::{HirLiteral, HirPathValue, HirStringLiteral};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};

use super::super::{
    StagedHirModuleTransaction, id_ref_projection, literal_projection, name_projection,
    path_projection, require_limit,
};
use super::{LoweredItemProjection, item_state};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_entry_declaration(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        syntax: &AstNode<EntryDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = syntax
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), parent_scope)?;
        let kind = lower_entry_kind(attached.kind())?;
        let id = lower_entry_id(attached.id())?;
        let (body, member_recovery) = self.lower_entry_body(parent_scope, attached.body())?;

        let issue = prefix
            .issue
            .or_else(|| kind.has_recovery().then_some(HirItemIssue::MissingKind))
            .or_else(|| entry_id_issue(attached.id(), &id))
            .or_else(|| {
                attached
                    .has_header_trailing_recovery()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                matches!(attached.body(), AttachedEntryBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or_else(|| member_recovery.then_some(HirItemIssue::InvalidMember))
            .or_else(|| {
                (!matches!(attached.body(), AttachedEntryBody::Missing(_))
                    && !attached.body().is_closed())
                .then_some(HirItemIssue::Recovery)
            });

        let declaration = HirEntryDeclaration::try_new(
            owner.module(),
            kind,
            id,
            attached.has_header_trailing_recovery(),
            body,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item: HirItem::try_new_with_state(
                owner,
                parent_scope,
                prefix.value,
                HirItemKind::Entry(declaration),
                Box::new([]),
                item_state(issue),
            )
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            members: None,
        })
    }

    fn lower_entry_body(
        &mut self,
        scope: ScopeId,
        attached: &AttachedEntryBody,
    ) -> Result<(HirEntryBody, bool), HirLowerFailure> {
        let AttachedEntryBody::Braced { members, .. } = attached else {
            return Ok((HirEntryBody::Missing, false));
        };
        preflight_entry_members(members.len())?;
        let mut recovery = false;
        let mut lowered = Vec::with_capacity(members.len());
        for member in members {
            let (member, member_recovery) = self.lower_entry_member(scope, member)?;
            recovery |= member_recovery;
            lowered.push(member);
        }
        Ok((
            HirEntryBody::braced(lowered.into_boxed_slice(), attached.is_closed()),
            recovery,
        ))
    }

    fn lower_entry_member(
        &mut self,
        scope: ScopeId,
        attached: &AttachedEntryMember,
    ) -> Result<(HirEntryMember, bool), HirLowerFailure> {
        match attached {
            AttachedEntryMember::StateType(binding) => {
                let (binding, recovery) = self.lower_entry_type_binding(scope, binding)?;
                Ok((HirEntryMember::StateType(binding), recovery))
            }
            AttachedEntryMember::EventType(binding) => {
                let (binding, recovery) = self.lower_entry_type_binding(scope, binding)?;
                Ok((HirEntryMember::EventType(binding), recovery))
            }
            AttachedEntryMember::Initializer(binding) => {
                let retained = lower_entry_path_binding(binding)?;
                let recovery = retained.has_recovery();
                Ok((HirEntryMember::Initializer(retained), recovery))
            }
            AttachedEntryMember::Reducer(binding) => {
                let retained = lower_entry_path_binding(binding)?;
                let recovery = retained.has_recovery();
                Ok((HirEntryMember::Reducer(retained), recovery))
            }
            AttachedEntryMember::Controller(binding) => {
                let retained = lower_entry_path_binding(binding)?;
                let recovery = retained.has_recovery();
                Ok((HirEntryMember::Controller(retained), recovery))
            }
            AttachedEntryMember::Goto {
                target,
                trailing_recovery,
                ..
            } => {
                let target = lower_entry_target(target)?;
                let goto = HirEntryGoto::new(target, trailing_recovery.is_some());
                let recovery = goto.has_recovery();
                Ok((HirEntryMember::Goto(goto), recovery))
            }
            AttachedEntryMember::Route {
                method,
                path,
                arrow,
                target,
                bindings,
                trailing_recovery,
                ..
            } => {
                let route = lower_entry_route(
                    method,
                    path,
                    arrow,
                    target,
                    bindings,
                    trailing_recovery.is_some(),
                )?;
                let recovery = route.has_recovery();
                Ok((HirEntryMember::Route(route), recovery))
            }
            AttachedEntryMember::Option {
                source_ordinal: _,
                name,
                assignment,
                value,
                trailing_recovery,
                ..
            } => {
                let (retained_value, child_recovery) = match value {
                    AttachedEntryValue::Authored(expression)
                    | AttachedEntryValue::Recovered(expression) => {
                        let expression = self.lower_attached_expression(expression, scope)?;
                        (
                            HirEntryOptionValue::Expression(expression),
                            self.staged_expression_is_poisoned(expression)?,
                        )
                    }
                    AttachedEntryValue::Missing(_) => (HirEntryOptionValue::Missing, true),
                    AttachedEntryValue::Invalid(_) => (HirEntryOptionValue::Invalid, true),
                };
                let option = HirEntryOption::new(
                    lower_entry_name(name)?,
                    punctuation(assignment),
                    retained_value,
                    trailing_recovery.is_some(),
                );
                Ok((
                    HirEntryMember::Option(option),
                    name.has_recovery()
                        || assignment.is_missing()
                        || value.has_recovery()
                        || child_recovery
                        || trailing_recovery.is_some(),
                ))
            }
            AttachedEntryMember::Error { .. } => Ok((HirEntryMember::Error, true)),
        }
    }

    fn lower_entry_type_binding(
        &mut self,
        scope: ScopeId,
        attached: &AttachedEntryRoleBinding<arcweft_lang_syntax::attachment::AttachedTypeRefNode>,
    ) -> Result<(HirEntryTypeBinding, bool), HirLowerFailure> {
        let ty = attached
            .value()
            .value()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let ty = self.lower_attached_type(ty, scope)?;
        let child_recovery = self.staged_type_is_poisoned(ty)?;
        Ok((
            HirEntryTypeBinding::new(
                punctuation(attached.assignment()),
                ty,
                attached.has_trailing_recovery(),
            ),
            attached.has_recovery() || child_recovery,
        ))
    }
}

fn lower_entry_kind(attached: &AttachedEntryKind) -> Result<HirEntryKind, HirLowerFailure> {
    Ok(match attached {
        AttachedEntryKind::Game(_) => HirEntryKind::Game,
        AttachedEntryKind::Editor(_) => HirEntryKind::Editor,
        AttachedEntryKind::Cli(_) => HirEntryKind::Cli,
        AttachedEntryKind::Server(_) => HirEntryKind::Server,
        AttachedEntryKind::Activity(_) => HirEntryKind::Activity,
        AttachedEntryKind::Test(_) => HirEntryKind::Test,
        AttachedEntryKind::Bench(_) => HirEntryKind::Bench,
        AttachedEntryKind::Agent(_) => HirEntryKind::Agent,
        AttachedEntryKind::Custom { value, .. } => {
            HirEntryKind::Custom(name_projection::name(value)?)
        }
        AttachedEntryKind::Missing(_) => HirEntryKind::Recovered(HirEntryKindIssue::Missing),
    })
}

fn lower_entry_id(attached: &AttachedEntryId) -> Result<HirEntryId, HirLowerFailure> {
    match attached {
        AttachedEntryId::Authored {
            reference,
            canonical_entry_family,
            ..
        } => Ok(HirEntryId::Authored {
            value: id_ref_projection::id_ref(reference)?,
            canonical_entry_family: *canonical_entry_family,
        }),
        AttachedEntryId::Missing(_) => Ok(HirEntryId::Missing),
    }
}

fn entry_id_issue(attached: &AttachedEntryId, retained: &HirEntryId) -> Option<HirItemIssue> {
    match attached {
        AttachedEntryId::Missing(_) => Some(HirItemIssue::MissingId),
        AttachedEntryId::Authored {
            canonical_entry_family: false,
            ..
        } => Some(HirItemIssue::MalformedHeader),
        AttachedEntryId::Authored { .. } if retained.has_recovery() => Some(HirItemIssue::Recovery),
        AttachedEntryId::Authored { .. } => None,
    }
}

fn lower_entry_path_binding(
    attached: &AttachedEntryRoleBinding<AttachedPath>,
) -> Result<HirEntryPathBinding, HirLowerFailure> {
    let value = match attached.value() {
        AttachedEntryValue::Authored(path) | AttachedEntryValue::Recovered(path) => {
            let path = match path_projection::project_attached_path(path)? {
                path_projection::TypedPathProjection::Resolved(path) => {
                    HirPathValue::Resolved(path)
                }
                path_projection::TypedPathProjection::Recovered(recovery) => {
                    HirPathValue::Recovered(recovery)
                }
            };
            HirEntryPathValue::Authored(path)
        }
        AttachedEntryValue::Missing(_) => HirEntryPathValue::Missing,
        AttachedEntryValue::Invalid(_) => HirEntryPathValue::Invalid,
    };
    Ok(HirEntryPathBinding::new(
        punctuation(attached.assignment()),
        value,
        attached.has_trailing_recovery(),
    ))
}

fn lower_entry_target(
    attached: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedExpressionNode>,
) -> Result<HirEntryTarget, HirLowerFailure> {
    match attached {
        AttachedEntryValue::Authored(expression) | AttachedEntryValue::Recovered(expression) => {
            let ExpressionProjection::EntityReference(reference) = expression.projection() else {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            };
            Ok(HirEntryTarget::Authored(id_ref_projection::id_ref(
                reference,
            )?))
        }
        AttachedEntryValue::Missing(_) => Ok(HirEntryTarget::Missing),
        AttachedEntryValue::Invalid(_) => Ok(HirEntryTarget::Invalid),
    }
}

fn lower_entry_route(
    method: &AttachedEntryHttpMethod,
    path: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedExpressionNode>,
    arrow: &AttachedEntryPunctuation,
    target: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedExpressionNode>,
    bindings: &AttachedEntryRouteBindings,
    trailing_recovery: bool,
) -> Result<HirEntryRoute, HirLowerFailure> {
    Ok(HirEntryRoute::new(
        lower_http_method(method)?,
        lower_route_path(path)?,
        punctuation(arrow),
        lower_entry_target(target)?,
        lower_route_bindings(bindings)?,
        trailing_recovery,
    ))
}

fn lower_http_method(
    attached: &AttachedEntryHttpMethod,
) -> Result<HirHttpMethodValue, HirLowerFailure> {
    Ok(match attached {
        AttachedEntryHttpMethod::Get(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Get),
        AttachedEntryHttpMethod::Post(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Post),
        AttachedEntryHttpMethod::Put(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Put),
        AttachedEntryHttpMethod::Patch(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Patch),
        AttachedEntryHttpMethod::Delete(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Delete),
        AttachedEntryHttpMethod::Head(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Head),
        AttachedEntryHttpMethod::Options(_) => HirHttpMethodValue::Resolved(HirHttpMethod::Options),
        AttachedEntryHttpMethod::Unsupported {
            value: Some(value), ..
        } => HirHttpMethodValue::Recovered {
            authored: Some(name_projection::name(value)?),
            issue: HirHttpMethodIssue::Unsupported,
        },
        AttachedEntryHttpMethod::Unsupported { value: None, .. } => HirHttpMethodValue::Recovered {
            authored: None,
            issue: HirHttpMethodIssue::InvalidName,
        },
        AttachedEntryHttpMethod::Missing(_) => HirHttpMethodValue::Recovered {
            authored: None,
            issue: HirHttpMethodIssue::Missing,
        },
    })
}

fn lower_route_path(
    attached: &AttachedEntryValue<arcweft_lang_syntax::attachment::AttachedExpressionNode>,
) -> Result<HirRoutePathValue, HirLowerFailure> {
    match attached {
        AttachedEntryValue::Authored(expression) | AttachedEntryValue::Recovered(expression) => {
            let ExpressionProjection::Literal(literal) = expression.projection() else {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            };
            match literal_projection::literal(literal)? {
                HirLiteral::String(HirStringLiteral::Value(value)) => {
                    let decoded = value.clone();
                    match HirRoutePath::try_new(value) {
                        Ok(path) => Ok(HirRoutePathValue::Resolved(path)),
                        Err(_) => Ok(HirRoutePathValue::Recovered {
                            decoded: Some(decoded),
                            issue: HirRoutePathIssue::InvalidPath,
                        }),
                    }
                }
                HirLiteral::String(HirStringLiteral::Invalid(issue)) => {
                    Ok(HirRoutePathValue::Recovered {
                        decoded: None,
                        issue: HirRoutePathIssue::InvalidString(issue),
                    })
                }
                _ => Err(HirInvariantFailure::InvalidArenaCommit.into()),
            }
        }
        AttachedEntryValue::Missing(_) => Ok(HirRoutePathValue::Recovered {
            decoded: None,
            issue: HirRoutePathIssue::Missing,
        }),
        AttachedEntryValue::Invalid(_) => Ok(HirRoutePathValue::Recovered {
            decoded: None,
            issue: HirRoutePathIssue::InvalidExpression,
        }),
    }
}

fn lower_route_bindings(
    attached: &AttachedEntryRouteBindings,
) -> Result<HirEntryRouteBindings, HirLowerFailure> {
    match attached {
        AttachedEntryRouteBindings::Absent => Ok(HirEntryRouteBindings::Absent),
        AttachedEntryRouteBindings::Parenthesized { bindings, .. } => {
            preflight_entry_route_bindings(bindings.len())?;
            let bindings = bindings
                .iter()
                .map(lower_route_binding)
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(HirEntryRouteBindings::Parenthesized {
                items: bindings,
                closed: attached.is_closed(),
            })
        }
    }
}

fn lower_route_binding(
    attached: &AttachedEntryRouteBinding,
) -> Result<HirEntryRouteBinding, HirLowerFailure> {
    Ok(HirEntryRouteBinding::new(
        lower_entry_name(attached.parameter())?,
        punctuation(attached.equals()),
        punctuation(attached.colon()),
        lower_entry_name(attached.capture())?,
        attached.has_trailing_recovery(),
    ))
}

fn lower_entry_name(attached: &AttachedEntryName) -> Result<HirRequiredName, HirLowerFailure> {
    match attached {
        AttachedEntryName::Authored { value, .. } => {
            Ok(HirRequiredName::Resolved(name_projection::name(value)?))
        }
        AttachedEntryName::Missing(_) => Ok(HirRequiredName::Missing),
    }
}

const fn punctuation(attached: &AttachedEntryPunctuation) -> HirEntryPunctuationState {
    if attached.is_missing() {
        HirEntryPunctuationState::Missing
    } else {
        HirEntryPunctuationState::Present
    }
}

pub(super) fn preflight_entry_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::DeclarationMembers, member_count)
}

pub(super) fn preflight_entry_route_bindings(binding_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::CallArguments, binding_count)
}
