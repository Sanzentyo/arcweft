//! Attached source-file header lowering into the final HIR item arena.

use arcweft_lang_syntax::attachment::source_file::{
    AttachedDelimiterState, AttachedPath, AttachedUseAlias, AttachedUseGroupChild, AttachedUseTree,
    AttachedVisibilityKind, SourceFileEntryNode,
};
use arcweft_lang_syntax::attachment::{
    AstNode, AttachedAttributeValue, AttachedGenericParameter, AttachedItemPrefix,
    AttachedOuterAttribute, AttachedRequiredName, AttachedWhereClause, TypedItemNode,
};
use arcweft_lang_syntax::expressions::{
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentProjection, SyntaxRequiredTokenState,
};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use crate::arena::ArenaReservation;
use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirCallArgument, HirCallChildPoison, HirCallExpr, HirCallValue, HirRequiredTokenState,
};
use crate::identity::SyntheticOwner;
use crate::identity::{HirLimit, ScopeId};
use crate::item::{
    HirAttribute, HirDeclarationMemberArena, HirDocumentation, HirErrorItem, HirGenericParameter,
    HirItem, HirItemIssue, HirItemKind, HirItemPoisonState, HirItemPrefix, HirModuleDeclaration,
    HirRequiredName, HirUseBinding, HirUseBindingKind, HirUseDeclaration, HirVisibility,
    HirWherePredicate,
};
use crate::leaf::{HirName, HirPathIssue, HirPathValue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure, HirLoweringCheckpoint};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;

use super::name_projection::recovered_name;
use super::path_projection::{TypedPathProjection, TypedPathSegment, project_attached_path};
use super::{StagedHirModuleTransaction, require_limit};

mod callable;
mod entry;
mod extern_capability;
mod flow;
mod host;
mod nominal;
mod retained;
mod style;
mod trait_impl;

struct ItemProjection<T> {
    value: T,
    issue: Option<HirItemIssue>,
}

struct LoweredItemProjection {
    item: HirItem,
    members: Option<HirDeclarationMemberArena>,
}

/// One source-backed Proof item whose accepted scope/signature prefix is
/// frozen while body statements, expressions, and tail remain unallocated
/// until semantic return facts arrive.
pub(super) struct PendingProofDeclaration {
    reservation: ArenaReservation<crate::identity::ItemId>,
    root_scope: ScopeId,
    node: TypedItemNode,
    item_site: HirSourceSite,
}

impl<T> ItemProjection<T> {
    const fn resolved(value: T) -> Self {
        Self { value, issue: None }
    }

    const fn recovered(value: T, issue: HirItemIssue) -> Self {
        Self {
            value,
            issue: Some(issue),
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    /// Lowers source-file module/import headers without publishing a second
    /// production reader. Ordinary item families remain a hard private-slice
    /// boundary until their final typed lowerers join this transaction.
    pub(crate) fn lower_parsed_source_items(
        &mut self,
        source: &ParsedSource,
    ) -> Result<ScopeId, HirLowerFailure> {
        let result = self.lower_parsed_source_items_inner(source);
        if result.is_err() {
            self.slots.poison();
        }
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the source-order transaction validates and publishes one complete module item inventory"
    )]
    fn lower_parsed_source_items_inner(
        &mut self,
        source: &ParsedSource,
    ) -> Result<ScopeId, HirLowerFailure> {
        let expected = self.request.source().snapshot_id();
        let supplied = source.snapshot_id();
        if expected.lineage().database() != supplied.lineage().database() {
            return Err(HirLowerFailure::WrongSyntaxDatabase {
                expected: expected.lineage().database(),
                actual: supplied.lineage().database(),
            });
        }
        if expected.lineage() != supplied.lineage() {
            return Err(HirLowerFailure::WrongSyntaxLineage {
                expected: expected.lineage(),
                actual: supplied.lineage(),
            });
        }
        if expected != supplied {
            return Err(HirLowerFailure::StaleSource {
                current: expected.clone(),
                supplied: supplied.clone(),
            });
        }
        let root = source.root_syntax();
        let root_span = root.source_span();
        if root_span.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: root_span.source().clone(),
            });
        }

        let entries = source
            .entries()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let item_count = entries
            .iter()
            .filter(|entry| !matches!(entry, SourceFileEntryNode::Attribute(_)))
            .count();
        preflight_source_file_inventory(item_count)?;
        self.control
            .checkpoint(HirLoweringCheckpoint::BeforeRootScopeReservation)?;

        let root_scope = {
            let module = self.snapshot_id().module();
            let scope = HirScope::try_new(
                module,
                HirScopeKind::Module,
                None,
                HirScopeOwner::Module(module),
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            let (slots, arenas) = self.storage_mut();
            arenas.scopes().allocate_source(
                slots,
                root.id(),
                HirSourceSite::Span(root_span),
                scope,
            )?
        };

        for entry in entries {
            let SourceFileEntryNode::Item(item_node) = entry else {
                // Source-file attributes are module-policy syntax inputs, not
                // fabricated HIR items.
                continue;
            };
            let item_site = HirSourceSite::Span(item_node.source_span());
            let reservation = {
                let (slots, arenas) = self.storage_mut();
                arenas
                    .items()
                    .reserve_source(slots, item_node.id(), item_site.clone())?
            };
            let owner = reservation.id();
            self.control
                .checkpoint(HirLoweringCheckpoint::ItemReserved)?;

            if let TypedItemNode::Proof(node) = &item_node
                && let Some(header) =
                    self.stage_authored_proof_return_header(owner, root_scope, node)?
            {
                self.stage_proof_return_header(header);
                self.pending_proofs.push(PendingProofDeclaration {
                    reservation,
                    root_scope,
                    node: item_node,
                    item_site,
                });
                continue;
            }

            let lowered = match &item_node {
                TypedItemNode::Module(node) => {
                    let (kind, state) = Self::lower_module_declaration(node)?;
                    LoweredItemProjection {
                        item: HirItem::try_new_with_state(
                            owner,
                            root_scope,
                            HirItemPrefix::new(None, Box::new([]), None),
                            kind,
                            Box::new([]),
                            state,
                        )
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        members: None,
                    }
                }
                TypedItemNode::Use(node) => {
                    let (visibility, kind, state) = Self::lower_use_declaration(node)?;
                    LoweredItemProjection {
                        item: HirItem::try_new_with_state(
                            owner,
                            root_scope,
                            HirItemPrefix::new(None, Box::new([]), visibility),
                            kind,
                            Box::new([]),
                            state,
                        )
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        members: None,
                    }
                }
                TypedItemNode::Character(node) => {
                    self.lower_character_declaration(owner, root_scope, node)?
                }
                TypedItemNode::View(node) => {
                    self.lower_view_declaration(owner, root_scope, node)?
                }
                TypedItemNode::TypeAlias(node) => self.lower_type_alias(owner, root_scope, node)?,
                TypedItemNode::Struct(node) => self.lower_struct(owner, root_scope, node)?,
                TypedItemNode::Enum(node) => self.lower_enum(owner, root_scope, node)?,
                TypedItemNode::Resource(node) => {
                    self.lower_resource_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Signal(node) => {
                    self.lower_signal_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Action(node) => {
                    self.lower_action_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Activity(node) => {
                    self.lower_activity_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Metric(node) => {
                    self.lower_metric_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Layer(node) => {
                    self.lower_layer_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Entry(node) => {
                    self.lower_entry_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Style(node) => {
                    self.lower_style_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Function(node) => {
                    self.lower_function_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Trait(node) => {
                    self.lower_trait_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Impl(node) => {
                    self.lower_impl_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Predicate(node) => {
                    self.lower_predicate_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Proof(node) => {
                    self.lower_proof_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Flow(node) => {
                    self.lower_flow_declaration(owner, root_scope, node)?
                }
                TypedItemNode::ExternCapability(node) => {
                    self.lower_extern_capability_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Test(node) => {
                    self.lower_test_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Bench(node) => {
                    self.lower_bench_declaration(owner, root_scope, node)?
                }
                TypedItemNode::Error(_) => LoweredItemProjection {
                    item: HirItem::try_new(
                        owner,
                        root_scope,
                        HirItemPrefix::new(None, Box::new([]), None),
                        HirItemKind::Error(HirErrorItem::new()),
                        Box::new([]),
                    )
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                    members: None,
                },
            };

            self.finalize_source_item(reservation, &item_node, item_site, lowered)?;
        }

        Ok(root_scope)
    }

    pub(super) fn resume_pending_proof_declarations(&mut self) -> Result<(), HirLowerFailure> {
        let pending = core::mem::take(&mut self.pending_proofs);
        for pending in pending {
            let TypedItemNode::Proof(node) = &pending.node else {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            };
            let owner = pending.reservation.id();
            let lowered = self.lower_proof_declaration(owner, pending.root_scope, node)?;
            self.finalize_source_item(
                pending.reservation,
                &pending.node,
                pending.item_site,
                lowered,
            )?;
        }
        Ok(())
    }

    fn finalize_source_item(
        &mut self,
        reservation: ArenaReservation<crate::identity::ItemId>,
        item_node: &TypedItemNode,
        item_site: HirSourceSite,
        lowered: LoweredItemProjection,
    ) -> Result<(), HirLowerFailure> {
        let owner = reservation.id();
        let is_recovery = lowered.item.is_poisoned();
        self.source_components.stage_attached_declaration(
            self.request.source(),
            owner,
            item_node,
            lowered.item.kind(),
        )?;
        self.source_components.stage_attached_entry(
            self.request.source(),
            owner,
            item_node,
            lowered.item.kind(),
        )?;
        self.source_components.stage_attached_callable(
            self.request.source(),
            owner,
            item_node,
            lowered.item.kind(),
        )?;
        self.source_components.stage_attached_use(
            self.request.source(),
            owner,
            item_node,
            lowered.item.kind(),
        )?;
        self.source_components.stage_attached_view(
            self.request.source(),
            owner,
            item_node,
            lowered.item.kind(),
        )?;
        if let Some(members) = lowered.members {
            self.stage_declaration_members(owner, &lowered.item, members)?;
        }
        let item = {
            let (slots, arenas) = self.storage_mut();
            arenas.items().finalize(slots, reservation, lowered.item)?
        };
        self.stage_source_ordered_item(item);
        if is_recovery {
            let owner = SyntheticOwner::Item(item);
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                owner,
                HirRecoveryPrimary::owner_whole(owner),
                item_site,
            ));
        }
        Ok(())
    }

    fn lower_module_declaration(
        node: &AstNode<arcweft_lang_syntax::attachment::node::ModuleDeclarationKind>,
    ) -> Result<(HirItemKind, HirItemPoisonState), HirLowerFailure> {
        let path = node
            .path()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let projected = project_item_path(&path)?;
        let state = item_state(projected.issue);
        Ok((
            HirItemKind::Module(HirModuleDeclaration::new(projected.value)),
            state,
        ))
    }

    fn lower_use_declaration(
        node: &AstNode<arcweft_lang_syntax::attachment::node::UseDeclarationKind>,
    ) -> Result<(Option<HirVisibility>, HirItemKind, HirItemPoisonState), HirLowerFailure> {
        let visibility = match node
            .visibility()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            None => ItemProjection::resolved(None),
            Some(visibility) => match visibility.kind() {
                AttachedVisibilityKind::Public => {
                    ItemProjection::resolved(Some(HirVisibility::Public))
                }
                AttachedVisibilityKind::Crate => {
                    ItemProjection::resolved(Some(HirVisibility::Crate))
                }
                AttachedVisibilityKind::Super => {
                    ItemProjection::resolved(Some(HirVisibility::Super))
                }
                AttachedVisibilityKind::Recovery => {
                    ItemProjection::recovered(None, HirItemIssue::MalformedHeader)
                }
            },
        };
        let tree = node
            .tree()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let bindings = project_use_tree(&tree)?;
        let has_recovery = !node
            .recoveries()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .is_empty();

        let mut issue = visibility.issue.or(bindings.issue);
        if has_recovery {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        if bindings.value.is_empty() {
            issue.get_or_insert(HirItemIssue::Recovery);
        }
        let declaration = match issue {
            Some(_) => HirUseDeclaration::recovered(bindings.value),
            None => HirUseDeclaration::try_new(bindings.value)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
        };
        Ok((
            visibility.value,
            HirItemKind::Use(declaration),
            item_state(issue),
        ))
    }

    fn lower_item_prefix(
        &mut self,
        attached: &AttachedItemPrefix,
        scope: ScopeId,
    ) -> Result<ItemProjection<HirItemPrefix>, HirLowerFailure> {
        let visibility = match attached
            .visibility()
            .map(arcweft_lang_syntax::attachment::source_file::AttachedVisibility::kind)
        {
            None => ItemProjection::resolved(None),
            Some(AttachedVisibilityKind::Public) => {
                ItemProjection::resolved(Some(HirVisibility::Public))
            }
            Some(AttachedVisibilityKind::Crate) => {
                ItemProjection::resolved(Some(HirVisibility::Crate))
            }
            Some(AttachedVisibilityKind::Super) => {
                ItemProjection::resolved(Some(HirVisibility::Super))
            }
            Some(AttachedVisibilityKind::Recovery) => {
                ItemProjection::recovered(None, HirItemIssue::MalformedHeader)
            }
        };
        let documentation = attached.documentation().map(project_documentation);
        let mut issue = None;
        let mut attributes = Vec::with_capacity(attached.attributes().len());
        for attribute in attached.attributes() {
            let projected = self.lower_item_attribute(attribute, scope)?;
            issue = issue.or(projected.issue);
            attributes.extend(projected.value);
        }
        issue = issue.or(visibility.issue);
        Ok(ItemProjection {
            value: HirItemPrefix::new(
                documentation,
                attributes.into_boxed_slice(),
                visibility.value,
            ),
            issue,
        })
    }

    fn lower_item_attribute(
        &mut self,
        attached: &AttachedOuterAttribute,
        scope: ScopeId,
    ) -> Result<ItemProjection<Option<HirAttribute>>, HirLowerFailure> {
        require_limit(HirLimit::CallArguments, attached.arguments().len())?;
        let mut recovered = attached.issue().is_some()
            || attached.recovery().is_some()
            || matches!(attached.close_state(), AttachedDelimiterState::Missing(_))
            || attached.form().terminator()
                == Some(SyntaxCallArgumentListTerminator::RecoveredMissing);
        let path = match project_attached_path(attached.path())? {
            TypedPathProjection::Resolved(path) => Some(path),
            TypedPathProjection::Recovered(_) => {
                recovered = true;
                None
            }
        };

        let mut arguments = Vec::with_capacity(attached.arguments().len());
        let mut child_states = Vec::with_capacity(attached.arguments().len());
        for source_argument in attached.arguments() {
            recovered |= source_argument.projection().has_recovery();
            let (value, child_state) = match source_argument.value() {
                AttachedAttributeValue::Authored(expression) => {
                    let value = self.lower_attached_expression(expression, scope)?;
                    let child_state = if self.staged_expression_is_poisoned(value)? {
                        recovered = true;
                        HirCallChildPoison::Poisoned
                    } else {
                        HirCallChildPoison::Clean
                    };
                    (HirCallValue::Present { value }, child_state)
                }
                AttachedAttributeValue::Missing(_) => {
                    recovered = true;
                    continue;
                }
            };
            let argument = match source_argument.projection() {
                SyntaxCallArgumentProjection::Positional { .. } => {
                    HirCallArgument::Positional { value }
                }
                SyntaxCallArgumentProjection::Named { name, equals, .. } => {
                    HirCallArgument::Named {
                        name: recovered_name(name)?,
                        equals: required_token_state(*equals),
                        value,
                    }
                }
                SyntaxCallArgumentProjection::Spread { ellipsis, .. } => HirCallArgument::Spread {
                    value,
                    ellipsis: required_token_state(*ellipsis),
                },
            };
            arguments.push(argument);
            child_states.push(child_state);
        }

        if arguments.len() == attached.arguments().len() {
            recovered |= !HirCallExpr::argument_issues(&arguments, &child_states)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                .is_empty();
        } else {
            recovered = true;
        }

        match (path, recovered) {
            (Some(path), false) => Ok(ItemProjection::resolved(Some(HirAttribute::new(
                path,
                arguments.into_boxed_slice(),
            )))),
            _ => Ok(ItemProjection::recovered(None, HirItemIssue::Recovery)),
        }
    }

    fn lower_generic_parameters(
        &mut self,
        attached: Option<&arcweft_lang_syntax::attachment::AttachedGenericParameterGroup>,
        scope: ScopeId,
    ) -> Result<(Box<[HirGenericParameter]>, bool), HirLowerFailure> {
        let Some(attached) = attached else {
            return Ok((Box::new([]), false));
        };
        let mut recovery = attached.has_recovery();
        let values = attached
            .parameters()
            .iter()
            .map(|parameter| match parameter {
                AttachedGenericParameter::Lifetime { name, .. } => {
                    let name = project_required_name(name)?;
                    recovery |= name.issue.is_some();
                    Ok(HirGenericParameter::lifetime(name.value))
                }
                AttachedGenericParameter::Type { name, bounds, .. } => {
                    let name = project_required_name(name)?;
                    recovery |= name.issue.is_some();
                    let bounds = bounds
                        .iter()
                        .map(|bound| self.lower_attached_type(bound, scope))
                        .collect::<Result<Vec<_>, _>>()?;
                    recovery |= bounds
                        .iter()
                        .copied()
                        .map(|bound| self.staged_type_is_poisoned(bound))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .any(|poisoned| poisoned);
                    Ok(HirGenericParameter::ty(
                        name.value,
                        bounds.into_boxed_slice(),
                    ))
                }
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        Ok((values, recovery))
    }

    fn lower_where_clauses(
        &mut self,
        clauses: &[AttachedWhereClause],
        scope: ScopeId,
    ) -> Result<(Box<[HirWherePredicate]>, bool), HirLowerFailure> {
        let mut recovery = false;
        let predicates = clauses
            .iter()
            .flat_map(AttachedWhereClause::predicates)
            .map(|predicate| {
                recovery |= predicate.has_recovery();
                let subject = self.lower_attached_type(predicate.subject(), scope)?;
                let bounds = predicate
                    .bounds()
                    .iter()
                    .map(|bound| self.lower_attached_type(bound, scope))
                    .collect::<Result<Vec<_>, _>>()?;
                recovery |= self.staged_type_is_poisoned(subject)?;
                recovery |= bounds
                    .iter()
                    .copied()
                    .map(|bound| self.staged_type_is_poisoned(bound))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .any(|poisoned| poisoned);
                HirWherePredicate::try_new(subject, bounds.into_boxed_slice())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit.into())
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        Ok((predicates, recovery))
    }

    fn staged_type_is_poisoned(
        &mut self,
        ty: crate::identity::TypeId,
    ) -> Result<bool, HirLowerFailure> {
        self.arenas
            .types()
            .resolve_staged(&self.slots, ty)
            .map(crate::type_ref::HirType::is_poisoned)
            .map_err(Into::into)
    }
}

const fn required_token_state(source: SyntaxRequiredTokenState) -> HirRequiredTokenState {
    match source {
        SyntaxRequiredTokenState::Present => HirRequiredTokenState::Present,
        SyntaxRequiredTokenState::Missing => HirRequiredTokenState::Missing,
        SyntaxRequiredTokenState::InvalidPresent => HirRequiredTokenState::InvalidPresent,
    }
}

fn project_required_name(
    attached: &AttachedRequiredName,
) -> Result<ItemProjection<HirRequiredName>, HirLowerFailure> {
    match attached {
        AttachedRequiredName::Resolved { value, .. } => {
            require_limit(HirLimit::NameBytes, value.as_str().len())?;
            let name = HirName::try_new(value.as_str().into())
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            Ok(ItemProjection::resolved(HirRequiredName::Resolved(name)))
        }
        AttachedRequiredName::Missing { .. } => Ok(ItemProjection::recovered(
            HirRequiredName::Missing,
            HirItemIssue::MissingName,
        )),
    }
}

fn project_documentation(
    attached: &arcweft_lang_syntax::attachment::AttachedDocumentation,
) -> HirDocumentation {
    HirDocumentation::new(attached.markdown().into())
}

fn project_use_tree(
    tree: &AttachedUseTree,
) -> Result<ItemProjection<Box<[HirUseBinding]>>, HirLowerFailure> {
    match tree {
        AttachedUseTree::Path { path, alias } => {
            let path = project_item_path(path)?;
            let alias = project_alias(alias.as_ref())?;
            let issue = path.issue.or(alias.issue);
            Ok(ItemProjection {
                value: Box::new([HirUseBinding::new(
                    path.value,
                    alias.value,
                    HirUseBindingKind::Item,
                )]),
                issue,
            })
        }
        AttachedUseTree::Glob { module, alias, .. } => {
            let path = project_item_path(module)?;
            let alias = project_alias(alias.as_ref())?;
            let issue = path.issue.or(alias.issue);
            Ok(ItemProjection {
                value: Box::new([HirUseBinding::new(
                    path.value,
                    alias.value,
                    HirUseBindingKind::Glob,
                )]),
                issue,
            })
        }
        AttachedUseTree::Group {
            module,
            children,
            close,
            ..
        } => {
            let base = project_item_path(module)?;
            let mut projected = Vec::with_capacity(children.len());
            let mut recovery = base.issue;

            for child in children {
                let AttachedUseGroupChild::Binding(binding) = child else {
                    recovery.get_or_insert(HirItemIssue::InvalidMember);
                    continue;
                };
                let path = project_group_binding_path(
                    module,
                    TypedPathSegment::from_attached_kind(
                        binding.kind(),
                        binding.name().source_text(),
                    ),
                )?;
                let alias = project_alias(binding.alias())?;
                if let Some(issue) = path.issue.or(alias.issue) {
                    recovery.get_or_insert(issue);
                }
                if binding.recovery().is_some() {
                    recovery.get_or_insert(HirItemIssue::InvalidMember);
                }
                projected.push(HirUseBinding::new(
                    path.value,
                    alias.value,
                    HirUseBindingKind::Item,
                ));
            }
            if matches!(close.delimiter_state(), AttachedDelimiterState::Missing(_)) {
                recovery.get_or_insert(HirItemIssue::Recovery);
            }
            if projected.is_empty() {
                recovery.get_or_insert(HirItemIssue::Recovery);
            }
            Ok(ItemProjection {
                value: projected.into_boxed_slice(),
                issue: recovery,
            })
        }
    }
}

fn project_item_path(path: &AttachedPath) -> Result<ItemProjection<HirPathValue>, HirLowerFailure> {
    let projection = super::path_projection::project_attached_path(path)?;
    Ok(match projection {
        TypedPathProjection::Resolved(path) => {
            ItemProjection::resolved(HirPathValue::Resolved(path))
        }
        TypedPathProjection::Recovered(recovery) => {
            let issue = if path.missing_name().is_some()
                || matches!(recovery.issue(), HirPathIssue::Empty)
            {
                HirItemIssue::MissingName
            } else {
                HirItemIssue::MalformedHeader
            };
            ItemProjection::recovered(HirPathValue::Recovered(recovery), issue)
        }
    })
}

fn project_group_binding_path(
    path: &AttachedPath,
    segment: TypedPathSegment<'_>,
) -> Result<ItemProjection<HirPathValue>, HirLowerFailure> {
    let projection = super::path_projection::project_attached_path_with_segment(path, segment)?;
    Ok(match projection {
        TypedPathProjection::Resolved(path) => {
            ItemProjection::resolved(HirPathValue::Resolved(path))
        }
        TypedPathProjection::Recovered(recovery) => {
            let issue = if path.missing_name().is_some()
                || matches!(recovery.issue(), HirPathIssue::Empty)
            {
                HirItemIssue::MissingName
            } else {
                HirItemIssue::InvalidMember
            };
            ItemProjection::recovered(HirPathValue::Recovered(recovery), issue)
        }
    })
}

fn project_alias(
    alias: Option<&AttachedUseAlias>,
) -> Result<ItemProjection<Option<HirName>>, HirLowerFailure> {
    let Some(alias) = alias else {
        return Ok(ItemProjection::resolved(None));
    };
    let name = alias.name();
    if name.kind() == SyntaxKind::MissingName {
        return Ok(ItemProjection::recovered(None, HirItemIssue::MissingName));
    }
    require_limit(HirLimit::NameBytes, name.source_text().len())?;
    Ok(match HirName::try_new(name.source_text().into()) {
        Ok(name) => ItemProjection::resolved(Some(name)),
        Err(_) => ItemProjection::recovered(None, HirItemIssue::MalformedHeader),
    })
}

const fn item_state(issue: Option<HirItemIssue>) -> HirItemPoisonState {
    match issue {
        Some(issue) => HirItemPoisonState::Poisoned(issue),
        None => HirItemPoisonState::Clean,
    }
}

fn preflight_source_file_inventory(item_count: usize) -> Result<(), HirLowerFailure> {
    require_limit(HirLimit::Items, item_count)?;
    require_limit(HirLimit::Scopes, 1)
}

#[cfg(test)]
mod tests;
