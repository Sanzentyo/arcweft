//! Attached E01-E07 expression source manifests and freeze validation.

mod call;
pub(in crate::source_index) mod candidate_projection;
mod dialogue_projection;
pub(super) mod leaf;
pub(super) mod projection;
mod requirements;

use self::projection::{expression_children_match, expression_payload_matches};
use self::requirements::{candidate_dialogue_requirements, expression_requirements};

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::attachment::node::MissingExpressionKind;
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedCandidateGraph, AttachedExpressionNode, RequiredStatementExpressionNode,
};
use arcweft_lang_syntax::expressions::{
    ExpressionComponentRole, ExpressionLiteralPart, ExpressionProjection,
    ExpressionRecordFieldPart, SyntaxCallArgumentPart, SyntaxCallTypeApplicationComponentRole,
    SyntaxCallTypeArgumentPart, SyntaxClosureParameterPart,
    SyntaxDialogueConfigurationArgumentPart, SyntaxDialogueNodeSourcePart, SyntaxMatchArmPart,
    SyntaxRichTextArgumentSourcePart, SyntaxRichTextTagSourcePart,
};
use arcweft_lang_syntax::id_ref::SyntaxIdRefPart;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::types::TypeRefComponentRole;
use arcweft_source::SourceDocumentIdentity;

use super::{
    HirCallArgumentSourcePart, HirCallTypeApplicationSourceRole, HirCallTypeArgumentSourcePart,
    HirClosureParameterSourcePart, HirDialogueNodeSourcePart, HirExprSourceRole,
    HirIdRefSourcePart, HirMatchArmSourcePart, HirRecordFieldSourcePart,
    HirRichTextArgumentSourcePart, HirRichTextTagSourcePart, HirSourceCommitInvariantError,
    HirSourceIndex, HirSourcePresence, HirSourceQuery, HirSourceRequirement, HirSourceSite,
    StagedHirSourceIndex, validate_component_source,
};
use crate::arena::ArenaSnapshot;
use crate::dialogue_application::HirDialogueContentApplication;
use crate::expr::{
    HirCallArgumentOrdinal, HirCallTypeArgumentOrdinal, HirExpr, HirExprKind, HirGenericExprIssue,
    HirPoisonState, HirRecoveryIssue, HirRecoveryOperandSlot,
};
use crate::identity::{ExprId, ItemId, SyntheticOwner, SyntheticRole, TypeId};
use crate::item::HirItem;
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::type_ref::HirType;

impl StagedHirSourceIndex {
    /// Binds one zero-width parser-owned missing expression as a source-backed
    /// expression owner. Its whole insertion remains exclusively on the slot.
    #[allow(
        clippy::result_large_err,
        reason = "missing-expression staging preserves complete typed owner and source evidence"
    )]
    pub(crate) fn stage_attached_missing_expression(
        &mut self,
        parsed: &ParsedSource,
        owner: ExprId,
        attached: &AstNode<MissingExpressionKind>,
        payload: &HirExpr,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.syntax().snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.syntax().snapshot_id().clone(),
            });
        }
        let site = HirSourceSite::from_attached_span(parsed.document(), &attached.source_span())?;
        if !matches!(site, HirSourceSite::Insertion(_))
            || !missing_expression_payload_matches(payload)
        {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        }
        self.bind_syntax_owner(SyntheticOwner::Expr(owner), attached.id())
    }

    /// Projects one final leaf-expression manifest from the exact attached
    /// grammar transaction. `Whole` remains owned exclusively by slot metadata.
    #[allow(
        clippy::result_large_err,
        reason = "expression staging preserves complete typed owner and source evidence"
    )]
    pub(crate) fn stage_attached_expression(
        &mut self,
        parsed: &ParsedSource,
        owner: ExprId,
        attached: &AttachedExpressionNode,
        payload: &HirExprKind,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }
        if !expression_payload_matches(payload, attached) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        }
        let Some(requirements) = expression_requirements(payload, attached.projection()) else {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        };
        let components =
            match expression_component_sites(&self.source, parsed, owner, payload, attached) {
                Ok(components) => components,
                Err(error) => return self.reject(error),
            };
        let present = components.keys().copied().collect::<BTreeSet<_>>();

        if let Some(role) = present
            .iter()
            .find(|role| !requirements.contains_key(role))
            .copied()
        {
            return self.reject(HirSourceCommitInvariantError::UndeclaredComponent {
                query: HirSourceQuery::Expr { owner, role },
            });
        }
        if let Some(role) = requirements
            .iter()
            .find(|(role, requirement)| {
                **requirement == HirSourceRequirement::Required && !present.contains(role)
            })
            .map(|(role, _)| *role)
        {
            return self.reject(HirSourceCommitInvariantError::MissingRequiredComponent {
                query: HirSourceQuery::Expr { owner, role },
            });
        }

        self.bind_syntax_owner(SyntheticOwner::Expr(owner), attached.id())?;
        for (role, requirement) in requirements {
            self.require(&HirSourceQuery::Expr { owner, role }, requirement)?;
        }
        for (role, site) in components {
            self.stage(&HirSourceQuery::Expr { owner, role }, site)?;
        }
        Ok(())
    }

    /// Freezes the source-role manifest for the Dialogue interpretation of an
    /// ambiguous postfix expression.
    ///
    /// The final E33 candidate is keyed by its exact synthetic `ExprId`, while
    /// every span is projected from the source-backed outer E34 owner. No
    /// candidate syntax identity or reconstructed source reader is created.
    #[allow(
        clippy::result_large_err,
        reason = "candidate staging preserves complete typed owner and source evidence"
    )]
    pub(crate) fn stage_candidate_dialogue_expression(
        &mut self,
        parsed: &ParsedSource,
        outer: ExprId,
        owner: ExprId,
        attached: &AttachedExpressionNode,
        graph: AttachedCandidateGraph<'_>,
        application: &HirDialogueContentApplication,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }
        let Some(content) = graph.dialogue_content() else {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        };
        if outer == owner
            || application.content().id().owner() != owner
            || application.plan().is_some()
            || !matches!(
                attached.projection(),
                ExpressionProjection::PostfixBracket(
                    arcweft_lang_syntax::expressions::SyntaxPostfixBracketProjection::Ambiguous { .. }
                )
            )
        {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        }

        let requirements = candidate_dialogue_requirements(application, content);
        let components = match candidate_dialogue_component_sites(
            &self.source,
            |query| match self.component_presence(query) {
                Some(HirSourcePresence::Present(site)) => Some(site.clone()),
                Some(HirSourcePresence::AbsentOptional) | None => None,
            },
            parsed,
            owner,
            attached,
            graph,
            application,
        ) {
            Ok(components) => components,
            Err(error) => return self.reject(error),
        };
        let present = components.keys().copied().collect::<BTreeSet<_>>();
        if let Some(role) = present
            .iter()
            .find(|role| !requirements.contains_key(role))
            .copied()
        {
            return self.reject(HirSourceCommitInvariantError::UndeclaredComponent {
                query: HirSourceQuery::Expr { owner, role },
            });
        }
        if let Some(role) = requirements
            .iter()
            .find(|(role, requirement)| {
                **requirement == HirSourceRequirement::Required && !present.contains(role)
            })
            .map(|(role, _)| *role)
        {
            return self.reject(HirSourceCommitInvariantError::MissingRequiredComponent {
                query: HirSourceQuery::Expr { owner, role },
            });
        }

        for (role, requirement) in requirements {
            self.require(&HirSourceQuery::Expr { owner, role }, requirement)?;
        }
        for (role, site) in components {
            self.stage(&HirSourceQuery::Expr { owner, role }, site)?;
        }
        Ok(())
    }
}

impl HirSourceIndex {
    /// Re-derives every source-backed expression manifest from the exact
    /// accepted syntax snapshot and rejects source rows on synthetic owners.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "one exhaustive projection validates every source-backed and synthetic expression owner across the complete arena context"
    )]
    pub(crate) fn validates_attached_expressions(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        items: &ArenaSnapshot<HirItem, ItemId>,
        expressions: &ArenaSnapshot<HirExpr, ExprId>,
        types: &ArenaSnapshot<HirType, TypeId>,
        statements: &ArenaSnapshot<crate::stmt::HirStmt, crate::identity::StmtId>,
        scopes: &ArenaSnapshot<crate::scope::HirScope, crate::identity::ScopeId>,
        locals: &ArenaSnapshot<crate::scope::HirLocal, crate::identity::LocalId>,
        patterns: &ArenaSnapshot<crate::pattern::HirPattern, crate::identity::PatternId>,
    ) -> bool {
        let Some(local_resolver) =
            crate::module::HirLocalResolver::prepared(slots, scopes, locals, statements)
        else {
            return false;
        };
        let block_arenas = super::block_projection::BlockValidationArenas {
            expressions,
            statements,
            scopes,
            locals,
            patterns,
        };
        let Ok(entries) = expressions.try_iter_prepared(slots) else {
            return false;
        };
        let entries = entries.collect::<Vec<_>>();
        let expression_rows = ExpressionManifestRows::from_index(self);
        let Some(retained_style_expressions) =
            super::item_projection::retained_style_expression_owners(items, slots)
        else {
            return false;
        };
        let Some(candidate_expressions) = candidate_projection::validate_candidate_expressions(
            self,
            &expression_rows,
            parsed,
            slots,
            expressions,
            statements,
            types,
            scopes,
            locals,
            patterns,
            &local_resolver,
            &retained_style_expressions,
        ) else {
            return false;
        };
        if !entries.iter().all(|(owner, payload)| {
            let owner = *owner;
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            match metadata.origin() {
                HirOrigin::Source(source) => match parsed.attached_expression(source.syntax()) {
                    Ok(attached) => {
                        self.syntax_owners
                            .get(&SyntheticOwner::Expr(owner))
                            .is_some_and(|syntax| *syntax == attached.id())
                            && metadata.source_site()
                                == &HirSourceSite::Span(attached.whole_source_span())
                            && expression_payload_matches(payload.kind(), &attached)
                            && expression_manifest_matches(
                                &expression_rows,
                                parsed,
                                owner,
                                payload.kind(),
                                &attached,
                            )
                            && expression_children_match(
                                self,
                                parsed,
                                slots,
                                &block_arenas,
                                &local_resolver,
                                types,
                                owner,
                                payload,
                                &attached,
                            )
                    }
                    Err(_) => parsed
                        .typed_node::<MissingExpressionKind>(source.syntax())
                        .is_ok_and(|attached| {
                            let Ok(expected_site) = HirSourceSite::from_attached_span(
                                parsed.document(),
                                &attached.source_span(),
                            ) else {
                                return false;
                            };
                            self.syntax_owners
                                .get(&SyntheticOwner::Expr(owner))
                                .is_some_and(|syntax| *syntax == attached.id())
                                && metadata.source_site() == &expected_site
                                && matches!(expected_site, HirSourceSite::Insertion(_))
                                && missing_expression_payload_matches(payload)
                                && expression_rows.owner_has_no_rows(owner)
                        }),
                },
                HirOrigin::Synthetic(key) => {
                    let candidate_role = matches!(
                        key.role(),
                        SyntheticRole::PostfixIndexCandidateExpression
                            | SyntheticRole::DialogueContentCandidateExpression
                    );
                    let candidate_dialogue_source = key.role()
                        == SyntheticRole::DialogueContentCandidateExpression
                        && key.ordinal() == 0
                        && matches!(payload.kind(), HirExprKind::DialogueContentApplication(_));
                    (!candidate_role || candidate_expressions.contains(&owner))
                        && (expression_rows.has_owner(owner) == candidate_dialogue_source)
                }
            }
        }) {
            return false;
        }

        entries.iter().all(|(child, _)| {
            expr_recovery_operand_is_referenced(parsed, slots, expressions, *child)
        })
    }
}

/// Validation-local grouping of the immutable expression source rows.
///
/// The committed [`HirSourceIndex`] remains the only source authority. This
/// borrowed projection prevents the exhaustive validator from rescanning every
/// source row once per expression owner.
pub(super) struct ExpressionManifestRows<'index> {
    source: &'index SourceDocumentIdentity,
    requirements: BTreeMap<ExprId, BTreeMap<HirExprSourceRole, HirSourceRequirement>>,
    components: BTreeMap<ExprId, BTreeMap<HirExprSourceRole, &'index HirSourceSite>>,
    source_owners: BTreeSet<SyntheticOwner>,
}

impl<'index> ExpressionManifestRows<'index> {
    fn from_index(index: &'index HirSourceIndex) -> Self {
        let mut rows = Self {
            source: &index.source,
            requirements: BTreeMap::new(),
            components: BTreeMap::new(),
            source_owners: index.syntax_owners.keys().copied().collect(),
        };
        for (query, requirement) in index.requirements.iter() {
            rows.source_owners.insert(query.owner());
            if let HirSourceQuery::Expr { owner, role } = query {
                rows.requirements
                    .entry(*owner)
                    .or_default()
                    .insert(*role, *requirement);
            }
        }
        for (query, site) in index.components.iter() {
            rows.source_owners.insert(query.owner());
            if let HirSourceQuery::Expr { owner, role } = query {
                rows.components
                    .entry(*owner)
                    .or_default()
                    .insert(*role, site);
            }
        }
        rows
    }

    fn owner_has_no_rows(&self, owner: ExprId) -> bool {
        !self.requirements.contains_key(&owner) && !self.components.contains_key(&owner)
    }

    pub(super) fn has_typed_owner(&self, owner: SyntheticOwner) -> bool {
        self.source_owners.contains(&owner)
    }

    fn has_owner(&self, owner: ExprId) -> bool {
        self.has_typed_owner(SyntheticOwner::Expr(owner))
    }

    fn requirements_match(
        &self,
        owner: ExprId,
        expected: &BTreeMap<HirExprSourceRole, HirSourceRequirement>,
    ) -> bool {
        self.requirements
            .get(&owner)
            .map_or_else(|| expected.is_empty(), |actual| actual == expected)
    }

    fn components_match(
        &self,
        owner: ExprId,
        expected: &BTreeMap<HirExprSourceRole, HirSourceSite>,
    ) -> bool {
        self.components.get(&owner).map_or_else(
            || expected.is_empty(),
            |actual| {
                actual.len() == expected.len()
                    && expected.iter().all(|(role, expected_site)| {
                        actual
                            .get(role)
                            .is_some_and(|actual_site| *actual_site == expected_site)
                    })
            },
        )
    }
}

fn missing_expression_payload_matches(payload: &HirExpr) -> bool {
    matches!(
        (payload.kind(), payload.state()),
        (
            HirExprKind::Error(error),
            crate::expr::HirPoisonState::Poisoned(crate::expr::HirRecoveryIssue::MissingOperand {
                role: HirExprSourceRole::Whole,
            })
        ) if error.issue() == crate::expr::HirGenericExprIssue::TransactionalChildFailure
    )
}

fn expr_recovery_operand_is_referenced(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    child: ExprId,
) -> bool {
    let Ok(child_metadata) = slots.resolve_prepared(child) else {
        return false;
    };
    let HirOrigin::Synthetic(key) = child_metadata.origin() else {
        // Source-backed expressions are independently frozen against their
        // exact attached owner above. Parent item/statement/expression
        // validators own reachability; only synthetic Expr-to-Expr recovery
        // operands need this additional owner-edge check.
        return true;
    };
    let SyntheticOwner::Expr(parent) = key.owner() else {
        // Statement-owned recovery expressions are checked by the statement
        // manifest; this expression pass owns only Expr-to-Expr reachability.
        return true;
    };
    if key.role() != SyntheticRole::RecoveryOperand {
        return true;
    }

    let Ok(parent_metadata) = slots.resolve_prepared(parent) else {
        return false;
    };
    let Ok(parent_payload) = expressions.resolve_prepared(slots, parent) else {
        return false;
    };
    let Ok(child_payload) = expressions.resolve_prepared(slots, child) else {
        return false;
    };
    let parent_attached = match parent_metadata.origin() {
        HirOrigin::Source(source) => parsed.attached_expression(source.syntax()).ok(),
        HirOrigin::Synthetic(_) => None,
    };
    let choice_slot = match (parent_payload.kind(), parent_attached.as_ref()) {
        (HirExprKind::Choice(_), Some(attached)) => attached.choice().and_then(|choice| {
            let ordinal = usize::try_from(key.ordinal()).ok()?;
            let syntax_slots = choice.required_expression_slots();
            syntax_slots.get(ordinal).copied()
        }),
        _ => None,
    };
    let retained_by_parent = match parent_payload.kind().recovery_operand_slot(key.ordinal()) {
        Some(HirRecoveryOperandSlot::Retained(expected)) => expected == child,
        Some(HirRecoveryOperandSlot::SyntheticOnly) => true,
        None => false,
    };
    let expected_scope = match (parent_payload.kind(), key.ordinal()) {
        (HirExprKind::IfLet(expression), 1 | 2) => expression.scope(),
        _ => parent_payload.scope(),
    };
    let scope_matches = matches!(parent_payload.kind(), HirExprKind::Choice(_))
        || expected_scope == child_payload.scope();
    if !parent_payload.is_poisoned() || !scope_matches || !retained_by_parent {
        return false;
    }

    match (parent_metadata.origin(), choice_slot) {
        (HirOrigin::Source(_), Some(RequiredStatementExpressionNode::Missing(missing))) => {
            let Ok(expected_site) =
                HirSourceSite::from_attached_span(parsed.document(), &missing.source_span())
            else {
                return false;
            };
            child_metadata.source_site() == &expected_site
                && matches!(
                    (child_payload.kind(), child_payload.state()),
                    (
                        HirExprKind::Error(error),
                        HirPoisonState::Poisoned(HirRecoveryIssue::MissingOperand {
                            role: HirExprSourceRole::Recovery,
                        })
                    ) if error.issue() == HirGenericExprIssue::TransactionalChildFailure
                )
        }
        (HirOrigin::Source(_), Some(RequiredStatementExpressionNode::Expression(_))) => false,
        (HirOrigin::Source(_), None) if matches!(parent_payload.kind(), HirExprKind::Choice(_)) => {
            false
        }
        (HirOrigin::Synthetic(_), _) => true,
        (HirOrigin::Source(_), None) => {
            let Some(attached) = parent_attached else {
                return false;
            };
            attached
                .children()
                .iter()
                .find(|candidate| candidate.ordinal() == key.ordinal())
                .is_some_and(|candidate| candidate.missing().is_some())
        }
    }
}

fn expression_manifest_matches(
    rows: &ExpressionManifestRows<'_>,
    parsed: &ParsedSource,
    owner: ExprId,
    payload: &HirExprKind,
    attached: &AttachedExpressionNode,
) -> bool {
    let Some(expected_requirements) = expression_requirements(payload, attached.projection())
    else {
        return false;
    };
    if !rows.requirements_match(owner, &expected_requirements) {
        return false;
    }

    let Ok(expected_components) =
        expression_component_sites(rows.source, parsed, owner, payload, attached)
    else {
        return false;
    };
    if expected_components
        .keys()
        .any(|role| !expected_requirements.contains_key(role))
    {
        return false;
    }
    rows.components_match(owner, &expected_components)
        && expected_requirements.iter().all(|(role, requirement)| {
            *requirement != HirSourceRequirement::Required || expected_components.contains_key(role)
        })
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one expression-family projection owns the complete typed component-site and recovery matrix"
)]
fn expression_component_sites(
    source: &SourceDocumentIdentity,
    parsed: &ParsedSource,
    owner: ExprId,
    payload: &HirExprKind,
    attached: &AttachedExpressionNode,
) -> Result<BTreeMap<HirExprSourceRole, HirSourceSite>, HirSourceCommitInvariantError> {
    let mut sites = BTreeMap::new();
    for component in attached.components() {
        validate_component_source(source, component.source_span().source())?;
        let Some(role) = expression_component_role(attached.projection(), component.role()) else {
            continue;
        };
        let site = HirSourceSite::from_attached_span(parsed.document(), component.source_span())?;
        insert_expression_site(&mut sites, owner, role, site)?;
    }

    if matches!(attached.projection(), ExpressionProjection::Path) {
        match (attached.path(), attached.nominal_path_type()) {
            (Some(path), None) => {
                if let Some(root) = path.root_source_span() {
                    validate_component_source(source, root.source())?;
                    let site = HirSourceSite::from_attached_span(parsed.document(), &root)?;
                    insert_expression_site(&mut sites, owner, HirExprSourceRole::PathRoot, site)?;
                }
                for (ordinal, segment) in path.segments().iter().enumerate() {
                    let span = segment.source_span();
                    validate_component_source(source, span.source())?;
                    let role = HirExprSourceRole::PathSegment {
                        ordinal: u32::try_from(ordinal)
                            .expect("attached path limits keep expression ordinals within u32"),
                    };
                    let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
                    insert_expression_site(&mut sites, owner, role, site)?;
                }
                if let Some(missing) = path.missing_name() {
                    let span = missing.source_span();
                    validate_component_source(source, span.source())?;
                    let role = HirExprSourceRole::PathSegment {
                        ordinal: u32::try_from(path.segments().len())
                            .expect("attached path limits keep expression ordinals within u32"),
                    };
                    let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
                    insert_expression_site(&mut sites, owner, role, site)?;
                }
            }
            (None, Some(type_ref)) => {
                let nominal_path = type_ref.value().nominal_path().ok_or(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: SyntheticOwner::Expr(owner),
                    },
                )?;
                if let Some(root) = type_ref.component(TypeRefComponentRole::PathRoot) {
                    validate_component_source(source, root.source())?;
                    let site = HirSourceSite::from_attached_span(parsed.document(), &root)?;
                    insert_expression_site(&mut sites, owner, HirExprSourceRole::PathRoot, site)?;
                }
                for ordinal in 0..nominal_path.segments().len() {
                    let ordinal = u32::try_from(ordinal)
                        .expect("attached type path limits keep expression ordinals within u32");
                    let span = type_ref
                        .component(TypeRefComponentRole::PathSegment { ordinal })
                        .ok_or(
                            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                                owner: SyntheticOwner::Expr(owner),
                            },
                        )?;
                    validate_component_source(source, span.source())?;
                    let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
                    insert_expression_site(
                        &mut sites,
                        owner,
                        HirExprSourceRole::PathSegment { ordinal },
                        site,
                    )?;
                }
            }
            _ => {
                return Err(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: SyntheticOwner::Expr(owner),
                    },
                );
            }
        }
    }
    if let HirExprKind::DialogueContentApplication(application) = payload
        && !application.coordinates().is_empty()
    {
        let target = attached
            .children()
            .first()
            .filter(|target| target.ordinal() == 0)
            .ok_or(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            )?;
        let target = target
            .authored_semantic()
            .map_err(
                |error| HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: SyntheticOwner::Expr(owner),
                    error,
                },
            )?
            .ok_or(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            )?;
        if !matches!(target.projection(), ExpressionProjection::Call(_)) {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        }
        for coordinate in application.coordinates() {
            for (syntax_part, hir_part) in [
                (
                    SyntaxCallArgumentPart::Whole,
                    HirCallArgumentSourcePart::Whole,
                ),
                (
                    SyntaxCallArgumentPart::Name,
                    HirCallArgumentSourcePart::Name,
                ),
                (
                    SyntaxCallArgumentPart::Value,
                    HirCallArgumentSourcePart::Value,
                ),
            ] {
                let span = target
                    .component(ExpressionComponentRole::CallArgument {
                        argument: coordinate.argument().get(),
                        part: syntax_part,
                    })
                    .ok_or(
                        HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                            owner: SyntheticOwner::Expr(owner),
                        },
                    )?;
                validate_component_source(source, span.source())?;
                let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
                insert_expression_site(
                    &mut sites,
                    owner,
                    HirExprSourceRole::ConfigurationArgument {
                        argument: coordinate.argument(),
                        part: hir_part,
                    },
                    site,
                )?;
            }
        }
    }
    Ok(sites)
}

#[allow(
    clippy::result_large_err,
    reason = "candidate component projection preserves complete typed source evidence"
)]
fn candidate_dialogue_component_sites(
    source: &SourceDocumentIdentity,
    mut component_site: impl FnMut(&HirSourceQuery) -> Option<HirSourceSite>,
    parsed: &ParsedSource,
    owner: ExprId,
    attached: &AttachedExpressionNode,
    graph: AttachedCandidateGraph<'_>,
    application: &HirDialogueContentApplication,
) -> Result<BTreeMap<HirExprSourceRole, HirSourceSite>, HirSourceCommitInvariantError> {
    let mut sites = BTreeMap::new();
    for syntax_role in [
        ExpressionComponentRole::Target,
        ExpressionComponentRole::OpenBracket,
        ExpressionComponentRole::CloseBracket,
        ExpressionComponentRole::Content,
    ] {
        let span = attached.component(syntax_role).ok_or(
            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                owner: SyntheticOwner::Expr(owner),
            },
        )?;
        validate_component_source(source, span.source())?;
        let role = expression_component_role(attached.projection(), syntax_role).ok_or(
            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                owner: SyntheticOwner::Expr(owner),
            },
        )?;
        let site = HirSourceSite::from_attached_span(parsed.document(), &span)?;
        insert_expression_site(&mut sites, owner, role, site)?;
    }
    let content = sites.get(&HirExprSourceRole::Content).cloned().ok_or(
        HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
            owner: SyntheticOwner::Expr(owner),
        },
    )?;
    insert_expression_site(&mut sites, owner, HirExprSourceRole::ContentBody, content)?;

    let components = graph.dialogue_components().ok_or(
        HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
            owner: SyntheticOwner::Expr(owner),
        },
    )?;
    for component in components {
        validate_component_source(source, component.source_span().source())?;
        let role = expression_component_role(attached.projection(), component.role()).ok_or(
            HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                owner: SyntheticOwner::Expr(owner),
            },
        )?;
        if !matches!(
            role,
            HirExprSourceRole::DialogueNode { .. }
                | HirExprSourceRole::RichTextTag { .. }
                | HirExprSourceRole::RichTextArgument { .. }
        ) {
            return Err(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: SyntheticOwner::Expr(owner),
                },
            );
        }
        let site = HirSourceSite::from_attached_span(parsed.document(), component.source_span())?;
        insert_expression_site(&mut sites, owner, role, site)?;
    }

    for coordinate in application.coordinates() {
        for part in [
            HirCallArgumentSourcePart::Whole,
            HirCallArgumentSourcePart::Name,
            HirCallArgumentSourcePart::Value,
        ] {
            let source_query = HirSourceQuery::Expr {
                owner: application.target(),
                role: HirExprSourceRole::CallArgument {
                    argument: coordinate.argument(),
                    part,
                },
            };
            let Some(site) = component_site(&source_query) else {
                return Err(HirSourceCommitInvariantError::MissingRequiredComponent {
                    query: source_query,
                });
            };
            insert_expression_site(
                &mut sites,
                owner,
                HirExprSourceRole::ConfigurationArgument {
                    argument: coordinate.argument(),
                    part,
                },
                site,
            )?;
        }
    }
    Ok(sites)
}

fn candidate_dialogue_manifest_matches(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: ExprId,
    attached: &AttachedExpressionNode,
    graph: AttachedCandidateGraph<'_>,
    application: &HirDialogueContentApplication,
) -> bool {
    if index
        .syntax_owners
        .contains_key(&SyntheticOwner::Expr(owner))
    {
        return false;
    }
    let Some(content) = graph.dialogue_content() else {
        return false;
    };
    let expected_requirements = candidate_dialogue_requirements(application, content);
    let actual_requirements = index
        .requirements
        .iter()
        .filter_map(|(query, requirement)| match *query {
            HirSourceQuery::Expr {
                owner: candidate,
                role,
            } if candidate == owner => Some((role, *requirement)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    if actual_requirements != expected_requirements {
        return false;
    }
    let Ok(expected_components) = candidate_dialogue_component_sites(
        &index.source,
        |query| match index.component_presence(query) {
            Some(HirSourcePresence::Present(site)) => Some(site.clone()),
            Some(HirSourcePresence::AbsentOptional) | None => None,
        },
        parsed,
        owner,
        attached,
        graph,
        application,
    ) else {
        return false;
    };
    if expected_components
        .keys()
        .any(|role| !expected_requirements.contains_key(role))
    {
        return false;
    }
    let actual_components = index
        .components
        .iter()
        .filter_map(|(query, site)| match *query {
            HirSourceQuery::Expr {
                owner: candidate,
                role,
            } if candidate == owner => Some((role, site.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    actual_components == expected_components
        && expected_requirements.iter().all(|(role, requirement)| {
            *requirement != HirSourceRequirement::Required || expected_components.contains_key(role)
        })
}

#[allow(
    clippy::result_large_err,
    reason = "component insertion preserves the complete conflicting typed source role"
)]
fn insert_expression_site(
    sites: &mut BTreeMap<HirExprSourceRole, HirSourceSite>,
    owner: ExprId,
    role: HirExprSourceRole,
    site: HirSourceSite,
) -> Result<(), HirSourceCommitInvariantError> {
    if sites.insert(role, site).is_some() {
        Err(HirSourceCommitInvariantError::ConflictingComponent {
            query: HirSourceQuery::Expr { owner, role },
        })
    } else {
        Ok(())
    }
}

#[allow(
    clippy::match_same_arms,
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "one exhaustive mapping owns every attached expression component and its final typed source role"
)]
pub(crate) fn expression_component_role(
    projection: &ExpressionProjection,
    role: ExpressionComponentRole,
) -> Option<HirExprSourceRole> {
    match role {
        ExpressionComponentRole::Literal(part) => Some(match part {
            ExpressionLiteralPart::Body => HirExprSourceRole::LiteralBody,
            ExpressionLiteralPart::Prefix => HirExprSourceRole::LiteralPrefix,
            ExpressionLiteralPart::Suffix => HirExprSourceRole::LiteralSuffix,
            ExpressionLiteralPart::Unit => HirExprSourceRole::LiteralUnit,
        }),
        ExpressionComponentRole::EntityReference(part) => {
            Some(HirExprSourceRole::EntityReference(part.into()))
        }
        ExpressionComponentRole::LifetimeScope => Some(HirExprSourceRole::RegistryScope),
        ExpressionComponentRole::LifetimeKeySegment { ordinal } => {
            Some(HirExprSourceRole::RegistryKeySegment { ordinal })
        }
        ExpressionComponentRole::LifetimeOptionalMarker => Some(HirExprSourceRole::OptionalMarker),
        // The leading dot participates in syntax identity but is not a final
        // semantic coordinate. Its source revision is still validated above.
        ExpressionComponentRole::ShortVariantMarker => None,
        ExpressionComponentRole::ShortVariantName => Some(HirExprSourceRole::ShortVariantName),
        ExpressionComponentRole::PlaceholderMarker => Some(HirExprSourceRole::PlaceholderMarker),
        ExpressionComponentRole::Element { ordinal } => {
            Some(HirExprSourceRole::Element { ordinal })
        }
        ExpressionComponentRole::NumericElement { ordinal } => {
            Some(HirExprSourceRole::NumericElement { ordinal })
        }
        ExpressionComponentRole::NumericCommonSuffix => {
            Some(HirExprSourceRole::NumericCommonSuffix)
        }
        ExpressionComponentRole::RepeatValue => Some(HirExprSourceRole::RepeatValue),
        ExpressionComponentRole::RepeatLength => Some(HirExprSourceRole::RepeatLength),
        ExpressionComponentRole::CallCallee => Some(HirExprSourceRole::CallCallee),
        ExpressionComponentRole::CallAssociatedReceiver => {
            Some(HirExprSourceRole::CallAssociatedReceiver)
        }
        ExpressionComponentRole::CallAssociatedSeparator => {
            Some(HirExprSourceRole::CallAssociatedSeparator)
        }
        ExpressionComponentRole::CallAssociatedMember => {
            Some(HirExprSourceRole::CallAssociatedMember)
        }
        ExpressionComponentRole::CallArgumentListOpen => {
            Some(HirExprSourceRole::CallArgumentListOpen)
        }
        ExpressionComponentRole::CallArgumentListClose => {
            Some(HirExprSourceRole::CallArgumentListClose)
        }
        ExpressionComponentRole::CallArgumentListRecoveryEnd => {
            Some(HirExprSourceRole::CallArgumentListRecoveryEnd)
        }
        ExpressionComponentRole::CallArgumentListEmptyInsertion => {
            Some(HirExprSourceRole::CallArgumentListEmptyInsertion)
        }
        ExpressionComponentRole::CallArgumentSeparator { following } => {
            Some(HirExprSourceRole::CallArgumentSeparator {
                following: HirCallArgumentOrdinal::try_new(usize::from(following)).ok()?,
            })
        }
        ExpressionComponentRole::CallArgumentTrailingSeparator => {
            Some(HirExprSourceRole::CallArgumentTrailingSeparator)
        }
        ExpressionComponentRole::CallArgument { argument, part } => {
            Some(HirExprSourceRole::CallArgument {
                argument: HirCallArgumentOrdinal::try_new(usize::from(argument)).ok()?,
                part: match part {
                    SyntaxCallArgumentPart::Whole => HirCallArgumentSourcePart::Whole,
                    SyntaxCallArgumentPart::Name => HirCallArgumentSourcePart::Name,
                    SyntaxCallArgumentPart::Equals => HirCallArgumentSourcePart::Equals,
                    SyntaxCallArgumentPart::Value => HirCallArgumentSourcePart::Value,
                    SyntaxCallArgumentPart::Spread => HirCallArgumentSourcePart::Spread,
                },
            })
        }
        ExpressionComponentRole::CallTypeApplication(role) => {
            Some(HirExprSourceRole::CallTypeApplication(match role {
                SyntaxCallTypeApplicationComponentRole::Whole => {
                    HirCallTypeApplicationSourceRole::Whole
                }
                SyntaxCallTypeApplicationComponentRole::TurbofishSeparator => {
                    HirCallTypeApplicationSourceRole::TurbofishSeparator
                }
                SyntaxCallTypeApplicationComponentRole::OpenAngle => {
                    HirCallTypeApplicationSourceRole::OpenAngle
                }
                SyntaxCallTypeApplicationComponentRole::CloseAngle => {
                    HirCallTypeApplicationSourceRole::CloseAngle
                }
                SyntaxCallTypeApplicationComponentRole::RecoveryEnd => {
                    HirCallTypeApplicationSourceRole::RecoveryEnd
                }
                SyntaxCallTypeApplicationComponentRole::EmptyInsertion => {
                    HirCallTypeApplicationSourceRole::EmptyInsertion
                }
                SyntaxCallTypeApplicationComponentRole::Argument { argument, part } => {
                    HirCallTypeApplicationSourceRole::Argument {
                        argument: HirCallTypeArgumentOrdinal::try_new(usize::from(argument))
                            .ok()?,
                        part: match part {
                            SyntaxCallTypeArgumentPart::Whole => {
                                HirCallTypeArgumentSourcePart::Whole
                            }
                            SyntaxCallTypeArgumentPart::Type => HirCallTypeArgumentSourcePart::Type,
                        },
                    }
                }
                SyntaxCallTypeApplicationComponentRole::Separator { following } => {
                    HirCallTypeApplicationSourceRole::Separator {
                        following: HirCallTypeArgumentOrdinal::try_new(usize::from(following))
                            .ok()?,
                    }
                }
                SyntaxCallTypeApplicationComponentRole::TrailingSeparator => {
                    HirCallTypeApplicationSourceRole::TrailingSeparator
                }
            }))
        }
        ExpressionComponentRole::Target => Some(HirExprSourceRole::Target),
        ExpressionComponentRole::OpenBracket => Some(HirExprSourceRole::OpenBracket),
        ExpressionComponentRole::CloseBracket => Some(HirExprSourceRole::CloseBracket),
        ExpressionComponentRole::Colon => Some(HirExprSourceRole::Colon),
        ExpressionComponentRole::Content => Some(HirExprSourceRole::Content),
        ExpressionComponentRole::ContentBody => Some(HirExprSourceRole::ContentBody),
        ExpressionComponentRole::Plan => Some(HirExprSourceRole::Plan),
        ExpressionComponentRole::ConfigurationArgument { argument, part } => {
            Some(HirExprSourceRole::ConfigurationArgument {
                argument: HirCallArgumentOrdinal::try_new(usize::from(argument)).ok()?,
                part: match part {
                    SyntaxDialogueConfigurationArgumentPart::Whole => {
                        HirCallArgumentSourcePart::Whole
                    }
                    SyntaxDialogueConfigurationArgumentPart::Name => {
                        HirCallArgumentSourcePart::Name
                    }
                    SyntaxDialogueConfigurationArgumentPart::Value => {
                        HirCallArgumentSourcePart::Value
                    }
                },
            })
        }
        ExpressionComponentRole::DialogueNode { ordinal, part } => {
            Some(HirExprSourceRole::DialogueNode {
                ordinal,
                part: match part {
                    SyntaxDialogueNodeSourcePart::Whole => HirDialogueNodeSourcePart::Whole,
                    SyntaxDialogueNodeSourcePart::Text => HirDialogueNodeSourcePart::Text,
                    SyntaxDialogueNodeSourcePart::Raw => HirDialogueNodeSourcePart::Raw,
                    SyntaxDialogueNodeSourcePart::Escape => HirDialogueNodeSourcePart::Escape,
                    SyntaxDialogueNodeSourcePart::RubyBase => HirDialogueNodeSourcePart::RubyBase,
                    SyntaxDialogueNodeSourcePart::RubyText => HirDialogueNodeSourcePart::RubyText,
                    SyntaxDialogueNodeSourcePart::Interpolation => {
                        HirDialogueNodeSourcePart::Interpolation
                    }
                    SyntaxDialogueNodeSourcePart::LineBreak => HirDialogueNodeSourcePart::LineBreak,
                    SyntaxDialogueNodeSourcePart::Error => HirDialogueNodeSourcePart::Error,
                },
            })
        }
        ExpressionComponentRole::RichTextTag { tag, part } => {
            Some(HirExprSourceRole::RichTextTag {
                tag,
                part: match part {
                    SyntaxRichTextTagSourcePart::Whole => HirRichTextTagSourcePart::Whole,
                    SyntaxRichTextTagSourcePart::OpenDelimiter => {
                        HirRichTextTagSourcePart::OpenDelimiter
                    }
                    SyntaxRichTextTagSourcePart::Name => HirRichTextTagSourcePart::Name,
                    SyntaxRichTextTagSourcePart::Payload => HirRichTextTagSourcePart::Payload,
                    SyntaxRichTextTagSourcePart::CloseDelimiter => {
                        HirRichTextTagSourcePart::CloseDelimiter
                    }
                    SyntaxRichTextTagSourcePart::InferenceInsertion => {
                        HirRichTextTagSourcePart::InferenceInsertion
                    }
                    SyntaxRichTextTagSourcePart::EndTag => HirRichTextTagSourcePart::EndTag,
                    SyntaxRichTextTagSourcePart::Marker(part) => {
                        HirRichTextTagSourcePart::Marker(match part {
                            SyntaxIdRefPart::Whole => HirIdRefSourcePart::Whole,
                            SyntaxIdRefPart::AbsoluteMarker => HirIdRefSourcePart::AbsoluteMarker,
                            SyntaxIdRefPart::Family => HirIdRefSourcePart::Family,
                            SyntaxIdRefPart::FamilySeparator => HirIdRefSourcePart::FamilySeparator,
                            SyntaxIdRefPart::ParentMarker { ordinal } => {
                                HirIdRefSourcePart::ParentMarker { ordinal }
                            }
                            SyntaxIdRefPart::SuffixSegment { ordinal } => {
                                HirIdRefSourcePart::SuffixSegment { ordinal }
                            }
                        })
                    }
                },
            })
        }
        ExpressionComponentRole::RichTextArgument {
            tag,
            argument,
            part,
        } => Some(HirExprSourceRole::RichTextArgument {
            tag,
            argument,
            part: match part {
                SyntaxRichTextArgumentSourcePart::Whole => HirRichTextArgumentSourcePart::Whole,
                SyntaxRichTextArgumentSourcePart::Name => HirRichTextArgumentSourcePart::Name,
                SyntaxRichTextArgumentSourcePart::Equals => HirRichTextArgumentSourcePart::Equals,
                SyntaxRichTextArgumentSourcePart::Value => HirRichTextArgumentSourcePart::Value,
            },
        }),
        ExpressionComponentRole::SelectedMember => Some(HirExprSourceRole::SelectedMember),
        ExpressionComponentRole::Index => Some(HirExprSourceRole::Index),
        ExpressionComponentRole::LeftOperand => Some(HirExprSourceRole::LeftOperand),
        ExpressionComponentRole::RightOperand => Some(HirExprSourceRole::RightOperand),
        ExpressionComponentRole::Operand => Some(HirExprSourceRole::Operand),
        // `with` owns the Await branch attachment; its delimiter/keyword is
        // syntax identity, while each nested branch body is staged through
        // its expression-owned scope and has no duplicate scalar role here.
        ExpressionComponentRole::AwaitWith => None,
        ExpressionComponentRole::AwaitBranch { .. } => None,
        ExpressionComponentRole::RangeStart => Some(HirExprSourceRole::RangeStart),
        ExpressionComponentRole::RangeEnd => Some(HirExprSourceRole::RangeEnd),
        ExpressionComponentRole::RangeInclusiveMarker => {
            Some(HirExprSourceRole::RangeInclusiveMarker)
        }
        ExpressionComponentRole::RecordPath => Some(HirExprSourceRole::RecordPath),
        ExpressionComponentRole::RecordField { field, part } => {
            Some(HirExprSourceRole::RecordField {
                field,
                part: match part {
                    ExpressionRecordFieldPart::Whole => HirRecordFieldSourcePart::Whole,
                    ExpressionRecordFieldPart::Name => HirRecordFieldSourcePart::Name,
                    ExpressionRecordFieldPart::Colon => HirRecordFieldSourcePart::Colon,
                    ExpressionRecordFieldPart::Value => HirRecordFieldSourcePart::Value,
                },
            })
        }
        ExpressionComponentRole::ClosureParameter { parameter, part } => {
            Some(HirExprSourceRole::ClosureParameter {
                parameter: u32::from(parameter),
                part: match part {
                    SyntaxClosureParameterPart::Whole => HirClosureParameterSourcePart::Whole,
                    SyntaxClosureParameterPart::Pattern => HirClosureParameterSourcePart::Pattern,
                    SyntaxClosureParameterPart::Colon => HirClosureParameterSourcePart::Colon,
                    SyntaxClosureParameterPart::Type => HirClosureParameterSourcePart::Type,
                },
            })
        }
        ExpressionComponentRole::ClosureOpenDelimiter
        | ExpressionComponentRole::ClosureCloseDelimiter
        | ExpressionComponentRole::ClosureRecoveryEnd
        | ExpressionComponentRole::ClosureParameterSeparator { .. }
        | ExpressionComponentRole::ClosureFatArrow => None,
        ExpressionComponentRole::ReturnType => Some(HirExprSourceRole::ReturnType),
        ExpressionComponentRole::ThreadMode => Some(HirExprSourceRole::ThreadModifier),
        ExpressionComponentRole::Body => Some(HirExprSourceRole::Body),
        // Pipe source identity is exactly its left and right operands. The
        // token remains an attached-syntax component, but it is not a final
        // HIR source coordinate.
        ExpressionComponentRole::Operator
            if matches!(projection, ExpressionProjection::Pipe(_)) =>
        {
            None
        }
        ExpressionComponentRole::Operator => Some(HirExprSourceRole::Operator),
        ExpressionComponentRole::Name if matches!(projection, ExpressionProjection::Thread(_)) => {
            Some(HirExprSourceRole::ThreadName)
        }
        ExpressionComponentRole::Name => Some(HirExprSourceRole::Name),
        ExpressionComponentRole::Statement { ordinal } => {
            Some(HirExprSourceRole::Statement { ordinal })
        }
        ExpressionComponentRole::Tail => Some(HirExprSourceRole::Tail),
        ExpressionComponentRole::Condition => Some(HirExprSourceRole::Condition),
        ExpressionComponentRole::ThenBranch => Some(HirExprSourceRole::ThenBranch),
        ExpressionComponentRole::ElseBranch => Some(HirExprSourceRole::ElseBranch),
        ExpressionComponentRole::Pattern => Some(HirExprSourceRole::Pattern),
        ExpressionComponentRole::Scrutinee => Some(HirExprSourceRole::Scrutinee),
        ExpressionComponentRole::Guard => Some(HirExprSourceRole::Guard),
        ExpressionComponentRole::MatchArm { arm, part } => Some(HirExprSourceRole::MatchArm {
            arm,
            part: match part {
                SyntaxMatchArmPart::Whole => HirMatchArmSourcePart::Whole,
                SyntaxMatchArmPart::Pattern => HirMatchArmSourcePart::Pattern,
                SyntaxMatchArmPart::Guard => HirMatchArmSourcePart::Guard,
                SyntaxMatchArmPart::Arrow => HirMatchArmSourcePart::Arrow,
                SyntaxMatchArmPart::Value => HirMatchArmSourcePart::Value,
            },
        }),
        ExpressionComponentRole::Recovery => Some(HirExprSourceRole::Recovery),
    }
}
