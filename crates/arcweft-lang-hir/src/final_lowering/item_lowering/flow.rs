//! Final attached lowering for ordinary `flow` declarations.

use arcweft_lang_syntax::attachment::node::FlowItemKind;
use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCallableParameterKind, AttachedFlowContractClause, AttachedFlowContractMode,
    AttachedFlowContractOperands, AttachedFlowIdSyntax, AttachedFlowIdentity,
    AttachedFlowReturnSyntax, AttachedGenericParameter, AttachedRequiredFlowBody,
};

use crate::expr::{HirThreadFlowItem, HirThreadIssue};
use crate::identity::{ItemId, LocalId, ScopeId, SyntheticOwner};
use crate::item::{
    HirContractCondition, HirContractMode, HirContractOperandList, HirFlowContractClause,
    HirFlowIdentity, HirFlowIssue, HirFlowIssueClass, HirFlowIssueOwner, HirFlowItem,
    HirFlowPoison, HirFlowResultLocal, HirFlowReturn, HirGenericParameter, HirItem, HirItemIssue,
    HirItemKind, HirParameter, HirParameterKind, HirWherePredicate,
};
use crate::leaf::{
    HirFamilyRelativeId, HirIdFamily, HirIdRef, HirIdRefValue, HirIdSuffix, HirName, HirRelativeId,
};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind};
use crate::source_index::{
    HirFlowContractSourcePart, HirFlowParameterSourcePart, HirFlowSourceRole, HirItemSourceRole,
    HirSourceQuery, HirSourceSite, HirThreadBodySourceRole, HirThreadFlowItemSourcePart,
};

use super::super::{StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state};

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_flow_declaration(
        &mut self,
        owner: ItemId,
        parent_scope: ScopeId,
        node: &AstNode<FlowItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), parent_scope)?;

        let callable_scope = self.allocate_item_callable_scope(node, owner, parent_scope)?;
        let requires_site = attached
            .contracts()
            .iter()
            .find(|clause| !matches!(clause, AttachedFlowContractClause::Ensures { .. }))
            .map(AttachedFlowContractClause::keyword)
            .map(|source| self.attached_component_site(source))
            .transpose()?
            .unwrap_or(self.attached_insertion_site(attached.signature().end().clone())?);
        let ensures_site = attached
            .contracts()
            .iter()
            .find(|clause| matches!(clause, AttachedFlowContractClause::Ensures { .. }))
            .map(AttachedFlowContractClause::keyword)
            .map(|source| self.attached_component_site(source))
            .transpose()?
            .unwrap_or(self.attached_insertion_site(attached.signature().end().clone())?);
        let contract_scopes =
            self.allocate_item_contract_scopes(owner, callable_scope, requires_site, ensures_site)?;
        let body_scope = match attached.body() {
            AttachedRequiredFlowBody::Present(body) => self.allocate_item_body_scope(
                body.syntax(),
                owner,
                callable_scope,
                HirScopeKind::Flow,
            )?,
            AttachedRequiredFlowBody::Missing { syntax, .. } => {
                let source_site = HirSourceSite::from_attached_span(
                    self.request.source().document(),
                    &syntax.source_span(),
                )
                .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
                self.allocate_item_body_scope_from_syntax_at_site(
                    &syntax.syntax(),
                    owner,
                    callable_scope,
                    HirScopeKind::Flow,
                    source_site,
                )?
            }
        };

        let (identity, identity_issues) = lower_flow_identity(owner, attached.identity())?;
        let (generic_parameters, _) =
            self.lower_generic_parameters(attached.signature().generics(), callable_scope)?;
        let generic_issues =
            self.flow_generic_issues(owner, attached.signature().generics(), &generic_parameters)?;
        let (parameters, parameter_locals, parameter_issues) =
            self.lower_flow_parameters(owner, attached.signature().parameters(), callable_scope)?;
        self.close_scope_members(callable_scope, parameter_locals)?;

        let (result, result_issues) = match attached.signature().result() {
            AttachedFlowReturnSyntax::Omitted => (HirFlowReturn::OmittedUnit, Vec::new()),
            AttachedFlowReturnSyntax::Authored(authored) => {
                let ty = self.lower_attached_type(authored.ty(), callable_scope)?;
                let poisoned = authored.has_recovery() || self.staged_type_is_poisoned(ty)?;
                (
                    HirFlowReturn::Authored(ty),
                    poisoned
                        .then(|| {
                            flow_owned_issue(
                                owner,
                                HirFlowIssueClass::Signature,
                                HirFlowIssueOwner::Type(ty),
                                HirFlowSourceRole::Return {
                                    part: crate::source_index::HirFlowReturnSourcePart::Type,
                                },
                            )
                        })
                        .into_iter()
                        .collect(),
                )
            }
        };
        let where_clauses = attached
            .signature()
            .where_clause()
            .map(std::slice::from_ref)
            .unwrap_or(&[]);
        let (where_predicates, _) = self.lower_where_clauses(where_clauses, callable_scope)?;
        let where_issues = self.flow_where_issues(
            owner,
            attached.signature().where_clause(),
            &where_predicates,
        )?;

        let has_ensures = attached
            .contracts()
            .iter()
            .any(|clause| matches!(clause, AttachedFlowContractClause::Ensures { .. }));
        let result_local = has_ensures
            .then(|| {
                self.allocate_postcondition_result_local(
                    contract_scopes.ensures(),
                    result.authored_type(),
                    attached.signature().end().clone(),
                )
                .map(HirFlowResultLocal::new)
            })
            .transpose()?;
        self.close_scope_members(contract_scopes.requires(), Box::new([]))?;
        self.close_scope_members(
            contract_scopes.ensures(),
            result_local
                .map(HirFlowResultLocal::local)
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )?;

        let (contracts, contract_issues) =
            self.lower_flow_contracts(owner, attached.contracts(), contract_scopes)?;
        let lowered_body = self.lower_attached_flow_body(attached.body(), owner, body_scope)?;

        let mut issues = Vec::new();
        if prefix.issue.is_some() {
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Prefix,
                HirFlowSourceRole::Whole,
            ));
        }
        issues.extend(identity_issues);
        issues.extend(generic_issues);
        issues.extend(parameter_issues);
        for position in 0..attached.signature().recovery().len() {
            issues.push(signature_recovery_issue(owner, position)?);
        }
        issues.extend(result_issues);
        issues.extend(where_issues);
        issues.extend(contract_issues);
        for recovery in &lowered_body.recoveries {
            issues.push(flow_body_issue(owner, &lowered_body.body, recovery)?);
        }
        let trailing_base = u32::try_from(attached.signature().recovery().len())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        for position in 0..attached.trailing_recovery().len() {
            let position =
                u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let ordinal = trailing_base
                .checked_add(position)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::TrailingRecovery,
                HirFlowSourceRole::TrailingRecovery { ordinal },
            ));
        }
        let poison = if issues.is_empty() {
            HirFlowPoison::clean()
        } else {
            HirFlowPoison::from_ordered_issues(issues.into_boxed_slice())
        };
        let item_issue = poison.primary().map(|issue| match issue.class() {
            HirFlowIssueClass::MissingBody => HirItemIssue::MissingBody,
            HirFlowIssueClass::Prefix
            | HirFlowIssueClass::Identity
            | HirFlowIssueClass::Signature => HirItemIssue::MalformedHeader,
            HirFlowIssueClass::Contract
            | HirFlowIssueClass::BodyChild
            | HirFlowIssueClass::UnclosedBody
            | HirFlowIssueClass::TrailingRecovery => HirItemIssue::Recovery,
        });

        let declaration = HirFlowItem::try_new(
            owner,
            identity,
            generic_parameters,
            parameters,
            result,
            where_predicates,
            contract_scopes,
            result_local,
            contracts,
            lowered_body.body,
            poison,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components
            .stage_attached_flow(self.request.source(), owner, &attached)?;
        let item = HirItem::try_new_with_state(
            owner,
            parent_scope,
            prefix.value,
            HirItemKind::Flow(declaration),
            Box::new([]),
            item_state(item_issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    fn lower_flow_parameters(
        &mut self,
        owner: ItemId,
        group: Option<&arcweft_lang_syntax::attachment::AttachedFixedParameterGroup>,
        callable_scope: ScopeId,
    ) -> Result<(Box<[HirParameter]>, Box<[LocalId]>, Vec<HirFlowIssue>), HirLowerFailure> {
        let Some(group) = group else {
            return Ok((Box::new([]), Box::new([]), Vec::new()));
        };
        if group.source_ordinal() != 0 {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let mut issues = Vec::new();
        if group.open_state().is_missing() {
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Signature,
                HirFlowSourceRole::ParameterGroup,
            ));
        }
        let mut parameters = Vec::with_capacity(group.parameters().len());
        let mut locals = Vec::new();
        for (position, parameter) in group.parameters().iter().enumerate() {
            let ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if parameter.source_ordinal() != ordinal
                || parameter.group_ordinal() != 0
                || parameter.parameter_ordinal() != ordinal
            {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
            let pattern = self.lower_attached_pattern_binding(
                parameter.pattern(),
                callable_scope,
                HirPatternBindingPolicy::FlowParameter,
            )?;
            let structural_recovery =
                !matches!(parameter.kind(), AttachedCallableParameterKind::Fixed)
                    || parameter.default().is_some();
            if structural_recovery {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowSourceRole::Parameter {
                        ordinal,
                        part: HirFlowParameterSourcePart::Whole,
                    },
                ));
            }
            if pattern.poisoned {
                issues.push(flow_owned_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowIssueOwner::Pattern(pattern.owner),
                    HirFlowSourceRole::Parameter {
                        ordinal,
                        part: HirFlowParameterSourcePart::Pattern,
                    },
                ));
            }
            if parameter.colon().is_missing() {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowSourceRole::Parameter {
                        ordinal,
                        part: HirFlowParameterSourcePart::Colon,
                    },
                ));
            }
            let type_poisoned = self.staged_type_is_poisoned(ty)?;
            if type_poisoned {
                issues.push(flow_owned_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowIssueOwner::Type(ty),
                    HirFlowSourceRole::Parameter {
                        ordinal,
                        part: HirFlowParameterSourcePart::Type,
                    },
                ));
            }
            locals.extend_from_slice(&pattern.locals);
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
        if group.close_state().is_missing() {
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Signature,
                HirFlowSourceRole::ParameterGroup,
            ));
        }
        require_limit(crate::identity::HirLimit::LocalsPerScope, locals.len())?;
        Ok((
            parameters.into_boxed_slice(),
            locals.into_boxed_slice(),
            issues,
        ))
    }

    fn flow_generic_issues(
        &mut self,
        owner: ItemId,
        attached: Option<&arcweft_lang_syntax::attachment::AttachedGenericParameterGroup>,
        lowered: &[HirGenericParameter],
    ) -> Result<Vec<HirFlowIssue>, HirLowerFailure> {
        let Some(attached) = attached else {
            if lowered.is_empty() {
                return Ok(Vec::new());
            }
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        if attached.parameters().len() != lowered.len() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let mut issues = Vec::new();
        for (position, (attached, lowered)) in attached.parameters().iter().zip(lowered).enumerate()
        {
            let ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let role = HirFlowSourceRole::GenericParameter { ordinal };
            let mut represented = false;
            if attached.name().is_missing() {
                issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                represented = true;
            }
            if matches!(
                attached,
                AttachedGenericParameter::Type {
                    colon: Some(colon),
                    ..
                } if colon.is_missing()
            ) {
                issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                represented = true;
            }
            if attached.bounds().len() != lowered.bounds().len() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            for bound in lowered.bounds() {
                if self.staged_type_is_poisoned(*bound)? {
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowIssueOwner::Type(*bound),
                        role,
                    ));
                    represented = true;
                }
            }
            if attached.has_recovery() && !represented {
                issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
            }
        }
        if matches!(attached.close_state(), AttachedDelimiterState::Missing(_)) {
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Signature,
                HirFlowSourceRole::GenericGroup,
            ));
        }
        Ok(issues)
    }

    fn flow_where_issues(
        &mut self,
        owner: ItemId,
        attached: Option<&arcweft_lang_syntax::attachment::AttachedWhereClause>,
        lowered: &[HirWherePredicate],
    ) -> Result<Vec<HirFlowIssue>, HirLowerFailure> {
        let Some(attached) = attached else {
            if lowered.is_empty() {
                return Ok(Vec::new());
            }
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        };
        if attached.predicates().len() != lowered.len() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let mut issues = Vec::new();
        for (position, (attached, lowered)) in attached.predicates().iter().zip(lowered).enumerate()
        {
            let ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let role = HirFlowSourceRole::WherePredicate { ordinal };
            let mut represented = false;
            if self.staged_type_is_poisoned(lowered.subject())? {
                issues.push(flow_owned_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowIssueOwner::Type(lowered.subject()),
                    role,
                ));
                represented = true;
            }
            if attached.colon().is_missing() {
                issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                represented = true;
            }
            if attached.bounds().len() != lowered.bounds().len() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            for bound in lowered.bounds() {
                if self.staged_type_is_poisoned(*bound)? {
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowIssueOwner::Type(*bound),
                        role,
                    ));
                    represented = true;
                }
            }
            if attached.has_recovery() && !represented {
                issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
            }
        }
        Ok(issues)
    }

    fn lower_flow_contracts(
        &mut self,
        owner: ItemId,
        attached: &[AttachedFlowContractClause],
        scopes: crate::item::HirContractScopes,
    ) -> Result<(Box<[HirFlowContractClause]>, Vec<HirFlowIssue>), HirLowerFailure> {
        let mut contracts = Vec::with_capacity(attached.len());
        let mut issues = Vec::new();
        let mut first_decreases = None;
        for (position, clause) in attached.iter().enumerate() {
            let ordinal =
                u16::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if clause.source_ordinal() != ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let scope = if matches!(clause, AttachedFlowContractClause::Ensures { .. }) {
                scopes.ensures()
            } else {
                scopes.requires()
            };
            let (contract, expression_ids) = match clause {
                AttachedFlowContractClause::Requires { condition, .. } => {
                    let expression =
                        self.lower_attached_expression(condition.expression(), scope)?;
                    (
                        HirFlowContractClause::Requires(HirContractCondition::new(
                            lower_contract_mode(condition.mode()),
                            expression,
                        )),
                        vec![expression],
                    )
                }
                AttachedFlowContractClause::Ensures { condition, .. } => {
                    let expression =
                        self.lower_attached_expression(condition.expression(), scope)?;
                    (
                        HirFlowContractClause::Ensures(HirContractCondition::new(
                            lower_contract_mode(condition.mode()),
                            expression,
                        )),
                        vec![expression],
                    )
                }
                AttachedFlowContractClause::Invariant { condition, .. } => {
                    let expression =
                        self.lower_attached_expression(condition.expression(), scope)?;
                    (
                        HirFlowContractClause::Invariant(HirContractCondition::new(
                            lower_contract_mode(condition.mode()),
                            expression,
                        )),
                        vec![expression],
                    )
                }
                AttachedFlowContractClause::Assume { expression, .. } => {
                    let expression = self.lower_attached_expression(expression, scope)?;
                    (
                        HirFlowContractClause::Assume { expression },
                        vec![expression],
                    )
                }
                AttachedFlowContractClause::Reads { operands, .. } => {
                    let operands =
                        self.lower_flow_contract_operand_list(operands.operands(), scope)?;
                    let expression_ids = operands.operands().to_vec();
                    (HirFlowContractClause::Reads(operands), expression_ids)
                }
                AttachedFlowContractClause::Effects { operands, .. } => {
                    let operands =
                        self.lower_flow_contract_operand_list(operands.operands(), scope)?;
                    let expression_ids = operands.operands().to_vec();
                    (HirFlowContractClause::Effects(operands), expression_ids)
                }
                AttachedFlowContractClause::NoEffect { expression, .. } => {
                    let expression = self.lower_attached_expression(expression, scope)?;
                    (
                        HirFlowContractClause::NoEffect { expression },
                        vec![expression],
                    )
                }
                AttachedFlowContractClause::Modifies { operands, .. } => {
                    let operands =
                        self.lower_flow_contract_operand_list(operands.operands(), scope)?;
                    let expression_ids = operands.operands().to_vec();
                    (HirFlowContractClause::Modifies(operands), expression_ids)
                }
                AttachedFlowContractClause::Decreases { expression, .. } => {
                    let expression = self.lower_attached_expression(expression, scope)?;
                    (
                        HirFlowContractClause::Decreases { expression },
                        vec![expression],
                    )
                }
            };

            let issue_start = issues.len();
            if matches!(clause, AttachedFlowContractClause::Decreases { .. }) {
                if let Some(first) = first_decreases {
                    // The later duplicate is the diagnostic primary and the
                    // first syntactic decreases remains its related authority.
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Contract,
                        HirFlowSourceRole::ContractClause {
                            ordinal,
                            part: HirFlowContractSourcePart::ClauseKeyword,
                        },
                    ));
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Contract,
                        HirFlowSourceRole::ContractClause {
                            ordinal: first,
                            part: HirFlowContractSourcePart::ClauseKeyword,
                        },
                    ));
                } else {
                    first_decreases = Some(ordinal);
                }
            }

            if clause
                .list()
                .and_then(|list| list.open())
                .is_some_and(|open| open.range().is_empty())
            {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Contract,
                    HirFlowSourceRole::ContractClause {
                        ordinal,
                        part: HirFlowContractSourcePart::OpenDelimiter,
                    },
                ));
            }
            let attached_operands = match clause.operands() {
                AttachedFlowContractOperands::One(expression) => vec![expression],
                AttachedFlowContractOperands::Many(expressions) => expressions.iter().collect(),
            };
            if attached_operands.len() != expression_ids.len() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            for (position, (attached, expression)) in attached_operands
                .into_iter()
                .zip(expression_ids.iter().copied())
                .enumerate()
            {
                if attached.projection().has_recovery()
                    || self.staged_expression_is_poisoned(expression)?
                {
                    let operand = u16::try_from(position)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Contract,
                        HirFlowIssueOwner::Expr(expression),
                        HirFlowSourceRole::ContractClause {
                            ordinal,
                            part: HirFlowContractSourcePart::Operand { ordinal: operand },
                        },
                    ));
                }
            }
            if matches!(
                clause.list().and_then(|list| list.close_state()),
                Some(AttachedDelimiterState::Missing(_))
            ) {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Contract,
                    HirFlowSourceRole::ContractClause {
                        ordinal,
                        part: HirFlowContractSourcePart::CloseDelimiter,
                    },
                ));
            }
            if clause.has_recovery() && issues.len() == issue_start {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Contract,
                    HirFlowSourceRole::ContractClause {
                        ordinal,
                        part: HirFlowContractSourcePart::Whole,
                    },
                ));
            }
            contracts.push(contract);
        }
        Ok((contracts.into_boxed_slice(), issues))
    }

    fn lower_flow_contract_operand_list(
        &mut self,
        attached: &[arcweft_lang_syntax::attachment::AttachedExpressionNode],
        scope: ScopeId,
    ) -> Result<HirContractOperandList, HirLowerFailure> {
        let operands = attached
            .iter()
            .map(|operand| self.lower_attached_expression(operand, scope))
            .collect::<Result<Vec<_>, _>>()?;
        let operands = HirContractOperandList::try_new(scope.module(), operands.into_boxed_slice())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(operands)
    }

    fn attached_component_site(
        &self,
        source: &arcweft_source::SourceSpan,
    ) -> Result<crate::source_index::HirSourceSite, HirLowerFailure> {
        crate::source_index::HirSourceSite::from_attached_span(
            self.request.source().document(),
            source,
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan.into())
    }
}

fn lower_flow_identity(
    owner: ItemId,
    attached: &AttachedFlowIdentity,
) -> Result<(HirFlowIdentity, Vec<HirFlowIssue>), HirLowerFailure> {
    match attached {
        AttachedFlowIdentity::Name { name } => {
            let name = lower_flow_name(name.value().as_str())?;
            Ok((HirFlowIdentity::Name { name }, Vec::new()))
        }
        AttachedFlowIdentity::PublicId { public_id } => {
            let (public_id, recovered) = lower_flow_public_id(public_id, None)?;
            let issues = recovered
                .then(|| {
                    flow_item_issue(
                        owner,
                        HirFlowIssueClass::Identity,
                        HirFlowSourceRole::PublicId,
                    )
                })
                .into_iter()
                .collect();
            Ok((
                public_id
                    .map(|public_id| HirFlowIdentity::PublicId { public_id })
                    .unwrap_or(HirFlowIdentity::Missing),
                issues,
            ))
        }
        AttachedFlowIdentity::PublicIdAndName { public_id, name } => {
            let name = lower_flow_name(name.value().as_str())?;
            let (public_id, recovered) = lower_flow_public_id(public_id, Some(&name))?;
            if let Some(public_id) = public_id {
                let mut issues = Vec::new();
                if recovered {
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Identity,
                        HirFlowSourceRole::PublicId,
                    ));
                } else if !flow_id_matches_name(&public_id, &name) {
                    // ID/name mismatch is the explicit exception to ordinary
                    // source ordering: Name is primary and PublicId related.
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Identity,
                        HirFlowSourceRole::Name,
                    ));
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Identity,
                        HirFlowSourceRole::PublicId,
                    ));
                }
                Ok((HirFlowIdentity::PublicIdAndName { public_id, name }, issues))
            } else {
                Ok((
                    HirFlowIdentity::Name { name },
                    vec![flow_item_issue(
                        owner,
                        HirFlowIssueClass::Identity,
                        HirFlowSourceRole::PublicId,
                    )],
                ))
            }
        }
        AttachedFlowIdentity::Missing {
            attempted_public_id,
            ..
        } => {
            let mut issues = Vec::new();
            if attempted_public_id.is_some() {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Identity,
                    HirFlowSourceRole::PublicId,
                ));
            }
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Identity,
                HirFlowSourceRole::Name,
            ));
            Ok((HirFlowIdentity::Missing, issues))
        }
    }
}

fn lower_flow_public_id(
    attached: &arcweft_lang_syntax::attachment::AttachedFlowPublicId,
    name: Option<&HirName>,
) -> Result<(Option<HirIdRef>, bool), HirLowerFailure> {
    let recovered = attached.has_recovery();
    match attached.value() {
        AttachedFlowIdSyntax::Authored(value) => {
            let value = super::super::id_ref_projection::id_ref(value)?;
            match value {
                HirIdRefValue::Resolved(value) => Ok((Some(value), recovered)),
                HirIdRefValue::Recovered(_) => Ok((None, true)),
            }
        }
        AttachedFlowIdSyntax::DerivedFromEmptyMarker { marker_family } => {
            let Some(name) = name else {
                return Ok((None, true));
            };
            let suffix = HirIdSuffix::try_new(name.as_str().into())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let relative = HirRelativeId::new(suffix, 0);
            let id = match marker_family {
                Some(family) => {
                    let family = HirIdFamily::try_new(family.as_str().into())
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                    HirIdRef::family_relative(HirFamilyRelativeId::new(family, relative))
                }
                None => HirIdRef::relative(relative),
            };
            Ok((Some(id), recovered))
        }
    }
}

fn lower_flow_name(value: &str) -> Result<HirName, HirLowerFailure> {
    require_limit(crate::identity::HirLimit::NameBytes, value.len())?;
    HirName::try_new(value.into()).map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
}

fn flow_id_matches_name(public_id: &HirIdRef, name: &HirName) -> bool {
    let suffix = match public_id {
        HirIdRef::Absolute(reference) => reference.as_str(),
        HirIdRef::Relative(relative) => relative.suffix().as_str(),
        HirIdRef::FamilyRelative(relative) => relative.relative().suffix().as_str(),
    };
    suffix.rsplit('.').next() == Some(name.as_str())
}

const fn lower_contract_mode(mode: &AttachedFlowContractMode) -> HirContractMode {
    match mode {
        AttachedFlowContractMode::Default => HirContractMode::Default,
        AttachedFlowContractMode::Prove(_) => HirContractMode::Prove,
        AttachedFlowContractMode::Check(_) => HirContractMode::CheckRuntime,
        AttachedFlowContractMode::Debug(_) => HirContractMode::DebugCheck,
    }
}

fn flow_item_issue(
    owner: ItemId,
    class: HirFlowIssueClass,
    role: HirFlowSourceRole,
) -> HirFlowIssue {
    flow_owned_issue(owner, class, HirFlowIssueOwner::Item(owner), role)
}

fn flow_owned_issue(
    owner: ItemId,
    class: HirFlowIssueClass,
    issue_owner: HirFlowIssueOwner,
    role: HirFlowSourceRole,
) -> HirFlowIssue {
    HirFlowIssue::new(
        class,
        issue_owner,
        HirSourceQuery::Item {
            owner,
            role: HirItemSourceRole::Flow(role),
        },
    )
}

fn signature_recovery_issue(
    owner: ItemId,
    position: usize,
) -> Result<HirFlowIssue, HirLowerFailure> {
    let ordinal = u32::try_from(position).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
    Ok(flow_item_issue(
        owner,
        HirFlowIssueClass::Signature,
        HirFlowSourceRole::TrailingRecovery { ordinal },
    ))
}

fn flow_body_issue(
    owner: ItemId,
    body: &crate::expr::HirThreadBody,
    issue: &HirThreadIssue,
) -> Result<HirFlowIssue, HirLowerFailure> {
    Ok(match issue {
        HirThreadIssue::MissingBody => flow_item_issue(
            owner,
            HirFlowIssueClass::MissingBody,
            HirFlowSourceRole::Body,
        ),
        HirThreadIssue::UnclosedBody => flow_item_issue(
            owner,
            HirFlowIssueClass::UnclosedBody,
            HirFlowSourceRole::BodyClose,
        ),
        HirThreadIssue::RecoveredBodyChild { ordinal } => {
            let item = body
                .items()
                .get(
                    usize::try_from(*ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                )
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
            HirFlowIssue::new(
                HirFlowIssueClass::BodyChild,
                flow_child_issue_owner(item),
                HirSourceQuery::ThreadBody {
                    owner: crate::expr::HirThreadBodyOwner::Flow(owner),
                    role: HirThreadBodySourceRole::Item {
                        ordinal: *ordinal,
                        part: HirThreadFlowItemSourcePart::ChildWhole,
                    },
                },
            )
        }
        HirThreadIssue::InvalidName
        | HirThreadIssue::DetachedBorrowedCapture { .. }
        | HirThreadIssue::DetachedEphemeralRegistryAccess => {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
    })
}

fn flow_child_issue_owner(item: &HirThreadFlowItem) -> HirFlowIssueOwner {
    match item.owner() {
        SyntheticOwner::Stmt(owner) => HirFlowIssueOwner::Stmt(owner),
        SyntheticOwner::Expr(owner) => HirFlowIssueOwner::Expr(owner),
        SyntheticOwner::Item(owner) => HirFlowIssueOwner::Item(owner),
        SyntheticOwner::Scope(owner) => HirFlowIssueOwner::Scope(owner),
        SyntheticOwner::Local(owner) => HirFlowIssueOwner::Local(owner),
        SyntheticOwner::Pattern(owner) => HirFlowIssueOwner::Pattern(owner),
        SyntheticOwner::Type(owner) => HirFlowIssueOwner::Type(owner),
        SyntheticOwner::Capture(_) => {
            unreachable!("Thread/Flow item roots cannot be capture-owned")
        }
    }
}
