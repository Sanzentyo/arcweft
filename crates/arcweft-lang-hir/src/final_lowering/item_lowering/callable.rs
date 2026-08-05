//! Final callable-item lowering shared by retained and ordinary declarations.

use arcweft_lang_syntax::attachment::node::{FunctionItemKind, PredicateItemKind, ProofItemKind};
use arcweft_lang_syntax::attachment::{
    AstKind, AstNode, AttachedCallableContractClause, AttachedCallableParameterKind,
    AttachedFixedParameterGroup, AttachedFunctionBody, AttachedPredicateBody, AttachedProofBody,
    SyntaxNodeHandle,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_source::SourceSpan;

use crate::expr::HirPoisonState;
use crate::identity::{
    HirLimit, ItemId, LocalId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole, TypeId,
};
use crate::item::{
    HirCallableSignature, HirContractScopes, HirFunctionBody, HirFunctionItem,
    HirFunctionParameterGroup, HirFunctionSignature, HirItem, HirItemIssue, HirItemKind,
    HirParameter, HirParameterKind, HirPredicate, HirPredicateBody, HirProof, HirProofBody,
};
use crate::leaf::{HirName, HirPath, HirPathRoot, HirPathSegment};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{
    HirLocal, HirLocalKind, HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner,
};
use crate::source_index::HirSourceSite;
use crate::type_ref::{HirType, HirTypeKind};

use super::super::{LocalGenerationLedgerEntry, StagedHirModuleTransaction, require_limit};
use super::{LoweredItemProjection, item_state, project_required_name};

pub(super) struct LoweredFunctionParameterGroups {
    pub(super) groups: Box<[HirFunctionParameterGroup]>,
    pub(super) missing_type: bool,
    pub(super) recovery: bool,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_function_parameter_groups(
        &mut self,
        groups: &[AttachedFixedParameterGroup],
        callable_scope: ScopeId,
        has_shape_recovery: bool,
    ) -> Result<LoweredFunctionParameterGroups, HirLowerFailure> {
        let mut recovery =
            groups.iter().any(AttachedFixedParameterGroup::has_recovery) || has_shape_recovery;
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
                let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
                let pattern = self.lower_attached_pattern_binding(
                    parameter.pattern(),
                    callable_scope,
                    HirPatternBindingPolicy::CallableParameter,
                )?;
                let type_poisoned = self.staged_type_is_poisoned(ty)?;
                let kind = match parameter.kind() {
                    AttachedCallableParameterKind::Fixed => HirParameterKind::Fixed,
                    AttachedCallableParameterKind::Rest { .. } => HirParameterKind::RestPositional,
                };
                let default = parameter
                    .default()
                    .map(|default| self.lower_attached_expression(default.value(), callable_scope))
                    .transpose()?;
                let default_poisoned = match default {
                    Some(default) => self.staged_expression_is_poisoned(default)?,
                    None => false,
                };
                missing_type |=
                    type_poisoned && parameter.ty().syntax().kind() == SyntaxKind::MissingType;
                recovery |= pattern.poisoned
                    || type_poisoned
                    || parameter.has_recovery()
                    || default_poisoned;
                callable_locals.extend_from_slice(&pattern.locals);
                parameters.push(
                    HirParameter::try_new(pattern.owner, ty, kind, default, pattern.locals)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                );
            }
            lowered_groups.push(
                HirFunctionParameterGroup::try_new(
                    callable_scope.module(),
                    parameters.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
        }
        require_limit(HirLimit::LocalsPerScope, callable_locals.len())?;
        self.close_scope_members(callable_scope, callable_locals.into_boxed_slice())?;

        Ok(LoweredFunctionParameterGroups {
            groups: lowered_groups.into_boxed_slice(),
            missing_type,
            recovery,
        })
    }

    pub(super) fn allocate_item_callable_scope<K: AstKind>(
        &mut self,
        syntax: &AstNode<K>,
        owner: ItemId,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            syntax.id(),
            HirSourceSite::Span(syntax.source_span()),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                HirScopeKind::Callable,
                Some(parent),
                HirScopeOwner::Item(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }

        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == HirScopeKind::Callable
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Item(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    pub(super) fn allocate_item_body_scope<K: AstKind>(
        &mut self,
        syntax: &AstNode<K>,
        owner: ItemId,
        parent: ScopeId,
        kind: HirScopeKind,
    ) -> Result<ScopeId, HirLowerFailure> {
        self.allocate_item_body_scope_from_syntax(&syntax.syntax(), owner, parent, kind)
    }

    pub(super) fn allocate_item_body_scope_from_syntax(
        &mut self,
        syntax: &SyntaxNodeHandle,
        owner: ItemId,
        parent: ScopeId,
        kind: HirScopeKind,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            syntax.id(),
            HirSourceSite::Span(syntax.source_span()),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                kind,
                Some(parent),
                HirScopeOwner::Item(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }

        let retained = self
            .arenas
            .scopes()
            .resolve_staged(&self.slots, scope)?
            .clone();
        if retained.kind() != kind
            || retained.parent() != Some(parent)
            || retained.owner() != &HirScopeOwner::Item(owner)
        {
            return Err(HirInvariantFailure::InvalidScopeParent.into());
        }
        if !retained.children().is_empty() || !retained.locals().is_empty() {
            let reset = retained
                .try_with_members(Box::new([]), Box::new([]))
                .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .revise_finalized(&mut self.slots, scope, reset)?;
        }
        Ok(scope)
    }

    pub(super) fn allocate_item_contract_scopes(
        &mut self,
        owner: ItemId,
        callable: ScopeId,
        requires_site: HirSourceSite,
        ensures_site: HirSourceSite,
    ) -> Result<HirContractScopes, HirLowerFailure> {
        let requires = self.allocate_item_contract_scope(
            owner,
            callable,
            SyntheticRole::ContractRequiresScope,
            HirScopeKind::ContractRequires,
            requires_site,
        )?;
        let ensures = self.allocate_item_contract_scope(
            owner,
            callable,
            SyntheticRole::ContractEnsuresScope,
            HirScopeKind::ContractEnsures,
            ensures_site,
        )?;
        HirContractScopes::try_new(callable, requires, ensures)
            .map_err(|_| HirInvariantFailure::InvalidScopeParent.into())
    }

    pub(super) fn lower_function_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<FunctionItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;
        let requires_site = self.attached_insertion_site(attached.requires_scope_source_span())?;
        let ensures_site = self.attached_insertion_site(attached.ensures_scope_source_span())?;
        let contract_scopes =
            self.allocate_item_contract_scopes(owner, callable_scope, requires_site, ensures_site)?;
        let body_scope = match attached.body() {
            AttachedFunctionBody::Block { .. } => Some(self.allocate_item_body_scope(
                attached.body().syntax(),
                owner,
                callable_scope,
                HirScopeKind::Block,
            )?),
            AttachedFunctionBody::Missing { .. } => None,
        };

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
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), callable_scope)?;

        let mut requires = Vec::new();
        let mut contract_recovery = false;
        for (position, contract) in attached.contracts().iter().enumerate() {
            if usize::from(contract.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            if !matches!(contract, AttachedCallableContractClause::Requires { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.requires())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            requires.push(condition);
        }
        self.close_scope_members(contract_scopes.requires(), Box::<[LocalId]>::from([]))?;

        let result_local = attached
            .postcondition_result_source_span()
            .map(|source| {
                self.allocate_postcondition_result_local(
                    contract_scopes.ensures(),
                    return_type,
                    source,
                )
            })
            .transpose()?;
        self.close_scope_members(
            contract_scopes.ensures(),
            result_local
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )?;
        let mut ensures = Vec::new();
        for contract in attached.contracts() {
            if !matches!(contract, AttachedCallableContractClause::Ensures { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.ensures())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            ensures.push(condition);
        }

        let (body, body_issue) = match attached.body() {
            AttachedFunctionBody::Block { block, .. } => {
                let body_scope = body_scope.ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let lowered = self.lower_attached_function_block(block, owner, body_scope)?;
                let issue = (attached.body().has_recovery() || lowered.recovery.is_some())
                    .then_some(HirItemIssue::Recovery);
                (
                    HirFunctionBody::Block {
                        scope: lowered.scope,
                        statements: lowered.statements,
                        tail: lowered.tail,
                    },
                    issue,
                )
            }
            AttachedFunctionBody::Missing { missing, .. } => (
                HirFunctionBody::Error(self.lower_missing_required_tail_for_scope(
                    callable_scope,
                    missing.source_span(),
                )?),
                Some(HirItemIssue::MissingBody),
            ),
        };

        let signature = HirFunctionSignature::try_new(
            owner.module(),
            generic_parameters,
            parameter_groups.groups,
            where_predicates,
            requires.into_boxed_slice(),
            ensures.into_boxed_slice(),
            return_type,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let declaration = HirFunctionItem::try_new(name.value, signature, body, contract_scopes)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
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
            .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| contract_recovery.then_some(HirItemIssue::Recovery))
            .or(body_issue)
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Function(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_predicate_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<PredicateItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;
        let requires_site = self.attached_insertion_site(attached.requires_scope_source_span())?;
        let ensures_site = self.attached_insertion_site(attached.ensures_scope_source_span())?;
        let contract_scopes =
            self.allocate_item_contract_scopes(owner, callable_scope, requires_site, ensures_site)?;
        let body_scope = self.allocate_item_body_scope(
            attached.body().syntax(),
            owner,
            callable_scope,
            HirScopeKind::Predicate,
        )?;

        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), callable_scope)?;

        let mut parameter_recovery = attached.parameter_group().has_recovery();
        let mut parameter_missing_type = false;
        let mut parameters = Vec::with_capacity(attached.parameter_group().parameters().len());
        let mut callable_locals = Vec::new();
        for (position, parameter) in attached.parameter_group().parameters().iter().enumerate() {
            if usize::from(parameter.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
            let pattern = self.lower_attached_pattern_binding(
                parameter.pattern(),
                callable_scope,
                HirPatternBindingPolicy::PredicateParameter,
            )?;
            let type_poisoned = self.staged_type_is_poisoned(ty)?;
            parameter_missing_type |=
                type_poisoned && parameter.ty().syntax().kind() == SyntaxKind::MissingType;
            parameter_recovery |= pattern.poisoned
                || type_poisoned
                || parameter.has_recovery()
                || parameter.is_rest()
                || parameter.default().is_some();
            callable_locals.extend_from_slice(&pattern.locals);
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
        self.close_scope_members(callable_scope, callable_locals.into_boxed_slice())?;

        let return_type = self.lower_predicate_bool_return(
            owner,
            callable_scope,
            attached.parameter_group().end_source_span(),
        )?;
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), callable_scope)?;

        let mut requires = Vec::new();
        let mut contract_recovery = false;
        for (position, contract) in attached.contracts().iter().enumerate() {
            if usize::from(contract.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            if !matches!(contract, AttachedCallableContractClause::Requires { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.requires())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            requires.push(condition);
        }
        self.close_scope_members(contract_scopes.requires(), Box::<[LocalId]>::from([]))?;

        let result_local = attached
            .postcondition_result_source_span()
            .map(|source| {
                self.allocate_postcondition_result_local(
                    contract_scopes.ensures(),
                    Some(return_type),
                    source,
                )
            })
            .transpose()?;
        self.close_scope_members(
            contract_scopes.ensures(),
            result_local
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )?;
        let mut ensures = Vec::new();
        for contract in attached.contracts() {
            if !matches!(contract, AttachedCallableContractClause::Ensures { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.ensures())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            ensures.push(condition);
        }

        let (body, body_issue) = match attached.body() {
            AttachedPredicateBody::Expression { expression, .. } => {
                let expression = self.lower_attached_expression(expression, body_scope)?;
                self.close_scope_members(body_scope, Box::new([]))?;
                let issue = (attached.body().has_recovery()
                    || self.staged_expression_is_poisoned(expression)?)
                .then_some(HirItemIssue::Recovery);
                (
                    HirPredicateBody::Expression {
                        scope: body_scope,
                        expression,
                    },
                    issue,
                )
            }
            AttachedPredicateBody::Block { block, .. } => {
                let lowered = self.lower_attached_predicate_block(block, owner, body_scope)?;
                let issue = (attached.body().has_recovery() || lowered.recovery.is_some())
                    .then_some(HirItemIssue::Recovery);
                (
                    HirPredicateBody::Block {
                        scope: lowered.scope,
                        statements: lowered.statements,
                        tail: lowered.tail,
                    },
                    issue,
                )
            }
            AttachedPredicateBody::Missing { missing, .. } => {
                let expression =
                    self.lower_missing_required_tail_for_scope(body_scope, missing.source_span())?;
                self.close_scope_members(body_scope, Box::new([]))?;
                (
                    HirPredicateBody::Error {
                        scope: body_scope,
                        expression,
                    },
                    Some(HirItemIssue::MissingBody),
                )
            }
        };

        let signature = HirCallableSignature::try_new(
            generic_parameters,
            parameters.into_boxed_slice(),
            where_predicates,
            requires.into_boxed_slice(),
            ensures.into_boxed_slice(),
            return_type,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let declaration = HirPredicate::try_new(name.value, signature, body, contract_scopes)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| parameter_missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| parameter_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| contract_recovery.then_some(HirItemIssue::Recovery))
            .or_else(|| {
                attached
                    .authored_return()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
            .or(body_issue)
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Predicate(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    pub(super) fn lower_proof_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<ProofItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let name = project_required_name(attached.name())?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;
        let requires_site = self.attached_insertion_site(attached.requires_scope_source_span())?;
        let ensures_site = self.attached_insertion_site(attached.ensures_scope_source_span())?;
        let contract_scopes =
            self.allocate_item_contract_scopes(owner, callable_scope, requires_site, ensures_site)?;
        let body_scope = self.allocate_item_body_scope(
            attached.body().syntax(),
            owner,
            callable_scope,
            HirScopeKind::Proof,
        )?;

        let (generic_parameters, generic_recovery) =
            self.lower_generic_parameters(attached.generics(), callable_scope)?;

        let mut parameter_recovery = attached.parameter_group().has_recovery();
        let mut parameter_missing_type = false;
        let mut parameters = Vec::with_capacity(attached.parameter_group().parameters().len());
        let mut callable_locals = Vec::new();
        for (position, parameter) in attached.parameter_group().parameters().iter().enumerate() {
            if usize::from(parameter.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let ty = self.lower_attached_type(parameter.ty(), callable_scope)?;
            let pattern = self.lower_attached_pattern_binding(
                parameter.pattern(),
                callable_scope,
                HirPatternBindingPolicy::ProofParameter,
            )?;
            let type_poisoned = self.staged_type_is_poisoned(ty)?;
            parameter_missing_type |=
                type_poisoned && parameter.ty().syntax().kind() == SyntaxKind::MissingType;
            parameter_recovery |= pattern.poisoned
                || type_poisoned
                || parameter.has_recovery()
                || parameter.is_rest()
                || parameter.default().is_some();
            callable_locals.extend_from_slice(&pattern.locals);
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
        self.close_scope_members(callable_scope, callable_locals.into_boxed_slice())?;

        let (return_type, return_missing_type, return_recovery) = match attached.authored_return() {
            Some(authored) => {
                let ty = self.lower_attached_type(authored.ty(), callable_scope)?;
                let poisoned = self.staged_type_is_poisoned(ty)?;
                (
                    ty,
                    poisoned && authored.ty().syntax().kind() == SyntaxKind::MissingType,
                    poisoned,
                )
            }
            None => (
                self.lower_proof_unit_return(
                    owner,
                    callable_scope,
                    attached.implicit_return_source_span(),
                )?,
                false,
                false,
            ),
        };
        let (where_predicates, where_recovery) =
            self.lower_where_clauses(attached.where_clauses(), callable_scope)?;
        let return_is_unit = self
            .arenas
            .types()
            .resolve_staged(&self.slots, return_type)?
            .kind()
            .is_unit();

        let mut requires = Vec::new();
        let mut contract_recovery = false;
        for (position, contract) in attached.contracts().iter().enumerate() {
            if usize::from(contract.source_ordinal()) != position {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            if !matches!(contract, AttachedCallableContractClause::Requires { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.requires())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            requires.push(condition);
        }
        self.close_scope_members(contract_scopes.requires(), Box::<[LocalId]>::from([]))?;

        let result_local = attached
            .postcondition_result_source_span()
            .map(|source| {
                self.allocate_postcondition_result_local(
                    contract_scopes.ensures(),
                    Some(return_type),
                    source,
                )
            })
            .transpose()?;
        self.close_scope_members(
            contract_scopes.ensures(),
            result_local
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )?;
        let mut ensures = Vec::new();
        for contract in attached.contracts() {
            if !matches!(contract, AttachedCallableContractClause::Ensures { .. }) {
                continue;
            }
            let condition =
                self.lower_attached_expression(contract.condition(), contract_scopes.ensures())?;
            contract_recovery |=
                contract.has_recovery() || self.staged_expression_is_poisoned(condition)?;
            ensures.push(condition);
        }

        let (body, body_issue) = match attached.body() {
            AttachedProofBody::Expression { expression, .. } => {
                let expression = self.lower_attached_expression(expression, body_scope)?;
                self.close_scope_members(body_scope, Box::new([]))?;
                let issue = (attached.body().has_recovery()
                    || self.staged_expression_is_poisoned(expression)?)
                .then_some(HirItemIssue::Recovery);
                (
                    HirProofBody::Expression {
                        scope: body_scope,
                        expression,
                    },
                    issue,
                )
            }
            AttachedProofBody::Block { block, .. } => {
                let lowered =
                    self.lower_attached_proof_block(block, owner, body_scope, return_is_unit)?;
                let issue = (attached.body().has_recovery() || lowered.recovery.is_some())
                    .then_some(HirItemIssue::Recovery);
                (
                    HirProofBody::Block {
                        scope: lowered.scope,
                        statements: lowered.statements,
                        tail: lowered.tail,
                    },
                    issue,
                )
            }
            AttachedProofBody::Missing { missing, .. } => {
                let expression =
                    self.lower_missing_required_tail_for_scope(body_scope, missing.source_span())?;
                self.close_scope_members(body_scope, Box::new([]))?;
                (
                    HirProofBody::Error {
                        scope: body_scope,
                        expression,
                    },
                    Some(HirItemIssue::MissingBody),
                )
            }
        };

        let signature = HirCallableSignature::try_new(
            generic_parameters,
            parameters.into_boxed_slice(),
            where_predicates,
            requires.into_boxed_slice(),
            ensures.into_boxed_slice(),
            return_type,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let public_id = match attached.public_id() {
            arcweft_lang_syntax::attachment::AttachedDeclarationPublicId::Explicit {
                value,
                ..
            } => Some(value.clone()),
            arcweft_lang_syntax::attachment::AttachedDeclarationPublicId::Derived
            | arcweft_lang_syntax::attachment::AttachedDeclarationPublicId::Recovered { .. } => {
                None
            }
        };
        let declaration =
            HirProof::try_new(name.value, public_id, signature, body, contract_scopes)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let issue = prefix
            .issue
            .or(name.issue)
            .or_else(|| {
                matches!(
                    attached.public_id(),
                    arcweft_lang_syntax::attachment::AttachedDeclarationPublicId::Recovered { .. }
                )
                .then_some(HirItemIssue::MalformedHeader)
            })
            .or_else(|| generic_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| parameter_missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| parameter_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| return_missing_type.then_some(HirItemIssue::MissingType))
            .or_else(|| return_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| where_recovery.then_some(HirItemIssue::MalformedHeader))
            .or_else(|| contract_recovery.then_some(HirItemIssue::Recovery))
            .or(body_issue)
            .or_else(|| {
                (!attached.trailing_recovery().is_empty()).then_some(HirItemIssue::Recovery)
            });
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Proof(declaration),
            Box::new([]),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok(LoweredItemProjection {
            item,
            members: None,
        })
    }

    fn allocate_item_contract_scope(
        &mut self,
        owner: ItemId,
        parent: ScopeId,
        role: SyntheticRole,
        kind: HirScopeKind,
        site: HirSourceSite,
    ) -> Result<ScopeId, HirLowerFailure> {
        let key = SyntheticKey::try_new(SyntheticOwner::Item(owner), role, 0)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation = self
            .arenas
            .scopes()
            .reserve_synthetic(&mut self.slots, key, site)?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                kind,
                Some(parent),
                HirScopeOwner::Item(owner),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }

        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == kind
            && retained.parent() == Some(parent)
            && retained.owner() == &HirScopeOwner::Item(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    fn lower_predicate_bool_return(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        source: SourceSpan,
    ) -> Result<TypeId, HirLowerFailure> {
        let site = self.attached_insertion_site(source)?;
        let key = SyntheticKey::try_new(
            SyntheticOwner::Item(owner),
            SyntheticRole::PredicateBoolReturn,
            0,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation = self
            .arenas
            .types()
            .reserve_synthetic(&mut self.slots, key, site)?;
        let ty = reservation.id();
        let name =
            HirName::try_new("Bool".into()).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let path = HirPath::try_new(
            HirPathRoot::ImplicitCrate,
            Box::new([HirPathSegment::Identifier(name)]),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let payload = HirType::try_new(
            ty,
            HirTypeKind::Path(path),
            scope,
            HirPoisonState::Clean,
            self,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .types()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(Into::into)
    }

    fn lower_proof_unit_return(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        source: SourceSpan,
    ) -> Result<TypeId, HirLowerFailure> {
        let site = self.attached_insertion_site(source)?;
        let key = SyntheticKey::try_new(
            SyntheticOwner::Item(owner),
            SyntheticRole::ProofUnitReturn,
            0,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation = self
            .arenas
            .types()
            .reserve_synthetic(&mut self.slots, key, site)?;
        let ty = reservation.id();
        let payload = HirType::try_new(
            ty,
            HirTypeKind::Tuple(Box::new([])),
            scope,
            HirPoisonState::Clean,
            self,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .types()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(Into::into)
    }

    fn allocate_postcondition_result_local(
        &mut self,
        scope: ScopeId,
        annotation: Option<TypeId>,
        source: SourceSpan,
    ) -> Result<LocalId, HirLowerFailure> {
        let start = source.range().start();
        let site = self.attached_insertion_site(source)?;
        let name = HirName::try_new("result".into())
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let generation = self.next_sequential_local_generation(scope, &name, start)?;
        let key = SyntheticKey::try_new(
            SyntheticOwner::Scope(scope),
            SyntheticRole::PostconditionResult,
            0,
        )
        .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation = self
            .arenas
            .locals()
            .reserve_synthetic(&mut self.slots, key, site)?;
        let payload = HirLocal::try_new(
            scope,
            HirLocalKind::PostconditionResult,
            name.clone(),
            generation,
            None,
            annotation,
            false,
            false,
        )
        .map_err(|_| HirInvariantFailure::InvalidLocalTimeline)?;
        let local = self
            .arenas
            .locals()
            .finalize(&mut self.slots, reservation, payload)?;
        self.local_timelines
            .entry((scope, name))
            .or_default()
            .publish(LocalGenerationLedgerEntry::new(local, generation, start))?;
        Ok(local)
    }

    fn attached_insertion_site(
        &self,
        source: SourceSpan,
    ) -> Result<HirSourceSite, HirLowerFailure> {
        let site = HirSourceSite::from_attached_span(self.request.source().document(), &source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if matches!(site, HirSourceSite::Insertion(_)) {
            Ok(site)
        } else {
            Err(HirInvariantFailure::InvalidSourceSpan.into())
        }
    }
}
