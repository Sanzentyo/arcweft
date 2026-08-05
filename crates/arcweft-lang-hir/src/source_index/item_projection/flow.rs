//! Ordinary Flow item source manifest and attached-payload freeze.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AttachedCallableParameterKind, AttachedFlowContractClause, AttachedFlowContractMode,
    AttachedFlowContractOperands, AttachedFlowDeclaration, AttachedFlowIdSyntax,
    AttachedFlowIdentity, AttachedFlowPublicId, AttachedFlowReturnSyntax, AttachedFlowSignature,
    AttachedGenericParameter, AttachedRequiredFlowBody,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use crate::expr::{HirThreadBodyOwner, HirThreadFlowItem};
use crate::identity::{ItemId, SyntheticOwner};
use crate::item::{
    HirContractMode, HirFlowContractClause, HirFlowIdentity, HirFlowIssue, HirFlowIssueClass,
    HirFlowIssueOwner, HirFlowItem, HirFlowPoison, HirItem, HirItemIssue, HirItemKind,
};
use crate::leaf::{
    HirFamilyRelativeId, HirIdFamily, HirIdRef, HirIdRefValue, HirIdSuffix, HirName, HirRelativeId,
};
use crate::scope::{HirPatternBindingPolicy, HirScopeKind};
use crate::slot::SlotSnapshot;
use crate::source_index::block_projection::{BlockValidationArenas, source_expression_matches};

use super::super::{
    HirFlowContractSourcePart, HirFlowParameterSourcePart, HirFlowReturnSourcePart,
    HirFlowSourceRole, HirItemSourceRole, HirSourceCommitInvariantError, HirSourceIndex,
    HirSourceQuery, HirSourceQueryError, HirSourceRequirement, HirSourceSite,
    HirThreadBodySourceRole, HirThreadFlowItemSourcePart, StagedHirSourceIndex,
};
use super::callable::{
    CallableScopeIds, CallableScopeSource, ParameterSurfacePolicy, contract_scopes_match,
    direct_children_are_exact, item_body_scope_matches_at_site, parameters_match,
    postcondition_result_matches,
};
use super::{
    ItemValidationArenas, generic_parameters_match, item_prefix_matches, item_state, prefix_issue,
    slot_is_poisoned, type_owner_matches, where_predicates_match,
};

#[derive(Default)]
struct FlowManifest {
    requirements: BTreeMap<HirSourceQuery, HirSourceRequirement>,
    components: BTreeMap<HirSourceQuery, HirSourceSite>,
}

impl FlowManifest {
    fn required(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        role: HirFlowSourceRole,
        span: SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        let query = flow_query(owner, role);
        if self
            .requirements
            .insert(query.clone(), HirSourceRequirement::Required)
            .is_some()
        {
            return Err(HirSourceCommitInvariantError::ConflictingRequirement { query });
        }
        let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
        if self.components.insert(query.clone(), site).is_some() {
            return Err(HirSourceCommitInvariantError::ConflictingComponent { query });
        }
        Ok(())
    }
}

#[allow(
    clippy::result_large_err,
    reason = "source staging preserves the exact typed Flow query on failure"
)]
impl StagedHirSourceIndex {
    /// Stages one ordinary Flow's exact item-owned component manifest.
    ///
    /// Expression, pattern, type, local, scope, and Thread-body children retain
    /// their existing source owners and are not copied into this item family.
    pub(crate) fn stage_attached_flow(
        &mut self,
        parsed: &ParsedSource,
        owner: ItemId,
        attached: &AttachedFlowDeclaration,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.syntax().snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.syntax().snapshot_id().clone(),
            });
        }
        let manifest = match flow_manifest(parsed, owner, attached) {
            Ok(manifest) => manifest,
            Err(error) => return self.reject(error),
        };
        for (query, requirement) in manifest.requirements {
            self.require(&query, requirement)?;
        }
        for (query, site) in manifest.components {
            self.stage(&query, site)?;
        }
        Ok(())
    }
}

pub(super) fn payload_matches(
    index: &HirSourceIndex,
    owner: ItemId,
    attached: &AttachedFlowDeclaration,
    item: &HirItem,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let HirItemKind::Flow(flow) = item.kind() else {
        return false;
    };
    let Some(identity_issues) = flow_identity_matches(owner, flow, attached.identity()) else {
        return false;
    };
    let signature = attached.signature();
    let where_clauses = signature
        .where_clause()
        .map_or(&[][..], std::slice::from_ref);
    if !item.members().is_empty()
        || !item_prefix_matches(item, attached.prefix(), slots)
        || !generic_parameters_match(flow.generic_parameters(), signature.generics(), slots)
        || !where_predicates_match(flow.where_predicates(), where_clauses, slots)
        || !flow_source_ordinals_match(flow, attached)
        || !flow_scope_ids_belong_to_module(flow, owner)
        || !exact_flow_manifest(index, parsed, owner, attached)
    {
        return false;
    }

    let block_arenas = block_arenas(arenas);
    let attached_parameters = signature
        .parameters()
        .map_or(&[][..], |group| group.parameters());
    let Some(_parameter_state) = parameters_match(
        attached_parameters,
        signature.parameters().is_some_and(
            arcweft_lang_syntax::attachment::AttachedFixedParameterGroup::has_recovery,
        ),
        ParameterSurfacePolicy::FixedOnly,
        flow.parameters(),
        flow.callable_scope(),
        HirPatternBindingPolicy::FlowParameter,
        slots,
        arenas,
        &block_arenas,
    ) else {
        return false;
    };
    let Some(return_recovery) = flow_return_matches(flow, signature.result(), slots, arenas) else {
        return false;
    };
    let Some(signature_issues) =
        flow_signature_issues(owner, flow, signature, slots, return_recovery)
    else {
        return false;
    };
    let Some(contract_issues) =
        flow_contracts_match(owner, flow, attached.contracts(), slots, arenas)
    else {
        return false;
    };
    let Some(body_issues) = flow_body_matches(index, parsed, owner, flow, attached.body(), slots)
    else {
        return false;
    };
    if !flow_scope_graph_matches(owner, item, flow, attached, parsed, slots, arenas)
        || !flow_result_local_matches(flow, attached, parsed, slots, arenas)
    {
        return false;
    }

    let mut issues = Vec::new();
    if prefix_issue(attached.prefix(), item.prefix(), slots).is_some() {
        issues.push(flow_item_issue(
            owner,
            HirFlowIssueClass::Prefix,
            HirFlowSourceRole::Whole,
        ));
    }
    issues.extend(identity_issues);
    issues.extend(signature_issues);
    issues.extend(contract_issues);
    issues.extend(body_issues);
    let trailing_base = match u32::try_from(signature.recovery().len()) {
        Ok(value) => value,
        Err(_) => return false,
    };
    for position in 0..attached.trailing_recovery().len() {
        let Ok(position) = u32::try_from(position) else {
            return false;
        };
        let Some(ordinal) = trailing_base.checked_add(position) else {
            return false;
        };
        issues.push(flow_item_issue(
            owner,
            HirFlowIssueClass::TrailingRecovery,
            HirFlowSourceRole::TrailingRecovery { ordinal },
        ));
    }
    flow_poison_matches(item, flow, issues)
}

fn flow_manifest(
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AttachedFlowDeclaration,
) -> Result<FlowManifest, HirSourceCommitInvariantError> {
    let mut manifest = FlowManifest::default();
    manifest.required(
        parsed,
        owner,
        HirFlowSourceRole::Keyword,
        attached.keyword().clone(),
    )?;
    if let Some(visibility) = attached.prefix().visibility() {
        manifest.required(
            parsed,
            owner,
            HirFlowSourceRole::Visibility,
            visibility.syntax().source_span(),
        )?;
    }
    project_identity(&mut manifest, parsed, owner, attached.identity())?;
    project_signature(&mut manifest, parsed, owner, attached)?;
    project_contracts(&mut manifest, parsed, owner, attached.contracts())?;
    project_body(&mut manifest, parsed, owner, attached.body())?;
    for (position, recovery) in attached
        .signature()
        .recovery()
        .iter()
        .map(|recovery| recovery.syntax())
        .chain(attached.trailing_recovery().iter())
        .enumerate()
    {
        let ordinal = u32::try_from(position).map_err(|_| flow_state_mismatch(owner))?;
        manifest.required(
            parsed,
            owner,
            HirFlowSourceRole::TrailingRecovery { ordinal },
            recovery.source_span(),
        )?;
    }
    Ok(manifest)
}

fn project_identity(
    manifest: &mut FlowManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    identity: &AttachedFlowIdentity,
) -> Result<(), HirSourceCommitInvariantError> {
    let (public_id, name) = match identity {
        AttachedFlowIdentity::Name { name } => (None, Some(name.syntax().source_span())),
        AttachedFlowIdentity::PublicId { public_id } => {
            (Some(public_id.syntax().source_span()), None)
        }
        AttachedFlowIdentity::PublicIdAndName { public_id, name } => (
            Some(public_id.syntax().source_span()),
            Some(name.syntax().source_span()),
        ),
        AttachedFlowIdentity::Missing {
            insertion,
            attempted_public_id,
            ..
        } => (
            attempted_public_id
                .as_ref()
                .map(|public_id| public_id.syntax().source_span()),
            Some(insertion.clone()),
        ),
    };
    if let Some(public_id) = public_id {
        manifest.required(parsed, owner, HirFlowSourceRole::PublicId, public_id)?;
    }
    if let Some(name) = name {
        manifest.required(parsed, owner, HirFlowSourceRole::Name, name)?;
    }
    Ok(())
}

fn project_signature(
    manifest: &mut FlowManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AttachedFlowDeclaration,
) -> Result<(), HirSourceCommitInvariantError> {
    let signature = attached.signature();
    if let Some(group) = signature.generics() {
        manifest.required(
            parsed,
            owner,
            HirFlowSourceRole::GenericGroup,
            group.syntax().source_span(),
        )?;
        for (position, parameter) in group.parameters().iter().enumerate() {
            let ordinal = u16::try_from(position).map_err(|_| flow_state_mismatch(owner))?;
            manifest.required(
                parsed,
                owner,
                HirFlowSourceRole::GenericParameter { ordinal },
                parameter.syntax().source_span(),
            )?;
        }
    }

    if let Some(group) = signature.parameters() {
        manifest.required(
            parsed,
            owner,
            HirFlowSourceRole::ParameterGroup,
            group.syntax().source_span(),
        )?;
        for (position, parameter) in group.parameters().iter().enumerate() {
            let ordinal = u16::try_from(position).map_err(|_| flow_state_mismatch(owner))?;
            for (part, span) in [
                (
                    HirFlowParameterSourcePart::Whole,
                    parameter.syntax().source_span(),
                ),
                (
                    HirFlowParameterSourcePart::Pattern,
                    parameter.pattern().whole_source_span(),
                ),
                (
                    HirFlowParameterSourcePart::Colon,
                    parameter.colon().source_span().clone(),
                ),
                (
                    HirFlowParameterSourcePart::Type,
                    parameter.ty().whole_source_span(),
                ),
            ] {
                manifest.required(
                    parsed,
                    owner,
                    HirFlowSourceRole::Parameter { ordinal, part },
                    span,
                )?;
            }
        }
    }

    match signature.result() {
        AttachedFlowReturnSyntax::Omitted => {}
        AttachedFlowReturnSyntax::Authored(result) => {
            for (part, span) in [
                (
                    HirFlowReturnSourcePart::Whole,
                    result.syntax().source_span(),
                ),
                (
                    HirFlowReturnSourcePart::Arrow,
                    result.arrow().source_span().clone(),
                ),
                (
                    HirFlowReturnSourcePart::Type,
                    result.ty().whole_source_span(),
                ),
            ] {
                manifest.required(parsed, owner, HirFlowSourceRole::Return { part }, span)?;
            }
        }
    }

    if let Some(clause) = signature.where_clause() {
        manifest.required(
            parsed,
            owner,
            HirFlowSourceRole::WhereClause,
            clause.syntax().source_span(),
        )?;
        for (position, predicate) in clause.predicates().iter().enumerate() {
            let ordinal = u16::try_from(position).map_err(|_| flow_state_mismatch(owner))?;
            manifest.required(
                parsed,
                owner,
                HirFlowSourceRole::WherePredicate { ordinal },
                predicate.syntax().source_span(),
            )?;
        }
    }
    Ok(())
}

fn project_contracts(
    manifest: &mut FlowManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    clauses: &[AttachedFlowContractClause],
) -> Result<(), HirSourceCommitInvariantError> {
    for (position, clause) in clauses.iter().enumerate() {
        let ordinal = u16::try_from(position).map_err(|_| flow_state_mismatch(owner))?;
        if clause.source_ordinal() != ordinal {
            return Err(flow_state_mismatch(owner));
        }
        let role = |part| HirFlowSourceRole::ContractClause { ordinal, part };
        manifest.required(
            parsed,
            owner,
            role(HirFlowContractSourcePart::Whole),
            clause.syntax().source_span(),
        )?;
        manifest.required(
            parsed,
            owner,
            role(HirFlowContractSourcePart::ClauseKeyword),
            clause.keyword().clone(),
        )?;
        if let Some(no_effect_keyword) = clause.no_effect_keyword() {
            manifest.required(
                parsed,
                owner,
                role(HirFlowContractSourcePart::NoEffectKeyword),
                no_effect_keyword.clone(),
            )?;
        }
        if let Some(mode) = clause
            .mode()
            .and_then(AttachedFlowContractMode::source_span)
        {
            manifest.required(
                parsed,
                owner,
                role(HirFlowContractSourcePart::Mode),
                mode.clone(),
            )?;
        }
        for operand_position in 0..clause_operands(clause).len() {
            let operand =
                u16::try_from(operand_position).map_err(|_| flow_state_mismatch(owner))?;
            manifest.required(
                parsed,
                owner,
                role(HirFlowContractSourcePart::Operand { ordinal: operand }),
                operand_source(clause, operand_position),
            )?;
        }
        if let Some(list) = clause.list() {
            match (list.open(), list.close()) {
                (Some(open), Some(close)) => {
                    manifest.required(
                        parsed,
                        owner,
                        role(HirFlowContractSourcePart::OpenDelimiter),
                        open.source_span(),
                    )?;
                    manifest.required(
                        parsed,
                        owner,
                        role(HirFlowContractSourcePart::CloseDelimiter),
                        close.source_span(),
                    )?;
                }
                (None, None) => {}
                _ => return Err(flow_state_mismatch(owner)),
            }
        }
    }
    Ok(())
}

fn clause_operands(
    clause: &AttachedFlowContractClause,
) -> &[arcweft_lang_syntax::attachment::AttachedExpressionNode] {
    match clause.operands() {
        AttachedFlowContractOperands::One(expression) => std::slice::from_ref(expression),
        AttachedFlowContractOperands::Many(expressions) => expressions,
    }
}

fn operand_source(clause: &AttachedFlowContractClause, ordinal: usize) -> SourceSpan {
    clause_operands(clause)[ordinal].whole_source_span()
}

fn project_body(
    manifest: &mut FlowManifest,
    parsed: &ParsedSource,
    owner: ItemId,
    body: &AttachedRequiredFlowBody,
) -> Result<(), HirSourceCommitInvariantError> {
    let (whole, open, close) = match body {
        AttachedRequiredFlowBody::Present(body) => (
            body.syntax().source_span(),
            body.open().source_span(),
            body.close().source_span(),
        ),
        AttachedRequiredFlowBody::Missing {
            syntax, insertion, ..
        } => {
            if syntax.source_span() != *insertion {
                return Err(flow_state_mismatch(owner));
            }
            (insertion.clone(), insertion.clone(), insertion.clone())
        }
    };
    for (role, span) in [
        (HirFlowSourceRole::Body, whole),
        (HirFlowSourceRole::BodyOpen, open),
        (HirFlowSourceRole::BodyClose, close),
    ] {
        manifest.required(parsed, owner, role, span)?;
    }
    Ok(())
}

fn exact_flow_manifest(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    attached: &AttachedFlowDeclaration,
) -> bool {
    let Ok(expected) = flow_manifest(parsed, owner, attached) else {
        return false;
    };
    let is_flow_item_query = |query: &&HirSourceQuery| {
        matches!(
            query,
            HirSourceQuery::Item {
                owner: actual,
                role: HirItemSourceRole::Flow(_),
            } if *actual == owner
        )
    };
    index
        .requirements
        .iter()
        .filter(|(query, _)| is_flow_item_query(query))
        .eq(expected.requirements.iter())
        && index
            .components
            .iter()
            .filter(|(query, _)| is_flow_item_query(query))
            .eq(expected.components.iter())
}

/// Re-derives the four-state semantic identity and its recovery bit from the
/// attached owner. This intentionally mirrors the final lowering projection,
/// including empty-marker derivation and malformed authored IDs, instead of
/// comparing only the happy-path authored spelling.
fn flow_identity_matches(
    owner: ItemId,
    flow: &HirFlowItem,
    attached: &AttachedFlowIdentity,
) -> Option<Vec<HirFlowIssue>> {
    let (expected, recovered) = project_flow_identity(attached)?;
    if flow.identity() != &expected {
        return None;
    }
    let mut issues = Vec::new();
    match attached {
        AttachedFlowIdentity::Name { .. } => {}
        AttachedFlowIdentity::PublicId { .. } => {
            if recovered {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Identity,
                    HirFlowSourceRole::PublicId,
                ));
            }
        }
        AttachedFlowIdentity::PublicIdAndName { public_id, name } => {
            if recovered {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Identity,
                    HirFlowSourceRole::PublicId,
                ));
            } else if let HirFlowIdentity::PublicIdAndName {
                public_id: retained,
                name: retained_name,
            } = flow.identity()
                && retained_name.as_str() == name.value().as_str()
                && !flow_id_matches_name(retained, retained_name)
                && !public_id.has_recovery()
            {
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
        }
        AttachedFlowIdentity::Missing {
            attempted_public_id,
            ..
        } => {
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
        }
    }
    Some(issues)
}

fn project_flow_identity(attached: &AttachedFlowIdentity) -> Option<(HirFlowIdentity, bool)> {
    match attached {
        AttachedFlowIdentity::Name { name } => Some((
            HirFlowIdentity::Name {
                name: HirName::try_new(name.value().as_str().into()).ok()?,
            },
            false,
        )),
        AttachedFlowIdentity::PublicId { public_id } => {
            let (public_id, recovered) = project_flow_public_id(public_id, None)?;
            Some((
                public_id
                    .map(|public_id| HirFlowIdentity::PublicId { public_id })
                    .unwrap_or(HirFlowIdentity::Missing),
                recovered,
            ))
        }
        AttachedFlowIdentity::PublicIdAndName { public_id, name } => {
            let name = HirName::try_new(name.value().as_str().into()).ok()?;
            let (public_id, recovered) = project_flow_public_id(public_id, Some(&name))?;
            match public_id {
                Some(public_id) => Some((
                    HirFlowIdentity::PublicIdAndName { public_id, name },
                    recovered,
                )),
                None => Some((HirFlowIdentity::Name { name }, true)),
            }
        }
        AttachedFlowIdentity::Missing { .. } => Some((HirFlowIdentity::Missing, true)),
    }
}

fn project_flow_public_id(
    attached: &AttachedFlowPublicId,
    name: Option<&HirName>,
) -> Option<(Option<HirIdRef>, bool)> {
    let recovered = attached.has_recovery();
    match attached.value() {
        AttachedFlowIdSyntax::Authored(value) => {
            match crate::final_lowering::id_ref_projection::id_ref(value).ok()? {
                HirIdRefValue::Resolved(value) => Some((Some(value), recovered)),
                HirIdRefValue::Recovered(_) => Some((None, true)),
            }
        }
        AttachedFlowIdSyntax::DerivedFromEmptyMarker { marker_family } => {
            let Some(name) = name else {
                return Some((None, true));
            };
            let suffix = HirIdSuffix::try_new(name.as_str().into()).ok()?;
            let relative = HirRelativeId::new(suffix, 0);
            let id = match marker_family {
                Some(family) => {
                    let family = HirIdFamily::try_new(family.as_str().into()).ok()?;
                    HirIdRef::family_relative(HirFamilyRelativeId::new(family, relative))
                }
                None => HirIdRef::relative(relative),
            };
            Some((Some(id), recovered))
        }
    }
}

fn flow_id_matches_name(public_id: &HirIdRef, name: &HirName) -> bool {
    let suffix = match public_id {
        HirIdRef::Absolute(reference) => reference.as_str(),
        HirIdRef::Relative(relative) => relative.suffix().as_str(),
        HirIdRef::FamilyRelative(relative) => relative.relative().suffix().as_str(),
    };
    suffix.rsplit('.').next() == Some(name.as_str())
}

fn flow_return_matches(
    flow: &HirFlowItem,
    attached: &AttachedFlowReturnSyntax,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<bool> {
    match (flow.result(), attached) {
        (crate::item::HirFlowReturn::OmittedUnit, AttachedFlowReturnSyntax::Omitted) => Some(false),
        (
            crate::item::HirFlowReturn::Authored(retained),
            AttachedFlowReturnSyntax::Authored(attached),
        ) => {
            if !type_owner_matches(*retained, attached.ty(), slots)
                || !arenas
                    .types
                    .resolve_prepared(slots, *retained)
                    .is_ok_and(|payload| payload.scope() == flow.callable_scope())
            {
                return None;
            }
            Some(attached.has_recovery() || slot_is_poisoned(slots, *retained))
        }
        _ => None,
    }
}

fn flow_signature_issues(
    owner: ItemId,
    flow: &HirFlowItem,
    attached: &AttachedFlowSignature,
    slots: &SlotSnapshot,
    return_recovery: bool,
) -> Option<Vec<HirFlowIssue>> {
    let mut issues = Vec::new();

    match attached.generics() {
        None if !flow.generic_parameters().is_empty() => return None,
        None => {}
        Some(group) => {
            if group.parameters().len() != flow.generic_parameters().len() {
                return None;
            }
            for (position, (attached, retained)) in group
                .parameters()
                .iter()
                .zip(flow.generic_parameters())
                .enumerate()
            {
                let ordinal = u16::try_from(position).ok()?;
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
                if attached.bounds().len() != retained.bounds().len() {
                    return None;
                }
                for &bound in retained.bounds() {
                    if slot_is_poisoned(slots, bound) {
                        issues.push(flow_owned_issue(
                            owner,
                            HirFlowIssueClass::Signature,
                            HirFlowIssueOwner::Type(bound),
                            role,
                        ));
                        represented = true;
                    }
                }
                if attached.has_recovery() && !represented {
                    issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                }
            }
            if matches!(group.close_state(), AttachedDelimiterState::Missing(_)) {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowSourceRole::GenericGroup,
                ));
            }
        }
    }

    match attached.parameters() {
        None if !flow.parameters().is_empty() => return None,
        None => {}
        Some(group) => {
            if group.parameters().len() != flow.parameters().len() {
                return None;
            }
            if group.open_state().is_missing() {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowSourceRole::ParameterGroup,
                ));
            }
            for (position, (attached, retained)) in
                group.parameters().iter().zip(flow.parameters()).enumerate()
            {
                let ordinal = u16::try_from(position).ok()?;
                let whole = HirFlowSourceRole::Parameter {
                    ordinal,
                    part: HirFlowParameterSourcePart::Whole,
                };
                if !matches!(attached.kind(), AttachedCallableParameterKind::Fixed)
                    || attached.default().is_some()
                {
                    issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, whole));
                }
                if slot_is_poisoned(slots, retained.pattern())
                    || retained
                        .locals()
                        .iter()
                        .any(|local| slot_is_poisoned(slots, *local))
                {
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowIssueOwner::Pattern(retained.pattern()),
                        HirFlowSourceRole::Parameter {
                            ordinal,
                            part: HirFlowParameterSourcePart::Pattern,
                        },
                    ));
                }
                if attached.colon().is_missing() {
                    issues.push(flow_item_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowSourceRole::Parameter {
                            ordinal,
                            part: HirFlowParameterSourcePart::Colon,
                        },
                    ));
                }
                if slot_is_poisoned(slots, retained.ty()) {
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowIssueOwner::Type(retained.ty()),
                        HirFlowSourceRole::Parameter {
                            ordinal,
                            part: HirFlowParameterSourcePart::Type,
                        },
                    ));
                }
            }
            if group.close_state().is_missing() {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::Signature,
                    HirFlowSourceRole::ParameterGroup,
                ));
            }
        }
    }

    for position in 0..attached.recovery().len() {
        issues.push(flow_item_issue(
            owner,
            HirFlowIssueClass::Signature,
            HirFlowSourceRole::TrailingRecovery {
                ordinal: u32::try_from(position).ok()?,
            },
        ));
    }

    if return_recovery {
        let crate::item::HirFlowReturn::Authored(ty) = flow.result() else {
            return None;
        };
        issues.push(flow_owned_issue(
            owner,
            HirFlowIssueClass::Signature,
            HirFlowIssueOwner::Type(*ty),
            HirFlowSourceRole::Return {
                part: HirFlowReturnSourcePart::Type,
            },
        ));
    }

    match attached.where_clause() {
        None if !flow.where_predicates().is_empty() => return None,
        None => {}
        Some(where_clause) => {
            if where_clause.predicates().len() != flow.where_predicates().len() {
                return None;
            }
            for (position, (attached, retained)) in where_clause
                .predicates()
                .iter()
                .zip(flow.where_predicates())
                .enumerate()
            {
                let ordinal = u16::try_from(position).ok()?;
                let role = HirFlowSourceRole::WherePredicate { ordinal };
                let mut represented = false;
                if slot_is_poisoned(slots, retained.subject()) {
                    issues.push(flow_owned_issue(
                        owner,
                        HirFlowIssueClass::Signature,
                        HirFlowIssueOwner::Type(retained.subject()),
                        role,
                    ));
                    represented = true;
                }
                if attached.colon().is_missing() {
                    issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                    represented = true;
                }
                if attached.bounds().len() != retained.bounds().len() {
                    return None;
                }
                for &bound in retained.bounds() {
                    if slot_is_poisoned(slots, bound) {
                        issues.push(flow_owned_issue(
                            owner,
                            HirFlowIssueClass::Signature,
                            HirFlowIssueOwner::Type(bound),
                            role,
                        ));
                        represented = true;
                    }
                }
                if attached.has_recovery() && !represented {
                    issues.push(flow_item_issue(owner, HirFlowIssueClass::Signature, role));
                }
            }
        }
    }

    Some(issues)
}

fn flow_contracts_match(
    owner: ItemId,
    flow: &HirFlowItem,
    attached: &[AttachedFlowContractClause],
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> Option<Vec<HirFlowIssue>> {
    if flow.contracts().len() != attached.len() {
        return None;
    }
    let mut issues = Vec::new();
    let mut first_decreases = None;
    for (position, (retained, attached)) in flow.contracts().iter().zip(attached).enumerate() {
        let ordinal = u16::try_from(position).ok()?;
        if attached.source_ordinal() != ordinal {
            return None;
        }
        let scope = if matches!(attached, AttachedFlowContractClause::Ensures { .. }) {
            flow.ensures_scope()
        } else {
            flow.requires_scope()
        };
        let (attached_operands, retained_operands) = match (retained, attached) {
            (
                HirFlowContractClause::Requires(retained),
                AttachedFlowContractClause::Requires { condition, .. },
            )
            | (
                HirFlowContractClause::Ensures(retained),
                AttachedFlowContractClause::Ensures { condition, .. },
            )
            | (
                HirFlowContractClause::Invariant(retained),
                AttachedFlowContractClause::Invariant { condition, .. },
            ) => {
                if retained.mode() != contract_mode(condition.mode())
                    || !source_expression_matches(
                        slots,
                        arenas.expressions,
                        retained.expression(),
                        condition.expression(),
                        scope,
                    )
                {
                    return None;
                }
                (vec![condition.expression()], vec![retained.expression()])
            }
            (
                HirFlowContractClause::Assume {
                    expression: retained,
                },
                AttachedFlowContractClause::Assume {
                    expression: attached,
                    ..
                },
            )
            | (
                HirFlowContractClause::NoEffect {
                    expression: retained,
                },
                AttachedFlowContractClause::NoEffect {
                    expression: attached,
                    ..
                },
            )
            | (
                HirFlowContractClause::Decreases {
                    expression: retained,
                },
                AttachedFlowContractClause::Decreases {
                    expression: attached,
                    ..
                },
            ) => {
                if !source_expression_matches(slots, arenas.expressions, *retained, attached, scope)
                {
                    return None;
                }
                (vec![attached], vec![*retained])
            }
            (
                HirFlowContractClause::Reads(retained),
                AttachedFlowContractClause::Reads {
                    operands: attached, ..
                },
            )
            | (
                HirFlowContractClause::Effects(retained),
                AttachedFlowContractClause::Effects {
                    operands: attached, ..
                },
            )
            | (
                HirFlowContractClause::Modifies(retained),
                AttachedFlowContractClause::Modifies {
                    operands: attached, ..
                },
            ) => {
                if retained.operands().len() != attached.operands().len() {
                    return None;
                }
                for (&retained, attached) in retained.operands().iter().zip(attached.operands()) {
                    if !source_expression_matches(
                        slots,
                        arenas.expressions,
                        retained,
                        attached,
                        scope,
                    ) {
                        return None;
                    }
                }
                (
                    attached.operands().iter().collect::<Vec<_>>(),
                    retained.operands().to_vec(),
                )
            }
            _ => return None,
        };

        let issue_start = issues.len();
        if matches!(attached, AttachedFlowContractClause::Decreases { .. }) {
            if let Some(first) = first_decreases {
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
        if attached
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
        if attached_operands.len() != retained_operands.len() {
            return None;
        }
        for (position, (attached_operand, retained_operand)) in attached_operands
            .into_iter()
            .zip(retained_operands)
            .enumerate()
        {
            if attached_operand.projection().has_recovery()
                || slot_is_poisoned(slots, retained_operand)
            {
                issues.push(flow_owned_issue(
                    owner,
                    HirFlowIssueClass::Contract,
                    HirFlowIssueOwner::Expr(retained_operand),
                    HirFlowSourceRole::ContractClause {
                        ordinal,
                        part: HirFlowContractSourcePart::Operand {
                            ordinal: u16::try_from(position).ok()?,
                        },
                    },
                ));
            }
        }
        if matches!(
            attached.list().and_then(|list| list.close_state()),
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
        if attached.has_recovery() && issues.len() == issue_start {
            issues.push(flow_item_issue(
                owner,
                HirFlowIssueClass::Contract,
                HirFlowSourceRole::ContractClause {
                    ordinal,
                    part: HirFlowContractSourcePart::Whole,
                },
            ));
        }
    }
    Some(issues)
}

const fn contract_mode(mode: &AttachedFlowContractMode) -> HirContractMode {
    match mode {
        AttachedFlowContractMode::Default => HirContractMode::Default,
        AttachedFlowContractMode::Prove(_) => HirContractMode::Prove,
        AttachedFlowContractMode::Check(_) => HirContractMode::CheckRuntime,
        AttachedFlowContractMode::Debug(_) => HirContractMode::DebugCheck,
    }
}

fn flow_scope_graph_matches(
    owner: ItemId,
    item: &HirItem,
    flow: &HirFlowItem,
    attached: &AttachedFlowDeclaration,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let requires_source = attached
        .contracts()
        .iter()
        .find(|clause| !matches!(clause, AttachedFlowContractClause::Ensures { .. }))
        .map_or_else(
            || attached.signature().end().clone(),
            |clause| clause.keyword().clone(),
        );
    let ensures_source = attached
        .contracts()
        .iter()
        .find(|clause| matches!(clause, AttachedFlowContractClause::Ensures { .. }))
        .map_or_else(
            || attached.signature().end().clone(),
            |clause| clause.keyword().clone(),
        );
    if !contract_scopes_match(
        owner,
        CallableScopeSource {
            syntax: attached.syntax().id(),
            item: &attached.syntax().source_span(),
            requires: &requires_source,
            ensures: &ensures_source,
        },
        CallableScopeIds {
            item: item.scope(),
            callable: flow.callable_scope(),
            requires: flow.requires_scope(),
            ensures: flow.ensures_scope(),
        },
        parsed,
        slots,
        arenas,
    ) {
        return false;
    }
    let Ok(callable) = arenas.scopes.resolve_prepared(slots, flow.callable_scope()) else {
        return false;
    };
    let (body_syntax, body_source) = match attached.body() {
        AttachedRequiredFlowBody::Present(body) => (
            body.syntax().id(),
            HirSourceSite::Span(body.syntax().source_span()),
        ),
        AttachedRequiredFlowBody::Missing { syntax, .. } => {
            let Ok(source) =
                HirSourceSite::from_attached_span(parsed.document(), &syntax.source_span())
            else {
                return false;
            };
            (syntax.id(), source)
        }
    };
    direct_children_are_exact(
        flow.callable_scope(),
        flow.requires_scope(),
        flow.ensures_scope(),
        flow.body_scope(),
        callable,
        slots,
        arenas,
    ) && item_body_scope_matches_at_site(
        owner,
        flow.callable_scope(),
        flow.body_scope(),
        HirScopeKind::Flow,
        body_syntax,
        body_source,
        slots,
        arenas,
    )
}

fn flow_result_local_matches(
    flow: &HirFlowItem,
    attached: &AttachedFlowDeclaration,
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &ItemValidationArenas<'_>,
) -> bool {
    let has_ensures = attached
        .contracts()
        .iter()
        .any(|clause| matches!(clause, AttachedFlowContractClause::Ensures { .. }));
    let expected_source = has_ensures.then(|| attached.signature().end().clone());
    if !postcondition_result_matches(
        expected_source,
        flow.ensures_scope(),
        flow.result().authored_type(),
        parsed,
        slots,
        arenas,
    ) {
        return false;
    }
    let Ok(ensures) = arenas.scopes.resolve_prepared(slots, flow.ensures_scope()) else {
        return false;
    };
    match (has_ensures, flow.result_local(), ensures.locals()) {
        (false, None, []) => true,
        (true, Some(result), [local]) => result.local() == *local,
        _ => false,
    }
}

fn flow_body_matches(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ItemId,
    flow: &HirFlowItem,
    attached: &AttachedRequiredFlowBody,
    slots: &SlotSnapshot,
) -> Option<Vec<HirFlowIssue>> {
    if !index.validates_attached_flow_thread_body(
        parsed,
        slots,
        HirThreadBodyOwner::Flow(owner),
        attached,
        flow.body(),
    ) {
        return None;
    }
    match attached {
        AttachedRequiredFlowBody::Missing { .. } => {
            if !flow.body().items().is_empty() {
                return None;
            }
            Some(vec![flow_item_issue(
                owner,
                HirFlowIssueClass::MissingBody,
                HirFlowSourceRole::Body,
            )])
        }
        AttachedRequiredFlowBody::Present(body) => {
            if body.items().len() != flow.body().items().len() {
                return None;
            }
            let mut issues = Vec::new();
            for (position, (attached, retained)) in
                body.items().iter().zip(flow.body().items()).enumerate()
            {
                if attached.has_recovery() || flow_child_is_poisoned(slots, retained) {
                    let ordinal = u32::try_from(position).ok()?;
                    issues.push(HirFlowIssue::new(
                        HirFlowIssueClass::BodyChild,
                        flow_child_issue_owner(retained),
                        HirSourceQuery::ThreadBody {
                            owner: HirThreadBodyOwner::Flow(owner),
                            role: HirThreadBodySourceRole::Item {
                                ordinal,
                                part: HirThreadFlowItemSourcePart::ChildWhole,
                            },
                        },
                    ));
                }
            }
            if matches!(body.close_state(), AttachedDelimiterState::Missing(_)) {
                issues.push(flow_item_issue(
                    owner,
                    HirFlowIssueClass::UnclosedBody,
                    HirFlowSourceRole::BodyClose,
                ));
            }
            Some(issues)
        }
    }
}

fn flow_child_is_poisoned(slots: &SlotSnapshot, item: &HirThreadFlowItem) -> bool {
    match item {
        HirThreadFlowItem::DialogueApplication(owner) => slot_is_poisoned(slots, *owner),
        HirThreadFlowItem::Statement(owner)
        | HirThreadFlowItem::Choice(owner)
        | HirThreadFlowItem::If(owner)
        | HirThreadFlowItem::IfLet(owner)
        | HirThreadFlowItem::Match(owner)
        | HirThreadFlowItem::Loop(owner)
        | HirThreadFlowItem::While(owner)
        | HirThreadFlowItem::WhileLet(owner)
        | HirThreadFlowItem::For(owner)
        | HirThreadFlowItem::Select(owner)
        | HirThreadFlowItem::SourceLocale(owner)
        | HirThreadFlowItem::Scope(owner)
        | HirThreadFlowItem::Include(owner)
        | HirThreadFlowItem::AwaitWith(owner)
        | HirThreadFlowItem::Error(owner) => slot_is_poisoned(slots, *owner),
    }
}

const fn flow_child_issue_owner(item: &HirThreadFlowItem) -> HirFlowIssueOwner {
    match item {
        HirThreadFlowItem::DialogueApplication(owner) => HirFlowIssueOwner::Expr(*owner),
        HirThreadFlowItem::Statement(owner)
        | HirThreadFlowItem::Choice(owner)
        | HirThreadFlowItem::If(owner)
        | HirThreadFlowItem::IfLet(owner)
        | HirThreadFlowItem::Match(owner)
        | HirThreadFlowItem::Loop(owner)
        | HirThreadFlowItem::While(owner)
        | HirThreadFlowItem::WhileLet(owner)
        | HirThreadFlowItem::For(owner)
        | HirThreadFlowItem::Select(owner)
        | HirThreadFlowItem::SourceLocale(owner)
        | HirThreadFlowItem::Scope(owner)
        | HirThreadFlowItem::Include(owner)
        | HirThreadFlowItem::AwaitWith(owner)
        | HirThreadFlowItem::Error(owner) => HirFlowIssueOwner::Stmt(*owner),
    }
}

fn flow_poison_matches(item: &HirItem, flow: &HirFlowItem, issues: Vec<HirFlowIssue>) -> bool {
    let expected = HirFlowPoison::from_ordered_issues(issues.into_boxed_slice());
    let item_issue = expected.primary().map(|issue| match issue.class() {
        HirFlowIssueClass::MissingBody => HirItemIssue::MissingBody,
        HirFlowIssueClass::Prefix | HirFlowIssueClass::Identity | HirFlowIssueClass::Signature => {
            HirItemIssue::MalformedHeader
        }
        HirFlowIssueClass::Contract
        | HirFlowIssueClass::BodyChild
        | HirFlowIssueClass::UnclosedBody
        | HirFlowIssueClass::TrailingRecovery => HirItemIssue::Recovery,
    });
    flow.poison() == &expected && item.state() == &item_state(item_issue)
}

const fn flow_item_issue(
    owner: ItemId,
    class: HirFlowIssueClass,
    role: HirFlowSourceRole,
) -> HirFlowIssue {
    flow_owned_issue(owner, class, HirFlowIssueOwner::Item(owner), role)
}

const fn flow_owned_issue(
    owner: ItemId,
    class: HirFlowIssueClass,
    issue_owner: HirFlowIssueOwner,
    role: HirFlowSourceRole,
) -> HirFlowIssue {
    HirFlowIssue::new(class, issue_owner, flow_query(owner, role))
}

fn block_arenas<'arena>(arenas: &ItemValidationArenas<'arena>) -> BlockValidationArenas<'arena> {
    BlockValidationArenas {
        expressions: arenas.expressions,
        statements: arenas.statements,
        scopes: arenas.scopes,
        locals: arenas.locals,
        patterns: arenas.patterns,
    }
}

fn flow_source_ordinals_match(flow: &HirFlowItem, attached: &AttachedFlowDeclaration) -> bool {
    attached
        .contracts()
        .iter()
        .enumerate()
        .all(|(position, clause)| usize::from(clause.source_ordinal()) == position)
        && attached.signature().parameters().is_none_or(|group| {
            group
                .parameters()
                .iter()
                .enumerate()
                .all(|(position, parameter)| {
                    usize::from(parameter.source_ordinal()) == position
                        && parameter.group_ordinal() == 0
                        && usize::from(parameter.parameter_ordinal()) == position
                })
        })
        && flow.parameters().len()
            == attached
                .signature()
                .parameters()
                .map_or(0, |group| group.parameters().len())
}

fn flow_scope_ids_belong_to_module(flow: &HirFlowItem, owner: ItemId) -> bool {
    [
        flow.callable_scope(),
        flow.requires_scope(),
        flow.ensures_scope(),
        flow.body_scope(),
    ]
    .into_iter()
    .all(|scope| scope.module() == owner.module())
}

const fn flow_query(owner: ItemId, role: HirFlowSourceRole) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Flow(role),
    }
}

const fn flow_state_mismatch(owner: ItemId) -> HirSourceCommitInvariantError {
    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
        owner: SyntheticOwner::Item(owner),
    }
}

impl HirFlowItem {
    pub(crate) fn validate_source_role(
        &self,
        owner: ItemId,
        role: HirFlowSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        match role {
            HirFlowSourceRole::Whole
            | HirFlowSourceRole::Keyword
            | HirFlowSourceRole::Visibility
            | HirFlowSourceRole::PublicId
            | HirFlowSourceRole::Name
            | HirFlowSourceRole::GenericGroup
            | HirFlowSourceRole::ParameterGroup
            | HirFlowSourceRole::Return { .. }
            | HirFlowSourceRole::WhereClause
            | HirFlowSourceRole::Body
            | HirFlowSourceRole::BodyOpen
            | HirFlowSourceRole::BodyClose
            | HirFlowSourceRole::TrailingRecovery { .. } => Ok(()),
            HirFlowSourceRole::GenericParameter { ordinal } => {
                validate_item_ordinal(owner, role, ordinal, self.generic_parameters().len())
            }
            HirFlowSourceRole::Parameter { ordinal, .. } => {
                validate_item_ordinal(owner, role, ordinal, self.parameters().len())
            }
            HirFlowSourceRole::WherePredicate { ordinal } => {
                validate_item_ordinal(owner, role, ordinal, self.where_predicates().len())
            }
            HirFlowSourceRole::ContractClause { ordinal, part } => {
                validate_item_ordinal(owner, role, ordinal, self.contracts().len())?;
                let clause = &self.contracts()[usize::from(ordinal)];
                validate_contract_part(owner, role, clause, part)
            }
        }
    }
}

impl HirSourceIndex {
    /// Validates one Flow role against semantic cardinality and the exact
    /// committed source manifest before the caller checks source identity.
    pub(crate) fn validate_flow_source_role(
        &self,
        flow: &HirFlowItem,
        owner: ItemId,
        role: HirFlowSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        flow.validate_source_role(owner, role)?;
        if let HirFlowSourceRole::TrailingRecovery { ordinal } = role {
            let length = u32::try_from(
                self.requirements
                    .keys()
                    .filter(|query| {
                        matches!(
                            query,
                            HirSourceQuery::Item {
                                owner: actual,
                                role: HirItemSourceRole::Flow(
                                    HirFlowSourceRole::TrailingRecovery { .. }
                                ),
                            } if *actual == owner
                        )
                    })
                    .count(),
            )
            .expect("Flow recovery source rows are bounded below u32::MAX");
            if ordinal >= length {
                return Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                    owner,
                    role: HirItemSourceRole::Flow(role),
                    length,
                });
            }
        }
        if role != HirFlowSourceRole::Whole && self.requirement(&flow_query(owner, role)).is_none()
        {
            return Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::Flow(role),
            });
        }
        Ok(())
    }
}

fn validate_contract_part(
    owner: ItemId,
    role: HirFlowSourceRole,
    clause: &HirFlowContractClause,
    part: HirFlowContractSourcePart,
) -> Result<(), HirSourceQueryError> {
    match part {
        HirFlowContractSourcePart::Whole | HirFlowContractSourcePart::ClauseKeyword => Ok(()),
        HirFlowContractSourcePart::NoEffectKeyword
            if matches!(clause, HirFlowContractClause::NoEffect { .. }) =>
        {
            Ok(())
        }
        HirFlowContractSourcePart::Mode
            if matches!(
                clause,
                HirFlowContractClause::Requires(condition)
                    | HirFlowContractClause::Ensures(condition)
                    | HirFlowContractClause::Invariant(condition)
                    if condition.mode() != HirContractMode::Default
            ) =>
        {
            Ok(())
        }
        HirFlowContractSourcePart::Operand { ordinal } => {
            validate_item_ordinal(owner, role, ordinal, contract_operand_count(clause))
        }
        HirFlowContractSourcePart::OpenDelimiter | HirFlowContractSourcePart::CloseDelimiter
            if matches!(
                clause,
                HirFlowContractClause::Reads(_)
                    | HirFlowContractClause::Effects(_)
                    | HirFlowContractClause::Modifies(_)
            ) =>
        {
            Ok(())
        }
        HirFlowContractSourcePart::NoEffectKeyword
        | HirFlowContractSourcePart::Mode
        | HirFlowContractSourcePart::OpenDelimiter
        | HirFlowContractSourcePart::CloseDelimiter => {
            Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner,
                role: HirItemSourceRole::Flow(role),
            })
        }
    }
}

const fn contract_operand_count(clause: &HirFlowContractClause) -> usize {
    match clause {
        HirFlowContractClause::Requires(_)
        | HirFlowContractClause::Ensures(_)
        | HirFlowContractClause::Invariant(_)
        | HirFlowContractClause::Assume { .. }
        | HirFlowContractClause::NoEffect { .. }
        | HirFlowContractClause::Decreases { .. } => 1,
        HirFlowContractClause::Reads(operands)
        | HirFlowContractClause::Effects(operands)
        | HirFlowContractClause::Modifies(operands) => operands.operands().len(),
    }
}

fn validate_item_ordinal(
    owner: ItemId,
    role: HirFlowSourceRole,
    ordinal: u16,
    length: usize,
) -> Result<(), HirSourceQueryError> {
    if usize::from(ordinal) < length {
        return Ok(());
    }
    Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
        owner,
        role: HirItemSourceRole::Flow(role),
        length: u32::try_from(length)
            .expect("Flow source ordinal lengths are bounded below u32::MAX"),
    })
}
