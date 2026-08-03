//! Final Activity interface lowering into retained item, member, scope, and local owners.

use arcweft_id::RetainedIdentityFamily;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedActivityContractBody, AttachedActivityContractClause,
    AttachedActivityContractEntry, AttachedActivityDeclaration, AttachedActivityEntry,
    AttachedActivityLifecycle, AttachedActivityMode, AttachedActivityPort,
    AttachedActivityPortBody, AttachedRequiredName,
};

use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::identity::{HirLimit, ItemId, LocalId, ScopeId, SyntheticOwner};
use crate::item::{
    HirActivityDeclaration, HirActivityLifecycle, HirActivityMode, HirActivityPortMember,
    HirDeclarationMember, HirDeclarationMemberArena, HirDeclarationMemberId,
    HirDeclarationMemberIssue, HirDeclarationMemberKind, HirDeclarationMemberPoisonState, HirItem,
    HirItemFamily, HirItemIssue, HirItemKind,
};
use crate::leaf::HirName;
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirLocal, HirLocalKind};
use crate::source_index::HirSourceSite;

use super::super::super::{LocalGenerationLedgerEntry, StagedHirModuleTransaction, require_limit};
use super::super::{LoweredItemProjection, item_state, project_required_name};
use super::{project_retained_header, retained_header_issue};

#[derive(Clone, Copy)]
enum ActivityPortDirection {
    Input,
    Output,
}

impl StagedHirModuleTransaction<'_> {
    pub(in crate::final_lowering::item_lowering) fn lower_activity_declaration(
        &mut self,
        owner: ItemId,
        scope: ScopeId,
        node: &AstNode<arcweft_lang_syntax::attachment::node::ActivityDeclarationItemKind>,
    ) -> Result<LoweredItemProjection, HirLowerFailure> {
        let attached = node
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        preflight_activity_inventory(&attached)?;

        let prefix = self.lower_item_prefix(attached.prefix(), scope)?;
        let prefix_issue = prefix.issue;
        let header = project_retained_header(attached.header(), RetainedIdentityFamily::Activity)?;
        let callable_scope = self.allocate_item_callable_scope(node, owner, scope)?;
        let contract_scopes = self.allocate_item_contract_scopes(
            owner,
            callable_scope,
            HirSourceSite::Span(attached.requires_scope_source_span()),
            HirSourceSite::Span(attached.ensures_scope_source_span()),
        )?;

        let mut mode = HirActivityMode::Deterministic;
        let mut lifecycle = HirActivityLifecycle::Stateless;
        let mut mode_selected = false;
        let mut lifecycle_selected = false;
        let mut retained_members = Vec::new();
        let mut member_ids = Vec::new();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut callable_locals = Vec::new();
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut first_body_issue = None;

        for (entry_position, entry) in attached.body().entries().iter().enumerate() {
            let expected_ordinal = u16::try_from(entry_position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if entry.source_ordinal() != expected_ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let entry_has_issue = match entry {
                AttachedActivityEntry::Mode(member) => {
                    if !mode_selected {
                        mode = project_mode(member.value());
                        mode_selected = true;
                    }
                    member.state().has_recovery()
                        || member.assignment().is_missing()
                        || member.value().has_recovery()
                }
                AttachedActivityEntry::Lifecycle(member) => {
                    if !lifecycle_selected {
                        lifecycle = project_lifecycle(member.value());
                        lifecycle_selected = true;
                    }
                    member.state().has_recovery()
                        || member.assignment().is_missing()
                        || member.value().has_recovery()
                }
                AttachedActivityEntry::Input(section) => {
                    let mut issue = section.state().has_recovery() || section.body().has_recovery();
                    self.lower_activity_ports(
                        owner,
                        callable_scope,
                        section.body(),
                        ActivityPortDirection::Input,
                        &mut retained_members,
                        &mut member_ids,
                        &mut inputs,
                        &mut callable_locals,
                        &mut issue,
                    )?;
                    issue
                }
                AttachedActivityEntry::Output(section) => {
                    let mut issue = section.state().has_recovery() || section.body().has_recovery();
                    self.lower_activity_ports(
                        owner,
                        callable_scope,
                        section.body(),
                        ActivityPortDirection::Output,
                        &mut retained_members,
                        &mut member_ids,
                        &mut outputs,
                        &mut callable_locals,
                        &mut issue,
                    )?;
                    issue
                }
                AttachedActivityEntry::Contract(section) => {
                    let mut issue = section.state().has_recovery() || section.body().has_recovery();
                    self.lower_activity_contract(
                        section.body(),
                        contract_scopes.requires(),
                        contract_scopes.ensures(),
                        &mut requires,
                        &mut ensures,
                        &mut issue,
                    )?;
                    issue
                }
                AttachedActivityEntry::Recovery { .. } => true,
            };
            if entry_has_issue {
                first_body_issue.get_or_insert(HirItemIssue::InvalidMember);
            }
        }

        require_limit(HirLimit::LocalsPerScope, callable_locals.len())?;
        self.close_scope_members(callable_scope, callable_locals.into_boxed_slice())?;

        let members = if retained_members.is_empty() {
            None
        } else {
            Some(
                HirDeclarationMemberArena::try_new(
                    owner,
                    HirItemFamily::Activity,
                    retained_members.into_boxed_slice(),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            )
        };
        let issue = prefix_issue
            .or_else(|| retained_header_issue(attached.header()))
            .or_else(|| {
                attached
                    .unexpected_header_recovery()
                    .is_some()
                    .then_some(HirItemIssue::MalformedHeader)
            })
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
                attached
                    .trailing_recovery()
                    .is_some()
                    .then_some(HirItemIssue::Recovery)
            });
        let declaration = HirActivityDeclaration::try_new(
            owner,
            header,
            contract_scopes,
            mode,
            lifecycle,
            inputs.into_boxed_slice(),
            outputs.into_boxed_slice(),
            requires.into_boxed_slice(),
            ensures.into_boxed_slice(),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item = HirItem::try_new_with_state(
            owner,
            scope,
            prefix.value,
            HirItemKind::Activity(declaration),
            member_ids.into_boxed_slice(),
            item_state(issue),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        Ok(LoweredItemProjection { item, members })
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_activity_ports(
        &mut self,
        owner: ItemId,
        callable_scope: ScopeId,
        body: &AttachedActivityPortBody,
        direction: ActivityPortDirection,
        retained_members: &mut Vec<HirDeclarationMember>,
        member_ids: &mut Vec<HirDeclarationMemberId>,
        direction_ids: &mut Vec<HirDeclarationMemberId>,
        callable_locals: &mut Vec<LocalId>,
        entry_has_issue: &mut bool,
    ) -> Result<(), HirLowerFailure> {
        for (port_position, port) in body.ports().iter().enumerate() {
            let expected_ordinal = u16::try_from(port_position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if port.source_ordinal() != expected_ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            let ordinal = u32::try_from(retained_members.len())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let id = HirDeclarationMemberId::new(owner, ordinal);
            let (member, local) = self.lower_activity_port(id, callable_scope, port, direction)?;
            *entry_has_issue |= member.is_poisoned();
            if let Some(local) = local {
                callable_locals.push(local);
            }
            retained_members.push(member);
            member_ids.push(id);
            direction_ids.push(id);
        }
        Ok(())
    }

    fn lower_activity_port(
        &mut self,
        id: HirDeclarationMemberId,
        callable_scope: ScopeId,
        port: &AttachedActivityPort,
        direction: ActivityPortDirection,
    ) -> Result<(HirDeclarationMember, Option<LocalId>), HirLowerFailure> {
        let name = project_required_name(port.name())?;
        let ty = self.lower_attached_type(port.ty(), callable_scope)?;
        let type_poisoned = self.staged_type_is_poisoned(ty)?;
        let member_issue = if port.is_duplicate() {
            Some(HirDeclarationMemberIssue::Duplicate)
        } else if name.issue.is_some()
            || port.colon().is_missing()
            || type_poisoned
            || port.initializer_recovery().is_some()
        {
            Some(HirDeclarationMemberIssue::RecoveredChild)
        } else {
            None
        };
        let state = member_issue.map_or(
            HirDeclarationMemberPoisonState::Clean,
            HirDeclarationMemberPoisonState::Poisoned,
        );
        let local = match (name.value.resolved(), port.name()) {
            (Some(name), AttachedRequiredName::Resolved { syntax, .. }) => {
                Some(self.allocate_activity_port_local(
                    syntax,
                    callable_scope,
                    name.clone(),
                    ty,
                    state.is_poisoned(),
                )?)
            }
            (None, AttachedRequiredName::Missing { .. }) => None,
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };
        let port = HirActivityPortMember::try_new(name.value, ty, local)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let kind = match direction {
            ActivityPortDirection::Input => HirDeclarationMemberKind::ActivityInput(port),
            ActivityPortDirection::Output => HirDeclarationMemberKind::ActivityOutput(port),
        };
        let member = HirDeclarationMember::try_new(id, kind, state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((member, local))
    }

    fn allocate_activity_port_local(
        &mut self,
        syntax: &arcweft_lang_syntax::attachment::NameNode,
        scope: ScopeId,
        name: HirName,
        annotation: crate::identity::TypeId,
        poisoned: bool,
    ) -> Result<LocalId, HirLowerFailure> {
        let source_site = HirSourceSite::Span(syntax.source_span());
        let binding_name_start = syntax.range().start();
        let generation = self.next_sequential_local_generation(scope, &name, binding_name_start)?;
        let reservation = self.arenas.locals().reserve_source(
            &mut self.slots,
            syntax.id(),
            source_site.clone(),
        )?;
        let payload = HirLocal::try_new(
            scope,
            HirLocalKind::Parameter,
            name.clone(),
            generation,
            None,
            Some(annotation),
            false,
            poisoned,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let local = self
            .arenas
            .locals()
            .finalize(&mut self.slots, reservation, payload)?;
        self.local_timelines
            .entry((scope, name))
            .or_default()
            .publish(LocalGenerationLedgerEntry::new(
                local,
                generation,
                binding_name_start,
            ))?;
        if poisoned {
            let owner = SyntheticOwner::Local(local);
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                owner,
                HirRecoveryPrimary::owner_whole(owner),
                source_site,
            ));
        }
        Ok(local)
    }

    fn lower_activity_contract(
        &mut self,
        body: &AttachedActivityContractBody,
        requires_scope: ScopeId,
        ensures_scope: ScopeId,
        requires: &mut Vec<crate::identity::ExprId>,
        ensures: &mut Vec<crate::identity::ExprId>,
        entry_has_issue: &mut bool,
    ) -> Result<(), HirLowerFailure> {
        for (entry_position, entry) in body.entries().iter().enumerate() {
            let expected_ordinal = u16::try_from(entry_position)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            if entry.source_ordinal() != expected_ordinal {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            match entry {
                AttachedActivityContractEntry::Clause(clause) => {
                    let (scope, destination) = match clause.as_ref() {
                        AttachedActivityContractClause::Requires { .. } => {
                            (requires_scope, &mut *requires)
                        }
                        AttachedActivityContractClause::Ensures { .. } => {
                            (ensures_scope, &mut *ensures)
                        }
                    };
                    let expression =
                        self.lower_attached_expression(clause.condition().expression(), scope)?;
                    *entry_has_issue |= clause.is_out_of_order()
                        || self.staged_expression_is_poisoned(expression)?;
                    destination.push(expression);
                }
                AttachedActivityContractEntry::Recovery { .. } => *entry_has_issue = true,
            }
        }
        Ok(())
    }
}

const fn project_mode(mode: &AttachedActivityMode) -> HirActivityMode {
    match mode {
        AttachedActivityMode::Deterministic(_) => HirActivityMode::Deterministic,
        AttachedActivityMode::CheckpointedRealtime(_) => HirActivityMode::CheckpointedRealtime,
        AttachedActivityMode::ExternalRealtime(_) => HirActivityMode::ExternalRealtime,
        AttachedActivityMode::Missing(_) | AttachedActivityMode::Invalid(_) => {
            HirActivityMode::Deterministic
        }
    }
}

const fn project_lifecycle(lifecycle: &AttachedActivityLifecycle) -> HirActivityLifecycle {
    match lifecycle {
        AttachedActivityLifecycle::Stateless(_) => HirActivityLifecycle::Stateless,
        AttachedActivityLifecycle::Snapshot(_) => HirActivityLifecycle::Snapshot,
        AttachedActivityLifecycle::Missing(_) | AttachedActivityLifecycle::Invalid(_) => {
            HirActivityLifecycle::Stateless
        }
    }
}

fn preflight_activity_inventory(
    attached: &AttachedActivityDeclaration,
) -> Result<(), HirLowerFailure> {
    let mut declaration_members = attached.body().entries().len();
    let mut ports = 0_usize;
    for entry in attached.body().entries() {
        let nested = match entry {
            AttachedActivityEntry::Input(section) => section.body().ports().len(),
            AttachedActivityEntry::Output(section) => section.body().ports().len(),
            AttachedActivityEntry::Contract(section) => section.body().entries().len(),
            AttachedActivityEntry::Mode(_)
            | AttachedActivityEntry::Lifecycle(_)
            | AttachedActivityEntry::Recovery { .. } => 0,
        };
        declaration_members = declaration_members
            .checked_add(nested)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        if matches!(
            entry,
            AttachedActivityEntry::Input(_) | AttachedActivityEntry::Output(_)
        ) {
            ports = ports
                .checked_add(nested)
                .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        }
    }
    require_limit(HirLimit::DeclarationMembers, declaration_members)?;
    require_limit(HirLimit::LocalsPerScope, ports)
}
