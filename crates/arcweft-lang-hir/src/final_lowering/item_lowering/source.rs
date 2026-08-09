//! Attached temporary Source lowering into its final typed HIR owner.
//!
//! This is a one-way bridge for the Proof authority switch. It consumes only
//! the parser-owned attachment and deliberately does not call the detached
//! Source reader that Lang-01.3 deletes.

use arcweft_lang_syntax::attachment::node::SourceItemKind;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedSourceBackpressurePolicy, AttachedSourceBody, AttachedSourceBoundedArgument,
    AttachedSourceDeclaration, AttachedSourceExpression, AttachedSourceHandlerBody,
    AttachedSourceHandlerEvent, AttachedSourceId, AttachedSourceMember, AttachedSourceName,
    AttachedSourceOverflowPolicy, AttachedSourcePattern, AttachedSourcePrivacyPolicy,
    AttachedSourcePunctuation, AttachedSourceReplayPolicy,
};

use crate::identity::{HirLimit, ItemId, LocalId, ScopeId};
use crate::item::{
    HirItem, HirItemIssue, HirItemKind, HirRequiredName, HirSourceBackpressurePolicy,
    HirSourceBackpressureValue, HirSourceBody, HirSourceBoundedArgument, HirSourceChildState,
    HirSourceEventIssue, HirSourceEventPattern, HirSourceExpressionValue, HirSourceHandler,
    HirSourceHandlerBody, HirSourceHeaders, HirSourceId, HirSourceItem, HirSourceOverflowPolicy,
    HirSourceOverflowValue, HirSourcePatternValue, HirSourcePolicyBinding, HirSourcePolicyIssue,
    HirSourcePrivacyPolicy, HirSourcePrivacyValue, HirSourcePunctuationState,
    HirSourceReplayPolicy, HirSourceReplayValue, HirSourceRequiredSlot,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind};

use super::super::{StagedHirModuleTransaction, id_ref_projection, name_projection, require_limit};
use super::{LoweredItemProjection, item_state};

struct SelectedHeader<T> {
    value: T,
    duplicate: bool,
}

struct LoweredSourceBody {
    headers: HirSourceHeaders,
    handlers: Box<[HirSourceHandler]>,
    body: HirSourceBody,
    issue: Option<HirItemIssue>,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_source_declaration(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        syntax: &AstNode<SourceItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = syntax
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), parent_scope)?;
        let id = lower_source_id(attached.id())?;
        let name = lower_source_name(attached.name())?;
        let source_type = self.lower_attached_type(attached.source_type().node(), parent_scope)?;
        let type_poisoned = self.staged_type_is_poisoned(source_type)?;
        let lowered_body = self.lower_source_body(owner, parent_scope, &attached)?;

        let issue = prefix
            .issue
            .or_else(|| source_identity_issue(&attached, id.as_ref(), name.as_ref()))
            .or_else(|| {
                attached
                    .has_missing_type_colon()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| {
                attached
                    .source_type()
                    .has_recovery()
                    .then_some(HirItemIssue::MissingType)
            })
            .or_else(|| type_poisoned.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                matches!(attached.body(), AttachedSourceBody::Missing(_))
                    .then_some(HirItemIssue::MissingBody)
            })
            .or(lowered_body.issue)
            .or_else(|| {
                (!matches!(attached.body(), AttachedSourceBody::Missing(_))
                    && !attached.body().is_closed())
                .then_some(HirItemIssue::Recovery)
            });

        let declaration = HirSourceItem::try_new(
            owner.module(),
            id,
            name,
            source_type,
            lowered_body.headers,
            lowered_body.handlers,
            lowered_body.body,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item: HirItem::try_new_with_state(
                owner,
                parent_scope,
                prefix.value,
                HirItemKind::Source(declaration),
                Box::new([]),
                item_state(issue),
            )
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            members: None,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Source body lowering selects and seals the closed header/handler inventory in source order"
    )]
    fn lower_source_body(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        attached: &AttachedSourceDeclaration,
    ) -> Result<LoweredSourceBody, HirLowerFailure> {
        let AttachedSourceBody::Braced { members, .. } = attached.body() else {
            return Ok(LoweredSourceBody {
                headers: missing_headers(),
                handlers: Box::new([]),
                body: HirSourceBody::Missing,
                issue: None,
            });
        };
        preflight_source_members(members.len())?;

        let mut from: Option<SelectedHeader<HirSourceExpressionValue>> = None;
        let mut backpressure: Option<
            SelectedHeader<HirSourcePolicyBinding<HirSourceBackpressureValue>>,
        > = None;
        let mut replay: Option<SelectedHeader<HirSourcePolicyBinding<HirSourceReplayValue>>> = None;
        let mut privacy: Option<SelectedHeader<HirSourcePolicyBinding<HirSourcePrivacyValue>>> =
            None;
        let mut handlers = Vec::new();
        let mut issue = None;

        for member in members {
            match member {
                AttachedSourceMember::From { value, .. } => {
                    if let Some(selected) = &mut from {
                        selected.duplicate = true;
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                        continue;
                    }
                    let (value, poisoned) = self.lower_source_from(value, parent_scope)?;
                    if value.has_recovery() || poisoned {
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    from = Some(SelectedHeader {
                        value,
                        duplicate: false,
                    });
                }
                AttachedSourceMember::Backpressure {
                    assignment, policy, ..
                } => {
                    if let Some(selected) = &mut backpressure {
                        selected.duplicate = true;
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                        continue;
                    }
                    let (value, poisoned) =
                        self.lower_source_backpressure(assignment, policy, parent_scope)?;
                    if matches!(value.assignment(), HirSourcePunctuationState::Missing)
                        || value.value().has_recovery()
                        || poisoned
                    {
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    backpressure = Some(SelectedHeader {
                        value,
                        duplicate: false,
                    });
                }
                AttachedSourceMember::Replay {
                    assignment, policy, ..
                } => {
                    if let Some(selected) = &mut replay {
                        selected.duplicate = true;
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                        continue;
                    }
                    let value = HirSourcePolicyBinding::new(
                        punctuation(assignment),
                        lower_source_replay(policy)?,
                    );
                    if matches!(value.assignment(), HirSourcePunctuationState::Missing)
                        || value.value().has_recovery()
                    {
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    replay = Some(SelectedHeader {
                        value,
                        duplicate: false,
                    });
                }
                AttachedSourceMember::Privacy {
                    assignment, policy, ..
                } => {
                    if let Some(selected) = &mut privacy {
                        selected.duplicate = true;
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                        continue;
                    }
                    let value = HirSourcePolicyBinding::new(
                        punctuation(assignment),
                        lower_source_privacy(policy)?,
                    );
                    if matches!(value.assignment(), HirSourcePunctuationState::Missing)
                        || value.value().has_recovery()
                    {
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    privacy = Some(SelectedHeader {
                        value,
                        duplicate: false,
                    });
                }
                AttachedSourceMember::Handler {
                    event, arrow, body, ..
                } => {
                    let (handler, recovered) =
                        self.lower_source_handler(owner, parent_scope, event, arrow, body)?;
                    if recovered {
                        issue.get_or_insert(HirItemIssue::InvalidMember);
                    }
                    handlers.push(handler);
                }
                AttachedSourceMember::UnsupportedContract { .. }
                | AttachedSourceMember::Recovery { .. } => {
                    issue.get_or_insert(HirItemIssue::InvalidMember);
                }
            }
        }

        let from = required_slot(from, &mut issue);
        let backpressure = required_slot(backpressure, &mut issue);
        let replay = required_slot(replay, &mut issue);
        let privacy = required_slot(privacy, &mut issue);
        Ok(LoweredSourceBody {
            headers: HirSourceHeaders::new(from, backpressure, replay, privacy),
            handlers: handlers.into_boxed_slice(),
            body: HirSourceBody::Braced {
                closed: attached.body().is_closed(),
            },
            issue,
        })
    }

    fn lower_source_from(
        &mut self,
        attached: &AttachedSourceExpression,
        scope: ScopeId,
    ) -> Result<(HirSourceExpressionValue, bool), HirLowerFailure> {
        match attached {
            AttachedSourceExpression::Authored(expression) => {
                let expression = self.lower_attached_expression(expression, scope)?;
                let poisoned = self.staged_expression_is_poisoned(expression)?;
                Ok((HirSourceExpressionValue::Expression(expression), poisoned))
            }
            AttachedSourceExpression::Recovered(_) => {
                Ok((HirSourceExpressionValue::Invalid, false))
            }
            AttachedSourceExpression::Missing(_) => Ok((HirSourceExpressionValue::Missing, false)),
        }
    }

    fn lower_source_backpressure(
        &mut self,
        assignment: &AttachedSourcePunctuation,
        attached: &AttachedSourceBackpressurePolicy,
        scope: ScopeId,
    ) -> Result<(HirSourcePolicyBinding<HirSourceBackpressureValue>, bool), HirLowerFailure> {
        let (value, poisoned) = match attached {
            AttachedSourceBackpressurePolicy::Latest(expression) => (
                known_backpressure_policy(expression, HirSourceBackpressurePolicy::Latest),
                false,
            ),
            AttachedSourceBackpressurePolicy::Bounded {
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call,
                ..
            } => {
                let (capacity, poisoned) = self.lower_source_capacity(capacity, scope)?;
                let overflow = lower_source_overflow(overflow)?;
                (
                    HirSourceBackpressureValue::Resolved(HirSourceBackpressurePolicy::Bounded {
                        capacity,
                        overflow,
                        unexpected_arguments: *unexpected_arguments,
                        recovered_call: *recovered_call,
                    }),
                    poisoned,
                )
            }
            AttachedSourceBackpressurePolicy::BlockingNotAllowed(expression) => (
                known_backpressure_policy(
                    expression,
                    HirSourceBackpressurePolicy::BlockingNotAllowed,
                ),
                false,
            ),
            AttachedSourceBackpressurePolicy::Missing(_) => (
                HirSourceBackpressureValue::Recovered {
                    authored: None,
                    issue: HirSourcePolicyIssue::Missing,
                },
                false,
            ),
            AttachedSourceBackpressurePolicy::Unknown { value, .. } => {
                (recovered_policy_name(value.as_ref())?, false)
            }
            AttachedSourceBackpressurePolicy::Invalid(_) => (
                HirSourceBackpressureValue::Recovered {
                    authored: None,
                    issue: HirSourcePolicyIssue::Invalid,
                },
                false,
            ),
        };
        Ok((
            HirSourcePolicyBinding::new(punctuation(assignment), value),
            poisoned,
        ))
    }

    fn lower_source_capacity(
        &mut self,
        attached: &AttachedSourceBoundedArgument,
        scope: ScopeId,
    ) -> Result<(HirSourceBoundedArgument<HirSourceExpressionValue>, bool), HirLowerFailure> {
        let AttachedSourceBoundedArgument::Present {
            value, duplicate, ..
        } = attached
        else {
            return Ok((
                HirSourceBoundedArgument::new(HirSourceExpressionValue::Missing, false),
                false,
            ));
        };
        let (value, poisoned) = self.lower_source_from(value, scope)?;
        Ok((HirSourceBoundedArgument::new(value, *duplicate), poisoned))
    }

    fn lower_source_handler(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        event: &AttachedSourceHandlerEvent,
        arrow: &AttachedSourcePunctuation,
        body: &AttachedSourceHandlerBody,
    ) -> Result<(HirSourceHandler, bool), HirLowerFailure> {
        let scope = match body {
            AttachedSourceHandlerBody::Missing(syntax) => {
                self.allocate_item_body_scope(syntax, owner, parent_scope, HirScopeKind::Block)?
            }
            AttachedSourceHandlerBody::Statement(statement) => {
                let syntax = statement.syntax();
                self.allocate_item_body_scope_from_syntax(
                    &syntax,
                    owner,
                    parent_scope,
                    HirScopeKind::Block,
                )?
            }
            AttachedSourceHandlerBody::Block { syntax, .. } => {
                self.allocate_item_body_scope(syntax, owner, parent_scope, HirScopeKind::Block)?
            }
        };
        let (event, locals, event_poisoned) = self.lower_source_event(event, scope)?;
        let (body, body_poisoned) = self.lower_source_handler_body(body, scope, locals)?;
        let handler = HirSourceHandler::new(event, punctuation(arrow), scope, body);
        let recovered = handler.has_recovery() || event_poisoned || body_poisoned;
        Ok((handler, recovered))
    }

    fn lower_source_event(
        &mut self,
        attached: &AttachedSourceHandlerEvent,
        scope: ScopeId,
    ) -> Result<(HirSourceEventPattern, Box<[LocalId]>, bool), HirLowerFailure> {
        match attached {
            AttachedSourceHandlerEvent::Item(pattern) => {
                self.lower_source_pattern(pattern, scope, HirSourceEventPattern::Item)
            }
            AttachedSourceHandlerEvent::Error(pattern) => {
                self.lower_source_pattern(pattern, scope, HirSourceEventPattern::Error)
            }
            AttachedSourceHandlerEvent::Progress(pattern) => {
                self.lower_source_pattern(pattern, scope, HirSourceEventPattern::Progress)
            }
            AttachedSourceHandlerEvent::Disconnected(condition) => Ok((
                HirSourceEventPattern::Disconnected(source_child_state(condition)),
                Box::new([]),
                false,
            )),
            AttachedSourceHandlerEvent::PermissionRevoked(condition) => Ok((
                HirSourceEventPattern::PermissionRevoked(source_child_state(condition)),
                Box::new([]),
                false,
            )),
            AttachedSourceHandlerEvent::End(condition) => Ok((
                HirSourceEventPattern::End(source_child_state(condition)),
                Box::new([]),
                false,
            )),
            AttachedSourceHandlerEvent::Unknown { value, condition } => {
                let authored = value.as_ref().map(name_projection::name).transpose()?;
                let issue = if authored.is_some() {
                    HirSourceEventIssue::Unsupported
                } else if matches!(condition, AttachedSourceExpression::Missing(_)) {
                    HirSourceEventIssue::Missing
                } else {
                    HirSourceEventIssue::Invalid
                };
                Ok((
                    HirSourceEventPattern::Recovered {
                        authored,
                        condition: source_child_state(condition),
                        issue,
                    },
                    Box::new([]),
                    false,
                ))
            }
        }
    }

    fn lower_source_pattern(
        &mut self,
        attached: &AttachedSourcePattern,
        scope: ScopeId,
        wrap: impl FnOnce(HirSourcePatternValue) -> HirSourceEventPattern,
    ) -> Result<(HirSourceEventPattern, Box<[LocalId]>, bool), HirLowerFailure> {
        match attached {
            AttachedSourcePattern::Authored(pattern) => {
                let lowered = self.lower_attached_pattern_binding(
                    pattern,
                    scope,
                    HirPatternBindingPolicy::PatternBinding,
                )?;
                Ok((
                    wrap(HirSourcePatternValue::Pattern(lowered.owner)),
                    lowered.locals,
                    lowered.poisoned,
                ))
            }
            AttachedSourcePattern::Recovered(_) => {
                Ok((wrap(HirSourcePatternValue::Invalid), Box::new([]), false))
            }
            AttachedSourcePattern::Missing(_) => {
                Ok((wrap(HirSourcePatternValue::Missing), Box::new([]), false))
            }
        }
    }

    fn lower_source_handler_body(
        &mut self,
        attached: &AttachedSourceHandlerBody,
        scope: ScopeId,
        pattern_locals: Box<[LocalId]>,
    ) -> Result<(HirSourceHandlerBody, bool), HirLowerFailure> {
        match attached {
            AttachedSourceHandlerBody::Missing(_) => {
                self.close_scope_members(scope, pattern_locals)?;
                Ok((HirSourceHandlerBody::Missing, false))
            }
            AttachedSourceHandlerBody::Statement(statement) => {
                let lowered =
                    self.lower_attached_single_statement_body(statement, scope, pattern_locals)?;
                let [statement] = lowered.statements.as_ref() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                Ok((
                    HirSourceHandlerBody::Statement(*statement),
                    lowered.poisoned,
                ))
            }
            AttachedSourceHandlerBody::Block { syntax, closed, .. } => {
                let lowered = self.lower_attached_statement_only_block_with_prefix(
                    syntax,
                    scope,
                    pattern_locals,
                )?;
                Ok((
                    HirSourceHandlerBody::Block {
                        statements: lowered.statements,
                        closed: *closed,
                    },
                    lowered.poisoned,
                ))
            }
        }
    }
}

fn lower_source_id(attached: &AttachedSourceId) -> Result<Option<HirSourceId>, HirLowerFailure> {
    match attached {
        AttachedSourceId::Absent => Ok(None),
        AttachedSourceId::Authored {
            reference,
            canonical_source_family,
            requires_name,
            ..
        } => Ok(Some(HirSourceId::new(
            id_ref_projection::id_ref(reference)?,
            *canonical_source_family,
            *requires_name,
        ))),
    }
}

fn lower_source_name(
    attached: &AttachedSourceName,
) -> Result<Option<HirRequiredName>, HirLowerFailure> {
    match attached {
        AttachedSourceName::Absent => Ok(None),
        AttachedSourceName::Missing(_) => Ok(Some(HirRequiredName::Missing)),
        AttachedSourceName::Authored {
            value: Ok(value), ..
        } => Ok(Some(HirRequiredName::Resolved(name_projection::name(
            value,
        )?))),
        AttachedSourceName::Authored {
            value: Err(issue), ..
        } => {
            name_projection::require_attempted_name_limit(issue)?;
            Ok(Some(HirRequiredName::Invalid))
        }
    }
}

fn source_identity_issue(
    attached: &AttachedSourceDeclaration,
    id: Option<&HirSourceId>,
    name: Option<&HirRequiredName>,
) -> Option<HirItemIssue> {
    match attached.name() {
        AttachedSourceName::Missing(_) => return Some(HirItemIssue::MissingName),
        AttachedSourceName::Authored { value: Err(_), .. } => {
            return Some(HirItemIssue::MalformedHeader);
        }
        AttachedSourceName::Absent | AttachedSourceName::Authored { .. } => {}
    }
    if id.is_some_and(|id| !id.is_canonical_source_family()) {
        return Some(HirItemIssue::MalformedHeader);
    }
    if id.is_some_and(HirSourceId::has_recovery) {
        return Some(HirItemIssue::Recovery);
    }
    if attached.id().requires_name() && name.is_none() {
        return Some(HirItemIssue::MissingName);
    }
    if id.is_none() && name.is_none() {
        return Some(HirItemIssue::MissingName);
    }
    None
}

fn lower_source_replay(
    attached: &AttachedSourceReplayPolicy,
) -> Result<HirSourceReplayValue, HirLowerFailure> {
    Ok(match attached {
        AttachedSourceReplayPolicy::Full(expression) => {
            known_replay_policy(expression, HirSourceReplayPolicy::Full)
        }
        AttachedSourceReplayPolicy::HashOnly(expression) => {
            known_replay_policy(expression, HirSourceReplayPolicy::HashOnly)
        }
        AttachedSourceReplayPolicy::Summary(expression) => {
            known_replay_policy(expression, HirSourceReplayPolicy::Summary)
        }
        AttachedSourceReplayPolicy::EventOnly(expression) => {
            known_replay_policy(expression, HirSourceReplayPolicy::EventOnly)
        }
        AttachedSourceReplayPolicy::None(expression) => {
            known_replay_policy(expression, HirSourceReplayPolicy::None)
        }
        AttachedSourceReplayPolicy::Missing(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
        AttachedSourceReplayPolicy::Unknown { value, .. } => {
            return recovered_replay_policy_name(value.as_ref());
        }
        AttachedSourceReplayPolicy::Invalid(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

fn lower_source_privacy(
    attached: &AttachedSourcePrivacyPolicy,
) -> Result<HirSourcePrivacyValue, HirLowerFailure> {
    Ok(match attached {
        AttachedSourcePrivacyPolicy::Transient(expression) => {
            known_privacy_policy(expression, HirSourcePrivacyPolicy::Transient)
        }
        AttachedSourcePrivacyPolicy::Redacted(expression) => {
            known_privacy_policy(expression, HirSourcePrivacyPolicy::Redacted)
        }
        AttachedSourcePrivacyPolicy::Recordable(expression) => {
            known_privacy_policy(expression, HirSourcePrivacyPolicy::Recordable)
        }
        AttachedSourcePrivacyPolicy::Private(expression) => {
            known_privacy_policy(expression, HirSourcePrivacyPolicy::Private)
        }
        AttachedSourcePrivacyPolicy::Missing(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
        AttachedSourcePrivacyPolicy::Unknown { value, .. } => {
            return recovered_privacy_policy_name(value.as_ref());
        }
        AttachedSourcePrivacyPolicy::Invalid(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

fn lower_source_overflow(
    attached: &AttachedSourceOverflowPolicy,
) -> Result<HirSourceBoundedArgument<HirSourceOverflowValue>, HirLowerFailure> {
    let (value, duplicate) = match attached {
        AttachedSourceOverflowPolicy::DropOldest(argument) => (
            known_overflow_policy(argument, HirSourceOverflowPolicy::DropOldest),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::DropNewest(argument) => (
            known_overflow_policy(argument, HirSourceOverflowPolicy::DropNewest),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Error(argument) => (
            known_overflow_policy(argument, HirSourceOverflowPolicy::Error),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Coalesce(argument) => (
            known_overflow_policy(argument, HirSourceOverflowPolicy::Coalesce),
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Missing => (
            HirSourceOverflowValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Missing,
            },
            false,
        ),
        AttachedSourceOverflowPolicy::Unknown { argument, value } => (
            recovered_overflow_policy_name(value.as_ref())?,
            argument.is_duplicate(),
        ),
        AttachedSourceOverflowPolicy::Invalid(argument) => (
            HirSourceOverflowValue::Recovered {
                authored: None,
                issue: HirSourcePolicyIssue::Invalid,
            },
            argument.is_duplicate(),
        ),
    };
    Ok(HirSourceBoundedArgument::new(value, duplicate))
}

fn known_backpressure_policy(
    expression: &AttachedSourceExpression,
    policy: HirSourceBackpressurePolicy,
) -> HirSourceBackpressureValue {
    match expression {
        AttachedSourceExpression::Authored(_) => HirSourceBackpressureValue::Resolved(policy),
        AttachedSourceExpression::Recovered(_) => HirSourceBackpressureValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        AttachedSourceExpression::Missing(_) => HirSourceBackpressureValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn known_replay_policy(
    expression: &AttachedSourceExpression,
    policy: HirSourceReplayPolicy,
) -> HirSourceReplayValue {
    match expression {
        AttachedSourceExpression::Authored(_) => HirSourceReplayValue::Resolved(policy),
        AttachedSourceExpression::Recovered(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        AttachedSourceExpression::Missing(_) => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn known_privacy_policy(
    expression: &AttachedSourceExpression,
    policy: HirSourcePrivacyPolicy,
) -> HirSourcePrivacyValue {
    match expression {
        AttachedSourceExpression::Authored(_) => HirSourcePrivacyValue::Resolved(policy),
        AttachedSourceExpression::Recovered(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        AttachedSourceExpression::Missing(_) => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn known_overflow_policy(
    argument: &AttachedSourceBoundedArgument,
    policy: HirSourceOverflowPolicy,
) -> HirSourceOverflowValue {
    match argument.value() {
        Some(AttachedSourceExpression::Authored(_)) => HirSourceOverflowValue::Resolved(policy),
        Some(AttachedSourceExpression::Recovered(_)) => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
        Some(AttachedSourceExpression::Missing(_)) | None => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Missing,
        },
    }
}

fn recovered_policy_name(
    value: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> Result<HirSourceBackpressureValue, HirLowerFailure> {
    Ok(match value {
        Some(value) => HirSourceBackpressureValue::Recovered {
            authored: Some(name_projection::name(value)?),
            issue: HirSourcePolicyIssue::Unsupported,
        },
        None => HirSourceBackpressureValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

fn recovered_replay_policy_name(
    value: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> Result<HirSourceReplayValue, HirLowerFailure> {
    Ok(match value {
        Some(value) => HirSourceReplayValue::Recovered {
            authored: Some(name_projection::name(value)?),
            issue: HirSourcePolicyIssue::Unsupported,
        },
        None => HirSourceReplayValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

fn recovered_privacy_policy_name(
    value: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> Result<HirSourcePrivacyValue, HirLowerFailure> {
    Ok(match value {
        Some(value) => HirSourcePrivacyValue::Recovered {
            authored: Some(name_projection::name(value)?),
            issue: HirSourcePolicyIssue::Unsupported,
        },
        None => HirSourcePrivacyValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

fn recovered_overflow_policy_name(
    value: Option<&arcweft_lang_syntax::name::SyntaxName>,
) -> Result<HirSourceOverflowValue, HirLowerFailure> {
    Ok(match value {
        Some(value) => HirSourceOverflowValue::Recovered {
            authored: Some(name_projection::name(value)?),
            issue: HirSourcePolicyIssue::Unsupported,
        },
        None => HirSourceOverflowValue::Recovered {
            authored: None,
            issue: HirSourcePolicyIssue::Invalid,
        },
    })
}

const fn punctuation(attached: &AttachedSourcePunctuation) -> HirSourcePunctuationState {
    if attached.is_missing() {
        HirSourcePunctuationState::Missing
    } else {
        HirSourcePunctuationState::Present
    }
}

const fn source_child_state(attached: &AttachedSourceExpression) -> HirSourceChildState {
    match attached {
        AttachedSourceExpression::Authored(_) => HirSourceChildState::Authored,
        AttachedSourceExpression::Recovered(_) => HirSourceChildState::Invalid,
        AttachedSourceExpression::Missing(_) => HirSourceChildState::Missing,
    }
}

fn required_slot<T>(
    selected: Option<SelectedHeader<T>>,
    issue: &mut Option<HirItemIssue>,
) -> HirSourceRequiredSlot<T> {
    let Some(selected) = selected else {
        issue.get_or_insert(HirItemIssue::InvalidMember);
        return HirSourceRequiredSlot::Missing;
    };
    HirSourceRequiredSlot::authored(selected.value, selected.duplicate)
}

fn missing_headers() -> HirSourceHeaders {
    HirSourceHeaders::new(
        HirSourceRequiredSlot::Missing,
        HirSourceRequiredSlot::Missing,
        HirSourceRequiredSlot::Missing,
        HirSourceRequiredSlot::Missing,
    )
}

pub(super) fn preflight_source_members(member_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::Statements, member_count)
}
